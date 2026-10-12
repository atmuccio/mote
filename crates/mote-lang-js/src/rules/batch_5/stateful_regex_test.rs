//! `js/stateful-regex-test`

use crate::prelude::*;

pub struct StatefulRegexTest;

pub static STATEFUL_REGEX_TEST: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `.test()` calls on a shared regular expression with the global (`g`) or sticky (`y`)
flag: a constant such as `const WORD = /\w+/g` whose `.test()` runs again and again — in another
function, in a loop, or several times in a row.

**Why:** with `g` or `y`, a regular expression object remembers where its last match ended
(`lastIndex`) and the next `.test()` starts searching from there. Reusing the same object makes
the result depend on the previous call: testing `'abc'` twice gives `true`, then `false`. The bug
is intermittent and depends on the order of inputs, which makes it hard to track down.

**Fix:** drop the `g`/`y` flag (`.test()` does not need it), create the regular expression where it
is used, or reset `lastIndex` to `0` before each test.

```js
const DIGITS = /\d+/g;
function hasNumber(text) {
  return DIGITS.test(text);        // flagged: the second call starts at the old lastIndex
}

const DIGITS = /\d+/;              // compliant: no state between calls
function hasNumber(text) {
  return DIGITS.test(text);
}
```

Not reported: regular expressions whose `lastIndex` the code reads or sets anywhere (the author
manages the state) or that are also passed to a call such as `text.replace(RE, …)` (which resets
`lastIndex`; testing first and then replacing is a common, working pattern), a `.test()` used as the condition of a loop (iterating over the matches is
the purpose of the `g` flag), and a single `.test()` on a regular expression created in the same
function outside any loop.

`.exec()` on such a regular expression is reported when the expression is created at the top level
of the file and a function calls `exec()` on it outside any loop: the first call leaves
`lastIndex` after its match, so the next call of the function starts searching there. Not
reported: expressions whose `exec()` also runs in a loop somewhere (`while ((m = RE.exec(s)))`, or
a first call before a loop that continues it), since walking through all the matches until `null`
resets `lastIndex`, and expressions created inside a function (a closure keeping its own position
is a tokenizer on purpose).

```js
const KEY = /(\w+)=/g;
function firstKey(line) {
  return KEY.exec(line)?.[1];      // flagged: the next line is searched from the old position
}
```
"#,
    ..RuleMeta::new(
        "js/stateful-regex-test",
        "js",
        "Global or sticky regular expressions should not be reused with test()",
    )
};

impl JsRule for StatefulRegexTest {
    fn meta(&self) -> &'static RuleMeta {
        &STATEFUL_REGEX_TEST
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.file_contains(AstType::RegExpLiteral)
            || ctx
                .scoping()
                .root_unresolved_references()
                .contains_key("RegExp")
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["test", "exec"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::CallExpression(call) = node.kind() else {
            return;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return;
        };
        let is_exec = match member.property.name.as_str() {
            "test" => false,
            "exec" => true,
            _ => return,
        };
        let Expression::Identifier(id) = helpers::unwrap_expr(&member.object) else {
            return;
        };
        let Some(symbol) = ctx.reference_symbol(id) else {
            return;
        };
        let declaration = ctx.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = declaration.kind() else {
            return;
        };
        if declarator.kind != VariableDeclarationKind::Const
            || !declarator
                .init
                .as_ref()
                .is_some_and(|init| is_stateful_regex(ctx, init))
        {
            return;
        }
        // The loop-condition idiom: `while (re.test(s)) { … }` walks the matches on purpose.
        if is_loop_condition(ctx, node) {
            return;
        }
        let mut tests = 0usize;
        for reference in ctx.references_of(symbol) {
            match ctx.parent_kind(reference.node_id()) {
                Some(AstKind::StaticMemberExpression(m)) => match m.property.name.as_str() {
                    "lastIndex" => return,
                    "test" | "exec" => tests += 1,
                    _ => {}
                },
                // Passed on (`text.replace(RE, …)`, `text.match(RE)`): those calls reset `lastIndex`,
                // and the typical test-then-replace code relies on it.
                Some(AstKind::CallExpression(_) | AstKind::NewExpression(_)) => return,
                _ => {}
            }
        }
        let shared = ctx.enclosing_function(node).map(AstNode::id)
            != ctx.enclosing_function(declaration).map(AstNode::id);
        let name = id.name.as_str();
        if is_exec {
            // In a loop, `exec()` walks through the matches until it returns `null`, which resets
            // `lastIndex`: the intended use of the flag. A closure over an expression created in a
            // function keeps its own position on purpose.
            if !shared || ctx.enclosing_function(declaration).is_some() || ctx.is_in_loop(node) {
                return;
            }
            let iterated = ctx.references_of(symbol).any(|r| {
                matches!(ctx.parent_kind(r.node_id()), Some(AstKind::StaticMemberExpression(m)) if m.property.name == "exec")
                    && ctx.is_in_loop(r.node_id())
            });
            if iterated {
                return;
            }
            ctx.emit(
                member.property.span,
                format!("'{name}' has the g or y flag, so this exec() starts where the previous call's match ended; remove the flag or reset '{name}.lastIndex'"),
            );
            return;
        }
        let repeated = in_loop_below(ctx, node, declaration);
        if !shared && !repeated && tests < 2 {
            return;
        }
        ctx.emit(
            member.property.span,
            format!("'{name}' has the g or y flag, so this test() starts where the previous match ended; remove the flag or reset '{name}.lastIndex'"),
        );
    }
}

/// A regular expression literal (or `new RegExp(…, 'flags')` with literal flags) with `g` or `y`.
fn is_stateful_regex(ctx: &Ctx<'_>, init: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(init) {
        Expression::RegExpLiteral(lit) => {
            lit.regex.flags.intersects(RegExpFlags::G | RegExpFlags::Y)
        }
        Expression::NewExpression(new) if ctx.is_global(&new.callee, "RegExp") => new
            .arguments
            .get(1)
            .and_then(helpers::argument_expr)
            .and_then(helpers::string_value)
            .is_some_and(|flags| flags.contains('g') || flags.contains('y')),
        _ => false,
    }
}

/// Is the call (possibly negated or combined with `&&`/`||`) the test of a loop?
fn is_loop_condition(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    let mut current = node.span();
    for ancestor in ctx.ancestors(node) {
        match ancestor.kind() {
            AstKind::UnaryExpression(_)
            | AstKind::LogicalExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::AssignmentExpression(_) => current = ancestor.span(),
            AstKind::WhileStatement(w) => return w.test.span() == current,
            AstKind::DoWhileStatement(w) => return w.test.span() == current,
            AstKind::ForStatement(f) => {
                return f.test.as_ref().is_some_and(|t| t.span() == current)
            }
            _ => return false,
        }
    }
    false
}

/// Does the call run repeatedly in a loop that does not also recreate the regular expression?
fn in_loop_below(ctx: &Ctx<'_>, node: &AstNode<'_>, declaration: &AstNode<'_>) -> bool {
    let declared = declaration.span();
    for ancestor in ctx.ancestors(node) {
        if ancestor.kind().is_function_like() {
            return false;
        }
        if helpers::is_loop(ancestor.kind()) {
            let span = ancestor.span();
            // A loop that contains the declaration creates a fresh object on every iteration.
            return !(span.start <= declared.start && declared.end <= span.end);
        }
    }
    false
}
