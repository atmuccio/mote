//! In-process working-tree discovery: the fast path of
//! [`WorktreeSource`](crate::source::WorktreeSource).
//!
//! Instead of spawning `git rev-parse`, `git ls-files -s`, `git ls-files -o` and `git diff-files`
//! (tens of milliseconds each on Windows, hundreds for the untracked walk of a large tree), this
//! module locates the repository, parses the index (versions 2–4, as documented in git's
//! `Documentation/gitformat-index.txt`), resolves `HEAD` from the ref files, and walks the working
//! tree once, in parallel, collecting untracked files and the stat data of tracked ones.
//!
//! A tracked file whose size and modification time match its index entry — and whose entry is not
//! *racily clean* (modified in the same second the index was written, or later) — keeps the index
//! blob id, exactly like `git diff-files` would report it clean. Every other file gets
//! `blob_id: None` and is hashed from disk, which is always correct (at worst a cache miss).
//!
//! Anything unusual — a split or sparse index, SHA-256 or reftable repositories, `core.worktree`,
//! config includes, `GIT_DIR`-style environment overrides, a checksum mismatch — returns
//! `Err(reason)` and the caller falls back to the git command line. `MOTE_DISCOVERY=git`
//! forces the fallback.
//!
//! Known differences from `git ls-files -o --exclude-standard` (untracked files): ignore rules come
//! from the `ignore` crate (`.gitignore` files, `info/exclude`, and the global excludes file named
//! in `~/.gitconfig` / `$XDG_CONFIG_HOME/git/config` — not the system config), and untracked
//! nested repositories are skipped rather than listed as `dir/`.
//!
//! Links: untracked symbolic links and Windows junctions — to files or directories — are never
//! followed or listed (Git for Windows would list the files below a junction; the git-CLI path
//! drops them too, see [`WorktreeSource`](crate::source::WorktreeSource)), so a link cannot pull
//! code from outside the repository into a scan. Tracked links (mode `120000`) are never analyzed.

use crate::git::Git;
use crate::source::{EntryLocation, SourceEntry};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::Match;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

/// Environment variables that change where git looks for the repository or index.
const GIT_ENV_OVERRIDES: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_CEILING_DIRECTORIES",
];

/// Where a repository's files are.
#[derive(Clone, Debug)]
pub struct Layout {
    /// The working-tree top level (contains `.git`).
    pub toplevel: PathBuf,
    /// Per-worktree git directory (`index`, `HEAD`).
    pub git_dir: PathBuf,
    /// Shared git directory (`config`, `refs`, `packed-refs`).
    pub common_dir: PathBuf,
    /// The scan root below the top level, `/`-separated with a trailing `/` (empty at the top).
    pub prefix: String,
    /// `core.ignorecase`.
    pub ignore_case: bool,
}

impl Layout {
    /// A [`Git`] handle for this layout (no process is started).
    pub fn git(&self, root: &Path) -> Git {
        Git::from_parts(root, &self.prefix, &self.toplevel)
    }
}

/// Result of [`locate`].
#[derive(Debug)]
pub enum Located {
    /// A repository the fast path can read.
    Repo(Layout),
    /// No `.git` above the root: not a repository.
    NotARepo,
    /// A repository (or an environment) the fast path does not handle; use the git CLI.
    Unsupported(String),
}

/// Whether the fast path is disabled (`MOTE_DISCOVERY=git`).
pub fn disabled() -> bool {
    std::env::var("MOTE_DISCOVERY").is_ok_and(|v| v.eq_ignore_ascii_case("git"))
}

