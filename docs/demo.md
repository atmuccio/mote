# A 10-minute Mote demo

A script for showing Mote live on any laptop: download one binary, clone two public repositories,
and walk through speed, a first look at a code base, migrating an existing linter configuration,
a pull-request gate, a baseline for legacy code and, if there is time, the agent integration.

The commands are for bash or zsh: macOS, Linux, or Git Bash on Windows. Everything runs offline
once the binary and the two clones are on the machine.

## Before the demo (once, about 5 minutes)

**1. Get the binary.** Pick the archive for the machine from the
[release page](https://github.com/atmuccio/mote/releases) and check it against its `.sha256` file.
Nothing else needs installing (no JVM, no Node.js, no server); `git` is only needed to clone.

```sh
mkdir -p ~/mote-demo/bin && cd ~/mote-demo
version=0.2.0
target=aarch64-apple-darwin   # x86_64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu
base=https://github.com/atmuccio/mote/releases/download/v$version
curl -fsSLO "$base/mote-$version-$target.tar.gz" -O "$base/mote-$version-$target.tar.gz.sha256"
shasum -a 256 -c "mote-$version-$target.tar.gz.sha256"     # Linux: sha256sum -c …
tar -xzf "mote-$version-$target.tar.gz" -C bin mote
export PATH="$HOME/mote-demo/bin:$PATH"
mote version
```

On Windows (Git Bash), the archive is a zip (same `version` and `base` as above):

```sh
target=x86_64-pc-windows-msvc
curl -fsSLO "$base/mote-$version-$target.zip" -O "$base/mote-$version-$target.zip.sha256"
sha256sum -c "mote-$version-$target.zip.sha256"
unzip -o "mote-$version-$target.zip" mote.exe -d bin
export PATH="$HOME/mote-demo/bin:$PATH"
mote version
```

**2. Clone the two targets** (shallow: about 1 MB and 35 MB):

```sh
cd ~/mote-demo
git clone --depth 1 https://github.com/spring-projects/spring-petclinic.git   # Java, 50 files
git clone --depth 1 https://github.com/excalidraw/excalidraw.git              # TypeScript/React, ~700 files
```

**3. Rehearse.** Run the whole script once, then reset both clones so that the first scan on
stage is a cold one:

```sh
cd ~/mote-demo/spring-petclinic && git checkout -q main && git branch -D add-visit-stats; git clean -fdxq
cd ~/mote-demo/excalidraw && git clean -fdxq
```

(`git clean -fdx` deletes the untracked files the demo creates: Mote's cache in `.mote/`, the
reports, `.mote.yaml`.) Keep a terminal with a large font and `~/mote-demo/bin` on `PATH`.

## 1. Speed (2 minutes)

```sh
cd ~/mote-demo/excalidraw
mote scan --no-gate                        # first scan: nothing cached yet
mote scan --no-gate                        # again: every file comes from the cache
mote scan --no-gate --no-cache --timings   # a full analysis again, with the time per phase
```

**Point out**

* The `Done in` line: about 700 files and 216,000 lines analyzed in a fraction of a second, then
  tens of milliseconds once cached. (The very first scan after cloning also waits for the disk
  and, on Windows, for antivirus software to look at the new files: up to about a second.) In
  that time the process starts, walks the tree, parses every file, runs every default rule, finds
  duplicated blocks and prints; there is no server or daemon behind it.
* `Files … analyzed, … cached`: the cache is content-addressed, so a CI job that restores it only
  re-analyzes what changed. It lives in `.mote/` inside the repository.
* `--timings`: discovery, analysis on all cores, duplication, output.
* For scale: VS Code (14,000 files, 4.2 million lines) scans in 1.4 s cold and 0.3 s cached on a
  6-core desktop ([benchmark.md](benchmark.md#results)). Optional, if the network is fast:
  `git clone --depth 1 https://github.com/microsoft/vscode.git` (about 70 MB) and scan it.

## 2. First look at a code base (2 minutes)

```sh
mote scan --no-gate --min-severity HIGH          # only the likely bugs, then the summary
mote rules explain js/no-async-promise-executor  # why one of them matters, and the fix
mote scan --no-gate --format html -o mote-report.html
open mote-report.html                            # Linux: xdg-open; Windows: start
```

**Point out**

* The HIGH findings are bugs, not style: `async` Promise executors whose errors are lost, a
  `return` inside `finally` that swallows exceptions, a reassigned function, a number literal that
  cannot be represented exactly. Each line has the file, line, rule and a message saying what to
  do.
* The summary: findings by severity, complexity, estimated effort, duplication, and the
  **grades** (A to E) for maintainability and reliability.
* `Quality gate FAILED` with `--no-gate` still exiting 0: without a base, every existing finding
  counts. Section 4 shows the gate on new code only, which is how it is meant to run.
* The HTML report is one file, no network access, no source code in it: overview, a directory
  tree, a filterable findings list (by severity, rule, text), and every rule with its description.
  It can be attached to any CI job as an artifact.

## 3. Bring the existing linter configuration (2 minutes)

Excalidraw has an ESLint configuration (`.eslintrc.json`):

```sh
mote init --from eslint      # writes .mote.yaml from the ESLint configuration
mote scan --no-gate          # compare the summary line with section 1
```

**Point out**

* `init` lists what it mapped and what it could not (presets such as `react-app` are named, not
  silently expanded), and every migrated line in `.mote.yaml` says where it came from.
* It counted the `eslint-disable` comments already in the code and turned on
  `suppressions: { compat: [eslint] }`: the second scan reports a couple of hundred findings as
  suppressed, the ones the team had already silenced in ESLint, and the HIGH count drops.
* `mote config validate` checks the file; unknown keys and ESLint or PMD rule names get a
  "did you mean" answer.

A modern flat configuration in JavaScript is read the same way, **without running it**: its
imports need not be installed.

```sh
d=$(mktemp -d) && cat > "$d/eslint.config.mjs" <<'EOF'
import js from "@eslint/js";
import tseslint from "typescript-eslint";

export default tseslint.config(
  js.configs.recommended,
  tseslint.configs.recommended,
  { ignores: ["dist/**"] },
  {
    rules: {
      eqeqeq: "error",
      complexity: ["error", 15],
      "max-depth": ["warn", 4],
      "no-console": "warn",
    },
  },
  { files: ["**/*.test.ts"], rules: { "no-console": "off" } },
);
EOF
cd "$d" && mote migrate --from eslint eslint.config.mjs   # prints the proposal, writes nothing
```

The same works for a PMD ruleset (`mote init --from pmd` also finds the ruleset a `pom.xml` or
Gradle build names). Spring PetClinic has none, so use a small one:

```sh
cat > "$d/pmd-ruleset.xml" <<'EOF'
<?xml version="1.0"?>
<ruleset name="team" xmlns="http://pmd.sourceforge.net/ruleset/2.0.0">
  <description>Our PMD rules</description>
  <rule ref="category/java/errorprone.xml/EmptyCatchBlock"/>
  <rule ref="category/java/bestpractices.xml/SystemPrintln"><priority>1</priority></rule>
  <rule ref="category/java/design.xml/CognitiveComplexity">
    <properties><property name="reportLevel" value="20"/></properties>
  </rule>
  <rule ref="category/java/documentation.xml/CommentRequired"/>
</ruleset>
EOF
mote migrate --from pmd pmd-ruleset.xml
```

**Point out**: thresholds are converted (`reportLevel` 20 becomes `threshold: 19`, as PMD reports
above the level), priority 1 becomes `HIGH`, and the rule without an equivalent is listed at the
end instead of being dropped silently.

## 4. Gate a pull request on its new code (2 minutes)

Make a change on a branch, the way a pull request would, in Spring PetClinic:

```sh
cd ~/mote-demo/spring-petclinic
git switch -c add-visit-stats
cat > src/main/java/org/springframework/samples/petclinic/owner/VisitStats.java <<'EOF'
package org.springframework.samples.petclinic.owner;

import java.util.List;

class VisitStats {

	double averageVisits(List<Pet> pets) {
		int visits = pets.stream().mapToInt(p -> p.getVisits().size()).sum();
		return visits / pets.size();
	}

	boolean isCat(Pet pet) {
		return pet.getType().getName() == "cat";
	}

}
EOF
mote scan --base main --new-only; echo "exit code: $?"
```

**Point out**

* `New code vs main (merge base …): 1 file changed, 16 new lines`, and two findings marked
  `[new]`: an integer division that truncates the average (MEDIUM) and strings compared with `==`
  (HIGH). The 20 existing findings of the project are still counted in the summary, not listed,
  and do not fail anything.
* `Quality gate FAILED`, `New findings 1 (<= 0)`, exit code 1: in CI that is the failed check.
  The default gate fails on new HIGH findings only; duplication and coverage of the new lines are
  checked when there are enough of them (and coverage reports).
* Nothing had to be committed: the working tree is compared with the merge base, like the
  "Files changed" tab of a pull request. In CI the same command runs on the checkout.

The reports a CI system reads, written by the same run:

```sh
mote scan --base main --new-only --sarif mote.sarif --markdown mote-summary.md >/dev/null
head -25 mote-summary.md
```

* `mote-summary.md` is the pull-request comment the GitHub Action keeps up to date (and the job
  summary); `mote.sarif` goes to GitHub code scanning, where the new findings appear as alerts and
  annotations in the diff. GitLab Code Quality, JUnit, Checkstyle and Bitbucket reports work the
  same way ([examples](../examples/README.md)).

Fix the HIGH finding and the gate passes:

```sh
perl -pi -e 's/return (.+) == "cat";/return "cat".equals($1);/' \
  src/main/java/org/springframework/samples/petclinic/owner/VisitStats.java
mote scan --base main --new-only; echo "exit code: $?"
```

## 5. A baseline for legacy code (1 minute)

Excalidraw's existing findings fail the plain gate (section 2). Record them once, then gate only
what is added later — the way to roll Mote out on an old code base without fixing everything
first, and without git history in the CI job:

```sh
cd ~/mote-demo/excalidraw
mote scan --no-gate --format json -o mote-baseline.json   # the current state, recorded once
mote scan --baseline mote-baseline.json                    # passes: nothing is new
printf 'export const isMissing = (value: number) => value === NaN;\n' > demo-new-code.ts
mote scan --baseline mote-baseline.json --new-only; echo "exit code: $?"   # the one new bug fails it
rm demo-new-code.ts
```

**Point out**: findings are matched by fingerprint (rule, enclosing symbol, line text), so they
survive lines moving and files being renamed. The baseline can be committed or kept as a CI
artifact of the default branch. Single findings can also be accepted or marked false positive in
a reviewed triage file (`mote triage add`) instead of a comment in the code.

## 6. Optional: AI coding agents (1 minute)

`mote mcp` is a Model Context Protocol server on stdin/stdout. To see it answer:

```sh
cd ~/mote-demo/spring-petclinic
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"scan","arguments":{"base":"main","new_only":true}}}' \
  | mote mcp
```

To give it to an agent, register the command `mote mcp` in the agent's MCP configuration, for
example:

```json
{ "mcpServers": { "mote": { "command": "mote", "args": ["mcp"] } } }
```

**Point out**: the agent calls `scan`, gets the findings split into `work_units` (disjoint sets
of files, one per parallel fixer), and after editing calls `verify`, which re-analyzes only the
edited files and says which findings are fixed, still present or newly introduced.

> **Talking points**
>
> * **One binary, nothing else.** No server, database, JVM or Node.js. It runs where the code is:
>   a laptop, any CI runner, an air-gapped network.
> * **Nothing leaves the machine.** The binary makes no network calls (only `--remote` asks git
>   to clone) and never runs code from the repository, ESLint configurations included.
> * **Fast enough to run on every change.** Hundreds of milliseconds for a mid-size repository,
>   about 1.4 s for 4 million lines; cached re-scans in tens of milliseconds.
> * **New code only.** Pull requests are gated on what they introduce; old debt is reported, never
>   blocking. Baselines cover code bases without usable history.
> * **Keeps what the team has.** ESLint and PMD configurations migrate in one command, and the
>   suppressions already in the code are honored.
> * **Precision first.** 552 rules (Java and JavaScript/TypeScript). Every stable rule that fires
>   on the benchmark repositories has hand-labeled findings, none below 95% precision; new rules
>   report without gating until they earn it.
> * **Fits every pipeline.** SARIF, GitHub checks and comments, GitLab Code Quality, JUnit,
>   Checkstyle, HTML, JSON; an MCP server for AI agents.
> * **Scope.** A code-quality tool, not a security scanner. Version 0.1: free and open source
>   (MIT or Apache-2.0); issues welcome.

> **If something goes wrong**
>
> * `mote: command not found`: `export PATH="$HOME/mote-demo/bin:$PATH"` in this terminal.
> * macOS says the binary "cannot be opened": it was downloaded with a browser. Run
>   `xattr -d com.apple.quarantine ~/mote-demo/bin/mote`, or download with `curl` as above.
> * Linux: ``version `GLIBC_2.3x' not found``: the `-gnu` build needs glibc 2.35 or newer. Use the
>   static `-musl` archive if the release has one for the machine, or another machine.
> * `--base main` fails with an unknown revision: check the branch name with `git branch -a`
>   (Excalidraw's default branch is `master`; Spring PetClinic's is `main`).
> * A scan exits 1: that is the gate failing, not an error (2 is a usage or configuration error,
>   3 an internal one). Use `--no-gate` to look without a verdict.
> * The first scans on Windows are slower than expected: antivirus software is scanning the new
>   cache file; run the scan again.
> * Something odd in the cache: `rm -rf .mote` (or `--no-cache`) and scan again.
> * No network on stage: everything except the downloads and clones works offline; do those
>   beforehand.

## Expected results

Measured on an Intel Core i7-8700K desktop (6 cores / 12 threads, Windows 10, Git Bash) with
local copies of the two repositories at the commits the [benchmark](benchmark.md#corpora) pins;
a fresh clone has moved on, so counts will differ a little, and a laptop is usually somewhat
slower.

| Step | Time | What it shows |
|---|--:|---|
| 1. Excalidraw, first scan after cloning | 0.3–1.1 s | 695 files, 179,078 lines of code, 2,264 findings (18 HIGH) |
| 1. Excalidraw, cached scan | 0.05–0.06 s | `0 analyzed, 695 cached` |
| 1. Excalidraw, full analysis again (`--no-cache`) | 0.1–0.2 s | the same findings; `--timings` shows the phases |
| 2. HIGH findings, HTML report | 0.05–0.3 s each | 18 HIGH; grades maintainability D, reliability C; a 0.5 MB page |
| 3. `init --from eslint` | 0.15 s | 7 rules read, 2 mapped; 44 `eslint-disable` comments found |
| 3. Scan with the migrated config | 0.1–0.4 s | 236 findings suppressed; 15 HIGH left |
| 3. `migrate` (flat config, PMD ruleset) | < 0.1 s each | 5 of 5 rules mapped, 2 presets listed; 3 of 4 rules mapped |
| 4. PR gate, then fixed | 0.1–0.15 s | 2 new findings (1 HIGH) → gate failed, exit 1; after the fix: passed, exit 0 |
| 5. Baseline and the new bug | 0.1 s, 0.03 s | baseline 2.5 MB; gate passes; one new `js/use-isnan` → failed, exit 1 |
| 6. MCP scan call | 0.1–0.2 s | a `scan_id` and the counts, as JSON |
| Spring PetClinic, any scan | 0.04–0.2 s | 50 files, 2,466 lines of code, 20 findings, none HIGH |
| VS Code (optional) | 2.4–3.7 s first, 0.3–0.7 s cached | 14,036 files, 3.3 million lines of code |

## Afterwards

```sh
cd ~ && rm -rf ~/mote-demo
```

More to read: [overview.md](overview.md) (one-page summary), [cli.md](cli.md) (every command),
[github-action.md](github-action.md) and [ci.md](ci.md) (CI setups),
[migration.md](migration.md) (moving from ESLint or PMD).
