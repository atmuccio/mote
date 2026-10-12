//! GitHub Checks API payload: `{name, conclusion, title, summary, annotations}`.
//!
//! Annotations cover new and worsened findings when the scan has a base (all findings otherwise):
//! HIGH → `failure`, MEDIUM → `warning`, LOW is omitted. Most severe first, capped (default 200)
//! with the overflow noted in the summary. Columns are only set on single-line ranges (the API
//! rejects them otherwise). Annotation paths are relative to the repository root (GitHub drops
//! annotations whose path is not a file of the repository): see [`GithubOptions::path_prefix`].

use crate::markdown::{self, MarkdownOptions};
use crate::{has_base, repo_path, thousands};
use mote_core::{Finding, FindingStatus, Severity};
use mote_engine::ScanResult;
use serde::Serialize;
use std::io::{self, Write};

/// Options for the checks payload.
#[derive(Clone, Copy, Debug)]
pub struct GithubOptions<'a> {
    pub max_annotations: usize,
    /// Limit of `summary`, in characters (the API allows 65,535).
    pub summary_max_chars: usize,
    /// Options of the Markdown `summary` (its `path_prefix` is replaced by [`Self::path_prefix`]).
    pub markdown: MarkdownOptions<'a>,
    /// Repository-relative path of the scan root (`backend/`), prepended to annotation and
    /// summary paths; empty when the scan root is the repository top level.
    pub path_prefix: &'a str,
}

