//! `js/naming-convention` (off by default; part of the `conventions` set)

use crate::prelude::*;

pub struct NamingConvention;

pub static NAMING_CONVENTION: RuleMeta = RuleMeta {
    severity: Severity::Low,
    status: RuleStatus::Stable,
    default_enabled: false,
    tags: &["conventions", "naming"],
    remediation_minutes: 2,
    params: &[
        ParamSpec {
            name: "types",
            default: ParamDefault::Str("PascalCase"),
            description: "Allowed styles for classes, interfaces, type aliases, enums and type parameters (comma-separated: camelCase, PascalCase, UPPER_CASE, snake_case; empty = not checked).",
        },
        ParamSpec {
            name: "functions",
            default: ParamDefault::Str("camelCase, PascalCase"),
            description: "Allowed styles for named functions (PascalCase covers components and constructor functions).",
        },
        ParamSpec {
            name: "methods",
            default: ParamDefault::Str("camelCase"),
            description: "Allowed styles for class methods.",
        },
        ParamSpec {
            name: "variables",
            default: ParamDefault::Str("camelCase, PascalCase"),
            description: "Allowed styles for variables other than module-level constants. PascalCase is accepted only when the initial value is not a plain string, number or boolean literal.",
        },
        ParamSpec {
            name: "constants",
            default: ParamDefault::Str("camelCase, PascalCase, UPPER_CASE"),
            description: "Allowed styles for `const` declarations at module (or namespace) level. PascalCase as for variables.",
        },
        ParamSpec {
            name: "local_constants",
            default: ParamDefault::Str("camelCase, PascalCase, UPPER_CASE"),
            description: "Allowed styles for `const` declarations inside functions and blocks (the Google guides want camelCase there: remove UPPER_CASE to follow them). PascalCase as for variables.",
        },
        ParamSpec {
            name: "pascal_case_values",
            default: ParamDefault::Bool(false),
            description: "Accept PascalCase also for variables and constants whose initial value is a plain string, number or boolean literal.",
        },
        ParamSpec {
            name: "parameters",
            default: ParamDefault::Str("camelCase, PascalCase"),
            description: "Allowed styles for function parameters and catch parameters.",
        },
        ParamSpec {
            name: "enum_members",
            default: ParamDefault::Str(""),
            description: "Allowed styles for enum members (empty = not checked: projects differ between PascalCase and UPPER_CASE, and member names often mirror external identifiers).",
        },
        ParamSpec {
            name: "properties",
            default: ParamDefault::Str(""),
            description: "Allowed styles for class fields and accessors (empty = not checked: field names often mirror external data).",
        },
        ParamSpec {
            name: "allow",
            default: ParamDefault::Str(""),
            description: "Comma-separated names that are never reported.",
        },
        ParamSpec {
            name: "allow_prefixes",
            default: ParamDefault::Str("UNSAFE_"),
            description: "Comma-separated prefixes; names starting with one are never reported (framework-reserved names such as React's `UNSAFE_` methods).",
        },
        ParamSpec {
            name: "skip_dirs",
            default: ParamDefault::Str("vendor, vendors, third_party, third-party, thirdparty, external, generated, __generated__, fixtures, __fixtures__"),
            description: "Comma-separated folder names: files under such a folder are not checked (vendored or generated code, mirrors of other packages, sample code used as test input). Files whose name contains `generated` are skipped too.",
        },
        ParamSpec {
            name: "leading_underscore",
            default: ParamDefault::Bool(true),
            description: "Accept leading underscores (`_private`, `_unused`); when false they are reported.",
        },
        ParamSpec {
            name: "trailing_underscore",
            default: ParamDefault::Bool(true),
            description: "Accept trailing underscores (`default_`); when false they are reported.",
        },
        ParamSpec {
            name: "dollar",
            default: ParamDefault::Bool(true),
            description: "Accept `$` in names (`$el`, `value$`); when false they are reported.",
        },
    ],
    description: r#"Flags declared names whose casing does not match the convention of their kind: classes,
interfaces, type aliases, enums and type parameters in PascalCase; functions, methods,
variables and parameters in camelCase; `const` declarations may also use UPPER_CASE.

**Why:** in JavaScript and TypeScript the casing of a name tells the reader what it is: a type
or constructor (`UserService`), a value or function (`userService`, `loadUser`), or a fixed
module constant (`MAX_RETRIES`). Mixing conventions (`user_name` next to `userName`, a class
called `parser`) hides that signal and makes names harder to guess and to search for.

**Fix:** rename the declaration (and its uses) to the expected style.

```ts
class user_service {}                 // flagged: PascalCase expected
function load_user(user_id) {}        // flagged twice: camelCase expected
const MAX_RETRIES = 3;                // compliant: module-level constant

class UserService {}                  // compliant
function loadUser(userId) {}          // compliant
```

The styles are loose, as in common linters: camelCase starts with a lower-case letter and has
no underscore (`parseXMLDocument` is fine), PascalCase starts with an upper-case letter and has
no underscore (`URL`, `T`), UPPER_CASE has no lower-case letter; an underscore between two
digits (`draft2020_12`) or before a trailing number (`editor_1`) does not count. Leading and trailing underscores and `$` are accepted
(parameters), and each kind's styles can be configured.
Defaults that keep the rule quiet where the name is not the author's choice or where the
convention itself allows more than one style:

* functions and variables may also be PascalCase (components, constructor functions, classes
  and schemas held in variables), except a variable whose value is a plain string, number or
  boolean literal;
* `const` declarations may be UPPER_CASE whatever their value (the Google guides keep
  UPPER_CASE for module-level constants: set `local_constants` to follow them);
* names that come from elsewhere are not checked: imports and `require(...)` results,
  destructured names, variables exported from a `namespace` (read as members, like enum
  members), declarations documented as `@deprecated` (kept under their old name for
  compatibility), variables initialized from a field of the same name
  (`const user_id = body.user_id`), variables and parameters also used as shorthand properties
  (`{ user_id }`: the name is a key of some data format), object-literal keys, interface and
  type-literal members, quoted or computed keys, class fields (the `properties` parameter turns
  them on), members of classes that extend or implement another type (their names follow the
  base type), decorated members and members marked `override`, function or class expressions
  stored under a property name, anything declared with `declare` or inside `declare module` /
  `declare global`, names wrapped in double underscores (`__dirname`,
  `__webpack_public_path__`) or starting with `UNSAFE_`, top-level names of scripts (files without
  `import` or `export`, whose top-level names are globals other scripts and pages call), and
  files under a vendored, external, generated or test-fixture folder (`skip_dirs`), whose name contains
  `generated`, or that start with an `eslint-disable` comment for all rules (bindings and
  bundles copied from elsewhere);
* enum members are not checked unless `enum_members` lists the accepted styles.

Off by default: part of the opt-in `conventions` set.
"#,
    ..RuleMeta::new(
        "js/naming-convention",
        "js",
        "Names should follow the casing convention of their kind",
    )
};

