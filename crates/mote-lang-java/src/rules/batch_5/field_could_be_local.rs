//! `java/field-could-be-local` — a private field used by a single method, which always
//! assigns it before reading it.

use super::util::{self, Mods, Walk, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["design", "scope"],
    remediation_minutes: 5,
    description: r#"Detects private instance fields that only one method uses, and that this
method always assigns before reading.

**Why it matters.** Such a field carries no state from one call to the next: it is a local
variable in disguise. As a field it widens the scope for no reason, keeps the value (and
anything it references) alive as long as the object, and makes the method unsafe to run from
two threads at once.

**How to fix.** Declare the variable inside the method and remove the field.

```java
// Noncompliant
class Exporter {
    private int written;

    void export(List<Row> rows) {
        written = 0;
        for (Row r : rows) { out(r); written++; }
        log(written);
    }
}

// Compliant
class Exporter {
    void export(List<Row> rows) {
        int written = 0;
        for (Row r : rows) { out(r); written++; }
        log(written);
    }
}
```

The check is deliberately strict: the first use in the method must be a plain assignment
(`x = …` or `this.x = …`) that is a top-level statement of the method body and does not read
the field; every other mention of the name in the file must be in the same method, outside
lambdas and inner classes, and refer to this field (a parameter or local variable of the same
name in another method, or the name inside a string or a comment, does not count). Not
reported: fields with an initializer other than the default value (`= null`, `= false`, `= 0`,
which the first assignment overwrites anyway), `static`, `volatile`, or annotated fields,
fields of classes with Lombok annotations that generate code reading or setting fields (`@Data`,
`@Getter`, `@Builder`, … — a logger annotation such as `@Slf4j` is fine), fields used by
constructors or initializers, methods that call themselves, and set-up callbacks
(`@BeforeEach`, `@Before`, `@PostConstruct`, …, and JUnit 3's `setUp()`), where fixtures are kept in
fields by convention."#,
    ..RuleMeta::new(
        "java/field-could-be-local",
        "java",
        "Field could be a local variable",
    )
};

/// Life-cycle callbacks that set up fixtures (JUnit, TestNG, JMH, Jakarta).
const SETUP_ANNOTATIONS: &[&str] = &[
    "Before",
    "BeforeEach",
    "BeforeAll",
    "BeforeClass",
    "BeforeMethod",
    "BeforeTest",
    "BeforeSuite",
    "Setup",
    "PostConstruct",
];

struct Candidate<'t> {
    class: Node<'t>,
    name_node: Node<'t>,
    name: &'t str,
    declarator: Node<'t>,
    class_body: Node<'t>,
}

#[derive(Default)]
pub struct FieldCouldBeLocal<'t> {
    candidates: Vec<Candidate<'t>>,
}

/// How one whole-word occurrence of the field's name relates to the field.
enum Occurrence<'t> {
    /// Not the field: a comment, a string, a method name, or another variable of that name.
    Other,
    /// The field (`x` or `this.x`); the identifier node.
    Field(Node<'t>),
    /// Something the rule cannot rule out (`other.x`, a type name, an unresolved name).
    Unclear,
}

/// The node at the occurrence of the field's name at `at` (`root.descendant_for_byte_range`):
/// for an occurrence inside the field's class body, the descent from the root passes through the
/// body, so it starts there.
fn node_at<'t>(ctx: &Ctx<'t>, at: usize, c: &Candidate<'t>) -> Option<Node<'t>> {
    let end = at + c.name.len();
    let body = c.class_body;
    if body.start_byte() <= at && end <= body.end_byte() && at < end {
        body.descendant_for_byte_range(at, end)
    } else {
        ctx.root().descendant_for_byte_range(at, end)
    }
}

