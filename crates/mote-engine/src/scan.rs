//! The scanner: discovery → parallel map (cache-aware) → reduce (CPD, measures) → new code →
//! coverage → gate → [`ScanResult`].

use crate::analyze::{
    self, AnalysisSettings, FileJob, FileOutcome, FileResult, Pending, SkipReason,
};
use crate::baseline::{self, Baseline};
use crate::cache::{self, Cache, CacheKey, CacheReader, KeyParts};
use crate::config::{
    CompiledConfig, Config, ConfigError, PathFilter, ResolvedRules, CONFIG_FILE_NAMES,
    ENGINE_VERSION,
};
use crate::coverage::{self, RepoCoverage};
use crate::cpd;
use crate::gate;
use crate::generic::{GenericRule, MetaRule};
use crate::git::{self, Git, GitError};
use crate::java_release::{self, JavaRelease, JavaReleases};
use crate::measures::{
    finding_effort, percent, DirectoryRollup, EffortMinutes, Measures, MAX_DIRECTORIES,
    THRESHOLD_PARAMS,
};
use crate::newcode::{self, ChangeKind, ChangedFile, DiffInfo, HeadSpec};
use crate::packages::PackageNames;
use crate::registry::{Registry, RegistryError, RuleInfo};
use crate::result::{
    BaselineInfo, Diagnostic, DuplicationBlock, DuplicationOccurrence, FileStatus, FileSummary,
    LanguageStats, NewCodeMeasures, NewCodeResult, ProjectMeasures, ScanResult, ScanStats,
    SeverityCounts, Timings,
};
use crate::source::{
    self, BlobLoader, DirectorySource, Discovery, EntryLocation, GitLoader, RevSource, Source,
    SourceEntry, SourceInfo, StagedSource, WorktreeSource,
};
use crate::suppress;
use crate::triage::{self, TriageFile, TriageReport};
use crate::vendored;
use crate::worktree::{self, Located};
use mote_core::{
    ActiveRule, Finding, FindingStatus, Location, ParamValue, Quality, RawFinding, SuppressionInfo,
    TextRange,
};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Which files to scan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SourceSpec {
    /// The git working tree when `root` is in a repository, else the plain directory.
    #[default]
    Auto,
    /// The git working tree (error outside a repository).
    Worktree,
    /// A plain directory walk (respects `.gitignore` files; no git needed).
    Directory,
    /// A revision read from the object database, e.g. `HEAD~3` or `origin/main`.
    Rev(String),
    /// The index (staged changes).
    Staged,
}

/// Options for one scan.
#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    pub source: SourceSpec,
    /// Restrict to these paths: globs (containing `*?[{`) or path prefixes. Empty = everything.
    pub paths: Vec<String>,
    /// Compare with this base (branch, tag, commit) to classify findings as new/unchanged/worsened.
    pub base: Option<String>,
    /// Override `cache.enabled`.
    pub cache: Option<bool>,
    /// Override the cache directory (also `MOTE_CACHE_DIR`, then `cache.dir`).
    pub cache_dir: Option<PathBuf>,
    /// Worker threads (default: rayon's global pool, one per core).
    pub threads: Option<usize>,
    /// Coverage reports in addition to `coverage.reports`.
    pub coverage_reports: Vec<PathBuf>,
    /// Only analyze and report files changed relative to `base` (requires `base`). Duplication is
    /// then detected among the changed files only.
    pub changed_only: bool,
    /// Compare with a saved result instead of a git base (see [`baseline`](crate::baseline));
    /// conflicts with `base`.
    pub baseline: Option<Arc<Baseline>>,
    /// Triage file to use instead of `triage.file` (read from disk; relative paths are relative
    /// to the process's current directory).
    pub triage_file: Option<PathBuf>,
}

/// Errors that stop a scan. Per-file problems never do; they become findings or diagnostics.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("{0}")]
    Invalid(String),
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
}

/// Load the configuration for `root`: from the working tree, or from `rev` when scanning a
/// revision (so the config matches the scanned code). Returns the config and where it came from.
pub fn discover_config(
    root: &Path,
    rev: Option<&str>,
) -> Result<(Config, Option<String>), EngineError> {
    match rev {
        None => {
            let (c, p) = Config::discover(root)?;
            Ok((c, p.map(|p| p.to_string_lossy().into_owned())))
        }
        Some(rev) => {
            let g = Git::open(root)?;
            let commit = g.resolve_commit(rev)?;
            for name in CONFIG_FILE_NAMES {
                if let Some(bytes) = g.show_file(&commit, name)? {
                    let origin = format!("{rev}:{name}");
                    let text = String::from_utf8_lossy(&bytes);
                    return Ok((Config::from_yaml_named(&text, &origin)?, Some(origin)));
                }
            }
            Ok((Config::default(), None))
        }
    }
}

/// Effective settings of one file (`mote config explain`).
#[derive(Clone, Debug)]
pub struct EffectiveRules {
    pub language: String,
    pub is_test: bool,
    pub excluded: bool,
    /// Rule id → settings, sorted by id (without the engine-set
    /// [`JAVA_RELEASE_PARAM`](crate::java_release::JAVA_RELEASE_PARAM), shown in `java_release`).
    pub rules: BTreeMap<String, ActiveRule>,
    /// Java files: the Java release (configured, or read from the nearest build file on disk);
    /// `None` when unknown or not a Java file.
    pub java_release: Option<JavaRelease>,
}

#[derive(Default)]
struct CacheSlot {
    dir: Option<PathBuf>,
    cache: Option<Cache>,
}

/// Scans one repository with one configuration. Cheap to keep alive (the MCP server keeps one and
/// rescans changed files; the cache stays open between scans).
pub struct Scanner {
    registry: Arc<Registry>,
    compiled: CompiledConfig,
    root: PathBuf,
    settings: AnalysisSettings,
    /// Analysis settings hash (part of the cache key) of the files the skip checks apply to …
    settings_hash: u64,
    /// … and of the others (`skip_generated: false`, or `skip_generated_except`).
    settings_hash_unskipped: u64,
    cache: Mutex<CacheSlot>,
    /// Identifies this scanner in [`Retained`] state (a rescan needs the same configuration).
    id: u64,
}

static NEXT_SCANNER_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl std::fmt::Debug for Scanner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scanner")
            .field("root", &self.root)
            .field("registry", &self.registry)
            .finish_non_exhaustive()
    }
}

/// Per-file state through the pipeline.
struct Record {
    path: String,
    plugin: usize,
    is_test: bool,
    rules: Option<Arc<ResolvedRules>>,
    status: FileStatus,
    skip: Option<SkipReason>,
    result: Option<FileResult>,
    fresh_bytes: u64,
    analyze_ns: u64,
    /// Cache key of the analyzed content (`None` when not read: excluded, too large, unreadable).
    key: Option<CacheKey>,
    /// Whether the generated / minified / vendored checks apply (see
    /// [`CompiledConfig::skips_generated`]).
    skip_generated: bool,
    /// The file's npm packages, for the vendored check (JavaScript files it applies to).
    packages: Option<Arc<[String]>>,
}

/// Per-scan lookups of data outside a file that its analysis depends on (part of its cache key).
struct Lookups {
    /// The Java release of Java files.
    releases: JavaReleases,
    /// The npm packages of JavaScript files.
    packages: PackageNames,
}

/// A freshly analyzed record, its cache write, and a read diagnostic.
type AnalyzedMiss = (Record, Option<(CacheKey, Vec<u8>)>, Option<Diagnostic>);

struct MapOutput {
    records: Vec<Record>,
    writes: Vec<(CacheKey, Vec<u8>)>,
    used: Vec<CacheKey>,
    diagnostics: Vec<Diagnostic>,
}

enum Stage {
    /// Resolved without analysis (skipped, or a cache hit).
    Done(Record),
    /// Needs analysis: the entry, and its content when already read (to compute the blob id).
    Miss(Record, SourceEntry, Option<Vec<u8>>),
    /// The file disappeared.
    Gone,
}

/// One file to map: the entry, its plugin, and the cache key when already known (rescans).
type MapInput = (SourceEntry, usize, Option<CacheKey>);

/// What a scan maps: the filtered entries plus what the result needs about the source.
struct Plan {
    inputs: Vec<MapInput>,
    /// Every discovered path (before filters), for coverage mapping.
    all_paths: Vec<String>,
    files_seen: u64,
    loader: Arc<dyn BlobLoader>,
    /// Java release and npm packages of each file (read from the build files and manifests of
    /// this source).
    lookups: Lookups,
    info: SourceInfo,
    /// Source warnings (kept for rescans).
    warnings: Vec<String>,
    /// Warnings about this run's input only.
    notes: Vec<String>,
    /// The plan covers every file of the source (no path filters, not only changed files):
    /// triage entries for files missing from it are about files that are gone.
    full: bool,
}

/// The cache for one scan: the locked slot (one scan at a time per scanner), a read snapshot,
/// the cache file, and problems opening it.
struct CacheSession<'a> {
    slot: std::sync::MutexGuard<'a, CacheSlot>,
    reader: Option<CacheReader>,
    path: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

/// Per-file state kept in a [`ScanResult`] for [`Scanner::rescan`].
#[derive(Debug)]
struct RetainedFile {
    path: String,
    plugin: usize,
    key: Option<CacheKey>,
}

#[derive(Debug)]
pub(crate) struct RetainedState {
    /// [`Scanner`] id: a rescan needs the scanner (configuration) that made the result.
    scanner: u64,
    /// Every mapped file, sorted by path.
    files: Vec<RetainedFile>,
    all_paths: Vec<String>,
    /// The scan's path filters.
    paths: Vec<String>,
    /// Source warnings.
    warnings: Vec<String>,
}

/// Opaque state that lets [`Scanner::rescan`] update a [`ScanResult`] incrementally: per file,
/// the plugin and the cache key of the analyzed content. Not serialized.
#[derive(Clone)]
pub struct Retained(pub(crate) Arc<RetainedState>);

impl std::fmt::Debug for Retained {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Retained({} files)", self.0.files.len())
    }
}

