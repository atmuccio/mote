//! Regression tests for review findings: new-code matching, deleted files, diff paths with
//! spaces, unrelated histories, path filters, duplicated-block ranges, damaged caches and
//! generated-file detection.

mod common;

use common::{commit_all, dup_body, git, init_repo, opts, scanner, write};
use mote_core::{Finding, FindingStatus};
use mote_engine::result::{FileStatus, ScanResult};
use mote_engine::scan::{ScanOptions, SourceSpec};
use mote_engine::EngineError;

fn find<'a>(r: &'a ScanResult, path: &str, rule: &str) -> Vec<&'a Finding> {
    r.findings
        .iter()
        .filter(|f| f.path == path && f.rule_id == rule)
        .collect()
}

fn status_of(r: &ScanResult, path: &str) -> FileStatus {
    r.files
        .iter()
        .find(|f| f.path == path)
        .map(|f| f.status)
        .unwrap_or_else(|| panic!("no file {path}"))
}

fn with_base(base: &str) -> ScanOptions {
    ScanOptions {
        base: Some(base.into()),
        ..opts(SourceSpec::Worktree, None)
    }
}

#[test]
fn new_code_pairs_findings_by_the_code_they_sit_on() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    // A HIGH bug that gets fixed while a new one is added in the same function.
    write(root, "same.fk", "fn m\n  x;\n  HIGHBUG s == w\n  y;\nend\n");
    // A function (the "class") that gets renamed: its issues are not new.
    write(
        root,
        "old.fk",
        "fn old\n  LOUD\n  HIGHBUG z\n  a;\n  b;\nend\n",
    );
    // A file that gets deleted: its issues are fixed.
    write(root, "gone.fk", "HIGHBUG\nMAGIC\n");
    // A path with spaces: git ends its diff header with a TAB.
    write(root, "dir with space/sp.fk", "a;\nb;\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(
        root,
        "same.fk",
        "fn m\n  x;\n  ok s.equals(w)\n  y;\n  t = s;\n  HIGHBUG t == w2\n  z;\nend\n",
    );
    git(root, &["mv", "old.fk", "newer.fk"]);
    write(
        root,
        "newer.fk",
        "fn newer\n  LOUD\n  HIGHBUG z\n  a;\n  b;\nend\n",
    );
    std::fs::remove_file(root.join("gone.fk")).unwrap();
    write(root, "dir with space/sp.fk", "a;\nb;\nHIGHBUG\n");
    commit_all(root, "feature");

    let (s, _) = scanner(root, "");
    let r = s.scan(&with_base("main")).unwrap();
    let st = |path: &str, rule: &str| {
        find(&r, path, rule)
            .iter()
            .map(|f| (f.range.start_line, f.status))
            .collect::<Vec<_>>()
    };
    assert_eq!(st("same.fk", "fk/high-bug"), vec![(6, FindingStatus::New)]);
    assert_eq!(
        st("newer.fk", "fk/loud"),
        vec![(2, FindingStatus::Unchanged)]
    );
    assert_eq!(
        st("newer.fk", "fk/high-bug"),
        vec![(3, FindingStatus::Unchanged)]
    );
    assert_eq!(
        st("dir with space/sp.fk", "fk/high-bug"),
        vec![(3, FindingStatus::New)]
    );
    let nc = r.new_code.as_ref().unwrap();
    assert_eq!(nc.renamed["newer.fk"], "old.fk");
    assert_eq!(nc.new_lines["dir with space/sp.fk"], vec![(3, 3)]);
    assert_eq!(nc.new_lines["same.fk"], vec![(3, 3), (5, 7)]);
    // same.fk's old HIGHBUG, and gone.fk's HIGHBUG + MAGIC
    assert_eq!(nc.measures.fixed_findings.high, 2);
    assert_eq!(nc.measures.fixed_findings.low, 1);
    assert_eq!(nc.measures.new_findings.high, 2);
    assert!(!r.gate.passed);
}

#[test]
fn an_identical_line_added_above_an_old_one_is_the_new_one() {
    // Found by replaying real commits (docs/benchmark.md, Part 5): a commit adds a line identical
    // to an existing flagged one, above it. The added line takes over occurrence 0 of the shared
    // fingerprint, so fingerprints alone call the old, untouched line new and the added one
    // unchanged; the PR annotation lands on the wrong line.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a.fk", "fn m\n  a;\n  x = LOUD\n  b;\nend\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(
        root,
        "a.fk",
        "fn m\n  x = LOUD\n  a;\n  x = LOUD\n  b;\nend\n",
    );
    commit_all(root, "feature");

    let (s, _) = scanner(root, "");
    let r = s.scan(&with_base("main")).unwrap();
    let nc = r.new_code.as_ref().unwrap();
    assert_eq!(nc.new_lines["a.fk"], vec![(2, 2)]);
    let st: Vec<(u32, FindingStatus)> = find(&r, "a.fk", "fk/loud")
        .iter()
        .map(|f| (f.range.start_line, f.status))
        .collect();
    assert_eq!(
        st,
        vec![(2, FindingStatus::New), (4, FindingStatus::Unchanged)]
    );
    assert_eq!(nc.measures.new_findings.medium, 1);
    assert_eq!(nc.measures.fixed_findings.total(), 0);
}

