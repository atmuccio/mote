//! `java/identical-methods` — two methods of one file with the same signature types and the
//! same body.

use super::util;
use crate::prelude::*;
use std::hash::Hasher;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["duplication"],
    remediation_minutes: 10,
    params: &[ParamSpec {
        name: "min_lines",
        default: ParamDefault::Int(3),
        description:
            "Smallest number of lines the statements of a body span for the method to be compared.",
    }],
    description: r#"Detects two methods of one class whose bodies are the same code token for
token (comments and layout aside), with the same return type and the same parameter list.
Bodies shorter than three lines are not compared.

**Why it matters.** Two copies of the same logic have to be kept in step by hand: a fix or a
change applied to one of them is easily forgotten in the other, and the two then drift apart
silently. The duplicate also makes the reader compare the bodies to find out whether they
really are the same.

**How to fix.** Keep the code in one place: let one method call the other, or move the body
into a private method both call (safer when both are hooks subclasses may override
separately), or remove the second method and update its callers.

```java
// Noncompliant
String describeOrder(Order order) {
    StringBuilder sb = new StringBuilder(order.id());
    sb.append(": ").append(order.total());
    return sb.toString();
}
String describeInvoice(Order order) {
    StringBuilder sb = new StringBuilder(order.id());
    sb.append(": ").append(order.total());
    return sb.toString();
}

// Compliant
String describeInvoice(Order order) {
    return describeOrder(order);
}
```

Not reported: test files, methods of different classes (sibling implementations would need a
shared parent first), methods of anonymous classes and enum constant bodies (each implements
its own callback), two methods that both override (each fulfils its own contract, and stubs
often give the same answer to several questions), methods whose parameter or return types
differ (overloads for
different primitive types often have to repeat their body), and bodies whose statements span
fewer lines than `min_lines`. The larger token-level duplication across files is measured
separately."#,
    ..RuleMeta::new(
        "java/identical-methods",
        "java",
        "Methods with identical bodies",
    )
};

#[derive(Default)]
pub struct IdenticalMethods<'t> {
    /// Methods with a body in a named type declaration: (method, body, owning type body id).
    /// Everything else is computed in `finish`, only for methods that share their owner and
    /// signature with another one (a pair needs both).
    methods: Vec<(Node<'t>, Node<'t>, usize)>,
}

/// A method compared in `finish`: (method, body, signature hash, body hash, owning type body
/// id, has `@Override`).
type Compared<'t> = (Node<'t>, Node<'t>, u64, u64, usize, bool);

/// Token texts of `node` in order, comments skipped.
fn tokens<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Vec<&'t str> {
    let mut out = Vec::new();
    util::for_each_token(node, |t| {
        out.push(ctx.text(t));
        out.len() < 50_000
    });
    out
}

/// The signature parts compared: type parameters, return type, parameter list.
fn signature_parts(method: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    [
        method.field(Field::TypeParameters),
        method.field(Field::Type),
        method.field(Field::Parameters),
    ]
    .into_iter()
    .flatten()
}

/// A cheap hash of source text: whitespace and comments skipped. Equal code gives equal
/// hashes; candidates are then compared token by token.
fn text_hash(text: &str, h: &mut rustc_hash::FxHasher) {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < b.len() {
            if b[i + 1] == b'/' {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                continue;
            }
            if b[i + 1] == b'*' {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
        }
        h.write_u8(c);
        i += 1;
    }
}

