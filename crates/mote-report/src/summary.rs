//! `scan --summary-json FILE`: one compact metrics record per scan, for CI to archive and chart
//! as a trend (append the records of successive runs to a JSONL history).
//!
//! The record is a single line of JSON: `summary_version`, `date` (supplied by the caller — this
//! crate never reads the clock), the engine and results format versions, the source and its
//! `revision`, the gate verdict, the main measures, the grades, finding counts by severity and by
//! quality, and — when the scan was compared with a base or a baseline — the new-code counts.
//! Fields are only added within a `summary_version`.

use mote_core::{Quality, Severity};
use mote_engine::ScanResult;
use serde_json::{json, Value};

/// Version of the summary record.
pub const SUMMARY_VERSION: u32 = 1;

/// Options of the summary record.
#[derive(Clone, Copy, Debug, Default)]
pub struct SummaryOptions<'a> {
    /// When the scan ran (RFC 3339), or `None` to leave `date` null.
    pub date: Option<&'a str>,
}

/// The summary record as JSON.
pub fn summary(r: &ScanResult, opts: &SummaryOptions<'_>) -> Value {
    let t = &r.measures.total;
    let (mut sev, mut quality) = ([0u64; 3], [0u64; 2]);
    let mut incubating = 0u64;
    for f in r.active_findings() {
        sev[match f.severity {
            Severity::High => 0,
            Severity::Medium => 1,
            Severity::Low => 2,
        }] += 1;
        quality[usize::from(f.quality == Quality::Maintainability)] += 1;
        incubating += u64::from(f.incubating);
    }
    let rating =
        |g: &mote_engine::grades::Rating| json!({"grade": g.grade.as_str(), "value": g.value});
    let mut v = json!({
        "summary_version": SUMMARY_VERSION,
        "date": opts.date,
        "engine_version": r.engine_version,
        "format_version": r.format_version,
        "source": {
            "kind": r.source.kind,
            "rev": r.source.rev,
            "commit": r.source.commit,
            "prefix": r.source.prefix,
        },
        "revision": r.source.commit.as_ref().or(r.source.rev.as_ref()),
        "gate": {
            "passed": r.gate.passed,
            "failed": r.gate.conditions.iter().filter(|c| !c.passed).map(|c| c.name.as_str()).collect::<Vec<_>>(),
        },
        "measures": {
            "files": t.files,
            "lines": t.lines,
            "ncloc": t.ncloc,
            "test_ncloc": r.measures.test.ncloc,
            "comment_lines": t.comment_lines,
            "functions": t.functions,
            "classes": t.classes,
            "cognitive_complexity": t.cognitive_complexity,
            "cyclomatic_complexity": t.cyclomatic_complexity,
            "duplicated_lines": t.duplicated_lines,
            "duplicated_lines_density": t.duplicated_lines_density,
            "coverage_pct": r.measures.coverage.as_ref().map(|c| c.coverage_pct),
            "effort_minutes": t.effort_minutes,
        },
        "grades": r.grades.as_ref().map(|g| json!({
            "maintainability": rating(&g.maintainability),
            "reliability": rating(&g.reliability),
            "new_maintainability": g.new_maintainability.as_ref().map(rating),
            "new_reliability": g.new_reliability.as_ref().map(rating),
        })),
        "findings": {
            "total": sev.iter().sum::<u64>(),
            "high": sev[0],
            "medium": sev[1],
            "low": sev[2],
            "reliability": quality[0],
            "maintainability": quality[1],
            "incubating": incubating,
            "suppressed": r.findings.len() as u64 - sev.iter().sum::<u64>(),
        },
        "new_code": Value::Null,
        "triage": r.triage.as_ref().map(|t| json!({
            "entries": t.entries,
            "matched": t.matched,
            "stale": t.stale.len(),
        })),
    });
    if let Some(nc) = &r.new_code {
        let m = &nc.measures;
        v["new_code"] = json!({
            "base": nc.base,
            "baseline": nc.baseline.is_some(),
            "changed_files": m.changed_files,
            "new_lines": m.new_lines,
            "new_findings": m.new_findings,
            "worsened": m.worsened_findings.total(),
            "fixed": m.fixed_findings.total(),
            "new_duplication_pct": m.new_duplication_pct,
            "new_coverage_pct": m.new_coverage_pct,
            "new_effort_minutes": m.new_effort_minutes.total,
        });
    }
    // New-code grades are null when the change is too small to grade; say why (as in the
    // results JSON, only then).
    if let Some(n) = r.grades.as_ref().and_then(|g| g.new_not_rated.as_ref()) {
        v["grades"]["new_not_rated"] = json!(n);
    }
    v
}

