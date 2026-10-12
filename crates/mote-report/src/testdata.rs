//! Hand-built scan results and a tiny golden-file helper for the format tests.

use mote_core::{
    Finding, FindingStatus, Location, ParamValue, Quality, RuleKind, RuleStatus, Severity,
    SuppressionInfo, TextRange,
};
use mote_engine::gate::{ConditionResult, GateResult};
use mote_engine::measures::{DirectoryMeasures, EffortMinutes, FileMeasures, Measures};
use mote_engine::newcode::ChangeKind;
use mote_engine::registry::{ParamInfo, RuleOrigin};
use mote_engine::result::{
    Diagnostic, DuplicationBlock, DuplicationOccurrence, FileStatus, FileSummary, NewCodeMeasures,
    NewCodeResult, ProjectMeasures, ScanStats, SeverityCounts,
};
use mote_engine::source::SourceInfo;
use mote_engine::{RuleInfo, ScanResult};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn rule(id: &str, name: &str, severity: Severity, quality: Quality) -> RuleInfo {
    let language = id.split('/').next().unwrap_or("x").to_string();
    RuleInfo {
        id: id.into(),
        name: name.into(),
        language,
        kind: if quality == Quality::Reliability {
            RuleKind::Bug
        } else {
            RuleKind::CodeSmell
        },
        quality,
        severity,
        status: RuleStatus::Stable,
        default_enabled: true,
        tags: vec!["demo".into()],
        params: vec![],
        description: format!(
            "Flags {name}.\n\nWhy: it hurts.\n\n```java\nbad();\n```\n\nFix: `good()`."
        ),
        remediation_minutes: 5,
        origin: RuleOrigin::Plugin,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn finding(
    id: &str,
    rule_id: &str,
    path: &str,
    range: (u32, u32, u32, u32),
    severity: Severity,
    status: FindingStatus,
    message: &str,
) -> Finding {
    Finding {
        id: id.into(),
        rule_id: rule_id.into(),
        message: message.into(),
        path: path.into(),
        range: TextRange {
            start_line: range.0,
            start_col: range.1,
            end_line: range.2,
            end_col: range.3,
        },
        severity,
        quality: if severity == Severity::High {
            Quality::Reliability
        } else {
            Quality::Maintainability
        },
        symbol: Some("Demo#run".into()),
        value: None,
        fingerprint: format!("fp-{id}"),
        status,
        incubating: false,
        suppressed: None,
        secondary: vec![],
    }
}

fn dir(path: &str, files: u64, ncloc: u64, findings: u32, effort: u64) -> DirectoryMeasures {
    DirectoryMeasures {
        path: path.into(),
        files,
        lines: files * 100,
        ncloc,
        cognitive_complexity: 19 * files,
        cyclomatic_complexity: 26 * files,
        duplicated_lines: 0,
        findings: SeverityCounts {
            high: 0,
            medium: findings,
            low: 0,
        },
        effort_minutes: effort,
    }
}

fn file(path: &str, status: FileStatus, findings: u32, change: Option<ChangeKind>) -> FileSummary {
    FileSummary {
        path: path.into(),
        language: "java".into(),
        status,
        skip_reason: None,
        is_test: false,
        measures: Some(FileMeasures {
            lines: 100,
            ncloc: 80,
            ..Default::default()
        }),
        findings,
        effort_minutes: 5 * u64::from(findings),
        change,
    }
}

/// Compute the grades of `r` with the default bands and minimum (what `reevaluate_gate` does).
pub fn grade(r: &mut ScanResult) {
    grade_from(
        r,
        mote_engine::config::GradesConfig::default().min_new_lines,
    );
}

/// Compute the grades of `r` with the default bands, grading new code from `min_new_lines` new
/// lines on (`grades.min_new_lines`).
pub fn grade_from(r: &mut ScanResult, min_new_lines: u32) {
    r.grades = Some(mote_engine::grades::compute(
        &r.measures,
        r.new_code.as_ref(),
        &mote_engine::config::GradesConfig {
            min_new_lines,
            ..Default::default()
        },
    ));
}

/// [`with_base`] for a change of 11 new lines: too few to grade the new code.
pub fn small_change() -> ScanResult {
    change_of(11)
}

/// [`with_base`] for a change of `new_lines` new lines, graded with the default minimum.
pub fn change_of(new_lines: u64) -> ScanResult {
    let mut r = with_base();
    if let Some(nc) = r.new_code.as_mut() {
        nc.measures.new_lines = new_lines;
    }
    grade(&mut r);
    r
}

/// A result compared with a base: new, unchanged, worsened and suppressed findings, a failed gate.
pub fn with_base() -> ScanResult {
    use FindingStatus::*;
    use Severity::*;
    let mut sup = finding(
        "sup000000001",
        "java/todo-comment",
        "src/b/Legacy.java",
        (4, 1, 4, 20),
        Low,
        New,
        "Complete the task in this TODO comment",
    );
    sup.suppressed = Some(SuppressionInfo {
        kind: "mote-ignore".into(),
        justification: Some("tracked in JIRA-1".into()),
    });
    let mut worse = finding(
        "wrs000000001",
        "java/cognitive-complexity",
        "src/a/Order Service.java",
        (20, 10, 20, 15),
        Medium,
        Worsened,
        "Reduce the cognitive complexity of `total` from 22 to 15",
    );
    worse.value = Some(22.0);
    let mut dup = finding(
        "dup000000001",
        "java/duplicated-block",
        "src/a/Order Service.java",
        (40, 1, 60, 2),
        Medium,
        New,
        "This block of 21 lines is duplicated in 1 other place",
    );
    dup.secondary = vec![Location {
        path: "src/b/Legacy.java".into(),
        range: TextRange {
            start_line: 10,
            start_col: 1,
            end_line: 30,
            end_col: 2,
        },
        message: Some("Duplicate".into()),
    }];
    dup.symbol = None;
    let findings = vec![
        finding(
            "new000000001",
            "java/self-assignment",
            "src/a/Order Service.java",
            (3, 5, 3, 12),
            High,
            New,
            "Remove this self-assignment of `total` | it has no effect",
        ),
        worse,
        dup,
        finding(
            "unc000000001",
            "java/empty-catch-block",
            "src/b/Legacy.java",
            (2, 9, 2, 14),
            Medium,
            Unchanged,
            "Remove this empty catch block or explain why the exception is ignored",
        ),
        sup,
        finding(
            "new000000002",
            "java/todo-comment",
            "src/c/Util.java",
            (1, 1, 1, 1),
            Low,
            New,
            "Complete the task in this TODO comment",
        ),
    ];
    let mut total = Measures {
        files: 3,
        lines: 300,
        ncloc: 240,
        functions: 12,
        cognitive_complexity: 57,
        cyclomatic_complexity: 80,
        duplicated_lines: 42,
        duplicated_blocks: 2,
        duplicated_files: 2,
        findings: 5,
        findings_high: 1,
        findings_medium: 3,
        findings_low: 1,
        suppressed_findings: 1,
        effort_minutes: EffortMinutes {
            total: 57,
            high: 5,
            medium: 42,
            low: 10,
            reliability: 5,
            maintainability: 52,
        },
        reliability_findings: SeverityCounts {
            high: 1,
            medium: 0,
            low: 0,
        },
        ..Default::default()
    };
    total.finish();
    let mut languages = BTreeMap::new();
    languages.insert("java".to_string(), total.clone());
    let mut r = ScanResult {
        format_version: 1,
        engine_version: "0.1.0".into(),
        source: SourceInfo {
            kind: "worktree".into(),
            rev: None,
            commit: Some("0123456789abcdef0123456789abcdef01234567".into()),
            prefix: String::new(),
        },
        findings,
        files: vec![
            file(
                "src/a/Order Service.java",
                FileStatus::Analyzed,
                3,
                Some(ChangeKind::Modified),
            ),
            file("src/b/Legacy.java", FileStatus::Cached, 1, None),
            file(
                "src/c/Util.java",
                FileStatus::Analyzed,
                1,
                Some(ChangeKind::Added),
            ),
            FileSummary {
                skip_reason: Some(mote_engine::analyze::SkipReason::Generated),
                measures: None,
                ..file("src/gen/Gen.java", FileStatus::Skipped, 0, None)
            },
        ],
        duplications: vec![DuplicationBlock {
            language: "java".into(),
            tokens: 120,
            lines: 21,
            occurrences: vec![
                DuplicationOccurrence {
                    path: "src/a/Order Service.java".into(),
                    start_line: 40,
                    end_line: 60,
                },
                DuplicationOccurrence {
                    path: "src/b/Legacy.java".into(),
                    start_line: 10,
                    end_line: 30,
                },
            ],
        }],
        measures: ProjectMeasures {
            main: total.clone(),
            test: Measures::default(),
            directories: vec![
                dir("src", 3, 240, 5, 57),
                dir("src/a", 1, 80, 3, 42),
                dir("src/b", 1, 80, 1, 5),
                dir("src/c", 1, 80, 1, 10),
            ],
            directories_omitted: 0,
            total,
            languages,
            coverage: None,
        },
        new_code: Some(NewCodeResult {
            base: "origin/main".into(),
            merge_base: "fedcba9876543210fedcba9876543210fedcba98".into(),
            new_lines: BTreeMap::from([
                (
                    "src/a/Order Service.java".to_string(),
                    vec![(1, 5), (40, 60)],
                ),
                ("src/c/Util.java".to_string(), vec![(1, 12)]),
            ]),
            renamed: BTreeMap::new(),
            measures: NewCodeMeasures {
                changed_files: 2,
                new_lines: 38,
                new_duplicated_lines: 21,
                new_duplication_pct: 55.3,
                new_coverable_lines: Some(30),
                new_covered_lines: Some(24),
                new_coverage_pct: Some(80.0),
                new_findings: SeverityCounts {
                    high: 1,
                    medium: 1,
                    low: 1,
                },
                worsened_findings: SeverityCounts {
                    high: 0,
                    medium: 1,
                    low: 0,
                },
                fixed_findings: SeverityCounts {
                    high: 0,
                    medium: 2,
                    low: 0,
                },
                new_effort_minutes: EffortMinutes {
                    total: 25,
                    high: 5,
                    medium: 10,
                    low: 10,
                    reliability: 5,
                    maintainability: 20,
                },
                new_reliability_findings: SeverityCounts {
                    high: 1,
                    medium: 0,
                    low: 0,
                },
            },
            baseline: None,
        }),
        gate: GateResult {
            passed: false,
            conditions: vec![
                ConditionResult {
                    name: "new_findings".into(),
                    passed: false,
                    actual: Some(1.0),
                    threshold: Some(0.0),
                    skipped_reason: None,
                },
                ConditionResult {
                    name: "new_duplication_pct".into(),
                    passed: false,
                    actual: Some(55.3),
                    threshold: Some(3.0),
                    skipped_reason: None,
                },
                ConditionResult {
                    name: "new_coverage_pct".into(),
                    passed: true,
                    actual: Some(80.0),
                    threshold: Some(80.0),
                    skipped_reason: None,
                },
                ConditionResult {
                    name: "new_maintainability_grade".into(),
                    passed: false,
                    actual: Some(5.0),
                    threshold: Some(2.0),
                    skipped_reason: None,
                },
            ],
        },
        rules: vec![
            rule(
                "java/cognitive-complexity",
                "Cognitive complexity",
                Medium,
                Quality::Maintainability,
            ),
            rule(
                "java/duplicated-block",
                "Duplicated block",
                Medium,
                Quality::Maintainability,
            ),
            rule(
                "java/empty-catch-block",
                "Empty catch block",
                Medium,
                Quality::Maintainability,
            ),
            RuleInfo {
                params: vec![ParamInfo {
                    name: "threshold".into(),
                    default: ParamValue::Int(15),
                    description: "Maximum allowed.".into(),
                }],
                ..rule(
                    "java/self-assignment",
                    "Self-assignment",
                    High,
                    Quality::Reliability,
                )
            },
            rule(
                "java/todo-comment",
                "TODO comment",
                Low,
                Quality::Maintainability,
            ),
        ],
        diagnostics: vec![Diagnostic::warning(
            Some("coverage/lcov.info"),
            "cannot parse coverage report",
        )],
        stats: ScanStats::default(),
        triage: None,
        retained: None,
        grades: None,
    };
    // Its 38 new lines are graded (`grades.min_new_lines: 20`), so the outputs show new-code
    // grades; [`change_of`] grades with the default minimum.
    grade_from(&mut r, 20);
    r
}

/// The same findings without a base (status unknown), gate on all findings.
pub fn without_base() -> ScanResult {
    let mut r = with_base();
    for f in &mut r.findings {
        f.status = FindingStatus::Unknown;
    }
    for f in &mut r.files {
        f.change = None;
    }
    r.new_code = None;
    grade(&mut r);
    r.source = SourceInfo {
        kind: "directory".into(),
        rev: None,
        commit: None,
        prefix: String::new(),
    };
    r.gate = GateResult {
        passed: false,
        conditions: vec![
            ConditionResult {
                name: "findings".into(),
                passed: false,
                actual: Some(1.0),
                threshold: Some(0.0),
                skipped_reason: None,
            },
            ConditionResult {
                name: "new_duplication_pct".into(),
                passed: true,
                actual: None,
                threshold: Some(3.0),
                skipped_reason: Some("no base to compare with".into()),
            },
        ],
    };
    r
}

/// The findings of [`with_base`] compared with a baseline results file instead of a git base:
/// no line diff, line-based conditions skipped.
pub fn with_baseline() -> ScanResult {
    let mut r = with_base();
    for f in &mut r.files {
        f.change = None;
    }
    if let Some(nc) = r.new_code.as_mut() {
        nc.base = "mote-baseline.json".into();
        nc.merge_base = "abcdef0123456789abcdef0123456789abcdef01".into();
        nc.new_lines.clear();
        nc.measures.changed_files = 0;
        nc.measures.new_lines = 0;
        nc.measures.new_duplicated_lines = 0;
        nc.measures.new_duplication_pct = 0.0;
        nc.measures.new_coverable_lines = None;
        nc.measures.new_covered_lines = None;
        nc.measures.new_coverage_pct = None;
        nc.baseline = Some(mote_engine::result::BaselineInfo {
            file: "mote-baseline.json".into(),
            commit: Some(nc.merge_base.clone()),
            engine_version: "0.1.0".into(),
            findings: 5,
            matched: 3,
        });
    }
    grade(&mut r);
    let skipped = |name: &str, t: f64| ConditionResult {
        name: name.into(),
        passed: true,
        actual: None,
        threshold: Some(t),
        skipped_reason: Some(mote_engine::gate::NO_LINE_DIFF.into()),
    };
    r.gate = GateResult {
        passed: false,
        conditions: vec![
            ConditionResult {
                name: "new_findings".into(),
                passed: false,
                actual: Some(1.0),
                threshold: Some(0.0),
                skipped_reason: None,
            },
            skipped("new_duplication_pct", 3.0),
            skipped("new_coverage_pct", 80.0),
        ],
    };
    r
}

/// An empty, passing result.
pub fn empty() -> ScanResult {
    let mut r = without_base();
    r.findings.clear();
    r.files.clear();
    r.duplications.clear();
    r.diagnostics.clear();
    r.rules.clear();
    r.measures = ProjectMeasures::default();
    grade(&mut r);
    r.gate = GateResult {
        passed: true,
        conditions: vec![ConditionResult {
            name: "findings".into(),
            passed: true,
            actual: Some(0.0),
            threshold: Some(0.0),
            skipped_reason: None,
        }],
    };
    r
}

/// Compare `actual` with `tests/snapshots/<name>`; `MOTE_BLESS=1` (or a missing file) writes
/// it instead.
pub fn assert_snapshot(name: &str, actual: &str) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name);
    let bless = std::env::var_os("MOTE_BLESS").is_some_and(|v| v == "1");
    if bless || !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\r\n", "\n");
    if expected != actual {
        let first = expected
            .lines()
            .zip(actual.lines())
            .position(|(a, b)| a != b)
            .map(|i| i + 1);
        panic!(
            "snapshot {name} differs (first differing line: {first:?}); rerun with MOTE_BLESS=1 to update\n--- actual ---\n{actual}"
        );
    }
}
