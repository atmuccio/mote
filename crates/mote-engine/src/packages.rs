//! The npm packages a JavaScript file belongs to: the `name` of the nearest `package.json` up
//! the directory tree (the file's directory, then its parent, up to the scan root; a
//! `package.json` without a `name` passes the search on) and the `name` of the `package.json` at
//! the scan root.
//!
//! Used by the vendored-code check ([`crate::vendored`]): a library banner that names one of
//! these packages is the project's own release banner, not a copied library's. `package.json`
//! files are data: parsed as JSON, only `name` is read, nothing is run.

use crate::source::{BlobLoader, SourceEntry};
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The manifest file name.
pub const PACKAGE_JSON: &str = "package.json";

/// Larger manifests are ignored.
const MAX_PACKAGE_JSON: usize = 1 << 20;

/// Where manifests are looked up.
enum Manifests {
    /// The manifests among a discovery's entries, read through its loader (a scan of a revision
    /// reads the revision's manifests).
    Listed {
        entries: FxHashMap<String, SourceEntry>,
        loader: Arc<dyn BlobLoader>,
    },
    /// Files on disk below this root.
    Disk(PathBuf),
}

/// Per-scan lookup of the package names of files, memoized per directory and per manifest.
pub struct PackageNames {
    manifests: Manifests,
    /// Directory → the name of the nearest named manifest at or above it.
    nearest: Mutex<FxHashMap<String, Option<Arc<str>>>>,
    /// Directory → the names reported for its files (nearest, then the root's).
    names: Mutex<FxHashMap<String, Arc<[String]>>>,
    /// Manifest path → its `name`.
    parsed: Mutex<FxHashMap<String, Option<Arc<str>>>>,
}

impl PackageNames {
    fn new(manifests: Manifests) -> Self {
        PackageNames {
            manifests,
            nearest: Mutex::new(FxHashMap::default()),
            names: Mutex::new(FxHashMap::default()),
            parsed: Mutex::new(FxHashMap::default()),
        }
    }

    /// Manifests taken from discovered `entries` (paths relative to the scan root), read with
    /// `loader`.
    pub fn listed(entries: &[SourceEntry], loader: Arc<dyn BlobLoader>) -> Self {
        let entries = entries
            .iter()
            .filter(|e| file_name(&e.path) == PACKAGE_JSON)
            .map(|e| (e.path.clone(), e.clone()))
            .collect();
        Self::new(Manifests::Listed { entries, loader })
    }

    /// Manifests read from disk below `root`.
    pub fn disk(root: &Path) -> Self {
        Self::new(Manifests::Disk(root.to_path_buf()))
    }

    /// The package names of the file at `path` (relative to the scan root, `/`-separated): the
    /// nearest named manifest's, then the root manifest's when it differs. Empty when there is
    /// none.
    pub fn names_for(&self, path: &str) -> Arc<[String]> {
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
        if let Some(n) = lock(&self.names).get(dir) {
            return n.clone();
        }
        let mut out: Vec<String> = Vec::new();
        if let Some(n) = self.nearest_name(dir) {
            out.push(n.to_string());
        }
        if let Some(root) = self.name_of(PACKAGE_JSON) {
            if !out.iter().any(|n| **n == *root) {
                out.push(root.to_string());
            }
        }
        let out: Arc<[String]> = out.into();
        lock(&self.names)
            .entry(dir.to_string())
            .or_insert(out)
            .clone()
    }

    /// The name of the nearest named manifest in `dir` or above it.
    fn nearest_name(&self, dir: &str) -> Option<Arc<str>> {
        // Climb until a directory whose answer is known or whose manifest has a name; every
        // directory passed on the way shares that answer.
        let mut visited: Vec<&str> = Vec::new();
        let mut d = dir;
        let found = loop {
            if let Some(r) = lock(&self.nearest).get(d) {
                break r.clone();
            }
            visited.push(d);
            let manifest = if d.is_empty() {
                PACKAGE_JSON.to_string()
            } else {
                format!("{d}/{PACKAGE_JSON}")
            };
            if let Some(name) = self.name_of(&manifest) {
                break Some(name);
            }
            if d.is_empty() {
                break None;
            }
            d = d.rsplit_once('/').map_or("", |(p, _)| p);
        };
        let mut memo = lock(&self.nearest);
        for v in visited {
            memo.entry(v.to_string()).or_insert_with(|| found.clone());
        }
        found
    }

