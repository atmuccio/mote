//! `java/regex-compiled-per-call` — a constant regular expression compiled again on every call
//! of a method, or on every pass of a loop.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    tags: &["performance", "regex"],
    remediation_minutes: 5,
    description: r#"Detects a constant regular expression that is compiled again and again:

* `Pattern.compile("…")` or `Pattern.matches("…", input)` with a constant pattern inside a
  method body, so the pattern is compiled on every call;
* `String.matches`, `replaceAll`, `replaceFirst` or `split` with a constant pattern that
  needs the regular-expression engine, inside a loop, so the pattern is compiled on every
  pass.

**Why it matters.** Compiling a pattern parses it and builds a matcher program; for anything
but trivial patterns that costs far more than matching a short string. The `String`
convenience methods hide a `Pattern.compile` on every call. A pattern that never changes can
be compiled once, in a `static final` field, and reused (a compiled `Pattern` is immutable
and safe to share between threads).

**How to fix.** Compile the pattern once into a constant and use it.

```java
// Noncompliant
boolean valid(String code) {
    return Pattern.compile("[A-Z]{3}-\\d{4}").matcher(code).matches();
}
for (String line : lines) {
    String[] cols = line.split("\\s*,\\s*");
}

// Compliant
private static final Pattern CODE = Pattern.compile("[A-Z]{3}-\\d{4}");
private static final Pattern COMMA = Pattern.compile("\\s*,\\s*");
boolean valid(String code) {
    return CODE.matcher(code).matches();
}
for (String line : lines) {
    String[] cols = COMMA.split(line);
}
```

Constant patterns are string literals, concatenations of them and `static final` string
constants of the file. Not reported: patterns built at run time, compilation in field and
static initializers, constructors and `main`, a compiled pattern stored into a field (lazy
initialization), `split` on a single ordinary character or an escaped punctuation character
(`split(",")`, `split("\\|")`, which the JDK handles without the regular-expression engine),
`String` calls outside loops and patterns without any regular-expression syntax (see
`java/replace-all-without-regex`), and test code."#,
    ..RuleMeta::new(
        "java/regex-compiled-per-call",
        "java",
        "Constant regular expression compiled repeatedly",
    )
};

#[derive(Default)]
pub struct RegexCompiledPerCall {
    gate: util::Gate,
}

/// Characters that make a pattern need the regular-expression engine.
const META_CHARS: &[char] = &[
    '\\', '^', '$', '.', '|', '?', '*', '+', '(', ')', '[', ']', '{', '}',
];

/// Whether `String.split(pattern)` takes the JDK's fast path (no `Pattern` compiled): one
/// character that is not a metacharacter, or a backslash and one character that is neither an
/// ASCII letter nor a digit.
fn split_fast_path(p: &str) -> bool {
    let chars: Vec<char> = p.chars().take(3).collect();
    match chars.as_slice() {
        [c] => !".$|()[{^?*+\\".contains(*c),
        ['\\', c] => !c.is_ascii_alphanumeric(),
        _ => false,
    }
}

/// Whether `node` runs inside a loop of its own function (not across lambdas or classes).
fn in_loop<'t>(ctx: &Ctx<'t>, node: Node<'t>) -> bool {
    let mut child = node;
    for a in ctx.ancestors(node) {
        match kind(a) {
            "for_statement" => {
                // Not in the init part, which runs once.
                let mut cursor = a.walk();
                let in_init = a
                    .children_by_field_name("init", &mut cursor)
                    .any(|i| i.id() == child.id());
                if !in_init {
                    return true;
                }
            }
            "enhanced_for_statement" => {
                if a.field(Field::Value).is_none_or(|v| v.id() != child.id()) {
                    return true;
                }
            }
            "while_statement" | "do_statement" => return true,
            "lambda_expression"
            | "class_body"
            | "method_declaration"
            | "constructor_declaration"
            | "static_initializer" => return false,
            _ => {}
        }
        child = a;
    }
    false
}

/// Whether a flags expression is made of constants only: `Pattern.CASE_INSENSITIVE`,
/// `DOTALL | MULTILINE`, numbers.
fn constant_flags(text: &str) -> bool {
    if !text.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '|' | '(' | ')') || c.is_whitespace()
    }) {
        return false;
    }
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .all(|w| {
            w == "Pattern"
                || w == "java"
                || w == "util"
                || w == "regex"
                || w.chars().all(|c| c.is_ascii_digit())
                || w.chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
        })
}

