//! Team workflows with the `.tl` test plugin: triage decisions, baselines, comparing results,
//! trend records, other CI hosts and migration aids.

mod common;

use common::*;
use serde_json::Value;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("invalid JSON ({e}): {text}"))
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn finding<'a>(results: &'a Value, rule: &str) -> &'a Value {
    results["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rule_id"] == rule)
        .unwrap_or_else(|| panic!("no {rule} finding"))
}

#[test]
fn triage_add_list_prune_and_scan() {
    let d = tmp();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\nLOUD\n");
    let r = mote(root, &["scan", "--no-cache", "-q", "--json", "r1.json"]);
    assert_eq!(r.code, 1, "the HIGH finding fails the gate: {r:?}");
    let v = json(&read(root, "r1.json"));
    assert_eq!(v["triage"], Value::Null, "no triage file yet");
    let id = finding(&v, "tl/high-bug")["id"]
        .as_str()
        .unwrap()
        .to_string();

    let add = |status: &str| {
        mote(
            root,
            &[
                "triage",
                "add",
                &id,
                "--status",
                status,
                "--reason",
                "the marker is a test fixture",
                "--results",
                "r1.json",
                "--by",
                "alice",
                "--date",
                "2026-09-28",
            ],
        )
    };
    let r = add("false-positive");
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout
            .starts_with("Added: false-positive tl/high-bug in a.tl"),
        "{r:?}"
    );
    let first = read(root, ".mote-triage.yaml");
    assert!(first.contains("status: false-positive"), "{first}");
    assert!(
        first.contains("    by: \"alice\"\n    date: \"2026-09-28\""),
        "{first}"
    );
    let r = add("accepted");
    assert!(r.stdout.starts_with("Updated: accepted"), "{r:?}");
    let text = read(root, ".mote-triage.yaml");
    assert_eq!(text.matches("fingerprint:").count(), 1, "replaced: {text}");
    assert_eq!(text, first.replace("false-positive", "accepted"));

    // The scan honors the decision: the gate passes, JSON and SARIF carry it.
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "--json",
            "r2.json",
            "--sarif",
            "r.sarif",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let v = json(&read(root, "r2.json"));
    let sup = &finding(&v, "tl/high-bug")["suppressed"];
    assert_eq!(sup["kind"], "accepted");
    assert_eq!(sup["justification"], "the marker is a test fixture");
    assert_eq!(v["triage"]["matched"], 1);
    let sarif = json(&read(root, "r.sarif"));
    let res = sarif["runs"][0]["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["ruleId"] == "tl/high-bug")
        .unwrap()
        .clone();
    assert_eq!(res["suppressions"][0]["kind"], "external");

    let r = mote(root, &["triage", "list", "--results", "r2.json"]);
    assert!(
        r.stdout.contains("[matched] accepted tl/high-bug in a.tl"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("the marker is a test fixture (alice, 2026-09-28)"),
        "{r:?}"
    );
    let r = mote(root, &["triage", "list", "--format", "json"]);
    let l = json(&r.stdout);
    assert_eq!(l["entries"][0]["status"], "accepted");
    assert_eq!(l["entries"][0]["state"], Value::Null, "no results given");

    // The flagged line changes: the entry goes stale and is reported on the triage file.
    write(root, "a.tl", "LOUD\n");
    let r = mote(root, &["scan", "--no-cache", "--json", "r3.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains(".mote-triage.yaml"),
        "stale entry reported: {r:?}"
    );
    let v = json(&read(root, "r3.json"));
    let stale = finding(&v, "mote/unused-suppression");
    assert_eq!(stale["path"], ".mote-triage.yaml");
    assert!(stale["message"]
        .as_str()
        .unwrap()
        .contains("stale triage entry"));
    let r = mote(root, &["triage", "list", "--results", "r3.json"]);
    assert!(r.stdout.contains("[stale] accepted"), "{r:?}");
    let r = mote(
        root,
        &["triage", "prune", "--results", "r3.json", "--dry-run"],
    );
    assert!(r.stdout.starts_with("Would remove: accepted"), "{r:?}");
    assert_eq!(read(root, ".mote-triage.yaml"), text, "dry run");
    let r = mote(root, &["triage", "prune", "--results", "r3.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("0 of 1 entries left"), "{r:?}");
    assert!(read(root, ".mote-triage.yaml").ends_with("entries: []\n"));

    // Mistakes are usage errors.
    for args in [
        &[
            "triage",
            "add",
            "xyz",
            "--status",
            "accepted",
            "--reason",
            "r",
            "--results",
            "r1.json",
        ][..],
        &[
            "triage",
            "add",
            "000000000000",
            "--status",
            "accepted",
            "--reason",
            "r",
            "--results",
            "r1.json",
        ],
        &[
            "triage",
            "add",
            &id,
            "--status",
            "accepted",
            "--reason",
            " ",
            "--results",
            "r1.json",
        ],
        &[
            "triage",
            "add",
            &id,
            "--status",
            "accepted",
            "--reason",
            "r",
            "--results",
            "r1.json",
            "--date",
            "28.9.2026",
        ],
        &[
            "triage",
            "add",
            &id,
            "--status",
            "maybe",
            "--reason",
            "r",
            "--results",
            "r1.json",
        ],
        &["triage", "prune", "--results", "r1.json"],
    ] {
        let r = mote(root, args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
    }
    write(root, ".mote-triage.yaml", "entries: [oops\n");
    let r = mote(root, &["scan", "--no-cache"]);
    assert_eq!(r.code, 2, "a broken triage file is a usage error: {r:?}");
    assert!(r.stderr.contains("invalid triage file"), "{r:?}");
}

#[test]
fn scan_against_a_baseline_file() {
    let d = tmp();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\n");
    let r = mote(
        root,
        &["scan", "--no-cache", "--no-gate", "--json", "base.json"],
    );
    assert_eq!(r.code, 0, "{r:?}");

    // Old debt does not fail the gate; a new HIGH finding does.
    let r = mote(root, &["scan", "--no-cache", "--baseline", "base.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout
            .contains("vs baseline base.json: 1 of its 1 findings still present"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("baseline results file, which has no line diff"),
        "{r:?}"
    );
    write(root, "b.tl", "HIGHBUG\n");
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "--baseline",
            "base.json",
            "--format",
            "jsonl",
        ],
    );
    assert_eq!(r.code, 1, "{r:?}");
    let statuses: Vec<(String, String)> = r
        .stdout
        .lines()
        .map(json)
        .map(|l| {
            (
                l["path"].as_str().unwrap().to_string(),
                l["status"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        statuses,
        vec![
            ("a.tl".to_string(), "unchanged".to_string()),
            ("b.tl".to_string(), "new".to_string())
        ]
    );
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "--baseline",
            "base.json",
            "--new-only",
            "--format",
            "json",
        ],
    );
    let v = json(&r.stdout);
    assert_eq!(v["new_code"]["baseline"]["file"], "base.json");
    assert_eq!(v["new_code"]["measures"]["new_findings"]["high"], 1);

    // `mote gate` re-evaluates a baseline result the same way.
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "-q",
            "--baseline",
            "base.json",
            "--json",
            "head.json",
        ],
    );
    assert_eq!(r.code, 1);
    let r = mote(root, &["gate", "--results", "head.json"]);
    assert_eq!(r.code, 1, "{r:?}");

    for args in [
        &["scan", "--baseline", "base.json", "--base", "main"][..],
        &["scan", "--baseline", "missing.json"],
        &["scan", "--baseline", "a.tl"],
    ] {
        let r = mote(root, args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
    }
}

#[test]
fn compare_two_results_and_write_trend_records() {
    let d = tmp();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\nLOUD\n");
    write(root, "b.tl", "MAGIC\n");
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "--no-gate",
            "--json",
            "old.json",
            "--summary-json",
            "trend/1.json",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    // The next revision: the HIGH finding fixed, a new MEDIUM one, b.tl renamed.
    write(root, "a.tl", "LOUD\nLOUD\n");
    std::fs::rename(root.join("b.tl"), root.join("c.tl")).unwrap();
    let mut cmd = std::process::Command::new(BIN);
    isolate(&mut cmd)
        .current_dir(root)
        .env("SOURCE_DATE_EPOCH", "1790553600")
        .args([
            "scan",
            "--no-cache",
            "--no-gate",
            "--json",
            "new.json",
            "--summary-json",
            "trend/2.json",
        ]);
    assert!(cmd.output().unwrap().status.success());

    let rec = read(root, "trend/2.json");
    assert_eq!(rec.lines().count(), 1, "one line per record: {rec}");
    let v = json(&rec);
    assert_eq!(v["summary_version"], 1);
    assert_eq!(v["date"], "2026-09-28T00:00:00Z", "SOURCE_DATE_EPOCH");
    assert_eq!(v["findings"]["medium"], 2);
    assert_eq!(v["findings"]["high"], 0);
    assert!(v["measures"]["ncloc"].as_u64().is_some());
    assert!(v["grades"]["maintainability"]["grade"].is_string());
    let first = json(&read(root, "trend/1.json"));
    assert!(first["date"].as_str().unwrap().ends_with('Z'));

    let r = mote(root, &["compare", "old.json", "new.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains(
            "Findings: 1 added, 1 fixed, 0 with another severity, 0 newly suppressed, 0 no longer suppressed, 2 unchanged"
        ),
        "{r:?}"
    );
    assert!(
        r.stdout.contains("Added (1)\n  MEDIUM   a.tl:2  tl/loud"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("Fixed (1)\n  HIGH     a.tl:1  tl/high-bug"),
        "{r:?}"
    );
    let r = mote(
        root,
        &["compare", "old.json", "new.json", "--format", "json"],
    );
    let v = json(&r.stdout);
    assert_eq!(v["counts"]["added"], 1);
    assert_eq!(v["counts"]["fixed"], 1);
    assert!(v["warnings"].as_array().unwrap().is_empty(), "{v}");
    let r = mote(
        root,
        &[
            "compare", "old.json", "new.json", "--format", "markdown", "-o", "cmp.md",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(read(root, "cmp.md").contains("### Fixed (1)"));

    // A rule set change is pointed out.
    write(root, ".mote.yaml", "rules: { tl/magic: off }\n");
    mote(
        root,
        &["scan", "--no-cache", "--no-gate", "--json", "fewer.json"],
    );
    let r = mote(
        root,
        &["compare", "new.json", "fewer.json", "--format", "json"],
    );
    assert!(r.stderr.contains("different rules"), "{r:?}");
    let r = mote(root, &["compare", "old.json", "missing.json"]);
    assert_eq!(r.code, 2, "{r:?}");
}

#[test]
fn bitbucket_code_insights_files() {
    let d = tmp();
    let root = d.path();
    init_repo(root);
    write(root, "svc/a.tl", "HIGHBUG\n");
    commit_all(root, "init");
    let r = mote(
        &root.join("svc"),
        &["scan", "--no-cache", "--bitbucket-insights", "../insights"],
    );
    assert_eq!(r.code, 1, "{r:?}");
    let report = json(&read(root, "insights/report.json"));
    assert_eq!(report["result"], "FAILED");
    assert_eq!(report["report_type"], "BUG");
    let ann = json(&read(root, "insights/annotations.json"));
    let a = &ann.as_array().unwrap()[0];
    assert_eq!(a["path"], "svc/a.tl", "repository-relative");
    assert_eq!(a["annotation_type"], "BUG");
    assert_eq!(a["severity"], "HIGH");
    assert_eq!(a["line"], 1);
}

fn fixture(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/migrate")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// A fixture copied to the top of `dir` (where the proposal applies), by its file name.
fn copied(dir: &std::path::Path, name: &str) -> String {
    let base = std::path::Path::new(name)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write(dir, &base, &std::fs::read_to_string(fixture(name)).unwrap());
    base
}

/// The proposal must be a configuration `mote config validate` accepts.
fn assert_valid_config(dir: &std::path::Path, text: &str) {
    write(dir, "proposed.yaml", text);
    let r = mote(dir, &["config", "validate", "--config", "proposed.yaml"]);
    assert_eq!(r.code, 0, "{text}\n{r:?}");
}

#[test]
fn migrate_from_a_pmd_ruleset() {
    let d = tmp();
    let ruleset = fixture("pmd-ruleset.xml");
    let r = mote(d.path(), &["migrate", "--from", "pmd", &ruleset]);
    assert_eq!(r.code, 0, "{r:?}");
    let y = &r.stdout;
    for want in [
        "extends: default\n",
        "  - \"**/generated/**\"  # <exclude-pattern>.*/generated/.*</exclude-pattern>\n",
        "  java/empty-catch-block: on      # EmptyCatchBlock\n",
        "  java/unused-private-field: HIGH    # UnusedPrivateField, priority 1\n",
        "  java/unused-import: LOW     # UnnecessaryImport, priority 5\n",
        "  java/empty-block: on      # EmptyIfStmt\n",
        "  java/field-could-be-local: off     # SingularField excluded from category/java/design.xml\n",
        "suppressions:\n  compat: [pmd]  # honor the suppression comments and annotations pmd reads\n",
        "  # other tools' markers in your code? add their sources: checkstyle, eslint, intellij, nosonar, spotbugs, suppress-warnings-all; PMD also honors `@SuppressWarnings(\"unused\")`: the intellij source covers it (docs/migration.md)\n",
        ":\n#   (the mapping tables cover PMD's quickstart ruleset and the other PMD rules with a clear equivalent; for a rule they miss, `mote rules --all` may still list a Mote rule for the same issue)\n",
        // Rules outside the quickstart ruleset, and thresholds carried over as parameters (PMD
        // reports from its level on, Mote above its threshold).
        // A property the ruleset does not set keeps PMD's default, not Mote's own.
        "  java/duplicate-string-literal: { params: { min_length: 3, threshold: 5 } } # AvoidDuplicateLiterals, PMD's default minimumLength=3\n",
        "  java/too-many-methods: { params: { max: 10 } } # TooManyMethods, PMD's default maxmethods=10\n",
        "  java/cognitive-complexity: { params: { threshold: 14 } } # CognitiveComplexity, PMD's default reportLevel=15 is threshold 14 (reported above it)\n",
        "  java/system-out-print: on      # SystemPrintln\n",
        "  java/cyclomatic-complexity: { params: { threshold: 11 } } # CyclomaticComplexity, methodReportLevel=12 is threshold 11 (reported above it)\n",
        "  java/too-many-parameters: { params: { max: 5 } } # ExcessiveParameterList, minimum=6 is max 5 (reported above it)\n",
        "  java/too-many-assertions: { params: { max: 3 } } # JUnitTestContainsTooManyAsserts\n",
        "#   - properties of CyclomaticComplexity not carried over: classReportLevel=90\n",
        "#   - category/java/design.xml: a whole category or ruleset is not expanded",
        "#   - category/apex/errorprone.xml/EmptyCatchBlock: not a Java rule\n",
        "#   - custom rule NoSystemExit",
        "#   - <exclude-pattern>.*/src/main/java/legacy/(a|b)/.*</exclude-pattern>: not translatable",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert!(r.stderr.contains("mapped 12 of 15 rules"), "{r:?}");
    assert_valid_config(d.path(), y);

    // -o writes the file (never over an existing one without --force) and lists what is left.
    let out = d.path().join(".mote.yaml");
    let out = out.to_string_lossy().to_string();
    let args = [
        "migrate",
        "--from",
        "pmd",
        ruleset.as_str(),
        "-o",
        out.as_str(),
    ];
    let r = mote(d.path(), &args);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains("Wrote ") && r.stdout.contains("Not mapped:"),
        "{r:?}"
    );
    assert_eq!(read(d.path(), ".mote.yaml"), *y);
    let r = mote(d.path(), &args);
    assert_eq!(r.code, 2, "refuses to overwrite: {r:?}");
    assert!(
        r.stderr
            .contains("copy its `rules`, `exclude` and `overrides` into yours"),
        "{r:?}"
    );
    // Printed next to an existing configuration (one from `mote init`), it says how to use it.
    let r = mote(d.path(), &["migrate", "--from", "pmd", ruleset.as_str()]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stderr
            .contains("note: .mote.yaml exists here: copy the proposal's `rules`"),
        "{r:?}"
    );
    let mut forced = args.to_vec();
    forced.push("--force");
    let r = mote(d.path(), &forced);
    assert_eq!(r.code, 0, "{r:?}");

    write(d.path(), "broken.xml", "<ruleset><rule");
    let r = mote(d.path(), &["migrate", "--from", "pmd", "broken.xml"]);
    assert_eq!(r.code, 2, "{r:?}");
}

#[test]
fn migrate_from_eslint_configs() {
    let d = tmp();
    let r = mote(
        d.path(),
        &[
            "migrate",
            "--from",
            "eslint",
            &copied(d.path(), "eslintrc.json"),
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let y = &r.stdout;
    for want in [
        "  - \"**/dist/**\"  # ignorePatterns: dist/\n",
        "# From ESLint's ignores, which only concerned JavaScript and TypeScript files:\n",
        "  compat: [eslint]  # honor the suppression comments and annotations eslint reads\n",
        "  # other tools' markers in your code? add their sources: checkstyle, intellij, nosonar, pmd, spotbugs, suppress-warnings-all (docs/migration.md)\n",
        "  js/eqeqeq: on      # eqeqeq\n",
        "  js/no-console: on      # no-console: warn\n",
        "  js/no-self-compare: MEDIUM  # no-self-compare: warn\n",
        "  js/no-unused-vars: on      # @typescript-eslint/no-unused-vars, no-unused-vars\n",
        "  js/no-explicit-any: off     # @typescript-eslint/no-explicit-any\n",
        "  js/no-empty-interface: on      # @typescript-eslint/no-empty-object-type\n",
        "  js/cyclomatic-complexity: { params: { threshold: 12 } } # complexity: warn\n",
        "  js/deep-nesting: { params: { max: 5 } } # max-depth\n",
        "  - paths: [\"**/*.test.ts\", \"test/**/*.ts\"]  # overrides[0].files\n",
        "      js/no-console: off     # no-console\n",
        "#   - extends `eslint:recommended`: presets are not expanded",
        "#   - import/no-cycle: no Mote equivalent\n",
        "#   - 1 formatting rule, a formatter's job (Mote checks no layout): semi\n",
        "#   - options of eqeqeq not carried over",
        "#   - ignorePatterns: negations re-include files, which an exclude cannot express: `!keep.min.js`\n",
        // `!keep.min.js` brings back a file `*.min.js` ignores: an exclude cannot.
        "#   - ignorePatterns: `*.min.js` not carried over: a later negation (`!keep.min.js`) re-includes part of what it ignores",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert!(!y.contains("\"**/*.min.js\""), "{y}");
    assert_valid_config(d.path(), y);

    // Jenkins' flat config ignores every directory, then re-includes its source directories: a
    // translation of `*/**` would exclude the whole repository (Java included).
    let r = mote(
        d.path(),
        &[
            "migrate",
            "--from",
            "eslint",
            &copied(d.path(), "flat-config-negations.json"),
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(!r.stdout.contains("\"*/**\""), "{}", r.stdout);
    for want in [
        "  - \"**/target/**\"  # config[0].ignores: **/target/\n",
        "#   - config[0].ignores: `*/**` not carried over: a later negation (`!core/**`, …)",
    ] {
        assert!(
            r.stdout.contains(want),
            "missing `{want}` in:\n{}",
            r.stdout
        );
    }
    assert_valid_config(d.path(), &r.stdout);

    let r = mote(
        d.path(),
        &[
            "migrate",
            "--from",
            "eslint",
            &copied(d.path(), "flat-config.json"),
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    for want in [
        "  - \"build/**\"  # config[0].ignores: build/**\n",
        "  js/no-debugger: on      # no-debugger\n",
        "  js/no-var: off     # no-var\n",
        "  - paths: [\"scripts/**/*.js\"]  # config[2].files\n",
        "#   - no-process-exit (config[2]): no Mote equivalent\n",
    ] {
        assert!(
            r.stdout.contains(want),
            "missing `{want}` in:\n{}",
            r.stdout
        );
    }
    assert_valid_config(d.path(), &r.stdout);

    let r = mote(
        d.path(),
        &[
            "migrate",
            "--from",
            "eslint",
            &copied(d.path(), "eslintrc.yaml"),
        ],
    );
    assert!(
        r.stdout
            .contains("  js/prefer-const: on      # prefer-const: warn\n"),
        "{r:?}"
    );
    assert_valid_config(d.path(), &r.stdout);

    write(
        d.path(),
        "package.json",
        "{ \"name\": \"app\", \"eslintConfig\": { \"rules\": { \"no-var\": 2 } } }",
    );
    let r = mote(d.path(), &["migrate", "--from", "eslint", "package.json"]);
    assert!(
        r.stdout.contains("  js/no-var: on      # no-var\n"),
        "{r:?}"
    );

    write(d.path(), "bad.json", "{ nope");
    let r = mote(d.path(), &["migrate", "--from", "eslint", "bad.json"]);
    assert_eq!(r.code, 2, "{r:?}");
}

/// JavaScript and TypeScript configurations are parsed and read statically, never run: what only
/// exists at run time is listed as not statically readable, shared configurations as presets.
#[test]
fn migrate_from_javascript_eslint_configs() {
    let d = tmp();
    let migrate = |file: &str| {
        let r = mote(
            d.path(),
            &["migrate", "--from", "eslint", &copied(d.path(), file)],
        );
        assert_eq!(r.code, 0, "{r:?}");
        assert_valid_config(d.path(), &r.stdout);
        r.stdout
    };
    let y = migrate("flat-js/eslint.config.mjs");
    for want in [
        "  - \"dist/**\"  # config[0].ignores (line 20): dist/\n",
        "  js/cyclomatic-complexity: { params: { threshold: 12 } } # complexity: warn\n",
        "  js/eqeqeq: on      # eqeqeq\n",
        "  js/no-var: on      # no-var\n",
        // `...tseslint.configs.recommended` turns it on (off by default in Mote); config[4]
        // turns it off for the files it names, and config[13] on again through `strict`.
        "  js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/recommended\n",
        "      js/no-explicit-any: off     # @typescript-eslint/no-explicit-any\n",
        "  - paths: [\"**/*.ts\"]  # config[13].files (line 45)\n    rules:\n      js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/strict\n      js/no-var: on      # no-var\n",
        // `files: sourceFiles`, a const spreading two others.
        "  - paths: [\"**/*.js\", \"**/*.mjs\", \"**/*.ts\"]  # config[4].files (line 33)\n",
        // A nested array is flattened; a template literal with a static part is read.
        "  - paths: [\"test/**\"]  # config[5].files (line 37)\n",
        "  - paths: [\"scripts/**/*.js\"]  # config[6].files (line 38)\n",
        "#   - config[1]: preset `eslint:recommended` (`js.configs.recommended`, line 21) is not expanded; `extends: default` covers Mote's own selection\n",
        "#   - config[2]: preset `plugin:@typescript-eslint/recommended` (`...tseslint.configs.recommended`, line 22) is not expanded",
        "#   - config[3].rules (line 23): the rules of preset `plugin:react/recommended` (`...react.configs.flat.recommended.rules`, line 26) are not expanded",
        "#   - config[7].files (line 39): not statically readable: `custom.files` (line 39: imported from the file `./eslint/custom.js`); the settings of config[7] are not carried over\n",
        "#   - config[8] (line 40): preset `plugin:react/recommended` (`...react.configs.flat.recommended`, line 40) is not expanded (and it may replace the `files` written before it)",
        "#   - config[8].rules (line 40): not carried over: `...react.configs.flat.recommended` (line 40) may set `files`, so the files they apply to are unknown\n",
        "#   - config[9] (line 41): not statically readable: `...custom` (line 41: imported from the file `./eslint/custom.js`); what it sets is unknown\n",
        "#   - config[9].rules (line 41): not carried over: `...custom` (line 41) may set `files`",
        "#   - config[10]: not statically readable: `...(strict ? [{ rules: { 'no-alert': 'error' } }] : [])` (line 42: depends on a condition); what it configures is unknown\n",
        "#   - config[11]: not statically readable: `...shared` (line 43: modified elsewhere in the file)",
        "#   - config[12]: not statically readable: `custom.configs.recommended` (line 44: imported from the file `./eslint/custom.js`)",
        "#   - config[13].extends (line 45): preset `plugin:@typescript-eslint/strict` (`tseslint.configs.strict`, line 45) is not expanded",
        "#   - extends `js/recommended` (config[13], line 45): presets are not expanded",
        "#   - config[14]: preset `prettier` (`prettier`, line 46) is not expanded",
        "#   - no-console: not statically readable: `level` (line 29: declared with `let` or `var`, so it may change); its setting is unknown\n",
        "#   - config[0].ignores (line 20): `**/*.min.js` not carried over: a later negation (`!keep.min.js`)",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    // The settings of objects whose scope or contents are unknown are not guessed.
    assert!(
        !y.contains("js/no-debugger") && !y.contains("js/no-alert"),
        "{y}"
    );
    assert!(
        !y.contains("\"**/*.tsx\"") && !y.contains("\"**/*.min.js\""),
        "{y}"
    );

    let y = migrate("flat-ts/eslint.config.ts");
    for want in [
        "  - \"coverage/**\"  # config[0].ignores (line 12): coverage/**\n",
        "  js/no-debugger: on      # no-debugger\n",
        "  - paths: [\"src/**/*.ts\"]  # config[2].files (line 14)\n",
        "      js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: warn\n",
        "#   - config[3]: preset `plugin:@typescript-eslint/stylistic-type-checked` (`tseslint.configs.stylisticTypeChecked`, line 18) is not expanded",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }

    let y = migrate("eslintrc-js/.eslintrc.cjs");
    for want in [
        "  - \"**/build/**\"  # ignorePatterns: build/\n",
        "  js/no-var: on      # no-var\n",
        "  - paths: [\"**/*.ts\", \"**/*.tsx\"]  # overrides[0].files\n",
        "#   - extends `eslint:recommended`: presets are not expanded",
        "#   - rules: not statically readable: `...require('./rules/extra.js')` (line 14: imported from the file `./rules/extra.js`); the rules it sets are unknown\n",
        // A rule written before that spread may be set again by it.
        "#   - no-console (rules): not carried over: `...require('./rules/extra.js')` (line 14), written after it, may change its setting\n",
        "#   - prefer-const: not statically readable: `mode` (line 16: declared with `let` or `var`, so it may change)",
        "#   - overrides[1].files: not statically readable: `path.join('test', '**')` (line 20: the result of a function call); the settings of overrides[1] are not carried over\n",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert!(
        !y.contains("tsconfigRootDir") && !y.contains("__dirname"),
        "settings the migration does not use are not reported: {y}"
    );

    // The file is never run: what it would do when run does not happen.
    write(
        d.path(),
        "eslint.config.js",
        "import fs from 'node:fs';\nfs.writeFileSync('ran.txt', 'ran');\nexport default [{ rules: { 'no-console': 'error' } }];\n",
    );
    let r = mote(
        d.path(),
        &["migrate", "--from", "eslint", "eslint.config.js"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains("  js/no-console: on      # no-console\n"),
        "{r:?}"
    );
    assert!(!d.path().join("ran.txt").exists());
    // Files that cannot be read statically at all.
    write(d.path(), "eslint.config.cjs", "module.exports = [\n");
    let r = mote(
        d.path(),
        &["migrate", "--from", "eslint", "eslint.config.cjs"],
    );
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("cannot parse the file"), "{r:?}");
    write(d.path(), ".eslintrc.js", "const config = {};\n");
    let r = mote(d.path(), &["migrate", "--from", "eslint", ".eslintrc.js"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("no `export default"), "{r:?}");
    // An export that is computed: the proposal says so instead of guessing.
    write(
        d.path(),
        "eslint.config.mjs",
        "import neostandard from 'neostandard';\nexport default neostandard({ ts: true });\n",
    );
    let r = mote(
        d.path(),
        &["migrate", "--from", "eslint", "eslint.config.mjs"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains("#   - config[0]: not statically readable: `neostandard({ ts: true })` (line 2: the result of a function call); what it configures is unknown\n"),
        "{r:?}"
    );
}

/// `js/no-explicit-any` is off by default: the migration turns it on where the ESLint
/// configuration has `@typescript-eslint/no-explicit-any` on, by name or through one of
/// typescript-eslint's `recommended`, `strict` or `all` configurations, and leaves it as it is
/// where the team turned it off or where what turns it off may not be read.
#[test]
fn migrate_turns_no_explicit_any_on_where_eslint_does() {
    let d = tmp();
    let root = d.path();
    let migrate = |name: &str| {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let src = std::path::Path::new(&fixture("any")).join(name);
        let file = std::fs::read_dir(&src)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .next()
            .unwrap();
        write(
            &dir,
            &file,
            &std::fs::read_to_string(src.join(&file)).unwrap(),
        );
        let r = mote(&dir, &["migrate", "--from", "eslint", &file]);
        assert_eq!(r.code, 0, "{r:?}");
        assert_valid_config(&dir, &r.stdout);
        (dir, file, r.stdout)
    };
    // By name.
    let (_, _, y) = migrate("explicit-on");
    assert!(
        y.contains("  js/no-explicit-any: on      # @typescript-eslint/no-explicit-any\n"),
        "{y}"
    );
    // Through a preset for every file, off again in one directory.
    let (dir, file, y) = migrate("via-preset");
    for want in [
        "rules:\n  js/no-console: on      # no-console: warn\n  js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/strict-type-checked\n",
        "  - paths: [\"src/generated/**/*.ts\"]  # config[5].files (line 14)\n    rules:\n      js/no-explicit-any: off     # @typescript-eslint/no-explicit-any\n",
        "#   - config[2]: preset `plugin:@typescript-eslint/strict-type-checked` (`...tseslint.configs.strictTypeChecked`, line 9) is not expanded beyond `@typescript-eslint/no-explicit-any`, which Mote leaves off by default; `extends: default` covers Mote's own selection\n",
        // Presets without the rule turn nothing on.
        "#   - config[3]: preset `plugin:@typescript-eslint/stylistic` (`...tseslint.configs.stylistic`, line 10) is not expanded; `extends: default` covers Mote's own selection\n",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    // `init --from` writes the same setting into its starter.
    let r = mote(&dir, &["init", "--from", "eslint", &file]);
    assert_eq!(r.code, 0, "{r:?}");
    let starter = read(&dir, ".mote.yaml");
    assert!(
        starter.contains("  js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/strict-type-checked\n"),
        "{starter}"
    );
    // Through a preset of the TypeScript files only (eslintrc), and not through one of
    // typescript-eslint's configurations without the rule.
    let (_, _, y) = migrate("via-preset-eslintrc");
    assert!(
        y.contains("  - paths: [\"**/*.ts\", \"**/*.tsx\"]  # overrides[0].files\n    rules:\n      js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/recommended\n"),
        "{y}"
    );
    assert_eq!(y.matches("js/no-explicit-any").count(), 1, "{y}");
    assert!(!y.contains("\"**/*.spec.ts\""), "{y}");
    // The preset with the rule turned off: off; where a spread of unknown rules follows the
    // preset, not carried over (the default, off, stays).
    let (_, _, y) = migrate("preset-off");
    for want in [
        "  js/no-explicit-any: off     # @typescript-eslint/no-explicit-any\n",
        "#   - @typescript-eslint/no-explicit-any (config[2], line 16): not carried over: preset `plugin:@typescript-eslint/recommended` turns it on, but `...testRules` (line 19) may change its setting\n",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert_eq!(y.matches("js/no-explicit-any").count(), 1, "{y}");
    assert!(!y.contains("overrides:"), "{y}");
}

/// ESLint reads a configuration's patterns from its own directory and applies it to the files
/// below it: the proposal for the repository root says so in its globs.
#[test]
fn migrate_a_configuration_of_a_sub_directory() {
    let d = tmp();
    let root = d.path();
    let copy = |name: &str, to: &str| {
        write(root, to, &std::fs::read_to_string(fixture(name)).unwrap());
    };
    copy(
        "flat-js/eslint.config.mjs",
        "packages/app/eslint.config.mjs",
    );
    copy("eslintrc-js/.eslintrc.cjs", "web/.eslintrc.cjs");
    let migrate = |args: &[&str]| {
        let mut all = vec!["migrate", "--from", "eslint"];
        all.extend_from_slice(args);
        let r = mote(root, &all);
        assert_eq!(r.code, 0, "{r:?}");
        r.stdout
    };
    let y = migrate(&["packages/app/eslint.config.mjs"]);
    for want in [
        // A flat configuration anchors its patterns to its directory.
        "exclude:\n  - \"packages/app/dist/**\"  # config[0].ignores (line 20): dist/\n",
        // Its settings for every file are for the files below it.
        "overrides:\n  - paths: [\"packages/app/**\"]  # the configuration's own directory: ESLint applies it below packages/app/\n    rules:\n      js/cyclomatic-complexity: { params: { threshold: 12 } } # complexity: warn\n      js/eqeqeq: on      # eqeqeq\n      js/no-explicit-any: on      # @typescript-eslint/no-explicit-any: on in plugin:@typescript-eslint/recommended\n      js/no-var: on      # no-var\n",
        "  - paths: [\"packages/app/**/*.js\", \"packages/app/**/*.mjs\", \"packages/app/**/*.ts\"]  # config[4].files (line 33)\n",
        "  - paths: [\"packages/app/test/**\"]  # config[5].files (line 37)\n",
        // The negation is read from the same directory.
        "#   - config[0].ignores (line 20): `**/*.min.js` not carried over: a later negation (`!keep.min.js`)",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert!(!y.contains("\nrules:\n"), "{y}");
    assert_valid_config(root, &y);
    // eslintrc: `.gitignore` semantics and file names at any depth, below its directory.
    let y = migrate(&["web/.eslintrc.cjs"]);
    for want in [
        "  - \"web/**/build/**\"  # ignorePatterns: build/\n",
        "  - paths: [\"web/**\"]  # the configuration's own directory: ESLint applies it below web/\n    rules:\n      js/no-var: on      # no-var\n",
        "  - paths: [\"web/**/*.ts\", \"web/**/*.tsx\"]  # overrides[0].files\n",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert_valid_config(root, &y);
    // Written next to the configuration, the proposal is for that directory.
    migrate(&[
        "packages/app/eslint.config.mjs",
        "-o",
        "packages/app/.mote.yaml",
    ]);
    let y = read(root, "packages/app/.mote.yaml");
    assert!(
        y.contains("exclude:\n  - \"dist/**\"  # config[0].ignores (line 20): dist/\n"),
        "{y}"
    );
    assert!(
        y.contains("rules:\n  js/cyclomatic-complexity: { params: { threshold: 12 } }"),
        "{y}"
    );
    // `init --from` proposes for its directory.
    let r = mote(
        root,
        &["init", "--from", "eslint", "packages/app/eslint.config.mjs"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let y = read(root, ".mote.yaml");
    assert!(
        y.contains("  - \"packages/app/dist/**\"  # config[0].ignores"),
        "{y}"
    );
    assert!(y.contains("  - paths: [\"packages/app/**\"]"), "{y}");
    // A file ESLint does not load by itself (a flat configuration saved as JSON) is read from the
    // proposal's root wherever it is saved.
    copy("flat-config.json", "tmp/flat-config.json");
    let y = migrate(&["tmp/flat-config.json"]);
    for want in [
        "  - \"build/**\"  # config[0].ignores: build/**\n",
        "rules:\n  js/no-debugger: on      # no-debugger\n",
        "  - paths: [\"scripts/**/*.js\"]  # config[2].files\n",
        "#   - tmp/flat-config.json is not a file ESLint loads by itself: its patterns are read from ., where the proposal applies, not from tmp/\n",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    // A flat configuration's patterns without `/` stay in its directory: `./` anchors them.
    write(
        root,
        "eslint.config.mjs",
        "export default [\n  { ignores: ['*.snap', 'dist'] },\n  { files: ['*.mjs'], rules: { 'no-console': 'off' } },\n];\n",
    );
    let y = migrate(&["eslint.config.mjs"]);
    for want in [
        "  - \"./*.snap\"  # config[0].ignores (line 2): *.snap\n",
        "  - \"dist/**\"  # config[0].ignores (line 2): dist\n",
        "  - paths: [\"./*.mjs\"]  # config[1].files (line 3)\n",
    ] {
        assert!(y.contains(want), "missing `{want}` in:\n{y}");
    }
    assert_valid_config(root, &y);
    // A configuration outside the proposal's directory is read as if it were there, and said so.
    let r = mote(
        &root.join("packages"),
        &["migrate", "--from", "eslint", "../web/.eslintrc.cjs"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains("#   - ../web/.eslintrc.cjs is not below ., where the proposal applies: its patterns are read as if it were there; check them\n"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("  - \"**/build/**\"  # ignorePatterns: build/\n"),
        "{r:?}"
    );
}

#[test]
fn java_compat_sources_are_selected_one_by_one() {
    let d = tmp();
    let root = d.path();
    let src = "\
import edu.umd.cs.findbugs.annotations.SuppressFBWarnings;
class A {
    void a() {
        try { run(); } catch (Exception e) { } // NOPMD - probing
    }
    // CHECKSTYLE:OFF
    void c() {
        try { run(); } catch (Exception e) { }
    }
    // CHECKSTYLE:ON
    @SuppressFBWarnings(value = \"DE_MIGHT_IGNORE\", justification = \"best effort\")
    void s() {
        try { run(); } catch (Exception e) { }
    }
    void i() {
        //noinspection EmptyCatchBlock
        try { run(); } catch (Exception e) { }
    }
    @SuppressWarnings(\"all\")
    void w() {
        try { run(); } catch (Exception e) { }
    }
    void run() {}
}
";
    write(root, "A.java", src);
    // Suppressed lines of `java/empty-catch-block`, with the justification.
    let suppressed = |compat: &str| {
        write(
            root,
            ".mote.yaml",
            &format!("suppressions: {{ compat: {compat} }}\n"),
        );
        let r = mote(
            root,
            &["scan", "--no-cache", "--no-gate", "--format", "json"],
        );
        assert_eq!(r.code, 0, "{r:?}");
        let v = json(&r.stdout);
        let lines: Vec<(u64, Option<String>)> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["rule_id"] == "java/empty-catch-block")
            .filter(|f| f["suppressed"]["kind"] == "compat")
            .map(|f| {
                (
                    f["range"]["start_line"].as_u64().unwrap(),
                    f["suppressed"]["justification"]
                        .as_str()
                        .map(str::to_string),
                )
            })
            .collect();
        lines
    };
    let some = |s: &str| Some(s.to_string());
    // `true` is exactly the set honored before sources could be chosen.
    assert_eq!(
        suppressed("true"),
        vec![(4, some("probing")), (8, None), (21, None)]
    );
    assert_eq!(suppressed("false"), vec![]);
    assert_eq!(
        suppressed("[spotbugs, intellij]"),
        vec![(13, some("best effort")), (17, None)]
    );
    assert_eq!(
        suppressed("[pmd, suppress-warnings-all]"),
        vec![(4, some("probing")), (21, None)]
    );
    assert_eq!(suppressed("checkstyle"), vec![(8, None)]);
    // An unknown source is a configuration error.
    write(root, ".mote.yaml", "suppressions: { compat: [spotbug] }\n");
    let r = mote(root, &["config", "validate"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("did you mean `spotbugs`?"), "{r:?}");
}

/// `js/function-too-long` measures each test and hook, not the suite's callback that groups
/// them; a long ordinary callback and a long test still count.
#[test]
fn js_function_length_skips_suite_callbacks() {
    let d = tmp();
    let root = d.path();
    let body = |n: usize| "    step();\n".repeat(n);
    write(
        root,
        "cart.test.ts",
        &format!(
            "import {{ describe, it, beforeEach }} from 'vitest';\n\
             describe('cart', () => {{\n  beforeEach(() => {{\n{}  }});\n  it('short', () => {{\n{}  }});\n  it('long', () => {{\n{}  }});\n  describe.each([[1]])('nested %i', (n) => {{\n    it('short too', () => {{\n{}    }});\n  }});\n}});\n",
            body(3),
            body(3),
            body(8),
            body(3)
        ),
    );
    write(
        root,
        "jobs.ts",
        &format!(
            "export function run(): void {{\n  [1].forEach(() => {{\n{}  }});\n}}\n",
            body(8)
        ),
    );
    write(
        root,
        ".mote.yaml",
        "rules:\n  js/function-too-long: { params: { max_lines: 6 } }\n",
    );
    let r = mote(
        root,
        &["scan", "--no-cache", "--no-gate", "--format", "json"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let v = json(&r.stdout);
    let mut long: Vec<(String, u64, String)> = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule_id"] == "js/function-too-long")
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_string(),
                f["range"]["start_line"].as_u64().unwrap(),
                f["message"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    long.sort();
    assert_eq!(
        long,
        vec![
            (
                "cart.test.ts".to_string(),
                13,
                "Split `<anonymous>` into smaller functions; it has 10 lines of code, more than 6"
                    .to_string()
            ),
            (
                "jobs.ts".to_string(),
                1,
                "Split `run` into smaller functions; it has 12 lines of code, more than 6"
                    .to_string()
            ),
        ],
        "the long test and the long function, not the 29-line suite or the 7-line nested one: {long:?}"
    );
}

#[test]
fn js_compat_sources_eslint_and_nosonar() {
    let d = tmp();
    let root = d.path();
    write(
        root,
        "app.js",
        "// eslint-disable-next-line no-console -- debug output\nconsole.log(1);\nconsole.log(2); // NOSONAR\nconsole.log(3);\n",
    );
    // Suppressed lines of `js/no-console`, with the kind.
    let suppressed = |compat: &str| {
        write(
            root,
            ".mote.yaml",
            &format!("suppressions: {{ compat: {compat} }}\n"),
        );
        let r = mote(
            root,
            &["scan", "--no-cache", "--no-gate", "--format", "json"],
        );
        assert_eq!(r.code, 0, "{r:?}");
        let v = json(&r.stdout);
        let lines: Vec<(u64, String)> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["rule_id"] == "js/no-console")
            .filter_map(|f| {
                let kind = f["suppressed"]["kind"].as_str()?;
                Some((f["range"]["start_line"].as_u64().unwrap(), kind.to_string()))
            })
            .collect();
        lines
    };
    let line = |n: u64, kind: &str| (n, kind.to_string());
    assert_eq!(
        suppressed("true"),
        vec![line(2, "compat"), line(3, "nosonar")]
    );
    assert_eq!(suppressed("[eslint]"), vec![line(2, "compat")]);
    assert_eq!(suppressed("[nosonar, pmd]"), vec![line(3, "nosonar")]);
    assert_eq!(suppressed("[pmd, spotbugs, intellij]"), vec![]);
    assert_eq!(suppressed("false"), vec![]);
}

#[test]
fn java_compat_catch_all_forms_stay_within_their_source() {
    let d = tmp();
    let root = d.path();
    // `empty-catch-block` has a SpotBugs and an IntelliJ equivalent, `useless-catch` neither.
    let body = "{\n        try { run(); } catch (Exception e) { }\n        try { run(); } catch (RuntimeException e) { throw e; }\n    }\n";
    let src = format!(
        "import edu.umd.cs.findbugs.annotations.SuppressFBWarnings;\nclass A {{\n    @SuppressFBWarnings(justification = \"legacy\")\n    void s() {body}    @SuppressWarnings(\"ALL\")\n    void i() {body}    @SuppressWarnings(\"all\")\n    void w() {body}    void run() {{}}\n}}\n"
    );
    write(root, "A.java", &src);
    // Suppressed (line, rule) pairs of the two rules.
    let suppressed = |compat: &str| {
        write(
            root,
            ".mote.yaml",
            &format!("suppressions: {{ compat: {compat} }}\n"),
        );
        let r = mote(
            root,
            &["scan", "--no-cache", "--no-gate", "--format", "json"],
        );
        assert_eq!(r.code, 0, "{r:?}");
        let v = json(&r.stdout);
        let got: Vec<(u64, String)> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| {
                f["rule_id"] == "java/empty-catch-block" || f["rule_id"] == "java/useless-catch"
            })
            .filter(|f| f["suppressed"]["kind"] == "compat")
            .map(|f| {
                (
                    f["range"]["start_line"].as_u64().unwrap(),
                    f["rule_id"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        got
    };
    let at = |line: u64, rule: &str| (line, format!("java/{rule}"));
    // A bare `@SuppressFBWarnings` and IntelliJ's `ALL` silence only their tool's equivalents.
    assert_eq!(suppressed("[spotbugs]"), vec![at(5, "empty-catch-block")]);
    assert_eq!(suppressed("[intellij]"), vec![at(10, "empty-catch-block")]);
    // `"all"` silences everything, but only with its own source: `ALL` is not `all`.
    assert_eq!(
        suppressed("[suppress-warnings-all]"),
        vec![at(15, "empty-catch-block"), at(16, "useless-catch")]
    );
}

#[test]
fn java_compat_suppressions_from_other_tools() {
    let d = tmp();
    let root = d.path();
    let src = "\
class A {
    void a() {
        try { run(); } catch (Exception e) { } // NOPMD - probing, failure is expected
    }
    @SuppressWarnings(\"PMD.EmptyCatchBlock\")
    void b() {
        try { run(); } catch (Exception e) { }
    }
    // CHECKSTYLE:OFF
    void c() {
        try { run(); } catch (Exception e) { }
    }
    // CHECKSTYLE:ON
    void d() {
        try { run(); } catch (Exception e) { }
    }
    void run() {}
}
";
    write(root, "A.java", src);
    let scan = |root: &std::path::Path| {
        let r = mote(
            root,
            &["scan", "--no-cache", "--no-gate", "--format", "json"],
        );
        assert_eq!(r.code, 0, "{r:?}");
        let v = json(&r.stdout);
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["rule_id"] == "java/empty-catch-block")
            .map(|f| {
                (
                    f["range"]["start_line"].as_u64().unwrap(),
                    f["suppressed"]["kind"].as_str().map(str::to_string),
                    f["suppressed"]["justification"]
                        .as_str()
                        .map(str::to_string),
                )
            })
            .collect::<Vec<_>>()
    };
    let off = scan(root);
    assert_eq!(off.len(), 4, "{off:?}");
    assert!(off.iter().all(|f| f.1.is_none()), "compat off: {off:?}");
    write(root, ".mote.yaml", "suppressions: { compat: true }\n");
    let on = scan(root);
    assert_eq!(
        on,
        vec![
            (
                3,
                Some("compat".to_string()),
                Some("probing, failure is expected".to_string())
            ),
            (7, Some("compat".to_string()), None),
            (11, Some("compat".to_string()), None),
            (15, None, None),
        ]
    );
}
