//! Maintainability and reliability grades, A (best) to E.
//!
//! Both grades rate a density — how much there is to fix per amount of code — so code bases of
//! different sizes compare fairly:
//!
//! * **Maintainability**: estimated hours to fix the maintainability findings (see
//!   [`measures`](crate::measures) for the effort estimate) per 1,000 lines of code.
//! * **Reliability**: weighted reliability findings per 10,000 lines of code, where a HIGH
//!   finding weighs 1, a MEDIUM one 0.25 and a LOW one nothing (a likely bug matters; a minor
//!   reliability nit alone should not lower the grade).
//!
//! Only the findings that can gate count (unsuppressed findings of stable rules), as for the
//! effort. A value below the first band limit is an A, below the second a B, and so on; at or
//! above the fourth it is an E. The default limits come from scanning open-source repositories
//! (docs/benchmark.md, "Grades"): one estimated hour per 1,000 lines per maintainability grade
//! (1, 2, 3, 4), doubling reliability limits (0.5, 1, 2, 4). They can be changed under `grades:`
//! in the configuration ([`GradesConfig`]). The value is rounded to two decimals before it is
//! graded, so the grade always matches the value shown.
//!
//! On new code (with a base) the same densities are taken over the new findings and the new
//! lines: the number of added or changed lines, blank and comment lines included (the only line
//! count the new-code analysis keeps), so new-code values run slightly lower than a lines-of-code
//! basis would give. A change of fewer new lines than `grades.min_new_lines` (100 by default) is
//! not graded ([`Grades::new_not_rated`]): over a small change a single finding is a density no
//! band was made for — one HIGH reliability finding in 11 new lines would be an E at 909 per
//! 10,000 lines, and one estimated hour of fixes in 38 new lines an E as well — and the gate
//! skips its new-code grade conditions while the new code is not rated. This minimum is apart
//! from `gate.min_new_lines`, which only the gate's duplication and coverage conditions use.
//!
//! Grades are informational unless the gate asks for them (`gate.maintainability_grade`,
//! `gate.new_reliability_grade`, …: "no worse than"). [`GRADES_VERSION`] changes whenever the
//! inputs or the default bands change, so stored grades can be told apart.

use crate::config::GradesConfig;
use crate::result::{NewCodeResult, ProjectMeasures, SeverityCounts};
use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;

/// Version of the grading method and default bands.
pub const GRADES_VERSION: u32 = 1;

/// Weight of a reliability finding in the reliability grade, by severity (HIGH, MEDIUM, LOW).
pub const RELIABILITY_WEIGHTS: [f64; 3] = [1.0, 0.25, 0.0];

/// A grade, `A` (best) to `E`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Grade {
    #[serde(alias = "a")]
    A,
    #[serde(alias = "b")]
    B,
    #[serde(alias = "c")]
    C,
    #[serde(alias = "d")]
    D,
    #[serde(alias = "e")]
    E,
}

impl JsonSchema for Grade {
    fn schema_name() -> Cow<'static, str> {
        "Grade".into()
    }
    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A grade, A (best) to E (any case in the configuration).",
            "type": "string",
            "enum": ["A", "B", "C", "D", "E", "a", "b", "c", "d", "e"]
        })
    }
}

impl Grade {
    /// All grades, best first.
    pub const ALL: [Grade; 5] = [Grade::A, Grade::B, Grade::C, Grade::D, Grade::E];

    /// `A`…`E`.
    pub fn as_str(self) -> &'static str {
        match self {
            Grade::A => "A",
            Grade::B => "B",
            Grade::C => "C",
            Grade::D => "D",
            Grade::E => "E",
        }
    }

    /// 1 (A) to 5 (E): the number gate conditions compare (`actual <= threshold`).
    pub fn rank(self) -> u8 {
        self as u8 + 1
    }

    /// The grade of a rank (see [`rank`](Self::rank)).
    pub fn from_rank(rank: u8) -> Option<Grade> {
        Grade::ALL.get(usize::from(rank).checked_sub(1)?).copied()
    }

    /// Grade `value` against the upper limits of A, B, C and D.
    pub fn of(value: f64, limits: &[f64; 4]) -> Grade {
        limits
            .iter()
            .position(|&l| value < l)
            .map_or(Grade::E, |i| Grade::ALL[i])
    }
}

