//! New-code model: what changed between a base and the scanned head, and how head findings
//! relate to base findings.
//!
//! * The base is `merge-base(base, head)`, like GitHub's three-dot "Files changed" view.
//! * `git diff --raw -z -M` gives changed files, renames and base blob ids; `git diff -U0` gives
//!   the added/changed line ranges of each head file (and the removed/changed ranges of its base).
//!   Untracked files are entirely new.
//! * Unchanged files have identical findings in base and head by construction (analysis is
//!   file-local), so they are `Unchanged` without any work. Changed files are matched against the
//!   analysis of their base blob (see [`classify`]); unmatched head findings are `New`, matched
//!   ones whose value grew are `Worsened`, unmatched base findings are *fixed*. Findings of
//!   deleted files are fixed too — unless their code moved: [`pair_moved`] pairs new findings on
//!   added lines with gone ones on removed lines, across all changed and deleted files.

use crate::git::{self, Git, GitError};
use mote_core::{Finding, FindingStatus, Severity};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What the head of the comparison is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadSpec {
    /// The working tree (tracked changes; untracked files are handled separately).
    Worktree,
    /// The index.
    Index,
    /// A commit.
    Commit(String),
}

/// How a file changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Modified,
    Renamed,
    Copied,
    Untracked,
}

/// One changed head file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    pub kind: ChangeKind,
    /// Path in the base (differs from the head path for renames and copies).
    pub base_path: Option<String>,
    /// Base blob id (absent for added files).
    pub base_blob: Option<String>,
    /// Head blob id when known (absent for working-tree changes).
    pub head_blob: Option<String>,
    /// Added or changed line ranges (inclusive). Empty for mode-only changes.
    pub new_lines: Vec<(u32, u32)>,
    /// Base line ranges (inclusive) that were removed or changed.
    pub old_lines: Vec<(u32, u32)>,
    /// All lines are new (added / untracked files).
    pub all_new: bool,
}

/// A file of the base that the head deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletedFile {
    /// Path in the base.
    pub path: String,
    /// Base blob id.
    pub blob: Option<String>,
}

/// Differences between the merge base and the head.
#[derive(Clone, Debug, Default)]
pub struct DiffInfo {
    pub merge_base: String,
    /// Changed head files by head path.
    pub changed: BTreeMap<String, ChangedFile>,
    /// Deleted files.
    pub deleted: Vec<DeletedFile>,
}

/// Line ranges of one file's hunks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hunks {
    /// Added or changed head lines.
    pub new: Vec<(u32, u32)>,
    /// Removed or changed base lines.
    pub old: Vec<(u32, u32)>,
}

const NULL_OID: &str = "0000000000000000000000000000000000000000";

fn diff_args<'a>(head: &'a HeadSpec, mb: &'a str, relative: bool) -> Vec<&'a str> {
    let mut v = vec![
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--ignore-submodules",
        "-M",
    ];
    if relative {
        v.push("--relative");
    }
    match head {
        HeadSpec::Worktree => v.push(mb),
        HeadSpec::Index => {
            v.push("--cached");
            v.push(mb);
        }
        HeadSpec::Commit(c) => {
            v.push(mb);
            v.push(c);
        }
    }
    v.push("--");
    v
}

/// Compute the diff between `merge-base(base, head)` and the head.
pub fn compute_diff(g: &Git, base: &str, head: &HeadSpec) -> Result<DiffInfo, GitError> {
    let base_commit = g.resolve_commit(base)?;
    let head_commit = match head {
        HeadSpec::Commit(c) => c.clone(),
        _ => g.resolve_commit("HEAD")?,
    };
    let mb = g
        .merge_base(&base_commit, &head_commit)
        .map_err(|e| match e {
            GitError::NoMergeBase(_) => GitError::NoMergeBase(format!(
            "no common ancestor between `{base}` and {}; the comparison needs a shared history \
             (fetch more history, e.g. `git fetch --unshallow`, or pick another base)",
            match head {
                HeadSpec::Commit(_) => "the scanned revision",
                _ => "HEAD",
            }
        )),
            other => other,
        })?;
    let relative = !g.prefix().is_empty();

    let mut raw_args = vec!["diff", "--raw", "-z", "--no-abbrev"];
    raw_args.extend(diff_args(head, &mb, relative));
    let mut patch_args = vec!["diff", "-U0", "--src-prefix=a/", "--dst-prefix=b/"];
    patch_args.extend(diff_args(head, &mb, relative));
    let (raw, patch) = std::thread::scope(|s| {
        let r = s.spawn(|| g.run(&raw_args));
        let p = g.run(&patch_args);
        (r.join().unwrap_or_else(|p| std::panic::resume_unwind(p)), p)
    });
    let mut info = parse_raw(&raw?);
    info.merge_base = mb;
    let hunks = parse_hunks(&patch?);
    for (path, h) in hunks {
        if let Some(f) = info.changed.get_mut(&path) {
            f.new_lines = h.new;
            f.old_lines = h.old;
        }
    }
    Ok(info)
}

