//! File and project measures, estimated remediation effort, and per-directory rollups.
//!
//! # Estimated effort
//!
//! Every rule declares how long fixing one of its findings roughly takes
//! (`RuleMeta::remediation_minutes`). The estimated effort of a finding is that time; for a
//! threshold rule — a rule with a numeric `threshold`, `max` or `max_lines` parameter whose
//! finding carries the measured `value` — it is scaled by how far the value exceeds the limit in
//! force for the file: `minutes × value / limit`, at least 1× and at most
//! [`MAX_EFFORT_FACTOR`]× (a function with cognitive complexity 45 against a limit of 15 costs
//! three times the base effort). Each finding's effort is rounded to whole minutes, so sums are
//! exact and independent of the order they are added in.
//!
//! Effort counts the findings that can gate ([`gate::gating`](crate::gate::gating)):
//! unsuppressed findings of stable rules. Incubating rules report findings, but — as with the
//! gate — their findings feed neither the effort nor the grades until the rule is promoted, so a
//! rule on probation cannot move a team's numbers. `mote/internal-error` never counts.

use crate::result::SeverityCounts;
use mote_core::{Finding, Quality, Severity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Measures of one file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileMeasures {
    /// Physical lines.
    pub lines: u32,
    /// Lines containing code (non-blank, non-comment).
    pub ncloc: u32,
    /// Lines containing comment text.
    pub comment_lines: u32,
    pub functions: u32,
    pub classes: u32,
    pub statements: u32,
    /// Sum of the functions' cognitive complexity.
    pub cognitive_complexity: u32,
    /// Sum of the functions' cyclomatic complexity.
    pub cyclomatic_complexity: u32,
    /// Lines inside duplicated blocks (filled in by the reduce phase).
    pub duplicated_lines: u32,
    /// Duplicated blocks this file takes part in (reduce phase).
    pub duplicated_blocks: u32,
    /// Syntax errors the parser recovered from.
    pub syntax_errors: u32,
}

/// Estimated remediation effort in minutes, in total and split by severity and by quality (see
/// the [module documentation](self) for how it is estimated and which findings count).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EffortMinutes {
    pub total: u64,
    pub high: u64,
    pub medium: u64,
    pub low: u64,
    pub reliability: u64,
    pub maintainability: u64,
}

impl EffortMinutes {
    /// Add the effort of one finding.
    pub fn add(&mut self, severity: Severity, quality: Quality, minutes: u64) {
        self.total += minutes;
        match severity {
            Severity::High => self.high += minutes,
            Severity::Medium => self.medium += minutes,
            Severity::Low => self.low += minutes,
        }
        match quality {
            Quality::Reliability => self.reliability += minutes,
            Quality::Maintainability => self.maintainability += minutes,
        }
    }

    /// Add another effort.
    pub fn merge(&mut self, o: &EffortMinutes) {
        self.total += o.total;
        self.high += o.high;
        self.medium += o.medium;
        self.low += o.low;
        self.reliability += o.reliability;
        self.maintainability += o.maintainability;
    }
}

/// Parameters holding the limit a threshold rule compares its finding's `value` with.
pub const THRESHOLD_PARAMS: [&str; 3] = ["threshold", "max", "max_lines"];

/// Largest multiple of a rule's base effort one finding can cost.
pub const MAX_EFFORT_FACTOR: f64 = 10.0;

/// Estimated effort of one finding in whole minutes: `base_minutes`, scaled by `value / limit`
/// (clamped to 1..=[`MAX_EFFORT_FACTOR`]) when both are known and the limit is positive.
pub fn finding_effort(base_minutes: u32, value: Option<f64>, limit: Option<f64>) -> u64 {
    let factor = match (value, limit) {
        (Some(v), Some(l)) if v.is_finite() && l.is_finite() && l > 0.0 => {
            (v / l).clamp(1.0, MAX_EFFORT_FACTOR)
        }
        _ => 1.0,
    };
    (f64::from(base_minutes) * factor).round() as u64
}

/// Aggregated measures (per project, per language, main or test code).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Measures {
    /// Files analyzed (fresh or from cache).
    pub files: u64,
    pub lines: u64,
    pub ncloc: u64,
    pub comment_lines: u64,
    pub functions: u64,
    pub classes: u64,
    pub statements: u64,
    pub cognitive_complexity: u64,
    pub cyclomatic_complexity: u64,
    pub duplicated_lines: u64,
    /// Duplicated block occurrences.
    pub duplicated_blocks: u64,
    /// Files with at least one duplicated line.
    pub duplicated_files: u64,
    /// `duplicated_lines / lines × 100` (0 when there are no lines).
    pub duplicated_lines_density: f64,
    /// Findings that are not suppressed.
    pub findings: u64,
    pub findings_high: u64,
    pub findings_medium: u64,
    pub findings_low: u64,
    pub suppressed_findings: u64,
    /// Estimated remediation effort of the findings that can gate (unsuppressed findings of
    /// stable rules).
    #[serde(default)]
    pub effort_minutes: EffortMinutes,
    /// Unsuppressed reliability findings of stable rules, by severity (the reliability grade's
    /// input).
    #[serde(default)]
    pub reliability_findings: SeverityCounts,
}

