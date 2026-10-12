//! `js/rules-of-hooks`

use super::util::{is_component_name, is_hook_name};
use crate::prelude::*;

pub struct RulesOfHooks;

pub static RULES_OF_HOOKS: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["react", "hooks"],
    remediation_minutes: 15,
    description: r#"Flags React hooks (`useState`, `useEffect`, any `useXxx(…)`) that are not called
unconditionally at the top level of a function component or custom hook: calls inside an `if`, a
ternary, after `&&`/`||`/`??`, inside a `switch` case, a loop, or a `catch` block; calls placed after
an early `return`; and calls made from a nested function (a callback, an event handler, a helper
defined inside the component).

**Why:** React tells hooks apart only by the order in which they are called during a render. If a
hook is skipped on one render and not on the next — or called a different number of times — every
later hook reads the state of a different one. The result is corrupted state or a
"Rendered fewer hooks than expected" crash, often only in rare code paths.

**Fix:** call hooks at the top level of the component or custom hook, before any early return, and
move the condition *inside* the hook (`useEffect(() => { if (enabled) … })`). Logic that needs a hook
in a callback should read the hook's value in the component and pass it down.

Components are recognised by a capitalised name (also when wrapped in `memo`, `forwardRef`, or
another call assigned to a capitalised name) and custom hooks by a `use` + capital name (also when
wrapped in a call assigned to such a name, including a function memoized with `useCallback`:
`const useValue = useCallback((key) => { … }, [])` is a custom hook whose hooks run in the render
of whoever calls it, not in the render that defines it). Hook calls
in functions that are neither (class methods, plain helpers, test callbacks) are not reported. Only
files that show they use React or Preact (an import from a React-family package, a global
`React`, or a JSX pragma naming React) are checked, since `useXxx` is a common name elsewhere.

```jsx
function Profile({ user }) {
  if (!user) return null;
  const [tab, setTab] = useState('info');   // flagged: after an early return
  if (user.admin) {
    useAudit(user);                          // flagged: conditional
  }
}

function Profile({ user }) {
  const [tab, setTab] = useState('info');   // compliant
  useAudit(user, { enabled: user?.admin });  // compliant: condition moved into the hook
  if (!user) return null;
}
```
"#,
    ..RuleMeta::new(
        "js/rules-of-hooks",
        "js",
        "React hooks should be called unconditionally at the top level",
    )
};

#[derive(Clone, Copy)]
enum Problem {
    Conditional,
    Loop,
}

/// First `return` of each component/hook function already scanned (keyed by node index).
#[derive(Default)]
struct FirstReturns {
    by_function: FxHashMap<usize, Option<Span>>,
}

impl JsRule for RulesOfHooks {
    fn meta(&self) -> &'static RuleMeta {
        &RULES_OF_HOOKS
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        super::util::is_react_file(ctx)
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Some((hook, hook_span)) = hook_callee(&call.callee) else {
            return;
        };

        // Walk up to the nearest function, remembering the innermost condition or loop.
        let mut problem = None;
        let mut child = call.span;
        let mut function = None;
        for ancestor in ctx.ancestors(node) {
            let kind = ancestor.kind();
            if matches!(
                kind,
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            ) {
                function = Some(ancestor);
                break;
            }
            if matches!(
                kind,
                AstKind::StaticBlock(_)
                    | AstKind::PropertyDefinition(_)
                    | AstKind::AccessorProperty(_)
            ) {
                return;
            }
            if problem.is_none() {
                problem = condition_of(kind, child);
            }
            child = kind.span();
        }
        let Some(function) = function else {
            return; // module top level: not reported
        };