const CAMEL: u8 = 1;
const PASCAL: u8 = 2;
const UPPER: u8 = 4;
const SNAKE: u8 = 8;

/// The parameters, read once per rule set ([`Ctx::rule_memo`]).
struct Settings {
    types: u8,
    functions: u8,
    methods: u8,
    variables: u8,
    constants: u8,
    local_constants: u8,
    pascal_values: bool,
    parameters: u8,
    enum_members: u8,
    properties: u8,
    leading: bool,
    trailing: bool,
    dollar: bool,
    allow: Vec<String>,
    prefixes: Vec<String>,
}

impl Settings {
    fn load(ctx: &Ctx<'_>) -> Self {
        let formats = |name: &str| {
            ctx.param_list(name)
                .fold(0, |acc, item| acc | parse_format(item))
        };
        Settings {
            types: formats("types"),
            functions: formats("functions"),
            methods: formats("methods"),
            variables: formats("variables"),
            constants: formats("constants"),
            local_constants: formats("local_constants"),
            pascal_values: ctx.param_bool("pascal_case_values"),
            parameters: formats("parameters"),
            enum_members: formats("enum_members"),
            properties: formats("properties"),
            leading: ctx.param_bool("leading_underscore"),
            trailing: ctx.param_bool("trailing_underscore"),
            dollar: ctx.param_bool("dollar"),
            allow: ctx.param_list("allow").map(str::to_string).collect(),
            prefixes: ctx
                .param_list("allow_prefixes")
                .map(str::to_string)
                .collect(),
        }
    }

