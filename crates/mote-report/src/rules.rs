//! Rule catalog renderers (`mote rules`, `mote rules explain`, `docs/rules.md`).

use crate::markdown::cell;
use crate::precision::{PrecisionData, PrecisionView};
use mote_core::{ParamValue, RuleStatus};
use mote_engine::RuleInfo;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{self, Write};

/// A parameter default as written in YAML.
pub fn param_value(v: &ParamValue) -> String {
    match v {
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Int(i) => i.to_string(),
        ParamValue::Float(f) => f.to_string(),
        ParamValue::Str(s) => format!("\"{s}\""),
    }
}

fn status_note(r: &RuleInfo) -> &'static str {
    match r.status {
        RuleStatus::Stable => "",
        RuleStatus::Incubating => " (incubating)",
        RuleStatus::Deprecated => " (deprecated)",
    }
}

fn kind_str(r: &RuleInfo) -> &'static str {
    match r.kind {
        mote_core::RuleKind::Bug => "bug",
        mote_core::RuleKind::CodeSmell => "code-smell",
    }
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Display name of a rule language.
pub fn language_name(lang: &str) -> String {
    match lang {
        "java" => "Java".into(),
        "js" => "JavaScript / TypeScript".into(),
        "mote" => "Engine (meta rules)".into(),
        other => other.to_string(),
    }
}

/// Aligned text table: id, severity, quality, default, name.
pub fn write_text(rules: &[RuleInfo], w: &mut dyn Write) -> io::Result<()> {
    let idw = rules.iter().map(|r| r.id.len()).max().unwrap_or(2).max(4);
    writeln!(
        w,
        "{:<idw$}  {:<8}  {:<15}  {:<7}  NAME",
        "RULE", "SEVERITY", "QUALITY", "DEFAULT"
    )?;
    for r in rules {
        writeln!(
            w,
            "{:<idw$}  {:<8}  {:<15}  {:<7}  {}{}",
            r.id,
            r.severity.as_str(),
            r.quality.as_str(),
            if r.default_enabled { "on" } else { "off" },
            r.name,
            status_note(r)
        )?;
    }
    writeln!(w, "\n{} rules", rules.len())
}

/// One rule in JSON outputs: its `RuleInfo` fields in order, then a `precision` object
/// ([`PrecisionView`]) when measured precision data is given.
#[derive(Clone, Debug, Serialize)]
pub struct RuleJson<'a> {
    #[serde(flatten)]
    rule: &'a RuleInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    precision: Option<PrecisionView<'a>>,
}

/// One rule as JSON (`mote rules explain --format json`, the MCP `rule` tool).
pub fn rule_json<'a>(r: &'a RuleInfo, precision: Option<&'a PrecisionData>) -> RuleJson<'a> {
    RuleJson {
        rule: r,
        precision: precision.map(|p| p.json_for(&r.id)),
    }
}

/// The rules as a JSON array (with a `precision` object per rule when data is given).
pub fn write_json(
    rules: &[RuleInfo],
    precision: Option<&PrecisionData>,
    w: &mut dyn Write,
) -> io::Result<()> {
    let list: Vec<RuleJson<'_>> = rules.iter().map(|r| rule_json(r, precision)).collect();
    serde_json::to_writer_pretty(&mut *w, &list)?;
    writeln!(w)
}

/// GitHub-style heading anchor.
pub fn anchor(heading: &str) -> String {
    heading
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c.to_ascii_lowercase())
            } else if c == ' ' {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn params_table(r: &RuleInfo, s: &mut String) {
    if r.params.is_empty() {
        return;
    }
    let _ = writeln!(s, "**Parameters**\n");
    let _ = writeln!(s, "| Name | Default | Description |");
    let _ = writeln!(s, "|---|---|---|");
    for p in &r.params {
        let _ = writeln!(
            s,
            "| `{}` | `{}` | {} |",
            p.name,
            param_value(&p.default),
            cell(&p.description)
        );
    }
    s.push('\n');
}

fn rule_section(r: &RuleInfo, heading: &str, precision: Option<String>, s: &mut String) {
    let _ = writeln!(s, "{heading} {}\n", r.id);
    let _ = writeln!(s, "**{}**{}\n", r.name, status_note(r));
    let mut meta = format!(
        "Severity: **{}** · Quality: {} · Kind: {} · Enabled by default: {} · Remediation: ~{} min",
        r.severity.as_str(),
        r.quality.as_str(),
        kind_str(r),
        yes_no(r.default_enabled),
        r.remediation_minutes
    );
    if !r.tags.is_empty() {
        meta.push_str(&format!(" · Tags: {}", r.tags.join(", ")));
    }
    if r.fixable {
        meta.push_str(" · Fix available (`mote fix`)");
    }
    let _ = writeln!(s, "{meta}\n");
    if let Some(line) = precision {
        let _ = writeln!(s, "{line}\n");
    }
    let desc = r.description.trim();
    if !desc.is_empty() {
        // Demote headings inside descriptions so they nest under the rule heading (not inside
        // fenced code, where `#` is a comment).
        let mut fenced = false;
        for line in desc.lines() {
            if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
                fenced = !fenced;
            }
            if !fenced && line.starts_with('#') {
                let _ = writeln!(s, "###{line}");
            } else {
                let _ = writeln!(s, "{line}");
            }
        }
        s.push('\n');
    }
    params_table(r, s);
}

