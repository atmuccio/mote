//! `js/default-export` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct DefaultExport;

/// File names and folders where frameworks and tools require a default export.
const DEFAULT_ALLOWED: &str = "*.config.*, *.conf.*, .*rc.*, *.workspace.*, ormconfig.*, \
*.stories.*, *.story.*, *.fixture.*, .storybook/, generators/, transforms/, \
pages/, routes/, layouts/, middleware/, plugins/, server/, theme/, __mocks__/, \
page.*, layout.*, template.*, loading.*, error.*, global-error.*, not-found.*, default.*, \
opengraph-image.*, twitter-image.*, icon.*, apple-icon.*, sitemap.*, robots.*, manifest.*, \
root.*, entry.client.*, entry.server.*";

pub static DEFAULT_EXPORT: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Incubating,
    default_enabled: false,
    tags: &["conventions", "modules"],
    remediation_minutes: 5,
    params: &[ParamSpec {
        name: "allow_files",
        default: ParamDefault::Str(DEFAULT_ALLOWED),
        description: "Comma-separated patterns of files where default exports are accepted: an entry ending in `/` is a folder name anywhere in the path (`pages/`), any other entry is matched against the file name with `*` as a wildcard (`*.config.*`). The defaults cover configuration files, stories and the route, page and layout files of common frameworks, which require a default export.",
    }],
    description: r#"Flags default exports: `export default …` and `export { name as default }`.

**Why:** a default export has no name of its own, so every importer picks one: the same module
ends up imported as `Button`, `Btn` and `PrimaryButton` in different files, searches for the name
miss some uses, and renaming the export does not update its importers. Named exports keep one
name everywhere, are completed by editors, and make a typo in an import a compile error instead
of a silently different binding.

**Fix:** export the declaration by name and import it with braces.

```js
export default function formatDate(d) { … }        // flagged
import formatDate from './format-date';

export function formatDate(d) { … }                // compliant
import { formatDate } from './format-date';
```

Not reported: files where a framework or tool requires a default export, configured by the
`allow_files` parameter (by default configuration files such as `*.config.*` and `.*rc.*`,
Storybook stories and Cosmos fixtures, Yeoman `generators` and codemod `transforms` folders, the
page, route and layout files and folders of Next.js, Remix, Nuxt, Gatsby and Docusaurus, and
`__mocks__` folders), `export default` objects that define a `fetch`
handler (the module format of worker runtimes), and code inside `declare module`.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/default-export",
        "js",
        "Modules should use named exports instead of a default export",
    )
};

/// `*`-wildcard match of `pattern` against `name` (ASCII case-insensitive).
fn wildcard(pattern: &str, name: &str) -> bool {
    let (p, n) = (pattern.as_bytes(), name.as_bytes());
    let (mut pi, mut ni) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ni < n.len() {
        match p.get(pi) {
            Some(b'*') => {
                star = Some((pi, ni));
                pi += 1;
            }
            Some(c) if c.eq_ignore_ascii_case(&n[ni]) => {
                pi += 1;
                ni += 1;
            }
            _ => match star {
                Some((sp, sn)) => {
                    pi = sp + 1;
                    ni = sn + 1;
                    star = Some((sp, sn + 1));
                }
                None => return false,
            },
        }
    }
    p.get(pi..)
        .is_some_and(|rest| rest.iter().all(|c| *c == b'*'))
}

fn allowed_file(ctx: &Ctx<'_>) -> bool {
    let path = ctx.path();
    let mut segments = path.split('/');
    let file = segments.next_back().unwrap_or("");
    let dirs: Vec<&str> = segments.collect();
    ctx.param_list("allow_files")
        .any(|entry| match entry.strip_suffix('/') {
            Some(dir) => dirs.iter().any(|d| d.eq_ignore_ascii_case(dir)),
            None => wildcard(entry, file),
        })
}

/// [`allowed_file`], computed once per file, when the file has a default export.
fn allowed(ctx: &mut Ctx<'_>) -> bool {
    if let Some(allowed) = ctx.state::<Allowed>().0 {
        return allowed;
    }
    let allowed = allowed_file(ctx);
    ctx.state::<Allowed>().0 = Some(allowed);
    allowed
}

#[derive(Default)]
struct Allowed(Option<bool>);

/// `export default { fetch(…) { … } }`: the module shape worker runtimes require.
fn is_worker_module(expr: &Expression<'_>) -> bool {
    let Expression::ObjectExpression(obj) = helpers::unwrap_expr(expr) else {
        return false;
    };
    obj.properties.iter().any(|p| match p {
        ObjectPropertyKind::ObjectProperty(prop) => prop.key.is_specific_static_name("fetch"),
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

impl JsRule for DefaultExport {
    fn meta(&self) -> &'static RuleMeta {
        &DEFAULT_EXPORT
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::ExportDefaultDeclaration,
            AstType::ExportNamedDeclaration,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_module()
            && (ctx.file_contains(AstType::ExportDefaultDeclaration)
                || ctx.file_contains(AstType::ExportNamedDeclaration))
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::ExportDefaultDeclaration(export) => {
                if let Some(expr) = export.declaration.as_expression() {
                    if is_worker_module(expr) {
                        return;
                    }
                }
                if ctx.in_ambient_context(node) || allowed(ctx) {
                    return;
                }
                ctx.emit(
                    Span::new(export.span.start, export.span.start + 6),
                    "Use a named export instead of a default export",
                );
            }
            AstKind::ExportNamedDeclaration(export) => {
                if export.export_kind.is_type() {
                    return;
                }
                for specifier in &export.specifiers {
                    if specifier.exported.name() == "default" && !specifier.export_kind.is_type() {
                        if ctx.in_ambient_context(node) || allowed(ctx) {
                            return;
                        }
                        ctx.emit(
                            specifier.span,
                            "Use a named export instead of exporting 'default'",
                        );
                    }
                }
            }
            _ => {}
        }
    }
}
