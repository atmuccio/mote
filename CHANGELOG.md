# Changelog

All notable changes to Mote are listed in this file. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Mote follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). While the version is 0.x, a minor
release (0.1 → 0.2) may change the configuration file or the results formats (a breaking change
to the results JSON bumps its `format_version`); such changes are listed under **Changed** or
**Removed** with what to do.

## [Unreleased]

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

[Unreleased]: https://github.com/atmuccio/mote/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/atmuccio/mote/releases/tag/v0.1.0
