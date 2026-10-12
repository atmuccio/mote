//! `js/no-unused-vars` — a semantic, file-level rule: it subscribes to no nodes and walks the
//! symbol table once in `run_once`.

use crate::prelude::*;

pub struct NoUnusedVars;

pub static NO_UNUSED_VARS: RuleMeta = RuleMeta {
    severity: Severity::Medium,
    tags: &["unused"],
    remediation_minutes: 2,
    params: &[ParamSpec {
        name: "caught_errors",
        default: ParamDefault::Bool(false),
        description: "Also report unused catch-clause bindings (`catch (e)` where `e` is never used; `catch {` needs no binding).",
    }],
    description: r#"Flags variables, functions, classes, imports, types, enums, and parameters that are
declared but never used.

**Why:** unused declarations are noise: readers must work out that they do nothing, and they often
point at a bug — a value computed and then forgotten, or a typo that made the real code use another
name. Unused imports also slow builds and can keep dead modules in a bundle.

**Fix:** remove the declaration. If it must stay (e.g. a positional callback parameter), prefix its
name with `_`.

```ts
import { readFile, writeFile } from 'fs'; // flagged: writeFile

function load(path: string, options: object) { // flagged: options
  const unused = 42;                           // flagged
  let result;                                  // flagged: assigned but never read
  result = readFile(path);
}

items.map((item, index) => index);             // compliant: `item` precedes a used parameter
const { id, ...rest } = user;                  // compliant: `id` is excluded from `rest`
```

What counts as a use: any read, any type reference (`x: Foo`, `typeof x`), JSX usage (`<Foo />`),
a mention in JSDoc (`{@link Foo}`, `@param {Foo}`), and export. Updating a variable from its own
value in a statement (`count++;`, `total += n;`) is not a use: a variable that is only ever
updated that way is never read, so it is reported like one that is only assigned. Not reported: names starting with
`_`, exported declarations, `declare` / ambient declarations, merged or overloaded declarations,
catch parameters, `using` declarations, decorated classes, the `React` import in files with JSX,
and top-level declarations of non-module scripts (they are globals). With the `caught_errors`
parameter on, unused catch parameters are reported too (`catch {` needs no binding since ES2019).

Parameters are reported only where the author controls the signature — function declarations,
constructors, methods of classes that neither extend nor implement anything (and are not
`override`), and functions assigned to a variable without a type annotation — and only when no
later parameter is used. Callbacks, object-literal members, setters, parameter properties,
decorated, destructured, and rest parameters are never reported, nor are the parameters of an
empty method (a hook whose signature is there for subclasses to override).

Type parameters of functions and methods are reported when nothing mentions them
(`function first<T>(list: string[])`): a caller can pass a type argument that changes nothing.
Only signatures that nothing outside can depend on are checked: functions and arrow functions
that are not exported, overloaded, merged, or ambient, and `private` / `#` methods and fields
that are not overloaded or decorated. Public methods (a subclass or an interface may share the
signature) and type parameters of classes, interfaces, and type aliases (an unused one can
deliberately tag a type) are left alone.
"#,
    ..RuleMeta::new(
        "js/no-unused-vars",
        "js",
        "Unused declarations should be removed",
    )
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum What {
    Variable,
    Function,
    Class,
    Import,
    Type,
    Enum,
    Parameter,
    CaughtError,
    TypeParameter,
}

impl JsRule for NoUnusedVars {
    fn meta(&self) -> &'static RuleMeta {
        &NO_UNUSED_VARS
    }

    fn applies_to(&self, ctx: &Ctx<'_>) -> bool {
        // Direct `eval` and `with` can reach any local by name.
        !ctx.file_contains(AstType::WithStatement) && !has_global(ctx, "eval")
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let scoping = ctx.scoping();
        let root = scoping.root_scope_id();
        // In scripts, top-level declarations are globals that other files may use.
        let top_level_private =
            ctx.is_module() || ctx.source_type().is_commonjs() || is_commonjs(ctx);
        let jsx_names = jsx_pragma_names(ctx);
        let caught_errors = ctx.param_bool("caught_errors");
        let mut reports: Vec<(Span, String, &str, What)> = Vec::new();

        for symbol in scoping.symbol_ids() {
            let name = scoping.symbol_name(symbol);
            if name.starts_with('_') {
                continue;
            }
            let flags = scoping.symbol_flags(symbol);
            if flags.contains(SymbolFlags::TypeParameter) {
                if ctx.is_typescript()
                    && ctx.is_unused_symbol(symbol)
                    && unused_type_parameter_reportable(ctx, ctx.symbol_declaration(symbol))
                {
                    reports.push((
                        scoping.symbol_span(symbol),
                        format!("Remove the unused type parameter '{name}'"),
                        name,
                        What::TypeParameter,
                    ));
                }
                continue;
            }
            if !flags.intersects(
                SymbolFlags::Variable
                    | SymbolFlags::Function
                    | SymbolFlags::Class
                    | SymbolFlags::Import
                    | SymbolFlags::TypeImport
                    | SymbolFlags::TypeAlias
                    | SymbolFlags::Interface
                    | SymbolFlags::Enum,
            ) || flags.intersects(
                SymbolFlags::Ambient
                    | SymbolFlags::EnumMember
                    | SymbolFlags::TypeParameter
                    | SymbolFlags::NamespaceModule
                    | SymbolFlags::ValueModule,
            ) {
                continue;
            }
            if flags.contains(SymbolFlags::CatchVariable) && !caught_errors {
                continue;
            }
            if !scoping.symbol_redeclarations(symbol).is_empty() {
                continue;
            }
            if !top_level_private && scoping.symbol_scope_id(symbol) == root {
                continue;
            }

            let decl = ctx.symbol_declaration(symbol);
            let own_body = self_reference_span(decl.kind());
            let mut referenced = false;
            let mut used = false;
            for reference in ctx.references_of(symbol) {
                referenced = true;
                if counts_as_use(ctx, reference, own_body) {
                    used = true;
                    break;
                }
            }
            if used {
                continue;
            }
            let Some(what) = classify(ctx, symbol, decl) else {
                continue;
            };
            if is_exported(ctx, decl) || ctx.in_ambient_context(decl) || jsx_names.contains(&name) {
                continue;
            }
            let assigned_only = referenced
                && what == What::Variable
                && ctx.references_of(symbol).all(|r| {
                    r.flags() == ReferenceFlags::Write || is_discarded_self_update(ctx, r.node_id())
                });
            let message = if assigned_only {
                format!("Remove the variable '{name}': it is assigned but never read")
            } else {
                let noun = match what {
                    What::Variable => "variable",
                    What::Function => "function",
                    What::Class => "class",
                    What::Import => "import",
                    What::Type => "type",
                    What::Enum => "enum",
                    What::Parameter => "parameter",
                    What::CaughtError => "catch binding",
                    What::TypeParameter => "type parameter",
                };
                format!("Remove the unused {noun} '{name}'")
            };
            reports.push((scoping.symbol_span(symbol), message, name, what));
        }
        if reports.is_empty() {
            return;
        }
        // Names mentioned in JSDoc (`{@link Foo}`, `@param {Foo}`, `@see Foo`) count as used, as
        // they do for TypeScript. Only computed when there is something to report.
        let documented = if reports.iter().any(|r| r.3 != What::Parameter) {
            jsdoc_names(ctx)
        } else {
            FxHashSet::default()
        };
        for (span, message, name, what) in reports {
            if matches!(what, What::Parameter | What::CaughtError) || !documented.contains(name) {
                ctx.emit(span, message);
            }
        }
    }
}

