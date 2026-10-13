//! Mote JavaScript/TypeScript plugin, built on [Oxc](https://oxc.rs).
//!
//! Per file: a linear nesting pre-scan (a file nested deeply enough to risk overflowing the stack
//! in the recursive parser or semantic builder is reported as one syntax error) → parse (Oxc,
//! error-recovering) → semantic model (scopes, symbols, references) → one linear pass computing
//! function metrics and file facts → one linear pass dispatching nodes to the rules subscribed to
//! their [`AstType`](oxc_ast::AstType) → `run_once` for file-level rules → comments and
//! ESLint-compatible suppressions.
//!
//! Writing rules: see `RULE_AUTHORING.md` in this crate, the [`JsRule`] trait, [`Ctx`], and the
//! [`prelude`].

#[cfg(test)]
mod api_tests;
mod comments;
mod ctx;
mod depth;
mod facts;
pub mod helpers;
pub mod prelude;
mod rule;
pub mod rules;
#[cfg(test)]
mod template_check;
pub mod timing;

pub use ctx::{AsNodeId, Ctx, Emitted};
pub use rule::JsRule;

use ctx::FileInfo;
use mote_core::{
    FileAnalysis, LanguagePlugin, LexerSpec, LineIndex, RawFinding, RuleMeta, RuleSet, SourceFile,
};
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use rule::{Registry, Resolved};
use std::cell::Cell;
use timing::{FileTimes, Phase};

/// Number of node types (`AstType as usize` is below this).
const TYPES: usize = oxc_ast::ast_kind::AST_TYPE_MAX as usize + 1;

/// Arena memory kept per thread between files; larger arenas are released after use.
const ARENA_RETAIN_BYTES: usize = 64 << 20;

/// Stack size the plugin assumes its analysis threads have unless the host declares one with
/// [`JsPlugin::with_analysis_stack`]: 2 MiB, Rust's default for spawned threads.
pub const DEFAULT_ANALYSIS_STACK: usize = 2 << 20;

/// The JavaScript/TypeScript language plugin (`js`).
pub struct JsPlugin {
    registry: Registry,
    /// Nesting estimate above which a file is reported instead of analysed (see `depth`).
    depth_limit: u64,
}

impl Default for JsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl JsPlugin {
    /// Plugin with every built-in rule.
    pub fn new() -> Self {
        Self::with_rules(rules::all())
    }

    /// Plugin with a custom rule list (tests, experiments).
    pub fn with_rules(rules: Vec<Box<dyn JsRule>>) -> Self {
        JsPlugin {
            registry: Registry::new(rules),
            depth_limit: depth::limit_for_stack(DEFAULT_ANALYSIS_STACK),
        }
    }

    /// Declare the stack size of the threads `analyze` runs on (default
    /// [`DEFAULT_ANALYSIS_STACK`]). Oxc parses and builds its semantic model recursively; a file
    /// nested deeply enough to overflow that stack (which would abort the process) is reported as
    /// one syntax error instead of analysed. The larger the stack, the deeper the nesting that is
    /// analysed, and the fewer files need the nesting pre-scan at all.
    ///
    /// The host must guarantee the stack: every thread that calls `analyze` has at least `bytes`.
    pub fn with_analysis_stack(mut self, bytes: usize) -> Self {
        self.depth_limit = depth::limit_for_stack(bytes);
        self
    }
}

