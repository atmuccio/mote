//! A tiny line-oriented language (`.tl` files) for testing the CLI and MCP server without the
//! real plugins. Only compiled with the `test-plugin` feature.
//!
//! Per line: `HIGHBUG` → `tl/high-bug` (HIGH), `LOUD` → `tl/loud` (MEDIUM), `MAGIC` → `tl/magic`
//! (LOW). `fn NAME cog=N params=N` starts a function that ends at a line `end`. `//` starts a
//! comment (so engine rules such as `tl/todo-comment` and `mote-ignore` suppressions work).
//! `PANIC_NOW` makes the plugin panic (the engine turns that into `mote/internal-error`).
//! The plugin also panics when called on the process main thread or the CLI's command thread
//! (analysis must run on the engine's worker pool).

use mote_core::{
    Comment, FileAnalysis, FunctionMetric, LanguagePlugin, LexerSpec, Quality, RawFinding,
    RuleKind, RuleMeta, RuleSet, Severity, SourceFile, TextRange,
};

pub struct TestPlugin;

static HIGH_BUG: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["test"],
    description:
        "Flags the `HIGHBUG` marker.\n\nIt stands for a real bug in tests.\n\n```\nHIGHBUG\n```",
    ..RuleMeta::new("tl/high-bug", "tl", "High bug marker")
};
static LOUD: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    description: "Flags the `LOUD` marker.",
    ..RuleMeta::new("tl/loud", "tl", "Loud marker")
};
static MAGIC: RuleMeta = RuleMeta {
    severity: Severity::Low,
    description: "Flags the `MAGIC` marker.",
    ..RuleMeta::new("tl/magic", "tl", "Magic marker")
};

const MARKERS: [(&str, &str); 3] = [
    ("HIGHBUG", "tl/high-bug"),
    ("LOUD", "tl/loud"),
    ("MAGIC", "tl/magic"),
];

impl LanguagePlugin for TestPlugin {
    fn id(&self) -> &'static str {
        "tl"
    }
    fn display_name(&self) -> &'static str {
        "Test language"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["tl"]
    }
    fn version(&self) -> u32 {
        1
    }
    fn rules(&self) -> Vec<&'static RuleMeta> {
        vec![&HIGH_BUG, &LOUD, &MAGIC]
    }
    fn lexer(&self) -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: false,
            template_literals: false,
            cpd_skip_line_prefixes: &[],
        }
    }
    fn default_test_globs(&self) -> &'static [&'static str] {
        &["**/test/**"]
    }
    fn analyze(&self, file: &SourceFile<'_>, rules: &RuleSet) -> FileAnalysis {
        // Analysis belongs on the engine's worker pool: never on the process main thread (1 MB
        // of stack on Windows) nor on the CLI's command thread. A panic here becomes a
        // `mote/internal-error` finding, which the CLI tests would notice.
        let here = std::thread::current();
        if let Some(name @ ("main" | crate::COMMAND_THREAD)) = here.name() {
            panic!("analysis ran on the `{name}` thread instead of the engine's worker pool");
        }
        let mut out = FileAnalysis::default();
        let mut open: Option<FunctionMetric> = None;
        for (i, line) in file.text.lines().enumerate() {
            let ln = u32::try_from(i + 1).unwrap_or(u32::MAX);
            let (code, comment) = match line.find("//") {
                Some(pos) => (&line[..pos], Some(pos)),
                None => (line, None),
            };
            if let Some(pos) = comment {
                out.comments.push(Comment {
                    start_line: ln,
                    end_line: ln,
                    start_col: col(line, pos),
                    text: line[pos..].to_string(),
                    trailing: !code.trim().is_empty(),
                });
            }
            let t = code.trim();
            if t == "PANIC_NOW" {
                panic!("test plugin asked to panic");
            }
            if let Some(rest) = t.strip_prefix("fn ") {
                let mut parts = rest.split_whitespace();
                let name = parts.next().unwrap_or("<anon>").to_string();
                let start = code.find(&name).unwrap_or(0);
                let mut f = FunctionMetric {
                    range: TextRange {
                        start_line: ln,
                        start_col: col(line, start),
                        end_line: ln,
                        end_col: col(line, start + name.len()),
                    },
                    name,
                    start_line: ln,
                    end_line: ln,
                    ..Default::default()
                };
                for p in parts {
                    if let Some((k, v)) = p.split_once('=') {
                        let v: u32 = v.parse().unwrap_or(0);
                        match k {
                            "cog" => f.cognitive = v,
                            "cyc" => f.cyclomatic = v,
                            "params" => f.params = v,
                            "nest" => f.max_nesting = v,
                            _ => {}
                        }
                    }
                }
                open = Some(f);
            } else if t == "end" {
                if let Some(mut f) = open.take() {
                    f.end_line = ln;
                    out.functions.push(f);
                }
            }
            for (marker, rule) in MARKERS {
                if let Some(pos) = code.find(marker) {
                    if rules.is_enabled(rule) {
                        out.findings.push(RawFinding {
                            rule_id: rule.to_string(),
                            message: format!("Remove the {marker} marker"),
                            range: TextRange {
                                start_line: ln,
                                start_col: col(line, pos),
                                end_line: ln,
                                end_col: col(line, pos + marker.len()),
                            },
                            symbol: open.as_ref().map(|f| f.name.clone()),
                            value: None,
                            secondary: Vec::new(),
                            fix: None,
                        });
                    }
                }
            }
            out.statements += u32::from(!t.is_empty());
        }
        out
    }
}

/// 1-based column (in chars) of byte offset `pos`.
fn col(line: &str, pos: usize) -> u32 {
    let pos = pos.min(line.len());
    let prefix = line.get(..pos).unwrap_or(line);
    u32::try_from(prefix.chars().count() + 1).unwrap_or(u32::MAX)
}