/// Parse `git diff --raw -z --no-abbrev` output.
pub fn parse_raw(out: &[u8]) -> DiffInfo {
    let mut info = DiffInfo::default();
    let fields: Vec<String> = git::split_nul(out).collect();
    let mut i = 0;
    while i < fields.len() {
        let meta = &fields[i];
        i += 1;
        let Some(meta) = meta.strip_prefix(':') else {
            continue;
        };
        let parts: Vec<&str> = meta.split(' ').collect();
        if parts.len() < 5 {
            continue;
        }
        let (src, dst, status) = (parts[2], parts[3], parts[4]);
        let letter = status.chars().next().unwrap_or('M');
        let two_paths = matches!(letter, 'R' | 'C');
        let Some(p1) = fields.get(i).cloned() else {
            break;
        };
        i += 1;
        let p2 = if two_paths {
            let p = fields.get(i).cloned();
            i += 1;
            p
        } else {
            None
        };
        let oid = |s: &str| (s != NULL_OID && !s.chars().all(|c| c == '0')).then(|| s.to_string());
        match letter {
            'D' => info.deleted.push(DeletedFile {
                path: p1,
                blob: oid(src),
            }),
            'A' => {
                info.changed.insert(
                    p1,
                    ChangedFile {
                        kind: ChangeKind::Added,
                        base_path: None,
                        base_blob: None,
                        head_blob: oid(dst),
                        new_lines: Vec::new(),
                        old_lines: Vec::new(),
                        all_new: true,
                    },
                );
            }
            'R' | 'C' => {
                let Some(head_path) = p2 else { continue };
                info.changed.insert(
                    head_path,
                    ChangedFile {
                        kind: if letter == 'R' {
                            ChangeKind::Renamed
                        } else {
                            ChangeKind::Copied
                        },
                        base_path: Some(p1),
                        base_blob: oid(src),
                        head_blob: oid(dst),
                        new_lines: Vec::new(),
                        old_lines: Vec::new(),
                        all_new: false,
                    },
                );
            }
            _ => {
                // M (modified), T (type change), U (unmerged), X: treat as modified.
                info.changed.insert(
                    p1.clone(),
                    ChangedFile {
                        kind: ChangeKind::Modified,
                        base_path: Some(p1),
                        base_blob: oid(src),
                        head_blob: oid(dst),
                        new_lines: Vec::new(),
                        old_lines: Vec::new(),
                        all_new: false,
                    },
                );
            }
        }
    }
    info
}

/// Undo git's C-style path quoting (`"a\tb\303\251"`).
pub fn c_unquote(s: &str) -> String {
    let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) else {
        return s.to_string();
    };
    let b = inner.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() {
            let c = b[i + 1];
            match c {
                b'n' => out.push(b'\n'),
                b't' => out.push(b'\t'),
                b'r' => out.push(b'\r'),
                b'a' => out.push(7),
                b'b' => out.push(8),
                b'f' => out.push(12),
                b'v' => out.push(11),
                b'0'..=b'7' if i + 4 <= b.len() => {
                    let oct = std::str::from_utf8(&b[i + 1..i + 4])
                        .ok()
                        .and_then(|o| u8::from_str_radix(o, 8).ok());
                    match oct {
                        Some(v) => {
                            out.push(v);
                            i += 4;
                            continue;
                        }
                        None => out.push(c),
                    }
                }
                other => out.push(other),
            }
            i += 2;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `a[,b]` of a hunk header → the inclusive range, `None` when it covers no line.
fn hunk_range(spec: &str) -> Option<(u32, u32)> {
    let (start, count) = match spec.split_once(',') {
        Some((s, c)) => (s.parse::<u32>().ok()?, c.parse::<u32>().ok()?),
        None => (spec.parse::<u32>().ok()?, 1),
    };
    (count > 0 && start > 0).then(|| (start, start.saturating_add(count - 1)))
}

/// Parse `git diff -U0` output into added (head) and removed (base) line ranges per head path.
pub fn parse_hunks(out: &[u8]) -> BTreeMap<String, Hunks> {
    let text = String::from_utf8_lossy(out);
    let mut map: BTreeMap<String, Hunks> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix("+++ ") {
            // Git ends the path with a TAB when it contains a space (for GNU patch).
            let rest = rest.trim_end_matches(['\r', '\t']);
            let p = c_unquote(rest);
            current = p.strip_prefix("b/").map(str::to_string);
            continue;
        }
        if line.starts_with("diff --git ") {
            current = None;
            continue;
        }
        if let (Some(path), Some(rest)) = (&current, line.strip_prefix("@@ ")) {
            // @@ -a[,b] +c[,d] @@
            let mut parts = rest.split(' ');
            let minus = parts.next().and_then(|p| p.strip_prefix('-'));
            let plus = parts.next().and_then(|p| p.strip_prefix('+'));
            let h = map.entry(path.clone()).or_default();
            h.new.extend(plus.and_then(hunk_range));
            h.old.extend(minus.and_then(hunk_range));
        }
    }
    map
}