/// The record as one line of JSON, with a trailing newline.
pub fn render(r: &ScanResult, opts: &SummaryOptions<'_>) -> String {
    let mut s = serde_json::to_string(&summary(r, opts)).unwrap_or_else(|_| "{}".into());
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{with_base, with_baseline, without_base};

    #[test]
    fn record_shape() {
        let text = render(
            &with_base(),
            &SummaryOptions {
                date: Some("2026-09-28T12:00:00Z"),
            },
        );
        assert_eq!(text.matches('\n').count(), 1, "one line");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["summary_version"], 1);
        assert_eq!(v["date"], "2026-09-28T12:00:00Z");
        assert_eq!(
            v["revision"], "0123456789abcdef0123456789abcdef01234567",
            "the commit"
        );
        assert_eq!(v["gate"]["passed"], false);
        assert_eq!(v["gate"]["failed"][0], "new_findings");
        assert_eq!(v["measures"]["ncloc"], 240);
        assert_eq!(v["measures"]["effort_minutes"]["total"], 57);
        assert_eq!(v["findings"]["total"], 5);
        assert_eq!(v["findings"]["high"], 1);
        assert_eq!(v["findings"]["suppressed"], 1);
        assert_eq!(
            v["findings"]["reliability"].as_u64().unwrap()
                + v["findings"]["maintainability"].as_u64().unwrap(),
            5
        );
        assert!(v["grades"]["maintainability"]["grade"].is_string());
        assert_eq!(v["new_code"]["base"], "origin/main");
        assert_eq!(v["new_code"]["baseline"], false);
        assert_eq!(v["new_code"]["new_findings"]["high"], 1);
        assert_eq!(v["triage"], Value::Null);

        let v = summary(&without_base(), &SummaryOptions::default());
        assert_eq!(v["date"], Value::Null);
        assert_eq!(v["revision"], Value::Null);
        assert_eq!(v["new_code"], Value::Null);
        let v = summary(&with_baseline(), &SummaryOptions::default());
        assert_eq!(v["new_code"]["baseline"], true);
    }

    #[test]
    fn a_small_change_is_not_graded() {
        let v = summary(&with_base(), &SummaryOptions::default());
        assert!(v["grades"]["new_maintainability"]["grade"].is_string());
        assert!(
            v["grades"].get("new_not_rated").is_none(),
            "only when not rated"
        );
        let v = summary(&crate::testdata::small_change(), &SummaryOptions::default());
        assert!(v["grades"]["new_maintainability"].is_null());
        assert!(v["grades"]["new_reliability"].is_null());
        assert_eq!(
            v["grades"]["new_not_rated"],
            serde_json::json!({"reason": "too_few_new_lines", "new_lines": 11, "min_new_lines": 100})
        );
    }

    /// The JSON summary at `grades.min_new_lines` (100 by default) and one line short of it.
    #[test]
    fn new_code_is_rated_from_the_grades_minimum() {
        let v = summary(&crate::testdata::change_of(99), &SummaryOptions::default());
        assert!(v["grades"]["new_maintainability"].is_null());
        assert_eq!(
            v["grades"]["new_not_rated"],
            serde_json::json!({"reason": "too_few_new_lines", "new_lines": 99, "min_new_lines": 100})
        );
        let v = summary(&crate::testdata::change_of(100), &SummaryOptions::default());
        assert!(v["grades"]["new_maintainability"]["grade"].is_string());
        assert!(v["grades"].get("new_not_rated").is_none());
        // The full results JSON says the same.
        let r = serde_json::to_value(crate::testdata::change_of(99)).unwrap();
        assert_eq!(r["grades"]["new_not_rated"]["new_lines"], 99);
        assert_eq!(r["grades"]["bands"]["min_new_lines"], 100);
        let r = serde_json::to_value(crate::testdata::change_of(100)).unwrap();
        assert!(r["grades"]["new_reliability"]["grade"].is_string());
    }
}
