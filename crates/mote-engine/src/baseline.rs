//! Comparing a scan with a saved results file instead of a git base (`mote scan
//! --baseline results.json`).
//!
//! Findings that match a baseline finding (see [`matching`](crate::matching)) are `unchanged`
//! (`worsened` when their measured value grew), the others `new`; unmatched, unsuppressed
//! baseline findings in files the scan covers are counted as fixed. There is no line diff, so the
//! gate conditions that need one (duplication and coverage on new code, new-code grades) are
//! skipped.

use crate::matching::{self, Files};
use crate::registry::RuleInfo;
use crate::result::{ScanResult, SeverityCounts, RESULTS_FORMAT_VERSION};
use mote_core::{Finding, FindingStatus};
use rustc_hash::FxHashSet;
use std::collections::BTreeSet;

/// A baseline: the parts of a saved [`ScanResult`] a comparison needs.
#[derive(Clone, Debug, Default)]
pub struct Baseline {
    /// The results file as given (shown in reports).
    pub file: String,
    pub findings: Vec<Finding>,
    /// Paths of the files the baseline scan covered.
    pub files: Vec<String>,
    /// Ids of the rules enabled in the baseline scan.
    pub rules: Vec<String>,
    pub engine_version: String,
    pub format_version: u32,
    /// Commit the baseline scan recorded.
    pub commit: Option<String>,
}

impl Baseline {
    /// Take what a comparison needs from a saved result.
    pub fn from_result(file: impl Into<String>, r: ScanResult) -> Baseline {
        Baseline {
            file: file.into(),
            files: r.files.into_iter().map(|f| f.path).collect(),
            rules: r.rules.into_iter().map(|r| r.id).collect(),
            findings: r.findings,
            engine_version: r.engine_version,
            format_version: r.format_version,
            commit: r.source.commit,
        }
    }

    /// Warnings about differences that make findings look new or fixed although the code did not
    /// change: another engine version, a newer results format, another rule set.
    pub fn warnings(&self, rules: &[RuleInfo]) -> Vec<String> {
        let mut out = Vec::new();
        let this = crate::config::ENGINE_VERSION;
        if !self.engine_version.is_empty() && self.engine_version != this {
            out.push(format!(
                "the baseline {} was written by mote {} (this is {this}); rule changes between \
                 versions show up as new or fixed findings",
                self.file, self.engine_version
            ));
        }
        if self.format_version > RESULTS_FORMAT_VERSION {
            out.push(format!(
                "the baseline {} has results format {} (this mote writes {RESULTS_FORMAT_VERSION}); fields it does not know are ignored",
                self.file, self.format_version
            ));
        }
        if let Some(d) = rule_set_difference(&self.rules, rules.iter().map(|r| r.id.as_str())) {
            out.push(format!(
                "the baseline {} was scanned with other rules ({d}); their findings show up as new or fixed",
                self.file
            ));
        }
        out
    }
}

/// `"2 rules only here: a, b; 1 only there: c"`-style description of two rule sets' difference,
/// or `None` when they are equal. `old` is the baseline's (or the older result's) set.
pub fn rule_set_difference<'a>(
    old: &'a [String],
    new: impl Iterator<Item = &'a str>,
) -> Option<String> {
    let old: BTreeSet<&str> = old.iter().map(String::as_str).collect();
    let new: BTreeSet<&str> = new.collect();
    let added: Vec<&str> = new.difference(&old).copied().collect();
    let removed: Vec<&str> = old.difference(&new).copied().collect();
    if added.is_empty() && removed.is_empty() {
        return None;
    }
    let list = |v: &[&str]| {
        const SHOWN: usize = 5;
        let mut s = v
            .iter()
            .take(SHOWN)
            .map(|x| format!("`{x}`"))
            .collect::<Vec<_>>()
            .join(", ");
        if v.len() > SHOWN {
            s.push_str(&format!(" and {} more", v.len() - SHOWN));
        }
        s
    };
    let mut parts = Vec::new();
    if !added.is_empty() {
        parts.push(format!("{} newly enabled: {}", added.len(), list(&added)));
    }
    if !removed.is_empty() {
        parts.push(format!(
            "{} no longer enabled: {}",
            removed.len(),
            list(&removed)
        ));
    }
    Some(parts.join("; "))
}

/// What classifying against a baseline found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Classified {
    /// Unsuppressed baseline findings with no counterpart, in files `in_scope` accepts.
    pub fixed: SeverityCounts,
    /// Baseline findings that matched.
    pub matched: u64,
}

