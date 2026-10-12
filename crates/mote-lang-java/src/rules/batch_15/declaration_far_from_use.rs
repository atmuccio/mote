//! `java/declaration-far-from-use` — a local variable declared many statements before the
//! statement that first uses it.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "readability"],
    params: &[ParamSpec {
        name: "max_statements",
        default: ParamDefault::Int(5),
        description: "Largest number of statements accepted between a declaration and the \
                      first statement that mentions the variable.",
    }],
    remediation_minutes: 2,
    description: r#"Detects local variables declared without a value, or with a literal value,
more than `max_statements` statements (5 by default) before the first statement of the same
block that mentions them.

**Why it matters.** A declaration far ahead of its use makes the reader carry the variable
through unrelated code, wondering whether something in between reads or changes it. It widens
the variable's scope for no benefit and is often left over from the habit of declaring
everything at the top of a method. Declared right where it is needed, the variable's purpose
is obvious from its context, and it can often get its real value at once.

**How to fix.** Move the declaration down to just before the statement that first uses it
(or into it, when it is only used inside a loop or a branch).

```java
// Noncompliant
int retries = 0;
log.info("starting");
connect();
loadSettings();
openSession();
registerHandlers();
warmUp();
while (!ready() && retries < 3) { retries++; }

// Compliant
log.info("starting");
connect();
loadSettings();
openSession();
registerHandlers();
warmUp();
int retries = 0;
while (!ready() && retries < 3) { retries++; }
```

Only declarations whose value is absent or a literal (a number, a string, a character, `true`,
`false`, `null`) are considered: moving them cannot change what the program does, whereas a
declaration such as `long start = System.nanoTime();` must stay where it is. Statements are
counted at the level of the block that holds the declaration, so a whole loop or `if` counts as
one; any mention of the name inside a statement (in a lambda or a comment within it too) counts
as a use. Variables never
mentioned again are left to `java/unused-local-variable`. The rule is off by default: it is
part of the `conventions` profile."#,
    ..RuleMeta::new(
        "java/declaration-far-from-use",
        "java",
        "Local variable declared long before its first use",
    )
};

pub struct DeclarationFarFromUse;

/// Whether moving a declaration with this initializer later is safe: no value, or a literal.
fn movable_value(value: Option<Node<'_>>) -> bool {
    let Some(value) = value else {
        return true;
    };
    let value = if kind(value) == "unary_expression" {
        match value.field(Field::Operand) {
            Some(operand) => operand,
            None => return false,
        }
    } else {
        value
    };
    matches!(
        kind(value),
        "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "string_literal"
            | "character_literal"
            | "true"
            | "false"
            | "null_literal"
    )
}

/// The first statement after `decl` (in `block`) that mentions `var`, with the number of
/// statements before it. One text search finds the next mention; the siblings are then only
/// compared by position. A mention in a comment between statements does not count.
fn first_use<'t>(
    ctx: &Ctx<'t>,
    block: Node<'t>,
    decl: Node<'t>,
    var: &str,
) -> Option<(usize, Node<'t>)> {
    let src = ctx.source();
    let end = block.end_byte();
    let find = |from: usize| {
        src.get(from..end)
            .and_then(|t| util::find_identifier(t, var))
            .map(|i| from + i)
    };
    let mut mention = find(decl.end_byte())?;
    let mut cursor = util::cursor_at(block, decl)?;
    let mut between = 0usize;
    while cursor.goto_next_sibling() {
        let n = cursor.node();
        if !n.is_named() {
            continue;
        }
        if n.end_byte() <= mention {
            if !syntax::is_comment(n) {
                between += 1;
            }
            continue;
        }
        if syntax::is_comment(n) {
            mention = find(n.end_byte())?;
            continue;
        }
        return Some((between, n));
    }
    None
}

impl<'t> JavaRule<'t> for DeclarationFarFromUse {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // Cheapest first: a call, an object creation or a lambda in the declaration means a
        // value that may not move (a string literal holding a parenthesis is skipped too).
        if ctx.text(node).contains('(') {
            return;
        }
        let Some(parent) = ctx.parent(node) else {
            return;
        };
        if !matches!(
            kind(parent),
            "block" | "constructor_body" | "switch_block_statement_group"
        ) {
            return;
        }
        let max = usize::try_from(ctx.param_i64("max_statements")).unwrap_or(0);
        for declarator in syntax::declarators(node) {
            if !movable_value(declarator.field(Field::Value)) {
                continue;
            }
            let Some(name) = declarator.field(Field::Name) else {
                continue;
            };
            let var = ctx.text(name);
            let Some((between, first_use)) = first_use(ctx, parent, node, var) else {
                continue;
            };
            if between <= max {
                continue;
            }
            ctx.emit(
                name,
                format!(
                    "Declare `{var}` closer to its first use: {between} statements separate them"
                ),
            )
            .value(between as f64)
            .secondary(first_use, "first used here");
        }
    }
}
