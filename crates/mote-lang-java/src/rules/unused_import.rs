//! `java/unused-import` — a file-level rule: collects uses during the walk, reports in `finish`.

use crate::prelude::*;
use rustc_hash::FxHashMap;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["imports", "clutter"],
    remediation_minutes: 1,
    description: r#"Detects imports that serve no purpose:

* single-type imports whose name is never used in the file (Javadoc references such as
  `{@link Foo}`, `@see Foo`, and `@throws FooException` count as uses);
* the same import written twice;
* imports of `java.lang` classes, which are always visible;
* imports of classes from the file's own package.

**Why it matters.** Dead imports are noise: they suggest dependencies that don't exist, cause
merge conflicts, and hide the imports that matter.

**How to fix.** Delete the import (most IDEs can "organize imports" automatically).

```java
// Noncompliant
import java.util.List;      // never used
import java.lang.String;    // implicit

// Compliant
import java.util.Map;       // used below
```

Wildcard imports are never reported as unused."#,
    ..RuleMeta::new("java/unused-import", "java", "Unused or redundant import")
};

/// Per-file state: simple names of single-type imports → whether they are used.
#[derive(Default)]
pub struct UnusedImport<'t> {
    initialized: bool,
    wanted: FxHashMap<&'t str, bool>,
    /// Number of names in `wanted` not marked used yet: at zero, nothing is left to look for.
    unused: usize,
    /// Bit `min(len, 63)` for the length of every name in `wanted`: most identifiers are
    /// rejected without hashing.
    lengths: u64,
}

impl<'t> UnusedImport<'t> {
    fn init(&mut self, ctx: &Ctx<'t>) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        for imp in ctx.imports().iter().filter(|i| !i.is_wildcard) {
            self.wanted.insert(imp.simple_name(), false);
        }
        self.unused = self.wanted.len();
        for name in self.wanted.keys() {
            self.lengths |= 1 << name.len().min(63);
        }
    }

    fn mark(&mut self, name: &str) {
        if self.lengths & (1 << name.len().min(63)) == 0 {
            return;
        }
        if let Some(used) = self.wanted.get_mut(name) {
            if !*used {
                *used = true;
                self.unused -= 1;
            }
        }
    }

    /// Mark type names referenced from Javadoc tags.
    fn scan_javadoc(&mut self, text: &'t str) {
        const TAGS: &[&str] = &[
            "{@link ",
            "{@linkplain ",
            "{@value ",
            "@see ",
            "@throws ",
            "@exception ",
        ];
        for tag in TAGS {
            let mut rest = text;
            while let Some(pos) = rest.find(tag) {
                rest = &rest[pos + tag.len()..];
                let reference = rest.trim_start();
                let end = reference
                    .find(|c: char| c.is_whitespace() || c == '}')
                    .unwrap_or(reference.len());
                let reference = &reference[..end];
                // `Type#member(ParamType, Other)`: the type and every parameter type count.
                let (ty, params) = match reference.split_once('(') {
                    Some((head, tail)) => (head, Some(tail)),
                    None => (reference, None),
                };
                let ty = ty.split('#').next().unwrap_or("");
                if let Some(first) = ty.split('.').next() {
                    self.mark(first);
                }
                if let Some(params) = params {
                    // Parameters may continue past the whitespace cut; re-read up to `)`.
                    let full = &rest[rest.find('(').map(|i| i + 1).unwrap_or(0)..];
                    let inner = full.split(')').next().unwrap_or(params);
                    for p in inner.split(',') {
                        let p = p.trim().split(['<', '[', ' ', '.']).next().unwrap_or("");
                        self.mark(p);
                    }
                }
            }
        }
    }
}

impl<'t> JavaRule<'t> for UnusedImport<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["identifier", "type_identifier", "block_comment"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        self.init(ctx);
        // Done once every import is used (also when there is none).
        if self.unused == 0 || node.start_byte() < ctx.header_end() {
            return;
        }
        if kind(node) == "block_comment" {
            let text = ctx.text(node);
            if text.starts_with("/**") {
                self.scan_javadoc(text);
            }
        } else {
            self.mark(ctx.text(node));
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        self.init(ctx);
        let package = ctx.package_name();
        let mut seen: Vec<(&str, bool, bool)> = Vec::new();
        for imp in ctx.imports() {
            let key = (imp.path, imp.is_static, imp.is_wildcard);
            if imp.path.is_empty() {
                continue;
            }
            if seen.contains(&key) {
                ctx.emit(
                    imp.node,
                    format!("Remove this duplicate import of `{}`", imp.path),
                );
                continue;
            }
            seen.push(key);
            if !imp.is_static {
                if imp.qualifier() == "java.lang" || (imp.is_wildcard && imp.path == "java.lang") {
                    ctx.emit(
                        imp.node,
                        format!(
                            "Remove this import of `{}`: java.lang is imported implicitly",
                            imp.path
                        ),
                    );
                    continue;
                }
                if package.is_some_and(|p| p == imp.qualifier()) {
                    ctx.emit(
                        imp.node,
                        format!(
                            "Remove this import of `{}` from the file's own package",
                            imp.path
                        ),
                    );
                    continue;
                }
            }
            if !imp.is_wildcard && self.wanted.get(imp.simple_name()) == Some(&false) {
                ctx.emit(
                    imp.node,
                    format!("Remove this unused import of `{}`", imp.path),
                );
            }
        }
    }
}
