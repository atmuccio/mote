//! Triage decisions kept in the repository (`.mote-triage.yaml`, see `triage.file`).
//!
//! Each entry records a decision about one finding — `accepted` (a real issue the team keeps on
//! purpose) or `false-positive` — with a reason:
//!
//! ```yaml
//! version: 1
//! entries:
//!   - fingerprint: 0f3c2d6e8a9b1c4d5e6f708192a3b4c5
//!     rule: "java/empty-catch-block"
//!     path: "src/main/java/app/Probe.java"
//!     status: false-positive
//!     reason: "The exception is the probe's expected outcome"
//!     by: "alice"
//!     date: "2026-09-28"
//! ```
//!
//! A scan marks the matching finding suppressed (`suppressed.kind` = the status, the reason as
//! justification), so it never gates and does not count towards effort or grades. Entries are
//! matched by **fingerprint** — rule, enclosing symbol and the text of the finding's line, without
//! the line number or the path — at the entry's path first and, when that file no longer exists,
//! in a renamed or moved file. An entry may name a finding `id` instead (path + fingerprint: exact
//! only, it does not follow renames). An entry goes **stale** when the finding's line is edited,
//! its enclosing class or function is renamed, an identical finding is inserted before it in the
//! same symbol, the rule is switched off, or the finding is fixed. Stale entries are reported as
//! `mote/unused-suppression` findings on the triage file (when the scan covered the entry's
//! file) and listed in the results (`triage.stale`) for `mote triage prune`.

use mote_core::Finding;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Default triage file name at the scan root.
pub const DEFAULT_TRIAGE_FILE: &str = ".mote-triage.yaml";

/// Version of the triage file format.
pub const TRIAGE_FILE_VERSION: u32 = 1;

/// The decision recorded for a finding.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum TriageStatus {
    /// A real finding the team accepts as it is (won't fix, by design).
    Accepted,
    /// The rule is wrong here.
    FalsePositive,
}

impl TriageStatus {
    /// The serialized name (`accepted`, `false-positive`), also the suppression kind.
    pub fn as_str(self) -> &'static str {
        match self {
            TriageStatus::Accepted => "accepted",
            TriageStatus::FalsePositive => "false-positive",
        }
    }

    /// Whether a suppression kind is a triage status.
    pub fn is_triage_kind(kind: &str) -> bool {
        kind == "accepted" || kind == "false-positive"
    }
}

/// One triage decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriageEntry {
    /// Finding fingerprint (32 hex digits); the preferred key.
    #[serde(default)]
    pub fingerprint: Option<String>,
    /// Finding id (12 hex digits), when no fingerprint is given.
    #[serde(default)]
    pub id: Option<String>,
    /// Rule id of the finding.
    pub rule: String,
    /// Scan-root-relative path of the file the finding was in.
    pub path: String,
    pub status: TriageStatus,
    /// Why (required).
    pub reason: String,
    /// Who decided.
    #[serde(default)]
    pub by: Option<String>,
    /// When (`YYYY-MM-DD`).
    #[serde(default)]
    pub date: Option<String>,
    /// Line of the entry in the file (1-based; 1 when unknown). Not part of the file.
    #[serde(skip)]
    pub line: u32,
}

impl TriageEntry {
    /// What identifies the entry (in results and for `prune`).
    pub fn key(&self) -> TriageKey {
        TriageKey {
            fingerprint: self.fingerprint.clone(),
            id: self.id.clone(),
            rule: self.rule.clone(),
            path: self.path.clone(),
        }
    }

    fn sort_key(&self) -> (&str, &str, &str, &str) {
        (
            &self.path,
            &self.rule,
            self.fingerprint.as_deref().unwrap_or(""),
            self.id.as_deref().unwrap_or(""),
        )
    }

    /// Whether two entries are about the same finding (same path and fingerprint, or same id).
    pub fn same_finding(&self, other: &TriageEntry) -> bool {
        self.path == other.path
            && match (&self.fingerprint, &other.fingerprint) {
                (Some(a), Some(b)) => a == b,
                (None, None) => self.id.is_some() && self.id == other.id,
                _ => false,
            }
    }
}

/// Identity of a triage entry.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct TriageKey {
    pub fingerprint: Option<String>,
    pub id: Option<String>,
    pub rule: String,
    pub path: String,
}