/// Counts from matching head and base findings of changed files.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchCounts {
    /// Base findings with no head counterpart, by severity.
    pub fixed: BTreeMap<Severity, u32>,
}

/// A message with every run of digits replaced by `#` (values in messages change with the code).
pub(crate) fn normalized_message(m: &str) -> String {
    let mut out = String::with_capacity(m.len());
    let mut in_digits = false;
    for c in m.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                out.push('#');
            }
            in_digits = true;
        } else {
            in_digits = false;
            out.push(c);
        }
    }
    out
}

/// Where one side's findings sit: the anchor hash of each finding's start line (aligned with the
/// findings; see [`crate::fingerprint::anchor_hash`]) and the lines the diff touched on that
/// side (head: added or changed; base: removed or changed; sorted, merged).
#[derive(Clone, Copy, Debug)]
pub struct Side<'a> {
    pub anchors: &'a [Option<u64>],
    pub changed: &'a [(u32, u32)],
}

impl Side<'_> {
    fn anchor(&self, i: usize) -> Option<u64> {
        self.anchors.get(i).copied().flatten()
    }

    fn touched(&self, f: &Finding) -> bool {
        overlaps(self.changed, f.range.start_line, f.range.start_line)
    }
}

/// The base line of a head line the diff did not touch (`None` for a touched line).
///
/// Lines outside the hunks are the diff's common lines, in the same order on both sides, so the
/// k-th untouched head line is the k-th untouched base line. `head_changed` and `base_changed`
/// are the touched lines of each side (sorted, merged).
pub fn untouched_base_line(
    head_changed: &[(u32, u32)],
    base_changed: &[(u32, u32)],
    line: u32,
) -> Option<u32> {
    if overlaps(head_changed, line, line) {
        return None;
    }
    let touched_before: u32 = head_changed
        .iter()
        .take_while(|r| r.1 < line)
        .map(|r| r.1 - r.0 + 1)
        .sum();
    // 1-based rank among the untouched lines, then the base line with the same rank.
    let mut base = line - touched_before;
    for &(s, e) in base_changed {
        if s > base {
            break;
        }
        base += e - s + 1;
    }
    Some(base)
}

