//! `js/invalid-regexp`

use crate::prelude::*;
use oxc_regular_expression::{ConstructorParser, Options};

pub struct InvalidRegexp;

pub static INVALID_REGEXP: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["regex", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags `new RegExp(…)` and `RegExp(…)` calls whose pattern or flags, given as literals, are not a
valid regular expression.

**Why:** a regular expression literal (`/…/`) is checked when the file is parsed, but a pattern
passed as a string is only compiled when the call runs. An unbalanced bracket, a dangling
quantifier, or an unknown flag then throws a `SyntaxError` at run time — often on a rarely taken
path, long after the code shipped.

**Fix:** correct the pattern or the flags. Remember that backslashes in a string literal need
doubling (`'\\d+'`), or use a regular expression literal when the pattern is constant.

```js
new RegExp('[a-z');           // flagged: unterminated character class
new RegExp('a+', 'gg');       // flagged: duplicated flag
RegExp('(?<name>x)(?<name>y)'); // flagged: duplicated group name

new RegExp('[a-z]+', 'gi');   // compliant
new RegExp(userPattern);      // compliant: not a literal, not checked
```

Only string literals (and template literals without substitutions) are checked, and only when
`RegExp` is the global constructor.
"#,
    ..RuleMeta::new(
        "js/invalid-regexp",
        "js",
        "Regular expressions built from literals should be valid",
    )
};

impl JsRule for InvalidRegexp {
    fn meta(&self) -> &'static RuleMeta {
        &INVALID_REGEXP
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::NewExpression, AstType::CallExpression]
    }

    fn callee_names(&self) -> &'static [&'static str] {
        &["RegExp"]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (callee, arguments) = match node.kind() {
            AstKind::NewExpression(new) => (&new.callee, &new.arguments),
            AstKind::CallExpression(call) => (&call.callee, &call.arguments),
            _ => return,
        };
        if arguments.is_empty() || arguments.len() > 2 || !ctx.is_global(callee, "RegExp") {
            return;
        }
        let Some(pattern) = arguments.first().and_then(|a| literal_text(ctx, a)) else {
            return;
        };
        let flags = match arguments.get(1) {
            None => None,
            Some(arg) => match literal_text(ctx, arg) {
                Some(text) => Some(text),
                None => return,
            },
        };
        // The parser recurses per group / class level: a deeply nested pattern would overflow
        // the stack. Too deep to check is not reported (it may well be valid).
        if !crate::helpers::regex_within_limits(pattern.1, crate::helpers::REGEX_MAX_NESTING) {
            return;
        }
        let allocator = oxc_allocator::Allocator::default();
        let Err(error) = ConstructorParser::new(
            &allocator,
            pattern.1,
            flags.map(|f| f.1),
            Options::default(),
        )
        .parse() else {
            return;
        };
        let message = error.to_string();
        let detail = message
            .strip_prefix("Invalid regular expression:")
            .unwrap_or(&message)
            .trim();
        // The literal itself was accepted by the JavaScript parser; if the regex crate cannot
        // decode it, the problem is ours, not the code's.
        if detail.is_empty() || detail.starts_with("Invalid input string literal") {
            return;
        }
        // Report on the flags when they are the problem.
        let span = match flags {
            Some((span, _)) if detail.contains("flag") => span,
            _ => pattern.0,
        };
        ctx.emit(
            span,
            format!(
                "Fix this invalid regular expression: {}",
                lowercase_first(detail)
            ),
        );
    }
}

/// Span and source text (quotes included) of a string literal or a template literal without
/// substitutions.
fn literal_text<'a>(ctx: &Ctx<'a>, arg: &Argument<'a>) -> Option<(Span, &'a str)> {
    let expr = arg.as_expression()?;
    let span = match expr {
        Expression::StringLiteral(s) => s.span,
        Expression::TemplateLiteral(t) if t.expressions.is_empty() => t.span,
        _ => return None,
    };
    let text = ctx.text(span);
    (text.len() >= 2).then_some((span, text))
}

fn lowercase_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}