/// Find the repository containing `root` without running git.
pub fn locate(root: &Path) -> Located {
    if let Some(v) = GIT_ENV_OVERRIDES
        .iter()
        .find(|v| std::env::var_os(v).is_some_and(|x| !x.is_empty()))
    {
        return Located::Unsupported(format!("{v} is set"));
    }
    let abs = match std::fs::canonicalize(root) {
        Ok(p) => p,
        Err(e) => return Located::Unsupported(format!("cannot resolve {}: {e}", root.display())),
    };
    let mut dir: &Path = &abs;
    let (toplevel, dotgit) = loop {
        let candidate = dir.join(".git");
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => break (dir.to_path_buf(), candidate),
            Err(_) => match dir.parent() {
                Some(p) => dir = p,
                None => return Located::NotARepo,
            },
        }
    };
    let git_dir = if dotgit.is_dir() {
        dotgit
    } else {
        match read_gitfile(&dotgit) {
            Ok(p) => p,
            Err(e) => return Located::Unsupported(e),
        }
    };
    if !git_dir.join("HEAD").is_file() {
        return Located::Unsupported(format!("{} is not a git directory", git_dir.display()));
    }
    let common_dir = match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(s) => {
            let p = PathBuf::from(s.trim());
            if p.is_absolute() {
                p
            } else {
                git_dir.join(p)
            }
        }
        Err(_) => git_dir.clone(),
    };
    let cfg = match GitConfig::read(&common_dir.join("config")) {
        Ok(c) => c,
        Err(e) => return Located::Unsupported(e),
    };
    if let Err(e) = cfg.check_supported() {
        return Located::Unsupported(e);
    }
    let rel = abs.strip_prefix(&toplevel).unwrap_or(Path::new(""));
    let mut prefix = String::new();
    for c in rel.components() {
        prefix.push_str(&c.as_os_str().to_string_lossy());
        prefix.push('/');
    }
    Located::Repo(Layout {
        toplevel,
        git_dir,
        common_dir,
        prefix,
        ignore_case: cfg.ignore_case,
    })
}

/// Read a `.git` file (`gitdir: <path>`), as used by linked worktrees and submodules.
fn read_gitfile(path: &Path) -> Result<PathBuf, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let target = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("gitdir:"))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| format!("{} is not a gitdir file", path.display()))?;
    let p = PathBuf::from(target);
    Ok(if p.is_absolute() {
        p
    } else {
        path.parent().unwrap_or(Path::new(".")).join(p)
    })
}

/// The few repository config values the fast path cares about.
#[derive(Default, Debug)]
struct GitConfig {
    ignore_case: bool,
    bare: bool,
    worktree: bool,
    object_format: Option<String>,
    ref_storage: Option<String>,
    includes: bool,
    sparse_index: bool,
}

impl GitConfig {
    /// A minimal parser: sections, `key = value` lines, comments. Good enough for the keys we
    /// read; includes make the caller fall back to the CLI.
    fn read(path: &Path) -> Result<GitConfig, String> {
        let text = match std::fs::read(path) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        let mut cfg = GitConfig::default();
        let mut section = String::new();
        for raw in text.lines() {
            let mut line = raw.trim();
            if line.starts_with('[') {
                let Some(end) = line.find(']') else {
                    return Err(format!("{}: unparsable section `{line}`", path.display()));
                };
                let head = line[1..end].trim();
                // `[section "subsection"]`: keep the section name only.
                section = head
                    .split(|c: char| c.is_whitespace() || c == '"')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if section == "include" || section == "includeif" {
                    cfg.includes = true;
                }
                line = line[end + 1..].trim();
                if line.is_empty() {
                    continue;
                }
            }
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            let (key, value) = match line.split_once('=') {
                Some((k, v)) => (k.trim().to_ascii_lowercase(), unquote(v)),
                // A bare key means `true`.
                None => (line.to_ascii_lowercase(), "true".to_string()),
            };
            let truthy = || {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "true" | "yes" | "on" | "1"
                )
            };
            match (section.as_str(), key.as_str()) {
                ("core", "ignorecase") => cfg.ignore_case = truthy(),
                ("core", "bare") => cfg.bare = truthy(),
                ("core", "worktree") => cfg.worktree = true,
                ("extensions", "objectformat") => {
                    cfg.object_format = Some(value.to_ascii_lowercase())
                }
                ("extensions", "refstorage") => cfg.ref_storage = Some(value.to_ascii_lowercase()),
                ("index", "sparse") => cfg.sparse_index = truthy(),
                _ => {}
            }
        }
        Ok(cfg)
    }

    fn check_supported(&self) -> Result<(), String> {
        if self.includes {
            return Err("the repository config uses includes".into());
        }
        if self.bare {
            return Err("bare repository".into());
        }
        if self.worktree {
            return Err("core.worktree is set".into());
        }
        if self.object_format.as_deref().is_some_and(|f| f != "sha1") {
            return Err("not a SHA-1 repository".into());
        }
        if self.ref_storage.as_deref().is_some_and(|f| f != "files") {
            return Err("refs are not stored as files".into());
        }
        if self.sparse_index {
            return Err("sparse index".into());
        }
        Ok(())
    }
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    let v = v
        .split_once(" #")
        .or_else(|| v.split_once(" ;"))
        .map(|(a, _)| a.trim())
        .unwrap_or(v);
    v.trim_matches('"').to_string()
}