    fn exempt_name(&self, name: &str) -> bool {
        self.allow.iter().any(|a| a == name)
            || self.prefixes.iter().any(|p| name.starts_with(p.as_str()))
    }
}

fn parse_format(item: &str) -> u8 {
    // Case-insensitive (ASCII), without lowercasing a copy: read for every file.
    [
        ("camelcase", CAMEL),
        ("pascalcase", PASCAL),
        ("upper_case", UPPER),
        ("snake_case", SNAKE),
    ]
    .iter()
    .find(|(name, _)| item.eq_ignore_ascii_case(name))
    .map_or(0, |&(_, bit)| bit)
}

fn format_names(bits: u8) -> String {
    let names: Vec<&str> = [
        (CAMEL, "camelCase"),
        (PASCAL, "PascalCase"),
        (UPPER, "UPPER_CASE"),
        (SNAKE, "snake_case"),
    ]
    .iter()
    .filter(|(bit, _)| bits & bit != 0)
    .map(|(_, name)| *name)
    .collect();
    names.join(" or ")
}

/// The styles `name` satisfies (`Some(0)` for none), or `None` when the name is exempt by shape
/// (only underscores and dollars, or wrapped in double underscores).
fn classify(name: &str, s: &Settings) -> Option<u8> {
    if name.len() > 4 && name.starts_with("__") && name.ends_with("__") {
        return None;
    }
    let after_leading = name.trim_start_matches('_');
    let core = after_leading.trim_end_matches('_');
    if core.bytes().all(|b| b == b'$') {
        return None;
    }
    // `editor_1`, `MESSAGE_2`: a trailing number after an underscore numbers variants (and is
    // what compilers and refactoring tools append to avoid clashes).
    let core = match core.rsplit_once('_') {
        Some((head, digits))
            if !head.is_empty()
                && !head.ends_with('_')
                && !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit()) =>
        {
            head
        }
        _ => core,
    };
    if (!s.leading && after_leading.len() != name.len())
        || (!s.trailing && core.len() != after_leading.len())
        || (!s.dollar && core.contains('$'))
    {
        return Some(0);
    }
    let mut underscore = false;
    let mut upper = false;
    let mut lower = false;
    let mut first: Option<char> = None;
    let mut previous: Option<char> = None;
    let mut chars = core.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '_' {
            // `draft2020_12`: an underscore between two digits only separates numbers.
            let between_digits = previous.is_some_and(|p| p.is_ascii_digit())
                && chars.peek().is_some_and(char::is_ascii_digit);
            if !between_digits {
                underscore = true;
            }
        } else if c.is_uppercase() {
            upper = true;
        } else if c.is_lowercase() {
            lower = true;
        }
        if first.is_none() && c != '$' {
            first = Some(c);
        }
        previous = Some(c);
    }
    let first_upper = first.is_some_and(char::is_uppercase);
    let first_lower = first.is_some_and(char::is_lowercase);
    let mut bits = 0;
    if !underscore && !first_upper {
        bits |= CAMEL;
    }
    if !underscore && !first_lower {
        bits |= PASCAL;
    }
    if !lower {
        bits |= UPPER;
    }
    if !upper {
        bits |= SNAKE;
    }
    Some(bits)
}

