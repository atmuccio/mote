//! Markdown summary for `$GITHUB_STEP_SUMMARY`, the sticky PR comment and the check-run summary.
//!
//! The first line is always exactly [`MARKER`](crate::MARKER). The document stays below
//! `max_bytes` (default 60,000; GitHub limits check-run summaries to 65,535 characters and
//! comments to 65,536): the findings table is cut first, with a note.

use crate::{
    by_severity, condition_is_grade, condition_label, condition_op, fmt_condition_threshold,
    fmt_condition_value, fmt_hours, repo_path, short_commit, thousands, truncate_bytes, Filter,
    MARKER,
};
use mote_core::{Finding, FindingStatus, Severity};
use mote_engine::grades::Rating;
use mote_engine::result::FileStatus;
use mote_engine::ScanResult;
use std::fmt::Write as _;

/// Markdown options.
#[derive(Clone, Copy, Debug)]
pub struct MarkdownOptions<'a> {
    /// Which findings the table lists (suppressed findings never are).
    pub filter: Filter,
    /// Rows in the top-findings table.
    pub max_findings: usize,
    /// Size limit of the whole document, in bytes.
    pub max_bytes: usize,
    /// Repository-relative path of the scan root (`backend/`), prepended to locations so they
    /// match the repository GitHub shows; empty at the top level.
    pub path_prefix: &'a str,
}

impl Default for MarkdownOptions<'_> {
    fn default() -> Self {
        MarkdownOptions {
            filter: Filter::default(),
            max_findings: 50,
            max_bytes: 60_000,
            path_prefix: "",
        }
    }
}

const SEVS: [Severity; 3] = [Severity::High, Severity::Medium, Severity::Low];
const MAX_MESSAGE: usize = 240;

/// Render plain text that may contain `code spans` (a finding message, which can quote the
/// scanned code) as the content of a GitHub-flavored Markdown table cell.
///
/// Code spans are found with the CommonMark rule (a backtick run opens a span only when a later
/// run of the same length closes it) and re-emitted with [`code`], so their content — `<h1>`
/// included — always shows literally. Everything else is plain text: Markdown punctuation is
/// backslash-escaped (backslashes and stray backticks too, so nothing in the text can open a span,
/// escape the next character or start a link), `<`, `>` and `&` become entities (no raw HTML),
/// `|` is escaped and line breaks become spaces (the row stays one table row).
pub fn cell(s: &str) -> String {
    let s = truncate_chars(s, MAX_MESSAGE);
    let chars: Vec<char> = s.chars().collect();
    // Backtick runs: (start, length).
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            let start = i;
            while i < chars.len() && chars[i] == '`' {
                i += 1;
            }
            runs.push((start, i - start));
        } else {
            i += 1;
        }
    }
    let mut out = String::with_capacity(s.len() + 16);
    let mut pos = 0; // next unconsumed char
    let mut r = 0;
    while r < runs.len() {
        let (start, len) = runs[r];
        if start < pos {
            r += 1;
            continue;
        }
        match runs[r + 1..].iter().position(|&(_, l)| l == len) {
            Some(k) => {
                let (close, _) = runs[r + 1 + k];
                push_text(&mut out, &chars[pos..start]);
                let content: String = chars[start + len..close].iter().collect();
                out.push_str(&code(&strip_code_padding(&content)));
                pos = close + len;
                r += k + 2;
            }
            // No closing run: these backticks are literal text.
            None => r += 1,
        }
    }
    push_text(&mut out, &chars[pos..]);
    out
}

/// CommonMark strips one space from both ends of a code span's content when it starts and ends
/// with a space (and is not all spaces).
fn strip_code_padding(s: &str) -> String {
    let s = s.replace(['\n', '\r'], " ");
    if s.len() >= 2 && s.starts_with(' ') && s.ends_with(' ') && !s.bytes().all(|b| b == b' ') {
        s[1..s.len() - 1].to_string()
    } else {
        s
    }
}