// ---------------------------------------------------------------------------------------------
// Index

/// One index entry (paths relative to the top level).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub path: String,
    pub oid: [u8; 20],
    pub mode: u32,
    pub stage: u8,
    pub mtime_sec: u32,
    pub mtime_nsec: u32,
    pub size: u32,
    pub assume_valid: bool,
    pub skip_worktree: bool,
    pub intent_to_add: bool,
}

impl IndexEntry {
    /// A regular file (`100644` / `100755`).
    pub fn is_regular(&self) -> bool {
        self.mode >> 12 == 0b1000
    }

    /// A submodule (`160000`).
    pub fn is_gitlink(&self) -> bool {
        self.mode >> 12 == 0b1110
    }

    /// The blob id as lowercase hex.
    pub fn oid_hex(&self) -> String {
        hex(&self.oid)
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(DIGITS[usize::from(b >> 4)] as char);
        s.push(DIGITS[usize::from(b & 15)] as char);
    }
    s
}

fn be32(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

fn be16(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([d[at], d[at + 1]])
}

/// Verify the trailing SHA-1 checksum (all zeros when `index.skipHash` is set).
pub fn verify_index_checksum(data: &[u8]) -> Result<(), String> {
    if data.len() < 12 + 20 {
        return Err("index too short".into());
    }
    let (body, sum) = data.split_at(data.len() - 20);
    if sum.iter().all(|&b| b == 0) {
        return Ok(());
    }
    let mut h = sha1_smol::Sha1::new();
    h.update(body);
    if h.digest().bytes() == sum {
        Ok(())
    } else {
        Err("index checksum mismatch".into())
    }
}

/// Parse an index file (versions 2, 3, 4). Does not verify the checksum (see
/// [`verify_index_checksum`]). Required extensions (split index, sparse directories) are errors.
pub fn parse_index(data: &[u8]) -> Result<Vec<IndexEntry>, String> {
    const HASH: usize = 20;
    if data.len() < 12 + HASH || &data[..4] != b"DIRC" {
        return Err("not an index file".into());
    }
    let version = be32(data, 4);
    if !(2..=4).contains(&version) {
        return Err(format!("unsupported index version {version}"));
    }
    let count = be32(data, 8) as usize;
    let end = data.len() - HASH;
    let mut entries = Vec::with_capacity(count.min(end / 62));
    let mut pos = 12;
    let mut prev_path: Vec<u8> = Vec::new();
    let bad = |what: &str| Err(format!("corrupt index ({what})"));
    for _ in 0..count {
        // ctime(8) mtime(8) dev ino mode uid gid size (4 each) = 40, then the object id, flags.
        if pos + 40 + HASH + 2 > end {
            return bad("truncated entry");
        }
        let mtime_sec = be32(data, pos + 8);
        let mtime_nsec = be32(data, pos + 12);
        let mode = be32(data, pos + 24);
        let size = be32(data, pos + 36);
        let mut oid = [0u8; 20];
        oid.copy_from_slice(&data[pos + 40..pos + 40 + HASH]);
        let flags = be16(data, pos + 40 + HASH);
        let mut p = pos + 40 + HASH + 2;
        let (mut skip_worktree, mut intent_to_add) = (false, false);
        if flags & 0x4000 != 0 {
            if version < 3 {
                return bad("extended flags in a version 2 index");
            }
            if p + 2 > end {
                return bad("truncated extended flags");
            }
            let ext = be16(data, p);
            skip_worktree = ext & 0x4000 != 0;
            intent_to_add = ext & 0x2000 != 0;
            p += 2;
        }
        let stage = ((flags >> 12) & 3) as u8;
        let path: Vec<u8> = if version == 4 {
            // Prefix compression: strip N bytes from the previous path, append the NUL-terminated
            // suffix. N uses the offset encoding of OFS_DELTA pack entries.
            let mut n: usize = 0;
            let mut first = true;
            loop {
                if p >= end {
                    return bad("truncated path prefix");
                }
                let c = data[p];
                p += 1;
                if first {
                    n = usize::from(c & 0x7f);
                    first = false;
                } else {
                    n = n
                        .checked_add(1)
                        .and_then(|n| n.checked_mul(128))
                        .and_then(|n| n.checked_add(usize::from(c & 0x7f)))
                        .ok_or("corrupt index (path prefix overflow)")?;
                }
                if c & 0x80 == 0 {
                    break;
                }
            }
            if n > prev_path.len() {
                return bad("path prefix longer than the previous path");
            }
            let Some(nul) = data[p..end].iter().position(|&b| b == 0) else {
                return bad("unterminated path");
            };
            let mut path = prev_path[..prev_path.len() - n].to_vec();
            path.extend_from_slice(&data[p..p + nul]);
            pos = p + nul + 1;
            path
        } else {
            let Some(nul) = data[p..end].iter().position(|&b| b == 0) else {
                return bad("unterminated path");
            };
            let name_len = usize::from(flags & 0xfff);
            if name_len < 0xfff && name_len != nul {
                return bad("path length mismatch");
            }
            let path = data[p..p + nul].to_vec();
            // Entries are padded with 1–8 NULs to a multiple of 8 bytes.
            pos += ((p - pos) + nul + 8) & !7;
            path
        };
        if pos > end {
            return bad("entry past the end");
        }
        entries.push(IndexEntry {
            path: String::from_utf8_lossy(&path).into_owned(),
            oid,
            mode,
            stage,
            mtime_sec,
            mtime_nsec,
            size,
            assume_valid: flags & 0x8000 != 0,
            skip_worktree,
            intent_to_add,
        });
        prev_path = path;
    }
    // Extensions: optional ones start with an uppercase letter and can be ignored.
    while pos + 8 <= end {
        let sig = &data[pos..pos + 4];
        let len = be32(data, pos + 4) as usize;
        if !sig[0].is_ascii_uppercase() {
            return Err(format!(
                "index extension `{}` is not supported",
                String::from_utf8_lossy(sig)
            ));
        }
        pos = pos
            .checked_add(8 + len)
            .filter(|&p| p <= end)
            .ok_or("corrupt index (extension past the end)")?;
    }
    if pos != end {
        return bad("trailing bytes");
    }
    Ok(entries)
}

// ---------------------------------------------------------------------------------------------
// HEAD

/// Resolve `HEAD` to a commit id from the ref files. `Ok(None)` for an unborn branch.
pub fn read_head(layout: &Layout) -> Result<Option<String>, String> {
    let mut content = std::fs::read_to_string(layout.git_dir.join("HEAD"))
        .map_err(|e| format!("cannot read HEAD: {e}"))?;
    for _ in 0..5 {
        let t = content.trim();
        let Some(name) = t.strip_prefix("ref:") else {
            return if is_hex_oid(t) {
                Ok(Some(t.to_string()))
            } else {
                Err(format!("unexpected HEAD content `{t}`"))
            };
        };
        let name = name.trim();
        if name.contains("..") || name.starts_with('/') {
            return Err(format!("unexpected ref name `{name}`"));
        }
        // Per-worktree refs live in the worktree's git dir, the rest in the common dir.
        let per_worktree = name.starts_with("refs/worktree/") || !name.starts_with("refs/");
        let base = if per_worktree {
            &layout.git_dir
        } else {
            &layout.common_dir
        };
        match std::fs::read_to_string(base.join(name)) {
            Ok(c) => content = c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return packed_ref(&layout.common_dir, name);
            }
            Err(e) => return Err(format!("cannot read {name}: {e}")),
        }
    }
    Err("symbolic ref loop".into())
}

