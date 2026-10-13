# mote-engine

The scan engine behind the `mote` binary: configuration, plugin registry, engine-implemented
rules, lexer and duplication detection, suppressions, file sources (working tree, directory, git
revision, index, remote), the result cache, new-code classification, coverage ingestion and the
quality gate. Language plugins (`mote-lang-*`) implement `mote_core::LanguagePlugin`;
reporters and the CLI consume [`ScanResult`](src/result.rs).

> This crate is part of Mote's implementation, not a published library: it is not on crates.io,
> and its Rust API (like the plugin contract in `mote-core`) may change in any 0.x release. The
> stable surfaces for users are the `mote` command line, `.mote.yaml` and the results formats,
> and even those may still change before 1.0 (see the [changelog](../../CHANGELOG.md)).

## Quick start

```rust
use std::sync::Arc;
use mote_core::LanguagePlugin;
use mote_engine::{scan::discover_config, Registry, ScanOptions, Scanner, SourceSpec};

let plugins: Vec<Box<dyn LanguagePlugin>> = vec![
    Box::new(mote_lang_java::JavaPlugin::default()),
    Box::new(mote_lang_js::JsPlugin::default()),
];
let registry = Arc::new(Registry::new(plugins)?);
let root = std::path::Path::new(".");
let (config, _config_path) = discover_config(root, None)?;      // .mote.yaml / .mote.yml
let scanner = Scanner::new(registry, config, root)?;             // validates the config
let result = scanner.scan(&ScanOptions {
    source: SourceSpec::Auto,                                     // git worktree, else plain dir
    base: Some("origin/main".into()),                             // new-code comparison
    ..Default::default()
})?;
println!("gate passed: {}", result.gate.passed);
```

## Entry points

