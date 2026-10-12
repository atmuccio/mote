//! `java/nested-code-too-long` — lambdas, anonymous classes and nested classes with more lines
//! of code than their limit.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["brain-overload", "size"],
    params: &[
        ParamSpec {
            name: "max_lambda_lines",
            default: ParamDefault::Int(20),
            description: "Most lines of code allowed in one lambda expression.",
        },
        ParamSpec {
            name: "max_anonymous_class_lines",
            default: ParamDefault::Int(40),
            description: "Most lines of code allowed in one anonymous class.",
        },
        ParamSpec {
            name: "max_nested_class_lines",
            default: ParamDefault::Int(300),
            description: "Most lines of code allowed in one nested, inner or local type.",
        },
    ],
    remediation_minutes: 20,
    description: r#"Detects code nested inside another unit that has grown too long to stay there:

* a lambda expression with more than `max_lambda_lines` lines of code (20 by default);
* an anonymous class with more than `max_anonymous_class_lines` (40);
* a nested, inner or local class, interface, enum or record with more than
  `max_nested_class_lines` (300).

Lines of code are lines holding something other than blanks and comments.

**Why it matters.** Lambdas and anonymous classes are meant for small pieces of behavior
passed along where they are used; the method-length limit counts them as part of the
enclosing method, where a long one makes the method hard to read and cannot be tested or
named on its own. A nested type that grows to hundreds of lines has become a unit in its own
right, sharing the file (and often the private state) of its outer class.

**How to fix.** Move the lambda's body to a named method and pass a method reference; turn
the anonymous class into a named class; move the nested type to its own file.

```java
// Noncompliant
orders.forEach(order -> {
    // 30 lines of validation, pricing and persistence
});

// Compliant
orders.forEach(this::process);

private void process(Order order) {
    // …
}
```

Nested units are measured on their own; the enclosing method still counts their lines for
`function-too-long`. Top-level types are not measured here (see `file-too-long`)."#,
    ..RuleMeta::new(
        "java/nested-code-too-long",
        "java",
        "Lambda, anonymous class or nested type too long",
    )
};

pub struct NestedCodeTooLong;

/// Lines of `text` holding code: something other than whitespace and comments. String, text
/// block and character literals count as code on every line they span.
fn code_lines(text: &str) -> u32 {
    #[derive(PartialEq)]
    enum St {
        Code,
        Line,
        Block,
        Str,
        Text,
        Chr,
    }
    let b = text.as_bytes();
    let mut st = St::Code;
    let mut count = 0u32;
    let mut has_code = false;
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            if has_code {
                count += 1;
            }
            has_code = false;
            if st == St::Line {
                st = St::Code;
            }
            i += 1;
            continue;
        }
        match st {
            St::Code => {
                if c == b'/' && b.get(i + 1) == Some(&b'/') {
                    st = St::Line;
                    i += 2;
                    continue;
                }
                if c == b'/' && b.get(i + 1) == Some(&b'*') {
                    st = St::Block;
                    i += 2;
                    continue;
                }
                if !c.is_ascii_whitespace() {
                    has_code = true;
                }
                if c == b'"' {
                    if b.get(i + 1) == Some(&b'"') && b.get(i + 2) == Some(&b'"') {
                        st = St::Text;
                        i += 3;
                        continue;
                    }
                    st = St::Str;
                } else if c == b'\'' {
                    st = St::Chr;
                }
            }
            St::Line => {}
            St::Block => {
                if c == b'*' && b.get(i + 1) == Some(&b'/') {
                    st = St::Code;
                    i += 2;
                    continue;
                }
            }
            St::Str | St::Chr | St::Text => {
                if !c.is_ascii_whitespace() {
                    has_code = true;
                }
                if c == b'\\' {
                    i += 2;
                    continue;
                }
                let done = match st {
                    St::Str => c == b'"',
                    St::Chr => c == b'\'',
                    _ => c == b'"' && b.get(i + 1) == Some(&b'"') && b.get(i + 2) == Some(&b'"'),
                };
                if done {
                    if st == St::Text {
                        i += 3;
                        st = St::Code;
                        continue;
                    }
                    st = St::Code;
                }
            }
        }
        i += 1;
    }
    if has_code {
        count += 1;
    }
    count
}

/// Number of lines a node spans.
fn span_lines(n: Node<'_>) -> usize {
    n.end_position().row - n.start_position().row + 1
}

impl<'t> JavaRule<'t> for NestedCodeTooLong {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "lambda_expression",
            "object_creation_expression",
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let (param, what) = match kind(node) {
            "lambda_expression" => ("max_lambda_lines", "lambda"),
            "object_creation_expression" => ("max_anonymous_class_lines", "anonymous class"),
            _ => ("max_nested_class_lines", "nested type"),
        };
        // Cheapest test first: a node spanning fewer lines cannot have more lines of code.
        let max = ctx.param_i64(param);
        if span_lines(node) as i64 <= max {
            return;
        }
        let (measured, at) = match kind(node) {
            "lambda_expression" => {
                let arrow = syntax::children(node).find(|c| kind(*c) == "->");
                let end = arrow.map_or(node.start_byte(), |a| a.end_byte());
                (node, Span::new(node.start_byte(), end))
            }
            "object_creation_expression" => {
                let Some(body) = syntax::child_of_kind(node, "class_body") else {
                    return;
                };
                let Some(t) = node.field(Field::Type) else {
                    return;
                };
                (body, Span::new(node.start_byte(), t.end_byte()))
            }
            _ => {
                // Top-level types are the file's business.
                if ctx.parent(node).is_none_or(|p| kind(p) == "program") {
                    return;
                }
                let Some(name) = node.field(Field::Name) else {
                    return;
                };
                (node, Span::from(name))
            }
        };
        let n = code_lines(ctx.text(measured));
        if i64::from(n) <= max {
            return;
        }
        let msg = match what {
            "lambda" => format!(
                "Move the body of this lambda to a named method: it has {n} lines of code (more than {max})"
            ),
            "anonymous class" => format!(
                "Turn this anonymous class into a named class: it has {n} lines of code (more than {max})"
            ),
            _ => format!(
                "Move this nested type to its own file: it has {n} lines of code (more than {max})"
            ),
        };
        ctx.emit_range(at, msg).value(f64::from(n));
    }
}

#[cfg(test)]
mod tests {
    use super::code_lines;

    #[test]
    fn counts_code_lines_only() {
        assert_eq!(code_lines(""), 0);
        assert_eq!(code_lines("a\n\n  \nb"), 2);
        assert_eq!(code_lines("a // x\n// y\n/* z\n z */ b"), 2);
        assert_eq!(code_lines("s = \"// not a comment\";\nt"), 2);
        assert_eq!(code_lines("s = \"\"\"\n  text\n\n  \"\"\";\n"), 3);
        assert_eq!(code_lines("c = '\\'';\n/* a */\nd"), 2);
        assert_eq!(code_lines("x = \"a\\\"b\"; // c\n"), 1);
    }
}