#[test]
fn moved_code_keeps_its_findings_within_a_file_and_across_files() {
    // Found by replaying real commits (docs/benchmark.md, Part 5): extracting a method, or moving
    // code into a new file, changes the findings' symbols and puts them on added lines, so they
    // were reported as new although the change only moved them.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(
        root,
        "a.fk",
        "fn big\n  a;\n  b;\n  x = LOUD\n  c;\n  d;\nend\n",
    );
    write(root, "old.fk", "fn tool\n  p;\n  y = HIGHBUG\n  q;\nend\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "feature"]);
    // `b; x = LOUD c;` extracted into `helper` (re-indented, so the lines are changed ones).
    write(
        root,
        "a.fk",
        "fn big\n  a;\n  d;\nend\nfn helper\n    b;\n    x = LOUD\n    c;\nend\n",
    );
    // `old.fk` deleted; its code moved into a new file next to new code, with a genuinely new
    // HIGHBUG on the same text but between other lines.
    std::fs::remove_file(root.join("old.fk")).unwrap();
    write(
        root,
        "new.fk",
        "fn other\n  r1;\n  r2;\n  r3;\n  y = HIGHBUG\n  r4;\n  r5;\n  r6;\nend\nfn tool2\n  p;\n  y = HIGHBUG\n  q;\nend\n",
    );
    commit_all(root, "feature");

    let (s, _) = scanner(root, "");
    let r = s.scan(&with_base("main")).unwrap();
    let nc = r.new_code.as_ref().unwrap();
    assert!(nc.renamed.is_empty(), "{:?}", nc.renamed);
    let st = |path: &str, rule: &str| {
        find(&r, path, rule)
            .iter()
            .map(|f| (f.range.start_line, f.status))
            .collect::<Vec<_>>()
    };
    assert_eq!(st("a.fk", "fk/loud"), vec![(7, FindingStatus::Unchanged)]);
    assert_eq!(
        st("new.fk", "fk/high-bug"),
        vec![(5, FindingStatus::New), (12, FindingStatus::Unchanged)]
    );
    assert_eq!(nc.measures.new_findings.total(), 1);
    assert_eq!(nc.measures.fixed_findings.total(), 0);
}

#[test]
fn unrelated_histories_are_a_clear_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a.fk", "LOUD\n");
    commit_all(root, "main");
    git(root, &["checkout", "-q", "--orphan", "other"]);
    write(root, "b.fk", "LOUD\n");
    commit_all(root, "other");
    let (s, _) = scanner(root, "");
    match s.scan(&with_base("main")) {
        Err(EngineError::Invalid(m)) => {
            assert!(
                m.contains("no common ancestor between `main` and HEAD"),
                "{m}"
            )
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn path_filters_accept_absolute_paths_and_reject_ones_matching_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "src/a.fk", "LOUD\n");
    write(root, "lib/b.fk", "LOUD\n");
    let (s, _) = scanner(root, "");
    let scan = |paths: Vec<String>| {
        s.scan(&ScanOptions {
            paths,
            ..opts(SourceSpec::Directory, None)
        })
    };
    let abs = root.join("src").to_string_lossy().into_owned();
    let r = scan(vec![abs.clone()]).unwrap();
    let paths: Vec<&str> = r.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["src/a.fk"]);
    let r = scan(vec![format!("{}/**/*.fk", abs.replace('\\', "/"))]).unwrap();
    assert_eq!(r.files.len(), 1);
    let r = scan(vec![format!("{abs}/../lib")]).unwrap();
    assert_eq!(r.files[0].path, "lib/b.fk");
    // A leading `/` anchors at the root (not the file system root, on any OS).
    for anchored in ["/src/**", "/src"] {
        let r = scan(vec![anchored.into()]).unwrap();
        let paths: Vec<&str> = r.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["src/a.fk"], "{anchored}");
    }
    // A typo, and a path outside the root, are errors instead of "0 files analyzed, passed".
    let outside = std::env::temp_dir()
        .join("mote-elsewhere")
        .to_string_lossy()
        .into_owned();
    for bad in [
        vec!["src/mian".to_string()],
        vec!["src".into(), "**/*.fkx".into()],
        vec![outside],
    ] {
        match scan(bad.clone()) {
            Err(EngineError::Invalid(m)) => {
                assert!(m.contains("no file") || m.contains("outside"), "{m}")
            }
            other => panic!("{bad:?}: {other:?}"),
        }
    }
}

