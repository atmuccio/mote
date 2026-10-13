//! Per-file result cache in a single `redb` file.
//!
//! Keys are 128-bit hashes of everything that determines a file's map-phase output (engine
//! version, plugin id and version, effective rule-set hash, git blob id, test flag, analysis
//! settings, path). Values are postcard-encoded [`FileOutcome`]s. Reads happen in parallel from
//! one read transaction during the map phase; all misses are written in one write transaction
//! afterwards. Any cache problem (lock held by another process, corruption, I/O error) degrades
//! to running without a cache and is reported as a warning.
//!
//! Corruption is contained: every value carries an xxh3 checksum (a mismatch is a miss), and
//! every database call runs under `catch_unwind` — redb may panic on a damaged file. A failure
//! marks the cache [corrupt](Cache::is_corrupt); lookups then miss, and the scanner deletes and
//! recreates the file ([`Cache::destroy`]) instead of failing the scan.

use crate::analyze::FileOutcome;
use crate::config::ENGINE_VERSION;
use redb::backends::FileBackend;
use redb::{
    BackendError, Database, DatabaseError, ReadableDatabase, ReadableTableMetadata, StorageBackend,
    TableDefinition,
};
use rustc_hash::FxHashSet;
use std::io;
use std::ops::Bound;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use xxhash_rust::xxh3::Xxh3;

/// Bump when the cached value format or map-phase semantics change (11: findings carry an
/// optional fix, always encoded; 10: the project is renamed
/// Mote, so cached findings carry `mote/…` rule ids and `mote-ignore` suppression kinds;
/// 9: JavaScript test-suite callbacks are containers, not functions, and the tests in them are;
/// 8: fingerprint anchors of lines longer than 1 KiB are capped; 7: a library banner naming
/// the project's own npm package is not a vendored signal, `skip_generated_except`; 6: vendored
/// JavaScript is skipped (`SkipReason::Vendored`), compatibility suppressions per source; 5: line
/// anchors carry a context hash for moved-code matching; 4: the `generated code … do not edit`
/// header marker).
pub const ENGINE_CACHE_VERSION: u32 = 11;

/// Cache file name inside the cache directory.
pub const CACHE_FILE: &str = "cache.redb";

const TABLE: TableDefinition<&[u8; 16], &[u8]> = TableDefinition::new("files");

/// A cache key.
pub type CacheKey = [u8; 16];

/// Inputs of a cache key.
pub struct KeyParts<'a> {
    pub plugin_id: &'a str,
    pub plugin_version: u32,
    pub rules_hash: u64,
    pub blob_id: &'a str,
    pub is_test: bool,
    pub settings_hash: u64,
    pub path: &'a str,
}

/// Compute a cache key.
pub fn cache_key(p: &KeyParts<'_>) -> CacheKey {
    let mut h = Xxh3::new();
    h.update(&ENGINE_CACHE_VERSION.to_le_bytes());
    h.update(ENGINE_VERSION.as_bytes());
    h.update(&[0]);
    h.update(p.plugin_id.as_bytes());
    h.update(&[0]);
    h.update(&p.plugin_version.to_le_bytes());
    h.update(&p.rules_hash.to_le_bytes());
    h.update(p.blob_id.as_bytes());
    h.update(&[0, u8::from(p.is_test)]);
    h.update(&p.settings_hash.to_le_bytes());
    h.update(p.path.as_bytes());
    h.digest128().to_le_bytes()
}

/// Checksum of the entry stored under `key`: the key and the payload, so that a value is only
/// ever accepted for the key it was written for (a torn page could otherwise pair a key with
/// another entry's intact value).
fn entry_checksum(key: &CacheKey, payload: &[u8]) -> u64 {
    let mut h = Xxh3::new();
    h.update(key);
    h.update(payload);
    h.digest()
}