impl Default for GithubOptions<'_> {
    fn default() -> Self {
        GithubOptions {
            max_annotations: 200,
            summary_max_chars: 65_000,
            markdown: MarkdownOptions::default(),
            path_prefix: "",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Payload {
    pub name: &'static str,
    pub conclusion: &'static str,
    pub title: String,
    pub summary: String,
    pub annotations: Vec<Annotation>,
}

#[derive(Debug, Serialize)]
pub struct Annotation {
    pub path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
    pub annotation_level: &'static str,
    pub title: String,
    pub message: String,
}

fn level(f: &Finding) -> Option<&'static str> {
    match f.severity {
        Severity::High if !f.incubating => Some("failure"),
        Severity::High | Severity::Medium => Some("warning"),
        Severity::Low => None,
    }
}

fn annotation(f: &Finding, level: &'static str, prefix: &str) -> Annotation {
    let r = &f.range;
    let start_line = r.start_line.max(1);
    let end_line = r.end_line.max(start_line);
    let (start_column, end_column) =
        if end_line == start_line && r.start_col >= 1 && r.end_col > r.start_col {
            // GitHub columns are inclusive; ours end one past the last character.
            (Some(r.start_col), Some(r.end_col - 1))
        } else {
            (None, None)
        };
    let mut message = f.message.clone();
    if f.status == FindingStatus::Worsened {
        message.push_str(" (worsened)");
    }
    Annotation {
        path: repo_path(prefix, &f.path).into_owned(),
        start_line,
        end_line,
        start_column,
        end_column,
        annotation_level: level,
        title: format!("{} ({})", f.rule_id, f.severity.as_str()),
        message,
    }
}

fn title(r: &ScanResult) -> String {
    let verdict = if r.gate.passed {
        "Quality gate passed"
    } else {
        "Quality gate failed"
    };
    let base = has_base(r);
    let mut counts = [0u64; 3];
    let mut worsened = 0u64;
    for f in r.findings.iter().filter(|f| f.suppressed.is_none()) {
        let counted = !base || f.status == FindingStatus::New;
        if base && f.status == FindingStatus::Worsened {
            worsened += 1;
        }
        if counted {
            counts[match f.severity {
                Severity::High => 0,
                Severity::Medium => 1,
                Severity::Low => 2,
            }] += 1;
        }
    }
    let prefix = if base { "new " } else { "" };
    let mut parts: Vec<String> = [
        ("HIGH", counts[0]),
        ("MEDIUM", counts[1]),
        ("LOW", counts[2]),
    ]
    .iter()
    .filter(|(_, n)| *n > 0)
    .map(|(s, n)| format!("{} {prefix}{s}", thousands(*n)))
    .collect();
    if worsened > 0 {
        parts.push(format!("{} worsened", thousands(worsened)));
    }
    if parts.is_empty() {
        format!("{verdict}: no {}findings", if base { "new " } else { "" })
    } else {
        format!("{verdict}: {}", parts.join(", "))
    }
}

/// Build the payload.
pub fn build(r: &ScanResult, opts: &GithubOptions) -> Payload {
    let base = has_base(r);
    let mut selected: Vec<(&Finding, &'static str)> = r
        .findings
        .iter()
        .filter(|f| f.suppressed.is_none())
        .filter(|f| !base || matches!(f.status, FindingStatus::New | FindingStatus::Worsened))
        .filter_map(|f| level(f).map(|l| (f, l)))
        .collect();
    selected.sort_by(|a, b| {
        b.0.severity
            .cmp(&a.0.severity)
            .then_with(|| a.0.sort_key().cmp(&b.0.sort_key()))
    });
    let total = selected.len();
    let annotations: Vec<Annotation> = selected
        .iter()
        .take(opts.max_annotations)
        .map(|(f, l)| annotation(f, l, opts.path_prefix))
        .collect();
    let overflow = if total > annotations.len() {
        format!(
            "\n> [!NOTE]\n> {} annotations shown; {} more {}findings are not annotated (GitHub limit). See the full results.\n",
            thousands(annotations.len() as u64),
            thousands((total - annotations.len()) as u64),
            if base { "new " } else { "" }
        )
    } else {
        String::new()
    };
    let md_opts = MarkdownOptions {
        max_bytes: opts
            .markdown
            .max_bytes
            .min(opts.summary_max_chars.saturating_sub(overflow.len())),
        path_prefix: opts.path_prefix,
        ..opts.markdown
    };
    let mut summary = markdown::render(r, &md_opts);
    summary.push_str(&overflow);
    Payload {
        name: "Mote",
        conclusion: if r.gate.passed { "success" } else { "failure" },
        title: title(r),
        summary,
        annotations,
    }
}

/// Write the payload as JSON.
pub fn write(r: &ScanResult, opts: &GithubOptions, w: &mut dyn Write) -> io::Result<()> {
    serde_json::to_writer_pretty(&mut *w, &build(r, opts))?;
    writeln!(w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, finding, with_base, without_base};
    use serde_json::Value;

    #[test]
    fn with_base_payload() {
        let mut out = Vec::new();
        write(&with_base(), &GithubOptions::default(), &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_snapshot("checks_with_base.json", &text);
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["name"], "Mote");
        assert_eq!(v["conclusion"], "failure");
        assert_eq!(
            v["title"],
            "Quality gate failed: 1 new HIGH, 1 new MEDIUM, 1 new LOW, 1 worsened"
        );
        assert!(v["summary"]
            .as_str()
            .unwrap()
            .starts_with("<!-- mote:summary -->"));
        let ann = v["annotations"].as_array().unwrap();
        // new HIGH, worsened MEDIUM, new MEDIUM dup; LOW, unchanged and suppressed omitted
        assert_eq!(ann.len(), 3, "{ann:?}");
        assert_eq!(ann[0]["annotation_level"], "failure");
        assert_eq!(ann[0]["start_column"], 5);
        assert_eq!(ann[0]["end_column"], 11);
        let multi = ann
            .iter()
            .find(|a| a["start_line"] != a["end_line"])
            .unwrap();
        assert!(multi.get("start_column").is_none() && multi.get("end_column").is_none());
        assert!(ann.iter().all(|a| a["annotation_level"] != "notice"));
    }

    /// `mote scan backend`: GitHub resolves annotation paths from the repository root.
    #[test]
    fn a_small_change_is_not_graded_in_the_summary() {
        let p = build(&crate::testdata::small_change(), &GithubOptions::default());
        assert!(
            p.summary
                .contains("| Reliability grade | _not rated_ | **E** (41.67) |"),
            "{}",
            p.summary
        );
    }

    #[test]
    fn subdirectory_scan_prefixes_annotation_and_summary_paths() {
        let opts = GithubOptions {
            path_prefix: "backend/",
            ..Default::default()
        };
        let p = build(&with_base(), &opts);
        assert!(!p.annotations.is_empty());
        assert!(
            p.annotations
                .iter()
                .all(|a| a.path.starts_with("backend/src/")),
            "{:?}",
            p.annotations
        );
        assert!(
            p.summary.contains("`backend/src/a/Order Service.java:3`"),
            "{}",
            p.summary
        );
        let p = build(&with_base(), &GithubOptions::default());
        assert_eq!(p.annotations[0].path, "src/a/Order Service.java");
    }

    #[test]
    fn without_base_annotates_all_and_caps() {
        let p = build(&without_base(), &GithubOptions::default());
        assert_eq!(p.annotations.len(), 4);
        assert_eq!(p.title, "Quality gate failed: 1 HIGH, 3 MEDIUM, 1 LOW");
        let mut r = without_base();
        r.findings = (0..250)
            .map(|i| {
                finding(
                    &format!("{i}"),
                    "java/x",
                    "a.java",
                    (i + 1, 1, i + 1, 2),
                    if i % 2 == 0 {
                        Severity::Medium
                    } else {
                        Severity::High
                    },
                    FindingStatus::Unknown,
                    "m",
                )
            })
            .collect();
        let p = build(&r, &GithubOptions::default());
        assert_eq!(p.annotations.len(), 200);
        assert!(p.annotations[..125]
            .iter()
            .all(|a| a.annotation_level == "failure"));
        assert!(p.summary.contains("200 annotations shown; 50 more"));
        assert!(p.summary.chars().count() <= 65_000);
        let e = build(&empty(), &GithubOptions::default());
        assert_eq!(e.conclusion, "success");
        assert_eq!(e.title, "Quality gate passed: no findings");
        assert!(e.annotations.is_empty());
    }
}