/// Set the status of every `head` finding against `baseline`. `head_files` are the files the scan
/// covered; `in_scope` says whether a baseline finding's file is within the scan's path filters
/// (only those count as fixed).
pub fn classify(
    head: &mut [&mut Finding],
    head_files: &FxHashSet<&str>,
    baseline: &Baseline,
    in_scope: &dyn Fn(&str) -> bool,
) -> Classified {
    let old: Vec<&Finding> = baseline.findings.iter().collect();
    let old_files: FxHashSet<&str> = baseline.files.iter().map(String::as_str).collect();
    let matched = {
        let view: Vec<&Finding> = head.iter().map(|f| &**f).collect();
        matching::match_findings(
            &old,
            &view,
            Files {
                old: &old_files,
                new: head_files,
            },
        )
    };
    let mut used = vec![false; old.len()];
    for (f, m) in head.iter_mut().zip(&matched) {
        f.status = match m {
            None => FindingStatus::New,
            Some(i) => {
                used[*i] = true;
                match (f.value, old[*i].value) {
                    (Some(h), Some(b)) if h > b => FindingStatus::Worsened,
                    _ => FindingStatus::Unchanged,
                }
            }
        };
    }
    let mut out = Classified {
        matched: used.iter().filter(|u| **u).count() as u64,
        ..Default::default()
    };
    for (b, _) in old.iter().zip(&used).filter(|(_, u)| !**u) {
        if b.suppressed.is_none() && in_scope(&b.path) {
            out.fixed.add(b.severity, 1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::{Quality, Severity, SuppressionInfo, TextRange};

    fn f(path: &str, fp: &str, sev: Severity, value: Option<f64>) -> Finding {
        Finding {
            id: String::new(),
            rule_id: "x/r".into(),
            message: "m".into(),
            path: path.into(),
            range: TextRange::line(1),
            severity: sev,
            quality: Quality::Maintainability,
            symbol: None,
            value,
            fingerprint: fp.into(),
            status: FindingStatus::Unknown,
            incubating: false,
            suppressed: None,
            secondary: vec![],
        }
    }

    #[test]
    fn statuses_and_fixed_counts() {
        let mut suppressed_gone = f("a", "s", Severity::High, None);
        suppressed_gone.suppressed = Some(SuppressionInfo {
            kind: "mote-ignore".into(),
            justification: None,
        });
        let baseline = Baseline {
            file: "base.json".into(),
            findings: vec![
                f("a", "1", Severity::High, None),
                f("a", "2", Severity::Low, Some(10.0)),
                f("a", "3", Severity::Medium, None),   // fixed
                f("out/x", "4", Severity::High, None), // outside the scan's filters
                suppressed_gone,
            ],
            files: vec!["a".into(), "out/x".into()],
            ..Default::default()
        };
        let mut head = [
            f("a", "1", Severity::High, None),
            f("a", "2", Severity::Low, Some(12.0)),
            f("a", "9", Severity::High, None),
        ];
        let mut refs: Vec<&mut Finding> = head.iter_mut().collect();
        let files: FxHashSet<&str> = ["a"].into_iter().collect();
        let c = classify(&mut refs, &files, &baseline, &|p| !p.starts_with("out/"));
        let st: Vec<FindingStatus> = head.iter().map(|f| f.status).collect();
        assert_eq!(
            st,
            [
                FindingStatus::Unchanged,
                FindingStatus::Worsened,
                FindingStatus::New
            ]
        );
        assert_eq!(c.matched, 2);
        assert_eq!(
            c.fixed,
            SeverityCounts {
                high: 0,
                medium: 1,
                low: 0
            }
        );
    }

    #[test]
    fn rule_set_differences_are_described() {
        let old = vec!["a/x".to_string(), "a/y".to_string()];
        assert_eq!(rule_set_difference(&old, ["a/x", "a/y"].into_iter()), None);
        let d = rule_set_difference(&old, ["a/x", "a/z"].into_iter()).unwrap();
        assert_eq!(d, "1 newly enabled: `a/z`; 1 no longer enabled: `a/y`");
        let many: Vec<String> = (0..8).map(|i| format!("r/{i}")).collect();
        let d = rule_set_difference(&[], many.iter().map(String::as_str)).unwrap();
        assert!(d.ends_with("and 3 more"), "{d}");
    }
}
