//! `java/jdbc-statement-in-loop` — JDBC updates executed one by one inside a loop without
//! batching, and constant parameters bound again on every pass.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["jdbc", "performance"],
    remediation_minutes: 15,
    description: r#"Detects two inefficiencies of a `PreparedStatement` reused in a loop:

* `executeUpdate()` / `execute()` called on every pass, after binding that pass's parameters,
  in a method that never uses `addBatch`;
* a parameter bound to the same constant on every pass (`ps.setString(1, "ACTIVE")` inside the
  loop), with no `clearParameters()` in the loop.

**Why it matters.** Each `executeUpdate` is a round trip to the database — for a thousand rows,
a thousand network exchanges and statement executions. `addBatch` collects the rows and
`executeBatch` sends them together, which is typically an order of magnitude faster. Parameter
values stay bound until they are replaced or cleared, so binding a constant again on every pass
is wasted work that also hides which values actually change.

**How to fix.** Call `addBatch()` in the loop and `executeBatch()` after it (in chunks for very
large inputs); bind constant parameters once, before the loop.

```java
// Noncompliant
for (Order o : orders) {
    ps.setString(1, "SHIPPED");
    ps.setLong(2, o.getId());
    ps.executeUpdate();
}

// Compliant
ps.setString(1, "SHIPPED");
for (Order o : orders) {
    ps.setLong(2, o.getId());
    ps.addBatch();
}
ps.executeBatch();
```

Not reported: statements prepared inside the loop, updates whose result is used (an update
count that decides what to do next, as in insert-or-update), loops that read generated keys, results or
update counts, or commit on every pass, updates followed by `break`, `return` or `throw` (retry loops), methods
that already use batching, and receivers not provably a `PreparedStatement`. A parameter bound
conditionally or to different values in the same loop is not a constant."#,
    ..RuleMeta::new(
        "java/jdbc-statement-in-loop",
        "java",
        "JDBC statement executed row by row in a loop",
    )
};

#[derive(Default)]
pub struct JdbcStatementInLoop {
    gate: util::Gate,
}

const EXECUTE: &[&str] = &["executeUpdate", "execute", "executeLargeUpdate"];

const SETTERS: &[&str] = &[
    "setString",
    "setNString",
    "setInt",
    "setLong",
    "setShort",
    "setByte",
    "setBoolean",
    "setFloat",
    "setDouble",
    "setBigDecimal",
    "setNull",
    "setObject",
];

const NAMES: &[&str] = &[
    "executeUpdate",
    "execute",
    "executeLargeUpdate",
    "setString",
    "setNString",
    "setInt",
    "setLong",
    "setShort",
    "setByte",
    "setBoolean",
    "setFloat",
    "setDouble",
    "setBigDecimal",
    "setNull",
    "setObject",
];

impl<'t> JavaRule<'t> for JdbcStatementInLoop {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        let execute = EXECUTE.contains(&name);
        if !execute && !SETTERS.contains(&name) {
            return;
        }
        let argc = util::arg_count(node);
        if (execute && argc != 0) || (!execute && argc < 2) {
            return;
        }
        let Some(recv) = node.field(Field::Object) else {
            return;
        };
        if kind(recv) != "identifier" {
            return;
        }
        // The call is a statement of its own.
        let Some(stmt) = ctx.parent(node) else {
            return;
        };
        if kind(stmt) != "expression_statement" {
            return;
        }
        let Some(lp) = enclosing_loop(ctx, node) else {
            return;
        };
        if !self.gate.open(ctx, "PreparedStatement") {
            return;
        }
        let r = ctx.text(recv);
        let Some(loop_body) = lp.field(Field::Body) else {
            return;
        };
        let body_text: String = ctx
            .text(loop_body)
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        // Prepared outside the loop and not replaced in it.
        if body_text.contains(&format!("{r}=")) && !body_text.contains(&format!("{r}==")) {
            return;
        }
        if execute {
            if !self.check_execute(ctx, node, stmt, lp, r, &body_text) {
                return;
            }
        } else if !self.check_constant(ctx, node, stmt, loop_body, r, &body_text) {
            return;
        }
        // Resolver checks last.
        if !util::is_a(ctx, recv, "java.sql.PreparedStatement") {
            return;
        }
        let Some(decl) = ctx.declaration_of(recv) else {
            return;
        };
        if decl.kind == VarKind::Local && decl.decl.start_byte() >= lp.start_byte() {
            return;
        }
        let at = node.field(Field::Name).unwrap_or(node);
        if execute {
            ctx.emit(
                at,
                format!("Batch these updates: call `{r}.addBatch()` in the loop and `{r}.executeBatch()` after it instead of one round trip per pass"),
            );
        } else {
            let idx = util::arg(node, 0).map_or("", |a| ctx.text(a));
            ctx.emit(
                at,
                format!("Bind parameter {idx} once, before the loop: it gets the same constant on every pass"),
            );
        }
    }
}

