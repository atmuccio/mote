//! Quality gate evaluation.
//!
//! Conditions (a skipped condition counts as passed and says why it was skipped):
//!
//! * `new_findings` — no *new*, non-suppressed, non-incubating finding at or above `fail_on`
//!   (`mote/internal-error` never counts). Without a base every finding counts and the
//!   condition is named `findings`.
//! * `new_duplication_pct` — duplication on new lines at most the threshold; needs a base and at
//!   least `min_new_lines` new lines.
//! * `new_coverage_pct` — coverage of new coverable lines at least the threshold; needs a base,
//!   coverage reports, and at least `min_new_lines` new coverable lines.
//! * `worsened` — only when `worsened: fail`: no worsened findings.
//! * `maintainability_grade`, `reliability_grade` — only when set: the overall grade is no worse
//!   than the setting (see [`grades`](crate::grades)); `new_maintainability_grade`,
//!   `new_reliability_grade` — the same on new code; they need a base and new code that is
//!   rated, at least `grades.min_new_lines` new lines (not `gate.min_new_lines`, which only the
//!   duplication and coverage conditions use). A grade condition's `actual` and `threshold` are
//!   ranks (A = 1 … E = 5), so it passes when `actual <= threshold`.
//!
//! A comparison with a baseline results file (`scan --baseline`) counts as a base for the
//! findings conditions, but has no line diff: the duplication, coverage and new-code grade
//! conditions are then skipped ([`NO_LINE_DIFF`]).
//!
//! Suppressed findings never gate: in-source suppressions and findings the triage file marks
//! `accepted` or `false-positive` alike.

use crate::config::{Config, GateConfig, WorsenedPolicy};
use crate::coverage::RepoCoverage;
use crate::generic::MetaRule;
use crate::grades::{self, Grade, Grades, Rating};
use crate::measures::percent;
use crate::result::{CoverageSummary, FileStatus, ScanResult};
use mote_core::{Finding, FindingStatus};
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};

/// Result of one gate condition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ConditionResult {
    pub name: String,
    pub passed: bool,
    pub actual: Option<f64>,
    pub threshold: Option<f64>,
    /// Why the condition was not evaluated (it then counts as passed).
    pub skipped_reason: Option<String>,
}

/// Overall gate result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GateResult {
    pub passed: bool,
    pub conditions: Vec<ConditionResult>,
}

/// New-code numbers the gate needs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NewCodeNumbers {
    pub new_lines: u64,
    pub new_duplicated_lines: u64,
    /// `None` when no coverage reports were supplied.
    pub coverable: Option<u64>,
    pub covered: u64,
    /// The comparison was with a baseline results file: there is no line diff, so conditions
    /// on new lines are skipped.
    pub baseline: bool,
}

/// Skip reason of line-based new-code conditions when comparing with a baseline file.
pub const NO_LINE_DIFF: &str = "compared with a baseline results file, which has no line diff";

/// Whether a finding can fail a gate at all.
pub fn gating(f: &Finding) -> bool {
    f.suppressed.is_none() && !f.incubating && f.rule_id != MetaRule::InternalError.id()
}

fn skipped(name: &str, threshold: Option<f64>, reason: &str) -> ConditionResult {
    ConditionResult {
        name: name.into(),
        passed: true,
        actual: None,
        threshold,
        skipped_reason: Some(reason.into()),
    }
}

/// The skip reason of a condition on new lines below its minimum (`gate.min_new_lines` for the
/// duplication and coverage conditions, `grades.min_new_lines` for the new-code grades).
fn too_few_new_lines(new_lines: u64, min: u32) -> String {
    let s = if new_lines == 1 { "" } else { "s" };
    format!("only {new_lines} new line{s} (minimum {min})")
}

fn pct(part: u64, whole: u64) -> f64 {
    crate::measures::percent(part, whole)
}

