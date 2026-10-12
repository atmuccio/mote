//! JUnit XML, for the test-report views of CI systems (Jenkins, GitLab, Azure Pipelines, …).
//!
//! * One `<testsuite>` for the quality gate: a test case per condition, failed when the
//!   condition failed and skipped (with the reason) when it was not evaluated.
//! * One `<testsuite>` per rule, named by the rule id: a failed test case per finding, named
//!   `path:line:col`, with the message as the failure message, the severity as its type, and the
//!   details (rule, severity, quality, status, location) as its text.
//!
//! Findings follow [`JunitOptions::filter`] (`--new-only`, `--min-severity`; suppressed findings
//! never appear). At most [`JunitOptions::max_findings`] test cases are written (the most severe,
//! like the SARIF cap); the rest are counted in a final skipped test case. Paths are relative to
//! the repository root ([`JunitOptions::path_prefix`]). No timestamps or durations (`time="0"`):
//! the same result always renders to the same bytes.

use crate::xml::{attr, push_text, DECLARATION};
use crate::{
    condition_label, fmt_condition_threshold, fmt_condition_value, most_severe, repo_path,
    status_str, thousands, Filter,
};
use mote_core::{Finding, FindingStatus};
use mote_engine::gate::ConditionResult;
use mote_engine::{RuleInfo, ScanResult};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::io::{self, Write};

/// Default [`JunitOptions::max_findings`]: CI test views stay usable well below this.
pub const MAX_FINDINGS: usize = 5_000;

/// JUnit options.
#[derive(Clone, Copy, Debug)]
pub struct JunitOptions<'a> {
    /// Which findings become test cases (suppressed findings never do).
    pub filter: Filter,
    /// Most finding test cases written; the most severe are kept.
    pub max_findings: usize,
    /// Repository-relative path of the scan root (`backend/`), prepended to every path; empty
    /// when the scan root is the repository top level.
    pub path_prefix: &'a str,
}

impl Default for JunitOptions<'_> {
    fn default() -> Self {
        JunitOptions {
            filter: Filter::default(),
            max_findings: MAX_FINDINGS,
            path_prefix: "",
        }
    }
}

/// Name of the gate suite.
pub const GATE_SUITE: &str = "Mote quality gate";

/// `Label: actual (op threshold)` for a gate condition.
fn condition_text(c: &ConditionResult) -> String {
    let mut s = condition_label(&c.name);
    if let Some(v) = c.actual {
        let _ = write!(s, ": {}", fmt_condition_value(&c.name, v));
    }
    if let Some(t) = c.threshold {
        let _ = write!(s, " ({})", fmt_condition_threshold(&c.name, t));
    }
    s
}

fn open_suite(out: &mut String, name: &str, tests: usize, failures: usize, skipped: usize) {
    out.push_str("  <testsuite");
    attr(out, "name", name);
    attr(out, "tests", &tests.to_string());
    attr(out, "failures", &failures.to_string());
    attr(out, "errors", "0");
    attr(out, "skipped", &skipped.to_string());
    attr(out, "time", "0");
    out.push_str(">\n");
}

fn open_case(out: &mut String, classname: &str, name: &str) {
    out.push_str("    <testcase");
    attr(out, "classname", classname);
    attr(out, "name", name);
}

fn gate_suite(out: &mut String, conditions: &[ConditionResult]) {
    let failures = conditions
        .iter()
        .filter(|c| !c.passed && c.skipped_reason.is_none())
        .count();
    let skipped = conditions
        .iter()
        .filter(|c| c.skipped_reason.is_some())
        .count();
    open_suite(out, GATE_SUITE, conditions.len(), failures, skipped);
    for c in conditions {
        open_case(out, "mote.gate", &condition_label(&c.name));
        attr(out, "time", "0");
        if let Some(reason) = &c.skipped_reason {
            out.push_str(">\n      <skipped");
            attr(out, "message", reason);
            out.push_str("/>\n    </testcase>\n");
        } else if c.passed {
            out.push_str("/>\n");
        } else {
            let text = condition_text(c);
            out.push_str(">\n      <failure");
            attr(out, "message", &text);
            attr(out, "type", "gate");
            out.push('>');
            push_text(out, &format!("Quality gate condition failed: {text}"));
            out.push_str("</failure>\n    </testcase>\n");
        }
    }
    out.push_str("  </testsuite>\n");
}