impl TriageKey {
    /// Whether `entry` has this key.
    pub fn is(&self, entry: &TriageEntry) -> bool {
        self.fingerprint == entry.fingerprint
            && self.id == entry.id
            && self.rule == entry.rule
            && self.path == entry.path
    }
}

/// What a scan did with the triage file (`ScanResult::triage`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TriageReport {
    /// The triage file (relative to the scan root when it is below it).
    pub file: String,
    /// Entries in the file.
    pub entries: u64,
    /// Entries that matched a finding of this scan.
    pub matched: u64,
    /// Entries that matched nothing although the scan covered their file (or their file is gone
    /// from a full scan), sorted. `mote triage prune` removes them.
    pub stale: Vec<TriageKey>,
}

/// A parsed triage file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TriageFile {
    pub entries: Vec<TriageEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    entries: Option<Vec<TriageEntry>>,
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Line of each block-sequence item of `entries:` (`- ...`), in order.
fn item_lines(text: &str) -> Vec<u32> {
    let mut out = Vec::new();
    let mut in_entries = false;
    let mut indent: Option<usize> = None;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let ind = line.len() - trimmed.len();
        if ind == 0 && !trimmed.starts_with('-') {
            in_entries = trimmed.starts_with("entries:");
            indent = None;
            continue;
        }
        if !in_entries || !(trimmed == "-" || trimmed.starts_with("- ")) {
            continue;
        }
        match indent {
            None => {
                indent = Some(ind);
                out.push(i as u32 + 1);
            }
            Some(n) if n == ind => out.push(i as u32 + 1),
            _ => {}
        }
    }
    out
}

impl TriageFile {
    /// Parse the YAML text of a triage file. Errors name the entry (`entries[2]: ...`).
    pub fn parse(text: &str) -> Result<TriageFile, String> {
        if text.trim().is_empty() {
            return Ok(TriageFile::default());
        }
        let raw: RawFile = crate::yaml::from_str(text).map_err(|e| e.to_string())?;
        if let Some(v) = raw.version {
            if v != TRIAGE_FILE_VERSION {
                return Err(format!(
                    "unsupported triage file version {v} (this mote reads version {TRIAGE_FILE_VERSION})"
                ));
            }
        }
        let mut entries = raw.entries.unwrap_or_default();
        let lines = item_lines(text);
        let lines_known = lines.len() == entries.len();
        let mut problems = Vec::new();
        for (i, e) in entries.iter_mut().enumerate() {
            e.line = if lines_known { lines[i] } else { 1 };
            let mut bad = |m: String| problems.push(format!("entries[{i}]: {m}"));
            e.fingerprint = e.fingerprint.take().map(|f| f.trim().to_ascii_lowercase());
            e.id = e.id.take().map(|f| f.trim().to_ascii_lowercase());
            match (&e.fingerprint, &e.id) {
                (None, None) => bad("needs a `fingerprint` (or a finding `id`)".into()),
                (Some(f), _) if !is_hex(f, 32) => {
                    bad(format!("`fingerprint` must be 32 hex digits, got `{f}`"))
                }
                (_, Some(id)) if !is_hex(id, 12) => {
                    bad(format!("`id` must be 12 hex digits, got `{id}`"))
                }
                _ => {}
            }
            e.rule = e.rule.trim().to_string();
            if !e.rule.contains('/') {
                bad(format!(
                    "`rule` must be a full rule id such as `java/empty-catch-block`, got `{}`",
                    e.rule
                ));
            }
            e.path = e.path.trim().replace('\\', "/");
            if e.path.is_empty() {
                bad("`path` must not be empty".into());
            }
            e.reason = e.reason.trim().to_string();
            if e.reason.is_empty() {
                bad("`reason` must say why the finding is accepted or a false positive".into());
            }
            for v in [&mut e.by, &mut e.date] {
                if v.as_deref().is_some_and(|s| s.trim().is_empty()) {
                    *v = None;
                }
            }
        }
        if problems.is_empty() {
            Ok(TriageFile { entries })
        } else {
            Err(problems.join("; "))
        }
    }

    /// Sort entries by path, rule and key (the order [`render`](Self::render) writes).
    pub fn sort(&mut self) {
        self.entries.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    }

