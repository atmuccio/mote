//! A tiny line-oriented fake language plugin for engine tests.
//!
//! Shared by the crate's unit tests (via `#[path]`) and integration tests. It depends only on
//! `mote-core`.
//!
//! Syntax understood (per line):
//! * `LOUD` → `<id>/loud` (MEDIUM); `HIGHBUG` → `<id>/high-bug` (HIGH); `INCUB` →
//!   `<id>/incubating` (incubating rule); `MAGIC` → `<id>/magic`.
//! * `fn NAME cog=N cyc=N params=N nest=N` starts a function that ends at a line `end`.
//! * `SYNTAX_ERROR` counts a syntax error; `PANIC_NOW` makes the plugin panic; `DEEP_STACK`
//!   recurses deeply (~24 MB of stack), like a parser on deeply nested input.
//! * `@Suppress(a, b)` → annotation suppression for this and the next line;
//!   `@SuppressCompat` → compat blanket suppression (source `pmd`) for this and the next line.
//! * `SHOW_RELEASE` → `<id>/magic` with the message `release <java_release param or none>`.
//! * `PKG dir` → `<id>/magic` unless the file's directory is `dir` (a path-dependent rule).
//! * `MANY_LOUD` → `<id>/loud` at every `LOUD` of the line (and nothing else on it).
#![allow(dead_code)]

use mote_core::{
    FileAnalysis, FunctionMetric, LanguagePlugin, LexerSpec, RawFinding, RuleMeta, RuleSet,
    RuleStatus, ScopedSuppression, Severity, SourceFile, TextRange,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub struct FakePlugin {
    pub id: &'static str,
    pub exts: &'static [&'static str],
    pub metas: Vec<&'static RuleMeta>,
    pub version: u32,
    pub calls: Arc<AtomicUsize>,
}

