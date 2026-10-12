//! Tests pinning the rule-author API (`JsRule`, `Ctx`) with a probe rule.

use crate::prelude::*;
use crate::JsPlugin;
use mote_core::testing::ruleset_for;
use mote_core::{LanguagePlugin, ParamValue, RawFinding, SourceFile};
use std::collections::BTreeMap;

static PROBE: RuleMeta = RuleMeta {
    params: &[
        ParamSpec {
            name: "limit",
            default: ParamDefault::Int(3),
            description: "",
        },
        ParamSpec {
            name: "flag",
            default: ParamDefault::Bool(true),
            description: "",
        },
        ParamSpec {
            name: "names",
            default: ParamDefault::Str("a, b,,c "),
            description: "",
        },
    ],
    ..RuleMeta::new("js/probe", "js", "Probe")
};

struct Probe;

impl JsRule for Probe {
    fn meta(&self) -> &'static RuleMeta {
        &PROBE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::DebuggerStatement, AstType::CallExpression]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::DebuggerStatement(_) => {
                *ctx.state::<u32>() += 1;
                let message = format!(
                    "loop={} finally={} fn={} sym={}",
                    ctx.is_in_loop(node),
                    ctx.is_in_finally(node),
                    ctx.is_in_function(node),
                    ctx.symbol_of(node).unwrap_or("-"),
                );
                ctx.emit(node.span(), message);
            }
            AstKind::CallExpression(call) => {
                let outer = ctx
                    .outer_parent(node)
                    .map(|n| format!("{:?}", n.kind().ty()));
                let console = match helpers::unwrap_expr(&call.callee) {
                    Expression::StaticMemberExpression(m) => ctx.is_global(&m.object, "console"),
                    _ => false,
                };
                ctx.emit(
                    call.span,
                    format!("outer={} console={console}", outer.unwrap_or_default()),
                )
                .secondary(call.callee.span(), "callee")
                .value(1.5);
            }
            _ => {}
        }
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let count = *ctx.state::<u32>();
        let names: Vec<&str> = ctx.param_list("names").collect();
        let message = format!(
            "count={count} limit={} flag={} names={names:?} missing={} rule={} ts={} module={}",
            ctx.param_i64("limit"),
            ctx.param_bool("flag"),
            ctx.param_i64("missing"),
            ctx.rule_id(),
            ctx.is_typescript(),
            ctx.is_module(),
        );
        ctx.emit(Span::new(0, 0), message).symbol("file");
    }
}

fn run(source: &str, overrides: &[(&str, ParamValue)]) -> Vec<RawFinding> {
    let plugin = JsPlugin::with_rules(vec![Box::new(Probe)]);
    let overrides: BTreeMap<String, ParamValue> = overrides
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let rules = ruleset_for(&plugin.rules(), &["js/probe"], &overrides);
    let file = SourceFile {
        path: "src/probe.ts",
        text: source,
        is_test: false,
    };
    plugin.analyze(&file, &rules).findings
}

fn messages(findings: &[RawFinding]) -> Vec<(u32, &str)> {
    findings
        .iter()
        .map(|f| (f.range.start_line, f.message.as_str()))
        .collect()
}

const SOURCE: &str = r"export function outer(console: Console) {
  for (const x of xs) {
    debugger;
  }
  try { work(); } finally { debugger; }
  while (a) { (() => { debugger; })(); }
  console.log(1);
}
debugger;
const v = foo() as any;
a?.b();
globalThis.console.log(2);
console.log(3);
";

#[test]
fn navigation_and_symbols() {
    let found = run(SOURCE, &[]);
    assert_eq!(
        messages(&found),
        vec![
            (1, "count=4 limit=3 flag=true names=[\"a\", \"b\", \"c\"] missing=0 rule=js/probe ts=true module=true"),
            (3, "loop=true finally=false fn=true sym=outer"),
            (5, "outer=ExpressionStatement console=false"),
            (5, "loop=false finally=true fn=true sym=outer"),
            (6, "outer=ExpressionStatement console=false"),
            (6, "loop=false finally=false fn=true sym=outer"),
            (7, "outer=ExpressionStatement console=false"),
            (9, "loop=false finally=false fn=false sym=-"),
            (10, "outer=VariableDeclarator console=false"),
            (11, "outer=ExpressionStatement console=false"),
            (12, "outer=ExpressionStatement console=false"),
            (13, "outer=ExpressionStatement console=true"),
        ]
    );
}

#[test]
fn emitted_extras_and_symbols() {
    let found = run(SOURCE, &[]);
    let file_level = &found[0];
    assert_eq!(file_level.symbol.as_deref(), Some("file"));
    let call = found.iter().find(|f| f.range.start_line == 7).unwrap();
    assert_eq!(call.value, Some(1.5));
    assert_eq!(call.symbol.as_deref(), Some("outer"));
    assert_eq!(call.secondary.len(), 1);
    assert_eq!(call.secondary[0].path, "src/probe.ts");
    assert_eq!(call.secondary[0].message.as_deref(), Some("callee"));
    assert_eq!((call.range.start_col, call.range.end_col), (3, 17));
}

#[test]
fn parameter_overrides_and_fallbacks() {
    let found = run(
        "debugger;\n",
        &[
            ("limit", ParamValue::Int(7)),
            ("flag", ParamValue::Bool(false)),
            ("names", ParamValue::Str("x".into())),
        ],
    );
    assert!(
        found[0]
            .message
            .starts_with("count=1 limit=7 flag=false names=[\"x\"]"),
        "{}",
        found[0].message
    );
    // A mistyped value falls back to the declared default.
    let found = run("debugger;\n", &[("limit", ParamValue::Str("seven".into()))]);
    assert!(found[0].message.contains("limit=3"), "{}", found[0].message);
}