impl LanguagePlugin for JsPlugin {
    fn id(&self) -> &'static str {
        "js"
    }

    fn display_name(&self) -> &'static str {
        "JavaScript/TypeScript"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"]
    }

    fn version(&self) -> u32 {
        // 2: deeply nested files are reported as a syntax error instead of analysed; rule fixes.
        // 3: promoted to stable after precision review: valid-expect, unread-collection,
        //    self-comparing-assertion, identical-operands, no-unsafe-optional-chaining,
        //    floating-promise (still off by default).
        // 4: 69 more rules (136 -> 205; batches 6-11); rules widened and narrowed after precision
        //    review; incubating rules promoted to stable; broader default test globs; ESLint
        //    directives are compatibility suppressions of the `eslint` source (kind
        //    `compat:eslint`); the getter-return change; `no-explicit-any` off by default;
        //    test-suite callbacks (`describe(…, () => …)`) are containers, not functions, so
        //    function-level rules measure each test instead of the whole suite. (The engine cache
        //    version invalidates caches.)
        // 5: findings of fixable rules carry a fix (`mote fix`).
        5
    }

    fn rules(&self) -> Vec<&'static RuleMeta> {
        self.registry.metas()
    }

    fn lexer(&self) -> LexerSpec {
        LexerSpec {
            line_comment: "//",
            block_comment: ("/*", "*/"),
            text_blocks: false,
            template_literals: true,
            cpd_skip_line_prefixes: &["import "],
        }
    }

    fn default_test_globs(&self) -> &'static [&'static str] {
        &[
            "**/*.spec.*",
            "**/*.test.*",
            "**/__tests__/**",
            "**/test/**",
            "**/tests/**",
            "**/e2e/**",
            "**/cypress/**",
            "**/*.cy.*",
            "**/playwright*/**",
        ]
    }

    fn analyze(&self, file: &SourceFile<'_>, rules: &RuleSet) -> FileAnalysis {
        let Some(source_type) = source_type_for(file.path) else {
            // Type declaration files (`.d.ts`) carry no executable code.
            return FileAnalysis::default();
        };
        if depth::too_deep(file.text, source_type.is_jsx(), self.depth_limit) {
            // Parsing or building the semantic model would recurse deep enough to risk
            // overflowing the stack (which aborts the process): report the file as unparseable.
            return FileAnalysis {
                syntax_errors: 1,
                ..FileAnalysis::default()
            };
        }
        with_allocator(|allocator| {
            analyze_file(&self.registry, allocator, file, rules, source_type)
        })
    }
}

/// Source type for a path; `None` for declaration files. Plain JavaScript always allows JSX.
fn source_type_for(path: &str) -> Option<SourceType> {
    match SourceType::from_path(path) {
        Ok(st) if st.is_typescript_definition() => None,
        Ok(st) if st.is_javascript() => Some(st.with_jsx(true)),
        Ok(st) => Some(st),
        // Unknown extension: the most permissive grammar.
        Err(_) => Some(SourceType::tsx()),
    }
}

thread_local! {
    /// Parser arena reused across files on this thread (taken out while in use).
    static ARENA: Cell<Option<Allocator>> = const { Cell::new(None) };
}

/// Run `f` with this thread's arena (reset), or a fresh one. The arena is put back afterwards
/// unless it grew beyond [`ARENA_RETAIN_BYTES`] or `f` panicked.
fn with_allocator<R>(f: impl FnOnce(&Allocator) -> R) -> R {
    let mut arena = ARENA
        .try_with(Cell::take)
        .ok()
        .flatten()
        .unwrap_or_default();
    arena.reset();
    let result = f(&arena);
    if arena.capacity() <= ARENA_RETAIN_BYTES {
        let _ = ARENA.try_with(|cell| cell.set(Some(arena)));
    }
    result
}

fn analyze_file(
    registry: &Registry,
    allocator: &Allocator,
    file: &SourceFile<'_>,
    rules: &RuleSet,
    source_type: SourceType,
) -> FileAnalysis {
    // Profiling (`MOTE_RULE_TIMINGS`): phase laps, merged into the process totals at the end.
    let mut prof = timing::enabled().then(|| FileTimes::new(registry.rules.len()));

    let options = ParseOptions {
        allow_return_outside_function: true,
        preserve_parens: false,
        ..ParseOptions::default()
    };
    let parsed = Parser::new(allocator, file.text, source_type)
        .with_options(options)
        .parse();
    let lines = LineIndex::new(file.text);

    let mut out = FileAnalysis {
        syntax_errors: parsed.errors.len() as u32,
        comments: comments::convert(&parsed.program.comments, file.text, &lines),
        suppressions: comments::suppressions(&parsed.program.comments, file.text, &lines),
        ..FileAnalysis::default()
    };
    lap(&mut prof, Phase::Parse);
    if parsed.panicked {
        // Unrecoverable syntax error: the program is empty; comments are still useful.
        if let Some(p) = prof {
            p.merge(&registry.ids());
        }
        return out;
    }

    let program = parsed.program;
    let semantic = SemanticBuilder::new().build(&program).semantic;
    lap(&mut prof, Phase::Semantic);
    let mut facts = facts::compute(&semantic, file.text, &lines);
    lap(&mut prof, Phase::Facts);
    let info = FileInfo {
        path: file.path,
        source: file.text,
        is_test: file.is_test,
        source_type: *semantic.source_type(),
        lines,
    };

    if !rules.is_empty() {
        let resolved = Resolved::of(registry, rules);
        out.findings = run_rules(registry, &semantic, &info, &facts, &resolved, &mut prof);
    }
    out.functions = std::mem::take(&mut facts.functions);
    out.classes = facts.classes;
    out.statements = facts.statements;
    if let Some(p) = prof {
        p.merge(&registry.ids());
    }
    out
}