impl Scanner {
    /// Validate `config` against `registry` and create a scanner for `root`.
    pub fn new(
        registry: Arc<Registry>,
        config: Config,
        root: impl Into<PathBuf>,
    ) -> Result<Scanner, EngineError> {
        let compiled = CompiledConfig::new(config, &registry)?;
        let settings = AnalysisSettings::from_config(compiled.config());
        let settings_hash = compiled.config().analysis_settings_hash();
        let settings_hash_unskipped = compiled.config().analysis_settings_hash_for(false);
        Ok(Scanner {
            registry,
            compiled,
            root: root.into(),
            settings,
            settings_hash,
            settings_hash_unskipped,
            cache: Mutex::new(CacheSlot::default()),
            id: NEXT_SCANNER_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }

    /// The plugin registry.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The configuration.
    pub fn config(&self) -> &Config {
        self.compiled.config()
    }

    /// The scanned root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Effective language, test status and rules for a repo-relative path (`None` if no plugin
    /// handles it).
    pub fn effective_rules(&self, path: &str) -> Option<EffectiveRules> {
        let path = path.replace('\\', "/");
        let plugin = self.registry.plugin_for_path(&path)?;
        let is_test = self.compiled.is_test(plugin, &path);
        // Rules as configured; the release that a scan adds to Java rules is reported apart.
        let resolved = self
            .compiled
            .resolve(&self.registry, plugin, is_test, &path);
        let rules = resolved
            .enabled
            .iter()
            .filter_map(|id| resolved.rules.get(id).map(|r| (id.to_string(), r.clone())))
            .collect();
        let language = self.registry.plugin(plugin).id();
        let java_release = (language == java_release::JAVA)
            .then(|| JavaReleases::disk(self.config().java.release, &self.root).release_for(&path))
            .flatten()
            .map(|r| (*r).clone());
        Some(EffectiveRules {
            language: language.to_string(),
            is_test,
            excluded: self.compiled.is_excluded(&path),
            rules,
            java_release,
        })
    }

    /// The Java release and npm package lookups for a discovery: its build files and
    /// `package.json` files, read with its loader (`listed`), or those on disk below the root.
    fn lookups(&self, discovery: Option<&Discovery>) -> Lookups {
        let configured = self.config().java.release;
        match discovery {
            Some(d) => Lookups {
                releases: JavaReleases::listed(configured, &d.entries, d.loader.clone()),
                packages: PackageNames::listed(&d.entries, d.loader.clone()),
            },
            None => Lookups {
                releases: JavaReleases::disk(configured, &self.root),
                packages: PackageNames::disk(&self.root),
            },
        }
    }

    /// Whether the generated / minified / vendored checks apply to `path`, and the npm packages
    /// of the file when the vendored check needs them.
    fn skip_checks(&self, path: &str, lookups: &Lookups) -> (bool, Option<Arc<[String]>>) {
        let skip = self.compiled.skips_generated(path);
        let packages = (skip && vendored::applies_to(path))
            .then(|| lookups.packages.names_for(path))
            .filter(|names| !names.is_empty());
        (skip, packages)
    }

    /// The analysis settings part of a file's cache key: the settings hash for whether the skip
    /// checks apply to it, with the names of its npm packages (a library banner naming one of
    /// them is the project's own, so a renamed package changes the outcome).
    fn file_settings_hash(&self, rec: &Record) -> u64 {
        if !rec.skip_generated {
            return self.settings_hash_unskipped;
        }
        match &rec.packages {
            Some(names) => {
                let mut h = xxhash_rust::xxh3::Xxh3::new();
                h.update(&self.settings_hash.to_le_bytes());
                for n in names.iter() {
                    h.update(n.as_bytes());
                    h.update(&[0]);
                }
                h.digest()
            }
            None => self.settings_hash,
        }
    }

    /// Close the cache database (it is reopened on the next scan).
    pub fn close_cache(&self) {
        let mut slot = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        *slot = CacheSlot::default();
    }

    /// Run a scan.
    pub fn scan(&self, opts: &ScanOptions) -> Result<ScanResult, EngineError> {
        self.with_pool(opts, || self.scan_inner(opts))
    }

    /// Normalize requested paths (absolute or root-relative) to root-relative ones: `.`/`..`
    /// resolved, on-disk case on case-insensitive file systems. A path outside the root is an
    /// error; `known` (paths of a previous scan) fixes the case of paths no longer on disk.
    fn requested_paths(
        &self,
        paths: &[String],
        known: &[String],
    ) -> Result<Vec<String>, EngineError> {
        let mut out = Vec::with_capacity(paths.len());
        for p in paths {
            let rel = source::normalize_request(&self.root, p).map_err(EngineError::Invalid)?;
            let rel = if known.binary_search(&rel).is_ok() || !source::case_insensitive_fs() {
                rel
            } else {
                let lower = rel.to_lowercase();
                let mut hits = known.iter().filter(|k| k.to_lowercase() == lower);
                match (hits.next(), hits.next()) {
                    (Some(k), None) => k.clone(),
                    _ => rel,
                }
            };
            out.push(rel);
        }
        Ok(out)
    }

    /// The entries a full discovery (like `source`) would list among the files `rels`: not
    /// ignored, not below a nested repository or a link.
    fn discover_restricted(
        &self,
        rels: &[String],
        source: &SourceSpec,
    ) -> Result<Discovery, EngineError> {
        let directory = || -> Result<Discovery, EngineError> {
            Ok(DirectorySource {
                root: self.root.clone(),
            }
            .discover_paths(rels)?)
        };
        if *source == SourceSpec::Directory {
            return directory();
        }
        if !worktree::disabled() {
            match worktree::locate(&self.root) {
                Located::Repo(layout) => {
                    let only = worktree::Only::new(rels);
                    match source::discover_worktree_fast_only(&self.root, &layout, Some(&only)) {
                        Ok(d) => return Ok(d),
                        Err(e) => tracing::debug!("fast discovery unavailable ({e}); using git"),
                    }
                }
                Located::NotARepo => return directory(),
                Located::Unsupported(e) => {
                    tracing::debug!("fast discovery unavailable ({e}); using git")
                }
            }
        }
        match Git::open(&self.root) {
            Ok(git) => Ok(WorktreeSource { git }.discover_paths(rels)?),
            Err(_) => directory(),
        }
    }

    /// Analyze specific files from disk (absolute or root-relative paths), without new-code
    /// comparison. Files a full scan would not see — gitignored, below a nested repository or a
    /// linked directory — are skipped with a warning; excluded files are skipped like in a scan.
    /// A path outside the root is an error. Duplication is detected among these files only (use
    /// [`rescan`](Self::rescan) to re-check files against a whole previous scan). Meant for
    /// editors; the cache makes repeated calls cheap.
    pub fn analyze_paths(
        &self,
        paths: &[String],
        opts: &ScanOptions,
    ) -> Result<ScanResult, EngineError> {
        self.with_pool(opts, || {
            let started = Instant::now();
            let mut warnings = Vec::new();
            let mut rels = Vec::new();
            for rel in self.requested_paths(paths, &[])? {
                match std::fs::metadata(self.root.join(&rel)) {
                    Ok(m) if m.is_file() => rels.push(rel),
                    _ => warnings.push(format!(
                        "{}: not a file",
                        if rel.is_empty() { "." } else { rel.as_str() }
                    )),
                }
            }
            rels.sort();
            rels.dedup();
            let mut discovery = self.discover_restricted(&rels, &opts.source)?;
            let found: FxHashSet<&str> =
                discovery.entries.iter().map(|e| e.path.as_str()).collect();
            for rel in &rels {
                if !found.contains(rel.as_str()) {
                    warnings.push(not_discovered(rel));
                }
            }
            discovery.warnings.extend(warnings);
            discovery.untracked.clear();
            discovery.git = None;
            discovery.info = SourceInfo {
                kind: "paths".into(),
                rev: None,
                commit: None,
                prefix: discovery.info.prefix,
            };
            let opts = ScanOptions {
                base: None,
                baseline: None,
                changed_only: false,
                ..opts.clone()
            };
            // The discovery holds the requested files only: build files and manifests are read
            // from disk.
            let lookups = self.lookups(None);
            let mut plan = self.plan(discovery, &opts, lookups)?;
            plan.full = false;
            let cache = self.cache_session(&opts);
            self.run(plan, &opts, None, cache, started, 0.0)
        })
    }

    /// Update `prev` (a result of this scanner) after the files in `changed` were edited, created
    /// or deleted: only those files are read and analyzed again; every other file's map-phase
    /// result comes from the cache (by the key recorded in `prev`), and duplication, suppressions,
    /// measures and the gate are recomputed across the whole set — the same result as a full
    /// scan of the same files, without discovery.
    ///
    /// `changed` holds absolute or root-relative paths; new files are added when a plugin handles
    /// them, they pass the previous scan's path filters, and a full scan would see them (not
    /// gitignored, not below a nested repository or a link). New-code classification is not
    /// recomputed (the result has no `new_code`; use [`scan`](Self::scan) with a base for that),
    /// and files are always read from disk, whatever the previous source. Fails when `prev` was
    /// not produced by this scanner (or was read back from JSON), and for a path outside the
    /// root.
    pub fn rescan(
        &self,
        prev: &ScanResult,
        changed: &[String],
        opts: &ScanOptions,
    ) -> Result<ScanResult, EngineError> {
        let state = prev
            .retained
            .as_ref()
            .map(|r| &r.0)
            .filter(|r| r.scanner == self.id)
            .ok_or_else(|| {
                EngineError::Invalid(
                    "this result cannot be updated incrementally (it comes from another scanner \
                     or from a file); run a full scan"
                        .into(),
                )
            })?;
        self.with_pool(opts, || {
            let started = Instant::now();
            let opts = ScanOptions {
                base: None,
                baseline: None,
                changed_only: false,
                paths: state.paths.clone(),
                ..opts.clone()
            };
            let filter = PathFilter::new(&opts.paths).map_err(EngineError::Invalid)?;
            let mut notes = Vec::new();
            // Changed path → its entry, or `None` when it is gone (deleted, not a file, or not
            // something a full scan would see).
            let mut now: BTreeMap<String, Option<SourceEntry>> = BTreeMap::new();
            let mut present = Vec::new();
            for rel in self.requested_paths(changed, &state.all_paths)? {
                if rel.is_empty() {
                    continue;
                }
                let size = std::fs::metadata(self.root.join(&rel))
                    .ok()
                    .filter(|m| m.is_file())
                    .map(|m| m.len());
                if let Some(size) = size {
                    present.push((rel.clone(), size));
                }
                now.insert(rel, None);
            }
            if !present.is_empty() {
                let rels: Vec<String> = present.iter().map(|(r, _)| r.clone()).collect();
                let spec = match prev.source.kind.as_str() {
                    "directory" => SourceSpec::Directory,
                    _ => SourceSpec::Auto,
                };
                let d = self.discover_restricted(&rels, &spec)?;
                let mut by_path: FxHashMap<String, SourceEntry> =
                    d.entries.into_iter().map(|e| (e.path.clone(), e)).collect();
                for (rel, size) in present {
                    match by_path.remove(&rel) {
                        Some(mut e) => {
                            e.size = Some(size);
                            now.insert(rel, Some(e));
                        }
                        None => notes.push(not_discovered(&rel)),
                    }
                }
            }
            let file_entry = |path: &str| SourceEntry {
                path: path.to_string(),
                blob_id: None,
                size: None,
                location: EntryLocation::File,
            };
            let mut inputs: Vec<MapInput> = state
                .files
                .iter()
                .filter(|f| !now.contains_key(&f.path))
                .map(|f| (file_entry(&f.path), f.plugin, f.key))
                .collect();
            let mut all_paths: Vec<String> = state
                .all_paths
                .iter()
                .filter(|p| !now.contains_key(*p))
                .cloned()
                .collect();
            for (rel, entry) in now {
                let Some(entry) = entry else { continue };
                all_paths.push(rel.clone());
                if !filter.matches(&rel) {
                    continue;
                }
                if let Some(plugin) = self.registry.plugin_for_path(&rel) {
                    inputs.push((entry, plugin, None));
                }
            }
            inputs.sort_by(|a, b| a.0.path.cmp(&b.0.path));
            all_paths.sort_unstable();
            let files_seen = all_paths.iter().filter(|p| filter.matches(p)).count() as u64;
            let plan = Plan {
                inputs,
                all_paths,
                files_seen,
                loader: Arc::new(GitLoader::new(&self.root, None)),
                // Changed files are read from disk, and so are their build files and manifests.
                // Unchanged files keep their previous key (a build file edited since is not
                // noticed).
                lookups: self.lookups(None),
                info: prev.source.clone(),
                warnings: state.warnings.clone(),
                notes,
                full: state.paths.is_empty(),
            };
            let cache = self.cache_session(&opts);
            self.run(plan, &opts, None, cache, started, 0.0)
        })
    }

    /// Run `f` on a dedicated analysis pool whose workers have [`ANALYSIS_STACK_SIZE`] of
    /// stack, never on the caller's thread: parsers recurse on deeply nested input, and a stack
    /// overflow aborts the process. (Running `f` itself on a worker matters: rayon runs a
    /// parallel iterator that does not split — one file — on the calling thread.)
    fn with_pool<T: Send>(&self, opts: &ScanOptions, f: impl FnOnce() -> T + Send) -> T {
        match analysis_pool(opts.threads) {
            Some(pool) => pool.install(f),
            // Not even a one-thread pool with the full stack could be reserved. Running on
            // smaller stacks (rayon's global pool) would let deeply nested input abort the
            // process, so fail loudly instead.
            None => panic!(
                "cannot reserve a {} MiB stack for an analysis thread (address space exhausted?)",
                ANALYSIS_STACK_SIZE >> 20
            ),
        }
    }

    fn discover(&self, opts: &ScanOptions) -> Result<Discovery, EngineError> {
        if matches!(opts.source, SourceSpec::Auto | SourceSpec::Worktree) && !worktree::disabled() {
            // In-process fast path; anything it does not handle falls through to the git CLI.
            match worktree::locate(&self.root) {
                Located::Repo(layout) => {
                    match source::discover_worktree_fast(&self.root, &layout) {
                        Ok(d) => return Ok(d),
                        Err(e) => tracing::debug!("fast discovery unavailable ({e}); using git"),
                    }
                }
                Located::NotARepo if opts.source == SourceSpec::Auto => {
                    return Ok(DirectorySource {
                        root: self.root.clone(),
                    }
                    .discover()?);
                }
                Located::NotARepo => {}
                Located::Unsupported(e) => {
                    tracing::debug!("fast discovery unavailable ({e}); using git")
                }
            }
        }
        let open = || Git::open(&self.root);
        let d = match &opts.source {
            SourceSpec::Auto => match open() {
                Ok(g) => WorktreeSource { git: g }.discover()?,
                Err(GitError::NotInstalled(e)) => {
                    let mut d = DirectorySource {
                        root: self.root.clone(),
                    }
                    .discover()?;
                    d.warnings.push(format!(
                        "git is not available ({e}); scanning as a plain directory"
                    ));
                    d
                }
                Err(_) => DirectorySource {
                    root: self.root.clone(),
                }
                .discover()?,
            },
            SourceSpec::Worktree => WorktreeSource { git: open()? }.discover()?,
            SourceSpec::Directory => DirectorySource {
                root: self.root.clone(),
            }
            .discover()?,
            SourceSpec::Rev(rev) => RevSource {
                git: open()?,
                rev: rev.clone(),
            }
            .discover()?,
            SourceSpec::Staged => StagedSource { git: open()? }.discover()?,
        };
        Ok(d)
    }

    fn scan_inner(&self, opts: &ScanOptions) -> Result<ScanResult, EngineError> {
        let started = Instant::now();
        if opts.changed_only && opts.base.is_none() {
            return Err(EngineError::Invalid(
                "`changed_only` requires a base".into(),
            ));
        }
        if opts.base.is_some() && opts.baseline.is_some() {
            return Err(EngineError::Invalid(
                "compare with a git base or with a baseline results file, not both".into(),
            ));
        }
        // Path filters may be written as absolute paths below the root.
        let paths = opts
            .paths
            .iter()
            .map(|p| source::relative_pattern(&self.root, p))
            .collect::<Result<Vec<String>, String>>()
            .map_err(EngineError::Invalid)?;
        let opts = &ScanOptions {
            paths,
            ..opts.clone()
        };
        // Discovery and opening the cache (which loads it into memory) run concurrently.
        let mut slot = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        let slot_ref: &mut CacheSlot = &mut slot;
        let (discovery, (reader, cache_path, cache_diags)) =
            rayon::join(|| self.discover(opts), || self.open_reader(slot_ref, opts));
        let cache = CacheSession {
            slot,
            reader,
            path: cache_path,
            diagnostics: cache_diags,
        };
        let mut discovery = discovery?;
        let t_discovery = ms(started);
        check_filters_match(&opts.paths, &discovery.entries)?;
        // Before `changed_only` narrows the entries: build files and manifests apply to
        // unchanged ones too.
        let lookups = self.lookups(Some(&discovery));

        let t = Instant::now();
        let diff = match &opts.base {
            None => None,
            Some(base) => {
                let Some(g) = &discovery.git else {
                    return Err(EngineError::Invalid(
                        "comparing with a base needs a git repository (the scanned source is a plain directory)".into(),
                    ));
                };
                let head = match discovery.info.kind.as_str() {
                    "staged" => HeadSpec::Index,
                    "rev" => HeadSpec::Commit(discovery.info.commit.clone().unwrap_or_default()),
                    _ => HeadSpec::Worktree,
                };
                let mut d = newcode::compute_diff(g, base, &head).map_err(|e| match e {
                    // Unrelated histories are the user's to fix (another base, a deeper fetch).
                    GitError::NoMergeBase(m) => EngineError::Invalid(m),
                    other => EngineError::Git(other),
                })?;
                for p in &discovery.untracked {
                    d.changed.entry(p.clone()).or_insert(ChangedFile {
                        kind: ChangeKind::Untracked,
                        base_path: None,
                        base_blob: None,
                        head_blob: None,
                        new_lines: Vec::new(),
                        old_lines: Vec::new(),
                        all_new: true,
                    });
                }
                Some((base.clone(), d))
            }
        };
        if opts.changed_only {
            if let Some((_, d)) = &diff {
                discovery
                    .entries
                    .retain(|e| d.changed.contains_key(&e.path));
            }
        }
        let t_diff = ms(t);
        let plan = self.plan(discovery, opts, lookups)?;
        let mut result = self.run(plan, opts, diff, cache, started, t_diff)?;
        result.stats.timings.discovery_ms = t_discovery;
        Ok(result)
    }

    /// Filter discovered entries (path filters, language plugins).
    fn plan(
        &self,
        discovery: Discovery,
        opts: &ScanOptions,
        lookups: Lookups,
    ) -> Result<Plan, EngineError> {
        let filter = PathFilter::new(&opts.paths).map_err(EngineError::Invalid)?;
        let mut all_paths = Vec::with_capacity(discovery.entries.len());
        let mut inputs = Vec::with_capacity(discovery.entries.len());
        let mut files_seen = 0u64;
        for e in discovery.entries {
            let matches = filter.matches(&e.path);
            files_seen += u64::from(matches);
            all_paths.push(e.path.clone());
            if matches {
                if let Some(p) = self.registry.plugin_for_path(&e.path) {
                    inputs.push((e, p, None));
                }
            }
        }
        Ok(Plan {
            inputs,
            all_paths,
            files_seen,
            loader: discovery.loader,
            lookups,
            info: discovery.info,
            warnings: discovery.warnings,
            notes: Vec::new(),
            full: opts.paths.is_empty() && !opts.changed_only,
        })
    }

    fn open_cache<'a>(
        &self,
        slot: &'a mut CacheSlot,
        opts: &ScanOptions,
        diags: &mut Vec<Diagnostic>,
    ) -> Option<&'a mut Cache> {
        let cfg = &self.compiled.config().cache;
        if !opts.cache.unwrap_or(cfg.enabled) {
            return None;
        }
        let dir = opts
            .cache_dir
            .clone()
            .or_else(|| {
                std::env::var_os("MOTE_CACHE_DIR")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
            })
            .or_else(|| cfg.dir.as_ref().map(|d| self.root.join(d)))
            .unwrap_or_else(|| self.root.join(".mote").join("cache"));
        if slot.dir.as_ref() != Some(&dir) || slot.cache.is_none() {
            slot.cache = None;
            slot.dir = Some(dir.clone());
            match Cache::open(&dir, cfg.max_size_mb) {
                Ok(c) => slot.cache = Some(c),
                Err(e) => {
                    diags.push(Diagnostic::warning(None, e));
                    return None;
                }
            }
        }
        slot.cache.as_mut()
    }

