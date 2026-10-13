//! End-to-end tests of the `mote` binary with the `.tl` test plugin.

mod common;

use common::*;
use serde_json::Value;
use std::path::Path;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("invalid JSON ({e}): {text}"))
}

fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn version_and_help() {
    let d = tmp();
    let r = mote(d.path(), &["version"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r
        .stdout
        .starts_with(&format!("mote {}\n", env!("CARGO_PKG_VERSION"))));
    assert!(r.stdout.contains("tl Test language"), "{r:?}");
    let r = mote(d.path(), &["--version"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains(env!("CARGO_PKG_VERSION")));
    let r = mote(d.path(), &["scan", "--help"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("--github-checks"));
}

#[test]
fn usage_errors_exit_2() {
    let d = tmp();
    for args in [
        &["scan", "--bogus"][..],
        &["scan", "--changed-only"],
        &["scan", "no-such-dir"],
        &["scan", "--format", "xml"],
        &["scan", "--min-severity", "CRITICAL"],
        &["frobnicate"],
    ] {
        let r = mote(d.path(), args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
        assert!(r.stdout.is_empty(), "{args:?}: {r:?}");
    }
    let r = mote(d.path(), &["rules", "explain", "tl/high-bugg"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("did you mean `tl/high-bug`"), "{r:?}");
    let r = mote(d.path(), &["rules", "--language", "cobol"]);
    assert_eq!(r.code, 2, "{r:?}");
}

#[test]
fn config_errors_exit_2() {
    let d = tmp();
    write(d.path(), "a.tl", "HIGHBUG\n");
    write(d.path(), ".mote.yaml", "gatee: {}\n");
    let r = mote(d.path(), &["scan"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("gatee"), "{r:?}");
    let r = mote(d.path(), &["config", "validate"]);
    assert_eq!(r.code, 2, "{r:?}");

    write(
        d.path(),
        ".mote.yaml",
        "rules:\n  tl/high-bugg: off\n  tl/loud: { params: { nope: 1 } }\n",
    );
    let r = mote(d.path(), &["config", "validate"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("did you mean"), "{r:?}");
    assert!(r.stderr.contains(".mote.yaml"), "names the file: {r:?}");
    let r = mote(d.path(), &["scan"]);
    assert_eq!(r.code, 2, "{r:?}");
    let r = mote(d.path(), &["scan", "--config", "missing.yaml"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("missing.yaml"), "{r:?}");

    write(d.path(), ".mote.yaml", "rules:\n  tl/loud: HIGH\n");
    let r = mote(d.path(), &["config", "validate"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("valid"));
}

/// A configuration from an untrusted pull request cannot make a scan hang: a "billion laughs"
/// document is refused at once, with a configuration error.
#[test]
fn hostile_yaml_is_refused_quickly() {
    let d = tmp();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\n");
    let mut laughs = String::from("a0: &a0 [lol, lol, lol, lol, lol, lol, lol, lol, lol]\n");
    for i in 1..10 {
        let p = i - 1;
        laughs.push_str(&format!(
            "a{i}: &a{i} [*a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}]\n"
        ));
    }
    let start = std::time::Instant::now();
    write(root, ".mote.yaml", &laughs);
    let want = "error: invalid configuration in .mote.yaml:6:10: not valid YAML: repetition limit exceeded (aliases expand to more than 100000 values)\n";
    for args in [&["config", "validate"][..], &["scan"]] {
        let r = mote(root, args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
        assert_eq!(r.stderr, want, "{args:?}");
    }
    // Any YAML read: an ESLint configuration too.
    write(root, ".eslintrc.yaml", &laughs);
    let r = mote(root, &["migrate", "--from", "eslint", ".eslintrc.yaml"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(
        r.stderr
            .contains(".eslintrc.yaml: repetition limit exceeded"),
        "{r:?}"
    );
    assert!(start.elapsed() < std::time::Duration::from_secs(60));
}

#[test]
fn every_config_problem_at_its_position_the_same_for_every_command() {
    let d = tmp();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\n");
    let r = mote(root, &["scan", "--no-gate", "--json", "results.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    // Unknown keys and values (the parse) and unknown rules (validation), with other tools'
    // rule names explained, in one run.
    write(
        root,
        ".mote.yaml",
        "exclud: [vendor]\n\
         rules:\n  \
           no-unused-vars: off\n  \
           \"@typescript-eslint/no-explicit-any\": error\n  \
           category/java/errorprone.xml/EmptyCatchBlock: off\n  \
           tl/high-bugg: off\n\
         gate:\n  \
           fail_on: hihg\n",
    );
    let validate = mote(root, &["config", "validate"]);
    assert_eq!(validate.code, 2, "{validate:?}");
    assert!(validate.stdout.is_empty(), "{validate:?}");
    let eslint = "(`mote migrate --from eslint <config>` converts a whole ESLint configuration)";
    let want = format!(
        "error: invalid configuration in .mote.yaml (7 problems):\n  \
         .mote.yaml:1:1: unknown key `exclud`; did you mean `exclude`?\n  \
         .mote.yaml:3:3: rules: unknown rule `no-unused-vars`: that is an ESLint rule; the \
         Mote rule is `js/no-unused-vars` {eslint}\n  \
         .mote.yaml:4:3: rules: unknown rule `@typescript-eslint/no-explicit-any`: that is an \
         ESLint rule; the Mote rule is `js/no-explicit-any` {eslint}\n  \
         .mote.yaml:4:41: rules.@typescript-eslint/no-explicit-any: invalid value: string \
         \"error\", expected `off`, `on`, a severity (LOW, MEDIUM, HIGH), or a map with `enabled`, \
         `severity`, `params`; `error` is an ESLint level: write `on` (or a severity: LOW, MEDIUM, \
         HIGH)\n  \
         .mote.yaml:5:3: rules: unknown rule `category/java/errorprone.xml/EmptyCatchBlock`: \
         that is a PMD rule reference; the Mote rule is `java/empty-catch-block` (`mote \
         migrate --from pmd <ruleset.xml>` converts a whole PMD ruleset)\n  \
         .mote.yaml:6:3: rules: unknown rule `tl/high-bugg`; did you mean `tl/high-bug`?\n  \
         .mote.yaml:8:12: gate.fail_on: unknown variant `hihg`, expected one of `HIGH`, \
         `high`, `MEDIUM`, `medium`, `LOW`, `low`, `NONE`, `none`; did you mean `HIGH`?\n"
    );
    assert_eq!(validate.stderr, want);
    for args in [
        &["scan"][..],
        &["scan", "--format", "json"],
        &["gate", "--results", "results.json"],
        &["config", "explain", "a.tl"],
    ] {
        let r = mote(root, args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
        assert_eq!(r.stderr, want, "{args:?}");
    }

    // A file that parses: its validation problems, located the same way.
    write(
        root,
        ".mote.yaml",
        "rules:\n  SystemOutPrintln: off\n  tl/loud: { params: { threshhold: 3 } }\n",
    );
    let validate = mote(root, &["config", "validate"]);
    assert_eq!(validate.code, 2, "{validate:?}");
    assert!(
        validate.stderr.contains(
            ".mote.yaml:2:3: rules: unknown rule `SystemOutPrintln`: that looks like a PMD \
             rule name, and Mote has no equivalent"
        ),
        "{validate:?}"
    );
    assert!(
        validate
            .stderr
            .contains(".mote.yaml:3:24: rules.tl/loud: unknown parameter `threshhold`"),
        "{validate:?}"
    );
    let scan = mote(root, &["scan"]);
    assert_eq!(scan.code, 2, "{scan:?}");
    assert_eq!(scan.stderr, validate.stderr);

    // One problem: one line.
    write(root, ".mote.yaml", "cpd:\n  min_tokns: 50\n");
    let r = mote(root, &["config", "validate"]);
    assert_eq!(
        r.stderr,
        "error: invalid configuration in .mote.yaml:2:3: cpd: unknown key `min_tokns`; did \
         you mean `min_tokens`?\n"
    );
}

#[test]
fn init_writes_a_valid_starter_and_refuses_to_overwrite() {
    let d = tmp();
    let r = mote(d.path(), &["init"]);
    assert_eq!(r.code, 0, "{r:?}");
    // `/` in the path on every platform; then what to do next, the cache first.
    assert!(
        r.stdout.starts_with(
            "wrote ./.mote.yaml; check it with `mote config validate`\n\
             add `.mote/` to .gitignore: scans keep their analysis cache there\n\
             next: `mote scan --no-gate` for a first look"
        ),
        "{r:?}"
    );
    assert!(!r.stdout.contains("migrate"), "no other tool found: {r:?}");
    let text = read(d.path(), ".mote.yaml");
    assert!(text.starts_with("# Mote configuration"));
    let r = mote(d.path(), &["config", "validate"]);
    assert_eq!(r.code, 0, "{r:?}");
    let r = mote(d.path(), &["init"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("--force"));
    write(d.path(), ".mote.yaml", "# mine\n");
    write(d.path(), ".gitignore", "target/\n/.mote/\n");
    let r = mote(d.path(), &["init", "--force"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(!r.stdout.contains(".gitignore"), "already ignored: {r:?}");
    assert_eq!(read(d.path(), ".mote.yaml"), text);
}

#[test]
fn init_lists_third_party_code_commented_out() {
    let d = tmp();
    let java = |package: &str, imports: &str, class: &str| {
        format!("package {package};\n\n{imports}\npublic class {class} {{}}\n")
    };
    for i in 0..10 {
        write(
            d.path(),
            &format!("src/main/java/com/acme/shop/S{i}.java"),
            &java("com.acme.shop", "", &format!("S{i}")),
        );
    }
    write(
        d.path(),
        "src/main/java/org/apache/wink/client/MockHttpServer.java",
        &java(
            "org.apache.wink.client",
            "import java.net.ServerSocket;\n",
            "MockHttpServer",
        ),
    );
    let r = mote(d.path(), &["init", "--suggest-excludes"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout
            .contains("  src/main/java/org/apache/wink/**  # 1 Java file in org.apache.wink.*"),
        "{r:?}"
    );
    assert!(
        !d.path().join(".mote.yaml").exists(),
        "the listing writes nothing"
    );

    let r = mote(d.path(), &["init"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains("1 possible third-party location"),
        "{r:?}"
    );
    let text = read(d.path(), ".mote.yaml");
    assert!(
        text.contains("  # - \"src/main/java/org/apache/wink/**\"  # 1 Java file"),
        "{text}"
    );
    // Still analyzed: the suggestion is a comment.
    let r = mote(d.path(), &["config", "validate"]);
    assert_eq!(r.code, 0, "{r:?}");
    // The listing also works next to an existing configuration.
    let r = mote(d.path(), &["init", "--suggest-excludes"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("org/apache/wink/**"), "{r:?}");
    // What that configuration already excludes is not proposed again, by any glob covering it.
    for exclude in [
        "**/wink/**",
        "src/main/java/org/apache/**",
        "MockHttpServer.java",
    ] {
        write(
            d.path(),
            ".mote.yaml",
            &format!("exclude: [\"{exclude}\"]\n"),
        );
        let r = mote(d.path(), &["init", "--suggest-excludes"]);
        assert_eq!(r.code, 0, "{r:?}");
        assert!(
            r.stdout.contains("No third-party code found to exclude."),
            "{exclude}: {r:?}"
        );
    }
    // An exclude elsewhere changes nothing.
    write(d.path(), ".mote.yaml", "exclude: [\"docs/**\"]\n");
    let r = mote(d.path(), &["init", "--suggest-excludes"]);
    assert!(r.stdout.contains("org/apache/wink/**"), "{r:?}");
    // A configuration that does not parse is an error, as for the other commands.
    write(d.path(), ".mote.yaml", "exclud: []\n");
    let r = mote(d.path(), &["init", "--suggest-excludes"]);
    assert_eq!(r.code, 2, "{r:?}");
}

#[test]
fn init_proposes_compat_sources_from_the_suppressions_found() {
    let d = tmp();
    let root = d.path();
    let r = mote(root, &["init", "--suggest-compat"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.contains(
            "No suppressions of other tools found: keep `suppressions: { compat: false }`."
        ),
        "{r:?}"
    );
    write(
        root,
        "src/main/java/app/A.java",
        r#"package app;

import edu.umd.cs.findbugs.annotations.SuppressFBWarnings;

class A {
    int a = 1; // NOPMD - wire format
    @SuppressFBWarnings("DM_GC")
    void gc() { System.gc(); }
    @SuppressWarnings("unused") private int b;
    @SuppressWarnings({"unused", "rawtypes"}) private java.util.List c;
    void d() {
        //noinspection ConstantConditions
        run();
        String s = "// NOSONAR in a string is not a comment";
    }
}
"#,
    );
    write(
        root,
        "web/src/app.ts",
        "// eslint-disable-next-line no-console\nconsole.log(1);\n",
    );
    // Not counted: files a scan skips (vendored, generated, the starter's excluded directories).
    write(
        root,
        "web/public/jquery.cookie.js",
        "/*!\n * jQuery Cookie Plugin v1.4.1\n * Copyright 2013 Klaus Hartl\n * Released under the MIT license\n */\nf(); // eslint-disable-line\n",
    );
    write(
        root,
        "src/main/java/app/Gen.java",
        "// Code generated by protoc. DO NOT EDIT.\npackage app;\nclass Gen { int x; } // NOPMD\n",
    );
    write(root, "web/node_modules/lib/index.js", "f(); // NOSONAR\n");

    let r = mote(root, &["init", "--suggest-compat"]);
    assert_eq!(r.code, 0, "{r:?}");
    for line in [
        "Suppressions of other tools found:\n",
        "  eslint                      1 in 1 file\n",
        "  intellij                    1 in 1 file\n",
        "  pmd                         1 in 1 file\n",
        "  spotbugs                    1 in 1 file\n",
        "  note: Not proposed: intellij, with 1 //noinspection comment and 0 @SuppressWarnings",
        "  note: 2 @SuppressWarnings with javac/Eclipse keys (rawtypes 1, unused 2)",
        "Proposed (in .mote.yaml): suppressions: { compat: [eslint, pmd, spotbugs] }\n",
    ] {
        assert!(r.stdout.contains(line), "{line}: {r:?}");
    }
    assert!(!r.stdout.contains("nosonar"), "{r:?}");
    assert!(
        !root.join(".mote.yaml").exists(),
        "the listing writes nothing"
    );

    // The starter carries the proposal, with the counts in a comment.
    let r = mote(root, &["init"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout
            .contains("suppressions.compat: [eslint, pmd, spotbugs], from the suppressions"),
        "{r:?}"
    );
    // The linters whose markers were found get a pointer to `init --from`.
    for tool in ["pmd", "eslint"] {
        assert!(
            r.stdout.contains(&format!(
                "{tool} markers found: `mote init --force --from {tool}` writes this starter again"
            )),
            "{tool}: {r:?}"
        );
    }
    let text = read(root, ".mote.yaml");
    assert!(
        text.contains("  # Other tools' suppressions found in this repository (occurrences): eslint 1, intellij 1, pmd 1,\n  #   spotbugs 1.\n"),
        "{text}"
    );
    assert!(
        text.contains("  # Other sources: checkstyle, intellij, nosonar, suppress-warnings-all"),
        "{text}"
    );
    assert!(
        text.contains("\n  compat: [eslint, pmd, spotbugs]\n"),
        "{text}"
    );
    let r = mote(root, &["config", "validate"]);
    assert_eq!(r.code, 0, "{r:?}");

    // Next to an existing configuration, its excludes and skip settings decide what is counted.
    write(
        root,
        ".mote.yaml",
        "exclude: [\"src/main/java/**\"]\nskip_generated_except: [\"web/public/**\"]\n",
    );
    let r = mote(root, &["init", "--suggest-compat", "--suggest-excludes"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout.starts_with("No third-party code found to exclude.\n\nSuppressions of other tools found:\n  eslint                      2 in 2 files\n"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("Proposed (in .mote.yaml): suppressions: { compat: [eslint] }"),
        "{r:?}"
    );
}

fn migrate_fixture(name: &str) -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/migrate")
            .join(name),
    )
    .unwrap()
}

fn yaml(text: &str) -> serde_json::Value {
    mote_engine::yaml::from_str(text).unwrap_or_else(|e| panic!("invalid YAML ({e}): {text}"))
}

/// `init --from` writes one starter holding init's proposals and the migration, with the same
/// rules, overrides and excludes `migrate` proposes.
#[test]
fn init_from_merges_the_migration_into_the_starter() {
    let d = tmp();
    let root = d.path();
    write(
        root,
        "src/main/java/app/A.java",
        "package app;\nclass A {}\n",
    );
    // The ruleset a Maven build names (no NOPMD in the code: `pmd` comes from the migration).
    write(
        root,
        "pom.xml",
        "<project><build><plugins><plugin><artifactId>maven-pmd-plugin</artifactId>\n<configuration><rulesets>\n  <ruleset>${basedir}/src/conf/team-pmd.xml</ruleset>\n  <ruleset>/category/java/bestpractices.xml</ruleset>\n</rulesets></configuration></plugin></plugins></build></project>\n",
    );
    write(
        root,
        "src/conf/team-pmd.xml",
        &migrate_fixture("pmd-ruleset.xml"),
    );
    let r = mote(root, &["init", "--from", "pmd"]);
    assert_eq!(r.code, 0, "{r:?}");
    for want in [
        "migrated from pmd src/conf/team-pmd.xml: mapped 12 of 15 rules; 6 items not mapped",
        "Not mapped:\n",
        "  - properties of CyclomaticComplexity not carried over: classReportLevel=90\n",
    ] {
        assert!(r.stdout.contains(want), "missing `{want}`: {r:?}");
    }
    let text = read(root, ".mote.yaml");
    for want in [
        "# Starter written by `mote init --from pmd src/conf/team-pmd.xml`: besides init's own",
        // The rules replace the commented examples; the explanation above them stays.
        "# Keys are rule ids or globs such as \"js/*\".\nrules:\n  java/cognitive-complexity: { params: { threshold: 14 } } # CognitiveComplexity, PMD's default reportLevel=15 is threshold 14 (reported above it)\n  java/cyclomatic-complexity: { params: { threshold: 11 } } # CyclomaticComplexity, methodReportLevel=12 is threshold 11 (reported above it)\n",
        "  java/unused-private-field: HIGH    # UnusedPrivateField, priority 1\n",
        // The ruleset's exclude is the starter's own: listed once.
        "  - \"**/generated/**\"\n  - \"**/.yarn/**\"\n\n",
        "  # `pmd` is selected for the migrated configuration (`mote init --from`): the suppressions written\n",
        "\n  compat: [pmd]\n",
        "\n# Migrated from pmd src/conf/team-pmd.xml:\n# Not mapped (6):\n",
        "#   - custom rule NoSystemExit",
        // Settings the migration does not touch keep their commented examples.
        "# overrides:\n#   - paths: [\"legacy/**\"]\n",
    ] {
        assert!(text.contains(want), "missing `{want}` in:\n{text}");
    }
    assert!(!text.contains("#   java/empty-catch-block: off"), "{text}");
    assert!(!text.contains("**/generated/**\"  #"), "{text}");
    let r = mote(root, &["config", "validate"]);
    assert_eq!(r.code, 0, "{r:?}");
    // The same settings as `migrate` proposes.
    let r = mote(root, &["migrate", "--from", "pmd", "src/conf/team-pmd.xml"]);
    assert_eq!(r.code, 0, "{r:?}");
    let (init, migrated) = (yaml(&text), yaml(&r.stdout));
    assert_eq!(init["rules"], migrated["rules"]);
    let excludes = init["exclude"].as_array().unwrap();
    for g in migrated["exclude"].as_array().unwrap() {
        assert!(excludes.contains(g), "{g:?}");
    }

    // ESLint: the configuration ESLint would use (flat before eslintrc), with its overrides.
    let d = tmp();
    let root = d.path();
    write(
        root,
        ".eslintrc.json",
        "{ \"rules\": { \"no-var\": \"off\" } }",
    );
    write(
        root,
        "eslint.config.mjs",
        &migrate_fixture("flat-js/eslint.config.mjs"),
    );
    write(
        root,
        "web/a.js",
        "// eslint-disable-next-line no-console\nconsole.log(1);\n",
    );
    let r = mote(root, &["init", "--from", "eslint"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout
            .contains("migrated from eslint eslint.config.mjs: mapped 7 of 8 rules"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("suppressions.compat: [eslint], from the suppressions"),
        "{r:?}"
    );
    assert!(!r.stdout.contains("eslint markers found"), "{r:?}");
    let text = read(root, ".mote.yaml");
    for want in [
        "  - \"**/.yarn/**\"\n  # From ESLint's ignores, which only concerned JavaScript and TypeScript files:\n",
        "  - \"dist/**\"  # config[0].ignores (line 20): dist/\n",
        "# Path-specific settings, applied in order.\noverrides:\n  - paths: [\"**/*.js\", \"**/*.mjs\", \"**/*.ts\"]  # config[4].files (line 33)\n",
        "\n  compat: [eslint]\n",
    ] {
        assert!(text.contains(want), "missing `{want}` in:\n{text}");
    }
    assert!(
        !text.contains("selected for the migrated configuration"),
        "eslint was counted: {text}"
    );
    let r = mote(root, &["config", "validate"]);
    assert_eq!(r.code, 0, "{r:?}");
    let r = mote(root, &["migrate", "--from", "eslint", "eslint.config.mjs"]);
    let (init, migrated) = (yaml(&text), yaml(&r.stdout));
    assert_eq!(init["rules"], migrated["rules"]);
    assert_eq!(init["overrides"], migrated["overrides"]);
    // A file named on the command line, for a directory named before `--from`.
    let r = mote(
        root,
        &["init", ".", "--force", "--from", "eslint", ".eslintrc.json"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        read(root, ".mote.yaml").contains("  js/no-var: off     # no-var\n"),
        "{r:?}"
    );
}

#[test]
fn init_from_refuses_what_it_cannot_read() {
    let d = tmp();
    let root = d.path();
    let expect_usage_error = |args: &[&str], want: &str| {
        let r = mote(root, args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
        assert!(r.stderr.contains(want), "{args:?}: {r:?}");
        assert!(!root.join(".mote.yaml").exists(), "{args:?}");
    };
    expect_usage_error(
        &["init", "--from", "eslint"],
        "no ESLint configuration found",
    );
    expect_usage_error(&["init", "--from", "pmd"], "no PMD ruleset found");
    expect_usage_error(
        &["init", "--from", "checkstyle"],
        "the tool is `pmd` or `eslint`",
    );
    write(root, "sub/keep.txt", "");
    expect_usage_error(
        &["init", "--from", "pmd", "sub"],
        "`mote init sub --from pmd`",
    );
    expect_usage_error(
        &["init", "--from", "pmd", "nope.xml"],
        "nope.xml: no such file",
    );
    expect_usage_error(
        &["init", "--from", "pmd", "--suggest-compat"],
        "cannot be used with",
    );
    let ruleset = "<ruleset name=\"x\"><rule ref=\"category/java/errorprone.xml/EmptyCatchBlock\"/></ruleset>\n";
    write(root, "pmd.xml", ruleset);
    write(root, "config/pmd/ruleset.xml", ruleset);
    expect_usage_error(
        &["init", "--from", "pmd"],
        "several PMD rulesets found in .: pmd.xml, config/pmd/ruleset.xml",
    );
    // A JavaScript configuration that cannot be parsed.
    write(root, "eslint.config.js", "export default [\n");
    expect_usage_error(&["init", "--from", "eslint"], "cannot parse the file");
}

#[test]
fn skip_generated_except_analyzes_matching_files() {
    let d = tmp();
    let root = d.path();
    let banner = "/*!\n * jQuery Cookie Plugin v1.4.1\n * Copyright 2013 Klaus Hartl\n * Released under the MIT license\n */\n";
    write(
        root,
        "web/js/jquery.cookie.js",
        &format!("{banner}debugger;\n"),
    );
    write(root, "web/js/other.js", &format!("{banner}debugger;\n"));
    write(
        root,
        "web/app.js",
        "/*! app v1.0.0 | (c) Acme | MIT */\ndebugger;\n",
    );
    let skips = |yaml: &str| -> Vec<(String, Value)> {
        write(root, ".mote.yaml", yaml);
        let r = mote(
            root,
            &["scan", "--no-cache", "--no-gate", "--format", "json"],
        );
        assert_eq!(r.code, 0, "{r:?}");
        json(&r.stdout)["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["path"].as_str().unwrap().to_string(),
                    f["skip_reason"].clone(),
                )
            })
            .collect()
    };
    let skipped = |path: &str, reason: &str| (path.to_string(), Value::from(reason));
    let analyzed = |path: &str| (path.to_string(), Value::Null);
    assert_eq!(
        skips(""),
        vec![
            skipped("web/app.js", "vendored"),
            skipped("web/js/jquery.cookie.js", "vendored"),
            skipped("web/js/other.js", "vendored"),
        ]
    );
    // Per-path exceptions; `exclude` still wins.
    assert_eq!(
        skips("skip_generated_except: [\"web/js\"]\nexclude: [\"other.js\"]\n"),
        vec![
            skipped("web/app.js", "vendored"),
            analyzed("web/js/jquery.cookie.js"),
            skipped("web/js/other.js", "excluded"),
        ]
    );
    // A banner naming the project's own package is not a vendored signal.
    write(
        root,
        "package.json",
        "{\"name\": \"app\", \"private\": true}\n",
    );
    assert_eq!(
        skips(""),
        vec![
            analyzed("web/app.js"),
            skipped("web/js/jquery.cookie.js", "vendored"),
            skipped("web/js/other.js", "vendored"),
        ]
    );
    // An invalid glob is a configuration error.
    write(root, ".mote.yaml", "skip_generated_except: [\"web/[\"]\n");
    let r = mote(root, &["config", "validate"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("skip_generated_except"), "{r:?}");
}

#[test]
fn rules_catalog_and_schema() {
    let d = tmp();
    let r = mote(d.path(), &["rules"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("tl/high-bug"));
    assert!(
        !r.stdout.contains("tl/cyclomatic-complexity"),
        "off by default"
    );
    let r = mote(d.path(), &["rules", "--all", "--format", "json"]);
    let v = json(&r.stdout);
    let ids: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"tl/cyclomatic-complexity"));
    assert!(ids.contains(&"mote/internal-error"));
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    // Every rule carries the measured precision (additive): the embedded data covers the
    // product's rules, the test plugin's are newer than it.
    for r in v.as_array().unwrap() {
        let state = r["precision"]["state"].as_str().unwrap_or_default();
        if r["id"].as_str().unwrap().starts_with("tl/") {
            assert_eq!(state, "not-measured-yet", "{r}");
        } else {
            assert!(!state.is_empty(), "{r}");
        }
        assert!(r["precision"]["measured"].is_string(), "{r}");
    }
    let r = mote(
        d.path(),
        &["rules", "--language", "tl", "--all", "--format", "markdown"],
    );
    assert!(r.stdout.starts_with("# Mote rules"), "{r:?}");
    assert!(!r.stdout.contains("mote/internal-error"));
    let r = mote(d.path(), &["rules", "explain", "tl/high-bug"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("Flags the `HIGHBUG` marker."), "{r:?}");
    assert!(
        r.stdout.contains(
            "Measured precision: not measured yet: the rule is newer than the last measurement"
        ),
        "{r:?}"
    );
    let r = mote(d.path(), &["rules", "explain", "mote/internal-error"]);
    assert!(
        r.stdout.contains(
            "
Measured precision: "
        ),
        "{r:?}"
    );
    let r = mote(
        d.path(),
        &[
            "rules",
            "explain",
            "tl/cognitive-complexity",
            "--format",
            "json",
        ],
    );
    let explained = json(&r.stdout);
    assert!(explained["params"]
        .as_array()
        .is_some_and(|p| !p.is_empty()));
    assert!(explained["precision"]["state"].is_string(), "{explained}");

    let r = mote(d.path(), &["config", "schema"]);
    assert_eq!(r.code, 0);
    let schema = json(&r.stdout);
    assert!(schema["properties"]["gate"].is_object(), "{schema}");

    write(d.path(), "src/a.tl", "x\n");
    write(
        d.path(),
        ".mote.yaml",
        "rules:\n  tl/loud: { severity: HIGH }\n",
    );
    let r = mote(d.path(), &["config", "explain", "src/a.tl"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("tl/loud"));
    let r = mote(
        d.path(),
        &["config", "explain", "src/a.tl", "--format", "json"],
    );
    let v = json(&r.stdout);
    assert_eq!(v["rules"]["tl/loud"], "HIGH");
    assert_eq!(v["language"], "tl");
    let r = mote(d.path(), &["config", "explain", "README.md"]);
    assert_eq!(r.code, 2, "{r:?}");
}

#[test]
fn scan_plain_directory_exit_codes_and_formats() {
    let d = tmp();
    let root = d.path();
    write(root, "src/a.tl", "fn run\n  LOUD\n  HIGHBUG\nend\n");
    write(root, "src/b.tl", "MAGIC // TODO: later\n");
    // Without a base every finding counts: the HIGH one fails the default gate.
    let r = mote(root, &["scan", "--color", "never", "--no-cache"]);
    assert_eq!(r.code, 1, "{r:?}");
    assert!(r.stdout.contains("src/a.tl:3:3"), "{r:?}");
    assert!(r.stdout.contains("tl/high-bug"));
    assert!(r.stdout.contains("Quality gate  FAILED"), "{r:?}");
    assert!(!r.stdout.contains('\x1b'));
    let r = mote(root, &["scan", "--no-gate", "--color", "always"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains('\x1b'));

    let r = mote(
        root,
        &["scan", "--format", "jsonl", "--min-severity", "medium"],
    );
    assert_eq!(r.code, 1);
    let lines: Vec<Value> = r.stdout.lines().map(json).collect();
    assert_eq!(lines.len(), 2, "{r:?}");
    assert!(lines
        .iter()
        .all(|l| l["status"] == "unknown" && l["id"].is_string()));

    let r = mote(root, &["scan", "--format", "sarif", "-o", "out/x.sarif"]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.is_empty(), "{r:?}");
    let sarif = json(&read(root, "out/x.sarif"));
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(sarif["runs"][0]["tool"]["driver"]["name"], "Mote");

    let r = mote(root, &["scan", "--format", "json"]);
    let v = json(&r.stdout);
    assert_eq!(v["gate"]["passed"], false);
    assert_eq!(v["source"]["kind"], "directory");

    // Only a MEDIUM finding: the gate passes.
    write(root, "src/a.tl", "LOUD\n");
    let r = mote(root, &["scan", "--include", "src/a.tl"]);
    assert_eq!(r.code, 0, "{r:?}");
    let r = mote(root, &["scan", "--timings", "-q"]);
    assert!(r.stderr.contains("Timings (wall clock)"), "{r:?}");
    assert!(r.stderr.contains("analysis (map)"));
}

#[test]
fn strict_and_internal_errors() {
    let d = tmp();
    write(d.path(), "a.tl", "PANIC_NOW\n");
    write(d.path(), "b.tl", "LOUD\n");
    let r = mote(d.path(), &["scan", "--no-cache"]);
    assert_eq!(r.code, 0, "internal errors never gate: {r:?}");
    assert!(r.stdout.contains("mote/internal-error"), "{r:?}");
    let r = mote(d.path(), &["scan", "--no-cache", "--strict"]);
    assert_eq!(r.code, 3, "{r:?}");
    // An output file that cannot be written is an error of the run (exit 3).
    write(d.path(), "blocker", "file, not a directory");
    let r = mote(d.path(), &["scan", "--json", "blocker/results.json"]);
    assert_eq!(r.code, 3, "{r:?}");
}

/// Base repository: `a.tl` has an old HIGH finding; the feature branch adds a new HIGH in `c.tl`
/// and a new MEDIUM in `b.tl`.
fn repo_with_changes(root: &Path, new_high: bool) {
    init_repo(root);
    write(root, "a.tl", "fn old\n  HIGHBUG\nend\n");
    write(root, "b.tl", "fn b\n  x\nend\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(root, "b.tl", "fn b\n  x\n  LOUD\nend\n");
    write(
        root,
        "c.tl",
        if new_high {
            "fn c\n  HIGHBUG\nend\n"
        } else {
            "fn c\n  y\nend\n"
        },
    );
    commit_all(root, "feature");
}

#[test]
fn scan_with_base_writes_every_output() {
    let d = tmp();
    let root = d.path();
    repo_with_changes(root, true);
    let r = mote(
        root,
        &[
            "scan",
            "--base",
            "main",
            "--json",
            "out/results.json",
            "--jsonl",
            "out/findings.jsonl",
            "--sarif",
            "out/results.sarif",
            "--sarif-min-severity",
            "MEDIUM",
            "--markdown",
            "out/summary.md",
            "--github-checks",
            "out/checks.json",
            "--color",
            "never",
        ],
    );
    assert_eq!(r.code, 1, "new HIGH fails the gate: {r:?}");
    assert!(r.stdout.contains("[new]"), "{r:?}");

    let results = json(&read(root, "out/results.json"));
    let status_of = |path: &str| -> Vec<String> {
        results["findings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|f| f["path"] == path)
            .map(|f| f["status"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(status_of("a.tl"), ["unchanged"]);
    assert_eq!(status_of("c.tl"), ["new"]);
    assert_eq!(status_of("b.tl"), ["new"]);
    assert_eq!(results["new_code"]["base"], "main");
    assert_eq!(results["gate"]["conditions"][0]["name"], "new_findings");
    // What the GitHub Action's scan.sh computes with jq for its `new-findings` output.
    let new_unsuppressed = results["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["suppressed"].is_null() && f["status"] == "new")
        .count();
    assert_eq!(new_unsuppressed, 2);

    let md = read(root, "out/summary.md");
    assert!(md.starts_with("<!-- mote:summary -->\n"), "{md}");
    assert!(md.contains("quality gate failed"));

    let checks = json(&read(root, "out/checks.json"));
    assert_eq!(checks["name"], "Mote");
    assert_eq!(checks["conclusion"], "failure");
    let ann = checks["annotations"].as_array().unwrap();
    let paths: Vec<&str> = ann.iter().map(|a| a["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["c.tl", "b.tl"], "new only, HIGH first: {checks}");
    assert_eq!(ann[0]["annotation_level"], "failure");
    assert_eq!(ann[1]["annotation_level"], "warning");
    assert_eq!(ann[0]["start_column"], 3);

    let sarif = json(&read(root, "out/results.sarif"));
    let res = sarif["runs"][0]["results"].as_array().unwrap();
    assert_eq!(res.len(), 3, "min severity MEDIUM");
    assert!(res.iter().any(|r| r["baselineState"] == "unchanged"));
    assert!(res.iter().any(|r| r["baselineState"] == "new"));

    let jsonl = read(root, "out/findings.jsonl");
    assert_eq!(jsonl.lines().count(), 3);

    // --new-only hides the unchanged finding; --changed-only doesn't analyze a.tl at all.
    let r = mote(
        root,
        &["scan", "--base", "main", "--new-only", "--format", "jsonl"],
    );
    assert_eq!(r.stdout.lines().count(), 2, "{r:?}");
    let r = mote(
        root,
        &[
            "scan",
            "--base",
            "main",
            "--changed-only",
            "--format",
            "json",
        ],
    );
    let v = json(&r.stdout);
    assert!(v["files"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["path"] != "a.tl"));

    // Scanning the base revision itself: only the old finding, no base comparison.
    let r = mote(
        root,
        &["scan", "--rev", "main", "--format", "jsonl", "--no-gate"],
    );
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout.lines().count(), 1, "{r:?}");
    assert!(r.stdout.contains("\"path\":\"a.tl\""));

    let r = mote(root, &["scan", "--base", "no-such-branch"]);
    assert_eq!(r.code, 2, "{r:?}");
}

/// GitLab Code Quality, Checkstyle and JUnit: full reports for the CI services' own base
/// comparison (GitLab, Checkstyle), the listing filters for JUnit.
#[test]
fn ci_formats_for_gitlab_jenkins_and_azure() {
    let d = tmp();
    let root = d.path();
    repo_with_changes(root, true);
    let r = mote(
        root,
        &[
            "scan",
            "--base",
            "main",
            "--format",
            "gitlab",
            "--checkstyle",
            "out/checkstyle.xml",
            "--junit",
            "out/junit.xml",
            "--html",
            "out/report.html",
        ],
    );
    assert_eq!(r.code, 1, "the gate still decides the exit code: {r:?}");
    let html = read(root, "out/report.html");
    assert!(html.starts_with("<!DOCTYPE html>"), "{html}");
    assert!(html.contains("http-equiv=\"Content-Security-Policy\""));
    assert!(
        html.contains("\"total_findings\":3"),
        "counts in the embedded data"
    );
    let cq = json(&r.stdout);
    let issues = cq.as_array().unwrap();
    assert_eq!(issues.len(), 3, "old findings included: {cq}");
    let sev: Vec<(&str, &str)> = issues
        .iter()
        .map(|i| {
            (
                i["location"]["path"].as_str().unwrap(),
                i["severity"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        sev,
        [
            ("a.tl", "critical"),
            ("b.tl", "major"),
            ("c.tl", "critical")
        ]
    );
    let fps: std::collections::BTreeSet<&str> = issues
        .iter()
        .map(|i| i["fingerprint"].as_str().unwrap())
        .collect();
    assert_eq!(fps.len(), 3, "unique fingerprints: {cq}");
    let cs = read(root, "out/checkstyle.xml");
    assert!(cs.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<checkstyle"));
    assert_eq!(cs.matches("<error ").count(), 3, "{cs}");
    assert!(cs.contains("<file name=\"c.tl\">"));
    let ju = read(root, "out/junit.xml");
    assert!(ju.contains("<testsuite name=\"Mote quality gate\""), "{ju}");
    assert!(ju.contains("<testsuite name=\"tl/high-bug\" tests=\"2\" failures=\"2\""));
    assert!(ju.contains("<failure message=\"New findings: 1 (&lt;= 0)\" type=\"gate\">"));

    // --new-only narrows the JUnit listing (not the GitLab / Checkstyle reports).
    let r = mote(
        root,
        &[
            "scan",
            "--base",
            "main",
            "--new-only",
            "--no-gate",
            "--format",
            "junit",
            "--gitlab-codequality",
            "out/cq.json",
            "--checkstyle",
            "out/checkstyle.xml",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(
        r.stdout
            .contains("<testsuite name=\"tl/high-bug\" tests=\"1\""),
        "{r:?}"
    );
    assert!(!r.stdout.contains("name=\"a.tl:"), "{r:?}");
    assert_eq!(
        json(&read(root, "out/cq.json")).as_array().unwrap().len(),
        3
    );
    assert_eq!(
        read(root, "out/checkstyle.xml").matches("<error ").count(),
        3
    );
}

/// Grades are computed on every scan; gate conditions on them are opt-in, re-evaluated by
/// `mote gate` with the bands of its configuration, and selectable with `--only grades`.
#[test]
fn grades_are_informational_unless_the_gate_asks() {
    let d = tmp();
    let root = d.path();
    // 5 minutes of maintainability work in 3 lines of code: grade E; one HIGH reliability bug.
    write(root, "a.tl", "fn run\n  LOUD\nend\n");
    let r = mote(root, &["scan", "--no-cache", "--json", "r.json"]);
    assert_eq!(r.code, 0, "grades don't gate by default: {r:?}");
    assert!(
        r.stdout.contains("Grades        maintainability E"),
        "{r:?}"
    );
    let v = json(&read(root, "r.json"));
    assert_eq!(v["format_version"], 1);
    let g = &v["grades"];
    assert_eq!(g["grades_version"], 1);
    assert_eq!(g["maintainability"]["grade"], "E");
    assert_eq!(g["reliability"]["grade"], "A");
    assert_eq!(
        g["bands"]["maintainability"],
        serde_json::json!([1.0, 2.0, 3.0, 4.0])
    );
    assert_eq!(
        g["bands"]["reliability"],
        serde_json::json!([0.5, 1.0, 2.0, 4.0])
    );
    assert_eq!(g["bands"]["min_new_lines"], 100);
    assert!(g["new_maintainability"].is_null(), "no base");
    assert_eq!(
        v["measures"]["total"]["effort_minutes"]["maintainability"],
        5
    );

    write(
        root,
        ".mote.yaml",
        "gate: { maintainability_grade: C, reliability_grade: a, new_reliability_grade: B }\n",
    );
    let r = mote(root, &["scan", "--no-cache", "--color", "never"]);
    assert_eq!(r.code, 1, "{r:?}");
    assert!(
        r.stdout
            .contains("FAIL  Maintainability grade          E (C or better)"),
        "{r:?}"
    );
    assert!(r.stdout.contains("pass  Reliability grade"), "{r:?}");
    assert!(
        r.stdout
            .contains("skip  Reliability grade on new code  no base to compare with"),
        "{r:?}"
    );
    let r = mote(root, &["gate", "--results", "r.json", "--only", "grades"]);
    assert_eq!(r.code, 1, "{r:?}");
    // Wider bands in the gate job's configuration: the saved measures are graded again.
    write(
        root,
        ".mote.yaml",
        "gate: { maintainability_grade: C }\ngrades: { maintainability: [100, 200, 300, 400] }\n",
    );
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            "r.json",
            "--only",
            "grades",
            "--format",
            "json",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let v = json(&r.stdout);
    assert_eq!(v["grades"]["maintainability"]["grade"], "A");
    assert_eq!(v["gate"]["conditions"][0]["name"], "maintainability_grade");
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            "r.json",
            "--only",
            "grades",
            "--format",
            "markdown",
        ],
    );
    assert!(
        r.stdout
            .contains("| Maintainability grade | :white_check_mark: passed | A | C or better |"),
        "{r:?}"
    );
}

/// A change of a few lines gets no new-code grades (a density over a handful of lines means
/// nothing): "not rated" in every output, the new-code grade conditions skipped for the same
/// reason, and the gate's own verdict on new HIGH findings unchanged.
#[test]
fn new_code_grades_need_min_new_lines() {
    let d = tmp();
    let root = d.path();
    init_repo(root);
    write(root, "a.tl", "fn run\nend\n");
    commit_all(root, "base");
    write(root, "a.tl", "fn run\n  HIGHBUG\nend\n");
    write(root, ".mote.yaml", "gate: { new_reliability_grade: B }\n");
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "--base",
            "HEAD",
            "--color",
            "never",
            "--json",
            "r.json",
            "--markdown",
            "s.md",
            "--summary-json",
            "sum.json",
        ],
    );
    assert_eq!(r.code, 1, "the new HIGH finding fails the gate: {r:?}");
    assert!(
        r.stdout
            .contains("on new code: not rated (only 1 new line; grades need at least 100)\n"),
        "{r:?}"
    );
    assert!(
        r.stdout
            .contains("skip  Reliability grade on new code  only 1 new line (minimum 100)"),
        "{r:?}"
    );
    assert!(r.stdout.contains("FAIL  New findings"), "{r:?}");
    let v = json(&read(root, "r.json"));
    let g = &v["grades"];
    assert!(g["new_maintainability"].is_null() && g["new_reliability"].is_null());
    assert_eq!(
        g["new_not_rated"],
        serde_json::json!({"reason": "too_few_new_lines", "new_lines": 1, "min_new_lines": 100})
    );
    assert_eq!(v["format_version"], 1, "an additive field");
    assert!(
        read(root, "s.md").contains("| Reliability grade | _not rated_ |"),
        "{}",
        read(root, "s.md")
    );
    let sum = json(&read(root, "sum.json"));
    assert_eq!(sum["grades"]["new_not_rated"]["new_lines"], 1);

    // `mote gate` grades the saved result with its own configuration's minimum: the grades'
    // one (`grades.min_new_lines`), not the gate's (`gate.min_new_lines`, for the duplication and
    // coverage conditions).
    let gate_grades = [
        "gate",
        "--results",
        "r.json",
        "--only",
        "grades",
        "--format",
        "json",
    ];
    write(
        root,
        ".mote.yaml",
        "gate: { new_reliability_grade: B, min_new_lines: 1 }\n",
    );
    let r = mote(root, &gate_grades);
    assert_eq!(r.code, 0, "still not rated: {r:?}");
    let v = json(&r.stdout);
    assert_eq!(v["grades"]["new_not_rated"]["min_new_lines"], 100, "{v}");
    write(
        root,
        ".mote.yaml",
        "gate: { new_reliability_grade: B }\ngrades: { min_new_lines: 1 }\n",
    );
    let r = mote(root, &gate_grades);
    assert_eq!(r.code, 1, "{r:?}");
    let v = json(&r.stdout);
    assert_eq!(v["grades"]["new_reliability"]["grade"], "E");
    assert!(v["grades"].get("new_not_rated").is_none(), "{v}");
}

/// Every key of a real results file is described by `config schema --results` (following `$ref`
/// and `anyOf`), so consumers can generate types from it.
#[test]
fn results_schema_describes_the_results_json() {
    fn resolve<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
        match schema.get("$ref").and_then(Value::as_str) {
            Some(r) => {
                let name = r.rsplit('/').next().unwrap();
                resolve(&root["$defs"][name], root)
            }
            None => schema,
        }
    }
    /// The object schemas `schema` allows (itself, or its `anyOf` / `oneOf` alternatives).
    fn object_schemas<'a>(schema: &'a Value, root: &'a Value) -> Vec<&'a Value> {
        let s = resolve(schema, root);
        let mut out = Vec::new();
        for key in ["anyOf", "oneOf", "allOf"] {
            for alt in s.get(key).and_then(Value::as_array).into_iter().flatten() {
                out.extend(object_schemas(alt, root));
            }
        }
        if s.get("properties").is_some() || s.get("additionalProperties").is_some() {
            out.push(s);
        }
        out
    }
    fn check(value: &Value, schema: &Value, root: &Value, at: &str) {
        match value {
            Value::Object(map) => {
                let alts = object_schemas(schema, root);
                assert!(!alts.is_empty(), "{at}: no object schema");
                for (k, v) in map {
                    let sub = alts.iter().find_map(|s| {
                        s.get("properties")
                            .and_then(|p| p.get(k))
                            .or_else(|| s.get("additionalProperties").filter(|a| a.is_object()))
                    });
                    let sub = sub.unwrap_or_else(|| panic!("{at}.{k}: not in the schema"));
                    check(v, sub, root, &format!("{at}.{k}"));
                }
            }
            Value::Array(items) => {
                let s = resolve(schema, root);
                let item = s.get("items").or_else(|| {
                    s.get("anyOf")
                        .and_then(Value::as_array)
                        .and_then(|a| a.iter().find_map(|x| resolve(x, root).get("items")))
                });
                if let (Some(item), Some(first)) = (item, items.first()) {
                    if item.is_object() {
                        check(first, item, root, &format!("{at}[0]"));
                    }
                }
            }
            _ => {}
        }
    }

    let d = tmp();
    let root = d.path();
    repo_with_changes(root, true);
    let r = mote(
        root,
        &["scan", "--base", "main", "--no-gate", "--json", "r.json"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let results = json(&read(root, "r.json"));
    let r = mote(root, &["config", "schema", "--results"]);
    assert_eq!(r.code, 0, "{r:?}");
    let schema = json(&r.stdout);
    assert!(schema["$schema"].as_str().unwrap().contains("2020-12"));
    assert!(schema["properties"]["format_version"].is_object());
    for def in ["Finding", "Measures", "Grades", "GateResult", "RuleInfo"] {
        assert!(schema["$defs"][def].is_object(), "{def}");
    }
    check(&results, &schema, &schema, "$");
    // The configuration schema is unchanged by the flag's existence.
    let r = mote(root, &["config", "schema"]);
    assert!(json(&r.stdout)["properties"]["grades"].is_object());
}

#[test]
fn old_high_findings_do_not_fail_the_new_code_gate() {
    let d = tmp();
    let root = d.path();
    repo_with_changes(root, false);
    let r = mote(root, &["scan", "--base", "main"]);
    assert_eq!(r.code, 0, "{r:?}");
    let r = mote(root, &["scan"]);
    assert_eq!(r.code, 1, "without a base the old HIGH counts: {r:?}");
}

#[test]
fn an_unknown_remote_base_gets_a_hint() {
    let d = tmp();
    let root = d.path();
    init_repo(root);
    write(root, "a.tl", "x\n");
    commit_all(root, "init");
    // The README's `--base origin/main` in a clone without a remote: the local branch exists.
    let r = mote(root, &["scan", "--base", "origin/main"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(
        r.stderr.contains(
            "unknown revision `origin/main`; did you mean the local branch `main`? A remote \
             branch must be fetched first"
        ),
        "{r:?}"
    );
    // No local branch of that name: only the fetch hint.
    let r = mote(root, &["scan", "--base", "origin/develop"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(
        r.stderr
            .contains("unknown revision `origin/develop`. A remote branch must be fetched first"),
        "{r:?}"
    );
    assert!(r.stderr.contains("`git fetch origin develop`"), "{r:?}");
    // A plain typo: the message as before.
    let r = mote(root, &["scan", "--base", "mian"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("unknown revision `mian`"), "{r:?}");
    assert!(!r.stderr.contains("fetch"), "{r:?}");
}

#[test]
fn scanning_a_subdirectory_says_the_top_level_config_is_not_read() {
    let d = tmp();
    let root = d.path();
    init_repo(root);
    write(root, ".mote.yaml", "rules:\n  tl/loud: off\n");
    write(root, "lib/core/a.tl", "LOUD\n");
    commit_all(root, "init");
    // From the top level, with --include: the configuration applies, no note.
    let r = mote(root, &["scan", "--include", "lib/core/**"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(!r.stdout.contains("tl/loud"), "{r:?}");
    assert!(!r.stderr.contains("note:"), "{r:?}");
    // The subdirectory as the scan root: its own (missing) configuration, said once.
    let r = mote(root, &["scan", "lib/core", "--no-gate"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("tl/loud"), "defaults apply: {r:?}");
    assert!(
        r.stderr
            .contains("note: lib/core has no .mote.yaml, so this scan uses the default settings"),
        "{r:?}"
    );
    assert!(
        r.stderr.contains("`--include 'lib/core/**'`")
            && r.stderr.contains("`--config .mote.yaml`"),
        "{r:?}"
    );
    // From inside the project, the path to the top-level file is relative to it.
    let r = mote(&root.join("lib/core"), &["scan", "--no-gate"]);
    assert!(r.stderr.contains("`--config ../../.mote.yaml`"), "{r:?}");
    // A configuration of its own: nothing to say.
    write(root, "lib/core/.mote.yaml", "extends: default\n");
    let r = mote(root, &["scan", "lib/core", "--no-gate"]);
    assert!(!r.stderr.contains("note:"), "{r:?}");
}

#[test]
fn staged_scan_reads_the_index() {
    let d = tmp();
    let root = d.path();
    init_repo(root);
    write(root, "a.tl", "x\n");
    commit_all(root, "init");
    write(root, "a.tl", "HIGHBUG\n");
    git(root, &["add", "a.tl"]);
    write(root, "a.tl", "x\n"); // the working tree is clean again
    let r = mote(root, &["scan", "--staged", "--base", "HEAD", "--new-only"]);
    assert_eq!(r.code, 1, "{r:?}");
    assert!(r.stdout.contains("tl/high-bug"));
    let r = mote(root, &["scan", "--base", "HEAD"]);
    assert_eq!(r.code, 0, "{r:?}");
}

#[test]
fn gate_reevaluates_with_coverage() {
    let d = tmp();
    let root = d.path();
    repo_with_changes(root, false);
    write(
        root,
        ".mote.yaml",
        "gate:\n  new_coverage_pct: 80\n  min_new_lines: 1\n  new_duplication_pct: null\n",
    );
    git(root, &["add", ".mote.yaml"]);
    git(root, &["commit", "-q", "-m", "cfg"]);
    let r = mote(root, &["scan", "--base", "main", "--json", "results.json"]);
    assert_eq!(
        r.code, 0,
        "coverage condition skipped without reports: {r:?}"
    );

    // c.tl lines 1-3 and b.tl line 3 are new; cover none of them.
    write(
        root,
        "cov/lcov.info",
        "SF:c.tl\nDA:1,0\nDA:2,0\nDA:3,0\nend_of_record\nSF:b.tl\nDA:3,0\nend_of_record\n",
    );
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            "results.json",
            "--coverage",
            "cov/*.info",
        ],
    );
    assert_eq!(r.code, 1, "{r:?}");
    assert!(r.stdout.contains("Coverage on new code: 0.0%"), "{r:?}");
    assert!(r.stdout.contains("FAILED"));

    write(
        root,
        "cov/lcov.info",
        "SF:c.tl\nDA:1,1\nDA:2,1\nDA:3,1\nend_of_record\nSF:b.tl\nDA:3,1\nend_of_record\n",
    );
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            "results.json",
            "--coverage",
            "cov/lcov.info",
            "--format",
            "markdown",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.starts_with("<!-- mote:summary -->"));
    assert!(r.stdout.contains("| Coverage | 100.0% |"), "{r:?}");
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            "results.json",
            "--coverage",
            "cov/lcov.info",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        json(&r.stdout)["new_code"]["measures"]["new_coverage_pct"],
        100.0
    );

    // Garbage results file: usage error.
    write(root, "bad.json", "{\"version\": 1}");
    let r = mote(root, &["gate", "--results", "bad.json"]);
    assert_eq!(r.code, 2, "{r:?}");
}

#[test]
fn remote_scan_clones_into_the_cache() {
    let d = tmp();
    let origin = d.path().join("origin");
    std::fs::create_dir_all(&origin).unwrap();
    init_repo(&origin);
    write(&origin, "src/a.tl", "HIGHBUG\n");
    commit_all(&origin, "init");
    let p = origin.display().to_string().replace('\\', "/");
    let url = if p.starts_with('/') {
        format!("file://{p}")
    } else {
        format!("file:///{p}")
    };
    let cache = d.path().join("cache");
    let cache_s = cache.display().to_string();
    let work = d.path().join("elsewhere");
    std::fs::create_dir_all(&work).unwrap();
    let r = mote(
        &work,
        &[
            "scan",
            "--remote",
            &url,
            "--cache-dir",
            &cache_s,
            "--format",
            "jsonl",
        ],
    );
    assert_eq!(r.code, 1, "{r:?}");
    assert!(r.stdout.contains("\"path\":\"src/a.tl\""), "{r:?}");
    assert!(cache.join("repos").is_dir());
    let r = mote(
        &work,
        &[
            "scan",
            "--remote",
            &url,
            "--rev",
            "main",
            "--cache-dir",
            &cache_s,
            "--no-gate",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let r = mote(
        &work,
        &[
            "scan",
            "--remote",
            "file:///definitely/not/here",
            "--cache-dir",
            &cache_s,
        ],
    );
    assert_eq!(r.code, 3, "{r:?}");
}

/// Lines that the lexer turns into ~20 tokens each (enough for a duplicated block).
fn dup_lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("  total = compute(value{i}, \"text\", {i}) + other.call(x, y);\n"))
        .collect()
}

#[test]
fn outputs_are_identical_across_thread_counts_and_cache_states() {
    let d = tmp();
    let root = d.path();
    let caches = tmp();
    for i in 0..40 {
        let body = if i % 3 == 0 {
            dup_lines(14)
        } else {
            format!("  x{i} = {i};\n")
        };
        write(
            root,
            &format!("m{}/f{i}.tl", i % 5),
            &format!("fn f{i} cog={}\n  LOUD\n{body}end\nHIGHBUG\n", 10 + i),
        );
    }
    let out = caches.path().join("out");
    let scan = |threads: &str, cache: &str| -> (String, String, String) {
        let cache = caches.path().join(cache);
        let json_path = out.join("r.json");
        let sarif_path = out.join("r.sarif");
        let ci = [
            ("--gitlab-codequality", out.join("cq.json")),
            ("--checkstyle", out.join("cs.xml")),
            ("--junit", out.join("junit.xml")),
            ("--html", out.join("r.html")),
        ];
        let r = mote(
            root,
            &[
                "scan",
                "--no-gate",
                "--threads",
                threads,
                "--cache-dir",
                cache.to_str().unwrap(),
                "--json",
                json_path.to_str().unwrap(),
                "--sarif",
                sarif_path.to_str().unwrap(),
                ci[0].0,
                ci[0].1.to_str().unwrap(),
                ci[1].0,
                ci[1].1.to_str().unwrap(),
                ci[2].0,
                ci[2].1.to_str().unwrap(),
                ci[3].0,
                ci[3].1.to_str().unwrap(),
                "--format",
                "jsonl",
            ],
        );
        assert_eq!(r.code, 0, "{r:?}");
        // `stats` (timings, cache counters) and analyzed/cached vary; nothing else may.
        let mut v = json(&std::fs::read_to_string(&json_path).unwrap());
        let o = v.as_object_mut().unwrap();
        o.remove("stats");
        for f in o["files"].as_array_mut().unwrap() {
            if f["status"] == "cached" {
                f["status"] = "analyzed".into();
            }
        }
        (
            serde_json::to_string_pretty(&v).unwrap(),
            std::fs::read_to_string(&sarif_path).unwrap()
                + &ci
                    .iter()
                    .map(|(_, p)| std::fs::read_to_string(p).unwrap())
                    .collect::<String>(),
            r.stdout,
        )
    };
    let one = scan("1", "c1");
    assert!(
        one.0.contains("tl/duplicated-block"),
        "the fixture has duplicates"
    );
    assert_eq!(one, scan("8", "c8"), "1 vs 8 threads, cold caches");
    assert_eq!(one, scan("8", "c1"), "warm cache");
    assert_eq!(one, scan("1", "c8"), "warm cache, 1 thread");
}

#[test]
fn gate_validates_the_config_like_scan() {
    let d = tmp();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\n");
    let r = mote(root, &["scan", "--no-gate", "--json", "results.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    write(root, ".mote.yaml", "rules:\n  tl/high-bugg: off\n");
    let r = mote(root, &["gate", "--results", "results.json"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stderr.contains("did you mean `tl/high-bug`"), "{r:?}");
    assert!(r.stderr.contains(".mote.yaml"), "names the file: {r:?}");
    write(root, ".mote.yaml", "gate: { fail_on: none }\n");
    let r = mote(root, &["gate", "--results", "results.json"]);
    assert_eq!(r.code, 0, "{r:?}");
    write(root, ".mote.yaml", "gate: { fail_on: HIGH }\n");
    let r = mote(root, &["gate", "--results", "results.json"]);
    assert_eq!(r.code, 1, "{r:?}");
}

/// `gate --only coverage` (the Action's `mode: gate` job after the tests) judges coverage alone,
/// without failing again on the findings the scan job already gated.
#[test]
fn gate_only_evaluates_the_selected_conditions() {
    let d = tmp();
    let root = d.path();
    repo_with_changes(root, true); // a new HIGH finding
    write(
        root,
        ".mote.yaml",
        "gate:\n  new_coverage_pct: 80\n  min_new_lines: 1\n  new_duplication_pct: null\n",
    );
    git(root, &["add", ".mote.yaml"]);
    git(root, &["commit", "-q", "-m", "cfg"]);
    let r = mote(
        root,
        &[
            "scan",
            "--base",
            "main",
            "--no-gate",
            "--json",
            "results.json",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let covered =
        "SF:c.tl\nDA:1,1\nDA:2,1\nDA:3,1\nend_of_record\nSF:b.tl\nDA:3,1\nend_of_record\n";
    write(root, "cov/full.info", covered);
    write(root, "cov/none.info", &covered.replace(",1\n", ",0\n"));
    let gate = |extra: &[&str]| {
        let mut args = vec!["gate", "--results", "results.json", "--format", "json"];
        args.extend_from_slice(extra);
        mote(root, &args)
    };
    let names = |r: &Run| -> Vec<String> {
        json(&r.stdout)["gate"]["conditions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    };

    let r = gate(&["--coverage", "cov/full.info"]);
    assert_eq!(r.code, 1, "the whole gate still fails on the HIGH: {r:?}");
    let r = gate(&["--coverage", "cov/full.info", "--only", "coverage"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert_eq!(names(&r), ["new_coverage_pct"]);
    assert_eq!(json(&r.stdout)["gate"]["passed"], true);
    let r = gate(&["--coverage", "cov/none.info", "--only", "coverage"]);
    assert_eq!(r.code, 1, "{r:?}");
    let r = gate(&["--coverage", "cov/full.info", "--only", "severity"]);
    assert_eq!(r.code, 1, "{r:?}");
    assert_eq!(names(&r), ["new_findings"]);
    // Repeatable and comma-separated.
    for extra in [
        &["--only", "coverage,severity"][..],
        &["--only", "coverage", "--only", "severity"][..],
    ] {
        let mut a = vec!["--coverage", "cov/full.info"];
        a.extend_from_slice(extra);
        let r = gate(&a);
        assert_eq!(r.code, 1, "{extra:?}: {r:?}");
        assert_eq!(names(&r), ["new_findings", "new_coverage_pct"], "{extra:?}");
    }
    // Not configured: a warning, and nothing left to fail.
    let r = gate(&["--only", "worsened"]);
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stderr.contains("--only worsened"), "{r:?}");
    let r = gate(&["--only", "bogus"]);
    assert_eq!(r.code, 2, "{r:?}");
    // The Markdown verdict lists only the selected condition.
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            "results.json",
            "--coverage",
            "cov/full.info",
            "--only",
            "coverage",
            "--format",
            "markdown",
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("| Coverage on new code |"), "{r:?}");
    assert!(!r.stdout.contains("| New findings | :x:"), "{r:?}");
}

/// The GitHub Action's `path: backend` runs `mote scan backend`: annotations, SARIF and the
/// Markdown summary must name files relative to the repository root (`backend/src/a.tl`), while
/// the local formats stay relative to the scan root.
#[test]
fn subdirectory_scan_reports_repository_relative_paths_to_github() {
    let d = tmp();
    let root = d.path();
    init_repo(root);
    write(root, "backend/src/a.tl", "fn a\n  HIGHBUG\n  LOUD\nend\n");
    write(root, "frontend/b.tl", "HIGHBUG\n");
    commit_all(root, "init");
    let out = tmp();
    let o = |name: &str| out.path().join(name).display().to_string();
    let backend = root.join("backend");
    let cases: [(&Path, &[&str]); 3] = [
        (root, &["scan", "backend"]),
        (&backend, &["scan"]),
        (root, &["scan", "backend", "--rev", "HEAD"]),
    ];
    for (dir, head) in cases {
        let mut args: Vec<String> = head.iter().map(|s| s.to_string()).collect();
        for (flag, file) in [
            ("--sarif", "r.sarif"),
            ("--github-checks", "checks.json"),
            ("--markdown", "summary.md"),
            ("--jsonl", "findings.jsonl"),
            ("--gitlab-codequality", "cq.json"),
            ("--checkstyle", "checkstyle.xml"),
            ("--junit", "junit.xml"),
        ] {
            args.push(flag.into());
            args.push(o(file));
        }
        args.extend(["--no-gate", "--no-cache", "--format", "json"].map(String::from));
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let r = mote(dir, &args);
        assert_eq!(r.code, 0, "{head:?}: {r:?}");

        // Local formats: relative to the scan root.
        let v = json(&r.stdout);
        let paths: Vec<&str> = v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap())
            .collect();
        assert_eq!(paths, ["src/a.tl", "src/a.tl"], "{head:?}");
        let jsonl = std::fs::read_to_string(o("findings.jsonl")).unwrap();
        assert!(
            jsonl.lines().all(|l| json(l)["path"] == "src/a.tl"),
            "{head:?}: {jsonl}"
        );

        // GitHub formats: relative to the repository root.
        let checks = json(&std::fs::read_to_string(o("checks.json")).unwrap());
        let ann: Vec<&str> = checks["annotations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["path"].as_str().unwrap())
            .collect();
        assert_eq!(ann, ["backend/src/a.tl", "backend/src/a.tl"], "{head:?}");
        assert!(
            checks["summary"]
                .as_str()
                .unwrap()
                .contains("`backend/src/a.tl:2`"),
            "{head:?}: {checks}"
        );
        let md = std::fs::read_to_string(o("summary.md")).unwrap();
        assert!(md.contains("`backend/src/a.tl:2`"), "{head:?}: {md}");

        // GitLab, Checkstyle and JUnit: relative to the repository root too.
        let cq = json(&std::fs::read_to_string(o("cq.json")).unwrap());
        let cq_paths: Vec<&str> = cq
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["location"]["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            cq_paths,
            ["backend/src/a.tl", "backend/src/a.tl"],
            "{head:?}"
        );
        let cs = std::fs::read_to_string(o("checkstyle.xml")).unwrap();
        assert!(
            cs.contains("<file name=\"backend/src/a.tl\">"),
            "{head:?}: {cs}"
        );
        let ju = std::fs::read_to_string(o("junit.xml")).unwrap();
        assert!(ju.contains("file=\"backend/src/a.tl\""), "{head:?}: {ju}");

        let sarif = json(&std::fs::read_to_string(o("r.sarif")).unwrap());
        let run = &sarif["runs"][0];
        let uris: Vec<&str> = run["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                    .as_str()
                    .unwrap()
            })
            .collect();
        assert_eq!(uris, ["backend/src/a.tl", "backend/src/a.tl"], "{head:?}");
        let srcroot = run["originalUriBaseIds"]["%SRCROOT%"]["uri"]
            .as_str()
            .unwrap_or_else(|| panic!("{head:?}: no %SRCROOT% in {run}"));
        let top = std::fs::canonicalize(root).unwrap();
        let top_name = top.file_name().unwrap().to_str().unwrap();
        assert!(
            srcroot.starts_with("file:///") && srcroot.ends_with(&format!("/{top_name}/")),
            "{head:?}: {srcroot}"
        );
        assert!(
            !srcroot.contains('\\') && !srcroot.contains("%3A"),
            "{srcroot}"
        );
    }

    // `gate --format markdown` on saved results uses the prefix the scan recorded.
    let r = mote(
        root,
        &[
            "scan",
            "backend",
            "--no-gate",
            "--no-cache",
            "--json",
            &o("results.json"),
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let saved = json(&std::fs::read_to_string(o("results.json")).unwrap());
    assert_eq!(saved["source"]["prefix"], "backend/", "{}", saved["source"]);
    let r = mote(
        root,
        &[
            "gate",
            "--results",
            &o("results.json"),
            "--format",
            "markdown",
        ],
    );
    assert!(r.stdout.contains("`backend/src/a.tl:2`"), "{r:?}");

    // At the top level nothing changes.
    let r = mote(
        root,
        &[
            "scan",
            "--no-gate",
            "--no-cache",
            "--github-checks",
            &o("top.json"),
        ],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let checks = json(&std::fs::read_to_string(o("top.json")).unwrap());
    assert!(checks["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["path"] == "frontend/b.tl"));
}

/// `--show-suppressed` is for the text output only: JSONL (and the other listings) never carry
/// suppressed findings.
#[test]
fn show_suppressed_only_affects_text_output() {
    let d = tmp();
    let root = d.path();
    write(
        root,
        "a.tl",
        "LOUD // mote-ignore[loud]: intended\nHIGHBUG\n",
    );
    let r = mote(
        root,
        &["scan", "--no-gate", "--show-suppressed", "--color", "never"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    assert!(r.stdout.contains("tl/loud"), "text shows it: {r:?}");
    for extra in [&[][..], &["--show-suppressed"][..]] {
        let mut args = vec!["scan", "--no-gate", "--format", "jsonl"];
        args.extend_from_slice(extra);
        let r = mote(root, &args);
        assert_eq!(r.code, 0, "{r:?}");
        let rules: Vec<String> = r
            .stdout
            .lines()
            .map(|l| json(l)["rule"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(rules, ["tl/high-bug"], "{extra:?}: {r:?}");
        let mut args = vec!["scan", "--no-gate", "--format", "markdown"];
        args.extend_from_slice(extra);
        let r = mote(root, &args);
        assert!(!r.stdout.contains("tl/loud"), "{extra:?}: {r:?}");
    }
}

/// A file that could not be analyzed makes the SARIF invocation unsuccessful.
#[test]
fn sarif_marks_incomplete_scans_unsuccessful() {
    let d = tmp();
    write(d.path(), "a.tl", "LOUD\n");
    let out = d.path().join("out");
    let sarif = |name: &str| {
        let p = out.join(name);
        let r = mote(
            d.path(),
            &["scan", "--no-cache", "--sarif", p.to_str().unwrap()],
        );
        assert_eq!(r.code, 0, "{r:?}");
        json(&std::fs::read_to_string(p).unwrap())
    };
    let v = sarif("ok.sarif");
    assert_eq!(v["runs"][0]["invocations"][0]["executionSuccessful"], true);
    write(d.path(), "b.tl", "PANIC_NOW\n");
    let v = sarif("failed.sarif");
    assert_eq!(
        v["runs"][0]["invocations"][0]["executionSuccessful"], false,
        "{v}"
    );
}

/// Plugin analysis never runs on the process main thread (1 MB of stack on Windows) or the CLI's
/// command thread — the test plugin panics there, which would surface as an internal error. This
/// covers `scan` (default pool, `--threads`, base-blob analysis, staged) and the MCP `scan` and
/// `verify` tools over stdio.
#[test]
fn analysis_runs_on_the_engine_pool_only() {
    let d = tmp();
    let root = d.path();
    repo_with_changes(root, true);
    for args in [
        &["scan", "--no-cache", "--no-gate", "--format", "jsonl"][..],
        &[
            "scan",
            "--no-cache",
            "--no-gate",
            "--format",
            "jsonl",
            "--threads",
            "1",
        ],
        &[
            "scan",
            "--no-cache",
            "--no-gate",
            "--format",
            "jsonl",
            "--base",
            "main",
        ],
        &[
            "scan",
            "--no-cache",
            "--no-gate",
            "--format",
            "jsonl",
            "--staged",
        ],
        &[
            "scan",
            "--no-cache",
            "--no-gate",
            "--format",
            "jsonl",
            "--rev",
            "main",
        ],
    ] {
        let r = mote(root, args);
        assert_eq!(r.code, 0, "{args:?}: {r:?}");
        assert!(!r.stdout.is_empty(), "{args:?}: {r:?}");
        assert!(!r.stdout.contains("mote/internal-error"), "{args:?}: {r:?}");
    }

    use std::io::Write as _;
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(BIN);
    isolate(&mut cmd)
        .args(["mcp", "--root"])
        .arg(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for m in [
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}}),
            serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "scan", "arguments": {}}}),
            serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "verify", "arguments": {"scan_id": "s1", "paths": ["b.tl", "c.tl"]}}}),
        ] {
            writeln!(stdin, "{m}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let replies: Vec<Value> = stdout.lines().map(json).collect();
    assert_eq!(replies.len(), 3, "{stdout}");
    let scan = &replies[1]["result"]["structuredContent"];
    assert_eq!(scan["findings"]["total"], 3, "{scan}");
    assert!(
        scan["top_rules"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["rule"] != "mote/internal-error"),
        "{scan}"
    );
    let verify = &replies[2]["result"]["structuredContent"];
    assert_eq!(verify["counts"]["still_present"], 2, "{verify}");
    assert_eq!(verify["counts"]["introduced"], 0, "{verify}");
}

/// `--include` patterns that match no file are a usage error (exit 2), not an empty, passing scan.
#[test]
fn include_matching_nothing_is_a_usage_error() {
    let d = tmp();
    let root = d.path();
    write(root, "src/a.tl", "HIGHBUG\n");
    let r = mote(root, &["scan", "--include", "nope/**"]);
    assert_eq!(r.code, 2, "{r:?}");
    assert!(r.stdout.is_empty(), "{r:?}");
    assert!(r.stderr.contains("nope/**"), "names the pattern: {r:?}");
    let r = mote(root, &["scan", "--include", "src/a.tl", "--no-gate"]);
    assert_eq!(r.code, 0, "{r:?}");
}
