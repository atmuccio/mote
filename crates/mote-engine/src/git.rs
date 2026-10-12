//! A thin wrapper over the `git` command line.
//!
//! Every call is one process per *scan* (never per file); blob contents come from a single
//! long-lived `git cat-file --batch` process. Paths are returned relative to the wrapper's root
//! with `/` separators.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Git failures.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git is not installed or not on PATH ({0})")]
    NotInstalled(io::Error),
    #[error("{0} is not inside a git repository")]
    NotARepo(PathBuf),
    #[error("`git {args}` failed: {stderr}")]
    Failed { args: String, stderr: String },
    #[error("unknown revision `{0}`")]
    UnknownRev(String),
    /// Two revisions share no history; the message says which.
    #[error("{0}")]
    NoMergeBase(String),
    #[error("git I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("unexpected git output: {0}")]
    Parse(String),
}

/// A git repository (or a subdirectory of one) on disk.
#[derive(Clone, Debug)]
pub struct Git {
    root: PathBuf,
    /// Path of `root` below the repository top level, with a trailing `/` (empty at top level).
    prefix: String,
    toplevel: PathBuf,
}

impl Git {
    /// Open the repository containing `root`.
    pub fn open(root: &Path) -> Result<Git, GitError> {
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["rev-parse", "--show-toplevel", "--show-prefix"])
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .env("LC_ALL", "C")
            .output()
            .map_err(GitError::NotInstalled)?;
        if !out.status.success() {
            return Err(GitError::NotARepo(root.to_path_buf()));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut lines = text.lines();
        let toplevel = PathBuf::from(lines.next().unwrap_or("").trim());
        let prefix = lines.next().unwrap_or("").trim().to_string();
        Ok(Git {
            root: root.to_path_buf(),
            prefix,
            toplevel,
        })
    }

    /// A handle for a repository located without running git (see [`crate::worktree::locate`]).
    pub(crate) fn from_parts(root: &Path, prefix: &str, toplevel: &Path) -> Git {
        Git {
            root: root.to_path_buf(),
            prefix: prefix.to_string(),
            toplevel: toplevel.to_path_buf(),
        }
    }

    /// The directory commands run in.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Repository top level.
    pub fn toplevel(&self) -> &Path {
        &self.toplevel
    }

    /// Path prefix of the root below the top level (`""` or `"sub/dir/"`).
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn command(&self) -> Command {
        let mut c = Command::new("git");
        c.arg("-C")
            .arg(&self.root)
            .args(["-c", "core.quotepath=off"])
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            // Porcelain commands (`git diff`) refresh and rewrite the index when they can take
            // its lock. A scanner must not modify the repository, and two concurrent diffs would
            // race on it (on Windows the loser fails to open the index while it is replaced).
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::null());
        c
    }

    /// Run `git <args>` and return stdout; non-zero exit is an error.
    ///
    /// A command that fails because another git process was replacing the index at that moment
    /// (porcelain commands such as `git diff` rewrite it after refreshing stat data; on Windows
    /// opening a file that is being replaced fails) is retried a few times.
    pub fn run(&self, args: &[&str]) -> Result<Vec<u8>, GitError> {
        let mut attempt = 0;
        loop {
            let out = self
                .command()
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()?;
            if out.status.success() {
                return Ok(out.stdout);
            }
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            attempt += 1;
            if attempt < 4 && is_transient_index_error(&stderr) {
                std::thread::sleep(std::time::Duration::from_millis(10 * attempt));
                continue;
            }
            return Err(GitError::Failed {
                args: args.join(" "),
                stderr,
            });
        }
    }

    /// Run and return stdout, or `None` on non-zero exit.
    pub fn run_opt(&self, args: &[&str]) -> Result<Option<Vec<u8>>, GitError> {
        let out = self
            .command()
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        Ok(out.status.success().then_some(out.stdout))
    }

    /// Resolve a revision to a full commit id.
    pub fn resolve_commit(&self, rev: &str) -> Result<String, GitError> {
        let spec = format!("{rev}^{{commit}}");
        match self.run_opt(&[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ])? {
            Some(out) => Ok(String::from_utf8_lossy(&out).trim().to_string()),
            None => Err(GitError::UnknownRev(rev.to_string())),
        }
    }

    /// `git merge-base a b`. Unrelated histories (git exits 1 without output) are
    /// [`GitError::NoMergeBase`].
    pub fn merge_base(&self, a: &str, b: &str) -> Result<String, GitError> {
        let out = self
            .command()
            .args(["merge-base", a, b])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !s.is_empty() {
            return Ok(s);
        }
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(GitError::NoMergeBase(format!(
                "no common ancestor between {a} and {b}"
            )))
        } else {
            Err(GitError::Failed {
                args: format!("merge-base {a} {b}"),
                stderr,
            })
        }
    }

    /// Read a file at a revision (`rev:path`, path relative to the root). `None` if absent.
    pub fn show_file(&self, rev: &str, path: &str) -> Result<Option<Vec<u8>>, GitError> {
        let spec = format!("{rev}:./{path}");
        self.run_opt(&["cat-file", "blob", &spec])
    }

    /// Start a `git cat-file --batch` process.
    pub fn cat_file(&self) -> Result<CatFile, GitError> {
        let mut child = self
            .command()
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| GitError::Parse("no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| GitError::Parse("no stdout".into()))?;
        Ok(CatFile {
            child,
            stdin: Some(stdin),
            stdout: BufReader::with_capacity(1 << 16, stdout),
        })
    }

    /// Strip this wrapper's prefix from a top-level-relative path; `None` if outside the root.
    pub fn strip_prefix<'p>(&self, path: &'p str) -> Option<&'p str> {
        if self.prefix.is_empty() {
            Some(path)
        } else {
            path.strip_prefix(self.prefix.as_str())
        }
    }
}

