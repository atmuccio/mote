//! `java/modifier-order` — modifiers written out of the usual order (`final static public`).

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "readability"],
    params: &[ParamSpec {
        name: "type_use_annotations",
        default: ParamDefault::Bool(false),
        description: "Also report a nullness type annotation of JSpecify or the Checker \
                      Framework written before the keyword modifiers (`@Nullable public String \
                      name()`) instead of right before the type it annotates.",
    }],
    remediation_minutes: 1,
    description: r#"Detects keyword modifiers written out of the customary order, which is the
order the Java Language Specification lists them in: `public` / `protected` / `private`,
`abstract`, `default`, `static`, `final`, `sealed` / `non-sealed`, `transient`, `volatile`,
`synchronized`, `native`, `strictfp`.

**Why it matters.** The compiler accepts any order, but readers do not scan modifiers one by
one: they expect the visibility first and `static final` together. `final static public`
or `static public` slows every reader down a little and makes searches such as
`public static final` miss the declaration.

**How to fix.** Reorder the keywords; annotations on the declaration stay in front of them.

```java
// Noncompliant
final static public int LIMIT = 10;
synchronized public void reset() { … }

// Compliant
public static final int LIMIT = 10;
public synchronized void reset() { … }
```

With the parameter `type_use_annotations`, the rule also reports a nullness annotation that
targets types (JSpecify's or the Checker Framework's `@Nullable` / `@NonNull`, recognized by
their imports) placed before the keyword modifiers: such an annotation describes the type, so it
is written right before it (`public @Nullable String name()`). Annotations in other positions
are never reported. The rule is off by default: it is part of the `conventions` profile.

**Fix available.** `mote fix` puts the keyword modifiers in the order above, each keyword
moving into the place of another so that annotations and comments stay where they are. The
order of modifiers means nothing to the compiler. Nullness annotations reported with
`type_use_annotations` are not moved."#,
    fixable: true,
    ..RuleMeta::new(
        "java/modifier-order",
        "java",
        "Modifiers out of the usual order",
    )
};

/// Keyword modifiers in their customary order.
const ORDER: [&str; 14] = [
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "final",
    "sealed",
    "non-sealed",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];

/// Nullness annotations that target types (type-use annotations).
const TYPE_USE_ANNOTATIONS: &[&str] = &[
    "org.jspecify.annotations.Nullable",
    "org.jspecify.annotations.NonNull",
    "org.checkerframework.checker.nullness.qual.Nullable",
    "org.checkerframework.checker.nullness.qual.NonNull",
    "org.checkerframework.checker.nullness.qual.MonotonicNonNull",
    "org.checkerframework.checker.nullness.qual.PolyNull",
];

fn rank(kw: &str) -> Option<usize> {
    ORDER.iter().position(|k| *k == kw)
}

pub struct ModifierOrder;

impl<'t> JavaRule<'t> for ModifierOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["modifiers"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        // A single modifier or annotation cannot be out of order.
        if node.child_count() < 2 {
            return;
        }
        let mut max = 0;
        let mut seen = 0;
        let mut misplaced = None;
        for c in syntax::modifier_keywords(node) {
            let Some(r) = rank(kind(c)) else {
                continue;
            };
            seen += 1;
            if seen > 1 && r < max && misplaced.is_none() {
                misplaced = Some(c);
            }
            max = max.max(r);
        }
        if let Some(at) = misplaced {
            let mut keywords: Vec<usize> = syntax::modifier_keywords(node)
                .filter_map(|c| rank(kind(c)))
                .collect();
            keywords.sort_unstable();
            let order: Vec<&str> = keywords.iter().map(|&r| ORDER[r]).collect();
            // The ranked keywords, in source order, each replaced by the keyword its slot gets.
            let slots: Vec<Node<'t>> = syntax::modifier_keywords(node)
                .filter(|c| rank(kind(*c)).is_some())
                .collect();
            let edits: Vec<Edit> = slots
                .iter()
                .zip(&order)
                .filter(|(slot, want)| ctx.text(**slot) != **want)
                .map(|(slot, want)| Edit::replace(slot.start_byte(), slot.end_byte(), *want))
                .collect();
            let emitted = ctx.emit(at, format!("Order the modifiers as `{}`", order.join(" ")));
            if slots.len() == order.len() && !edits.is_empty() {
                emitted.fix("Reorder the modifiers", edits);
            }
        }
        if seen > 0 && ctx.param_bool("type_use_annotations") {
            self.check_type_use(node, ctx);
        }
    }
}

impl ModifierOrder {
    /// A type-use nullness annotation followed by a keyword modifier.
    fn check_type_use<'t>(&self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.parent(node).is_some_and(|p| {
            matches!(
                kind(p),
                "method_declaration" | "field_declaration" | "local_variable_declaration"
            )
        }) {
            return;
        }
        let mut pending: Option<Node<'t>> = None;
        for c in syntax::children(node) {
            let keyword = syntax::modifier_keyword(c);
            if keyword.is_none() && c.is_named() {
                if pending.is_none()
                    && matches!(kind(c), "marker_annotation" | "annotation")
                    && matches!(
                        syntax::annotation_name(c, ctx.source()),
                        "Nullable" | "NonNull" | "MonotonicNonNull" | "PolyNull"
                    )
                    && util::annotation_is(ctx, c, TYPE_USE_ANNOTATIONS)
                {
                    pending = Some(c);
                }
                continue;
            }
            if let (Some(ann), Some(kw)) = (pending, keyword) {
                if rank(kind(kw)).is_some() {
                    ctx.emit(
                        ann,
                        format!(
                            "Move `{}` after the modifiers, right before the type it annotates",
                            ctx.text(ann)
                        ),
                    );
                    return;
                }
            }
        }
    }
}