impl fmt::Display for Grade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One grade with the value it was given for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Rating {
    pub grade: Grade,
    /// The density graded (rounded to two decimals): hours per 1,000 lines of code for
    /// maintainability, weighted findings per 10,000 lines of code for reliability.
    pub value: f64,
    /// Lines the density is taken over: lines of code overall, new lines on new code.
    pub lines: u64,
}

/// The grades of a result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Grades {
    /// [`GRADES_VERSION`] of the method that produced them.
    pub grades_version: u32,
    /// Estimated hours to fix the maintainability findings, per 1,000 lines of code.
    pub maintainability: Rating,
    /// Weighted reliability findings (HIGH 1, MEDIUM 0.25, LOW 0) per 10,000 lines of code.
    pub reliability: Rating,
    /// The maintainability grade of the new code (with a base; `null` when not rated, see
    /// `new_not_rated`).
    pub new_maintainability: Option<Rating>,
    /// The reliability grade of the new code (with a base; `null` when not rated, see
    /// `new_not_rated`).
    pub new_reliability: Option<Rating>,
    /// Why the new code has no grades although there is a base to compare with: too few new
    /// lines for a density to mean anything. Absent when the new code is graded, without a base,
    /// and with a baseline results file (no line diff).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_not_rated: Option<NotRated>,
    /// The settings used: the band limits, and the fewest new lines the new code is graded from
    /// (`min_new_lines`; absent from results written before it existed).
    pub bands: GradesConfig,
}

/// Why the new code is not graded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NotRated {
    /// `too_few_new_lines`. Other reasons may be added; treat unknown ones like this one.
    pub reason: NotRatedReason,
    /// The new lines of the change.
    pub new_lines: u64,
    /// The fewest new lines graded (`grades.min_new_lines`).
    pub min_new_lines: u32,
}

/// See [`NotRated`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotRatedReason {
    /// Fewer new lines than `grades.min_new_lines`: one finding in a small change is a density
    /// no grade band was made for (one HIGH reliability finding in 11 lines is 909 per 10,000
    /// lines, an E).
    TooFewNewLines,
}

impl NotRated {
    /// Why, for people: `only 11 new lines; grades need at least 20`.
    pub fn why(&self) -> String {
        let s = if self.new_lines == 1 { "" } else { "s" };
        format!(
            "only {} new line{s}; grades need at least {}",
            self.new_lines, self.min_new_lines
        )
    }

    /// `not rated (only 11 new lines; grades need at least 20)`.
    pub fn describe(&self) -> String {
        format!("not rated ({})", self.why())
    }
}
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Weighted reliability findings.
pub fn reliability_weight(c: &SeverityCounts) -> f64 {
    f64::from(c.high) * RELIABILITY_WEIGHTS[0]
        + f64::from(c.medium) * RELIABILITY_WEIGHTS[1]
        + f64::from(c.low) * RELIABILITY_WEIGHTS[2]
}

fn density(amount: f64, lines: u64, per: f64) -> f64 {
    if lines == 0 {
        0.0
    } else {
        round2(amount * per / lines as f64)
    }
}

fn maintainability(minutes: u64, lines: u64, bands: &GradesConfig) -> Rating {
    let value = density(minutes as f64 / 60.0, lines, 1_000.0);
    Rating {
        grade: Grade::of(value, &bands.maintainability),
        value,
        lines,
    }
}

fn reliability(c: &SeverityCounts, lines: u64, bands: &GradesConfig) -> Rating {
    let value = density(reliability_weight(c), lines, 10_000.0);
    Rating {
        grade: Grade::of(value, &bands.reliability),
        value,
        lines,
    }
}