/// Append plain text escaped for a table cell (see [`cell`]).
fn push_text(out: &mut String, text: &[char]) {
    for &c in text {
        match c {
            '\\' | '`' | '*' | '_' | '[' | ']' | '~' | '|' => {
                out.push('\\');
                out.push(c);
            }
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\n' | '\r' => out.push(' '),
            c => out.push(c),
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Inline code span for a table cell, showing `s` literally: the fence is one backtick longer
/// than the longest backtick run inside, `|` is escaped (GFM splits cells before parsing code
/// spans) and line breaks become spaces.
pub fn code(s: &str) -> String {
    code_span(s, true)
}

/// Inline code span; `table` escapes `|` for use inside a table cell.
fn code_span(s: &str, table: bool) -> String {
    if s.is_empty() {
        // "``" would be a literal backtick pair, not an empty span.
        return String::new();
    }
    let mut content = String::with_capacity(s.len() + 2);
    let (mut run, mut longest) = (0usize, 0usize);
    for c in s.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
        match c {
            '|' if table => content.push_str("\\|"),
            '\n' | '\r' => content.push(' '),
            c => content.push(c),
        }
    }
    let fence = "`".repeat(longest + 1);
    // A space on both sides keeps a leading/trailing backtick from merging with the fence;
    // CommonMark strips exactly one on each side, so content already padded that way gets more.
    let pad = content.starts_with('`')
        || content.ends_with('`')
        || (content.len() >= 2
            && content.starts_with(' ')
            && content.ends_with(' ')
            && !content.bytes().all(|b| b == b' '));
    if pad {
        format!("{fence} {content} {fence}")
    } else {
        format!("{fence}{content}{fence}")
    }
}

fn icon(passed: bool) -> &'static str {
    if passed {
        ":white_check_mark:"
    } else {
        ":x:"
    }
}

/// Render the summary.
pub fn render(result: &ScanResult, opts: &MarkdownOptions) -> String {
    let head = head(result);
    let tail = tail(result);
    let findings = findings_section(
        result,
        opts,
        opts.max_bytes.saturating_sub(head.len() + tail.len() + 256),
    );
    let mut out = String::with_capacity(head.len() + findings.len() + tail.len());
    out.push_str(&head);
    out.push_str(&findings);
    out.push_str(&tail);
    if out.len() > opts.max_bytes {
        let note =
            "\n\n> [!NOTE]\n> Summary truncated to fit GitHub limits; see the full results file.\n";
        let keep = truncate_bytes(&out, opts.max_bytes.saturating_sub(note.len())).len();
        out.truncate(keep);
        out.push_str(note);
    }
    out
}

fn head(r: &ScanResult) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{MARKER}");
    let verdict = if r.gate.passed { "passed" } else { "failed" };
    let _ = writeln!(
        s,
        "## {} Mote: quality gate {verdict}\n",
        icon(r.gate.passed)
    );
    match &r.new_code {
        Some(nc) if nc.baseline.is_some() => {
            let b = nc.baseline.as_ref().map(|b| (b.matched, b.findings));
            let (matched, total) = b.unwrap_or_default();
            let _ = writeln!(
                s,
                "Compared with the baseline {}{}: findings not in it are new (**{}** of its {} findings still present).\n",
                code_span(&nc.base, false),
                if nc.merge_base.is_empty() {
                    String::new()
                } else {
                    format!(" (commit {})", code_span(short_commit(&nc.merge_base), false))
                },
                thousands(matched),
                thousands(total)
            );
        }
        Some(nc) => {
            let _ = writeln!(
                s,
                "Compared with {} (merge base {}): **{}** file{} changed, **{}** new line{}.\n",
                code_span(&nc.base, false),
                code_span(short_commit(&nc.merge_base), false),
                thousands(nc.measures.changed_files),
                if nc.measures.changed_files == 1 {
                    ""
                } else {
                    "s"
                },
                thousands(nc.measures.new_lines),
                if nc.measures.new_lines == 1 { "" } else { "s" }
            );
        }
        None => {
            let _ = writeln!(
                s,
                "No base to compare with: the gate applies to all findings.\n"
            );
        }
    }
    if !r.gate.conditions.is_empty() {
        let _ = writeln!(s, "| Condition | Result | Actual | Threshold |");
        let _ = writeln!(s, "|---|---|---:|---:|");
        for c in &r.gate.conditions {
            let threshold = c
                .threshold
                .map(|v| {
                    if condition_is_grade(&c.name) {
                        fmt_condition_threshold(&c.name, v)
                    } else {
                        format!(
                            "{} {}",
                            if condition_op(&c.name) == ">=" {
                                "≥"
                            } else {
                                "≤"
                            },
                            fmt_condition_value(&c.name, v)
                        )
                    }
                })
                .unwrap_or_default();
            let (result, actual) = match &c.skipped_reason {
                Some(reason) => (
                    format!(":fast_forward: skipped ({})", cell(reason)),
                    String::new(),
                ),
                None => (
                    format!(
                        "{} {}",
                        icon(c.passed),
                        if c.passed { "passed" } else { "failed" }
                    ),
                    c.actual
                        .map(|v| fmt_condition_value(&c.name, v))
                        .unwrap_or_default(),
                ),
            };
            let _ = writeln!(
                s,
                "| {} | {result} | {actual} | {threshold} |",
                condition_label(&c.name)
            );
        }
        s.push('\n');
    }

    let idx = |sev: Severity| match sev {
        Severity::High => 0,
        Severity::Medium => 1,
        Severity::Low => 2,
    };
    let (mut total, mut new, mut worse) = ([0u64; 3], [0u64; 3], [0u64; 3]);
    for f in r.findings.iter().filter(|f| f.suppressed.is_none()) {
        total[idx(f.severity)] += 1;
        match f.status {
            FindingStatus::New => new[idx(f.severity)] += 1,
            FindingStatus::Worsened => worse[idx(f.severity)] += 1,
            _ => {}
        }
    }
    match &r.new_code {
        Some(nc) => {
            let fixed = &nc.measures.fixed_findings;
            let fixed = [fixed.high, fixed.medium, fixed.low];
            let _ = writeln!(s, "### New findings\n");
            let _ = writeln!(s, "| Severity | New | Worsened | Fixed | Total |");
            let _ = writeln!(s, "|---|---:|---:|---:|---:|");
            for sev in SEVS {
                let i = idx(sev);
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} | {} |",
                    sev.as_str(),
                    thousands(new[i]),
                    thousands(worse[i]),
                    thousands(u64::from(fixed[i])),
                    thousands(total[i])
                );
            }
        }
        None => {
            let _ = writeln!(s, "### Findings\n");
            let _ = writeln!(s, "| Severity | Total |");
            let _ = writeln!(s, "|---|---:|");
            for sev in SEVS {
                let _ = writeln!(s, "| {} | {} |", sev.as_str(), thousands(total[idx(sev)]));
            }
        }
    }
    s.push('\n');
    s
}

