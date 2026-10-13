//! Fixes end to end with the fake plugin: findings carry them through the cache, suppressed
//! findings lose them, and `Scanner::fix_files` applies them in memory with its safety net.

mod common;

use common::{opts, scanner, write};
use mote_engine::fix::{FixOptions, FixProblem};
use mote_engine::scan::SourceSpec;
use mote_engine::triage::{TriageEntry, TriageFile, TriageStatus};

fn fix(root: &std::path::Path, files: &[&str], o: &FixOptions) -> Vec<mote_engine::fix::FileFix> {
    let (s, _) = scanner(root, "");
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    s.fix_files(&files, o, &opts(SourceSpec::Directory, None))
        .expect("fix")
}

#[test]
fn findings_carry_fixes_through_the_cache_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let cache = root.join(".cache");
    write(root, "a.fk", "x FIX_ME y\nLOUD\n");
    let (s, _) = scanner(root, "");
    let first = s.scan(&opts(SourceSpec::Directory, Some(&cache))).unwrap();
    let second = s.scan(&opts(SourceSpec::Directory, Some(&cache))).unwrap();
    assert_eq!(
        second.stats.cache_hits, 1,
        "the second scan reads the cache"
    );
    for r in [&first, &second] {
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "fk/fixable")
            .unwrap();
        let fix = f.fix.as_ref().expect("a fix");
        assert_eq!(fix.description, "Write fixed");
        assert_eq!((fix.edits[0].start, fix.edits[0].end), (2, 8));
        let r = fix.edits[0].range;
        assert_eq!(
            (r.start_line, r.start_col, r.end_line, r.end_col),
            (1, 3, 1, 9)
        );
    }
    // JSON: `fix` only where there is one, and the result reads back.
    let json = serde_json::to_value(&first).unwrap();
    let findings = json["findings"].as_array().unwrap();
    let with: Vec<bool> = findings.iter().map(|f| f.get("fix").is_some()).collect();
    assert_eq!(with.iter().filter(|w| **w).count(), 1, "{with:?}");
    let back: mote_engine::ScanResult = serde_json::from_value(json).unwrap();
    assert!(back.findings.iter().any(|f| f.fix.is_some()));
    let rules = serde_json::to_value(&first.rules).unwrap();
    let fixable: Vec<&str> = rules
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["fixable"] == true)
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(fixable, ["fk/fixable"]);
}

#[test]
fn suppressed_findings_have_no_fix() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "@Suppress(fk/fixable)\nFIX_ME\nFIX_ME\n");
    let (s, _) = scanner(root, "");
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let fixes: Vec<(u32, bool, bool)> = r
        .findings
        .iter()
        .filter(|f| f.rule_id == "fk/fixable")
        .map(|f| (f.range.start_line, f.suppressed.is_some(), f.fix.is_some()))
        .collect();
    assert_eq!(fixes, [(2, true, false), (3, false, true)]);
    let out = fix(root, &["a.fk"], &FixOptions::default());
    assert_eq!(
        String::from_utf8(out[0].fixed.clone()).unwrap(),
        "@Suppress(fk/fixable)\nFIX_ME\nfixed\n"
    );
}

#[test]
fn fixes_are_applied_in_memory_keeping_bom_and_line_ends() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let text = "\u{feff}a FIX_ME\r\nb FIX_ME FIX_ME\r\n";
    write(root, "a.fk", text);
    let out = fix(root, &["a.fk"], &FixOptions::default());
    assert_eq!(out.len(), 1);
    let f = &out[0];
    assert!(f.problem.is_none(), "{:?}", f.problem);
    assert_eq!(f.original, text.as_bytes());
    // The plugin reports one marker per line: the second one of line 2 in the next pass.
    assert_eq!(
        String::from_utf8(f.fixed.clone()).unwrap(),
        "\u{feff}a fixed\r\nb fixed fixed\r\n"
    );
    assert_eq!(f.applied.get("fk/fixable"), Some(&3));
    assert_eq!((f.passes, f.remaining), (2, 0));
    // Nothing was written.
    assert_eq!(std::fs::read(root.join("a.fk")).unwrap(), text.as_bytes());
}

#[test]
fn passes_repeat_until_no_fix_is_left() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "FIX_ME FIX_ME FIX_ME\n");
    let out = fix(root, &["a.fk"], &FixOptions::default());
    assert_eq!(out[0].fixed, b"fixed fixed fixed\n");
    assert_eq!((out[0].passes, out[0].remaining), (3, 0));
    // At most one pass: a fix is still offered after it.
    let one = fix(
        root,
        &["a.fk"],
        &FixOptions {
            max_passes: 1,
            ..Default::default()
        },
    );
    assert_eq!(one[0].fixed, b"fixed FIX_ME FIX_ME\n");
    assert_eq!(one[0].remaining, 1);
}

#[test]
fn a_fix_that_adds_syntax_errors_is_discarded() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "FIX_ME\n");
    write(root, "b.fk", "FIX_ME\nBREAK_IT\n");
    let out = fix(root, &["a.fk", "b.fk"], &FixOptions::default());
    assert_eq!(out[0].fixed, b"fixed\n");
    assert!(out[0].problem.is_none());
    let b = &out[1];
    assert_eq!(
        b.problem,
        Some(FixProblem::SyntaxErrors {
            before: 0,
            after: 1
        })
    );
    assert!(b.problem.as_ref().unwrap().is_discard());
    assert!(!b.changed(), "every change of the file is discarded");
    assert!(b.applied.is_empty());
}

#[test]
fn only_selected_rules_and_utf8_files_are_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "FIX_ME\n");
    std::fs::write(root.join("latin1.fk"), b"FIX_ME caf\xe9\n").unwrap();
    let none = fix(
        root,
        &["a.fk"],
        &FixOptions {
            rules: vec!["fk/loud".into()],
            ..Default::default()
        },
    );
    assert!(!none[0].changed());
    let out = fix(root, &["latin1.fk"], &FixOptions::default());
    assert_eq!(out[0].problem, Some(FixProblem::Encoding));
    assert!(!out[0].changed());
}

#[test]
fn triaged_findings_are_not_fixed() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "FIX_ME\nother FIX_ME\n");
    let (s, _) = scanner(root, "");
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let first = r
        .findings
        .iter()
        .find(|f| f.rule_id == "fk/fixable" && f.range.start_line == 1)
        .unwrap();
    let entry = TriageEntry {
        fingerprint: Some(first.fingerprint.clone()),
        id: None,
        rule: first.rule_id.clone(),
        path: first.path.clone(),
        status: TriageStatus::Accepted,
        reason: "kept on purpose".into(),
        by: None,
        date: None,
        line: 0,
    };
    write(
        root,
        ".mote-triage.yaml",
        &TriageFile {
            entries: vec![entry],
        }
        .render(),
    );
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let triaged = r
        .findings
        .iter()
        .find(|f| f.rule_id == "fk/fixable" && f.range.start_line == 1)
        .unwrap();
    assert!(triaged.suppressed.is_some() && triaged.fix.is_none());
    let out = fix(root, &["a.fk"], &FixOptions::default());
    assert_eq!(out[0].fixed, b"FIX_ME\nother fixed\n");
}
