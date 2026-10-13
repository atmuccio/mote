//! Matching the findings of two scans without a diff: a scan against a baseline results file
//! (`mote scan --baseline`) and two saved results (`mote compare`).
//!
//! New-code analysis ([`newcode`](crate::newcode)) knows which lines changed; here only the two
//! finding lists (and the files each scan covered) are known. Findings are paired one to one, in
//! this order:
//!
//! 1. same path and fingerprint (the fingerprint leaves the line number out, so a finding whose
//!    line moved keeps it);
//! 2. same fingerprint in another file, when the old finding's file is gone from the new scan and
//!    the new finding's file did not exist in the old one — a renamed or moved file. Among several
//!    candidates the one with the same file name wins, then the same directory, then the first
//!    in result order;
//! 3. same path, rule, symbol and message (digits ignored) when both carry a measured value:
//!    metrics such as complexity exist once per symbol, and their anchor line (the declaration)
//!    may change without the finding being a different one.
//!
//! Both lists are expected in result order (sorted); the outcome is deterministic.

use crate::newcode::normalized_message;
use mote_core::Finding;
use rustc_hash::{FxHashMap, FxHashSet};

/// Files each side of a comparison covered (repo-relative paths). Empty sets mean "unknown":
/// every file then counts as present on the other side, so rule 2 applies to any pair.
#[derive(Clone, Copy, Debug)]
pub struct Files<'a> {
    pub old: &'a FxHashSet<&'a str>,
    pub new: &'a FxHashSet<&'a str>,
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn dir_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