fn packed_ref(common_dir: &Path, name: &str) -> Result<Option<String>, String> {
    let text = match std::fs::read_to_string(common_dir.join("packed-refs")) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read packed-refs: {e}")),
    };
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        if let Some((oid, r)) = line.split_once(' ') {
            if r.trim_end() == name {
                return if is_hex_oid(oid) {
                    Ok(Some(oid.to_string()))
                } else {
                    Err(format!("unexpected packed-refs line `{line}`"))
                };
            }
        }
    }
    Ok(None)
}

fn is_hex_oid(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

// ---------------------------------------------------------------------------------------------
// Working-tree walk

/// Size and modification time of a file.
type Stat = (u64, Option<(u64, u32)>);

fn mtime_of(m: &std::fs::Metadata) -> Option<(u64, u32)> {
    let d = m
        .modified()
        .ok()?
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?;
    Some((d.as_secs(), d.subsec_nanos()))
}

/// A `/`-separated relative path with native separators.
fn native(rel: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(rel.replace('/', "\\"))
    } else {
        PathBuf::from(rel)
    }
}

/// The `.gitignore` rules in effect in a directory: its own file and its parents'.
struct IgnoreChain {
    gitignore: Gitignore,
    /// Length of the directory's top-level-relative path (with `/`): candidates are matched
    /// relative to the directory.
    base: usize,
    parent: Option<Arc<IgnoreChain>>,
}

