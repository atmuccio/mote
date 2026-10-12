//! `java/literal-first-in-equals` — `name.equals("admin")` throws when `name` is null.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Low,
    tags: &["null", "strings"],
    remediation_minutes: 1,
    params: &[ParamSpec {
        name: "external_constants",
        default: ParamDefault::Bool(false),
        description: "Also report a call on the result of a method call whose argument is an `UPPER_SNAKE` constant of a class declared in another file (`lookup(key).equals(Names.TYPE)`), whose value and type are not visible.",
    }],
    description: r#"Detects `s.equals("literal")` and `s.equalsIgnoreCase("literal")` where `s`
may be `null`, and the same with a `String` constant declared in this file
(`static final String ADMIN = "admin"`) as the argument.

**Why it matters.** Called on a `null` string, `equals` throws a `NullPointerException`. With
the literal as the receiver the comparison is simply `false` for `null`, which is almost
always what the code means ("is it this value?").

**How to fix.** Swap the operands (or use `Objects.equals`).

```java
// Noncompliant
if (role.equals("admin")) { ... }
if (role.equals(ADMIN)) { ... }

// Compliant
if ("admin".equals(role)) { ... }
if (ADMIN.equals(role)) { ... }
```

The receiver is reported when it is known to be a `String`, and also when its type is unknown
(the result of a call into another class, an inherited field, a lambda parameter): with a
string literal as the argument, swapping the operands gives the same answer for any string.
Receivers of a known type other than `String` are skipped (`Object`, numbers, enums, or a
variable declared in this file with another type). Receivers that cannot be `null` are
skipped too: literals, constants, `this`, `new` expressions, concatenations, and the
results of `String` methods such as `trim()`, `toLowerCase()`, `name()`, or
`String.valueOf(..)` (for those, a `null` would already have failed earlier), local variables
initialized with such a value and never assigned anything that may be `null`
(`String t = key.trim(); … t.equals("|")`), and the elements of `main`'s argument array
(`args[0].equals("start")`), which the JVM never fills with `null`. Receivers that an
enclosing condition has just checked (`s != null && s.equals("x")`, `if (!isEmpty(s)) { … }`)
are not reported either. A constant argument counts only when this file declares it
`static final` with a `String` type and a literal value (also `Nested.NAME` of a class of this
file); constants of other classes are not visible, unless `external_constants` is set: then
`lookup(key).equals(Names.TYPE)` is reported too, on the convention that an `UPPER_SNAKE` field
of a class is a non-null constant."#,
    ..RuleMeta::new(
        "java/literal-first-in-equals",
        "java",
        "String literal should be the receiver of equals",
    )
};

/// Methods whose result is never `null` (or that fail before returning on a `null` input).
const NON_NULL_RESULTS: &[&str] = &[
    "toString",
    "name",
    "trim",
    "strip",
    "stripLeading",
    "stripTrailing",
    "toLowerCase",
    "toUpperCase",
    "substring",
    "replace",
    "replaceAll",
    "replaceFirst",
    "intern",
    "concat",
    "valueOf",
    "format",
    "formatted",
    "repeat",
    "getSimpleName",
    "getName",
    "join",
    "toBinaryString",
    "toHexString",
    "toOctalString",
];

pub struct LiteralFirstInEquals;

/// Whether `text` checks `recv` for `null` (`recv != null`, `null == recv`, `isEmpty(recv)`,
/// `nonNull(recv)`, `recv instanceof …`) or dereferences it (`recv.length()`), so that `recv`
/// cannot be `null` after it.
fn guards(text: &str, recv: &str) -> bool {
    // Cheap pre-filter on the receiver's first token run (the receiver may span lines).
    let head = recv.split(char::is_whitespace).next().unwrap_or(recv);
    if !text.contains(head) {
        return false;
    }
    let squeezed: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let r: String = recv.chars().filter(|c| !c.is_whitespace()).collect();
    let b = squeezed.as_bytes();
    let word_start = |i: usize| {
        i == 0 || {
            let c = b[i - 1];
            !(c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'.')
        }
    };
    [
        format!("{r}!=null"),
        format!("{r}==null"),
        format!("null!={r}"),
        format!("null=={r}"),
        format!("({r})"),
        format!("{r}instanceof"),
        format!("{r}."),
    ]
    .iter()
    .any(|p| {
        squeezed.match_indices(p.as_str()).any(|(i, _)| {
            if p.starts_with('(') {
                // `(recv)` only as the single argument of a check such as `isEmpty(recv)`.
                let before = &squeezed[..i];
                ["Empty", "Blank", "Null", "nonNull", "isNull"]
                    .iter()
                    .any(|w| before.ends_with(w))
            } else {
                p.starts_with("null") || word_start(i)
            }
        })
    })
}

