# Mote at a glance

A one-page summary for a technical reader deciding whether to try Mote. Details are in the linked
documents; every number below comes from [benchmark.md](benchmark.md) or [rules.md](rules.md).

## What it is

Mote is a code-quality scanner for **Java** and **JavaScript/TypeScript** (TSX, JSX and React
hooks included). It finds bugs, code smells, excessive complexity and duplicated code, and gates
pull requests on the code they change. It ships as **one binary** with no server, database, JVM,
Node.js or build step: you download it, point it at a repository, and it prints findings, writes
reports and exits with the gate's verdict.

## Why teams look at it

* **Speed.** Measured on public repositories (Intel Core i7-8700K, 6 cores / 12 threads,
  Windows 10; [benchmark.md, Part 3](benchmark.md#part-3-real-repositories)):

  | Repository | Size | First scan | Re-scan (cache) |
  |---|---|--:|--:|
  | VS Code (TypeScript) | 14,036 files, 4.2 million lines | 1.4 s | 0.3 s |
  | SymmetricDS (Java) | 1,763 files, 330,000 lines | 0.63 s | 0.07 s |
  | Excalidraw (TypeScript/React) | 695 files, 216,000 lines | 0.10 s | 0.05 s |

  A pull-request scan only analyzes the changed files again; the rest comes from the analysis
  cache, which CI can restore between runs.
* **Nothing leaves the machine.** The binary makes no network calls (the one exception is
  `--remote`, which asks git to clone a repository) and never executes code from the repository it
  scans, configuration files included. It works offline and in air-gapped networks: copy the
  verified binary in and it needs nothing else. Results are files you control.
* **Fits what a team already has.** `mote init --from eslint` or `--from pmd` turns an existing
  ESLint configuration or PMD ruleset into a Mote configuration: rules, severities, thresholds,
  excludes and path-specific overrides, each line saying where it came from, and a list of what
  could not be carried over. JavaScript and TypeScript configuration files are parsed, never run,
  so no `npm install` is needed. Suppression comments and annotations already in the code
  (ESLint, PMD, Checkstyle, SpotBugs, IntelliJ and `@SuppressWarnings` markers) can be honored, so
  findings the team has already looked at stay quiet ([migration.md](migration.md)).
* **Deterministic.** The same input gives the same findings, byte for byte, whatever the thread
  count; finding ids are stable while the flagged code is unchanged.

## What it checks

**552 rules**: 326 for Java, 221 for JavaScript/TypeScript and 5 engine rules; 485 are on by
default ([rules.md](rules.md), or `mote rules --all`). Each has a severity — **HIGH** (almost
certainly a bug), **MEDIUM** (a likely problem or a significant maintainability cost), **LOW**
(minor or stylistic) — and a description with examples (`mote rules explain <id>`). Complexity,
function and file length, nesting, cross-file duplication, TODO/FIXME and commented-out code apply
to both languages. Maintainability and reliability grades (A to E) summarize a code base and, on
request, gate new code.

**How precision is kept.** A rule starts as *incubating*: it reports findings, but never fails the
gate. It becomes *stable* once a hand-labeled sample of its findings on real open-source code shows
it is right. The rules are measured on about 30 pinned public repositories; at the last review
(2026-10-06) every stable rule that fired there — 355 rules — had labeled findings, and none was
below **95% precision** ([benchmark.md, Part 4](benchmark.md#part-4-coverage-yardstick), criterion 4).
Today 393 rules are stable and 159 incubating. Every rule also has unit fixtures with
false-positive traps, and two sample projects are checked against golden outputs on every change.

## How it is used

* **Pull-request gate on new code.** `mote scan --base origin/main` classifies every finding as
  *new*, *unchanged* or *worsened*, and the gate (by default: no new HIGH findings) applies only to
  what the change introduced; old debt never blocks a merge. Replayed on 500 real commits of two
  projects as one-commit pull requests, the engine reported no finding as new whose code was
  unchanged, and missed none that was new ([benchmark.md, Part 5](benchmark.md#part-5-pr-replay)).
  The gate also checks duplication on new lines and, given coverage reports, coverage of new lines.
* **Baselines for legacy code.** Without git history, or for a first rollout:
  `mote scan --no-gate --json mote-baseline.json` once, then `mote scan --baseline
  mote-baseline.json` gates only findings that are not in it. Single findings can be marked
  accepted or false positive in a reviewed triage file instead of comments in the code.
* **Outputs.** Text, JSON, JSONL, SARIF 2.1.0 (code-scanning dashboards), GitHub Checks
  annotations and a Markdown summary, GitLab Code Quality, JUnit XML, Checkstyle XML, Bitbucket
  Code Insights, a self-contained HTML report, and a one-line metrics record for trend charts —
  several in one run ([cli.md, Output formats](cli.md#output-formats)).
* **CI.** A GitHub Action (check-run annotations, a sticky pull-request comment, optional SARIF
  upload, override label) and recipes for GitLab CI, Jenkins, Azure Pipelines and Bitbucket
  Pipelines ([github-action.md](github-action.md), [ci.md](ci.md),
  [examples](../examples/README.md)). A pre-commit hook checks staged changes.
* **AI coding agents.** `mote mcp` is a Model Context Protocol server: an agent scans, gets
  findings split into disjoint work units for parallel fixers, and re-verifies only the files it
  edited ([cli.md, `mote mcp`](cli.md#mote-mcp)).

## Scope and status

* **Not a security scanner.** Mote does not look for dependency vulnerabilities, does not track
  tainted data, and does not detect secrets or cloud misconfiguration; keep a dedicated security
  tool for those. Its rules about risky code (such as `eval`) are code-quality checks.
* **Languages.** Java and JavaScript/TypeScript today. Analysis is per file plus cross-file
  duplication; there is no whole-program type resolution or data-flow analysis.
* **Status: 0.x.** The current release is 0.2.0. Rules, defaults and output details
  may still change between 0.x versions, so pin the version you use in CI. The results JSON has a
  `format_version` and only gains fields within it.
* **License and contributions.** MIT or Apache-2.0, at your option. Bug reports, false positives
  and ideas are welcome as GitHub issues; pull requests are not accepted.

To see it run, follow the [10-minute demo](demo.md).