    /// Open the cache (if enabled) and start a read snapshot. Returns the reader, the cache file
    /// (with `/` separators) and any problems.
    fn open_reader(
        &self,
        slot: &mut CacheSlot,
        opts: &ScanOptions,
    ) -> (Option<CacheReader>, Option<String>, Vec<Diagnostic>) {
        let mut diags = Vec::new();
        let mut reader = None;
        let mut path = None;
        // A damaged cache file is deleted and recreated once.
        for attempt in 0..2 {
            let Some(cache) = self.open_cache(slot, opts, &mut diags) else {
                break;
            };
            path = Some(cache.path().to_string_lossy().replace('\\', "/"));
            match cache.reader() {
                Ok(r) => {
                    reader = Some(r);
                    break;
                }
                Err(e) if attempt == 0 && cache.is_corrupt() => {
                    tracing::warn!("cache unreadable ({e}); recreating it");
                    discard_cache(slot, &mut diags);
                }
                Err(e) => {
                    diags.push(Diagnostic::warning(
                        None,
                        format!("cache unreadable ({e}); running without cache"),
                    ));
                    break;
                }
            }
        }
        let path = path.filter(|_| reader.is_some());
        (reader, path, diags)
    }

    /// Lock the cache slot for one scan and open a read snapshot.
    fn cache_session(&self, opts: &ScanOptions) -> CacheSession<'_> {
        let mut slot = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        let (reader, path, diagnostics) = self.open_reader(&mut slot, opts);
        CacheSession {
            slot,
            reader,
            path,
            diagnostics,
        }
    }

    fn run(
        &self,
        plan: Plan,
        opts: &ScanOptions,
        diff: Option<(String, DiffInfo)>,
        cache: CacheSession<'_>,
        started: Instant,
        t_diff: f64,
    ) -> Result<ScanResult, EngineError> {
        let Plan {
            inputs,
            all_paths,
            files_seen,
            loader,
            lookups,
            info,
            warnings,
            notes,
            full,
        } = plan;
        // Before the analysis: a malformed triage file fails the scan early.
        let triage = self.load_triage(opts, &info)?;
        let CacheSession {
            mut slot,
            reader,
            path: cache_path,
            diagnostics: mut cache_diags,
        } = cache;
        let mut diagnostics: Vec<Diagnostic> = warnings
            .iter()
            .chain(&notes)
            .map(|w| Diagnostic::warning(None, w.clone()))
            .collect();
        diagnostics.append(&mut cache_diags);
        let use_cache = reader.is_some();

        // MAP
        let t = Instant::now();
        let matched = inputs.len() as u64;
        let mut out = self.map(inputs, loader.as_ref(), reader.as_ref(), &lookups);
        diagnostics.append(&mut out.diagnostics);
        let t_map = ms(t);
        let retained_files: Vec<RetainedFile> = out
            .records
            .iter()
            .map(|r| RetainedFile {
                path: r.path.clone(),
                plugin: r.plugin,
                key: r.key,
            })
            .collect();

        // REDUCE: duplication, extra findings
        let t = Instant::now();
        let (duplications, dup_ranges) = self.reduce(&mut out.records);
        let t_cpd = ms(t);

        // TRIAGE: an overlay on the (cached) file results, never part of them.
        let (triage_report, mut stale) = match &triage {
            Some(t) => {
                let (report, stale) = self.apply_triage(t, &mut out.records, &all_paths, full);
                (Some(report), stale)
            }
            None => (None, Vec::new()),
        };
        let triaged: FxHashMap<&str, &str> = triage
            .iter()
            .flat_map(|t| &t.file.entries)
            .filter_map(|e| Some((e.fingerprint.as_deref()?, e.status.as_str())))
            .collect();

        // NEW CODE
        let t = Instant::now();
        let mut base_stats = (0u64, 0u64);
        let mut new_code_parts = None;
        let filter = PathFilter::new(&opts.paths).map_err(EngineError::Invalid)?;
        if let Some((base, d)) = &diff {
            let (parts, base_out) = self.new_code(
                &mut out.records,
                d,
                &dup_ranges,
                &filter,
                loader.as_ref(),
                reader.as_ref(),
                &lookups,
                &triaged,
            );
            base_stats = (base_out.analyzed, base_out.hits);
            out.writes.extend(base_out.writes);
            out.used.extend(base_out.used);
            diagnostics.extend(base_out.diagnostics);
            // A stale entry is new when the change touched its file or the triage file.
            let triage_file = triage.as_ref().map(|t| t.rel.as_str());
            for (f, path) in &mut stale {
                let touched =
                    |p: &str| d.changed.contains_key(p) || d.deleted.iter().any(|x| x.path == p);
                f.status = if touched(path) || triage_file.is_some_and(touched) {
                    FindingStatus::New
                } else {
                    FindingStatus::Unchanged
                };
            }
            new_code_parts = Some((base.clone(), d.merge_base.clone(), parts));
        } else if let Some(b) = &opts.baseline {
            let head_files: FxHashSet<&str> = all_paths.iter().map(String::as_str).collect();
            let mut heads: Vec<&mut Finding> = out
                .records
                .iter_mut()
                .filter_map(|r| r.result.as_mut())
                .flat_map(|r| r.findings.iter_mut())
                .chain(stale.iter_mut().map(|(f, _)| f))
                .collect();
            let changed_only = opts.changed_only;
            let c = baseline::classify(&mut heads, &head_files, b, &|p| {
                !changed_only && filter.matches(p)
            });
            let parts = NewCodeParts {
                fixed: c.fixed,
                baseline: Some(BaselineInfo {
                    file: b.file.clone(),
                    commit: b.commit.clone(),
                    engine_version: b.engine_version.clone(),
                    findings: b.findings.len() as u64,
                    matched: c.matched,
                }),
                ..Default::default()
            };
            new_code_parts = Some((b.file.clone(), b.commit.clone().unwrap_or_default(), parts));
        }
        drop(triaged);
        let t_new = ms(t);

        // CACHE WRITE (one transaction) + prune
        drop(reader);
        let t = Instant::now();
        if use_cache {
            // Lookups found the file damaged (they missed from then on): replace it with a fresh
            // one holding this scan's results.
            if slot.cache.as_ref().is_some_and(Cache::is_corrupt) {
                discard_cache(&mut slot, &mut diagnostics);
                let _ = self.open_cache(&mut slot, opts, &mut diagnostics);
            }
            if let Some(c) = slot.cache.as_mut() {
                if let Err(e) = c.write(&out.writes) {
                    diagnostics.push(Diagnostic::warning(
                        None,
                        format!("could not write cache ({e})"),
                    ));
                }
                let used: FxHashSet<CacheKey> = out.used.iter().copied().collect();
                if let Err(e) = c.prune(&used) {
                    diagnostics.push(Diagnostic::warning(
                        None,
                        format!("could not prune cache ({e})"),
                    ));
                }
                if c.is_corrupt() {
                    discard_cache(&mut slot, &mut diagnostics);
                }
            }
        }
        drop(slot);
        let t_write = ms(t);

        // COVERAGE
        let t = Instant::now();
        let coverage = self.load_coverage(opts, &all_paths, &mut diagnostics);
        let t_cov = ms(t);

        let mut result = self.assemble(
            out.records,
            stale.into_iter().map(|(f, _)| f).collect(),
            duplications,
            coverage,
            new_code_parts,
            diagnostics,
            info,
            Stats {
                files_seen,
                matched,
                base_stats,
                cache_path,
            },
        );
        result.triage = triage_report;
        if let Some(b) = &opts.baseline {
            for w in b.warnings(&result.rules) {
                result.diagnostics.push(Diagnostic::warning(None, w));
            }
            result
                .diagnostics
                .sort_by(|a, b| (&a.path, &a.message).cmp(&(&b.path, &b.message)));
            result.diagnostics.dedup();
        }
        result.retained = Some(Retained(Arc::new(RetainedState {
            scanner: self.id,
            files: retained_files,
            all_paths,
            paths: opts.paths.clone(),
            warnings,
        })));

        result.stats.timings = Timings {
            discovery_ms: 0.0,
            diff_ms: t_diff,
            map_ms: t_map,
            cpd_ms: t_cpd,
            new_code_ms: t_new,
            coverage_ms: t_cov,
            cache_write_ms: t_write,
            total_ms: ms(started),
        };
        Ok(result)
    }

    fn job<'a>(&'a self, rec: &'a Record, rules: &'a ResolvedRules) -> FileJob<'a> {
        let cpd = &self.compiled.config().cpd;
        FileJob {
            registry: &self.registry,
            plugin: rec.plugin,
            path: &rec.path,
            is_test: rec.is_test,
            rules,
            settings: &self.settings,
            // Both covered by the cache key (see `file_settings_hash`).
            skip_generated: rec.skip_generated,
            packages: rec.packages.as_deref().unwrap_or(&[]),
            // Must only depend on what the cache key covers (settings hash, test flag, path).
            keep_tokens: cpd.enabled
                && (!rec.is_test || cpd.include_tests)
                && !self.compiled.is_cpd_excluded(&rec.path),
        }
    }

    fn key(&self, rec: &Record, rules: &ResolvedRules, blob_id: &str) -> CacheKey {
        let plugin = self.registry.plugin(rec.plugin);
        cache::cache_key(&KeyParts {
            plugin_id: plugin.id(),
            plugin_version: plugin.version(),
            rules_hash: rules.hash,
            blob_id,
            is_test: rec.is_test,
            settings_hash: self.file_settings_hash(rec),
            path: &rec.path,
        })
    }

    /// Parallel map over entries with cache lookups; misses are loaded, sorted largest first and
    /// analyzed in parallel.
    fn map(
        &self,
        entries: Vec<MapInput>,
        loader: &dyn BlobLoader,
        reader: Option<&CacheReader>,
        lookups: &Lookups,
    ) -> MapOutput {
        let stages: Vec<Stage> = entries
            .into_par_iter()
            .map(|(entry, plugin, key)| self.stage(entry, plugin, key, loader, reader, lookups))
            .collect();
        let mut records = Vec::with_capacity(stages.len());
        let mut used = Vec::new();
        let mut misses = Vec::new();
        for s in stages {
            match s {
                Stage::Done(r) => {
                    used.extend(r.key);
                    records.push(r);
                }
                Stage::Miss(r, e, c) => misses.push((r, e, c)),
                Stage::Gone => {}
            }
        }
        // Load contents not read yet (parallel; object reads serialize on one cat-file process).
        let mut loaded: Vec<(Record, io::Result<Vec<u8>>)> = misses
            .into_par_iter()
            .map(|(r, e, c)| {
                let content = match c {
                    Some(c) => Ok(c),
                    None => loader.load(&e),
                };
                (r, content)
            })
            .collect();
        loaded.sort_by_key(|(_, c)| std::cmp::Reverse(c.as_ref().map(|c| c.len()).unwrap_or(0)));
        let analyzed: Vec<AnalyzedMiss> = loaded
            .into_par_iter()
            // One task per file: sizes vary a lot, and coarse chunks (what rayon picks when the
            // scan itself runs on a pool worker) leave threads idle — measured 1.8x slower.
            .with_max_len(1)
            .map(|(mut rec, content)| {
                let rules = rec.rules.clone().unwrap_or_else(empty_rules);
                let t = Instant::now();
                let fresh_bytes = content.as_ref().map(|b| b.len() as u64).unwrap_or(0);
                let (outcome, diag) = {
                    let job = self.job(&rec, &rules);
                    match content {
                        Ok(bytes) => (analyze::analyze_bytes(&job, &bytes), None),
                        Err(e) => {
                            let msg = format!("Mote could not read this file: {e}");
                            let d = Diagnostic::warning(Some(&rec.path), msg.clone());
                            (
                                FileOutcome::Analyzed(Box::new(analyze::internal_error_result(
                                    &job, &msg,
                                ))),
                                Some(d),
                            )
                        }
                    }
                };
                rec.fresh_bytes = fresh_bytes;
                rec.analyze_ns = t.elapsed().as_nanos() as u64;
                let write = match (&outcome, rec.key) {
                    (FileOutcome::Analyzed(r), _) if r.internal_error => None,
                    (_, Some(k)) if reader.is_some() => cache::encode(&k, &outcome).map(|b| (k, b)),
                    _ => None,
                };
                apply_outcome(&mut rec, outcome, FileStatus::Analyzed);
                (rec, write, diag)
            })
            .collect();
        let mut writes = Vec::new();
        let mut diagnostics = Vec::new();
        for (rec, w, d) in analyzed {
            if let Some((k, b)) = w {
                used.push(k);
                writes.push((k, b));
            }
            diagnostics.extend(d);
            records.push(rec);
        }
        records.sort_by(|a, b| a.path.cmp(&b.path));
        MapOutput {
            records,
            writes,
            used,
            diagnostics,
        }
    }

    /// Resolve one entry: skip checks, then the cache (by `known_key` when the caller knows it,
    /// else by blob id — hashing the content when the source does not know it).
    fn stage(
        &self,
        entry: SourceEntry,
        plugin: usize,
        known_key: Option<CacheKey>,
        loader: &dyn BlobLoader,
        reader: Option<&CacheReader>,
        lookups: &Lookups,
    ) -> Stage {
        let mut rec = Record {
            path: entry.path.clone(),
            plugin,
            is_test: false,
            rules: None,
            status: FileStatus::Skipped,
            skip: None,
            result: None,
            fresh_bytes: 0,
            analyze_ns: 0,
            key: None,
            skip_generated: false,
            packages: None,
        };
        rec.is_test = self.compiled.is_test(plugin, &rec.path);
        // `exclude` wins over `skip_generated_except`.
        if self.compiled.is_excluded(&rec.path) {
            rec.skip = Some(SkipReason::Excluded);
            return Stage::Done(rec);
        }
        if entry.size.is_some_and(|s| s > self.settings.max_file_size) {
            rec.skip = Some(SkipReason::TooLarge);
            return Stage::Done(rec);
        }
        (rec.skip_generated, rec.packages) = self.skip_checks(&rec.path, lookups);
        // Tool-written files known by their path (Yarn Plug'n'Play): skipped before reading.
        if rec.skip_generated && analyze::is_generated_path(&rec.path) {
            rec.skip = Some(SkipReason::Generated);
            return Stage::Done(rec);
        }
        // The Java release is part of the rule set, hence of the cache key.
        let release = (self.registry.plugin(plugin).id() == java_release::JAVA)
            .then(|| lookups.releases.release_for(&rec.path))
            .flatten()
            .map(|r| r.release);
        let rules = self.compiled.resolve_with_release(
            &self.registry,
            plugin,
            rec.is_test,
            &rec.path,
            release,
        );
        rec.rules = Some(rules.clone());
        if let Some(key) = known_key {
            // Unchanged since the previous scan: reuse its outcome. If the cache lost it, the
            // file is read again below (its blob id is then unknown, so it gets a fresh key).
            if let Some(outcome) = reader.and_then(|r| r.get_outcome(&key)) {
                rec.key = Some(key);
                apply_outcome(&mut rec, outcome, FileStatus::Cached);
                return Stage::Done(rec);
            }
        }
        let blob_id = entry.blob_id.as_ref().filter(|_| known_key.is_none());
        let (blob_id, content) = match blob_id {
            Some(id) => (id.clone(), None),
            None => match loader.load(&entry) {
                Ok(bytes) => (git::blob_id(&bytes), Some(bytes)),
                Err(e)
                    if e.kind() == io::ErrorKind::NotFound
                        && entry.location == EntryLocation::File =>
                {
                    return Stage::Gone;
                }
                Err(e) => {
                    let job = self.job(&rec, &rules);
                    let r = analyze::internal_error_result(
                        &job,
                        &format!("Mote could not read this file: {e}"),
                    );
                    apply_outcome(
                        &mut rec,
                        FileOutcome::Analyzed(Box::new(r)),
                        FileStatus::Analyzed,
                    );
                    return Stage::Done(rec);
                }
            },
        };
        let key = self.key(&rec, &rules, &blob_id);
        rec.key = Some(key);
        if let Some(outcome) = reader.and_then(|r| r.get_outcome(&key)) {
            apply_outcome(&mut rec, outcome, FileStatus::Cached);
            return Stage::Done(rec);
        }
        Stage::Miss(rec, entry, content)
    }

    /// Cross-file reduce: duplication blocks, duplicated-block findings, unused suppressions.
    /// Returns the blocks and each record's merged duplicated line ranges.
    fn reduce(&self, records: &mut [Record]) -> (Vec<DuplicationBlock>, Vec<Vec<(u32, u32)>>) {
        let cfg = &self.compiled.config().cpd;
        let mut dup_ranges: Vec<Vec<(u32, u32)>> = vec![Vec::new(); records.len()];
        let mut blocks_out = Vec::new();
        let mut per_record_occ: Vec<Vec<DupOcc>> = vec![Vec::new(); records.len()];
        if cfg.enabled {
            let mut by_lang: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            for (i, r) in records.iter().enumerate() {
                let Some(res) = &r.result else { continue };
                if res.tokens.is_empty()
                    || (r.is_test && !cfg.include_tests)
                    || self.compiled.is_cpd_excluded(&r.path)
                {
                    continue;
                }
                by_lang.entry(r.plugin).or_default().push(i);
            }
            for (plugin, idxs) in by_lang {
                let files: Vec<cpd::CpdFile> = idxs
                    .iter()
                    .map(|&i| match records[i].result.as_ref() {
                        Some(r) => cpd::CpdFile {
                            hashes: &r.tokens.hashes,
                            runs: &r.tokens.runs,
                            fingerprints: (r.tokens.fp_min_tokens == cfg.min_tokens)
                                .then_some(r.tokens.fingerprints.as_slice()),
                        },
                        None => cpd::CpdFile {
                            hashes: &[],
                            runs: &[],
                            fingerprints: None,
                        },
                    })
                    .collect();
                let blocks = cpd::find_duplicates(&files, cfg.min_tokens, cfg.min_lines);
                let language = self.registry.plugin(plugin).id().to_string();
                for b in blocks {
                    let occ: Vec<DuplicationOccurrence> = b
                        .occurrences
                        .iter()
                        .map(|o| DuplicationOccurrence {
                            path: records[idxs[o.file]].path.clone(),
                            start_line: o.start_line,
                            end_line: o.end_line,
                        })
                        .collect();
                    for (k, o) in b.occurrences.iter().enumerate() {
                        let ri = idxs[o.file];
                        dup_ranges[ri].push((o.start_line, o.end_line));
                        let others: Vec<Other> = b
                            .occurrences
                            .iter()
                            .enumerate()
                            .filter(|(j, _)| *j != k)
                            .map(|(_, x)| (idxs[x.file] as u32, x.start_line, x.end_line))
                            .collect();
                        per_record_occ[ri].push(DupOcc {
                            start: o.start_line,
                            end: o.end_line,
                            hash: b.content_hash,
                            tokens: b.tokens,
                            others,
                            n_others: 0,
                        });
                    }
                    let lines = occ
                        .iter()
                        .map(|o| o.end_line - o.start_line + 1)
                        .max()
                        .unwrap_or(0);
                    blocks_out.push(DuplicationBlock {
                        language: language.clone(),
                        tokens: b.tokens,
                        lines,
                        occurrences: occ,
                    });
                }
            }
        }
        for ranges in &mut dup_ranges {
            *ranges = cpd::merge_ranges(std::mem::take(ranges));
        }
        // Paths of files that other files' duplicates point to.
        let referenced: FxHashMap<u32, String> = per_record_occ
            .iter()
            .flatten()
            .flat_map(|o| o.others.iter().map(|x| x.0))
            .collect::<FxHashSet<u32>>()
            .into_iter()
            .map(|i| (i, records[i as usize].path.clone()))
            .collect();
        let referenced = &referenced;
        // End column of every occurrence's last line (the tokens are freed below): ranges end at
        // the end of that line, so exclusive-end consumers (SARIF) keep it.
        let end_cols: FxHashMap<(u32, u32), u32> = per_record_occ
            .iter()
            .enumerate()
            .flat_map(|(ri, occs)| occs.iter().map(move |o| (ri, o.end)))
            .filter_map(|(ri, end)| {
                let col = records[ri].result.as_ref()?.tokens.line_end(end)?;
                Some(((ri as u32, end), col))
            })
            .collect();
        let end_cols = &end_cols;
        let dup_range = move |file: u32, start: u32, end: u32| TextRange {
            start_line: start,
            start_col: 1,
            end_line: end,
            end_col: end_cols.get(&(file, end)).copied().unwrap_or(1),
        };
        records.par_iter_mut().enumerate().zip(dup_ranges.par_iter()).zip(per_record_occ.into_par_iter()).for_each(|(((ri, rec), ranges), occs)| {
            let Some(rules) = rec.rules.clone() else { return };
            let Some(res) = rec.result.as_mut() else { return };
            // Duplication is done: free the token arrays now, in parallel.
            res.tokens = analyze::CpdTokens::default();
            let occs = merge_occurrences(ri as u32, occs);
            res.measures.duplicated_lines = cpd::count_lines(ranges);
            res.measures.duplicated_blocks = occs.len() as u32;
            let dup_id = self.registry.generic_set(rec.plugin).id(GenericRule::DuplicatedBlock);
            let mut pending = Vec::new();
            if rules.rules.is_enabled(dup_id) && !self.registry.overridden_generics(rec.plugin).contains(&GenericRule::DuplicatedBlock) {
                for DupOcc { start, end, hash, others, n_others, .. } in occs {
                    let lines = end - start + 1;
                    let suppressed = suppress::suppress(dup_id, start, &mut res.suppressions);
                    let s = if n_others == 1 { "" } else { "s" };
                    pending.push(Pending {
                        raw: RawFinding {
                            rule_id: dup_id.to_string(),
                            message: format!(
                                "Extract this block of {lines} duplicated lines; the same code appears in {n_others} other place{s}"
                            ),
                            range: dup_range(ri as u32, start, end),
                            symbol: None,
                            value: Some(f64::from(lines)),
                            secondary: others
                                .into_iter()
                                .map(|(file, s, e)| Location {
                                    path: referenced.get(&file).cloned().unwrap_or_default(),
                                    range: dup_range(file, s, e),
                                    message: Some("Duplicate".into()),
                                })
                                .collect(),
                        },
                        anchor: format!("dup:{hash:016x}").into(),
                        suppressed,
                    });
                }
            }
            for (raw, anchor) in suppress::unused_findings(&res.suppressions, &rules.rules) {
                pending.push(Pending {
                    raw,
                    anchor: anchor.into(),
                    suppressed: None,
                });
            }
            if !pending.is_empty() {
                let extra = analyze::finalize(&self.registry, &rules.rules, &rec.path, pending);
                res.findings.extend(extra);
                res.findings.sort_by(finding_order);
            }
        });
        blocks_out.sort_by(|a, b| {
            let (x, y) = (&a.occurrences[0], &b.occurrences[0]);
            (
                &x.path,
                x.start_line,
                std::cmp::Reverse(a.tokens),
                &a.language,
            )
                .cmp(&(
                    &y.path,
                    y.start_line,
                    std::cmp::Reverse(b.tokens),
                    &b.language,
                ))
        });
        (blocks_out, dup_ranges)
    }

    #[allow(clippy::too_many_arguments)]
    fn new_code(
        &self,
        records: &mut [Record],
        diff: &DiffInfo,
        dup_ranges: &[Vec<(u32, u32)>],
        filter: &PathFilter,
        loader: &dyn BlobLoader,
        reader: Option<&CacheReader>,
        lookups: &Lookups,
        triaged: &FxHashMap<&str, &str>,
    ) -> (NewCodeParts, BaseOutput) {
        let mut parts = NewCodeParts::default();
        let mut notes = Vec::new();
        // Head records needing a base analysis: (record index, base blob, touched base lines).
        let mut base_needed: Vec<BaseNeed> = Vec::new();
        for (i, rec) in records.iter_mut().enumerate() {
            let changed_skip = match rec.skip {
                Some(SkipReason::Generated) => Some(if analyze::is_generated_path(&rec.path) {
                    "generated code (a Yarn Plug'n'Play file)"
                } else {
                    "generated code (marker in its header comment)"
                }),
                Some(SkipReason::Vendored) => {
                    Some("vendored code (a library banner or bundler output)")
                }
                _ => None,
            };
            if let Some(why) = changed_skip.filter(|_| diff.changed.contains_key(&rec.path)) {
                // A marker opts a file out of analysis: make that visible when it changes.
                notes.push(Diagnostic::warning(
                    Some(&rec.path),
                    format!(
                        "{}: changed, but skipped as {why}; its changes are not analyzed",
                        rec.path
                    ),
                ));
            }
            let Some(res) = rec.result.as_mut() else {
                continue;
            };
            let Some(ch) = diff.changed.get(&rec.path) else {
                for f in &mut res.findings {
                    f.status = FindingStatus::Unchanged;
                }
                continue;
            };
            parts.changed_files += 1;
            parts.changes.insert(rec.path.clone(), ch.kind);
            if let Some(bp) = &ch.base_path {
                if bp != &rec.path {
                    parts.renamed.insert(rec.path.clone(), bp.clone());
                }
            }
            let lines = res.measures.lines.max(1);
            let ranges: Vec<(u32, u32)> = if ch.all_new {
                vec![(1, lines)]
            } else {
                let clipped: Vec<(u32, u32)> = ch
                    .new_lines
                    .iter()
                    .filter(|r| r.0 <= lines)
                    .map(|&(s, e)| (s, e.min(lines)))
                    .collect();
                cpd::merge_ranges(clipped)
            };
            if ch.all_new {
                for f in &mut res.findings {
                    f.status = FindingStatus::New;
                }
            } else if ch.base_blob.is_some() && ch.base_blob == ch.head_blob {
                for f in &mut res.findings {
                    f.status = FindingStatus::Unchanged;
                }
            } else if let Some(b) = &ch.base_blob {
                let base_path = ch.base_path.clone().unwrap_or_else(|| rec.path.clone());
                base_needed.push((
                    i,
                    b.clone(),
                    base_path,
                    cpd::merge_ranges(ch.old_lines.clone()),
                ));
            } else {
                for f in &mut res.findings {
                    f.status = FindingStatus::New;
                }
            }
            parts.new_duplicated_lines +=
                u64::from(newcode::intersect_count(&dup_ranges[i], &ranges));
            parts.new_lines_total += ranges
                .iter()
                .map(|(s, e)| u64::from(e - s + 1))
                .sum::<u64>();
            if !ranges.is_empty() {
                parts.new_lines.insert(rec.path.clone(), ranges);
            }
        }

        // Analyze base blobs at their base path (a moved file's base is what a scan of the base
        // reported for it: path-dependent rules and settings follow the path), cache-aware.
        let base_entries: Vec<MapInput> = base_needed
            .iter()
            .map(|(i, blob, base_path, _)| {
                (
                    SourceEntry {
                        path: base_path.clone(),
                        blob_id: Some(blob.clone()),
                        size: None,
                        location: EntryLocation::Object,
                    },
                    records[*i].plugin,
                    None,
                )
            })
            .collect();
        let base = self.map(base_entries, &BaseLoader(loader), reader, lookups);
        // Deleted files the scan would have covered: their findings are fixed (unless moved).
        let deleted_entries: Vec<MapInput> = diff
            .deleted
            .iter()
            .filter(|d| filter.matches(&d.path))
            .filter_map(|d| {
                let plugin = self.registry.plugin_for_path(&d.path)?;
                Some((
                    SourceEntry {
                        path: d.path.clone(),
                        blob_id: Some(d.blob.clone()?),
                        size: None,
                        location: EntryLocation::Object,
                    },
                    plugin,
                    None,
                ))
            })
            .collect();
        let mut deleted = self.map(deleted_entries, &BaseLoader(loader), reader, lookups);
        // Base findings no head finding of their file took: fixed, unless their code moved (see
        // `newcode::pair_moved`).
        let mut gone: Vec<Gone> = Vec::new();
        for (k, r) in deleted.records.iter().enumerate() {
            for f in self.base_findings(r, triaged) {
                let at = r.result.as_ref().and_then(|res| move_hashes(res, &f));
                gone.push((records.len() + k, f, at));
            }
        }
        let mut base_out = BaseOutput {
            analyzed: 0,
            hits: 0,
            writes: base.writes,
            used: base.used,
            diagnostics: notes,
        };
        base_out.diagnostics.extend(base.diagnostics);
        base_out.writes.append(&mut deleted.writes);
        base_out.used.append(&mut deleted.used);
        base_out.diagnostics.append(&mut deleted.diagnostics);
        for r in base.records.iter().chain(&deleted.records) {
            match r.status {
                FileStatus::Analyzed => base_out.analyzed += 1,
                FileStatus::Cached => base_out.hits += 1,
                FileStatus::Skipped => {}
            }
        }
        let base_by_path: FxHashMap<&str, &Record> =
            base.records.iter().map(|r| (r.path.as_str(), r)).collect();
        let dup_rule: FxHashMap<usize, &str> = records
            .iter()
            .map(|r| {
                (
                    r.plugin,
                    self.registry
                        .generic_set(r.plugin)
                        .id(GenericRule::DuplicatedBlock),
                )
            })
            .collect();
        for (i, _, base_path, old_lines) in &base_needed {
            let rec = &mut records[*i];
            let ranges = parts.new_lines.get(&rec.path).cloned().unwrap_or_default();
            let dup_id = dup_rule.get(&rec.plugin).copied().unwrap_or("");
            let (base_findings, base_result) = match base_by_path.get(base_path.as_str()) {
                Some(b) => (self.base_findings(b, triaged), b.result.as_ref()),
                None => (Vec::new(), None),
            };
            let Some(res) = rec.result.as_mut() else {
                continue;
            };
            let (mut comparable, mut dups): (Vec<Finding>, Vec<Finding>) =
                std::mem::take(&mut res.findings)
                    .into_iter()
                    .partition(|f| f.rule_id != dup_id);
            let head_anchors: Vec<Option<u64>> = comparable
                .iter()
                .map(|f| res.anchor_of(f.range.start_line))
                .collect();
            let base_anchors: Vec<Option<u64>> = base_findings
                .iter()
                .map(|f| base_result.and_then(|r| r.anchor_of(f.range.start_line)))
                .collect();
            let unpaired = newcode::classify(
                &mut comparable,
                newcode::Side {
                    anchors: &head_anchors,
                    changed: &ranges,
                },
                &base_findings,
                newcode::Side {
                    anchors: &base_anchors,
                    changed: old_lines,
                },
            );
            let mut base_findings: Vec<Option<Finding>> =
                base_findings.into_iter().map(Some).collect();
            for j in unpaired {
                let Some(f) = base_findings[j].take() else {
                    continue;
                };
                // Only code on a removed or changed line can have moved.
                let removed = newcode::overlaps(old_lines, f.range.start_line, f.range.start_line);
                let at = base_result
                    .filter(|_| removed)
                    .and_then(|r| move_hashes(r, &f));
                gone.push((*i, f, at));
            }
            for f in &mut dups {
                f.status = if newcode::overlaps(&ranges, f.range.start_line, f.range.end_line) {
                    FindingStatus::New
                } else {
                    FindingStatus::Unchanged
                };
            }
            comparable.extend(dups);
            comparable.sort_by(finding_order);
            res.findings = comparable;
        }

        // MOVED CODE: new findings on added or changed lines (of any changed file, added ones
        // included) whose code the change removed elsewhere, in the same file or another one.
        let mut head_at: Vec<(usize, usize)> = Vec::new();
        let paired = {
            let mut heads = Vec::new();
            for (i, rec) in records.iter().enumerate() {
                let (Some(res), Some(ranges)) =
                    (rec.result.as_ref(), parts.new_lines.get(&rec.path))
                else {
                    continue;
                };
                let dup_id = dup_rule.get(&rec.plugin).copied().unwrap_or("");
                for (k, f) in res.findings.iter().enumerate() {
                    if f.status != FindingStatus::New
                        || f.rule_id == dup_id
                        || !newcode::overlaps(ranges, f.range.start_line, f.range.start_line)
                    {
                        continue;
                    }
                    if let Some((anchor, context)) = move_hashes(res, f) {
                        heads.push(newcode::MoveSite {
                            file: i,
                            finding: f,
                            anchor,
                            context,
                        });
                        head_at.push((i, k));
                    }
                }
            }
            let gone_sites: Vec<newcode::MoveSite<'_>> = gone
                .iter()
                .filter_map(|(file, f, at)| {
                    let (anchor, context) = (*at)?;
                    Some(newcode::MoveSite {
                        file: *file,
                        finding: f,
                        anchor,
                        context,
                    })
                })
                .collect();
            // Indices into `gone_sites` → indices into `gone`.
            let gone_index: Vec<usize> = gone
                .iter()
                .enumerate()
                .filter(|(_, g)| g.2.is_some())
                .map(|(j, _)| j)
                .collect();
            newcode::pair_moved(&heads, &gone_sites)
                .into_iter()
                .map(|p| p.map(|s| gone_index[s]))
                .collect::<Vec<Option<usize>>>()
        };
        let mut moved = vec![false; gone.len()];
        for ((i, k), partner) in head_at.into_iter().zip(paired) {
            let Some(j) = partner else { continue };
            moved[j] = true;
            if let Some(f) = records[i]
                .result
                .as_mut()
                .and_then(|r| r.findings.get_mut(k))
            {
                f.status = newcode::status_against(f, &gone[j].1);
            }
        }
        let fixed = newcode::fixed_by_severity(
            gone.iter()
                .zip(&moved)
                .filter(|(_, &m)| !m)
                .map(|((_, f, _), _)| f),
        );
        for (sev, n) in fixed {
            parts.fixed.add(sev, n);
        }
        (parts, base_out)
    }

    /// Findings of a base-side record, including its unused suppressions (which head files get
    /// after the reduce phase). Findings whose fingerprint a triage entry names (`triaged`:
    /// fingerprint → status) count as suppressed, so they are not "fixed" when they go away.
    fn base_findings(&self, b: &Record, triaged: &FxHashMap<&str, &str>) -> Vec<Finding> {
        let (Some(br), Some(rules)) = (&b.result, &b.rules) else {
            return Vec::new();
        };
        let mut out = br.findings.clone();
        let unused: Vec<Pending> = suppress::unused_findings(&br.suppressions, &rules.rules)
            .into_iter()
            .map(|(raw, anchor)| Pending {
                raw,
                anchor: anchor.into(),
                suppressed: None,
            })
            .collect();
        out.extend(analyze::finalize(
            &self.registry,
            &rules.rules,
            &b.path,
            unused,
        ));
        if !triaged.is_empty() {
            for f in out.iter_mut().filter(|f| f.suppressed.is_none()) {
                if let Some(kind) = triaged.get(f.fingerprint.as_str()) {
                    f.suppressed = Some(SuppressionInfo {
                        kind: (*kind).to_string(),
                        justification: None,
                    });
                }
            }
        }
        out
    }

    /// The triage file of this scan: `opts.triage_file` from disk, else `triage.file` below the
    /// root (from the scanned commit when scanning a revision). `None` when there is none.
    fn load_triage(
        &self,
        opts: &ScanOptions,
        info: &SourceInfo,
    ) -> Result<Option<LoadedTriage>, EngineError> {
        let (text, rel) = match &opts.triage_file {
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(|e| {
                    EngineError::Invalid(format!("cannot read triage file {}: {e}", p.display()))
                })?;
                (text, display_path(&self.root, p))
            }
            None => {
                let rel = self
                    .config()
                    .triage
                    .file
                    .trim()
                    .replace('\\', "/")
                    .trim_start_matches("./")
                    .to_string();
                let bytes = match (info.kind.as_str(), &info.commit) {
                    ("rev", Some(commit)) => Git::open(&self.root)
                        .and_then(|g| g.show_file(commit, &rel))
                        .map_err(EngineError::Git)?,
                    _ => match std::fs::read(self.root.join(&rel)) {
                        Ok(b) => Some(b),
                        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
                        Err(e) => {
                            return Err(EngineError::Invalid(format!(
                                "cannot read triage file {rel}: {e}"
                            )))
                        }
                    },
                };
                let Some(bytes) = bytes else {
                    return Ok(None);
                };
                (String::from_utf8_lossy(&bytes).into_owned(), rel)
            }
        };
        let file = TriageFile::parse(&text)
            .map_err(|e| EngineError::Invalid(format!("invalid triage file {rel}: {e}")))?;
        Ok(Some(LoadedTriage { file, rel }))
    }

    /// Mark the findings that triage entries match as suppressed (unless already suppressed in
    /// the source), and build the report and the findings for stale entries (with each stale
    /// entry's path). Entries match on every finding of the scan; stale are those whose file the
    /// scan analyzed, or — in a full scan — whose file is gone.
    fn apply_triage(
        &self,
        t: &LoadedTriage,
        records: &mut [Record],
        all_paths: &[String],
        full: bool,
    ) -> (TriageReport, Vec<(Finding, String)>) {
        let entries = &t.file.entries;
        let (matches, positions, stale_idx) = {
            let mut positions: Vec<(usize, usize)> = Vec::new();
            let mut view: Vec<&Finding> = Vec::new();
            let mut analyzed: FxHashSet<&str> = FxHashSet::default();
            for (ri, rec) in records.iter().enumerate() {
                if let Some(res) = &rec.result {
                    analyzed.insert(rec.path.as_str());
                    for (fi, f) in res.findings.iter().enumerate() {
                        positions.push((ri, fi));
                        view.push(f);
                    }
                }
            }
            let present: FxHashSet<&str> = all_paths.iter().map(String::as_str).collect();
            // A partial scan does not know which files are gone: no rename matching there.
            let exists = |p: &str| !full || present.contains(p);
            let matches = triage::match_entries(entries, &view, &exists);
            let mut used = vec![false; entries.len()];
            for e in matches.iter().flatten() {
                used[*e] = true;
            }
            let stale: Vec<usize> = (0..entries.len())
                .filter(|&e| {
                    let p = entries[e].path.as_str();
                    !used[e] && (analyzed.contains(p) || (full && !present.contains(p)))
                })
                .collect();
            (matches, positions, stale)
        };
        let mut matched = 0u64;
        for ((ri, fi), m) in positions.into_iter().zip(matches) {
            let Some(e) = m else { continue };
            matched += 1;
            let entry = &entries[e];
            if let Some(f) = records[ri]
                .result
                .as_mut()
                .and_then(|r| r.findings.get_mut(fi))
            {
                if f.suppressed.is_none() {
                    f.suppressed = Some(SuppressionInfo {
                        kind: entry.status.as_str().to_string(),
                        justification: Some(entry.reason.clone()),
                    });
                }
            }
        }
        let mut stale_keys = Vec::new();
        let mut stale_findings = Vec::new();
        for e in stale_idx {
            let entry = &entries[e];
            stale_keys.push(entry.key());
            let plugin = self.registry.plugin_for_path(&entry.path).or((!self
                .registry
                .plugins()
                .is_empty())
            .then_some(0));
            let Some(plugin) = plugin else { continue };
            let rules = self.compiled.resolve(&self.registry, plugin, false, &t.rel);
            let line = entry.line.max(1);
            let pending = Pending {
                raw: RawFinding {
                    rule_id: MetaRule::UnusedSuppression.id().to_string(),
                    message: triage::stale_message(entry),
                    range: TextRange::line(line),
                    symbol: None,
                    value: None,
                    secondary: Vec::new(),
                },
                anchor: triage::stale_anchor(entry).into(),
                suppressed: None,
            };
            for f in analyze::finalize(&self.registry, &rules.rules, &t.rel, vec![pending]) {
                stale_findings.push((f, entry.path.clone()));
            }
        }
        stale_keys.sort();
        stale_findings.sort_by(|a, b| finding_order(&a.0, &b.0));
        (
            TriageReport {
                file: t.rel.clone(),
                entries: entries.len() as u64,
                matched,
                stale: stale_keys,
            },
            stale_findings,
        )
    }

    fn load_coverage(
        &self,
        opts: &ScanOptions,
        all_paths: &[String],
        diags: &mut Vec<Diagnostic>,
    ) -> Option<RepoCoverage> {
        let globs = &self.compiled.config().coverage.reports;
        if globs.is_empty() && opts.coverage_reports.is_empty() {
            return None;
        }
        let files = coverage::find_reports(&self.root, globs, &opts.coverage_reports);
        if files.is_empty() {
            diags.push(Diagnostic::warning(None, "no coverage report found"));
            return None;
        }
        let root = &self.root;
        let (report, mut d) = coverage::load_reports(&files, |f| {
            f.strip_prefix(root)
                .unwrap_or(f)
                .to_string_lossy()
                .replace('\\', "/")
        });
        diags.append(&mut d);
        let refs: Vec<&str> = all_paths.iter().map(String::as_str).collect();
        let (mapped, mut d) = coverage::map_reports(report.unwrap_or_default(), &refs);
        diags.append(&mut d);
        Some(mapped)
    }

    /// Estimated effort of `f` in whole minutes: the rule's remediation time, scaled for threshold
    /// rules by the finding's value against the limit in force for the file (`rules`, else the
    /// parameter's default). See [`measures`](crate::measures).
    fn effort_of(&self, f: &Finding, rules: Option<&ResolvedRules>) -> u64 {
        let Some(entry) = self.registry.rule(&f.rule_id) else {
            return 0;
        };
        let meta = entry.meta;
        let limit = f.value.and_then(|_| {
            let p = meta
                .params
                .iter()
                .find(|p| THRESHOLD_PARAMS.contains(&p.name))?;
            rules
                .and_then(|r| r.rules.param(&f.rule_id, p.name))
                .and_then(ParamValue::as_f64)
                .or_else(|| p.default.to_value().as_f64())
        });
        finding_effort(meta.remediation_minutes, f.value, limit)
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        &self,
        records: Vec<Record>,
        extra: Vec<Finding>,
        duplications: Vec<DuplicationBlock>,
        coverage: Option<RepoCoverage>,
        new_code: Option<(String, String, NewCodeParts)>,
        mut diagnostics: Vec<Diagnostic>,
        source: SourceInfo,
        stats_in: Stats,
    ) -> ScanResult {
        let mut findings: Vec<Finding> = Vec::new();
        let mut files = Vec::with_capacity(records.len());
        let mut total = Measures::default();
        let (mut main, mut test) = (Measures::default(), Measures::default());
        let mut directories = DirectoryRollup::new(self.compiled.config().metrics.depth);
        let (mut new_effort, mut new_reliability) =
            (EffortMinutes::default(), SeverityCounts::default());
        let mut languages: BTreeMap<&'static str, Measures> = BTreeMap::new();
        let mut lang_stats: BTreeMap<&'static str, LanguageStats> = BTreeMap::new();
        let mut enabled: FxHashSet<&'static str> = FxHashSet::default();
        let mut seen_sets: FxHashSet<*const ResolvedRules> = FxHashSet::default();
        let mut stats = ScanStats {
            files_seen: stats_in.files_seen,
            files_matched: stats_in.matched,
            base_files_analyzed: stats_in.base_stats.0,
            base_cache_hits: stats_in.base_stats.1,
            threads: rayon::current_num_threads(),
            cache_enabled: stats_in.cache_path.is_some(),
            cache_path: stats_in.cache_path,
            ..Default::default()
        };
        // Records are sorted by path, and a file's findings all carry its path: once each file's
        // findings are in order, so is their concatenation.
        let mut ordered = true;
        for rec in records {
            ordered &= files.last().is_none_or(|f: &FileSummary| f.path < rec.path);
            let language = self.registry.plugin(rec.plugin).id();
            if let Some(r) = &rec.rules {
                if seen_sets.insert(Arc::as_ptr(r)) {
                    enabled.extend(r.enabled.iter().copied());
                }
            }
            match rec.status {
                FileStatus::Analyzed => {
                    stats.files_analyzed += 1;
                    let ls = lang_stats.entry(language).or_default();
                    ls.files += 1;
                    ls.bytes += rec.fresh_bytes;
                    ls.analyze_ms += rec.analyze_ns as f64 / 1e6;
                }
                FileStatus::Cached => stats.cache_hits += 1,
                FileStatus::Skipped => {
                    stats.files_skipped += 1;
                    if let Some(reason) = rec.skip {
                        *stats
                            .skipped_by_reason
                            .entry(reason.as_str().to_string())
                            .or_default() += 1;
                    }
                }
            }
            let mut n_findings = 0u32;
            let mut file_effort = 0u64;
            let measures = rec.result.as_ref().map(|r| r.measures.clone());
            if let Some(mut res) = rec.result {
                if !res
                    .findings
                    .is_sorted_by(|a, b| finding_order(a, b) != std::cmp::Ordering::Greater)
                {
                    res.findings.sort_by(finding_order);
                }
                let lm = languages.entry(language).or_default();
                let side = if rec.is_test { &mut test } else { &mut main };
                lm.add_file(&res.measures);
                total.add_file(&res.measures);
                side.add_file(&res.measures);
                let mut counts = SeverityCounts::default();
                for f in &res.findings {
                    // Effort and grades count what can gate (see `measures`).
                    let effort = gate::gating(f).then(|| self.effort_of(f, rec.rules.as_deref()));
                    for m in [&mut total, &mut *lm, &mut *side] {
                        m.add_finding(f, effort);
                    }
                    if f.suppressed.is_none() {
                        n_findings += 1;
                        counts.add(f.severity, 1);
                    }
                    if let Some(e) = effort {
                        file_effort += e;
                        if f.status == FindingStatus::New {
                            new_effort.add(f.severity, f.quality, e);
                            if f.quality == Quality::Reliability {
                                new_reliability.add(f.severity, 1);
                            }
                        }
                    }
                }
                directories.add_file(&rec.path, &res.measures, &counts, file_effort);
                findings.extend(res.findings);
            }
            let change = new_code
                .as_ref()
                .and_then(|(_, _, p)| p.change_of(&rec.path));
            files.push(FileSummary {
                path: rec.path,
                language: language.to_string(),
                status: rec.status,
                skip_reason: rec.skip,
                is_test: rec.is_test,
                measures,
                findings: n_findings,
                effort_minutes: file_effort,
                change,
            });
        }
        // Findings on files that are not source files (stale triage entries): project totals.
        if !extra.is_empty() {
            ordered = false;
            for f in &extra {
                let effort = gate::gating(f).then(|| self.effort_of(f, None));
                total.add_finding(f, effort);
                main.add_finding(f, effort);
                if let (Some(e), FindingStatus::New) = (effort, f.status) {
                    new_effort.add(f.severity, f.quality, e);
                    if f.quality == Quality::Reliability {
                        new_reliability.add(f.severity, 1);
                    }
                }
            }
            findings.extend(extra);
        }
        total.finish();
        main.finish();
        test.finish();
        let (directories, directories_omitted) = directories.finish(MAX_DIRECTORIES);
        for m in languages.values_mut() {
            m.finish();
        }
        if !ordered {
            findings.sort_by(finding_order);
            files.sort_by(|a, b| a.path.cmp(&b.path));
        }
        debug_assert!(
            findings.is_sorted_by(|a, b| finding_order(a, b) != std::cmp::Ordering::Greater)
        );
        let languages: BTreeMap<String, Measures> = languages
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        let lang_stats: BTreeMap<String, LanguageStats> = lang_stats
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();

        // New code
        let new_code_result = new_code.map(|(base, merge_base, parts)| {
            let mut m = NewCodeMeasures {
                changed_files: parts.changed_files,
                new_lines: parts.new_lines_total,
                new_duplicated_lines: parts.new_duplicated_lines,
                new_duplication_pct: percent(parts.new_duplicated_lines, parts.new_lines_total),
                fixed_findings: parts.fixed.clone(),
                new_effort_minutes: new_effort,
                new_reliability_findings: new_reliability,
                ..Default::default()
            };
            for f in findings.iter().filter(|f| f.suppressed.is_none()) {
                match f.status {
                    FindingStatus::New => m.new_findings.add(f.severity, 1),
                    FindingStatus::Worsened => m.worsened_findings.add(f.severity, 1),
                    _ => {}
                }
            }
            NewCodeResult {
                base,
                merge_base,
                new_lines: parts.new_lines,
                renamed: parts.renamed,
                measures: m,
                baseline: parts.baseline,
            }
        });

        for f in &findings {
            enabled.insert(
                self.registry
                    .rule(&f.rule_id)
                    .map(|e| e.meta.id)
                    .unwrap_or(""),
            );
        }
        let mut ids: Vec<&str> = enabled.into_iter().filter(|s| !s.is_empty()).collect();
        ids.sort_unstable();
        let rules: Vec<RuleInfo> = ids
            .iter()
            .filter_map(|id| self.registry.rule_info(id))
            .collect();
        diagnostics.sort_by(|a, b| (&a.path, &a.message).cmp(&(&b.path, &b.message)));
        diagnostics.dedup();

        let mut result = ScanResult {
            format_version: crate::result::RESULTS_FORMAT_VERSION,
            engine_version: ENGINE_VERSION.to_string(),
            source,
            findings,
            files,
            duplications,
            grades: None,
            measures: ProjectMeasures {
                total,
                languages,
                coverage: None,
                main,
                test,
                directories,
                directories_omitted,
            },
            new_code: new_code_result,
            gate: gate::GateResult {
                passed: true,
                conditions: Vec::new(),
            },
            rules,
            diagnostics,
            triage: None,
            stats: ScanStats {
                languages: lang_stats,
                ..stats
            },
            retained: None,
        };
        // Coverage and the gate: the same functions `mote gate` uses on a saved result.
        if let Some(c) = &coverage {
            gate::apply_coverage(&mut result, c);
        }
        gate::reevaluate_gate(&mut result, self.compiled.config());
        result
    }
}

