//! `js/prefer-arrow-callback` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct PreferArrowCallback;

pub static PREFER_ARROW_CALLBACK: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "es2015"],
    remediation_minutes: 2,
    params: &[ParamSpec {
        name: "allow_named_functions",
        default: ParamDefault::Bool(true),
        description: "Accept named function expressions as callbacks (the name shows up in stack traces, which an arrow function passed as an argument would lose).",
    }],
    description: r#"Flags anonymous `function` expressions passed as callbacks when an arrow function would
behave the same: the function does not use `this`, `arguments`, `super` or `new.target`, and is
not a generator. A `function` expression bound with `.bind(this)` is flagged too.

**Why:** an arrow function is shorter, and it says at a glance that the callback has no `this`
of its own: it cannot be called with a surprising receiver, and code later added to it that
uses `this` sees the enclosing one, which is almost always what was meant. `function () { … }
.bind(this)` is the older spelling of exactly that.

**Fix:** write the callback as an arrow function.

```js
items.forEach(function (item) { total += item.price; });   // flagged
items.forEach((item) => { total += item.price; });          // compliant

button.addEventListener('click', function () {
  this.classList.toggle('active');                          // compliant: uses its own `this`
});
```

Not reported: functions that mention `this`, `arguments`, `super` or `new.target` anywhere in
their text (even in a nested function: the rule stays on the safe side), generators, functions
with a TypeScript `this` parameter, named function expressions (unless `allow_named_functions`
is false and the name is not used inside), and functions passed to AngularJS-style
`controller`, `service` and `provider` registrations, which are instantiated with `new`.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/prefer-arrow-callback",
        "js",
        "Callbacks should be arrow functions when they do not need their own 'this'",
    )
};

/// Registrations whose callback is called with `new` (an arrow function would throw).
const CONSTRUCTED: &[&str] = &["controller", "service", "provider"];

/// `word` occurs in `text` as a whole identifier.
fn mentions(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    text.match_indices(word).any(|(at, _)| {
        let before = at.checked_sub(1).and_then(|i| bytes.get(i));
        let after = bytes.get(at + word.len());
        let ident = |b: Option<&u8>| {
            b.is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$')
        };
        !ident(before) && !ident(after)
    })
}

/// How the function expression `node` is passed: directly as an argument, or as
/// `function () {}.bind(this)` given as an argument. `None` when it is not a callback.
fn callback_form<'a>(ctx: &Ctx<'a>, node: &AstNode<'a>) -> Option<(bool, &'a AstNode<'a>)> {
    let parent = ctx.parent(node)?;
    match parent.kind() {
        AstKind::CallExpression(call) if call.arguments.iter().any(|a| a.span() == node.span()) => {
            Some((false, parent))
        }
        AstKind::NewExpression(new) if new.arguments.iter().any(|a| a.span() == node.span()) => {
            Some((false, parent))
        }
        AstKind::StaticMemberExpression(member) if member.property.name == "bind" => {
            let bind_call = ctx.parent(parent)?;
            let AstKind::CallExpression(call) = bind_call.kind() else {
                return None;
            };
            let bound_to_this = call.callee.span() == member.span
                && call.arguments.len() == 1
                && matches!(
                    call.arguments.first().and_then(Argument::as_expression),
                    Some(Expression::ThisExpression(_))
                );
            if !bound_to_this {
                return None;
            }
            let outer = ctx.parent(bind_call)?;
            let is_argument = match outer.kind() {
                AstKind::CallExpression(c) => c.arguments.iter().any(|a| a.span() == call.span),
                AstKind::NewExpression(n) => n.arguments.iter().any(|a| a.span() == call.span),
                _ => false,
            };
            is_argument.then_some((true, outer))
        }
        _ => None,
    }
}

impl JsRule for PreferArrowCallback {
    fn meta(&self) -> &'static RuleMeta {
        &PREFER_ARROW_CALLBACK
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Function]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Function(f) = node.kind() else {
            return;
        };
        if !f.is_expression() || f.generator || f.body.is_none() || f.this_param.is_some() {
            return;
        }
        let Some((bound, call)) = callback_form(ctx, node) else {
            return;
        };
        let callee_name = match call.kind() {
            AstKind::CallExpression(c) => helpers::callee_name(&c.callee),
            AstKind::NewExpression(n) => helpers::callee_name(&n.callee),
            _ => None,
        };
        if callee_name.is_some_and(|name| CONSTRUCTED.contains(&name)) {
            return;
        }
        let text = ctx.text(f.span);
        let uses_own_context = (!bound && (mentions(text, "this") || mentions(text, "super")))
            || mentions(text, "arguments")
            || text.contains("new.target");
        if uses_own_context {
            return;
        }
        if let Some(id) = &f.id {
            if ctx.param_bool("allow_named_functions") {
                return;
            }
            let self_referenced = ctx
                .binding_symbol(id)
                .is_some_and(|symbol| ctx.references_of(symbol).next().is_some());
            if self_referenced {
                return;
            }
        }
        let head = ctx.text(Span::new(f.span.start, f.params.span.start));
        let Some(offset) = head.find("function") else {
            return;
        };
        let start = f.span.start + offset as u32;
        let message = if bound {
            "Use an arrow function instead of binding 'this' to this callback"
        } else {
            "Use an arrow function for this callback"
        };
        ctx.emit(Span::new(start, start + 8), message);
    }
}