fn load_gitignore(dir: &Path, file: &Path, ignore_case: bool) -> Option<Gitignore> {
    let mut b = GitignoreBuilder::new(dir);
    let _ = b.case_insensitive(ignore_case);
    // Unreadable files and invalid lines are skipped, as git does.
    let _ = b.add(file);
    b.build().ok().filter(|g| !g.is_empty())
}

/// What the walk found.
#[derive(Default)]
struct WalkOut {
    /// (index into the tracked list, stat) of tracked files.
    stats: Vec<(usize, Option<Stat>)>,
    /// Root-relative paths of untracked, not ignored files.
    untracked: Vec<String>,
}

struct Walk<'a> {
    /// The scan root's path below the top level (`""` or `sub/dir/`).
    prefix: &'a str,
    /// Root-relative tracked path → index into the tracked list.
    tracked: &'a FxHashMap<&'a str, usize>,
    /// Lower-cased tracked paths, for `core.ignorecase` repositories (built on first use).
    tracked_lower: OnceLock<FxHashSet<String>>,
    /// Root-relative submodule directories (never descended into).
    gitlinks: &'a FxHashSet<&'a str>,
    /// `info/exclude` and the global excludes file (lowest precedence).
    base: Vec<Gitignore>,
    ignore_case: bool,
    /// Restrict the walk to these files (see [`Only`]).
    only: Option<&'a Only>,
    out: Mutex<Vec<WalkOut>>,
}

/// A restriction of discovery to some root-relative files: only their ancestor directories are
/// read, and only they can be listed.
#[derive(Debug, Default)]
pub struct Only {
    files: FxHashSet<String>,
    /// Root-relative ancestor directories, with a trailing `/`.
    dirs: FxHashSet<String>,
}

impl Only {
    /// Restrict to `files` (root-relative, `/`-separated).
    pub fn new<S: AsRef<str>>(files: &[S]) -> Only {
        let mut o = Only::default();
        for f in files {
            let f = f.as_ref();
            let mut end = 0;
            while let Some(i) = f[end..].find('/') {
                end += i + 1;
                o.dirs.insert(f[..end].to_string());
            }
            o.files.insert(f.to_string());
        }
        o
    }

    /// Whether `path` is one of the files.
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains(path)
    }

    fn descends(&self, dir_with_slash: &str) -> bool {
        self.dirs.contains(dir_with_slash)
    }

    /// Whether the root-relative directory `dir` (no trailing `/`) holds one of the files.
    pub fn descends_dir(&self, dir: &str) -> bool {
        self.dirs.contains(&format!("{dir}/"))
    }
}

