//! Static reading of JavaScript and TypeScript ESLint configurations (`eslint.config.js` and
//! `.mjs`, `.cjs`, `.ts`, `.mts`, `.cts`; `.eslintrc.js` and `.cjs`).
//!
//! The file is parsed (Oxc) and **never run**. Only static data is evaluated: literals, object
//! and array literals (with spreads of static values), `module.exports =` / `export default`, the
//! data helpers of ESLint and typescript-eslint (`defineConfig(…)`, `globalIgnores(…)`,
//! `tseslint.config(…)`), and the `const`s of the same file whose initializer is itself static
//! and that nothing in the file modifies. Well-known shared configurations are recognized by
//! name without being evaluated (`js.configs.recommended` is `eslint:recommended`,
//! `tseslint.configs.recommended` is `plugin:@typescript-eslint/recommended`, a plugin's
//! `configs['flat/recommended']` is `plugin:<name>/recommended`). Anything else — calls of other
//! functions, conditions, imports of local files, `process.env`, … — becomes a stand-in value, a
//! [`Marker`], which the migration lists as "not statically readable" instead of guessing.
//!
//! Stand-ins travel inside the `serde_json::Value` the JSON path reads too: an object with the
//! single key [`MARKER`], and, for spreads whose keys are unknown (`{ files, ...preset }`), a list
//! under [`SPREADS`] in the object. Both keys start with a NUL character, which no real
//! configuration uses; [`is_reserved`] tells them apart.

use serde_json::{Map, Value};

/// Key of the object that stands for a value that was not read: `{MARKER: {…}}`.
pub const MARKER: &str = "\u{0}mote";
/// Key of an object's list of spreads whose keys are unknown (each a [`MARKER`] object).
pub const SPREADS: &str = "\u{0}spreads";

/// Whether an object key is one of the reader's own ([`MARKER`], [`SPREADS`]).
pub fn is_reserved(key: &str) -> bool {
    key.starts_with('\u{0}')
}

/// A value that was not read: a recognized shared configuration, or a dynamic expression.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marker {
    /// The shared configuration's name (`eslint:recommended`); `None` for a dynamic value.
    pub preset: Option<String>,
    /// Only the preset's `rules` (`js.configs.recommended.rules`).
    pub rules_only: bool,
    /// The expression as written, on one line and shortened.
    pub source: String,
    /// 1-based line of the expression.
    pub line: u32,
    /// Why a dynamic value is not static (`the result of a function call`).
    pub reason: String,
    /// For a spread in an object: the keys written before it, which it may replace.
    pub before: Vec<String>,
}

impl Marker {
    /// The stand-in value.
    #[cfg(feature = "js")]
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        if let Some(p) = &self.preset {
            m.insert("preset".into(), Value::String(p.clone()));
        }
        if self.rules_only {
            m.insert("rules_only".into(), Value::Bool(true));
        }
        m.insert("source".into(), Value::String(self.source.clone()));
        m.insert("line".into(), Value::from(self.line));
        if !self.reason.is_empty() {
            m.insert("reason".into(), Value::String(self.reason.clone()));
        }
        if !self.before.is_empty() {
            m.insert(
                "before".into(),
                Value::Array(self.before.iter().cloned().map(Value::String).collect()),
            );
        }
        let mut outer = Map::new();
        outer.insert(MARKER.into(), Value::Object(m));
        Value::Object(outer)
    }

    /// The stand-in `v` is, if it is one.
    pub fn of(v: &Value) -> Option<Marker> {
        let m = v.as_object()?.get(MARKER)?.as_object()?;
        let s = |k: &str| m.get(k).and_then(Value::as_str).map(str::to_string);
        Some(Marker {
            preset: s("preset"),
            rules_only: m.get("rules_only").and_then(Value::as_bool) == Some(true),
            source: s("source").unwrap_or_default(),
            line: m
                .get("line")
                .and_then(Value::as_u64)
                .and_then(|l| u32::try_from(l).ok())
                .unwrap_or(0),
            reason: s("reason").unwrap_or_default(),
            before: m
                .get("before")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }
}

/// The spreads of `obj` whose keys are unknown.
pub fn spreads(obj: &Map<String, Value>) -> Vec<Marker> {
    obj.get(SPREADS)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Marker::of).collect())
        .unwrap_or_default()
}

/// The keys of `obj` without the reader's own.
pub fn keys(obj: &Map<String, Value>) -> impl Iterator<Item = &String> {
    obj.keys().filter(|k| !is_reserved(k))
}

/// Whether `v` holds a stand-in anywhere.
pub fn contains_marker(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.iter().any(contains_marker),
        Value::Object(o) => o.keys().any(|k| is_reserved(k)) || o.values().any(contains_marker),
        _ => false,
    }
}

/// `v` as JSON for a message, each stand-in written as `<its source>`.
pub fn show(v: &Value) -> String {
    fn clean(v: &Value) -> Value {
        if let Some(m) = Marker::of(v) {
            return Value::String(format!("<{}>", m.source));
        }
        match v {
            Value::Array(a) => Value::Array(a.iter().map(clean).collect()),
            Value::Object(o) => Value::Object(
                o.iter()
                    .filter(|(k, _)| !is_reserved(k))
                    .map(|(k, v)| (k.clone(), clean(v)))
                    .collect(),
            ),
            v => v.clone(),
        }
    }
    clean(v).to_string()
}

#[cfg(feature = "js")]
pub use reader::read;

#[cfg(feature = "js")]
mod reader {
    use super::{is_reserved, Marker, SPREADS};
    use oxc_allocator::Allocator;
    use oxc_ast::ast::*;
    use oxc_ast_visit::{walk, Visit};
    use oxc_parser::{ParseOptions, Parser};
    use oxc_span::{GetSpan, SourceType, Span};
    use serde_json::{Map, Number, Value};
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    /// How deeply values may nest (arrays, objects, references to `const`s) before the rest is
    /// treated as not readable.
    const MAX_DEPTH: u32 = 100;
    /// How many values may be evaluated in all (each reference to a `const` evaluates it again).
    const MAX_STEPS: u32 = 200_000;
    /// Longest excerpt of an expression in a message, in characters.
    const EXCERPT: usize = 60;

    /// What a file exports, read statically.
    #[derive(Debug)]
    pub struct Read {
        /// The configuration: for a flat configuration, an array of configuration objects
        /// (nested arrays and the helpers' arguments flattened, as ESLint does).
        pub value: Value,
        /// The line of each element of a flat configuration array.
        pub lines: Vec<u32>,
    }

