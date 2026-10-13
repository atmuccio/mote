//! `mote fix` and the fix data of the other outputs, end to end with the real plugins.

mod common;

use common::*;
use serde_json::Value;

const A_JAVA: &str = "package a;\n\nimport java.util.List;\nimport java.util.Map;\n\nclass A {\n    Map<String, String> m;\n    long x = 10l;\n}\n";
const A_FIXED: &str =
    "package a;\n\nimport java.util.Map;\n\nclass A {\n    Map<String, String> m;\n    long x = 10L;\n}\n";
/// A byte-order mark, CRLF line ends, and an import the code keeps on purpose.
const B_JAVA: &str = "\u{feff}package a;\r\n\r\nimport java.util.Set;\r\nimport java.util.List; // mote-ignore[java/unused-import]: kept for the docs\r\n\r\nclass B {\r\n}\r\n";
const B_FIXED: &str = "\u{feff}package a;\r\n\r\nimport java.util.List; // mote-ignore[java/unused-import]: kept for the docs\r\n\r\nclass B {\r\n}\r\n";
const C_JS: &str = "import { used, unused } from './m';\nused();\n";

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "src/A.java", A_JAVA);
    write(root, "src/B.java", B_JAVA);
    write(root, "web/c.js", C_JS);
    dir
}

#[test]
fn fix_prints_a_diff_then_writes_and_a_second_run_changes_nothing() {
    let dir = project();
    let root = dir.path();
    let r = mote(root, &["fix", "--no-cache", "--color", "never"]);
    assert_eq!(r.code, 1, "a dry run with fixes to apply exits 1: {r:?}");
    assert!(
        r.stdout.starts_with(
            "--- a/src/A.java\n+++ b/src/A.java\n@@ -1,9 +1,8 @@\n package a;\n \n-import java.util.List;\n"
        ),
        "{r:?}"
    );
    assert!(r
        .stdout
        .contains("-    long x = 10l;\n+    long x = 10L;\n"));
    assert!(r.stdout.contains("--- a/web/c.js\n+++ b/web/c.js\n"));
    assert!(
        r.stderr.contains("Would fix 4 findings in 3 files:\n"),
        "{r:?}"
    );
    assert!(r.stderr.contains("  java/unused-import"));
    assert!(r.stderr.contains("Run `mote fix --write` to apply them."));
    assert_eq!(read(root, "src/A.java"), A_JAVA, "a dry run writes nothing");

    let only = mote(
        root,
        &["fix", "--no-cache", "--rule", "java/long-literal-lowercase"],
    );
    assert_eq!(only.code, 1, "{only:?}");
    assert!(!only.stdout.contains("import"), "{only:?}");
    assert!(only.stderr.contains("Would fix 1 finding in 1 file:"));

    let w = mote(root, &["fix", "--no-cache", "--write"]);
    assert_eq!(w.code, 0, "{w:?}");
    assert!(w.stdout.contains("Fixed 4 findings in 3 files:"), "{w:?}");
    assert_eq!(read(root, "src/A.java"), A_FIXED);
    assert_eq!(
        std::fs::read(root.join("src/B.java")).unwrap(),
        B_FIXED.as_bytes(),
        "the byte-order mark, CRLF line ends and the suppressed import stay"
    );
    assert_eq!(
        read(root, "web/c.js"),
        "import { used } from './m';\nused();\n"
    );

    let again = mote(root, &["fix", "--no-cache"]);
    assert_eq!(again.code, 0, "{again:?}");
    assert_eq!(again.stdout, "");
    assert!(again.stderr.contains("Nothing to fix."), "{again:?}");
}