impl JdbcStatementInLoop {
    /// `ps.executeUpdate();` after per-pass bindings, in a method without batching.
    fn check_execute<'t>(
        &self,
        ctx: &Ctx<'t>,
        call: Node<'t>,
        stmt: Node<'t>,
        lp: Node<'t>,
        r: &str,
        body_text: &str,
    ) -> bool {
        if !body_text.contains(&format!("{r}.set"))
            || body_text.contains("getGeneratedKeys")
            || body_text.contains(".commit(")
            || body_text.contains("getResultSet")
            || body_text.contains("getUpdateCount")
        {
            return false;
        }
        // Retry loops: the update is followed by leaving the loop.
        if ctx.next_named_sibling(stmt).is_some_and(|n| {
            matches!(
                kind(n),
                "break_statement" | "return_statement" | "throw_statement"
            )
        }) {
            return false;
        }
        let scope = ctx.enclosing_function(call).unwrap_or(lp);
        let text = ctx.text(scope);
        !text.contains("addBatch") && !text.contains("executeBatch")
    }

    /// `ps.setX(<index literal>, <constant>);` as an unconditional statement of the loop body,
    /// the only binding of that index in the loop, with no `clearParameters()`.
    fn check_constant<'t>(
        &self,
        ctx: &Ctx<'t>,
        call: Node<'t>,
        stmt: Node<'t>,
        loop_body: Node<'t>,
        r: &str,
        body_text: &str,
    ) -> bool {
        if ctx.parent(stmt).map(|p| p.id()) != Some(loop_body.id()) || kind(loop_body) != "block" {
            return false;
        }
        if body_text.contains("clearParameters") {
            return false;
        }
        let (Some(idx), Some(value)) = (util::arg(call, 0), util::arg(call, 1)) else {
            return false;
        };
        if kind(idx) != "decimal_integer_literal" || !is_constant(ctx, value) {
            return false;
        }
        // Every other argument (`setNull(1, Types.VARCHAR)`, `setObject(1, x, type)`) constant too.
        if !util::args(call).skip(2).all(|a| is_constant(ctx, a)) {
            return false;
        }
        // The only binding of that index in the loop.
        let needle = format!("{r}.set");
        let idx_text = ctx.text(idx);
        let mut bindings = 0;
        let mut from = 0;
        while let Some(pos) = body_text[from..].find(&needle) {
            let start = from + pos + needle.len();
            from = start;
            let rest = &body_text[start..];
            let Some(open) = rest.find('(') else {
                break;
            };
            if !rest[..open].chars().all(|c| c.is_ascii_alphanumeric()) {
                continue;
            }
            let args = &rest[open + 1..];
            if args.starts_with(idx_text) && args[idx_text.len()..].starts_with(',') {
                bindings += 1;
            } else if !args.starts_with(|c: char| c.is_ascii_digit()) {
                // A computed index could be this one.
                return false;
            }
        }
        bindings == 1
    }
}

/// A literal, or a name that is (by convention) a constant: `UPPER_CASE`, possibly qualified
/// (`Types.VARCHAR`, `Status.ACTIVE`).
fn is_constant<'t>(ctx: &Ctx<'t>, n: Node<'t>) -> bool {
    let n = syntax::strip_parens(n);
    match kind(n) {
        "string_literal"
        | "decimal_integer_literal"
        | "hex_integer_literal"
        | "decimal_floating_point_literal"
        | "true"
        | "false"
        | "character_literal" => true,
        "identifier" | "field_access" => {
            let last = util::last_segment(ctx.text(n));
            last.len() > 1
                && last.chars().any(|c| c.is_ascii_uppercase())
                && last
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        }
        _ => false,
    }
}

/// The nearest loop around `node` in the same function, if any.
fn enclosing_loop<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> Option<Node<'t>> {
    for a in ctx.ancestors(node) {
        match kind(a) {
            "for_statement" | "enhanced_for_statement" | "while_statement" | "do_statement" => {
                return Some(a);
            }
            k if syntax::is_function_like(a) || k == "class_body" => return None,
            _ => {}
        }
    }
    None
}