/// Evaluate the gate. `new_code` is `None` when no base was given.
pub fn evaluate(
    config: &GateConfig,
    findings: &[Finding],
    new_code: Option<&NewCodeNumbers>,
) -> GateResult {
    let mut conditions = Vec::new();
    let has_base = new_code.is_some();

    if let Some(min) = config.fail_on.severity() {
        let count = findings
            .iter()
            .filter(|f| {
                gating(f) && f.severity >= min && (!has_base || f.status == FindingStatus::New)
            })
            .count();
        conditions.push(ConditionResult {
            name: if has_base { "new_findings" } else { "findings" }.into(),
            passed: count == 0,
            actual: Some(count as f64),
            threshold: Some(0.0),
            skipped_reason: None,
        });
    }

    if let Some(max) = config.new_duplication_pct {
        const NAME: &str = "new_duplication_pct";
        conditions.push(match new_code {
            None => skipped(NAME, Some(max), "no base to compare with"),
            Some(n) if n.baseline => skipped(NAME, Some(max), NO_LINE_DIFF),
            Some(n) if n.new_lines < u64::from(config.min_new_lines) => skipped(
                NAME,
                Some(max),
                &too_few_new_lines(n.new_lines, config.min_new_lines),
            ),
            Some(n) => {
                let actual = pct(n.new_duplicated_lines, n.new_lines);
                ConditionResult {
                    name: NAME.into(),
                    passed: actual <= max,
                    actual: Some(actual),
                    threshold: Some(max),
                    skipped_reason: None,
                }
            }
        });
    }

    if let Some(min) = config.new_coverage_pct {
        const NAME: &str = "new_coverage_pct";
        conditions.push(match new_code {
            None => skipped(NAME, Some(min), "no base to compare with"),
            Some(n) if n.baseline => skipped(NAME, Some(min), NO_LINE_DIFF),
            Some(NewCodeNumbers {
                coverable: None, ..
            }) => skipped(NAME, Some(min), "no coverage reports"),
            Some(n) => {
                let coverable = n.coverable.unwrap_or(0);
                if coverable < u64::from(config.min_new_lines) {
                    skipped(
                        NAME,
                        Some(min),
                        &format!(
                            "only {coverable} new coverable lines (minimum {})",
                            config.min_new_lines
                        ),
                    )
                } else {
                    let actual = pct(n.covered, coverable);
                    ConditionResult {
                        name: NAME.into(),
                        passed: actual >= min,
                        actual: Some(actual),
                        threshold: Some(min),
                        skipped_reason: None,
                    }
                }
            }
        });
    }

    if config.worsened == WorsenedPolicy::Fail {
        conditions.push(if has_base {
            let count = findings
                .iter()
                .filter(|f| gating(f) && f.status == FindingStatus::Worsened)
                .count();
            ConditionResult {
                name: "worsened".into(),
                passed: count == 0,
                actual: Some(count as f64),
                threshold: Some(0.0),
                skipped_reason: None,
            }
        } else {
            skipped("worsened", Some(0.0), "no base to compare with")
        });
    }

    GateResult {
        passed: conditions.iter().all(|c| c.passed),
        conditions,
    }
}

/// The new-code numbers of `result` (`None` without a base).
pub fn new_code_numbers(result: &ScanResult) -> Option<NewCodeNumbers> {
    result.new_code.as_ref().map(|nc| NewCodeNumbers {
        new_lines: nc.measures.new_lines,
        new_duplicated_lines: nc.measures.new_duplicated_lines,
        coverable: nc.measures.new_coverable_lines,
        covered: nc.measures.new_covered_lines.unwrap_or(0),
        baseline: nc.baseline.is_some(),
    })
}

/// Set the coverage measures of `result` from `coverage`: overall line coverage over the files
/// that were analyzed (not skipped), and — with a base — coverage of the new lines. A scan uses
/// this for `coverage.reports`; `mote gate --coverage` uses it to add coverage to a saved
/// result. Call [`reevaluate_gate`] afterwards.
pub fn apply_coverage(result: &mut ScanResult, coverage: &RepoCoverage) {
    let analyzed: FxHashSet<&str> = result
        .files
        .iter()
        .filter(|f| f.status != FileStatus::Skipped)
        .map(|f| f.path.as_str())
        .collect();
    let (mut coverable, mut covered) = (0u64, 0u64);
    for (path, lines) in &coverage.files {
        if analyzed.contains(path.as_str()) {
            coverable += lines.len() as u64;
            covered += lines.values().filter(|&&v| v).count() as u64;
        }
    }
    result.measures.coverage = Some(CoverageSummary {
        coverable_lines: coverable,
        covered_lines: covered,
        coverage_pct: percent(covered, coverable),
    });
    if let Some(nc) = result.new_code.as_mut() {
        let (mut coverable, mut covered) = (0u64, 0u64);
        for (path, ranges) in &nc.new_lines {
            let Some(lines) = coverage.files.get(path) else {
                continue;
            };
            for &(s, e) in ranges {
                if s > e {
                    continue; // hand-edited results: `range` panics on inverted bounds
                }
                for (_, &c) in lines.range(s..=e) {
                    coverable += 1;
                    covered += u64::from(c);
                }
            }
        }
        nc.measures.new_coverable_lines = Some(coverable);
        nc.measures.new_covered_lines = Some(covered);
        nc.measures.new_coverage_pct = Some(percent(covered, coverable));
    }
}