/// One rule as Markdown (`mote rules explain --format markdown`).
pub fn explain_markdown(r: &RuleInfo, precision: Option<&PrecisionData>) -> String {
    let mut s = String::new();
    rule_section(r, "#", precision.map(|p| p.line(&r.id, true, true)), &mut s);
    s
}

/// One rule as text (`mote rules explain`).
pub fn explain_text(
    r: &RuleInfo,
    precision: Option<&PrecisionData>,
    w: &mut dyn Write,
) -> io::Result<()> {
    writeln!(w, "{} — {}{}", r.id, r.name, status_note(r))?;
    writeln!(
        w,
        "Language: {} · Severity: {} · Quality: {} · Kind: {} · Enabled by default: {} · Remediation: ~{} min",
        r.language,
        r.severity.as_str(),
        r.quality.as_str(),
        kind_str(r),
        yes_no(r.default_enabled),
        r.remediation_minutes
    )?;
    if !r.tags.is_empty() {
        writeln!(w, "Tags: {}", r.tags.join(", "))?;
    }
    if r.fixable {
        writeln!(
            w,
            "Fix available: `mote fix` shows the rewrite of its findings, `mote fix --write` applies it"
        )?;
    }
    if let Some(p) = precision {
        writeln!(w, "{}", p.line(&r.id, true, false))?;
    }
    if !r.params.is_empty() {
        writeln!(w, "\nParameters:")?;
        for p in &r.params {
            writeln!(
                w,
                "  {} (default {}): {}",
                p.name,
                param_value(&p.default),
                p.description
            )?;
        }
    }
    let desc = r.description.trim();
    if !desc.is_empty() {
        writeln!(w, "\n{desc}")?;
    }
    writeln!(
        w,
        "\nConfigure in .mote.yaml:\n  rules:\n    {}: {{ severity: {} }}   # or: off",
        r.id,
        r.severity.as_str()
    )
}

