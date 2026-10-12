//! `js/adjacent-overload-signatures`

use crate::prelude::*;

pub struct AdjacentOverloadSignatures;

pub static ADJACENT_OVERLOAD_SIGNATURES: RuleMeta = RuleMeta {
    severity: Severity::Low,
    tags: &["typescript", "convention"],
    remediation_minutes: 5,
    description: r#"Flags overloads of the same function or method that are separated by other
declarations.

**Why:** the overload signatures of one function are a single unit: together they describe every
way it can be called. When they are scattered between other members, readers see only part of the
contract and are likely to miss an overload — or to add a duplicate.

**Fix:** move all overloads (and the implementation) of the name next to each other.

Checked lists: top-level and namespace function declarations (also `export function` and
`declare function`), class methods (static and instance methods are separate names), and the
method, call (`(): T`), and construct (`new (): T`) signatures of interfaces and type literals.
Getters and setters are not overloads and are not checked.

```ts
interface Emitter {
  on(event: 'open', cb: () => void): void;
  close(): void;
  on(event: 'data', cb: (d: string) => void): void; // flagged
}

interface Emitter {
  on(event: 'open', cb: () => void): void;
  on(event: 'data', cb: (d: string) => void): void; // compliant
  close(): void;
}
```
"#,
    ..RuleMeta::new(
        "js/adjacent-overload-signatures",
        "js",
        "Overload signatures should be adjacent",
    )
};

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key<'a> {
    Name {
        name: Cow<'a, str>,
        is_static: bool,
        private: bool,
    },
    Call,
    New,
}

impl Key<'_> {
    fn message(&self) -> String {
        match self {
            Key::Name {
                name,
                is_static,
                private,
            } => {
                let prefix = if *private { "#" } else { "" };
                let qualifier = if *is_static { "static " } else { "" };
                format!(
                    "Move this overload of {qualifier}'{prefix}{name}' next to its other overloads"
                )
            }
            Key::Call => "Move this call signature next to the other call signatures".to_string(),
            Key::New => {
                "Move this construct signature next to the other construct signatures".to_string()
            }
        }
    }
}

impl JsRule for AdjacentOverloadSignatures {
    fn meta(&self) -> &'static RuleMeta {
        &ADJACENT_OVERLOAD_SIGNATURES
    }

    fn types(&self) -> &'static [AstType] {
        &[
            AstType::Program,
            AstType::TSModuleBlock,
            AstType::BlockStatement,
            AstType::FunctionBody,
            AstType::ClassBody,
            AstType::TSInterfaceBody,
            AstType::TSTypeLiteral,
        ]
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        ctx.is_typescript()
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        match node.kind() {
            AstKind::Program(p) => check(ctx, p.body.iter().map(statement_key)),
            AstKind::TSModuleBlock(b) => check(ctx, b.body.iter().map(statement_key)),
            AstKind::BlockStatement(b) => check(ctx, b.body.iter().map(statement_key)),
            AstKind::FunctionBody(b) => check(ctx, b.statements.iter().map(statement_key)),
            AstKind::ClassBody(b) => check(ctx, b.body.iter().map(class_key)),
            AstKind::TSInterfaceBody(b) => check(ctx, b.body.iter().map(signature_key)),
            AstKind::TSTypeLiteral(t) => check(ctx, t.members.iter().map(signature_key)),
            _ => {}
        }
    }
}

/// Report members whose key was seen before but not on the directly preceding member.
fn check<'a>(ctx: &mut Ctx<'a>, members: impl Iterator<Item = Option<(Key<'a>, Span)>> + Clone) {
    // Cheap pre-check: fewer than two overloadable members cannot conflict.
    if members.clone().flatten().nth(1).is_none() {
        return;
    }
    let mut last_span: FxHashMap<Key<'a>, Span> = FxHashMap::default();
    let mut previous: Option<Key<'a>> = None;
    for member in members {
        let Some((key, span)) = member else {
            previous = None;
            continue;
        };
        if previous.as_ref() != Some(&key) {
            if let Some(&earlier) = last_span.get(&key) {
                ctx.emit(span, key.message())
                    .secondary(earlier, "Previous overload");
            }
        }
        last_span.insert(key.clone(), span);
        previous = Some(key);
    }
}

fn statement_key<'a>(stmt: &'a Statement<'a>) -> Option<(Key<'a>, Span)> {
    let function = match stmt {
        Statement::FunctionDeclaration(f) => f,
        Statement::ExportNamedDeclaration(e) => match &e.declaration {
            Some(Declaration::FunctionDeclaration(f)) => f,
            _ => return None,
        },
        _ => return None,
    };
    let id = function.id.as_ref()?;
    Some((
        Key::Name {
            name: Cow::Borrowed(id.name.as_str()),
            is_static: false,
            private: false,
        },
        id.span,
    ))
}

fn class_key<'a>(element: &'a ClassElement<'a>) -> Option<(Key<'a>, Span)> {
    let ClassElement::MethodDefinition(method) = element else {
        return None;
    };
    if !matches!(
        method.kind,
        MethodDefinitionKind::Method | MethodDefinitionKind::Constructor
    ) {
        return None;
    }
    let (name, private) = match &method.key {
        PropertyKey::PrivateIdentifier(p) => (Cow::Borrowed(p.name.as_str()), true),
        key => (helpers::property_key_name(key)?, false),
    };
    Some((
        Key::Name {
            name,
            is_static: method.r#static,
            private,
        },
        method.key.span(),
    ))
}

fn signature_key<'a>(signature: &'a TSSignature<'a>) -> Option<(Key<'a>, Span)> {
    match signature {
        TSSignature::TSMethodSignature(m) => {
            if m.kind != TSMethodSignatureKind::Method {
                return None;
            }
            let name = helpers::property_key_name(&m.key)?;
            Some((
                Key::Name {
                    name,
                    is_static: false,
                    private: false,
                },
                m.key.span(),
            ))
        }
        TSSignature::TSCallSignatureDeclaration(c) => Some((Key::Call, c.span)),
        TSSignature::TSConstructSignatureDeclaration(c) => Some((Key::New, c.span)),
        _ => None,
    }
}
