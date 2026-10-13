//! `java/redundant-default-constructor` — an explicit constructor identical to the one the
//! compiler would generate.

use super::util::{self, Mods, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "redundant"],
    remediation_minutes: 1,
    description: r#"Detects a class whose only constructor takes no arguments, has an empty body
(or just `super();`), and has the same access as the class — exactly the default constructor
the compiler generates when none is written.

**Why it matters.** The constructor is dead weight: it has to be read and scrolled past, and
it looks as if it did something special.

**How to fix.** Delete it.

```java
// Noncompliant
public class Report {
    public Report() {
    }

    public String render() { ... }
}

// Compliant
public class Report {
    public String render() { ... }
}
```

Not reported: constructors with a comment (right before them, inside the body, or at the end of
their last line), which is often kept to explain why they exist — a comment separated from the
constructor by a blank line heads a section and does not count; annotated constructors and
those with a `throws` clause or type parameters; constructors whose access differs from the
class (e.g. a `private` constructor that prevents instantiation; a class declared in an
interface is implicitly `public`); enums and records; and
classes with other constructors. `java/empty-constructor` (off by default) covers empty
constructors in general.

**Fix available.** `mote fix` deletes the constructor; the compiler then adds one with the same
access, no parameters and the same implicit `super()` call. It leaves alone the constructor of
a class with an annotation other than `@Deprecated` or `@SuppressWarnings` (an annotation such
as Lombok's `@AllArgsConstructor` or `@Builder` generates a constructor, and the default one
then exists only because this one is written), one with a comment inside its declaration, and
that of a class whose other members are all static (there the written constructor says that
instances are meant)."#,
    fixable: true,
    ..RuleMeta::new(
        "java/redundant-default-constructor",
        "java",
        "Redundant default constructor",
    )
};

pub struct RedundantDefaultConstructor;

/// Whether every member of `body` but `ctor` is static and one of them is a method.
fn only_static_methods<'t>(ctx: &Ctx<'t>, body: Node<'t>, ctor: Node<'t>) -> bool {
    let mut methods = 0;
    for m in ctx.members(body) {
        if m.id() == ctor.id() {
            continue;
        }
        match kind(m) {
            "method_declaration" | "field_declaration" | "class_declaration"
                if Mods::of(m).has("static") =>
            {
                methods += usize::from(kind(m) == "method_declaration");
            }
            "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration"
            | "static_initializer" => {}
            _ => return false,
        }
    }
    methods > 0
}

/// Delete a member declared on lines of its own, with one blank line after it when it sits
/// between a blank line (or the opening brace) and that blank line.
fn delete_member(src: &str, member: Node<'_>) -> Option<Edit> {
    let (start, mut end) = fix::whole_line(src, member.start_byte(), member.end_byte())?;
    let before = src[..start].trim_end_matches([' ', '\t']);
    let prev_blank_or_open = before.ends_with('{')
        || before.ends_with("\n\n")
        || before.ends_with("\r\n\r\n")
        || before.is_empty();
    let rest = &src[end..];
    let next_line_end = rest.find('\n').map(|i| end + i + 1);
    if let Some(next_end) = next_line_end {
        if prev_blank_or_open && src[end..next_end].trim().is_empty() {
            end = next_end;
        }
    }
    Some(Edit::delete(start, end))
}

const ACCESS: &[&str] = &["public", "protected", "private"];

/// Whether only whitespace precedes `offset` on its line.
fn starts_line(src: &str, offset: usize) -> bool {
    let before = src.get(..offset).unwrap_or("");
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    before
        .get(line_start..)
        .is_some_and(|s| s.chars().all(char::is_whitespace))
}

fn access<'t>(mods: Mods<'t>) -> Option<&'static str> {
    ACCESS.iter().copied().find(|a| mods.has(a))
}

/// Whether a blank line separates the end of `before` from the start of the next member.
fn blank_line_between(src: &str, before_end: usize, next_start: usize) -> bool {
    let gap = src.get(before_end..next_start).unwrap_or("");
    gap.split('\n')
        .skip(1)
        .take(gap.matches('\n').count().saturating_sub(1))
        .any(|line| line.trim().is_empty())
}