/// Pair `new` findings with `old` ones; returns, for each new finding, the index of its old
/// counterpart.
pub fn match_findings(old: &[&Finding], new: &[&Finding], files: Files<'_>) -> Vec<Option<usize>> {
    let mut matched: Vec<Option<usize>> = vec![None; new.len()];
    let mut used = vec![false; old.len()];

    // 1. Path and fingerprint.
    let mut exact: FxHashMap<(&str, &str), Vec<usize>> = FxHashMap::default();
    for (i, f) in old.iter().enumerate() {
        exact
            .entry((f.path.as_str(), f.fingerprint.as_str()))
            .or_default()
            .push(i);
    }
    for (n, f) in new.iter().enumerate() {
        if let Some(list) = exact.get_mut(&(f.path.as_str(), f.fingerprint.as_str())) {
            if let Some(pos) = list.iter().position(|&i| !used[i]) {
                let i = list.remove(pos);
                used[i] = true;
                matched[n] = Some(i);
            }
        }
    }

    // 2. Fingerprint across a rename or move.
    let old_gone = |p: &str| files.new.is_empty() || !files.new.contains(p);
    let new_fresh = |p: &str| files.old.is_empty() || !files.old.contains(p);
    let mut by_fp: FxHashMap<&str, Vec<usize>> = FxHashMap::default();
    for (i, f) in old.iter().enumerate() {
        if !used[i] && old_gone(&f.path) {
            by_fp.entry(f.fingerprint.as_str()).or_default().push(i);
        }
    }
    // Every candidate pair, best first: same file name, same directory, then result order.
    let mut pairs: Vec<(u8, usize, usize)> = Vec::new();
    for (n, f) in new.iter().enumerate() {
        if matched[n].is_some() || !new_fresh(&f.path) {
            continue;
        }
        for &i in by_fp.get(f.fingerprint.as_str()).into_iter().flatten() {
            let o = &old[i].path;
            let score = if file_name(o) == file_name(&f.path) {
                0
            } else if dir_of(o) == dir_of(&f.path) {
                1
            } else {
                2
            };
            pairs.push((score, n, i));
        }
    }
    pairs.sort_unstable();
    for (_, n, i) in pairs {
        if matched[n].is_none() && !used[i] {
            used[i] = true;
            matched[n] = Some(i);
        }
    }

    // 3. Measured findings of the same symbol.
    let mut by_symbol: FxHashMap<(&str, &str, Option<&str>, String), Vec<usize>> =
        FxHashMap::default();
    for (i, f) in old.iter().enumerate() {
        if !used[i] && f.value.is_some() {
            by_symbol
                .entry((
                    f.path.as_str(),
                    f.rule_id.as_str(),
                    f.symbol.as_deref(),
                    normalized_message(&f.message),
                ))
                .or_default()
                .push(i);
        }
    }
    for (n, f) in new.iter().enumerate() {
        if matched[n].is_some() || f.value.is_none() {
            continue;
        }
        let key = (
            f.path.as_str(),
            f.rule_id.as_str(),
            f.symbol.as_deref(),
            normalized_message(&f.message),
        );
        if let Some(list) = by_symbol.get_mut(&key) {
            if let Some(pos) = list.iter().position(|&i| !used[i]) {
                let i = list.remove(pos);
                used[i] = true;
                matched[n] = Some(i);
            }
        }
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::{FindingStatus, Quality, Severity, TextRange};

    pub(crate) fn finding(path: &str, rule: &str, fp: &str, line: u32) -> Finding {
        Finding {
            id: format!("{path}:{fp}"),
            rule_id: rule.into(),
            message: "m".into(),
            path: path.into(),
            range: TextRange::line(line),
            severity: Severity::Medium,
            quality: Quality::Maintainability,
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

    fn run(old: &[Finding], new: &[Finding], of: &[&str], nf: &[&str]) -> Vec<Option<usize>> {
        let o: Vec<&Finding> = old.iter().collect();
        let n: Vec<&Finding> = new.iter().collect();
        let of: FxHashSet<&str> = of.iter().copied().collect();
        let nf: FxHashSet<&str> = nf.iter().copied().collect();
        match_findings(&o, &n, Files { old: &of, new: &nf })
    }

    #[test]
    fn exact_then_rename_then_measured() {
        let old = vec![
            finding("a.java", "j/x", "f1", 3),
            finding("gone/Old.java", "j/x", "f2", 5),
            finding("b.java", "j/y", "f3", 9),
            {
                let mut f = finding("c.java", "j/cc", "f4", 1);
                f.value = Some(12.0);
                f.symbol = Some("C#m".into());
                f.message = "Complexity 12 over 10".into();
                f
            },
        ];
        let new = vec![
            finding("a.java", "j/x", "f1", 30), // moved down: same fingerprint
            {
                let mut f = finding("c.java", "j/cc", "f4-changed", 1);
                f.value = Some(15.0);
                f.symbol = Some("C#m".into());
                f.message = "Complexity 15 over 10".into();
                f
            },
            finding("moved/Old.java", "j/x", "f2", 5), // renamed file
            finding("z.java", "j/new", "f9", 1),
        ];
        let m = run(
            &old,
            &new,
            &["a.java", "gone/Old.java", "b.java", "c.java"],
            &["a.java", "c.java", "moved/Old.java", "z.java"],
        );
        assert_eq!(m, vec![Some(0), Some(3), Some(1), None]);
    }

    #[test]
    fn fingerprints_do_not_jump_between_files_that_both_still_exist() {
        // The same symbol-less line in two files: a finding fixed in one file and one added in
        // the other are not a rename.
        let old = vec![finding("a.js", "js/todo", "t", 1)];
        let new = vec![finding("b.js", "js/todo", "t", 1)];
        assert_eq!(
            run(&old, &new, &["a.js", "b.js"], &["a.js", "b.js"]),
            vec![None]
        );
        // Without file lists any file may be a rename.
        assert_eq!(run(&old, &new, &[], &[]), vec![Some(0)]);
    }

    #[test]
    fn rename_prefers_the_same_file_name() {
        let old = vec![finding("x/A.java", "j/x", "f", 1)];
        let new = vec![
            finding("y/B.java", "j/x", "f", 1),
            finding("z/A.java", "j/x", "f", 1),
        ];
        assert_eq!(
            run(&old, &new, &["x/A.java"], &["y/B.java", "z/A.java"]),
            vec![None, Some(0)]
        );
    }
}
