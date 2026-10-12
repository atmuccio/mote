//! `js/no-unexpected-multiline`

use super::util::{indentation_at, scan_gap};
use crate::prelude::*;

pub struct NoUnexpectedMultiline;

pub static NO_UNEXPECTED_MULTILINE: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious"],
    remediation_minutes: 2,
    description: r#"Flags a line that starts with `(`, `[` or a template literal and is therefore glued to the
expression on the line above: a call, an index, or a tagged template instead of a new statement.

**Why:** JavaScript only inserts a semicolon at a line break when the next line cannot continue
the current statement. A line starting with `(`, `[` or `` ` `` always can, so the two lines run
as one expression: the previous value is called with the parenthesized text as arguments, indexed
with the bracketed list, or used as a template tag. The result is a `TypeError` ("… is not a
function") or, worse, a silently wrong value — and the layout makes it look like two statements.

**Fix:** end the previous statement with a semicolon, or start the new line with one
(`;[a, b] = [b, a]`). If the continuation is intended, keep it on the same line.

```js
const total = a + b
(items || []).forEach(add)        // flagged: calls `b(...)`

const parts = line.split(',')
[first, second] = parts           // flagged: indexes `split(...)`

const total = a + b;
(items || []).forEach(add);       // compliant
```

A continuation line indented deeper than the line it continues is taken as deliberate layout
(`makeHandler(options)` followed by an indented `(listener)` line) and is not reported, nor are
optional calls and indexes (`?.(`, `?.[`) and a function expression called on the next line (an
IIFE laid out that way on purpose).
"#,
    ..RuleMeta::new(
        "js/no-unexpected-multiline",
        "js",
        "A line should not start with '(', '[' or a template that continues the previous line",
    )
};

impl JsRule for NoUnexpectedMultiline {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNEXPECTED_MULTILINE
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::CallExpression,
            AstType::ComputedMemberExpression,
            AstType::TaggedTemplateExpression,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (head, gap_start, gap_end, opener, what) = match node.kind() {
            AstKind::CallExpression(call) => {
                if call.optional
                    || matches!(
                        helpers::unwrap_expr(&call.callee),
                        Expression::FunctionExpression(_)
                            | Expression::ArrowFunctionExpression(_)
                            | Expression::ClassExpression(_)
                    )
                    || matches!(ctx.parent_kind(node), Some(AstKind::Decorator(_)))
                {
                    return;
                }
                let start = call
                    .type_arguments
                    .as_ref()
                    .map_or(call.callee.span().end, |t| t.span.end);
                (
                    call.callee.span().start,
                    start,
                    call.span.end,
                    '(',
                    "a call of",
                )
            }
            AstKind::ComputedMemberExpression(member) => {
                if member.optional {
                    return;
                }
                (
                    member.object.span().start,
                    member.object.span().end,
                    member.expression.span().start,
                    '[',
                    "an index into",
                )
            }
            AstKind::TaggedTemplateExpression(tagged) => {
                let start = tagged
                    .type_arguments
                    .as_ref()
                    .map_or(tagged.tag.span().end, |t| t.span.end);
                (
                    tagged.tag.span().start,
                    start,
                    tagged.quasi.span.start + 1,
                    '`',
                    "a template tagged by",
                )
            }
            _ => return,
        };
        let Some(gap) = scan_gap(ctx, gap_start, gap_end) else {
            return;
        };
        if !gap.newline || gap.ch != opener {
            return;
        }
        // A continuation indented deeper than the line it continues is laid out on purpose
        // (`filterEvent(…)\n    (listener, this)`); the hazard is a line that looks like a new
        // statement.
        if indentation_at(ctx, gap.at).len() > indentation_at(ctx, head).len() {
            return;
        }
        ctx.emit(
            Span::new(gap.at, gap.at + 1),
            format!(
                "This '{opener}' makes the line {what} the expression on the line above: end the previous statement with a semicolon, or join the lines if that is intended"
            ),
        );
    }
}