/// A triage file read for a scan; `rel` is its path relative to the scan root when it is below
/// it (else as given), with `/` separators.
struct LoadedTriage {
    file: TriageFile,
    rel: String,
}

/// `path` relative to `root` when it is below it (both resolved), else `path` itself; `/`
/// separators.
fn display_path(root: &Path, path: &Path) -> String {
    let below = std::fs::canonicalize(path)
        .ok()
        .zip(std::fs::canonicalize(root).ok())
        .and_then(|(p, r)| {
            p.strip_prefix(&r)
                .ok()
                .map(|x| x.to_string_lossy().into_owned())
        });
    below
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
        .replace('\\', "/")
}

/// Warning for a requested file that a full scan would not see.
fn not_discovered(rel: &str) -> String {
    format!(
        "{rel}: not analyzed; a scan does not include it (ignored by .gitignore, or below a \
         nested repository or a linked directory)"
    )
}

/// Every path filter must match at least one discovered file: a filter that matches nothing is
/// almost always a typo or a path outside the scanned tree, and would silently scan nothing.
fn check_filters_match(patterns: &[String], entries: &[SourceEntry]) -> Result<(), EngineError> {
    if patterns.is_empty() {
        return Ok(());
    }
    let mut unmatched = Vec::new();
    for p in patterns {
        let f = PathFilter::new(std::slice::from_ref(p)).map_err(EngineError::Invalid)?;
        if !entries.iter().any(|e| f.matches(&e.path)) {
            unmatched.push(format!("`{p}`"));
        }
    }
    if unmatched.is_empty() {
        return Ok(());
    }
    let s = if unmatched.len() == 1 { "" } else { "s" };
    Err(EngineError::Invalid(format!(
        "path filter{s} {} match{} no file in the scanned source (paths are relative to the \
         scanned root; check for typos)",
        unmatched.join(", "),
        if unmatched.len() == 1 { "es" } else { "" }
    )))
}

