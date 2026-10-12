//! `java/octal-literal` — `int port = 0100;` is 64, not 100.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["suspicious", "numbers"],
    remediation_minutes: 2,
    params: &[
        ParamSpec {
            name: "report_tables",
            default: ParamDefault::Bool(false),
            description: "Also report octal literals in array initializers that hold several of \
                          them (lookup tables written in octal on purpose). Such a table usually \
                          makes the whole file count as octal by convention: set \
                          `file_convention` to 0 as well to see them.",
        },
        ParamSpec {
            name: "file_convention",
            default: ParamDefault::Int(3),
            description: "A file holding at least this many octal literals (of 8 and above) \
                          uses octal on purpose (archive formats, file modes, generated \
                          tables): none of them is reported. 0 turns the file-level test off.",
        },
    ],
    description: r#"Detects integer literals written with a leading zero, such as `010` or
`030`. In Java a leading zero makes the literal octal: `010` is eight, not ten.

**Why it matters.** Most readers — and most authors padding numbers to line them up — read
`010` as ten. When the octal value was not intended, the program silently uses a different
number (a month, a port, a size); when it was intended, the next maintainer is likely to
"fix" or copy it wrongly. Octal is rarely the clearest way to write a value in Java.

**How to fix.** Drop the leading zero for a decimal value. For bit patterns and file modes
use hexadecimal or binary, or a named constant; `Integer.parseInt("755", 8)` makes an octal
value explicit.

```java
// Noncompliant
int timeoutSeconds = 030;      // 24, not 30
record.setMonth(012);          // 10, not 12

// Compliant
int timeoutSeconds = 30;
record.setMonth(12);
```

The rule looks for octal that looks accidental — a decimal-looking number with a leading zero
— and leaves alone the places where octal is written on purpose:

* `0` itself and the single digits `00` to `07` (octal and decimal agree);
* files that use octal as their notation: at least `file_convention` (default 3) octal
  literals of 8 and above in the file (archive headers, file-mode tables, generated scanners);
* unless `report_tables` is set, literals in an array initializer that holds several octal
  literals (a lookup table such as a character-code map written in octal deliberately);
* file modes: permission-shaped values (`0644`, `0755`, `0600`, `04755`: an owner digit of
  4 to 7 and every permission digit 0, 4, 5, 6 or 7; special bits such as `04000`), Unix mode
  words (`0100644`, `040755`, `0170000`), and any octal value up to `07777` assigned to, compared or combined with,
  returned from, or passed to a name about modes or permissions (`fileMode`, `umask`,
  `setMode(0123)`, `chmod(path, 0700)`, `assertEquals(0644, entry.getMode())`);
* bit patterns: operands of `&`, `|`, `^` and shifts (`c & 077`), masks made of sevens
  (`0777`, `07777777L`), literals with five or more significant digits (`070707`,
  `0100000000000L`), and literals grouped with underscores (`0777_776`), none of which reads
  like a decimal number that lost its meaning to a leading zero."#,
    ..RuleMeta::new("java/octal-literal", "java", "Octal integer literal")
};

/// Per-file state: the literals that look accidental, reported in `finish` unless the file
/// turns out to use octal as its notation.
#[derive(Default)]
pub struct OctalLiteral<'t> {
    candidates: Vec<(Node<'t>, u64)>,
    /// Octal literals of 8 and above seen in the file.
    octal_count: usize,
}

/// Value of an octal literal (`0755`, `0_17L`); `None` when malformed or out of range.
fn octal_value(text: &str) -> Option<u64> {
    let digits = text.trim_end_matches(['l', 'L']);
    let mut v: u64 = 0;
    let mut any = false;
    for c in digits.chars().skip(1) {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(8)?;
        v = v.checked_mul(8)?.checked_add(u64::from(d))?;
        any = true;
    }
    any.then_some(v)
}

fn is_octal(n: Node<'_>) -> bool {
    kind(n) == "octal_integer_literal"
        || (kind(n) == "unary_expression"
            && n.field(Field::Operand)
                .is_some_and(|o| kind(o) == "octal_integer_literal"))
}

