//! Helpers shared by the batch-11 rules (maintainability smells): bounded subtree visits, call
//! arguments, catch clauses, logger recognition, type names and token streams. Nothing here walks
//! the whole tree: every function looks at one bounded subtree (a catch block, a method body, an
//! expression) or at a node's ancestors.

use crate::prelude::*;

// ------------------------------------------------------------------------- per-file gate

/// Per-file "does the source text contain `needle`?", computed on first use. Lets a rule skip
/// resolver work in the (many) files that cannot contain what it looks for.
#[derive(Default)]
pub(super) struct Gate(Option<bool>);

impl Gate {
    pub(super) fn open(&mut self, ctx: &Ctx<'_>, needle: &str) -> bool {
        *self.0.get_or_insert_with(|| ctx.source().contains(needle))
    }
}

// ------------------------------------------------------------------------- calls

/// The argument expressions of a call or object creation (comments skipped).
pub(super) fn args(call: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    call.field(Field::Arguments)
        .into_iter()
        .flat_map(named_children)
}

/// Number of arguments of a call or object creation.
pub(super) fn arg_count(call: Node<'_>) -> usize {
    args(call).count()
}

/// The `i`-th argument of a call or object creation.
pub(super) fn arg(call: Node<'_>, i: usize) -> Option<Node<'_>> {
    args(call).nth(i)
}

/// Whether `expr` names the type `fqn` (the receiver of a static call like `Pattern.compile`).
pub(super) fn names_type<'t>(ctx: &Ctx<'t>, expr: Node<'t>, fqn: &str) -> bool {
    let simple = fqn.rsplit('.').next().unwrap_or(fqn);
    let text = ctx.text(expr);
    if text != simple && text != fqn {
        return false;
    }
    ctx.type_name_of(expr).as_deref() == Some(fqn)
}

/// The last segment of a dotted name (`java.lang.Integer` → `Integer`).
pub(super) fn last_segment(text: &str) -> &str {
    text.rsplit('.').next().unwrap_or(text).trim()
}

// ------------------------------------------------------------------------- string constants

/// The runtime value of a string constant: plain string literals, possibly joined with `+` and
/// wrapped in parentheses, and names of `static final String` fields of this file initialized
/// that way. `None` for text blocks, Unicode escapes and anything else.
pub(super) fn string_constant<'t>(ctx: &Ctx<'t>, node: Node<'t>, depth: u32) -> Option<String> {
    if depth > 8 {
        return None;
    }
    let node = syntax::strip_parens(node);
    match kind(node) {
        "string_literal" => {
            if ctx.text(node).starts_with("\"\"\"") {
                return None;
            }
            unescape(syntax::string_literal_content(node, ctx.source())?)
        }
        "binary_expression" if syntax::operator_str(node) == "+" => {
            let l = string_constant(ctx, node.field(Field::Left)?, depth + 1)?;
            let r = string_constant(ctx, node.field(Field::Right)?, depth + 1)?;
            Some(l + &r)
        }
        "identifier" => {
            let decl = ctx.declaration_of(node)?;
            if decl.kind != VarKind::Field {
                return None;
            }
            let stmt = decl.statement;
            // Interface constants are implicitly static and final.
            let constant = kind(stmt) == "constant_declaration"
                || (syntax::has_modifier(stmt, "static") && syntax::has_modifier(stmt, "final"));
            if !constant {
                return None;
            }
            string_constant(ctx, decl.value?, depth + 1)
        }
        _ => None,
    }
}

/// Decode the escape sequences of Java string-literal source text. `None` on Unicode escapes
/// or unknown escapes.
pub(super) fn unescape(raw: &str) -> Option<String> {
    if !raw.contains('\\') {
        return Some(raw.to_string());
    }
    let mut out = String::with_capacity(raw.len());
    let mut it = raw.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next()? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            's' => out.push(' '),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            _ => return None,
        }
    }
    Some(out)
}

// ------------------------------------------------------------------------- bounded visits

/// What to do after visiting a node in [`visit`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Walk {
    /// Descend into the node's children.
    Continue,
    /// Don't look inside this node.
    Skip,
    /// End the whole visit.
    Stop,
}

/// Pre-order visit of the **named** descendants of `root` (not `root` itself), in source order.
/// Returns `true` when the callback stopped the visit. Linear in the size of the subtree.
pub(super) fn visit<'t>(root: Node<'t>, mut f: impl FnMut(Node<'t>) -> Walk) -> bool {
    let mut cursor = root.walk();
    if !cursor.goto_first_child() {
        return false;
    }
    loop {
        let n = cursor.node();
        let action = if n.is_named() { f(n) } else { Walk::Skip };
        match action {
            Walk::Stop => return true,
            Walk::Continue if cursor.goto_first_child() => continue,
            _ => {}
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() || cursor.node().id() == root.id() {
                return false;
            }
        }
    }
}

/// Nodes whose code runs in another context: lambdas and class bodies (anonymous and local
/// classes).
pub(super) fn is_other_context(n: Node<'_>) -> bool {
    matches!(kind(n), "lambda_expression" | "class_body")
}