#[derive(Clone, Copy)]
enum Kind {
    Type(&'static str),
    Function,
    Variable { pascal_ok: bool },
    Constant { pascal_ok: bool },
    LocalConstant { pascal_ok: bool },
    Parameter,
    EnumMember,
}

impl Kind {
    fn allowed(self, s: &Settings) -> u8 {
        match self {
            Kind::Type(_) => s.types,
            Kind::Function => s.functions,
            Kind::Variable { pascal_ok } => {
                without_pascal(s.variables, pascal_ok || s.pascal_values)
            }
            Kind::Constant { pascal_ok } => {
                without_pascal(s.constants, pascal_ok || s.pascal_values)
            }
            Kind::LocalConstant { pascal_ok } => {
                without_pascal(s.local_constants, pascal_ok || s.pascal_values)
            }
            Kind::Parameter => s.parameters,
            Kind::EnumMember => s.enum_members,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kind::Type(label) => label,
            Kind::Function => "function",
            Kind::Variable { .. } => "variable",
            Kind::Constant { .. } | Kind::LocalConstant { .. } => "constant",
            Kind::Parameter => "parameter",
            Kind::EnumMember => "enum member",
        }
    }
}

fn without_pascal(bits: u8, pascal_ok: bool) -> u8 {
    if pascal_ok {
        bits
    } else {
        bits & !PASCAL
    }
}

/// A plain primitive literal: such a value is data, never a component or a class.
fn is_primitive_literal(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::BooleanLiteral(_) => true,
        Expression::TemplateLiteral(t) => t.expressions.is_empty(),
        Expression::UnaryExpression(u) => {
            matches!(
                u.operator,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus
            ) && matches!(u.argument, Expression::NumericLiteral(_))
        }
        _ => false,
    }
}

/// `require('x')`, `require('x').y`, `await import('x')`: the name mirrors another module.
fn is_import_like(expr: &Expression<'_>) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::CallExpression(call) => call.callee.is_specific_id("require"),
        Expression::StaticMemberExpression(m) => is_import_like(&m.object),
        Expression::ComputedMemberExpression(m) => is_import_like(&m.object),
        Expression::AwaitExpression(a) => is_import_like(&a.argument),
        Expression::ImportExpression(_) => true,
        _ => false,
    }
}

/// `expr.name`, `expr?.name`, `expr['name']` (behind TS wrappers): the value of a field that
/// already has this name.
fn reads_same_name(expr: &Expression<'_>, name: &str) -> bool {
    match helpers::unwrap_expr(expr) {
        Expression::StaticMemberExpression(m) => m.property.name == name,
        Expression::ComputedMemberExpression(m) => {
            helpers::string_value(&m.expression) == Some(name)
        }
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::StaticMemberExpression(m) => m.property.name == name,
            ChainElement::ComputedMemberExpression(m) => {
                helpers::string_value(&m.expression) == Some(name)
            }
            _ => false,
        },
        _ => false,
    }
}

/// The symbol is used as a shorthand property (`{ user_id }`): its name doubles as an object key,
/// which follows the data format it is written to.
fn used_as_shorthand_key(ctx: &Ctx<'_>, symbol: SymbolId) -> bool {
    ctx.references_of(symbol).any(
        |r| matches!(ctx.parent_kind(r.node_id()), Some(AstKind::ObjectProperty(p)) if p.shorthand),
    )
}

/// Whether the file is skipped (see [`skipped_path`]), computed once, when it has a finding.
fn skip_file(ctx: &mut Ctx<'_>) -> bool {
    if let Some(skip) = ctx.state::<SkipFile>().0 {
        return skip;
    }
    let skip = skipped_path(ctx) || lint_disabled_file(ctx);
    ctx.state::<SkipFile>().0 = Some(skip);
    skip
}

#[derive(Default)]
struct SkipFile(Option<bool>);

/// The file opts out of linting as a whole: an `eslint-disable` comment without a rule list
/// before its first statement. Such files are generated or copied from elsewhere (bindings,
/// bundles), and their names follow that origin.
fn lint_disabled_file(ctx: &Ctx<'_>) -> bool {
    let program = ctx.nodes().program();
    let first = program
        .body
        .first()
        .map_or(program.span.end, |s| s.span().start);
    ctx.comments()
        .iter()
        .take_while(|c| c.span.start < first)
        .any(|c| {
            let text = ctx.text(c.content_span());
            let directive = text.split(" -- ").next().unwrap_or("").trim();
            directive == "eslint-disable"
        })
}

/// The file is vendored, mirrors another package, or was generated: `skip_dirs` names a folder
/// of its path, or its file name says it was generated.
fn skipped_path(ctx: &Ctx<'_>) -> bool {
    let path = ctx.path();
    let mut segments = path.split('/');
    let file = segments.next_back().unwrap_or("");
    if file.to_ascii_lowercase().contains("generated") {
        return true;
    }
    let dirs: Vec<&str> = segments.collect();
    ctx.param_list("skip_dirs")
        .any(|skip| dirs.iter().any(|d| d.eq_ignore_ascii_case(skip)))
}

