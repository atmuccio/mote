# Mote — Architecture & Engineering Conventions

Mote is a fast, serverless code-quality scanner (smells, bugs, complexity, duplication) written in
Rust. One static binary: scan a working tree, any git ref, staged changes, or a remote repo; gate
pull requests on **new code**; feed results to CI (SARIF, GitHub checks/summary) and to AI agents
(MCP server, JSONL).

This document describes how Mote is built and the conventions its code follows. It is about
internals: the Rust interfaces between the crates (the `LanguagePlugin` contract, the rule APIs of
the language plugins) are not a stable API and may change in any 0.x release.

## 1. Non-negotiables

1. **Provenance.** Rules come from general software-engineering knowledge and permissively
   licensed tools (ESLint / typescript-eslint — MIT, PMD — BSD, Error Prone — Apache-2.0).
   Borrowing an *idea or rule name* from those is fine; the implementation and the wording are our
   own. Nothing is taken from proprietary or source-available analyzers: no source code, rule
   descriptions, rule keys or output of their services. No GPL/LGPL code (Checkstyle, SpotBugs,
   etc.).
2. **Never execute code from the scanned repository.** No build tools, no scripts, no plugins from the
   repo. Configuration is pure data.
3. **The binary never touches the network** (the GitHub Action does API calls with `gh`).
4. **Deterministic output.** Same input ⇒ byte-identical output regardless of thread count. Sort
   findings by `(path, line, column, rule_id, message)`. Paths are repo-relative with `/`.
5. **Never panic on input.** Plugins must handle any bytes; the engine additionally isolates each
   file with `catch_unwind` and turns panics into `mote/internal-error` findings.
6. **Precision over recall.** A rule that cannot be sure stays silent. In Java, type-dependent rules
   only fire when the resolver returns a known type.
7. **Every rule ships with fixtures** (see §6). No fixture, no rule.
8. **Windows, Linux, macOS** all build and pass tests.

## 2. Performance targets (acceptance criteria)

| Metric | Target |
|---|---|
| Process start → first file parsed | ≤ 20 ms |
| Cold scan, 1M LOC, 8-core laptop | ≤ 5 s |
| Cold scan, 1M LOC, 2-vCPU CI runner | ≤ 25 s |
| Warm PR scan (≤ 50 changed files) | ≤ 2 s |
| Single-file rescan (MCP) | ≤ 50 ms |