#[test]
fn duplicated_blocks_end_at_the_end_of_their_last_line() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let body = dup_body("d", 12);
    write(root, "a.fk", &format!("fn a\n{body}end\n"));
    write(root, "b.fk", &format!("x;\nfn b\n{body}end\n"));
    let (s, _) = scanner(root, "");
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let line_len = |path: &str, line: u32| {
        std::fs::read_to_string(root.join(path))
            .unwrap()
            .lines()
            .nth(line as usize - 1)
            .unwrap()
            .chars()
            .count() as u32
    };
    let dups = find(&r, "a.fk", "fk/duplicated-block");
    assert_eq!(dups.len(), 1);
    let f = dups[0];
    assert!(line_len("a.fk", f.range.end_line) > 0);
    assert_eq!(
        f.range.end_col,
        line_len("a.fk", f.range.end_line) + 1,
        "{:?}",
        f.range
    );
    let sec = &f.secondary[0];
    assert_eq!(sec.path, "b.fk");
    assert_eq!(
        sec.range.end_col,
        line_len("b.fk", sec.range.end_line) + 1,
        "{:?}",
        sec.range
    );
}

#[test]
fn a_damaged_cache_never_fails_a_scan() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let root = dir.path();
    for i in 0..60 {
        write(
            root,
            &format!("m/f{i}.fk"),
            &format!("fn f{i} cog={}\nLOUD\nend\n{}", 10 + i, dup_body("q", 3)),
        );
    }
    let (s, _) = scanner(root, "");
    let o = opts(SourceSpec::Directory, Some(cache.path()));
    let clean = serde_json::to_string(&s.scan(&o).unwrap().findings).unwrap();
    s.close_cache();
    let file = cache.path().join(mote_engine::cache::CACHE_FILE);
    let original = std::fs::read(&file).unwrap();
    let mut noticed = 0;
    for offset in (0..original.len().min(512 << 10)).step_by(4096) {
        let mut damaged = original.clone();
        for b in damaged.iter_mut().skip(offset).take(512) {
            *b ^= 0x5A;
        }
        std::fs::write(&file, &damaged).unwrap();
        let r = s.scan(&o).unwrap();
        assert_eq!(
            serde_json::to_string(&r.findings).unwrap(),
            clean,
            "offset {offset}"
        );
        noticed += usize::from(r.stats.files_analyzed > 0);
        s.close_cache();
    }
    assert!(noticed > 0);
    // Once the file is intact again the cache is warm again.
    std::fs::write(&file, &original).unwrap();
    s.scan(&o).unwrap();
    let warm = s.scan(&o).unwrap();
    assert_eq!(warm.stats.files_analyzed, 0);
}

#[test]
fn a_changed_file_skipped_as_generated_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a.fk", "LOUD\n");
    write(root, "b.fk", "x;\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "feature"]);
    write(root, "a.fk", "// @generated\nHIGHBUG\n");
    // An ordinary file that mentions generated code is analyzed.
    write(
        root,
        "b.fk",
        "/* Entity with an auto-generated id; do not edit by hand */\nHIGHBUG\n",
    );
    commit_all(root, "feature");
    let (s, _) = scanner(root, "");
    let r = s.scan(&with_base("main")).unwrap();
    assert_eq!(status_of(&r, "a.fk"), FileStatus::Skipped);
    assert_eq!(r.stats.skipped_by_reason.get("generated"), Some(&1));
    assert!(r
        .diagnostics
        .iter()
        .any(|d| d.path.as_deref() == Some("a.fk") && d.message.contains("generated")));
    assert_eq!(
        find(&r, "b.fk", "fk/high-bug")[0].status,
        FindingStatus::New
    );
}

