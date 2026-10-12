//! `java/stream-missing-bulk-method` — an `InputStream` / `OutputStream` subclass that overrides
//! the single-byte `read()` / `write(int)` but not the array form.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Stable,
    tags: &["io", "performance"],
    remediation_minutes: 15,
    description: r#"Detects a class extending `InputStream`, `FilterInputStream`, `OutputStream` or
`FilterOutputStream` that overrides the single-byte `read()` or `write(int)` but not
`read(byte[], int, int)` or `write(byte[], int, int)`.

**Why it matters.** Almost every caller — buffered streams, readers, `transferTo`, `copy`
utilities — reads and writes whole arrays. `InputStream` and `OutputStream` implement the array
methods by calling the single-byte method once per byte: a virtual call, bounds checks and
often a lock for every byte, which makes such a stream orders of magnitude slower than it needs
to be. For the `Filter` variants it is worse than slow: `FilterInputStream.read(byte[], int,
int)` forwards to the wrapped stream directly, so bulk reads bypass the overridden `read()` and
whatever it does to the bytes; `FilterOutputStream.write(byte[], int, int)` does call
`write(int)`, one byte at a time.

**How to fix.** Override the array method as well, processing the whole range at once.

```java
// Noncompliant
class CountingInputStream extends FilterInputStream {
    @Override
    public int read() throws IOException {
        int b = super.read();
        if (b >= 0) count++;
        return b;
    }
}

// Compliant: also
    @Override
    public int read(byte[] buf, int off, int len) throws IOException {
        int n = super.read(buf, off, len);
        if (n > 0) count += n;
        return n;
    }
```

Not reported: abstract classes (a subclass may add the method), classes of other files in the
hierarchy (their methods are unknown), and test code."#,
    ..RuleMeta::new(
        "java/stream-missing-bulk-method",
        "java",
        "Stream overrides only its single-byte method",
    )
};

pub struct StreamMissingBulkMethod;

/// Parameter count of a method declaration.
fn param_count(m: Node<'_>) -> usize {
    syntax::parameters(m).count()
}

impl<'t> JavaRule<'t> for StreamMissingBulkMethod {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["class_declaration", "object_creation_expression"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let (superclass, body, report_at) = match kind(node) {
            "class_declaration" => {
                if syntax::has_modifier(node, "abstract") {
                    return;
                }
                let Some(sc) = node
                    .field(Field::Superclass)
                    .and_then(syntax::first_named_child)
                else {
                    return;
                };
                let Some(body) = node.field(Field::Body) else {
                    return;
                };
                let Some(name) = node.field(Field::Name) else {
                    return;
                };
                (sc, body, name)
            }
            _ => {
                let Some(body) = syntax::child_of_kind(node, "class_body") else {
                    return;
                };
                let Some(ty) = node.field(Field::Type) else {
                    return;
                };
                (ty, body, ty)
            }
        };
        let simple = util::last_segment(ctx.text(util::type_base(superclass)));
        let (single, input) = match simple {
            "InputStream" | "FilterInputStream" => ("read", true),
            "OutputStream" | "FilterOutputStream" => ("write", false),
            _ => return,
        };
        let Some(fqn) = util::type_fqn(ctx, superclass) else {
            return;
        };
        if !matches!(
            fqn.as_str(),
            "java.io.InputStream"
                | "java.io.FilterInputStream"
                | "java.io.OutputStream"
                | "java.io.FilterOutputStream"
        ) {
            return;
        }
        let src = ctx.source();
        let mut has_single = false;
        let mut has_bulk = false;
        for m in named_children(body) {
            if kind(m) != "method_declaration" || syntax::name_of(m, src) != single {
                continue;
            }
            match (param_count(m), input) {
                (0, true) | (1, false) => has_single = true,
                (3, _) => has_bulk = true,
                _ => {}
            }
        }
        if !has_single || has_bulk {
            return;
        }
        let what = if input {
            "read(byte[], int, int)"
        } else {
            "write(byte[], int, int)"
        };
        let why = if fqn == "java.io.FilterInputStream" {
            "bulk reads go straight to the wrapped stream and bypass the overridden read()"
        } else {
            "every bulk operation falls back to one call per byte"
        };
        ctx.emit(report_at, format!("Override `{what}` as well: {why}"));
    }
}