/// The full catalog as a Markdown document (`docs/rules.md`); with precision data, an
/// introduction to the measurement and a precision line per rule.
pub fn render_markdown(rules: &[RuleInfo], precision: Option<&PrecisionData>) -> String {
    let mut by_lang: BTreeMap<&str, Vec<&RuleInfo>> = BTreeMap::new();
    for r in rules {
        by_lang.entry(r.language.as_str()).or_default().push(r);
    }
    // Meta rules last.
    let mut langs: Vec<&str> = by_lang.keys().copied().collect();
    langs.sort_by_key(|l| (*l == "mote", *l));
    let mut s = String::new();
    let _ = writeln!(s, "# Mote rules\n");
    let _ = writeln!(
        s,
        "<!-- Generated by `mote rules --all --format markdown`; do not edit by hand. -->\n"
    );
    let enabled = rules.iter().filter(|r| r.default_enabled).count();
    let _ = writeln!(
        s,
        "{} rules, {} enabled by default. Severity: **HIGH** — almost certainly a bug or a serious reliability hazard; \
**MEDIUM** — a likely problem or a significant maintainability cost; **LOW** — minor or stylistic. \
The default quality gate fails a pull request on new HIGH findings only. Change severities, parameters \
and enablement in `.mote.yaml` (`mote config explain <file>` shows the effective settings).\n",
        rules.len(),
        enabled
    );
    for l in &langs {
        let _ = writeln!(
            s,
            "- [{}](#{}) — {} rules",
            language_name(l),
            anchor(&language_name(l)),
            by_lang[l].len()
        );
    }
    s.push('\n');
    if let Some(p) = precision {
        let _ = writeln!(
            s,
            "{}",
            p.intro_markdown(rules.iter().map(|r| r.id.as_str()))
        );
    }
    for l in &langs {
        let list = &by_lang[l];
        let _ = writeln!(s, "## {}\n", language_name(l));
        let _ = writeln!(s, "| Rule | Name | Severity | Quality | Default |");
        let _ = writeln!(s, "|---|---|---|---|---|");
        for r in list.iter() {
            let _ = writeln!(
                s,
                "| [`{}`](#{}) | {}{} | {} | {} | {} |",
                r.id,
                anchor(&r.id),
                cell(&r.name),
                status_note(r),
                r.severity.as_str(),
                r.quality.as_str(),
                if r.default_enabled { "on" } else { "off" }
            );
        }
        s.push('\n');
        for r in list.iter() {
            rule_section(
                r,
                "###",
                precision.map(|p| p.line(&r.id, false, true)),
                &mut s,
            );
        }
    }
    while s.ends_with("\n\n") {
        s.pop();
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, with_base};

    #[test]
    fn catalog_renderers() {
        let rules = with_base().rules;
        let mut out = Vec::new();
        write_text(&rules, &mut out).unwrap();
        assert_snapshot("rules.txt", &String::from_utf8(out).unwrap());
        let md = render_markdown(&rules, None);
        assert_snapshot("rules.md", &md);
        assert!(md.contains("[`java/self-assignment`](#javaself-assignment)"));
        assert!(md.contains("| `threshold` | `15` | Maximum allowed. |"));
        let mut out = Vec::new();
        write_json(&rules, None, &mut out).unwrap();
        let v: Vec<RuleInfo> = serde_json::from_slice(&out).unwrap();
        assert_eq!(v, rules);
        let mut out = Vec::new();
        explain_text(&rules[3], None, &mut out).unwrap();
        let t = String::from_utf8(out).unwrap();
        assert!(t.starts_with("java/self-assignment — Self-assignment"));
        assert!(t.contains("threshold (default 15)"));
        assert!(!t.contains("Measured precision"));
        assert!(explain_markdown(&rules[3], None).starts_with("# java/self-assignment"));
    }

    #[test]
    fn catalog_with_precision() {
        let rules = with_base().rules;
        let data = crate::precision::tests::sample();
        // JSON: the RuleInfo fields are unchanged, `precision` is added.
        let mut out = Vec::new();
        write_json(&rules, Some(&data), &mut out).unwrap();
        let v: Vec<RuleInfo> = serde_json::from_slice(&out).unwrap();
        assert_eq!(v, rules);
        let raw: Vec<serde_json::Value> = serde_json::from_slice(&out).unwrap();
        assert_eq!(raw[2]["id"], "java/empty-catch-block");
        assert_eq!(raw[2]["precision"]["state"], "measured");
        assert_eq!(raw[2]["precision"]["tp"], 59);
        assert_eq!(raw[0]["precision"]["state"], "not-measured-yet");
        let one = |p| serde_json::to_value(rule_json(&rules[4], p)).unwrap();
        assert_eq!(one(Some(&data))["precision"]["state"], "too-few-labels");
        assert!(one(None).get("precision").is_none());
        // Without data the bytes are those of the plain RuleInfo list (field order kept).
        let mut plain = Vec::new();
        serde_json::to_writer_pretty(&mut plain, &rules).unwrap();
        plain.push(b'\n');
        let mut none = Vec::new();
        write_json(&rules, None, &mut none).unwrap();
        assert_eq!(
            String::from_utf8(none).unwrap(),
            String::from_utf8(plain).unwrap()
        );
        assert!(String::from_utf8(out.clone())
            .unwrap()
            .contains("\"fixable\": false,\n    \"precision\": {\n      \"state\""));
        // Text: one line after the metadata.
        let mut out = Vec::new();
        explain_text(&rules[2], Some(&data), &mut out).unwrap();
        let t = String::from_utf8(out).unwrap();
        // After the metadata (and tags), before the description.
        let line = t
            .lines()
            .position(|l| l == "Measured precision: 98% (59 of 60 labeled findings correct, measured 2026-10-12 on 34 open-source repositories)")
            .unwrap_or_else(|| panic!("{t}"));
        assert!(line >= 2 && t.lines().nth(line + 1) == Some(""), "{t}");
        let md = explain_markdown(&rules[2], Some(&data));
        assert!(
            md.contains("\n\nMeasured precision: **98%** (59 of 60 labeled findings correct, measured 2026-10-12 on 34 open-source repositories)\n\n"),
            "{md}"
        );
        // Catalog: the introduction before the first language, a short line per rule.
        let md = render_markdown(&rules, Some(&data));
        let intro = md.find("## Measured precision").expect("intro");
        assert!(intro < md.find("## Java").unwrap(), "{md}");
        assert!(
            md.contains("Of the 5 rules listed here: 2 with a measured precision"),
            "{md}"
        );
        assert!(
            md.contains("\n\nMeasured precision: **98%** (59 of 60 labeled findings correct)\n\n"),
            "{md}"
        );
        assert!(
            md.contains(
                "Measured precision: not measured: no findings in the benchmark repositories"
            ),
            "{md}"
        );
        assert!(
            md.contains("Measured precision: not measured yet: the rule is newer than the last measurement (2026-10-12)"),
            "{md}"
        );
        assert!(!render_markdown(&rules, None).contains("Measured precision"));
    }
}
