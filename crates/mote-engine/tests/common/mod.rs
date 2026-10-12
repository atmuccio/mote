//! Helpers for engine integration tests: a fake plugin and throwaway git repositories.
#![allow(dead_code)]

pub mod fake_plugin;

use fake_plugin::FakePlugin;
use mote_engine::scan::{ScanOptions, Scanner, SourceSpec};
use mote_engine::{Config, Registry};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Once};

static ISOLATE: Once = Once::new();

/// Make every git process in this test binary ignore the machine's system/global config
/// (Git for Windows ships `core.autocrlf=true` system-wide).
pub fn isolate_git() {
    ISOLATE.call_once(|| {
        let home = std::env::temp_dir().join(format!("mote-test-home-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&home);
        // Tests never read these concurrently with writes: this runs once, before any git call.
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_CONFIG_HOME", &home);
        std::env::set_var("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z");
        std::env::set_var("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z");
        std::env::remove_var("MOTE_CACHE_DIR");
    });
}

/// Registry with one fake plugin (`fk`, extension `.fk`); returns the plugin's call counter.
pub fn registry() -> (Arc<Registry>, Arc<AtomicUsize>) {
    let p = FakePlugin::new("fk");
    let calls = p.calls.clone();
    (
        Arc::new(Registry::new(vec![Box::new(p)]).expect("registry")),
        calls,
    )
}

pub fn scanner(root: &Path, yaml: &str) -> (Scanner, Arc<AtomicUsize>) {
    let (reg, calls) = registry();
    let config = Config::from_yaml_str(yaml).expect("config");
    (Scanner::new(reg, config, root).expect("scanner"), calls)
}

pub fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, text).unwrap();
}

pub fn git(root: &Path, args: &[&str]) -> String {
    isolate_git();
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `git init` with deterministic, local-only settings.
pub fn init_repo(root: &Path) {
    isolate_git();
    git(root, &["init", "-q"]);
    for (k, v) in [
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
        ("core.autocrlf", "false"),
        ("core.safecrlf", "false"),
    ] {
        git(root, &["config", k, v]);
    }
    git(root, &["checkout", "-q", "-b", "main"]);
}

pub fn commit_all(root: &Path, msg: &str) -> String {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", msg]);
    git(root, &["rev-parse", "HEAD"])
}

/// Options with the cache in `dir` (or disabled when `None`).
pub fn opts(source: SourceSpec, cache: Option<&Path>) -> ScanOptions {
    ScanOptions {
        source,
        cache: Some(cache.is_some()),
        cache_dir: cache.map(PathBuf::from),
        ..Default::default()
    }
}

/// A block of `n` lines that the lexer turns into ~20 tokens each (for duplication tests).
pub fn dup_body(seed: &str, n: usize) -> String {
    (0..n)
        .map(|i| format!("  total_{seed} = compute(value{i}, \"text\", {i}) + other.call(x, y);\n"))
        .collect()
}
