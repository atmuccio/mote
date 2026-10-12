//! Triage files and baseline comparisons, end to end with the fake plugin.

mod common;

use common::{commit_all, init_repo, opts, scanner, write};
use mote_core::{Finding, FindingStatus};
use mote_engine::baseline::Baseline;
use mote_engine::gate::NO_LINE_DIFF;
use mote_engine::result::ScanResult;
use mote_engine::scan::{ScanOptions, SourceSpec};
use mote_engine::triage::{TriageEntry, TriageFile, TriageStatus};
use std::path::Path;
use std::sync::Arc;

fn find<'a>(r: &'a ScanResult, path: &str, rule: &str) -> &'a Finding {
    r.findings
        .iter()
        .find(|f| f.path == path && f.rule_id == rule)
        .unwrap_or_else(|| panic!("no {rule} in {path}: {:#?}", r.findings))
}

fn entry(f: &Finding, status: TriageStatus) -> TriageEntry {
    TriageEntry {
        fingerprint: Some(f.fingerprint.clone()),
        id: None,
        rule: f.rule_id.clone(),
        path: f.path.clone(),
        status,
        reason: "checked by hand".into(),
        by: Some("tester".into()),
        date: Some("2026-09-28".into()),
        line: 0,
    }
}

fn write_triage(root: &Path, entries: Vec<TriageEntry>) {
    let text = TriageFile { entries }.render();
    write(root, ".mote-triage.yaml", &text);
}

fn stale_findings(r: &ScanResult) -> Vec<(&str, u32)> {
    r.findings
        .iter()
        .filter(|f| f.rule_id == "mote/unused-suppression")
        .map(|f| (f.path.as_str(), f.range.start_line))
        .collect()
}

#[test]
fn triage_suppresses_matches_and_reports_stale_entries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "LOUD\nHIGHBUG\n");
    write(root, "b.fk", "LOUD\n");
    let (s, _) = scanner(root, "");
    let first = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    assert!(
        !first.gate.passed,
        "the HIGH finding fails the default gate"
    );
    assert!(first.triage.is_none());

    let high = find(&first, "a.fk", "fk/high-bug").clone();
    let loud_b = find(&first, "b.fk", "fk/loud").clone();
    let mut by_id = entry(&loud_b, TriageStatus::Accepted);
    by_id.fingerprint = None;
    by_id.id = Some(loud_b.id.clone());
    let mut stale_here = entry(&high, TriageStatus::Accepted);
    stale_here.fingerprint = Some("0".repeat(32));
    let mut gone = entry(&high, TriageStatus::Accepted);
    gone.path = "deleted.fk".into();
    gone.fingerprint = Some("1".repeat(32));
    write_triage(
        root,
        vec![
            entry(&high, TriageStatus::FalsePositive),
            by_id,
            stale_here,
            gone,
        ],
    );
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let h = find(&r, "a.fk", "fk/high-bug");
    let sup = h.suppressed.as_ref().expect("triaged");
    assert_eq!(sup.kind, "false-positive");
    assert_eq!(sup.justification.as_deref(), Some("checked by hand"));
    assert_eq!(
        find(&r, "b.fk", "fk/loud")
            .suppressed
            .as_ref()
            .unwrap()
            .kind,
        "accepted"
    );
    assert!(r.gate.passed, "triaged findings never gate: {:?}", r.gate);
    assert_eq!(r.measures.total.findings_high, 0);
    assert_eq!(r.measures.total.suppressed_findings, 2);
    let report = r.triage.as_ref().unwrap();
    assert_eq!(report.file, ".mote-triage.yaml");
    assert_eq!((report.entries, report.matched), (4, 2));
    let stale_paths: Vec<&str> = report.stale.iter().map(|k| k.path.as_str()).collect();
    assert_eq!(stale_paths, vec!["a.fk", "deleted.fk"]);
    // Sorted file: a.fk (high-bug fp, then the zero fp), b.fk, deleted.fk → lines of the items.
    let stale = stale_findings(&r);
    assert_eq!(stale.len(), 2, "{stale:?}");
    assert!(stale.iter().all(|(p, _)| *p == ".mote-triage.yaml"));
    assert!(stale.iter().all(|(_, l)| *l > 5), "{stale:?}");

    // A partial scan reports only entries of the files it covered.
    let r = s
        .scan(&ScanOptions {
            paths: vec!["b.fk".into()],
            ..opts(SourceSpec::Directory, None)
        })
        .unwrap();
    assert!(r.triage.as_ref().unwrap().stale.is_empty());
    assert!(stale_findings(&r).is_empty());

    // A renamed file keeps its entries.
    std::fs::create_dir_all(root.join("moved")).unwrap();
    std::fs::rename(root.join("a.fk"), root.join("moved/a.fk")).unwrap();
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    assert_eq!(
        find(&r, "moved/a.fk", "fk/high-bug")
            .suppressed
            .as_ref()
            .map(|s| s.kind.as_str()),
        Some("false-positive")
    );

    // The meta rule can be switched off; the report still lists the stale entries.
    let (quiet, _) = scanner(root, "rules: { mote/unused-suppression: off }\n");
    let r = quiet.scan(&opts(SourceSpec::Directory, None)).unwrap();
    assert!(stale_findings(&r).is_empty());
    assert_eq!(r.triage.as_ref().unwrap().stale.len(), 2);
}

