# Mote CLI

```
mote <COMMAND>

Commands:
  scan      Analyze code (working tree, a git revision, staged changes, or a remote repo)
  gate      Re-evaluate the quality gate from a saved results file (e.g. adding coverage later)
  compare   Compare two saved results files: added, fixed and changed findings, measure deltas
  rules     List rules or explain one
  config    Validate, explain, or print the JSON Schema of .mote.yaml or of the results
  init      Write a starter .mote.yaml (proposed excludes and compat sources; --from merges in a PMD or ESLint configuration)
  triage    Record, list and prune triage decisions (accepted findings, false positives)
  migrate   Propose a .mote.yaml from a PMD ruleset or an ESLint configuration (prints it)
  mcp       Run the MCP server on stdio (for AI agents)
  version   Print version information
```

## `mote scan`

```
mote scan [PATH]                     Scan the working tree rooted at PATH (default: .)

Source (pick one; default = working tree):
  --rev <REV>                        Scan a git revision straight from the object DB (no checkout)
  --staged                           Scan the staged (index) content — for pre-commit hooks
  --remote <URL>                     Shallow-clone URL into the cache and scan it (use --rev for a ref)

New code:
  --base <REV>                       Compare against merge-base(REV, head): classify findings as
                                     new / unchanged / worsened; gate applies to new code
  --changed-only                     Only analyze and report files changed vs --base
  --baseline <FILE>                  Compare with a saved results file (`scan --json`) instead of a
                                     git base: findings not in it are new (see Baselines)

Selection:
  --include <GLOB>...                Only these paths (repeatable; a pattern matching no file is
                                     a usage error, exit 2)
  --config <FILE>                    Config file (default: <root>/.mote.yaml if present)
  --coverage <GLOB>...               Coverage reports (LCOV, JaCoCo XML, Cobertura XML)
  --triage <FILE>                    Triage file (default: triage.file, .mote-triage.yaml)

Output:
  --format <FMT>                     text (default) | json | jsonl | sarif | markdown | github |
                                     gitlab | checkstyle | junit | html
  -o, --output <FILE>                Write --format output to FILE instead of stdout
  --json <FILE>                      Also write full results JSON (input for `mote gate`)
  --jsonl <FILE>                     Also write one finding per line
  --sarif <FILE>                     Also write SARIF 2.1.0 (at most 25,000 results; the most severe are kept)
  --sarif-min-severity <SEV>         Lowest severity included in SARIF (default: LOW)
  --markdown <FILE>                  Also write the Markdown summary (job summary / PR comment)
  --github-checks <FILE>             Also write a GitHub Checks API payload (annotations for new findings)
  --gitlab-codequality <FILE>        Also write a GitLab Code Quality report
  --checkstyle <FILE>                Also write Checkstyle XML
  --junit <FILE>                     Also write JUnit XML (the gate and one test suite per rule)
  --html <FILE>                      Also write a self-contained HTML report (at most 25,000 findings listed)
  --summary-json <FILE>              Also write a one-line metrics record to archive for trends
  --bitbucket-insights <DIR>         Also write a Bitbucket Code Insights report and annotations
                                     (DIR/report.json, DIR/annotations.json)
  --new-only                         Show only new findings (text/jsonl/markdown/junit)
  --min-severity <SEV>               Hide findings below SEV in text/jsonl/markdown/junit output
  --show-suppressed                  Include suppressed findings in text output (only; JSONL and
                                     Markdown never list them, JSON and SARIF always carry them)
  --color <auto|always|never>

Behavior:
  --no-gate                          Never fail on the gate (exit 0 unless an error occurs)
  --strict                           Exit 3 if any file could not be analyzed
  --no-cache                         Disable the analysis cache
  --cache-dir <DIR>                  Cache location (default: <root>/.mote/cache, or $MOTE_CACHE_DIR)
  --threads <N>                      Worker threads (default: all cores)
  --timings                          Print phase and per-language timings to stderr
  -q, --quiet / -v, --verbose
```

Working-tree scans read `.git/index` and walk the tree in-process (no git processes); unusual
repositories fall back to the git command line automatically, and `MOTE_DISCOVERY=git` forces
that fallback. With `--timings`, `discovery` includes opening the cache, which runs concurrently.

The configuration is the scan root's `.mote.yaml` (or `--config`), and the cache goes below
the scan root too. In a monorepo, `mote scan lib/core` therefore does not read the
repository's top-level `.mote.yaml` (a note on stderr says so when `lib/core` has none):
scan from the top level with `--include 'lib/core/**'` to apply it, give the project its own
`.mote.yaml`, or pass `--config` (whose globs are then relative to `lib/core`).

`--base origin/main` needs that remote branch in the clone: a CI checkout that is shallow or
fetched a single branch does not have it (see [ci.md](ci.md) for each CI system), and a local
clone may only have `main`. The error names the local branch when one of that name exists.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Scan completed; gate passed (or no gate) |
| 1 | Scan completed; gate failed |
| 2 | Usage or configuration error |
| 3 | Internal error — the scan could not complete (or `--strict` and a file failed) |

