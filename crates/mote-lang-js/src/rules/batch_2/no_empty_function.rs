//! `js/no-empty-function`

use crate::prelude::*;

pub struct NoEmptyFunction;

pub static NO_EMPTY_FUNCTION: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["suspicious"],
    remediation_minutes: 5,
    description: r#"Flags functions, methods, and arrow functions with an empty body `{}` that contains
no comment.

**Why:** an empty body may be unfinished work, or a deliberate no-op; the reader cannot tell
which. A short comment removes the doubt and documents why doing nothing is correct.

**Fix:** implement the function, or put a comment inside the body explaining why it is empty.
Not reported: constructors (see `js/no-useless-constructor`; TypeScript constructors with
parameter properties and `private`/`protected` constructors are meaningful), TypeScript overload
signatures and abstract methods (they have no body), arrow functions used as a parameter's
default value, functions whose name says they do nothing (`noop`), members of null objects,
mocks, and stubs (classes or objects named `Null…`, `Noop…`, `Mock…`, `Fake…`, `Stub…`,
`Dummy…`), and test files.

```js
function onError() {}           // flagged
const handlers = {
  close() {},                   // flagged
};

function onError() {
  // Errors are reported by the caller.
}                               // compliant
const noop = () => {};          // compliant: the name documents it
```
"#,
    ..RuleMeta::new(
        "js/no-empty-function",
        "js",
        "Functions should not be empty",
    )
};

/// Names that document a deliberate no-op.
fn is_noop_name(name: &str) -> bool {
    let lower = name.trim_start_matches('_').to_ascii_lowercase();
    lower == "nop" || lower.contains("noop") || lower == "emptyfunction" || lower == "donothing"
}

/// Class or object names that mark a deliberate do-nothing implementation: null objects,
/// mocks, stubs, fakes.
fn is_placeholder_name(name: &str) -> bool {
    let lower = name.trim_start_matches('_').to_ascii_lowercase();
    ["null", "noop", "nop", "mock", "fake", "stub", "dummy"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// The name a value is bound to: `const x = …`, `{ x: … }`, `x = …`, `class { x = … }`.
fn bound_name<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<Cow<'a, str>> {
    match ctx.outer_parent(node).map(AstNode::kind) {
        Some(AstKind::MethodDefinition(m)) => m.key.static_name(),
        Some(AstKind::ObjectProperty(p)) => p.key.static_name(),
        Some(AstKind::PropertyDefinition(p)) => p.key.static_name(),
        Some(AstKind::VariableDeclarator(d)) => match &d.id {
            BindingPattern::BindingIdentifier(id) => Some(Cow::Borrowed(id.name.as_str())),
            _ => None,
        },
        Some(AstKind::AssignmentExpression(a)) => match &a.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                Some(Cow::Borrowed(id.name.as_str()))
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                Some(Cow::Borrowed(m.property.name.as_str()))
            }
            _ => None,
        },
        _ => None,
    }
}

fn function_name<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<Cow<'a, str>> {
    if let AstKind::Function(f) = node.kind() {
        if let Some(id) = &f.id {
            return Some(Cow::Borrowed(id.name.as_str()));
        }
    }
    bound_name(ctx, node)
}

/// Name of the class or object literal that holds the member `member`.
fn holder_name<'a>(ctx: &Ctx<'a>, member: &AstNode<'a>) -> Option<Cow<'a, str>> {
    let container = ctx.parent(member)?;
    match container.kind() {
        AstKind::ClassBody(_) => {
            let class = ctx.parent(container)?;
            match class.kind() {
                AstKind::Class(c) => match &c.id {
                    Some(id) => Some(Cow::Borrowed(id.name.as_str())),
                    None => bound_name(ctx, class),
                },
                _ => None,
            }
        }
        AstKind::ObjectExpression(_) => bound_name(ctx, container),
        _ => None,
    }
}

impl JsRule for NoEmptyFunction {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_FUNCTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function, AstType::ArrowFunctionExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Tests are full of deliberate do-nothing stubs and callbacks.
        !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let body = match node.kind() {
            AstKind::Function(f) => match &f.body {
                Some(body) if !f.declare => body,
                _ => return,
            },
            AstKind::ArrowFunctionExpression(a) if !a.expression => &a.body,
            _ => return,
        };
        if !body.statements.is_empty() || !body.directives.is_empty() {
            return;
        }
        if ctx.has_comments_in(body.span) {
            return;
        }
        let Some(parent) = ctx.outer_parent(node) else {
            return;
        };
        let (what, member) = match parent.kind() {
            AstKind::MethodDefinition(m) => {
                if m.kind == MethodDefinitionKind::Constructor {
                    return;
                }
                ("method", true)
            }
            // `callback = () => {}`: a deliberate no-op default.
            AstKind::FormalParameter(_) | AstKind::AssignmentPattern(_) => return,
            AstKind::ObjectProperty(p) => (if p.method { "method" } else { "function" }, true),
            AstKind::PropertyDefinition(_) => ("function", true),
            _ => ("function", false),
        };
        if function_name(ctx, node).is_some_and(|n| is_noop_name(&n)) {
            return;
        }
        if member && holder_name(ctx, parent).is_some_and(|n| is_placeholder_name(&n)) {
            return;
        }
        ctx.emit(
            body.span,
            format!("Add a comment explaining why this {what} is empty, or implement it"),
        );
    }
}