/// Identifiers referenced from JSDoc comments: inside `{…}` (type expressions and inline
/// `{@link …}` tags) and after `@see`/`@link`.
fn jsdoc_names<'a>(ctx: &Ctx<'a>) -> FxHashSet<&'a str> {
    let mut names = FxHashSet::default();
    for comment in ctx.comments() {
        let text = ctx.text(comment.span);
        if !text.starts_with("/**") {
            continue;
        }
        let bytes = text.as_bytes();
        let (mut depth, mut i, mut after_tag) = (0u32, 0usize, false);
        while i < bytes.len() {
            let b = bytes[i];
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                depth = depth.saturating_sub(1);
            } else if b.is_ascii_alphabetic() || b == b'_' || b == b'$' {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'$')
                {
                    i += 1;
                }
                let word = &text[start..i];
                let is_tag = start > 0 && bytes[start - 1] == b'@';
                if is_tag {
                    after_tag = matches!(word, "see" | "link" | "linkcode" | "linkplain");
                } else {
                    if depth > 0 || after_tag {
                        names.insert(word);
                    }
                    after_tag = false;
                }
                continue;
            }
            i += 1;
        }
    }
    names
}

/// Is `name` referenced as a global anywhere in the file?
fn has_global(ctx: &Ctx<'_>, name: &str) -> bool {
    ctx.scoping()
        .root_unresolved_references()
        .contains_key(name)
}