/// A long-lived `git cat-file --batch` process.
pub struct CatFile {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl CatFile {
    /// Read an object (`<sha>` or `<rev>:<path>`). Returns `None` if it doesn't exist.
    pub fn read(&mut self, object: &str) -> io::Result<Option<Vec<u8>>> {
        if object.contains('\n') {
            return Ok(None);
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::other("cat-file closed"))?;
        stdin.write_all(object.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        let mut header = String::new();
        if self.stdout.read_line(&mut header)? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "git cat-file exited",
            ));
        }
        let header = header.trim_end();
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            return Ok(None);
        }
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("bad cat-file header `{header}`"),
                )
            })?;
        let mut buf = vec![0u8; size + 1];
        self.stdout.read_exact(&mut buf)?;
        buf.truncate(size);
        Ok(Some(buf))
    }
}

impl Drop for CatFile {
    fn drop(&mut self) {
        // Closing stdin makes cat-file exit; reap it.
        self.stdin.take();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.wait();
        }
    }
}

/// A failure caused by a concurrent git process replacing `.git/index`.
fn is_transient_index_error(stderr: &str) -> bool {
    stderr.contains("index file open failed")
        || stderr.contains("index file smaller than expected")
        || stderr.contains("bad index file")
}

/// Git's blob id (`sha1("blob <len>\0" + content)`) as lowercase hex.
pub fn blob_id(content: &[u8]) -> String {
    let mut h = sha1_smol::Sha1::new();
    h.update(format!("blob {}\0", content.len()).as_bytes());
    h.update(content);
    h.digest().to_string()
}

/// Split NUL-terminated output into UTF-8 strings (lossy), skipping empty records.
pub fn split_nul(out: &[u8]) -> impl Iterator<Item = String> + '_ {
    out.split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
}

/// Result of cloning a remote repository.
#[derive(Clone, Debug)]
pub struct RemoteCheckout {
    /// Local clone directory (no working tree checked out).
    pub dir: PathBuf,
    /// Commit to scan (use it as a `rev` source).
    pub commit: String,
}

/// Shallow-clone `url` (optionally at branch/tag/commit `rev`) into `cache_dir/repos/<hash>`
/// without checking out a working tree, and return the commit to scan with a rev source. An
/// existing clone for the same URL and revision is refreshed with a shallow fetch.
///
/// This is the only engine function that makes git touch the network, and only when asked to.
pub fn clone_remote(
    url: &str,
    rev: Option<&str>,
    cache_dir: &Path,
) -> Result<RemoteCheckout, GitError> {
    let key = xxhash_rust::xxh3::xxh3_64(format!("{url}\0{}", rev.unwrap_or("")).as_bytes());
    let dir = cache_dir.join("repos").join(format!("{key:016x}"));
    let git_in = |args: &[&str], cwd: Option<&Path>| -> Result<Vec<u8>, GitError> {
        let mut c = Command::new("git");
        if let Some(cwd) = cwd {
            c.arg("-C").arg(cwd);
        }
        let out = c
            .args(args)
            .env("LC_ALL", "C")
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(GitError::NotInstalled)?;
        if !out.status.success() {
            return Err(GitError::Failed {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        Ok(out.stdout)
    };
    let fetch_ref = rev.unwrap_or("HEAD");
    if dir.join("HEAD").is_file() || dir.join(".git").exists() {
        git_in(
            &["fetch", "--depth", "1", "--no-tags", "origin", fetch_ref],
            Some(&dir),
        )?;
        let g = Git {
            root: dir.clone(),
            prefix: String::new(),
            toplevel: dir.clone(),
        };
        let commit = g.resolve_commit("FETCH_HEAD")?;
        return Ok(RemoteCheckout { dir, commit });
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let dir_s = dir.to_string_lossy().to_string();
    let mut args = vec!["clone", "--depth", "1", "--no-checkout", "--no-tags"];
    if let Some(r) = rev {
        args.extend(["--branch", r]);
    }
    args.extend(["--", url, &dir_s]);
    match git_in(&args, None) {
        Ok(_) => {}
        Err(e) if rev.is_some() => {
            // `--branch` accepts only branch and tag names; fall back to fetching a commit.
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir)?;
            git_in(&["init", "--quiet"], Some(&dir)).map_err(|_| e)?;
            git_in(&["remote", "add", "origin", url], Some(&dir))?;
            git_in(
                &["fetch", "--depth", "1", "--no-tags", "origin", fetch_ref],
                Some(&dir),
            )?;
            let g = Git {
                root: dir.clone(),
                prefix: String::new(),
                toplevel: dir.clone(),
            };
            let commit = g.resolve_commit("FETCH_HEAD")?;
            return Ok(RemoteCheckout { dir, commit });
        }
        Err(e) => return Err(e),
    }
    let g = Git {
        root: dir.clone(),
        prefix: String::new(),
        toplevel: dir.clone(),
    };
    let commit = g.resolve_commit("HEAD")?;
    Ok(RemoteCheckout { dir, commit })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_id_matches_git() {
        // `printf 'hello\n' | git hash-object --stdin`
        assert_eq!(
            blob_id(b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        assert_eq!(blob_id(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
    }

    #[test]
    fn nul_split() {
        let v: Vec<String> = split_nul(b"a\0b c\0\0").collect();
        assert_eq!(v, vec!["a", "b c"]);
    }
}
