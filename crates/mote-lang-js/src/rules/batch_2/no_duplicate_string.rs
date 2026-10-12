//! `js/no-duplicate-string` — collects qualifying string literals during the traversal and groups
//! them once in `run_once`.

use super::util;
use crate::prelude::*;
use rustc_hash::FxBuildHasher;
use std::hash::BuildHasher;

pub struct NoDuplicateString;

pub static NO_DUPLICATE_STRING: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["duplication"],
    remediation_minutes: 5,
    params: &[
        ParamSpec {
            name: "min_length",
            default: ParamDefault::Int(10),
            description: "Minimum length (in characters) of a string literal to be considered.",
        },
        ParamSpec {
            name: "threshold",
            default: ParamDefault::Int(3),
            description: "Number of occurrences from which a literal is reported.",
        },
    ],
    description: r#"Flags a string literal that appears several times in the same file (by default:
at least 10 characters long, 3 or more times).

**Why:** a repeated literal — an error message, a setting name, an event name — has to be kept
in sync by hand. Changing one copy and forgetting another is a classic source of bugs, and the
repetition hides the fact that all these places mean the same thing.

**Fix:** define the value once as a named constant (or enum member) and use it everywhere.

Not counted: module specifiers (`import`/`export … from`, `require(…)`, `import(…)`),
directives such as `'use strict'`, property and member names (`{ 'a-b': 1 }`, `obj['key']`),
TypeScript literal types (`type Mode = 'light-theme'`), JSX attribute values, and declarations
in `declare` blocks. Test files are not checked.

```js
log('Connection refused by server');
retry('Connection refused by server');
throw new Error('Connection refused by server');   // flagged (reported at the first one)

const REFUSED = 'Connection refused by server';     // compliant
log(REFUSED);
```
"#,
    ..RuleMeta::new(
        "js/no-duplicate-string",
        "js",
        "String literals should not be duplicated",
    )
};

#[derive(Default)]
struct Collected {
    min_length: Option<usize>,
    /// (hash of the value, literal)
    literals: Vec<(u64, NodeId)>,
}

/// Is the literal a key, a module specifier, a type, or markup rather than a value?
fn is_excluded(ctx: &Ctx<'_>, node: &AstNode<'_>, span: Span) -> bool {
    let Some(parent) = ctx.parent(node) else {
        return true;
    };
    match parent.kind() {
        AstKind::Directive(_)
        | AstKind::ImportDeclaration(_)
        | AstKind::ExportNamedDeclaration(_)
        | AstKind::ExportAllDeclaration(_)
        | AstKind::ImportExpression(_)
        | AstKind::ImportSpecifier(_)
        | AstKind::ExportSpecifier(_)
        | AstKind::ImportAttribute(_)
        | AstKind::TSLiteralType(_)
        | AstKind::TSModuleDeclaration(_)
        | AstKind::TSExternalModuleReference(_)
        | AstKind::TSImportType(_)
        | AstKind::TSPropertySignature(_)
        | AstKind::TSMethodSignature(_)
        | AstKind::JSXAttribute(_) => true,
        AstKind::ObjectProperty(p) => p.key.span() == span,
        AstKind::BindingProperty(p) => p.key.span() == span,
        AstKind::AssignmentTargetPropertyProperty(p) => p.name.span() == span,
        AstKind::MethodDefinition(m) => m.key.span() == span,
        AstKind::PropertyDefinition(p) => p.key.span() == span,
        AstKind::AccessorProperty(p) => p.key.span() == span,
        AstKind::TSEnumMember(m) => m.id.span() == span,
        AstKind::ComputedMemberExpression(m) => m.expression.span() == span,
        AstKind::CallExpression(call) => {
            // `require('x')`, `require.resolve('x')`
            let callee = helpers::unwrap_expr(&call.callee);
            callee.is_specific_id("require")
                || matches!(callee, Expression::StaticMemberExpression(m)
                    if m.object.is_specific_id("require"))
        }
        _ => false,
    }
}

impl JsRule for NoDuplicateString {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_STRING
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::StringLiteral]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        !ctx.is_test()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let AstKind::StringLiteral(s) = node.kind() else {
            return;
        };
        let min_length = match ctx.state::<Collected>().min_length {
            Some(n) => n,
            None => {
                let n = ctx.param_i64("min_length").max(1) as usize;
                ctx.state::<Collected>().min_length = Some(n);
                n
            }
        };
        let value = s.value.as_str();
        // Byte length bounds the character count from above (and a character is at most 4 bytes).
        if value.len() < min_length
            || (value.len() < min_length.saturating_mul(4) && value.chars().count() < min_length)
        {
            return;
        }
        if is_excluded(ctx, node, s.span) {
            return;
        }
        // The ancestor walk only matters in files that can have `declare` blocks.
        let may_be_ambient = ctx.file_contains(AstType::TSModuleDeclaration)
            || ctx.file_contains(AstType::TSGlobalDeclaration);
        if may_be_ambient && ctx.in_ambient_context(node) {
            return;
        }
        let hash = FxBuildHasher.hash_one(value);
        ctx.state::<Collected>().literals.push((hash, node.id()));
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let mut literals = std::mem::take(&mut ctx.state::<Collected>().literals);
        let threshold = ctx.param_i64("threshold").max(2) as usize;
        if literals.len() < threshold {
            return;
        }
        // Group by hash (cheap integer sort), then confirm equal values within each group.
        literals.sort_unstable();
        let mut values: Vec<(&'a str, Span)> = Vec::new();
        for same_hash in literals.chunk_by(|a, b| a.0 == b.0) {
            if same_hash.len() < threshold {
                continue;
            }
            values.clear();
            values.extend(
                same_hash
                    .iter()
                    .filter_map(|&(_, id)| match ctx.node(id).kind() {
                        AstKind::StringLiteral(s) => Some((s.value.as_str(), s.span)),
                        _ => None,
                    }),
            );
            values.sort_unstable_by(|a, b| a.0.cmp(b.0).then(a.1.start.cmp(&b.1.start)));
            for group in values.chunk_by(|a, b| a.0 == b.0) {
                if group.len() < threshold {
                    continue;
                }
                let (value, first) = group[0];
                let mut emitted = ctx
                    .emit(
                        first,
                        format!(
                            "Define a constant instead of duplicating the literal \"{}\" {} times",
                            util::preview(value, 40),
                            group.len()
                        ),
                    )
                    .value(group.len() as f64);
                for &(_, span) in &group[1..] {
                    emitted = emitted.secondary(span, "Duplicate");
                }
            }
        }
    }
}