fn finding_case(out: &mut String, f: &Finding, rule: Option<&RuleInfo>, prefix: &str) {
    let path = repo_path(prefix, &f.path);
    let line = f.range.start_line.max(1);
    let location = if f.range.start_col >= 1 {
        format!("{path}:{line}:{}", f.range.start_col)
    } else {
        format!("{path}:{line}")
    };
    open_case(out, &f.rule_id, &location);
    attr(out, "file", &path);
    attr(out, "line", &line.to_string());
    attr(out, "time", "0");
    out.push_str(">\n      <failure");
    attr(out, "message", &f.message);
    attr(out, "type", f.severity.as_str());
    out.push('>');
    let mut details = String::new();
    let _ = writeln!(
        details,
        "Rule: {}{}",
        f.rule_id,
        rule.map(|r| format!(" ({})", r.name)).unwrap_or_default()
    );
    let mut status = status_str(f.status).to_string();
    if f.status == FindingStatus::Unknown {
        status = "no base".into();
    }
    let _ = writeln!(
        details,
        "Severity: {}, quality: {}, status: {status}{}",
        f.severity.as_str(),
        f.quality.as_str(),
        if f.incubating {
            ", incubating rule"
        } else {
            ""
        }
    );
    let _ = writeln!(details, "Location: {location}");
    details.push_str(&f.message);
    push_text(out, &details);
    out.push_str("</failure>\n    </testcase>\n");
}

/// Render the report.
pub fn render(result: &ScanResult, opts: &JunitOptions) -> String {
    let filter = Filter {
        show_suppressed: false,
        ..opts.filter
    };
    let listed: Vec<&Finding> = filter.apply(result).collect();
    let total = listed.len();
    let kept = most_severe(listed, opts.max_findings);
    let omitted = total - kept.len();
    let mut by_rule: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in &kept {
        by_rule.entry(f.rule_id.as_str()).or_default().push(f);
    }
    let rules: HashMap<&str, &RuleInfo> = result.rules.iter().map(|r| (r.id.as_str(), r)).collect();

    let conditions = &result.gate.conditions;
    let gate_failures = conditions
        .iter()
        .filter(|c| !c.passed && c.skipped_reason.is_none())
        .count();
    let gate_skipped = conditions
        .iter()
        .filter(|c| c.skipped_reason.is_some())
        .count();
    let tests = conditions.len() + kept.len() + usize::from(omitted > 0);
    let failures = gate_failures + kept.len();
    let skipped = gate_skipped + usize::from(omitted > 0);

    let mut out = String::with_capacity(512 + kept.len() * 400);
    out.push_str(DECLARATION);
    out.push_str("<testsuites");
    attr(&mut out, "name", "Mote");
    attr(&mut out, "tests", &tests.to_string());
    attr(&mut out, "failures", &failures.to_string());
    attr(&mut out, "errors", "0");
    attr(&mut out, "skipped", &skipped.to_string());
    attr(&mut out, "time", "0");
    out.push_str(">\n");
    gate_suite(&mut out, conditions);
    for (rule_id, findings) in &by_rule {
        open_suite(&mut out, rule_id, findings.len(), findings.len(), 0);
        let rule = rules.get(rule_id).copied();
        for f in findings {
            finding_case(&mut out, f, rule, opts.path_prefix);
        }
        out.push_str("  </testsuite>\n");
    }
    if omitted > 0 {
        let note = format!(
            "{} of {} findings (the least severe) left out to stay within {} test cases; the JSON and SARIF results have them all",
            thousands(omitted as u64),
            thousands(total as u64),
            thousands(opts.max_findings as u64)
        );
        open_suite(&mut out, "Findings left out", 1, 0, 1);
        open_case(
            &mut out,
            "mote.findings",
            &format!("{} findings left out", thousands(omitted as u64)),
        );
        attr(&mut out, "time", "0");
        out.push_str(">\n      <skipped");
        attr(&mut out, "message", &note);
        out.push_str("/>\n    </testcase>\n  </testsuite>\n");
    }
    out.push_str("</testsuites>\n");
    out
}

