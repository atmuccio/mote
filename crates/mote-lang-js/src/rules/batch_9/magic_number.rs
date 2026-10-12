//! `js/no-magic-numbers` (off by default)

use crate::prelude::*;

pub struct MagicNumber;

pub static MAGIC_NUMBER: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["readability", "conventions"],
    params: &[ParamSpec {
        name: "ignore",
        default: ParamDefault::Str("-1,0,1,2"),
        description: "Comma-separated numbers that are never reported (compared by value, so `1` also accepts `1.0` and `0x1`).",
    }],
    remediation_minutes: 5,
    description: r#"Flags numeric literals ("magic numbers") used directly in expressions, conditions and calls
instead of through a named constant.

**Why:** a bare `86400` or `0.075` says nothing about what it means, and when the same value
appears in several places, changing it means finding every copy, and not changing an unrelated
literal that happens to have the same value.

**Fix:** give the value a name: a module-level `const`, a `static readonly` field, or an enum
member.

```js
if (retries > 5) { setTimeout(poll, 86400000); }     // flagged: 5 and 86400000

const MAX_RETRIES = 5;
const ONE_DAY_MS = 24 * 60 * 60 * 1000;
if (retries > MAX_RETRIES) { setTimeout(poll, ONE_DAY_MS); }   // compliant
```

Not reported: the numbers listed in `ignore` (default `-1, 0, 1, 2`); a literal that is the whole
value given to a name (a variable, a class field, an object property, an assignment target, a
default parameter, a JSX attribute, or the state created by a one-argument call such as
`ref(10)` or `useState(5)`); entries of array literals that are themselves named, turned into a
`Map` or `Set` that is named, or iterated by `for … of` (data tables); any literal in the
initializer of a module-level `const`, a `static` or `readonly` class field, or an enum member;
array indexes; literals in types and decorators; the radix of `parseInt(s, 16)` and
`n.toString(16)`; and test files. Off by default: the rule is useful for focused clean-ups but
noisy on most code bases.
"#,
    ..RuleMeta::new("js/no-magic-numbers", "js", "Numbers should be named constants")
};

/// The rule's per-file state: the parsed `ignore` parameter, and the answers of
/// [`in_named_declaration`] by node.
#[derive(Default)]
struct Ignore(Option<Vec<f64>>, FxHashMap<NodeId, bool>);

/// Does the whole value `top` (the literal with its sign and type wrappers) get a name from its
/// parent?
fn named_by_parent(ctx: &Ctx<'_>, parent: &AstNode<'_>) -> bool {
    match parent.kind() {
        AstKind::VariableDeclarator(_)
        | AstKind::PropertyDefinition(_)
        | AstKind::AccessorProperty(_)
        | AstKind::ObjectProperty(_)
        | AstKind::BindingProperty(_)
        | AstKind::MethodDefinition(_)
        | AstKind::AssignmentPattern(_)
        | AstKind::AssignmentTargetWithDefault(_)
        | AstKind::AssignmentTargetPropertyIdentifier(_)
        | AstKind::FormalParameter(_)
        | AstKind::TSEnumMember(_)
        | AstKind::TSLiteralType(_)
        | AstKind::ComputedMemberExpression(_) => true,
        // `x = 5` names the value; `x *= 5` computes with it.
        AstKind::AssignmentExpression(a) => a.operator == AssignmentOperator::Assign,
        AstKind::JSXExpressionContainer(_) => {
            matches!(ctx.parent_kind(parent), Some(AstKind::JSXAttribute(_)))
        }
        _ => false,
    }
}

/// `new Map([...])`, `new Set([...])`: a lookup table built from a data table.
fn is_table_constructor(ctx: &Ctx<'_>, callee: &Expression<'_>) -> bool {
    matches!(
        helpers::unwrap_expr(callee),
        Expression::Identifier(id)
            if matches!(id.name.as_str(), "Map" | "Set" | "WeakMap" | "WeakSet")
                && ctx.is_global_reference(id)
    )
}

/// `parseInt(s, 16)`, `Number.parseInt(s, 16)`, `n.toString(16)`: the radix explains itself.
fn is_radix(call: &CallExpression<'_>, span: Span) -> bool {
    let position = call.arguments.iter().position(|a| a.span() == span);
    let callee = helpers::unwrap_expr(&call.callee);
    match position {
        Some(1) => {
            matches!(callee, Expression::Identifier(id) if id.name == "parseInt")
                || helpers::static_member_names(callee) == Some(("Number", "parseInt"))
        }
        Some(0) => {
            matches!(callee, Expression::StaticMemberExpression(m) if m.property.name == "toString")
        }
        _ => false,
    }
}

/// Is the literal inside a declaration that names everything in it: a module-level `const`, a
/// `static` or `readonly` field, an enum member, a type, a decorator? Stops at the first function:
/// code inside a function is not a declaration.
/// [`in_named_declaration`] remembered per node passed on the way up: the answer only depends
/// on the first deciding ancestor, which the literals of deeply nested code share (a walk per
/// literal was quadratic in the depth).
fn in_named_declaration_memo(ctx: &mut Ctx<'_>, node: &AstNode<'_>) -> bool {
    let mut memo = std::mem::take(&mut ctx.state::<Ignore>().1);
    let mut passed = Vec::new();
    let answer = in_named_declaration(ctx, node, &memo, &mut passed);
    for id in passed {
        memo.insert(id, answer);
    }
    ctx.state::<Ignore>().1 = memo;
    answer
}