/// Recurse `depth` levels with a 4 KiB frame each.
#[inline(never)]
fn deep_recursion(depth: u32) -> u64 {
    let frame = std::hint::black_box([depth as u8; 4096]);
    if depth == 0 {
        return u64::from(frame[0]);
    }
    deep_recursion(depth - 1) + u64::from(std::hint::black_box(frame)[7])
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

impl FakePlugin {
    pub fn new(id: &str) -> FakePlugin {
        let id: &'static str = leak(id.to_string());
        let exts: &'static [&'static str] = Box::leak(vec![id].into_boxed_slice());
        let mk = |name: &str, f: &dyn Fn(RuleMeta) -> RuleMeta| -> &'static RuleMeta {
            let rid = leak(format!("{id}/{name}"));
            let base = RuleMeta {
                description: "Fake rule.",
                ..RuleMeta::new(rid, id, "Fake rule")
            };
            Box::leak(Box::new(f(base)))
        };
        let metas = vec![
            mk("loud", &|m| RuleMeta {
                severity: Severity::Medium,
                ..m
            }),
            mk("high-bug", &|m| RuleMeta {
                severity: Severity::High,
                ..m
            }),
            mk("magic", &|m| RuleMeta {
                severity: Severity::Low,
                ..m
            }),
            mk("empty-block", &|m| RuleMeta {
                severity: Severity::Low,
                ..m
            }),
            mk("incubating", &|m| RuleMeta {
                severity: Severity::High,
                status: RuleStatus::Incubating,
                ..m
            }),
            mk("off-by-default", &|m| RuleMeta {
                default_enabled: false,
                ..m
            }),
            mk("deprecated", &|m| RuleMeta {
                status: RuleStatus::Deprecated,
                tags: &["conventions"],
                ..m
            }),
            mk("style", &|m| RuleMeta {
                severity: Severity::Low,
                default_enabled: false,
                tags: &["conventions"],
                ..m
            }),
        ];
        FakePlugin {
            id,
            exts,
            metas,
            version: 1,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn boxed(id: &str) -> Box<dyn LanguagePlugin> {
        Box::new(FakePlugin::new(id))
    }
}

impl LanguagePlugin for FakePlugin {
    fn id(&self) -> &'static str {
        self.id
    }
    fn display_name(&self) -> &'static str {
        "Fake"
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.exts
    }
    fn version(&self) -> u32 {
        self.version
    }
    fn rules(&self) -> Vec<&'static RuleMeta> {
        self.metas.clone()
    }
    fn lexer(&self) -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: false,
            template_literals: false,
            cpd_skip_line_prefixes: &["import "],
        }
    }
    fn default_test_globs(&self) -> &'static [&'static str] {
        &["**/src/test/**"]
    }
    fn analyze(&self, file: &SourceFile<'_>, rules: &RuleSet) -> FileAnalysis {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut out = FileAnalysis::default();
        let mut open: Option<FunctionMetric> = None;
        for (i, line) in file.text.lines().enumerate() {
            let ln = i as u32 + 1;
            if line.contains("PANIC_NOW") {
                panic!("fake plugin asked to panic");
            }
            if line.contains("SYNTAX_ERROR") {
                out.syntax_errors += 1;
            }
            if line.contains("DEEP_STACK") {
                // Like a recursive-descent parser on deeply nested input: needs ~24 MB of stack.
                std::hint::black_box(deep_recursion(6000));
            }
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("fn ") {
                let mut f = FunctionMetric {
                    start_line: ln,
                    end_line: ln,
                    ..Default::default()
                };
                let mut parts = rest.split_whitespace();
                f.name = parts.next().unwrap_or("<anon>").to_string();
                let col = line.find(&f.name).unwrap_or(0) as u32 + 1;
                f.range = TextRange {
                    start_line: ln,
                    start_col: col,
                    end_line: ln,
                    end_col: col + f.name.len() as u32,
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
            if let Some(pos) = t.find("@Suppress(") {
                let inner = &t[pos + "@Suppress(".len()..];
                let inner = inner.split(')').next().unwrap_or("");
                out.suppressions.push(ScopedSuppression {
                    start_line: ln,
                    end_line: ln + 1,
                    rules: Some(
                        inner
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect(),
                    ),
                    kind: "annotation".into(),
                    justification: None,
                });
            }
            if t.contains("@SuppressCompat") {
                out.suppressions.push(ScopedSuppression {
                    start_line: ln,
                    end_line: ln + 1,
                    rules: None,
                    kind: "compat:pmd".into(),
                    justification: None,
                });
            }
            let magic = format!("{}/magic", self.id);
            // `PKG dir`: like a package declaration, a finding unless the file is in `dir`.
            if let Some(pkg) = t.strip_prefix("PKG ") {
                let dir = file.path.rsplit_once('/').map_or("", |(d, _)| d);
                if dir != pkg.trim() && rules.is_enabled(&magic) {
                    out.findings.push(RawFinding {
                        rule_id: magic.clone(),
                        message: format!("package {} is not the directory {dir}", pkg.trim()),
                        range: TextRange::line(ln),
                        symbol: None,
                        value: None,
                        secondary: Vec::new(),
                    });
                }
            }
            if line.contains("SHOW_RELEASE") && rules.is_enabled(&magic) {
                let release = rules
                    .param(&magic, "java_release")
                    .and_then(|v| v.as_i64())
                    .map_or("none".to_string(), |r| r.to_string());
                out.findings.push(RawFinding {
                    rule_id: magic.clone(),
                    message: format!("release {release}"),
                    range: TextRange {
                        start_line: ln,
                        start_col: 1,
                        end_line: ln,
                        end_col: 2,
                    },
                    symbol: None,
                    value: None,
                    secondary: Vec::new(),
                });
            }
            // `MANY_LOUD`: `<id>/loud` at every `LOUD` of the line (many findings on one line).
            if line.contains("MANY_LOUD") && rules.is_enabled(&format!("{}/loud", self.id)) {
                for (pos, _) in line.match_indices("LOUD") {
                    let col = pos as u32 + 1;
                    out.findings.push(RawFinding {
                        rule_id: format!("{}/loud", self.id),
                        message: "Avoid LOUD here".into(),
                        range: TextRange {
                            start_line: ln,
                            start_col: col,
                            end_line: ln,
                            end_col: col + 4,
                        },
                        symbol: None,
                        value: None,
                        secondary: Vec::new(),
                    });
                }
                continue;
            }
            for (marker, rule) in [
                ("LOUD", "loud"),
                ("HIGHBUG", "high-bug"),
                ("INCUB", "incubating"),
                ("MAGIC", "magic"),
            ] {
                if let Some(pos) = line.find(marker) {
                    let id = format!("{}/{rule}", self.id);
                    if rules.is_enabled(&id) {
                        let col = pos as u32 + 1;
                        out.findings.push(RawFinding {
                            rule_id: id,
                            message: format!("Avoid {marker} here"),
                            range: TextRange {
                                start_line: ln,
                                start_col: col,
                                end_line: ln,
                                end_col: col + marker.len() as u32,
                            },
                            symbol: open.as_ref().map(|f| f.name.clone()),
                            value: None,
                            secondary: Vec::new(),
                        });
                    }
                }
            }
        }
        out.statements = file.text.matches(';').count() as u32;
        out.classes = file.text.matches("class ").count() as u32;
        out
    }
}