fn classify<'t>(ctx: &Ctx<'t>, at: usize, c: &Candidate<'t>) -> Occurrence<'t> {
    let Some(n) = node_at(ctx, at, c) else {
        return Occurrence::Unclear;
    };
    if syntax::is_comment(n) || matches!(kind(n), "string_fragment" | "string_literal") {
        return Occurrence::Other;
    }
    if kind(n) != "identifier" {
        return Occurrence::Unclear;
    }
    let Some(parent) = ctx.parent(n) else {
        return Occurrence::Unclear;
    };
    let is_name = util::fld(parent, F.name).is_some_and(|x| x.id() == n.id());
    match kind(parent) {
        "method_invocation" | "method_declaration" if is_name => return Occurrence::Other,
        // The name of another parameter or variable (a field of another class included).
        "formal_parameter" | "variable_declarator" | "catch_formal_parameter" if is_name => {
            return Occurrence::Other
        }
        "field_access" if util::fld(parent, F.field).is_some_and(|f| f.id() == n.id()) => {
            return if util::fld(parent, F.object).is_some_and(|o| kind(o) == "this") {
                Occurrence::Field(n)
            } else {
                Occurrence::Unclear
            };
        }
        _ => {}
    }
    match ctx.declaration_of(n) {
        Some(d) if d.kind == VarKind::Field && d.decl.id() == c.declarator.id() => {
            Occurrence::Field(n)
        }
        // Another variable, or a field of another class.
        Some(_) => Occurrence::Other,
        None => Occurrence::Unclear,
    }
}

/// Text-only test: the occurrence at `at` of `name` inside the member `src[start..end]` is a
/// plain read or write of a variable (not after a `.` other than `this.`, not a call, not inside
/// a string or comment on its line), and the member declares nothing of that name (no
/// `Type name` before `=`, `;`, `,`, `)`, `:`, and no `name ->` lambda; computed once per member
/// into `declares`). Then it can only be the field. `false` means "not sure".
fn plainly_the_field(
    ctx: &Ctx<'_>,
    at: usize,
    name: &str,
    (start, end): (usize, usize),
    declares: &mut Option<bool>,
) -> bool {
    let src = ctx.source();
    let b = src.as_bytes();
    let prev_non_ws = |mut i: usize| -> Option<usize> {
        while i > start {
            i -= 1;
            if !b[i].is_ascii_whitespace() {
                return Some(i);
            }
        }
        None
    };
    let next_non_ws = |mut i: usize| -> Option<u8> {
        while i < end {
            if !b[i].is_ascii_whitespace() {
                return Some(b[i]);
            }
            i += 1;
        }
        None
    };
    if let Some(p) = prev_non_ws(at) {
        if b[p] == b'.' && !src[..p].trim_end().ends_with("this") {
            return false;
        }
    }
    if next_non_ws(at + name.len()) == Some(b'(') {
        return false;
    }
    // Inside a string or a comment on this line (approximately): not sure.
    let line_start = src[start..at].rfind('\n').map_or(start, |i| start + i + 1);
    let head = &src[line_start..at];
    let quotes = head.bytes().filter(|&ch| ch == b'"').count() - head.matches("\\\"").count();
    if quotes % 2 == 1
        || head.contains("//")
        || head.contains("/*")
        || head.trim_start().starts_with('*')
        || head.contains('\'')
    {
        return false;
    }
    let is_word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$';
    let declared = *declares.get_or_insert_with(|| {
        util::source_words(ctx, name, start, end).any(|o| {
            let rest = src.get(o + name.len()..end).unwrap_or("").trim_start();
            if rest.starts_with("->") {
                return true;
            }
            let after = rest.bytes().next();
            if !matches!(after, Some(b'=' | b';' | b',' | b')' | b':')) || rest.starts_with("==") {
                return false;
            }
            let Some(p) = prev_non_ws(o) else {
                return false;
            };
            if !(is_word(b[p]) || b[p] == b'>' || b[p] == b']') {
                return false;
            }
            // The word before: a type name, not a keyword such as `return x;`.
            let mut q = p + 1;
            while q > start && is_word(b[q - 1]) {
                q -= 1;
            }
            !matches!(
                &src[q..=p],
                "return"
                    | "throw"
                    | "case"
                    | "else"
                    | "new"
                    | "yield"
                    | "assert"
                    | "do"
                    | "instanceof"
            )
        })
    });
    !declared
}

/// `null`, `false`, or a zero literal: the value the field has anyway.
fn is_default_value(ctx: &Ctx<'_>, value: Node<'_>) -> bool {
    let v = syntax::strip_parens(value);
    match kind(v) {
        "null_literal" | "false" => true,
        "decimal_integer_literal" => matches!(ctx.text(v), "0" | "0L" | "0l"),
        "decimal_floating_point_literal" => {
            let t = ctx.text(v).trim_end_matches(['f', 'F', 'd', 'D']);
            !t.is_empty() && t.bytes().all(|b| b == b'0' || b == b'.')
        }
        _ => false,
    }
}