    /// The `name` of the manifest at `path` (memoized; `None` when absent, unreadable or
    /// unnamed).
    fn name_of(&self, path: &str) -> Option<Arc<str>> {
        if let Some(r) = lock(&self.parsed).get(path) {
            return r.clone();
        }
        let found = self
            .read(path)
            .and_then(|bytes| name_in(&bytes))
            .map(Arc::from);
        lock(&self.parsed).insert(path.to_string(), found.clone());
        found
    }

    fn read(&self, path: &str) -> Option<Vec<u8>> {
        let bytes = match &self.manifests {
            Manifests::Listed { entries, loader } => {
                let e = entries.get(path)?;
                if e.size.is_some_and(|s| s > MAX_PACKAGE_JSON as u64) {
                    return None;
                }
                loader.load(e).ok()?
            }
            Manifests::Disk(root) => {
                let full = root.join(path);
                let meta = std::fs::metadata(&full).ok()?;
                if !meta.is_file() || meta.len() > MAX_PACKAGE_JSON as u64 {
                    return None;
                }
                std::fs::read(full).ok()?
            }
        };
        (bytes.len() <= MAX_PACKAGE_JSON).then_some(bytes)
    }
}

/// The `name` of a `package.json` (a non-empty string, trimmed).
pub fn name_in(bytes: &[u8]) -> Option<String> {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let name = v.get("name")?.as_str()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::EntryLocation;

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn names(p: &PackageNames, path: &str) -> Vec<String> {
        p.names_for(path).to_vec()
    }

    #[test]
    fn reads_only_the_name() {
        assert_eq!(
            name_in(br#"{"name": " socket.io ", "version": "4.8.4"}"#).as_deref(),
            Some("socket.io")
        );
        assert_eq!(
            name_in(b"\xEF\xBB\xBF{\"name\": \"@scope/pkg\"}").as_deref(),
            Some("@scope/pkg")
        );
        for text in [
            &br#"{"private": true}"#[..],
            br#"{"name": ""}"#,
            br#"{"name": 3}"#,
            br#"["name"]"#,
            b"not json",
            b"",
        ] {
            assert_eq!(name_in(text), None);
        }
    }

    #[test]
    fn nearest_named_manifest_and_the_root() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        write(root, "package.json", r#"{"name": "monorepo"}"#);
        write(
            root,
            "packages/socket.io/package.json",
            r#"{"name": "socket.io"}"#,
        );
        write(
            root,
            "packages/unnamed/package.json",
            r#"{"private": true}"#,
        );
        write(root, "packages/socket.io/client-dist/socket.io.js", "x");
        let p = PackageNames::disk(root);
        assert_eq!(
            names(&p, "packages/socket.io/client-dist/socket.io.js"),
            ["socket.io", "monorepo"]
        );
        assert_eq!(
            names(&p, "packages/socket.io/lib/index.js"),
            ["socket.io", "monorepo"]
        );
        // A manifest without a name passes the search on.
        assert_eq!(names(&p, "packages/unnamed/src/a.js"), ["monorepo"]);
        assert_eq!(names(&p, "a.js"), ["monorepo"]);
        let empty = tempfile::tempdir().unwrap();
        assert!(PackageNames::disk(empty.path())
            .names_for("a/b.js")
            .is_empty());
    }

    struct MapLoader(FxHashMap<String, String>);

    impl BlobLoader for MapLoader {
        fn load(&self, entry: &SourceEntry) -> std::io::Result<Vec<u8>> {
            self.0
                .get(&entry.path)
                .map(|s| s.as_bytes().to_vec())
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
        fn load_blob(&self, _: &str) -> std::io::Result<Option<Vec<u8>>> {
            Ok(None)
        }
    }

    #[test]
    fn listed_manifests_come_from_the_loader() {
        let files: FxHashMap<String, String> = [
            ("package.json", r#"{"name": "app"}"#),
            ("lib/package.json", r#"{"name": "@acme/lib"}"#),
            ("lib/dist/lib.js", "x"),
        ]
        .into_iter()
        .map(|(p, t)| (p.to_string(), t.to_string()))
        .collect();
        let entries: Vec<SourceEntry> = files
            .keys()
            .map(|p| SourceEntry {
                path: p.clone(),
                blob_id: None,
                size: None,
                location: EntryLocation::Object,
            })
            .collect();
        let p = PackageNames::listed(&entries, Arc::new(MapLoader(files)));
        assert_eq!(names(&p, "lib/dist/lib.js"), ["@acme/lib", "app"]);
        assert_eq!(names(&p, "src/index.js"), ["app"]);
    }
}
