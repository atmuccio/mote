//! `js/no-redeclare` — a symbol-table rule: walks the redeclared symbols once in `run_once`.

use crate::prelude::*;

pub struct NoRedeclare;

pub static NO_REDECLARE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["suspicious", "pitfall"],
    remediation_minutes: 5,
    description: r#"Flags a function declared twice in the same scope, and a function and a `var` with a
value sharing one name.

**Why:** function and `var` declarations may legally repeat, but they all bind the *same*
variable. A second `function save() {}` silently replaces the first one for every caller, even
those written above it (declarations are hoisted), and `var save = …` in the same scope as
`function save` overwrites the function once that line runs. Such repeats are usually a merge or
copy-paste accident.

**Fix:** rename one of the declarations, or delete the stale one.

```js
function format(value) { return value.trim(); }
function format(value) { return value.toUpperCase(); } // flagged: replaces the first

var render = createRenderer();
function render() {}                                   // flagged: overwritten by the var

function format(value) { return value.trim(); }        // compliant
function formatUpper(value) { return value.toUpperCase(); }
```

Not flagged: TypeScript overloads and declaration merging (interfaces, namespaces, enums, classes),
`declare` statements, parameters redeclared with `var`, `var name;` without a value (it changes
nothing), and `var` declared twice — that only reuses one variable, which is untidy rather than
wrong.
"#,
    ..RuleMeta::new(
        "js/no-redeclare",
        "js",
        "Functions should not be redeclared",
    )
};

impl JsRule for NoRedeclare {
    fn meta(&self) -> &'static RuleMeta {
        &NO_REDECLARE
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let mut reports: Vec<(Span, Span, String)> = Vec::new();
        for symbol in scoping.symbol_ids() {
            let redeclarations = scoping.symbol_redeclarations(symbol);
            if redeclarations.len() < 2 || !ctx.symbol_flags(symbol).contains(SymbolFlags::Function)
            {
                continue;
            }
            let mut decls: Vec<Decl> = Vec::with_capacity(redeclarations.len());
            for redeclaration in redeclarations {
                match classify(ctx, redeclaration.declaration) {
                    Some(kind) => decls.push(Decl {
                        kind,
                        span: redeclaration.span,
                    }),
                    None => {
                        decls.clear();
                        break;
                    }
                }
            }
            if decls.is_empty() || ctx.in_ambient_context(ctx.symbol_declaration(symbol)) {
                continue;
            }
            let name = scoping.symbol_name(symbol);
            for (i, later) in decls.iter().enumerate().skip(1) {
                let Some(earlier) = decls[..i].iter().find(|earlier| conflicts(earlier, later))
                else {
                    continue;
                };
                let message = match (earlier.kind, later.kind) {
                    (Kind::Function, Kind::Function) => format!(
                        "Rename or remove this function: 'function {name}' is already declared in this scope and is replaced"
                    ),
                    (_, Kind::Function) => format!(
                        "Rename this function: 'var {name}' in the same scope overwrites it"
                    ),
                    _ => format!(
                        "Rename this variable: assigning 'var {name}' overwrites the function of the same name"
                    ),
                };
                reports.push((later.span, earlier.span, message));
            }
        }
        for (span, first, message) in reports {
            ctx.emit(span, message)
                .secondary(first, "First declared here");
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Function,
    /// `var x = value`
    VarWithValue,
    /// `var x;`
    VarWithoutValue,
}

#[derive(Clone, Copy)]
struct Decl {
    kind: Kind,
    span: Span,
}

/// A plain `var` declarator or a function declaration with a body; `None` for anything else
/// (parameters, `let`/`const`, classes, TypeScript merges, overloads, `declare`).
fn classify(ctx: &Ctx<'_>, declaration: NodeId) -> Option<Kind> {
    let node = ctx.node(declaration);
    match node.kind() {
        AstKind::Function(f) => {
            (f.is_declaration() && f.body.is_some() && !f.declare).then_some(Kind::Function)
        }
        AstKind::VariableDeclarator(d) => {
            if d.kind != VariableDeclarationKind::Var {
                return None;
            }
            if let Some(AstKind::VariableDeclaration(v)) = ctx.parent_kind(node) {
                if v.declare {
                    return None;
                }
            }
            Some(if d.init.is_some() {
                Kind::VarWithValue
            } else {
                Kind::VarWithoutValue
            })
        }
        _ => None,
    }
}

/// Only pairs involving a function: `var x; var x = 1;` merely reuses one variable.
fn conflicts(earlier: &Decl, later: &Decl) -> bool {
    matches!(
        (earlier.kind, later.kind),
        (Kind::Function, Kind::Function)
            | (Kind::Function, Kind::VarWithValue)
            | (Kind::VarWithValue, Kind::Function)
    )
}
