# Changelog

All notable changes to Mote are listed in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Mote follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is 0.x, a minor
release (0.1 → 0.2) may change the configuration file or the results formats (a breaking change
to the results JSON bumps its `format_version`); such changes are listed under **Changed** or
**Removed** with what to do.

## [Unreleased]

## [0.2.0] - 2026-10-13

### Added

- **Autofix:** `mote fix` applies the mechanical, behaviour-preserving fixes some rules attach to
  their findings. By default it prints a unified diff and changes nothing (exit 1 when there is
  something to fix, so it doubles as a CI check); `--write` applies it. It works on the working
  tree only, with the scan's configuration, excludes, generated-file skipping, suppressions,
  triage decisions and enabled rules (`--rule` narrows it), applies fixes in passes until none is
  left, keeps encodings, byte-order marks and CRLF line ends, and discards a file's changes when
  they would add a syntax error. 17 rules have fixes: `java/unused-import`,
  `java/empty-statement`, `java/modifier-order`, `java/long-literal-lowercase`,
  `java/array-brackets-on-name`, `java/redundant-default-constructor`, `java/missing-override`,
  `java/redundant-annotation-value`, `java/unnecessary-return`, `js/no-unused-vars` (imports),
  `js/no-useless-escape`, `js/prefer-const`, `js/no-extra-boolean-cast`, `js/no-useless-return`,
  `js/prefer-as-const`, `js/no-extra-non-null-assertion` and
  `js/no-unnecessary-type-constraint`; each rule's description says what its fix does and which
  cases it leaves alone. The results JSON gives such findings a `fix` (edits as byte offsets and
  line/column ranges) and each rule `fixable`, both additions within `format_version` 1; SARIF
  results carry `fixes`; the text output marks fixable findings; `mote rules` JSON has `fixable`
  and `rules explain` says "Fix available"; the MCP server's findings carry their fix, and a new
  read-only `fixes` tool returns the edits and the diff per file. See
  [docs/cli.md](docs/cli.md#mote-fix). `mote fix` is new in this release: review the diff before
  the first `--write` on a codebase. Analysis caches are invalidated once (engine cache
  version 11, Java plugin version 7, JavaScript plugin version 5).

- **Measured precision per rule:** `mote rules explain` prints each rule's precision on labeled
  findings in public open-source repositories (`Measured precision: 97% (40 of 41 labeled
  findings correct, measured 2026-10-13 on 34 open-source repositories)`), or that too few of
  its findings are labeled yet, or that it had none there. `docs/rules.md` has the line for
  every rule and an introduction to the method and its limits. `mote rules --format json`,
  `mote rules explain --format json` and the MCP `rule` tool add a `precision` object per rule
  (an additive change; see [docs/cli.md](docs/cli.md#mote-rules)). Of the 552 rules, 112 have a
  measured precision (all stable ones at 95% or more), 306 have findings but too few labeled ones
  for a percentage, and 134 had no findings.

### Changed

- **Java grammar:** the Java plugin now parses with tree-sitter-java-orchard 0.5.22, a maintained
  fork of tree-sitter-java 0.23.5, and reads Java 22–25 syntax: module imports
  (`import module java.base;`), statements before `super(...)` / `this(...)` in constructors,
  `case A _, B _` labels, qualified record patterns (`case Outer.Point(int x, int y)`) and
  numeric literals with repeated underscores (`1__000`). Such files no longer get a
  `mote/parse-error` finding. Module imports are never reported by `java/unused-import` or
  `java/wildcard-import`. Findings on existing code are unchanged apart from files that
  failed to parse before and a few old misparses (for example `case X -> a = b` read as a
  lambda). Java analysis caches are invalidated once (plugin version 5).

### Fixed

- **Java 23 Markdown documentation comments:** a run of `///` lines before a declaration
  (JEP 467) now counts as its documentation comment. `java/missing-javadoc` no longer reports
  declarations documented that way, `java/unused-import` counts their links (`[Foo]`,
  `[Foo#bar(Baz)]`, `[text][Foo]`) and tags as uses, and `java/collection-return-null` reads
  their `@return … null`; these accept `///` documentation whatever release the code is
  compiled for, since that depends on the JDK running javadoc (a project compiled for Java 17
  may be documented with `///`). `java/dangling-doc-comment` (after annotations, nothing after
  it in a type body), `java/javadoc-empty-tag` and `java/deprecated-annotation-missing` read
  `///` comments only on a release known to be 23 or later, as older code also writes `///`
  notes. `////` lines, `///` after code, and jbang's `///usr/bin/env` shebang and directives
  (`///DEPS …`) are never documentation. Findings on code without `///` comments are
  unchanged; Java analysis caches are invalidated once (plugin version 6).

## [0.1.0] - 2026-10-11

The first public release of Mote. It is a single binary for Windows, Linux and macOS with no
server, database or runtime to install; the binary makes no network connections except the `git`
clone that `--remote` asks for.

### Added

- **Languages:** Java (tree-sitter-java) and JavaScript/TypeScript, including JSX, TSX and React
  hooks (Oxc). The Java release is read per file from `pom.xml` / `build.gradle(.kts)` as data,
  or set with `java.release`.
- **552 rules:** 326 for Java, 221 for JavaScript/TypeScript and 5 for suppressions and analysis
  errors; 393 stable and 159 incubating (incubating rules report but never fail the gate); 485 on
  by default. Severities `HIGH` (almost certainly a bug), `MEDIUM` and `LOW`. Complexity
  (cognitive and cyclomatic), function and file length, nesting, parameter counts, TODO/FIXME,
  commented-out code and cross-file duplication apply to both languages. Rule profiles
  `default`, `conventions` (an opt-in style pass), `all` and `none`.
- **Sources:** scan the working tree, any commit or tag from the git object database without a
  checkout (`--rev`), the staged changes (`--staged`) or a remote repository (`--remote`), with a
  content-addressed analysis cache that makes rescans incremental.
- **New-code gating:** `--base` classifies findings as new, unchanged or worsened against the
  merge base, matching by fingerprint so line shifts, renames and moved code don't count as new;
  `--baseline` compares with a saved results file instead; `--changed-only` limits the scan to
  changed files. A quality gate on new findings by severity, duplication and coverage of new
  lines (LCOV, JaCoCo and Cobertura reports) and, optionally, the maintainability grade of the new
  code; the exit code is the verdict. `mote gate` re-evaluates a saved result (for example once
  coverage is available) and `mote compare` diffs two results.
- **Measures:** estimated remediation effort, maintainability and reliability grades (A–E),
  duplication, complexity, per-directory rollups, main and test code apart.
- **Configuration:** optional `.mote.yaml` with excludes, test globs, per-rule severities and
  parameters, test-only rule settings and path overrides; generated, minified and vendored files
  are skipped by default. `mote config validate` reports every problem at once with "did you
  mean" hints, `mote config explain` shows the effective rules for a file, and
  `mote config schema` prints JSON Schemas for the configuration and the results.
- **Suppressions and triage:** rule-scoped `mote-ignore[rule]: reason` comments and
  `@SuppressWarnings("mote:rule")`, with meta rules for blanket, unexplained and unused
  suppressions. Opt-in compatibility with other tools' suppressions: `eslint-disable` comments,
  `NOSONAR`, `NOPMD` and `@SuppressWarnings("PMD…")`, Checkstyle on/off comments,
  `@SuppressWarnings("all")`, `@SuppressFBWarnings` and IntelliJ `//noinspection`. A triage file
  (`.mote-triage.yaml`, `mote triage add|list|prune`) records accepted findings and false
  positives outside the code.
- **Migration:** `mote init` writes a commented starter configuration, proposing excludes for
  third-party code copied into the repository and the suppression sources it already uses;
  `mote init --from eslint|pmd` and `mote migrate` carry over the rules, severities, thresholds
  and ignores of an ESLint configuration (read statically, never run) or a PMD ruleset.
- **Outputs:** text, JSON, JSONL, SARIF 2.1.0, Markdown, GitHub Checks, GitLab Code Quality,
  Checkstyle XML, JUnit XML, a self-contained HTML report, Bitbucket Code Insights and a one-line
  summary record for trends; several formats can be written in one run.
- **CI:** a GitHub Action that installs a checksum-verified binary, scans the pull request
  against its merge base, annotates new findings, keeps one sticky PR comment, writes the job
  summary, supports an override label and a separate coverage job, and caches the analysis;
  recipes for GitLab CI, Jenkins, Azure Pipelines and Bitbucket Pipelines; a pre-commit hook.
- **MCP server:** `mote mcp` for AI coding agents, with `scan`, `summary`, `work_units` (disjoint
  per-file groups for parallel fixers), `findings`, `rule`, `verify` (re-checks only edited files)
  and `explain_config`.
- **Release binaries** for Linux (x86_64, ARM64), macOS (Apple silicon, Intel) and Windows
  (x86_64), each with a SHA-256 checksum.

### Known limitations

- No stable plugin or rule API: language plugins are compiled in, and the crates' Rust
  interfaces are internal.
- Analysis is per file (duplication aside): no whole-program type information, TypeScript type
  checker or data-flow analysis, so rules that would need them are not included.

[Unreleased]: https://github.com/atmuccio/mote/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/atmuccio/mote/releases/tag/v0.2.0
[0.1.0]: https://github.com/atmuccio/mote/releases/tag/v0.1.0
