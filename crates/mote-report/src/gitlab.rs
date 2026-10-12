//! GitLab Code Quality report: a JSON array of issues (the Code Climate issue subset GitLab
//! reads), for the `artifacts:reports:codequality` artifact.
//!
//! Every unsuppressed finding is an issue (GitLab compares the merge request's report with the
//! target branch's report itself, so the report is never limited to new code):
//!
//! * `check_name` is the rule id, `description` the finding message;
//! * `severity`: HIGH → `critical`, MEDIUM → `major`, LOW → `minor`;
//! * `fingerprint` is the Mote fingerprint, which leaves the path out so a finding keeps it
//!   when its file is renamed or its line moves. Fingerprints must be unique within a report,
//!   and two files can hold the same finding on the same line text (a `// TODO` without an
//!   enclosing symbol): such colliding fingerprints get the finding id, which includes the path,
//!   appended (`<fingerprint>:<id>`) — unique and still stable across line shifts, but not across
//!   a rename of those files;
//! * `location.path` is relative to the repository root ([`GitlabOptions::path_prefix`], like
//!   SARIF), `location.lines.begin`/`end` the finding's lines;
//! * `categories` from the rule's quality and tags (`Bug Risk`, `Complexity`, `Duplication`,
//!   `Performance`, `Security`, `Style`, `Compatibility`, else `Clarity`).
//!
//! One issue per line inside the array keeps large reports compact and diffable.

use crate::repo_path;
use mote_core::{Finding, Quality, Severity};
use mote_engine::{RuleInfo, ScanResult};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{self, Write};

/// GitLab Code Quality options.
#[derive(Clone, Copy, Debug, Default)]
pub struct GitlabOptions<'a> {
    /// Repository-relative path of the scan root (`backend/`), prepended to every path; empty
    /// when the scan root is the repository top level.
    pub path_prefix: &'a str,
}

/// One Code Quality issue.
#[derive(Debug, Serialize)]
pub struct Issue<'a> {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub check_name: &'a str,
    pub description: &'a str,
    pub categories: Vec<&'static str>,
    pub severity: &'static str,
    pub fingerprint: String,
    pub location: IssueLocation,
}

#[derive(Debug, Serialize)]
pub struct IssueLocation {
    pub path: String,
    pub lines: Lines,
}

#[derive(Debug, Serialize)]
pub struct Lines {
    pub begin: u32,
    pub end: u32,
}

/// GitLab severity of a finding severity.
pub fn severity(s: Severity) -> &'static str {
    match s {
        Severity::High => "critical",
        Severity::Medium => "major",
        Severity::Low => "minor",
    }
}

/// Code Climate categories for a finding of `rule` (or of an unknown rule with `quality`).
pub fn categories(quality: Quality, rule: Option<&RuleInfo>) -> Vec<&'static str> {
    let tags: &[String] = rule.map(|r| r.tags.as_slice()).unwrap_or(&[]);
    let has = |names: &[&str]| tags.iter().any(|t| names.contains(&t.as_str()));
    let mut out = Vec::new();
    if quality == Quality::Reliability {
        out.push("Bug Risk");
    }
    if has(&["complexity", "brain-overload"]) {
        out.push("Complexity");
    }
    if has(&["duplication", "copy-paste"]) {
        out.push("Duplication");
    }
    if has(&["performance"]) {
        out.push("Performance");
    }
    if has(&["security"]) {
        out.push("Security");
    }
    if has(&["naming", "convention", "format", "formatting", "style"]) {
        out.push("Style");
    }
    if has(&["deprecated", "compatibility"]) {
        out.push("Compatibility");
    }
    if out.is_empty() {
        out.push("Clarity");
    }
    out
}

/// The issues of `result`, in result order.
pub fn issues<'a>(result: &'a ScanResult, opts: &GitlabOptions) -> Vec<Issue<'a>> {
    let rules: HashMap<&str, &RuleInfo> = result.rules.iter().map(|r| (r.id.as_str(), r)).collect();
    let active: Vec<&Finding> = result.active_findings().collect();
    let mut uses: HashMap<&str, u32> = HashMap::with_capacity(active.len());
    for f in &active {
        *uses.entry(f.fingerprint.as_str()).or_default() += 1;
    }
    active
        .into_iter()
        .map(|f| {
            let fingerprint = if uses[f.fingerprint.as_str()] > 1 || f.fingerprint.is_empty() {
                format!("{}:{}", f.fingerprint, f.id)
            } else {
                f.fingerprint.clone()
            };
            let begin = f.range.start_line.max(1);
            Issue {
                kind: "issue",
                check_name: &f.rule_id,
                description: &f.message,
                categories: categories(f.quality, rules.get(f.rule_id.as_str()).copied()),
                severity: severity(f.severity),
                fingerprint,
                location: IssueLocation {
                    path: repo_path(opts.path_prefix, &f.path).into_owned(),
                    lines: Lines {
                        begin,
                        end: f.range.end_line.max(begin),
                    },
                },
            }
        })
        .collect()
}