/// Whether `node` is, or contains, the identifier `name` (a text pre-filter first).
pub(super) fn mentions<'t>(ctx: &Ctx<'t>, node: Node<'t>, name: &str) -> bool {
    if !ctx.text(node).contains(name) {
        return false;
    }
    if kind(node) == "identifier" {
        return ctx.text(node) == name;
    }
    visit(node, |n| {
        if kind(n) == "identifier" && ctx.text(n) == name {
            Walk::Stop
        } else {
            Walk::Continue
        }
    })
}

// ------------------------------------------------------------------------- catch clauses

/// The `catch_formal_parameter` of a `catch_clause`.
pub(super) fn catch_param(catch_clause: Node<'_>) -> Option<Node<'_>> {
    named_children(catch_clause).find(|c| kind(*c) == "catch_formal_parameter")
}

/// The caught types of a `catch_clause` (one, or several for a multi-catch).
pub(super) fn catch_types(catch_clause: Node<'_>) -> impl Iterator<Item = Node<'_>> {
    catch_param(catch_clause)
        .and_then(|p| named_children(p).find(|c| kind(*c) == "catch_type"))
        .into_iter()
        .flat_map(named_children)
}

/// The name of the caught exception variable (`None` for `_`).
pub(super) fn catch_param_name<'t>(catch_clause: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let name = catch_param(catch_clause)?.field(Field::Name)?;
    (kind(name) == "identifier" && ctx.text(name) != "_").then(|| ctx.text(name))
}

// ------------------------------------------------------------------------- types

/// The type node without type arguments (`Foo` of `Foo<T>`).
pub(super) fn type_base(type_node: Node<'_>) -> Node<'_> {
    if kind(type_node) == "generic_type" {
        syntax::first_named_child(type_node).unwrap_or(type_node)
    } else {
        type_node
    }
}

/// Fully-qualified name of a class type node as seen from its position (imports, this file,
/// `java.lang`, and imported library types). `None` when unresolvable.
pub(super) fn type_fqn<'t>(ctx: &Ctx<'t>, type_node: Node<'t>) -> Option<String> {
    let base = type_base(type_node);
    if !matches!(kind(base), "type_identifier" | "scoped_type_identifier") {
        return None;
    }
    let text = ctx.text(base);
    if text.contains(|c: char| c.is_whitespace() || c == '/') {
        return None;
    }
    ctx.resolve_type_name(text, base)
}

/// Whether the class type node names the `java.lang` class `fqn`. A simple name the resolver
/// cannot place (not declared in this file, not imported by name) is the implicitly imported
/// `java.lang` class: the resolver's JDK table does not list every `java.lang` class.
pub(super) fn is_java_lang_type<'t>(ctx: &Ctx<'t>, type_node: Node<'t>, fqn: &str) -> bool {
    let base = type_base(type_node);
    let text = ctx.text(base);
    if text == fqn {
        return true;
    }
    match type_fqn(ctx, type_node) {
        Some(f) => f == fqn,
        None => kind(base) == "type_identifier" && fqn.strip_prefix("java.lang.") == Some(text),
    }
}

