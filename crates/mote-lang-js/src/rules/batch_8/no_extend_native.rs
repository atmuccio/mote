//! `js/no-extend-native`

use crate::prelude::*;

pub struct NoExtendNative;

pub static NO_EXTEND_NATIVE: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["pitfall", "globals"],
    remediation_minutes: 15,
    description: r#"Flags code that adds or replaces properties on the prototypes of built-in objects:
`Array.prototype.last = …`, `Object.defineProperty(String.prototype, 'x', …)`,
`Object.assign(Date.prototype, {…})`.

**Why:** a built-in prototype is shared by every script in the process or page, so the change
leaks into code that did not ask for it: libraries iterating with `for…in` suddenly see the new
property, two packages adding the same name overwrite each other, and when the language later
gains a method of that name (as happened with `Array.prototype.flatten`, renamed `flat` because of
this) the old definition breaks code relying on the standard one.

**Fix:** write a plain helper function (`last(array)`), or a subclass, instead of patching the
built-in.

```js
Array.prototype.last = function () {          // flagged
  return this[this.length - 1];
};

export function last(array) {                 // compliant
  return array[array.length - 1];
}

if (!Array.prototype.at) {                    // compliant: a guarded polyfill
  Array.prototype.at = function (i) { /* … */ };
}
```

Only the global ECMAScript constructors are checked (`Object`, `Array`, `String`, `Number`,
`Function`, `Promise`, `Map`, `Date`, `RegExp`, the error types, typed arrays, …); a local
variable of the same name is not a built-in. Not reported: polyfills guarded by a test that
mentions the property (`if (!Array.prototype.at)`, `if (!('at' in Array.prototype))`,
`X.prototype.at = X.prototype.at || …`, `X.prototype.at ??= …`), and putting back a property saved
earlier from the same place, as test cleanup does (`Object.defineProperty(Array.prototype, 'at',
saved)` where `saved` only ever holds `Object.getOwnPropertyDescriptor(Array.prototype, 'at')`, or
`Array.prototype.at = originalAt` where `originalAt` only ever holds `Array.prototype.at`); the
temporary override itself is still reported.
"#,
    ..RuleMeta::new(
        "js/no-extend-native",
        "js",
        "Built-in prototypes should not be extended",
    )
};

const BUILTINS: &[&str] = &[
    "Object",
    "Function",
    "Array",
    "String",
    "Number",
    "Boolean",
    "Symbol",
    "BigInt",
    "Date",
    "RegExp",
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "AggregateError",
    "Promise",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "WeakRef",
    "ArrayBuffer",
    "SharedArrayBuffer",
    "DataView",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
];

impl JsRule for NoExtendNative {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EXTEND_NATIVE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::AssignmentExpression, AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["defineProperty", "defineProperties", "assign"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::AssignmentExpression(assign) => {
                let Some(member) = assign.left.as_member_expression() else {
                    return;
                };
                let Some(builtin) = builtin_prototype(ctx, member.object()) else {
                    return;
                };
                let Some(property) = member_property(member) else {
                    return;
                };
                // `X.prototype.y ||= …`, `X.prototype.y = X.prototype.y || …`.
                if assign.operator.is_logical()
                    || ctx
                        .text(assign.right.span())
                        .contains(ctx.text(member.span()))
                    || is_guarded(ctx, node, &property)
                {
                    return;
                }
                // `X.prototype.y = original`, where `original` saved `X.prototype.y`: a restore.
                if assign.operator == AssignmentOperator::Assign
                    && restores(ctx, &assign.right, |saved| {
                        saved.as_member_expression().is_some_and(|m| {
                            builtin_prototype(ctx, m.object()) == Some(builtin)
                                && member_property(m).as_deref() == Some(property.as_str())
                        })
                    })
                {
                    return;
                }
                ctx.emit(
                    member.span(),
                    format!(
                        "Do not add '{property}' to the built-in '{builtin}.prototype': use a helper function"
                    ),
                );
            }
            AstKind::CallExpression(call) => {
                let Some(("Object", method)) = helpers::call_member_names(call) else {
                    return;
                };
                if !matches!(method, "defineProperty" | "defineProperties" | "assign") {
                    return;
                }
                let Expression::StaticMemberExpression(callee) = helpers::unwrap_expr(&call.callee)
                else {
                    return;
                };
                if !ctx.is_global(&callee.object, "Object") {
                    return;
                }
                let Some(target) = call.arguments.first().and_then(Argument::as_expression) else {
                    return;
                };
                let Some(builtin) = builtin_prototype(ctx, target) else {
                    return;
                };
                let property = match (method, call.arguments.get(1)) {
                    ("defineProperty", Some(arg)) => {
                        match arg.as_expression().and_then(helpers::string_value) {
                            Some(name) => name.to_string(),
                            None => return,
                        }
                    }
                    _ => String::new(),
                };
                if !property.is_empty() && is_guarded(ctx, node, &property) {
                    return;
                }
                if property.is_empty() && is_guarded(ctx, node, "prototype") {
                    return;
                }
                // Putting back a descriptor saved from the same place: a restore (test cleanup).
                let saved = match method {
                    "defineProperty" => call.arguments.get(2),
                    "defineProperties" => call.arguments.get(1),
                    _ => None,
                };
                if let Some(saved) = saved.and_then(Argument::as_expression) {
                    let is_original = |value: &Expression<'_>| {
                        let Expression::CallExpression(get) = helpers::unwrap_expr(value) else {
                            return false;
                        };
                        let getter = match method {
                            "defineProperty" => "getOwnPropertyDescriptor",
                            _ => "getOwnPropertyDescriptors",
                        };
                        let Some(("Object", name)) = helpers::call_member_names(get) else {
                            return false;
                        };
                        let Expression::StaticMemberExpression(object) =
                            helpers::unwrap_expr(&get.callee)
                        else {
                            return false;
                        };
                        name == getter
                            && ctx.is_global(&object.object, "Object")
                            && get
                                .arguments
                                .first()
                                .and_then(Argument::as_expression)
                                .and_then(|t| builtin_prototype(ctx, t))
                                == Some(builtin)
                            && (property.is_empty()
                                || get
                                    .arguments
                                    .get(1)
                                    .and_then(Argument::as_expression)
                                    .and_then(helpers::string_value)
                                    == Some(property.as_str()))
                    };
                    if restores(ctx, saved, is_original) {
                        return;
                    }
                }
                let what = if property.is_empty() {
                    "properties".to_string()
                } else {
                    format!("'{property}'")
                };
                ctx.emit(
                    call.span,
                    format!(
                        "Do not add {what} to the built-in '{builtin}.prototype': use a helper function"
                    ),
                );
            }
            _ => {}
        }
    }
}