/// Encode the outcome stored under `key`: an 8-byte little-endian xxh3 checksum of the key and
/// the postcard payload, then the payload.
pub fn encode(key: &CacheKey, outcome: &FileOutcome) -> Option<Vec<u8>> {
    let payload = postcard::to_allocvec(outcome).ok()?;
    let mut out = Vec::with_capacity(payload.len() + 8);
    out.extend_from_slice(&entry_checksum(key, &payload).to_le_bytes());
    out.extend_from_slice(&payload);
    Some(out)
}

/// Decode the outcome stored under `key` (`None` if the bytes are not a valid entry for that key
/// or fail their checksum).
pub fn decode(key: &CacheKey, bytes: &[u8]) -> Option<FileOutcome> {
    if bytes.len() < 8 {
        return None;
    }
    let (sum, payload) = bytes.split_at(8);
    if u64::from_le_bytes(sum.try_into().ok()?) != entry_checksum(key, payload) {
        return None;
    }
    postcard::from_bytes(payload).ok()
}

fn panic_text(p: &(dyn std::any::Any + Send)) -> String {
    p.downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_string())
}

thread_local! {
    /// Set while a guarded database call runs: its panics are expected and handled, so the
    /// panic hook stays quiet for them.
    static QUIET_PANICS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Wrap the process panic hook once so that panics inside [`guarded`] calls print nothing (a
/// damaged cache is reported as one warning, not as a panic message per thread).
fn install_quiet_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !QUIET_PANICS.with(|q| q.get()) {
                previous(info);
            }
        }));
    });
}

/// Whether a database failure means a damaged file (a panic in redb, or corruption it detected),
/// as opposed to e.g. a full disk.
fn is_damage(message: &str) -> bool {
    message.contains(" panicked: ") || message.to_ascii_lowercase().contains("corrupt")
}

/// Run a database operation, turning a panic into an error.
fn guarded<T>(what: &str, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    install_quiet_hook();
    let was = QUIET_PANICS.with(|q| q.replace(true));
    let r = catch_unwind(AssertUnwindSafe(f));
    QUIET_PANICS.with(|q| q.set(was));
    match r {
        Ok(r) => r,
        Err(p) => Err(format!("{what} panicked: {}", panic_text(p.as_ref()))),
    }
}

/// Largest cache file loaded into memory for a read snapshot (see [`ImageBackend`]).
const MAX_IMAGE_BYTES: u64 = 256 << 20;

/// The redb file backend, plus an optional in-memory copy of the whole file.
///
/// redb reads every page it has not cached with its own positioned read, and synchronous reads
/// through one Windows file handle are serialized: a warm scan that touches every entry spent
/// ~100 ms (VS Code, 67 MB cache) on page reads whatever the thread count. Loading the file with
/// one sequential read when a read snapshot starts (~15 ms) and serving page reads from memory
/// removes that. The copy is dropped when the snapshot ends and on any write, so writes always go
/// to — and later reads come from — the file.
#[derive(Debug)]
struct ImageBackend {
    file: FileBackend,
    image: RwLock<Option<Vec<u8>>>,
    /// A database operation failed or panicked: the file is damaged.
    corrupt: AtomicBool,
    /// Writes not yet passed to the file: one contiguous run `(offset, bytes)`. redb writes a
    /// commit page by page, in order (a cold scan of 1,771 files: ~1,800 writes, each a system
    /// call); consecutive pages go to the file as one write. The run is flushed when a write does
    /// not extend it, before anything that reads, measures or resizes the file, and at every sync
    /// point, so redb's ordering of data before headers is kept.
    pending: Mutex<(u64, Vec<u8>)>,
}

/// Largest run of pending writes kept in memory before it is written out.
const MAX_PENDING_BYTES: usize = 32 << 20;

impl ImageBackend {
    fn set_corrupt(&self) {
        self.corrupt.store(true, Ordering::Relaxed);
    }

    fn is_corrupt(&self) -> bool {
        self.corrupt.load(Ordering::Relaxed)
    }

    fn load_image(&self) {
        if self.flush_pending().is_err() {
            return;
        }
        let Ok(len) = self.file.len() else { return };
        if len == 0 || len > MAX_IMAGE_BYTES {
            return;
        }
        let mut buf = vec![0u8; len as usize];
        if self.file.read(0, &mut buf).is_ok() {
            *self.image.write().unwrap_or_else(|p| p.into_inner()) = Some(buf);
        }
    }

