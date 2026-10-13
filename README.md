# Mote

**Mote is a fast code-quality scanner for Java and JavaScript/TypeScript.** It finds bugs, code
smells, complexity and duplication, and gates pull requests only on the problems they introduce.
It is a single binary: no server, database, JVM or Node.js to install, no build step, and nothing
leaves your machine (the binary never touches the network; only `--remote` asks git to clone).
A first scan of a 4-million-line TypeScript code base takes under two seconds. Already using
ESLint or PMD? `mote init --from eslint` or `--from pmd` carries your rule choices, severities and
ignores over, and Mote can honor the suppression comments already in your code.

> **Status: 0.x.** Mote works and is tested on Windows, Linux and macOS, but it is young: the
> configuration file and the results formats may still change between 0.x releases (results
> files carry a `format_version`; changes are listed in the [changelog](CHANGELOG.md)). There is
> no stable plugin or rule API yet: language plugins and rules are compiled into the binary, and
> their Rust interfaces are internal.

New to Mote? [docs/overview.md](docs/overview.md) explains the ideas in a few minutes, and
[docs/demo.md](docs/demo.md) walks through a scan, a pull request and the agent loop.

Here it is on a spring-petclinic branch that adds one file. The existing findings (elided) are
reported but don't fail the gate; the new HIGH finding the branch introduces does:

```text
$ mote scan --base origin/main
…
src/main/java/org/springframework/samples/petclinic/owner/VisitStats.java:9:22   MEDIUM  java/integer-division-in-floating-context  Cast an operand to double before dividing; this integer division truncates the result [new]
src/main/java/org/springframework/samples/petclinic/owner/VisitStats.java:13:21  HIGH    java/string-reference-equality             Compare these strings with equals() instead of == [new]
…
Files         51 analyzed, 0 cached, 0 skipped (2,477 lines of code)
Findings      1 HIGH, 5 MEDIUM, 7 LOW (13 total)
New code      vs origin/main (merge base 818c4136ea97): 1 file changed, 15 new lines
New findings  1 HIGH, 1 MEDIUM, 0 LOW (2 new, 0 worsened, 0 fixed)
Duplication   0.0% overall, 0.0% on new code
Quality gate  FAILED
  FAIL  New findings              1 (<= 0)
  skip  Duplication on new code   only 15 new lines (minimum 20)
  skip  Coverage on new code      no coverage reports
Done in       93 ms
```

## Why Mote

* **Fast.** Tree-sitter (Java) and Oxc (JavaScript/TypeScript) parsers, one traversal per file,
  all cores, and a content-addressed cache. A first scan of VS Code (14,036 files, 4.2 million
  lines) takes 1.4 s on a 6-core desktop, a rescan 0.3 s.
* **Serverless and private.** Nothing to host or keep running, no account, no telemetry. The
  binary never executes code from the repository it scans: build files and ESLint configurations
  are read as data.
* **Any branch or ref.** Scan the working tree, a commit or tag straight from the git object
  database (`--rev`, no checkout), the staged changes (`--staged`), or a remote URL (`--remote`).
* **New-code gating.** With `--base origin/main`, findings are classified as *new*, *unchanged*
  or *worsened*, and the gate fails only on new problems (by default: new HIGH findings, plus
  duplication and, when you provide coverage reports, coverage on the new lines). Legacy debt
  doesn't block anyone.
* **Drop-in for existing setups.** Import an ESLint or PMD configuration, honor `eslint-disable`,
  `NOPMD`, `@SuppressWarnings`, `NOSONAR` and other tools' suppression comments, roll out with a
  baseline, and record accepted findings in a triage file.
* **Agent-friendly.** `mote mcp` is an MCP server: scans, findings split into disjoint work units
  for parallel fixers, and a `verify` tool that re-checks only the edited files. Finding ids are
  stable across scans.
* **Deterministic.** The same input gives the same findings, byte for byte, whatever the thread
  count.