#[test]
fn triage_file_location_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "HIGHBUG\n");
    let (s, _) = scanner(root, "triage: { file: config/triage.yaml }\n");
    let first = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let e = entry(&first.findings[0], TriageStatus::Accepted);
    write(
        root,
        "config/triage.yaml",
        &TriageFile {
            entries: vec![e.clone()],
        }
        .render(),
    );
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    assert!(r.findings[0].suppressed.is_some());
    assert_eq!(r.triage.as_ref().unwrap().file, "config/triage.yaml");

    // An explicit file wins over the configured one.
    let other = tempfile::tempdir().unwrap();
    let explicit = other.path().join("t.yaml");
    std::fs::write(&explicit, TriageFile::default().render()).unwrap();
    let r = s
        .scan(&ScanOptions {
            triage_file: Some(explicit.clone()),
            ..opts(SourceSpec::Directory, None)
        })
        .unwrap();
    assert!(r.findings[0].suppressed.is_none());
    assert_eq!(r.triage.as_ref().unwrap().entries, 0);

    write(root, "config/triage.yaml", "entries:\n  - rule: nope\n");
    let err = s.scan(&opts(SourceSpec::Directory, None)).unwrap_err();
    assert!(
        err.to_string()
            .contains("invalid triage file config/triage.yaml"),
        "{err}"
    );
    let err = s
        .scan(&ScanOptions {
            triage_file: Some(root.join("missing.yaml")),
            ..opts(SourceSpec::Directory, None)
        })
        .unwrap_err();
    assert!(err.to_string().contains("cannot read triage file"), "{err}");
}

#[test]
fn revision_scans_read_the_triage_file_of_the_revision() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a.fk", "HIGHBUG\n");
    let (s, _) = scanner(root, "");
    let first = s.scan(&opts(SourceSpec::Auto, None)).unwrap();
    write_triage(
        root,
        vec![entry(&first.findings[0], TriageStatus::FalsePositive)],
    );
    commit_all(root, "init");
    write_triage(root, vec![]);
    let r = s.scan(&opts(SourceSpec::Rev("HEAD".into()), None)).unwrap();
    assert!(r.findings[0].suppressed.is_some(), "the committed file");
    let r = s.scan(&opts(SourceSpec::Auto, None)).unwrap();
    assert!(
        r.findings[0].suppressed.is_none(),
        "the working tree's file"
    );
}

#[test]
fn baseline_classifies_findings_and_skips_line_conditions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "a.fk", "LOUD\nHIGHBUG\n");
    write(root, "b.fk", "fn f cog=30\nend\n");
    let (s, _) = scanner(root, "gate: { new_maintainability_grade: A }\n");
    let old = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    // Round trip through JSON, like a file written by `scan --json`.
    let json = serde_json::to_string(&old).unwrap();
    let saved: ScanResult = serde_json::from_str(&json).unwrap();
    let baseline = Arc::new(Baseline::from_result("baseline.json", saved));

    // HIGHBUG fixed, a new HIGHBUG elsewhere, the LOUD line moved down, complexity grew.
    write(root, "a.fk", "\n\nLOUD\n");
    write(root, "c.fk", "HIGHBUG\n");
    write(root, "b.fk", "fn f cog=40\nend\n");
    let r = s
        .scan(&ScanOptions {
            baseline: Some(baseline.clone()),
            ..opts(SourceSpec::Directory, None)
        })
        .unwrap();
    assert_eq!(find(&r, "a.fk", "fk/loud").status, FindingStatus::Unchanged);
    assert_eq!(find(&r, "c.fk", "fk/high-bug").status, FindingStatus::New);
    assert_eq!(
        find(&r, "b.fk", "fk/cognitive-complexity").status,
        FindingStatus::Worsened
    );
    let nc = r.new_code.as_ref().expect("a comparison");
    let info = nc.baseline.as_ref().expect("baseline info");
    assert_eq!(info.file, "baseline.json");
    assert_eq!(nc.base, "baseline.json");
    assert_eq!(nc.measures.new_findings.high, 1);
    assert_eq!(nc.measures.fixed_findings.high, 1);
    assert!(nc.new_lines.is_empty());
    assert!(!r.gate.passed, "the new HIGH finding fails the gate");
    let cond = |name: &str| r.gate.conditions.iter().find(|c| c.name == name).unwrap();
    assert_eq!(cond("new_findings").actual, Some(1.0));
    for name in [
        "new_duplication_pct",
        "new_coverage_pct",
        "new_maintainability_grade",
    ] {
        assert_eq!(cond(name).skipped_reason.as_deref(), Some(NO_LINE_DIFF));
    }
    let g = r.grades.as_ref().unwrap();
    assert!(g.new_maintainability.is_none());

    // The gate on the saved result agrees.
    let mut again: ScanResult = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    mote_engine::gate::reevaluate_gate(&mut again, s.config());
    assert_eq!(again.gate, r.gate);

    // Other engine versions are pointed out; a git base and a baseline exclude each other.
    let mut foreign = (*baseline).clone();
    foreign.engine_version = "0.0.1".into();
    let r = s
        .scan(&ScanOptions {
            baseline: Some(Arc::new(foreign)),
            ..opts(SourceSpec::Directory, None)
        })
        .unwrap();
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.message.contains("written by mote 0.0.1")),
        "{:?}",
        r.diagnostics
    );
    assert!(s
        .scan(&ScanOptions {
            baseline: Some(baseline),
            base: Some("main".into()),
            ..opts(SourceSpec::Directory, None)
        })
        .is_err());
}