    fn drop_image(&self) {
        let mut img = self.image.write().unwrap_or_else(|p| p.into_inner());
        *img = None;
    }

    /// Write the pending run to the file.
    fn flush_pending(&self) -> Result<(), io::Error> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        if pending.1.is_empty() {
            return Ok(());
        }
        let (offset, bytes) = std::mem::take(&mut *pending);
        self.file.write(offset, &bytes)
    }

    fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let (start, run) = &mut *pending;
        if !run.is_empty()
            && *start + run.len() as u64 == offset
            && run.len() + data.len() <= MAX_PENDING_BYTES
        {
            run.extend_from_slice(data);
            return Ok(());
        }
        if !run.is_empty() {
            let (offset, bytes) = std::mem::take(&mut *pending);
            self.file.write(offset, &bytes)?;
        }
        *pending = (offset, data.to_vec());
        Ok(())
    }
}

/// Shared handle so the [`Cache`] can load and drop the image while redb owns the backend.
#[derive(Debug, Clone)]
struct SharedBackend(Arc<ImageBackend>);

impl StorageBackend for SharedBackend {
    fn len(&self) -> Result<u64, io::Error> {
        self.0.flush_pending()?;
        self.0.file.len()
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> Result<(), io::Error> {
        self.0.flush_pending()?;
        {
            let img = self.0.image.read().unwrap_or_else(|p| p.into_inner());
            if let Some(src) = img.as_ref().and_then(|img| {
                let start = usize::try_from(offset).ok()?;
                img.get(start..start.checked_add(out.len())?)
            }) {
                out.copy_from_slice(src);
                return Ok(());
            }
        }
        self.0.file.read(offset, out)
    }

    fn set_len(&self, len: u64) -> Result<(), io::Error> {
        self.0.drop_image();
        self.0.flush_pending()?;
        self.0.file.set_len(len)
    }

    /// Does nothing: the cache trades durability against power loss for speed. redb syncs the
    /// file about fifteen times while creating it and committing one scan's entries, and on
    /// Windows each sync took ~70 ms (a cold scan of 1,771 files spent over a second in them).
    /// Writes still reach the operating system in order, so a crashed or killed process loses
    /// nothing; only an operating-system crash or power loss can leave the file torn, and a torn
    /// file is detected (redb's checksums, then [`entry_checksum`], which binds every value to
    /// its key) and rebuilt like any damaged cache.
    fn sync_data(&self) -> Result<(), io::Error> {
        self.0.flush_pending()
    }

    fn write(&self, offset: u64, data: &[u8]) -> Result<(), io::Error> {
        self.0.drop_image();
        self.0.write(offset, data)
    }

    fn close(&self) -> Result<(), io::Error> {
        self.0.drop_image();
        self.0.flush_pending()?;
        self.0.file.close()
    }

    fn try_lock_range(&self, start: Bound<u64>, end: Bound<u64>) -> Result<bool, BackendError> {
        self.0.file.try_lock_range(start, end)
    }

    fn try_lock_shared_range(
        &self,
        start: Bound<u64>,
        end: Bound<u64>,
    ) -> Result<bool, BackendError> {
        self.0.file.try_lock_shared_range(start, end)
    }

    fn lock_range(&self, start: Bound<u64>, end: Bound<u64>) -> Result<(), BackendError> {
        self.0.file.lock_range(start, end)
    }

    fn lock_shared_range(&self, start: Bound<u64>, end: Bound<u64>) -> Result<(), BackendError> {
        self.0.file.lock_shared_range(start, end)
    }

    fn unlock_range(&self, start: Bound<u64>, end: Bound<u64>) -> Result<(), BackendError> {
        self.0.file.unlock_range(start, end)
    }

    fn query_lock_range(&self, start: Bound<u64>, end: Bound<u64>) -> Result<bool, BackendError> {
        self.0.file.query_lock_range(start, end)
    }
}

/// An open cache database.
pub struct Cache {
    db: Database,
    backend: Arc<ImageBackend>,
    path: PathBuf,
    max_bytes: u64,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache").field("path", &self.path).finish()
    }
}