/// Whether `node` is inside a declaration that names its value; `passed` collects the nodes
/// (`node` included) whose answer is the same, for [`in_named_declaration_memo`].
fn in_named_declaration(
    ctx: &Ctx<'_>,
    node: &AstNode<'_>,
    memo: &FxHashMap<NodeId, bool>,
    passed: &mut Vec<NodeId>,
) -> bool {
    if let Some(&known) = memo.get(&node.id()) {
        return known;
    }
    passed.push(node.id());
    for ancestor in ctx.ancestors(node) {
        if let Some(&known) = memo.get(&ancestor.id()) {
            return known;
        }
        match ancestor.kind() {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Program(_) => {
                return false;
            }
            AstKind::VariableDeclarator(d) => {
                if d.kind != VariableDeclarationKind::Const {
                    return false;
                }
                let declaration = ctx.parent(ancestor);
                let container = declaration.and_then(|d| ctx.parent_kind(d));
                return matches!(
                    container,
                    Some(AstKind::Program(_) | AstKind::ExportNamedDeclaration(_))
                );
            }
            AstKind::PropertyDefinition(p) => return p.r#static || p.readonly,
            AstKind::TSEnumMember(_)
            | AstKind::TSTypeAnnotation(_)
            | AstKind::TSTypeAliasDeclaration(_)
            | AstKind::TSInterfaceDeclaration(_)
            | AstKind::TSLiteralType(_)
            | AstKind::Decorator(_) => return true,
            _ => passed.push(ancestor.id()),
        }
    }
    false
}

impl JsRule for MagicNumber {
    fn meta(&self) -> &'static RuleMeta {
        &MAGIC_NUMBER
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NumericLiteral]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::NumericLiteral(literal) = node.kind() else {
            return;
        };
        let mut value = literal.value;
        // The reported text: the literal with its sign.
        let mut shown = literal.span;
        // Climb through the sign and TypeScript wrappers to the whole value (`-5`, `5 as T`).
        let mut top: &AstNode<'a> = node;
        let mut parent = ctx.parent(node);
        while let Some(p) = parent {
            match p.kind() {
                AstKind::UnaryExpression(u)
                    if matches!(
                        u.operator,
                        UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
                    ) =>
                {
                    if u.operator == UnaryOperator::UnaryNegation {
                        value = -value;
                    }
                    shown = u.span;
                }
                AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_) => {}
                _ => break,
            }
            top = p;
            parent = ctx.parent(p);
        }
        if is_ignored(ctx, value) {
            return;
        }
        let Some(mut parent) = parent else {
            return;
        };
        if let AstKind::CallExpression(call) = parent.kind() {
            if is_radix(call, top.span()) {
                return;
            }
        }
        // `ref(10)`, `useState(5)`, `signal(3)`: the literal is the initial value of the state the
        // result names.
        if let AstKind::CallExpression(call) = parent.kind() {
            if call.arguments.len() == 1
                && call
                    .arguments
                    .first()
                    .is_some_and(|a| a.span() == top.span())
            {
                if let Some(outer) = ctx.parent(parent) {
                    if named_by_parent(ctx, outer) {
                        return;
                    }
                }
            }
        }
        // Array literals are data tables: the name, if any, is given to the whole array (or to the
        // map or set built from it).
        let mut in_table = false;
        loop {
            match parent.kind() {
                AstKind::ArrayExpression(_) => in_table = true,
                AstKind::TSAsExpression(_)
                | AstKind::TSSatisfiesExpression(_)
                | AstKind::TSNonNullExpression(_)
                | AstKind::TSTypeAssertion(_) => {}
                AstKind::NewExpression(new)
                    if in_table && is_table_constructor(ctx, &new.callee) => {}
                _ => break,
            }
            let Some(next) = ctx.parent(parent) else {
                return;
            };
            parent = next;
        }
        // `for (const size of [12, 6, 3])`: the loop variable names each entry.
        if in_table && matches!(parent.kind(), AstKind::ForOfStatement(_)) {
            return;
        }
        if named_by_parent(ctx, parent)
            || in_named_declaration_memo(ctx, parent)
            || ctx.in_ambient_context(node)
        {
            return;
        }
        ctx.emit(
            shown,
            format!(
                "Replace the magic number {} with a named constant",
                ctx.text(shown)
            ),
        );
    }
}

/// Is `value` in the `ignore` parameter? Parsed once per file.
fn is_ignored(ctx: &mut Ctx<'_>, value: f64) -> bool {
    if ctx.state::<Ignore>().0.is_none() {
        let list: Vec<f64> = ctx
            .param_str("ignore")
            .split(',')
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        ctx.state::<Ignore>().0 = Some(list);
    }
    ctx.state::<Ignore>()
        .0
        .as_ref()
        .is_some_and(|list| list.contains(&value))
}