/// Whether the receiver was checked for `null` by an enclosing condition: the left side of an
/// enclosing `&&` / `||`, the condition of an enclosing `if` or conditional expression.
fn is_guarded<'t>(ctx: &Ctx<'t>, call: Node<'t>, recv: &str) -> bool {
    let mut child = call;
    for a in ctx.ancestors(call) {
        let guard = match kind(a) {
            "binary_expression" if matches!(syntax::operator_str(a), "&&" | "||") => {
                util::fld(a, F.right)
                    .filter(|r| r.id() == child.id())
                    .and_then(|_| util::fld(a, F.left))
            }
            "if_statement" | "ternary_expression" | "while_statement" => {
                match util::fld(a, F.condition) {
                    Some(c) if c.id() == child.id() => {
                        // `else if (x.equals("b"))`: the previous condition of the chain.
                        if kind(a) == "if_statement" {
                            match ctx.parent(a) {
                                Some(p)
                                    if kind(p) == "if_statement"
                                        && util::fld(p, F.alternative)
                                            .is_some_and(|alt| alt.id() == a.id()) =>
                                {
                                    util::fld(p, F.condition)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        }
                    }
                    c => c,
                }
            }
            "method_declaration"
            | "constructor_declaration"
            | "class_body"
            | "lambda_expression" => return false,
            _ => None,
        };
        if guard.is_some_and(|g| guards(ctx.text(g), recv)) {
            return true;
        }
        child = a;
    }
    false
}

/// Whether a receiver the resolver could not type is nevertheless declared in this file with a
/// type other than `String` (`Unknown u`, `T value`, `Foo[] items` indexed): such a variable is
/// not a string, and `x.equals("lit")` on it is a different problem.
fn declared_not_string<'t>(ctx: &Ctx<'t>, receiver: Node<'t>) -> bool {
    let mut r = syntax::strip_parens(receiver);
    let mut array_element = false;
    if kind(r) == "array_access" {
        let Some(a) = r.field(Field::Array) else {
            return false;
        };
        r = syntax::strip_parens(a);
        array_element = true;
    }
    let ident = match kind(r) {
        "identifier" => r,
        "field_access" => match (r.field(Field::Object), r.field(Field::Field)) {
            (Some(o), Some(f)) if kind(o) == "this" && kind(f) == "identifier" => f,
            _ => return false,
        },
        _ => return false,
    };
    let Some(decl) = ctx.declaration_of(ident) else {
        return false;
    };
    let Some(ty) = decl.type_node else {
        return false;
    };
    let text = ctx.text(ty);
    let base = text.trim_end_matches(|c: char| c == '[' || c == ']' || c.is_whitespace());
    if !matches!(base, "String" | "java.lang.String") {
        return true;
    }
    // `String[] a; a.equals("x")` compares the array itself.
    let declared_array = text.len() != base.len() || decl.dims > 0;
    declared_array != array_element
}

/// Whether a `static final` field declaration (or an interface constant) declares `name` with
/// type `String` and a literal value (`"admin"`, `"a" + "b"`).
fn declares_string_constant<'t>(ctx: &Ctx<'t>, statement: Node<'t>, declarator: Node<'t>) -> bool {
    let is_constant = kind(statement) == "constant_declaration"
        || (kind(statement) == "field_declaration"
            && syntax::has_modifier(statement, "static")
            && syntax::has_modifier(statement, "final"));
    is_constant
        && util::fld(declarator, F.dimensions).is_none()
        && util::fld(statement, F.ty)
            .is_some_and(|t| matches!(ctx.text(t), "String" | "java.lang.String"))
        && util::fld(declarator, F.value).is_some_and(|v| {
            syntax::is_string_literal(syntax::strip_parens(v))
                || (kind(syntax::strip_parens(v)) == "binary_expression"
                    && util::is_literal_constant_expr(v, 0))
        })
}