/// The expression a field occurrence stands for (`x`, or `this.x`), when it refers to the
/// candidate field.
fn field_expr<'t>(ctx: &Ctx<'t>, ident: Node<'t>, c: &Candidate<'t>) -> Option<Node<'t>> {
    let parent = ctx.parent(ident)?;
    if kind(parent) == "field_access" {
        let obj = util::fld(parent, F.object)?;
        let field = util::fld(parent, F.field)?;
        return (field.id() == ident.id() && kind(obj) == "this").then_some(parent);
    }
    if kind(parent) == "method_invocation"
        && util::fld(parent, F.name).is_some_and(|n| n.id() == ident.id())
    {
        return None;
    }
    let d = ctx.declaration_of(ident)?;
    (d.kind == VarKind::Field && d.decl.id() == c.declarator.id()).then_some(ident)
}

/// The method holding `n`, provided it is a direct member of `class_body` and nothing in
/// between runs in another context (lambda, inner class).
fn owning_method<'t>(ctx: &Ctx<'t>, n: Node<'t>, class_body: Node<'t>) -> Option<Node<'t>> {
    let mut method: Option<Node<'t>> = None;
    for a in ctx.ancestors(n) {
        if let Some(m) = method {
            return (a.id() == class_body.id()).then_some(m);
        }
        match kind(a) {
            "method_declaration" => method = Some(a),
            "lambda_expression"
            | "class_body"
            | "constructor_declaration"
            | "static_initializer"
            | "field_declaration" => return None,
            _ => {}
        }
    }
    None
}

/// Whether `expr` is the target of a plain `=` assignment; returns the assignment.
fn plain_assignment_target<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<Node<'t>> {
    let parent = ctx.parent(expr)?;
    (kind(parent) == "assignment_expression"
        && syntax::operator_str(parent) == "="
        && util::fld(parent, F.left).is_some_and(|l| l.id() == expr.id()))
    .then_some(parent)
}

impl<'t> FieldCouldBeLocal<'t> {
    /// Pre-check: every use of the field lies in one and the same method of the class. Walks
    /// the whole-word occurrences of the name once; an occurrence counts as a use unless it is
    /// in a comment or string, a method name, or another variable of that name. A plain use
    /// (text test) is taken as is; only the unclear ones are resolved, and the rest of a method
    /// already known to use the field is skipped. `members` are the class's members as
    /// `(start, end, is_method)`.
    fn one_method_by_text(
        ctx: &Ctx<'t>,
        c: &Candidate<'t>,
        members: &[(usize, usize, bool)],
    ) -> bool {
        let src = ctx.source();
        let decl_at = c.name_node.start_byte();
        let mut only: Option<usize> = None;
        // `plainly_the_field`'s per-member answer, for the few members with occurrences (a
        // vector per member and candidate would be quadratic in a class of many fields).
        let mut declares: Vec<(usize, Option<bool>)> = Vec::new();
        for (count, at) in util::source_words(ctx, c.name, 0, src.len()).enumerate() {
            if at == decl_at {
                continue;
            }
            if count > 64 {
                return false;
            }
            // Members are in source order and do not overlap.
            let member = members
                .partition_point(|(s, _, _)| *s <= at)
                .checked_sub(1)
                .filter(|&i| at < members[i].1);
            if member.is_some() && member == only {
                continue;
            }
            let Some(i) = member else {
                // Outside the members (another class of the file): harmless unless it is this
                // field.
                match classify(ctx, at, c) {
                    Occurrence::Other => continue,
                    _ => return false,
                }
            };
            let slot = match declares.iter().position(|(m, _)| *m == i) {
                Some(k) => k,
                None => {
                    declares.push((i, None));
                    declares.len() - 1
                }
            };
            let plain = plainly_the_field(
                ctx,
                at,
                c.name,
                (members[i].0, members[i].1),
                &mut declares[slot].1,
            );
            if !plain {
                match classify(ctx, at, c) {
                    Occurrence::Other => continue,
                    Occurrence::Unclear => return false,
                    Occurrence::Field(_) => {}
                }
            }
            // A use in a constructor, an initializer or a nested class: not a local.
            if !members[i].2 || only.is_some_and(|o| o != i) {
                return false;
            }
            only = Some(i);
        }
        only.is_some()
    }