/// Classify `head` findings of one changed file against its `base` findings; returns the indices
/// of the base findings left unpaired, in order. They are fixed unless [`pair_moved`] finds their
/// code moved elsewhere (see [`fixed_by_severity`]).
///
/// Matching, in order (each base finding matches at most once):
///
/// 0. for head findings on lines the diff did not touch: same rule and message (digits ignored)
///    on the base line that line maps to (see [`untouched_base_line`]; both lines must have the
///    same text) — the same code with the same issue, whatever the fingerprints say. An identical
///    line added further up shifts the occurrence numbers of the identical findings below it,
///    and a renamed method or class changes their symbol; neither makes them new;
/// 1. same fingerprint (rule, symbol, line text, occurrence);
/// 2. for head findings on lines the diff did not touch: same rule, line text and message
///    (digits ignored), whatever the symbol — a renamed class or method does not make the issues
///    of its unchanged code new;
/// 3. same rule, symbol and message, when the two are plausibly the same code: the head finding
///    is on an untouched line, or both lines have the same text, or both carry a measured value
///    (metrics such as complexity exist once per symbol). A finding on an added or changed line
///    is never paired with an unrelated base finding just because they share a method.
pub fn classify(
    head: &mut [Finding],
    head_side: Side<'_>,
    base: &[Finding],
    base_side: Side<'_>,
) -> Vec<usize> {
    let mut used = vec![false; base.len()];
    let mut matched: Vec<Option<usize>> = vec![None; head.len()];
    let take = |list: &mut Vec<usize>,
                used: &mut Vec<bool>,
                ok: &dyn Fn(usize) -> bool,
                prefer: &dyn Fn(usize) -> bool|
     -> Option<usize> {
        let pos = list
            .iter()
            .position(|&i| !used[i] && ok(i) && prefer(i))
            .or_else(|| list.iter().position(|&i| !used[i] && ok(i)))?;
        let i = list.remove(pos);
        used[i] = true;
        Some(i)
    };

    // 0. The same untouched code: rule + message on the mapped base line.
    let mut by_line: FxHashMap<(u32, &str), Vec<usize>> = FxHashMap::default();
    for (i, b) in base.iter().enumerate() {
        by_line
            .entry((b.range.start_line, b.rule_id.as_str()))
            .or_default()
            .push(i);
    }
    for (h, f) in head.iter().enumerate() {
        let Some(line) =
            untouched_base_line(head_side.changed, base_side.changed, f.range.start_line)
        else {
            continue;
        };
        let Some(list) = by_line.get_mut(&(line, f.rule_id.as_str())) else {
            continue;
        };
        let (ha, msg) = (head_side.anchor(h), normalized_message(&f.message));
        let same = |i: usize| {
            ha.is_some() && ha == base_side.anchor(i) && normalized_message(&base[i].message) == msg
        };
        matched[h] = take(list, &mut used, &same, &|i| {
            base[i].fingerprint == f.fingerprint
        });
    }

    // 1. Fingerprints.
    let mut by_fp: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
    for (i, b) in base.iter().enumerate() {
        if !used[i] {
            by_fp.entry(b.fingerprint.as_str()).or_default().push(i);
        }
    }
    for (h, f) in head.iter().enumerate() {
        if matched[h].is_some() {
            continue;
        }
        if let Some(list) = by_fp.get_mut(f.fingerprint.as_str()) {
            matched[h] = take(list, &mut used, &|_| true, &|_| true);
        }
    }

    // 2. Rule + line text + message on untouched head lines (symbol ignored).
    let mut by_text: FxHashMap<(&str, u64, String), Vec<usize>> = FxHashMap::default();
    for (i, b) in base.iter().enumerate() {
        if let (false, Some(a)) = (used[i], base_side.anchor(i)) {
            by_text
                .entry((b.rule_id.as_str(), a, normalized_message(&b.message)))
                .or_default()
                .push(i);
        }
    }
    for (h, f) in head.iter().enumerate() {
        if matched[h].is_some() || head_side.touched(f) {
            continue;
        }
        let Some(a) = head_side.anchor(h) else {
            continue;
        };
        if let Some(list) =
            by_text.get_mut(&(f.rule_id.as_str(), a, normalized_message(&f.message)))
        {
            // Prefer a base finding whose line survived.
            matched[h] = take(list, &mut used, &|_| true, &|i| {
                !base_side.touched(&base[i])
            });
        }
    }

    // 3. Rule + symbol + message, for plausibly the same code.
    let mut by_key: FxHashMap<(&str, Option<&str>, String), Vec<usize>> = FxHashMap::default();
    for (i, b) in base.iter().enumerate() {
        if !used[i] {
            by_key
                .entry((
                    b.rule_id.as_str(),
                    b.symbol.as_deref(),
                    normalized_message(&b.message),
                ))
                .or_default()
                .push(i);
        }
    }
    for (h, f) in head.iter().enumerate() {
        if matched[h].is_some() {
            continue;
        }
        let key = (
            f.rule_id.as_str(),
            f.symbol.as_deref(),
            normalized_message(&f.message),
        );
        let Some(list) = by_key.get_mut(&key) else {
            continue;
        };
        let head_untouched = !head_side.touched(f);
        let ha = head_side.anchor(h);
        let plausible = |i: usize| {
            head_untouched
                || (ha.is_some() && ha == base_side.anchor(i))
                || (f.value.is_some() && base[i].value.is_some())
        };
        matched[h] = take(list, &mut used, &plausible, &|i| {
            ha.is_some() && ha == base_side.anchor(i)
        });
    }

    for (h, f) in head.iter_mut().enumerate() {
        f.status = match matched[h] {
            None => FindingStatus::New,
            Some(i) => status_against(f, &base[i]),
        };
    }
    (0..base.len()).filter(|&i| !used[i]).collect()
}