/// A script that uses CommonJS (`require`, `module`, `exports`) has module-private top level.
fn is_commonjs(ctx: &Ctx<'_>) -> bool {
    let globals = ctx.scoping().root_unresolved_references();
    globals.contains_key("require")
        || globals.contains_key("module")
        || globals.contains_key("exports")
}

/// Names used implicitly by JSX: `React` (classic runtime) and `@jsx` / `@jsxFrag` pragmas.
fn jsx_pragma_names<'a>(ctx: &Ctx<'a>) -> Vec<&'a str> {
    if !ctx.file_contains(AstType::JSXElement) && !ctx.file_contains(AstType::JSXFragment) {
        return Vec::new();
    }
    let mut names = vec!["React"];
    for comment in ctx.comments() {
        let text = ctx.text(comment.span);
        for (i, _) in text.match_indices("@jsx") {
            let rest = &text[i + 4..];
            let rest = rest.strip_prefix("Frag").unwrap_or(rest);
            if !rest.starts_with(char::is_whitespace) {
                continue;
            }
            let ident: &str = rest
                .trim_start()
                .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                .next()
                .unwrap_or("");
            if !ident.is_empty() {
                names.push(ident);
            }
        }
    }
    names
}

/// For functions, classes, recursive types, and function-valued variables: references inside their
/// own body (recursion) do not count as uses.
fn self_reference_span(kind: AstKind<'_>) -> Option<Span> {
    match kind {
        AstKind::Function(f) => Some(f.span),
        AstKind::Class(c) => Some(c.span),
        AstKind::TSTypeAliasDeclaration(t) => Some(t.span),
        AstKind::TSInterfaceDeclaration(i) => Some(i.span),
        AstKind::VariableDeclarator(d) => match d.init.as_ref().map(helpers::unwrap_expr) {
            Some(
                init @ (Expression::FunctionExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::ClassExpression(_)),
            ) => Some(init.span()),
            _ => None,
        },
        _ => None,
    }
}

fn counts_as_use(ctx: &Ctx<'_>, reference: &Reference, own_body: Option<Span>) -> bool {
    if let Some(body) = own_body {
        let at = ctx.node(reference.node_id()).span().start;
        if body.start <= at && at < body.end {
            return false;
        }
    }
    reference.flags() != ReferenceFlags::Write
        && !is_discarded_self_update(ctx, reference.node_id())
}

/// Is the reference the target of `x++` / `x += v` whose result is thrown away (a statement, a
/// `for` update, or a comma sequence in those places)? Such an update reads the variable only to
/// write it back.
fn is_discarded_self_update(ctx: &Ctx<'_>, reference: NodeId) -> bool {
    let Some(parent) = ctx.parent(reference) else {
        return false;
    };
    // The reference must be the updated target, not a value read on the right-hand side.
    let target = ctx.node(reference).span();
    match parent.kind() {
        AstKind::UpdateExpression(u) if u.argument.span() == target => {}
        AstKind::AssignmentExpression(a)
            if a.operator != AssignmentOperator::Assign
                && !a.operator.is_logical()
                && a.left.span() == target => {}
        _ => return false,
    }
    let mut current = parent;
    loop {
        let Some(up) = ctx.parent(current) else {
            return false;
        };
        match up.kind() {
            AstKind::SequenceExpression(_) => current = up,
            AstKind::ExpressionStatement(_) => {
                // An arrow function's expression body is its result.
                let arrow_body = ctx.parent(up).is_some_and(|body| {
                    matches!(body.kind(), AstKind::FunctionBody(_))
                        && matches!(ctx.parent_kind(body), Some(AstKind::ArrowFunctionExpression(a)) if a.expression)
                });
                return !arrow_body;
            }
            AstKind::ForStatement(f) => {
                return f
                    .update
                    .as_ref()
                    .is_some_and(|u| u.span() == current.span());
            }
            _ => return false,
        }
    }
}