impl Measures {
    /// Add one file.
    pub fn add_file(&mut self, m: &FileMeasures) {
        self.files += 1;
        self.lines += u64::from(m.lines);
        self.ncloc += u64::from(m.ncloc);
        self.comment_lines += u64::from(m.comment_lines);
        self.functions += u64::from(m.functions);
        self.classes += u64::from(m.classes);
        self.statements += u64::from(m.statements);
        self.cognitive_complexity += u64::from(m.cognitive_complexity);
        self.cyclomatic_complexity += u64::from(m.cyclomatic_complexity);
        self.duplicated_lines += u64::from(m.duplicated_lines);
        self.duplicated_blocks += u64::from(m.duplicated_blocks);
        if m.duplicated_lines > 0 {
            self.duplicated_files += 1;
        }
    }

    /// Add one finding. `effort` is its estimated effort when it counts toward effort and grades
    /// (it can gate), `None` otherwise.
    pub fn add_finding(&mut self, f: &Finding, effort: Option<u64>) {
        if f.suppressed.is_some() {
            self.suppressed_findings += 1;
            return;
        }
        self.findings += 1;
        match f.severity {
            Severity::High => self.findings_high += 1,
            Severity::Medium => self.findings_medium += 1,
            Severity::Low => self.findings_low += 1,
        }
        if let Some(minutes) = effort {
            self.effort_minutes.add(f.severity, f.quality, minutes);
            if f.quality == Quality::Reliability {
                self.reliability_findings.add(f.severity, 1);
            }
        }
    }

    /// Recompute derived ratios.
    pub fn finish(&mut self) {
        self.duplicated_lines_density = percent(self.duplicated_lines, self.lines);
    }
}

/// Measures of one directory, rolled up from its files at every depth (see `metrics.depth`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DirectoryMeasures {
    /// Directory path relative to the scan root, `/`-separated, no trailing `/`.
    pub path: String,
    /// Analyzed files below it.
    pub files: u64,
    pub lines: u64,
    pub ncloc: u64,
    pub cognitive_complexity: u64,
    pub cyclomatic_complexity: u64,
    pub duplicated_lines: u64,
    /// Unsuppressed findings, by severity.
    pub findings: SeverityCounts,
    /// Estimated remediation effort (see [`Measures::effort_minutes`]).
    pub effort_minutes: u64,
}

/// Most directories a result lists; beyond it the smallest (by lines of code) are left out.
pub const MAX_DIRECTORIES: usize = 1_000;

/// Builds [`DirectoryMeasures`] for every directory down to a depth.
#[derive(Debug, Default)]
pub struct DirectoryRollup {
    depth: usize,
    dirs: BTreeMap<String, DirectoryMeasures>,
}

impl DirectoryRollup {
    /// Roll up directories down to `depth` levels below the root (0: none).
    pub fn new(depth: u32) -> Self {
        DirectoryRollup {
            depth: depth as usize,
            dirs: BTreeMap::new(),
        }
    }

    /// Add an analyzed file: to each of its first `depth` ancestor directories. Files at the root
    /// belong to no directory (the project total covers them).
    pub fn add_file(
        &mut self,
        path: &str,
        m: &FileMeasures,
        findings: &SeverityCounts,
        effort_minutes: u64,
    ) {
        for (end, _) in path.match_indices('/').take(self.depth) {
            let dir = &path[..end];
            if dir.is_empty() {
                continue;
            }
            let d = match self.dirs.get_mut(dir) {
                Some(d) => d,
                None => self
                    .dirs
                    .entry(dir.to_string())
                    .or_insert_with(|| DirectoryMeasures {
                        path: dir.to_string(),
                        ..Default::default()
                    }),
            };
            d.files += 1;
            d.lines += u64::from(m.lines);
            d.ncloc += u64::from(m.ncloc);
            d.cognitive_complexity += u64::from(m.cognitive_complexity);
            d.cyclomatic_complexity += u64::from(m.cyclomatic_complexity);
            d.duplicated_lines += u64::from(m.duplicated_lines);
            d.findings.high += findings.high;
            d.findings.medium += findings.medium;
            d.findings.low += findings.low;
            d.effort_minutes += effort_minutes;
        }
    }

    /// The directories sorted by path, at most `max` of them (the largest by lines of code; a
    /// parent is never smaller than its children, so kept directories keep their parents), and
    /// how many were left out.
    pub fn finish(self, max: usize) -> (Vec<DirectoryMeasures>, u64) {
        let mut dirs: Vec<DirectoryMeasures> = self.dirs.into_values().collect();
        let omitted = dirs.len().saturating_sub(max) as u64;
        if omitted > 0 {
            dirs.sort_by(|a, b| b.ncloc.cmp(&a.ncloc).then_with(|| a.path.cmp(&b.path)));
            dirs.truncate(max);
            dirs.sort_by(|a, b| a.path.cmp(&b.path));
        }
        (dirs, omitted)
    }
}