/// Status of a head finding paired with a base finding: worsened when its measured value grew.
pub fn status_against(head: &Finding, base: &Finding) -> FindingStatus {
    match (head.value, base.value) {
        (Some(hv), Some(bv)) if hv > bv => FindingStatus::Worsened,
        _ => FindingStatus::Unchanged,
    }
}

/// Fixed findings by severity: the given unpaired base findings that are not suppressed.
pub fn fixed_by_severity<'a>(
    unpaired: impl IntoIterator<Item = &'a Finding>,
) -> BTreeMap<Severity, u32> {
    let mut fixed = BTreeMap::new();
    for b in unpaired {
        if b.suppressed.is_none() {
            *fixed.entry(b.severity).or_insert(0) += 1;
        }
    }
    fixed
}

/// A finding's place for moved-code matching: its file (any index identifying it) and the
/// hashes of its line ([`crate::fingerprint::anchor_hash`]) and of the line with its neighbors
/// ([`crate::fingerprint::context_hash`]).
#[derive(Clone, Copy, Debug)]
pub struct MoveSite<'a> {
    pub file: usize,
    pub finding: &'a Finding,
    pub anchor: u64,
    pub context: u64,
}

/// Moved code: pairs new head findings on added or changed lines (`heads`) with unpaired base
/// findings on removed or changed lines (`gone`) that have the same rule, the same message
/// (digits ignored), the same line text and the same lines around it. That is code the change
/// moved rather than wrote: into a new method of the same file (the symbol changed, so the
/// fingerprint did), or into another file. A pair in the same file is preferred, then the
/// first in order. Returns, for each head site, the index of its `gone` partner.
pub fn pair_moved(heads: &[MoveSite<'_>], gone: &[MoveSite<'_>]) -> Vec<Option<usize>> {
    type Key<'k> = (&'k str, String, u64, u64);
    let key = |s: &MoveSite<'_>| -> (String, u64, u64) {
        (normalized_message(&s.finding.message), s.anchor, s.context)
    };
    let mut by_key: FxHashMap<Key<'_>, Vec<usize>> = FxHashMap::default();
    for (i, g) in gone.iter().enumerate() {
        let (m, a, c) = key(g);
        by_key
            .entry((g.finding.rule_id.as_str(), m, a, c))
            .or_default()
            .push(i);
    }
    heads
        .iter()
        .map(|h| {
            let (m, a, c) = key(h);
            let list = by_key.get_mut(&(h.finding.rule_id.as_str(), m, a, c))?;
            let pos = list
                .iter()
                .position(|&i| gone[i].file == h.file)
                .unwrap_or(0);
            (!list.is_empty()).then(|| list.remove(pos))
        })
        .collect()
}

/// Whether any line of `[start, end]` falls in `ranges` (sorted, merged).
pub fn overlaps(ranges: &[(u32, u32)], start: u32, end: u32) -> bool {
    let i = ranges.partition_point(|r| r.1 < start);
    ranges.get(i).is_some_and(|r| r.0 <= end)
}