## Install

**Release binaries.** Each [GitHub release](https://github.com/atmuccio/mote/releases) has an
archive per platform, `mote-<version>-<target>.tar.gz` (`.zip` on Windows; `<version>` without
the leading `v`, e.g. `mote-0.2.0-x86_64-unknown-linux-gnu.tar.gz`), with a `.sha256` checksum
file next to it and all checksums in `SHA256SUMS`. Each archive holds the `mote` binary, the
licenses, NOTICE, THIRD_PARTY_LICENSES.md and this README.

| Platform | `<target>` |
|---|---|
| Linux x86_64 (glibc 2.35 or later) | `x86_64-unknown-linux-gnu` |
| Linux ARM64 (glibc 2.35 or later) | `aarch64-unknown-linux-gnu` |
| Linux x86_64, static (any distribution, Alpine included)\* | `x86_64-unknown-linux-musl` |
| Linux ARM64, static (any distribution, Alpine included)\* | `aarch64-unknown-linux-musl` |
| macOS Apple silicon | `aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` |
| Windows x86_64 | `x86_64-pc-windows-msvc` |
| Windows ARM64\* | `aarch64-pc-windows-msvc` |

\* Best effort: attached to a release when it built.

Linux and macOS (set `version` to the release you want):

```sh
version=0.2.0
target=x86_64-unknown-linux-gnu        # pick yours from the table
archive="mote-$version-$target.tar.gz"
base="https://github.com/atmuccio/mote/releases/download/v$version"
curl -fsSLO "$base/$archive" -O "$base/$archive.sha256"
sha256sum -c "$archive.sha256"         # macOS: shasum -a 256 -c "$archive.sha256"
tar xzf "$archive" mote && sudo mv mote /usr/local/bin/
mote version
```

Windows (PowerShell):

```powershell
$version = '0.2.0'
$archive = "mote-$version-x86_64-pc-windows-msvc.zip"
$base = "https://github.com/atmuccio/mote/releases/download/v$version"
Invoke-WebRequest -UseBasicParsing "$base/$archive" -OutFile $archive
Invoke-WebRequest -UseBasicParsing "$base/$archive.sha256" -OutFile "$archive.sha256"
$expected = (Get-Content "$archive.sha256").Split(' ')[0]
if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $expected) { throw "checksum mismatch: $archive" }
Expand-Archive $archive -DestinationPath mote
.\mote\mote.exe version
```

Then put the `mote` folder (or just `mote.exe`) on your `PATH`.

**With cargo** (Rust 1.85 or later):

```sh
cargo install --locked --git https://github.com/atmuccio/mote mote-cli   # installs `mote` into ~/.cargo/bin
```

Or from a clone: `cargo build --release -p mote-cli` puts the binary in `target/release/`.
Languages are cargo features (`java`, `js`, both on by default):
`--no-default-features --features java` builds a Java-only binary.

## Quick start

```sh
cd your-repo
mote init                          # write a commented .mote.yaml (excludes for copied third-party code, other tools' suppressions honored)
mote scan --no-gate                # a first look at everything
mote scan --base origin/main       # gate only what this branch changes; exit code 1 if the gate fails
```

Coming from ESLint or PMD:

```sh
mote init --from eslint                  # finds eslint.config.*, .eslintrc* or package.json in the directory
mote init --from pmd config/ruleset.xml  # or a PMD ruleset
mote migrate --from pmd ruleset.xml      # just print the proposal, write nothing
```

The import maps each rule to the Mote rule for the same problem, keeps your severities,
thresholds and ignores, and lists what it could not map ([docs/migration.md](docs/migration.md)).

More things to try:

```sh
mote scan --rev v2.3.0                                        # any commit or tag, no checkout
mote scan --remote https://github.com/spring-projects/spring-petclinic --rev main
mote scan --staged --base HEAD --new-only                     # as a pre-commit hook
mote rules explain java/empty-catch-block                     # what a rule checks and why
mote config explain src/app/Order.java                        # the effective rules for one file
```

The analysis cache lives in `.mote/cache` under the scan root (add `.mote/` to `.gitignore`); a
second scan only re-analyzes the files that changed.

| Exit code | Meaning |
|---|---|
| 0 | Scan completed; the gate passed (or `--no-gate`) |
| 1 | Scan completed; the gate failed |
| 2 | Usage or configuration error |
| 3 | Internal error (or `--strict` and a file couldn't be analyzed) |

Rolling out on a code base with many old findings? Either gate on `--base` in pull requests, or
save a baseline once (`mote scan --no-gate --json baseline.json`) and scan with
`--baseline baseline.json`: only findings not in the baseline count.

## Pull-request gating in CI

**GitHub Actions.** The Mote action installs the binary (checksum-verified), scans the PR head
against its merge base, annotates new findings, posts a sticky PR comment and a job summary, and
fails the job when the gate fails:

```yaml
on:
  pull_request:
  push:
    branches: [main]
permissions:
  contents: read
  checks: write
  pull-requests: write
jobs:
  mote:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - uses: atmuccio/mote@v0.2.0   # the action ref pins the engine version
        with:
          fail-on-gate: ${{ github.event_name == 'pull_request' }}
```

Complete workflows, with coverage on new code and an override label, are in
[examples/workflows/](examples/workflows/); every input and output is in
[docs/github-action.md](docs/github-action.md).

**Other CI systems** run the binary directly, for example
`mote scan --base origin/main --sarif results.sarif --markdown summary.md`.
[docs/ci.md](docs/ci.md) has recipes for GitLab (Code Quality widget and merge-request notes),
Jenkins, Azure Pipelines and Bitbucket Pipelines.

**Pre-commit.** [`examples/hooks/pre-commit`](examples/hooks/pre-commit) is a ready-made git hook
that blocks commits whose staged changes fail the gate.

## Output formats

| `--format` | Use |
|---|---|
| `text` (default) | Humans: grouped by file, colored on a terminal, with a summary and the gate verdict |
| `json` | The complete result; input for `mote gate` (re-evaluate with coverage later) and `mote compare` |
| `jsonl` | One finding per line: easy to grep, diff, or feed to a script or an agent |
| `sarif` | SARIF 2.1.0 for GitHub code scanning and other viewers |
| `markdown` | Job summaries and PR comments |
| `github` | A GitHub Checks API payload with annotations for new findings |
| `gitlab` | A GitLab Code Quality report (merge-request widget and diff annotations) |
| `checkstyle` | Checkstyle XML for CI dashboards (e.g. Jenkins Warnings NG) |
| `junit` | JUnit XML for CI test views: the gate as a test suite, one suite per rule |
| `html` | A self-contained HTML report: overview, grades, directory tree, findings and rules |

`--json`, `--jsonl`, `--sarif`, `--markdown`, `--github-checks`, `--gitlab-codequality`,
`--checkstyle`, `--junit` and `--html` write extra formats in the same run;
`--bitbucket-insights DIR` writes a Bitbucket Code Insights report, and `--summary-json` a one-line
metrics record to archive for trends.

## Configuration

`.mote.yaml` at the scan root is optional; without it you get the default rules and gate.
`mote init` writes a commented one. A typical file:

```yaml
mote: ">=0.1"                     # engine version this file needs
extends: default                  # default | conventions | all | none

exclude: ["**/generated/**", "vendor/**"]
tests: ["**/it/**"]               # extra test globs (tests are found by location)

rules:                            # rule ids or globs such as "js/*"
  java/magic-number: on           # off by default
  js/no-console: { severity: HIGH }
  java/cognitive-complexity: { params: { threshold: 20 } }

overrides:                        # path-specific settings, applied in order
  - paths: ["legacy/**"]
    rules:
      java/cognitive-complexity: { params: { threshold: 30 } }

gate:
  fail_on: HIGH                   # new findings at or above this severity fail
  new_duplication_pct: 3.0        # max duplication on new lines
  new_coverage_pct: 80.0          # min coverage on new lines (needs coverage reports)

coverage: { reports: ["**/jacoco.xml", "coverage/lcov.info"] }
suppressions: { require_reason: true, compat: [eslint, pmd] }
```

Suppress one finding in code with a reason:

```java
// mote-ignore[java/system-out-print]: the batch operator reads this summary on the console
System.out.println("Exported orders: " + ids.length);
```

`mote config validate` reports every problem at once with "did you mean" hints, and
`mote config schema` prints a JSON Schema for editor completion. The full reference (every
option, profiles, test files, suppressions, triage, baselines, grades and the results format) is
in [docs/cli.md](docs/cli.md).

## Rules

552 rules: 326 for Java, 221 for JavaScript/TypeScript (including TSX and React hooks) and 5
for suppressions and analysis errors; 393 are stable and 159 incubating, and 485 are on by
default. They cover bugs (`HIGH`: almost certainly wrong), likely problems and significant
maintainability costs (`MEDIUM`), and minor or stylistic issues (`LOW`). Complexity, function and
file length, nesting, parameter counts, TODO/FIXME, commented-out code and cross-file duplication
apply to both languages. Incubating rules report but never fail the gate; opinionated and
style rules are off by default (the `conventions` profile adds Java and TypeScript style-guide
rules).

* The catalog: [docs/rules.md](docs/rules.md), or `mote rules --all`.
* One rule in detail: `mote rules explain js/rules-of-hooks`.

Rules favor precision over recall: a rule that cannot be sure stays silent. Each rule's measured
precision on labeled findings in public open-source repositories, or why it has none yet, is
listed in [docs/rules.md](docs/rules.md#measured-precision) and shown by `mote rules explain`;
how precision and recall are measured is in [docs/benchmark.md](docs/benchmark.md).

### Fixing findings automatically

17 rules can rewrite their findings mechanically, where the rewrite cannot change what the
program does — unused imports, modifier order, `10l` → `10L`, missing `@Override`, `let` →
`const`, unnecessary escapes and the like (marked `[fixable]` in the scan output and "Fix
available" in [docs/rules.md](docs/rules.md)). New in 0.2.0, so review the diff before your first
`--write`:

```sh
mote fix                       # show the changes as a unified diff; writes nothing (exit 1 if any)
mote fix --write               # apply them
mote fix --rule java/unused-import --write
```

`mote fix` works on the working tree with your configuration; it never touches suppressed or
triaged findings, keeps encodings and line endings, repeats until no fix is left, and throws
away a file's changes if they would add a syntax error. The results JSON and SARIF carry the
fixes, and the MCP server returns them to agents. Details:
[docs/cli.md](docs/cli.md#mote-fix).

## AI agents: MCP server

`mote mcp` speaks the [Model Context Protocol](https://modelcontextprotocol.io) over stdio. Add it
to an MCP client, for example in `.mcp.json` at the repository root:

```json
{
  "mcpServers": {
    "mote": { "command": "mote", "args": ["mcp"] }
  }
}
```

It offers `scan`, `summary`, `work_units` (findings split into disjoint groups of files, one per
parallel fixer), `findings`, `fixes` (the mechanical fixes of fixable findings as edits and a
diff; read-only), `rule`, `verify` (re-analyzes only the edited files: each earlier finding is
`fixed` or `still_present`, plus anything `introduced`) and `explain_config`. A typical
agent loop: `scan` → `work_units` → one sub-agent per unit reads `findings`, edits, and calls
`verify` until its unit is clean. Details: [docs/cli.md](docs/cli.md#mote-mcp).

## Performance

Wall time of the whole process with all default rules, median of 5 runs on an Intel i7-8700K
(6 cores / 12 threads) under Windows 10. *Cold* means an empty analysis cache, *warm* a primed
one. Measured with Mote 0.2.0 (552 rules, 485 on by default); method, all numbers and the precision
sampling are in [docs/benchmark.md](docs/benchmark.md#results).

| Repository (pinned commit) | Files | Lines | Cold | Warm | Cold, `--rev` (no checkout) |
|---|--:|--:|--:|--:|--:|
| [spring-petclinic](https://github.com/spring-projects/spring-petclinic) (Java) | 50 | 4,214 | 0.05 s | 0.03 s | 0.17 s |
| [SymmetricDS](https://github.com/JumpMind/symmetric-ds) (Java) | 1,763 | 330,130 | 0.63 s | 0.07 s | 0.87 s |
| [Excalidraw](https://github.com/excalidraw/excalidraw) (TypeScript/React) | 695 | 215,689 | 0.10 s | 0.05 s | 0.28 s |
| [VS Code](https://github.com/microsoft/vscode) (TypeScript) | 14,036 | 4,152,081 | 1.45 s | 0.29 s | 2.55 s |

## Scope

Mote is a code-quality tool: bugs, smells, complexity, duplication and coverage of new code. It
is **not a security scanner** and does not replace one: dependency vulnerabilities, taint
tracking, secrets detection and infrastructure configuration belong to dedicated security tools.
It analyzes Java (syntax up to Java 25) and JavaScript/TypeScript (including JSX/TSX); other
languages are not supported yet. Analysis is per file (duplication aside): Java types are resolved from the file,
its imports and common JDK classes, and there is no TypeScript type checker. Rules that would need
whole-program types or data flow are left out rather than guessed at.

## Documentation

| Document | What's in it |
|---|---|
| [docs/overview.md](docs/overview.md) | What Mote does and how it fits into a workflow |
| [docs/demo.md](docs/demo.md) | A guided tour: first scan, a pull request, the agent loop |
| [docs/cli.md](docs/cli.md) | Every command and option, configuration, suppressions, results format |
| [docs/migration.md](docs/migration.md) | Moving from ESLint, PMD or a server-based tool |
| [docs/github-action.md](docs/github-action.md) | The GitHub Action: inputs, outputs, coverage, fork PRs |
| [docs/ci.md](docs/ci.md) | GitLab, Jenkins, Azure Pipelines, Bitbucket and trend recipes |
| [docs/rules.md](docs/rules.md) | The rule catalog |
| [docs/benchmark.md](docs/benchmark.md) | How accuracy, speed, recall and new-code classification are measured, with results |
| [docs/architecture.md](docs/architecture.md) | How Mote is built: crates, data flow, rule conventions |

## Feedback and issues

Bug reports, false positives, missed problems and feature requests are welcome as
[GitHub issues](https://github.com/atmuccio/mote/issues/new/choose). For a false positive, the
rule id, the Mote version (`mote version`) and a minimal code snippet make it quick to fix. Pull
requests are not accepted for now; [CONTRIBUTING.md](CONTRIBUTING.md) explains why and how to
report. Security problems in Mote itself: see [SECURITY.md](SECURITY.md).

## Provenance

The rules are original work: their descriptions, messages and implementations were written for
Mote. Some rule ideas and names follow permissively licensed tools: ESLint and typescript-eslint
(MIT), PMD (BSD) and Error Prone (Apache-2.0). No code, rule descriptions or rule keys were taken
from proprietary analyzers or from GPL/LGPL-licensed tools. The opt-in `conventions` rules check
ideas from the Google Java, TypeScript and JavaScript style guides (CC-BY 3.0) and the archived
Code Conventions for the Java Programming Language; the rules and their wording are ours.

## License

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT), at your option. The release binaries include third-party crates
under permissive licenses; their notices are in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