/// An unused type parameter is reported only on functions and methods whose signature belongs to
/// this file: not exported, overloaded, merged, ambient, decorated, or part of a class contract.
fn unused_type_parameter_reportable(ctx: &Ctx<'_>, decl: &AstNode<'_>) -> bool {
    if !matches!(decl.kind(), AstKind::TSTypeParameter(_)) || ctx.in_ambient_context(decl) {
        return false;
    }
    let Some(list) = ctx.parent(decl) else {
        return false;
    };
    if !matches!(list.kind(), AstKind::TSTypeParameterDeclaration(_)) {
        return false;
    }
    let Some(function) = ctx.parent(list) else {
        return false;
    };
    match function.kind() {
        AstKind::Function(f) => {
            if f.body.is_none() || f.declare {
                return false;
            }
            if f.is_declaration() {
                // Overloads share the symbol of the implementation.
                let Some(symbol) = f.id.as_ref().and_then(|id| ctx.binding_symbol(id)) else {
                    return false;
                };
                let merged = !ctx.scoping().symbol_redeclarations(symbol).is_empty();
                return !merged && !is_exported(ctx, function) && !exported_by_name(ctx, symbol);
            }
        }
        AstKind::ArrowFunctionExpression(_) => {}
        _ => return false,
    }
    // A function expression or arrow: a method, a class field, or a variable's value.
    match ctx.outer_parent(function).map(|n| (n, n.kind())) {
        Some((method, AstKind::MethodDefinition(m))) => method_signature_private(ctx, method, m),
        Some((_, AstKind::PropertyDefinition(p))) => {
            p.decorators.is_empty()
                && (p.accessibility == Some(TSAccessibility::Private)
                    || matches!(p.key, PropertyKey::PrivateIdentifier(_)))
        }
        Some((declarator, AstKind::VariableDeclarator(d))) => {
            let BindingPattern::BindingIdentifier(id) = &d.id else {
                return false;
            };
            d.type_annotation.is_none()
                && !is_exported(ctx, declarator)
                && ctx
                    .binding_symbol(id)
                    .is_some_and(|symbol| !exported_by_name(ctx, symbol))
        }
        _ => false,
    }
}

/// `export { f }`, `export default f`, `export = f`: exported after its declaration.
fn exported_by_name(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    ctx.references_of(symbol).any(|r| {
        matches!(
            ctx.outer_parent(r.node_id()).map(AstNode::kind),
            Some(
                AstKind::ExportSpecifier(_)
                    | AstKind::ExportDefaultDeclaration(_)
                    | AstKind::TSExportAssignment(_)
            )
        )
    })
}

/// Is the method's signature private to the class (`private` or `#name`, not overloaded,
/// decorated, or `override`)?
fn method_signature_private(ctx: &Ctx<'_>, method: &AstNode<'_>, m: &MethodDefinition<'_>) -> bool {
    if m.r#override
        || !m.decorators.is_empty()
        || m.r#type == MethodDefinitionType::TSAbstractMethodDefinition
        || m.kind != MethodDefinitionKind::Method
        || !is_private_member(m)
    {
        return false;
    }
    let Some(class_node) = ctx
        .ancestors(method)
        .find(|n| matches!(n.kind(), AstKind::Class(_)))
    else {
        return false;
    };
    let AstKind::Class(class) = class_node.kind() else {
        return false;
    };
    // Overload signatures are sibling methods with the same name and no body.
    let name = m.key.static_name();
    let overloaded = name.is_some()
        && class.body.body.iter().any(|element| {
            matches!(element, ClassElement::MethodDefinition(other)
                if other.span != m.span && other.value.body.is_none() && other.key.static_name() == name)
        });
    !overloaded && !class.declare
}

