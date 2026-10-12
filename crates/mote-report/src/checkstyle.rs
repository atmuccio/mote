//! Checkstyle XML: the de-facto interchange format many CI dashboards read (for example the
//! Jenkins Warnings Next Generation plugin's Checkstyle parser).
//!
//! ```xml
//! <checkstyle version="4.3">
//!   <file name="src/A.java">
//!     <error line="3" column="5" severity="error" message="…" source="java/self-assignment"/>
//!   </file>
//! </checkstyle>
//! ```
//!
//! Every unsuppressed finding is an `<error>` (the dashboards compare builds themselves, so the
//! report is never limited to new code), grouped by file in result order. `severity`: HIGH →
//! `error`, MEDIUM → `warning`, LOW → `info`; `source` is the rule id. File names are relative
//! to the repository root ([`CheckstyleOptions::path_prefix`]). `version` is the format version
//! consumers expect, not the Mote version. Text is escaped by [`crate::xml`].

use crate::repo_path;
use crate::xml::{attr, DECLARATION};
use mote_core::{Finding, Severity};
use mote_engine::ScanResult;
use std::io::{self, Write};

/// Checkstyle options.
#[derive(Clone, Copy, Debug, Default)]
pub struct CheckstyleOptions<'a> {
    /// Repository-relative path of the scan root (`backend/`), prepended to every file name;
    /// empty when the scan root is the repository top level.
    pub path_prefix: &'a str,
}

/// Checkstyle severity of a finding severity.
pub fn severity(s: Severity) -> &'static str {
    match s {
        Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low => "info",
    }
}

/// Render the report.
pub fn render(result: &ScanResult, opts: &CheckstyleOptions) -> String {
    let active: Vec<&Finding> = result.active_findings().collect();
    let mut out = String::with_capacity(128 + active.len() * 160);
    out.push_str(DECLARATION);
    out.push_str("<checkstyle version=\"4.3\">\n");
    for group in active.chunk_by(|a, b| a.path == b.path) {
        out.push_str("  <file");
        attr(
            &mut out,
            "name",
            &repo_path(opts.path_prefix, &group[0].path),
        );
        out.push_str(">\n");
        for f in group {
            out.push_str("    <error");
            attr(&mut out, "line", &f.range.start_line.max(1).to_string());
            if f.range.start_col >= 1 {
                attr(&mut out, "column", &f.range.start_col.to_string());
            }
            attr(&mut out, "severity", severity(f.severity));
            attr(&mut out, "message", &f.message);
            attr(&mut out, "source", &f.rule_id);
            out.push_str("/>\n");
        }
        out.push_str("  </file>\n");
    }
    out.push_str("</checkstyle>\n");
    out
}

/// Write the report.
pub fn write(result: &ScanResult, opts: &CheckstyleOptions, w: &mut dyn Write) -> io::Result<()> {
    w.write_all(render(result, opts).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, with_base};
    use crate::xml::tests::parse_all;

    #[test]
    fn snapshot_and_structure() {
        let xml = render(&with_base(), &CheckstyleOptions::default());
        assert_snapshot("checkstyle.xml", &xml);
        let values = parse_all(&xml);
        assert!(values.contains(&"src/a/Order Service.java".to_string()));
        assert_eq!(xml.matches("<file ").count(), 3);
        assert_eq!(xml.matches("<error ").count(), 5, "suppressed left out");
        assert!(xml.contains("severity=\"error\""));
        assert!(xml.contains("source=\"java/self-assignment\""));
        let e = render(&empty(), &CheckstyleOptions::default());
        parse_all(&e);
        assert!(e.ends_with("<checkstyle version=\"4.3\">\n</checkstyle>\n"));
    }

    #[test]
    fn repository_relative_names() {
        let xml = render(
            &with_base(),
            &CheckstyleOptions {
                path_prefix: "backend/",
            },
        );
        assert!(xml.contains("<file name=\"backend/src/a/Order Service.java\">"));
        assert!(!xml.contains("<file name=\"src/"));
    }

    #[test]
    fn hostile_strings_are_escaped() {
        let mut r = with_base();
        let msg =
            "</script><img src=x onerror=alert(1)> & \"q\" 'a' ]]> <![CDATA[x]]>\u{0}\u{1b}\nline2";
        r.findings[0].message = msg.into();
        for f in r
            .findings
            .iter_mut()
            .filter(|f| f.path == "src/a/Order Service.java")
        {
            f.path = "src/<a> & \"b\"/C.java".into();
        }
        let xml = render(&r, &CheckstyleOptions::default());
        let values = parse_all(&xml);
        let expected = msg.replace(['\u{0}', '\u{1b}'], "\u{FFFD}");
        assert!(values.contains(&expected), "{values:?}");
        assert!(values.contains(&"src/<a> & \"b\"/C.java".to_string()));
        assert!(!xml.contains("<img"));
        assert!(!xml.contains("]]>"));
        assert_eq!(
            xml.lines().count(),
            render(&with_base(), &CheckstyleOptions::default())
                .lines()
                .count(),
            "line breaks stay character references"
        );
    }
}