fn row(f: &Finding, prefix: &str) -> String {
    let mut msg = cell(&f.message);
    if f.status == FindingStatus::Worsened {
        msg.push_str(" _(worsened)_");
    }
    format!(
        "| {} | {} | {} | {} |\n",
        f.severity.as_str(),
        code(&format!(
            "{}:{}",
            repo_path(prefix, &f.path),
            f.range.start_line
        )),
        code(&f.rule_id),
        msg
    )
}

fn findings_section(r: &ScanResult, opts: &MarkdownOptions, budget: usize) -> String {
    let base = r.new_code.is_some();
    let filter = Filter {
        new_only: base,
        show_suppressed: false,
        ..opts.filter
    };
    let list = by_severity(filter.apply(r));
    if list.is_empty() {
        return if base {
            "No new findings. :tada:\n\n".to_string()
        } else {
            String::new()
        };
    }
    let title = if base {
        "Top new findings"
    } else {
        "Top findings"
    };
    let mut s = String::new();
    let mut rows = String::new();
    let mut shown = 0usize;
    let mut cut_for_size = false;
    for f in list.iter().take(opts.max_findings) {
        let line = row(f, opts.path_prefix);
        if rows.len() + line.len() + 200 > budget {
            cut_for_size = true;
            break;
        }
        rows.push_str(&line);
        shown += 1;
    }
    let _ = writeln!(
        s,
        "### {title} ({} of {})\n",
        thousands(shown as u64),
        thousands(list.len() as u64)
    );
    if shown > 0 {
        s.push_str("| Severity | Location | Rule | Message |\n|---|---|---|---|\n");
        s.push_str(&rows);
    }
    if shown < list.len() {
        let why = if cut_for_size {
            " (list truncated to fit GitHub size limits)"
        } else {
            ""
        };
        let _ = writeln!(
            s,
            "\n_… and {} more{why}; see the full results._",
            thousands((list.len() - shown) as u64)
        );
    }
    s.push('\n');
    s
}