/// `Array.prototype` (the global `Array`): the constructor's name.
fn builtin_prototype<'a>(ctx: &Ctx<'_>, expr: &Expression<'a>) -> Option<&'a str> {
    let Expression::StaticMemberExpression(m) = helpers::unwrap_expr(expr) else {
        return None;
    };
    if m.property.name != "prototype" {
        return None;
    }
    let Expression::Identifier(id) = helpers::unwrap_expr(&m.object) else {
        return None;
    };
    let name = id.name.as_str();
    (BUILTINS.contains(&name) && ctx.is_global_reference(id)).then_some(name)
}

/// `value` is a local variable and every value stored in it (its initializer and each plain `=`
/// to it, e.g. in a `beforeEach`) is the original, as judged by `is_original`.
fn restores<'a>(
    ctx: &Ctx<'a>,
    value: &Expression<'a>,
    is_original: impl Fn(&Expression<'a>) -> bool,
) -> bool {
    let Expression::Identifier(id) = helpers::unwrap_expr(value) else {
        return false;
    };
    let Some(symbol) = ctx.reference_symbol(id) else {
        return false;
    };
    let AstKind::VariableDeclarator(decl) = ctx.symbol_declaration(symbol).kind() else {
        return false;
    };
    let mut stored = 0;
    if let Some(init) = &decl.init {
        if !is_original(init) {
            return false;
        }
        stored += 1;
    }
    for reference in ctx.references_of(symbol) {
        if !reference.is_write() {
            continue;
        }
        let at = ctx.node(reference.node_id()).span();
        let assigned = ctx
            .ancestors(reference.node_id())
            .find_map(|n| match n.kind() {
                AstKind::AssignmentExpression(a) => Some(a),
                _ => None,
            })
            .filter(|a| a.operator == AssignmentOperator::Assign && a.left.span() == at);
        match assigned {
            Some(a) if is_original(&a.right) => stored += 1,
            _ => return false,
        }
    }
    stored > 0
}

fn member_property(member: &MemberExpression<'_>) -> Option<String> {
    match member {
        MemberExpression::StaticMemberExpression(m) => Some(m.property.name.to_string()),
        MemberExpression::ComputedMemberExpression(m) => {
            helpers::string_value(&m.expression).map(str::to_string)
        }
        MemberExpression::PrivateFieldExpression(_) => None,
    }
}

/// Does `text` mention `name` as a whole word?
fn mentions(text: &str, name: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + name.len()..].chars().next();
        !before.is_some_and(is_word) && !after.is_some_and(is_word)
    })
}

/// Is the node under a condition (`if`, `?:`, `&&`/`||`) whose test mentions `name`?
fn is_guarded(ctx: &Ctx<'_>, node: &AstNode<'_>, name: &str) -> bool {
    let at = node.span();
    if exits_early_on(ctx, node, name) {
        return true;
    }
    ctx.ancestors(node).any(|ancestor| match ancestor.kind() {
        AstKind::IfStatement(s) => {
            !s.test.span().contains_inclusive(at) && mentions(ctx.text(s.test.span()), name)
        }
        AstKind::ConditionalExpression(c) => {
            !c.test.span().contains_inclusive(at) && mentions(ctx.text(c.test.span()), name)
        }
        AstKind::LogicalExpression(l) => {
            l.right.span().contains_inclusive(at) && mentions(ctx.text(l.left.span()), name)
        }
        _ => false,
    })
}

/// `if (Array.prototype.at) return;` before the statement holding the node, in the same list.
fn exits_early_on(ctx: &Ctx<'_>, node: &AstNode<'_>, name: &str) -> bool {
    let Some(stmt) = ctx
        .ancestors(node)
        .find(|a| matches!(a.kind(), AstKind::ExpressionStatement(_)))
    else {
        return false;
    };
    let statements: &[Statement<'_>] = match ctx.parent_kind(stmt) {
        Some(AstKind::BlockStatement(b)) => &b.body,
        Some(AstKind::FunctionBody(b)) => &b.statements,
        _ => return false,
    };
    statements
        .iter()
        .take_while(|s| s.span().start < stmt.span().start)
        .any(|s| match s {
            Statement::IfStatement(i) => {
                mentions(ctx.text(i.test.span()), name) && exits(&i.consequent)
            }
            _ => false,
        })
}

fn exits(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(b) => b.body.last().is_some_and(exits),
        _ => false,
    }
}