#[test]
fn authoring_guide_template_compiles_and_works() {
    // `template_check.rs` is the guide's copy-paste template, verbatim.
    let guide = include_str!("../RULE_AUTHORING.md").replace("\r\n", "\n");
    let template = include_str!("template_check.rs").replace("\r\n", "\n");
    assert!(
        guide.contains(&template),
        "RULE_AUTHORING.md template and src/template_check.rs differ"
    );

    let plugin = JsPlugin::with_rules(vec![Box::new(crate::template_check::NoSelfCompare)]);
    let rules = ruleset_for(&plugin.rules(), &["js/no-self-compare"], &BTreeMap::new());
    let file = SourceFile {
        path: "src/a.js",
        text: "if (a === a) {}\nif (a === b) {}\n",
        is_test: false,
    };
    let found = plugin.analyze(&file, &rules).findings;
    assert_eq!(
        messages(&found),
        vec![(1, "Compare 'a' with the intended value, not itself")]
    );
}

#[test]
fn each_rule_set_is_resolved_on_its_own() {
    // One plugin, two rule sets used alternately (each resolved once and memoized in the set),
    // and a second plugin with the same rules in another order sharing a set: every analysis
    // must follow its own set.
    let rules = || -> Vec<Box<dyn JsRule>> {
        vec![
            Box::new(Probe),
            Box::new(crate::template_check::NoSelfCompare),
        ]
    };
    let plugin = JsPlugin::with_rules(rules());
    let mut reversed = rules();
    reversed.reverse();
    let other = JsPlugin::with_rules(reversed);
    let set = |ids: &[&str], limit: i64| {
        let params = BTreeMap::from([("limit".to_string(), ParamValue::Int(limit))]);
        ruleset_for(&plugin.rules(), ids, &params)
    };
    let a = set(&["js/probe"], 7);
    let b = set(&["js/probe", "js/no-self-compare"], 2);
    let file = SourceFile {
        path: "src/a.js",
        text: "if (a === a) {}\n",
        is_test: false,
    };
    let summary = |p: &JsPlugin, rules| -> Vec<String> {
        p.analyze(&file, rules)
            .findings
            .iter()
            .map(|f| {
                let words: Vec<&str> = f.message.split(' ').take(2).collect();
                format!("{} {}", f.rule_id, words.join(" "))
            })
            .collect()
    };
    let expect_a = vec!["js/probe count=0 limit=7".to_string()];
    let expect_b = vec![
        "js/probe count=0 limit=2".to_string(),
        "js/no-self-compare Compare 'a'".to_string(),
    ];
    for _ in 0..2 {
        assert_eq!(summary(&plugin, &a), expect_a);
        assert_eq!(summary(&plugin, &b), expect_b);
        assert_eq!(summary(&other, &b), expect_b);
        assert_eq!(summary(&other, &a), expect_a);
    }
}

static AMBIENT_PROBE: RuleMeta = RuleMeta::new("js/ambient-probe", "js", "Ambient probe");

/// Reports every node where `in_ambient_context` disagrees with the walk up.
struct AmbientProbe;

impl JsRule for AmbientProbe {
    fn meta(&self) -> &'static RuleMeta {
        &AMBIENT_PROBE
    }

    fn types(&self) -> &'static [AstType] {
        &[]
    }

    fn run<'a>(&self, _node: &AstNode<'a>, _ctx: &mut Ctx<'a>) {}

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let mut wrong = Vec::new();
        for node in ctx.nodes().iter() {
            let walk = ctx.ancestors(node).any(|n| match n.kind() {
                AstKind::TSModuleDeclaration(m) => m.declare,
                AstKind::TSGlobalDeclaration(_) => true,
                _ => false,
            });
            if ctx.in_ambient_context(node) != walk {
                wrong.push(node.span());
            }
        }
        let checked = ctx.nodes().iter().count();
        for span in wrong {
            ctx.emit(span, "disagrees");
        }
        ctx.emit(Span::new(0, 0), format!("checked {checked}"));
    }
}

/// `in_ambient_context` reads merged spans of the ambient declarations instead of walking up:
/// the same answer for every node, ambient declarations themselves included.
#[test]
fn ambient_context_spans_agree_with_the_walk_up() {
    let plugin = JsPlugin::with_rules(vec![Box::new(AmbientProbe)]);
    let rules = ruleset_for(&plugin.rules(), &["js/ambient-probe"], &BTreeMap::new());
    let source = "namespace N { export const a = 1; }\n\
        declare module 'm' { export function f(x: number): void; namespace Inner { const b: 2; } }\n\
        function g() { return 1 + 2; }\n\
        declare global { interface Window { w: string } declare namespace D { let c: 3 } }\n\
        declare namespace Outer { namespace Mid { declare module 'n' { type T = 4 } } }\n\
        export declare namespace Ex { const d: 5; }\n\
        const after = () => [1, 2];\n";
    let file = SourceFile {
        path: "src/ambient.ts",
        text: source,
        is_test: false,
    };
    let findings = plugin.analyze(&file, &rules).findings;
    let (checked, wrong): (Vec<_>, Vec<_>) = findings
        .iter()
        .partition(|f| f.message.starts_with("checked "));
    assert!(wrong.is_empty(), "{:?}", messages(&findings));
    let n: usize = checked[0].message["checked ".len()..].parse().unwrap();
    assert!(n > 50, "{n}");
}