/// A function or class expression whose name repeats the property or field it is stored under.
fn stored_under_property(ctx: &Ctx<'_>, node: &AstNode<'_>) -> bool {
    matches!(
        ctx.outer_parent(node).map(AstNode::kind),
        Some(
            AstKind::ObjectProperty(_)
                | AstKind::AssignmentExpression(_)
                | AstKind::PropertyDefinition(_)
                | AstKind::AccessorProperty(_)
        )
    )
}

/// Words that may stand between a doc comment and the name it documents.
const MODIFIERS: &[&str] = &[
    "export",
    "default",
    "declare",
    "async",
    "function",
    "class",
    "const",
    "let",
    "var",
    "public",
    "private",
    "protected",
    "static",
    "readonly",
    "abstract",
    "override",
    "get",
    "set",
];

/// The declaration starting at `start` is documented as `@deprecated`: it is kept under its old
/// name for compatibility, so renaming it is not an option.
fn deprecated(ctx: &Ctx<'_>, start: u32) -> bool {
    let comments = ctx.comments();
    let before = comments.partition_point(|c| c.span.end <= start);
    let Some(comment) = before.checked_sub(1).and_then(|i| comments.get(i)) else {
        return false;
    };
    if !ctx.text(comment.span).contains("@deprecated") {
        return false;
    }
    ctx.text(Span::new(comment.span.end, start))
        .split_whitespace()
        .all(|word| MODIFIERS.contains(&word))
}

/// Where the statement declaring `decl` starts (before `export`, `const`, …).
fn statement_start(ctx: &Ctx<'_>, decl: &AstNode<'_>) -> u32 {
    let mut node = decl;
    if matches!(node.kind(), AstKind::VariableDeclarator(_)) {
        if let Some(declaration) = ctx.parent(node) {
            node = declaration;
        }
    }
    if let Some(parent) = ctx.parent(node) {
        if matches!(
            parent.kind(),
            AstKind::ExportNamedDeclaration(_) | AstKind::ExportDefaultDeclaration(_)
        ) {
            return parent.span().start;
        }
    }
    node.span().start
}

/// `namespace Kind { export const Local = 'local'; }`: an exported namespace variable is read as
/// a member (`Kind.Local`), like an enum member or an object property.
fn namespace_member(ctx: &Ctx<'_>, declarator: &AstNode<'_>) -> bool {
    let export = ctx
        .parent(declarator)
        .and_then(|declaration| ctx.parent(declaration));
    match export {
        Some(export) if matches!(export.kind(), AstKind::ExportNamedDeclaration(_)) => {
            matches!(ctx.parent_kind(export), Some(AstKind::TSModuleBlock(_)))
        }
        _ => false,
    }
}

/// The declaring `VariableDeclaration` is at module or namespace level.
fn at_module_level(ctx: &Ctx<'_>, declarator: &AstNode<'_>) -> bool {
    let Some(declaration) = ctx.parent(declarator) else {
        return false;
    };
    let mut parent = ctx.parent(declaration);
    if let Some(AstKind::ExportNamedDeclaration(_)) = parent.map(AstNode::kind) {
        parent = parent.and_then(|p| ctx.parent(p));
    }
    matches!(
        parent.map(AstNode::kind),
        Some(AstKind::Program(_) | AstKind::TSModuleBlock(_))
    )
}

/// Parameters of a function that has a body (not of overload signatures, `declare` functions
/// or function types), and not TypeScript parameter properties.
fn real_parameter(ctx: &Ctx<'_>, param_list: Option<&AstNode<'_>>) -> bool {
    let Some(list) = param_list else {
        return false;
    };
    let AstKind::FormalParameters(params) = list.kind() else {
        return false;
    };
    if params.kind == FormalParameterKind::Signature {
        return false;
    }
    match ctx.parent_kind(list) {
        Some(AstKind::Function(f)) => f.body.is_some(),
        Some(AstKind::ArrowFunctionExpression(_)) => true,
        _ => false,
    }
}

fn simple_binding(pattern: &BindingPattern<'_>) -> bool {
    match pattern {
        BindingPattern::BindingIdentifier(_) => true,
        BindingPattern::AssignmentPattern(p) => {
            matches!(p.left, BindingPattern::BindingIdentifier(_))
        }
        _ => false,
    }
}