#[test]
fn fix_respects_include_and_triage_decisions() {
    let dir = project();
    let root = dir.path();
    let r = mote(root, &["fix", "--no-cache", "--include", "web/**"]);
    assert_eq!(r.code, 1, "{r:?}");
    assert!(!r.stdout.contains("A.java") && r.stdout.contains("web/c.js"));

    // An accepted finding is not rewritten.
    let scan = mote(
        root,
        &["scan", "--no-cache", "--no-gate", "--json", "results.json"],
    );
    assert_eq!(scan.code, 0, "{scan:?}");
    let results: Value = serde_json::from_str(&read(root, "results.json")).unwrap();
    let id = results["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rule_id"] == "java/long-literal-lowercase")
        .map(|f| f["id"].as_str().unwrap().to_string())
        .unwrap();
    let t = mote(
        root,
        &[
            "triage",
            "add",
            &id,
            "--status",
            "accepted",
            "--reason",
            "kept",
            "--results",
            "results.json",
        ],
    );
    assert_eq!(t.code, 0, "{t:?}");
    let r = mote(root, &["fix", "--no-cache"]);
    assert!(!r.stdout.contains("10L"), "{r:?}");
    assert!(
        r.stderr.contains("Would fix 3 findings in 3 files:"),
        "{r:?}"
    );
}

#[test]
fn fix_usage_errors_exit_2() {
    let dir = project();
    let root = dir.path();
    for (args, needle) in [
        (&["fix", "--rev", "HEAD"][..], "working tree only"),
        (&["fix", "--staged"][..], "working tree only"),
        (&["fix", "--rule", "java/no-such-rule"][..], "unknown rule"),
        (
            &["fix", "--rule", "java/magic-number"][..],
            "offers no fixes",
        ),
    ] {
        let r = mote(root, args);
        assert_eq!(r.code, 2, "{args:?}: {r:?}");
        assert!(r.stderr.contains(needle), "{args:?}: {r:?}");
    }
    assert_eq!(read(root, "src/A.java"), A_JAVA);
}

#[test]
fn outputs_mark_fixable_findings() {
    let dir = project();
    let root = dir.path();
    let text = mote(
        root,
        &["scan", "--no-cache", "--no-gate", "--color", "never"],
    );
    assert!(
        text.stdout
            .lines()
            .any(|l| l.contains("java/unused-import") && l.ends_with("[fixable]")),
        "{text:?}"
    );
    assert!(
        text.stdout
            .contains("4 findings can be fixed automatically: `mote fix` shows the changes"),
        "{text:?}"
    );
    let r = mote(
        root,
        &[
            "scan",
            "--no-cache",
            "--no-gate",
            "--format",
            "sarif",
            "--json",
            "r.json",
        ],
    );
    let sarif: Value = serde_json::from_str(&r.stdout).unwrap();
    let results = sarif["runs"][0]["results"].as_array().unwrap();
    let fixed: Vec<&Value> = results
        .iter()
        .filter(|r| r.get("fixes").is_some())
        .collect();
    assert_eq!(fixed.len(), 4, "{results:#?}");
    let long = fixed
        .iter()
        .find(|r| r["ruleId"] == "java/long-literal-lowercase")
        .unwrap();
    let change = &long["fixes"][0]["artifactChanges"][0];
    assert_eq!(change["artifactLocation"]["uri"], "src/A.java");
    assert_eq!(
        change["replacements"][0],
        serde_json::json!({
            "deletedRegion": {"startLine": 8, "startColumn": 16, "endLine": 8, "endColumn": 17},
            "insertedContent": {"text": "L"}
        })
    );
    let import = fixed
        .iter()
        .find(|r| r["ruleId"] == "java/unused-import")
        .unwrap();
    let region = &import["fixes"][0]["artifactChanges"][0]["replacements"][0];
    assert_eq!(
        region["deletedRegion"],
        serde_json::json!({"startLine": 3, "startColumn": 1, "endLine": 4, "endColumn": 1})
    );
    assert!(region.get("insertedContent").is_none(), "a deletion");
    let json: Value = serde_json::from_str(&read(root, "r.json")).unwrap();
    let with_fix = json["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f.get("fix").is_some())
        .count();
    assert_eq!(with_fix, 4);
    // The rule catalog says which rules offer fixes.
    let rules = mote(root, &["rules", "--all", "--format", "json"]);
    let rules: Value = serde_json::from_str(&rules.stdout).unwrap();
    let fixable = |id: &str| {
        rules
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .map(|r| r["fixable"].clone())
    };
    assert_eq!(fixable("java/unused-import"), Some(Value::Bool(true)));
    assert_eq!(fixable("java/magic-number"), Some(Value::Bool(false)));
    let explain = mote(root, &["rules", "explain", "js/prefer-const"]);
    assert!(explain.stdout.contains("Fix available:"), "{explain:?}");
}