/// A read snapshot, shareable across threads.
pub struct CacheReader {
    table: Option<redb::ReadOnlyTable<&'static [u8; 16], &'static [u8]>>,
    backend: Arc<ImageBackend>,
}

impl Drop for CacheReader {
    fn drop(&mut self) {
        let table = self.table.take();
        if guarded("closing a cache snapshot", move || {
            drop(table);
            Ok(())
        })
        .is_err()
        {
            self.backend.set_corrupt();
        }
        self.backend.drop_image();
    }
}

impl CacheReader {
    /// Look `key` up and map the stored bytes. A database error or panic marks the cache corrupt
    /// and every later lookup misses.
    fn lookup<T>(&self, key: &CacheKey, map: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
        if self.backend.is_corrupt() {
            return None;
        }
        let t = self.table.as_ref()?;
        let r = guarded("cache lookup", || match t.get(key) {
            Ok(Some(guard)) => Ok(map(guard.value())),
            Ok(None) => Ok(None),
            Err(e) => Err(e.to_string()),
        });
        match r {
            Ok(v) => v,
            Err(e) => {
                tracing::debug!("{e}");
                if is_damage(&e) {
                    self.backend.set_corrupt();
                }
                None
            }
        }
    }

    /// Raw bytes for `key`.
    pub fn get(&self, key: &CacheKey) -> Option<Vec<u8>> {
        self.lookup(key, |b| Some(b.to_vec()))
    }

    /// Decoded outcome for `key` (decoded straight from the database page, without a copy).
    /// Entries failing their checksum are misses.
    pub fn get_outcome(&self, key: &CacheKey) -> Option<FileOutcome> {
        self.lookup(key, |bytes| decode(key, bytes))
    }

    /// Whether a lookup found the database damaged (see [`Cache::is_corrupt`]).
    pub fn is_corrupt(&self) -> bool {
        self.backend.is_corrupt()
    }
}

impl Cache {
    /// Open (or create) `dir/cache.redb`. A corrupt file is deleted and recreated once.
    pub fn open(dir: &Path, max_size_mb: u64) -> Result<Cache, String> {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create cache directory {}: {e}", dir.display()))?;
        let path = dir.join(CACHE_FILE);
        type Opened = (Database, Arc<ImageBackend>);
        let open = || -> Result<Result<Opened, DatabaseError>, String> {
            guarded("opening the cache", || {
                Ok((|| {
                    let file = std::fs::OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .truncate(false)
                        .open(&path)?;
                    let backend = Arc::new(ImageBackend {
                        file: FileBackend::new(file)?,
                        image: RwLock::new(None),
                        corrupt: AtomicBool::new(false),
                        pending: Mutex::new((0, Vec::new())),
                    });
                    let db = Database::builder()
                        .set_cache_size(64 << 20)
                        .create_with_backend(SharedBackend(backend.clone()))?;
                    Ok((db, backend))
                })())
            })
        };
        let (db, backend) = match open() {
            Ok(Ok(x)) => x,
            Ok(Err(DatabaseError::DatabaseAlreadyOpen)) => {
                return Err(format!(
                    "cache {} is in use by another process; running without cache",
                    path.display()
                ))
            }
            other => {
                let why = match other {
                    Ok(Err(e)) => e.to_string(),
                    Err(e) => e,
                    Ok(Ok(_)) => unreachable!(),
                };
                tracing::warn!("cache {} unusable ({why}); recreating it", path.display());
                let _ = std::fs::remove_file(&path);
                let fail = |e: String| {
                    format!(
                        "cannot open cache {}: {e}; running without cache",
                        path.display()
                    )
                };
                open().map_err(fail)?.map_err(|e| fail(e.to_string()))?
            }
        };
        Ok(Cache {
            db,
            backend,
            path,
            max_bytes: max_size_mb.saturating_mul(1 << 20),
        })
    }