/// Delete a damaged cache (the slot reopens a fresh one on the next scan).
fn discard_cache(slot: &mut CacheSlot, diags: &mut Vec<Diagnostic>) {
    let Some(c) = slot.cache.take() else { return };
    let path = c.path().to_string_lossy().replace('\\', "/");
    let msg = match c.destroy() {
        Ok(()) => format!(
            "the cache {path} was damaged; it was deleted and is rebuilt (results were \
             recomputed, not read from it)"
        ),
        Err(e) => format!("the cache {path} is damaged and could not be deleted ({e})"),
    };
    diags.push(Diagnostic::warning(None, msg));
}

/// Stack reserved for every analysis thread. Parsers recurse on nested input; the reservation is
/// virtual memory, committed only as it is used.
pub const ANALYSIS_STACK_SIZE: usize = 256 << 20;

/// The analysis threads' stack size ([`ANALYSIS_STACK_SIZE`]), for hosts to declare to plugins
/// that size their nesting limits from it. Every analysis pool thread gets exactly this much:
/// when the reservation fails, `build_pool` uses fewer threads, never smaller stacks.
pub const ANALYSIS_STACK_BYTES: usize = ANALYSIS_STACK_SIZE;

fn build_pool(threads: usize) -> Option<rayon::ThreadPool> {
    let build = |n: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .stack_size(ANALYSIS_STACK_SIZE)
            .thread_name(|i| format!("mote-{i}"))
            .build()
    };
    // Address space can be short (32-bit targets): trade threads for stack, never the reverse —
    // plugins size their nesting limits from ANALYSIS_STACK_BYTES.
    let mut n = if threads == 0 {
        std::thread::available_parallelism().map_or(1, |p| p.get())
    } else {
        threads
    };
    loop {
        match build(n) {
            Ok(pool) => return Some(pool),
            Err(_) if n > 1 => n /= 2,
            Err(_) => return None,
        }
    }
}