/// What kind of name the symbol declared by `decl` is, or `None` when it is not checked.
fn kind_of(ctx: &Ctx<'_>, decl: &AstNode<'_>) -> Option<Kind> {
    match decl.kind() {
        AstKind::VariableDeclarator(d) => {
            if !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                return None; // destructured: the names come from the value's shape
            }
            if d.init.as_ref().is_some_and(is_import_like) || namespace_member(ctx, decl) {
                return None;
            }
            if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&d.id, &d.init) {
                if reads_same_name(init, id.name.as_str()) {
                    return None; // `const user_id = body.user_id`: mirrors a field, like destructuring
                }
            }
            let pascal_ok = !d.init.as_ref().is_some_and(is_primitive_literal);
            if d.kind != VariableDeclarationKind::Const {
                Some(Kind::Variable { pascal_ok })
            } else if at_module_level(ctx, decl) {
                Some(Kind::Constant { pascal_ok })
            } else {
                Some(Kind::LocalConstant { pascal_ok })
            }
        }
        AstKind::FormalParameter(p) => {
            if p.accessibility.is_some() || p.readonly || p.r#override {
                return None; // a parameter property: a class field
            }
            (simple_binding(&p.pattern) && real_parameter(ctx, ctx.parent(decl)))
                .then_some(Kind::Parameter)
        }
        AstKind::FormalParameterRest(r) => {
            (matches!(r.rest.argument, BindingPattern::BindingIdentifier(_))
                && real_parameter(ctx, ctx.parent(decl)))
            .then_some(Kind::Parameter)
        }
        AstKind::CatchParameter(c) => simple_binding(&c.pattern).then_some(Kind::Parameter),
        AstKind::Function(f) => {
            if f.declare || (f.is_expression() && stored_under_property(ctx, decl)) {
                return None;
            }
            Some(Kind::Function)
        }
        AstKind::Class(c) => {
            if c.declare || (c.is_expression() && stored_under_property(ctx, decl)) {
                return None;
            }
            Some(Kind::Type("class"))
        }
        AstKind::TSInterfaceDeclaration(i) => (!i.declare).then_some(Kind::Type("interface")),
        AstKind::TSTypeAliasDeclaration(t) => (!t.declare).then_some(Kind::Type("type alias")),
        AstKind::TSEnumDeclaration(e) => (!e.declare).then_some(Kind::Type("enum")),
        AstKind::TSTypeParameter(_) => Some(Kind::Type("type parameter")),
        AstKind::TSEnumMember(m) => {
            matches!(m.id, TSEnumMemberName::Identifier(_)).then_some(Kind::EnumMember)
        }
        _ => None,
    }
}

