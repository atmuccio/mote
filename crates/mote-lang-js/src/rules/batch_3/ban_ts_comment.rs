//! `js/ban-ts-comment`

use crate::prelude::*;
use oxc_ast::Comment;

pub struct BanTsComment;

pub static BAN_TS_COMMENT: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["typescript", "suppression"],
    remediation_minutes: 10,
    params: &[ParamSpec {
        name: "minimum_description_length",
        default: ParamDefault::Int(3),
        description: "Minimum number of characters explaining a `@ts-expect-error` comment.",
    }],
    description: r#"Flags TypeScript comment directives that switch type checking off:
`@ts-ignore`, `@ts-nocheck` at the top of a file, and `@ts-expect-error` without an explanation.

**Why:** these directives silence the compiler instead of fixing the problem it reports.
`@ts-ignore` hides *every* error on the next line — including ones introduced later — and keeps
doing so after the original problem is gone. `@ts-nocheck` disables checking for the whole file.
`@ts-expect-error` is safer (it fails once the error disappears), but without a reason nobody can
tell later whether it is still needed.

**Fix:** correct the types. If a suppression is unavoidable, use `@ts-expect-error` followed by a
short explanation (at least `minimum_description_length` characters, default 3).

The directives are recognised the way TypeScript reads them: at the start of a `//` comment, or on
the last line of a `/* */` comment. Mentions elsewhere in a comment are not flagged.

```ts
// @ts-ignore
legacy.call(1);                          // flagged (the comment above)

// @ts-expect-error
legacy.call(1);                          // flagged: no explanation

// @ts-expect-error: the typings of `legacy` miss the numeric overload
legacy.call(1);                          // compliant
```
"#,
    ..RuleMeta::new(
        "js/ban-ts-comment",
        "js",
        "TypeScript directive comments should not disable type checking",
    )
};

impl JsRule for BanTsComment {
    fn meta(&self) -> &'static RuleMeta {
        &BAN_TS_COMMENT
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let comments = ctx.comments();
        if comments.is_empty() {
            return;
        }
        let first_code = first_code_offset(ctx);
        let mut min_description: Option<usize> = None;
        for comment in comments {
            let Some((directive, rest)) = directive_of(ctx, comment) else {
                continue;
            };
            let span = Span::new(directive.0, directive.0 + directive.1.len() as u32 + 4);
            match directive.1 {
                "ignore" => {
                    ctx.emit(
                        span,
                        "Replace '@ts-ignore' with '@ts-expect-error' and a reason, or fix the type error",
                    );
                }
                "nocheck" if comment.span.end <= first_code => {
                    ctx.emit(
                        span,
                        "Remove '@ts-nocheck' and fix the type errors in this file",
                    );
                }
                "expect-error" => {
                    let min = *min_description.get_or_insert_with(|| {
                        usize::try_from(ctx.param_i64("minimum_description_length")).unwrap_or(0)
                    });
                    if description_length(rest) < min {
                        ctx.emit(
                            span,
                            "Explain after '@ts-expect-error' why the type error is expected",
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Offset of the first directive or statement of the program (end of file when there is none).
fn first_code_offset(ctx: &Ctx<'_>) -> u32 {
    let Some(program) = super::util::program(ctx) else {
        return 0;
    };
    let directive = program.directives.first().map(|d| d.span.start);
    let statement = program.body.first().map(|s| s.span().start);
    match (directive, statement) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => u32::try_from(ctx.source().len()).unwrap_or(u32::MAX),
    }
}

/// The `@ts-<name>` directive a comment carries: `((offset of '@', name), text after the name)`.
fn directive_of<'a>(ctx: &Ctx<'a>, comment: &Comment) -> Option<((u32, &'static str), &'a str)> {
    let content_span = comment.content_span();
    let content = ctx.text(content_span);
    // Line comments: the directive starts the comment. Block comments: TypeScript reads the last
    // line of the comment.
    let (line, line_start, trimmed) = if comment.is_line() {
        // `// @ts-x` or `/// @ts-x`.
        let rest = content.strip_prefix('/').unwrap_or(content);
        (content, content_span.start, rest.trim_start())
    } else {
        let last = content.rfind('\n').map_or(0, |i| i + 1);
        let line = content.get(last..).unwrap_or("");
        let trimmed = line.trim_start_matches(|c: char| c == '/' || c == '*' || c.is_whitespace());
        (line, content_span.start + last as u32, trimmed)
    };
    let after_at = trimmed.strip_prefix("@ts-")?;
    let name = ["expect-error", "ignore", "nocheck"]
        .into_iter()
        .find(|name| after_at.starts_with(name))?;
    // `@ts-nocheck` is a pragma: it must be the whole word.
    let rest = &after_at[name.len()..];
    if name == "nocheck" && rest.starts_with(|c: char| c.is_alphanumeric() || c == '-') {
        return None;
    }
    let offset = line_start + (line.len() - trimmed.len()) as u32;
    Some(((offset, name), rest))
}

/// Length of the explanation after a directive, ignoring separators such as `:` or `--`.
fn description_length(rest: &str) -> usize {
    rest.trim_start_matches(|c: char| c == ':' || c == '-' || c.is_whitespace())
        .trim_end()
        .chars()
        .count()
}
