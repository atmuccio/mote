//! Human-readable output: findings grouped by file (`path:line:col  SEV  rule  message`), then a
//! summary with the gate verdict. ANSI colors only when asked for.

use crate::{
    condition_label, fmt_condition_threshold, fmt_condition_value, fmt_hours, fmt_ms, has_base,
    short_commit, thousands, Filter,
};
use mote_core::{Finding, FindingStatus, Severity};
use mote_engine::result::FileStatus;
use mote_engine::{GateResult, ScanResult};
use std::io::{self, Write};

/// Text output options.
#[derive(Clone, Copy, Debug, Default)]
pub struct TextOptions {
    pub filter: Filter,
    /// Emit ANSI color codes.
    pub color: bool,
}

/// ANSI styling that turns into nothing when colors are off.
#[derive(Clone, Copy, Debug)]
pub struct Painter {
    pub enabled: bool,
}

impl Painter {
    fn paint(&self, code: &str, s: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn bold(&self, s: &str) -> String {
        self.paint("1", s)
    }
    pub fn dim(&self, s: &str) -> String {
        self.paint("2", s)
    }
    pub fn red(&self, s: &str) -> String {
        self.paint("1;31", s)
    }
    pub fn green(&self, s: &str) -> String {
        self.paint("1;32", s)
    }
    pub fn yellow(&self, s: &str) -> String {
        self.paint("33", s)
    }
    pub fn cyan(&self, s: &str) -> String {
        self.paint("36", s)
    }
    pub fn severity(&self, sev: Severity, s: &str) -> String {
        match sev {
            Severity::High => self.red(s),
            Severity::Medium => self.yellow(s),
            Severity::Low => self.cyan(s),
        }
    }
}

/// Write findings and the summary.
pub fn write(result: &ScanResult, opts: &TextOptions, w: &mut dyn Write) -> io::Result<()> {
    let p = Painter {
        enabled: opts.color,
    };
    let visible: Vec<&Finding> = opts.filter.apply(result).collect();
    for (i, group) in visible.chunk_by(|a, b| a.path == b.path).enumerate() {
        if i > 0 {
            writeln!(w)?;
        }
        let loc_w = group.iter().map(|f| loc(f).len()).max().unwrap_or(0);
        let rule_w = group.iter().map(|f| f.rule_id.len()).max().unwrap_or(0);
        for f in group {
            write_finding_padded(f, &p, loc_w, rule_w, w)?;
        }
    }
    if !visible.is_empty() {
        writeln!(w)?;
    }
    let hidden = result
        .findings
        .iter()
        .filter(|f| f.suppressed.is_none() && !opts.filter.accepts(f))
        .count();
    write_summary(result, &p, hidden, w)
}

fn loc(f: &Finding) -> String {
    format!("{}:{}:{}", f.path, f.range.start_line, f.range.start_col)
}

/// One finding line: `path:line:col  SEV  rule-id  message [new]`.
pub fn write_finding(f: &Finding, p: &Painter, w: &mut dyn Write) -> io::Result<()> {
    write_finding_padded(f, p, 0, 0, w)
}

fn write_finding_padded(
    f: &Finding,
    p: &Painter,
    loc_w: usize,
    rule_w: usize,
    w: &mut dyn Write,
) -> io::Result<()> {
    let loc = format!("{:<loc_w$}", loc(f));
    let sev = format!("{:<6}", f.severity.as_str());
    let rule = format!("{:<rule_w$}", f.rule_id);
    let mut tags = String::new();
    match f.status {
        FindingStatus::New => tags.push_str(&format!(" {}", p.bold("[new]"))),
        FindingStatus::Worsened => tags.push_str(&format!(" {}", p.bold("[worsened]"))),
        _ => {}
    }
    if let Some(s) = &f.suppressed {
        let why = s.justification.as_deref().unwrap_or("no reason");
        tags.push_str(&p.dim(&format!(" [suppressed: {why}]")));
    }
    writeln!(
        w,
        "{}  {}  {}  {}{}",
        p.bold(&loc),
        p.severity(f.severity, &sev),
        p.dim(&rule),
        f.message,
        tags
    )
}

struct Counts {
    total: [u64; 3],
    new: [u64; 3],
    worsened: u64,
    suppressed: u64,
}

fn idx(s: Severity) -> usize {
    match s {
        Severity::High => 0,
        Severity::Medium => 1,
        Severity::Low => 2,
    }
}

fn counts(result: &ScanResult) -> Counts {
    let mut c = Counts {
        total: [0; 3],
        new: [0; 3],
        worsened: 0,
        suppressed: 0,
    };
    for f in &result.findings {
        if f.suppressed.is_some() {
            c.suppressed += 1;
            continue;
        }
        c.total[idx(f.severity)] += 1;
        match f.status {
            FindingStatus::New => c.new[idx(f.severity)] += 1,
            FindingStatus::Worsened => c.worsened += 1,
            _ => {}
        }
    }
    c
}

const SEVS: [Severity; 3] = [Severity::High, Severity::Medium, Severity::Low];

/// The summary block: files, findings, new code, duplication, coverage, gate, timing.
pub fn write_summary(
    result: &ScanResult,
    p: &Painter,
    hidden: usize,
    w: &mut dyn Write,
) -> io::Result<()> {
    let (mut analyzed, mut cached, mut skipped) = (0u64, 0u64, 0u64);
    for f in &result.files {
        match f.status {
            FileStatus::Analyzed => analyzed += 1,
            FileStatus::Cached => cached += 1,
            FileStatus::Skipped => skipped += 1,
        }
    }
    let label = |s: &str| p.bold(&format!("{s:<14}"));
    // e.g. " (generated 3, too-large 1)" — skipped files are otherwise invisible.
    let reasons: Vec<String> = result
        .stats
        .skipped_by_reason
        .iter()
        .filter(|(reason, _)| reason.as_str() != "excluded")
        .map(|(reason, n)| format!("{reason} {}", thousands(*n)))
        .collect();
    let reasons = if reasons.is_empty() {
        String::new()
    } else {
        format!(" [{}]", reasons.join(", "))
    };
    let t = &result.measures.total;
    let in_tests = if result.measures.test.ncloc > 0 {
        format!(", {} in tests", thousands(result.measures.test.ncloc))
    } else {
        String::new()
    };
    writeln!(
        w,
        "{}{} analyzed, {} cached, {} skipped{} ({} lines of code{in_tests})",
        label("Files"),
        thousands(analyzed),
        thousands(cached),
        thousands(skipped),
        reasons,
        thousands(t.ncloc)
    )?;
    let c = counts(result);
    let base = has_base(result);
    let sev_list = |vals: &[u64; 3]| -> String {
        SEVS.iter()
            .map(|s| p.severity(*s, &format!("{} {}", thousands(vals[idx(*s)]), s.as_str())))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let total: u64 = c.total.iter().sum();
    let mut line = format!("{} ({} total", sev_list(&c.total), thousands(total));
    if c.suppressed > 0 {
        line.push_str(&format!(", {} suppressed", thousands(c.suppressed)));
    }
    line.push(')');
    writeln!(w, "{}{}", label("Findings"), line)?;
    if let Some(nc) = &result.new_code {
        match &nc.baseline {
            Some(b) => writeln!(
                w,
                "{}vs baseline {}{}: {} of its {} findings still present",
                label("New code"),
                b.file,
                b.commit
                    .as_deref()
                    .map(|c| format!(" (commit {})", short_commit(c)))
                    .unwrap_or_default(),
                thousands(b.matched),
                thousands(b.findings)
            )?,
            None => writeln!(
                w,
                "{}vs {} (merge base {}): {} file{} changed, {} new line{}",
                label("New code"),
                nc.base,
                short_commit(&nc.merge_base),
                thousands(nc.measures.changed_files),
                if nc.measures.changed_files == 1 {
                    ""
                } else {
                    "s"
                },
                thousands(nc.measures.new_lines),
                if nc.measures.new_lines == 1 { "" } else { "s" }
            )?,
        }
        let new_total: u64 = c.new.iter().sum();
        writeln!(
            w,
            "{}{} ({} new, {} worsened, {} fixed)",
            label("New findings"),
            sev_list(&c.new),
            thousands(new_total),
            thousands(c.worsened),
            thousands(u64::from(nc.measures.fixed_findings.total()))
        )?;
    }
    if t.files > 0 {
        writeln!(
            w,
            "{}{} cognitive, {} cyclomatic ({} function{})",
            label("Complexity"),
            thousands(t.cognitive_complexity),
            thousands(t.cyclomatic_complexity),
            thousands(t.functions),
            if t.functions == 1 { "" } else { "s" }
        )?;
        let e = &t.effort_minutes;
        let mut effort = format!(
            "{} estimated (reliability {}, maintainability {})",
            fmt_hours(e.total),
            fmt_hours(e.reliability),
            fmt_hours(e.maintainability)
        );
        if let Some(nc) = &result.new_code {
            effort.push_str(&format!(
                ", {} on new code",
                fmt_hours(nc.measures.new_effort_minutes.total)
            ));
        }
        writeln!(w, "{}{effort}", label("Effort"))?;
        if let Some(g) = &result.grades {
            writeln!(
                w,
                "{}maintainability {} ({:.2} h per 1,000 lines), reliability {} ({:.2} per 10,000 lines)",
                label("Grades"),
                p.bold(g.maintainability.grade.as_str()),
                g.maintainability.value,
                p.bold(g.reliability.grade.as_str()),
                g.reliability.value
            )?;
            if let (Some(m), Some(r)) = (&g.new_maintainability, &g.new_reliability) {
                writeln!(
                    w,
                    "{}on new code: maintainability {} ({:.2}), reliability {} ({:.2})",
                    label(""),
                    p.bold(m.grade.as_str()),
                    m.value,
                    p.bold(r.grade.as_str()),
                    r.value
                )?;
            } else if let Some(n) = &g.new_not_rated {
                writeln!(w, "{}on new code: {}", label(""), n.describe())?;
            }
        }
    }
    let mut dup = format!("{:.1}% overall", t.duplicated_lines_density);
    if let Some(nc) = result.new_code.as_ref().filter(|nc| nc.baseline.is_none()) {
        dup.push_str(&format!(
            ", {:.1}% on new code",
            nc.measures.new_duplication_pct
        ));
    }
    writeln!(w, "{}{}", label("Duplication"), dup)?;
    let mut cov = Vec::new();
    if let Some(c) = &result.measures.coverage {
        cov.push(format!("{:.1}% overall", c.coverage_pct));
    }
    if let Some(pct) = result
        .new_code
        .as_ref()
        .and_then(|n| n.measures.new_coverage_pct)
    {
        cov.push(format!("{pct:.1}% on new code"));
    }
    if !cov.is_empty() {
        writeln!(w, "{}{}", label("Coverage"), cov.join(", "))?;
    }
    write_gate(&result.gate, p, w)?;
    // Incubating rules report but never gate: say so, or the gate's count and the HIGH count
    // above disagree without a reason ("33 HIGH" findings, "Findings 30").
    let mut incubating = [0u64; 3];
    for f in result.findings.iter().filter(|f| {
        f.incubating && f.suppressed.is_none() && (!base || f.status == FindingStatus::New)
    }) {
        incubating[idx(f.severity)] += 1;
    }
    let n_incubating: u64 = incubating.iter().sum();
    if n_incubating > 0 {
        let by_sev: Vec<String> = SEVS
            .iter()
            .filter(|s| incubating[idx(**s)] > 0)
            .map(|s| format!("{} {}", thousands(incubating[idx(*s)]), s.as_str()))
            .collect();
        writeln!(
            w,
            "{}",
            p.dim(&format!(
                "{} {}finding{} of incubating rules ({}) {} reported but never gate{}",
                thousands(n_incubating),
                if base { "new " } else { "" },
                if n_incubating == 1 { "" } else { "s" },
                by_sev.join(", "),
                if n_incubating == 1 { "is" } else { "are" },
                if n_incubating == 1 { "s" } else { "" },
            ))
        )?;
    }
    if hidden > 0 {
        writeln!(
            w,
            "{}",
            p.dim(&format!(
                "{} findings hidden by --min-severity / --new-only",
                thousands(hidden as u64)
            ))
        )?;
    }
    if !base && result.findings.iter().any(|f| f.suppressed.is_none()) {
        // Without a base every finding counts; say so once to avoid surprises.
        writeln!(
            w,
            "{}",
            p.dim("No --base given: the gate applies to all findings.")
        )?;
    }
    if result.stats.timings.total_ms > 0.0 {
        writeln!(
            w,
            "{}{}",
            label("Done in"),
            fmt_ms(result.stats.timings.total_ms)
        )?;
    }
    Ok(())
}

/// `Quality gate: PASSED|FAILED` and one line per condition.
pub fn write_gate(gate: &GateResult, p: &Painter, w: &mut dyn Write) -> io::Result<()> {
    let verdict = if gate.passed {
        p.green("PASSED")
    } else {
        p.red("FAILED")
    };
    writeln!(
        w,
        "{}{}",
        p.bold(&format!("{:<14}", "Quality gate")),
        verdict
    )?;
    // Labels in one column (26 wide, wider when a label needs it).
    let width = gate
        .conditions
        .iter()
        .map(|c| condition_label(&c.name).chars().count() + 2)
        .max()
        .unwrap_or(0)
        .max(26);
    for c in &gate.conditions {
        let label = format!("{:<width$}", condition_label(&c.name));
        if let Some(reason) = &c.skipped_reason {
            writeln!(w, "  {}  {}{}", p.dim("skip"), label, p.dim(reason))?;
            continue;
        }
        let status = if c.passed {
            p.green("pass")
        } else {
            p.red("FAIL")
        };
        let actual = c
            .actual
            .map(|v| fmt_condition_value(&c.name, v))
            .unwrap_or_else(|| "-".into());
        let threshold = c
            .threshold
            .map(|v| format!(" ({})", fmt_condition_threshold(&c.name, v)))
            .unwrap_or_default();
        writeln!(w, "  {status}  {label}{actual}{threshold}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, with_base, without_base};

    fn render(r: &ScanResult, opts: TextOptions) -> String {
        let mut out = Vec::new();
        write(r, &opts, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn with_base_snapshot() {
        assert_snapshot(
            "text_with_base.txt",
            &render(&with_base(), TextOptions::default()),
        );
    }

    /// `grades.min_new_lines` (100 by default): a change one line short of it is not rated, one
    /// of exactly that size is.
    #[test]
    fn new_code_is_rated_from_the_grades_minimum() {
        let out = render(&crate::testdata::change_of(99), TextOptions::default());
        assert!(
            out.contains("on new code: not rated (only 99 new lines; grades need at least 100)\n"),
            "{out}"
        );
        let out = render(&crate::testdata::change_of(100), TextOptions::default());
        assert!(!out.contains("not rated"), "{out}");
        assert!(out.contains("on new code: maintainability "), "{out}");
    }

    #[test]
    fn a_small_change_is_not_graded() {
        let out = render(&crate::testdata::small_change(), TextOptions::default());
        assert!(
            out.contains(
                "Grades        maintainability D (3.61 h per 1,000 lines), reliability E (41.67 per 10,000 lines)\n              \
                 on new code: not rated (only 11 new lines; grades need at least 100)\n"
            ),
            "{out}"
        );
    }

    #[test]
    fn without_base_snapshot() {
        assert_snapshot(
            "text_without_base.txt",
            &render(&without_base(), TextOptions::default()),
        );
    }

    #[test]
    fn baseline_summary() {
        let out = render(&crate::testdata::with_baseline(), TextOptions::default());
        assert!(
            out.contains(
                "vs baseline mote-baseline.json (commit abcdef012345): 3 of its 5 findings still present"
            ),
            "{out}"
        );
        assert!(!out.contains("merge base"), "{out}");
        assert!(
            !out.contains("% on new code"),
            "no line-based new-code measures: {out}"
        );
        assert!(out.contains(mote_engine::gate::NO_LINE_DIFF), "{out}");
    }

    #[test]
    fn filters_and_suppressed() {
        let r = with_base();
        let opts = TextOptions {
            filter: Filter {
                new_only: true,
                min_severity: Severity::Medium,
                show_suppressed: true,
            },
            color: false,
        };
        let out = render(&r, opts);
        assert!(out.contains("java/self-assignment"));
        assert!(out.contains("[worsened]"));
        assert!(!out.contains("java/empty-catch-block"), "{out}");
        assert!(!out.contains("src/c/Util.java:1:1"), "LOW hidden");
        assert!(out.contains("findings hidden"));
        // suppressed LOW is below min severity here; show it with a LOW threshold
        let opts = TextOptions {
            filter: Filter {
                show_suppressed: true,
                ..Filter::default()
            },
            color: false,
        };
        let out = render(&r, opts);
        assert!(out.contains("[suppressed: tracked in JIRA-1]"), "{out}");
        assert!(!render(&r, TextOptions::default()).contains("[suppressed"));
    }

    #[test]
    fn colors_only_when_enabled() {
        let r = with_base();
        assert!(!render(&r, TextOptions::default()).contains('\x1b'));
        let colored = render(
            &r,
            TextOptions {
                color: true,
                ..Default::default()
            },
        );
        assert!(colored.contains("\x1b[1;31mHIGH"));
        assert!(colored.contains("\x1b[1;31mFAILED"));
    }

    #[test]
    fn one_file_and_one_line_read_in_the_singular() {
        let mut r = with_base();
        let nc = r.new_code.as_mut().unwrap();
        nc.measures.changed_files = 1;
        nc.measures.new_lines = 1;
        let out = render(&r, TextOptions::default());
        assert!(out.contains("): 1 file changed, 1 new line\n"), "{out}");
    }

    #[test]
    fn incubating_findings_are_said_not_to_gate() {
        assert!(!render(&without_base(), TextOptions::default()).contains("incubating"));
        let mut r = without_base();
        let high = r
            .findings
            .iter_mut()
            .find(|f| f.severity == Severity::High && f.suppressed.is_none())
            .unwrap();
        high.incubating = true;
        let out = render(&r, TextOptions::default());
        assert!(
            out.contains("1 finding of incubating rules (1 HIGH) is reported but never gates\n"),
            "{out}"
        );
        // With a base, only the new ones are counted, like the gate.
        let mut r = with_base();
        for f in &mut r.findings {
            f.incubating = true;
        }
        let new = r
            .findings
            .iter()
            .filter(|f| f.suppressed.is_none() && f.status == FindingStatus::New)
            .count();
        let out = render(&r, TextOptions::default());
        assert!(
            out.contains(&format!("{new} new findings of incubating rules (")),
            "{out}"
        );
    }

    #[test]
    fn empty_result() {
        let out = render(&empty(), TextOptions::default());
        assert!(out.starts_with("Files"), "{out}");
        assert!(out.contains("PASSED"));
    }
}