fn is_private_member(m: &MethodDefinition<'_>) -> bool {
    m.accessibility == Some(TSAccessibility::Private)
        || matches!(m.key, PropertyKey::PrivateIdentifier(_))
}

fn classify(ctx: &Ctx<'_>, symbol: SymbolId, decl: &AstNode<'_>) -> Option<What> {
    match decl.kind() {
        AstKind::VariableDeclarator(d) => {
            if matches!(
                d.kind,
                VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing
            ) {
                return None;
            }
            if position_exempt(ctx, &d.id, symbol).unwrap_or(true) {
                return None;
            }
            Some(What::Variable)
        }
        AstKind::FormalParameter(p) => classify_parameter(ctx, decl, p),
        // Only reached when `caught_errors` is on; destructured bindings are left alone.
        AstKind::CatchParameter(p) => {
            matches!(p.pattern, BindingPattern::BindingIdentifier(_)).then_some(What::CaughtError)
        }
        AstKind::Function(f) => (f.is_declaration() && f.body.is_some()).then_some(What::Function),
        AstKind::Class(c) => {
            (c.is_declaration() && !c.declare && c.decorators.is_empty()).then_some(What::Class)
        }
        AstKind::ImportSpecifier(_)
        | AstKind::ImportDefaultSpecifier(_)
        | AstKind::ImportNamespaceSpecifier(_)
        | AstKind::TSImportEqualsDeclaration(_) => Some(What::Import),
        AstKind::TSTypeAliasDeclaration(_) | AstKind::TSInterfaceDeclaration(_) => Some(What::Type),
        AstKind::TSEnumDeclaration(e) => (!e.declare).then_some(What::Enum),
        _ => None,
    }
}

fn classify_parameter(
    ctx: &Ctx<'_>,
    decl: &AstNode<'_>,
    param: &FormalParameter<'_>,
) -> Option<What> {
    let simple = match &param.pattern {
        BindingPattern::BindingIdentifier(_) => true,
        BindingPattern::AssignmentPattern(p) => {
            matches!(p.left, BindingPattern::BindingIdentifier(_))
        }
        _ => false,
    };
    // `x: never` parameters exist for compile-time exhaustiveness checks.
    let typed_never = param
        .type_annotation
        .as_ref()
        .is_some_and(|t| matches!(t.type_annotation, TSType::TSNeverKeyword(_)));
    if !simple
        || typed_never
        || param.accessibility.is_some()
        || param.readonly
        || param.r#override
        || !param.decorators.is_empty()
    {
        return None;
    }
    let params_node = ctx.parent(decl)?;
    let AstKind::FormalParameters(params) = params_node.kind() else {
        return None;
    };
    if params.kind == FormalParameterKind::Signature {
        return None;
    }
    let function = ctx.parent(params_node)?;
    match function.kind() {
        AstKind::Function(f) if f.body.is_none() => return None,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => {}
        _ => return None,
    }
    if !owns_signature(ctx, function) || uses_arguments_object(ctx, function.span()) {
        return None;
    }
    // Only parameters after the last used one can be removed without shifting positions.
    let index = params.items.iter().position(|p| p.span == param.span)?;
    let later_used = params.items[index + 1..]
        .iter()
        .any(|p| pattern_referenced(ctx, &p.pattern))
        || params
            .rest
            .as_ref()
            .is_some_and(|r| pattern_referenced(ctx, &r.rest.argument));
    if later_used {
        return None;
    }
    Some(What::Parameter)
}