The gate is evaluated on every scan. With `--base`, conditions apply to new code; without it, only the
severity condition applies (to all findings) and ratio conditions are skipped. With `--baseline`,
the findings conditions apply to findings not in the baseline, and the conditions that need a
line diff are skipped (see [Baselines](#baselines)).

### New code

`--base REV` compares the scan with `merge-base(REV, head)`, like GitHub's "Files changed" view of
a pull request, and gives every finding a status:

* Findings of files the change did not touch are `unchanged` without further work (analysis is
  per file). Findings of added and untracked files are `new`, unless their code moved there
  (step 5 below).
* A changed file is also analyzed as it was at the merge base, and its findings are paired with
  the base's, one to one, in this order (messages are compared with digits ignored):
  1. on a line the diff did not touch, the same rule and message on the base line it corresponds
     to — the same code, whatever its fingerprint says (an identical line added above it, a
     renamed method);
  2. the same fingerprint (rule, enclosing symbol, line text, occurrence);
  3. on an untouched line, the same rule, line text and message, whatever the symbol;
  4. the same rule, symbol and message when the two are plausibly the same code: an untouched
     line, the same line text, or both measured (complexity and other metrics, once per symbol);
  5. moved code: a finding on an added or changed line, and a base finding on a removed or changed
     line of this or another changed or deleted file, with the same rule, message, line text and
     lines around it (a method extracted from another one, code moved to a new file).
* Paired findings are `unchanged`, or `worsened` when their measured value grew; the others are
  `new`. Unpaired, unsuppressed base findings (deleted files included) are counted as fixed
  (`new_code.measures.fixed_findings`).
* Duplicated blocks are `new` when they overlap an added or changed line.

[benchmark.md, Part 5](benchmark.md#part-5-pr-replay) measures this on 500 real commits.

### Baselines

`--baseline results.json` compares the scan with a results file saved earlier (`mote scan
--json results.json`) instead of a git base — for a first rollout on a code base with old debt,
for CI without the git history, or to keep a reference scan of a release:

```sh
mote scan --no-gate --json mote-baseline.json              # once, e.g. on the default branch
mote scan --baseline mote-baseline.json                    # later: only findings not in it gate
```

* Findings are paired with the baseline's without a diff: same path and fingerprint (line moves
  keep the fingerprint), then the same fingerprint in a renamed or moved file (the old file is
  gone and the new one did not exist), then — for measured findings such as complexity — the same
  path, rule, symbol and message. Paired findings are `unchanged` (`worsened` when their value
  grew), the rest `new`; unpaired, unsuppressed baseline findings in the scanned paths are counted
  as fixed.
* The results carry `new_code` with `baseline` set (`file`, `commit` the baseline recorded,
  `engine_version`, `findings`, `matched`) and no line ranges. The gate's findings and worsened
  conditions apply as with `--base`; duplication and coverage on new code and the new-code grade
  conditions are skipped ("compared with a baseline results file, which has no line diff"), and no
  new-code grades are computed. `mote gate --results` re-evaluates such a result the same way.
* A baseline written by another engine version or with another rule set gets a warning: rule
  changes show up as new (or fixed) findings. Refresh the baseline when you upgrade.
* The baseline is the complete results file, about 1 KB per finding (17 MB for a 270,000-line
  Java code base with 19,000 findings): commit it, or keep it as a CI artifact of the default
  branch. `--new-only` lists just the findings that are not in it.
* `--baseline` excludes `--base` and `--changed-only`.

### Triage decisions

Some findings are right but will stay (`accepted`), some are wrong (`false-positive`). Instead of
a comment in the code, record the decision in the repository's triage file
(`.mote-triage.yaml` at the scan root; `triage.file` in `.mote.yaml` or `--triage FILE`
choose another):

```yaml
version: 1
entries:
  - fingerprint: 0f3c2d6e8a9b1c4d5e6f708192a3b4c5
    rule: "java/empty-catch-block"
    path: "src/main/java/app/Probe.java"
    status: false-positive          # or: accepted
    reason: "The exception is the probe's expected outcome"
    by: "alice"                     # optional
    date: "2026-09-28"              # optional
```

* A scan marks the matching finding suppressed with `kind` = the status and the reason as
  justification: it never gates, does not count towards effort or grades, is listed in JSON with
  `suppressed`, and in SARIF with an `external` suppression. A finding the source already
  suppresses keeps its in-source suppression.
* **Matching.** An entry names a finding by its **fingerprint** (rule, enclosing symbol and the
  whitespace-normalized text of the finding's line — no line number, no path), which survives
  lines moving and, when the entry's file is gone, a rename or move (a finding with the same
  fingerprint and rule in another file; same file name preferred). An entry may use the finding
  `id` instead (path + fingerprint): exact only, it does not follow renames.
* **Stale entries.** An entry goes stale when the finding's line is edited, its class or function
  is renamed, an identical finding is added before it in the same symbol, the rule is switched
  off, or the finding is fixed. A scan that analyzed the entry's file (or a full scan in which the
  file no longer exists) reports each stale entry as a `mote/unused-suppression` finding on
  the triage file, at the entry's line, and lists it in the results (`triage.stale`). Review the
  finding again (it may be a new one) and record a new decision, or prune.
* A malformed triage file is a usage error (exit 2). A scan of a revision (`--rev`) reads the
  triage file of that revision.

```
mote triage add <FINDING> --status accepted|false-positive --reason TEXT --results FILE
                    [--by NAME] [--date YYYY-MM-DD] [--file FILE] [--config FILE]
mote triage list [--results FILE] [--format text|json] [--file FILE] [--config FILE]
mote triage prune --results FILE [--dry-run] [--file FILE] [--config FILE]
```

* `add` looks the finding up in a results file (`scan --json`) by its `id` (12 hex digits, as in
  JSON, JSONL and SARIF `properties.id`) or its fingerprint (32), and records its fingerprint,
  rule and path with the decision (today's date in UTC unless `--date`). A decision about the
  same finding replaces the earlier one. Findings of `mote/*` rules cannot be triaged.
* `list` prints the entries; with `--results`, each is marked `matched`, `stale` (the scan said
  so) or `not covered` (the scan did not look at its file).
* `prune` removes the entries the results list as stale (`--dry-run` only prints them).
* The file is rewritten whole: entries sorted by path, rule and fingerprint, one key per line,
  strings double-quoted, so the same decisions always give the same bytes and diffs stay small.
  Comments other than the generated header are not kept. Run the commands from the scan root.

### Output formats

* **text** — grouped by file, colored on a TTY, then a summary and the gate verdict (and a line
  counting the findings of incubating rules, which are reported but never gate).
* **json** — the complete `ScanResult` (`format_version`, findings, files, measures with estimated
  effort, grades, duplication, new-code summary (with `baseline` for `--baseline` scans), new-lines
  map, gate, triage report (`triage`: file, entries, matched, stale entries), stats; see
  [Results format](#results-format)). Input for `mote gate --results`, `mote compare`,
  `--baseline` and `mote triage`.
* **jsonl** — one compact finding per line: `{"id","rule","severity","quality","path","line","col",
  "end_line","end_col","message","status","symbol"}`. Suppressed findings are never included.
* **sarif** — SARIF 2.1.0; `partialFingerprints.primaryLocationLineHash` = Mote fingerprint;
  `baselineState` from new-code status (`new` / `unchanged` / `updated`; the result keeps only
  counts of fixed findings, so no `absent` entries); suppressed findings carry `suppressions`
  (`inSource` for comments and annotations, `external` for triage decisions).
  Artifact URIs are relative to `%SRCROOT%` = the repository top level, given as an absolute
  `file:` URI in `run.originalUriBaseIds` (the scanned directory for non-git scans). At most
  25,000 results (GitHub's limit per run): the most severe are kept, and `run.properties`
  (`totalResults`, `omittedResults`, `note`) plus a warning on stderr record the rest.
  `invocations[0].executionSuccessful` is `false` when a file could not be analyzed
  (`mote/internal-error` findings or error diagnostics).
* **markdown** — summary for `$GITHUB_STEP_SUMMARY` and the sticky PR comment; begins with the
  marker `<!-- mote:summary -->`. Finding messages quote scanned code, so they are escaped for
  GitHub-flavored Markdown: code spans are re-fenced (a fence longer than any backtick run inside),
  and outside them `<`, `>`, `&` become entities and `|`, backslashes, backticks and emphasis/link
  punctuation are backslash-escaped; line breaks become spaces.
* **github** — `{ "name", "conclusion", "title", "summary", "annotations": [...] }` for the Checks API.
  Annotations: new findings only when `--base` is given (else all), HIGH → `failure`,
  MEDIUM → `warning`, LOW omitted; capped at 200 (overflow noted in `summary`).
* **gitlab** (`--gitlab-codequality`) — a GitLab Code Quality report for
  `artifacts:reports:codequality`: a JSON array with one issue per line, `{"type": "issue",
  "check_name": <rule id>, "description": <message>, "categories", "severity", "fingerprint",
  "location": {"path", "lines": {"begin", "end"}}}`. Every unsuppressed finding is listed, whatever
  `--new-only` says: GitLab compares the merge request's report with the target branch's report
  itself. Severity HIGH → `critical`, MEDIUM → `major`, LOW → `minor`. The fingerprint is the
  Mote fingerprint, which leaves the path out, so a finding keeps it when its line moves or its
  file is renamed; when two files hold the same finding on the same line text (a symbol-less
  `// TODO`), those fingerprints get the finding id appended (`<fingerprint>:<id>`) to stay unique
  within the report — still stable across line shifts, not across renames. Categories come from
  the rule's quality and tags (`Bug Risk`, `Complexity`, `Duplication`, `Performance`,
  `Security`, `Style`, `Compatibility`, else `Clarity`).
* **checkstyle** (`--checkstyle`) — Checkstyle XML (`<checkstyle version="4.3"><file name><error
  line column severity message source/>`), read by CI dashboards such as the Jenkins Warnings
  Next Generation plugin. Every unsuppressed finding, grouped by file; HIGH → `error`, MEDIUM →
  `warning`, LOW → `info`; `source` is the rule id. `version` is the format version consumers
  expect, not the Mote version.
* **junit** (`--junit`) — JUnit XML for CI test views (Jenkins, GitLab, Azure Pipelines): a
  `Mote quality gate` test suite with one test case per condition (failed when the condition
  failed, skipped with the reason when it was not evaluated), then one test suite per rule, named
  by the rule id, with a failed test case per finding (named `path:line:col`; the failure message
  is the finding message, its type the severity, its text the rule, severity, quality, status and
  location). Follows `--new-only` and `--min-severity`; suppressed findings never appear. At most
  5,000 finding test cases (the most severe are kept); the rest are counted in a final skipped
  test case. No timestamps or durations (`time="0"`).

* **html** (`--html`) — one self-contained page to publish as a CI artifact or open locally: the
  stylesheet and script are inline, the report data is embedded as JSON, nothing is loaded from
  the network and no source code is included. Views: an **overview** (quality gate and its
  conditions, grades, measures, findings by severity, quality and rule, new code against
  overall, main against test code, languages, diagnostics), the **files** as a directory tree
  with each file's findings, a filterable **findings** list (severity, new only, rule, text), and
  the **rules** with their descriptions (shown as plain text) and parameters. Light and dark
  follow the system setting; the layout works at phone width. At most 25,000 findings are listed
  (the most severe, like SARIF); the page and a warning on stderr say how many were left out,
  and every count and measure still covers all findings. Suppressed findings are counted, not
  listed. Paths are relative to the scan root, like text and JSON. The page is safe to open
  whatever the scanned code contains: messages and paths are embedded as JSON with `<`, `>` and
  `&` escaped, the script builds the page from text (never from HTML strings), and a
  Content-Security-Policy meta tag allows only that script and stylesheet (by their SHA-256
  hashes) and no network connections. Size: a scan of VS Code (3.3M lines of code, 14,036
  files, 35,808 findings) gives a 3.9 MB page listing 25,000 findings.

* **Bitbucket Code Insights** (`--bitbucket-insights DIR`) — two files for the public Bitbucket
  Cloud Code Insights API, which the CI job uploads (see [ci.md](ci.md#bitbucket-pipelines)):
  `DIR/report.json`, the report (`title`, `details`, `external_id: "mote"`, `reporter`,
  `report_type: "BUG"`, `result` = the gate, `PASSED` or `FAILED`; `details` names the failed
  conditions; `data` holds up to 10 typed values: findings and HIGH findings — new ones with a
  base or baseline — the two grades, duplication and coverage, new code's when there is a line
  diff, estimated effort, lines of code), and `DIR/annotations.json`, an array of annotations
  (`external_id` = finding id, `annotation_type` `BUG` for reliability rules, `VULNERABILITY` for
  rules tagged `security`, else `CODE_SMELL`; `severity` HIGH / MEDIUM / LOW; `path` relative to
  the repository root; `line`; `summary` = the message, at most 450 bytes; `details` = rule,
  severity, quality, status and rule name). Annotations cover new and worsened findings with a
  base or baseline and every unsuppressed finding otherwise, at most 1,000 (the API's limit per
  report; the most severe are kept and `details` says how many were left out). The API takes at
  most 100 annotations per request, so the upload splits the array.
* **Summary record** (`--summary-json FILE`) — one line of JSON with the numbers a trend chart
  needs, meant to be archived per run (or appended to a JSONL history): `summary_version` (1),
  `date` (RFC 3339 UTC, now; `SOURCE_DATE_EPOCH` overrides it for reproducible runs),
  `engine_version`, `format_version`, `source` and `revision` (the scanned commit, else the
  requested revision), `gate` (`passed`, the names of the `failed` conditions), `measures`
  (files, lines, lines of code and test lines of code, comment lines, functions, classes, both
  complexities, duplicated lines and density, overall coverage, estimated effort), `grades`
  (grade and value of each, overall and on new code, and `new_not_rated` when a change is too
  small to grade, see [Grades](#grades)), `findings` (unsuppressed: total, by severity, by
  quality, incubating; and the suppressed count), `new_code` (with a base or
  baseline: `base`, `baseline`, changed files, new lines, new findings by severity, worsened,
  fixed, duplication and coverage on new code, new effort; `null` otherwise) and `triage`
  (entries, matched, stale; `null` without a triage file). Fields are only added within a
  `summary_version`.

Checkstyle and JUnit escape every message and path for XML: markup characters become entities,
tab / line feed / carriage return become character references, and control characters XML 1.0
cannot carry become U+FFFD. None of these formats carries suppressed findings (SARIF and JSON do).

### Results format

The results JSON (`--json`, `--format json`, `mote gate --format json`) starts with
`format_version` (currently `1`); `mote config schema --results` prints its JSON Schema
(draft 2020-12), from which consumers can generate types or validate files. What a consumer can
rely on:

* **Within one `format_version`, fields are only added.** No field is removed, renamed, or given
  another type or meaning, and no enum loses a value. Consumers should ignore fields they don't
  know. New enum values (a new skip reason, a new gate condition name) may appear; treat unknown
  ones generically.
* **A breaking change bumps `format_version`.** Results files without the field were written
  before it existed and read back as version 0; `mote gate --results` still accepts them
  (missing newer fields take their defaults, e.g. no effort, so grades computed from such a file
  are all A) and writes its JSON in the current format.
* **Everything except `stats` is deterministic**: the same input gives the same bytes regardless
  of thread count or cache state. `stats` holds timings and cache counters.
* **Grades carry their own version** (`grades.grades_version`) and the band limits used
  (`grades.bands`): a change of the grading method or of the default bands bumps
  `grades_version`, not `format_version`, since the fields stay the same. New-code grades are
  `null` both without a base and for a change too small to grade; `grades.new_not_rated`, present
  only in the second case, tells them apart (see [Grades](#grades)).
* The other formats follow their own specifications (SARIF 2.1.0, GitLab Code Quality,
  Checkstyle, JUnit); the JSONL line shape is fixed (see above). The text, Markdown and HTML
  outputs are for people and may change in any release.

### Measures and estimated effort

The results JSON (`measures`) carries, besides lines, functions, duplication and coverage:

* **Complexity** — `cognitive_complexity` and `cyclomatic_complexity`: the sums over all
  functions. The text and Markdown summaries show them next to the estimated effort.
* **Estimated effort** — `effort_minutes` `{total, high, medium, low, reliability,
  maintainability}`: the time fixing the findings would roughly take. Each rule declares a
  remediation time for one finding (`remediation_minutes` in `mote rules --format json`); a
  threshold rule's finding (a rule with a `threshold`, `max` or `max_lines` parameter whose
  finding carries the measured value) costs that time multiplied by how far the value exceeds
  the limit in force for the file, `value / limit`, between 1× and 10×: a function with cognitive
  complexity 45 against a limit of 15 counts three times the base effort. Each finding is rounded
  to whole minutes. The summaries show hours and label them *estimated*: they are a relative
  yardstick for comparing code bases, directories and changes, not a schedule.
* **Which findings count** — effort (and the [grades](#grades)) count the findings that can gate:
  unsuppressed findings of stable rules. Suppressed findings never count. Incubating rules
  report findings, but like the gate they don't feed effort or grades until the rule is promoted,
  so a rule still on probation cannot move the numbers. `reliability_findings` counts those
  findings of reliability rules by severity (the reliability grade's input).
* **Main and test code** — `measures.main` and `measures.test` hold the same measures for
  non-test and test files (`total` covers both, as before); the summaries show the lines of test
  code.
* **Directories** — `measures.directories`: per-directory rollups (files, lines, lines of code,
  complexity, duplicated lines, findings by severity, effort) for every directory down to
  `metrics.depth` levels below the scan root (default 2; 0 turns them off), sorted by path. At
  most 1,000 are listed (the largest by lines of code, so a listed directory's parents are listed
  too); `directories_omitted` counts the rest. Files directly in the scan root belong to no
  directory. Each file also has its own `effort_minutes`.
* **New code** — `new_code.measures.new_effort_minutes` and `new_reliability_findings` cover the
  findings whose status is `new` (like the gate's new-findings condition; worsened findings are
  counted separately in `worsened_findings`).

```yaml
metrics:
  depth: 2            # directory levels rolled up in measures.directories (0-10; 0 = none)
```

### Grades

Every scan grades the code from **A** (best) to **E** on two axes (`grades` in the results JSON,
shown in the text and Markdown summaries):

* **Maintainability** — the estimated hours to fix the maintainability findings per 1,000 lines
  of code. Effort already weighs findings by how long they take to fix, and dividing by size lets
  a 5,000-line service and a 500,000-line monolith be compared on the same scale.
* **Reliability** — weighted reliability findings per 10,000 lines of code: a HIGH finding (a
  near-certain bug) weighs 1, a MEDIUM one 0.25, a LOW one 0. Counting likely bugs rather than
  minutes keeps one serious bug from being diluted by the time estimate of trivial ones; LOW
  reliability findings are nits and don't move the grade.

Only findings that can gate count (unsuppressed, stable rules), as for the effort. With `--base`
the same two grades are computed for the new code (`new_maintainability`, `new_reliability`):
over the new findings and the new lines (added or changed lines, blank and comment lines
included — the line count new-code analysis keeps — so new-code values run a little lower than
a lines-of-code basis would give). Each grade carries the value it was given for (`value`,
rounded to two decimals; the grade always matches the rounded value), the lines it was taken
over, the settings used (`bands`: the band limits and `min_new_lines`) and `grades_version`
(bumped whenever the method or the default bands change).

**Small changes are not rated.** Below `grades.min_new_lines` new lines (100 by default) the new
code gets no grades: over a small change one finding is a density no band was made for — a
single HIGH reliability finding in 11 new lines would be an E at 909 weighted findings per
10,000 lines, and one estimated hour of fixes in a 38-line change an E for maintainability. The
overall grades are computed as always, and the gate's verdict on new findings (`gate.fail_on`)
is not affected. The outputs say so instead of showing a grade:

* text — `on new code: not rated (only 11 new lines; grades need at least 100)`;
* Markdown (job summary, PR comment, the GitHub Checks summary) — `_not rated_` in the new-code
  column of both grade rows and a note under the table; HTML — a "Grades, new code: not rated"
  card;
* results JSON — `grades.new_maintainability` and `grades.new_reliability` are `null` (as
  without a base) and `grades.new_not_rated` says why: `{"reason": "too_few_new_lines",
  "new_lines": 11, "min_new_lines": 100}`. The field is only present when the new code is not
  rated (an addition within `format_version` 1; a reason other than `too_few_new_lines` may be
  added later, so treat unknown ones alike). The summary record (`--summary-json`) carries it
  under `grades` the same way.

SARIF, JUnit, Checkstyle, GitLab and Bitbucket reports carry no new-code grades. Set
`grades.min_new_lines: 0` to grade every change; `mote gate --results` applies the minimum of
its own configuration. This minimum is the grades' own: `gate.min_new_lines` (20 by default)
only decides from how many new lines the gate's duplication and coverage conditions apply.

The default bands (the upper limit of A, B, C and D; anything at or above the last is E):

| Grade | Maintainability: estimated hours per 1,000 lines | Reliability: weighted findings per 10,000 lines |
|:-:|---|---|
| A | below 1 | below 0.5 |
| B | below 2 | below 1 |
| C | below 3 | below 2 |
| D | below 4 | below 4 |
| E | 4 or more | 4 or more |

Why these: they were set by scanning 25 open-source Java and TypeScript repositories with the
default rules (see [benchmark.md](benchmark.md#grades)) and asking for a spread in which a clean
small project gets an A, most mature code bases land in the middle, and only the outliers get an
E. Reliability densities vary a lot between projects (from none to 10 per 10,000 lines), so its
bands double from one grade to the next: each step down means twice as many likely bugs for the
same amount of code. Maintainability densities vary much less (most repositories need between 2
and 5 estimated hours per 1,000 lines), and doubling bands would put nearly all of them in one
grade; there each grade step is one more estimated hour per 1,000 lines. Neither scale is a
verdict on a project: they compare code bases, directories and changes against each other.

Adjust the bands per project (four increasing limits each), and the size from which new code is
graded:

```yaml
grades:
  maintainability: [1, 2, 3, 4]   # upper limits of A, B, C, D: estimated hours per 1,000 lines of code
  reliability: [0.5, 1, 2, 4]     # upper limits of A, B, C, D: weighted reliability findings per 10,000 lines
  min_new_lines: 100              # new code with fewer new lines is not rated (0: grade every change)
```

Grades are informational. To gate on them, set the worst grade allowed ("no worse than"):

```yaml
gate:
  new_maintainability_grade: B      # new code: B or better
  new_reliability_grade: A
  maintainability_grade: D          # the whole code base (applies with or without --base)
  reliability_grade: null           # null (the default): not a condition
```

The new-code conditions need `--base` and are skipped (they pass) while the new code is not
rated, below `grades.min_new_lines` new lines (see above; the duplication and coverage
conditions follow `gate.min_new_lines` instead). In the results a grade condition's `actual` and
`threshold` are ranks, A = 1 to E = 5 (it passes when `actual <= threshold`); the text, Markdown
and JUnit outputs show letters (`E (C or better)`). `mote gate --results` grades the saved
measures again with the bands and `grades.min_new_lines` of its own configuration.

### Skipped files

Besides `exclude` globs and files larger than `max_file_size`, a scan skips with
`skip_generated: true` (the default):

* **generated** files: an explicit marker in the leading comments (`@generated`, "Code generated
  … DO NOT EDIT", "This file was generated") or in the doc comment of the first type, and Yarn
  Plug'n'Play files (`.yarn/**`, `.pnp.cjs`, `.pnp.js`, `.pnp.loader.mjs`);
* **minified** files (very long lines);
* **vendored** JavaScript (`.js`, `.mjs`, `.cjs`) — a library copied into the repository or a
  bundler's output — recognized by strong signals only, never by a directory name:
  * a banner that minifiers preserve, naming a release and a copyright or license, at the top
    of the file: `/*! jQuery v3.7.1 | (c) OpenJS Foundation | jquery.org/license */`, a
    `/*! … */` block such as `Socket.IO v4.8.4 (c) … Released under the MIT License`, `//!`
    lines, or `/** @license React v16.14.0 … */` (a header without a release number, such as
    `/*! Copyright (c) Example Corp. Licensed under the Apache License 2.0 */`, is not one);
  * a plain block comment at the top of the file that names all three of a library release, a
    copyright and a license: `/*  Prototype JavaScript framework, version 1.7.3 · (c) 2005-2010
    Sam Stephenson · … MIT-style license */`. The release is `vX.Y`, `version X.Y` or a bare
    `X.Y.Z`, and never a license's own version: the usual license headers (`Licensed under the
    Apache License, Version 2.0`, `GNU Lesser General Public License version 2.1`) do not
    match. A first-party file whose header names its own release, copyright and license is
    skipped too — set `skip_generated: false` if your sources carry such headers;
  * webpack's `/*! For license information please see main.js.LICENSE.txt */`;
  * a bundler's runtime: webpack's bootstrap (`/******/` lines with `webpackBootstrap`) or a
    webpack chunk (`(self["webpackChunk…"] = …).push(…)`), Parcel's prelude (`$parcel$…`
    helpers), Browserify's prelude, or two or more esbuild module markers
    (`// node_modules/pkg/index.js` lines);
  * a trailing `//# sourceMappingURL=` comment on a file of at least 2 KB whose code is one
    wrapped expression (an IIFE or UMD wrapper spanning at least 90% of it). Compiled modules
    with many top-level statements — `tsc` output, which ends with the same comment — are not
    matched.

  A banner that names the project's own package is the project's release banner, not a copied
  library's, and does not count: when the library name the banner starts with (`jQuery` in
  `/*! jQuery v3.7.1 | … */`, the text before the release) is the `name` of the nearest
  `package.json` above the file or of the `package.json` at the scan root — compared ignoring
  case, punctuation and a trailing `js` (`Socket.IO` = `socket.io`, `Chart.js` = `chart.js`), a
  scoped `@scope/name` also by `name` — the file is judged by the other signals alone, like a
  file without a banner. The project's own build output still shows those other signals: socket.io's
  committed `packages/socket.io/client-dist/socket.io.js` names "Socket.IO" and its nearest
  `package.json` is `socket.io`, but it is one UMD wrapper ending with a source-map comment, so
  it stays skipped. `package.json` files are read as data (`name` only); renaming the package
  re-analyzes the files it covers.

The text summary counts them next to the other files (`Files  120 analyzed, 0 cached, 5 skipped
[generated 2, minified 1, vendored 2]`), the results JSON gives each skipped file's
`skip_reason` (`generated`, `minified`, `vendored`, `excluded`, `too-large`, `binary`) and the
counts in `stats.skipped_by_reason`. A changed file skipped as generated or vendored is
reported with a warning when scanning against a base.

To analyze such files anyway, list them in `skip_generated_except` (globs, like `exclude`): they
are analyzed even when they look generated, minified or vendored. `exclude` still wins — an
explicit exclusion beats an exception to the automatic detection — and files larger than
`max_file_size` or binary files are still skipped. `skip_generated: false` turns all three checks
off for every file.

```yaml
skip_generated_except:
  - "web/static/app.bundle.js"     # our own build, committed on purpose: analyze it
  - "src/main/resources/js/**"     # hand-written, despite their release banners
```

Editing the list re-analyzes only the files whose status changes (the cache key of a file says
whether the checks apply to it, not the list).

### Test files

Test files are recognized by their location and the standard name globs only — never by what
the code in them looks like. A file is test code when its path matches its language's default
test globs or one of the globs in `tests` (added to the defaults):

| Language | Default test globs |
|---|---|
| Java | `**/src/test/**`, `**/src/testFixtures/**`, `**/src/integrationTest/**`, `**/src/it/**`, `**/*-integration-tests/**`, `**/test/**`, `**/*Test.java`, `**/*Tests.java`, `**/*IT.java` |
| JavaScript, TypeScript | `**/*.spec.*`, `**/*.test.*`, `**/__tests__/**`, `**/test/**`, `**/tests/**`, `**/e2e/**`, `**/cypress/**`, `**/*.cy.*`, `**/playwright*/**` |

Test code gets `test_rules`, is counted apart in the measures (`measures.test`) and stays out of
duplication detection unless `cpd.include_tests`. If your tests live elsewhere — a `qa/` module,
`*Check.java` classes next to the main code — configure the test globs with `tests`; otherwise
those files are treated as main code:

```yaml
tests: ["qa/**", "**/*Check.java"]
```

Rules about test code judge a class by its contents, not by where its file is:
`java/test-class-naming` looks at classes with JUnit or TestNG test methods wherever they are,
and a class named `TestInvoiceParser` or `InvoiceParserTestCase` outside a test directory is
main code unless `tests` says otherwise.

### Suppressions from other tools

`suppressions.compat` honors the suppression comments and annotations other tools read, so a
code base keeps its existing suppressions when it moves to Mote. It takes `false` (the
default), `true`, or a list of sources:

```yaml
suppressions:
  compat: [pmd, spotbugs, intellij]
```

| Source | Honors | Silences |
|---|---|---|
| `eslint` | `// eslint-disable-next-line [rules]`, `// eslint-disable-line [rules]`, `/* eslint-disable [rules] */` … `/* eslint-enable */` (JS/TS) | the listed rules (`name` and `@typescript-eslint/name` → `js/name`), or every rule |
| `nosonar` | `NOSONAR` in a comment (any language) | every rule on the comment's line |
| `pmd` | `// NOPMD` (reason after the marker), `@SuppressWarnings("PMD")`, `@SuppressWarnings("PMD.RuleName")` (Java) | every rule on the line / the declaration; for `PMD.RuleName`, the rule PMD's rule maps to |
| `checkstyle` | `// CHECKSTYLE:OFF` … `// CHECKSTYLE:ON` (Java) | every rule in between (to the end of the file without `ON`) |
| `suppress-warnings-all` | `@SuppressWarnings("all")` (Java) | every rule on the declaration |
| `spotbugs` | `@SuppressFBWarnings("PATTERN")`, `@SuppressFBWarnings(value = {"A", "B"}, justification = "…")` (`edu.umd.cs.findbugs.annotations`) | the rules the bug patterns map to; a value also covers every pattern it starts (`DM` for `DM_GC`, as in SpotBugs); without a value (every SpotBugs warning), every rule of the SpotBugs table on the declaration — rules without a SpotBugs equivalent stay reported |
| `intellij` | `//noinspection Name[,Name…]` before a statement or declaration, `@SuppressWarnings("Name")` with IntelliJ IDEA inspection names (Java), including the javac- and Eclipse-style `unused`, `rawtypes`, `fallthrough`, `finally`, `resource` | the rules the inspections map to on that statement / declaration; `ALL` (case-sensitive; `"all"` belongs to `suppress-warnings-all`): every rule of the IntelliJ table — rules without an IntelliJ equivalent stay reported |

`true` stands for `[eslint, nosonar, pmd, checkstyle, suppress-warnings-all]` — exactly what it
honored before sources could be chosen; `spotbugs` and `intellij` must be listed. `mote
init` proposes the sources the repository already uses, with their counts (see
[`mote init`](#mote-init)). A rule name
of another tool without a Mote equivalent suppresses nothing (the PMD, SpotBugs and
IntelliJ tables are in `crates/mote-lang-java/src/{pmd,spotbugs,intellij}.rs`), and the
new sources' catch-all forms (`@SuppressFBWarnings` without a value, IntelliJ's `ALL`) silence
only the rules of their tool's table. Names are compared as written (`"all"` and `"ALL"` are
different sources). An unknown source is a configuration error (`mote config validate`
suggests the closest name).
Findings these suppress are reported with the kind `compat` (`nosonar` for `NOSONAR`); such
suppressions need no reason and are never reported as unused — `mote-ignore[rule]: reason`
comments still are, whatever the sources. See [migration.md](migration.md#suppressions-already-in-the-code).

### Paths

Every format uses `/`-separated relative paths. **text**, **json** and **jsonl** (and the MCP tools)
use paths relative to the scan root — what you work with locally, and what `--include` takes.
The formats CI services read — **github** annotations (`--github-checks`), **sarif**,
**markdown**, **gitlab**, **checkstyle** and **junit** — use paths relative to the repository top
level: `mote scan backend` (the Action's `path:` input) reports `backend/src/A.java` there and
`src/A.java` in text/json/jsonl. `mote gate`
re-renders from a results file, which does not record the scan root's place in the repository, so
its Markdown lists findings relative to the scan root.

## `mote gate`

```
mote gate --results <FILE> [--coverage <GLOB>...] [--config <FILE>]
              [--only <CONDITION>...] [--format text|markdown|json]

  --only <CONDITION>   Evaluate only these conditions (repeatable or comma-separated):
                       severity     new findings at or above gate.fail_on (all findings without a base)
                       duplication  duplication on new code (gate.new_duplication_pct)
                       coverage     coverage on new code (gate.new_coverage_pct)
                       worsened     worsened findings (gate.worsened: fail)
                       grades       maintainability and reliability grades (gate.*_grade)
```
Loads a `--json` results file, optionally adds coverage reports, re-evaluates the gate, prints the
verdict, and exits 0/1. No source analysis happens — used by a CI job that runs after tests. The
configuration is validated like `scan` does (unknown rule ids, bad parameters → exit 2); coverage
and the gate are computed by the same engine functions as in a scan.

`--only coverage` is for a job that adds coverage after the scan job already gated the findings:
the verdict (exit code, text/Markdown/JSON output) covers just the selected conditions. A selected
condition the configuration does not set is reported as a warning and ignored; a skipped condition
(e.g. no base, too few new lines) passes as usual.

## `mote compare`

```
mote compare <OLD> <NEW> [--format text|markdown|json] [--max-listed N] [-o FILE]
```

Compares two results files written by `mote scan --json` — two releases, last week and
today, the default branch before and after a merge — without scanning anything:

* **Findings** are paired like a baseline comparison: same path and fingerprint, then the same
  fingerprint across a renamed or moved file (the old file is gone and the new one did not
  exist), then measured findings (complexity and the like) of the same path, rule and symbol.
  Unsuppressed findings are then **added** (no counterpart in OLD), **fixed** (no counterpart in
  NEW), **severity changed**, **newly suppressed** or **no longer suppressed** (a suppression,
  triage decision included, was added or removed), or unchanged (counted only).
* **Measures**: files, lines, lines of code, functions, both complexities, duplicated lines and
  density, coverage, findings by severity, suppressed findings and estimated effort, old, new and
  the change; the **grades** on both sides (results written before grades existed are graded
  with the default bands); the quality gate verdict on both sides.
* **Warnings** — when the two results come from different engine versions, results formats,
  rule sets (listing rules enabled on one side only), grade bands or scan roots — say which
  differences may not come from the code: a rule added between the two scans shows up as added
  findings, one removed as fixed. Compare results of the same configuration and version where
  you can.
* `text` (default) and `markdown` (a job summary or merge-request note) list at most
  `--max-listed` findings per section (default 50); `json` lists them all:
  `{compare_version: 1, old, new, warnings, counts, added, fixed, severity_changed,
  newly_suppressed, unsuppressed, measures, grades}`, each finding with `id`, `rule`,
  `severity` (`old_severity` for severity changes), `path` (`old_path` after a rename), `line`,
  `message` and `fingerprint`. Warnings also go to stderr for markdown and json.
* Exit code 0 (2 when a file cannot be read or is not a results file): compare reports, the gate
  decides.

## `mote init`

```
mote init [DIR] [--force]              Write a commented starter DIR/.mote.yaml
mote init [DIR] [--force] --from pmd|eslint [CONFIG]
                                       The same starter, with the rules, severities, thresholds
                                       and excludes of a PMD ruleset or an ESLint configuration
mote init [DIR] --suggest-excludes Only list the excludes proposed for third-party code
mote init [DIR] --suggest-compat       Only list the suppressions of other tools found and the
                                       suppressions.compat sources proposed from them
```

The starter lists every setting with its default and a comment. `init` refuses to overwrite an
existing `.mote.yaml` (or `.mote.yml`) without `--force`. After writing it, `init` says
what to do next: add `.mote/` (the cache) to `.gitignore` when it is not there, take a first
look with `mote scan --no-gate`, and — when PMD or ESLint markers were found — carry over
that tool's rule choices with `mote init --force --from pmd|eslint`
([migration.md](migration.md#with-mote-init)). Two settings are proposed from
the repository: excludes for copied third-party code (listed commented out) and the
`suppressions.compat` sources (set). The two `--suggest-…` flags print those proposals and write
nothing; they can be combined. Next to an existing configuration, both leave out the files its
`exclude` covers, and the compat count also follows its `max_file_size`, `skip_generated` and
`skip_generated_except` (a configuration that does not parse is an error, exit 2).

### From another tool's configuration

`--from pmd [RULESET]` or `--from eslint [CONFIG]` merges what
[`mote migrate`](#mote-migrate) proposes for that file into the starter, so there is one
file to review instead of two to merge by hand:

* the mapped rules, with their severities and parameters, in place of the commented `rules`
  examples, and the path-specific settings in place of the `overrides` example — each line
  saying where it comes from;
* the translated excludes after the starter's own (a glob the starter already has is not
  repeated), above the commented third-party proposals;
* `suppressions.compat`: the sources counted in the code, plus the tool's own (`pmd`,
  `eslint`) when its markers were not found, with a comment saying so;
* what could not be carried over, listed at the end of the file and on stdout.

The file is read as data, like `migrate` reads it (JavaScript configurations statically, never
run). Without CONFIG, `init` looks in DIR: for ESLint, the file ESLint itself would use
(`eslint.config.js`, `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`, then `.eslintrc.js`, `.cjs`,
`.yaml`, `.yml`, `.json`, `.eslintrc`, and `eslintConfig` in `package.json`); for PMD, the
rulesets a `pom.xml` names (`<ruleset>` of the maven-pmd-plugin, `${basedir}/` resolved) or a
Gradle build names (`ruleSetFiles`), else `pmd-ruleset.xml`, `pmd.xml`, `ruleset.xml` or the
same under `config/pmd/` (and `config/pmd.xml`, `config/pmd-ruleset.xml`). It stops with exit 2
when it finds none, or several rulesets (name the one to use), before writing anything. CONFIG is
a path from the current directory; DIR comes before `--from` (`mote init web --from eslint
web/eslint.config.mjs`). The migrated globs are relative to DIR: an ESLint configuration in a
sub-directory of DIR (`mote init --from eslint packages/app/eslint.config.mjs`) gets that
directory in front of its patterns, as ESLint reads them from there
([migration.md](migration.md#from-eslint)). `--from` cannot be combined with the `--suggest-…`
flags.

### Other tools' suppressions

`init` counts the suppression comments and annotations of other tools already in the code, per
[source](#suppressions-from-other-tools), in the files a scan would analyze (the same discovery,
`.gitignore` respected, without the starter's excluded directories, oversized files and files
skipped as generated, minified or vendored) — the way the plugins read them, so comments and
annotations count and string literals do not:

| Source | Counted |
|---|---|
| `eslint` | comments starting with `eslint-disable`, `eslint-disable-line`, `eslint-disable-next-line` (JS/TS) |
| `nosonar` | comments containing `NOSONAR` |
| `pmd` | `NOPMD` in a comment's first line, `@SuppressWarnings` naming `PMD` or `PMD.<rule>` (Java) |
| `checkstyle` | comments containing `CHECKSTYLE:OFF` (Java) |
| `suppress-warnings-all` | `@SuppressWarnings("all")` (Java) |
| `spotbugs` | `@SuppressFBWarnings` (Java) |
| `intellij` | `//noinspection` comments, `@SuppressWarnings` naming an inspection of the IntelliJ table (`ALL` included) (Java) |

A source is proposed as soon as one of its markers is found: a `NOPMD` or an `eslint-disable`
exists only because someone used that tool, and honoring it silences only what that marker was
written to silence. `intellij` needs at least **three** IntelliJ-only markers (`//noinspection`
comments and `@SuppressWarnings` naming IntelliJ inspections, together): honoring it also honors
the keys javac and the Eclipse compiler define (`unused`, `rawtypes`, `fallthrough`, `finally`,
`resource`), which most Java code bases carry by the dozen whatever the IDE, so a stray comment
must not decide for all of them. Those keys are counted apart and never propose `intellij` by
themselves. (On the 39 benchmark repositories, `intellij` is proposed for seven, with 3 to 129
markers — six of them 7 or more; six others show one or two strays, such as a test's
`//noinspection` or a `@SuppressWarnings("SimplifiableIfStatement")` in copied code, next to
up to 213 javac/Eclipse keys, and are not proposed.)

The starter then sets `compat` with the counts above it (nothing found: `compat: false`, as
before):

```yaml
suppressions:
  require_reason: true
  # Other tools' suppressions found in this repository (occurrences): intellij 105, nosonar 47.
  # 215 @SuppressWarnings with javac/Eclipse keys (fallthrough 7, rawtypes 4, resource 69, unused
  #   136): every IDE writes them, so they never propose intellij on their own; intellij honors them
  #   too.
  # Other sources: checkstyle, eslint, pmd, spotbugs, suppress-warnings-all (`mote init
  #   --suggest-compat` counts them again).
  compat: [intellij, nosonar]
```

```
$ mote init --suggest-compat
Suppressions of other tools found:
  intellij                  105 in 61 files
  nosonar                    47 in 30 files
  note: 215 @SuppressWarnings with javac/Eclipse keys (fallthrough 7, rawtypes 4, resource 69, unused 136): every IDE writes them, so they never propose intellij on their own; intellij honors them too
Proposed (in .mote.yaml): suppressions: { compat: [intellij, nosonar] }
```

### Third-party code

`init` also looks for third-party code copied into the tree (read-only; `.gitignore`d paths and the
directories the starter already excludes — `node_modules`, `vendor`, `generated`, `.yarn` — are
not looked at) and lists what it finds **commented out** under `exclude`: those files stay
analyzed until you uncomment a line. `--suggest-excludes` prints the same list and writes
nothing; next to an existing configuration it leaves out every file that configuration's
`exclude` already covers (a configuration that does not parse is an error, exit 2). Two
heuristics, tuned to stay quiet when unsure:

* **Java packages of another project.** Java files are grouped by namespace: the organization of
  a reverse-domain package (`com.acme`, `org.springframework`), organization and project under
  hosts of many projects (`org.apache.wink`, `com.google.common`), or the first segment
  (`hudson`). Namespaces with at least 10% of the Java files are the project's own. A smaller
  namespace is proposed — one glob per source root, `examples/src/test/java/org/apache/wink/**`
  — when none of its files imports the project's packages or names the project in its package.
  Never proposed: the Java platform and specification namespaces (`java`, `javax`, `jakarta`, …),
  placeholder domains (`com.example`), Java files under a `resources` directory, and a module or
  source set named after the namespace (`xmpbox/src/main/java/org/apache/xmpbox`).
* **Another holder's license header** (Java, JavaScript, TypeScript). When at least 70% of the
  files whose header comment names a copyright holder (`Copyright (c) 2020 Acme Inc.`,
  `Licensed to the Apache Software Foundation`) name the project's usual holders, a file that
  names only other holders, none of which ever appears next to a usual holder (a contributor),
  is proposed: by directory when a directory holds two or more of them and nothing of the
  project, else one by one.

```yaml
exclude:
  - "**/node_modules/**"
  ...
  # Possibly third-party code copied into the repository: still analyzed; uncomment a
  # line to skip it (`mote init --suggest-excludes` lists these again).
  # - "examples/src/test/java/org/apache/wink/**"  # 1 Java file in org.apache.wink.*, which does not use the project's packages (org.apache.pdfbox)
  # - "src/main/webapp/js/plotkit/**"  # 6 files with a license header of "alastair tse", not of the project's "apache software foundation"
```

## `mote migrate`

```
mote migrate --from pmd <ruleset.xml> [-o FILE [--force]]
mote migrate --from eslint <eslint.config.js | .mjs | .cjs | .ts | .eslintrc.js | .eslintrc.cjs | .eslintrc.json | .eslintrc.yaml | .eslintrc | package.json | flat-config.json> [-o FILE [--force]]
```

Reads another tool's configuration as data and prints a proposed `.mote.yaml`: the mapped
rules (on, off, severity; limits of the ESLint limit rules and PMD's threshold properties),
`exclude` globs translated from the tool's ignore patterns, path-specific `overrides`, and the
tool's source of other tools' suppressions (`suppressions: { compat: [pmd] }` or `[eslint]`,
with the other sources listed in a comment); each line says where it comes from, and everything
that could not be mapped is listed at the end. `-o FILE`
writes it instead (an existing file needs `--force`) and prints the list. Printed next to an
existing `.mote.yaml` (one from `mote init`, say), a note on stderr says to copy the
proposal's rules into it, or to write the starter again with them,
`mote init --force --from …` ([migration.md](migration.md#with-mote-init)). JavaScript
and TypeScript configuration files (`eslint.config.js`, `.eslintrc.cjs` and the like) are parsed
and read statically, never run: what only exists when the file runs (function calls, conditions,
imports of local files, `process.env`) is listed as "not statically readable", and shared
configurations it names (`js.configs.recommended`, `tseslint.configs.recommended`) as presets
that are not expanded ([migration.md](migration.md#from-eslint)) — except that
typescript-eslint's `recommended`, `strict` and `all` configurations turn
`js/no-explicit-any` (off by default) on unless the configuration turns it off. ESLint reads a
configuration's patterns from the configuration file's directory; the proposal's globs are
relative to the directory it is for — that of `-o FILE`, or the current directory when printed —
so the configuration of a sub-directory (`packages/app/eslint.config.js`) gets that directory in
front of its patterns, and its settings for every file become an override for
`packages/app/**`. PMD thresholds a ruleset leaves unset get PMD's defaults, converted. The
mapping rules are in [migration.md](migration.md). Exit 0, or 2 when the file cannot be read or
parsed (a JavaScript file with a syntax error, or without `export default` / `module.exports =`
at its top level).

## `mote rules`

```
mote rules [--language <LANG>] [--format text|json|markdown] [--all]   List rules (default: enabled by default)
mote rules explain <RULE_ID>                                            Description, params, examples
```

## `mote config`

```
mote config validate [--config FILE]         Every problem at FILE:LINE:COLUMN; exit 2 if invalid
mote config schema                           Print the JSON Schema of .mote.yaml
mote config schema --results                 Print the JSON Schema of the results JSON (scan --json)
mote config explain <PATH>                   Effective rules/params/severities for a file
```

### Configuration errors

`config validate` lists **every** problem of the file in one run, in file order, each as
`FILE:LINE:COLUMN: problem` (the form editors and terminals link to), and exits 2 when there is
any. `scan`, `gate` and `config explain` stop on an invalid configuration with exactly the same
message, so a CI log says as much as a local `config validate`. For this file, carried over by
hand from other tools' settings:

```yaml
exclud: [vendor]
rules:
  no-unused-vars: off
  category/java/errorprone.xml/EmptyCatchBlock: off
  java/magic-number: error
  java/empty-catch-blok: off
gate:
  min_new_lnes: 5
```

```
$ mote config validate
error: invalid configuration in .mote.yaml (6 problems):
  .mote.yaml:1:1: unknown key `exclud`; did you mean `exclude`?
  .mote.yaml:3:3: rules: unknown rule `no-unused-vars`: that is an ESLint rule; the Mote rule is `js/no-unused-vars` (`mote migrate --from eslint <config>` converts a whole ESLint configuration)
  .mote.yaml:4:3: rules: unknown rule `category/java/errorprone.xml/EmptyCatchBlock`: that is a PMD rule reference; the Mote rule is `java/empty-catch-block` (`mote migrate --from pmd <ruleset.xml>` converts a whole PMD ruleset)
  .mote.yaml:5:22: rules.java/magic-number: invalid value: string "error", expected `off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, `severity`, `params`; `error` is an ESLint level: write `on` (or a severity: LOW, MEDIUM, HIGH)
  .mote.yaml:6:3: rules: unknown rule `java/empty-catch-blok`; did you mean `java/empty-catch-block`?
  .mote.yaml:8:3: gate: unknown key `min_new_lnes`; did you mean `min_new_lines`?
```

* **Unknown keys** at every level (top level, `gate`, `cpd`, a rule's options, an `overrides`
  entry, …) name the closest known key, or list the keys of that level when none is close.
  Unknown values of a fixed set (`extends`, `gate.fail_on`, `gate.worsened`, a rule's level, a
  grade, a `suppressions.compat` source) and unknown rule parameters name the closest one too.
* **Unknown rule ids** get the closest rule id (`java/empty-catch-blok` → `java/empty-catch-block`;
  a missing language prefix, `cognitive-complexity` → `java/cognitive-complexity` or
  `js/cognitive-complexity`; a name in PMD's style, `EmptyCatchBlock` → `java/empty-catch-block`).
* **Other tools' rule names** in `rules`, `test_rules` and `overrides` — ESLint rules
  (`no-unused-vars`, `@typescript-eslint/no-explicit-any`, `complexity`) and PMD rules
  (`EmptyCatchBlock`, `category/java/errorprone.xml/EmptyCatchBlock`, older names such as
  `UnusedImports`) — name their Mote equivalent from the tables `mote migrate` uses (for
  ESLint's limit rules, also the parameter that holds the limit), or say that there is none, and
  point at `mote migrate`, which converts a whole configuration. ESLint's levels
  (`error`, `warn`, `0`–`2`, `[level, options]`) get the Mote setting to write instead.
* Positions are those of the key in error (an unknown key or rule id) or of the value. A
  configuration read from a revision (`scan --rev`) is named `REV:.mote.yaml`.

For a Java file, `config explain` also prints the Java release rules see and where it came
from: `java release: 17 (backend/pom.xml: maven.compiler.release)`, `java release: 21
(java.release in the configuration)`, or `unknown`. JSON output has a `java_release` key
(`{"release": 17, "source": "backend/pom.xml", "setting": "maven.compiler.release"}`,
`"source": "config"`, or `null`). The release comes from `java: { release: N }` in
`.mote.yaml`, else from the nearest `pom.xml` / `build.gradle.kts` / `build.gradle` up the
directory tree (read as data; see the
[engine README](../crates/mote-engine/README.md#java-release)).

### Rule profiles

`extends` in `.mote.yaml` picks the base set of rules; `rules`, `test_rules` and `overrides`
then apply on top of it.

| `extends` | Rules |
|---|---|
| `default` | the rules enabled by default (the column "Default" of [rules.md](rules.md)) |
| `conventions` | `default` plus the style rules tagged `conventions` (plural): an opt-in style pass |
| `all` | every rule that is not deprecated |
| `none` | nothing; enable rules one by one |

```yaml
extends: conventions
rules:
  java/missing-javadoc: off        # take the style pass, minus the rules you don't want
```

The `conventions` rules check style choices that published style guides make and a formatter
does not: braces on every control body, one variable per declaration, array brackets on the
type, modifier order, wildcard imports, misplaced doc comments and the like. They are off by
default because they are conventions rather than defects; teams that follow such a guide switch
them on together. Each one is listed with the tag `conventions` in [rules.md](rules.md)
(`mote rules --all --format json` lists the tags too). Many of them follow ideas of the
Google Java, TypeScript and JavaScript style guides and of the Code Conventions for the Java
Programming Language (see [Provenance](../README.md#provenance)). Like every incubating rule, a
new conventions rule reports but does not fail the gate.

## `mote mcp`

Model Context Protocol server over stdio (JSON-RPC 2.0, newline-delimited). Tools:

| Tool | Purpose |
|---|---|
| `scan` | `{root?, paths?, base?, baseline?, rev?, changed_only?, new_only?}` → scan id + summary (with `triage` counts when a triage file applies). `baseline` is a results file relative to the root, like `scan --baseline`. Re-scans are incremental. |
| `summary` | `{scan_id, group_by: severity\|rule\|file\|directory, new_only?}` → counts |
| `work_units` | `{scan_id, severity?, rule?, max_findings_per_unit?, strategy: by_file\|by_directory}` → disjoint file groups for parallel fixers |
| `findings` | `{scan_id, unit_id?, path?, rule?, severity?, new_only?, limit?, cursor?}` → findings grouped by file |
| `rule` | `{id}` → description, rationale, examples, parameters |
| `verify` | `{scan_id, unit_id? \| paths?}` → re-analyzes only those files, against the rest of the scan (duplication is checked across the whole scan, other files come from the cache): each prior finding `fixed`/`still_present`, plus `introduced` |
| `explain_config` | `{path}` → effective rules for a file |

Finding ids are stable short hashes: identical code keeps its id across scans.

The server opens the analysis cache only while a tool runs and closes it afterwards, so CLI scans
next to an idle server (or a scanner rebuilt after a `.mote.yaml` edit) still get the cache.