/// Whether the literal belongs to a table of octal values: its array initializer (or, for a
/// row such as `{0101, "A"}`, the enclosing initializer's first rows) holds several octal
/// literals. Bounded: looks at no more than a few dozen nodes.
fn in_octal_table<'t>(ctx: &Ctx<'t>, lit: Node<'t>) -> bool {
    let mut ancestors = ctx.ancestors(lit);
    let mut parent = ancestors.next();
    if parent.is_some_and(|p| kind(p) == "unary_expression") {
        parent = ancestors.next();
    }
    let Some(init) = parent.filter(|p| kind(*p) == "array_initializer") else {
        return false;
    };
    if named_children(init)
        .take(32)
        .filter(|c| is_octal(*c))
        .count()
        >= 3
    {
        return true;
    }
    // A row of a two-dimensional table: the other rows start with an octal literal too.
    let Some(outer) = ancestors.next().filter(|g| kind(*g) == "array_initializer") else {
        return false;
    };
    named_children(outer)
        .take(4)
        .filter(|row| {
            kind(*row) == "array_initializer" && named_children(*row).take(4).any(is_octal)
        })
        .count()
        >= 3
}

/// Unix file-type bits (`S_IFIFO` … `S_IFSOCK`) of a mode word.
const FILE_TYPES: [u64; 7] = [
    0o010000, 0o020000, 0o040000, 0o060000, 0o100000, 0o120000, 0o140000,
];

/// A permission-style literal: 3 or 4 digits after the leading zero (`0644`, `0755`, `01777`),
/// or a Unix mode word — a file type plus permission bits (`0100644`, `040755`) or the file-type
/// mask `0170000`.
fn is_mode_literal(text: &str, value: u64) -> bool {
    let digits = text
        .trim_end_matches(['l', 'L'])
        .get(1..)
        .unwrap_or("")
        .chars()
        .filter(|c| *c != '_')
        .count();
    (3..=4).contains(&digits)
        || (value <= 0o177777 && (FILE_TYPES.contains(&(value & 0o170000)) || value == 0o170000))
}

/// Whether `text` names a file mode or permission: one of its words (split at camelCase humps,
/// underscores and punctuation) is `mode`, `modes`, `perm`, `perms`, `permission(s)`, `umask`
/// or `chmod`.
fn names_permission(text: &str) -> bool {
    let mut word = String::new();
    let mut prev: Option<char> = None;
    let chars: Vec<char> = text.chars().collect();
    let check = |w: &mut String| {
        let hit = w == "mode"
            || w == "modes"
            || w == "perm"
            || w == "perms"
            || w.starts_with("permission")
            || w == "umask"
            || w == "chmod";
        w.clear();
        hit
    };
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_ascii_alphabetic() {
            if check(&mut word) {
                return true;
            }
            prev = None;
            continue;
        }
        let hump = c.is_ascii_uppercase()
            && prev.is_some_and(|p| {
                p.is_ascii_lowercase()
                    || (p.is_ascii_uppercase()
                        && chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase()))
            });
        if hump && check(&mut word) {
            return true;
        }
        word.push(c.to_ascii_lowercase());
        prev = Some(c);
    }
    check(&mut word)
}

/// Methods comparing their arguments: the other arguments tell what the literal is.
fn is_comparison(method: &str) -> bool {
    method.starts_with("assert") || matches!(method, "equals" | "isEqualTo")
}

/// Whether the literal is a file mode by its context: the variable, field or target it is
/// assigned to, the other operand of an expression it is part of, the method (or its receiver)
/// or constructor it is passed to, the other arguments of an assertion, or the method it is
/// returned from names a mode or permission ([`names_permission`]).
fn in_permission_context<'t>(ctx: &Ctx<'t>, lit: Node<'t>) -> bool {
    let named = |n: Option<Node<'t>>| n.is_some_and(|n| names_permission(ctx.text(n)));
    let mut child = lit;
    for a in ctx.ancestors(lit).take(16) {
        match kind(a) {
            "unary_expression"
            | "parenthesized_expression"
            | "cast_expression"
            | "ternary_expression"
            | "array_initializer"
            | "array_creation_expression"
            | "argument_list" => {}
            "binary_expression" => {
                let left = a.field(Field::Left);
                let other = if left.is_some_and(|l| l.id() == child.id()) {
                    a.field(Field::Right)
                } else {
                    left
                };
                if named(other) {
                    return true;
                }
            }
            "variable_declarator" => return named(a.field(Field::Name)),
            "assignment_expression" => return named(a.field(Field::Left)),
            "method_invocation" => {
                if kind(child) != "argument_list" {
                    return false;
                }
                let method = ctx.call_name(a).unwrap_or("");
                if names_permission(method) || named(a.field(Field::Object)) {
                    return true;
                }
                return is_comparison(method)
                    && syntax::arguments(a)
                        .filter(|x| {
                            x.id() != lit.id() && !x.byte_range().contains(&lit.start_byte())
                        })
                        .any(|x| names_permission(ctx.text(x)));
            }
            "object_creation_expression" => {
                return kind(child) == "argument_list" && named(a.field(Field::Type));
            }
            "return_statement" => {
                return named(ctx.enclosing_method(a).and_then(|m| m.field(Field::Name)));
            }
            _ => return false,
        }
        child = a;
    }
    false
}