/// The grade conditions `config` asks for, in order: overall maintainability and reliability, then
/// both on new code. `new_code` is `None` when no base was given.
pub fn evaluate_grades(
    config: &GateConfig,
    grades: &Grades,
    new_code: Option<&NewCodeNumbers>,
) -> Vec<ConditionResult> {
    let condition = |name: &str, worst: Grade, r: &Rating| {
        let (actual, threshold) = (r.grade.rank(), worst.rank());
        ConditionResult {
            name: name.into(),
            passed: actual <= threshold,
            actual: Some(f64::from(actual)),
            threshold: Some(f64::from(threshold)),
            skipped_reason: None,
        }
    };
    let mut out = Vec::new();
    for (name, worst, rating) in [
        (
            "maintainability_grade",
            config.maintainability_grade,
            &grades.maintainability,
        ),
        (
            "reliability_grade",
            config.reliability_grade,
            &grades.reliability,
        ),
    ] {
        if let Some(worst) = worst {
            out.push(condition(name, worst, rating));
        }
    }
    for (name, worst, rating) in [
        (
            "new_maintainability_grade",
            config.new_maintainability_grade,
            grades.new_maintainability.as_ref(),
        ),
        (
            "new_reliability_grade",
            config.new_reliability_grade,
            grades.new_reliability.as_ref(),
        ),
    ] {
        let Some(worst) = worst else { continue };
        let threshold = Some(f64::from(worst.rank()));
        // The minimum that decides whether the new code is graded at all (`grades::compute`,
        // `grades.min_new_lines`, carried in `grades.bands`) decides here too, so a skipped
        // condition and a "not rated" grade always go together.
        let min = grades.bands.min_new_lines;
        out.push(match (new_code, rating) {
            (Some(n), _) if n.baseline => skipped(name, threshold, NO_LINE_DIFF),
            (Some(n), _) if n.new_lines < u64::from(min) => {
                skipped(name, threshold, &too_few_new_lines(n.new_lines, min))
            }
            (Some(_), Some(r)) => condition(name, worst, r),
            _ => skipped(name, threshold, "no base to compare with"),
        });
    }
    out
}

