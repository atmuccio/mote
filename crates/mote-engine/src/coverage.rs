//! Line-coverage ingestion: LCOV, JaCoCo XML and Cobertura XML.
//!
//! Reports are pure data; nothing is executed. Report paths are mapped to repository files by the
//! longest matching path suffix (JaCoCo's `package/SourceFile.java` matches
//! `module/src/main/java/package/SourceFile.java`). A line that a report does not mention is not
//! coverable.

use crate::result::Diagnostic;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use rustc_hash::FxHashMap;
use std::collections::BTreeMap;
use std::path::Path;

/// Coverage of one file: line → covered.
pub type LineCoverage = BTreeMap<u32, bool>;

/// Coverage as parsed from reports, keyed by the path the report used.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoverageReport {
    pub files: BTreeMap<String, LineCoverage>,
}

impl CoverageReport {
    fn record(&mut self, path: &str, line: u32, covered: bool) {
        if line == 0 {
            return;
        }
        let path = normalize(path);
        let e = self
            .files
            .entry(path)
            .or_default()
            .entry(line)
            .or_insert(false);
        *e |= covered;
    }

    /// Merge another report (a line is covered if any report covers it).
    pub fn merge(&mut self, other: CoverageReport) {
        for (p, lines) in other.files {
            let dst = self.files.entry(p).or_default();
            for (l, c) in lines {
                *dst.entry(l).or_insert(false) |= c;
            }
        }
    }
}

fn normalize(p: &str) -> String {
    p.trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

/// Parse a report, detecting the format from its content.
pub fn parse_report(text: &str) -> Result<CoverageReport, String> {
    let head = text.trim_start_matches('\u{FEFF}').trim_start();
    if head.starts_with('<') {
        parse_xml(head)
    } else {
        Ok(parse_lcov(text))
    }
}

/// Parse LCOV (`SF:` / `DA:` records).
pub fn parse_lcov(text: &str) -> CoverageReport {
    let mut r = CoverageReport::default();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(p) = line.strip_prefix("SF:") {
            current = Some(p.to_string());
        } else if let Some(rest) = line.strip_prefix("DA:") {
            if let Some(file) = &current {
                let mut parts = rest.split(',');
                let ln = parts.next().and_then(|s| s.trim().parse::<u32>().ok());
                let hits = parts.next().and_then(|s| s.trim().parse::<f64>().ok());
                if let (Some(ln), Some(hits)) = (ln, hits) {
                    r.record(file, ln, hits > 0.0);
                }
            }
        } else if line == "end_of_record" {
            current = None;
        }
    }
    r
}

fn attr(e: &BytesStart<'_>, name: &str) -> Option<String> {
    e.try_get_attribute(name).ok().flatten().and_then(|a| {
        a.normalized_value(XmlVersion::default())
            .ok()
            .map(|v| v.into_owned())
    })
}

fn attr_u32(e: &BytesStart<'_>, name: &str) -> Option<u32> {
    attr(e, name).and_then(|v| v.trim().parse().ok())
}

enum XmlKind {
    Unknown,
    Jacoco,
    Cobertura,
}