/// The type declaration of this file whose FQN is `fqn` (nested ones included, bounded depth).
pub(super) fn find_type_decl<'t>(
    ctx: &Ctx<'t>,
    scope: Node<'t>,
    fqn: &str,
    depth: u32,
) -> Option<Node<'t>> {
    if depth > 4 {
        return None;
    }
    let simple = last_segment(fqn);
    let members: Vec<Node<'t>> = if kind(scope) == "program" {
        named_children(scope).collect()
    } else {
        syntax::type_body(scope)
            .into_iter()
            .flat_map(|b| ctx.members(b))
            .collect()
    };
    for m in members {
        if !syntax::is_type_declaration(m) {
            continue;
        }
        if syntax::name_of(m, ctx.source()) == simple && ctx.class_fqn(m).as_deref() == Some(fqn) {
            return Some(m);
        }
        if let Some(found) = find_type_decl(ctx, m, fqn, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// Whether `ann` (`annotation` / `marker_annotation`) is one of `fqns`, through a single-type
/// import, an on-demand import, or a fully-qualified name.
pub(super) fn annotation_is(ctx: &Ctx<'_>, ann: Node<'_>, fqns: &[&str]) -> bool {
    let Some(name) = ann.field(Field::Name) else {
        return false;
    };
    let text = ctx.text(name);
    if text.contains('.') {
        return fqns.contains(&text);
    }
    if let Some(imp) = ctx
        .imports()
        .iter()
        .find(|i| !i.is_static && !i.is_wildcard && i.simple_name() == text)
    {
        return fqns.contains(&imp.path);
    }
    ctx.imports()
        .iter()
        .filter(|i| !i.is_static && i.is_wildcard)
        .any(|imp| {
            fqns.iter()
                .any(|f| f.strip_suffix(text).and_then(|q| q.strip_suffix('.')) == Some(imp.path))
        })
}

// ------------------------------------------------------------------------- loggers

/// Logging API families.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LogFamily {
    /// SLF4J, Log4j 2, JBoss Logging, Logback.
    Modern,
    /// Log4j 1, Commons Logging.
    Legacy,
    /// `java.util.logging`.
    Jul,
}

fn family_of_fqn(fqn: &str) -> Option<LogFamily> {
    match fqn {
        "org.slf4j.Logger"
        | "org.apache.logging.log4j.Logger"
        | "org.jboss.logging.Logger"
        | "org.apache.logging.log4j.core.Logger"
        | "ch.qos.logback.classic.Logger" => Some(LogFamily::Modern),
        "org.apache.log4j.Logger"
        | "org.apache.log4j.Category"
        | "org.apache.commons.logging.Log" => Some(LogFamily::Legacy),
        "java.util.logging.Logger" => Some(LogFamily::Jul),
        _ => None,
    }
}

/// The single logging family whose package the file imports (`None` when none or several).
fn imported_family(ctx: &Ctx<'_>) -> Option<LogFamily> {
    let mut found = None;
    for i in ctx.imports() {
        let p = i.path;
        let f = if p.starts_with("org.slf4j")
            || p.starts_with("org.apache.logging.log4j")
            || p.starts_with("org.jboss.logging")
            || p.starts_with("lombok.extern.slf4j")
            || p.starts_with("lombok.extern.log4j")
        {
            LogFamily::Modern
        } else if p.starts_with("org.apache.log4j") || p.starts_with("org.apache.commons.logging") {
            LogFamily::Legacy
        } else if p.starts_with("java.util.logging") {
            LogFamily::Jul
        } else {
            continue;
        };
        if found.is_some_and(|x| x != f) {
            return None;
        }
        found = Some(f);
    }
    found
}

/// Conventional names of logger fields (trusted for inherited or generated fields only when
/// the file imports a logging API).
const CONVENTIONAL_LOGGERS: &[&str] = &["log", "logger", "LOG", "LOGGER", "Log", "Logger"];

/// The logging family of a call receiver: a variable or field whose declared type is a known
/// logger, or a conventional logger name in a file importing one logging API.
pub(super) fn logger_family<'t>(ctx: &Ctx<'t>, receiver: Node<'t>) -> Option<LogFamily> {
    let ident = match kind(receiver) {
        "identifier" => receiver,
        "field_access" => {
            let obj = receiver.field(Field::Object)?;
            if kind(obj) != "this" {
                return None;
            }
            receiver.field(Field::Field)?
        }
        _ => return None,
    };
    if let Some(decl) = ctx.declaration_of(ident) {
        let ty = decl.type_node?;
        let fqn = type_fqn(ctx, ty)?;
        return family_of_fqn(&fqn);
    }
    if CONVENTIONAL_LOGGERS.contains(&ctx.text(ident)) {
        return imported_family(ctx);
    }
    None
}

/// Level methods of the logging APIs, by severity.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum LogLevel {
    /// `trace`, `debug`, `fine`, `finer`, `finest`, `config`.
    Diagnostic,
    /// `info`.
    Info,
    /// `warn`, `warning`.
    Warn,
    /// `error`, `fatal`, `severe`.
    Error,
}

/// The level of a logging call on a recognized logger, `None` when `call` is not one.
/// `log(Level.X, …)` (`java.util.logging`, Log4j 2) reads the level from its first argument.
pub(super) fn log_call_level<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> Option<LogLevel> {
    let name = ctx.call_name(call)?;
    let level = match name {
        "error" | "fatal" | "severe" => LogLevel::Error,
        "warn" | "warning" => LogLevel::Warn,
        "info" => LogLevel::Info,
        "debug" | "trace" | "fine" | "finer" | "finest" | "config" => LogLevel::Diagnostic,
        "log" => {
            let first = arg(call, 0)?;
            let text = ctx.text(first);
            let lvl = last_segment(text);
            match lvl {
                "SEVERE" | "ERROR" | "FATAL" => LogLevel::Error,
                "WARNING" | "WARN" => LogLevel::Warn,
                "INFO" => LogLevel::Info,
                "FINE" | "FINER" | "FINEST" | "CONFIG" | "DEBUG" | "TRACE" | "ALL" => {
                    LogLevel::Diagnostic
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let receiver = call.field(Field::Object)?;
    logger_family(ctx, receiver)?;
    Some(level)
}

// ------------------------------------------------------------------------- tokens

/// Tokens (leaves) of a subtree in source order, comments skipped. Stops early (returning
/// `false`) when `f` returns `false`. Iterative.
pub(super) fn for_each_token<'t>(node: Node<'t>, mut f: impl FnMut(Node<'t>) -> bool) -> bool {
    let mut c = node.walk();
    let mut depth = 0u32;
    loop {
        let n = c.node();
        let descend = !syntax::is_comment(n) && c.goto_first_child();
        if descend {
            depth += 1;
            continue;
        }
        if !syntax::is_comment(n) && !f(n) {
            return false;
        }
        loop {
            if depth == 0 {
                return true;
            }
            if c.goto_next_sibling() {
                break;
            }
            if !c.goto_parent() {
                return true;
            }
            depth -= 1;
        }
    }
}