impl Walk<'_> {
    /// Whether the top-level-relative `candidate` is ignored: the deepest `.gitignore` with a
    /// matching pattern decides, then `info/exclude`, then the global excludes file.
    fn ignored(&self, chain: Option<&IgnoreChain>, candidate: &str, is_dir: bool) -> bool {
        let mut c = chain;
        while let Some(link) = c {
            let rel = candidate.get(link.base..).unwrap_or(candidate);
            match link.gitignore.matched(Path::new(rel), is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => c = link.parent.as_deref(),
            }
        }
        for g in &self.base {
            match g.matched(Path::new(candidate), is_dir) {
                Match::Ignore(_) => return true,
                Match::Whitelist(_) => return false,
                Match::None => {}
            }
        }
        false
    }

    fn tracked_ignoring_case(&self, rel: &str) -> bool {
        self.ignore_case
            && self
                .tracked_lower
                .get_or_init(|| self.tracked.keys().map(|p| p.to_lowercase()).collect())
                .contains(&rel.to_lowercase())
    }

    /// List `dir` (top-level-relative `top_rel`, `""` or ending in `/`) and recurse into its
    /// subdirectories in parallel.
    fn visit(&self, dir: &Path, top_rel: &str, chain: Option<Arc<IgnoreChain>>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let entries: Vec<std::fs::DirEntry> = rd.flatten().collect();
        let is_root = top_rel.len() == self.prefix.len();
        // A nested repository: git lists it as `dir/` and does not descend into it.
        if !is_root && entries.iter().any(|e| e.file_name() == ".git") {
            let rel = &top_rel[self.prefix.len()..];
            if !self.tracked.contains_key(rel.trim_end_matches('/')) {
                self.out
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push(WalkOut {
                        stats: Vec::new(),
                        untracked: vec![rel.to_string()],
                    });
            }
            return;
        }
        let chain = match entries.iter().any(|e| e.file_name() == ".gitignore") {
            true => match load_gitignore(dir, &dir.join(".gitignore"), self.ignore_case) {
                Some(gitignore) => Some(Arc::new(IgnoreChain {
                    gitignore,
                    base: top_rel.len(),
                    parent: chain,
                })),
                None => chain,
            },
            false => chain,
        };
        let mut local = WalkOut::default();
        let mut subdirs: Vec<(PathBuf, String)> = Vec::new();
        let mut cand = String::with_capacity(top_rel.len() + 64);
        for e in &entries {
            let name = e.file_name();
            if name == ".git" {
                continue;
            }
            let Ok(ft) = e.file_type() else {
                continue;
            };
            cand.clear();
            cand.push_str(top_rel);
            cand.push_str(&name.to_string_lossy());
            let rel = &cand[self.prefix.len()..];
            if ft.is_dir() {
                if self.only.is_some_and(|o| !o.descends(&format!("{rel}/"))) {
                    continue;
                }
                if !self.gitlinks.contains(rel) && !self.ignored(chain.as_deref(), &cand, true) {
                    subdirs.push((e.path(), format!("{cand}/")));
                }
            } else if self.only.is_some_and(|o| !o.contains(rel)) {
                continue;
            } else if let Some(&i) = self.tracked.get(rel) {
                let stat = if ft.is_file() {
                    e.metadata().ok().map(|m| (m.len(), mtime_of(&m)))
                } else {
                    None
                };
                local.stats.push((i, stat));
            } else if ft.is_symlink() {
                // An untracked link (or junction): never followed, never listed.
            } else if !self.tracked_ignoring_case(rel)
                && !self.ignored(chain.as_deref(), &cand, false)
            {
                local.untracked.push(rel.to_string());
            }
        }
        if !local.stats.is_empty() || !local.untracked.is_empty() {
            self.out
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(local);
        }
        subdirs
            .into_par_iter()
            .for_each(|(path, rel)| self.visit(&path, &rel, chain.clone()));
    }
}

// ---------------------------------------------------------------------------------------------
// Discovery

/// Result of the fast worktree discovery.
#[derive(Debug)]
pub struct Discovered {
    /// Sorted by path.
    pub entries: Vec<SourceEntry>,
    pub untracked: Vec<String>,
    pub warnings: Vec<String>,
    pub commit: Option<String>,
}

/// Discover the working tree of `layout` below `root`.
pub fn discover(root: &Path, layout: &Layout) -> Result<Discovered, String> {
    discover_only(root, layout, None)
}

