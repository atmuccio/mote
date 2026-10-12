//! `java/reversed-compound-assignment` — `x =+ 1` / `x =- 1` / `b =! c`: a compound operator
//! typed backwards.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "typo"],
    remediation_minutes: 1,
    description: r#"Detects assignments written `x =+ y`, `x =- y`, or `x =! y`: the `=` is glued
to a unary `+`, `-` or `!` while a space separates that sign from its operand.

**Why it matters.** Java reads `x =+ 5` as `x = +5` and `x =- 5` as `x = -5`, so the
variable is overwritten instead of incremented or decremented. `done =! ok` is `done = !ok`,
probably meant as the comparison `done != ok`. The code compiles, and the spacing makes the
typo easy to read past.

**How to fix.** Swap the characters (`+=`, `-=`, `!=`), or, if a plain assignment of a
negated value was intended, put the space after the `=`.

```java
// Noncompliant
total =+ price;
remaining =- 1;

// Compliant
total += price;
remaining -= 1;
offset = -1;
```

Only the typo-shaped spacing is reported (space before `=`, none between `=` and the sign,
space after the sign); `x = -1`, `x=-1` and `x = +y` are left alone."#,
    ..RuleMeta::new(
        "java/reversed-compound-assignment",
        "java",
        "Compound assignment operator typed backwards",
    )
};

pub struct ReversedCompoundAssignment;

impl<'t> JavaRule<'t> for ReversedCompoundAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["assignment_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(right) = node.field(Field::Right) else {
            return;
        };
        if kind(right) != "unary_expression" {
            return;
        }
        let Some(eq) = syntax::operator(node) else {
            return;
        };
        if kind(eq) != "=" || eq.end_byte() != right.start_byte() {
            return;
        }
        let (Some(sign), Some(operand), Some(left)) = (
            syntax::operator(right),
            right.field(Field::Operand),
            node.field(Field::Left),
        ) else {
            return;
        };
        let (fixed, what) = match kind(sign) {
            "+" => ("+=", "`x = +y`, overwriting the value"),
            "-" => ("-=", "`x = -y`, overwriting the value"),
            "!" => ("!=", "`x = !y`, an assignment of the negation"),
            _ => return,
        };
        let src = ctx.source().as_bytes();
        let gap_before = src.get(left.end_byte()..eq.start_byte());
        let gap_after = src.get(sign.end_byte()..operand.start_byte());
        let spaced = |g: Option<&[u8]>| {
            g.is_some_and(|g| !g.is_empty() && g.iter().all(|b| matches!(b, b' ' | b'\t')))
        };
        if !spaced(gap_before) || !spaced(gap_after) {
            return;
        }
        ctx.emit_range(
            Span::cover(eq, sign),
            format!("Did you mean `{fixed}`? This parses as {what}"),
        );
    }
}