impl<'t> JavaRule<'t> for RedundantDefaultConstructor {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(body) = syntax::type_body(node) else {
            return;
        };
        let mut ctor: Option<Node<'t>> = None;
        // The end of the comment that directly precedes the current member, if any.
        let mut prev_comment_end: Option<usize> = None;
        let mut ctor_after_comment = false;
        let mut prev_is_ctor = false;
        let mut trailing_comment = false;
        for c in syntax::children(body) {
            // `Report() {} // for JPA`: a comment on the line the constructor ends explains it.
            if prev_is_ctor && syntax::is_comment(c) && !starts_line(ctx.source(), c.start_byte()) {
                trailing_comment = true;
            }
            prev_is_ctor = kind(c) == "constructor_declaration";
            if kind(c) == "constructor_declaration" {
                if ctor.is_some() {
                    return;
                }
                ctor = Some(c);
                ctor_after_comment = prev_comment_end
                    .is_some_and(|end| !blank_line_between(ctx.source(), end, c.start_byte()));
            }
            // A comment on its own line(s) documents what follows (unless a blank line comes
            // first: then it heads a section); a trailing comment belongs to the code before it.
            prev_comment_end = (syntax::is_comment(c) && starts_line(ctx.source(), c.start_byte()))
                .then(|| c.end_byte());
        }
        let Some(ctor) = ctor else {
            return;
        };
        if ctor_after_comment || trailing_comment {
            return;
        }
        let mods = Mods::of(ctor);
        // Member classes of interfaces are implicitly public.
        let class_access = access(Mods::of(node)).or_else(|| {
            ctx.parent(node)
                .filter(|p| matches!(kind(*p), "interface_body" | "annotation_type_body"))
                .map(|_| "public")
        });
        if mods.annotated() || access(mods) != class_access {
            return;
        }
        if syntax::parameters(ctor).next().is_some()
            || named_children(ctor).any(|c| matches!(kind(c), "throws" | "type_parameters"))
        {
            return;
        }
        // A receiver parameter or anything else in the list.
        if util::fld(ctor, F.parameters).is_some_and(|p| p.named_child_count() > 0) {
            return;
        }
        let Some(cbody) = util::fld(ctor, F.body) else {
            return;
        };
        if syntax::block_has_comment(cbody) {
            return;
        }
        let mut stmts = named_children(cbody);
        match (stmts.next(), stmts.next()) {
            (None, _) => {}
            (Some(s), None)
                if kind(s) == "explicit_constructor_invocation"
                    && util::fld(s, F.constructor).is_some_and(|c| kind(c) == "super")
                    && util::arg_count(s) == 0
                    && util::fld(s, F.object).is_none() => {}
            _ => return,
        }
        let Some(name) = util::fld(ctor, F.name) else {
            return;
        };
        let emitted = ctx.emit(
            name,
            "Remove this constructor: the compiler generates the same default constructor",
        );
        // An annotation on the class may generate constructors (Lombok's `@AllArgsConstructor`,
        // `@Builder`): the default one then exists only because this one is written. A comment
        // inside the declaration (between its modifiers and its name) is not deleted with it.
        // In a class of static methods only, the written constructor is what keeps it from
        // reading as a utility class meant never to be instantiated: deleting it would turn the
        // finding into `java/utility-class-constructor`'s.
        let text = ctx.text(ctor);
        // `@Deprecated` and `@SuppressWarnings` generate nothing.
        let generator = Mods::of(node).annotations().any(|a| {
            !matches!(
                syntax::annotation_name(a, ctx.source()),
                "Deprecated" | "SuppressWarnings"
            )
        });
        if !generator
            && !text.contains("//")
            && !text.contains("/*")
            && !only_static_methods(ctx, body, ctor)
        {
            if let Some(edit) = delete_member(ctx.source(), ctor) {
                emitted.fix("Delete the constructor", vec![edit]);
            }
        }
    }
}
