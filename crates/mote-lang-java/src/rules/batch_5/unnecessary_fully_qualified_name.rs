//! `java/unnecessary-fully-qualified-name` — `java.util.List` where `List` already means the
//! same type.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["clutter", "imports"],
    remediation_minutes: 1,
    params: &[ParamSpec {
        name: "static_imports_under_supertypes",
        default: ParamDefault::Bool(false),
        description: "Also report class-qualified static members inside classes that extend or implement a type declared elsewhere (such a supertype could declare a member of the same name, which the bare name would then mean).",
    }],
    description: r#"Detects fully qualified type names (`java.util.List`, `java.lang.String`,
`com.acme.model.Order`) where the simple name already refers to the same type — because it is
imported, lives in `java.lang`, or belongs to the file's own package.

**Why it matters.** The package prefix adds nothing but length. It makes readers wonder
whether a *different* `List` is meant, and it often survives from an automated refactoring or
a copy-paste.

**How to fix.** Use the simple name.

```java
// Noncompliant
import java.util.List;

class Report {
    java.util.List<String> lines = new java.util.ArrayList<>();
    java.lang.String title;
}

// Compliant
import java.util.ArrayList;
import java.util.List;

class Report {
    List<String> lines = new ArrayList<>();
    String title;
}
```

Also detects a class name in front of a static member that the file already imports
with a single static import from that same class: with `import static
org.apache.commons.lang3.StringUtils.isBlank;` in place, `StringUtils.isBlank(name)` can be
`isBlank(name)`. Mixing both spellings of the same call in one file is noise.

Covers type names, annotation names, and qualified static calls or constants
(`java.util.Collections.emptyList()`). For package prefixes (for `java.util.Map.Entry`,
whether `java.util.` can go), the finding is reported only when the simple name provably
resolves to the same type at that position through a single-type import, the file's own
package, or `java.lang` — a clash with another import or with a type of this file keeps the
qualified name. On-demand (`*`) imports don't count (a type of the same package in another
file would win), and `java.lang` types are left alone in files that have any, since they
could make the simple name ambiguous. For class qualifiers, the member must be imported by
exactly one single static import, from the class the qualifier names; the qualifier is kept
when the bare name could mean something else: a method or variable of the same name declared
anywhere in the file, a method every class inherits (`toString`, `equals`, …), members of an
enclosing class that extends or implements another type (it may inherit a member of that
name; `static_imports_under_supertypes` reports them anyway), or a call with explicit type
arguments (`Util.<T>cast(x)` has no unqualified form). Qualifiers naming the class a member belongs to without a static import (`Math.max`,
`Order.DEFAULT` inside `Order`) are a common readable style and are not reported."#,
    ..RuleMeta::new(
        "java/unnecessary-fully-qualified-name",
        "java",
        "Unnecessary fully qualified name",
    )
};

/// Methods every class has (`Object`'s), and the implicit members of enums and records: a
/// bare call of one of these names never reaches a static import.
const INHERITED_EVERYWHERE: &[&str] = &[
    "equals",
    "hashCode",
    "toString",
    "getClass",
    "notify",
    "notifyAll",
    "wait",
    "clone",
    "finalize",
    "values",
    "valueOf",
    "name",
    "ordinal",
    "compareTo",
    "getDeclaringClass",
    "describeConstable",
];

/// A class-qualified static member (`StringUtils.isBlank(..)`, `Limits.MAX`) whose member the
/// file also imports statically; checked in `finish`, when every declaration has been seen.
struct StaticCandidate<'t> {
    node: Node<'t>,
    qualifier: Node<'t>,
    member: &'t str,
    class: &'t str,
    is_call: bool,
}

#[derive(Default)]
pub struct UnnecessaryFullyQualifiedName<'t> {
    /// `(member, class)` of every single static import; computed on first use.
    statics: Option<Vec<(&'t str, &'t str)>>,
    candidates: Vec<StaticCandidate<'t>>,
    /// Names of the methods and enum constants declared in this file.
    names: Vec<&'t str>,
}

impl<'t> UnnecessaryFullyQualifiedName<'t> {
    /// Collects `Type.member` when `member` is imported by a single static import.
    fn static_member(&mut self, node: Node<'t>, ctx: &Ctx<'t>, is_call: bool) {
        let statics = self.statics.get_or_insert_with(|| {
            ctx.imports()
                .iter()
                .filter(|i| i.is_static && !i.is_wildcard)
                .filter_map(|i| {
                    let (class, member) = i.path.rsplit_once('.')?;
                    Some((member, class))
                })
                .collect()
        });
        if statics.is_empty() {
            return;
        }
        let member = if is_call {
            match ctx.call_name(node) {
                Some(m) => m,
                None => return,
            }
        } else {
            // Constants only (`MAX_SIZE`): a lambda or catch parameter never has such a name.
            match util::fld(node, F.field) {
                Some(f) if kind(f) == "identifier" && util::is_constant_name(ctx.text(f)) => {
                    ctx.text(f)
                }
                _ => return,
            }
        };
        let mut classes = statics
            .iter()
            .filter(|(m, _)| *m == member)
            .map(|(_, c)| *c);
        let Some(class) = classes.next() else {
            return;
        };
        if classes.any(|c| c != class) || INHERITED_EVERYWHERE.contains(&member) {
            return;
        }
        let Some(qualifier) = util::fld(node, F.object) else {
            return;
        };
        if kind(qualifier) != "identifier"
            || !ctx
                .text(qualifier)
                .starts_with(|c: char| c.is_ascii_uppercase())
            || node.start_byte() < ctx.header_end()
        {
            return;
        }
        // `Util.<T>cast(x)`: explicit type arguments need the qualifier.
        if is_call && node.field(Field::TypeArguments).is_some() {
            return;
        }
        self.candidates.push(StaticCandidate {
            node,
            qualifier,
            member,
            class,
            is_call,
        });
    }