        match classify(ctx, function) {
            Role::Component(_) | Role::Hook(_) => {
                let owner = owner_label(ctx, function);
                match problem {
                    Some(Problem::Conditional) => {
                        ctx.emit(
                            hook_span,
                            format!(
                                "Call the hook '{hook}' unconditionally in {owner}: hooks must run in the same order on every render"
                            ),
                        );
                    }
                    Some(Problem::Loop) => {
                        ctx.emit(
                            hook_span,
                            format!(
                                "Do not call the hook '{hook}' inside a loop in {owner}: hooks must run in the same order on every render"
                            ),
                        );
                    }
                    None => {
                        if let Some(ret) = first_return(ctx, function) {
                            if ret.end <= call.span.start {
                                ctx.emit(
                                    hook_span,
                                    format!(
                                        "Move the hook '{hook}' above the early return in {owner}: hooks must run on every render"
                                    ),
                                )
                                .secondary(ret, "Early return");
                            }
                        }
                    }
                }
            }
            Role::Other => {
                // A callback or helper inside a component or hook?
                let mut outer = None;
                for ancestor in ctx.ancestors(function) {
                    match ancestor.kind() {
                        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {
                            if !matches!(classify(ctx, ancestor), Role::Other) {
                                outer = Some(ancestor);
                                break;
                            }
                        }
                        AstKind::Class(_) | AstKind::StaticBlock(_) => return,
                        _ => {}
                    }
                }
                if let Some(outer) = outer {
                    let owner = owner_label(ctx, outer);
                    ctx.emit(
                        hook_span,
                        format!(
                            "Call the hook '{hook}' at the top level of {owner}, not inside a nested function"
                        ),
                    );
                }
            }
        }
    }
}

