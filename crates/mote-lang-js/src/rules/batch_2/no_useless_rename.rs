//! `js/no-useless-rename`

use crate::prelude::*;

pub struct NoUselessRename;

pub static NO_USELESS_RENAME: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["redundant"],
    remediation_minutes: 1,
    description: r#"Flags renames to the same name: `{ a: a }` in destructuring,
`import { a as a }`, and `export { a as a }`.

**Why:** the rename does nothing but makes the reader compare both names to find out.

**Fix:** use the short form: `{ a }`, `import { a }`, `export { a }`.

```js
const { id: id, name } = user;       // flagged
import { readFile as readFile } from 'fs';   // flagged

const { id, name } = user;           // compliant
import { readFile as read } from 'fs';       // compliant: a real rename
```
"#,
    ..RuleMeta::new(
        "js/no-useless-rename",
        "js",
        "Values should not be renamed to the same name",
    )
};

fn static_key<'a>(key: &PropertyKey<'a>) -> Option<&'a str> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str()),
        _ => None,
    }
}

impl JsRule for NoUselessRename {
    fn meta(&self) -> &'static RuleMeta {
        &NO_USELESS_RENAME
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::BindingProperty,
            AstType::AssignmentTargetPropertyProperty,
            AstType::ImportSpecifier,
            AstType::ExportSpecifier,
        ]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (span, name, what) = match node.kind() {
            AstKind::BindingProperty(p) => {
                if p.shorthand || p.computed {
                    return;
                }
                let Some(key) = static_key(&p.key) else {
                    return;
                };
                let target = match &p.value {
                    BindingPattern::BindingIdentifier(id) => id,
                    BindingPattern::AssignmentPattern(a) => match &a.left {
                        BindingPattern::BindingIdentifier(id) => id,
                        _ => return,
                    },
                    _ => return,
                };
                if target.name != key || target.span == p.key.span() {
                    return;
                }
                (p.span, key, "destructuring")
            }
            AstKind::AssignmentTargetPropertyProperty(p) => {
                if p.computed {
                    return;
                }
                let Some(key) = static_key(&p.name) else {
                    return;
                };
                let target = match &p.binding {
                    AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(d) => &d.binding,
                    other => match other.as_assignment_target() {
                        Some(t) => t,
                        None => return,
                    },
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = target else {
                    return;
                };
                if id.name != key || id.span == p.name.span() {
                    return;
                }
                (p.span, key, "destructuring")
            }
            AstKind::ImportSpecifier(s) => {
                let ModuleExportName::IdentifierName(imported) = &s.imported else {
                    return;
                };
                if imported.name != s.local.name || imported.span == s.local.span {
                    return;
                }
                (s.span, imported.name.as_str(), "import")
            }
            AstKind::ExportSpecifier(s) => {
                let local = match &s.local {
                    ModuleExportName::IdentifierName(n) => (n.name.as_str(), n.span),
                    ModuleExportName::IdentifierReference(r) => (r.name.as_str(), r.span),
                    ModuleExportName::StringLiteral(_) => return,
                };
                let ModuleExportName::IdentifierName(exported) = &s.exported else {
                    return;
                };
                if exported.name != local.0 || exported.span == local.1 {
                    return;
                }
                (s.span, local.0, "export")
            }
            _ => return,
        };
        ctx.emit(
            span,
            format!("Remove the redundant rename of '{name}' to itself in this {what}"),
        );
    }
}