    /// Read the configuration `text` exports. `flat`: a flat configuration (`eslint.config.*`:
    /// an exported object is one configuration object of the array); `false` for an eslintrc
    /// file, `None` when the name says neither (an array is then a flat configuration).
    pub fn read(path: &Path, text: &str, flat: Option<bool>) -> Result<Read, String> {
        let is_plain_js = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("js"));
        let source_type = match SourceType::from_path(path) {
            // `.js` is a module or a script depending on the package; decide from the syntax.
            Ok(st) if is_plain_js => st.with_unambiguous(true),
            Ok(st) => st,
            Err(_) => SourceType::unambiguous(),
        };
        let alloc = Allocator::default();
        let options = ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        };
        let ret = Parser::new(&alloc, text, source_type)
            .with_options(options)
            .parse();
        if ret.panicked || !ret.errors.is_empty() {
            let first = ret
                .errors
                .first()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "unrecoverable syntax error".into());
            return Err(format!("cannot parse the file: {first}"));
        }
        let mut reader = Reader::new(text, &ret.program);
        let Some(export) = reader.export.take() else {
            return Err(
                "no `export default …` or `module.exports = …` at the top level of the file: \
                 nothing to read"
                    .into(),
            );
        };
        if reader.exports_modified() {
            let m = reader.marker(
                export.span(),
                "`module.exports` is also changed elsewhere in the file".into(),
            );
            return Ok(Read {
                value: m,
                lines: Vec::new(),
            });
        }
        let flat = match flat {
            Some(f) => f,
            None => matches!(reader.eval_export(&export), Value::Array(_)),
        };
        if !flat {
            return Ok(Read {
                value: reader.eval_export(&export),
                lines: Vec::new(),
            });
        }
        let (values, lines) = reader.config_list(export).into_iter().unzip();
        Ok(Read {
            value: Value::Array(values),
            lines,
        })
    }

    /// What a top-level name stands for.
    enum Binding<'p, 'a> {
        /// A `const` with its initializer.
        Const(&'p Expression<'a>),
        /// An import (or `require`) of a module: the member path below its export.
        Import { module: String, path: Vec<String> },
        /// Anything else (`let`, a function, a destructured value): why it is not read.
        Other(&'static str),
    }

    /// The data helpers whose result is known from their arguments.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Helper {
        /// `defineConfig(…)`, `tseslint.config(…)`: the arguments, arrays flattened.
        Flatten,
        /// `globalIgnores(patterns[, name])`: `{ ignores: patterns }`.
        GlobalIgnores,
        /// `defineConfig(x)` of eslint-define-config: `x`.
        Identity,
    }

    /// The exported expression of the file.
    enum Export<'p, 'a> {
        Expr(&'p Expression<'a>),
        /// `export { name as default }`.
        Name(&'p str, Span),
        /// `export default function …` and the like.
        Other(Span, &'static str),
    }

    impl Export<'_, '_> {
        fn span(&self) -> Span {
            match self {
                Export::Expr(e) => e.span(),
                Export::Name(_, s) | Export::Other(s, _) => *s,
            }
        }
    }

    struct Reader<'p, 'a> {
        src: &'p str,
        line_starts: Vec<u32>,
        bindings: HashMap<&'p str, Binding<'p, 'a>>,
        export: Option<Export<'p, 'a>>,
        /// `module.exports = …` statements at the top level, and anywhere.
        top_exports: usize,
        all_exports: usize,
        /// Names that something in the file may modify (assignments, calls, …).
        modified: HashSet<String>,
        /// `const`s being evaluated (a reference to one of them is a cycle).
        active: Vec<&'p str>,
        depth: u32,
        steps: u32,
    }

    impl<'p, 'a> Reader<'p, 'a> {
        fn new(src: &'p str, program: &'p Program<'a>) -> Reader<'p, 'a> {
            let mut line_starts = vec![0u32];
            line_starts.extend(
                src.bytes()
                    .enumerate()
                    .filter(|(_, b)| *b == b'\n')
                    .map(|(i, _)| i as u32 + 1),
            );
            let mut r = Reader {
                src,
                line_starts,
                bindings: HashMap::new(),
                export: None,
                top_exports: 0,
                all_exports: 0,
                modified: HashSet::new(),
                active: Vec::new(),
                depth: 0,
                steps: 0,
            };
            for stmt in &program.body {
                r.statement(stmt);
            }
            let mut finder = Modifications {
                reader: &r,
                marking: 0,
                marked: HashSet::new(),
                exports: 0,
            };
            finder.visit_program(program);
            let (mut marked, exports) = (finder.marked, finder.exports);
            r.all_exports = exports;
            // A `const` that refers to another value shares it: modifying one modifies both.
            loop {
                let mut more = Vec::new();
                for (name, b) in &r.bindings {
                    if let Binding::Const(init) = b {
                        if marked.contains(*name) {
                            let mut ids = Identifiers(Vec::new());
                            ids.visit_expression(init);
                            more.extend(ids.0.into_iter().filter(|n| !marked.contains(n)));
                        }
                    }
                }
                if more.is_empty() {
                    break;
                }
                marked.extend(more);
            }
            r.modified = marked;
            r
        }

        fn statement(&mut self, stmt: &'p Statement<'a>) {
            match stmt {
                Statement::ImportDeclaration(d) => {
                    if d.import_kind.is_type() {
                        return;
                    }
                    let module = d.source.value.as_str().to_string();
                    for spec in d.specifiers.iter().flatten() {
                        let (local, path) = match spec {
                            ImportDeclarationSpecifier::ImportSpecifier(s) => {
                                if s.import_kind.is_type() {
                                    continue;
                                }
                                let imported = export_name(&s.imported);
                                let path = if imported == "default" {
                                    Vec::new()
                                } else {
                                    vec![imported]
                                };
                                (s.local.name.as_str(), path)
                            }
                            ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                                (s.local.name.as_str(), Vec::new())
                            }
                            ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                                (s.local.name.as_str(), Vec::new())
                            }
                        };
                        self.bindings.insert(
                            local,
                            Binding::Import {
                                module: module.clone(),
                                path,
                            },
                        );
                    }
                }
                Statement::VariableDeclaration(d) => self.declare(d),
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        self.bindings
                            .insert(id.name.as_str(), Binding::Other("a function"));
                    }
                }
                Statement::ClassDeclaration(c) => {
                    if let Some(id) = &c.id {
                        self.bindings
                            .insert(id.name.as_str(), Binding::Other("a class"));
                    }
                }
                Statement::ExportNamedDeclaration(d) => {
                    match &d.declaration {
                        Some(Declaration::VariableDeclaration(v)) => self.declare(v),
                        Some(Declaration::FunctionDeclaration(f)) => {
                            if let Some(id) = &f.id {
                                self.bindings
                                    .insert(id.name.as_str(), Binding::Other("a function"));
                            }
                        }
                        _ => {}
                    }
                    if d.source.is_none() {
                        for s in &d.specifiers {
                            if export_name(&s.exported) == "default" {
                                if let ModuleExportName::IdentifierReference(id) = &s.local {
                                    self.export = Some(Export::Name(id.name.as_str(), s.span));
                                }
                            }
                        }
                    }
                }
                Statement::ExportDefaultDeclaration(d) => {
                    self.export = Some(match d.declaration.as_expression() {
                        Some(e) => Export::Expr(e),
                        None => Export::Other(d.span, "a function or class"),
                    });
                }
                Statement::TSExportAssignment(e) => {
                    self.export = Some(Export::Expr(&e.expression));
                }
                Statement::ExpressionStatement(s) => {
                    if let Expression::AssignmentExpression(a) = s.expression.get_inner_expression()
                    {
                        if a.operator == AssignmentOperator::Assign && is_module_exports(&a.left) {
                            self.top_exports += 1;
                            self.export = Some(Export::Expr(&a.right));
                        }
                    }
                }
                _ => {}
            }
        }

        fn declare(&mut self, d: &'p VariableDeclaration<'a>) {
            for decl in &d.declarations {
                match (&decl.id, &decl.init) {
                    (BindingPattern::BindingIdentifier(id), Some(init)) => {
                        let b = match require_of(init) {
                            Some((module, path)) => Binding::Import { module, path },
                            None if d.kind == VariableDeclarationKind::Const => {
                                Binding::Const(init)
                            }
                            None => {
                                Binding::Other("declared with `let` or `var`, so it may change")
                            }
                        };
                        self.bindings.insert(id.name.as_str(), b);
                    }
                    (BindingPattern::ObjectPattern(pat), Some(init)) => {
                        let required = require_of(init);
                        for prop in &pat.properties {
                            match (&required, prop.key.static_name(), &prop.value) {
                                (
                                    Some((module, path)),
                                    Some(key),
                                    BindingPattern::BindingIdentifier(id),
                                ) if !prop.computed => {
                                    let mut path = path.clone();
                                    path.push(key.into_owned());
                                    self.bindings.insert(
                                        id.name.as_str(),
                                        Binding::Import {
                                            module: module.clone(),
                                            path,
                                        },
                                    );
                                }
                                _ => self.bind_other(&prop.value, "a destructured value"),
                            }
                        }
                        if let Some(rest) = &pat.rest {
                            self.bind_other(&rest.argument, "a destructured value");
                        }
                    }
                    (pattern, _) => self.bind_other(pattern, "a destructured value"),
                }
            }
        }

        fn bind_other(&mut self, pattern: &'p BindingPattern<'a>, why: &'static str) {
            match pattern {
                BindingPattern::BindingIdentifier(id) => {
                    self.bindings.insert(id.name.as_str(), Binding::Other(why));
                }
                BindingPattern::ObjectPattern(p) => {
                    for prop in &p.properties {
                        self.bind_other(&prop.value, why);
                    }
                    if let Some(rest) = &p.rest {
                        self.bind_other(&rest.argument, why);
                    }
                }
                BindingPattern::ArrayPattern(p) => {
                    for el in p.elements.iter().flatten() {
                        self.bind_other(el, why);
                    }
                    if let Some(rest) = &p.rest {
                        self.bind_other(&rest.argument, why);
                    }
                }
                BindingPattern::AssignmentPattern(p) => self.bind_other(&p.left, why),
            }
        }

        /// Whether `module.exports` is assigned somewhere else than at the top level, or changed
        /// (`module.exports.rules = …`, `Object.assign(module.exports, …)`).
        fn exports_modified(&self) -> bool {
            self.top_exports > 0
                && (self.all_exports > self.top_exports
                    || self.modified.contains("module")
                    || self.modified.contains("exports"))
        }

        // ---- values -------------------------------------------------------------------------

        fn line(&self, span: Span) -> u32 {
            self.line_starts.partition_point(|&s| s <= span.start) as u32
        }

        fn excerpt(&self, span: Span) -> String {
            let text = self
                .src
                .get(span.start as usize..span.end as usize)
                .unwrap_or("");
            let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if one_line.chars().count() > EXCERPT {
                let cut: String = one_line.chars().take(EXCERPT - 1).collect();
                format!("{}…", cut.trim_end())
            } else {
                one_line
            }
        }

        /// A dynamic value at `span`.
        fn marker(&self, span: Span, reason: String) -> Value {
            Marker {
                source: self.excerpt(span),
                line: self.line(span),
                reason,
                ..Marker::default()
            }
            .to_value()
        }

        /// The value of `e`.
        fn eval(&mut self, e: &'p Expression<'a>) -> Value {
            if self.depth >= MAX_DEPTH {
                return self.marker(e.span(), "nested too deeply".into());
            }
            self.steps += 1;
            if self.steps > MAX_STEPS {
                return self.marker(e.span(), "too large to read".into());
            }
            self.depth += 1;
            let v = self.eval_inner(e);
            self.depth -= 1;
            v
        }

        fn eval_inner(&mut self, e: &'p Expression<'a>) -> Value {
            let e = e.get_inner_expression();
            match e {
                Expression::BooleanLiteral(b) => Value::Bool(b.value),
                Expression::NullLiteral(_) => Value::Null,
                Expression::NumericLiteral(n) => number(n.value),
                Expression::StringLiteral(s) => Value::String(s.value.as_str().to_string()),
                Expression::TemplateLiteral(t) => self.template(t),
                Expression::RegExpLiteral(r) => Value::String(self.excerpt(r.span)),
                Expression::UnaryExpression(u) => match u.operator {
                    UnaryOperator::UnaryNegation => match self.eval(&u.argument) {
                        Value::Number(n) => number(-n.as_f64().unwrap_or(0.0)),
                        _ => self.marker(u.span, "an expression".into()),
                    },
                    UnaryOperator::Void => Value::Null,
                    _ => self.marker(u.span, "an expression".into()),
                },
                Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
                    match (self.eval(&b.left), self.eval(&b.right)) {
                        (Value::String(l), Value::String(r)) => Value::String(l + &r),
                        (Value::String(l), Value::Number(r)) => Value::String(format!("{l}{r}")),
                        (Value::Number(l), Value::String(r)) => Value::String(format!("{l}{r}")),
                        _ => self.marker(b.span, "an expression".into()),
                    }
                }
                Expression::ArrayExpression(a) => {
                    Value::Array(self.array(a, false).into_iter().map(|(v, _)| v).collect())
                }
                Expression::ObjectExpression(o) => self.object(o),
                Expression::Identifier(id) => self.identifier(id),
                Expression::CallExpression(c) => self.call(c),
                Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
                | Expression::PrivateFieldExpression(_) => self.member(e),
                Expression::ChainExpression(c) => self.marker(c.span, "an optional chain".into()),
                Expression::ConditionalExpression(c) => {
                    self.marker(c.span, "depends on a condition".into())
                }
                Expression::LogicalExpression(l) => {
                    self.marker(l.span, "depends on a condition".into())
                }
                Expression::ArrowFunctionExpression(f) => self.marker(f.span, "a function".into()),
                Expression::FunctionExpression(f) => self.marker(f.span, "a function".into()),
                Expression::ClassExpression(c) => self.marker(c.span, "a class".into()),
                Expression::NewExpression(n) => {
                    self.marker(n.span, "constructs an object at run time".into())
                }
                Expression::AwaitExpression(a) => {
                    self.marker(a.span, "awaits a value at run time".into())
                }
                Expression::ImportExpression(i) => {
                    self.marker(i.span, "imports a module at run time".into())
                }
                Expression::MetaProperty(m) => {
                    self.marker(m.span, "depends on the file's location".into())
                }
                other => self.marker(other.span(), "not a literal".into()),
            }
        }

        fn template(&mut self, t: &'p TemplateLiteral<'a>) -> Value {
            let mut out = String::new();
            for (i, q) in t.quasis.iter().enumerate() {
                let Some(cooked) = q.value.cooked else {
                    return self.marker(t.span, "a template with an invalid escape".into());
                };
                out.push_str(cooked.as_str());
                if let Some(e) = t.expressions.get(i) {
                    match self.eval(e) {
                        Value::String(s) => out.push_str(&s),
                        Value::Number(n) => out.push_str(&n.to_string()),
                        _ => return self.marker(t.span, "a template with a computed part".into()),
                    }
                }
            }
            Value::String(out)
        }

        fn identifier(&mut self, id: &'p IdentifierReference<'a>) -> Value {
            let name = id.name.as_str();
            let span = id.span;
            match self.bindings.get(name) {
                None => match name {
                    "undefined" => Value::Null,
                    "process" => self.marker(span, "reads the process environment".into()),
                    "__dirname" | "__filename" => {
                        self.marker(span, "depends on the file's location".into())
                    }
                    _ => self.marker(span, "not defined in the file".into()),
                },
                Some(Binding::Other(why)) => {
                    let why = (*why).to_string();
                    self.marker(span, why)
                }
                Some(Binding::Import { module, path }) => {
                    let (module, path) = (module.clone(), path.clone());
                    self.imported(span, &module, &path)
                }
                Some(Binding::Const(init)) => {
                    let init: &'p Expression<'a> = init;
                    if self.active.contains(&name) {
                        return self.marker(span, "refers to itself".into());
                    }
                    self.active.push(name);
                    let v = self.eval(init);
                    self.active.pop();
                    // A `const` cannot be assigned again, so a string, number or boolean keeps
                    // its value whatever the file does with it; an object or array may change.
                    let changeable = v.is_array() || (v.is_object() && Marker::of(&v).is_none());
                    if changeable && self.modified.contains(name) {
                        return self.marker(span, "modified elsewhere in the file".into());
                    }
                    match Marker::of(&v) {
                        // Report the name where it is used, and why its value is not static.
                        Some(m) if m.preset.is_none() => self.marker(
                            span,
                            format!("set at line {} to `{}`: {}", m.line, m.source, m.reason),
                        ),
                        _ => v,
                    }
                }
            }
        }

        /// Whether the `const` `name` can be evaluated here.
        fn usable(&self, name: &str) -> Result<(), &'static str> {
            if self.active.contains(&name) {
                Err("refers to itself")
            } else if self.modified.contains(name) {
                Err("modified elsewhere in the file")
            } else {
                Ok(())
            }
        }

        /// The value of an import's member `path` (`js.configs.recommended`): a known shared
        /// configuration, or not readable.
        fn imported(&self, span: Span, module: &str, path: &[String]) -> Value {
            match preset(module, path) {
                Some((name, rules_only)) => Marker {
                    preset: Some(name),
                    rules_only,
                    source: self.excerpt(span),
                    line: self.line(span),
                    ..Marker::default()
                }
                .to_value(),
                None if is_local(module) => {
                    self.marker(span, format!("imported from the file `{module}`"))
                }
                None => self.marker(span, format!("a value of the package `{module}`")),
            }
        }

        /// See [`callee_path`].
        fn import_path(&self, e: &Expression<'_>) -> Option<(String, Vec<String>)> {
            callee_path(self, e)
        }

        fn member(&mut self, e: &'p Expression<'a>) -> Value {
            let span = e.span();
            if let Some((module, path)) = self.import_path(e) {
                return self.imported(span, &module, &path);
            }
            let Some(m) = e.as_member_expression() else {
                return self.marker(span, "not a literal".into());
            };
            let index = match m {
                MemberExpression::ComputedMemberExpression(c) => match &c.expression {
                    Expression::NumericLiteral(n) if n.value >= 0.0 && n.value.fract() == 0.0 => {
                        Some((n.value as u64).to_string())
                    }
                    _ => None,
                },
                _ => None,
            };
            let Some(prop) = m.static_property_name().map(str::to_string).or(index) else {
                return self.marker(span, "a computed member".into());
            };
            let prop = prop.as_str();
            let object = self.eval(m.object());
            if let Some(inner) = Marker::of(&object) {
                if inner.preset.is_some() {
                    return self.marker(span, format!("a part of preset `{}`", inner.source));
                }
                return self.marker(span, inner.reason);
            }
            match object {
                Value::Object(o) => {
                    let replaced = super::spreads(&o)
                        .iter()
                        .any(|s| s.before.iter().any(|k| k == prop))
                        || (!o.contains_key(prop) && o.contains_key(SPREADS));
                    if replaced {
                        return self
                            .marker(span, "may be set by a spread whose keys are unknown".into());
                    }
                    o.get(prop).cloned().unwrap_or(Value::Null)
                }
                Value::Array(a) => match prop.parse::<usize>() {
                    Ok(i) => a.get(i).cloned().unwrap_or(Value::Null),
                    Err(_) if prop == "length" => Value::from(a.len()),
                    Err(_) => self.marker(span, "not a literal".into()),
                },
                _ => self.marker(span, "not a literal".into()),
            }
        }

        fn helper(&self, c: &'p CallExpression<'a>) -> Option<Helper> {
            let (module, path) = self.import_path(&c.callee)?;
            helper(&module, &path)
        }

        fn call(&mut self, c: &'p CallExpression<'a>) -> Value {
            if let Some(module) = require_call(c) {
                return self.imported(c.span, &module, &[]);
            }
            match self.helper(c) {
                Some(Helper::Flatten) => Value::Array(
                    self.flatten_arguments(c)
                        .into_iter()
                        .map(|(v, _)| v)
                        .collect(),
                ),
                Some(Helper::GlobalIgnores) => {
                    let mut obj = Map::new();
                    let patterns = match c.arguments.first().and_then(Argument::as_expression) {
                        Some(e) => self.eval(e),
                        None => Value::Array(Vec::new()),
                    };
                    obj.insert("ignores".into(), patterns);
                    if let Some(name) = c.arguments.get(1).and_then(Argument::as_expression) {
                        let name = self.eval(name);
                        if name.is_string() {
                            obj.insert("name".into(), name);
                        }
                    }
                    Value::Object(obj)
                }
                Some(Helper::Identity) => {
                    match c.arguments.first().and_then(Argument::as_expression) {
                        Some(e) => self.eval(e),
                        None => Value::Null,
                    }
                }
                None => self.marker(c.span, "the result of a function call".into()),
            }
        }

        /// The arguments of a flattening helper, each with its line.
        fn flatten_arguments(&mut self, c: &'p CallExpression<'a>) -> Vec<(Value, u32)> {
            let mut out = Vec::new();
            for arg in &c.arguments {
                match arg {
                    Argument::SpreadElement(s) => self.spread_into(s, true, &mut out),
                    _ => {
                        if let Some(e) = arg.as_expression() {
                            self.element_into(e, true, &mut out);
                        }
                    }
                }
            }
            out
        }

        /// The elements of an array literal, each with its line; `flatten`: nested arrays too
        /// (a list of configuration objects).
        fn array(&mut self, a: &'p ArrayExpression<'a>, flatten: bool) -> Vec<(Value, u32)> {
            let mut out = Vec::new();
            for el in &a.elements {
                match el {
                    ArrayExpressionElement::SpreadElement(s) => {
                        self.spread_into(s, flatten, &mut out)
                    }
                    ArrayExpressionElement::Elision(e) => {
                        out.push((Value::Null, self.line(e.span)));
                    }
                    _ => {
                        if let Some(e) = el.as_expression() {
                            self.element_into(e, flatten, &mut out);
                        }
                    }
                }
            }
            out
        }

        /// The elements of `e` when it is a list: an array literal, a flattening helper, or a
        /// `const` naming one (`None` otherwise).
        fn list(&mut self, e: &'p Expression<'a>, flatten: bool) -> Option<Vec<(Value, u32)>> {
            if self.depth >= MAX_DEPTH {
                return None;
            }
            match e.get_inner_expression() {
                Expression::ArrayExpression(a) => {
                    self.depth += 1;
                    let items = self.array(a, flatten);
                    self.depth -= 1;
                    Some(items)
                }
                Expression::CallExpression(c) => match self.helper(c)? {
                    Helper::Flatten => {
                        self.depth += 1;
                        let items = self.flatten_arguments(c);
                        self.depth -= 1;
                        Some(items)
                    }
                    Helper::Identity => {
                        let first = c.arguments.first()?.as_expression()?;
                        self.list(first, flatten)
                    }
                    Helper::GlobalIgnores => None,
                },
                Expression::Identifier(id) => {
                    let name = id.name.as_str();
                    let Some(Binding::Const(init)) = self.bindings.get(name) else {
                        return None;
                    };
                    let init: &'p Expression<'a> = init;
                    self.usable(name).ok()?;
                    self.active.push(name);
                    let items = self.list(init, flatten);
                    self.active.pop();
                    items
                }
                _ => None,
            }
        }

        fn element_into(
            &mut self,
            e: &'p Expression<'a>,
            flatten: bool,
            out: &mut Vec<(Value, u32)>,
        ) {
            if flatten {
                if let Some(items) = self.list(e, true) {
                    out.extend(items);
                    return;
                }
            }
            let line = self.line(e.span());
            let v = self.eval(e);
            push(v, line, flatten, out);
        }

        /// `...x` in an array: the elements of `x`.
        fn spread_into(
            &mut self,
            s: &'p SpreadElement<'a>,
            flatten: bool,
            out: &mut Vec<(Value, u32)>,
        ) {
            if let Some(items) = self.list(&s.argument, flatten) {
                out.extend(items);
                return;
            }
            let line = self.line(s.span);
            match self.eval(&s.argument) {
                Value::Array(a) => {
                    for v in a {
                        push(v, line, flatten, out);
                    }
                }
                v => {
                    let mut m = Marker::of(&v).unwrap_or_else(|| Marker {
                        reason: "not a list".into(),
                        ..Marker::default()
                    });
                    m.source = self.excerpt(s.span);
                    m.line = line;
                    out.push((m.to_value(), line));
                }
            }
        }

        fn object(&mut self, o: &'p ObjectExpression<'a>) -> Value {
            let mut map = Map::new();
            let mut spreads: Vec<Marker> = Vec::new();
            for p in &o.properties {
                match p {
                    ObjectPropertyKind::ObjectProperty(p) => {
                        let key = if p.computed {
                            match p.key.as_expression().map(|k| self.eval(k)) {
                                Some(Value::String(s)) => Some(s),
                                Some(Value::Number(n)) => Some(n.to_string()),
                                _ => None,
                            }
                        } else {
                            p.key.static_name().map(|k| k.into_owned())
                        };
                        let Some(key) = key else {
                            spreads.push(Marker {
                                source: self.excerpt(p.span),
                                line: self.line(p.span),
                                reason: "a computed key".into(),
                                before: map.keys().filter(|k| !is_reserved(k)).cloned().collect(),
                                ..Marker::default()
                            });
                            continue;
                        };
                        if is_reserved(&key) {
                            continue;
                        }
                        let value = if p.kind != PropertyKind::Init || p.method {
                            self.marker(p.value.span(), "a function".into())
                        } else {
                            self.eval(&p.value)
                        };
                        for s in &mut spreads {
                            s.before.retain(|k| *k != key);
                        }
                        map.insert(key, value);
                    }
                    ObjectPropertyKind::SpreadProperty(s) => match self.eval(&s.argument) {
                        Value::Object(src) if !src.contains_key(super::MARKER) => {
                            // The unknown spreads of `src` take effect here: they may replace
                            // what `src` wrote before them and what this object wrote so far
                            // (unless `src` sets it again).
                            let inner = super::spreads(&src);
                            let outer: Vec<String> = map
                                .keys()
                                .filter(|k| !is_reserved(k) && !src.contains_key(*k))
                                .cloned()
                                .collect();
                            for (k, v) in src {
                                if !is_reserved(&k) {
                                    for s in &mut spreads {
                                        s.before.retain(|x| *x != k);
                                    }
                                    map.insert(k, v);
                                }
                            }
                            for mut s in inner {
                                for k in &outer {
                                    if !s.before.contains(k) {
                                        s.before.push(k.clone());
                                    }
                                }
                                spreads.push(s);
                            }
                        }
                        Value::Null => {}
                        v => {
                            let mut m = Marker::of(&v).unwrap_or_else(|| Marker {
                                reason: "not an object".into(),
                                ..Marker::default()
                            });
                            m.source = self.excerpt(s.span);
                            m.line = self.line(s.span);
                            m.before = map.keys().filter(|k| !is_reserved(k)).cloned().collect();
                            spreads.push(m);
                        }
                    },
                }
            }
            if !spreads.is_empty() {
                map.insert(
                    SPREADS.into(),
                    Value::Array(spreads.iter().map(Marker::to_value).collect()),
                );
            }
            Value::Object(map)
        }

        /// The configuration objects a flat configuration exports, each with its line.
        fn config_list(&mut self, export: Export<'p, 'a>) -> Vec<(Value, u32)> {
            let line = self.line(export.span());
            let v = match export {
                Export::Expr(e) => {
                    if let Some(items) = self.list(e, true) {
                        return items;
                    }
                    self.eval(e)
                }
                Export::Name(name, span) => match self.bindings.get(name) {
                    Some(Binding::Const(init)) => {
                        let init: &'p Expression<'a> = init;
                        match self.usable(name) {
                            Ok(()) => return self.config_list(Export::Expr(init)),
                            Err(why) => self.marker(span, why.into()),
                        }
                    }
                    _ => self.marker(span, "not a literal".into()),
                },
                Export::Other(span, why) => self.marker(span, why.into()),
            };
            let mut out = Vec::new();
            push(v, line, true, &mut out);
            out
        }

        fn eval_export(&mut self, export: &Export<'p, 'a>) -> Value {
            match export {
                Export::Expr(e) => self.eval(e),
                Export::Name(name, span) => match self.bindings.get(name) {
                    Some(Binding::Const(init)) => {
                        let init: &'p Expression<'a> = init;
                        match self.usable(name) {
                            Ok(()) => self.eval(init),
                            Err(why) => self.marker(*span, why.into()),
                        }
                    }
                    _ => self.marker(*span, "not a literal".into()),
                },
                Export::Other(span, why) => self.marker(*span, (*why).into()),
            }
        }
    }

    /// Add `v` to a list; `flatten`: the elements of a nested array instead.
    fn push(v: Value, line: u32, flatten: bool, out: &mut Vec<(Value, u32)>) {
        match v {
            Value::Array(a) if flatten => {
                for x in a {
                    push(x, line, true, out);
                }
            }
            v => out.push((v, line)),
        }
    }

    fn number(f: f64) -> Value {
        if f.fract() == 0.0 && f.abs() < 9.0e15 {
            Value::from(f as i64)
        } else {
            Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
    }

    fn export_name(n: &ModuleExportName<'_>) -> String {
        match n {
            ModuleExportName::IdentifierName(i) => i.name.as_str().to_string(),
            ModuleExportName::IdentifierReference(i) => i.name.as_str().to_string(),
            ModuleExportName::StringLiteral(s) => s.value.as_str().to_string(),
        }
    }

    /// `module.exports` as an assignment target.
    fn is_module_exports(t: &AssignmentTarget<'_>) -> bool {
        t.as_simple_assignment_target()
            .and_then(|s| s.as_member_expression())
            .is_some_and(|m| m.is_specific_member_access("module", "exports"))
    }

    /// `require("m")` (and members of it: `require("m").configs`): the module and the path.
    fn require_of(e: &Expression<'_>) -> Option<(String, Vec<String>)> {
        let mut props = Vec::new();
        let mut cur = e.get_inner_expression();
        loop {
            match cur {
                Expression::StaticMemberExpression(m) => {
                    props.push(m.property.name.as_str().to_string());
                    cur = m.object.get_inner_expression();
                }
                Expression::ComputedMemberExpression(m) => {
                    props.push(m.static_property_name()?.as_str().to_string());
                    cur = m.object.get_inner_expression();
                }
                Expression::CallExpression(c) => {
                    let module = require_call(c)?;
                    let mut path: Vec<String> = props.into_iter().rev().collect();
                    if path.first().is_some_and(|p| p == "default") {
                        path.remove(0);
                    }
                    return Some((module, path));
                }
                _ => return None,
            }
        }
    }

    /// The module of `require("m")`.
    fn require_call(c: &CallExpression<'_>) -> Option<String> {
        if !c.callee.is_specific_id("require") || c.arguments.len() != 1 {
            return None;
        }
        match c.arguments[0].as_expression()? {
            Expression::StringLiteral(s) => Some(s.value.as_str().to_string()),
            _ => None,
        }
    }

    fn is_local(module: &str) -> bool {
        module.starts_with('.') || module.starts_with('/') || module.starts_with('#')
    }

    /// The data helper `path` of `module` is, if any.
    fn helper(module: &str, path: &[String]) -> Option<Helper> {
        let path: Vec<&str> = path.iter().map(String::as_str).collect();
        match (module, path.as_slice()) {
            ("eslint/config" | "@eslint/config-helpers", ["defineConfig"]) => Some(Helper::Flatten),
            ("eslint/config" | "@eslint/config-helpers", ["globalIgnores"]) => {
                Some(Helper::GlobalIgnores)
            }
            ("typescript-eslint", ["config"]) => Some(Helper::Flatten),
            ("eslint-define-config", ["defineConfig" | "defineFlatConfig"]) => {
                Some(Helper::Identity)
            }
            _ => None,
        }
    }

    /// The ESLint plugin a module is, and the part after it: `eslint-plugin-react` → `react`,
    /// `@angular-eslint/eslint-plugin-template` → `@angular-eslint/template`,
    /// `eslint-plugin-prettier/recommended` → (`prettier`, `recommended`).
    fn plugin_of(module: &str) -> Option<(String, Option<String>)> {
        if module == "typescript-eslint" || module == "@typescript-eslint/eslint-plugin" {
            return Some(("@typescript-eslint".into(), None));
        }
        let (scope, rest) = match module.strip_prefix('@') {
            Some(m) => {
                let (scope, rest) = m.split_once('/')?;
                (Some(scope), rest)
            }
            None => (None, module),
        };
        let (pkg, sub) = match rest.split_once('/') {
            Some((p, s)) => (p, Some(s.to_string())),
            None => (rest, None),
        };
        let name = match (scope, pkg) {
            (Some(scope), "eslint-plugin") => format!("@{scope}"),
            (Some(scope), p) => format!("@{scope}/{}", p.strip_prefix("eslint-plugin-")?),
            (None, p) => p.strip_prefix("eslint-plugin-")?.to_string(),
        };
        Some((name, sub))
    }

    /// The shareable configuration a module is: `eslint-config-prettier` → `prettier`.
    fn shareable_config_of(module: &str) -> Option<String> {
        let (scope, rest) = match module.strip_prefix('@') {
            Some(m) => {
                let (scope, rest) = m.split_once('/')?;
                (Some(scope), rest)
            }
            None => (None, module),
        };
        let (pkg, sub) = match rest.split_once('/') {
            Some((p, s)) => (p, Some(s)),
            None => (rest, None),
        };
        let base = match (scope, pkg) {
            (Some(scope), "eslint-config") => format!("@{scope}"),
            (Some(scope), p) => format!("@{scope}/{}", p.strip_prefix("eslint-config-")?),
            (None, p) => p.strip_prefix("eslint-config-")?.to_string(),
        };
        Some(match sub {
            None | Some("flat") => base,
            Some(s) => format!("{base}/{s}"),
        })
    }

    /// `recommendedTypeChecked` → `recommended-type-checked`.
    fn kebab(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            if c.is_ascii_uppercase() {
                if !out.is_empty() && !out.ends_with(['-', '/']) {
                    out.push('-');
                }
                out.push(c.to_ascii_lowercase());
            } else {
                out.push(c);
            }
        }
        out
    }

    /// The shared configuration `path` of `module` names, by ESLint's naming conventions:
    /// `(name, only its rules)`.
    fn preset(module: &str, path: &[String]) -> Option<(String, bool)> {
        let mut path: Vec<&str> = path.iter().map(String::as_str).collect();
        let rules_only = path.len() > 1 && path.last() == Some(&"rules");
        if rules_only {
            path.pop();
        }
        if module == "@eslint/js" {
            return match path.as_slice() {
                ["configs", name @ ("recommended" | "all")] => {
                    Some((format!("eslint:{name}"), rules_only))
                }
                _ => None,
            };
        }
        if path.is_empty() {
            if let Some(name) = shareable_config_of(module) {
                return Some((name, false));
            }
            return match plugin_of(module)? {
                (plugin, Some(sub)) => Some((format!("plugin:{plugin}/{sub}"), false)),
                (_, None) => None,
            };
        }
        let (plugin, _) = plugin_of(module)?;
        let at = path
            .iter()
            .position(|p| *p == "configs" || *p == "flatConfigs")?;
        let mut name: Vec<String> = path[at + 1..]
            .iter()
            .flat_map(|p| p.split('/'))
            .map(kebab)
            .collect();
        if name.first().is_some_and(|n| n == "flat") {
            name.remove(0);
        }
        if name.is_empty() {
            return None;
        }
        Some((format!("plugin:{plugin}/{}", name.join("/")), rules_only))
    }

    /// Finds what the file may modify: names assigned to (or their members), updated, deleted,
    /// called as `name.method(…)`, passed to a function other than the known helpers, or looped
    /// over. Names only (shadowing makes it stricter, never laxer).
    struct Modifications<'r, 'p, 'a> {
        reader: &'r Reader<'p, 'a>,
        /// > 0: every name visited is marked.
        marking: u32,
        marked: HashSet<String>,
        /// `module.exports = …` assignments anywhere.
        exports: usize,
    }

    impl<'a> Visit<'a> for Modifications<'_, '_, 'a> {
        fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
            if self.marking > 0 {
                self.marked.insert(it.name.as_str().to_string());
            }
        }

        fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
            if it.operator == AssignmentOperator::Assign && is_module_exports(&it.left) {
                self.exports += 1;
            } else {
                self.marking += 1;
                self.visit_assignment_target(&it.left);
                self.marking -= 1;
            }
            self.visit_expression(&it.right);
        }

        fn visit_update_expression(&mut self, it: &UpdateExpression<'a>) {
            self.marking += 1;
            walk::walk_update_expression(self, it);
            self.marking -= 1;
        }

        fn visit_unary_expression(&mut self, it: &UnaryExpression<'a>) {
            if it.operator == UnaryOperator::Delete {
                self.marking += 1;
                walk::walk_unary_expression(self, it);
                self.marking -= 1;
            } else {
                walk::walk_unary_expression(self, it);
            }
        }

        fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
            // The helpers only read their arguments.
            let known =
                callee_path(self.reader, &it.callee).is_some_and(|(m, p)| helper(&m, &p).is_some());
            if known || it.callee.is_specific_id("require") {
                walk::walk_call_expression(self, it);
            } else {
                self.marking += 1;
                walk::walk_call_expression(self, it);
                self.marking -= 1;
            }
        }

        fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
            self.marking += 1;
            walk::walk_new_expression(self, it);
            self.marking -= 1;
        }

        fn visit_tagged_template_expression(&mut self, it: &TaggedTemplateExpression<'a>) {
            self.marking += 1;
            walk::walk_tagged_template_expression(self, it);
            self.marking -= 1;
        }

        fn visit_for_of_statement(&mut self, it: &ForOfStatement<'a>) {
            self.marking += 1;
            self.visit_expression(&it.right);
            self.marking -= 1;
            walk::walk_for_of_statement(self, it);
        }

        fn visit_for_in_statement(&mut self, it: &ForInStatement<'a>) {
            self.marking += 1;
            self.visit_expression(&it.right);
            self.marking -= 1;
            walk::walk_for_in_statement(self, it);
        }
    }

    /// The module and member path of `e` when it is (a member of) an import:
    /// `tseslint.configs.recommended` → (`typescript-eslint`, [configs, recommended]).
    fn callee_path(r: &Reader<'_, '_>, e: &Expression<'_>) -> Option<(String, Vec<String>)> {
        let mut props = Vec::new();
        let mut cur = e.get_inner_expression();
        loop {
            match cur {
                Expression::StaticMemberExpression(m) => {
                    props.push(m.property.name.as_str().to_string());
                    cur = m.object.get_inner_expression();
                }
                Expression::ComputedMemberExpression(m) => {
                    props.push(m.static_property_name()?.as_str().to_string());
                    cur = m.object.get_inner_expression();
                }
                Expression::Identifier(id) => {
                    let Some(Binding::Import { module, path }) = r.bindings.get(id.name.as_str())
                    else {
                        return None;
                    };
                    let mut full = path.clone();
                    full.extend(props.into_iter().rev());
                    return Some((module.clone(), full));
                }
                Expression::CallExpression(_) => {
                    let (module, mut path) = require_of(cur)?;
                    path.extend(props.into_iter().rev());
                    return Some((module, path));
                }
                _ => return None,
            }
        }
    }

    /// The names an expression refers to.
    struct Identifiers(Vec<String>);

    impl<'a> Visit<'a> for Identifiers {
        fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
            self.0.push(it.name.as_str().to_string());
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::{contains_marker, spreads};
        use super::*;

        /// Read `src` as the file `name` (a flat configuration when it is `eslint.config.*`).
        fn read_as(name: &str, src: &str) -> Result<Read, String> {
            let flat = if name.starts_with("eslint.config.") {
                Some(true)
            } else if name.starts_with(".eslintrc") {
                Some(false)
            } else {
                None
            };
            read(Path::new(name), src, flat)
        }

        fn value(name: &str, src: &str) -> Value {
            read_as(name, src).unwrap().value
        }

        fn reason(v: &Value) -> String {
            Marker::of(v).map(|m| m.reason).unwrap_or_default()
        }

        #[test]
        fn literals_consts_and_helpers() {
            let src = r#"
                import { defineConfig, globalIgnores } from 'eslint/config';
                const dir = 'src';
                const level = 2;
                const rules = { 'no-var': 'error', eqeqeq: ['warn', 'always'] };
                const base = { files: [`${dir}/**/*.js`], rules };
                export default defineConfig(
                  globalIgnores(['dist/'], 'ignores'),
                  [base, [{ rules: { ...rules, 'no-var': 'off', ['max-depth']: ['error', -1 + 0 || 3] } }]],
                  { files: [dir + '/a.js'], rules: { complexity: ['error', { max: level }], quotes: void 0 } },
                  { rules: { a: base.rules.eqeqeq[1], b: base.files[0], c: undefined, d: `x` } },
                );
            "#;
            let r = read_as("eslint.config.mjs", src).unwrap();
            let items = r.value.as_array().unwrap();
            assert_eq!(items.len(), 5, "{:#?}", r.value);
            assert_eq!(r.lines, vec![8, 9, 9, 10, 11]);
            assert_eq!(items[0]["ignores"][0], "dist/");
            assert_eq!(items[0]["name"], "ignores");
            assert_eq!(items[1]["files"][0], "src/**/*.js");
            assert_eq!(items[1]["rules"]["eqeqeq"][1], "always");
            // A later key wins over a spread one; `||` is not static.
            assert_eq!(items[2]["rules"]["no-var"], "off");
            assert_eq!(items[2]["rules"]["eqeqeq"][0], "warn");
            assert_eq!(
                reason(&items[2]["rules"]["max-depth"][1]),
                "depends on a condition"
            );
            assert_eq!(items[3]["files"][0], "src/a.js");
            assert_eq!(items[3]["rules"]["complexity"][1]["max"], 2);
            assert_eq!(items[3]["rules"]["quotes"], Value::Null);
            assert_eq!(items[4]["rules"]["a"], "always");
            assert_eq!(items[4]["rules"]["b"], "src/**/*.js");
            assert_eq!(items[4]["rules"]["c"], Value::Null);
            assert_eq!(items[4]["rules"]["d"], "x");
        }

        #[test]
        fn commonjs_requires_and_named_exports() {
            let src = r#"
                'use strict';
                const { defineConfig } = require('eslint/config');
                const js = require('@eslint/js');
                const local = require('./local');
                module.exports = defineConfig([js.configs.recommended, local, { rules: { a: 1 } }]);
            "#;
            let v = value("eslint.config.cjs", src);
            let items = v.as_array().unwrap();
            assert_eq!(
                Marker::of(&items[0]).and_then(|m| m.preset).as_deref(),
                Some("eslint:recommended")
            );
            assert_eq!(reason(&items[1]), "imported from the file `./local`");
            assert_eq!(items[2]["rules"]["a"], 1);
            // `export { x as default }`, and an object from a flat configuration file.
            let v = value(
                "eslint.config.mjs",
                "const config = { rules: { a: 'off' } };\nexport { config as default };\n",
            );
            assert_eq!(v[0]["rules"]["a"], "off");
            // An eslintrc file is one object.
            let v = value(".eslintrc.js", "module.exports = { rules: { a: 'warn' } };");
            assert_eq!(v["rules"]["a"], "warn");
            // Neither name: the value decides.
            let v = value("custom.config.js", "export default [{ rules: { a: 0 } }];");
            assert!(v.is_array());
        }

        #[test]
        fn what_is_not_static_becomes_a_stand_in() {
            let src = r#"
                import globals from 'globals';
                import plugin from 'eslint-plugin-foo';
                let mode = 'warn';
                const shared = { rules: {} };
                Object.assign(shared.rules, globals.extra);
                const list = [{ rules: { a: 1 } }];
                const alias = list;
                alias.push({ rules: { a: 0 } });
                const ERROR = 2;
                report(ERROR);
                const scoped = { files: ['**/*.vue'], ...plugin.configs.base, name: 'vue' };
                export default [
                  { rules: { a: mode, b: process.env.LEVEL, c: __dirname, d: ERROR, e: maybe() } },
                  shared,
                  ...list,
                  plugin.configs['flat/strict'],
                  { files: ['a'], ...plugin, rules: { x: 1 } },
                  { get rules() { return {}; } },
                  function () {},
                  new Thing(),
                  { ...scoped, rules: { y: 1 } },
                  { rules: { z: 1 }, ignores: ['b'], ...scoped, files: ['c'] },
                ];
            "#;
            let v = value("eslint.config.js", src);
            let items = v.as_array().unwrap();
            let rules = &items[0]["rules"];
            assert_eq!(
                reason(&rules["a"]),
                "declared with `let` or `var`, so it may change"
            );
            assert_eq!(reason(&rules["b"]), "reads the process environment");
            assert_eq!(reason(&rules["c"]), "depends on the file's location");
            assert_eq!(rules["d"], 2, "a primitive const stays readable");
            assert_eq!(reason(&rules["e"]), "the result of a function call");
            assert_eq!(reason(&items[1]), "modified elsewhere in the file");
            assert_eq!(reason(&items[2]), "modified elsewhere in the file");
            assert_eq!(
                Marker::of(&items[3]).and_then(|m| m.preset).as_deref(),
                Some("plugin:foo/strict")
            );
            let s = spreads(items[4].as_object().unwrap());
            assert_eq!(s.len(), 1);
            assert_eq!(s[0].source, "...plugin");
            assert_eq!(s[0].before, vec!["files".to_string()]);
            assert_eq!(s[0].reason, "a value of the package `eslint-plugin-foo`");
            assert_eq!(reason(&items[5]["rules"]), "a function");
            assert_eq!(reason(&items[6]), "a function");
            assert_eq!(reason(&items[7]), "constructs an object at run time");
            // A const holding an unknown spread, spread in turn: what the inner spread may
            // replace is still known, in the const and in the object spreading it.
            let s = spreads(items[8].as_object().unwrap());
            assert_eq!(s.len(), 1);
            assert_eq!(s[0].preset.as_deref(), Some("plugin:foo/base"));
            assert_eq!(s[0].before, vec!["files".to_string()]);
            assert_eq!(items[8]["name"], "vue");
            let s = spreads(items[9].as_object().unwrap());
            let mut before = s[0].before.clone();
            before.sort();
            assert_eq!(before, vec!["ignores".to_string(), "rules".to_string()]);
            assert_eq!(
                items[9]["files"][0], "c",
                "written after the spread: certain"
            );
        }

        #[test]
        fn module_exports_changed_elsewhere_is_not_read() {
            let src = "const config = { rules: {} };\nmodule.exports = config;\nmodule.exports.rules.a = 'off';\n";
            let v = value(".eslintrc.cjs", src);
            assert_eq!(
                reason(&v),
                "`module.exports` is also changed elsewhere in the file"
            );
            let src =
                "module.exports = { rules: {} };\nif (process.env.CI) { module.exports = {}; }\n";
            assert!(Marker::of(&value(".eslintrc.cjs", src)).is_some());
        }

        #[test]
        fn unreadable_files_are_refused() {
            let e = read_as("eslint.config.js", "const a = 1;\n").unwrap_err();
            assert!(e.contains("no `export default"), "{e}");
            let e = read_as("eslint.config.js", "export default [\n").unwrap_err();
            assert!(e.starts_with("cannot parse the file"), "{e}");
            let v = value(
                "eslint.config.js",
                "export default function () { return []; }\n",
            );
            assert_eq!(reason(&v[0]), "a function or class");
        }

        #[test]
        fn cycles_depth_and_size_are_bounded() {
            let v = value(
                "eslint.config.js",
                "const a = { rules: b.rules };\nconst b = { rules: a.rules };\nexport default [a];\n",
            );
            assert!(contains_marker(&v), "{v}");
            let deep = format!(
                "export default {{ rules: {{ a: {}1{} }} }};\n",
                "[".repeat(300),
                "]".repeat(300)
            );
            let v = value(".eslintrc.js", &deep);
            assert!(contains_marker(&v));
            let mut src = String::from("const a0 = ['x', 'x'];\n");
            for i in 1..=20 {
                let p = i - 1;
                src.push_str(&format!("const a{i} = [a{p}, a{p}, a{p}, a{p}];\n"));
            }
            src.push_str("export default { rules: { r: a20 } };\n");
            let v = value(".eslintrc.js", &src);
            assert!(contains_marker(&v));
        }

        #[test]
        fn typescript_wrappers_are_read_through() {
            let src = r#"
                import type { Linter } from 'eslint';
                import tseslint from 'typescript-eslint';
                const files = ['**/*.ts'] as const;
                export default tseslint.config(
                  { files: files as unknown as string[], rules: { a: 'error' } satisfies Linter.RulesRecord },
                  <Linter.Config>{ rules: { b: 'off' } },
                  ...tseslint.configs.strict!,
                ) satisfies Linter.Config[];
            "#;
            let v = value("eslint.config.ts", src);
            let items = v.as_array().unwrap();
            assert_eq!(items[0]["files"][0], "**/*.ts");
            assert_eq!(items[0]["rules"]["a"], "error");
            assert_eq!(items[1]["rules"]["b"], "off");
            assert_eq!(
                Marker::of(&items[2]).and_then(|m| m.preset).as_deref(),
                Some("plugin:@typescript-eslint/strict")
            );
        }

        #[test]
        fn names_of_shared_configurations() {
            let p = |m: &str, path: &[&str]| {
                preset(m, &path.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            };
            let named = |s: &str| Some((s.to_string(), false));
            assert_eq!(
                p("@eslint/js", &["configs", "recommended"]),
                named("eslint:recommended")
            );
            assert_eq!(
                p("@eslint/js", &["configs", "recommended", "rules"]),
                Some(("eslint:recommended".into(), true))
            );
            assert_eq!(
                p("typescript-eslint", &["configs", "recommendedTypeChecked"]),
                named("plugin:@typescript-eslint/recommended-type-checked")
            );
            assert_eq!(
                p("eslint-plugin-storybook", &["configs", "flat/recommended"]),
                named("plugin:storybook/recommended")
            );
            assert_eq!(
                p("eslint-plugin-react", &["configs", "flat", "recommended"]),
                named("plugin:react/recommended")
            );
            assert_eq!(
                p(
                    "@angular-eslint/eslint-plugin-template",
                    &["configs", "recommended", "rules"]
                ),
                Some(("plugin:@angular-eslint/template/recommended".into(), true))
            );
            assert_eq!(
                p("@nx/eslint-plugin", &["configs", "angular-template"]),
                named("plugin:@nx/angular-template")
            );
            assert_eq!(
                p("eslint-plugin-import-x", &["flatConfigs", "recommended"]),
                named("plugin:import-x/recommended")
            );
            assert_eq!(p("eslint-config-prettier", &[]), named("prettier"));
            assert_eq!(p("eslint-config-prettier/flat", &[]), named("prettier"));
            assert_eq!(
                p("eslint-plugin-prettier/recommended", &[]),
                named("plugin:prettier/recommended")
            );
            // Not shared configurations.
            assert_eq!(p("globals", &["node"]), None);
            assert_eq!(p("eslint-plugin-react", &[]), None);
            assert_eq!(p("typescript-eslint", &["parser"]), None);
            assert_eq!(p("./lib/eslint/index.ts", &["base"]), None);
            assert_eq!(p("neostandard", &[]), None);
        }

        #[test]
        fn helpers_and_requires() {
            let h = |m: &str, f: &str| helper(m, &[f.to_string()]);
            assert_eq!(h("eslint/config", "defineConfig"), Some(Helper::Flatten));
            assert_eq!(
                h("eslint/config", "globalIgnores"),
                Some(Helper::GlobalIgnores)
            );
            assert_eq!(h("typescript-eslint", "config"), Some(Helper::Flatten));
            assert_eq!(h("eslint-plugin-jsdoc", "jsdoc"), None);
            assert_eq!(
                kebab("stylisticTypeCheckedOnly"),
                "stylistic-type-checked-only"
            );
            assert_eq!(kebab("flat/recommended"), "flat/recommended");
        }
    }
}