/// The pool analysis runs on: a shared one per thread count (0 = one thread per core).
fn analysis_pool(threads: Option<usize>) -> Option<Arc<rayon::ThreadPool>> {
    static POOLS: Mutex<Vec<(usize, Arc<rayon::ThreadPool>)>> = Mutex::new(Vec::new());
    let n = threads.unwrap_or(0);
    let mut pools = POOLS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some((_, p)) = pools.iter().find(|(k, _)| *k == n) {
        return Some(p.clone());
    }
    let pool = Arc::new(build_pool(n)?);
    // Keep a few distinct sizes; callers rarely use more than one.
    if pools.len() >= 4 {
        pools.remove(0);
    }
    pools.push((n, pool.clone()));
    Some(pool)
}

/// Another copy of a duplicated block: (record index, start line, end line). Records are sorted
/// by path, so ordering by record index orders by path.
type Other = (u32, u32, u32);

/// One duplicated-block occurrence in a file, with the other copies.
#[derive(Clone)]
struct DupOcc {
    start: u32,
    end: u32,
    /// Content hash of the largest merged block (fingerprint anchor).
    hash: u64,
    tokens: u32,
    others: Vec<Other>,
    /// Distinct other places (before truncating `others`).
    n_others: u32,
}

/// Most secondary locations listed on one duplicated-block finding.
const MAX_SECONDARY: usize = 10;