/// [`discover`], restricted to the files of `only` when given: exactly the entries a full
/// discovery would list among them (tracked, or untracked and not ignored, not below a nested
/// repository or a link), reading only their directories. Restricted entries carry no blob id
/// (the few files are hashed), so the index checksum is not verified for them.
pub fn discover_only(
    root: &Path,
    layout: &Layout,
    only: Option<&Only>,
) -> Result<Discovered, String> {
    let t0 = std::time::Instant::now();
    let index_path = layout.git_dir.join("index");
    let (index_data, index_mtime) = match std::fs::read(&index_path) {
        Ok(d) => {
            let m = std::fs::metadata(&index_path)
                .ok()
                .as_ref()
                .and_then(mtime_of)
                .ok_or("cannot read the index timestamp")?;
            (Some(d), m)
        }
        // A fresh repository without an index: everything is untracked.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, (0, 0)),
        Err(e) => return Err(format!("cannot read the index: {e}")),
    };
    let index: Vec<IndexEntry> = match &index_data {
        Some(d) => parse_index(d)?,
        None => Vec::new(),
    };
    // Entries below the scan root, with the prefix stripped.
    let prefix = layout.prefix.as_str();
    let mut gitlinks: FxHashSet<&str> = FxHashSet::default();
    let mut tracked: Vec<(&str, &IndexEntry)> = Vec::with_capacity(index.len());
    let mut tracked_idx: FxHashMap<&str, usize> = FxHashMap::default();
    tracked_idx.reserve(index.len());
    for e in &index {
        let Some(rel) = e.path.strip_prefix(prefix) else {
            continue;
        };
        if rel.is_empty() {
            continue;
        }
        if e.is_gitlink() {
            gitlinks.insert(rel);
        }
        if only.is_some_and(|o| !o.contains(rel)) {
            continue;
        }
        tracked_idx.entry(rel).or_insert(tracked.len());
        tracked.push((rel, e));
    }

    // Ignore rules above the scan root: info/exclude, the global excludes file, and the
    // `.gitignore` files of the directories between the top level and the root.
    let ic = layout.ignore_case;
    let mut base = Vec::new();
    base.extend(load_gitignore(
        &layout.toplevel,
        &layout.common_dir.join("info").join("exclude"),
        ic,
    ));
    let mut global = GitignoreBuilder::new(&layout.toplevel);
    let _ = global.case_insensitive(ic);
    let (global, _) = global.build_global();
    if !global.is_empty() {
        base.push(global);
    }
    let mut chain: Option<Arc<IgnoreChain>> = None;
    let mut parent_rel = String::new();
    for comp in prefix.split_terminator('/') {
        let dir = layout.toplevel.join(native(&parent_rel));
        if let Some(gitignore) = load_gitignore(&dir, &dir.join(".gitignore"), ic) {
            chain = Some(Arc::new(IgnoreChain {
                gitignore,
                base: parent_rel.len(),
                parent: chain,
            }));
        }
        parent_rel.push_str(comp);
        parent_rel.push('/');
    }
    let walk = Walk {
        prefix,
        tracked: &tracked_idx,
        tracked_lower: OnceLock::new(),
        gitlinks: &gitlinks,
        base,
        ignore_case: ic,
        only,
        out: Mutex::new(Vec::new()),
    };
    let t_index = t0.elapsed();

    // Checksum, HEAD and the walk run concurrently. A restricted discovery (a few files, for
    // editors) skips the checksum and trusts no index blob id: it hashes its files instead, so
    // a torn index read can only misclassify them, never attach a wrong blob id.
    let (checksum, head) = rayon::join(
        || {
            (
                match (only, index_data.as_deref()) {
                    (None, Some(d)) => verify_index_checksum(d),
                    _ => Ok(()),
                },
                read_head(layout),
            )
        },
        || walk.visit(root, prefix, chain),
    )
    .0;
    checksum?;
    let commit = head?;
    let t_walk = t0.elapsed();

    let mut stats: Vec<Option<Stat>> = vec![None; tracked.len()];
    let mut untracked: Vec<String> = Vec::new();
    for out in walk.out.into_inner().unwrap_or_else(|p| p.into_inner()) {
        for (i, s) in out.stats {
            stats[i] = s;
        }
        untracked.extend(out.untracked);
    }
    // Tracked files the walk did not see (ignored or inside nested repositories): stat them.
    let unseen: Vec<usize> = (0..tracked.len())
        .filter(|&i| stats[i].is_none() && tracked[i].1.is_regular() && tracked[i].1.stage == 0)
        .collect();
    let late: Vec<(usize, Option<Stat>)> = unseen
        .par_iter()
        .map(|&i| {
            let m = std::fs::symlink_metadata(root.join(native(tracked[i].0))).ok();
            (
                i,
                m.filter(|m| m.is_file()).map(|m| (m.len(), mtime_of(&m))),
            )
        })
        .collect();
    for (i, s) in late {
        stats[i] = s;
    }

    let mut warnings = Vec::new();
    let mut entries: Vec<SourceEntry> = Vec::with_capacity(tracked.len() + untracked.len());
    let mut last: Option<&str> = None;
    for (i, (rel, e)) in tracked.iter().enumerate() {
        if !e.is_regular() {
            continue;
        }
        // Stages of one path are adjacent in the index.
        if last == Some(*rel) {
            continue;
        }
        last = Some(rel);
        if e.stage != 0 {
            warnings.push(format!(
                "{rel}: unmerged; analyzing the working-tree version"
            ));
            entries.push(SourceEntry {
                path: (*rel).to_string(),
                blob_id: None,
                size: None,
                location: EntryLocation::File,
            });
            continue;
        }
        let clean = !e.intent_to_add
            && !e.skip_worktree
            && stats[i].is_some_and(|(size, mtime)| stat_matches(e, size, mtime, index_mtime));
        entries.push(SourceEntry {
            path: (*rel).to_string(),
            blob_id: (clean && only.is_none()).then(|| e.oid_hex()),
            size: None,
            location: EntryLocation::File,
        });
    }
    untracked.sort_unstable();
    untracked.dedup();
    if let Some(o) = only {
        // Nested repositories are reported as `dir/`: not one of the files.
        untracked.retain(|p| o.contains(p));
    }
    for p in &untracked {
        entries.push(SourceEntry {
            path: p.clone(),
            blob_id: None,
            size: None,
            location: EntryLocation::File,
        });
    }
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    tracing::debug!(
        "fast discovery: index {:?}, walk {:?}, total {:?} ({} entries)",
        t_index,
        t_walk - t_index,
        t0.elapsed(),
        entries.len(),
    );
    Ok(Discovered {
        entries,
        untracked,
        warnings,
        commit,
    })
}

