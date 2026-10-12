# Moving to Mote from other tools

A team that switches keeps what its reports rely on: its rule choices, the files it ignores,
the suppressions already in the code, the decisions it made about individual findings, and a
gate that does not fail on old debt. This page covers each; the commands are in
[cli.md](cli.md).

| You had | In Mote |
|---|---|
| A PMD ruleset (`ruleset.xml`) | `mote init --from pmd` writes a starter `.mote.yaml` with its rules; `mote migrate --from pmd ruleset.xml` prints them |
| An ESLint configuration | `mote init --from eslint` (or `mote migrate --from eslint eslint.config.js`): `eslint.config.js` / `.mjs` / `.cjs` / `.ts`, `.eslintrc.js` / `.cjs` / `.json` / `.yaml`, `package.json`, a JSON flat config — JavaScript read statically, never run |
| `// NOPMD`, `@SuppressWarnings("PMD…")`, `// CHECKSTYLE:OFF`, `// eslint-disable…`, `NOSONAR`, `@SuppressWarnings("all")`, `@SuppressFBWarnings`, `//noinspection` | honored per source with `suppressions: { compat: [pmd, eslint, …] }` (`true`: `pmd`, `checkstyle`, `eslint`, `nosonar` and `suppress-warnings-all`) |
| Findings marked "won't fix" or "false positive" in a server | the triage file, `mote triage add` ([cli.md](cli.md#triage-decisions)) |
| A gate on new code only, set up against a server's reference | `--base <branch>` with git history, or `--baseline results.json` without ([cli.md](cli.md#baselines)) |
| Trend charts and measure history | `--summary-json` per run and `mote compare` ([ci.md](ci.md#trends-archive-a-record-per-run-compare-with-the-previous-run)) |
| Server-side PR decoration | CI reports: GitHub Checks, GitLab Code Quality, Bitbucket Code Insights, Checkstyle, JUnit, SARIF ([ci.md](ci.md)) |

## `mote migrate`

```
mote migrate --from pmd|eslint <FILE> [-o FILE [--force]]
```

It reads the file as data, maps what it can, and prints a proposed `.mote.yaml`; every rule
or setting it could not carry over is listed at the end of the file (and on stdout with `-o`,
which writes the proposal to a file and refuses to overwrite one without `--force`). The
proposal is checked like any configuration before it is shown. It always:

* keeps `extends: default` — Mote's own default selection — and adds the mapped rules on top,
  turning off those the source turns off (for a report of only your old rules, change it to
  `extends: none`);
* selects the tool's source of suppressions — `suppressions: { compat: [pmd] }` or
  `[eslint]` — so the suppression comments and annotations already in your code keep working,
  and names the other sources in a comment: add those whose markers your code also carries
  (see [below](#suppressions-already-in-the-code));
* says in a comment on each line which source rule or pattern it comes from.

Rules are mapped **by intent**: a Mote rule "targets the same kind of issue" as the source
rule, which does not make every finding identical — thresholds, exceptions and messages differ.
Review the proposal, then run `mote scan --no-gate` and look at the findings before you gate
on them.

### With `mote init`

`init` and `migrate` look at different sources: `init` at the code (copied third-party code, the
suppression markers of every tool, with their counts), `migrate` at the old tool's
configuration. `init --from` does both and writes one starter:

```sh
mote init --from pmd                             # or: --from pmd config/pmd-ruleset.xml; --from eslint
mote config validate
mote scan --no-gate                              # look at the findings first
```

The starter is the usual one — every setting with a comment, excludes proposed for copied
third-party code (commented out), `suppressions.compat` from the markers counted in the code —
with the migration merged in: the mapped rules, severities and parameters in place of the
commented `rules` examples, the path-specific settings in place of the `overrides` example, the
translated excludes after the starter's own, the tool's own `compat` source added when its
markers were not counted, and what could not be carried over listed at the end. Without a file
name, `init` looks for the configuration in the directory: the file ESLint itself would use, or
the PMD ruleset a `pom.xml` or Gradle build names (else a conventional name such as
`config/pmd/ruleset.xml`); see [cli.md](cli.md#from-another-tools-configuration).

`migrate` alone prints the same proposal, to read or to merge into a configuration you already
have: copy its `rules:`, `exclude:` and `overrides:` entries, and add its `compat` source if
yours lacks it. `migrate -o .mote.yaml` refuses to replace an existing configuration (and
`--force` would drop its settings); printed next to one, the proposal ends with a note saying so.

### From PMD

* `<rule ref="category/java/<category>.xml/<Name>"/>` (and the older `rulesets/java/<set>.xml/
  <Name>`, including rule names from before PMD 7 such as `EmptyIfStmt` or `UnusedImports`)
  enables the Mote rule that targets the same issue. The tables live in
  `crates/mote-lang-java/src/pmd.rs`: `PMD_EQUIVALENTS` for the rules of PMD's quickstart ruleset
  (the same mapping the benchmark uses to compare Mote with PMD), and `PMD_MIGRATION_EQUIVALENTS`
  for other PMD rules with a clear equivalent — `SystemPrintln` → `java/system-out-print`,
  `AvoidCatchingThrowable` → `java/catch-throwable-or-error`, `CollapsibleIfStatements` →
  `java/collapsible-if`, `CyclomaticComplexity`, `CognitiveComplexity`, `TooManyMethods`,
  `AvoidDuplicateLiterals`, … (35 rules). PMD names a rule was deprecated in favor of
  (`UseUtilityClass`, `GenericsNaming`, `JUnitTestContainsTooManyAsserts`, …) map like the
  current name. A rule in neither table is listed as having "no equivalent in the mapping
  table" — Mote may still have a rule for a similar issue: look in `mote rules --all`.
* `<priority>` sets the severity: 1–2 → `HIGH` (fails the default gate), 3 → `MEDIUM`, 4–5 →
  `LOW`. Without a priority the rule keeps its Mote severity.
* Threshold properties become the rule's parameters. PMD reports a metric from its level on,
  Mote above its threshold, so a level is one more than the threshold it becomes:
  `CyclomaticComplexity` `methodReportLevel` → `threshold` (level − 1), `CognitiveComplexity`
  `reportLevel` → `threshold` (level − 1), `ExcessiveParameterList` `minimum` → `max`
  (minimum − 1); the counts carry over as they are: `TooManyFields` `maxfields` and
  `TooManyMethods` `maxmethods` → `max`, `UnitTestContainsTooManyAsserts` `maximumAsserts` →
  `max`, `AvoidDuplicateLiterals` `maxDuplicateLiterals` → `threshold` and `minimumLength` →
  `min_length`. The comment on the line says how a level was converted. A rule enabled without
  the property gets PMD's default, converted, where it differs from Mote's own — the team's
  checks stay as strict as they were: `CyclomaticComplexity` 10 (`threshold` 9),
  `CognitiveComplexity` 15 (`threshold` 14), `ExcessiveParameterList` 10 (`max` 9),
  `TooManyFields` 15, `TooManyMethods` 10, `UnitTestContainsTooManyAsserts` 1,
  `AvoidDuplicateLiterals` 4 occurrences of 3 characters or more (PMD 7.28's defaults; the comment
  says "PMD's default …"). Other properties (options, `classReportLevel`,
  `violationSuppressXPath`, …) are listed.
* A reference to a whole category or ruleset (`category/java/design.xml`,
  `rulesets/java/quickstart.xml`) is not expanded — which rules it holds is PMD's data, not the
  file's. It is listed; its `<exclude name="…"/>` entries turn the mapped Mote rules off.
  Browse `mote rules --all` for rules to add.
* `<exclude-pattern>` regular expressions become `exclude` globs when they only use `.*`, `\.`
  and literal path characters (`.*/generated/.*` → `**/generated/**`, `.*Test\.java` →
  `**/*Test.java`); others are listed. `<include-pattern>` cannot be expressed.
* Custom rules (`<rule name="…" class="…">`), rules of other languages, and references to other
  ruleset files are listed.

### From ESLint

* Files: `eslint.config.js` (and `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`), `.eslintrc.js` /
  `.cjs`, `.eslintrc.json` and `.eslintrc` (JSON with comments, or YAML), `.eslintrc.yaml` /
  `.yml`, the `eslintConfig` of a `package.json`, and flat configurations saved as JSON — an
  array of configuration objects.
* **JavaScript and TypeScript files are parsed and read statically: Mote never runs code
  from the repository** (no Node.js, no `require`, no evaluation). What is read:
  * `export default …`, `module.exports = …` (or `export { config as default }`);
  * literals — objects, arrays, strings, numbers, booleans, template strings without
    expressions — and spreads of them (`...base`, `{ ...rules, eqeqeq: "off" }`, in order, later
    keys winning);
  * the data helpers `defineConfig(…)` and `globalIgnores(…)` (`eslint/config`) and
    `tseslint.config(…)` (`typescript-eslint`): their arguments, arrays flattened as ESLint does;
  * `const`s of the same file whose value is itself static (`const tsFiles = ["**/*.ts"]`,
    `files: [...jsFiles, ...tsFiles]`), as long as nothing in the file changes them
    (`list.push(…)`, `Object.assign(config.rules, …)`, `config.rules.x = …`; a string or number
    `const` cannot change); type annotations, `as` and `satisfies` are read through;
  * shared configurations by name, without reading them: `js.configs.recommended`
    (`@eslint/js`) is `eslint:recommended`, `tseslint.configs.recommendedTypeChecked` is
    `plugin:@typescript-eslint/recommended-type-checked`, a plugin's `configs["flat/recommended"]`
    is `plugin:<name>/recommended`, `eslint-config-prettier` is `prettier`. Like `extends`
    presets, they are listed, not expanded.

  Everything else — a call of another function (`neostandard({…})`, `jsdoc({…})`,
  `configs.map(…)`), a condition (`...(ci ? strict : [])`), `process.env`, a value imported from
  a local file or another package (`globals.node`), a `let` — is **listed as "not statically
  readable"** with its line and why, and what it would have decided is not guessed: a `files`
  list with such a value drops that object's settings; an object spreading such a value
  (`{ ...custom, rules: { … } }`, or a preset spread after `files`) keeps nothing that the spread
  may set or replace — its `rules` would apply to unknown files; such a spread in `rules`
  (`{ ...extra, eqeqeq: "error" }`) is listed, and so is each rule written before it, which it
  may set again, while the rules written after it are kept. Values the migration does not use (`plugins`,
  `languageOptions`, `settings`, `parserOptions`) are not looked at. A file that does not parse,
  or has no `export default` / `module.exports =` at its top level, is an error. The proposal
  names the objects of a flat configuration `config[N]` (in the flattened list) with their line.

  To check what a migration missed, `npx eslint --print-config src/index.ts` (run by you, not by
  Mote) prints the rules the presets turn on for one file.
* Rules: a core rule `name` maps to `js/name`, and so do `@typescript-eslint/name`,
  `react/name` and `react-hooks/name` when Mote has a rule of that name; a few equivalents
  have other names (`@typescript-eslint/no-empty-object-type` → `js/no-empty-interface`). The
  limit rules map to the generic rules with their limit: `complexity` →
  `js/cyclomatic-complexity` (`threshold`), `max-depth` → `js/deep-nesting` (`max`),
  `max-params` → `js/too-many-parameters` (`max`), `max-lines` → `js/file-too-long`
  (`max_lines`, which counts lines of code where ESLint counts every line by default),
  `max-lines-per-function` → `js/function-too-long` (`max_lines`, the same). Other options are
  listed, and so are rules without an equivalent (plugins such as `import/*`; the layout rules —
  `indent`, `semi`, `quotes`, `@stylistic/*`, … — together on one line, since Mote checks no
  layout).
* Levels: `"off"` / `0` → `off`; `"error"` / `2` → `on`; `"warn"` / `1` → `on`, except that a rule
  Mote rates `HIGH` becomes `MEDIUM` — a warning did not fail your lint, so it should not
  fail the default gate either. When several ESLint rules map to one Mote rule (the usual
  `"no-unused-vars": "off"` plus `"@typescript-eslint/no-unused-vars": "error"`), enabling wins.
  In a flat configuration, later objects override earlier ones, as in ESLint.
* Presets turning on a rule Mote leaves off: presets are not expanded (`extends: default`
  stands for them), with one exception. `js/no-explicit-any` is off by default — many teams
  allow `any` — while typescript-eslint's `recommended`, `strict` and `all` configurations (and
  `recommended-type-checked`, `strict-type-checked`) turn `@typescript-eslint/no-explicit-any`
  on. So where one of them applies (`extends`, an element of a flat configuration, a flat
  configuration's `extends`, or their `rules` spread into a `rules` object) and nothing after it
  turns the rule off, the proposal turns `js/no-explicit-any` on, noting the preset
  (`# @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/recommended`); where
  the team turned it off, it stays off. When something that is not statically readable follows
  the preset in the same object (a spread of unknown rules), the rule is listed instead and stays
  off. The type-aware-only configurations (`recommended-type-checked-only`,
  `strict-type-checked-only`, and `recommended-requiring-type-checking`, whose content changed
  between typescript-eslint versions) and `stylistic` do not turn it on.
* Files and scopes: `ignorePatterns` and flat-config global `ignores` become `exclude` globs;
  eslintrc `overrides` and flat-config objects with `files` become `overrides` (a nested list,
  matching files that match all of its patterns, cannot be expressed and drops the object's
  settings). The patterns are read as ESLint reads them:
  * a flat configuration anchors every pattern to its own directory: `dist/` → `dist/**`,
    `*.min.js` → `./*.min.js` (that directory only; Mote reads a glob without `/` at any
    depth, `./` keeps it at the root), `**/*.snap` → `**/*.snap`; `files: ["*.mjs"]` is the
    `.mjs` files of that directory, not of its sub-directories;
  * an eslintrc configuration reads `ignorePatterns` like a `.gitignore` (`dist/` →
    `**/dist/**`, `/build` → `build/**`) and a `files` pattern without `/` as a file name at any
    depth (`*.test.ts` → `**/*.test.ts`).

  ESLint reads them from the directory of the configuration file, and the proposal's globs are
  relative to the directory of the `.mote.yaml` it is for: the directory of `-o FILE`, or the
  current directory when it is printed (`init --from`: its directory). A configuration in a
  sub-directory gets that directory in front of its patterns — `packages/app/eslint.config.js`
  ignoring `dist/` is `packages/app/dist/**`, its `**/*.ts` is `packages/app/**/*.ts` — and,
  since ESLint applies it to the files below its directory only, its settings for every file
  become an override for `packages/app/**` (ahead of the others) instead of top-level `rules`.
  A pattern that leads out of the proposal's directory (`../shared/**` from the top) is listed;
  so is a configuration outside it, whose patterns are then read as if it were there. Only the
  files ESLint loads by itself (`eslint.config.*`, `.eslintrc*`, `package.json`) are read from
  their directory: a flat configuration saved as JSON, or `eslint --print-config` output, is
  read from the proposal's directory wherever it is saved (the proposal says so). Negated
  `files` patterns, `excludedFiles` and per-object `ignores` cannot be expressed. `extends`
  presets (`eslint:recommended`, `plugin:…`, and the presets of a flat configuration's
  `extends`) are not expanded.
* Negations: a `!pattern` re-includes files an earlier pattern ignored, which an exclude list
  cannot do. The negations are listed, and so is every earlier pattern that ignores some of the
  files a negation brings back: `["*/**", "!src/**", "!core/**", "**/target/"]` (ignore every
  directory but the source ones) becomes just `**/target/**`, since translating `*/**` would
  exclude the whole repository.
* ESLint's ignores only ever concerned JavaScript and TypeScript files; `exclude` applies to every
  language. In a repository with Java code too, check that the proposed excludes hide none of it
  (the proposal says so in a comment above them).

## Suppressions already in the code

Besides its own `mote-ignore[rule]: reason` comments, Mote honors the suppression
comments and annotations of other tools, source by source. `suppressions.compat` takes a list
of sources (the migration proposal selects the tool's own), `true` for `eslint`, `nosonar`,
`pmd`, `checkstyle` and `suppress-warnings-all` (everything Mote honored before sources
could be chosen), or `false` (the default):

```yaml
suppressions:
  compat: [pmd, checkstyle, spotbugs]
```

`mote init` counts the markers already in the code and proposes the sources it finds;
`mote init --suggest-compat` prints the counts and the proposal next to an existing
configuration ([cli.md](cli.md#other-tools-suppressions)).

| Source | Form | Covers |
|---|---|---|
| `pmd` | `// NOPMD` or `// NOPMD - reason` (Java) | every rule on the comment's line; the text after the marker is the reason |
| `pmd` | `@SuppressWarnings("PMD")` (Java) | every rule on the annotated declaration |
| `pmd` | `@SuppressWarnings("PMD.RuleName")` (Java) | the Mote rule the PMD rule maps to (see above); a PMD rule without an equivalent suppresses nothing |
| `checkstyle` | `// CHECKSTYLE:OFF` … `// CHECKSTYLE:ON` (Java) | every rule on the lines between them (to the end of the file without `CHECKSTYLE:ON`) |
| `suppress-warnings-all` | `@SuppressWarnings("all")` (Java) | every rule on the annotated declaration |
| `eslint` | `// eslint-disable-next-line [rules]`, `// eslint-disable-line [rules]`, `/* eslint-disable [rules] */` … `/* eslint-enable */` (JS/TS) | the listed rules (core and `@typescript-eslint/` names map to `js/<name>`), or all |
| `nosonar` | `NOSONAR` in a comment | every rule on the comment's line |
| `spotbugs` | `@SuppressFBWarnings("PATTERN")`, `@SuppressFBWarnings(value = {"A", "B"}, justification = "…")` (Java) | the Mote rules the bug patterns map to (`DM_GC` → `java/explicit-gc`, `ES_COMPARING_STRINGS_WITH_EQ` → `java/string-reference-equality`, …); like SpotBugs, a value also covers the patterns it starts (`DM`); without a value, every rule of the SpotBugs table (not the others); the justification is kept as the reason |
| `intellij` | `//noinspection Name[,Name…]` on the line before a statement or declaration; `@SuppressWarnings("Name")` with IntelliJ IDEA inspection names (Java) | the Mote rules the inspections map to (`OptionalAssignedToNull` → `java/optional-null`, `unused` → the `java/unused-*` rules, `DuplicatedCode` → `java/duplicated-block`, …) on that statement or declaration; `ALL` (upper case; `"all"` is the `suppress-warnings-all` source): every rule of the IntelliJ table, not the others |

A name of another tool without a Mote equivalent suppresses nothing. The tables mapping
names to rules are in `crates/mote-lang-java/src/pmd.rs`, `spotbugs.rs` and `intellij.rs`;
only patterns and inspections Mote covers are listed (bug categories such as `CORRECTNESS`
map to nothing). PMD also honors `@SuppressWarnings("unused")`; in Mote that name belongs
to the `intellij` source, which `migrate --from pmd` mentions.

These suppressions are reported as `compat` in the results (`suppressed.kind`; `nosonar` for
`NOSONAR`). They are never checked for being unused and need no reason; when you touch the
code, prefer `// mote-ignore[rule]: reason`, which names the rule, says why, and is
reported once it no longer suppresses anything. `mote/blanket-suppression` points out
`NOSONAR` comments.

## Decisions about findings

A server that marked findings "won't fix" or "false positive" kept those decisions next to the
code. In Mote they live in the repository, in the triage file, and go through review like
any change:

```sh
mote scan --json results.json
mote triage add 3f2a9c81d04e --status false-positive --reason "the value is validated upstream" --results results.json
```

Decisions follow their finding across line moves and file renames (they are matched by
fingerprint) and are reported as stale once the code they were about changes
([cli.md](cli.md#triage-decisions)).

## A gate that ignores old debt

With the git history available, `--base origin/main` classifies every finding as new or old and
gates only the new ones. Without it — a shallow CI checkout, or a first rollout where the
reference is "everything as it is today" — save a baseline once and compare with it:

```sh
mote scan --no-gate --json mote-baseline.json
mote scan --baseline mote-baseline.json
```

Findings in the baseline (matched by fingerprint) are not new; the gate fails only on the
others. Conditions on new lines (duplication and coverage on new code) need a diff and are
skipped with a baseline ([cli.md](cli.md#baselines)). The baseline is a complete results file,
about 1 KB per finding (17 MB for a 270,000-line Java code base with 19,000 findings): commit it,
or keep it as a CI artifact of the default branch. Add `--new-only` to list just the new
findings.
