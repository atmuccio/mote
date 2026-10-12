//! `java/mockito-mixed-matchers` — a stubbed or verified call that mixes Mockito argument
//! matchers with plain values.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["tests", "mockito"],
    remediation_minutes: 2,
    description: r#"Detects calls whose arguments mix Mockito argument matchers (`any()`, `eq(..)`,
`anyString()`, `argThat(..)`, …) with plain values (literals, variables, `new` objects), as in
`when(repo.find(anyLong(), "active"))` or `verify(mailer).send(eq(to), subject)`.

**Why it matters.** Mockito records matchers on a stack while the arguments are evaluated and
then pairs them with the call's parameters. It needs either a matcher for every argument or
none at all: with a mix it cannot tell which matcher belongs to which argument and throws
`InvalidUseOfMatchersException` at run time (sometimes in a later, unrelated test, which makes
the failure confusing).

**How to fix.** Wrap the plain values in `eq(..)` (or `same(..)`), or use plain values for all
arguments.

```java
// Noncompliant
when(repo.find(anyLong(), "active")).thenReturn(user);

// Compliant
when(repo.find(anyLong(), eq("active"))).thenReturn(user);
```

Only matchers that provably come from Mockito (statically imported from `org.mockito`, or
qualified with `Mockito`, `ArgumentMatchers` or `Matchers`) are recognised, and only
literals, variables, field reads, `new` expressions and `this` count as plain values: a call
to a helper method may itself return a matcher and is never counted."#,
    ..RuleMeta::new(
        "java/mockito-mixed-matchers",
        "java",
        "Mockito matchers mixed with plain values",
    )
};

#[derive(Default)]
pub struct MockitoMixedMatchers {
    /// The file imports from `org.mockito`.
    mockito: Option<bool>,
}

const MATCHERS: &[&str] = &[
    "any",
    "anyInt",
    "anyLong",
    "anyString",
    "anyBoolean",
    "anyDouble",
    "anyFloat",
    "anyShort",
    "anyByte",
    "anyChar",
    "anyList",
    "anyMap",
    "anySet",
    "anyCollection",
    "anyIterable",
    "anyVararg",
    "eq",
    "same",
    "isA",
    "isNull",
    "isNotNull",
    "notNull",
    "nullable",
    "argThat",
    "intThat",
    "longThat",
    "booleanThat",
    "contains",
    "startsWith",
    "endsWith",
    "matches",
    "refEq",
];

/// Whether `arg` is a call of a Mockito matcher.
fn is_matcher<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    let arg = syntax::strip_parens(arg);
    if kind(arg) != "method_invocation" {
        return false;
    }
    let Some(name) = arg.field(Field::Name).map(|n| ctx.text(n)) else {
        return false;
    };
    if !MATCHERS.contains(&name) {
        return false;
    }
    match arg.field(Field::Object) {
        None => {
            if ctx
                .enclosing_class(arg)
                .and_then(syntax::type_body)
                .is_some_and(|b| {
                    ctx.members(b).any(|m| {
                        kind(m) == "method_declaration" && syntax::name_of(m, ctx.source()) == name
                    })
                })
            {
                return false;
            }
            let imports = ctx.imports();
            let explicit = imports.iter().any(|i| {
                i.is_static
                    && !i.is_wildcard
                    && i.path.starts_with("org.mockito.")
                    && i.simple_name() == name
            });
            if explicit {
                return true;
            }
            // A wildcard import only counts when no other static wildcard (Hamcrest's
            // `contains`, `startsWith`, …) could provide the same name.
            imports
                .iter()
                .any(|i| i.is_static && i.is_wildcard && i.path.starts_with("org.mockito."))
                && !imports.iter().any(|i| {
                    i.is_static
                        && !i.path.starts_with("org.mockito.")
                        && (i.is_wildcard || i.simple_name() == name)
                })
        }
        Some(obj) => {
            matches!(
                ctx.text(obj),
                "Mockito" | "ArgumentMatchers" | "Matchers" | "org.mockito.ArgumentMatchers"
            ) && ctx
                .type_name_of(obj)
                .is_some_and(|f| f.starts_with("org.mockito."))
        }
    }
}

/// Whether `arg` is certainly a plain value (not something that could register a matcher).
fn is_plain_value(arg: Node<'_>) -> bool {
    let arg = syntax::strip_parens(arg);
    match kind(arg) {
        "string_literal"
        | "character_literal"
        | "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal"
        | "decimal_floating_point_literal"
        | "true"
        | "false"
        | "null_literal"
        | "identifier"
        | "this"
        | "class_literal" => true,
        "field_access" => arg
            .field(Field::Object)
            .is_some_and(|o| matches!(kind(o), "identifier" | "this" | "field_access")),
        "object_creation_expression" => true,
        "unary_expression" => arg.field(Field::Operand).is_some_and(is_plain_value),
        _ => false,
    }
}

/// Whether a variable argument may stand for a matcher: a local initialized (or assigned) with
/// a call, as in `String sql = anyString(); when(dao.query(sql, eq(1)))`, which registers the
/// matcher when the local is initialized. Parameters and fields are plain values.
fn may_hold_matcher<'t>(ctx: &Ctx<'t>, arg: Node<'t>) -> bool {
    let arg = syntax::strip_parens(arg);
    if kind(arg) != "identifier" {
        return false;
    }
    let Some(decl) = ctx.declaration_of(arg) else {
        return true;
    };
    match decl.kind {
        VarKind::Parameter | VarKind::Field | VarKind::EnumConstant | VarKind::CatchParameter => {
            false
        }
        VarKind::Local => match decl.value {
            Some(v) => {
                let v = syntax::strip_parens(v);
                matches!(kind(v), "method_invocation" | "cast_expression")
                    || ctx.text(v).contains('(') && !matches!(kind(v), "object_creation_expression")
            }
            None => true,
        },
        _ => true,
    }
}

impl<'t> JavaRule<'t> for MockitoMixedMatchers {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        MATCHERS
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // `node` is a call named like a matcher; look at the argument list it sits in.
        let Some(list) = ctx.parent(node) else {
            return;
        };
        if kind(list) != "argument_list" {
            return;
        }
        if !*self.mockito.get_or_insert_with(|| {
            ctx.imports()
                .iter()
                .any(|i| i.path.starts_with("org.mockito"))
        }) {
            return;
        }
        // Report each argument list once: from its first matcher-named argument.
        let first_call = named_children(list).find(|a| {
            let a = syntax::strip_parens(*a);
            kind(a) == "method_invocation"
                && a.field(Field::Name)
                    .is_some_and(|n| MATCHERS.contains(&ctx.text(n)))
        });
        if first_call.is_none_or(|f| syntax::strip_parens(f).id() != node.id()) {
            return;
        }
        let Some(plain) =
            named_children(list).find(|a| is_plain_value(*a) && !may_hold_matcher(ctx, *a))
        else {
            return;
        };
        if !is_matcher(ctx, node) {
            return;
        }
        // The call taking these arguments must not be a matcher itself (`eq(value)`).
        let Some(call) = ctx.parent(list) else {
            return;
        };
        if kind(call) != "method_invocation" || is_matcher(ctx, call) {
            return;
        }
        ctx.emit(
            plain,
            "Wrap this value in `eq(..)`: Mockito needs a matcher for every argument once one is used",
        );
    }
}
