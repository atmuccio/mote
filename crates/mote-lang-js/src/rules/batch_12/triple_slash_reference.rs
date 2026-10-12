//! `js/triple-slash-reference` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct TripleSlashReference;

pub static TRIPLE_SLASH_REFERENCE: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions", "typescript", "modules"],
    remediation_minutes: 2,
    params: &[
        ParamSpec {
            name: "path",
            default: ParamDefault::Str("never"),
            description: "`/// <reference path=\"…\" />`: `never` (report) or `always` (accept).",
        },
        ParamSpec {
            name: "types",
            default: ParamDefault::Str("prefer-import"),
            description: "`/// <reference types=\"…\" />`: `never` (report), `prefer-import` (report when the file also imports that package) or `always` (accept).",
        },
        ParamSpec {
            name: "lib",
            default: ParamDefault::Str("always"),
            description: "`/// <reference lib=\"…\" />`: `never` (report) or `always` (accept).",
        },
    ],
    description: r#"Flags triple-slash reference directives (`/// <reference path="…" />`, and
`/// <reference types="…" />` for a package the file also imports) in TypeScript source files.

**Why:** reference directives are the pre-module way of pulling declarations into a
compilation. A `path` reference makes the file depend on another file without an `import`, so
the dependency is invisible to bundlers, to "find usages" and to anyone reading the imports, and
it drags the referenced file's global declarations along. A `types` reference to a package the
file already imports is redundant: the import brings the types in.

**Fix:** use `import` (or `import type`) for what the file needs; list ambient type packages
in `tsconfig.json` (`types`, `include`) instead of in source files.

```ts
/// <reference path="./globals.ts" />     // flagged
/// <reference types="node" />            // flagged when the file also imports from 'node'
import { readFile } from 'node:fs/promises';

import type { Settings } from './globals'; // compliant
```

Not reported: directives marked `preserve="true"` (kept on purpose for declaration output),
`lib` references (there is no import for a built-in library; the `lib` parameter
can turn them on), `types` references to packages the file does not import (with the default
`prefer-import`), and declaration files (`.d.ts`, which are not analyzed). Only directives
before the first statement count; later ones are plain comments for the compiler. Only
TypeScript files are checked.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/triple-slash-reference",
        "js",
        "Triple-slash reference directives should be replaced by imports",
    )
};

/// `(attribute, value)` of a `/// <reference attr="value" />` comment.
fn parse_directive(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("///")?.trim_start();
    let rest = rest.strip_prefix("<reference")?;
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    let rest = rest.trim_start();
    let eq = rest.find('=')?;
    let attr = rest.get(..eq)?.trim();
    let after = rest.get(eq + 1..)?.trim_start();
    let quote = after.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let value_and_more = after.get(1..)?;
    let end = value_and_more.find(quote)?;
    Some((attr, value_and_more.get(..end)?))
}

/// The package part of an import source: `@scope/name/sub` → `@scope/name`, `name/sub` → `name`,
/// `node:fs` → `node`.
fn package_of(source: &str) -> &str {
    if source.starts_with("node:") {
        return "node";
    }
    let mut parts = source.splitn(3, '/');
    let first = parts.next().unwrap_or("");
    if first.starts_with('@') {
        match parts.next() {
            Some(second) => source
                .get(..first.len() + 1 + second.len())
                .unwrap_or(source),
            None => source,
        }
    } else {
        first
    }
}

impl JsRule for TripleSlashReference {
    fn meta(&self) -> &'static RuleMeta {
        &TRIPLE_SLASH_REFERENCE
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::Program]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::Program(program) = node.kind() else {
            return;
        };
        let first_statement = program
            .directives
            .first()
            .map(|d| d.span.start)
            .into_iter()
            .chain(program.body.first().map(|s| s.span().start))
            .min()
            .unwrap_or(program.span.end);
        let mut found: Vec<(Span, &str, &str)> = Vec::new();
        for comment in ctx.comments() {
            if comment.span.start >= first_statement {
                break;
            }
            if !comment.is_line() {
                continue;
            }
            let text = ctx.text(comment.span);
            // `preserve="true"` keeps the directive in emitted declaration files on purpose.
            if text.contains("preserve=\"true\"") || text.contains("preserve='true'") {
                continue;
            }
            if let Some((attr, value)) = parse_directive(text) {
                found.push((comment.span, attr, value));
            }
        }
        if found.is_empty() {
            return;
        }
        for (span, attr, value) in found {
            let setting = match attr {
                "path" => ctx.param_str("path"),
                "types" => ctx.param_str("types"),
                "lib" => ctx.param_str("lib"),
                _ => continue,
            };
            let report = match setting.trim() {
                "never" => true,
                "prefer-import" if attr == "types" => imports_package(program, value),
                _ => false,
            };
            if !report {
                continue;
            }
            let message = match attr {
                "path" if value.ends_with(".d.ts") => format!("Include '{value}' through the compiler options instead of a reference directive"),
                "path" => format!("Import what is needed from '{value}' instead of referencing the file"),
                "types" => format!("Remove the reference to the types of '{value}': import them instead"),
                _ => format!("Remove the reference to the '{value}' library: list it in the compiler options instead"),
            };
            ctx.emit(span, message);
        }
    }
}

fn imports_package(program: &Program<'_>, package: &str) -> bool {
    program.body.iter().any(|stmt| match stmt {
        Statement::ImportDeclaration(import) => package_of(import.source.value.as_str()) == package,
        Statement::TSImportEqualsDeclaration(import) => match &import.module_reference {
            TSModuleReference::ExternalModuleReference(ext) => {
                package_of(ext.expression.value.as_str()) == package
            }
            _ => false,
        },
        _ => false,
    })
}