#[test]
fn analyze_paths_and_rescan_see_exactly_what_a_scan_sees() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, ".gitignore", "ignored/\n*.gen.fk\n");
    write(root, "src/a/E.fk", "LOUD\n");
    write(root, "src/tracked.gen.fk", "LOUD\n");
    write(root, "legacy/l.fk", "LOUD\n");
    git(root, &["add", "-A"]);
    git(root, &["add", "-f", "src/tracked.gen.fk"]);
    git(root, &["commit", "-q", "-m", "init"]);
    write(root, "ignored/i.fk", "LOUD\n");
    write(root, "src/x.gen.fk", "LOUD\n");
    write(root, "nested/inner.fk", "LOUD\n");
    git(&root.join("nested"), &["init", "-q"]);
    let (s, _) = scanner(root, "exclude: [\"legacy/**\"]");
    let o = opts(SourceSpec::Auto, None);

    // `.` and `..` are resolved; escaping the root is an error.
    let r = s.analyze_paths(&["./src/b/../a/E.fk".into()], &o).unwrap();
    let paths: Vec<&str> = r.findings.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["src/a/E.fk"]);
    let outside = std::env::temp_dir()
        .join("x.fk")
        .to_string_lossy()
        .into_owned();
    for bad in ["../x.fk", "src/../../x.fk", outside.as_str()] {
        match s.analyze_paths(&[bad.into()], &o) {
            Err(EngineError::Invalid(m)) => assert!(m.contains("outside"), "{m}"),
            other => panic!("{bad}: {other:?}"),
        }
    }

    // Exactly the files (and skip reasons) a full scan has.
    let asked: Vec<String> = [
        "ignored/i.fk",
        "src/x.gen.fk",
        "src/tracked.gen.fk",
        "legacy/l.fk",
        "nested/inner.fk",
        "src/a/E.fk",
    ]
    .map(String::from)
    .to_vec();
    let r = s.analyze_paths(&asked, &o).unwrap();
    let full = s.scan(&o).unwrap();
    let summary = |r: &ScanResult| {
        r.files
            .iter()
            .filter(|f| asked.contains(&f.path))
            .map(|f| (f.path.clone(), f.skip_reason))
            .collect::<Vec<_>>()
    };
    assert_eq!(summary(&r), summary(&full));
    assert_eq!(
        summary(&r).iter().map(|x| x.0.as_str()).collect::<Vec<_>>(),
        ["legacy/l.fk", "src/a/E.fk", "src/tracked.gen.fk"]
    );
    for skipped in ["ignored/i.fk", "src/x.gen.fk", "nested/inner.fk"] {
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.message.starts_with(skipped) && d.message.contains("not analyzed")),
            "{skipped}: {:?}",
            r.diagnostics
        );
    }

    // Case-insensitive file systems: the on-disk spelling is used.
    if mote_engine::source::case_insensitive_fs() {
        let r = s.analyze_paths(&["SRC/A/e.FK".into()], &o).unwrap();
        let paths: Vec<&str> = r.findings.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/a/E.fk"]);
    }

    // rescan: an ignored new file is not added; outside paths are errors.
    write(root, "ignored/new.fk", "LOUD\n");
    let r2 = s.rescan(&full, &["ignored/new.fk".into()], &o).unwrap();
    assert!(r2.files.iter().all(|f| f.path != "ignored/new.fk"));
    assert!(s.rescan(&full, &["../x.fk".into()], &o).is_err());
    if mote_engine::source::case_insensitive_fs() {
        // a deleted file named with another case is removed from the result
        std::fs::remove_file(root.join("src/a/E.fk")).unwrap();
        let r3 = s.rescan(&full, &["SRC/A/E.FK".into()], &o).unwrap();
        assert!(r3.files.iter().all(|f| f.path != "src/a/E.fk"));
    }
}

#[test]
fn moved_file_is_compared_with_its_base_at_its_base_path() {
    // A file moved into another directory with its package line updated was analyzed in the base
    // at its *new* path: a path-dependent rule (`java/package-directory-mismatch`) fired on the
    // base version only, and the move reported a spurious fixed finding.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a/x.fk", "PKG a\nfn f\n  LOUD\n  p;\nend\n");
    write(root, "a/y.fk", "PKG a\nMAGIC\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "feature"]);
    std::fs::create_dir_all(root.join("b")).unwrap();
    git(root, &["mv", "a/x.fk", "b/x.fk"]);
    write(root, "b/x.fk", "PKG b\nfn f\n  LOUD\n  p;\nend\n");
    // Moved without updating its package line: the finding is new.
    git(root, &["mv", "a/y.fk", "b/y.fk"]);
    commit_all(root, "feature");

    let (s, _) = scanner(root, "");
    let r = s.scan(&with_base("main")).unwrap();
    let nc = r.new_code.as_ref().unwrap();
    assert_eq!(nc.renamed.get("b/x.fk").map(String::as_str), Some("a/x.fk"));
    let st = |path: &str, rule: &str| {
        find(&r, path, rule)
            .iter()
            .map(|f| (f.range.start_line, f.status))
            .collect::<Vec<_>>()
    };
    assert_eq!(st("b/x.fk", "fk/loud"), vec![(3, FindingStatus::Unchanged)]);
    assert!(st("b/x.fk", "fk/magic").is_empty());
    assert_eq!(nc.measures.fixed_findings.total(), 0, "{:?}", nc.measures);
    // `b/y.fk` moved unchanged (same blob): its findings are unchanged without a base analysis,
    // whatever the path-dependent rule says about its new place.
    assert_eq!(nc.measures.new_findings.total(), 0, "{:?}", nc.measures);
}