    /// The database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether an operation found the database damaged (a redb error or panic). Such a cache
    /// only misses; [`destroy`](Self::destroy) it.
    pub fn is_corrupt(&self) -> bool {
        self.backend.is_corrupt()
    }

    /// Record the outcome of a database operation: a damaged file marks the cache corrupt
    /// (other failures, such as a full disk, only fail the operation).
    fn track<T>(&self, r: Result<T, String>) -> Result<T, String> {
        if r.as_ref().is_err_and(|e| is_damage(e)) {
            self.backend.set_corrupt();
        }
        r
    }

    /// Close the database and delete its file (for a corrupt cache). Never panics.
    pub fn destroy(self) -> Result<(), String> {
        let path = self.path.clone();
        let _ = guarded("closing the cache", move || {
            drop(self);
            Ok(())
        });
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot delete {}: {e}", path.display())),
        }
    }

    /// Start a read snapshot. The database file is held in memory until the reader is dropped.
    pub fn reader(&self) -> Result<CacheReader, String> {
        self.backend.load_image();
        let r = guarded("opening a cache snapshot", || {
            let txn = self.db.begin_read().map_err(|e| e.to_string())?;
            match txn.open_table(TABLE) {
                Ok(table) => Ok(Some(table)),
                Err(redb::TableError::TableDoesNotExist(_)) => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        });
        match self.track(r) {
            Ok(table) => Ok(CacheReader {
                table,
                backend: self.backend.clone(),
            }),
            Err(e) => {
                self.backend.drop_image();
                Err(e)
            }
        }
    }

    /// Write entries in one transaction.
    pub fn write(&self, entries: &[(CacheKey, Vec<u8>)]) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }
        // Inserting in key order fills B-tree leaves left to right instead of splitting (and
        // copying) random ones, which keeps the file much smaller.
        let mut order: Vec<&(CacheKey, Vec<u8>)> = entries.iter().collect();
        order.sort_unstable_by_key(|a| a.0);
        let r = guarded("writing the cache", || {
            let txn = self.db.begin_write().map_err(|e| e.to_string())?;
            {
                let mut table = txn.open_table(TABLE).map_err(|e| e.to_string())?;
                for (k, v) in order {
                    table.insert(k, v.as_slice()).map_err(|e| e.to_string())?;
                }
            }
            txn.commit().map_err(|e| e.to_string())
        });
        self.track(r)
    }

    /// Number of entries.
    pub fn len(&self) -> u64 {
        self.reader()
            .ok()
            .and_then(|r| r.table.as_ref().and_then(|t| t.len().ok()))
            .unwrap_or(0)
    }

    /// Whether the cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// If the file exceeds the size limit, delete every entry not in `used` and compact.
    /// Returns the number of entries removed.
    pub fn prune(&mut self, used: &FxHashSet<CacheKey>) -> Result<u64, String> {
        let size = std::fs::metadata(&self.path).map(|m| m.len()).unwrap_or(0);
        if size <= self.max_bytes {
            return Ok(0);
        }
        let db = &mut self.db;
        let r = guarded("pruning the cache", || {
            let txn = db.begin_write().map_err(|e| e.to_string())?;
            let mut removed = 0u64;
            {
                let mut table = txn.open_table(TABLE).map_err(|e| e.to_string())?;
                table
                    .retain(|k, _| {
                        let keep = used.contains(k);
                        if !keep {
                            removed += 1;
                        }
                        keep
                    })
                    .map_err(|e| e.to_string())?;
            }
            txn.commit().map_err(|e| e.to_string())?;
            db.compact().map_err(|e| e.to_string())?;
            Ok(removed)
        });
        self.track(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::{FileResult, SkipReason};

    fn assert_sync<T: Sync + Send>() {}

    #[test]
    fn reader_is_sync() {
        assert_sync::<CacheReader>();
    }

    #[test]
    fn roundtrip_prune_and_lock() {
        let dir = tempfile::tempdir().unwrap();
        let k = |b: u8| [b; 16];
        {
            let mut c = Cache::open(dir.path(), 0).unwrap();
            assert!(c.reader().unwrap().get(&k(1)).is_none());
            let r = FileResult::default();
            let entries = vec![
                (
                    k(1),
                    encode(&k(1), &FileOutcome::Analyzed(Box::new(r))).unwrap(),
                ),
                (
                    k(2),
                    encode(&k(2), &FileOutcome::Skipped(SkipReason::Generated)).unwrap(),
                ),
            ];
            c.write(&entries).unwrap();
            let reader = c.reader().unwrap();
            assert!(matches!(
                reader.get_outcome(&k(2)),
                Some(FileOutcome::Skipped(SkipReason::Generated))
            ));
            assert!(matches!(
                reader.get_outcome(&k(1)),
                Some(FileOutcome::Analyzed(_))
            ));
            drop(reader);
            // a second handle in the same process cannot open the file
            assert!(Cache::open(dir.path(), 0).is_err());
            // max size 0 → prune everything not used
            let used: FxHashSet<CacheKey> = [k(1)].into_iter().collect();
            assert_eq!(c.prune(&used).unwrap(), 1);
            assert_eq!(c.len(), 1);
        }
        // corrupt file → recreated
        std::fs::write(dir.path().join(CACHE_FILE), b"garbage garbage garbage").unwrap();
        let c = Cache::open(dir.path(), 10).unwrap();
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn findings_keep_their_fix_through_the_cache() {
        let finding = |fix: Option<mote_core::Fix>| mote_core::Finding {
            id: "abc".into(),
            rule_id: "x/rule".into(),
            message: "m".into(),
            path: "a.x".into(),
            range: mote_core::TextRange::line(1),
            severity: mote_core::Severity::Low,
            quality: mote_core::Quality::Maintainability,
            symbol: None,
            value: None,
            fingerprint: "f".into(),
            status: Default::default(),
            incubating: false,
            suppressed: None,
            secondary: Vec::new(),
            fix,
        };
        let mut fix = mote_core::Fix::new("Write y", vec![mote_core::Edit::replace(0, 1, "y")]);
        fix.edits[0].range = mote_core::TextRange {
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        };
        let r = FileResult {
            findings: vec![finding(Some(fix.clone())), finding(None)],
            ..Default::default()
        };
        let key = [3u8; 16];
        let bytes = encode(&key, &FileOutcome::Analyzed(Box::new(r))).unwrap();
        let Some(FileOutcome::Analyzed(back)) = decode(&key, &bytes) else {
            panic!("not decoded");
        };
        assert_eq!(back.findings[0].fix.as_ref(), Some(&fix));
        assert!(back.findings[1].fix.is_none());
        // JSON leaves a missing fix out (an addition to the results format).
        let json = serde_json::to_string(&back.findings[1]).unwrap();
        assert!(!json.contains("\"fix\""), "{json}");
        let json = serde_json::to_string(&back.findings[0]).unwrap();
        assert!(json.ends_with(r#""secondary":[],"fix":{"description":"Write y","edits":[{"start":0,"end":1,"range":{"start_line":1,"start_col":1,"end_line":1,"end_col":2},"replacement":"y"}]}}"#), "{json}");
    }

    #[test]
    fn values_carry_a_checksum() {
        let r = FileResult {
            line_anchors: vec![crate::analyze::LineAnchor {
                line: 3,
                text: 42,
                context: 7,
            }],
            ..Default::default()
        };
        let key = [7u8; 16];
        let bytes = encode(&key, &FileOutcome::Analyzed(Box::new(r))).unwrap();
        assert!(matches!(
            decode(&key, &bytes),
            Some(FileOutcome::Analyzed(_))
        ));
        // any flipped bit, in the checksum or the payload, is a miss rather than altered data
        for i in 0..bytes.len() {
            for bit in [0x01u8, 0x80] {
                let mut b = bytes.clone();
                b[i] ^= bit;
                assert!(decode(&key, &b).is_none(), "byte {i} bit {bit:#x}");
            }
        }
        assert!(decode(&key, &bytes[..7]).is_none());
        // A valid entry is only accepted for its own key.
        let mut other = key;
        other[15] ^= 1;
        assert!(decode(&other, &bytes).is_none());
    }

    /// Damage the file of a populated cache at every 4 KiB page: opening, reading and writing
    /// never panic, reads never return altered values, and a damaged cache is detected.
    #[test]
    fn damaged_files_never_panic_or_return_garbage() {
        let dir = tempfile::tempdir().unwrap();
        let key = |i: u32| {
            let mut k = [0u8; 16];
            k[..4].copy_from_slice(&i.to_le_bytes());
            k[4..8].copy_from_slice(&i.wrapping_mul(2_654_435_761).to_le_bytes());
            k
        };
        let value = |i: u32| FileResult {
            line_anchors: (0..40)
                .map(|l| crate::analyze::LineAnchor {
                    line: l,
                    text: u64::from(i) * 1000 + u64::from(l),
                    context: u64::from(l),
                })
                .collect(),
            ..Default::default()
        };
        let n = 400;
        {
            let c = Cache::open(dir.path(), 1024).unwrap();
            let entries: Vec<(CacheKey, Vec<u8>)> = (0..n)
                .map(|i| {
                    (
                        key(i),
                        encode(&key(i), &FileOutcome::Analyzed(Box::new(value(i)))).unwrap(),
                    )
                })
                .collect();
            c.write(&entries).unwrap();
        }
        let file = dir.path().join(CACHE_FILE);
        let original = std::fs::read(&file).unwrap();
        assert!(original.len() > 64 << 10, "{}", original.len());
        let mut detected = 0;
        for offset in (0..original.len()).step_by(4096) {
            let mut damaged = original.clone();
            for b in damaged.iter_mut().skip(offset).take(512) {
                *b ^= 0xA5;
            }
            std::fs::write(&file, &damaged).unwrap();
            let Ok(c) = Cache::open(dir.path(), 1024) else {
                continue;
            };
            // Damage shows as a corrupt flag, or as misses (a rebuilt file, failed checksums).
            let mut damaged_seen = c.is_corrupt();
            if let Ok(r) = c.reader() {
                for i in 0..n {
                    match r.get_outcome(&key(i)) {
                        Some(FileOutcome::Analyzed(got)) => {
                            assert_eq!(got.line_anchors, value(i).line_anchors, "offset {offset}")
                        }
                        _ => damaged_seen = true,
                    }
                }
                damaged_seen |= r.is_corrupt();
            }
            let skipped = encode(&key(n + 1), &FileOutcome::Skipped(SkipReason::Binary)).unwrap();
            let _ = c.write(&[(key(n + 1), skipped)]);
            damaged_seen |= c.is_corrupt();
            detected += usize::from(damaged_seen);
            c.destroy().unwrap();
        }
        assert!(detected > 0, "no damage was detected");
    }

    #[test]
    fn keys_depend_on_every_part() {
        let base = KeyParts {
            plugin_id: "java",
            plugin_version: 1,
            rules_hash: 7,
            blob_id: "abc",
            is_test: false,
            settings_hash: 9,
            path: "a/B.java",
        };
        let k = cache_key(&base);
        assert_eq!(k, cache_key(&KeyParts { ..base }));
        assert_ne!(
            k,
            cache_key(&KeyParts {
                plugin_version: 2,
                ..base
            })
        );
        assert_ne!(
            k,
            cache_key(&KeyParts {
                rules_hash: 8,
                ..base
            })
        );
        assert_ne!(
            k,
            cache_key(&KeyParts {
                blob_id: "abd",
                ..base
            })
        );
        assert_ne!(
            k,
            cache_key(&KeyParts {
                is_test: true,
                ..base
            })
        );
        assert_ne!(
            k,
            cache_key(&KeyParts {
                settings_hash: 1,
                ..base
            })
        );
        assert_ne!(
            k,
            cache_key(&KeyParts {
                path: "a/C.java",
                ..base
            })
        );
    }
}
