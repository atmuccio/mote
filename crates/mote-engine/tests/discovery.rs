//! The in-process worktree discovery (index + walk) against the git command line.

mod common;

use common::{commit_all, git, init_repo, write};
use mote_engine::git::Git;
use mote_engine::source::{self, Discovery, Source, WorktreeSource};
use mote_engine::worktree::{self, Located};
use std::path::Path;
use std::time::{Duration, SystemTime};

fn fast(root: &Path) -> Discovery {
    match worktree::locate(root) {
        Located::Repo(layout) => source::discover_worktree_fast(root, &layout).unwrap(),
        other => panic!("fast path unavailable: {other:?}"),
    }
}

fn cli(root: &Path) -> Discovery {
    WorktreeSource {
        git: Git::open(root).unwrap(),
    }
    .discover()
    .unwrap()
}

/// Backdate every file below `root` (outside `.git`) so index entries are not racily clean.
fn backdate(root: &Path) {
    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    for e in walk(root) {
        let f = std::fs::File::options().write(true).open(&e).unwrap();
        f.set_modified(old).unwrap();
    }
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if e.file_name() == ".git" {
            continue;
        }
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

/// Compare both discoveries. The fast path may only be more conservative (no blob id where git
/// has one); returns the paths where it is.
fn compare(root: &Path) -> Vec<String> {
    let (f, c) = (fast(root), cli(root));
    let fp: Vec<&str> = f.entries.iter().map(|e| e.path.as_str()).collect();
    let cp: Vec<&str> = c.entries.iter().map(|e| e.path.as_str()).collect();
    assert_eq!(fp, cp, "entries");
    assert_eq!(f.untracked, c.untracked, "untracked");
    assert_eq!(f.warnings, c.warnings, "warnings");
    assert_eq!(f.info, c.info, "source info");
    assert_eq!(
        f.git.as_ref().map(|g| g.prefix().to_string()),
        c.git.as_ref().map(|g| g.prefix().to_string())
    );
    let mut conservative = Vec::new();
    for (a, b) in f.entries.iter().zip(&c.entries) {
        match (&a.blob_id, &b.blob_id) {
            (Some(x), Some(y)) => assert_eq!(x, y, "{}", a.path),
            (Some(_), None) => panic!("{}: fast path claims clean, git says dirty", a.path),
            (None, Some(_)) => conservative.push(a.path.clone()),
            (None, None) => {}
        }
    }
    conservative
}

#[test]
fn fast_discovery_matches_git() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "a.fk", "LOUD\n");
    write(root, "sub/b.fk", "x;\n");
    write(root, "sub/deep/c.fk", "MAGIC\n");
    write(root, "sp ace/ü.fk", "LOUD\n");
    write(root, "gone.fk", "LOUD\n");
    write(root, "same.fk", "aaaa\n");
    write(root, ".gitignore", "*.log\nlogs/\nbuild\n");
    write(root, "sub/.gitignore", "*.gen.fk\n!keep.gen.fk\n");
    write(root, "logs/tracked.fk", "LOUD\n");
    backdate(root);
    git(root, &["add", "-A"]);
    git(root, &["add", "-f", "logs/tracked.fk"]);
    git(root, &["commit", "-q", "-m", "init"]);
    assert!(compare(root).is_empty(), "clean files keep their blob ids");

    // modified (content and size), same size but new mtime, deleted, untracked, ignored,
    // negated ignore, untracked directory, intent-to-add, nested repository
    write(root, "a.fk", "LOUD\nLOUD\n");
    write(root, "same.fk", "bbbb\n");
    std::fs::remove_file(root.join("gone.fk")).unwrap();
    write(root, "new.fk", "HIGHBUG\n");
    write(root, "x.log", "log\n");
    write(root, "logs/ignored.fk", "LOUD\n");
    write(root, "build/out.fk", "LOUD\n");
    write(root, "sub/drop.gen.fk", "LOUD\n");
    write(root, "sub/keep.gen.fk", "LOUD\n");
    write(root, "newdir/deeper/n.fk", "LOUD\n");
    write(root, "ita.fk", "LOUD\n");
    git(root, &["add", "-N", "ita.fk"]);
    write(root, "nested/inner/x.fk", "LOUD\n");
    git(&root.join("nested"), &["init", "-q"]);
    let conservative = compare(root);
    assert!(conservative.is_empty(), "{conservative:?}");
    let f = fast(root);
    assert!(f.untracked.contains(&"sub/keep.gen.fk".to_string()));
    assert!(f.untracked.contains(&"nested/".to_string()));
    assert!(!f.untracked.iter().any(|p| p.ends_with(".log")));

    // a scan root below the top level
    let sub = root.join("sub");
    assert!(compare(&sub).is_empty());
    assert_eq!(fast(&sub).git.unwrap().prefix(), "sub/");

    // index version 4 (prefix-compressed paths)
    git(root, &["update-index", "--index-version", "4"]);
    assert!(compare(root).is_empty());
    git(root, &["update-index", "--index-version", "2"]);

    // skip-worktree entries are conservatively hashed (git trusts the index for them)
    git(root, &["update-index", "--skip-worktree", "sub/b.fk"]);
    assert_eq!(compare(root), vec!["sub/b.fk".to_string()]);
    git(root, &["update-index", "--no-skip-worktree", "sub/b.fk"]);
}