/// `useThing(…)` or `React.useThing(…)` / `Lib.useThing(…)` (capitalised namespace object).
fn hook_callee<'a>(callee: &Expression<'a>) -> Option<(&'a str, Span)> {
    match helpers::unwrap_expr(callee) {
        Expression::Identifier(id) if is_hook_name(&id.name) => Some((id.name.as_str(), id.span)),
        Expression::StaticMemberExpression(m) if is_hook_name(&m.property.name) => {
            match helpers::unwrap_expr(&m.object) {
                Expression::Identifier(obj) if is_component_name(&obj.name) => {
                    Some((m.property.name.as_str(), m.span))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Does `child` (a direct child of `kind`) run conditionally or repeatedly relative to `kind`?
fn condition_of(kind: AstKind<'_>, child: Span) -> Option<Problem> {
    match kind {
        AstKind::IfStatement(s) => (s.test.span() != child).then_some(Problem::Conditional),
        AstKind::ConditionalExpression(c) => {
            (c.test.span() != child).then_some(Problem::Conditional)
        }
        AstKind::LogicalExpression(l) => (l.right.span() == child).then_some(Problem::Conditional),
        AstKind::AssignmentExpression(a) if a.operator.is_logical() => {
            (a.right.span() == child).then_some(Problem::Conditional)
        }
        AstKind::SwitchCase(c) => {
            let in_test = c.test.as_ref().is_some_and(|t| t.span() == child);
            (!in_test).then_some(Problem::Conditional)
        }
        AstKind::CatchClause(_) => Some(Problem::Conditional),
        AstKind::ForStatement(s) => {
            let in_init = s.init.as_ref().is_some_and(|i| i.span() == child);
            (!in_init).then_some(Problem::Loop)
        }
        AstKind::ForInStatement(s) => (s.body.span() == child).then_some(Problem::Loop),
        AstKind::ForOfStatement(s) => (s.body.span() == child).then_some(Problem::Loop),
        AstKind::WhileStatement(_) | AstKind::DoWhileStatement(_) => Some(Problem::Loop),
        _ => None,
    }
}

enum Role<'a> {
    Component(&'a str),
    Hook(&'a str),
    Other,
}

fn role_of_name(name: &str) -> Role<'_> {
    if is_hook_name(name) {
        Role::Hook(name)
    } else if is_component_name(name) {
        Role::Component(name)
    } else {
        Role::Other
    }
}

/// Is this function a component, a custom hook, or something else?
fn classify<'a>(ctx: &Ctx<'a>, function: &AstNode<'a>) -> Role<'a> {
    if let AstKind::Function(f) = function.kind() {
        if let Some(id) = &f.id {
            return role_of_name(id.name.as_str());
        }
    }
    let Some(parent) = ctx.parent(function) else {
        return Role::Other;
    };
    match parent.kind() {
        AstKind::VariableDeclarator(decl) => match &decl.id {
            BindingPattern::BindingIdentifier(id) => role_of_name(id.name.as_str()),
            _ => Role::Other,
        },
        AstKind::AssignmentExpression(assign) => match &assign.left {
            AssignmentTarget::AssignmentTargetIdentifier(id) => role_of_name(id.name.as_str()),
            AssignmentTarget::StaticMemberExpression(m) => role_of_name(m.property.name.as_str()),
            _ => Role::Other,
        },
        AstKind::ObjectProperty(prop) if !prop.computed => match prop.key.static_name() {
            Some(Cow::Borrowed(name)) => role_of_name(name),
            _ => Role::Other,
        },
        AstKind::CallExpression(call) => {
            // `memo(() => …)`, `forwardRef((props, ref) => …)`, or any wrapper whose result is
            // assigned to a component/hook name: `const List = observer(() => …)`.
            let wrapper = match helpers::unwrap_expr(&call.callee) {
                Expression::Identifier(id) => Some(id.name.as_str()),
                Expression::StaticMemberExpression(m) => Some(m.property.name.as_str()),
                _ => None,
            };
            if matches!(wrapper, Some("memo" | "forwardRef")) {
                return Role::Component("");
            }
            if wrapper == Some("useCallback")
                && call
                    .arguments
                    .first()
                    .is_some_and(|a| a.span() == function.span())
            {
                // `const useValue = useCallback((key) => { useThing(); … }, [])`: a memoized
                // custom hook. Its hooks run when it is called, in the render of whoever calls it.
                return match ctx.outer_parent(parent).map(AstNode::kind) {
                    Some(AstKind::VariableDeclarator(decl)) => match &decl.id {
                        BindingPattern::BindingIdentifier(id) if is_hook_name(&id.name) => {
                            Role::Hook(id.name.as_str())
                        }
                        _ => Role::Other,
                    },
                    _ => Role::Other,
                };
            }
            if wrapper.is_some_and(is_hook_name) {
                return Role::Other; // a callback passed to a hook (`useCallback(() => …)`)
            }
            match ctx.parent_kind(parent) {
                Some(AstKind::VariableDeclarator(decl)) => match &decl.id {
                    BindingPattern::BindingIdentifier(id) => role_of_name(id.name.as_str()),
                    _ => Role::Other,
                },
                _ => Role::Other,
            }
        }
        AstKind::ExportDefaultDeclaration(_) if super::util::has_jsx(ctx) => Role::Component(""),
        _ => Role::Other,
    }
}

/// `component 'Name'` / `hook 'useName'` / `this component` for messages.
fn owner_label(ctx: &Ctx<'_>, function: &AstNode<'_>) -> String {
    match classify(ctx, function) {
        Role::Component(name) if !name.is_empty() => format!("component '{name}'"),
        Role::Hook(name) => format!("hook '{name}'"),
        _ => "this component".to_string(),
    }
}

/// The first `return` statement of `function` itself (not of nested functions), memoised per
/// function. Nodes are stored in source pre-order, so the function's subtree is the contiguous run
/// of nodes after it that start before its end.
fn first_return(ctx: &mut Ctx<'_>, function: &AstNode<'_>) -> Option<Span> {
    let key = function.id().index();
    if let Some(&cached) = ctx.state::<FirstReturns>().by_function.get(&key) {
        return cached;
    }
    let outer = function.span();
    let mut skip_until = 0u32;
    let mut found = None;
    for node in ctx.nodes().iter().skip(key + 1) {
        let span = node.span();
        if span.start >= outer.end {
            break;
        }
        if span.start < skip_until {
            continue;
        }
        match node.kind() {
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) | AstKind::Class(_) => {
                skip_until = span.end;
            }
            AstKind::ReturnStatement(r) => {
                if r.span.start >= outer.start && r.span.end <= outer.end {
                    found = Some(r.span);
                }
                break;
            }
            _ => {}
        }
    }
    ctx.state::<FirstReturns>().by_function.insert(key, found);
    found
}
