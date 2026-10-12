//! `java/ignored-failure-return` — boolean/count results that report failure are discarded.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["api-misuse", "error-handling"],
    remediation_minutes: 5,
    description: r#"Detects calls whose return value is the only report of failure or of partial
work, used as statements:

* `java.io.File`: `delete`, `createNewFile`, `renameTo`, `setLastModified`,
  `setReadable`, `setWritable`, `setExecutable`, `setReadOnly` return `false` instead of
  throwing;
* `Lock.tryLock(..)` returns `false` when the lock was *not* acquired;
* `InputStream.read(buf..)` / `Reader.read(buf..)` return how many elements were actually
  read (possibly fewer than asked, or `-1` at the end), and `skip(n)` how many were skipped.

**Why it matters.** Ignoring these results turns failures into silent misbehaviour: a file
that was not deleted or renamed, a critical section entered without the lock, a buffer
processed as if it were full although only part of it was filled.

**How to fix.** Check the result — throw or log when it signals failure, loop until the
buffer is filled (or use `readNBytes` / `DataInputStream.readFully`), and only enter the
critical section when `tryLock()` returned `true`. For files, `java.nio.file.Files`
(`delete`, `createDirectories`, `move`) throws a descriptive exception instead.

```java
// Noncompliant
tmp.delete();
in.read(header);

// Compliant
Files.delete(tmp.toPath());
in.readNBytes(header, 0, header.length);
```

`File` operations are not reported in test code (cleanup there is best-effort by design).
`mkdir()` / `mkdirs()` are deliberately left out: they also return `false` when the directory
already exists, so ignoring the result is the usual "make sure it exists" idiom (a real
failure surfaces at the next file operation).
Only receivers whose type the resolver proves are reported: variables, parameters, and
fields declared in this file with such a type, or objects created in place (`new File(..)`)."#,
    ..RuleMeta::new(
        "java/ignored-failure-return",
        "java",
        "Failure-reporting result ignored",
    )
};

const FILE_METHODS: &[&str] = &[
    "delete",
    "createNewFile",
    "renameTo",
    "setLastModified",
    "setReadable",
    "setWritable",
    "setExecutable",
    "setReadOnly",
];

/// The API family a candidate call belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    File,
    Lock,
    Stream,
}

impl Family {
    fn owners(self) -> &'static [&'static str] {
        match self {
            Family::File => &["java.io.File"],
            Family::Lock => &["java.util.concurrent.locks.Lock"],
            Family::Stream => &["java.io.InputStream", "java.io.Reader"],
        }
    }

    /// Syntactic pre-check on the simple name of the receiver's declared type.
    fn plausible_type(self, simple: &str) -> bool {
        match self {
            Family::File => simple == "File",
            Family::Lock => simple.ends_with("Lock"),
            Family::Stream => simple.ends_with("InputStream") || simple.ends_with("Reader"),
        }
    }
}

/// Candidates are recorded during the walk with a syntactic index of declared types; `finish`
/// resolves only receivers declared with a plausible type (`File`, `…Lock`, `…InputStream`,
/// `…Reader`) or created in place (`new File(..)`).
#[derive(Default)]
pub struct IgnoredFailureReturn<'t> {
    /// Whether the file imports any of the types this rule is about.
    relevant: util::Gate,
    candidates: Vec<(Node<'t>, Node<'t>, Family)>,
}

impl<'t> IgnoredFailureReturn<'t> {
    fn collect_statement(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(call) = util::first_child(node) else {
            return;
        };
        if kind(call) != "method_invocation" {
            return;
        }
        let Some(name_node) = util::fld(call, F::Name) else {
            return;
        };
        let name = ctx.text(name_node);
        let family = match name {
            _ if FILE_METHODS.contains(&name) => {
                if ctx.is_test() {
                    return;
                }
                Family::File
            }
            "tryLock" => Family::Lock,
            "read" if util::arg_count(call) >= 1 => Family::Stream,
            "skip" if util::arg_count(call) == 1 => Family::Stream,
            _ => return,
        };
        if let Some(recv) = util::fld(call, F::Object) {
            self.candidates.push((name_node, recv, family));
        }
    }

    /// Cheap syntactic filter before the resolver.
    fn plausible(&self, ctx: &Ctx<'t>, recv: Node<'t>, family: Family) -> bool {
        let recv = syntax::strip_parens(recv);
        if kind(recv) == "object_creation_expression" {
            return util::fld(recv, F::Type)
                .is_some_and(|t| family.plausible_type(util::simple_type_name(ctx, t)));
        }
        let Some(name) = util::var_name(ctx, recv) else {
            return false;
        };
        ctx.declared_types()
            .types_of(name)
            .any(|t| family.plausible_type(util::simple_type_name(ctx, t)))
    }
}

impl<'t> JavaRule<'t> for IgnoredFailureReturn<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["expression_statement"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self.relevant.open_any(
            ctx,
            &[
                "java.io.File",
                "java.io.*",
                "InputStream;",
                "Reader;",
                "java.util.concurrent.locks",
            ],
        ) {
            return;
        }
        self.collect_statement(node, ctx);
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        let candidates = std::mem::take(&mut self.candidates);
        for (name_node, recv, family) in candidates {
            if !self.plausible(ctx, recv, family) {
                continue;
            }
            let Some(t) = ctx.type_of(recv) else {
                continue;
            };
            let Some(owner) = family.owners().iter().find(|o| ctx.is_subtype_of(&t, o)) else {
                continue;
            };
            let simple = owner.rsplit('.').next().unwrap_or(owner);
            let name = ctx.text(name_node);
            let message = match family {
                Family::Lock => "Check the result of `tryLock()`: when it returns false the lock is not held".to_string(),
                Family::Stream if name == "read" => format!("Check how many elements `{simple}.read(..)` returned: it may fill only part of the buffer, or return -1 at the end"),
                Family::Stream => format!("Check how many elements `{simple}.skip(..)` actually skipped; it may skip fewer than requested"),
                Family::File => format!("Check the result of `File.{name}()`: it returns false instead of throwing when the operation fails"),
            };
            ctx.emit(name_node, message);
        }
    }
}
