//! `java/duplicate-string-literal` — the same string literal repeated many times in a file.

use super::util::shorten;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["duplication"],
    params: &[
        ParamSpec {
            name: "min_length",
            default: ParamDefault::Int(5),
            description: "Shortest literal content (characters between the quotes, escapes counted as written) that is considered.",
        },
        ParamSpec {
            name: "threshold",
            default: ParamDefault::Int(3),
            description: "Number of occurrences in one file from which the literal is reported.",
        },
    ],
    remediation_minutes: 5,
    description: r#"Detects string literals that appear `threshold` (default 3) or more times in
the same file, with content at least `min_length` (default 5) characters long.

**Why it matters.** A repeated literal is usually a hidden constant — a column name, a
configuration key, a message. When it has to change, every copy must be found and updated,
and a single typo in one copy silently breaks behaviour.

**How to fix.** Extract the value into a named constant (or an enum) and use it everywhere.

```java
// Noncompliant
row.get("customer_id"); … row.put("customer_id", id); … log("customer_id");

// Compliant
private static final String CUSTOMER_ID = "customer_id";
row.get(CUSTOMER_ID); … row.put(CUSTOMER_ID, id); … log(CUSTOMER_ID);
```

The first occurrence is reported; the others are listed as secondary locations. Ignored:
literals inside annotations, text blocks, literals without any letter or digit (separators
such as `"-----"`), literals containing a run of four or more spaces (padded lines of
templates assembled by concatenation), and test files, where repeating readable sample
values is normal practice."#,
    ..RuleMeta::new(
        "java/duplicate-string-literal",
        "java",
        "Duplicated string literal",
    )
};

#[derive(Default)]
pub struct DuplicateStringLiteral<'t> {
    /// Qualifying literals: (content, node).
    literals: Vec<(&'t str, Node<'t>)>,
    /// End byte of the annotation being walked (its literals are ignored).
    skip_until: usize,
    /// `min_length`, read on first use.
    min_length: Option<usize>,
}

impl<'t> JavaRule<'t> for DuplicateStringLiteral<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "annotation",
            "annotation_type_element_declaration",
            "string_literal",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        if kind(node) != "string_literal" {
            self.skip_until = self.skip_until.max(node.end_byte());
            return;
        }
        if node.start_byte() < self.skip_until {
            return;
        }
        let text = ctx.text(node);
        if text.starts_with("\"\"\"") {
            return;
        }
        let Some(content) = syntax::string_literal_content(node, ctx.source()) else {
            return;
        };
        let min = *self
            .min_length
            .get_or_insert_with(|| ctx.param_i64("min_length").max(0) as usize);
        if content.len() < min || (!content.is_ascii() && content.chars().count() < min) {
            return;
        }
        // Separators and layout fragments (padded lines of templates built by concatenation)
        // are not hidden constants.
        if !content.chars().any(char::is_alphanumeric) || content.contains("    ") {
            return;
        }
        self.literals.push((content, node));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let threshold = ctx.param_i64("threshold").max(2) as usize;
        if self.literals.len() < threshold {
            return;
        }
        // Group equal contents; within a group keep source order.
        self.literals
            .sort_by(|a, b| a.0.cmp(b.0).then(a.1.start_byte().cmp(&b.1.start_byte())));
        let mut i = 0;
        while i < self.literals.len() {
            let content = self.literals[i].0;
            let mut j = i + 1;
            while j < self.literals.len() && self.literals[j].0 == content {
                j += 1;
            }
            let group = &self.literals[i..j];
            if group.len() >= threshold {
                let n = group.len();
                let mut e = ctx
                    .emit(
                        group[0].1,
                        format!(
                            "Define a constant instead of duplicating the literal \"{}\" {n} times",
                            shorten(content, 40)
                        ),
                    )
                    .value(n as f64);
                for (_, other) in &group[1..] {
                    e = e.secondary(*other, "duplicate");
                }
            }
            i = j;
        }
    }
}
