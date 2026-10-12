//! Fixture-driven rule tests shared by every language plugin.
//!
//! # Fixture format
//!
//! One file per rule (or several), stored in a plugin's `tests/fixtures/` directory:
//!
//! * The **rule under test** comes from the file name up to the first dot:
//!   `empty-catch-block.java` and `empty-catch-block.lombok.java` both test `<lang>/empty-catch-block`.
//! * `expect: <rule>[, <rule>…]` inside a comment (after `//`, `/*`, or on a `*` continuation
//!   line — text in code is ignored): a finding for each listed rule must start on
//!   **this** line. `expect-next-line: <rule>` refers to the following line. The language prefix
//!   is optional (`expect: empty-catch-block`). Repeat an id to expect several findings on a line.
//! * Every other line must produce **no** finding for the rule — compliant code in the same file
//!   is how false positives are tested.
//! * Directives (inside any comment): `mote-test: params threshold=3 other=true` overrides
//!   parameters; `mote-test: test-file` analyzes the file as test code;
//!   `mote-test: path src/main/Foo.java` sets the path the plugin sees.
//!
//! Only the rule under test is enabled, so fixtures are independent of each other.

use crate::model::{ParamValue, RuleMeta};
use crate::plugin::{ActiveRule, LanguagePlugin, RuleSet, SourceFile};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Build a rule set enabling `ids` with default parameters plus `overrides`.
pub fn ruleset_for(
    metas: &[&'static RuleMeta],
    ids: &[&str],
    overrides: &BTreeMap<String, ParamValue>,
) -> RuleSet {
    let mut map = FxHashMap::default();
    for meta in metas.iter().filter(|m| ids.contains(&m.id)) {
        let mut params: BTreeMap<String, ParamValue> = meta
            .params
            .iter()
            .map(|p| (p.name.to_string(), p.default.to_value()))
            .collect();
        for (k, v) in overrides {
            params.insert(k.clone(), v.clone());
        }
        map.insert(
            meta.id.to_string(),
            ActiveRule {
                severity: meta.severity,
                params,
            },
        );
    }
    RuleSet::new(map)
}

/// Enable every rule of the plugin with default parameters.
pub fn ruleset_all(plugin: &dyn LanguagePlugin) -> RuleSet {
    let metas = plugin.rules();
    let ids: Vec<&str> = metas.iter().map(|m| m.id).collect();
    ruleset_for(&metas, &ids, &BTreeMap::new())
}

struct Fixture {
    rule_id: String,
    expected: Vec<(u32, String)>,
    params: BTreeMap<String, ParamValue>,
    is_test: bool,
    path: Option<String>,
}

fn parse_param(v: &str) -> ParamValue {
    if let Ok(i) = v.parse::<i64>() {
        ParamValue::Int(i)
    } else if let Ok(f) = v.parse::<f64>() {
        ParamValue::Float(f)
    } else if v == "true" || v == "false" {
        ParamValue::Bool(v == "true")
    } else {
        ParamValue::Str(v.to_string())
    }
}

fn parse_ids(rest: &str, lang: &str) -> Vec<String> {
    let rest = rest.split("*/").next().unwrap_or("");
    rest.split([',', ' ', '\t'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.contains('/') {
                s.to_string()
            } else {
                format!("{lang}/{s}")
            }
        })
        .collect()
}

fn parse_fixture(lang: &str, file_name: &str, text: &str) -> Fixture {
    let stem = file_name.split('.').next().unwrap_or(file_name);
    let mut fx = Fixture {
        rule_id: format!("{lang}/{stem}"),
        expected: Vec::new(),
        params: BTreeMap::new(),
        is_test: false,
        path: None,
    };
    for (i, raw_line) in text.lines().enumerate() {
        let line_no = i as u32 + 1;
        // Directives only count inside comments: after `//`, `/*`, or on a `*` continuation line.
        let comment_start = [raw_line.find("//"), raw_line.find("/*")]
            .into_iter()
            .flatten()
            .min();
        let line = match comment_start {
            Some(pos) => &raw_line[pos..],
            None if raw_line.trim_start().starts_with('*') => raw_line,
            None => continue,
        };
        if let Some(pos) = line.find("expect-next-line:") {
            for id in parse_ids(&line[pos + "expect-next-line:".len()..], lang) {
                fx.expected.push((line_no + 1, id));
            }
        } else if let Some(pos) = line.find("expect:") {
            for id in parse_ids(&line[pos + "expect:".len()..], lang) {
                fx.expected.push((line_no, id));
            }
        }
        if let Some(pos) = line.find("mote-test:") {
            let directive = line[pos + "mote-test:".len()..]
                .split("*/")
                .next()
                .unwrap_or("")
                .trim();
            let mut parts = directive.split_whitespace();
            match parts.next() {
                Some("params") => {
                    for kv in parts {
                        if let Some((k, v)) = kv.split_once('=') {
                            fx.params.insert(k.to_string(), parse_param(v));
                        }
                    }
                }
                Some("test-file") => fx.is_test = true,
                Some("path") => fx.path = parts.next().map(str::to_string),
                _ => {}
            }
        }
    }
    fx
}

/// Result of running a fixture directory.
#[derive(Default, Debug)]
pub struct FixtureReport {
    pub files: usize,
    pub expectations: usize,
    pub failures: Vec<String>,
    pub rules_without_fixtures: Vec<String>,
}

fn fixture_files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read fixture dir {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| exts.contains(&e))
        })
        .collect();
    files.sort();
    files
}