    /// Add `entry`, replacing an entry about the same finding. Returns whether one was replaced.
    pub fn upsert(&mut self, entry: TriageEntry) -> bool {
        let replaced = match self.entries.iter_mut().find(|e| e.same_finding(&entry)) {
            Some(e) => {
                *e = entry;
                true
            }
            None => {
                self.entries.push(entry);
                false
            }
        };
        self.sort();
        replaced
    }

    /// The file's text: a header comment, then the entries sorted, one key per line, strings
    /// double-quoted. The same entries always give the same bytes.
    pub fn render(&self) -> String {
        let mut entries: Vec<&TriageEntry> = self.entries.iter().collect();
        entries.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
        let mut s = String::new();
        s.push_str(
            "# Mote triage decisions: findings accepted as they are, or false positives.\n\
             # Entries match findings by fingerprint (line moves and file renames keep them); an\n\
             # entry whose finding is gone is reported as stale. Edit with `mote triage\n\
             # add|list|prune` (rewriting keeps the entries, not other comments).\n",
        );
        let _ = writeln!(s, "version: {TRIAGE_FILE_VERSION}");
        if entries.is_empty() {
            s.push_str("entries: []\n");
            return s;
        }
        s.push_str("entries:\n");
        let q = |v: &str| serde_json::to_string(v).unwrap_or_else(|_| "\"\"".into());
        for e in entries {
            let mut first = true;
            let mut field = |s: &mut String, k: &str, v: String| {
                let _ = writeln!(s, "  {} {k}: {v}", if first { "-" } else { " " });
                first = false;
            };
            if let Some(f) = &e.fingerprint {
                field(&mut s, "fingerprint", f.clone());
            }
            if let Some(id) = &e.id {
                field(&mut s, "id", id.clone());
            }
            field(&mut s, "rule", q(&e.rule));
            field(&mut s, "path", q(&e.path));
            field(&mut s, "status", e.status.as_str().to_string());
            field(&mut s, "reason", q(&e.reason));
            if let Some(by) = &e.by {
                field(&mut s, "by", q(by));
            }
            if let Some(d) = &e.date {
                field(&mut s, "date", q(d));
            }
        }
        s
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// Match `entries` to `findings`; returns, for each finding, the index of its entry. Each entry
/// matches at most one finding, and meta findings (`mote/*`) never match.
///
/// 1. Fingerprint entries: the finding with the entry's fingerprint and rule at the entry's path;
///    id entries: the finding with that id.
/// 2. A fingerprint entry whose file is gone (`exists(path)` is false) takes a finding with its
///    fingerprint and rule in another file — a rename or move — preferring the same file name,
///    then the same directory, then result order.
pub fn match_entries(
    entries: &[TriageEntry],
    findings: &[&Finding],
    exists: &dyn Fn(&str) -> bool,
) -> Vec<Option<usize>> {
    let mut out: Vec<Option<usize>> = vec![None; findings.len()];
    let eligible = |f: &Finding| !f.rule_id.starts_with("mote/");
    let mut by_path_fp: FxHashMap<(&str, &str), Vec<usize>> = FxHashMap::default();
    let mut by_id: FxHashMap<&str, usize> = FxHashMap::default();
    let mut by_fp: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
    for (i, f) in findings.iter().enumerate() {
        if !eligible(f) {
            continue;
        }
        by_path_fp
            .entry((f.path.as_str(), f.fingerprint.as_str()))
            .or_default()
            .push(i);
        by_id.entry(f.id.as_str()).or_insert(i);
        by_fp.entry(f.fingerprint.as_str()).or_default().push(i);
    }
    let mut pending = Vec::new();
    for (e, entry) in entries.iter().enumerate() {
        let hit = match (&entry.fingerprint, &entry.id) {
            (Some(fp), _) => by_path_fp
                .get(&(entry.path.as_str(), fp.as_str()))
                .and_then(|l| {
                    l.iter()
                        .copied()
                        .find(|&i| out[i].is_none() && findings[i].rule_id == entry.rule)
                }),
            (None, Some(id)) => by_id
                .get(id.as_str())
                .copied()
                .filter(|&i| out[i].is_none() && findings[i].rule_id == entry.rule),
            (None, None) => None,
        };
        match hit {
            Some(i) => out[i] = Some(e),
            None if entry.fingerprint.is_some() && !exists(&entry.path) => pending.push(e),
            None => {}
        }
    }
    for e in pending {
        let entry = &entries[e];
        let Some(list) = entry.fingerprint.as_deref().and_then(|fp| by_fp.get(fp)) else {
            continue;
        };
        let score = |i: usize| {
            let p = &findings[i].path;
            if file_name(p) == file_name(&entry.path) {
                0
            } else if dir_of(p) == dir_of(&entry.path) {
                1
            } else {
                2
            }
        };
        let best = list
            .iter()
            .copied()
            .filter(|&i| out[i].is_none() && findings[i].rule_id == entry.rule)
            .min_by_key(|&i| (score(i), i));
        if let Some(i) = best {
            out[i] = Some(e);
        }
    }
    out
}

/// Message of the finding reported for a stale entry.
pub fn stale_message(entry: &TriageEntry) -> String {
    format!(
        "Remove this stale triage entry ({} `{}` in {}); it matches no finding (fixed, or the code changed)",
        entry.status.as_str(),
        entry.rule,
        entry.path
    )
}

/// Fingerprint anchor of the finding reported for a stale entry (stable while the entry is).
pub fn stale_anchor(entry: &TriageEntry) -> String {
    format!(
        "triage:{}:{}:{}:{}",
        entry.fingerprint.as_deref().unwrap_or(""),
        entry.id.as_deref().unwrap_or(""),
        entry.rule,
        entry.path
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::{FindingStatus, Quality, Severity, TextRange};

    const FP1: &str = "0123456789abcdef0123456789abcdef";
    const FP2: &str = "fedcba9876543210fedcba9876543210";

    fn finding(path: &str, rule: &str, fp: &str) -> Finding {
        Finding {
            id: crate::fingerprint::finding_id(path, fp),
            rule_id: rule.into(),
            message: "m".into(),
            path: path.into(),
            range: TextRange::line(1),
            severity: Severity::High,
            quality: Quality::Reliability,
            symbol: None,
            value: None,
            fingerprint: fp.into(),
            status: FindingStatus::Unknown,
            incubating: false,
            suppressed: None,
            secondary: vec![],
            fix: None,
        }
    }

    fn entry(path: &str, rule: &str, fp: &str) -> TriageEntry {
        TriageEntry {
            fingerprint: Some(fp.into()),
            id: None,
            rule: rule.into(),
            path: path.into(),
            status: TriageStatus::FalsePositive,
            reason: "because".into(),
            by: None,
            date: None,
            line: 0,
        }
    }

    #[test]
    fn parse_validate_and_render_round_trip() {
        let text = format!(
            "# team decisions\nversion: 1\nentries:\n  - fingerprint: {}\n    rule: java/x\n    path: src\\A.java\n    status: accepted\n    reason: '  legacy API, kept on purpose '\n    by: alice\n    date: 2026-09-28\n  - id: ABCDEF012345\n    rule: js/y\n    path: a.js\n    status: false-positive\n    reason: \"quote \\\" and: colon\"\n",
            FP1.to_uppercase()
        );
        let f = TriageFile::parse(&text).unwrap();
        assert_eq!(f.entries.len(), 2);
        let a = &f.entries[0];
        assert_eq!(a.fingerprint.as_deref(), Some(FP1), "lower-cased");
        assert_eq!(a.path, "src/A.java");
        assert_eq!(a.reason, "legacy API, kept on purpose");
        assert_eq!(a.date.as_deref(), Some("2026-09-28"));
        assert_eq!((a.line, f.entries[1].line), (4, 11));
        assert_eq!(f.entries[1].id.as_deref(), Some("abcdef012345"));
        let text = f.render();
        let back = TriageFile::parse(&text).unwrap();
        assert_eq!(
            back.entries
                .iter()
                .map(|e| (&e.path, &e.reason, e.status))
                .collect::<Vec<_>>(),
            vec![
                (
                    &"a.js".to_string(),
                    &"quote \" and: colon".to_string(),
                    TriageStatus::FalsePositive
                ),
                (
                    &"src/A.java".to_string(),
                    &"legacy API, kept on purpose".to_string(),
                    TriageStatus::Accepted
                ),
            ],
            "sorted by path"
        );
        assert_eq!(back.render(), text, "stable formatting");
        assert!(text.starts_with("# Mote triage decisions"));
        assert_eq!(
            TriageFile::default().render().lines().last(),
            Some("entries: []")
        );
        assert_eq!(TriageFile::parse("").unwrap(), TriageFile::default());
        assert_eq!(
            TriageFile::parse("entries:\n").unwrap(),
            TriageFile::default()
        );
    }

    #[test]
    fn parse_errors_name_the_entry() {
        let e = TriageFile::parse(
            "entries:\n  - rule: x\n    path: ''\n    status: accepted\n    reason: ' '\n",
        )
        .unwrap_err();
        assert!(e.contains("entries[0]: needs a `fingerprint`"), "{e}");
        assert!(e.contains("full rule id"), "{e}");
        assert!(e.contains("`path` must not be empty"), "{e}");
        assert!(e.contains("`reason`"), "{e}");
        let e = TriageFile::parse("entries:\n  - fingerprint: 12\n    rule: a/b\n    path: p\n    status: accepted\n    reason: r\n").unwrap_err();
        assert!(e.contains("32 hex digits"), "{e}");
        let e = TriageFile::parse("entries:\n  - fingerprint: x\n    rule: a/b\n    path: p\n    status: wontfix\n    reason: r\n").unwrap_err();
        assert!(e.contains("wontfix"), "{e}");
        assert!(TriageFile::parse("version: 2\n")
            .unwrap_err()
            .contains("version 2"));
        assert!(TriageFile::parse("entries: 3\n").is_err());
        assert!(TriageFile::parse("surprise: true\n").is_err());
        // Odd input never panics.
        for text in [
            "- a\n- b",
            "entries:\n  - [1, 2]\n",
            "\u{0}\u{1}",
            "entries: {a: 1}",
        ] {
            let _ = TriageFile::parse(text);
        }
    }

    #[test]
    fn upsert_replaces_the_same_finding() {
        let mut f = TriageFile::default();
        assert!(!f.upsert(entry("b.js", "js/x", FP1)));
        assert!(
            !f.upsert(entry("a.js", "js/x", FP1)),
            "other path, other finding"
        );
        let mut e = entry("b.js", "js/x", FP1);
        e.status = TriageStatus::Accepted;
        assert!(f.upsert(e));
        assert_eq!(f.entries.len(), 2);
        assert_eq!(f.entries[0].path, "a.js");
        assert_eq!(f.entries[1].status, TriageStatus::Accepted);
    }

    #[test]
    fn matching_by_path_then_rename() {
        let findings = [
            finding("a.java", "j/x", FP1),
            finding("b.java", "j/x", FP1), // same line text in another file
            finding("moved/Old.java", "j/y", FP2),
            finding("x.java", "mote/parse-error", FP2),
        ];
        let refs: Vec<&Finding> = findings.iter().collect();
        let entries = [
            entry("a.java", "j/x", FP1),
            entry("old/Old.java", "j/y", FP2), // renamed file
            entry("a.java", "j/other", FP1),   // rule mismatch
            entry("deleted.java", "j/x", FP1), // gone, but both candidates exist in files
        ];
        let exists = |p: &str| matches!(p, "a.java" | "b.java" | "moved/Old.java" | "x.java");
        let m = match_entries(&entries, &refs, &exists);
        // entry 3 (file gone) takes the remaining unmatched j/x finding (b.java).
        assert_eq!(m, vec![Some(0), Some(3), Some(1), None]);

        // Id entries match exactly.
        let mut by_id = entry("a.java", "j/x", FP1);
        by_id.fingerprint = None;
        by_id.id = Some(findings[0].id.clone());
        assert_eq!(
            match_entries(&[by_id], &refs, &exists),
            vec![Some(0), None, None, None]
        );
    }

    #[test]
    fn stale_anchors_differ_per_entry() {
        let a = entry("src/a1.java", "j/x", FP1);
        let b = entry("src/a2.java", "j/x", FP1);
        assert_ne!(stale_anchor(&a), stale_anchor(&b));
        assert!(stale_message(&a).contains("`j/x` in src/a1.java"));
    }
}