/// Number of lines in the intersection of two sorted, merged range lists.
pub fn intersect_count(a: &[(u32, u32)], b: &[(u32, u32)]) -> u32 {
    let (mut i, mut j, mut n) = (0, 0, 0u32);
    while i < a.len() && j < b.len() {
        let lo = a[i].0.max(b[j].0);
        let hi = a[i].1.min(b[j].1);
        if lo <= hi {
            n += hi - lo + 1;
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::{Quality, TextRange};

    /// [`classify`], returning the fixed counts.
    fn classify_fixed(
        head: &mut [Finding],
        head_side: Side<'_>,
        base: &[Finding],
        base_side: Side<'_>,
    ) -> BTreeMap<Severity, u32> {
        let unpaired = classify(head, head_side, base, base_side);
        fixed_by_severity(unpaired.iter().map(|&i| &base[i]))
    }

    fn f(rule: &str, fp: &str, msg: &str, value: Option<f64>) -> Finding {
        Finding {
            id: String::new(),
            rule_id: rule.into(),
            message: msg.into(),
            path: "a".into(),
            range: TextRange::line(1),
            severity: Severity::Medium,
            quality: Quality::Maintainability,
            symbol: Some("S".into()),
            value,
            fingerprint: fp.into(),
            status: FindingStatus::Unknown,
            incubating: false,
            suppressed: None,
            secondary: vec![],
            fix: None,
        }
    }

    #[test]
    fn classification() {
        let base = vec![
            f("r/a", "fp1", "m", None),
            f("r/cc", "fp2", "complexity 18 to 15", Some(18.0)),
            f("r/b", "fp3", "gone", None),
            f("r/d", "old-fp", "moved", None),
        ];
        let mut head = vec![
            f("r/a", "fp1", "m", None),
            f(
                "r/cc",
                "fp2-changed-line",
                "complexity 25 to 15",
                Some(25.0),
            ),
            f("r/d", "new-fp", "moved", None),
            f("r/new", "fp9", "fresh", None),
        ];
        let none = Side {
            anchors: &[],
            changed: &[],
        };
        let fixed = classify_fixed(&mut head, none, &base, none);
        let st: Vec<FindingStatus> = head.iter().map(|x| x.status).collect();
        assert_eq!(
            st,
            vec![
                FindingStatus::Unchanged,
                FindingStatus::Worsened,
                FindingStatus::Unchanged,
                FindingStatus::New
            ]
        );
        assert_eq!(fixed.get(&Severity::Medium), Some(&1));
    }

    fn at(mut x: Finding, line: u32, symbol: &str) -> Finding {
        x.range = TextRange::line(line);
        x.symbol = Some(symbol.into());
        x
    }

    #[test]
    fn a_finding_on_added_lines_is_not_paired_with_a_fixed_one_in_the_same_method() {
        // base: `if (s == "w")` on line 6 (changed to `s.equals("w")`); head: a new
        // `if (t == "w!")` on added line 10 of the same method.
        let msg = "Compare strings with equals()";
        let base = vec![at(f("j/str-eq", "fp-old", msg, None), 6, "Same#m")];
        let mut head = vec![at(f("j/str-eq", "fp-new", msg, None), 10, "Same#m")];
        let fixed = classify_fixed(
            &mut head,
            Side {
                anchors: &[Some(222)],
                changed: &[(6, 6), (9, 12)],
            },
            &base,
            Side {
                anchors: &[Some(111)],
                changed: &[(6, 6)],
            },
        );
        assert_eq!(head[0].status, FindingStatus::New);
        assert_eq!(fixed.get(&Severity::Medium), Some(&1));

        // The same finding merely moved down by inserted lines (its line text is unchanged) is
        // still matched, and so is one on an untouched line.
        for (head_anchor, changed) in [(Some(111), vec![(3, 5)]), (Some(999), vec![])] {
            let mut head = vec![at(f("j/str-eq", "fp-new", msg, None), 10, "Same#m")];
            let head_anchors = [head_anchor];
            classify(
                &mut head,
                Side {
                    anchors: &head_anchors,
                    changed: &changed,
                },
                &base,
                Side {
                    anchors: &[Some(111)],
                    changed: &[],
                },
            );
            assert_eq!(head[0].status, FindingStatus::Unchanged);
        }
    }

    #[test]
    fn renaming_a_class_keeps_the_issues_of_its_unchanged_code() {
        // `class Old` → `class Newer`: symbols and fingerprints change, line texts do not.
        let base = vec![
            at(
                f("j/self-assign", "fp-a", "Remove this self-assignment", None),
                5,
                "Old#m",
            ),
            at(
                f("j/cc", "fp-b", "Complexity 12 is over 10", Some(12.0)),
                4,
                "Old#m",
            ),
        ];
        let mut head = vec![
            at(
                f("j/self-assign", "fp-c", "Remove this self-assignment", None),
                5,
                "Newer#m",
            ),
            at(
                f("j/cc", "fp-d", "Complexity 12 is over 10", Some(12.0)),
                4,
                "Newer#m",
            ),
            // a genuinely new issue on an unchanged line (caused by a change elsewhere)
            at(
                f("j/unused", "fp-e", "Remove this unused field", None),
                2,
                "Newer",
            ),
        ];
        let fixed = classify_fixed(
            &mut head,
            Side {
                anchors: &[Some(5), Some(4), Some(2)],
                changed: &[(3, 3)],
            },
            &base,
            Side {
                anchors: &[Some(5), Some(4)],
                changed: &[(3, 3)],
            },
        );
        let st: Vec<FindingStatus> = head.iter().map(|x| x.status).collect();
        assert_eq!(
            st,
            vec![
                FindingStatus::Unchanged,
                FindingStatus::Unchanged,
                FindingStatus::New
            ]
        );
        assert!(fixed.is_empty(), "{fixed:?}");
    }

    #[test]
    fn untouched_lines_map_to_their_base_lines() {
        // base 1..=10; head: 1 X Y 2 3 6 7 8 9 Z (X, Y inserted; 4, 5 deleted; 10 changed)
        let (head, base) = ([(2, 3), (10, 10)], [(4, 5), (10, 10)]);
        let map = |l| untouched_base_line(&head, &base, l);
        assert_eq!(map(1), Some(1));
        assert_eq!(map(2), None);
        assert_eq!(map(4), Some(2));
        assert_eq!(map(5), Some(3));
        assert_eq!(map(6), Some(6));
        assert_eq!(map(9), Some(9));
        assert_eq!(map(10), None);
        // Unmerged adjacent base ranges count the same as merged ones.
        assert_eq!(untouched_base_line(&[], &[(4, 5), (6, 7)], 4), Some(8));
        assert_eq!(untouched_base_line(&[], &[], 7), Some(7));
    }

    #[test]
    fn an_identical_line_added_above_does_not_make_the_old_finding_new() {
        // base: `if (xs.size() > 0)` on line 5 (occurrence 0). The change adds the same line on
        // line 2 (it gets occurrence 0) and the old one moves to line 6 (occurrence 1).
        let msg = "Use isEmpty()";
        let base = vec![at(f("j/size", "occ0", msg, None), 5, "A#m")];
        let mut head = vec![
            at(f("j/size", "occ0", msg, None), 2, "A#m"),
            at(f("j/size", "occ1", msg, None), 6, "A#m"),
        ];
        let fixed = classify_fixed(
            &mut head,
            Side {
                anchors: &[Some(7), Some(7)],
                changed: &[(2, 2)],
            },
            &base,
            Side {
                anchors: &[Some(7)],
                changed: &[],
            },
        );
        let st: Vec<(u32, FindingStatus)> = head
            .iter()
            .map(|x| (x.range.start_line, x.status))
            .collect();
        assert_eq!(
            st,
            vec![(2, FindingStatus::New), (6, FindingStatus::Unchanged)]
        );
        assert!(fixed.is_empty(), "{fixed:?}");
    }

    #[test]
    fn the_same_code_matches_only_with_the_same_rule_message_and_text() {
        let base = vec![
            at(f("j/unused", "b1", "Unused variable 'a'", None), 4, "A#m"),
            at(f("j/size", "b2", "Use isEmpty()", None), 7, "A#m"),
        ];
        // Line 4 is untouched and maps to base line 4, but now reports another variable (a
        // different issue); line 8 maps to base line 7 but its text differs (an inconsistent
        // diff must not pair findings; another symbol keeps the later steps out of it).
        let mut head = vec![
            at(f("j/unused", "h1", "Unused variable 'b'", None), 4, "A#m"),
            at(f("j/size", "h2", "Use isEmpty()", None), 8, "B#n"),
        ];
        let fixed = classify_fixed(
            &mut head,
            Side {
                anchors: &[Some(1), Some(2)],
                changed: &[(6, 6)],
            },
            &base,
            Side {
                anchors: &[Some(1), Some(3)],
                changed: &[],
            },
        );
        let st: Vec<FindingStatus> = head.iter().map(|x| x.status).collect();
        assert_eq!(st, vec![FindingStatus::New, FindingStatus::New]);
        assert_eq!(fixed.get(&Severity::Medium), Some(&2));
    }

    #[test]
    fn moved_code_pairs_same_file_first_and_needs_the_same_neighbors() {
        let catch = f("j/catch", "x", "Catch a more specific exception", None);
        let long = f("j/long", "y", "Method has 81 lines", Some(81.0));
        let site = |file, finding, anchor, context| MoveSite {
            file,
            finding,
            anchor,
            context,
        };
        let gone = [
            site(0, &catch, 5, 50),
            site(1, &catch, 5, 50),
            site(0, &long, 6, 60),
        ];
        let long_now = f("j/long", "z", "Method has 95 lines", Some(95.0));
        let other_msg = f(
            "j/catch",
            "x",
            "Catch a more specific exception than Throwable",
            None,
        );
        let heads = [
            site(0, &other_msg, 5, 50), // another message: another issue
            site(1, &catch, 5, 50),     // same file as gone[1]
            site(2, &catch, 5, 50),     // another file: the remaining gone[0]
            site(2, &catch, 5, 50),     // nothing left
            site(3, &catch, 5, 51),     // other neighbors
            site(3, &long_now, 6, 60),  // digits ignored
        ];
        assert_eq!(
            pair_moved(&heads, &gone),
            vec![None, Some(1), Some(0), None, None, Some(2)]
        );
    }

    #[test]
    fn raw_parsing_with_renames() {
        let z = |s: &str| s.replace('|', "\0");
        let out = z(":100644 100644 aaaa000000000000000000000000000000000001 0000000000000000000000000000000000000000 M|src/a.java|:000000 100644 0000000000000000000000000000000000000000 bbbb000000000000000000000000000000000002 A|new file.java|:100644 100644 cccc000000000000000000000000000000000003 dddd000000000000000000000000000000000004 R087|old/x.java|new/x.java|:100644 000000 eeee000000000000000000000000000000000005 0000000000000000000000000000000000000000 D|gone.java|");
        let info = parse_raw(out.as_bytes());
        assert_eq!(info.changed.len(), 3);
        let a = &info.changed["src/a.java"];
        assert_eq!(a.kind, ChangeKind::Modified);
        assert_eq!(
            a.base_blob.as_deref(),
            Some("aaaa000000000000000000000000000000000001")
        );
        assert_eq!(a.head_blob, None);
        assert!(info.changed["new file.java"].all_new);
        let r = &info.changed["new/x.java"];
        assert_eq!(r.kind, ChangeKind::Renamed);
        assert_eq!(r.base_path.as_deref(), Some("old/x.java"));
        assert_eq!(
            info.deleted,
            vec![DeletedFile {
                path: "gone.java".into(),
                blob: Some("eeee000000000000000000000000000000000005".into())
            }]
        );
    }

    #[test]
    fn hunk_parsing() {
        let patch = "diff --git a/x.java b/x.java\nindex 1..2 100644\n--- a/x.java\n+++ b/x.java\n@@ -3 +3 @@ class\n-a\n+b\n@@ -10,0 +11,2 @@\n+c\n+d\n@@ -20,3 +22,0 @@\n-x\ndiff --git a/g.java b/g.java\ndeleted file mode 100644\n--- a/g.java\n+++ /dev/null\n@@ -1 +0,0 @@\n-x\ndiff --git \"a/sp ace\\303\\251.java\" \"b/sp ace\\303\\251.java\"\n--- \"a/sp ace\\303\\251.java\"\n+++ \"b/sp ace\\303\\251.java\"\n@@ -1 +1 @@\n-a\n+b\n";
        let h = parse_hunks(patch.as_bytes());
        assert_eq!(h["x.java"].new, vec![(3, 3), (11, 12)]);
        assert_eq!(h["x.java"].old, vec![(3, 3), (20, 22)]);
        assert!(!h.contains_key("g.java"));
        assert_eq!(h["sp aceé.java"].new, vec![(1, 1)]);
    }

    #[test]
    fn hunk_paths_with_spaces_end_with_a_tab() {
        // `git diff` appends a TAB to `---`/`+++` paths that contain a space.
        let patch = "diff --git a/dir with space/X.java b/dir with space/X.java\nindex 1..2 100644\n--- a/dir with space/X.java\t\n+++ b/dir with space/X.java\t\n@@ -8,0 +9,4 @@ class X {\n+a\n+b\n+c\n+d\n";
        let h = parse_hunks(patch.as_bytes());
        assert_eq!(
            h.get("dir with space/X.java").map(|h| h.new.clone()),
            Some(vec![(9, 12)])
        );
        assert!(h["dir with space/X.java"].old.is_empty());
    }

    #[test]
    fn range_helpers() {
        let r = [(3, 5), (10, 12)];
        assert!(overlaps(&r, 1, 3));
        assert!(overlaps(&r, 6, 10));
        assert!(!overlaps(&r, 6, 9));
        assert!(!overlaps(&r, 13, 20));
        assert_eq!(intersect_count(&r, &[(4, 11)]), 4);
        assert_eq!(c_unquote("\"a\\tb\""), "a\tb");
        assert_eq!(c_unquote("plain"), "plain");
    }
}
