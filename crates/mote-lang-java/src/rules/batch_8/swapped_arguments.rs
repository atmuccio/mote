//! `java/swapped-arguments` — two arguments named exactly like each other's parameters of a
//! method declared in this file.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "typo"],
    remediation_minutes: 2,
    description: r#"Detects calls where two arguments are variables named exactly like the
*other's* parameter: `resize(height, width)` for `void resize(int width, int height)`. Only
methods and constructors declared in the same file are checked, since their parameter names
are known.

**Why it matters.** When both parameters have the same type, the compiler cannot notice the
exchange. Names are the only clue, and here they say that each value goes to the parameter
meant for the other one: the width becomes the height, the source becomes the target, the
start becomes the end.

**How to fix.** Pass the arguments in the order of the parameters, or rename the variables if
the order is deliberate.

```java
// Noncompliant
void copy(Path source, Path target) { … }
copy(target, source);

// Compliant
copy(source, target);
```

Not reported: calls inside methods whose name suggests a deliberate exchange (`reverse`,
`swap`, `transpose`, `flip`, `invert`, `mirror`, `rotate`, `opposite`, …), recursive calls of a
method from within itself (symmetric delegation such as `compare(b, a)`),
overloaded methods with several declarations of the same arity, names of one or two letters
(`a`, `b`, `x`), calls made in both orders in the same method (a symmetric check), and
classes that inherit
from types declared elsewhere (an inherited overload could be the one called)."#,
    ..RuleMeta::new(
        "java/swapped-arguments",
        "java",
        "Arguments swapped with respect to parameter names",
    )
};

/// Method and constructor declarations with two or more parameters, by name (the class name
/// for constructors), collected during the walk; calls are checked in `finish`, only when a
/// declaration of their name and arity exists.
#[derive(Default)]
pub struct SwappedArguments<'t> {
    calls: Vec<Candidate<'t>>,
    /// Parameter names of the declarations, by method (or class) name.
    decls: FxHashMap<&'t str, Vec<Vec<&'t str>>>,
    /// Type bodies whose declarations are already collected.
    seen_bodies: FxHashSet<usize>,
}