    fn analyze(ctx: &Ctx<'t>, c: &Candidate<'t>) -> Option<Node<'t>> {
        let src = ctx.source();
        let decl_at = c.name_node.start_byte();
        let mut exprs: Vec<Node<'t>> = Vec::new();
        let mut method: Option<Node<'t>> = None;
        for at in util::source_words(ctx, c.name, 0, src.len()) {
            if at == decl_at {
                continue;
            }
            if exprs.len() >= 64 {
                return None;
            }
            let n = match classify(ctx, at, c) {
                Occurrence::Other => continue,
                Occurrence::Unclear => node_at(ctx, at, c)?,
                Occurrence::Field(n) => n,
            };
            if kind(n) != "identifier" {
                return None;
            }
            let m = owning_method(ctx, n, c.class_body)?;
            if method.is_some_and(|x| x.id() != m.id()) {
                return None;
            }
            method = Some(m);
            exprs.push(field_expr(ctx, n, c)?);
        }
        let method = method?;
        // Fixture set-up callbacks conventionally initialize fields, even single-use ones
        // (annotated, or JUnit 3's `setUp()`).
        if Mods::of(method)
            .annotations()
            .any(|a| SETUP_ANNOTATIONS.contains(&syntax::annotation_name(a, src)))
            || syntax::name_of(method, src) == "setUp"
        {
            return None;
        }
        let body = util::fld(method, F.body)?;
        let first = *exprs.first()?;
        // The first use: `x = …;` directly in the method body, not reading `x`.
        let assign = plain_assignment_target(ctx, first)?;
        let stmt = ctx.parent(assign)?;
        if kind(stmt) != "expression_statement" || ctx.parent(stmt)?.id() != body.id() {
            return None;
        }
        let rhs = util::fld(assign, F.right)?;
        if util::occurs_in_source(ctx, c.name, rhs.start_byte(), rhs.end_byte()) {
            return None;
        }
        // At least one read afterwards (write-only fields are another rule's business).
        if !exprs[1..]
            .iter()
            .any(|e| plain_assignment_target(ctx, *e).is_none())
        {
            return None;
        }
        // Recursion would see the value of the outer call: a call of the method on `this`
        // (`name(..)`, `this.name(..)`) or a reference to it (`this::name`).
        let method_name = syntax::name_of(method, src);
        if util::occurs_in_source(ctx, method_name, body.start_byte(), body.end_byte())
            && util::visit(body, |n| match kind(n) {
                "method_invocation"
                    if util::call_name(ctx, n) == method_name
                        && util::fld(n, F.object).is_none_or(|o| kind(o) == "this") =>
                {
                    Walk::Stop
                }
                "method_reference" if ctx.text(n).ends_with(method_name) => Walk::Stop,
                _ => Walk::Continue,
            })
        {
            return None;
        }
        Some(method)
    }
}

impl<'t> JavaRule<'t> for FieldCouldBeLocal<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["field_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let mods = Mods::of(node);
        if !mods.has("private") || mods.has("static") || mods.has("volatile") || mods.annotated() {
            return;
        }
        let Some(class) = ctx.enclosing_class(node) else {
            return;
        };
        if !matches!(kind(class), "class_declaration" | "enum_declaration") {
            return;
        }
        let Some(class_body) = ctx.parent(node) else {
            return;
        };
        if kind(class_body) != "class_body" {
            return;
        }
        for d in syntax::declarators(node) {
            if util::fld(d, F.value).is_some_and(|v| !is_default_value(ctx, v)) {
                continue;
            }
            let Some(name_node) = util::fld(d, F.name) else {
                continue;
            };
            self.candidates.push(Candidate {
                name_node,
                name: ctx.text(name_node),
                declarator: d,
                class_body,
                class,
            });
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let mut members: Vec<(usize, usize, bool)> = Vec::new();
        let mut members_of: Option<usize> = None;
        for c in &self.candidates {
            if members_of != Some(c.class_body.id()) {
                members.clear();
                members.extend(ctx.members(c.class_body).map(|m| {
                    (
                        m.start_byte(),
                        m.end_byte(),
                        kind(m) == "method_declaration",
                    )
                }));
                members_of = Some(c.class_body.id());
            }
            if !Self::one_method_by_text(ctx, c, &members)
                || util::lombok_annotations(ctx, c.class)
                    .any(|a| !util::LOMBOK_LOGGERS.contains(&a))
            {
                continue;
            }
            let Some(method) = Self::analyze(ctx, c) else {
                continue;
            };
            let m = syntax::name_of(method, ctx.source());
            ctx.emit(
                c.name_node,
                format!(
                    "Replace field `{}` with a local variable of `{m}()`: only that method uses \
                     it, and always assigns it first",
                    c.name
                ),
            );
        }
    }
}