/// Grade `measures` (and the new code, when there is a base) with the bands of `config`. The new
/// code is graded from `config.min_new_lines` new lines on (`grades.min_new_lines`); below that
/// its grades are `None` and [`Grades::new_not_rated`] says why.
pub fn compute(
    measures: &ProjectMeasures,
    new_code: Option<&NewCodeResult>,
    config: &GradesConfig,
) -> Grades {
    let min_new_lines = config.min_new_lines;
    let t = &measures.total;
    // A baseline comparison has no new lines to grade against.
    let new = new_code
        .filter(|nc| nc.baseline.is_none())
        .map(|nc| &nc.measures);
    let new_not_rated = new
        .filter(|m| m.new_lines < u64::from(min_new_lines))
        .map(|m| NotRated {
            reason: NotRatedReason::TooFewNewLines,
            new_lines: m.new_lines,
            min_new_lines,
        });
    let new = new.filter(|_| new_not_rated.is_none());
    Grades {
        grades_version: GRADES_VERSION,
        maintainability: maintainability(t.effort_minutes.maintainability, t.ncloc, config),
        reliability: reliability(&t.reliability_findings, t.ncloc, config),
        new_maintainability: new
            .map(|m| maintainability(m.new_effort_minutes.maintainability, m.new_lines, config)),
        new_reliability: new.map(|m| reliability(&m.new_reliability_findings, m.new_lines, config)),
        new_not_rated,
        bands: config.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measures::{EffortMinutes, Measures};
    use crate::result::NewCodeMeasures;

    #[test]
    fn bands_and_ranks() {
        let l = [0.5, 1.0, 2.0, 4.0];
        assert_eq!(Grade::of(0.0, &l), Grade::A);
        assert_eq!(Grade::of(0.49, &l), Grade::A);
        assert_eq!(Grade::of(0.5, &l), Grade::B);
        assert_eq!(Grade::of(1.99, &l), Grade::C);
        assert_eq!(Grade::of(3.99, &l), Grade::D);
        assert_eq!(Grade::of(4.0, &l), Grade::E);
        assert_eq!(Grade::of(f64::NAN, &l), Grade::E);
        for g in Grade::ALL {
            assert_eq!(Grade::from_rank(g.rank()), Some(g));
        }
        assert_eq!(Grade::from_rank(0), None);
        assert_eq!(Grade::from_rank(6), None);
        assert!(Grade::A < Grade::E);
        let g: Grade = serde_json::from_str("\"c\"").unwrap();
        assert_eq!(g, Grade::C);
        assert_eq!(serde_json::to_string(&Grade::D).unwrap(), "\"D\"");
    }

    #[test]
    fn densities() {
        let bands = GradesConfig::default();
        let mut pm = ProjectMeasures {
            total: Measures {
                ncloc: 20_000,
                effort_minutes: EffortMinutes {
                    maintainability: 60 * 30, // 30 h over 20 kLOC: 1.5 h per 1,000 lines
                    reliability: 999,         // not part of the maintainability grade
                    ..Default::default()
                },
                reliability_findings: SeverityCounts {
                    high: 1,
                    medium: 2,
                    low: 50,
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let g = compute(&pm, None, &bands);
        assert_eq!(g.maintainability.value, 1.5);
        // (1 + 2 × 0.25) weighted findings over 2 × 10,000 lines
        assert_eq!(g.reliability.value, 0.75);
        assert_eq!(g.maintainability.lines, 20_000);
        assert_eq!(g.grades_version, GRADES_VERSION);
        assert!(g.new_maintainability.is_none() && g.new_reliability.is_none());
        pm.total.ncloc = 0;
        let g = compute(&pm, None, &bands);
        assert_eq!(
            (g.maintainability.grade, g.reliability.grade),
            (Grade::A, Grade::A),
            "no code, nothing to fix"
        );
    }

    #[test]
    fn new_code_uses_new_findings_and_lines() {
        let nc = NewCodeResult {
            base: "main".into(),
            merge_base: "0".into(),
            new_lines: Default::default(),
            renamed: Default::default(),
            measures: NewCodeMeasures {
                new_lines: 400,
                new_effort_minutes: EffortMinutes {
                    maintainability: 12, // 0.2 h over 0.4 kLOC
                    ..Default::default()
                },
                new_reliability_findings: SeverityCounts {
                    high: 0,
                    medium: 0,
                    low: 3,
                },
                ..Default::default()
            },
            baseline: None,
        };
        let g = compute(
            &ProjectMeasures::default(),
            Some(&nc),
            &GradesConfig::default(),
        );
        let m = g.new_maintainability.unwrap();
        assert_eq!((m.value, m.lines), (0.5, 400));
        assert_eq!(
            g.new_reliability.unwrap().grade,
            Grade::A,
            "LOW weighs nothing"
        );
        assert_eq!(g.new_not_rated, None);
        // A baseline comparison has no new lines: no new-code grades.
        let based = NewCodeResult {
            baseline: Some(Default::default()),
            ..nc.clone()
        };
        let g = compute(
            &ProjectMeasures::default(),
            Some(&based),
            &GradesConfig::default(),
        );
        assert!(g.new_maintainability.is_none() && g.new_reliability.is_none());
        assert_eq!(g.new_not_rated, None);
    }

    #[test]
    fn a_few_new_lines_are_not_rated() {
        // One HIGH reliability finding in 11 new lines: 9,090.91 per 10,000 lines, an E that
        // says nothing about the change.
        let mut nc = NewCodeResult {
            base: "main".into(),
            merge_base: "0".into(),
            new_lines: Default::default(),
            renamed: Default::default(),
            measures: NewCodeMeasures {
                new_lines: 11,
                new_reliability_findings: SeverityCounts {
                    high: 1,
                    medium: 0,
                    low: 0,
                },
                ..Default::default()
            },
            baseline: None,
        };
        let bands = GradesConfig::default();
        let g = compute(&ProjectMeasures::default(), Some(&nc), &bands);
        assert!(g.new_maintainability.is_none() && g.new_reliability.is_none());
        let not_rated = g.new_not_rated.clone().unwrap();
        assert_eq!(
            not_rated,
            NotRated {
                reason: NotRatedReason::TooFewNewLines,
                new_lines: 11,
                min_new_lines: 100
            }
        );
        assert_eq!(
            not_rated.describe(),
            "not rated (only 11 new lines; grades need at least 100)"
        );
        // The overall grades are unaffected.
        assert_eq!(g.maintainability.grade, Grade::A);
        // In JSON: the ratings are null and the reason is given; graded results carry no
        // `new_not_rated` at all (the field is additive).
        let json = serde_json::to_value(&g).unwrap();
        assert!(json["new_reliability"].is_null());
        assert_eq!(
            json["new_not_rated"],
            serde_json::json!({"reason": "too_few_new_lines", "new_lines": 11, "min_new_lines": 100})
        );
        assert_eq!(json["bands"]["min_new_lines"], 100);
        let back: Grades = serde_json::from_value(json).unwrap();
        assert_eq!(back, g);
        // One line below the minimum: not rated; at the minimum: rated.
        nc.measures.new_lines = 99;
        let g = compute(&ProjectMeasures::default(), Some(&nc), &bands);
        assert_eq!(g.new_not_rated.as_ref().map(|n| n.new_lines), Some(99));
        assert!(g.new_reliability.is_none());
        nc.measures.new_lines = 100;
        let g = compute(&ProjectMeasures::default(), Some(&nc), &bands);
        assert_eq!(g.new_reliability.as_ref().unwrap().grade, Grade::E);
        assert_eq!(g.new_not_rated, None);
        let json = serde_json::to_value(&g).unwrap();
        assert!(json.get("new_not_rated").is_none());
        // `grades.min_new_lines: 0` grades every change.
        nc.measures.new_lines = 1;
        let every = GradesConfig {
            min_new_lines: 0,
            ..GradesConfig::default()
        };
        let g = compute(&ProjectMeasures::default(), Some(&nc), &every);
        assert!(g.new_reliability.is_some());
        // Results written before the fields existed read back without them (the minimum then
        // reads as the default).
        let old = serde_json::json!({
            "grades_version": 1,
            "maintainability": {"grade": "A", "value": 0.0, "lines": 0},
            "reliability": {"grade": "A", "value": 0.0, "lines": 0},
            "new_maintainability": null,
            "new_reliability": null,
            "bands": {"maintainability": [1.0, 2.0, 3.0, 4.0], "reliability": [0.5, 1.0, 2.0, 4.0]}
        });
        let g: Grades = serde_json::from_value(old).unwrap();
        assert_eq!(g.new_not_rated, None);
        assert_eq!(g.bands, GradesConfig::default());
    }
}
