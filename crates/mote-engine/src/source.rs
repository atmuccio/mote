//! Where files come from: a git working tree, a plain directory, a git revision, or the index.
//!
//! A [`Source`] discovers [`SourceEntry`]s — repo-relative path, git blob id when it is known
//! without reading the file, size when cheap — plus a [`BlobLoader`] that reads contents on
//! demand. Knowing the blob id up front lets the engine hit the cache without touching the file.

use crate::git::{self, CatFile, Git, GitError};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Where a file's content lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryLocation {
    /// Read from `<root>/<path>`.
    File,
    /// Read the blob from the object database.
    Object,
}

/// One discovered file.
#[derive(Clone, Debug)]
pub struct SourceEntry {
    /// Repo-relative path with `/`.
    pub path: String,
    /// Git blob id when known without reading the content.
    pub blob_id: Option<String>,
    /// Size in bytes when known cheaply.
    pub size: Option<u64>,
    pub location: EntryLocation,
}

/// Reads file contents for entries of one discovery.
pub trait BlobLoader: Send + Sync {
    /// Load the content of `entry`.
    fn load(&self, entry: &SourceEntry) -> io::Result<Vec<u8>>;
    /// Load a blob by id (for base-revision analysis).
    fn load_blob(&self, blob_id: &str) -> io::Result<Option<Vec<u8>>>;
}

/// What was scanned.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceInfo {
    /// `worktree`, `directory`, `rev` or `staged`.
    pub kind: String,
    /// The requested revision, for `rev`.
    pub rev: Option<String>,
    /// The resolved head commit, when there is one.
    pub commit: Option<String>,
    /// Path of the scanned root below the repository top level, `/`-separated with a trailing
    /// `/` (`backend/`); empty at the top level and for plain directories. Prepend it to a
    /// finding's path for a repository-relative path.
    #[serde(default)]
    pub prefix: String,
}

/// Result of discovery.
pub struct Discovery {
    pub entries: Vec<SourceEntry>,
    pub loader: Arc<dyn BlobLoader>,
    pub info: SourceInfo,
    /// Git access, when the source is a repository.
    pub git: Option<Git>,
    /// Untracked files (worktree sources) — entirely new code.
    pub untracked: Vec<String>,
    pub warnings: Vec<String>,
}

/// A way of discovering files.
pub trait Source {
    fn discover(&self) -> Result<Discovery, GitError>;
}

/// Loader for files on disk and, lazily, git objects.
pub struct GitLoader {
    root: PathBuf,
    git: Option<Git>,
    cat: OnceLock<Result<Mutex<CatFile>, String>>,
}

impl GitLoader {
    /// Loader rooted at `root`, optionally able to read git objects.
    pub fn new(root: &Path, git: Option<Git>) -> Self {
        GitLoader {
            root: root.to_path_buf(),
            git,
            cat: OnceLock::new(),
        }
    }

    fn cat(&self) -> io::Result<&Mutex<CatFile>> {
        let r = self.cat.get_or_init(|| match &self.git {
            None => Err("not a git repository".to_string()),
            Some(g) => g.cat_file().map(Mutex::new).map_err(|e| e.to_string()),
        });
        r.as_ref().map_err(|e| io::Error::other(e.clone()))
    }
}

impl BlobLoader for GitLoader {
    fn load(&self, entry: &SourceEntry) -> io::Result<Vec<u8>> {
        match entry.location {
            EntryLocation::File => std::fs::read(self.root.join(&entry.path)),
            EntryLocation::Object => {
                let id = entry
                    .blob_id
                    .as_deref()
                    .ok_or_else(|| io::Error::other("no blob id"))?;
                self.load_blob(id)?.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, format!("missing blob {id}"))
                })
            }
        }
    }

    fn load_blob(&self, blob_id: &str) -> io::Result<Option<Vec<u8>>> {
        let cat = self.cat()?;
        let mut guard = cat.lock().unwrap_or_else(|p| p.into_inner());
        guard.read(blob_id)
    }
}

type GitJob<'a> = Box<dyn FnOnce() -> Result<Vec<u8>, GitError> + Send + 'a>;

