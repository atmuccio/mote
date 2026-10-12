//! Path-specific `overrides` with the real Java and JavaScript/TypeScript plugins: many entries,
//! each file resolving to its own rule set (enabled rules, severities, parameters). The plugins
//! resolve every distinct rule set once and reuse it for all the files that share it, so each
//! file must still see exactly its own set, whatever the thread count.

mod common;

use common::*;
use serde_json::Value;

const JAVA: &str = "class A {
  int a, b, c;
  void f() {
    try { g(); } catch (Exception e) { }
  }
}
";

const TS: &str = "export function f(a: number) {
  var x = a == 1;
  debugger;
  return x;
}
";

const CONFIG: &str = r#"rules:
  java/too-many-fields: { params: { max: 2 } }
overrides:
  - paths: ["d0/**"]
    rules: { java/empty-catch-block: off }
  - paths: ["d1/**"]
    rules: { java/too-many-fields: { params: { max: 5 } } }
  - paths: ["d2/**"]
    rules: { js/no-var: off, js/eqeqeq: off }
  - paths: ["d3/**"]
    rules: { js/no-debugger: LOW, java/empty-catch-block: HIGH }
  - paths: ["d4/**", "d5/**"]
    rules: { java/too-many-fields: off }
  - paths: ["d5/**"]
    rules: { java/too-many-fields: on }
  - paths: ["d6/**"]
    rules: { java/empty-catch-block: off, js/no-debugger: off }
  - paths: ["**/*.ts"]
    rules: { js/no-var: LOW }
  - paths: ["d7/**"]
    rules: { js/no-var: HIGH, java/too-many-fields: { params: { max: 1 } } }
  - paths: ["d8/**"]
    rules: { "java/*": off }
  - paths: ["d8/a.ts", "d9/**"]
    rules: { "js/*": off }
"#;

/// `(path, rule, severity)` of every finding, in result order.
fn findings(results: &Value) -> Vec<String> {
    results["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|f| {
            format!(
                "{} {} {}",
                f["path"].as_str().unwrap_or(""),
                f["rule_id"].as_str().unwrap_or(""),
                f["severity"].as_str().unwrap_or("")
            )
        })
        .collect()
}

#[test]
fn many_overrides_give_each_file_its_own_rules() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..10 {
        write(d.path(), &format!("d{i}/A.java"), JAVA);
        write(d.path(), &format!("d{i}/a.ts"), TS);
    }
    write(d.path(), ".mote.yaml", CONFIG);

    let scan = |threads: &str| -> Value {
        let out = format!("results-{threads}.json");
        let r = mote(
            d.path(),
            &[
                "scan",
                ".",
                "--no-cache",
                "--no-gate",
                "-q",
                "--format",
                "json",
                "-o",
                &out,
                "--threads",
                threads,
            ],
        );
        assert_eq!(r.code, 0, "{r:?}");
        let mut v: Value = serde_json::from_str(&read(d.path(), &out)).expect("results JSON");
        v["stats"] = Value::Null;
        v
    };
    let one = scan("1");
    let mut got = findings(&one);
    got.sort();
    let java = |dir: &str, rules: &[(&str, &str)]| -> Vec<String> {
        rules
            .iter()
            .map(|(rule, sev)| format!("{dir}/A.java java/{rule} {sev}"))
            .collect()
    };
    let ts = |dir: &str, rules: &[(&str, &str)]| -> Vec<String> {
        rules
            .iter()
            .map(|(rule, sev)| format!("{dir}/a.ts js/{rule} {sev}"))
            .collect()
    };
    let pkg = ("default-package", "LOW");
    let fields = ("too-many-fields", "LOW");
    let catch = ("empty-catch-block", "MEDIUM");
    let var = ("no-var", "LOW");
    let eq = ("eqeqeq", "MEDIUM");
    let debugger = ("no-debugger", "MEDIUM");
    let mut want: Vec<String> = [
        // `max: 2` from `rules`; the empty catch is off.
        java("d0", &[pkg, fields]),
        ts("d0", &[var, eq, debugger]),
        // `max: 5`: three fields are fine.
        java("d1", &[pkg, catch]),
        ts("d1", &[var, eq, debugger]),
        // `no-var` is switched on again (LOW) by the later `**/*.ts` entry.
        java("d2", &[pkg, fields, catch]),
        ts("d2", &[var, debugger]),
        java("d3", &[pkg, fields, ("empty-catch-block", "HIGH")]),
        ts("d3", &[var, eq, ("no-debugger", "LOW")]),
        java("d4", &[pkg, catch]),
        ts("d4", &[var, eq, debugger]),
        // Off, then on again by a later entry (with the parameters of `rules`).
        java("d5", &[pkg, fields, catch]),
        ts("d5", &[var, eq, debugger]),
        java("d6", &[pkg, fields]),
        ts("d6", &[var, eq]),
        java("d7", &[pkg, fields, catch]),
        ts("d7", &[("no-var", "HIGH"), eq, debugger]),
        // d8: nothing at all.
        java("d9", &[pkg, fields, catch]),
    ]
    .concat();
    want.sort();
    assert_eq!(got, want);
    // The measured value follows each file's parameters.
    let values: Vec<(String, f64)> = one["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["rule_id"] == "java/too-many-fields")
        .map(|f| {
            (
                f["path"].as_str().unwrap().to_string(),
                f["value"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(values.len(), 7, "{values:?}");
    assert!(values.iter().all(|(_, v)| *v == 3.0), "{values:?}");

    // Same results on several threads (files sharing a rule set analysed concurrently).
    assert_eq!(scan("4"), one);
}
