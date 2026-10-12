//! Helpers: run the built `mote` binary and make throwaway git repositories.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub const BIN: &str = env!("CARGO_BIN_EXE_mote");

/// A home directory with no git config, so the machine's settings (e.g. Git for Windows'
/// system-wide `core.autocrlf`) don't leak into the tests.
fn isolated_home() -> PathBuf {
    let home = std::env::temp_dir().join(format!("mote-cli-test-home-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&home);
    home
}

/// Deterministic, local-only environment for git and mote child processes.
pub fn isolate(cmd: &mut Command) -> &mut Command {
    let home = isolated_home();
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", &home)
        .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
        .env_remove("MOTE_CACHE_DIR")
        .env_remove("MOTE_LOG")
        .env_remove("NO_COLOR")
        .stdin(Stdio::null())
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    fn from(o: Output) -> Run {
        Run {
            code: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }
}

impl std::fmt::Debug for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "exit {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code, self.stdout, self.stderr
        )
    }
}

/// Run `mote args...` in `dir`.
pub fn mote(dir: &Path, args: &[&str]) -> Run {
    let mut cmd = Command::new(BIN);
    isolate(&mut cmd).current_dir(dir).args(args);
    Run::from(cmd.output().expect("run mote"))
}

pub fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(p, text).unwrap();
}

pub fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

pub fn git(root: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    isolate(&mut cmd).arg("-C").arg(root).args(args);
    let out = cmd.output().expect("git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// `git init` with local-only settings on branch `main`.
pub fn init_repo(root: &Path) {
    git(root, &["init", "-q"]);
    git(root, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    write(root, ".gitignore", ".mote/\n");
    for (k, v) in [
        ("user.name", "Test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
        ("core.autocrlf", "false"),
    ] {
        git(root, &["config", k, v]);
    }
}

pub fn commit_all(root: &Path, msg: &str) {
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", msg]);
}