/// Parameters are only reported where the author controls the signature. Callbacks, object-literal
/// members, overrides, interface implementations, setters, and functions stored in typed
/// variables must accept whatever their caller or contract passes.
fn owns_signature(ctx: &Ctx<'_>, function: &AstNode<'_>) -> bool {
    if let AstKind::Function(f) = function.kind() {
        if f.is_declaration() {
            return true;
        }
    }
    match ctx.outer_parent(function).map(AstNode::kind) {
        Some(AstKind::MethodDefinition(m)) => {
            if m.kind == MethodDefinitionKind::Constructor {
                return true;
            }
            if m.kind == MethodDefinitionKind::Set || m.r#override {
                return false;
            }
            // `onEvent(event) {}`: an empty hook whose signature is there for overrides.
            if m.value
                .body
                .as_ref()
                .is_some_and(|b| b.statements.is_empty())
            {
                return false;
            }
            // Class of the method: MethodDefinition → ClassBody → Class.
            let class = ctx.ancestors(function).find_map(|n| {
                if let AstKind::Class(c) = n.kind() {
                    Some(c)
                } else {
                    None
                }
            });
            class.is_some_and(|c| c.super_class.is_none() && c.implements.is_empty())
        }
        Some(AstKind::VariableDeclarator(d)) => d.type_annotation.is_none(),
        _ => false,
    }
}

/// Does code inside `span` read the implicit `arguments` object?
fn uses_arguments_object(ctx: &Ctx<'_>, span: Span) -> bool {
    let scoping = ctx.scoping();
    scoping
        .root_unresolved_references()
        .get("arguments")
        .is_some_and(|ids| {
            ids.iter().any(|&id| {
                let at = ctx.node(scoping.get_reference(id).node_id()).span().start;
                span.start <= at && at < span.end
            })
        })
}

/// Is any identifier bound by the pattern referenced at all?
fn pattern_referenced(ctx: &Ctx<'_>, pattern: &BindingPattern<'_>) -> bool {
    let mut referenced = false;
    helpers::for_each_binding(pattern, &mut |id| {
        if let Some(symbol) = id.symbol_id.get() {
            referenced |= !ctx.is_unused_symbol(symbol);
        }
    });
    referenced
}

/// `Some(true)` if `symbol` is bound in `pattern` in a position where removing it is not a simple
/// cleanup: a sibling of an object rest element (`{ a, ...rest }` — `a` is excluded from `rest`)
/// or an array element followed by a used element (`[a, b]` — `a` holds `b`'s position).
fn position_exempt(ctx: &Ctx<'_>, pattern: &BindingPattern<'_>, symbol: SymbolId) -> Option<bool> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => {
            (id.symbol_id.get() == Some(symbol)).then_some(false)
        }
        BindingPattern::AssignmentPattern(p) => position_exempt(ctx, &p.left, symbol),
        BindingPattern::ObjectPattern(p) => {
            for prop in &p.properties {
                if let Some(exempt) = position_exempt(ctx, &prop.value, symbol) {
                    let direct = matches!(&prop.value, BindingPattern::BindingIdentifier(_))
                        || matches!(&prop.value, BindingPattern::AssignmentPattern(a)
                            if matches!(a.left, BindingPattern::BindingIdentifier(_)));
                    return Some(exempt || (direct && p.rest.is_some()));
                }
            }
            p.rest
                .as_ref()
                .and_then(|r| position_exempt(ctx, &r.argument, symbol))
        }
        BindingPattern::ArrayPattern(p) => {
            for (i, element) in p.elements.iter().enumerate() {
                let Some(element) = element else { continue };
                if let Some(exempt) = position_exempt(ctx, element, symbol) {
                    let later_used = p.elements[i + 1..]
                        .iter()
                        .flatten()
                        .any(|e| pattern_referenced(ctx, e))
                        || p.rest
                            .as_ref()
                            .is_some_and(|r| pattern_referenced(ctx, &r.argument));
                    return Some(exempt || later_used);
                }
            }
            p.rest
                .as_ref()
                .and_then(|r| position_exempt(ctx, &r.argument, symbol))
        }
    }
}

/// `export const x`, `export function f`, `export default class C`, `export type T`, …
fn is_exported(ctx: &Ctx<'_>, decl: &AstNode<'_>) -> bool {
    let statement = match decl.kind() {
        AstKind::VariableDeclarator(_) => match ctx.parent(decl) {
            Some(p) => p,
            None => return false,
        },
        _ => decl,
    };
    matches!(
        ctx.parent_kind(statement),
        Some(AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_))
    )
}
