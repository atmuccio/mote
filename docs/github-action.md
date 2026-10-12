# Mote GitHub Action

Gate pull requests on **new code**: the action installs `mote`, scans the PR head against its
merge-base, publishes the results where reviewers look, and fails the job when the quality gate
fails. The gate is mote's own exit code (see [`docs/cli.md`](cli.md)), so making the job a
required status check is all it takes to block merges. Nothing depends on GitHub Code Security.

| Where | What | Needs |
|---|---|---|
| Job status | pass/fail = the quality gate (exit code 0/1; 2/3 = error, always fails) | — |
| Check run **Mote** | inline annotations for new findings (HIGH → failure, MEDIUM → warning) | `checks: write` |
| PR comment | one sticky summary per scanned `path`, edited in place on every push ([details](#pr-comment)) | `pull-requests: write` |
| Job summary | the full Markdown report | — |
| Artifact `mote-results` | `results.json` (for `mote gate`), `results.sarif`, `summary.md`, `checks.json` | — |
| Code Scanning (optional) | SARIF upload, category `mote` | `security-events: write` (+ Code Security on private repos) |

> The action lives at **`atmuccio/mote`**. Pin it to a release tag such as `@v0.1.0`: the action
> ref also pins the engine version. `@v0` follows the latest 0.x release, whose configuration and
> results formats may still change.

## Quick start

Add `.github/workflows/mote.yml` (full version: [`examples/workflows/mote.yml`](../examples/workflows/mote.yml)):

```yaml
name: mote
on:
  pull_request:
    types: [opened, synchronize, reopened, labeled, unlabeled]
  push:
    branches: [main]
concurrency:
  group: mote-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
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
      - uses: atmuccio/mote@v0.1.0
        with:
          fail-on-gate: ${{ github.event_name == 'pull_request' }}
```

Why each piece is there:

* **`ref: …head.sha`** — `actions/checkout` defaults to the PR *merge* commit. Mote analyzes the
  PR head so annotation line numbers match the "Files changed" tab. The action warns if you forget.
* **`labeled`/`unlabeled`** — re-runs the gate when the [override label](#override-label) changes.
* **`push` to `main` with `fail-on-gate` off** — on a push there is no base, so every finding counts
  (cli.md: without `--base` the severity condition applies to all findings). Gating pushes would turn
  `main` red for existing debt. The push run still matters: it saves the analysis cache PRs restore,
  and (with `code-scanning`) refreshes the Code Scanning baseline.
* **`concurrency`** — a new push cancels the previous run for the same PR.
* Don't use `pull_request_target`: it runs with a write token on untrusted fork code, and
  `actions/checkout@v7` refuses to check out fork PR code there by default anyway.

### Permissions

| Permission | Used for | Without it |
|---|---|---|
| `contents: read` | checkout, compare API (merge-base) | the action can't run |
| `checks: write` | the **Mote** check run with annotations | falls back to ≤ 10 error + 10 warning workflow annotations |
| `pull-requests: write` | sticky PR comment; reading issue events for the override label | comment skipped (warning); override can't identify the labeler on non-`labeled` events |
| `security-events: write` | only with `code-scanning: true` | SARIF upload fails (non-blocking) |

`metadata: read` (always granted) covers the collaborator-permission lookup for the override label.

## Inputs

| Input | Default | Description |
|---|---|---|
| `mode` | `scan` | `scan`: install, scan, publish, gate. `gate`: re-evaluate the gate from a saved results file (`mote gate`), with the override label and job summary — for the [coverage job](#coverage-on-new-code). `install`: only put `mote` on `PATH`. |
| `version` | *action ref* | `1.4.2`, `v1.4.2` or `latest`. Default: the action ref if it is `vX.Y.Z`; otherwise the workspace version in the action's own `Cargo.toml` (so `@v0`, `@<sha>` and `@main` resolve to the matching engine); else `latest`. |
| `repository` | `atmuccio/mote` | Repository whose releases provide the binary. |
| `build-from-source` | `false` | `cargo install --locked` instead of downloading. Builds the action's own source tree when it matches the version, else `--git https://github.com/<repository> --tag v<version>`. Also the automatic fallback when no release asset exists for the runner. |
| `path` | `.` | Directory to scan (inside the checkout). Also scopes the analysis cache and the PR comment, so monorepo jobs scanning different directories don't overwrite each other. |
| `config` | | Config file (default `<path>/.mote.yaml`). |
| `base` | *auto* | New-code base (branch, tag or SHA; its merge-base with HEAD is used). Default: the PR base branch on `pull_request`, none otherwise. Must be a valid ref name (no leading `-`, spaces, `..`). |
| `results` | `mote-results/results.json` | `mode: gate`: the scan job's `results.json` (from its downloaded artifact). |
| `coverage` | | `mode: gate`: coverage report globs (LCOV, JaCoCo XML, Cobertura XML), one per line. |
| `gate-only` | `coverage` | `mode: gate`: judge only these gate conditions (`mote gate --only`, comma-separated: `severity`, `duplication`, `coverage`, `worsened`). Empty re-evaluates the whole gate. |
| `code-scanning` | `false` | Upload SARIF to Code Scanning with `category: mote`. |
| `sarif-min-severity` | `MEDIUM` | Lowest severity in SARIF. Keep it identical on `main` and PRs. |
| `annotations` | `true` | Publish the Checks API check run. |
| `comment` | `true` | Upsert the sticky PR comment. |
| `job-summary` | `true` | Write the report to the job summary. |
| `fail-on-gate` | `true` | Fail when the gate fails. When `false`, a failed gate publishes the **Mote** check run as `neutral`, titled *Advisory, gate not enforced*. Exit codes 2/3 always fail. |
| `override-label` | `mote-override` | Label that waives a failed gate (empty disables). |
| `override-roles` | `maintain,admin` | Repository roles (`role_name`) allowed to apply it. |
| `cache` | `true` | Restore the analysis cache; save it on default-branch runs. |
| `artifact-name` | `mote-results` | Results artifact name (unique per run: change it in matrix jobs; empty = no upload). |
| `extra-args` | | Extra `mote scan` arguments. Shell-style quotes are honored, nothing is evaluated: `--include 'src/**' --strict`. |
| `github-token` | `${{ github.token }}` | Token for the API calls and the SARIF upload. |

## Outputs

| Output | Description |
|---|---|
| `exit-code` | `0` passed, `1` gate failed, `2` usage/config error, `3` internal error (`scan` and `gate` modes) |
| `gate` | `passed`, `failed`, or `error` — mote's verdict before any override (`scan` and `gate` modes) |
| `new-findings` | new, unsuppressed findings (all unsuppressed findings when there is no base) |
| `results-json` | path of the full results JSON |
| `sarif` | path of the SARIF file |
| `override` | `none`, `honored`, or `ignored`; empty when not evaluated (gate passed, or not a PR) |
| `version` | the installed mote version |

## Make it a required check

Use a repository (or organization) **ruleset**: *Settings → Rules → Rulesets → New branch ruleset*,
target the default branch, enable **Require status checks to pass**, and add the check **`mote`**
(the job; pick the GitHub Actions source). Optionally enable *Require branches to be up to date*.

* Require the **job** (`mote`), not the API-created check run **Mote**: on fork and
  Dependabot PRs the token is read-only, the check run can't be created, and a required-but-missing
  check blocks the PR forever. The job always reports.
* A honored override makes the job pass and sets the **Mote** check run's conclusion to
  `neutral`; so does `fail-on-gate: false` (the title then says *Advisory, gate not enforced*), so a
  green job never sits next to a red check.
* The check name is the job's `name:` (or its id). Renaming the job means updating the ruleset.
* With the [coverage job](#coverage-on-new-code), also require `mote-coverage`.

## Checks API vs Code Scanning

The Checks API path is always on; Code Scanning is an optional display and triage layer, never the gate.

| | Checks API (default) | Code Scanning (`code-scanning: true`) |
|---|---|---|
| Licensing | works everywhere | public repos free; private/internal repos need **GitHub Code Security** (per active committer) |
| Inline in PRs | annotations on any line of a changed file (new findings only; ≤ 200 per run, overflow counted in the summary) | an alert shows inline **only if all its lines are in the PR diff** — e.g. a complexity finding on an unchanged method signature won't |
| Lifecycle | per commit | alerts persist, can be dismissed/triaged, tracked across commits by fingerprint |
| First run on a legacy repo | nothing special | the first default-branch upload opens an alert for **every existing** HIGH/MEDIUM finding; announce it or enable after cut-over |
| Timing | synchronous | processed asynchronously (the action doesn't wait) |
| Limits | 50 annotations per request (the action batches) | 25,000 results and 10 MB gzipped per upload; top 5,000 displayed — hence `sarif-min-severity: MEDIUM` |

Notes on the SARIF upload: `category` is fixed to `mote` on the default branch and PRs so new
alerts are computed against the same configuration; on PRs the SARIF is attached to
`refs/pull/<n>/head` and the head SHA, the commit that was actually analyzed. The upload step is
`continue-on-error` and is skipped on fork PRs. `partialFingerprints.primaryLocationLineHash`
carries Mote's fingerprint, so alert identity follows Mote's new/unchanged matching.

Check runs created with `GITHUB_TOKEN` belong to the GitHub Actions app; GitHub may list the
**Mote** check under another workflow's suite in the Checks tab. The annotations are still on the
PR's "Files changed" view.

## How the new-code base is resolved

On a PR the action asks the compare API for `merge_base_commit` and `ahead_by`
(`GET /repos/{o}/{r}/compare/{base}...{head}`), then fetches just enough history: HEAD is deepened by
`ahead_by + 1` commits (every commit between HEAD and the merge-base is "ahead", so this always reaches
it) and the merge-base commit is fetched if missing. A depth-1 checkout plus a depth-1 merge-base
would be two disconnected commits, and `mote scan --base` computes `merge-base(base, HEAD)`.
The scan then runs with `--base <merge-base-sha>`, which is exactly GitHub's three-dot "Files changed"
comparison. If the compare API is unavailable, the action falls back to fetching the base branch
and full history and asking git. `fetch-depth: 0` on checkout also works (nothing is fetched then).

## Coverage on new code

The scan needs no build, so the `mote` job runs in parallel with your build. The coverage
condition needs test reports, so a small job runs after both and evaluates it from the saved
results — no re-analysis ([`examples/workflows/mote-with-coverage.yml`](../examples/workflows/mote-with-coverage.yml)).
The action's `mode: gate` runs `mote gate --only coverage --results … --coverage …`, writes the
verdict to the job summary, and applies the same [override label](#override-label) check as the scan
job, so one maintainer label unblocks both jobs:

```yaml
  mote-coverage:
    needs: [mote, build]
    if: ${{ !cancelled() && github.event_name == 'pull_request' }}
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7          # only .mote.yaml is needed (gate thresholds)
        with:
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
          sparse-checkout: .mote.yaml
          sparse-checkout-cone-mode: false
      - uses: actions/download-artifact@v8
        with: { name: mote-results, path: mote-results }
      - uses: actions/download-artifact@v8
        with: { name: coverage, path: coverage-reports }
      - uses: atmuccio/mote@v0.1.0
        with:
          mode: gate                          # mote gate --only coverage ...
          results: mote-results/results.json
          coverage: |
            coverage-reports/**/lcov.info
```

* `--only coverage` (the `gate-only` default) judges just the coverage condition: the severity and
  duplication verdict is the `mote` job's, and re-evaluating it here would fail this job a
  second time for the same findings. Set `gate-only: ''` to re-evaluate the whole gate instead.
* `mode: gate` never touches the PR comment (it would overwrite the scan report), the check run,
  SARIF or the cache. It needs `pull-requests: read` for the override lookup, like the scan job.
* Hand-off is by artifact because PR runs can't write caches others can read. Make both `mote`
  and `mote-coverage` required checks.

## Override label

Adding `mote-override` to a PR re-runs the job (`labeled` trigger). GitHub lets anyone with
**triage** or higher apply labels, so the label alone proves nothing. The action:

1. finds who applied the label: the `sender` of a `labeled` event for that label; on any other event
   while the label is present (a new push, reopen, another label) the actor of the most recent
   `labeled` issue event for it;
2. looks up their role with `GET /repos/{o}/{r}/collaborators/{user}/permission` and honors the label
   only if `role_name` is in `override-roles` (`maintain`, `admin` by default; custom role names work);
3. records the decision in the PR comment and job summary: who, their role, when the label was
   applied, and the head SHA it was honored for. The PR history is the audit trail.

An ignored label (wrong role, bot, unknown labeler) leaves the gate failing, and the comment says why.
Removing the label (`unlabeled`) re-runs the gate. The override waives only the gate; errors (exit
2/3) still fail. The [coverage job](#coverage-on-new-code) (`mode: gate`) runs the same check, so
the label waives a failed coverage gate too (recorded in that job's summary).

## PR comment

One sticky comment per scanned directory, edited in place on every run:

* It is found by a hidden marker at the start of the body: `<!-- mote:summary -->` when
  `path` is `.`, `<!-- mote:summary key=<hash of path> -->` otherwise — so monorepo jobs with
  different `path`s each keep their own comment.
* Only a comment **written by the token's own identity** is ever edited: `GET /user`'s login for a
  personal access token; for `GITHUB_TOKEN` (and GitHub App installation tokens, where `GET /user`
  fails) comments by `github-actions[bot]` or another GitHub App bot. A human who pastes the report,
  marker included, keeps their comment; Mote posts its own.
* If editing fails (403, or the comment was deleted meanwhile), a new comment is posted instead of
  dropping the report.

## More reports in the artifact

The action writes `results.json`, `results.sarif`, `summary.md` and `checks.json` to
`${{ runner.temp }}/mote-results` and uploads that directory as the results artifact. Other
reports can go into it with `extra-args`; for example the self-contained HTML report:

```yaml
      - uses: atmuccio/mote@v0.1.0
        with:
          extra-args: --html ${{ runner.temp }}/mote-results/report.html
```

On Windows runners `runner.temp` contains backslashes, which the `extra-args` splitter treats as
escapes: write the report into the workspace instead (`--html mote-report.html`) and upload
it with `actions/upload-artifact`.
The GitLab, Checkstyle and JUnit reports (`--gitlab-codequality`, `--checkstyle`, `--junit`) are
for other CI systems; see [ci.md](ci.md).

## Fork and Dependabot pull requests

`GITHUB_TOKEN` is read-only for PRs from forks and for Dependabot PRs, whatever `permissions:` says.

| Feature | Behavior |
|---|---|
| Scan and gate | work normally; the job status is still the gate |
| Check run | creation returns 403; falls back to ≤ 10 error + 10 warning workflow-command annotations |
| PR comment | skipped (fork PRs are detected up front; Dependabot's 403 is handled) |
| Code Scanning | skipped on forks |
| Job summary, artifact | work normally: the full report is always in the job summary |
| Override label | works (the lookups only read) |

Never "fix" this with `pull_request_target`.

## Caching

The analysis cache (per-file results keyed by content, config and engine version) makes warm PR
scans take seconds.

* Key `mote-<runner.os>-<version>-<scope>-<github.sha>`, restore prefix
  `mote-<runner.os>-<version>-<scope>-`, where `<scope>` hashes the `path` input (normalized:
  `./a/` = `a`) and the config file's contents. Jobs scanning different directories of a monorepo,
  or with different configs, never restore or overwrite each other's cache; a config change starts
  a new cache lineage.
* **Saved only on default-branch runs** (push, schedule or dispatch on the default branch), after the
  scan. PR runs can restore caches from their base and default branches, but caches they wrote would
  be scoped to `refs/pull/<n>/merge` and useless to everyone else.
* Until `main` has had one run, PRs scan cold (still fast). A new engine version starts a new cache.
* `cache: false` skips restore/save; mote still caches within the run.
* GitHub evicts caches unused for 7 days and caps each repository at 10 GB (LRU).

## Installation details

* Release assets, verified before use: `mote-<version>-<target>.tar.gz` (`.zip` on Windows;
  `<version>` without the leading `v`) plus `<archive>.sha256`, for `x86_64`/`aarch64` Linux
  (glibc ≥ 2.35), macOS and Windows (Windows ARM64 is a best-effort asset). A checksum mismatch
  always fails; there is no fallback.
* The binary is kept in the runner tool cache (`$RUNNER_TOOL_CACHE/mote/<version>/<arch>`),
  which persists on self-hosted runners.
* Private release repository: set `github-token` to a token that can read it (`gh release download`
  is used when the plain download fails).
* GitHub Enterprise Server: API calls follow `GITHUB_SERVER_URL`/`GITHUB_API_URL`. `gh` only sends
  a token to an enterprise host from `GH_ENTERPRISE_TOKEN`, so the action exports `GH_HOST` and
  `GH_ENTERPRISE_TOKEN` (= `github-token`) there unless the workflow sets them. Point `repository`
  at a mirror that carries the release assets, or use `build-from-source`.
* Without `gh`, the API calls use `curl`; the token is passed on stdin (`curl --config -`), never
  on the command line where other processes could read it.
* Runners: GitHub-hosted Ubuntu, macOS and Windows images work out of the box. Self-hosted runners
  need `bash`, `git`, `jq`, `curl`, `tar` (plus `unzip`, `7z` or PowerShell on Windows); `gh` is
  optional (curl is the fallback). `cargo` is needed only for source builds.
* **Windows:** every step runs under Git Bash (`shell: bash`). Paths are converted with `cygpath`, and
  `jq` runs with `-b` so it doesn't emit CRLF. Windows *container* jobs are not supported. The
  scripts are also exercised under macOS's `/bin/bash` 3.2.

## Pre-commit hook

[`examples/hooks/pre-commit`](../examples/hooks/pre-commit) scans what is about to be committed and
blocks the commit only if it fails the gate:

```sh
mote scan --staged --base HEAD --new-only
```

Install it with `cp examples/hooks/pre-commit .git/hooks/ && chmod +x .git/hooks/pre-commit`, or
share it via `git config core.hooksPath .githooks`. `git commit --no-verify` bypasses it once; a
missing or failing `mote` doesn't block commits unless `MOTE_HOOK_STRICT=1`.

With the [pre-commit](https://pre-commit.com) framework, as a local hook:

```yaml
repos:
  - repo: local
    hooks:
      - id: mote
        name: mote
        entry: mote scan --staged --base HEAD --new-only
        language: system
        pass_filenames: false
```

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| Warning "Checked-out commit … is not the PR head" | Add `ref: ${{ github.event.pull_request.head.sha \|\| github.sha }}` to `actions/checkout`. |
| "Resource not accessible by integration" (403) | Missing `checks: write` / `pull-requests: write` in `permissions:`, or a fork/Dependabot PR (expected; see above). |
| Annotations missing on some findings | Only **new** findings are annotated, LOW is summary-only, and at most 200 per run (the summary counts the rest). Annotation and SARIF paths are relative to the repository root even when `path:` scans a subdirectory; an older mote that reports them relative to `path` gets annotations on the wrong files. |
| `main` is red | Use `fail-on-gate: ${{ github.event_name == 'pull_request' }}`; without a base every finding counts. |
| "Merge-base … is not connected to HEAD" | Unusual history (e.g. criss-cross merges). Use `fetch-depth: 0` on checkout. |
| Exit code 2 | Usage or configuration error — run `mote config validate` locally. |
| Exit code 3 | Internal error; the log and the `mote-results` artifact have details. Please report it. |
| "No release asset … building from source" (slow) | The version has no release for this OS/arch. Pin `version` to a published release, or use `build-from-source: true` deliberately. |
| "Checksum mismatch" | The download is corrupt or tampered with. Re-run; if it persists, report it. It is never bypassed. |
| Artifact upload conflict in a matrix | Give each leg its own `artifact-name`. |
| Two Mote comments | Expected once per `path` (monorepo jobs). Otherwise: the marker must stay at the very start of the comment (don't edit it by hand), and only comments by the token's identity are reused — switching `github-token` between a PAT and `GITHUB_TOKEN` starts a new one. Delete the stale one. |
| `Invalid base` | `base` must be a branch, tag or SHA (a valid git ref name, not starting with `-`). |
| `Could not parse extra-args` | Unbalanced quotes in `extra-args`. The step fails rather than scanning without your arguments. |
| API calls fail with 401/403 on GHES | Check `GITHUB_SERVER_URL`; a `GH_ENTERPRISE_TOKEN`/`GH_HOST` set by the workflow wins over `github-token`. |
| PRs never hit the cache | `main` needs a completed run first; caches expire after 7 days unused. |
| Code Scanning upload fails on a private repo | GitHub Code Security isn't enabled. Turn `code-scanning` off: annotations, summary and comment don't need it. |

## Maintaining the action

* All logic lives in `scripts/*.sh`; `action.yml` only wires inputs to them via `env:`.
* Offline tests: `bash scripts/tests/run.sh` runs every script against a fake mote
  (`scripts/tests/fake-mote`) with `MOTE_DRY_RUN=1`, which logs the GitHub API calls instead
  of making them and answers from fixtures (annotation chunking, sticky-comment upsert and
  authorship, 403 handling, override decisions in both modes, advisory check runs, exit codes,
  checksum verification, merge-base on a shallow clone, cache scope, GHES/curl token handling,
  input validation). CI runs them on Ubuntu, macOS and Windows, plus shellcheck and actionlint.
* Untrusted input (branch names, labels, file paths and messages from a PR): every `inputs.*` and
  step output reaches the scripts through `env:`, never `${{ }}` inside `run:`; API path segments
  are URL-encoded; git and mote operands can't be read as options; mote's log output is
  fenced with `::stop-commands::` so a crafted file name can't issue workflow commands; fallback
  annotations are escaped in jq.
* Releases: push a tag `vX.Y.Z` equal to the workspace version. `.github/workflows/release.yml`
  builds the assets above, publishes the release, and moves the major tag (`v0` for 0.x releases)
  used by `@v0`.
  The asset names are a contract with `scripts/install.sh`; change both together.