Parser measurements taken before the engine was written back these targets (tree-sitter-java
≈ 900k lines/s on 12 threads; Oxc ≈ 12.6M lines/s of TypeScript); current end-to-end numbers are in
[benchmark.md](benchmark.md#results). Consequences for code:

* **One traversal per file.** Rules subscribe to node kinds; the plugin walks the tree once and
  dispatches. No rule may do its own full-tree walk unless it is file-level and cheap.
* **Resolve configuration once per configuration, not per file.** The engine resolves the
  effective rules of a file (profile, `rules`, `test_rules`, matching `overrides`, Java release)
  into a `RuleSet` and hands the *same* instance to every file with the same effective
  configuration. A plugin derives what it needs from it — the enabled rules in its own rule order
  (`RuleSet::index` → `RuleIndex`, looked up by position), their parameters, dispatch tables
  restricted to the enabled rules — **once per set**, with `RuleSet::memo(key, build)` (`key`: one
  `MemoKey::unique()` per plugin instance; a set keeps the value of the first key that asks, other
  callers get a fresh, uncached value). Per file there is then no rule-id string lookup, and a
  disabled rule costs nothing during the walk. `build` must depend on the set alone (and on the
  plugin's fixed rule list), never on a file.
* Avoid per-node allocation; borrow `&str` from the source; build `String`s only when emitting.
* Rule-local precomputation (e.g. collecting identifiers) must be linear in file size.
* Parallelism lives in the engine (rayon over files). Plugins are single-threaded per file and must be
  `Send + Sync` with no global mutable state (thread-locals for parser reuse are fine).

## 3. Crates

```
mote-core        model + plugin contract + fixture harness      (small)
  ├─ mote-lang-java   Java plugin (tree-sitter-java)
  ├─ mote-lang-js     JavaScript/TypeScript plugin (Oxc)
  └─ mote-engine      config, registry, generic rules, lexer/CPD, suppression,
                      sources (worktree/rev/staged/git), cache, scan, new-code, gate, coverage
       └─ mote-report text / JSONL / SARIF / Markdown / GitHub-checks / GitLab /
                      Checkstyle / JUnit / HTML writers
            └─ mote-cli  `mote` binary: CLI + MCP server (stdio JSON-RPC)
```

* Dependencies point downward only. Plugins depend **only** on `mote-core`.
* `mote-core` changes rarely, and then additively where possible. Its plugin contract is still an
  internal interface, not a stable API.
* Languages are compile-time plugins behind cargo features in `mote-cli` (`java`, `js`), the
  Ruff/Oxlint pattern. Adding a language = new crate implementing `LanguagePlugin` + one line in the
  CLI registry.

## 4. Data flow

```
sources ─► discovery/filter ─► cache lookup ─► MAP (parallel, per file):
                                               plugin.analyze → FileAnalysis
                                               engine: lexer (NCLOC, CPD tokens), generic rules,
                                               suppressions, fingerprints → FileResult (cached)
         ─► REDUCE: CPD matching across files, measures (effort, main/test split,
                    per-directory rollups), grades
         ─► TRIAGE overlay (optional): .mote-triage.yaml entries mark findings accepted /
                    false-positive (never invalidates the cache); stale entries reported
         ─► NEW CODE (optional): diff vs base, analyze base blobs of changed files only,
                                 matching (same code, fingerprints, moved code)
                                 → new / unchanged / worsened;
                                 or a baseline results file (--baseline) instead of a git base
         ─► GATE ─► reporters (text, JSON/JSONL, SARIF, Markdown, GitHub Checks, GitLab Code
                    Quality, Checkstyle, JUnit, HTML, Bitbucket Code Insights, summary JSON)
```

Hard rule: **reduce never re-parses source.** Everything cross-file uses compact facts from the map
phase. Because analysis is file-local, an unchanged blob has identical findings in base and head.

## 5. Rules

* **Id:** `<language>/<kebab-name>`, e.g. `java/empty-catch-block`, `js/no-self-compare`. Names may
  follow ESLint/PMD naming where a well-known name exists.
* **Severity guide:**
  * `HIGH` — almost certainly a bug or a serious reliability hazard (self-assignment, `return` in
    `finally`, `equals` without `hashCode`, string compared by reference).
  * `MEDIUM` — likely problem or significant maintainability cost (empty catch, unused private field,
    cognitive complexity over threshold).
  * `LOW` — minor / stylistic (naming, redundant code, TODO comments).
* **Quality:** `reliability` for bugs, `maintainability` for smells.
* **Default gate** fails a PR only on *new* `HIGH` findings; keep `HIGH` honest.
* **Messages:** one sentence, imperative, specific, no trailing period is fine either way but be
  consistent: `Remove this empty catch block or explain why the exception is ignored`. Include the
  offending name where useful. Numbers (e.g. complexity) go in `value` as well as the message.
* **Location:** report the smallest meaningful range (the name, the operator, the keyword), not
  whole blocks. Set `symbol` to the enclosing `Class#method` (Java) / `function name` (JS) — plugins
  provide a helper.
* **Description** (rule metadata) is Markdown in our own words: what, why, how to fix, a short
  noncompliant and compliant example.
* **Tests count as code too**, but plugins may relax rules for `is_test` files where the rule makes
  no sense in tests (e.g. `System.out` in tests is fine).

## 6. Fixtures (the accuracy benchmark)

Format is defined in `crates/mote-core/src/testing.rs`. Summary:

```java
class Demo {
    void a() {
        try { run(); } catch (Exception e) { }   // expect: empty-catch-block
        try { run(); } catch (Exception e) { log(e); }   // compliant: no finding allowed
    }
}
```

* One or more files per rule in `crates/<plugin>/tests/fixtures/`, named `<rule-name>[.<variant>].<ext>`.
* `expect: <rule>` (this line) / `expect-next-line: <rule>`; every unmarked line must be clean.
* Cover positives **and** tricky negatives (the false-positive traps you can think of).
* `cargo test -p <plugin>` runs all fixtures (`assert_fixtures`) and runs every rule on every fixture
  (`assert_all_rules_robust`). `MOTE_FIXTURE=<rule-name>` filters while iterating.

## 7. Build & test

```
cargo build -p <crate>
cargo test  -p <crate>
cargo clippy -p <crate> --all-targets -- -D warnings
cargo fmt --all
```

The toolchain is pinned in `rust-toolchain.toml` (CI installs the same version), so new stable
releases can't change lints underneath us; bump it deliberately together with the workflows. The
minimum supported Rust version is `rust-version` in `Cargo.toml`.