/// Whether a file's stat data proves it unchanged since the index entry was written: same size
/// and modification time, and the entry is not racily clean (its mtime is not older than the
/// index file's, to the second).
fn stat_matches(
    e: &IndexEntry,
    size: u64,
    mtime: Option<(u64, u32)>,
    index_mtime: (u64, u32),
) -> bool {
    let Some((sec, nsec)) = mtime else {
        return false;
    };
    if u64::from(e.mtime_sec) >= index_mtime.0 {
        return false;
    }
    size as u32 == e.size
        && sec as u32 == e.mtime_sec
        && (e.mtime_nsec == 0 || nsec == e.mtime_nsec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parsing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config");
        std::fs::write(
            &p,
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\tignorecase = true # yes\n[remote \"origin\"]\n\turl = x\n",
        )
        .unwrap();
        let c = GitConfig::read(&p).unwrap();
        assert!(c.ignore_case);
        assert!(!c.bare);
        assert!(c.check_supported().is_ok());
        std::fs::write(&p, "[extensions]\n\tobjectFormat = sha256\n").unwrap();
        assert!(GitConfig::read(&p).unwrap().check_supported().is_err());
        std::fs::write(&p, "[includeIf \"gitdir:~/x\"]\n\tpath = y\n").unwrap();
        assert!(GitConfig::read(&p).unwrap().check_supported().is_err());
        std::fs::write(&p, "[core] worktree = ../elsewhere\n").unwrap();
        assert!(GitConfig::read(&p).unwrap().check_supported().is_err());
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        assert!(parse_index(b"").is_err());
        assert!(parse_index(b"DIRC\0\0\0\x02\0\0\0\x05").is_err());
        let mut v = b"DIRC\0\0\0\x02\0\0\0\x01".to_vec();
        v.extend_from_slice(&[0xff; 80]);
        assert!(parse_index(&v).is_err());
        for n in 0..200u8 {
            let mut v = b"DIRC\0\0\0\x04\0\0\0\x03".to_vec();
            v.extend((0..n).map(|i| i.wrapping_mul(37)));
            v.extend_from_slice(&[0; 20]);
            let _ = parse_index(&v);
        }
    }

    #[test]
    fn stat_rule() {
        let e = IndexEntry {
            path: "a".into(),
            oid: [0; 20],
            mode: 0o100644,
            stage: 0,
            mtime_sec: 100,
            mtime_nsec: 5,
            size: 10,
            assume_valid: false,
            skip_worktree: false,
            intent_to_add: false,
        };
        assert!(stat_matches(&e, 10, Some((100, 5)), (200, 0)));
        assert!(!stat_matches(&e, 11, Some((100, 5)), (200, 0)));
        assert!(!stat_matches(&e, 10, Some((100, 6)), (200, 0)));
        assert!(!stat_matches(&e, 10, Some((101, 5)), (200, 0)));
        // racily clean: written in the same second as the index
        assert!(!stat_matches(&e, 10, Some((100, 5)), (100, 900)));
        assert!(!stat_matches(&e, 10, None, (200, 0)));
    }
}