impl JsRule for NamingConvention {
    fn meta(&self) -> &'static RuleMeta {
        &NAMING_CONVENTION
    }

    fn types(&self) -> &'static [AstType] {
        &[AstType::MethodDefinition, AstType::PropertyDefinition]
    }

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &mut Ctx<'a>) {
        let (key, computed, accessor, has_override) = match node.kind() {
            AstKind::MethodDefinition(m) => {
                if m.kind == MethodDefinitionKind::Constructor
                    || (m.value.body.is_none()
                        && m.r#type != MethodDefinitionType::TSAbstractMethodDefinition)
                {
                    return; // constructors and overload signatures
                }
                let accessor = matches!(
                    m.kind,
                    MethodDefinitionKind::Get | MethodDefinitionKind::Set
                );
                (
                    &m.key,
                    m.computed,
                    accessor,
                    m.r#override || !m.decorators.is_empty(),
                )
            }
            AstKind::PropertyDefinition(p) => (
                &p.key,
                p.computed,
                true,
                p.r#override || p.declare || !p.decorators.is_empty(),
            ),
            _ => return,
        };
        if computed || has_override {
            return;
        }
        let (name, span) = match key {
            PropertyKey::StaticIdentifier(id) => (id.name.as_str(), id.span),
            PropertyKey::PrivateIdentifier(id) => (id.name.as_str(), id.span),
            _ => return, // quoted or numeric keys
        };
        let settings = ctx.rule_memo(Settings::load);
        let allowed = if accessor {
            settings.properties
        } else {
            settings.methods
        };
        if allowed == 0 || classify(name, settings).is_none_or(|bits| bits & allowed != 0) {
            return;
        }
        if settings.exempt_name(name) {
            return;
        }
        if class_follows_other_type(ctx, node)
            || ctx.in_ambient_context(node)
            || deprecated(ctx, node.span().start)
            || skip_file(ctx)
        {
            return;
        }
        let kind = if accessor { "property" } else { "method" };
        ctx.emit(
            span,
            format!("Use {} for the {kind} '{name}'", format_names(allowed)),
        );
    }

    fn run_once<'a>(&self, ctx: &mut Ctx<'a>) {
        let settings = ctx.rule_memo(Settings::load);
        let scoping = ctx.scoping();
        let skip = SymbolFlags::Import
            | SymbolFlags::TypeImport
            | SymbolFlags::Ambient
            | SymbolFlags::NamespaceModule
            | SymbolFlags::ValueModule;
        let type_like = SymbolFlags::Class
            | SymbolFlags::Interface
            | SymbolFlags::TypeAlias
            | SymbolFlags::Enum
            | SymbolFlags::TypeParameter;
        // Plain camelCase value names and PascalCase type names pass with the defaults: skip
        // them before any lookup.
        let values_camel = CAMEL
            & settings.functions
            & settings.variables
            & settings.constants
            & settings.local_constants
            & settings.parameters
            != 0;
        let types_pascal = settings.types & PASCAL != 0;
        // Top-level names of scripts (no import or export) are globals that other scripts and
        // pages call by name: they are an interface, not a local choice.
        let script = !ctx.is_module();
        let root = scoping.root_scope_id();
        let mut found: Vec<(Span, String)> = Vec::new();
        for symbol in scoping.symbol_ids() {
            let flags = scoping.symbol_flags(symbol);
            if flags.intersects(skip)
                || (flags.contains(SymbolFlags::EnumMember) && settings.enum_members == 0)
            {
                continue;
            }
            let name = scoping.symbol_name(symbol);
            let bytes = name.as_bytes();
            if let Some(first) = bytes.first() {
                if bytes.iter().all(u8::is_ascii_alphanumeric) {
                    let is_type = flags.intersects(type_like);
                    if (is_type && types_pascal && first.is_ascii_uppercase())
                        || (!is_type
                            && !flags.contains(SymbolFlags::EnumMember)
                            && values_camel
                            && first.is_ascii_lowercase())
                    {
                        continue;
                    }
                }
            }
            let Some(bits) = classify(name, settings) else {
                continue;
            };
            if bits == CAMEL | PASCAL | UPPER | SNAKE {
                continue; // e.g. `$1`: fits every style
            }
            if script && scoping.symbol_scope_id(symbol) == root {
                continue;
            }
            let decl = ctx.symbol_declaration(symbol);
            let Some(kind) = kind_of(ctx, decl) else {
                continue;
            };
            let allowed = kind.allowed(settings);
            if allowed == 0 || bits & allowed != 0 || settings.exempt_name(name) {
                continue;
            }
            if ctx.in_ambient_context(decl)
                || used_as_shorthand_key(ctx, symbol)
                || deprecated(ctx, statement_start(ctx, decl))
            {
                continue;
            }
            found.push((
                ctx.symbol_span(symbol),
                format!(
                    "Use {} for the {} '{name}'",
                    format_names(allowed),
                    kind.label()
                ),
            ));
        }
        if found.is_empty() || skip_file(ctx) {
            return;
        }
        found.sort_by_key(|(span, _)| span.start);
        for (span, message) in found {
            ctx.emit(span, message);
        }
    }
}

/// The member belongs to a class that extends or implements another type, or is declared with
/// `declare`: its names may be dictated by that type.
fn class_follows_other_type(ctx: &Ctx<'_>, member: &AstNode<'_>) -> bool {
    let Some(body) = ctx.parent(member) else {
        return true;
    };
    match ctx.parent_kind(body) {
        Some(AstKind::Class(class)) => {
            class.super_class.is_some() || !class.implements.is_empty() || class.declare
        }
        _ => true,
    }
}