/// Merge a file's overlapping duplicate occurrences (nested or overlapping matches of different
/// lengths) into one finding each; other places are merged per path and de-duplicated.
fn merge_occurrences(own: u32, mut occs: Vec<DupOcc>) -> Vec<DupOcc> {
    occs.sort_by(|a, b| {
        (
            a.start,
            std::cmp::Reverse(a.end),
            std::cmp::Reverse(a.tokens),
            a.hash,
        )
            .cmp(&(
                b.start,
                std::cmp::Reverse(b.end),
                std::cmp::Reverse(b.tokens),
                b.hash,
            ))
    });
    let mut out: Vec<DupOcc> = Vec::with_capacity(occs.len());
    for o in occs {
        match out.last_mut() {
            Some(last) if o.start <= last.end => {
                last.end = last.end.max(o.end);
                if (o.tokens, std::cmp::Reverse(o.hash))
                    > (last.tokens, std::cmp::Reverse(last.hash))
                {
                    last.tokens = o.tokens;
                    last.hash = o.hash;
                }
                last.others.extend(o.others);
            }
            _ => out.push(o),
        }
    }
    for g in &mut out {
        let mut others = std::mem::take(&mut g.others);
        others.retain(|&(file, s, e)| !(file == own && s <= g.end && e >= g.start));
        others.sort_unstable();
        let mut merged: Vec<Other> = Vec::with_capacity(others.len());
        for o in others {
            match merged.last_mut() {
                Some(m) if m.0 == o.0 && o.1 <= m.2 => m.2 = m.2.max(o.2),
                _ => merged.push(o),
            }
        }
        g.n_others = merged.len() as u32;
        merged.truncate(MAX_SECONDARY);
        g.others = merged;
    }
    out.retain(|g| g.n_others > 0);
    out
}

