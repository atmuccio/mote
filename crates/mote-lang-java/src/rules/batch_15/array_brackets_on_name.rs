//! `java/array-brackets-on-name` — `String args[]` or `int values()[]`: array brackets written
//! after the name instead of on the type.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "arrays"],
    remediation_minutes: 1,
    description: r#"Detects array brackets placed after a variable, parameter or field name
(`String args[]`) or after a method's parameter list (`int values()[]`) instead of on the type.

**Why it matters.** The brackets are part of the type. Written after the name, they split the
type in two: a reader scanning the left column sees `String` where the variable is a
`String[]`, and `int[] grid[]` is a two-dimensional array whose dimensions sit on both sides of
the name. The trailing form exists for compatibility with C; Java code reads more easily when
the whole type is written in one place.

**How to fix.** Move the brackets to the type.

```java
// Noncompliant
public static void main(String args[]) { … }
private int counts[];
int values()[] { … }

// Compliant
public static void main(String[] args) { … }
private int[] counts;
int[] values() { … }
```

Covered: fields, local variables, parameters (record components included), methods and
annotation elements. The rule is off by default: it is part of the `conventions` profile.

**Fix available.** `mote fix` moves the brackets to the end of the type (`String args[]` →
`String[] args`, `int values()[]` → `int[] values()`), which declares the same type. It
leaves alone a declaration of several variables (`int a[], b;`: the brackets belong to `a`
only) and brackets carrying annotations (their order would matter)."#,
    fixable: true,
    ..RuleMeta::new(
        "java/array-brackets-on-name",
        "java",
        "Array brackets after the name instead of on the type",
    )
};

pub struct ArrayBracketsOnName;

impl<'t> JavaRule<'t> for ArrayBracketsOnName {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        // `dimensions` nodes are rare (array types and these declarations): far fewer calls
        // than subscribing to every declarator.
        &["dimensions"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(decl) = ctx.parent(node) else {
            return;
        };
        let on_return_type = match kind(decl) {
            "variable_declarator" | "formal_parameter" => false,
            "method_declaration" | "annotation_type_element_declaration" => true,
            _ => return,
        };
        if decl.field(Field::Dimensions) != Some(node) {
            return;
        }
        let name = ctx.name_of(decl);
        let message = if on_return_type {
            format!("Move `{}` to the return type of `{name}`", ctx.text(node))
        } else {
            format!(
                "Move `{}` from the name `{name}` to its type",
                ctx.text(node)
            )
        };
        let emitted = ctx.emit(node, message);
        if let Some(edits) = move_brackets(ctx, decl, node) {
            emitted.fix("Move the brackets to the type", edits);
        }
    }
}

/// The edits moving `dims` (the brackets after the name or parameter list of `decl`) to the end
/// of the declared type, when that is the same type: plain `[]` pairs, on a declaration of one
/// variable, whose type carries no annotation.
fn move_brackets<'t>(ctx: &Ctx<'t>, decl: Node<'t>, dims: Node<'t>) -> Option<Vec<Edit>> {
    let src = ctx.source();
    let pairs = ctx
        .text(dims)
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .count();
    let plain = ctx
        .text(dims)
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect::<Vec<u8>>()
        .chunks(2)
        .all(|c| c == b"[]");
    if pairs == 0 || pairs % 2 != 0 || !plain {
        return None;
    }
    let ty = match kind(decl) {
        "variable_declarator" => {
            let owner = ctx.parent(decl)?;
            let declarators = named_children(owner)
                .filter(|c| kind(*c) == "variable_declarator")
                .count();
            if declarators != 1 {
                return None;
            }
            owner.field(Field::Type)?
        }
        _ => decl.field(Field::Type)?,
    };
    if ctx.text(ty).contains('@') || ctx.text(ty).contains("/*") || ctx.text(ty).contains("//") {
        return None;
    }
    // Delete the brackets with the blanks before them (not a comment).
    let before = dims.prev_sibling()?;
    let gap = src.get(before.end_byte()..dims.start_byte())?;
    let start = if gap.bytes().all(|b| b.is_ascii_whitespace()) {
        before.end_byte()
    } else {
        dims.start_byte()
    };
    let brackets = "[]".repeat(pairs / 2);
    Some(vec![
        Edit::insert(ty.end_byte(), brackets),
        Edit::delete(start, dims.end_byte()),
    ])
}
