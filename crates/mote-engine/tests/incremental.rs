//! `Scanner::rescan`: re-analyzing a few files against a previous scan gives the same result as a
//! full scan, duplication across files included.

mod common;

use common::{commit_all, dup_body, init_repo, opts, scanner, write};
use mote_engine::result::{FileStatus, ScanResult};
use mote_engine::scan::SourceSpec;
use std::sync::atomic::Ordering;

/// Everything about a result that does not depend on the cache state: all of it but `stats`,
/// and without the analyzed/cached distinction of files.
fn comparable(r: &ScanResult) -> String {
    let mut v = serde_json::to_value(r).unwrap();
    let o = v.as_object_mut().unwrap();
    o.remove("stats");
    for f in o["files"].as_array_mut().unwrap() {
        if f["status"] == "cached" {
            f["status"] = "analyzed".into();
        }
    }
    serde_json::to_string_pretty(&v).unwrap()
}

fn dups(r: &ScanResult, path: &str) -> usize {
    r.findings
        .iter()
        .filter(|f| f.path == path && f.rule_id == "fk/duplicated-block")
        .count()
}

#[test]
fn rescan_equals_a_full_scan() {
    let dir = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a.fk", &format!("fn a\n{}end\n", dup_body("a", 15)));
    write(
        root,
        "b.fk",
        &format!("LOUD\nfn b\n{}end\n", dup_body("a", 15)),
    );
    write(root, "c.fk", "MAGIC\n");
    for i in 0..20 {
        write(
            root,
            &format!("m/f{i}.fk"),
            &format!("fn f{i} cog={}\nend\n", 10 + i),
        );
    }
    commit_all(root, "init");
    let (s, calls) = scanner(root, "");
    let o = opts(SourceSpec::Auto, Some(cache.path()));
    let r1 = s.scan(&o).unwrap();
    assert_eq!(dups(&r1, "a.fk"), 1);
    assert_eq!(dups(&r1, "b.fk"), 1);
    assert!(
        r1.stats
            .cache_path
            .as_deref()
            .is_some_and(|p| !p.contains('\\')),
        "{:?}",
        r1.stats.cache_path
    );

    // Remove the copy in b.fk: a.fk's duplicated-block finding goes away too.
    write(root, "b.fk", "LOUD\nfn b\n  small = 1;\nend\n");
    let before = calls.load(Ordering::SeqCst);
    let r2 = s.rescan(&r1, &["b.fk".into()], &o).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst) - before,
        1,
        "only the changed file is analyzed"
    );
    assert_eq!(r2.stats.files_analyzed, 1);
    assert_eq!(dups(&r2, "a.fk"), 0);
    assert_eq!(comparable(&r2), comparable(&s.scan(&o).unwrap()));

    // A new file duplicating a.fk, and a deleted one; rescans chain.
    write(root, "d.fk", &format!("fn d\n{}end\n", dup_body("a", 15)));
    std::fs::remove_file(root.join("c.fk")).unwrap();
    let abs_d = root.join("d.fk").to_string_lossy().into_owned();
    let r3 = s.rescan(&r2, &[abs_d, "c.fk".into()], &o).unwrap();
    assert_eq!(dups(&r3, "a.fk"), 1);
    assert_eq!(dups(&r3, "d.fk"), 1);
    assert!(r3.files.iter().all(|f| f.path != "c.fk"));
    assert_eq!(
        r3.files.iter().find(|f| f.path == "a.fk").unwrap().status,
        FileStatus::Cached
    );
    assert_eq!(comparable(&r3), comparable(&s.scan(&o).unwrap()));

    // Paths outside the root are rejected, also when `..` climbs out below it.
    for p in ["../elsewhere.fk", "m/../../elsewhere.fk"] {
        match s.rescan(&r3, &[p.into()], &o) {
            Err(mote_engine::EngineError::Invalid(m)) => {
                assert!(m.contains("outside the scanned root"), "{m}")
            }
            other => panic!("{p}: {other:?}"),
        }
    }

    // Without the cache the other files are read again; the result is the same.
    let no_cache = opts(SourceSpec::Auto, None);
    let r5 = s.rescan(&r3, &["d.fk".into()], &no_cache).unwrap();
    assert_eq!(r5.findings.len(), r3.findings.len());

    // A result from another scanner, or read back from JSON, cannot be rescanned.
    let (other, _) = scanner(root, "");
    assert!(other.rescan(&r3, &[], &no_cache).is_err());
    let back: ScanResult = serde_json::from_str(&serde_json::to_string(&r3).unwrap()).unwrap();
    assert!(back.retained.is_none());
    assert!(s.rescan(&back, &[], &o).is_err());
}

#[test]
fn rescan_keeps_path_filters() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "src/a.fk", "LOUD\n");
    write(root, "lib/b.fk", "LOUD\n");
    let (s, _) = scanner(root, "");
    let o = mote_engine::ScanOptions {
        paths: vec!["src".into()],
        ..opts(SourceSpec::Directory, None)
    };
    let r1 = s.scan(&o).unwrap();
    write(root, "src/new.fk", "LOUD\n");
    write(root, "lib/new.fk", "LOUD\n");
    let r2 = s
        .rescan(&r1, &["src/new.fk".into(), "lib/new.fk".into()], &o)
        .unwrap();
    let paths: Vec<&str> = r2.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, vec!["src/a.fk", "src/new.fk"]);
}