fn run_parallel<T: Send>(jobs: Vec<Box<dyn FnOnce() -> T + Send + '_>>) -> Vec<T> {
    std::thread::scope(|s| {
        let handles: Vec<_> = jobs.into_iter().map(|j| s.spawn(j)).collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    })
}

/// `ls-files -s -z` record.
struct IndexEntry {
    mode: String,
    blob: String,
    stage: u8,
    path: String,
}

fn parse_ls_files_stage(out: &[u8]) -> Vec<IndexEntry> {
    git::split_nul(out)
        .filter_map(|rec| {
            let (meta, path) = rec.split_once('\t')?;
            let mut parts = meta.split(' ');
            let mode = parts.next()?.to_string();
            let blob = parts.next()?.to_string();
            let stage = parts.next()?.parse().ok()?;
            Some(IndexEntry {
                mode,
                blob,
                stage,
                path: path.to_string(),
            })
        })
        .collect()
}

fn is_regular_file_mode(mode: &str) -> bool {
    mode.starts_with("100")
}

/// Working tree of a git repository: tracked files (index blob ids for clean ones) plus
/// untracked, non-ignored files. Deleted tracked files are skipped, and so are untracked links
/// and everything below an untracked linked directory or junction (links are not followed; see
/// [`crate::worktree`]).
pub struct WorktreeSource {
    pub git: Git,
}

/// Whether `rel` (root-relative) is a link or lies below one. `cache` memoizes directories.
fn through_link(root: &Path, rel: &str, cache: &mut rustc_hash::FxHashMap<String, bool>) -> bool {
    let mut end = 0;
    loop {
        let next = rel[end..].find('/').map(|i| end + i);
        let part = &rel[..next.unwrap_or(rel.len())];
        let link = match cache.get(part) {
            Some(&l) => l,
            None => {
                let l = std::fs::symlink_metadata(root.join(part))
                    .is_ok_and(|m| m.file_type().is_symlink());
                if next.is_some() {
                    cache.insert(part.to_string(), l);
                }
                l
            }
        };
        if link {
            return true;
        }
        match next {
            Some(i) => end = i + 1,
            None => return false,
        }
    }
}

/// `:(literal)` pathspecs for `paths` (after `--`).
fn pathspecs(paths: &[String]) -> Vec<String> {
    paths.iter().map(|p| format!(":(literal){p}")).collect()
}

impl WorktreeSource {
    /// [`discover`](Source::discover) restricted to `paths` (root-relative files): the entries a
    /// full discovery would list among them.
    pub fn discover_paths(&self, paths: &[String]) -> Result<Discovery, GitError> {
        self.discover_impl(Some(paths))
    }

    fn discover_impl(&self, only: Option<&[String]>) -> Result<Discovery, GitError> {
        let g = &self.git;
        let specs = only.map(pathspecs).unwrap_or_default();
        let with_specs = |base: &[&'static str]| -> Vec<String> {
            let mut v: Vec<String> = base.iter().map(|s| s.to_string()).collect();
            if only.is_some() {
                v.push("--".into());
                v.extend(specs.iter().cloned());
            }
            v
        };
        let ls = with_specs(&["ls-files", "-s", "-z"]);
        let others = with_specs(&["ls-files", "-o", "--exclude-standard", "-z"]);
        let diff = with_specs(&[
            "diff-files",
            "--name-only",
            "-z",
            "--relative",
            "--ignore-submodules",
        ]);
        let run = |args: &Vec<String>| {
            let a: Vec<&str> = args.iter().map(String::as_str).collect();
            g.run(&a)
        };
        if only.is_some_and(|o| o.is_empty()) {
            return Ok(self.assemble(Vec::new(), Vec::new(), Vec::new()));
        }
        let jobs: Vec<GitJob<'_>> = vec![
            Box::new(|| run(&ls)),
            Box::new(|| run(&others)),
            Box::new(|| run(&diff)),
        ];
        let mut results = run_parallel(jobs).into_iter();
        let staged = results.next().unwrap_or_else(|| Ok(Vec::new()))?;
        let others = results.next().unwrap_or_else(|| Ok(Vec::new()))?;
        let dirty = results.next().unwrap_or_else(|| Ok(Vec::new()))?;
        let dirty: rustc_hash::FxHashSet<String> = git::split_nul(&dirty).collect();