/// Recompute `result.grades` and re-evaluate `result.gate` with the settings of `config` (the
/// findings, new-code and coverage measures already in `result` are used as they are).
pub fn reevaluate_gate(result: &mut ScanResult, config: &Config) {
    let numbers = new_code_numbers(result);
    let g = grades::compute(&result.measures, result.new_code.as_ref(), &config.grades);
    let mut gate = evaluate(&config.gate, &result.findings, numbers.as_ref());
    gate.conditions
        .extend(evaluate_grades(&config.gate, &g, numbers.as_ref()));
    gate.passed = gate.conditions.iter().all(|c| c.passed);
    result.gate = gate;
    result.grades = Some(g);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FailOn;
    use mote_core::{Quality, Severity, SuppressionInfo, TextRange};

    fn f(sev: Severity, status: FindingStatus) -> Finding {
        Finding {
            id: String::new(),
            rule_id: "x/r".into(),
            message: String::new(),
            path: "a".into(),
            range: TextRange::line(1),
            severity: sev,
            quality: Quality::Reliability,
            symbol: None,
            value: None,
            fingerprint: String::new(),
            status,
            incubating: false,
            suppressed: None,
            secondary: vec![],
        }
    }

    #[test]
    fn severity_condition_with_and_without_base() {
        let cfg = GateConfig::default();
        let mut sup = f(Severity::High, FindingStatus::New);
        sup.suppressed = Some(SuppressionInfo {
            kind: "mote-ignore".into(),
            justification: None,
        });
        let mut inc = f(Severity::High, FindingStatus::New);
        inc.incubating = true;
        let mut internal = f(Severity::High, FindingStatus::New);
        internal.rule_id = "mote/internal-error".into();
        let findings = vec![
            f(Severity::High, FindingStatus::Unchanged),
            f(Severity::Medium, FindingStatus::New),
            sup,
            inc,
            internal,
        ];
        let nc = NewCodeNumbers::default();
        let g = evaluate(&cfg, &findings, Some(&nc));
        assert!(g.passed, "{g:?}");
        let g = evaluate(&cfg, &findings, None);
        assert!(!g.passed);
        assert_eq!(g.conditions[0].name, "findings");
        assert_eq!(g.conditions[0].actual, Some(1.0));
        let medium = GateConfig {
            fail_on: FailOn::Medium,
            ..GateConfig::default()
        };
        assert!(!evaluate(&medium, &findings, Some(&nc)).passed);
        let none = GateConfig {
            fail_on: FailOn::None,
            ..GateConfig::default()
        };
        assert!(evaluate(&none, &findings, None).passed);
    }

    #[test]
    fn ratio_conditions() {
        let cfg = GateConfig::default();
        let small = NewCodeNumbers {
            new_lines: 10,
            new_duplicated_lines: 10,
            coverable: Some(5),
            covered: 0,
            baseline: false,
        };
        let g = evaluate(&cfg, &[], Some(&small));
        assert!(g.passed);
        assert!(g
            .conditions
            .iter()
            .filter(|c| c.name != "new_findings")
            .all(|c| c.skipped_reason.is_some()));
        let big = NewCodeNumbers {
            new_lines: 100,
            new_duplicated_lines: 5,
            coverable: Some(50),
            covered: 30,
            baseline: false,
        };
        let g = evaluate(&cfg, &[], Some(&big));
        let dup = g
            .conditions
            .iter()
            .find(|c| c.name == "new_duplication_pct")
            .unwrap();
        assert!(!dup.passed);
        assert_eq!(dup.actual, Some(5.0));
        let cov = g
            .conditions
            .iter()
            .find(|c| c.name == "new_coverage_pct")
            .unwrap();
        assert!(!cov.passed);
        assert_eq!(cov.actual, Some(60.0));
        assert!(!g.passed);
        let nocov = NewCodeNumbers {
            coverable: None,
            ..big
        };
        let g = evaluate(&cfg, &[], Some(&nocov));
        let cov = g
            .conditions
            .iter()
            .find(|c| c.name == "new_coverage_pct")
            .unwrap();
        assert_eq!(cov.skipped_reason.as_deref(), Some("no coverage reports"));
    }

    #[test]
    fn grade_conditions() {
        let g = grades::compute(
            &crate::result::ProjectMeasures {
                total: crate::measures::Measures {
                    ncloc: 1_000,
                    effort_minutes: crate::measures::EffortMinutes {
                        maintainability: 150, // 2.5 h per 1,000 lines: C
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            },
            None,
            &crate::config::GradesConfig::default(),
        );
        assert_eq!(g.maintainability.grade, Grade::C);
        assert!(
            evaluate_grades(&GateConfig::default(), &g, None).is_empty(),
            "opt-in"
        );
        let cfg = GateConfig {
            maintainability_grade: Some(Grade::B),
            reliability_grade: Some(Grade::A),
            new_maintainability_grade: Some(Grade::C),
            ..GateConfig::default()
        };
        let c = evaluate_grades(&cfg, &g, None);
        let names: Vec<&str> = c.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "maintainability_grade",
                "reliability_grade",
                "new_maintainability_grade"
            ]
        );
        assert!(!c[0].passed);
        assert_eq!((c[0].actual, c[0].threshold), (Some(3.0), Some(2.0)));
        assert!(c[1].passed, "no reliability findings: A");
        assert_eq!(
            c[2].skipped_reason.as_deref(),
            Some("no base to compare with")
        );
        let cfg = GateConfig {
            maintainability_grade: Some(Grade::C),
            ..cfg
        };
        assert!(
            evaluate_grades(&cfg, &g, None)[0].passed,
            "C is no worse than C"
        );

        // New code: skipped below `grades.min_new_lines` (100 by default), evaluated from it on.
        let mut g = g;
        g.new_maintainability = Some(Rating {
            grade: Grade::D,
            value: 3.0,
            lines: 10,
        });
        let few = NewCodeNumbers {
            new_lines: 10,
            ..Default::default()
        };
        let c = evaluate_grades(&cfg, &g, Some(&few));
        assert_eq!(
            c[2].skipped_reason.as_deref(),
            Some("only 10 new lines (minimum 100)")
        );
        // Not rated (what `grades::compute` gives for 10 new lines): the same reason, not "no
        // base".
        let unrated = Grades {
            new_maintainability: None,
            new_reliability: None,
            ..g.clone()
        };
        let c = evaluate_grades(&cfg, &unrated, Some(&few));
        assert_eq!(
            c[2].skipped_reason.as_deref(),
            Some("only 10 new lines (minimum 100)")
        );
        assert!(c[2].passed);
        // Between the two minimums: the gate's duplication condition is evaluated (it follows
        // `gate.min_new_lines`), the grade condition is not (the new code is not rated).
        let some = NewCodeNumbers {
            new_lines: 99,
            ..Default::default()
        };
        let c = evaluate_grades(&cfg, &unrated, Some(&some));
        assert_eq!(
            c[2].skipped_reason.as_deref(),
            Some("only 99 new lines (minimum 100)")
        );
        let dup = evaluate(&cfg, &[], Some(&some));
        let dup = dup
            .conditions
            .iter()
            .find(|c| c.name == "new_duplication_pct")
            .unwrap();
        assert!(dup.skipped_reason.is_none() && dup.passed, "{dup:?}");
        let at_min = NewCodeNumbers {
            new_lines: 100,
            ..Default::default()
        };
        let c = evaluate_grades(&cfg, &g, Some(&at_min));
        assert!(!c[2].passed && c[2].skipped_reason.is_none());
        // `grades.min_new_lines` lower than the gate's: graded, so evaluated.
        let low = Grades {
            bands: crate::config::GradesConfig {
                min_new_lines: 5,
                ..crate::config::GradesConfig::default()
            },
            ..g.clone()
        };
        let c = evaluate_grades(&cfg, &low, Some(&few));
        assert!(!c[2].passed && c[2].skipped_reason.is_none());
    }

    #[test]
    fn a_baseline_comparison_skips_line_based_conditions() {
        let cfg = GateConfig {
            new_maintainability_grade: Some(Grade::A),
            ..GateConfig::default()
        };
        let n = NewCodeNumbers {
            baseline: true,
            ..Default::default()
        };
        let findings = vec![
            f(Severity::High, FindingStatus::Unchanged),
            f(Severity::High, FindingStatus::New),
        ];
        let g = evaluate(&cfg, &findings, Some(&n));
        assert_eq!(g.conditions[0].name, "new_findings");
        assert_eq!(g.conditions[0].actual, Some(1.0), "only the new one");
        for c in g.conditions.iter().skip(1) {
            assert_eq!(c.skipped_reason.as_deref(), Some(NO_LINE_DIFF), "{c:?}");
        }
        let grades = grades::compute(
            &crate::result::ProjectMeasures::default(),
            None,
            &crate::config::GradesConfig::default(),
        );
        let c = evaluate_grades(&cfg, &grades, Some(&n));
        assert_eq!(c[0].skipped_reason.as_deref(), Some(NO_LINE_DIFF));
    }

    #[test]
    fn worsened_policy() {
        let cfg = GateConfig {
            worsened: WorsenedPolicy::Fail,
            ..GateConfig::default()
        };
        let findings = vec![f(Severity::Low, FindingStatus::Worsened)];
        assert!(!evaluate(&cfg, &findings, Some(&NewCodeNumbers::default())).passed);
        assert!(
            evaluate(
                &GateConfig::default(),
                &findings,
                Some(&NewCodeNumbers::default())
            )
            .passed
        );
    }
}
