//! `js/no-self-import`

use crate::prelude::*;

pub struct NoSelfImport;

pub static NO_SELF_IMPORT: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["modules", "suspicious"],
    remediation_minutes: 5,
    description: r#"Flags an `import` or `export … from` whose relative path points back at the file itself:
`import { helper } from './utils'` inside `utils.ts`.

**Why:** a module importing itself never gets anything it does not already have. At best the
import is dead weight; at worst the imported bindings are read before the module has finished
initializing and are `undefined` (or throw in the temporal dead zone). It is usually a wrong path —
the import was copied from a sibling file, or the file was renamed.

**Fix:** remove the import and use the local declarations directly, or correct the path to the
module that was meant.

```js
// file: src/format.js
import { pad } from './format';     // flagged: imports itself
import { pad } from './strings';    // compliant
```

Only relative specifiers (`./…`, `../…`) are checked, with or without a JavaScript/TypeScript
extension; `./dir` and `./dir/index` resolve to `dir/index.*`.
"#,
    ..RuleMeta::new(
        "js/no-self-import",
        "js",
        "A module should not import itself",
    )
};

impl JsRule for NoSelfImport {
    fn meta(&self) -> &'static RuleMeta {
        &NO_SELF_IMPORT
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::ImportDeclaration,
            AstType::ExportNamedDeclaration,
            AstType::ExportAllDeclaration,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let source = match node.kind() {
            AstKind::ImportDeclaration(d) => &d.source,
            AstKind::ExportNamedDeclaration(d) => match &d.source {
                Some(s) => s,
                None => return,
            },
            AstKind::ExportAllDeclaration(d) => &d.source,
            _ => return,
        };
        let spec = source.value.as_str();
        if !(spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == "..") {
            return;
        }
        let path = ctx.path();
        let (dir, file) = match path.rfind('/') {
            Some(i) => (path.get(..i).unwrap_or(""), path.get(i + 1..).unwrap_or("")),
            None => ("", path),
        };
        let Some(own) = strip_code_extension(file) else {
            return;
        };
        // Cheap filter before resolving: the last segment must name this file (or, for an index
        // file, its directory).
        let last = spec
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(spec);
        let last = strip_code_extension(last).unwrap_or(last);
        let dir_name = dir.rsplit('/').next().unwrap_or(dir);
        if last != own && !(own == "index" && (last == dir_name || matches!(last, "." | ".."))) {
            return;
        }
        let Some(target) = resolve(dir, spec) else {
            return;
        };
        let target = strip_code_extension(&target).map_or(target.clone(), str::to_string);
        let own_path = join(dir, own);
        let own_dir_index = (own == "index").then(|| dir.to_string());
        // `./name/` (or `.`, `..`) names a directory: only its index file matches.
        let directory = spec.ends_with('/') || spec.ends_with('.');
        let hits =
            (!directory && target == own_path) || own_dir_index.as_deref() == Some(target.as_str());
        if hits {
            ctx.emit(
                source.span,
                format!("Remove this import of '{spec}': it is this file itself"),
            );
        }
    }
}

// No `.d.ts`: this file is never a declaration file (those are not analyzed), and a `.d.ts`
// specifier names a sibling declaration file, not this one.
const CODE_EXTENSIONS: &[&str] = &[".tsx", ".ts", ".mts", ".cts", ".jsx", ".js", ".mjs", ".cjs"];

/// The file name without a JavaScript/TypeScript extension; `None` for other files.
fn strip_code_extension(name: &str) -> Option<&str> {
    CODE_EXTENSIONS
        .iter()
        .find_map(|ext| name.strip_suffix(ext))
        .filter(|stem| !stem.is_empty() && !stem.ends_with('/'))
}

/// Resolve a relative specifier against a directory; `None` when it climbs above the root.
fn resolve(dir: &str, spec: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for segment in spec.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}