/// Permission-shaped digits: `644`, `755`, `600` (an owner digit of 4 to 7, every digit one
/// of 0, 4, 5, 6, 7), optionally after a special-bits digit (`04755`, `01777`), or special bits
/// alone (`04000`).
fn is_permission_shaped(digits: &str) -> bool {
    let d = digits.trim_start_matches('0');
    let perms = |p: &[u8]| {
        p.len() == 3 && (b'4'..=b'7').contains(&p[0]) && p.iter().all(|c| b"04567".contains(c))
    };
    match d.len() {
        3 => perms(d.as_bytes()),
        // Special bits alone (`04000` setuid, `02000` setgid, `01000` sticky) or with
        // permissions (`04755`).
        4 => d.ends_with("000") || perms(&d.as_bytes()[1..]),
        _ => false,
    }
}

/// Whether the literal's shape shows that octal is meant: permission-shaped, a Unix mode word,
/// a mask of sevens, five or more significant digits, or digits grouped with underscores.
fn deliberate_shape(text: &str, value: u64) -> bool {
    let digits = text.trim_end_matches(['l', 'L']).get(1..).unwrap_or("");
    if digits.contains('_') {
        return true;
    }
    let significant = digits.trim_start_matches('0');
    significant.len() >= 5
        || (significant.len() >= 2 && significant.bytes().all(|c| c == b'7'))
        || is_permission_shaped(digits)
        || (value <= 0o177777 && FILE_TYPES.contains(&(value & 0o170000)))
}

/// Whether the literal is an operand of a bitwise or shift operator (`c & 077`, `x |= 0100`),
/// possibly through parentheses, casts or a unary sign.
fn in_bitwise_expression<'t>(ctx: &Ctx<'t>, lit: Node<'t>) -> bool {
    for a in ctx.ancestors(lit).take(6) {
        match kind(a) {
            "parenthesized_expression" | "cast_expression" | "unary_expression" => {}
            "binary_expression" => {
                return matches!(
                    syntax::operator_str(a),
                    "&" | "|" | "^" | "<<" | ">>" | ">>>"
                );
            }
            "assignment_expression" => {
                return matches!(
                    syntax::operator_str(a),
                    "&=" | "|=" | "^=" | "<<=" | ">>=" | ">>>="
                );
            }
            _ => return false,
        }
    }
    false
}

impl<'t> JavaRule<'t> for OctalLiteral<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["octal_integer_literal"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let text = ctx.text(node);
        // tree-sitter-java reads the float `07f` as `07` followed by an error: skip literals
        // glued to a letter or digit.
        let next = ctx.source().as_bytes().get(node.end_byte()).copied();
        if next.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.') {
            return;
        }
        let Some(value) = octal_value(text) else {
            return;
        };
        if value < 8 {
            return;
        }
        self.octal_count += 1;
        if !ctx.param_bool("report_tables") && in_octal_table(ctx, node) {
            return;
        }
        if deliberate_shape(text, value) || in_bitwise_expression(ctx, node) {
            return;
        }
        if (is_mode_literal(text, value) || value <= 0o7777) && in_permission_context(ctx, node) {
            return;
        }
        self.candidates.push((node, value));
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let convention = usize::try_from(ctx.param_i64("file_convention")).unwrap_or(0);
        if convention > 0 && self.octal_count >= convention {
            return;
        }
        for (node, value) in &self.candidates {
            let text = ctx.text(*node);
            ctx.emit(
                *node,
                format!(
                    "Write this value without the leading zero or in hexadecimal: `{text}` is \
                     octal ({value} in decimal)"
                ),
            )
            .value(*value as f64);
        }
    }
}