        let mut warnings = Vec::new();
        let mut entries = Vec::new();
        let mut seen = rustc_hash::FxHashSet::default();
        for e in parse_ls_files_stage(&staged) {
            if !is_regular_file_mode(&e.mode) {
                continue;
            }
            if e.stage != 0 {
                if seen.insert(e.path.clone()) {
                    warnings.push(format!(
                        "{}: unmerged; analyzing the working-tree version",
                        e.path
                    ));
                    entries.push(SourceEntry {
                        path: e.path,
                        blob_id: None,
                        size: None,
                        location: EntryLocation::File,
                    });
                }
                continue;
            }
            if !seen.insert(e.path.clone()) {
                continue;
            }
            let blob_id = if dirty.contains(&e.path) {
                None
            } else {
                Some(e.blob)
            };
            entries.push(SourceEntry {
                path: e.path,
                blob_id,
                size: None,
                location: EntryLocation::File,
            });
        }
        let mut links = rustc_hash::FxHashMap::default();
        let untracked: Vec<String> = git::split_nul(&others)
            .filter(|p| !seen.contains(p))
            .filter(|p| !through_link(g.root(), p.trim_end_matches('/'), &mut links))
            .collect();
        Ok(self.assemble(entries, untracked, warnings))
    }

    fn assemble(
        &self,
        mut entries: Vec<SourceEntry>,
        untracked: Vec<String>,
        warnings: Vec<String>,
    ) -> Discovery {
        let g = &self.git;
        for p in &untracked {
            entries.push(SourceEntry {
                path: p.clone(),
                blob_id: None,
                size: None,
                location: EntryLocation::File,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let commit = g.resolve_commit("HEAD").ok();
        Discovery {
            entries,
            loader: Arc::new(GitLoader::new(g.root(), Some(g.clone()))),
            info: SourceInfo {
                kind: "worktree".into(),
                rev: None,
                commit,
                prefix: g.prefix().to_string(),
            },
            git: Some(g.clone()),
            untracked,
            warnings,
        }
    }
}

impl Source for WorktreeSource {
    fn discover(&self) -> Result<Discovery, GitError> {
        self.discover_impl(None)
    }
}

/// The working tree of the repository at `layout`, discovered without running git (see
/// [`crate::worktree`]). Same entries as [`WorktreeSource`], except that racily clean files get
/// no blob id (they are hashed instead).
pub fn discover_worktree_fast(
    root: &Path,
    layout: &crate::worktree::Layout,
) -> Result<Discovery, String> {
    discover_worktree_fast_only(root, layout, None)
}

/// [`discover_worktree_fast`] restricted to some files (see [`crate::worktree::Only`]).
pub fn discover_worktree_fast_only(
    root: &Path,
    layout: &crate::worktree::Layout,
    only: Option<&crate::worktree::Only>,
) -> Result<Discovery, String> {
    let d = crate::worktree::discover_only(root, layout, only)?;
    let g = layout.git(root);
    Ok(Discovery {
        entries: d.entries,
        loader: Arc::new(GitLoader::new(root, Some(g.clone()))),
        info: SourceInfo {
            kind: "worktree".into(),
            rev: None,
            commit: d.commit,
            prefix: layout.prefix.clone(),
        },
        git: Some(g),
        untracked: d.untracked,
        warnings: d.warnings,
    })
}

/// A plain directory (not a git repository): walks it respecting `.gitignore` files.
pub struct DirectorySource {
    pub root: PathBuf,
}

impl DirectorySource {
    /// [`discover`](Source::discover) restricted to `paths` (root-relative files): the entries a
    /// full walk would list among them, reading only their directories.
    pub fn discover_paths(&self, paths: &[String]) -> Result<Discovery, GitError> {
        self.discover_impl(Some(Arc::new(crate::worktree::Only::new(paths))))
    }

    fn discover_impl(
        &self,
        only: Option<Arc<crate::worktree::Only>>,
    ) -> Result<Discovery, GitError> {
        let root = self.root.clone();
        let found: Mutex<Vec<SourceEntry>> = Mutex::new(Vec::new());
        let filter_root = root.clone();
        let walker = ignore::WalkBuilder::new(&root)
            .hidden(false)
            .require_git(false)
            .filter_entry(move |e| {
                let name = e.file_name();
                let is_dir = e.file_type().is_some_and(|t| t.is_dir());
                if is_dir && (name == ".git" || name == ".mote") {
                    return false;
                }
                let Some(o) = &only else { return true };
                if e.depth() == 0 {
                    return true;
                }
                let Ok(rel) = e.path().strip_prefix(&filter_root) else {
                    return false;
                };
                let rel = rel.to_string_lossy().replace('\\', "/");
                if is_dir {
                    o.descends_dir(&rel)
                } else {
                    o.contains(&rel)
                }
            })
            .build_parallel();
        walker.run(|| {
            let root = &root;
            let found = &found;
            Box::new(move |res| {
                if let Ok(entry) = res {
                    if entry.file_type().is_some_and(|t| t.is_file()) {
                        if let Ok(rel) = entry.path().strip_prefix(root) {
                            let path = rel.to_string_lossy().replace('\\', "/");
                            let size = entry.metadata().ok().map(|m| m.len());
                            let e = SourceEntry {
                                path,
                                blob_id: None,
                                size,
                                location: EntryLocation::File,
                            };
                            found.lock().unwrap_or_else(|p| p.into_inner()).push(e);
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });
        let mut entries = found.into_inner().unwrap_or_else(|p| p.into_inner());
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(Discovery {
            entries,
            loader: Arc::new(GitLoader::new(&self.root, None)),
            info: SourceInfo {
                kind: "directory".into(),
                rev: None,
                commit: None,
                prefix: String::new(),
            },
            git: None,
            untracked: Vec::new(),
            warnings: Vec::new(),
        })
    }
}

impl Source for DirectorySource {
    fn discover(&self) -> Result<Discovery, GitError> {
        self.discover_impl(None)
    }
}

/// Parse `ls-tree -r -z -l --full-tree` output into entries (paths made relative to `g`'s root).
pub fn parse_ls_tree(g: &Git, out: &[u8]) -> Vec<SourceEntry> {
    let mut entries = Vec::new();
    for rec in git::split_nul(out) {
        let Some((meta, path)) = rec.split_once('\t') else {
            continue;
        };
        let mut parts = meta.split_whitespace();
        let (Some(mode), Some(kind), Some(blob), size) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        if kind != "blob" || !is_regular_file_mode(mode) {
            continue;
        }
        let Some(rel) = g.strip_prefix(path) else {
            continue;
        };
        entries.push(SourceEntry {
            path: rel.to_string(),
            blob_id: Some(blob.to_string()),
            size: size.and_then(|s| s.parse().ok()),
            location: EntryLocation::Object,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

/// A git revision read straight from the object database (no checkout).
pub struct RevSource {
    pub git: Git,
    pub rev: String,
}

impl Source for RevSource {
    fn discover(&self) -> Result<Discovery, GitError> {
        let commit = self.git.resolve_commit(&self.rev)?;
        let out = self
            .git
            .run(&["ls-tree", "-r", "-z", "-l", "--full-tree", &commit])?;
        let entries = parse_ls_tree(&self.git, &out);
        Ok(Discovery {
            entries,
            loader: Arc::new(GitLoader::new(self.git.root(), Some(self.git.clone()))),
            info: SourceInfo {
                kind: "rev".into(),
                rev: Some(self.rev.clone()),
                commit: Some(commit),
                prefix: self.git.prefix().to_string(),
            },
            git: Some(self.git.clone()),
            untracked: Vec::new(),
            warnings: Vec::new(),
        })
    }
}

/// The index (staged changes), read from the object database.
pub struct StagedSource {
    pub git: Git,
}

impl Source for StagedSource {
    fn discover(&self) -> Result<Discovery, GitError> {
        let out = self.git.run(&["ls-files", "-s", "-z"])?;
        let mut warnings = Vec::new();
        let mut entries = Vec::new();
        for e in parse_ls_files_stage(&out) {
            if !is_regular_file_mode(&e.mode) {
                continue;
            }
            if e.stage != 0 {
                if e.stage == 1 {
                    warnings.push(format!("{}: unmerged in the index; skipped", e.path));
                }
                continue;
            }
            entries.push(SourceEntry {
                path: e.path,
                blob_id: Some(e.blob),
                size: None,
                location: EntryLocation::Object,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let commit = self.git.resolve_commit("HEAD").ok();
        Ok(Discovery {
            entries,
            loader: Arc::new(GitLoader::new(self.git.root(), Some(self.git.clone()))),
            info: SourceInfo {
                kind: "staged".into(),
                rev: None,
                commit,
                prefix: self.git.prefix().to_string(),
            },
            git: Some(self.git.clone()),
            untracked: Vec::new(),
            warnings,
        })
    }
}

/// Whether paths compare case-insensitively on this platform's usual file systems.
const CASE_INSENSITIVE_FS: bool = cfg!(any(windows, target_os = "macos"));

/// Whether this platform's usual file systems are case-insensitive (Windows, macOS).
pub fn case_insensitive_fs() -> bool {
    CASE_INSENSITIVE_FS
}

fn eq_path_text(a: &str, b: &str) -> bool {
    if CASE_INSENSITIVE_FS {
        a.eq_ignore_ascii_case(b) || a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// `path` with `/` separators (on Windows `\` is a separator too).
fn slashes(path: &str) -> String {
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_string()
    }
}

/// Resolve `.` and `..` in a `/`-separated path without touching the file system. `None` if
/// `..` climbs above the start.
fn lexical_normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            c => out.push(c),
        }
    }
    Some(out.join("/"))
}

/// The absolute forms of `root` a user may write (as given made absolute, and canonical), with
/// `/` separators, no trailing `/` and no Windows `\\?\` prefix.
fn root_forms(root: &Path) -> Vec<String> {
    let mut forms = Vec::new();
    let mut push = |p: &Path| {
        let s = slashes(&p.to_string_lossy());
        let s = s
            .strip_prefix("//?/UNC/")
            .map(|r| format!("//{r}"))
            .or_else(|| s.strip_prefix("//?/").map(str::to_string))
            .unwrap_or(s);
        let (prefix, rest) = match s.strip_prefix('/') {
            Some(r) => ("/", r),
            None => ("", s.as_str()),
        };
        if let Some(n) = lexical_normalize(rest) {
            let f = format!("{prefix}{n}");
            if !forms.contains(&f) {
                forms.push(f);
            }
        }
    };
    if let Ok(abs) = std::path::absolute(root) {
        push(&abs);
    }
    if let Ok(c) = std::fs::canonicalize(root) {
        push(&c);
    }
    forms
}

/// The part of the absolute, `/`-separated `path` below one of `forms` (`""` for the root).
fn strip_root_text(forms: &[String], path: &str) -> Option<String> {
    forms.iter().find_map(|f| {
        let f = f.trim_end_matches('/');
        let head = path.get(..f.len())?;
        if !eq_path_text(head, f) {
            return None;
        }
        let rest = &path[f.len()..];
        if rest.is_empty() {
            Some(String::new())
        } else {
            rest.strip_prefix('/').map(str::to_string)
        }
    })
}

/// Make a path filter pattern (glob or prefix) repo-relative: an absolute path (or glob) under
/// `root` loses the root prefix; `\` becomes `/`. `Err` for an absolute pattern outside the root.
pub fn relative_pattern(root: &Path, pattern: &str) -> Result<String, String> {
    let p = pattern.trim().replace('\\', "/");
    let forms = root_forms(root);
    // A pattern written as an absolute path (possibly a glob below it).
    let (prefix, rest) = match p.strip_prefix('/') {
        Some(r) => ("/", r),
        None => ("", p.as_str()),
    };
    let normalized = lexical_normalize(rest).map(|n| format!("{prefix}{n}"));
    if let Some(rel) = normalized
        .as_deref()
        .and_then(|n| strip_root_text(&forms, n))
    {
        return Ok(if rel.is_empty() { ".".into() } else { rel });
    }
    // On Unix a leading `/` also anchors a pattern at the root (`/src/**`), so only a Windows
    // drive or UNC path is certainly outside; elsewhere a path that is not below the root is
    // kept and, matching nothing, reported as such.
    if cfg!(windows) && Path::new(&p).is_absolute() {
        return Err(format!(
            "path filter `{pattern}` is outside the scanned root {}",
            shown_root(root)
        ));
    }
    // Relative (or `/`-anchored at the root): keep.
    Ok(p)
}

/// Normalize a requested file path (absolute, or relative to `root`) to repo-relative `/` form:
/// `.` and `..` are resolved, and on case-insensitive file systems the case of every existing
/// component is taken from the disk (`SRC/a.java` → `src/a.java`). `Err` for a path outside
/// `root`. The root itself normalizes to `""`.
pub fn normalize_request(root: &Path, path: &str) -> Result<String, String> {
    let outside = || format!("`{path}` is outside the scanned root {}", shown_root(root));
    let s = slashes(path.trim());
    let p = Path::new(&s);
    let rel = if p.is_absolute() {
        let (prefix, rest) = match s.strip_prefix('/') {
            Some(r) => ("/", r),
            None => ("", s.as_str()),
        };
        let abs = lexical_normalize(rest)
            .map(|n| format!("{prefix}{n}"))
            .ok_or_else(outside)?;
        let forms = root_forms(root);
        match strip_root_text(&forms, &abs) {
            Some(r) => r,
            // Symlinked roots, 8.3 names: compare canonical forms (of the parent, for a path
            // that no longer exists).
            None => {
                let ap = Path::new(&abs);
                let canon = std::fs::canonicalize(ap).ok().or_else(|| {
                    let parent = std::fs::canonicalize(ap.parent()?).ok()?;
                    Some(parent.join(ap.file_name()?))
                });
                let canon = canon.ok_or_else(outside)?;
                let c = slashes(&canon.to_string_lossy());
                let c = c.strip_prefix("//?/").map(str::to_string).unwrap_or(c);
                strip_root_text(&forms, &c).ok_or_else(outside)?
            }
        }
    } else {
        s.trim_start_matches('/').to_string()
    };
    let rel = lexical_normalize(&rel).ok_or_else(outside)?;
    Ok(if CASE_INSENSITIVE_FS {
        disk_case(root, &rel)
    } else {
        rel
    })
}

/// The on-disk spelling of the existing components of `rel` (case-insensitive file systems).
fn disk_case(root: &Path, rel: &str) -> String {
    if rel.is_empty() {
        return String::new();
    }
    let mut dir = root.to_path_buf();
    let mut out: Vec<String> = Vec::new();
    let comps: Vec<&str> = rel.split('/').collect();
    for (k, c) in comps.iter().enumerate() {
        let found = std::fs::read_dir(&dir).ok().and_then(|rd| {
            let mut exact = None;
            let mut folded = Vec::new();
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name == *c {
                    exact = Some(name);
                    break;
                }
                if name.to_lowercase() == c.to_lowercase() {
                    folded.push(name);
                }
            }
            exact.or_else(|| (folded.len() == 1).then(|| folded.remove(0)))
        });
        match found {
            Some(name) => {
                dir.push(&name);
                out.push(name);
            }
            None => {
                out.extend(comps[k..].iter().map(|s| s.to_string()));
                break;
            }
        }
    }
    out.join("/")
}

/// Normalize a user-supplied path (absolute under `root`, or relative) to repo-relative `/` form.
/// `None` when it is outside `root` or names the root itself. See [`normalize_request`].
pub fn relativize(root: &Path, path: &str) -> Option<String> {
    normalize_request(root, path).ok().filter(|s| !s.is_empty())
}

/// `root` for messages: absolute, so `.` reads as a real directory.
fn shown_root(root: &Path) -> String {
    std::path::absolute(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .display()
        .to_string()
}