/// A `String` constant with a literal value declared in this file: `ADMIN` (a field of this
/// class, an enclosing class, or a superclass of this file) or `Names.ADMIN` (a class of this
/// file).
fn is_file_string_constant<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    match kind(arg) {
        "identifier" => ctx.declaration_of(arg).is_some_and(|d| {
            d.kind == VarKind::Field && declares_string_constant(ctx, d.statement, d.decl)
        }),
        "field_access" => {
            let (Some(obj), Some(field)) = (util::fld(arg, F.object), util::fld(arg, F.field))
            else {
                return false;
            };
            if kind(field) != "identifier" || !matches!(kind(obj), "identifier" | "field_access") {
                return false;
            }
            let Some(fqn) = ctx.type_name_of(obj) else {
                return false;
            };
            let Some(class) = find_type_decl(ctx, ctx.root(), &fqn, 0) else {
                return false;
            };
            let name = ctx.text(field);
            util::class_members(ctx, class)
                .filter(|m| matches!(kind(*m), "field_declaration" | "constant_declaration"))
                .any(|m| {
                    syntax::declarators(m).any(|d| {
                        util::fld(d, F.name).is_some_and(|n| ctx.text(n) == name)
                            && declares_string_constant(ctx, m, d)
                    })
                })
        }
        _ => false,
    }
}

/// `Type.UPPER_SNAKE` where `Type` names a class that is not declared in this file.
fn is_external_constant<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    let (Some(obj), Some(field)) = (util::fld(arg, F.object), util::fld(arg, F.field)) else {
        return false;
    };
    if kind(arg) != "field_access"
        || kind(obj) != "identifier"
        || kind(field) != "identifier"
        || !util::is_constant_name(ctx.text(field))
        || !ctx.text(obj).starts_with(|c: char| c.is_ascii_uppercase())
    {
        return false;
    }
    ctx.type_name_of(obj)
        .is_some_and(|fqn| find_type_decl(ctx, ctx.root(), &fqn, 0).is_none())
}

/// The type declaration of this file whose FQN is `fqn` (type declarations and their member
/// types only, not method bodies).
fn find_type_decl<'t>(ctx: &Ctx<'t>, scope: Node<'t>, fqn: &str, depth: u32) -> Option<Node<'t>> {
    if depth > 8 {
        return None;
    }
    let members: Vec<Node<'t>> = if kind(scope) == "program" {
        named_children(scope).collect()
    } else {
        util::class_members(ctx, scope).collect()
    };
    members
        .into_iter()
        .filter(|m| syntax::is_type_declaration(*m))
        .find_map(|m| {
            let name = ctx.class_fqn(m)?;
            if name == fqn {
                Some(m)
            } else if fqn.starts_with(name.as_str()) {
                find_type_decl(ctx, m, fqn, depth + 1)
            } else {
                None
            }
        })
}

fn can_be_null<'t>(ctx: &Ctx<'t>, receiver: Node<'t>) -> bool {
    can_be_null_d(ctx, receiver, 0)
}

fn can_be_null_d<'t>(ctx: &Ctx<'t>, receiver: Node<'t>, depth: u32) -> bool {
    let r = syntax::strip_parens(receiver);
    match kind(r) {
        "string_literal"
        | "this"
        | "object_creation_expression"
        | "binary_expression"
        | "class_literal"
        | "super" => false,
        "method_invocation" => {
            let name = util::call_name(ctx, r);
            if name == "getName" {
                // `Class.getName()` / `File.getName()`: only when the receiver is a class/enum.
                return !util::fld(r, F.object).is_some_and(|o| {
                    kind(o) == "class_literal"
                        || ctx.call_name(o) == Some("getClass")
                        || ctx
                            .type_of(o)
                            .is_some_and(|t| t.is_class("java.lang.Class"))
                });
            }
            !NON_NULL_RESULTS.contains(&name)
        }
        "identifier" => {
            !(util::is_constant_like(ctx, r) || (depth < 4 && non_null_local(ctx, r, depth)))
        }
        "array_access" => !is_main_argument(ctx, r),
        _ => !util::is_constant_like(ctx, r),
    }
}

