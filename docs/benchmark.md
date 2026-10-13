# Benchmark

Mote is measured in five parts. The first two check **accuracy** on inputs where the right
answer is known. The third checks **speed** and **precision** on real repositories, the fourth
**recall**, and the fifth whether pull-request scans report as new only what a change introduced.

| Part | What it checks | Where | Run it with |
|---|---|---|---|
| 1. Per-rule fixtures | Each rule's positives and false-positive traps, line by line | `crates/mote-lang-*/tests/fixtures/` | `cargo test -p mote-lang-java -p mote-lang-js` |
| 2. Bad-code samples | Two realistic projects full of deliberate problems and idiomatic clean code, compared with golden snapshots | `bench/samples/`, `bench/expected/` | `cargo test -p mote-cli --test samples` |
| 3. Real repositories | Wall time cold and warm on pinned public repos; hand-sampled precision | `bench/corpora.toml`, `scripts/bench.sh`, `scripts/bench.ps1` | `bash scripts/bench.sh --checkout` |
| 4. Coverage yardstick | **Recall**: how much of what reference tools and real cleanup commits touch, Mote also reports; precision on labeled findings | 20 public repositories at pinned commits | run by the maintainers (needs PMD, ESLint and many clones); results in [Part 4](#part-4-coverage-yardstick) |
| 5. PR replay | **New-code classification**: on real commits replayed as pull requests, are the findings reported as new the ones the change introduced? Time per PR scan | 500 commits of two public repositories | run by the maintainers (needs clones with history); results in [Part 5](#part-5-pr-replay) |

Parts 1–3 run on Windows, Linux and macOS; parts 1 and 2 run in CI on every change.

## Part 1: per-rule fixtures

Every rule ships with at least one fixture: a small source file in which each line that must be
reported ends with `// expect: <rule>` (or the line before says `// expect-next-line: <rule>`),
and **every unmarked line must stay clean**. Fixtures cover the positives and the tricky
negatives, the false-positive traps someone thought of while writing the rule. The format is
defined in `crates/mote-core/src/testing.rs` and summarized in
[architecture.md §6](architecture.md#6-fixtures-the-accuracy-benchmark).

```sh
cargo test -p mote-lang-java                                 # every Java fixture
MOTE_FIXTURE=empty-catch-block cargo test -p mote-lang-java  # one rule while iterating
```

The same test also runs every rule on every fixture file (`assert_all_rules_robust`), so a rule
that panics or misfires on another rule's fixture is caught too.

## Part 2: bad-code sample projects

Fixtures test rules one at a time. The samples test them **together**, on code that looks like a
real project: the configuration, test-file detection, suppressions, duplication across files,
and everything the plugins must *not* report.

### `bench/samples/java`: an order and inventory service (Maven layout)

| Area | Deliberate problems | Clean code that must stay clean |
|---|---|---|
| `domain/` | `equals` without `hashCode`, `new BigDecimal(double)`, missing `@Override`, redundant null check before `instanceof` | Records with compact constructors, an enum with a switch expression, a Lombok `@Getter/@Setter/@Builder` entity, `Optional` and streams |
| `order/` | `==` on strings, `return` in `finally`, self-assignment, unreachable code, switch fall-through, `Optional.get()` without a check, `null` from an `Optional` method, a lost exception cause, a 65-point cognitive-complexity `checkout` nested 7 deep, 8 parameters, duplicated `if/else if` conditions, `compareTo(..) == 1`, `\|` on booleans, nested ternaries, magic numbers (enabled for this package only) | Spring `@Autowired` field injection, a repository interface, custom exceptions |
| `inventory/` | `synchronized` on a `String`, `wait()` outside `synchronized` | Constructor injection, try-with-resources, the `while ((line = reader.readLine()) != null)` idiom, a `static final Logger log` |
| `pricing/` | A 26-line method copied between two files, integer division returned as `double`, `==` on doubles, a public static mutable field, `new Double(..)`, `100l`, a classic `switch` without `default`, a reassigned parameter | Named constants, `BigDecimal` from strings |
| `report/` | A static `SimpleDateFormat`, a `FileWriter` outside try-with-resources, `System.out`/`System.err`/`System.exit`, `printStackTrace`, empty and useless catch blocks, `catch (Throwable)`, string `+=` in a loop, an array printed with `+`, unused import/field/local/private method, TODO/FIXME, commented-out code | An inline suppression with a reason |
| `legacyImport/` | A camelCase package, double-checked locking, `Thread.run()`/`Thread.stop()`, a labeled `continue`, `finalize()`, a `clone()` without `@Override`, a 22-field bean with 43 methods, snake_case names, an exception class not named `…Exception` | A 19-point parser that the package's `overrides:` entry allows |
| `src/test/` | A test without assertions, `Thread.sleep`, `@Disabled` without a reason, `assertEquals(true, ..)` | JUnit 5 + Mockito + AssertJ tests with `snake_case` names, `assertThrows`, `verify(..)`, `@Nested`, `@ParameterizedTest`, `@Disabled("reason")` |

Its `.mote.yaml` switches on some off-by-default rules (`else-after-return`,
`parameter-reassignment`, `cyclomatic-complexity` at 12), lowers `function-too-long` to 60 lines,
and has two `overrides:` entries: magic numbers are enabled for `order/`, and the complexity
threshold is raised to 30 for `legacyImport/`. Without that override the importer's parser (19)
would be reported. `ShopApplication.main` printing its version with `System.out` is not reported
either: command-line entry points may print.

### `bench/samples/ts`: a Node service, a React UI and a legacy script

| Area | Deliberate problems | Clean code that must stay clean |
|---|---|---|
| `src/server/http`, `domain`, `inventory` | Duplicate keys and cases, fall-through, `==`, loss of precision, `x === NaN`, `typeof x === 'undefind'`, `-0` comparison, self-compare, `!a in b`, a `map` callback without `return`, assignment in a condition, a getter without `return`, duplicate class members, a counter moving away from its bound, mixed and duplicate enum values, wrapper types, `namespace` | Discriminated unions with an exhaustive `switch` and `assertNever`, generics, `readonly` types, `as const`, `== null` checks |
| `src/server/orders`, `shared` | `return` in `finally`, unused imports and variables, a deeply nested high-complexity function, collapsible `if`s, unreachable code, a doubled non-null assertion (`x!!`), an empty function, destructuring pattern and block, a reassigned function, 8 parameters | `async`/`await` with `try/catch` that handles errors, retries, optional chaining and `??` |
| `src/server/legacy` | `require` imports, `eval`, `new String`, `new Array`, sparse arrays, chained assignment, `@ts-ignore`, `var`, `any`, an async promise executor, a reassigned catch parameter, identical functions, TODO/FIXME, commented-out code | |
| `src/web` | Hooks called conditionally and after an early return, `alert`, duplicate JSX props, a list without `key` | Hooks at the top level, `useEffect` with cleanup and `AbortController`, `useMemo`/`useCallback` with correct dependencies, a custom hook, keyed lists |
| `scripts/seed-db.js` | `var`; a bare `// mote-ignore` that names no rule and gives no reason (exercises the suppression meta rules) | `console` output allowed by an `overrides:` entry for `scripts/**` |
| `orders/orderService.test.ts` | | Detected as test code: its `console.log` is not reported |

Its `.mote.yaml` enables nine off-by-default rules, turns `js/no-explicit-any` (off by
default too) on at MEDIUM and turns rules off for `scripts/**` and `src/server/legacy/**`.
`app.ts` carries an inline suppression with a reason.

### How the check works

`crates/mote-cli/tests/samples.rs` copies each sample to a temporary directory (converting
CRLF to LF, so `core.autocrlf` doesn't matter), runs the built binary

```sh
mote scan . --format jsonl --no-cache --no-gate
```

and compares the output byte for byte with `bench/expected/<sample>.jsonl`. The output is the same
as `mote scan bench/samples/java --format jsonl --no-cache` run from the repository root:
paths are relative to the sample root and findings are sorted. The test also checks that:

* no file has a parse error or an internal error;
* one thread and four threads give identical output;
* the coverage table at the end of this page matches the goldens and the rule catalog.

A failure prints the findings that appeared (`+`) and disappeared (`-`).

### Blessing snapshots

After an intended change (a new rule, a fixed false positive, a changed message), regenerate
the goldens and the coverage table, then **review every changed line** before committing:

```sh
MOTE_BLESS=1 cargo test -p mote-cli --test samples
git diff bench/expected docs/benchmark.md
```

Each `+` line must be a problem the sample means to contain. Each `-` line must be a real fix. If
a rule starts firing on code meant to be clean, fix the rule (or report it), not the sample.

### Adding a rule's coverage

1. Write the rule and its fixtures (part 1).
2. Add code that triggers it to the sample for its language, the way it would appear in a real
   project, next to a clean variant where one makes sense. Say what it is in a short comment
   (`// bug: …`), not a code-like one.
3. If the rule is off by default, switch it on in the sample's `.mote.yaml`.
4. Bless and review: the new finding shows up in the golden, and the rule's row in the coverage
   table goes from `—` to a count.

### Known misses

A deliberate problem that no rule reports yet stays in the sample, marked `// known miss: <rule>`,
and is listed here. When a fix makes it fire, the golden gains the finding and the marker goes.
There are none at the moment. The first version of the samples found three, all fixed since:

* `java/string-reference-equality` didn't report `order.customerId() == "guest"` when the
  receiver's type is declared in another file. A string literal on one side is enough.
* `java/self-assignment` skipped `this.strict = strict` in a setter whose parameter is
  misspelled, where `strict` is the field itself.
* `js/no-confusing-non-null-assertion` missed `r?.reservedBy! == owner` behind optional chaining.

## Part 3: real repositories

### Corpora

`bench/corpora.toml` pins public repositories to exact commits, so results can be compared over
time:

| Corpus | Language | Why |
|---|---|---|
| [spring-petclinic](https://github.com/spring-projects/spring-petclinic) | Java | A small project: start-up and fixed costs |
| [SymmetricDS](https://github.com/JumpMind/symmetric-ds) | Java | ~1,800 files and 330k lines of long-lived enterprise Java |
| [Excalidraw](https://github.com/excalidraw/excalidraw) | TypeScript / React | A mid-size monorepo |
| [VS Code](https://github.com/microsoft/vscode) | TypeScript | ~14k files and 4M lines |

### Running

```sh
cargo build --release -p mote-cli
bash scripts/bench.sh --checkout                     # all corpora, 5 runs per mode
bash scripts/bench.sh --only symmetric-ds --runs 10  # one corpus
bash scripts/bench.sh --reuse ~/src --no-fetch       # reuse clones in ~/src/<name>, stay offline
```

On Windows without Git Bash, `scripts\bench.ps1` does the same:
`powershell -ExecutionPolicy Bypass -File scripts\bench.ps1 -Checkout`, with parameters
`-Only`, `-Runs`, `-Threads`, `-Reuse`, `-NoFetch`, `-Thresholds`, `-Machine` and `-Output`.

For each corpus the script:

1. Fetches exactly one commit into `bench/.corpora/<name>` (`git init` + `git fetch --depth 1
   origin <sha>`), unless it is already there or `--reuse DIR` has it. No checkout is needed,
   because Mote scans a commit straight from the object database with `--rev`.
2. Makes an untimed run that fills the OS file cache and a warm-cache directory.
3. Times `--runs` **cold** scans, each with a new, empty `--cache-dir`, then `--runs` **warm**
   scans with the primed cache.
4. Repeats this for the **working tree** when the corpus is checked out at the pinned commit
   (`--checkout`, or a clean reused clone).

Wall time covers the whole process: start-up, discovery, analysis, duplication, and writing the
full results JSON (`--format json -o …`). The report is a Markdown table: files, physical lines,
findings, min and median for cold and warm, and cold lines per second. `--output FILE` saves it.

`rev` scans read blobs through one `git cat-file --batch` process, so they pay git's start-up
cost. On Windows, `git` on `PATH` is often the `cmd\git.exe` wrapper, which adds about 0.1 s.

### Regression thresholds

`bench/thresholds.toml` holds median baselines per machine (`[machine.<name>]`, keys like
`"symmetric-ds.worktree.cold"`) and `max_regression_pct`. The check only runs when you ask for it:

```sh
bash scripts/bench.sh --thresholds bench/thresholds.toml --machine i7-8700k-windows
```

A median slower than baseline × (1 + pct/100) prints **Regression** and the script exits 1.
Timings don't carry across machines, so add a table per machine rather than widening the margin.

### Results

Measured 2026-10-13 with `bash scripts/bench.sh --checkout --reuse <clones> --runs 5` under Git Bash.
The machine: Intel Core i7-8700K (6 cores / 12 threads, 3.7 GHz), 32 GB RAM, Windows 10 22H2. The
corpora sit on a SATA hard disk but are read from the OS file cache after the priming run; cache
directories are on an NVMe SSD. Release build of Mote 0.2.0 (552 rules, 485 of them on by
default), all default rules, no configuration file.

| Corpus | Lang | Source | Files | Lines | Findings | Cold min | Cold median | Warm min | Warm median | Lines/s (cold) |
|---|---|---|--:|--:|--:|--:|--:|--:|--:|--:|
| spring-petclinic | java | `818c4136ea` rev | 50 | 4,214 | 20 | 0.17 s | 0.17 s | 0.15 s | 0.15 s | 25k/s |
| spring-petclinic | java | `818c4136ea` worktree | 50 | 4,214 | 20 | 0.04 s | 0.05 s | 0.03 s | 0.03 s | 90k/s |
| symmetric-ds | java | `876594332b` rev | 1,763 | 330,130 | 6,498 | 0.86 s | 0.87 s | 0.19 s | 0.20 s | 380k/s |
| symmetric-ds | java | `876594332b` worktree | 1,763 | 330,130 | 6,498 | 0.63 s | 0.63 s | 0.07 s | 0.07 s | 522k/s |
| excalidraw | ts | `438d89861f` rev | 695 | 215,689 | 2,264 | 0.28 s | 0.28 s | 0.17 s | 0.17 s | 779k/s |
| excalidraw | ts | `438d89861f` worktree | 695 | 215,689 | 2,264 | 0.10 s | 0.10 s | 0.05 s | 0.05 s | 2.1M/s |
| vscode | ts | `987bbdddaf` rev | 14,036 | 4,152,081 | 35,812 | 2.54 s | 2.55 s | 0.43 s | 0.43 s | 1.6M/s |
| vscode | ts | `987bbdddaf` worktree | 14,036 | 4,152,081 | 35,808 | 1.44 s | 1.45 s | 0.28 s | 0.29 s | 2.9M/s |

"Lines" are physical lines of the analyzed files, blank lines and comments included (SymmetricDS
has 252k lines of code, VS Code 3.3M). VS Code has 14,130 matching files; the other 94 were
skipped as generated, minified, vendored or larger than `max_file_size`. The runs were stable: min and
median are within 10% of each other in every row. `scripts/bench.ps1` under Windows
PowerShell gave the same working-tree times; its `rev` scans were 0.05–0.1 s slower because of
the `git` wrapper mentioned above.

What the numbers mean:

* **Cold is the first scan of a repository** (a fresh clone in CI with no cache restored):
  VS Code's 4.2 million lines in 1.4 s, and SymmetricDS in 0.63 s. That is inside the
  architecture target of 5 s per million lines of code on an 8-core laptop.
* **Warm is every scan after that**: re-running on an unchanged tree, a CI job that restored the
  cache, or an agent re-scanning after an edit. It costs discovery, cache reads, duplication
  matching and output: 0.07 s for SymmetricDS, 0.3 s for VS Code (writing its 35,808 findings as
  JSON is part of that).
* **`rev` vs working tree**: reading every blob through `git cat-file` costs 0.2 s on
  SymmetricDS and about 1.1 s on VS Code, still with no checkout at all.

### False-positive sampling

Speed is only half of the result: findings on real code have to be right, and HIGH findings most
of all, because they fail the default gate. After a scan, sample findings per rule and read each
one in context:

```sh
mote scan --no-gate --format jsonl --min-severity HIGH > high.jsonl       # every HIGH finding
mote scan --no-gate --format jsonl > all.jsonl                            # then sample per rule
```

Record each as a true positive, a false positive (a rule bug: report it with a minimal repro and
add the trap to the rule's fixtures), or noise that configuration should handle (vendored or
generated code: `exclude:`).

Results so far (same corpora and binary as the speed results; the HIGH findings are identical to
the previous build's):

* **SymmetricDS, HIGH:** 6 of 6 true positives. Among them: `c == SPACE || c == SPACE` in
  `CsvReader` (the second operand was surely meant to be a tab), a repeated `RELOAD` branch in
  `DataEventType.getEventType`, three `throw`s that leave a `finally` block, and
  `new BigDecimal(double)`.
* **SymmetricDS, MEDIUM:** one random finding for each of the 19 rules that fired, 19 true
  positives. One is a real bug: `OutgoingBatches.countBatches(includeOnlyErrors)` counts every
  batch because both branches do `count++` (`java/identical-branches`). `java/float-equality`
  on a comparator (`thisVal == anotherVal ? 0 : 1`) is correct by the rule's definition but
  harmless in that spot.
* **VS Code, HIGH:** 48 findings. All 24 that aren't `js/array-callback-return` and every third
  `array-callback-return` (8) were read, 32 in total: no false positives by the rules'
  definitions. Real bugs among them:
  * `typeof repository.onDidCommit !== undefined`, which is always true;
  * `newValue === newValue` where the old value was meant (`uriIdentityService.ts`);
  * a destructuring "swap" that assigns both sashes to themselves (`gridview.ts`);
  * `audience = audience` in a constructor (`extHostTypes.ts`).
* **Noise that configuration removes:** 12 of the 32 VS Code findings.
  * 8 are in deliberately broken test fixtures (`**/fixtures/**`) and a vendored bundle
    (`dompurify.js`): exclude them.
  * 4 sit on lines that already carry `// eslint-disable-next-line` for the same rule:
    `suppressions: { compat: true }` honors those.
* **Debatable:** `js/no-misused-new` on `interface FileSystemObserver { new(…):
  FileSystemObserver }`, a declaration of a browser constructor type. typescript-eslint reports
  this pattern too.

### Grades

The default grade bands ([cli.md](cli.md#grades)) were set from the 24 repositories of the Part 4
yardstick (at their pinned commits) plus VS Code, scanned 2026-09-28 with a release build,
the default rules and no configuration (`mote scan <dir> --no-cache --no-gate --format json`;
SymmetricDS is in both sets and listed once). Values as the results report them: maintainability
in estimated hours per 1,000 lines of code (stable rules only), reliability in weighted findings
(HIGH 1, MEDIUM 0.25) per 10,000 lines of code; H/M/L are the reliability findings behind it.

| Repository | Lines of code | Maintainability | Grade | Reliability | H/M/L | Grade |
|---|--:|--:|:-:|--:|--:|:-:|
| hmcts dg-docassembly-api | 4,896 | 0.38 | A | 0.00 | 0/0/1 | A |
| spring-petclinic | 2,466 | 0.59 | A | 0.00 | 0/0/0 | A |
| jhipster-lite | 67,020 | 0.70 | A | 0.04 | 0/1/3 | A |
| apache bval | 23,620 | 1.38 | B | 3.18 | 3/18/2 | D |
| apache creadur-rat | 29,282 | 1.59 | B | 0.77 | 0/9/28 | B |
| apache struts | 185,727 | 2.00 | C | 3.23 | 14/184/21 | D |
| apache pdfbox | 175,014 | 2.08 | C | 1.46 | 8/70/43 | C |
| spring-ldap | 43,432 | 2.18 | C | 1.38 | 2/16/9 | C |
| spring-amqp | 78,110 | 2.20 | C | 1.60 | 5/30/21 | C |
| apache dolphinscheduler | 210,575 | 2.63 | C | 1.37 | 14/59/36 | C |
| apache jmeter | 149,361 | 2.72 | C | 1.59 | 8/63/53 | C |
| apache commons-compress | 83,324 | 2.77 | C | 2.13 | 11/27/26 | D |
| apache poi | 444,688 | 2.91 | C | 1.36 | 20/162/100 | C |
| vscode | 3,275,738 | 3.41 | D | 0.47 | 89/255/0 | A |
| generator-jhipster | 103,252 | 3.43 | D | 0.48 | 0/20/0 | A |
| alfresco-ng2-components | 248,559 | 3.48 | D | 0.49 | 11/5/0 | A |
| hmcts rpx-xui-webapp | 300,277 | 3.57 | D | 0.57 | 16/4/0 | B |
| symmetric-ds | 252,219 | 3.59 | D | 3.90 | 14/337/186 | D |
| primefaces | 204,497 | 3.72 | D | 1.96 | 33/28/49 | C |
| hmcts ccd-case-ui-toolkit | 104,003 | 3.74 | D | 0.34 | 3/2/0 | A |
| excalidraw | 179,078 | 3.95 | D | 4.68 | 81/11/0 | E |
| apache echarts | 154,338 | 4.17 | E | 1.90 | 23/25/0 | C |
| typeorm | 229,716 | 4.17 | E | 1.76 | 38/10/0 | C |
| nestjs nest | 128,099 | 4.71 | E | 0.20 | 0/10/0 | A |
| eclipse-ee4j openmq | 268,500 | 8.03 | E | 9.98 | 32/944/181 | E |

Distribution over the 25 repositories: maintainability A 3, B 2, C 8, D 8, E 4; reliability
A 8, B 2, C 9, D 4, E 2. The first guess, doubling bands for both (0.5, 1, 2, 4), spread
reliability well but put 16 of 25 repositories in maintainability D: maintainability densities
cluster between 2 and 5 hours per 1,000 lines (median 3.4), a range doubling bands cannot
separate. Maintainability therefore uses one-hour steps (1, 2, 3, 4); reliability keeps the
doubling bands. What drives the maintainability effort differs by language: in the TypeScript
repositories `js/no-explicit-any` and `js/function-too-long` make up 45–82% of it in seven of the
nine (`no-explicit-any` alone is 66% in nestjs), in the Java ones it is spread over complexity,
duplication, generic exceptions and TODO comments. The calibration is re-run (and the grades
version bumped) when rules are promoted in bulk or remediation times change, since both move every
repository's numbers.

## Part 4: coverage yardstick

Parts 1–3 show that rules work and are fast; they can't show what's *missing*. Part 4 estimates
**recall** against two references, without using any proprietary analyzer or its output. It
needs PMD, ESLint and clones of many repositories, so it is run by the maintainers; the results
below are aggregate numbers.

**A. Reference tools.** PMD and ESLint + typescript-eslint (permissive licenses, no restrictions on
using their output) run on the same code as Mote, from fixed configurations. A reference finding
is *covered* when Mote reports a finding of the same category (per a curated map from reference
rules to categories) within ±2 lines. Only categories Mote claims are counted; findings in
categories it doesn't claim (formatting, security, …) are listed separately.

A few reference rules report an issue at a different place than the equivalent Mote rule, by
design of both: PMD reports every constant of a constants-only interface, Mote the interface once
at its name; ESLint reports an unused variable at its last write, Mote at its declaration. For
such a pair only, the reference finding also counts as covered when a finding of the
**equivalent rule** (never just the category) sits on the declared *anchor*: the enclosing type
declaration, or the same name reported on the reference finding's line. The results say how many
findings were matched this way.

**B. Public cleanup commits.** Many open-source projects commit focused quality cleanups ("remove
code smells", "fix static-analysis issues"). For each such commit in permissively licensed
repositories, Mote runs on the code *before* the commit; a changed hunk is *covered* when a Mote
finding intersects it (±2 lines). This approximates "would fixing Mote's findings have produced
the same cleanup?" without anyone's analysis output.

**Every miss is classified**, by reading samples of them:

* *rule exists but stayed silent* or *no rule exists* (where the miss comes from);
* *declined*: a deliberate precision choice of the Mote rule (a documented exclusion) or an
  opinion Mote doesn't share, with a written reason;
* *reference wrong*: the reference tool's finding is itself a false positive (for example PMD
  resolving names without a classpath);
* *blocked*: catching it needs a capability Mote doesn't have (cross-file types or references, a
  type checker, data flow, template files).

A classification may cover only a share of a reference rule's misses, estimated from a sample.
Recall is published raw and with the declined and reference-wrong misses left out.

**Precision** is measured on the same repositories plus a few more permissively licensed ones
(which widen the code rare rules are measured on but don't count for recall): findings are
sampled per rule and labeled true positive, false positive, or noise that configuration handles
(vendored or generated files). A stable rule needs at least 95% precision on its labeled
findings; incubating rules are promoted only once a sample of their findings is labeled.
The per-rule results (labeled findings, true and false positives, and the precision once enough
findings are labeled) are published with each release: in [rules.md](rules.md#measured-precision)
(a *Measured precision* line per rule), in `mote rules explain <id>`, and in the `precision`
object of `mote rules --format json` ([cli.md](cli.md#mote-rules)).

**Protocol:**

* No proprietary analyzer is run, and no output, rule text or rule key of one is used.
* The commit harvester records only the repository, the commit and the touched files. Commit
  messages (which may name another tool's rules) are never stored or shown to anyone measuring or
  writing rules.
* New rules and fixtures are written from the *pattern*, in Mote's own code, never copied from
  corpus code.
* Corpora are read-only: no pull requests, issues, comments or pushes to any project.
* Results are published as aggregate numbers only.

Run 2026-10-06 with a 0.1.0 pre-release build, profile `all` (every non-deprecated rule; the
`default` profile in parentheses) · PMD 7.28.0 `rulesets/java/quickstart.xml` on Eclipse Temurin
21.0.12.1+1, no auxiliary classpath · ESLint 10.11.0 + typescript-eslint 8.70.1 `recommended` (not
type-checked, TypeScript files only) + `eslint:recommended` without `no-undef`, inline
`eslint-disable` comments ignored on both sides · all three tools see the same files: the source
files at the pinned commit minus build output and vendored directories, restricted to what Mote
analyzes (it skips generated and minified files).

**Precision:** every stable rule with findings (355) has labeled findings, and none is below 95%
(the lowest is 95.0%); every incubating rule with findings (63) is labeled enough to decide on its
promotion.

**Recall**, with every miss classified:

| Part | Language | Items | Covered | Declined | Reference wrong | Blocked | Recall | Excluding declined and wrong |
|---|---|--:|--:|--:|--:|--:|--:|--:|
| A: reference findings | Java | 14,564 | 6,968 | 7,224 | 144 | 228 | 47.8% | 96.8% |
| A: reference findings | JS/TS | 14,469 | 11,582 | 2,815 | 15 | 57 | 80.0% | 99.5% |
| A: reference findings | **all** | 29,033 | 18,550 | 10,039 | 159 | 285 | 63.9% | **98.5%** |
| B: quality fixes | Java | 293 | 209 | 63 | 0 | 21 | 71.3% | 90.9% |
| B: quality fixes | JS/TS | 128 | 103 | 14 | 0 | 11 | 80.5% | 90.4% |
| B: quality fixes | **all** | 421 | 312 | 77 | 0 | 32 | 74.1% | **90.7%** |

Blocked misses by the capability they would need (Part A findings / Part B hunks): cross-file
types 182 / 10, data flow 103 / 6, cross-file references 0 / 7, template files 0 / 5, a type
checker 0 / 4.

**Part A — reference tools: 63.9% recall** (18,550 of 29,033 PMD/ESLint findings in claimed categories, non-test files; `default` profile 32.2%; 98.5% with the declined and reference-wrong misses left out). Averaged per reference rule instead of per finding: 64% over 138 rules. Without the category Type safety (TypeScript) (53.0% of the covered findings, mostly ESLint @typescript-eslint/no-explicit-any): 45.9%. Test files: 37.8% (8,989 of 23,752; 8,195 of them are PMD MethodNamingConventions). 254 of the covered findings are matched through the anchor that 3 reference rule(s) declare (PMD ConstantsInInterface: enclosing-type, ESLint @typescript-eslint/no-unused-vars: identifier, PMD UnsynchronizedStaticFormatter: identifier; see A above).

| Category | Reference findings | Covered | Recall (default) | Missed: rule exists, silent | Missed: no equivalent rule | Missed: no rule in category |
|---|--:|--:|--:|--:|--:|--:|
| Type safety (TypeScript) | 10,033 | 9,825 | 97.9% (10.7%) | 143 | 65 | 0 |
| Dead and unused code | 4,244 | 1,577 | 37.2% (27.9%) | 2,667 | 0 | 0 |
| Logging and debug output | 2,628 | 122 | 4.6% (4.6%) | 2,472 | 34 | 0 |
| Suspicious logic (typos, copy-paste, constant results) | 2,236 | 288 | 12.9% (12.6%) | 1,948 | 0 | 0 |
| Redundant code | 1,959 | 1,186 | 60.5% (60.3%) | 773 | 0 | 0 |
| Class and API design | 1,822 | 1,112 | 61.0% (59.7%) | 682 | 28 | 0 |
| Null and Optional handling | 1,324 | 714 | 53.9% (53.8%) | 610 | 0 | 0 |
| Language idioms and modern syntax | 1,089 | 955 | 87.7% (85.7%) | 134 | 0 | 0 |
| API misuse (collections, strings, numbers, dates, regex) | 1,086 | 1,014 | 93.4% (93.4%) | 69 | 3 | 0 |
| Exception handling | 753 | 481 | 64% (63%) | 272 | 0 | 0 |
| Variables and scope | 661 | 627 | 95% (95%) | 34 | 0 | 0 |
| Naming | 482 | 434 | 90% (89%) | 45 | 3 | 0 |
| Resource management | 335 | 57 | 17% (17%) | 278 | 0 | 0 |
| Equality and hashing | 147 | 14 | 10% (10%) | 131 | 2 | 0 |
| Control flow | 105 | 64 | 61% (61%) | 41 | 0 | 0 |
| Duplication | 62 | 55 | 89% (89%) | 7 | 0 | 0 |
| Concurrency | 54 | 12 | 22% (22%) | 10 | 32 | 0 |
| Async and promises | 13 | 13 | 100% (100%) | 0 | 0 | 0 |
| **All claimed (non-test)** | **29,033** | **18,550** | **63.9%** (32.2%) | 10,316 | 167 | 0 |

Claimed categories with no reference finding in non-test code (no reference rule, or none fired): Complexity and size, React, Tests.

<details>
<summary>Targets: 20 repositories at pinned commits</summary>

| Target | Commit | Files | Lines | PMD | ESLint | Mote (default) | Recall, non-test |
|---|---|--:|--:|--:|--:|--:|--:|
| spring-petclinic | `818c4136ea` | 50 | 4,214 | 9 | — | 45 (20) | 100% of 2 |
| symmetric-ds | `876594332b` | 1,763 | 330,130 | 14,955 | — | 22,663 (6,498) | 55.9% of 4,485 |
| excalidraw | `438d89861f` | 695 | 215,689 | — | 1,096 | 5,331 (2,264) | 69% of 891 |
| apache/pdfbox | `96c960b654` | 1,334 | 287,187 | 3,016 | — | 9,757 (1,746) | 17.6% of 2,502 |
| apache/creadur-rat | `e972e888fd` | 346 | 49,875 | 390 | — | 1,074 (625) | 36% of 216 |
| jhipster/jhipster-lite | `88cba4a609` | 1,499 | 82,409 | 143 | 51 | 3,016 (176) | 40% of 101 |
| apache/struts | `3e428e4387` | 2,365 | 328,849 | 3,875 | 44 | 10,880 (3,529) | 55.0% of 1,465 |
| apache/jmeter | `ae8f85eee0` | 1,399 | 247,033 | 2,776 | 16 | 10,189 (4,067) | 48.3% of 2,089 |
| primefaces/primefaces | `64942a4c86` | 2,405 | 340,866 | 1,496 | 1,032 | 21,687 (6,966) | 60.1% of 1,769 |
| apache/dolphinscheduler | `7cf1677133` | 3,145 | 321,852 | 3,270 | 1,121 | 14,816 (5,563) | 69.6% of 2,916 |
| hmcts/dg-docassembly-api | `9fa139964e` | 82 | 6,165 | 39 | — | 253 (25) | 59% of 29 |
| apache/commons-compress | `19278c4f77` | 709 | 146,082 | 1,388 | — | 4,889 (1,599) | 57% of 894 |
| spring-projects/spring-amqp | `38e308f1e4` | 679 | 124,499 | 1,149 | — | 3,000 (2,003) | 31% of 323 |
| jhipster/generator-jhipster | `c70fd3b163` | 1,156 | 138,792 | — | 1,120 | 2,769 (638) | 86% of 710 |
| Alfresco/alfresco-ng2-components | `afd53959c3` | 2,944 | 358,735 | — | 3,447 | 6,991 (1,439) | 96.8% of 2,502 |
| hmcts/rpx-xui-webapp | `6245aa7ec5` | 2,321 | 336,756 | — | 3,097 | 5,717 (2,609) | 96% of 727 |
| hmcts/ccd-case-ui-toolkit | `9f81b2de2d` | 1,121 | 121,697 | — | 1,675 | 2,675 (1,017) | 86% of 546 |
| nestjs/nest | `4654802cef` | 2,044 | 157,360 | — | 4,556 | 5,480 (881) | 90.8% of 2,425 |
| apache/echarts | `984bf46b2f` | 830 | 211,619 | — | 2,234 | 9,841 (4,206) | 46.9% of 2,017 |
| typeorm/typeorm | `f279fd1367` | 3,603 | 304,315 | — | 5,114 | 9,143 (2,912) | 82.0% of 2,424 |

PMD could not analyze 4 file(s) without a classpath and ESLint could not parse 8; their reference findings are missing, not missed.

</details>

Not counted (unclaimed categories), most frequent: PMD UnnecessaryModifier (style and formatting, 1,870), PMD UselessParentheses (style and formatting, 786), PMD ControlStatementBraces (style and formatting, 656), PMD AvoidUsingVolatile (deliberately not covered (opinionated reference rules), 334), PMD AvoidAccessibilityAlteration (security, 222).

**Part B — cleanup commits: 57% of changed hunks covered** (312 of 549 hunks in 100 commits; `default` profile 38%). Leaving out the uncovered hunks that are no quality fix in a claimed category (follow-on edits of the same commit, functional changes, formatting, comments, security): 74% of 421 (90.7% with the declined and reference-wrong ones left out). Commits are split at 2024-11-30: later ones reflect newer rule sets.

| Language | Period | Commits | Hunks | Covered | Recall (default) | Recall on quality fixes |
|---|---|--:|--:|--:|--:|--:|
| Java | ≤ 2024-11-30 | 36 | 230 | 105 | 46% (24%) | 67% of 157 |
| Java | > 2024-11-30 | 24 | 165 | 104 | 63% (51%) | 76% of 136 |
| **Java** | all | 60 | 395 | 209 | **53%** (35%) | 71% of 293 |
| JS/TS | ≤ 2024-11-30 | 21 | 45 | 22 | 49% (47%) | 67% of 33 |
| JS/TS | > 2024-11-30 | 19 | 109 | 81 | 74% (45%) | 85% of 95 |
| **JS/TS** | all | 40 | 154 | 103 | **67%** (45%) | 80% of 128 |

Rules whose findings cover the most hunks: `java/assert-argument-order` (43), `java/missing-javadoc` (41), `js/no-unused-vars` (27), `java/junit5-test-visibility` (19), `java/member-order` (19), `js/no-non-null-assertion` (19).

Uncovered hunks by what they change (classified by reading the before/after code; hunks nobody has read yet take the category of a PMD/ESLint finding on the changed lines):

| Category | Uncovered | ≤ 2024-11-30 | > 2024-11-30 | Rule exists, silent | No equivalent rule | No rule in category |
|---|--:|--:|--:|--:|--:|--:|
| Functional or follow-on change (not a quality fix) (unclaimed) | 76 | 42 | 34 |  |  |  |
| Style and formatting (unclaimed) | 22 | 22 | 0 |  |  |  |
| Dead and unused code | 21 | 11 | 10 | 9 | 12 |  |
| Class and API design | 15 | 8 | 7 | 10 | 5 |  |
| Redundant code | 14 | 8 | 6 |  | 14 |  |
| Language idioms and modern syntax | 14 | 6 | 8 | 4 | 10 |  |
| Mote meta rules and other tools' suppressions (unclaimed) | 14 | 12 | 2 |  |  |  |
| Null and Optional handling | 10 | 8 | 2 | 4 | 6 |  |
| Documentation and comments (unclaimed) | 10 | 6 | 4 |  |  |  |
| Tests | 8 | 5 | 3 | 8 |  |  |
| API misuse (collections, strings, numbers, dates, regex) | 7 | 2 | 5 | 4 | 3 |  |
| Security (unclaimed) | 6 | 3 | 3 |  |  |  |
| Logging and debug output | 4 | 4 | 0 | 4 |  |  |
| Exception handling | 4 | 1 | 3 | 4 |  |  |
| Type safety (TypeScript) | 4 | 3 | 1 |  | 4 |  |
| Duplication | 3 | 2 | 1 | 2 | 1 |  |
| Control flow | 2 | 2 | 0 |  | 2 |  |
| Naming | 2 | 2 | 0 |  | 2 |  |
| Variables and scope | 1 | 1 | 0 | 1 |  |  |

**Top 15 gaps, Part A** (missed reference findings in claimed categories; non-test, tests in parentheses):

| # | Reference rule | Category | Missed | Why missed | What it catches | Decisions (non-test misses) |
|--:|---|---|--:|---|---|---|
| 1 | PMD GuardLogStatement | Logging and debug output | 2,472 (389) | `java/unguarded-log-argument` silent | Expensive log message built without a level guard | declined 2,393, reference wrong 2, blocked 77 |
| 2 | PMD AvoidUsingOctalValues | Suspicious logic (typos, copy-paste, constant results) | 1,532 (82) | `java/octal-literal` silent | Integer literal with a leading zero (octal) | declined 1,532 |
| 3 | ESLint @typescript-eslint/no-unused-vars | Dead and unused code | 1,214 (592) | `js/no-unused-vars` silent: unused caught errors, _-prefixed and rest-sibling names accepted | Unused variable, import, parameter or caught error | declined 1,214 |
| 4 | ESLint @typescript-eslint/no-unused-expressions | Dead and unused code | 848 (2,266) | `js/no-unused-expressions` silent: short-circuit calls (a && f()) accepted | Expression statement with no effect | declined 848 |
| 5 | PMD UnnecessaryFullyQualifiedName | Redundant code | 562 (24) | `java/unnecessary-fully-qualified-name` silent | Fully qualified name where the simple name is already in scope | declined 477, blocked 85 |
| 6 | PMD LiteralsFirstInComparisons | Null and Optional handling | 493 (40) | `java/literal-first-in-equals` silent | equals() called on a variable with a literal argument (put the literal first) | declined 493 |
| 7 | PMD LooseCoupling | Class and API design | 464 (257) | `java/concrete-collection-type` silent | Declared type is a concrete collection class instead of its interface | declined 464 |
| 8 | PMD AssignmentInOperand | Suspicious logic (typos, copy-paste, constant results) | 405 (30) | `java/assignment-in-operand` silent: array-index increments, chained assignments and loop-condition updates accepted on purpose | Assignment inside an operand (condition, argument, increment) | declined 405 |
| 9 | ESLint no-unused-vars | Dead and unused code | 314 (42) | `js/no-unused-vars` silent: unused caught errors, _-prefixed and rest-sibling names accepted | Unused variable, import, parameter or caught error | declined 314 |
| 10 | PMD CloseResource | Resource management | 278 (1,044) | `java/resource-not-closed-with-try` silent: incubating rule stays conservative (JDBC handles, null-initialized streams) | Resource (stream, connection, statement) not closed | declined 268, reference wrong 5, blocked 5 |
| 11 | PMD SimplifyBooleanReturns | Redundant code | 171 (8) | `java/simplifiable-boolean-return` silent: only the if/else-return-literal shape | if/else returning boolean literals | declined 171 |
| 12 | PMD EmptyCatchBlock | Exception handling | 153 (107) | `java/empty-catch-block` silent: commented catch blocks and ignore-named variables accepted | Empty catch block | declined 153 |
| 13 | PMD CompareObjectsWithEquals | Equality and hashing | 117 (2) | `java/value-reference-equality` silent | Object references compared with == instead of equals() | declined 117 |
| 14 | PMD ReturnEmptyCollectionRatherThanNull | Null and Optional handling | 116 (106) | `java/collection-return-null` silent | Method returning null where an empty collection or array fits | declined 61, reference wrong 9, blocked 46 |
| 15 | PMD ClassWithOnlyPrivateConstructorsShouldBeFinal | Class and API design | 90 (64) | `java/private-constructor-class-not-final` silent | Class with only private constructors that is not final | declined 90 |

**Top 15 gaps, Part B** (uncovered hunks in claimed categories by pattern; `code` names are patterns read from the diffs):

| # | Pattern | Category | Hunks (≤ / > 2024-11-30) | Why missed | What it changes | Decisions |
|--:|---|---|--:|---|---|---|
| 1 | `arrow-body-return-only` | Language idioms and modern syntax | 8 (0 / 8) | no equivalent rule | Arrow function whose block body is a single return | declined 8 |
| 2 | `field-injection` | Class and API design | 7 (0 / 7) | `java/field-injection` silent | Spring @Autowired on fields instead of constructor injection | declined 7 |
| 3 | PMD UncommentedEmptyMethodBody | Dead and unused code | 7 (7 / 0) | `java/empty-method` silent: default interface methods and annotated methods skipped on purpose | Empty method without a comment | declined 7 |
| 4 | `single-use-local` | Redundant code | 6 (6 / 0) | no equivalent rule | Local variable assigned and used once on the next line | declined 6 |
| 5 | `unused-public-member` | Dead and unused code | 6 (2 / 4) | no equivalent rule | Non-private member (method, constant, enum value, property) that nothing in the repository uses | blocked 6 |
| 6 | `angular-unused-standalone-import` | Dead and unused code | 5 (0 / 5) | no equivalent rule | Angular standalone component importing components or directives its template never uses | blocked 5 |
| 7 | `null-dereference-guard` | Null and Optional handling | 5 (5 / 0) | no equivalent rule | Null check added before dereferencing a value that can be null (map lookup, optional field) | blocked 5 |
| 8 | `assertthrows-lambda-setup` | Tests | 4 (1 / 3) | `java/assert-throws-multiple-calls` silent | assertThrows lambda that also runs setup calls besides the one expected to throw | declined 4 |
| 9 | `unnecessary-type-assertion` | Type safety (TypeScript) | 4 (3 / 1) | no equivalent rule | Type assertion or optional chain on a value whose type is already narrowed | blocked 4 |
| 10 | PMD GuardLogStatement | Logging and debug output | 4 (4 / 0) | `java/unguarded-log-argument` silent | Expensive log message built without a level guard | declined 4 |
| 11 | `assert-size-zero` | API misuse (collections, strings, numbers, dates, regex) | 3 (0 / 3) | `java/collection-size-comparison` silent | assertEquals(0, c.size()) instead of an emptiness assertion | declined 3 |
| 12 | `generic-exception-wrap` | Exception handling | 3 (0 / 3) | `java/generic-exception-thrown` silent | catch (Exception) that rethrows a new RuntimeException | declined 3 |
| 13 | `missing-override` | Language idioms and modern syntax | 3 (3 / 0) | `java/missing-override` silent | @Override added to methods overriding project or JDK supertypes | blocked 3 |
| 14 | `ternary-common-call` | Redundant code | 3 (0 / 3) | no equivalent rule | Both branches of a conditional call the same method with one differing argument | declined 3 |
| 15 | `unneeded-throws` | Redundant code | 3 (0 / 3) | no equivalent rule | throws clause lists a checked exception the body cannot throw | blocked 3 |

## Part 5: PR replay

The point of `mote scan --base <merge-base>` on a pull request is to report, and gate on, only
the findings the change *introduces* (see [cli.md, New code](cli.md#new-code)). Part 5 checks that
promise on real history: it replays recent commits of two projects as one-commit pull requests
against their parents and looks at every finding the scan calls new.

### Method

The replay takes the newest non-merge commits that touch an analyzed source file and, for each
commit `C`, runs:

| Scan | What | Measures |
|---|---|---|
| `rev` | `scan --rev C --base C^`, empty cache | the classification, read straight from the object database |
| `prime` | `scan --rev C^`, into a fresh cache | the parent's full findings; the cache a default-branch CI run leaves behind |
| `wt-cold` | check out `C`, `scan --base C^`, empty cache | a PR job without a cache: what CI runs (checkout, then scan) |
| `wt-warm` | the same with the primed cache | a PR job that restored the cache |

The three PR scans must agree finding for finding; on all 500 commits they did. Every new,
unsuppressed finding is then placed against the commit's diff (`git diff -U0 -M C^ C`, the
engine's own options):

* **in an added file**, **on an added or changed line**, or a **duplicated block** overlapping one
  (duplication is classified by range, not matched): expected;
* **on an untouched line**. Lines outside the hunks are the diff's common lines, in the same order
  on both sides, so every untouched line maps to one line of the parent. If the parent had a
  finding of the same rule on that line, the new one is **spurious**: the same issue on the same
  code, and only the matching failed. Otherwise it is a **remote effect**: the change caused a
  new issue on a line it did not touch — a method that grew past the length or complexity limit
  (reported on its unchanged signature), a field whose last use was deleted, a file past 1,000
  lines.

An independent **reference matcher** pairs head and parent findings by the code they sit on, never
by fingerprint: the same rule on the mapped line of untouched code; the same rule, symbol and line
text on changed lines (re-indented code); measured findings by rule, symbol and message; and moved
code — the same rule, message, line text and neighboring lines on a line the commit removed, in the
same or another file. Its unpaired head findings are the expected new ones, its unpaired parent
findings the expected fixed ones. A finding the engine calls unchanged that the reference leaves
unpaired is a **missed** new one. The reference is lenient on untouched lines (any finding of the
same rule on the mapped line) and strict on changed ones (a moved line needs the same neighbors),
so its counts are estimates: every spurious, moved and missed finding was also read.

**Gate flips** re-evaluate the default gate's findings condition (no new HIGH finding of a stable
rule) without the spurious findings and with the missed ones; a commit flips when the verdict
changes. Wall time is the whole process (start-up, diff, analysis of the changed files and their
parent versions, duplication, JSON output), measured on the machine of [Part 3](#results)
(i7-8700K, Windows 10, release build, all threads) while other builds shared it: the p95 column
carries that noise.

### Repositories and commits

Clones with `--depth 400` (full blobs), `core.autocrlf=false` and `core.eol=lf` so that a checkout
is byte-identical to the blobs (with SymmetricDS's `* text=auto` and a CRLF checkout, the
working-tree scan hashes files to other blob ids than the `--rev` prime, and the warm cache misses).

| Repository | Branch | Commits replayed | Range | Changed analyzed files | Added or changed lines |
|---|---|--:|---|--:|--:|
| [SymmetricDS](https://github.com/JumpMind/symmetric-ds) (Java) | `release/3.18` | 250 of 348 | `23f6413520` (2026-01-14) … `0d2c02d44b` (2026-10-02) | 3,066 | 125,504 |
| [Excalidraw](https://github.com/excalidraw/excalidraw) (TypeScript) | `master` | 250 of 274 | `f3c16a600d` (2025-10-02) … `ed10ac7dca` (2026-10-01) | 1,796 | 92,742 |

The commits not replayed touched no analyzed source file: documentation, build and CI files, SQL
scripts and properties in SymmetricDS; translation files (JSON), CI and Docker files in Excalidraw.

### Results

Measured 2026-10-03 with engine 0.1.0, all default rules, no configuration file. "Before" is the
engine as it was; "after" adds the two matching fixes below.

| | SymmetricDS before | after | Excalidraw before | after |
|---|--:|--:|--:|--:|
| New findings (unsuppressed) | 719 | 694 | 945 | 915 |
| … in added files | 347 | 337 | 295 | 274 |
| … on added or changed lines | 270 | 258 | 447 | 446 |
| … duplicated blocks, by range | 46 | 46 | 62 | 62 |
| … on untouched lines | 56 | 53 | 141 | 133 |
| — spurious: the parent had it on that line | **3** (0.4%) | **0** | **9** (1.0%) | **0** |
| — remote effects | 53 | 53 | 132 | 133 |
| Commits with a spurious finding | 3 (1.2%) | 0 | 5 (2.0%) | 0 |
| Moved code reported new: changed lines / added files | 15 / 10 | 0 / 0 | 12 / 21 | 0 / 0 |
| Genuinely new reported as unchanged (missed) | 3 | 0 | 12 | 0 |
| Worsened | 286 | 286 | 1,198 | 1,198 |
| Fixed: engine / reference | 1,249 / 1,223 | 1,224 / 1,223 | 353 / 323 | 323 / 323 |
| Gate failed (duplication / new HIGH findings) | 5 (3 / 2) | 5 (3 / 2) | 11 (10 / 1) | 11 (10 / 1) |
| Gate decisions that flip | 0 | 0 | 0 | 0 |

**The promise held before the fixes.** Of 1,664 new findings on 500 commits, 12 (0.7%) were
spurious, in 8 commits (1.6%); none was a HIGH finding of a stable rule, so no gate decision
changed. Every gate failure came from duplication on new lines or from a genuinely new HIGH finding.
On untouched lines, 185 new findings had no counterpart in the parent and are what the change
caused elsewhere: 113 functions that crossed a length, complexity, nesting or parameter limit
(reported on their unchanged signature), 16 files past 1,000 lines, 27 string literals now
repeated often enough, 7 classes past the method or field limit, 5 fields, methods, variables and
parameters whose last use was deleted, 4 `if`s that became collapsible, and 13 others of the
same kind.

**Two causes, both fixed** (`crates/mote-engine/src/newcode.rs`, `scan.rs`):

1. **Identical lines** (12 spurious findings, 15 missed). A commit adds a line identical to a
   flagged one, above it: `if (activeBatches.size() > 0) {` in a long method, `} as any,` in a test,
   an action's `perform: (elements, appState, value, app) => {`. Fingerprints number identical
   findings in file order, so the added line took over the old line's fingerprint: the old,
   untouched line was reported as new and the added one as unchanged. The count was nearly right
   (Excalidraw under-reported 3), but the pull-request annotation sat on a line nobody touched, and
   the genuinely new one passed unflagged (the 15 missed findings). **Fix:** before fingerprints, a
   finding on an untouched line is paired with the finding of the same rule and message on the
   parent line it maps to (the k-th untouched line of the diff on both sides; both lines must have
   the same text).
2. **Moved code** (58 findings). Extracting a method moves code into a new symbol; moving code into
   a new file puts it in an added file (one SymmetricDS refactoring did both: 16 findings). Either
   way the fingerprint changed and the findings were reported as new, although the change wrote none
   of that code. These findings sit on added lines, so the definition above does not count them as
   spurious, but a refactoring pull request should not have to answer for them. **Fix:** after
   per-file matching, one pass over all changed and deleted files pairs a still-new finding on an
   added line with a still-unpaired parent finding on a removed line that has the same rule,
   message, line text **and neighboring lines** (a context hash of the line and the lines before and
   after it, kept in the analysis cache). A rewrite that merely repeats a common line, such as
   `} catch (Exception e) {` with a different body around it, stays new.

After the fixes the engine and the reference agree on every commit: no spurious, moved or missed
findings, and the same fixed counts except one SymmetricDS commit (below). Full-scan results are
unchanged: the results JSON of SymmetricDS (6,492 findings) and VS Code (44,449 findings) is
identical before and after, `stats` aside.

**Time per PR scan** (before run; seconds, p50 / p95 over 250 commits):

| Repository | PR scan, `rev`, cold | PR scan, working tree, cold | PR scan, working tree, warm | Full scan of the parent |
|---|--:|--:|--:|--:|
| SymmetricDS | 1.41 / 3.54 | 1.32 / 3.30 | 0.37 / 1.21 | 1.36 / 3.73 |
| Excalidraw | 0.74 / 1.92 | 0.68 / 2.06 | 0.31 / 1.09 | 0.65 / 1.53 |

A PR scan with a restored cache costs what a warm scan of the whole tree costs plus the changed
files and their parent versions: about a third of a second on both projects. Without a cache it
costs a full scan. The fixes cost nothing measurable: timing the two engines alternately on every
tenth commit (`rev` PR scans, p50), SymmetricDS took 1.48 s before and 1.47 s after cold, 0.43 s
and 0.42 s warm; Excalidraw 0.67 s and 0.67 s cold, 0.37 s and 0.36 s warm.

**Left as is:**

* **Duplicated blocks** are new when they overlap a changed line, and 76 of the 108 new ones in
  changed files were blocks the parent already had: an edit inside a duplicated block reports the
  whole block (median: 28 lines, one of them changed). Telling which blocks are new needs the
  parent's duplication: a second duplication pass over the parent versions of the changed files
  and every file they could match. The rule is LOW and does not gate; the gate's duplication
  condition counts duplicated *new lines*, which this does not affect.
* **Path-dependent rules on renamed files.** A changed file's parent version is analyzed at the
  head path, so its configuration applies. A file moved to another package directory therefore
  gets `package-directory-mismatch` in its parent analysis too: one SymmetricDS commit counts it as
  fixed when the commit corrected the package (the one fixed-count difference), and a rename that
  does not correct it would not report it as new. Analyzing at the old path would apply the old
  path's configuration (test globs, overrides) instead; neither is right for every rule.
* **Moved code whose neighbors changed** stays new by design: the same neighbors are the evidence
  that the code moved rather than being written again.

The lists behind these numbers (every spurious, moved and missed finding) stay with the
maintainers; only aggregate numbers are published.

## Coverage

Which rules the bad-code samples exercise. This section is generated: don't edit it by hand.
Rules missing here should get sample code (see [Adding a rule's coverage](#adding-a-rules-coverage)).
`java/file-too-long` and `js/file-too-long` need a file over 1,000 lines of code, which would
bury the rest of the sample. `mote/parse-error` and `mote/internal-error` must never
fire on the samples, and `mote/unused-suppression` is left out so that a golden without it
proves every suppression in the samples took effect.

<!-- coverage:begin (generated by crates/mote-cli/tests/samples.rs; bless with MOTE_BLESS=1) -->

| Sample | Findings | HIGH | MEDIUM | LOW |
|---|--:|--:|--:|--:|
| `bench/samples/java` | 106 | 17 | 33 | 56 |
| `bench/samples/ts` | 111 | 24 | 37 | 50 |

| Rules | Exercised | Total | Not exercised |
|---|---|---|---|
| Java | 91 | 326 | 235 |
| JavaScript / TypeScript | 90 | 221 | 131 |
| Engine (meta rules) | 2 | 5 | 3 |
| **All** | **183** | **552** | **369** |

<details>
<summary>Java: 91 of 326 rules exercised</summary>

Not exercised: `java/abs-of-hash-or-random`, `java/abstract-class-without-methods`, `java/accessor-uses-wrong-field`, `java/ambiguous-varargs-argument`, `java/anonymous-class-could-be-lambda`, `java/application-error-subclass`, `java/arithmetic-no-op`, `java/array-brackets-on-name`, `java/array-hash-code`, `java/arrays-as-list-stream`, `java/assert-argument-order`, `java/assert-public-argument`, `java/assert-throws-multiple-calls`, `java/assert-with-side-effect`, `java/assertion-description-after-check`, `java/assertion-in-thread`, `java/assignment-in-operand`, `java/atomic-equals`, `java/boxed-boolean-condition`, `java/boxing-roundtrip`, `java/broad-expected-exception`, `java/byte-sign-extension`, `java/case-conversion-without-locale`, `java/case-insensitive-equals`, `java/catch-programming-error`, `java/chained-assignment`, `java/class-literal-is-instance`, `java/class-name-string-comparison`, `java/collection-passed-to-itself`, `java/collection-return-null`, `java/colon-switch-expression`, `java/compare-to-min-value`, `java/compare-to-without-comparable`, `java/compare-to-without-equals`, `java/concrete-collection-type`, `java/conditional-mixed-boxed-numbers`, `java/confusing-member-name`, `java/constant-if-condition`, `java/constants-only-interface`, `java/constructor-calls-overridable`, `java/constructor-reference-capacity`, `java/dangling-doc-comment`, `java/date-field-out-of-range`, `java/declaration-far-from-use`, `java/default-charset`, `java/default-package`, `java/deprecated-annotation-missing`, `java/double-brace-initialization`, `java/double-checked-publish-early`, `java/duplicate-branch-body`, `java/empty-constructor`, `java/empty-source-file`, `java/equals-asymmetric-subclass`, `java/equals-overload`, `java/equals-unrelated-types`, `java/equals-without-type-check`, `java/exception-logged-without-trace`, `java/exception-not-thrown`, `java/explicit-finalize-call`, `java/explicit-gc`, `java/explicit-varargs-array`, `java/fail-swallowed-by-catch`, `java/field-could-be-local`, `java/field-injection`, `java/file-name-mismatch`, `java/file-too-long`, `java/float-loop-counter`, `java/for-loop-counter-misuse`, `java/foreach-element-cast`, `java/format-argument-mismatch`, `java/hard-coded-absolute-path`, `java/has-next-calls-next`, `java/hex-string-without-padding`, `java/hidden-declaration`, `java/identical-catch-branches`, `java/identical-methods`, `java/ignored-failure-return`, `java/ignored-return-value`, `java/immediately-returned-variable`, `java/impossible-size-check`, `java/incompatible-collection-lookup`, `java/incomplete-fluent-assertion`, `java/index-loop-could-be-foreach`, `java/index-of-positive-check`, `java/inner-class-could-be-static`, `java/instanceof-in-catch`, `java/int-constant-overflow`, `java/int-multiplication-overflow`, `java/int-to-long-bits-to-double`, `java/internal-jdk-api`, `java/interrupt-status-lost`, `java/intrinsic-lock-on-lock-object`, `java/invalid-range-arguments`, `java/invalid-regex-literal`, `java/invisible-character`, `java/iterator-contract`, `java/javadoc-empty-tag`, `java/jdbc-statement-in-loop`, `java/jdbc-zero-index`, `java/junit5-test-visibility`, `java/keyset-lookup-in-loop`, `java/lambda-could-be-method-reference`, `java/linear-access-in-loop`, `java/list-remove-in-index-loop`, `java/literal-first-in-equals`, `java/log-and-rethrow`, `java/log-message-concatenation`, `java/logger-for-wrong-class`, `java/lone-block`, `java/loop-counter-wrong-direction`, `java/loop-exits-first-pass`, `java/loop-may-skip-bound`, `java/loop-never-entered`, `java/manual-array-copy`, `java/map-callback-returns-null`, `java/member-order`, `java/misleading-if-layout`, `java/misleading-indentation`, `java/missing-braces`, `java/missing-javadoc`, `java/mockito-mixed-matchers`, `java/mocks-not-initialized`, `java/modifier-order`, `java/multiple-top-level-types`, `java/multiple-unary-operators`, `java/mutable-exception-field`, `java/negated-comparison`, `java/nested-code-too-long`, `java/nested-test-class-not-run`, `java/nested-try`, `java/non-atomic-volatile-update`, `java/nondeterministic-test`, `java/notify-instead-of-notify-all`, `java/null-check-after-dereference`, `java/nullability-annotation-misuse`, `java/numeric-escape-for-named-char`, `java/object-method-name-typo`, `java/octal-literal`, `java/one-variable-per-declaration`, `java/optional-null`, `java/overload-instead-of-override`, `java/overloads-not-adjacent`, `java/override-drops-synchronized`, `java/overwritten-before-read`, `java/package-directory-mismatch`, `java/parameter-overwritten-before-read`, `java/peek-side-effect`, `java/post-increment-self-assignment`, `java/prefer-enum-map`, `java/prefer-try-with-resources`, `java/presized-to-array`, `java/protected-in-final-class`, `java/random-cast-to-int`, `java/raw-type`, `java/record-array-component`, `java/redundant-annotation-value`, `java/redundant-cast`, `java/redundant-close-in-try-with-resources`, `java/redundant-default-constructor`, `java/redundant-inheritance-declaration`, `java/redundant-parentheses`, `java/redundant-qualified-this`, `java/redundant-record-member`, `java/redundant-regex-syntax`, `java/regex-case-insensitive-non-ascii`, `java/regex-compiled-per-call`, `java/regex-control-escape`, `java/regex-empty-alternative`, `java/regex-lazy-quantifier-at-end`, `java/regex-never-matches`, `java/regex-shadowed-alternative`, `java/regex-stack-overflow-risk`, `java/regex-suspicious-character-class`, `java/regex-too-complex`, `java/replace-all-without-regex`, `java/restricted-identifier`, `java/reversed-compound-assignment`, `java/scheduled-pool-without-core-threads`, `java/self-comparing-assertion`, `java/separator-used-as-regex`, `java/serialization-constructor-missing`, `java/serialization-hook-signature`, `java/shift-out-of-range`, `java/simplifiable-boolean-if-else`, `java/simplifiable-stream`, `java/simplifiable-ternary`, `java/sleep-in-synchronized`, `java/soft-assertions-not-verified`, `java/split-declaration`, `java/standard-charset-lookup`, `java/static-access-through-instance`, `java/static-field-set-in-constructor`, `java/static-init-uses-subclass`, `java/stream-missing-bulk-method`, `java/string-builder-char-constructor`, `java/string-builder-field`, `java/string-call-with-known-result`, `java/substring-then-search`, `java/suspicious-octal-escape`, `java/swapped-arguments`, `java/synchronized-on-get-class`, `java/synchronized-on-reassigned-field`, `java/test-assertion-in-production`, `java/test-catch-only-fails`, `java/test-class-naming`, `java/test-could-be-parameterized`, `java/test-lifecycle-method-not-run`, `java/test-method-not-run`, `java/this-escape-in-constructor`, `java/thread-passed-as-runnable`, `java/thread-subclass-without-run`, `java/too-many-assertions`, `java/tostring-or-clone-returns-null`, `java/type-parameter-naming`, `java/unclear-operator-precedence`, `java/unguarded-log-argument`, `java/unnecessary-fully-qualified-name`, `java/unnecessary-return`, `java/unretained-annotation-lookup`, `java/unsynchronized-accessor`, `java/untyped-to-array-cast`, `java/unused-private-method-parameter`, `java/unused-type-parameter`, `java/url-in-hash-collection`, `java/useless-override`, `java/value-reference-equality`, `java/virtual-thread-platform-setting`, `java/volatile-mutable-contents`, `java/wait-notify-on-thread`, `java/wait-notify-wrong-monitor`, `java/wait-with-nested-lock`, `java/wait-without-loop`, `java/week-year-pattern`, `java/wildcard-import`, `java/wildcard-return-type`.

| Rule | Severity | Default | Findings in the samples |
|---|---|---|---|
| `java/abs-of-hash-or-random` | MEDIUM | on | — |
| `java/abstract-class-without-methods` | LOW | on | — |
| `java/accessor-uses-wrong-field` | MEDIUM | on (incubating) | — |
| `java/ambiguous-varargs-argument` | MEDIUM | on (incubating) | — |
| `java/anonymous-class-could-be-lambda` | LOW | on | — |
| `java/application-error-subclass` | MEDIUM | on (incubating) | — |
| `java/arithmetic-no-op` | LOW | on | — |
| `java/array-brackets-on-name` | LOW | off | — |
| `java/array-hash-code` | HIGH | on | — |
| `java/array-to-string` | MEDIUM | on | 1 (java) |
| `java/arrays-as-list-stream` | LOW | on | — |
| `java/assert-argument-order` | LOW | on | — |
| `java/assert-boolean-literal` | LOW | on | 1 (java) |
| `java/assert-public-argument` | LOW | on | — |
| `java/assert-throws-multiple-calls` | LOW | on | — |
| `java/assert-with-side-effect` | HIGH | on (incubating) | — |
| `java/assertion-description-after-check` | LOW | on | — |
| `java/assertion-in-thread` | MEDIUM | on | — |
| `java/assignment-in-condition` | MEDIUM | on | 1 (java) |
| `java/assignment-in-operand` | LOW | on | — |
| `java/atomic-equals` | HIGH | on | — |
| `java/bigdecimal-from-double` | HIGH | on | 1 (java) |
| `java/boolean-literal-comparison` | LOW | on | 1 (java) |
| `java/boxed-boolean-condition` | MEDIUM | on | — |
| `java/boxed-primitive-constructor` | LOW | on | 1 (java) |
| `java/boxing-roundtrip` | LOW | on | — |
| `java/broad-expected-exception` | LOW | on | — |
| `java/byte-sign-extension` | HIGH | on (incubating) | — |
| `java/case-conversion-without-locale` | LOW | on | — |
| `java/case-insensitive-equals` | LOW | on | — |
| `java/catch-generic-exception` | LOW | on | 1 (java) |
| `java/catch-programming-error` | MEDIUM | on | — |
| `java/catch-throwable-or-error` | MEDIUM | on | 1 (java) |
| `java/chained-assignment` | LOW | off | — |
| `java/class-literal-is-instance` | LOW | on | — |
| `java/class-name-string-comparison` | MEDIUM | on | — |
| `java/class-naming` | LOW | on | 1 (java) |
| `java/clone-override` | LOW | on | 1 (java) |
| `java/cognitive-complexity` | MEDIUM | on | 1 (java) |
| `java/collapsible-if` | LOW | on | 1 (java) |
| `java/collection-passed-to-itself` | HIGH | on (incubating) | — |
| `java/collection-return-null` | MEDIUM | on | — |
| `java/collection-size-comparison` | LOW | on | 1 (java) |
| `java/colon-switch-expression` | LOW | off | — |
| `java/commented-out-code` | LOW | on | 1 (java) |
| `java/compare-to-min-value` | HIGH | on | — |
| `java/compare-to-specific-value` | MEDIUM | on | 1 (java) |
| `java/compare-to-without-comparable` | MEDIUM | on (incubating) | — |
| `java/compare-to-without-equals` | MEDIUM | on | — |
| `java/concrete-collection-type` | LOW | on | — |
| `java/conditional-mixed-boxed-numbers` | MEDIUM | on (incubating) | — |
| `java/confusing-member-name` | MEDIUM | on (incubating) | — |
| `java/constant-if-condition` | MEDIUM | on | — |
| `java/constant-naming` | LOW | on | 1 (java) |
| `java/constants-only-interface` | LOW | on | — |
| `java/constructor-calls-overridable` | MEDIUM | off | — |
| `java/constructor-reference-capacity` | MEDIUM | on (incubating) | — |
| `java/cyclomatic-complexity` | LOW | off | 1 (java) |
| `java/dangling-doc-comment` | LOW | off | — |
| `java/date-field-out-of-range` | HIGH | on (incubating) | — |
| `java/declaration-far-from-use` | LOW | off | — |
| `java/deep-nesting` | MEDIUM | on | 1 (java) |
| `java/default-charset` | MEDIUM | on (incubating) | — |
| `java/default-package` | LOW | on (incubating) | — |
| `java/deprecated-annotation-missing` | LOW | on | — |
| `java/deprecated-thread-methods` | HIGH | on | 1 (java) |
| `java/disabled-test-without-reason` | LOW | on | 1 (java) |
| `java/double-brace-initialization` | MEDIUM | on | — |
| `java/double-checked-locking` | HIGH | on | 1 (java) |
| `java/double-checked-publish-early` | HIGH | on (incubating) | — |
| `java/duplicate-branch-body` | MEDIUM | on | — |
| `java/duplicate-condition` | HIGH | on | 1 (java) |
| `java/duplicate-string-literal` | LOW | on | 1 (java) |
| `java/duplicated-block` | LOW | on | 2 (java) |
| `java/else-after-return` | LOW | off | 2 (java) |
| `java/empty-block` | MEDIUM | on | 1 (java) |
| `java/empty-catch-block` | MEDIUM | on | 1 (java) |
| `java/empty-constructor` | LOW | off | — |
| `java/empty-method` | LOW | on | 1 (java) |
| `java/empty-source-file` | LOW | on (incubating) | — |
| `java/empty-statement` | LOW | on | 1 (java) |
| `java/equals-asymmetric-subclass` | MEDIUM | on (incubating) | — |
| `java/equals-overload` | HIGH | on | — |
| `java/equals-unrelated-types` | HIGH | on (incubating) | — |
| `java/equals-with-null` | HIGH | on | 1 (java) |
| `java/equals-without-hashcode` | HIGH | on | 1 (java) |
| `java/equals-without-type-check` | MEDIUM | on | — |
| `java/exception-logged-without-trace` | MEDIUM | on | — |
| `java/exception-naming` | LOW | on | 1 (java) |
| `java/exception-not-thrown` | HIGH | on | — |
| `java/explicit-finalize-call` | MEDIUM | on (incubating) | — |
| `java/explicit-gc` | MEDIUM | on | — |
| `java/explicit-varargs-array` | LOW | on | — |
| `java/fail-swallowed-by-catch` | HIGH | on | — |
| `java/field-could-be-local` | LOW | on | — |
| `java/field-injection` | LOW | off | — |
| `java/field-naming` | LOW | on | 1 (java) |
| `java/file-name-mismatch` | LOW | off | — |
| `java/file-too-long` | LOW | on | — |
| `java/final-field-could-be-static` | LOW | on | 2 (java) |
| `java/finalize-override` | MEDIUM | on | 1 (java) |
| `java/fixme-comment` | MEDIUM | on | 1 (java) |
| `java/float-equality` | MEDIUM | on | 1 (java) |
| `java/float-loop-counter` | MEDIUM | on (incubating) | — |
| `java/for-loop-counter-misuse` | MEDIUM | on | — |
| `java/foreach-element-cast` | LOW | on (incubating) | — |
| `java/format-argument-mismatch` | HIGH | on | — |
| `java/function-too-long` | LOW | on | 1 (java) |
| `java/generic-exception-thrown` | LOW | on | 1 (java) |
| `java/hard-coded-absolute-path` | MEDIUM | on | — |
| `java/has-next-calls-next` | HIGH | on (incubating) | — |
| `java/hex-string-without-padding` | MEDIUM | on (incubating) | — |
| `java/hidden-declaration` | LOW | off | — |
| `java/identical-branches` | MEDIUM | on | 1 (java) |
| `java/identical-catch-branches` | LOW | on | — |
| `java/identical-methods` | MEDIUM | on | — |
| `java/identical-operands` | HIGH | on | 1 (java) |
| `java/ignored-failure-return` | MEDIUM | on | — |
| `java/ignored-return-value` | HIGH | on | — |
| `java/immediately-returned-variable` | LOW | on | — |
| `java/impossible-size-check` | HIGH | on | — |
| `java/incompatible-collection-lookup` | HIGH | on | — |
| `java/incomplete-fluent-assertion` | HIGH | on | — |
| `java/index-loop-could-be-foreach` | LOW | on | — |
| `java/index-of-positive-check` | MEDIUM | on | — |
| `java/inner-class-could-be-static` | LOW | on | — |
| `java/instanceof-in-catch` | LOW | on | — |
| `java/instanceof-pattern-candidate` | LOW | on | 1 (java) |
| `java/int-constant-overflow` | HIGH | on (incubating) | — |
| `java/int-multiplication-overflow` | MEDIUM | on | — |
| `java/int-to-long-bits-to-double` | MEDIUM | on (incubating) | — |
| `java/integer-division-in-floating-context` | MEDIUM | on | 1 (java) |
| `java/internal-jdk-api` | MEDIUM | on (incubating) | — |
| `java/interrupt-status-lost` | MEDIUM | on | — |
| `java/intrinsic-lock-on-lock-object` | HIGH | on (incubating) | — |
| `java/invalid-range-arguments` | HIGH | on (incubating) | — |
| `java/invalid-regex-literal` | HIGH | on | — |
| `java/invisible-character` | MEDIUM | on | — |
| `java/iterator-contract` | MEDIUM | on | — |
| `java/javadoc-empty-tag` | LOW | off | — |
| `java/jdbc-statement-in-loop` | MEDIUM | on (incubating) | — |
| `java/jdbc-zero-index` | HIGH | on (incubating) | — |
| `java/junit5-test-visibility` | LOW | on | — |
| `java/keyset-lookup-in-loop` | LOW | on | — |
| `java/labeled-statement` | LOW | on | 1 (java) |
| `java/lambda-could-be-method-reference` | LOW | on | — |
| `java/linear-access-in-loop` | MEDIUM | on | — |
| `java/list-remove-in-index-loop` | MEDIUM | on | — |
| `java/literal-first-in-equals` | LOW | on | — |
| `java/local-variable-naming` | LOW | on | 1 (java) |
| `java/log-and-rethrow` | LOW | on | — |
| `java/log-message-concatenation` | LOW | on | — |
| `java/logger-for-wrong-class` | LOW | on | — |
| `java/lone-block` | LOW | on | — |
| `java/long-literal-lowercase` | LOW | on | 1 (java) |
| `java/loop-counter-wrong-direction` | HIGH | on (incubating) | — |
| `java/loop-exits-first-pass` | MEDIUM | on | — |
| `java/loop-may-skip-bound` | MEDIUM | off | — |
| `java/loop-never-entered` | HIGH | on (incubating) | — |
| `java/lost-exception-cause` | MEDIUM | on | 1 (java) |
| `java/magic-number` | LOW | off | 4 (java) |
| `java/manual-array-copy` | LOW | on | — |
| `java/map-callback-returns-null` | MEDIUM | on (incubating) | — |
| `java/member-order` | LOW | off | — |
| `java/method-naming` | LOW | on | 2 (java) |
| `java/misleading-if-layout` | MEDIUM | on (incubating) | — |
| `java/misleading-indentation` | MEDIUM | on | — |
| `java/missing-braces` | LOW | off | — |
| `java/missing-javadoc` | LOW | off | — |
| `java/missing-override` | LOW | on | 2 (java) |
| `java/missing-switch-default` | LOW | on | 1 (java) |
| `java/mockito-mixed-matchers` | HIGH | on (incubating) | — |
| `java/mocks-not-initialized` | MEDIUM | on (incubating) | — |
| `java/modifier-order` | LOW | off | — |
| `java/multiple-top-level-types` | LOW | off | — |
| `java/multiple-unary-operators` | LOW | on (incubating) | — |
| `java/mutable-exception-field` | LOW | on | — |
| `java/negated-comparison` | LOW | off | — |
| `java/nested-code-too-long` | LOW | on | — |
| `java/nested-ternary` | LOW | on | 1 (java) |
| `java/nested-test-class-not-run` | HIGH | on (incubating) | — |
| `java/nested-try` | LOW | off | — |
| `java/non-atomic-volatile-update` | HIGH | on | — |
| `java/non-short-circuit-boolean` | MEDIUM | on | 1 (java) |
| `java/nondeterministic-test` | MEDIUM | on | — |
| `java/notify-instead-of-notify-all` | LOW | on | — |
| `java/null-check-after-dereference` | MEDIUM | on | — |
| `java/nullability-annotation-misuse` | LOW | on (incubating) | — |
| `java/numeric-escape-for-named-char` | LOW | off | — |
| `java/object-method-name-typo` | HIGH | on (incubating) | — |
| `java/octal-literal` | MEDIUM | on (incubating) | — |
| `java/one-variable-per-declaration` | LOW | off | — |
| `java/optional-get-without-check` | MEDIUM | on | 1 (java) |
| `java/optional-null` | MEDIUM | on | — |
| `java/optional-return-null` | HIGH | on | 1 (java) |
| `java/overload-instead-of-override` | HIGH | on (incubating) | — |
| `java/overloads-not-adjacent` | LOW | off | — |
| `java/override-drops-synchronized` | MEDIUM | on (incubating) | — |
| `java/overwritten-before-read` | HIGH | on | — |
| `java/package-directory-mismatch` | LOW | on (incubating) | — |
| `java/package-naming` | LOW | on | 2 (java) |
| `java/parameter-overwritten-before-read` | MEDIUM | on (incubating) | — |
| `java/parameter-reassignment` | LOW | off | 1 (java) |
| `java/peek-side-effect` | MEDIUM | on (incubating) | — |
| `java/post-increment-self-assignment` | HIGH | on (incubating) | — |
| `java/prefer-enum-map` | LOW | on (incubating) | — |
| `java/prefer-try-with-resources` | LOW | on | — |
| `java/presized-to-array` | LOW | on | — |
| `java/print-stack-trace` | MEDIUM | on | 1 (java) |
| `java/private-constructor-class-not-final` | LOW | on | 1 (java) |
| `java/protected-in-final-class` | LOW | on | — |
| `java/public-instance-field` | LOW | on | 1 (java) |
| `java/public-static-mutable-field` | MEDIUM | on | 1 (java) |
| `java/random-cast-to-int` | HIGH | on (incubating) | — |
| `java/raw-type` | LOW | off | — |
| `java/record-array-component` | MEDIUM | on | — |
| `java/redundant-annotation-value` | LOW | on | — |
| `java/redundant-cast` | LOW | on | — |
| `java/redundant-close-in-try-with-resources` | LOW | on (incubating) | — |
| `java/redundant-default-constructor` | LOW | on | — |
| `java/redundant-inheritance-declaration` | LOW | on | — |
| `java/redundant-null-check-before-instanceof` | LOW | on | 1 (java) |
| `java/redundant-parentheses` | LOW | off | — |
| `java/redundant-qualified-this` | LOW | on | — |
| `java/redundant-record-member` | LOW | on | — |
| `java/redundant-regex-syntax` | LOW | on | — |
| `java/redundant-tostring` | LOW | on | 1 (java) |
| `java/regex-case-insensitive-non-ascii` | MEDIUM | on (incubating) | — |
| `java/regex-compiled-per-call` | LOW | on | — |
| `java/regex-control-escape` | MEDIUM | on (incubating) | — |
| `java/regex-empty-alternative` | MEDIUM | on (incubating) | — |
| `java/regex-lazy-quantifier-at-end` | MEDIUM | on (incubating) | — |
| `java/regex-never-matches` | HIGH | on (incubating) | — |
| `java/regex-shadowed-alternative` | MEDIUM | on (incubating) | — |
| `java/regex-stack-overflow-risk` | MEDIUM | on (incubating) | — |
| `java/regex-suspicious-character-class` | MEDIUM | on (incubating) | — |
| `java/regex-too-complex` | MEDIUM | on (incubating) | — |
| `java/replace-all-without-regex` | LOW | on | — |
| `java/resource-not-closed-with-try` | MEDIUM | on | 1 (java) |
| `java/restricted-identifier` | MEDIUM | on (incubating) | — |
| `java/return-in-finally` | HIGH | on | 1 (java) |
| `java/reversed-compound-assignment` | HIGH | on (incubating) | — |
| `java/scheduled-pool-without-core-threads` | MEDIUM | on (incubating) | — |
| `java/self-assignment` | HIGH | on | 2 (java) |
| `java/self-comparing-assertion` | MEDIUM | on | — |
| `java/separator-used-as-regex` | HIGH | on (incubating) | — |
| `java/serialization-constructor-missing` | MEDIUM | on (incubating) | — |
| `java/serialization-hook-signature` | HIGH | on (incubating) | — |
| `java/shift-out-of-range` | HIGH | on (incubating) | — |
| `java/simplifiable-boolean-if-else` | LOW | on | — |
| `java/simplifiable-boolean-return` | LOW | on | 1 (java) |
| `java/simplifiable-stream` | LOW | on | — |
| `java/simplifiable-ternary` | LOW | on | — |
| `java/sleep-in-synchronized` | MEDIUM | on (incubating) | — |
| `java/sleep-in-test` | LOW | on | 1 (java) |
| `java/soft-assertions-not-verified` | HIGH | on (incubating) | — |
| `java/split-declaration` | LOW | off | — |
| `java/standard-charset-lookup` | LOW | on | — |
| `java/static-access-through-instance` | MEDIUM | on | — |
| `java/static-date-format` | HIGH | on | 1 (java) |
| `java/static-field-set-in-constructor` | MEDIUM | on (incubating) | — |
| `java/static-init-uses-subclass` | MEDIUM | on | — |
| `java/stream-missing-bulk-method` | MEDIUM | on | — |
| `java/string-builder-char-constructor` | HIGH | on | — |
| `java/string-builder-field` | LOW | off | — |
| `java/string-call-with-known-result` | MEDIUM | on | — |
| `java/string-concat-in-loop` | MEDIUM | on | 1 (java) |
| `java/string-reference-equality` | HIGH | on | 2 (java) |
| `java/substring-then-search` | LOW | on (incubating) | — |
| `java/suspicious-octal-escape` | LOW | on (incubating) | — |
| `java/swapped-arguments` | HIGH | on (incubating) | — |
| `java/switch-fallthrough` | MEDIUM | on | 1 (java) |
| `java/synchronized-on-get-class` | MEDIUM | on | — |
| `java/synchronized-on-reassigned-field` | HIGH | on (incubating) | — |
| `java/synchronized-on-value` | HIGH | on | 1 (java) |
| `java/system-exit` | MEDIUM | on | 1 (java) |
| `java/system-out-print` | MEDIUM | on | 3 (java) |
| `java/test-assertion-in-production` | MEDIUM | on (incubating) | — |
| `java/test-catch-only-fails` | LOW | on | — |
| `java/test-class-naming` | LOW | off | — |
| `java/test-could-be-parameterized` | LOW | on | — |
| `java/test-lifecycle-method-not-run` | HIGH | on (incubating) | — |
| `java/test-method-not-run` | HIGH | on | — |
| `java/test-without-assertion` | MEDIUM | on | 1 (java) |
| `java/this-escape-in-constructor` | MEDIUM | off | — |
| `java/thread-passed-as-runnable` | MEDIUM | on (incubating) | — |
| `java/thread-run-direct` | HIGH | on | 1 (java) |
| `java/thread-subclass-without-run` | MEDIUM | on (incubating) | — |
| `java/throws-generic-exception` | LOW | on | 1 (java) |
| `java/todo-comment` | LOW | on | 1 (java) |
| `java/too-many-assertions` | LOW | on | — |
| `java/too-many-fields` | LOW | on | 1 (java) |
| `java/too-many-methods` | LOW | on | 1 (java) |
| `java/too-many-parameters` | MEDIUM | on | 1 (java) |
| `java/tostring-or-clone-returns-null` | MEDIUM | on (incubating) | — |
| `java/type-parameter-naming` | LOW | on | — |
| `java/unclear-operator-precedence` | LOW | off | — |
| `java/unguarded-log-argument` | LOW | on (incubating) | — |
| `java/unnecessary-fully-qualified-name` | LOW | on | — |
| `java/unnecessary-return` | LOW | on | — |
| `java/unreachable-code` | HIGH | on | 1 (java) |
| `java/unretained-annotation-lookup` | HIGH | on (incubating) | — |
| `java/unsynchronized-accessor` | MEDIUM | on (incubating) | — |
| `java/untyped-to-array-cast` | HIGH | on (incubating) | — |
| `java/unused-import` | LOW | on | 1 (java) |
| `java/unused-local-variable` | MEDIUM | on | 2 (java) |
| `java/unused-private-class` | LOW | on | 1 (java) |
| `java/unused-private-field` | MEDIUM | on | 2 (java) |
| `java/unused-private-method` | MEDIUM | on | 1 (java) |
| `java/unused-private-method-parameter` | LOW | on | — |
| `java/unused-type-parameter` | LOW | on | — |
| `java/url-in-hash-collection` | MEDIUM | on | — |
| `java/useless-catch` | LOW | on | 1 (java) |
| `java/useless-override` | LOW | on | — |
| `java/utility-class-constructor` | LOW | on | 1 (java) |
| `java/value-reference-equality` | HIGH | on | — |
| `java/virtual-thread-platform-setting` | MEDIUM | on (incubating) | — |
| `java/volatile-mutable-contents` | MEDIUM | off | — |
| `java/wait-notify-on-thread` | MEDIUM | on | — |
| `java/wait-notify-outside-synchronized` | MEDIUM | on | 1 (java) |
| `java/wait-notify-wrong-monitor` | MEDIUM | on (incubating) | — |
| `java/wait-with-nested-lock` | MEDIUM | on (incubating) | — |
| `java/wait-without-loop` | MEDIUM | on | — |
| `java/week-year-pattern` | HIGH | on | — |
| `java/wildcard-import` | LOW | off | — |
| `java/wildcard-return-type` | LOW | on (incubating) | — |

</details>

<details>
<summary>JavaScript / TypeScript: 90 of 221 rules exercised</summary>

Not exercised: `js/accessor-field-mismatch`, `js/accessor-pairs`, `js/array-type`, `js/assert-argument-order`, `js/assignment-in-operand`, `js/async-in-constructor`, `js/bitwise-on-booleans`, `js/block-scoped-var`, `js/callback-extra-arguments`, `js/consistent-return`, `js/consistent-type-assertions`, `js/const-enum`, `js/constant-length-comparison`, `js/constructor-super`, `js/curly`, `js/default-case-last`, `js/default-export`, `js/default-param-last`, `js/double-assertion`, `js/empty-collection-access`, `js/enum-initializers`, `js/exception-not-thrown`, `js/extra-arguments`, `js/file-too-long`, `js/floating-promise`, `js/for-equality-termination`, `js/for-loop-counter-misuse`, `js/identical-operands`, `js/ignored-call-result`, `js/immediately-returned-variable`, `js/in-operator-misuse`, `js/index-of-positive-check`, `js/invalid-regexp`, `js/invalid-replacement-reference`, `js/invariant-return`, `js/invisible-character`, `js/jsx-no-array-index-key`, `js/jsx-no-leaked-render`, `js/jsx-no-unstable-key`, `js/max-nested-functions`, `js/misleading-array-mutation`, `js/misleading-if-layout`, `js/misleading-indentation`, `js/mutable-export`, `js/mutable-public-static-field`, `js/naming-convention`, `js/negated-comparison`, `js/no-case-declarations`, `js/no-code-after-done`, `js/no-compound-case-label`, `js/no-const-assign`, `js/no-delete-var`, `js/no-dupe-disjunctions`, `js/no-duplicate-imports`, `js/no-duplicated-branches`, `js/no-empty-alternative`, `js/no-empty-character-class`, `js/no-exclusive-tests`, `js/no-extend-native`, `js/no-extraneous-class`, `js/no-for-in-array`, `js/no-global-assign`, `js/no-import-assign`, `js/no-infinite-loop`, `js/no-labels`, `js/no-loop-func`, `js/no-magic-numbers`, `js/no-misleading-character-class`, `js/no-misused-promises`, `js/no-multi-str`, `js/no-nested-component-definition`, `js/no-new-native-nonconstructor`, `js/no-obj-calls`, `js/no-object-constructor`, `js/no-prototype-builtins`, `js/no-redeclare`, `js/no-redundant-type-constituents`, `js/no-self-import`, `js/no-self-state-update`, `js/no-set-state-in-render`, `js/no-shadow-restricted-names`, `js/no-this-before-super`, `js/no-unassigned-vars`, `js/no-undeclared-assignment`, `js/no-unexpected-multiline`, `js/no-unknown-property`, `js/no-unreachable-loop`, `js/no-unsafe-function-type`, `js/no-unsafe-optional-chaining`, `js/no-unused-private-class-members`, `js/no-use-before-define`, `js/no-useless-catch`, `js/no-useless-escape`, `js/not-callable`, `js/one-var`, `js/overwritten-before-read`, `js/prefer-array-some`, `js/prefer-arrow-callback`, `js/prefer-for-of`, `js/prefer-number-isnan`, `js/prefer-readonly`, `js/prefer-rest-params`, `js/prefer-spread`, `js/prefer-type-predicate`, `js/preserve-caught-error`, `js/reduce-without-initial-value`, `js/redundant-nested-condition`, `js/redundant-regex-syntax`, `js/regex-control-character`, `js/regex-lazy-quantifier-at-end`, `js/regex-too-complex`, `js/require-yield`, `js/reversed-compound-operator`, `js/self-comparing-assertion`, `js/skipped-test-without-reason`, `js/sort-without-comparator`, `js/stateful-regex-test`, `js/swapped-arguments`, `js/test-file-without-tests`, `js/test-retries`, `js/test-without-assertion`, `js/top-level-this`, `js/triple-slash-reference`, `js/unawaited-promise-in-try`, `js/unicode-escape-needs-flag`, `js/unread-collection`, `js/unspecific-throw-assertion`, `js/useless-increment`, `js/useless-regex-quantifier`, `js/valid-expect`, `js/void-result-used`.

| Rule | Severity | Default | Findings in the samples |
|---|---|---|---|
| `js/accessor-field-mismatch` | MEDIUM | on (incubating) | — |
| `js/accessor-pairs` | MEDIUM | on | — |
| `js/adjacent-overload-signatures` | LOW | on | 1 (ts) |
| `js/array-callback-return` | HIGH | on | 1 (ts) |
| `js/array-type` | LOW | off | — |
| `js/assert-argument-order` | LOW | on | — |
| `js/assignment-in-operand` | LOW | off | — |
| `js/async-in-constructor` | MEDIUM | on (incubating) | — |
| `js/ban-ts-comment` | MEDIUM | on | 1 (ts) |
| `js/bitwise-on-booleans` | MEDIUM | on (incubating) | — |
| `js/block-scoped-var` | LOW | off | — |
| `js/callback-extra-arguments` | HIGH | on (incubating) | — |
| `js/cognitive-complexity` | MEDIUM | on | 2 (ts) |
| `js/collapsible-if` | LOW | on | 3 (ts) |
| `js/commented-out-code` | LOW | on | 1 (ts) |
| `js/consistent-return` | MEDIUM | on | — |
| `js/consistent-type-assertions` | LOW | off | — |
| `js/const-enum` | LOW | off | — |
| `js/constant-length-comparison` | HIGH | on (incubating) | — |
| `js/constructor-super` | HIGH | on (incubating) | — |
| `js/curly` | LOW | off | — |
| `js/cyclomatic-complexity` | LOW | off | 1 (ts) |
| `js/deep-nesting` | MEDIUM | on | 1 (ts) |
| `js/default-case` | LOW | off | 1 (ts) |
| `js/default-case-last` | LOW | off | — |
| `js/default-export` | LOW | off | — |
| `js/default-param-last` | LOW | on | — |
| `js/double-assertion` | LOW | off | — |
| `js/duplicated-block` | LOW | on | 2 (ts) |
| `js/empty-collection-access` | HIGH | on (incubating) | — |
| `js/enum-initializers` | LOW | on (incubating) | — |
| `js/eqeqeq` | MEDIUM | on | 3 (ts) |
| `js/exception-not-thrown` | HIGH | on | — |
| `js/extra-arguments` | MEDIUM | on | — |
| `js/file-too-long` | LOW | on | — |
| `js/fixme-comment` | MEDIUM | on | 1 (ts) |
| `js/floating-promise` | MEDIUM | off | — |
| `js/for-direction` | HIGH | on | 1 (ts) |
| `js/for-equality-termination` | MEDIUM | on (incubating) | — |
| `js/for-loop-counter-misuse` | MEDIUM | on (incubating) | — |
| `js/function-too-long` | LOW | on | 1 (ts) |
| `js/getter-return` | HIGH | on | 1 (ts) |
| `js/identical-operands` | MEDIUM | on | — |
| `js/ignored-call-result` | MEDIUM | on (incubating) | — |
| `js/immediately-returned-variable` | LOW | on | — |
| `js/in-operator-misuse` | HIGH | on (incubating) | — |
| `js/incoming-value-overwritten` | MEDIUM | on (incubating) | 1 (ts) |
| `js/index-of-positive-check` | MEDIUM | on | — |
| `js/invalid-regexp` | HIGH | on | — |
| `js/invalid-replacement-reference` | HIGH | on (incubating) | — |
| `js/invariant-return` | MEDIUM | on (incubating) | — |
| `js/invisible-character` | MEDIUM | on (incubating) | — |
| `js/jsx-key` | HIGH | on | 1 (ts) |
| `js/jsx-no-array-index-key` | MEDIUM | on | — |
| `js/jsx-no-duplicate-props` | HIGH | on | 1 (ts) |
| `js/jsx-no-leaked-render` | MEDIUM | on (incubating) | — |
| `js/jsx-no-unstable-key` | MEDIUM | on (incubating) | — |
| `js/max-nested-functions` | MEDIUM | on | — |
| `js/misleading-array-mutation` | MEDIUM | on | — |
| `js/misleading-if-layout` | MEDIUM | on (incubating) | — |
| `js/misleading-indentation` | MEDIUM | on (incubating) | — |
| `js/mutable-export` | LOW | off | — |
| `js/mutable-public-static-field` | MEDIUM | on | — |
| `js/naming-convention` | LOW | off | — |
| `js/negated-comparison` | LOW | off | — |
| `js/no-alert` | LOW | on | 1 (ts) |
| `js/no-array-constructor` | LOW | on | 1 (ts) |
| `js/no-array-delete` | MEDIUM | on | 1 (ts) |
| `js/no-async-promise-executor` | HIGH | on | 1 (ts) |
| `js/no-case-declarations` | MEDIUM | on | — |
| `js/no-code-after-done` | MEDIUM | on (incubating) | — |
| `js/no-compare-neg-zero` | HIGH | on | 1 (ts) |
| `js/no-compound-case-label` | HIGH | on (incubating) | — |
| `js/no-cond-assign` | MEDIUM | on | 1 (ts) |
| `js/no-confusing-non-null-assertion` | MEDIUM | on | 2 (ts) |
| `js/no-console` | LOW | on | 1 (ts) |
| `js/no-const-assign` | HIGH | on (incubating) | — |
| `js/no-constant-condition` | MEDIUM | on | 1 (ts) |
| `js/no-debugger` | MEDIUM | on | 1 (ts) |
| `js/no-delete-var` | MEDIUM | on (incubating) | — |
| `js/no-dupe-class-members` | HIGH | on | 1 (ts) |
| `js/no-dupe-disjunctions` | MEDIUM | on (incubating) | — |
| `js/no-dupe-else-if` | HIGH | on | 1 (ts) |
| `js/no-dupe-keys` | HIGH | on | 1 (ts) |
| `js/no-duplicate-case` | HIGH | on | 1 (ts) |
| `js/no-duplicate-enum-values` | HIGH | on | 1 (ts) |
| `js/no-duplicate-imports` | LOW | on | — |
| `js/no-duplicate-string` | LOW | on | 2 (ts) |
| `js/no-duplicated-branches` | MEDIUM | on | — |
| `js/no-else-return` | LOW | off | 1 (ts) |
| `js/no-empty` | MEDIUM | on | 1 (ts) |
| `js/no-empty-alternative` | MEDIUM | on (incubating) | — |
| `js/no-empty-character-class` | HIGH | on (incubating) | — |
| `js/no-empty-function` | LOW | on | 1 (ts) |
| `js/no-empty-interface` | LOW | on | 1 (ts) |
| `js/no-empty-pattern` | MEDIUM | on | 1 (ts) |
| `js/no-eval` | MEDIUM | on | 1 (ts) |
| `js/no-ex-assign` | MEDIUM | on | 1 (ts) |
| `js/no-exclusive-tests` | HIGH | on | — |
| `js/no-explicit-any` | LOW | off | 1 (ts) |
| `js/no-extend-native` | MEDIUM | on | — |
| `js/no-extra-boolean-cast` | LOW | on | 1 (ts) |
| `js/no-extra-non-null-assertion` | MEDIUM | on | 1 (ts) |
| `js/no-extraneous-class` | LOW | off | — |
| `js/no-fallthrough` | MEDIUM | on | 1 (ts) |
| `js/no-for-in-array` | MEDIUM | on (incubating) | — |
| `js/no-func-assign` | HIGH | on | 1 (ts) |
| `js/no-global-assign` | HIGH | on (incubating) | — |
| `js/no-identical-functions` | MEDIUM | on | 1 (ts) |
| `js/no-import-assign` | HIGH | on (incubating) | — |
| `js/no-inferrable-types` | LOW | off | 2 (ts) |
| `js/no-infinite-loop` | MEDIUM | on (incubating) | — |
| `js/no-labels` | LOW | on | — |
| `js/no-lone-blocks` | LOW | on | 1 (ts) |
| `js/no-lonely-if` | LOW | on | 1 (ts) |
| `js/no-loop-func` | HIGH | on (incubating) | — |
| `js/no-loss-of-precision` | HIGH | on | 1 (ts) |
| `js/no-magic-numbers` | LOW | off | — |
| `js/no-misleading-character-class` | MEDIUM | on (incubating) | — |
| `js/no-misused-new` | HIGH | on | 1 (ts) |
| `js/no-misused-promises` | MEDIUM | on (incubating) | — |
| `js/no-mixed-enums` | MEDIUM | on | 1 (ts) |
| `js/no-multi-assign` | LOW | on | 1 (ts) |
| `js/no-multi-str` | LOW | off | — |
| `js/no-namespace` | LOW | off | 1 (ts) |
| `js/no-nested-component-definition` | MEDIUM | on | — |
| `js/no-nested-ternary` | LOW | on | 1 (ts) |
| `js/no-new-native-nonconstructor` | HIGH | on (incubating) | — |
| `js/no-new-wrappers` | MEDIUM | on | 1 (ts) |
| `js/no-non-null-assertion` | LOW | off | 3 (ts) |
| `js/no-obj-calls` | HIGH | on (incubating) | — |
| `js/no-object-constructor` | LOW | off | — |
| `js/no-param-reassign` | LOW | off | 1 (ts) |
| `js/no-prototype-builtins` | MEDIUM | on | — |
| `js/no-redeclare` | HIGH | on | — |
| `js/no-redundant-boolean` | LOW | on | 1 (ts) |
| `js/no-redundant-type-constituents` | LOW | on | — |
| `js/no-require-imports` | LOW | on | 1 (ts) |
| `js/no-return-assign` | MEDIUM | off | 1 (ts) |
| `js/no-self-assign` | HIGH | on | 1 (ts) |
| `js/no-self-compare` | HIGH | on | 1 (ts) |
| `js/no-self-import` | MEDIUM | on (incubating) | — |
| `js/no-self-state-update` | MEDIUM | on | — |
| `js/no-set-state-in-render` | HIGH | on (incubating) | — |
| `js/no-shadow-restricted-names` | HIGH | on | — |
| `js/no-sparse-arrays` | MEDIUM | on | 1 (ts) |
| `js/no-template-curly-in-string` | MEDIUM | on | 1 (ts) |
| `js/no-this-alias` | LOW | on | 1 (ts) |
| `js/no-this-before-super` | HIGH | on (incubating) | — |
| `js/no-throw-literal` | MEDIUM | on | 1 (ts) |
| `js/no-unassigned-vars` | MEDIUM | on | — |
| `js/no-undeclared-assignment` | HIGH | on | — |
| `js/no-unexpected-multiline` | MEDIUM | on (incubating) | — |
| `js/no-unknown-property` | MEDIUM | on (incubating) | — |
| `js/no-unnecessary-type-constraint` | LOW | on | 1 (ts) |
| `js/no-unreachable` | HIGH | on | 1 (ts) |
| `js/no-unreachable-loop` | MEDIUM | on (incubating) | — |
| `js/no-unsafe-finally` | HIGH | on | 1 (ts) |
| `js/no-unsafe-function-type` | LOW | on | — |
| `js/no-unsafe-negation` | HIGH | on | 1 (ts) |
| `js/no-unsafe-optional-chaining` | HIGH | on | — |
| `js/no-unused-expressions` | MEDIUM | on | 1 (ts) |
| `js/no-unused-private-class-members` | MEDIUM | on (incubating) | — |
| `js/no-unused-vars` | MEDIUM | on | 4 (ts) |
| `js/no-use-before-define` | HIGH | on (incubating) | — |
| `js/no-useless-assignment` | MEDIUM | on | 1 (ts) |
| `js/no-useless-catch` | LOW | on (incubating) | — |
| `js/no-useless-concat` | LOW | on | 1 (ts) |
| `js/no-useless-constructor` | LOW | on | 1 (ts) |
| `js/no-useless-empty-export` | LOW | on | 1 (ts) |
| `js/no-useless-escape` | LOW | on | — |
| `js/no-useless-rename` | LOW | on | 1 (ts) |
| `js/no-useless-return` | LOW | on | 1 (ts) |
| `js/no-var` | LOW | on | 4 (ts) |
| `js/no-wrapper-object-types` | MEDIUM | on | 1 (ts) |
| `js/not-callable` | HIGH | on (incubating) | — |
| `js/one-var` | LOW | off | — |
| `js/overwritten-before-read` | HIGH | on | — |
| `js/prefer-array-some` | LOW | on | — |
| `js/prefer-arrow-callback` | LOW | off | — |
| `js/prefer-as-const` | LOW | on | 1 (ts) |
| `js/prefer-const` | LOW | on | 2 (ts) |
| `js/prefer-for-of` | LOW | off | — |
| `js/prefer-number-isnan` | MEDIUM | on | — |
| `js/prefer-readonly` | LOW | off | — |
| `js/prefer-rest-params` | LOW | on | — |
| `js/prefer-spread` | LOW | on | — |
| `js/prefer-template` | LOW | off | 1 (ts) |
| `js/prefer-type-predicate` | LOW | on (incubating) | — |
| `js/preserve-caught-error` | MEDIUM | on | — |
| `js/reduce-without-initial-value` | MEDIUM | on (incubating) | — |
| `js/redundant-nested-condition` | MEDIUM | on (incubating) | — |
| `js/redundant-regex-syntax` | LOW | on | — |
| `js/regex-control-character` | MEDIUM | on (incubating) | — |
| `js/regex-lazy-quantifier-at-end` | MEDIUM | on (incubating) | — |
| `js/regex-too-complex` | MEDIUM | on | — |
| `js/require-yield` | MEDIUM | on (incubating) | — |
| `js/reversed-compound-operator` | HIGH | on (incubating) | — |
| `js/rules-of-hooks` | HIGH | on (incubating) | 2 (ts) |
| `js/self-comparing-assertion` | MEDIUM | on | — |
| `js/skipped-test-without-reason` | LOW | on | — |
| `js/sort-without-comparator` | MEDIUM | on | — |
| `js/stateful-regex-test` | HIGH | on (incubating) | — |
| `js/swapped-arguments` | MEDIUM | on (incubating) | — |
| `js/test-file-without-tests` | LOW | on (incubating) | — |
| `js/test-retries` | LOW | on (incubating) | — |
| `js/test-without-assertion` | MEDIUM | on | — |
| `js/todo-comment` | LOW | on | 1 (ts) |
| `js/too-many-parameters` | MEDIUM | on | 1 (ts) |
| `js/top-level-this` | MEDIUM | on (incubating) | — |
| `js/triple-slash-reference` | LOW | off | — |
| `js/unawaited-promise-in-try` | MEDIUM | on (incubating) | — |
| `js/unicode-escape-needs-flag` | HIGH | on (incubating) | — |
| `js/unread-collection` | MEDIUM | on | — |
| `js/unspecific-throw-assertion` | MEDIUM | on | — |
| `js/use-isnan` | HIGH | on | 1 (ts) |
| `js/useless-increment` | HIGH | on (incubating) | — |
| `js/useless-regex-quantifier` | MEDIUM | on (incubating) | — |
| `js/valid-expect` | MEDIUM | on | — |
| `js/valid-typeof` | HIGH | on | 1 (ts) |
| `js/void-result-used` | MEDIUM | on (incubating) | — |

</details>

<details>
<summary>Engine (meta rules): 2 of 5 rules exercised</summary>

Not exercised: `mote/internal-error`, `mote/parse-error`, `mote/unused-suppression`.

| Rule | Severity | Default | Findings in the samples |
|---|---|---|---|
| `mote/blanket-suppression` | LOW | on | 1 (ts) |
| `mote/internal-error` | LOW | on | — |
| `mote/parse-error` | LOW | on | — |
| `mote/suppression-without-reason` | LOW | on | 1 (ts) |
| `mote/unused-suppression` | LOW | on | — |

</details>

<!-- coverage:end -->