    /// Whether the bare `member` provably means the statically imported member at `c`.
    fn bare_name_is_import(&self, ctx: &Ctx<'t>, c: &StaticCandidate<'t>) -> bool {
        if ctx.type_name_of(c.qualifier).as_deref() != Some(c.class) {
            return false;
        }
        // A method or variable of that name declared anywhere in the file could shadow it.
        if self.names.contains(&c.member)
            || ctx.declared_types().types_of(c.member).next().is_some()
        {
            return false;
        }
        // So could a member inherited by an enclosing class from a supertype.
        let relaxed = ctx.param_bool("static_imports_under_supertypes");
        !ctx.ancestors(c.node).any(|a| match kind(a) {
            "class_declaration" | "enum_declaration" => {
                !relaxed
                    && (a.field(Field::Superclass).is_some()
                        || a.field(Field::Interfaces).is_some())
            }
            // Components are fields and accessor methods.
            "record_declaration" => true,
            "interface_declaration" => named_children(a).any(|ch| kind(ch) == "extends_interfaces"),
            // Anonymous classes always have a supertype.
            "object_creation_expression" | "enum_constant" => {
                named_children(a).any(|ch| kind(ch) == "class_body")
            }
            _ => false,
        })
    }
}

/// `pkg.sub.Type`: every segment but the last starts with a lowercase letter, the last with an
/// uppercase one. Returns the simple name.
fn package_qualified(text: &str) -> Option<&str> {
    let (pkg, simple) = text.rsplit_once('.')?;
    if !simple.starts_with(|c: char| c.is_ascii_uppercase())
        || !simple
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$')
    {
        return None;
    }
    let ok = pkg.split('.').all(|seg| {
        seg.starts_with(|c: char| c.is_ascii_lowercase())
            && seg
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$')
    });
    ok.then_some(simple)
}

impl<'t> JavaRule<'t> for UnnecessaryFullyQualifiedName<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "scoped_type_identifier",
            "scoped_identifier",
            "field_access",
            "method_invocation",
            "method_declaration",
            "enum_constant",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        match kind(node) {
            "method_invocation" => return self.static_member(node, ctx, true),
            "method_declaration" | "enum_constant" => {
                if let Some(n) = ctx.name_node(node) {
                    self.names.push(ctx.text(n));
                }
                return;
            }
            "field_access" => self.static_member(node, ctx, false),
            _ => {}
        }
        if node.start_byte() < ctx.header_end() {
            return;
        }
        let text = ctx.text(node);
        // Cheap shape test on the text first: `<lowercase…>.<Uppercase>`, short, no spaces.
        if text.len() > 160 || !text.starts_with(|c: char| c.is_ascii_lowercase()) {
            return;
        }
        let Some(simple) = package_qualified(text) else {
            return;
        };
        let k = kind(node);
        if k == "field_access"
            && !util::fld(node, F.object)
                .is_some_and(|o| matches!(kind(o), "identifier" | "field_access"))
        {
            return;
        }
        if k == "scoped_identifier" {
            // Only annotation names (`@java.lang.Deprecated`); other scoped identifiers are
            // package, import, and module names.
            let Some(parent) = ctx.parent(node) else {
                return;
            };
            if !matches!(kind(parent), "annotation" | "marker_annotation") {
                return;
            }
        }
        let full = if k == "field_access" {
            ctx.type_name_of(node)
        } else {
            ctx.resolve_type_name(text, node)
        };
        let Some(full) = full else {
            return;
        };
        if full != text {
            return;
        }
        // How the simple name reaches this type. An on-demand import is not enough: a type of
        // the same package declared in another file would take precedence.
        let package = text.rsplit_once('.').map_or("", |(p, _)| p);
        let single_import = ctx
            .imports()
            .iter()
            .any(|i| !i.is_static && !i.is_wildcard && i.path == text);
        let same_package = ctx.package_name() == Some(package);
        let java_lang =
            package == "java.lang" && !ctx.imports().iter().any(|i| i.is_wildcard && !i.is_static);
        if !(single_import || same_package || java_lang) {
            return;
        }
        if ctx.resolve_type_name(simple, node).as_deref() != Some(text) {
            return;
        }
        ctx.emit(
            node,
            format!("Drop the package: `{simple}` already refers to `{text}` here"),
        );
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        for c in &self.candidates {
            if !self.bare_name_is_import(ctx, c) {
                continue;
            }
            let q = ctx.text(c.qualifier);
            let what = if c.is_call { "method" } else { "constant" };
            ctx.emit(
                c.qualifier,
                format!(
                    "Drop the qualifier `{q}.`: the {what} `{}` is already statically imported from `{q}`",
                    c.member
                ),
            );
        }
    }
}