impl<'t> JavaRule<'t> for IdenticalMethods<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // Named type declarations only: not anonymous classes or enum constant bodies.
        let Some(owner) = ctx.parent(node).and_then(|b| ctx.parent(b).map(|o| (b, o))) else {
            return;
        };
        let owner_body = owner.0;
        let owner = match kind(owner.0) {
            "class_body" | "interface_body" => owner.1,
            "enum_body_declarations" => match ctx.parent(owner.1) {
                Some(e) => e,
                None => return,
            },
            _ => return,
        };
        if !syntax::is_type_declaration(owner) {
            return;
        }
        self.methods.push((node, body, owner_body.id()));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.methods.len() < 2 {
            return;
        }
        // Two methods can only pair up within one owner and with equal signature hashes, so
        // the bodies are measured and hashed only within such groups. Pairs are formed in
        // source order inside a group, as in one global pass over all methods.
        let mut keyed: Vec<(usize, u64, usize)> = self
            .methods
            .iter()
            .enumerate()
            .map(|(i, &(node, _, owner))| {
                let mut sig = rustc_hash::FxHasher::default();
                for part in signature_parts(node) {
                    text_hash(ctx.text(part), &mut sig);
                    sig.write_u8(b'|');
                }
                (owner, sig.finish(), i)
            })
            .collect();
        keyed.sort_unstable();
        let min_lines = ctx.param_i64("min_lines").max(1) as usize;
        let mut group: Vec<Compared<'t>> = Vec::new();
        let mut start = 0;
        while start < keyed.len() {
            let (owner, sig, _) = keyed[start];
            let end = start
                + keyed[start..]
                    .iter()
                    .take_while(|k| k.0 == owner && k.1 == sig)
                    .count();
            if end - start >= 2 {
                group.clear();
                for &(_, _, i) in &keyed[start..end] {
                    let (node, body, _) = self.methods[i];
                    let mut stmts = named_children(body);
                    let Some(first) = stmts.next() else {
                        continue;
                    };
                    let last = stmts.last().unwrap_or(first);
                    let span_lines = last.end_position().row + 1 - first.start_position().row;
                    if span_lines < min_lines {
                        continue;
                    }
                    let mut h = rustc_hash::FxHasher::default();
                    text_hash(ctx.text(body), &mut h);
                    let is_override = syntax::has_annotation(node, ctx.source(), "Override");
                    group.push((node, body, sig, h.finish(), owner, is_override));
                }
                report_pairs(ctx, &group);
            }
            start = end;
        }
    }
}

/// Reports the methods of `methods` (source order) whose body equals an earlier one's.
fn report_pairs<'t>(ctx: &Ctx<'t>, methods: &[Compared<'t>]) {
    let n = methods.len();
    if n < 2 {
        return;
    }
    // Group by body hash (stable: source order within a group).
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (methods[i].3, i));
    let mut reported = vec![false; n];
    for (pos, &i) in order.iter().enumerate() {
        if reported[i] {
            continue;
        }
        for &j in order[pos + 1..]
            .iter()
            .take_while(|&&j| methods[j].3 == methods[i].3)
        {
            if reported[j] {
                continue;
            }
            let (a, ab, asig, _, aowner, aov) = &methods[i];
            let (b, bb, bsig, _, bowner, bov) = &methods[j];
            // Same class; two overrides fulfil separate contracts whose answers may
            // coincide (a stub returning the same value for several queries).
            if aowner != bowner
                || (*aov && *bov)
                || asig != bsig
                || tokens(ctx, *ab) != tokens(ctx, *bb)
                || !same_signature(ctx, *a, *b)
            {
                continue;
            }
            reported[j] = true;
            let src = ctx.source();
            let a_name = syntax::name_of(*a, src);
            let Some(b_name) = syntax::name_node(*b) else {
                continue;
            };
            ctx.emit(
                b_name,
                format!(
                    "This method has the same body as `{a_name}` (line {}): keep the code in \
                     one place that both use",
                    ctx.line(*a)
                ),
            )
            .secondary(a.field(Field::Name).unwrap_or(*a), "same body here");
        }
    }
}

/// Whether two methods have the same signature parts, token for token.
fn same_signature<'t>(ctx: &Ctx<'t>, a: Node<'t>, b: Node<'t>) -> bool {
    let mut pa = signature_parts(a);
    let mut pb = signature_parts(b);
    loop {
        match (pa.next(), pb.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) if kind(x) == kind(y) && tokens(ctx, x) == tokens(ctx, y) => {}
            _ => return false,
        }
    }
}