/// Run every fixture in `dir` (optionally only those whose rule name contains `filter`).
pub fn run_fixture_dir(
    plugin: &dyn LanguagePlugin,
    dir: &Path,
    filter: Option<&str>,
) -> FixtureReport {
    let metas = plugin.rules();
    let known: FxHashMap<&str, &RuleMeta> = metas.iter().map(|m| (m.id, *m)).collect();
    let mut report = FixtureReport::default();
    let mut covered: FxHashMap<String, usize> = FxHashMap::default();

    for path in fixture_files(dir, plugin.extensions()) {
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("read fixture");
        let fx = parse_fixture(plugin.id(), &file_name, &text);
        if filter.is_some_and(|f| !fx.rule_id.contains(f)) {
            continue;
        }
        report.files += 1;
        if !known.contains_key(fx.rule_id.as_str()) {
            report
                .failures
                .push(format!("{file_name}: no rule named `{}`", fx.rule_id));
            continue;
        }
        *covered.entry(fx.rule_id.clone()).or_default() += fx.expected.len();
        report.expectations += fx.expected.len();

        let rules = ruleset_for(&metas, &[fx.rule_id.as_str()], &fx.params);
        let default_path = format!("src/{file_name}");
        let src = SourceFile {
            path: fx.path.as_deref().unwrap_or(&default_path),
            text: &text,
            is_test: fx.is_test,
        };
        let analysis = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            plugin.analyze(&src, &rules)
        })) {
            Ok(a) => a,
            Err(_) => {
                report
                    .failures
                    .push(format!("{file_name}: plugin panicked"));
                continue;
            }
        };

        let mut expected: Vec<(u32, String)> = fx
            .expected
            .iter()
            .filter(|(_, id)| *id == fx.rule_id)
            .cloned()
            .collect();
        for (_, id) in fx.expected.iter().filter(|(_, id)| *id != fx.rule_id) {
            report.failures.push(format!(
                "{file_name}: expectation for `{id}` but fixture tests `{}`",
                fx.rule_id
            ));
        }
        let mut unexpected = Vec::new();
        for f in analysis.findings.iter().filter(|f| f.rule_id == fx.rule_id) {
            if let Some(pos) = expected.iter().position(|(l, _)| *l == f.range.start_line) {
                expected.swap_remove(pos);
            } else {
                unexpected.push(f);
            }
        }
        for f in unexpected {
            report.failures.push(format!(
                "{file_name}:{}: unexpected {} — \"{}\"",
                f.range.start_line, f.rule_id, f.message
            ));
        }
        for (line, id) in expected {
            report
                .failures
                .push(format!("{file_name}:{line}: missing {id}"));
        }
    }

    if filter.is_none() {
        for meta in &metas {
            if covered.get(meta.id).copied().unwrap_or(0) == 0 {
                report.rules_without_fixtures.push(meta.id.to_string());
            }
        }
    }
    report
}

/// Test entry point: runs all fixtures and panics with a readable report on any failure,
/// including rules that have no positive (`expect:`) fixture.
pub fn assert_fixtures(plugin: &dyn LanguagePlugin, dir: &Path) {
    let filter = std::env::var("MOTE_FIXTURE").ok();
    let report = run_fixture_dir(plugin, dir, filter.as_deref());
    let mut problems = report.failures.clone();
    for id in &report.rules_without_fixtures {
        problems.push(format!("rule `{id}` has no fixture with an `expect:` line"));
    }
    if !problems.is_empty() {
        panic!(
            "{} fixture problem(s) in {} file(s):\n  {}",
            problems.len(),
            report.files,
            problems.join("\n  ")
        );
    }
}

/// Every rule enabled on every fixture: nothing may panic, and every finding must reference an
/// enabled rule and a line inside the file.
pub fn assert_all_rules_robust(plugin: &dyn LanguagePlugin, dir: &Path) {
    let rules = ruleset_all(plugin);
    let mut problems = Vec::new();
    for path in fixture_files(dir, plugin.extensions()) {
        let text = std::fs::read_to_string(&path).expect("read fixture");
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let fixture_path = format!("src/{name}");
        let src = SourceFile {
            path: &fixture_path,
            text: &text,
            is_test: false,
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            plugin.analyze(&src, &rules)
        })) {
            Err(_) => problems.push(format!("{name}: panicked with all rules enabled")),
            Ok(a) => {
                let lines = text.lines().count().max(1) as u32;
                for f in &a.findings {
                    if !rules.is_enabled(&f.rule_id) {
                        problems.push(format!(
                            "{name}: finding for unknown/disabled rule {}",
                            f.rule_id
                        ));
                    }
                    if f.range.start_line == 0 || f.range.start_line > lines + 1 {
                        problems.push(format!(
                            "{name}: {} reported at invalid line {}",
                            f.rule_id, f.range.start_line
                        ));
                    }
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "robustness problems:\n  {}",
        problems.join("\n  ")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_expectations_and_directives() {
        let text = "// mote-test: params threshold=3 strict=true\nfoo(); // expect: bar\n// expect-next-line: java/baz, bar\nx();\n";
        let fx = parse_fixture("java", "bar.extra.java", text);
        assert_eq!(fx.rule_id, "java/bar");
        assert_eq!(
            fx.expected,
            vec![
                (2, "java/bar".into()),
                (4, "java/baz".into()),
                (4, "java/bar".into())
            ]
        );
        assert_eq!(fx.params.get("threshold"), Some(&ParamValue::Int(3)));
        assert_eq!(fx.params.get("strict"), Some(&ParamValue::Bool(true)));
    }
}