fn pct(v: f64) -> String {
    format!("{v:.1}%")
}

/// `**B** (0.83)`: a grade and the value it was given for.
fn rating(r: &Rating) -> String {
    format!("**{}** ({:.2})", r.grade.as_str(), r.value)
}

fn tail(r: &ScanResult) -> String {
    let mut s = String::new();
    let t = &r.measures.total;
    let _ = writeln!(s, "### Measures\n");
    match &r.new_code {
        Some(nc) => {
            let m = &nc.measures;
            // A baseline comparison has no line diff: no line-based new-code measures.
            let lines = nc.baseline.is_none();
            let _ = writeln!(s, "| Measure | New code | Overall |");
            let _ = writeln!(s, "|---|---:|---:|");
            let _ = writeln!(
                s,
                "| Lines | {} | {} |",
                if lines {
                    thousands(m.new_lines)
                } else {
                    "–".into()
                },
                thousands(t.lines)
            );
            let _ = writeln!(
                s,
                "| Duplication | {} | {} |",
                if lines {
                    pct(m.new_duplication_pct)
                } else {
                    "–".into()
                },
                pct(t.duplicated_lines_density)
            );
            if m.new_coverage_pct.is_some() || r.measures.coverage.is_some() {
                let _ = writeln!(
                    s,
                    "| Coverage | {} | {} |",
                    m.new_coverage_pct.map(pct).unwrap_or_else(|| "–".into()),
                    r.measures
                        .coverage
                        .as_ref()
                        .map(|c| pct(c.coverage_pct))
                        .unwrap_or_else(|| "–".into())
                );
            }
            let _ = writeln!(
                s,
                "| Estimated effort | {} | {} |",
                fmt_hours(m.new_effort_minutes.total),
                fmt_hours(t.effort_minutes.total)
            );
            let _ = writeln!(
                s,
                "| Cognitive complexity | – | {} |",
                thousands(t.cognitive_complexity)
            );
            let _ = writeln!(
                s,
                "| Cyclomatic complexity | – | {} |",
                thousands(t.cyclomatic_complexity)
            );
            if let Some(g) = &r.grades {
                // Too few new lines for a grade: say so rather than leave the cell blank.
                let unrated = if g.new_not_rated.is_some() {
                    "_not rated_"
                } else {
                    "–"
                };
                let new =
                    |x: &Option<Rating>| x.as_ref().map(rating).unwrap_or_else(|| unrated.into());
                let _ = writeln!(
                    s,
                    "| Maintainability grade | {} | {} |",
                    new(&g.new_maintainability),
                    rating(&g.maintainability)
                );
                let _ = writeln!(
                    s,
                    "| Reliability grade | {} | {} |",
                    new(&g.new_reliability),
                    rating(&g.reliability)
                );
            }
        }
        None => {
            let _ = writeln!(s, "| Measure | Overall |");
            let _ = writeln!(s, "|---|---:|");
            let _ = writeln!(s, "| Lines | {} |", thousands(t.lines));
            let _ = writeln!(s, "| Duplication | {} |", pct(t.duplicated_lines_density));
            if let Some(c) = &r.measures.coverage {
                let _ = writeln!(s, "| Coverage | {} |", pct(c.coverage_pct));
            }
            let _ = writeln!(
                s,
                "| Estimated effort | {} |",
                fmt_hours(t.effort_minutes.total)
            );
            let _ = writeln!(
                s,
                "| Cognitive complexity | {} |",
                thousands(t.cognitive_complexity)
            );
            let _ = writeln!(
                s,
                "| Cyclomatic complexity | {} |",
                thousands(t.cyclomatic_complexity)
            );
            if let Some(g) = &r.grades {
                let _ = writeln!(
                    s,
                    "| Maintainability grade | {} |",
                    rating(&g.maintainability)
                );
                let _ = writeln!(s, "| Reliability grade | {} |", rating(&g.reliability));
            }
        }
    }
    if r.grades.is_some() {
        let _ = writeln!(
            s,
            "
_Grades run from A (best) to E: maintainability by the estimated hours of fixes per 1,000 lines of code, reliability by weighted reliability findings per 10,000 lines._"
        );
    }
    if let Some(n) = r.grades.as_ref().and_then(|g| g.new_not_rated.as_ref()) {
        let _ = writeln!(
            s,
            "
_New code not rated: {} (`grades.min_new_lines`)._",
            n.why()
        );
    }
    // Analyzed and cached files count together: the summary must not depend on cache state.
    let skipped = r
        .files
        .iter()
        .filter(|f| f.status == FileStatus::Skipped)
        .count() as u64;
    let analyzed = r.files.len() as u64 - skipped;
    let _ = writeln!(s, "\n<details><summary>Totals</summary>\n");
    let _ = writeln!(s, "| Total | Value |");
    let _ = writeln!(s, "|---|---:|");
    let _ = writeln!(
        s,
        "| Files | {} ({} analyzed, {} skipped) |",
        thousands(analyzed + skipped),
        thousands(analyzed),
        thousands(skipped)
    );
    let in_tests = if r.measures.test.ncloc > 0 {
        format!(" ({} in tests)", thousands(r.measures.test.ncloc))
    } else {
        String::new()
    };
    let _ = writeln!(s, "| Lines of code | {}{in_tests} |", thousands(t.ncloc));
    let _ = writeln!(
        s,
        "| Findings | {} ({} HIGH, {} MEDIUM, {} LOW) |",
        thousands(t.findings),
        thousands(t.findings_high),
        thousands(t.findings_medium),
        thousands(t.findings_low)
    );
    let _ = writeln!(s, "| Suppressed | {} |", thousands(t.suppressed_findings));
    let e = &t.effort_minutes;
    let _ = writeln!(
        s,
        "| Estimated effort | {} (reliability {}, maintainability {}) |",
        fmt_hours(e.total),
        fmt_hours(e.reliability),
        fmt_hours(e.maintainability)
    );
    let _ = writeln!(
        s,
        "| Duplicated blocks | {} |",
        thousands(r.duplications.len() as u64)
    );
    if r.measures.languages.len() > 1 {
        let _ = writeln!(
            s,
            "\n| Language | Files | Lines of code | Findings | Duplication |"
        );
        let _ = writeln!(s, "|---|---:|---:|---:|---:|");
        for (lang, m) in &r.measures.languages {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} |",
                cell(lang),
                thousands(m.files),
                thousands(m.ncloc),
                thousands(m.findings),
                pct(m.duplicated_lines_density)
            );
        }
    }
    let _ = writeln!(s, "\n</details>");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::{assert_snapshot, empty, finding, with_base, without_base};

    #[test]
    fn snapshots() {
        let md = render(&with_base(), &MarkdownOptions::default());
        assert!(md.starts_with("<!-- mote:summary -->\n"));
        assert_snapshot("summary_with_base.md", &md);
        let md = render(&without_base(), &MarkdownOptions::default());
        assert_snapshot("summary_without_base.md", &md);
        let md = render(&empty(), &MarkdownOptions::default());
        assert!(md.starts_with(MARKER));
        assert!(md.contains("quality gate passed"));
    }

    /// `grades.min_new_lines` (100 by default): a change one line short of it is not rated, one
    /// of exactly that size is.
    #[test]
    fn new_code_is_rated_from_the_grades_minimum() {
        let md = render(&crate::testdata::change_of(99), &MarkdownOptions::default());
        assert!(
            md.contains("| Maintainability grade | _not rated_ |"),
            "{md}"
        );
        assert!(
            md.contains(
                "\n_New code not rated: only 99 new lines; grades need at least 100 \
                 (`grades.min_new_lines`)._\n"
            ),
            "{md}"
        );
        let md = render(
            &crate::testdata::change_of(100),
            &MarkdownOptions::default(),
        );
        assert!(!md.contains("not rated"), "{md}");
    }

    #[test]
    fn a_small_change_is_not_graded() {
        let md = render(
            &crate::testdata::small_change(),
            &MarkdownOptions::default(),
        );
        assert!(
            md.contains("| Maintainability grade | _not rated_ | **D** (3.61) |\n"),
            "{md}"
        );
        assert!(
            md.contains("| Reliability grade | _not rated_ | **E** (41.67) |\n"),
            "{md}"
        );
        assert!(
            md.contains(
                "\n_New code not rated: only 11 new lines; grades need at least 100 \
                 (`grades.min_new_lines`)._\n"
            ),
            "{md}"
        );
        // Graded changes have no such note.
        let md = render(&with_base(), &MarkdownOptions::default());
        assert!(!md.contains("not rated"), "{md}");
    }

    #[test]
    fn baseline_comparison() {
        let md = render(
            &crate::testdata::with_baseline(),
            &MarkdownOptions::default(),
        );
        assert!(
            md.contains("Compared with the baseline `mote-baseline.json` (commit `abcdef012345`): findings not in it are new (**3** of its 5 findings still present)."),
            "{md}"
        );
        assert!(md.contains("| Lines | – |"), "{md}");
        assert!(md.contains("| Duplication | – |"), "{md}");
        assert!(md.contains("### Top new findings"), "{md}");
        assert!(!md.contains("merge base"), "{md}");
    }

    #[test]
    fn escapes_cells() {
        let md = render(&with_base(), &MarkdownOptions::default());
        assert!(md.contains("`total` \\| it has no effect"), "{md}");
        assert_eq!(cell("a<b>|c\nd"), "a&lt;b&gt;\\|c d");
        assert_eq!(code("x`y"), "``x`y``");
        assert_eq!(code("a<b>|c"), "`a<b>\\|c`");
        // entities would show literally inside code spans
        assert_eq!(
            cell("Refactor `<anonymous>#f` <now>"),
            "Refactor `<anonymous>#f` &lt;now&gt;"
        );
        assert_eq!(cell("odd ` <x>"), "odd \\` &lt;x&gt;");
    }

    /// Finding messages quote the scanned code: nothing in them may become raw HTML, a link, or
    /// break the table row, whatever the backticks look like.
    #[test]
    fn hostile_messages_stay_inert() {
        // A run of three with no closing run of three is literal: the old count-based pairing
        // treated `<h1>` as code here and let it through.
        assert_eq!(cell("```<h1>`"), "\\`\\`\\`&lt;h1&gt;\\`");
        // Backslash-escaped backticks don't delimit code in GFM; the backslash is text here.
        assert_eq!(cell("\\`<h1>\\`"), "\\\\`<h1>\\`");
        // ... and renders as: escaped backslash, then a code span holding `<h1>\`.
        // A single-backtick span containing a double run gets a triple fence.
        assert_eq!(cell("a `x`` <b>`"), "a ```x`` <b>```");
        assert_eq!(
            cell("<img src=x onerror=alert(1)>"),
            "&lt;img src=x onerror=alert(1)&gt;"
        );
        assert_eq!(
            cell("[x](javascript:alert(1))"),
            "\\[x\\](javascript:alert(1))"
        );
        assert_eq!(cell("a\\|b"), "a\\\\\\|b");
        assert_eq!(cell("line1\r\nline2 | <br>"), "line1  line2 \\| &lt;br&gt;");
        assert_eq!(cell("`a|b`"), "`a\\|b`");
        assert_eq!(cell("`` `x` ``"), "`` `x` ``");
        assert_eq!(cell("&lt;"), "&amp;lt;");
        // Code spans use a fence longer than any run inside, padded when content touches it.
        assert_eq!(code("``"), "``` `` ```");
        assert_eq!(code("a```b"), "````a```b````");
        assert_eq!(code(" x "), "`  x  `");
        assert_eq!(code_span("a|b", false), "`a|b`");
        assert_eq!(code(""), "");
        // Every `<` outside a code span is an entity; inside, the span is well-formed.
        for msg in [
            "```<h1>`",
            "\\`<h1>\\`",
            "``<h1>`x`",
            "`x``<h1>``",
            "<script>`",
            "`a`<h1>`",
            "````<h1>``` `",
        ] {
            let out = cell(msg);
            assert!(outside_code_has_no_tag(&out), "{msg:?} -> {out:?}");
        }
    }

    /// Walk `md` like a CommonMark inline parser (backslash escapes, backtick runs) and check that
    /// no `<` appears outside a code span.
    fn outside_code_has_no_tag(md: &str) -> bool {
        let c: Vec<char> = md.chars().collect();
        let mut i = 0;
        while i < c.len() {
            match c[i] {
                '\\' if i + 1 < c.len() && c[i + 1].is_ascii_punctuation() => i += 2,
                '`' => {
                    let start = i;
                    while i < c.len() && c[i] == '`' {
                        i += 1;
                    }
                    let n = i - start;
                    // find a closing run of exactly n
                    let mut j = i;
                    let mut closed = None;
                    while j < c.len() {
                        if c[j] == '`' {
                            let s = j;
                            while j < c.len() && c[j] == '`' {
                                j += 1;
                            }
                            if j - s == n {
                                closed = Some(j);
                                break;
                            }
                        } else {
                            j += 1;
                        }
                    }
                    if let Some(end) = closed {
                        i = end;
                    }
                }
                '<' => return false,
                _ => i += 1,
            }
        }
        true
    }

    #[test]
    fn top_findings_capped_and_size_limited() {
        let mut r = with_base();
        r.findings = (0..500)
            .map(|i| {
                finding(
                    &format!("id{i:010}"),
                    "java/self-assignment",
                    &format!("src/very/long/path/number/{i:04}/File.java"),
                    (i + 1, 1, i + 1, 5),
                    Severity::High,
                    FindingStatus::New,
                    &"x".repeat(400),
                )
            })
            .collect();
        let md = render(&r, &MarkdownOptions::default());
        assert!(md.len() < 60_000);
        assert!(
            md.contains("Top new findings (50 of 500)"),
            "{}",
            &md[..600]
        );
        assert!(md.contains("… and 450 more"));
        let small = MarkdownOptions {
            max_bytes: 6_000,
            ..Default::default()
        };
        let md = render(&r, &small);
        assert!(md.len() <= 6_000, "{}", md.len());
        assert!(md.contains("truncated to fit GitHub size limits"), "{md}");
        assert!(md.starts_with(MARKER));
        let tiny = MarkdownOptions {
            max_bytes: 500,
            ..Default::default()
        };
        let md = render(&r, &tiny);
        assert!(md.len() <= 500 && md.starts_with(MARKER));
    }

    #[test]
    fn min_severity_applies() {
        let opts = MarkdownOptions {
            filter: Filter {
                min_severity: Severity::High,
                ..Filter::default()
            },
            ..Default::default()
        };
        let md = render(&with_base(), &opts);
        assert!(md.contains("Top new findings (1 of 1)"), "{md}");
    }
}
