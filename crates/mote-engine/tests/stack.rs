//! Analysis never runs on a small stack: plugins that recurse deeply (parsers on deeply nested
//! input) get the analysis pool's large stacks in every entry point. A stack overflow aborts the
//! whole process, so these tests live in their own binary.

mod common;

use common::{commit_all, init_repo, opts, scanner, write};
use mote_engine::result::FileStatus;
use mote_engine::scan::{ScanOptions, SourceSpec};

const DEEP: &str = "x;\nDEEP_STACK\nLOUD\n";

#[test]
fn scan_analyze_paths_and_rescan_use_big_stacks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "deep.fk", DEEP);
    write(root, "other.fk", "MAGIC\n");
    let (s, _) = scanner(root, "");
    for threads in [None, Some(1), Some(3)] {
        let o = ScanOptions {
            threads,
            ..opts(SourceSpec::Directory, None)
        };
        let r = s.scan(&o).unwrap();
        let f = r.files.iter().find(|f| f.path == "deep.fk").unwrap();
        assert_eq!(f.status, FileStatus::Analyzed);
        assert!(r
            .findings
            .iter()
            .any(|f| f.path == "deep.fk" && f.rule_id == "fk/loud"));

        let r2 = s.analyze_paths(&["deep.fk".into()], &o).unwrap();
        assert_eq!(r2.findings.len(), 1);

        let r3 = s.rescan(&r, &["deep.fk".into()], &o).unwrap();
        assert_eq!(r3.stats.files_analyzed, 2, "no cache: every file is read");
    }
}

#[test]
fn base_side_analysis_uses_big_stacks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "deep.fk", DEEP);
    write(root, "gone.fk", DEEP);
    commit_all(root, "base");
    common::git(root, &["checkout", "-q", "-b", "feature"]);
    write(root, "deep.fk", "y;\nLOUD\nHIGHBUG\n");
    std::fs::remove_file(root.join("gone.fk")).unwrap();
    commit_all(root, "feature");
    let (s, _) = scanner(root, "");
    let r = s
        .scan(&ScanOptions {
            base: Some("main".into()),
            ..opts(SourceSpec::Worktree, None)
        })
        .unwrap();
    assert!(r.stats.base_files_analyzed >= 1);
    assert_eq!(r.new_code.unwrap().measures.new_findings.high, 1);
}