/// Methods whose body typically runs once: `main` and lifecycle callbacks.
fn runs_once(ctx: &Ctx<'_>, method: Node<'_>) -> bool {
    let src = ctx.source();
    if syntax::name_of(method, src) == "main" && syntax::has_modifier(method, "static") {
        return true;
    }
    syntax::annotations(method).any(|a| {
        matches!(
            syntax::annotation_name(a, src),
            "PostConstruct" | "Bean" | "BeforeAll" | "BeforeClass" | "Before" | "BeforeEach"
        )
    })
}

/// Whether the compiled pattern is stored into a field (`PATTERN = Pattern.compile(..)`,
/// lazy initialization).
fn stored_in_field<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> bool {
    let Some(parent) = ctx.parent(call) else {
        return false;
    };
    if kind(parent) != "assignment_expression" {
        return false;
    }
    let Some(left) = parent.field(Field::Left) else {
        return false;
    };
    match kind(left) {
        "field_access" => true,
        "identifier" => ctx
            .declaration_of(left)
            .is_some_and(|d| d.kind == VarKind::Field),
        _ => false,
    }
}

impl<'t> JavaRule<'t> for RegexCompiledPerCall {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["compile", "matches", "replaceAll", "replaceFirst", "split"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if ctx.is_test() {
            return;
        }
        let n = util::arg_count(node);
        let is_static = match (name, n) {
            ("compile", 1 | 2) | ("matches", 2) => true,
            ("matches", 1) | ("replaceAll" | "replaceFirst", 2) | ("split", 1 | 2) => false,
            _ => return,
        };
        let Some(pattern_arg) = util::arg(node, 0) else {
            return;
        };
        if !matches!(
            kind(syntax::strip_parens(pattern_arg)),
            "string_literal" | "binary_expression" | "identifier"
        ) {
            return;
        }
        // The function the call runs in must be an ordinary method.
        let Some(func) = ctx.enclosing_function(node) else {
            return;
        };
        let in_method = match kind(func) {
            "method_declaration" => !runs_once(ctx, func),
            "lambda_expression" => ctx
                .enclosing_method(node)
                .is_some_and(|m| kind(m) == "method_declaration" && !runs_once(ctx, m)),
            _ => false,
        };
        if !in_method {
            return;
        }
        if is_static {
            if !self.gate.open(ctx, "Pattern") {
                return;
            }
            let Some(recv) = node.field(Field::Object) else {
                return;
            };
            if !util::names_type(ctx, recv, "java.util.regex.Pattern") {
                return;
            }
            if name == "compile" && stored_in_field(ctx, node) {
                return;
            }
            if n == 2 && name == "compile" {
                // Flags must be constant too (`Pattern.CASE_INSENSITIVE | Pattern.DOTALL`).
                let Some(flags) = util::arg(node, 1) else {
                    return;
                };
                if !constant_flags(ctx.text(flags)) {
                    return;
                }
            }
            let Some(pattern) = util::string_constant(ctx, pattern_arg, 0) else {
                return;
            };
            if pattern.is_empty() {
                return;
            }
            ctx.emit(
                node,
                "Compile this constant pattern once into a static final field instead of on \
                 every call",
            );
            return;
        }
        // String methods: only inside a loop.
        if !in_loop(ctx, node) {
            return;
        }
        let Some(pattern) = util::string_constant(ctx, pattern_arg, 0) else {
            return;
        };
        if !pattern.contains(META_CHARS) || (name == "split" && split_fast_path(&pattern)) {
            return;
        }
        let Some(recv) = node.field(Field::Object).map(syntax::strip_parens) else {
            return;
        };
        if kind(recv) != "string_literal" && !ctx.type_of(recv).is_some_and(|t| t.is_string()) {
            return;
        }
        let Some(method) = node.field(Field::Name) else {
            return;
        };
        ctx.emit(
            method,
            format!(
                "`{name}` compiles this constant pattern on every pass of the loop: compile it \
                 once into a static final Pattern"
            ),
        );
    }
}