| Item | Purpose |
|---|---|
| `Registry::new(Vec<Box<dyn LanguagePlugin>>)` | Maps extensions to plugins and builds the rule catalog: plugin rules + generic rules per language (`java/cognitive-complexity`, …) + `mote/*` meta rules. Errors on duplicate plugin ids, extensions or rule ids. `registry.catalog()`, `rule(id)`, `rule_info(id)` (serializable `RuleInfo`). |
| `Config::discover(root)`, `Config::load(path)`, `Config::from_yaml_str` | Parse `.mote.yaml` (`deny_unknown_fields`). A file that does not parse gives every unknown key and invalid value at once, at its line and column (`ConfigError::problems`), and `ConfigError::complete(&registry)` adds the validation problems of the rest. `Config::json_schema()` returns the JSON Schema. |
| `scan::discover_config(root, rev)` | Same, but reads the config from a git revision when scanning `--rev` (config matches the scanned code). |
| `Config::validate(&registry)` | All problems at once, in file order with line and column when the config was parsed from text: unknown rule ids (with "did you mean"; other tools' rule names explained by `Registry::with_foreign_rule_names`, which the CLI fills with the `mote migrate` tables), unknown/mistyped params, bad globs, unmet `mote:` version requirement. `Scanner::new` calls it. |
| `Scanner::new(Arc<Registry>, Config, root)` | One scanner per repository/config. Keeps the cache open between scans (cheap to keep alive in the MCP server). |
| `Scanner::scan(&ScanOptions)` | Full scan. |
| `Scanner::rescan(&prev, &changed, &ScanOptions)` | Update a previous result of the same scanner after `changed` files (absolute or root-relative) were edited, created or deleted: only those are read and analyzed; every other file's map result comes from the cache by the key recorded in `prev` (`ScanResult::retained`, not serialized), and duplication, suppressions, measures and the gate are recomputed across the whole set — equal to a full scan of the same files, without discovery. Requested paths are normalized (`.`/`..`, on-disk case on case-insensitive file systems); a path outside the root is an error; files a scan would not see (gitignored, below a nested repository or a link) are dropped with a warning. No new-code classification (use `scan` with a base). The MCP `verify` tool uses it. |
| `Scanner::analyze_paths(&[String], &ScanOptions)` | Analyze specific files from disk (absolute or repo-relative), cache-backed; duplication among those files only. For editors. Same path rules as `rescan`: a path outside the root is an error, and exactly the files a full scan would include are analyzed (excluded ones are skipped as in a scan; gitignored ones get a warning). |
| `Scanner::fix_files(&[String], &FixOptions, &ScanOptions)` | Apply the fixes of the given files' findings in memory (`mote fix`, the MCP `fixes` tool): unsuppressed, not triaged, of the selected rules; every non-conflicting fix, then the file is analyzed again, at most 10 passes; all of a file's changes are discarded when they add a syntax error. Only UTF-8 files (byte-order mark kept; inserted line breaks follow the file's). Returns the original and fixed bytes per file and writes nothing (see `fix`). |
| `gate::apply_coverage(&mut ScanResult, &RepoCoverage)`, `gate::reevaluate_gate(&mut ScanResult, &Config)` | Add coverage to a (saved) result and re-evaluate its gate — the functions a scan itself uses, so `mote gate --coverage` cannot drift from `scan`. `coverage::load_reports` / `map_reports` parse, merge and map reports with warnings. |
| `Scanner::effective_rules(path)` | Language, test status, exclusion, effective rules and (Java) release with its origin of one file (`mote config explain`). |
| `git::clone_remote(url, rev, cache_dir)` | Shallow `--no-checkout` clone (or refresh) under `cache_dir/repos/`; then scan with `SourceSpec::Rev(checkout.commit)` and `discover_config(&checkout.dir, Some(&checkout.commit))`. The only engine call that lets git touch the network. |

### `ScanOptions`

| Field | Meaning |
|---|---|
| `source` | `Auto` (default), `Worktree`, `Directory`, `Rev(String)`, `Staged` |
| `paths` | Filters: globs (contain `*?[{`) or path prefixes, relative to the root (an absolute path below the root is accepted); empty = everything. A filter that matches no file is an error (`EngineError::Invalid`) |
| `base` | Branch/tag/commit to compare with (`merge-base(base, head)` is used) |
| `cache`, `cache_dir` | Override `cache.enabled` / cache directory (precedence: option → `MOTE_CACHE_DIR` → `cache.dir` → `<root>/.mote/cache`) |
| `threads` | Worker threads (default: one per core). Analysis always runs on a dedicated pool with 256 MB stacks (`scan::ANALYSIS_STACK_SIZE`) |
| `coverage_reports` | Extra LCOV / JaCoCo / Cobertura files |
| `changed_only` | Analyze/report only files changed vs `base` (needs `base`); duplication is then detected among the changed files only |
| `baseline` | `Arc<baseline::Baseline>` from a saved result (`Baseline::from_result`): compare with it instead of a git base (conflicts with `base`). Findings are paired without a diff (`matching`: path + fingerprint, then fingerprint across a rename, then measured findings of the same symbol); unmatched ones are `new`, and the result's `new_code` carries `baseline` with no line diff, so line-based gate conditions and new-code grades are skipped |
| `triage_file` | Triage file to read instead of `triage.file` (from disk, relative to the process's directory) |

### `ScanResult` (serde)

`format_version` (see docs/cli.md, "Results format"), `engine_version`, `source` (kind/rev/commit, and `prefix`: the root's path below the repository top level, e.g. `backend/`), `findings` (sorted by path, line, column, rule,
message, fingerprint; suppressed findings included with `suppressed` set), `files`
(`FileSummary`: status `analyzed|cached|skipped` + `skip_reason`, `is_test`, measures, change
kind, estimated effort), `duplications` (blocks with all occurrences), `measures` (total, per
language, main and test code, per-directory rollups, coverage; with estimated effort),
`grades` (maintainability and reliability, A–E, overall and on new code; see docs/cli.md),
`new_code` (merge base, per-file new line ranges, renames, new-code measures incl. new/fixed/
worsened counts by severity, new duplication %, new coverage %), `gate` (`passed` + conditions
with actual/threshold/skipped_reason), `rules` (`RuleInfo` for every rule enabled on a scanned
file — use for SARIF `tool.driver.rules`), `diagnostics` (warnings), `triage` (the triage file,
its entry count, how many matched, and the stale entries' keys; `null` without a triage file),
`stats`.
`retained` (not serialized) lets `Scanner::rescan` update the result incrementally.

Everything except `stats` (timings, cache counters) is deterministic across runs and thread
counts; leave `stats` out of outputs that must be byte-identical.

Finding status: `unknown` without a base; otherwise `new`, `unchanged` or `worsened` (map to SARIF
`baselineState` `new` / `unchanged` / `updated`). Gate-relevant findings are
`mote_engine::gate::gating(f)`: not suppressed, not incubating, not `mote/internal-error`.

## Configuration (`.mote.yaml`)

```yaml
mote: ">=0.1"                # semver requirement on the engine version
extends: default             # default | conventions | all | none
exclude: ["**/generated/**"] # repo-relative globs; a pattern without "/" matches at any depth
tests: ["**/it/**"]          # added to each plugin's default test globs
max_file_size: 1048576
skip_generated: true         # explicit markers in the leading comments (@generated, "Code generated … DO NOT EDIT", "GENERATED CODE -- DO NOT EDIT", "This file was generated"), Yarn Plug'n'Play files (.yarn/**, .pnp.cjs, .pnp.js, .pnp.loader.mjs; by path, before reading), minified files, and vendored JavaScript (library banners that do not name the project's own package.json `name`, bundler output; `vendored.rs`, `packages.rs`)
skip_generated_except: ["web/static/app.bundle.js"]   # analyzed even when they look generated, minified or vendored; `exclude` still wins
rules:                       # exact ids or globs ("java/*")
  java/empty-catch-block: off                 # off | on | LOW | MEDIUM | HIGH (true/false accepted)
  js/no-console: { severity: HIGH }
  java/cognitive-complexity: { params: { threshold: 20 } }
  "js/*": { enabled: false }
test_rules: { java/magic-number: off }
overrides:
  - paths: ["legacy/**"]
    rules: { java/cognitive-complexity: { params: { threshold: 30 } } }
gate: { fail_on: HIGH, new_duplication_pct: 3.0, new_coverage_pct: 80.0, min_new_lines: 20, worsened: annotate,
        maintainability_grade: null, reliability_grade: null, new_maintainability_grade: null, new_reliability_grade: null }
cpd: { enabled: true, min_tokens: 100, min_lines: 10, exclude: [], ignore_literals: true, include_tests: false }
coverage: { reports: ["**/jacoco.xml", "coverage/lcov.info"] }
suppressions: { require_reason: true, compat: false }   # compat: true | false | [eslint, nosonar, pmd, checkstyle, suppress-warnings-all, spotbugs, intellij]
triage: { file: .mote-triage.yaml }   # triage decisions (accepted findings, false positives); missing file = none
cache: { enabled: true, dir: null, max_size_mb: 1024 }
java: { release: 17 }        # Java language release; unset: detected per file (below)
metrics: { depth: 2 }        # directory levels with rolled-up measures in the results (0 = none)
grades: { maintainability: [1, 2, 3, 4], reliability: [0.5, 1, 2, 4] }   # upper limits of grades A-D
```

Profiles: `default` (rules enabled by default), `conventions` (`default` plus every rule tagged
`conventions`, an opt-in style pass), `all` (every non-deprecated rule), `none`.

Precedence: profile → `rules` → `test_rules` (test files) → matching `overrides` in order. Within a
layer, glob keys apply first (less specific first), exact ids last. A map value without `enabled`
enables an exact id but only adjusts rules matched by a glob; globs never enable deprecated rules.
Incubating rules report but never gate.

### Java release

Some Java rules depend on the language release the code is compiled for (a rule suggesting
Java 16 syntax stays silent on Java 11 code). `java.release` sets it for every file. When unset,
the engine reads it for each Java file from the nearest build file up the directory tree
(`java_release.rs`): `pom.xml`, `build.gradle.kts`, `build.gradle` in the file's directory,
then its parent, up to the scan root; a build file that declares nothing passes the search on
upward (a Maven module inheriting from `../pom.xml`, a Gradle subproject configured by the root
project). Build files are data: poms are parsed as XML, Gradle scripts scanned as text; nothing is
run or downloaded.

* Maven, first declared wins: the `maven-compiler-plugin` `<release>`, `maven.compiler.release`,
  the plugin's `<source>`, `maven.compiler.source`, `java.version`, the plugin's `<target>`,
  `maven.compiler.target` (`<build><plugins>`, then `<pluginManagement>`; executions and profiles
  ignored). `${name}` is resolved from the pom's own `<properties>`.
* Gradle (Groovy and Kotlin DSL): `options.release = N` / `.set(N)`, `sourceCompatibility`, a
  toolchain `languageVersion = JavaLanguageVersion.of(N)`, `jvmToolchain(N)`,
  `targetCompatibility`; values `17`, `'17'`, `"1.8"`, `JavaVersion.VERSION_17`,
  `JavaVersion.VERSION_1_8`, `JavaVersion.toVersion(..)`. Variables and `apply from:` scripts are
  not followed. Several assignments of one setting: the lowest value.

`1.8` means `8`. Unknown stays unknown. The engine passes the release to the Java plugin as the
reserved rule parameter `java_release` of every enabled Java rule (`JAVA_RELEASE_PARAM`; the
configuration cannot set it per rule), so it is part of the rule-set hash and the cache key:
editing a build file's release re-analyzes the files it covers. Scans of a revision or the index
read that revision's build files; `analyze_paths`, `rescan` and `config explain` read them from
disk. `Scanner::effective_rules` reports the release and its origin (`EffectiveRules::java_release`).

## Engine rules

Generic (per plugin language `L`): `L/cognitive-complexity` (threshold 15), `L/cyclomatic-complexity`
(10, off by default), `L/function-too-long` (80 lines of code), `L/too-many-parameters` (7),
`L/deep-nesting` (4), `L/file-too-long` (1000 lines of code), `L/todo-comment`, `L/fixme-comment`
(FIXME/XXX/HACK), `L/commented-out-code`, `L/duplicated-block`. A plugin may implement any of these
ids itself; the engine then skips its own version.

Meta: `mote/unused-suppression`, `mote/blanket-suppression`,
`mote/suppression-without-reason`, `mote/parse-error`, `mote/internal-error`.

Suppressions: `// mote-ignore[rule-a, java/rule-b]: reason` (own line if trailing, else the
next code line), `// mote-ignore: reason` (blanket), `// mote-ignore-file[rule]: reason`.
`suppressions.compat` selects other tools' suppressions by source (`suppress::COMPAT_SOURCES`,
held as a `CompatSet` in `AnalysisSettings` and hashed into the cache key): `nosonar` honors
`NOSONAR` comments, any other source the plugin suppressions of kind `compat:<source>`
(`compat:pmd`, `compat:eslint`, …; the untagged kind `compat` applies when any source is
selected). `true` selects `COMPAT_TRUE` (eslint, nosonar, pmd, checkstyle,
suppress-warnings-all); unknown names are configuration errors. They are reported with the kind
`compat` (`nosonar`). Plugin suppressions of other kinds (e.g. `annotation`) always apply.

Triage (`triage.rs`): the triage file's entries (`fingerprint` or `id`, `rule`, `path`, `status:
accepted | false-positive`, `reason`, optional `by`, `date`) are applied after the reduce phase —
an overlay on the cached per-file results, so editing the file never invalidates the cache. A
matched finding gets `suppressed: { kind: <status>, justification: <reason> }` unless the source
already suppresses it. Entries match by fingerprint at their path, then (when that file is gone)
in a renamed or moved file; `id` entries match exactly. Entries that match nothing although the
scan analyzed their file (or, in a full scan, their file is gone) are stale:
`mote/unused-suppression` findings on the triage file and `ScanResult::triage.stale`. A
revision scan reads the triage file of that revision. Base findings whose fingerprint an entry
names are not counted as fixed.

## Internals worth knowing

* **Cache key** = xxh3-128(cache format version, engine version, plugin id + version, effective
  rule-set hash, git blob id, test flag, analysis-settings hash, **path**). The path is included
  because plugins receive it and may use it. The analysis-settings hash is the file's: computed
  for whether the generated/minified/vendored checks apply to it (`skip_generated`, minus
  `skip_generated_except`) and, for JavaScript files they apply to, with the names of the file's
  npm packages (nearest and root `package.json`), so renaming a package re-analyzes its files
  and editing `skip_generated_except` only the files it adds or removes. Values are
  postcard-encoded per-file outcomes;
  skipped generated/minified/vendored outcomes are cached too; internal errors are not. One `redb` file,
  read in parallel from one snapshot, written in one transaction; above `max_size_mb`, entries
  not used by the current scan are deleted and the file is compacted. Any cache failure (lock
  held by another process, corruption) degrades to "no cache" plus a diagnostic.
* **Worktree discovery without git processes** (`worktree.rs`): the repository is located by
  walking up to `.git` (gitdir files of linked worktrees and submodules are followed), the index
  (versions 2–4) is parsed in-process, `HEAD` is resolved from the ref files, and one parallel
  walk of the tree (honoring `.gitignore` files, `info/exclude` and the global excludes file)
  finds untracked files and stat data. A tracked file whose size and mtime match its index entry,
  and whose entry is not racily clean (mtime not older than the index file, to the second), keeps
  the index blob id; every other file is hashed with git's blob SHA-1 — at worst a cache miss.
  Split/sparse indexes, SHA-256 or reftable repositories, `core.worktree`, config includes,
  `GIT_DIR`-style variables, a bad checksum, or `MOTE_DISCOVERY=git` fall back to the git
  CLI (`ls-files -s`, `ls-files -o --exclude-standard`, `diff-files`, run concurrently). Rev and
  staged sources read blobs through one long-lived `git cat-file --batch`.
* **Cache values**: CPD tokens are stored as a per-file dictionary of distinct hashes (most
  frequent first) plus one LEB128 index per token, lines as `(line delta, token count)` runs, and
  the winnowed fingerprints for `cpd.min_tokens` (so warm scans do not re-winnow): ~1.7 bytes
  per token instead of ~6. Files CPD ignores (tests unless `include_tests`, `cpd.exclude`) store
  no tokens. Entries are inserted in key order (keeps the redb file ~1.6× the payload instead
  of ~3×), and a read snapshot loads the whole file with one sequential read (up to 256 MB):
  redb reads pages one positioned read at a time, and those serialize on a Windows file handle.
* **Fingerprint** = xxh3-128(rule, symbol, whitespace-collapsed start-line text, occurrence index);
  duplicated blocks use the duplicated token sequence's hash instead of the line text. `id` is the
  first 12 hex chars of xxh3(path, fingerprint).
* **CPD**: k=25 rolling hash, winnowing with w = min_tokens − 25 + 1, sort-based hash join
  (buckets > 64 ignored as boilerplate), maximal extension, merge by content. Overlapping matches in
  one file produce one `duplicated-block` finding.
* **New code**: changed files' base blobs are analyzed with the head path's effective config (also
  cached by blob id), then matched (`newcode::classify`): the same untouched code (an untouched
  head line maps to its base line: the k-th untouched line on both sides; same rule and message
  with digits ignored) → exact fingerprint → (rule, line text, message) on untouched lines →
  (rule, symbol, message) when plausibly the same code. Then one pass over all changed and
  deleted files pairs the remaining new findings on added lines with gone base findings on
  removed lines by (rule, message, line text, context hash of the line and its two neighbors):
  moved code (`newcode::pair_moved`; the context hash is cached with the line anchors).
  Unmatched → `new`; matched with a larger `value` → `worsened`. Duplicated-block findings are
  `new` when they overlap new lines. Pure renames are `unchanged` without re-analysis.

## Benchmark

`cargo run --release -p mote-engine --example bench -- <repo> [threads]` scans with a trivial
Java plugin (no parsing) to measure engine overhead: discovery, lexer, generic rules, CPD, cache.

## Known limitations

* A scan root inside a repository subdirectory is supported (paths are relative to the root), but
  the config is read from the scan root.
* Worktree files are read from disk even when their blob id comes from the index; with
  `core.autocrlf` the analyzed text may have CRLF line endings while a rev scan of the same blob has
  LF. Findings are the same; the cache entry is shared.
* `changed_only` limits duplication detection to the changed files.
* The in-process discovery reads ignore rules with the `ignore` crate: the global excludes file
  is found through `~/.gitconfig` / `/git/config` (not git's system config), and
  `core.ignorecase` is honored for `.gitignore` files. Racily clean or skip-worktree files are
  hashed rather than trusted. Like git, the stat check cannot see a content change that keeps the
  size and mtime (`cp -p`, `rsync -a` of an equal-size file); git additionally compares ctime and
  inode, which are unreliable on Windows and not compared. Skip-worktree (sparse checkout) files
  absent from disk are dropped, where the git-CLI path could still report cached findings.
* `rescan` reads changed files from disk even when the previous scan was of a revision, and does
  not recompute new code. Unchanged files keep their previous result even when a build file that
  sets their Java release changed since: run a full scan after editing one.
* The Java release is only read from `pom.xml` / `build.gradle(.kts)` themselves: a version set
  in an external parent pom, `gradle.properties`, a version catalog, a convention plugin or an
  `apply from:` script is unknown (set `java.release`).
* Unused-suppression detection covers `mote-ignore` comments only (not plugin annotations or
  `NOSONAR`).