/// A local variable initialized with a value that cannot be `null` (`String t = s.trim();`)
/// and never assigned one that may be `null` in its method.
fn non_null_local<'t>(ctx: &Ctx<'t>, ident: Node<'t>, depth: u32) -> bool {
    let Some(decl) = ctx.declaration_of(ident) else {
        return false;
    };
    if decl.kind != VarKind::Local {
        return false;
    }
    let Some(value) = decl.value else {
        return false;
    };
    if syntax::is_null_literal(value) || can_be_null_d(ctx, value, depth + 1) {
        return false;
    }
    // Every assignment in the enclosing function (a captured local cannot be assigned at all).
    let Some(func) = ctx.enclosing_function(ident) else {
        return false;
    };
    let name = ctx.text(ident);
    let mut ok = true;
    util::visit(func, |n| {
        if kind(n) == "assignment_expression"
            && syntax::operator_str(n) == "="
            && util::fld(n, F.left).is_some_and(|l| kind(l) == "identifier" && ctx.text(l) == name)
            && util::fld(n, F.right)
                .is_none_or(|v| syntax::is_null_literal(v) || can_be_null_d(ctx, v, depth + 1))
        {
            ok = false;
            return util::Walk::Stop;
        }
        util::Walk::Continue
    });
    ok
}

/// `args[i]` where `args` is the parameter of `static void main(String[] args)`: the JVM fills
/// the array with the command-line arguments, never with `null`.
fn is_main_argument<'t>(ctx: &Ctx<'t>, access: Node<'t>) -> bool {
    let Some(array) = util::fld(access, F.array).map(syntax::strip_parens) else {
        return false;
    };
    if kind(array) != "identifier" {
        return false;
    }
    let Some(decl) = ctx.declaration_of(array) else {
        return false;
    };
    if decl.kind != VarKind::Parameter {
        return false;
    }
    let Some(method) = ctx.enclosing_method(access) else {
        return false;
    };
    if kind(method) != "method_declaration"
        || syntax::name_of(method, ctx.source()) != "main"
        || !syntax::has_modifier(method, "static")
    {
        return false;
    }
    let mut params = syntax::parameters(method);
    let (Some(p), None) = (params.next(), params.next()) else {
        return false;
    };
    // `String[] args`, `String... args`, `final String args[]`.
    let text = ctx.text(p);
    text.contains("String")
        && text
            .trim_end_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
            .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
            .next()
            == Some(ctx.text(array))
}

impl<'t> JavaRule<'t> for LiteralFirstInEquals {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["equals", "equalsIgnoreCase"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if util::call_named(ctx, node, &["equals", "equalsIgnoreCase"]).is_none() {
            return;
        }
        let Some(name) = util::fld(node, F.name) else {
            return;
        };
        let m = ctx.text(name);
        if m != "equals" && m != "equalsIgnoreCase" {
            return;
        }
        if util::arg_count(node) != 1 {
            return;
        }
        let Some(lit) = util::arg(node, 0) else {
            return;
        };
        let Some(receiver) = util::fld(node, F.object) else {
            return;
        };
        let constant = if syntax::is_string_literal(lit) {
            false
        } else if matches!(kind(lit), "identifier" | "field_access")
            && (is_file_string_constant(ctx, lit)
                || (kind(receiver) == "method_invocation"
                    && ctx.param_bool("external_constants")
                    && is_external_constant(ctx, lit)))
        {
            true
        } else {
            return;
        };
        if !can_be_null(ctx, receiver) {
            return;
        }
        match ctx.type_of(receiver) {
            Some(t) if !t.is_string() => return,
            Some(_) => {}
            None if declared_not_string(ctx, receiver) => return,
            None => {}
        }
        if is_guarded(ctx, node, ctx.text(receiver)) {
            return;
        }
        let what = if constant { "constant" } else { "literal" };
        ctx.emit(
            name,
            format!(
                "Call `{m}` on the {what} ({}.{m}(…)) so that a null receiver cannot throw",
                util::shorten(ctx.text(lit), 30)
            ),
        );
    }
}