impl<'t> SwappedArguments<'t> {
    /// Collect the methods and constructors with two or more parameters of a type body and of
    /// the types nested in it (members only, not method bodies). A body is visited before any
    /// call inside it, so the calls of a class see all its declarations.
    fn collect_declarations(&mut self, ctx: &Ctx<'t>, body: Node<'t>) {
        let mut stack = vec![body];
        let mut steps = 0;
        while let Some(b) = stack.pop() {
            steps += 1;
            if steps > 4096 || !self.seen_bodies.insert(b.id()) {
                continue;
            }
            for m in ctx.members(b) {
                match kind(m) {
                    "method_declaration" | "constructor_declaration" => {
                        // Two or more parameters, none of them varargs: their names, read in
                        // one pass.
                        let mut params = syntax::parameters(m);
                        let (Some(first), Some(second)) = (params.next(), params.next()) else {
                            continue;
                        };
                        let mut names = Vec::with_capacity(4);
                        let mut varargs = false;
                        for p in [first, second].into_iter().chain(params) {
                            if kind(p) == "spread_parameter" {
                                varargs = true;
                                break;
                            }
                            names.push(syntax::name_of(p, ctx.source()));
                        }
                        if !varargs {
                            let name = syntax::name_of(m, ctx.source());
                            self.decls.entry(name).or_default().push(names);
                        }
                    }
                    _ if syntax::is_type_declaration(m) => {
                        if let Some(nested) = syntax::type_body(m) {
                            stack.push(nested);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

struct Candidate<'t> {
    call: Node<'t>,
    /// Method name, or the class name for `new C(..)` / `this(..)`.
    name: &'t str,
    constructor: bool,
}

/// A call of a declared name in `finish`: index in `calls`, identifier arguments, argument
/// count, and whether a swapped pair is possible.
type Checked<'t> = (usize, Vec<(usize, &'t str)>, usize, bool);

const DELIBERATE: &[&str] = &[
    "reverse",
    "swap",
    "transpose",
    "flip",
    "invert",
    "inverse",
    "mirror",
    "rotate",
    "opposite",
    "backward",
    "exchange",
    "toggle",
];

/// (position, identifier text) of the plain-identifier arguments, and the argument count.
fn ident_args<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> (Vec<(usize, &'t str)>, usize) {
    let mut out = Vec::new();
    let mut n = 0;
    for (i, a) in util::args(call).enumerate() {
        n = i + 1;
        if kind(a) == "identifier" {
            out.push((i, ctx.text(a)));
        }
    }
    (out, n)
}

/// The first pair of arguments (positions `i < j`) named like each other's parameters.
fn swapped_pair<'t>(
    idents: &[(usize, &'t str)],
    params: &[&'t str],
) -> Option<(usize, &'t str, usize, &'t str)> {
    for (x, &(i, a)) in idents.iter().enumerate() {
        for &(j, b) in &idents[x + 1..] {
            let (Some(pi), Some(pj)) = (params.get(i), params.get(j)) else {
                continue;
            };
            // One- and two-letter names (`a`, `b`, `x`, `y`) carry no meaning.
            if a != b && a == *pj && b == *pi && a.len() >= 3 && b.len() >= 3 {
                return Some((i, a, j, b));
            }
        }
    }
    None
}

fn param_names<'t>(ctx: &Ctx<'t>, decl: Node<'t>) -> Option<Vec<&'t str>> {
    syntax::parameters(decl)
        .map(|p| (kind(p) != "spread_parameter").then(|| syntax::name_of(p, ctx.source())))
        .collect()
}

fn is_class_like(n: Node<'_>) -> bool {
    matches!(kind(n), "class_declaration" | "record_declaration")
}

impl<'t> SwappedArguments<'t> {
    /// The single method or constructor of `class` named `name` with `arity` parameters.
    fn callee(ctx: &Ctx<'t>, class: Node<'t>, c: &Candidate<'t>, arity: usize) -> Option<Node<'t>> {
        let body = syntax::type_body(class)?;
        let src = ctx.source();
        let want = if c.constructor {
            "constructor_declaration"
        } else {
            "method_declaration"
        };
        let mut found = None;
        for m in ctx.members(body) {
            if kind(m) != want || (!c.constructor && syntax::name_of(m, src) != c.name) {
                continue;
            }
            let mut count = 0;
            for p in syntax::parameters(m) {
                if kind(p) == "spread_parameter" {
                    return None;
                }
                count += 1;
            }
            if count == arity {
                if found.is_some() {
                    return None;
                }
                found = Some(m);
            }
        }
        found
    }

    /// The class declaring the method an unqualified call refers to: the innermost enclosing
    /// class that declares a method of that name, when no class on the way inherits from a
    /// type declared elsewhere.
    fn declaring_class(ctx: &Ctx<'t>, c: &Candidate<'t>) -> Option<Node<'t>> {
        let src = ctx.source();
        let mut cur = ctx.enclosing_class(c.call)?;
        for _ in 0..8 {
            if !matches!(
                kind(cur),
                "class_declaration" | "enum_declaration" | "record_declaration"
            ) {
                return None;
            }
            if cur.field(Field::Superclass).is_some() {
                return None;
            }
            let body = syntax::type_body(cur)?;
            let declares = ctx
                .members(body)
                .any(|m| kind(m) == "method_declaration" && syntax::name_of(m, src) == c.name);
            if declares {
                return Some(cur);
            }
            cur = ctx.enclosing_class(cur)?;
        }
        None
    }

    /// The class `new C(..)` / `this(..)` refers to: a class of this file with that simple
    /// name, visible from the call.
    fn constructed_class(ctx: &Ctx<'t>, c: &Candidate<'t>) -> Option<Node<'t>> {
        let src = ctx.source();
        let named =
            |m: &Node<'t>| syntax::is_type_declaration(*m) && syntax::name_of(*m, src) == c.name;
        let cls = ctx.enclosing_class(c.call)?;
        for a in std::iter::once(cls).chain(ctx.ancestors(cls)) {
            if named(&a) {
                return Some(a).filter(|f| is_class_like(*f));
            }
            if kind(a) == "program" {
                return named_children(a).find(named).filter(|f| is_class_like(*f));
            }
            if let Some(b) = syntax::type_body(a) {
                if let Some(m) = ctx.members(b).find(named) {
                    return Some(m).filter(|f| is_class_like(*f));
                }
            }
        }
        None
    }

    fn check_candidate(
        ctx: &Ctx<'t>,
        c: &Candidate<'t>,
        idents: &[(usize, &'t str)],
        arity: usize,
        others: &[(Node<'t>, &[(usize, &'t str)])],
    ) {
        let src = ctx.source();
        let class = if c.constructor {
            Self::constructed_class(ctx, c)
        } else {
            Self::declaring_class(ctx, c)
        };
        let Some(class) = class else { return };
        let Some(decl) = Self::callee(ctx, class, c, arity) else {
            return;
        };
        let Some(params) = param_names(ctx, decl) else {
            return;
        };
        let Some((i, a, j, b)) = swapped_pair(idents, &params) else {
            return;
        };
        let (pi, pj) = (params[i], params[j]);
        // Both orders called in the same method: a deliberate symmetric check.
        let method = ctx.enclosing_method(c.call).map(|m| m.id());
        let both_orders = others.iter().any(|(o, oi)| {
            o.id() != c.call.id()
                && oi.contains(&(i, b))
                && oi.contains(&(j, a))
                && ctx.enclosing_method(*o).map(|m| m.id()) == method
        });
        if both_orders {
            return;
        }
        // Deliberate exchanges.
        if let Some(f) = ctx.enclosing_method(c.call) {
            let fname = syntax::name_of(f, src).to_ascii_lowercase();
            let recursive = !c.constructor
                && syntax::name_of(f, src) == c.name
                && syntax::parameters(f).count() == arity;
            if recursive || DELIBERATE.iter().any(|d| fname.contains(d)) {
                return;
            }
        }
        ctx.emit(
            c.call,
            format!("Arguments `{a}` and `{b}` look swapped: they are the names of the parameters `{pj}` and `{pi}` in the other order"),
        );
    }
}

impl<'t> JavaRule<'t> for SwappedArguments<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_body",
            "enum_body",
            "method_invocation",
            "object_creation_expression",
            "explicit_constructor_invocation",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (name, constructor) = match kind(node) {
            "class_body" | "enum_body" => {
                self.collect_declarations(ctx, node);
                return;
            }
            "method_invocation" => {
                // Most calls stop here: no method of that name with two or more parameters.
                let Some(n) = ctx.call_name(node) else { return };
                if !self.decls.contains_key(n) {
                    return;
                }
                if node.field(Field::Object).is_some_and(|o| kind(o) != "this") {
                    return;
                }
                (n, false)
            }
            "object_creation_expression" => {
                let Some(t) = node.field(Field::Type) else {
                    return;
                };
                let n = ctx.text(t);
                if kind(t) != "type_identifier" || !self.decls.contains_key(n) {
                    return;
                }
                if syntax::type_body(node).is_some() {
                    return;
                }
                (n, true)
            }
            _ => {
                // `this(..)` only: `super(..)` calls a constructor of the superclass.
                if node
                    .field(Field::Constructor)
                    .is_none_or(|c| kind(c) != "this")
                {
                    return;
                }
                let Some(cls) = ctx.enclosing_class(node) else {
                    return;
                };
                (syntax::name_of(cls, ctx.source()), true)
            }
        };
        // "(a, b)" has at least five children with punctuation.
        if node
            .field(Field::Arguments)
            .is_none_or(|a| a.child_count() < 5)
        {
            return;
        }
        self.calls.push(Candidate {
            call: node,
            name,
            constructor,
        });
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let calls = std::mem::take(&mut self.calls);
        // Calls of a declared name, with their identifier arguments.
        let mut checked: Vec<Checked<'t>> = Vec::new();
        for (k, c) in calls.iter().enumerate() {
            let Some(decls) = self.decls.get(c.name) else {
                continue;
            };
            let (idents, arity) = ident_args(ctx, c.call);
            if idents.len() < 2 {
                continue;
            }
            // Some declaration of that name and arity has two parameters named like the
            // arguments in the other order.
            let possible = decls
                .iter()
                .any(|p| p.len() == arity && swapped_pair(&idents, p).is_some());
            checked.push((k, idents, arity, possible));
        }
        for (k, idents, arity, possible) in checked.iter().filter(|x| x.3).take(256) {
            let _ = possible;
            let c = &calls[*k];
            let others: Vec<(Node<'t>, &[(usize, &'t str)])> = checked
                .iter()
                .filter(|(o, ..)| {
                    let x = &calls[*o];
                    x.name == c.name && x.constructor == c.constructor
                })
                .map(|(o, oi, ..)| (calls[*o].call, oi.as_slice()))
                .collect();
            Self::check_candidate(ctx, c, idents, *arity, &others);
        }
    }
}