/// `part / whole × 100`, rounded to one decimal; 0 when `whole` is 0.
pub fn percent(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        ((part as f64 / whole as f64) * 1000.0).round() / 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::{FindingStatus, SuppressionInfo, TextRange};

    #[test]
    fn density() {
        let mut m = Measures::default();
        m.add_file(&FileMeasures {
            lines: 200,
            duplicated_lines: 30,
            ..Default::default()
        });
        m.add_file(&FileMeasures {
            lines: 100,
            ..Default::default()
        });
        m.finish();
        assert_eq!(m.files, 2);
        assert_eq!(m.duplicated_files, 1);
        assert_eq!(m.duplicated_lines_density, 10.0);
        assert_eq!(percent(1, 3), 33.3);
        assert_eq!(percent(0, 0), 0.0);
    }

    #[test]
    fn effort_scales_threshold_findings() {
        assert_eq!(finding_effort(10, None, None), 10);
        assert_eq!(finding_effort(10, Some(45.0), None), 10, "no limit known");
        assert_eq!(finding_effort(10, Some(45.0), Some(15.0)), 30);
        assert_eq!(finding_effort(10, Some(16.0), Some(15.0)), 11);
        assert_eq!(finding_effort(10, Some(3.0), Some(15.0)), 10, "at least 1x");
        assert_eq!(
            finding_effort(60, Some(1e9), Some(1000.0)),
            600,
            "at most 10x"
        );
        assert_eq!(finding_effort(10, Some(f64::NAN), Some(15.0)), 10);
        assert_eq!(finding_effort(10, Some(30.0), Some(0.0)), 10);
        assert_eq!(finding_effort(0, Some(30.0), Some(10.0)), 0);
    }

    fn finding(sev: Severity, quality: Quality) -> Finding {
        Finding {
            id: String::new(),
            rule_id: "x/r".into(),
            message: String::new(),
            path: "a".into(),
            range: TextRange::line(1),
            severity: sev,
            quality,
            symbol: None,
            value: None,
            fingerprint: String::new(),
            status: FindingStatus::Unknown,
            incubating: false,
            suppressed: None,
            secondary: vec![],
            fix: None,
        }
    }

    #[test]
    fn findings_and_effort() {
        let mut m = Measures::default();
        m.add_finding(&finding(Severity::High, Quality::Reliability), Some(10));
        m.add_finding(&finding(Severity::Low, Quality::Maintainability), Some(5));
        m.add_finding(&finding(Severity::Medium, Quality::Reliability), None);
        let mut sup = finding(Severity::High, Quality::Reliability);
        sup.suppressed = Some(SuppressionInfo {
            kind: "mote-ignore".into(),
            justification: None,
        });
        m.add_finding(&sup, Some(99));
        assert_eq!(
            (
                m.findings,
                m.findings_high,
                m.findings_medium,
                m.findings_low
            ),
            (3, 1, 1, 1)
        );
        assert_eq!(m.suppressed_findings, 1);
        assert_eq!(
            m.effort_minutes,
            EffortMinutes {
                total: 15,
                high: 10,
                medium: 0,
                low: 5,
                reliability: 10,
                maintainability: 5,
            }
        );
        assert_eq!(
            m.reliability_findings,
            SeverityCounts {
                high: 1,
                medium: 0,
                low: 0
            }
        );
    }

    #[test]
    fn directory_rollup() {
        let fm = FileMeasures {
            lines: 10,
            ncloc: 8,
            cognitive_complexity: 2,
            ..Default::default()
        };
        let one = SeverityCounts {
            high: 1,
            medium: 0,
            low: 2,
        };
        let mut r = DirectoryRollup::new(2);
        r.add_file("Root.java", &fm, &one, 5);
        r.add_file("src/main/a/A.java", &fm, &one, 5);
        r.add_file("src/main/B.java", &fm, &SeverityCounts::default(), 0);
        r.add_file("src/C.java", &fm, &one, 1);
        r.add_file("web/x.ts", &fm, &one, 1);
        let (dirs, omitted) = r.finish(MAX_DIRECTORIES);
        assert_eq!(omitted, 0);
        let paths: Vec<&str> = dirs.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(paths, ["src", "src/main", "web"]);
        assert_eq!(dirs[0].files, 3);
        assert_eq!(dirs[0].ncloc, 24);
        assert_eq!(dirs[0].findings.low, 4);
        assert_eq!(dirs[0].effort_minutes, 6);
        assert_eq!(dirs[1].files, 2);
        assert_eq!(dirs[1].cognitive_complexity, 4);

        let mut r = DirectoryRollup::new(2);
        r.add_file("src/main/a/A.java", &fm, &one, 5);
        r.add_file("src/main/b/B.java", &fm, &one, 5);
        r.add_file("src/test/T.java", &fm, &one, 5);
        r.add_file("docs/D.java", &fm, &one, 5);
        let (dirs, omitted) = r.finish(3);
        assert_eq!(omitted, 1);
        let paths: Vec<&str> = dirs.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(
            paths,
            ["docs", "src", "src/main"],
            "largest kept, parents first"
        );
        assert!(DirectoryRollup::new(0).finish(10).0.is_empty());
    }
}