#[test]
fn fast_discovery_handles_conflicts_and_racy_entries() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    init_repo(root);
    write(root, "c.fk", "base\n");
    write(root, "other.fk", "x;\n");
    commit_all(root, "base");
    git(root, &["checkout", "-q", "-b", "side"]);
    write(root, "c.fk", "side\n");
    commit_all(root, "side");
    git(root, &["checkout", "-q", "main"]);
    write(root, "c.fk", "main\n");
    commit_all(root, "main");
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["merge", "-q", "side"])
        .output()
        .unwrap();
    assert!(!out.status.success(), "a conflict is expected");
    // Files written in the same second as the index are racily clean: the fast path hashes
    // them, it never trusts their stat data.
    for p in compare(root) {
        assert!(p == "other.fk", "{p}");
    }
    let f = fast(root);
    assert_eq!(
        f.warnings,
        vec!["c.fk: unmerged; analyzing the working-tree version".to_string()]
    );
}

#[test]
fn outside_a_repository_and_opt_out() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.fk", "LOUD\n");
    assert!(matches!(worktree::locate(dir.path()), Located::NotARepo));
    // A `.git` that is not a repository is left to the git command line.
    write(dir.path(), ".git", "not a gitdir file\n");
    assert!(matches!(
        worktree::locate(dir.path()),
        Located::Unsupported(_)
    ));
}

/// Create a directory link (a symlink, or on Windows a junction when symlinks need privileges).
fn dir_link(link: &Path, target: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
            || std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .is_ok_and(|o| o.status.success())
    }
}

#[test]
fn links_to_directories_are_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(outside.path(), "x.fk", "LOUD\n");
    write(outside.path(), "deep/y.fk", "LOUD\n");
    init_repo(root);
    write(root, "a.fk", "LOUD\n");
    commit_all(root, "init");
    if !dir_link(&root.join("linked"), outside.path()) {
        eprintln!("skipped: cannot create a directory link here");
        return;
    }
    // Both discoveries agree, and neither lists the link or anything below it.
    compare(root);
    for d in [fast(root), cli(root)] {
        let paths: Vec<&str> = d.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a.fk"]);
    }
    // Restricted discoveries (analyze_paths / rescan) agree too.
    let asked: Vec<String> = ["a.fk", "linked/x.fk", "linked/deep/y.fk"]
        .map(String::from)
        .to_vec();
    let layout = match worktree::locate(root) {
        Located::Repo(l) => l,
        other => panic!("{other:?}"),
    };
    let only = worktree::Only::new(&asked);
    let f = source::discover_worktree_fast_only(root, &layout, Some(&only)).unwrap();
    let c = WorktreeSource {
        git: Git::open(root).unwrap(),
    }
    .discover_paths(&asked)
    .unwrap();
    for d in [f, c] {
        let paths: Vec<&str> = d.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a.fk"]);
    }
}
