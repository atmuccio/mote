//! Small helpers shared by the commands.

use crate::cli::ColorChoice;
use crate::CliError;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Whether to emit ANSI colors. `always`/`never` win; `auto` needs a terminal and no `NO_COLOR`.
pub fn use_color(choice: ColorChoice, is_tty: bool) -> bool {
    let on = match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => {
            is_tty
                && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
                && std::env::var_os("TERM").is_none_or(|t| t != "dumb")
        }
    };
    #[cfg(windows)]
    if on && is_tty {
        // Legacy consoles need virtual-terminal processing switched on for ANSI sequences.
        let _ = anstyle_query::windows::enable_ansi_colors();
    }
    on
}

/// Render into a byte buffer.
pub fn render(f: impl FnOnce(&mut dyn Write) -> std::io::Result<()>) -> Vec<u8> {
    let mut buf = Vec::new();
    // Writing into a Vec cannot fail except through serde errors, which would be bugs.
    if let Err(e) = f(&mut buf) {
        tracing::error!("rendering failed: {e}");
    }
    buf
}

/// Write an output file, creating parent directories.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| {
            CliError::internal(format!("cannot create directory {}: {e}", parent.display()))
        })?;
    }
    std::fs::write(path, bytes)
        .map_err(|e| CliError::internal(format!("cannot write {}: {e}", path.display())))
}

fn has_glob(s: &str) -> bool {
    s.contains(['*', '?', '[', '{'])
}

/// Expand report arguments (literal paths or globs, relative to `cwd` or absolute) into existing
/// files. Returns the files and one warning per argument that matched nothing.
pub fn expand_reports(args: &[String], cwd: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let mut files = Vec::new();
    let mut warnings = Vec::new();
    for arg in args {
        let norm = arg.replace('\\', "/");
        let found: Vec<PathBuf> = if has_glob(&norm) {
            // Split at the first component with a wildcard: base directory + pattern.
            let comps: Vec<&str> = norm.split('/').collect();
            let first_glob = comps.iter().position(|c| has_glob(c)).unwrap_or(0);
            let base_str = comps[..first_glob].join("/");
            let pattern = comps[first_glob..].join("/");
            let base = if base_str.is_empty() {
                cwd.to_path_buf()
            } else {
                let b = PathBuf::from(if base_str.ends_with(':') {
                    format!("{base_str}/")
                } else {
                    base_str.clone()
                });
                if b.is_absolute() {
                    b
                } else {
                    cwd.join(b)
                }
            };
            mote_engine::coverage::find_reports(&base, &[pattern], &[])
        } else {
            let p = PathBuf::from(arg);
            let p = if p.is_absolute() { p } else { cwd.join(p) };
            if p.is_file() {
                vec![p]
            } else {
                vec![]
            }
        };
        if found.is_empty() {
            warnings.push(format!("no coverage report matches `{arg}`"));
        }
        files.extend(found);
    }
    files.sort();
    files.dedup();
    (files, warnings)
}

/// Per-user cache directory for remote clones: `$MOTE_CACHE_DIR`, else the platform cache
/// directory (`%LOCALAPPDATA%`, `~/Library/Caches`, `$XDG_CACHE_HOME` or `~/.cache`) + `mote`,
/// else the temp directory.
pub fn user_cache_dir() -> PathBuf {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(d) = env("MOTE_CACHE_DIR") {
        return d;
    }
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Caches"))
    } else {
        env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache")))
    };
    base.unwrap_or_else(std::env::temp_dir).join("mote")
}

/// `YYYY-MM-DD` (UTC) of a Unix time in seconds.
pub fn civil_date(unix_secs: i64) -> String {
    // Days to civil date (Howard Hinnant's algorithm).
    let days = unix_secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// RFC 3339 UTC timestamp (`2026-09-28T13:45:00Z`) of a Unix time in seconds.
pub fn utc_timestamp(unix_secs: i64) -> String {
    let s = unix_secs.rem_euclid(86_400);
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        civil_date(unix_secs),
        s / 3600,
        s / 60 % 60,
        s % 60
    )
}

/// The current Unix time, or `SOURCE_DATE_EPOCH` when set (reproducible outputs).
pub fn now_secs() -> i64 {
    if let Some(v) = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
    {
        return v;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Current directory, or an internal error.
pub fn cwd() -> Result<PathBuf, CliError> {
    std::env::current_dir()
        .map_err(|e| CliError::internal(format!("cannot read the current directory: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_globs_and_literals() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/lcov.info"), "").unwrap();
        std::fs::write(root.join("a/jacoco.xml"), "").unwrap();
        let (f, w) = expand_reports(&["**/lcov.info".into(), "a/jacoco.xml".into()], root);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(w.is_empty());
        let abs = format!("{}/**/*.xml", root.display());
        let (f, _) = expand_reports(&[abs], Path::new("."));
        assert_eq!(f.len(), 1, "{f:?}");
        let (f, w) = expand_reports(&["nope/*.info".into()], root);
        assert!(f.is_empty());
        assert_eq!(w.len(), 1);
    }

    #[test]
    fn dates() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(1_790_553_600), "2026-09-28");
        assert_eq!(civil_date(951_782_400), "2000-02-29");
        assert_eq!(civil_date(-86_400), "1969-12-31");
        assert_eq!(
            utc_timestamp(1_790_553_600 + 13 * 3600 + 45 * 60 + 7),
            "2026-09-28T13:45:07Z"
        );
    }

    #[test]
    fn color_choice() {
        assert!(use_color(ColorChoice::Always, false));
        assert!(!use_color(ColorChoice::Never, true));
        assert!(!use_color(ColorChoice::Auto, false));
    }
}