/// Close the current profiling phase (no-op unless profiling).
#[inline]
fn lap(prof: &mut Option<FileTimes>, phase: Phase) {
    if let Some(p) = prof {
        p.lap(phase);
    }
}

fn run_rules<'a>(
    registry: &Registry,
    semantic: &'a oxc_semantic::Semantic<'a>,
    info: &'a FileInfo<'a>,
    facts: &'a facts::Facts,
    resolved: &'a Resolved,
    prof: &mut Option<FileTimes>,
) -> Vec<RawFinding> {
    let mut ctx = Ctx::new(semantic, info, facts, &resolved.index, &resolved.memos);

    // The enabled rules (resolved once per rule set) whose `applies_to` gate admits this file.
    let mut enabled = vec![false; registry.rules.len()];
    let mut any_enabled = false;
    let mut any_subscribed = false;
    for &index in &resolved.enabled {
        let i = usize::from(index);
        let (Some(rule), Some(&meta)) = (registry.rules.get(i), registry.metas.get(i)) else {
            continue;
        };
        ctx.select(i, meta);
        let applies = match prof {
            None => rule.applies_to(&ctx),
            Some(p) => {
                let start = timing::stamp();
                let applies = rule.applies_to(&ctx);
                p.add_gate(i, start);
                applies
            }
        };
        if applies {
            enabled[i] = true;
            any_enabled = true;
            any_subscribed |= registry.subscribed[i];
        }
    }
    lap(prof, Phase::Setup);
    if !any_enabled {
        return Vec::new();
    }

    if any_subscribed {
        // This file's dispatch table: the admitted subscribers of each node type present in the
        // file, in registry order; `flat[starts[t]..starts[t + 1]]` for type `t`.
        let mut starts = [0u32; TYPES + 1];
        let mut flat: Vec<u16> = Vec::with_capacity(resolved.flat.len());
        for (ty, start) in starts.iter_mut().enumerate().take(TYPES) {
            *start = flat.len() as u32;
            let subscribers = resolved.subscribers(ty);
            if !subscribers.is_empty() && facts.contains_index(ty) {
                flat.extend(
                    subscribers
                        .iter()
                        .copied()
                        .filter(|&i| enabled[usize::from(i)]),
                );
            }
        }
        starts[TYPES] = flat.len() as u32;
        // Rules keyed on callee names (`JsRule::callee_names`), when any is admitted.
        let keyed = resolved.keyed.iter().any(|&i| enabled[usize::from(i)]);
        let mut dispatch = |index: u16, node: &'a oxc_semantic::AstNode<'a>, ctx: &mut Ctx<'a>| {
            let index = usize::from(index);
            let rule = &registry.rules[index];
            ctx.select(index, registry.metas[index]);
            match prof {
                None => rule.run(node, ctx),
                Some(p) => {
                    let start = timing::stamp();
                    rule.run(node, ctx);
                    p.add_run(index, start);
                }
            }
        };
        for node in semantic.nodes().iter() {
            let kind = node.kind();
            let ty = kind.ty() as usize;
            let (Some(&start), Some(&end)) = (starts.get(ty), starts.get(ty + 1)) else {
                continue;
            };
            for &index in &flat[start as usize..end as usize] {
                dispatch(index, node, &mut ctx);
            }
            if keyed {
                let (slot, callee) = match kind {
                    AstKind::CallExpression(call) => (0, &call.callee),
                    AstKind::NewExpression(new) => (1, &new.callee),
                    _ => continue,
                };
                let Some(listed) = helpers::callee_name(callee)
                    .filter(|name| resolved.callee_filter[slot].may_contain(name))
                    .and_then(|name| resolved.by_callee[slot].get(name))
                else {
                    continue;
                };
                for &index in listed.iter() {
                    if enabled[usize::from(index)] {
                        dispatch(index, node, &mut ctx);
                    }
                }
            }
        }
    }
    lap(prof, Phase::Walk);
    for &index in &resolved.enabled {
        let i = usize::from(index);
        let (Some(rule), Some(&meta)) = (registry.rules.get(i), registry.metas.get(i)) else {
            continue;
        };
        if enabled[i] {
            ctx.select(i, meta);
            match prof {
                None => rule.run_once(&mut ctx),
                Some(p) => {
                    let start = timing::stamp();
                    rule.run_once(&mut ctx);
                    p.add_run_once(i, start);
                }
            }
        }
    }
    lap(prof, Phase::RunOnce);

    let mut findings = std::mem::take(&mut ctx.findings);
    findings.sort_by(|a, b| {
        (
            a.range.start_line,
            a.range.start_col,
            &a.rule_id,
            &a.message,
        )
            .cmp(&(
                b.range.start_line,
                b.range.start_col,
                &b.rule_id,
                &b.message,
            ))
    });
    lap(prof, Phase::Collect);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use mote_core::testing::ruleset_all;

    fn analyze(path: &str, text: &str) -> FileAnalysis {
        let plugin = JsPlugin::new();
        let rules = ruleset_all(&plugin);
        plugin.analyze(
            &SourceFile {
                path,
                text,
                is_test: false,
            },
            &rules,
        )
    }

    #[test]
    fn declaration_files_are_empty() {
        let a = analyze(
            "types/index.d.ts",
            "declare const x: any; // eslint-disable-line\n",
        );
        assert!(a.findings.is_empty() && a.comments.is_empty() && a.functions.is_empty());
    }

    #[test]
    fn fatal_syntax_errors_keep_comments() {
        let a = analyze("src/broken.ts", "// note\nfunction ( {{{\n");
        assert!(a.syntax_errors > 0);
        assert_eq!(a.comments.len(), 1);
        assert!(a.findings.is_empty());
    }

    #[test]
    fn jsx_in_plain_js() {
        let a = analyze("src/app.js", "export const App = () => <div>{1}</div>;\n");
        assert_eq!(a.syntax_errors, 0);
        assert_eq!(a.functions.len(), 1);
        assert_eq!(a.functions[0].name, "App");
    }

    #[test]
    fn findings_carry_symbols_and_are_sorted() {
        let a = analyze(
            "src/a.ts",
            "export class Shop {\n  total(a: any) {\n    debugger;\n    var x = a == 1;\n    return x;\n  }\n}\n",
        );
        let got: Vec<(u32, &str, Option<&str>)> = a
            .findings
            .iter()
            .map(|f| (f.range.start_line, f.rule_id.as_str(), f.symbol.as_deref()))
            .collect();
        assert_eq!(
            got,
            vec![
                (2, "js/no-explicit-any", Some("Shop#total")),
                (3, "js/no-debugger", Some("Shop#total")),
                (4, "js/no-var", Some("Shop#total")),
                (4, "js/immediately-returned-variable", Some("Shop#total")),
                (4, "js/eqeqeq", Some("Shop#total")),
            ]
        );
    }

    #[test]
    fn metadata_is_consistent() {
        let plugin = JsPlugin::new();
        let mut seen = std::collections::BTreeSet::new();
        for meta in plugin.rules() {
            assert!(seen.insert(meta.id), "duplicate rule id {}", meta.id);
            assert!(
                meta.id.starts_with("js/"),
                "{}: id must start with js/",
                meta.id
            );
            assert_eq!(meta.language, "js", "{}: language must be js", meta.id);
            let name = &meta.id[3..];
            assert!(
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "{}: id must be kebab-case",
                meta.id
            );
            assert!(
                meta.description.len() > 80,
                "{}: description too short",
                meta.id
            );
            assert!(!meta.name.is_empty(), "{}: missing name", meta.id);
        }
    }
}