/// Write the report: a JSON array, one issue per line.
pub fn write(result: &ScanResult, opts: &GitlabOptions, w: &mut dyn Write) -> io::Result<()> {
    let issues = issues(result, opts);
    if issues.is_empty() {
        return w.write_all(b"[]\n");
    }
    w.write_all(b"[\n")?;
    for (i, issue) in issues.iter().enumerate() {
        if i > 0 {
            w.write_all(b",\n")?;
        }
        serde_json::to_writer(&mut *w, issue)?;
    }
    w.write_all(b"\n]\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, finding, with_base};
    use mote_core::FindingStatus;
    use serde_json::Value;

    fn render(r: &ScanResult, opts: GitlabOptions) -> String {
        let mut out = Vec::new();
        write(r, &opts, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    /// What GitLab requires of every issue.
    fn validate(v: &Value) -> &Vec<Value> {
        let issues = v.as_array().expect("an array of issues");
        let mut fps = std::collections::HashSet::new();
        for i in issues {
            assert_eq!(i["type"], "issue");
            assert!(i["description"].is_string() && i["check_name"].is_string());
            assert!(["info", "minor", "major", "critical", "blocker"]
                .contains(&i["severity"].as_str().unwrap()));
            let fp = i["fingerprint"].as_str().unwrap();
            assert!(!fp.is_empty() && fps.insert(fp.to_string()), "unique {fp}");
            let path = i["location"]["path"].as_str().unwrap();
            assert!(!path.is_empty() && !path.starts_with('/') && !path.contains('\\'));
            let begin = i["location"]["lines"]["begin"].as_u64().unwrap();
            assert!(begin >= 1);
            assert!(i["location"]["lines"]["end"].as_u64().unwrap() >= begin);
            assert!(!i["categories"].as_array().unwrap().is_empty());
        }
        issues
    }

    #[test]
    fn snapshot_and_shape() {
        let text = render(&with_base(), GitlabOptions::default());
        assert_snapshot("codequality.json", &text);
        let v: Value = serde_json::from_str(&text).unwrap();
        let issues = validate(&v);
        assert_eq!(issues.len(), 5, "the suppressed finding is left out");
        assert_eq!(text.lines().count(), 7, "one issue per line");
        let hi = &issues[0];
        assert_eq!(hi["check_name"], "java/self-assignment");
        assert_eq!(hi["severity"], "critical");
        assert_eq!(hi["fingerprint"], "fp-new000000001");
        assert_eq!(hi["location"]["path"], "src/a/Order Service.java");
        assert_eq!(hi["categories"], serde_json::json!(["Bug Risk"]));
        let sev: Vec<&str> = issues
            .iter()
            .map(|i| i["severity"].as_str().unwrap())
            .collect();
        assert_eq!(sev, ["critical", "major", "major", "major", "minor"]);
        assert_eq!(render(&empty(), GitlabOptions::default()), "[]\n");
    }

    #[test]
    fn repository_relative_paths() {
        let text = render(
            &with_base(),
            GitlabOptions {
                path_prefix: "backend/",
            },
        );
        let v: Value = serde_json::from_str(&text).unwrap();
        for i in validate(&v) {
            assert!(i["location"]["path"]
                .as_str()
                .unwrap()
                .starts_with("backend/src/"));
        }
    }

    /// The same symbol-less finding on the same line text in two files shares a fingerprint;
    /// GitLab needs unique ones, and a unique fingerprint must stay path-independent.
    #[test]
    fn colliding_fingerprints_get_the_finding_id() {
        let mut r = with_base();
        let mut a = finding(
            "todo0000000a",
            "java/todo-comment",
            "src/x/A.java",
            (1, 1, 1, 9),
            Severity::Low,
            FindingStatus::New,
            "Complete the task in this TODO comment",
        );
        a.fingerprint = "same".into();
        let mut b = a.clone();
        b.id = "todo0000000b".into();
        b.path = "src/y/B.java".into();
        r.findings.push(a);
        r.findings.push(b);
        let v: Value = serde_json::from_str(&render(&r, GitlabOptions::default())).unwrap();
        let fps: Vec<&str> = validate(&v)
            .iter()
            .map(|i| i["fingerprint"].as_str().unwrap())
            .collect();
        assert!(fps.contains(&"same:todo0000000a") && fps.contains(&"same:todo0000000b"));
        assert!(fps.contains(&"fp-new000000001"), "unique ones unchanged");
    }

    #[test]
    fn hostile_strings_stay_valid_json() {
        let mut r = with_base();
        r.findings[0].message =
            "</script><img src=x onerror=alert(1)> \"q\" \\ ]]> \u{0}\u{1b}\n".into();
        r.findings[0].path = "src/\"odd\" dir/<x> & y.java".into();
        let text = render(&r, GitlabOptions::default());
        let v: Value = serde_json::from_str(&text).unwrap();
        let issues = validate(&v);
        assert_eq!(issues[0]["description"], r.findings[0].message.as_str());
        assert_eq!(issues[0]["location"]["path"], r.findings[0].path.as_str());
        assert_eq!(
            text.lines().count(),
            7,
            "line breaks and control characters stay escaped"
        );
    }

    #[test]
    fn category_mapping() {
        let mut rule =
            crate::testdata::rule("java/x", "X", Severity::Low, Quality::Maintainability);
        rule.tags = vec!["complexity".into(), "performance".into()];
        assert_eq!(
            categories(Quality::Maintainability, Some(&rule)),
            ["Complexity", "Performance"]
        );
        assert_eq!(categories(Quality::Maintainability, None), ["Clarity"]);
        assert_eq!(categories(Quality::Reliability, None), ["Bug Risk"]);
    }
}