struct Stats {
    files_seen: u64,
    matched: u64,
    base_stats: (u64, u64),
    cache_path: Option<String>,
}

#[derive(Default)]
struct NewCodeParts {
    changed_files: u64,
    new_lines: BTreeMap<String, Vec<(u32, u32)>>,
    new_lines_total: u64,
    new_duplicated_lines: u64,
    renamed: BTreeMap<String, String>,
    fixed: SeverityCounts,
    changes: BTreeMap<String, ChangeKind>,
    /// The comparison was with a baseline file.
    baseline: Option<BaselineInfo>,
}

impl NewCodeParts {
    fn change_of(&self, path: &str) -> Option<ChangeKind> {
        self.changes.get(path).copied()
    }
}

struct BaseOutput {
    writes: Vec<(CacheKey, Vec<u8>)>,
    used: Vec<CacheKey>,
    diagnostics: Vec<Diagnostic>,
    analyzed: u64,
    hits: u64,
}

/// A head record needing a base analysis: (record index, base blob, base path, touched base
/// lines).
type BaseNeed = (usize, String, String, Vec<(u32, u32)>);
/// A base finding no head finding of its file took: `(file key, finding, its anchor and context
/// hashes when it sat on a removed line)`.
type Gone = (usize, Finding, Option<(u64, u64)>);

/// Base blobs are always read from the object database.
struct BaseLoader<'a>(&'a dyn BlobLoader);

impl BlobLoader for BaseLoader<'_> {
    fn load(&self, entry: &SourceEntry) -> io::Result<Vec<u8>> {
        let id = entry
            .blob_id
            .as_deref()
            .ok_or_else(|| io::Error::other("no blob id"))?;
        self.0
            .load_blob(id)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("missing blob {id}")))
    }
    fn load_blob(&self, blob_id: &str) -> io::Result<Option<Vec<u8>>> {
        self.0.load_blob(blob_id)
    }
}

fn empty_rules() -> Arc<ResolvedRules> {
    Arc::new(ResolvedRules {
        rules: mote_core::RuleSet::default(),
        hash: 0,
        enabled: Vec::new(),
    })
}

fn apply_outcome(rec: &mut Record, outcome: FileOutcome, status: FileStatus) {
    match outcome {
        FileOutcome::Analyzed(r) => {
            rec.status = status;
            rec.result = Some(*r);
        }
        FileOutcome::Skipped(reason) => {
            rec.status = FileStatus::Skipped;
            rec.skip = Some(reason);
        }
    }
}

/// Anchor and context hashes of a finding's start line in its file's result (moved-code
/// matching); `None` when the result does not know the line.
fn move_hashes(res: &FileResult, f: &Finding) -> Option<(u64, u64)> {
    let line = f.range.start_line;
    Some((res.anchor_of(line)?, res.context_of(line)?))
}

fn finding_order(a: &Finding, b: &Finding) -> std::cmp::Ordering {
    a.sort_key()
        .cmp(&b.sort_key())
        .then_with(|| a.fingerprint.cmp(&b.fingerprint))
}

fn ms(t: Instant) -> f64 {
    (t.elapsed().as_secs_f64() * 1e6).round() / 1e3
}

#[cfg(test)]
mod tests {
    use super::*;

    // files: a = 0, b = 1, c = 2, d = 3
    fn occ(start: u32, end: u32, tokens: u32, hash: u64, others: &[Other]) -> DupOcc {
        DupOcc {
            start,
            end,
            hash,
            tokens,
            others: others.to_vec(),
            n_others: 0,
        }
    }

    #[test]
    fn overlapping_occurrences_become_one_finding() {
        let merged = merge_occurrences(
            0,
            vec![
                occ(62, 86, 200, 1, &[(1, 10, 34)]),
                occ(62, 85, 190, 2, &[(1, 10, 33), (2, 5, 28)]),
                occ(100, 120, 150, 3, &[(0, 119, 140), (3, 1, 21)]),
            ],
        );
        assert_eq!(merged.len(), 2);
        let g = &merged[0];
        assert_eq!((g.start, g.end, g.hash, g.n_others), (62, 86, 1, 2));
        assert_eq!(g.others, vec![(1, 10, 34), (2, 5, 28)]);
        // an "other" copy overlapping the finding itself (same file) is not listed
        assert_eq!(merged[1].n_others, 1);
        assert_eq!(merged[1].others[0].0, 3);
    }
}