/// Parse JaCoCo or Cobertura XML.
pub fn parse_xml(text: &str) -> Result<CoverageReport, String> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut r = CoverageReport::default();
    let mut kind = XmlKind::Unknown;
    let mut package = String::new();
    let mut file: Option<String> = None;
    loop {
        let ev = reader
            .read_event()
            .map_err(|e| format!("invalid XML at byte {}: {e}", reader.buffer_position()))?;
        match ev {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.local_name();
                match (name.as_ref(), &kind) {
                    ("report", XmlKind::Unknown) => kind = XmlKind::Jacoco,
                    ("coverage", XmlKind::Unknown) => kind = XmlKind::Cobertura,
                    ("package", XmlKind::Jacoco) => package = attr(&e, "name").unwrap_or_default(),
                    ("sourcefile", XmlKind::Jacoco) => {
                        let n = attr(&e, "name").unwrap_or_default();
                        file = Some(if package.is_empty() {
                            n
                        } else {
                            format!("{package}/{n}")
                        });
                    }
                    ("line", XmlKind::Jacoco) => {
                        if let (Some(f), Some(nr)) = (&file, attr_u32(&e, "nr")) {
                            let ci = attr_u32(&e, "ci").unwrap_or(0);
                            r.record(f, nr, ci > 0);
                        }
                    }
                    ("class", XmlKind::Cobertura) => file = attr(&e, "filename"),
                    ("line", XmlKind::Cobertura) => {
                        if let (Some(f), Some(nr)) = (&file, attr_u32(&e, "number")) {
                            let hits = attr(&e, "hits")
                                .and_then(|h| h.trim().parse::<f64>().ok())
                                .unwrap_or(0.0);
                            r.record(f, nr, hits > 0.0);
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => match (e.local_name().as_ref(), &kind) {
                ("sourcefile", XmlKind::Jacoco) | ("class", XmlKind::Cobertura) => file = None,
                ("package", XmlKind::Jacoco) => package.clear(),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    match kind {
        XmlKind::Unknown => Err("not a JaCoCo or Cobertura XML report".into()),
        _ => Ok(r),
    }
}

/// Coverage mapped onto repository files.
#[derive(Clone, Debug, Default)]
pub struct RepoCoverage {
    pub files: FxHashMap<String, LineCoverage>,
    /// Report paths that matched no repository file.
    pub unmatched: Vec<String>,
    /// Report paths that matched several files equally well (skipped).
    pub ambiguous: Vec<String>,
}

/// Map report paths to repository paths by longest common path suffix (in whole components).
pub fn map_to_repo(report: CoverageReport, repo_files: &[&str]) -> RepoCoverage {
    let mut by_name: FxHashMap<&str, Vec<&str>> = FxHashMap::default();
    for f in repo_files {
        let name = f.rsplit('/').next().unwrap_or(f);
        by_name.entry(name).or_default().push(f);
    }
    let mut out = RepoCoverage::default();
    for (path, lines) in report.files {
        let comps: Vec<&str> = path
            .split('/')
            .filter(|c| !c.is_empty() && *c != ".")
            .collect();
        let Some(name) = comps.last() else { continue };
        let Some(cands) = by_name.get(name) else {
            out.unmatched.push(path);
            continue;
        };
        let score = |cand: &str| -> usize {
            cand.rsplit('/')
                .zip(comps.iter().rev())
                .take_while(|(a, b)| a == *b)
                .count()
        };
        let mut best: Vec<(&str, usize)> = cands.iter().map(|c| (*c, score(c))).collect();
        best.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        if best.len() > 1 && best[0].1 == best[1].1 {
            out.ambiguous.push(path);
            continue;
        }
        let target = best[0].0.to_string();
        let dst = out.files.entry(target).or_default();
        for (l, c) in lines {
            *dst.entry(l).or_insert(false) |= c;
        }
    }
    out
}

/// Read, parse and merge report files (a line is covered if any report covers it). `shown`
/// names a file in warnings; unreadable or unparsable reports become warnings. `None` when no
/// report could be parsed.
pub fn load_reports(
    files: &[std::path::PathBuf],
    shown: impl Fn(&Path) -> String,
) -> (Option<CoverageReport>, Vec<Diagnostic>) {
    let mut merged = CoverageReport::default();
    let mut any = false;
    let mut diags = Vec::new();
    for f in files {
        let name = shown(f);
        match std::fs::read(f) {
            Err(e) => diags.push(Diagnostic::warning(
                Some(&name),
                format!("cannot read coverage report: {e}"),
            )),
            Ok(bytes) => match parse_report(&String::from_utf8_lossy(&bytes)) {
                Ok(r) => {
                    merged.merge(r);
                    any = true;
                }
                Err(e) => diags.push(Diagnostic::warning(
                    Some(&name),
                    format!("cannot parse coverage report: {e}"),
                )),
            },
        }
    }
    (any.then_some(merged), diags)
}

/// [`map_to_repo`], plus a warning when report entries matched several files.
pub fn map_reports(report: CoverageReport, repo_files: &[&str]) -> (RepoCoverage, Vec<Diagnostic>) {
    let mapped = map_to_repo(report, repo_files);
    let mut diags = Vec::new();
    if !mapped.ambiguous.is_empty() {
        diags.push(Diagnostic::warning(
            None,
            format!(
                "{} coverage entries matched several files and were ignored",
                mapped.ambiguous.len()
            ),
        ));
    }
    (mapped, diags)
}

/// Expand report globs (repo-relative) and literal paths into existing files.
pub fn find_reports(
    root: &Path,
    globs: &[String],
    extra: &[std::path::PathBuf],
) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut patterns = Vec::new();
    for g in globs {
        let has_wild = g.contains(['*', '?', '[', '{']);
        if has_wild {
            patterns.push(g.clone());
        } else {
            let p = root.join(g);
            if p.is_file() {
                out.push(p);
            }
        }
    }
    for p in extra {
        let p = if p.is_absolute() {
            p.clone()
        } else {
            root.join(p)
        };
        if p.is_file() {
            out.push(p);
        }
    }
    if !patterns.is_empty() {
        let set = crate::config::compile_path_globs(&patterns);
        let walker = ignore::WalkBuilder::new(root)
            .standard_filters(false)
            .hidden(false)
            .filter_entry(|e| {
                let n = e.file_name();
                !(e.file_type().is_some_and(|t| t.is_dir()) && (n == ".git" || n == "node_modules"))
            })
            .build();
        for entry in walker.flatten() {
            if entry.file_type().is_some_and(|t| t.is_file()) {
                if let Ok(rel) = entry.path().strip_prefix(root) {
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    if set.is_match(&rel) {
                        out.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lcov() {
        let text = "TN:\nSF:src/a.js\nDA:1,1\nDA:2,0\nDA:3,5,abc\nend_of_record\nSF:/abs/b.js\nDA:1,0\nend_of_record\n";
        let r = parse_report(text).unwrap();
        assert_eq!(
            r.files["src/a.js"],
            BTreeMap::from([(1, true), (2, false), (3, true)])
        );
        assert_eq!(r.files["/abs/b.js"], BTreeMap::from([(1, false)]));
    }

    #[test]
    fn jacoco() {
        let text = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<!DOCTYPE report PUBLIC "-//JACOCO//DTD Report 1.1//EN" "report.dtd">
<report name="demo">
  <package name="com/acme">
    <class name="com/acme/Foo" sourcefilename="Foo.java"><method name="a" desc="()V" line="3"/></class>
    <sourcefile name="Foo.java">
      <line nr="3" mi="0" ci="4" mb="0" cb="0"/>
      <line nr="4" mi="2" ci="0" mb="0" cb="0"/>
      <counter type="LINE" missed="1" covered="1"/>
    </sourcefile>
  </package>
</report>"#;
        let r = parse_report(text).unwrap();
        assert_eq!(
            r.files["com/acme/Foo.java"],
            BTreeMap::from([(3, true), (4, false)])
        );
    }

    #[test]
    fn cobertura() {
        let text = r#"<?xml version="1.0" ?>
<coverage line-rate="0.5" version="1">
  <sources><source>/work/src</source></sources>
  <packages><package name="p"><classes>
    <class name="m" filename="pkg/mod.py" line-rate="0.5">
      <methods><method name="f"><lines><line number="2" hits="1"/></lines></method></methods>
      <lines><line number="1" hits="0"/><line number="2" hits="1"/></lines>
    </class>
  </classes></package></packages>
</coverage>"#;
        let r = parse_report(text).unwrap();
        assert_eq!(
            r.files["pkg/mod.py"],
            BTreeMap::from([(1, false), (2, true)])
        );
        assert!(parse_report("<html></html>").is_err());
        // truncated input must not panic
        let _ = parse_report("<report><package name=\"a\"><sourcefile name=\"B.java\"><line nr=");
    }

    #[test]
    fn maps_by_longest_suffix() {
        let mut rep = CoverageReport::default();
        rep.record("com/acme/Foo.java", 1, true);
        rep.record("/ci/build/web/src/util.js", 2, false);
        rep.record("x/Dup.java", 1, true);
        rep.record("Nope.java", 1, true);
        let repo = [
            "core/src/main/java/com/acme/Foo.java",
            "core/src/test/java/com/other/Foo.java",
            "web/src/util.js",
            "a/Dup.java",
            "b/Dup.java",
        ];
        let m = map_to_repo(rep, &repo);
        assert_eq!(
            m.files["core/src/main/java/com/acme/Foo.java"],
            BTreeMap::from([(1, true)])
        );
        assert_eq!(m.files["web/src/util.js"], BTreeMap::from([(2, false)]));
        assert_eq!(m.ambiguous, vec!["x/Dup.java".to_string()]);
        assert_eq!(m.unmatched, vec!["Nope.java".to_string()]);
    }
}