/// Write the report.
pub fn write(result: &ScanResult, opts: &JunitOptions, w: &mut dyn Write) -> io::Result<()> {
    w.write_all(render(result, opts).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, finding, with_base, without_base};
    use crate::xml::tests::parse_all;
    use mote_core::Severity;

    /// Parse and check the counts of every suite and of the whole document against its children.
    fn check_counts(xml: &str) {
        use quick_xml::events::Event;
        parse_all(xml);
        let mut reader = quick_xml::Reader::from_str(xml);
        let get = |e: &quick_xml::events::BytesStart, k: &str| -> usize {
            e.attributes()
                .map(|a| a.unwrap())
                .find(|a| a.key.0 == k)
                .map(|a| a.value.parse().unwrap())
                .unwrap_or(0)
        };
        // (declared tests, failures, skipped) and counted (tests, failures, skipped)
        let mut root = [0usize; 6];
        let mut suite = [0usize; 6];
        let mut in_case = false;
        loop {
            match reader.read_event().unwrap() {
                Event::Eof => break,
                Event::Start(e) | Event::Empty(e) => match e.name().0 {
                    "testsuites" => root[..3].copy_from_slice(&[
                        get(&e, "tests"),
                        get(&e, "failures"),
                        get(&e, "skipped"),
                    ]),
                    "testsuite" => {
                        suite = [
                            get(&e, "tests"),
                            get(&e, "failures"),
                            get(&e, "skipped"),
                            0,
                            0,
                            0,
                        ]
                    }
                    "testcase" => {
                        suite[3] += 1;
                        root[3] += 1;
                        in_case = true;
                    }
                    "failure" if in_case => {
                        suite[4] += 1;
                        root[4] += 1;
                    }
                    "skipped" if in_case => {
                        suite[5] += 1;
                        root[5] += 1;
                    }
                    _ => {}
                },
                Event::End(e) if e.name().0 == "testsuite" => {
                    assert_eq!(suite[..3], suite[3..], "suite counts match its cases");
                }
                _ => {}
            }
        }
        assert_eq!(root[..3], root[3..], "document counts match its cases");
    }

    #[test]
    fn snapshot_and_counts() {
        let xml = render(&with_base(), &JunitOptions::default());
        assert_snapshot("junit.xml", &xml);
        check_counts(&xml);
        assert!(xml.contains("<testsuite name=\"Mote quality gate\" tests=\"4\" failures=\"3\""));
        assert!(xml.contains(
            "<testsuite name=\"java/self-assignment\" tests=\"1\" failures=\"1\" errors=\"0\""
        ));
        assert!(xml.contains("name=\"src/a/Order Service.java:3:5\""));
        assert!(!xml.contains("tracked in JIRA-1"), "suppressed left out");
        let xml = render(&without_base(), &JunitOptions::default());
        check_counts(&xml);
        assert!(xml.contains("<skipped message=\"no base to compare with\"/>"));
        let xml = render(&empty(), &JunitOptions::default());
        check_counts(&xml);
        assert_eq!(xml.matches("<testsuite ").count(), 1, "just the gate");
    }

    #[test]
    fn filter_and_prefix() {
        let opts = JunitOptions {
            filter: Filter {
                new_only: true,
                min_severity: Severity::Medium,
                show_suppressed: true, // ignored: suppressed findings never appear
            },
            path_prefix: "backend/",
            ..Default::default()
        };
        let xml = render(&with_base(), &opts);
        check_counts(&xml);
        assert!(
            !xml.contains("java/empty-catch-block"),
            "unchanged left out"
        );
        assert!(!xml.contains("java/todo-comment"), "LOW left out");
        assert!(xml.contains("file=\"backend/src/a/Order Service.java\""));
    }

    #[test]
    fn capped_keeping_the_most_severe() {
        let mut r = with_base();
        r.findings = (0..40)
            .map(|i| {
                finding(
                    &format!("id{i:010}"),
                    if i % 4 == 0 {
                        "java/self-assignment"
                    } else {
                        "java/todo-comment"
                    },
                    &format!("src/F{i:02}.java"),
                    (1, 1, 1, 5),
                    if i % 4 == 0 {
                        Severity::High
                    } else {
                        Severity::Low
                    },
                    FindingStatus::New,
                    "m",
                )
            })
            .collect();
        let xml = render(
            &r,
            &JunitOptions {
                max_findings: 15,
                ..Default::default()
            },
        );
        check_counts(&xml);
        assert!(xml.contains("<testsuite name=\"java/self-assignment\" tests=\"10\""));
        assert!(xml.contains("<testsuite name=\"java/todo-comment\" tests=\"5\""));
        assert!(xml.contains("25 of 40 findings (the least severe) left out to stay within 15"));
    }

    #[test]
    fn hostile_strings_are_escaped() {
        let mut r = with_base();
        let msg = "</testcase></testsuite><img src=x onerror=alert(1)> ]]> & \u{0}\u{7}\ttab\r\nx";
        r.findings[0].message = msg.into();
        r.findings[0].path = "src/\"<q>\"/A&B.java".into();
        r.gate.conditions[1].skipped_reason = Some("<b>\"odd\"</b>".into());
        let xml = render(&r, &JunitOptions::default());
        check_counts(&xml);
        let values = parse_all(&xml);
        let clean = msg.replace(['\u{0}', '\u{7}'], "\u{FFFD}");
        assert!(values.contains(&clean), "{values:?}");
        assert!(values.contains(&"src/\"<q>\"/A&B.java".to_string()));
        assert!(values.contains(&"<b>\"odd\"</b>".to_string()));
        assert!(!xml.contains("<img") && !xml.contains("]]>"));
    }
}
