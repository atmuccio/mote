//! Variable declarations and the per-member scope index.

use crate::kinds::{k, kind, K};
use crate::syntax::{named_children, node_text, Field, NodeExt};
use tree_sitter::Node;

/// What kind of variable a [`VarDecl`] declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VarKind {
    Local,
    Parameter,
    LambdaParameter,
    CatchParameter,
    ForEachVariable,
    Resource,
    PatternVariable,
    Field,
    EnumConstant,
    RecordComponent,
}

/// A resolved variable declaration (see [`crate::Ctx::declaration_of`]). Two `VarDecl`s are
/// equal when they declare the same variable, so it can key a map of uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VarDecl<'t> {
    pub kind: VarKind,
    /// The declared name (`identifier`).
    pub name: Node<'t>,
    /// The declaring node: `variable_declarator`, `formal_parameter`, `spread_parameter`,
    /// `catch_formal_parameter`, `enhanced_for_statement`, `resource`, `instanceof_expression`,
    /// `type_pattern`, `record_pattern_component`, `enum_constant`, or `lambda_expression`.
    pub decl: Node<'t>,
    /// The enclosing declaration statement carrying modifiers/annotations
    /// (`field_declaration`, `local_variable_declaration`, `constant_declaration`, or `decl`).
    pub statement: Node<'t>,
    /// Declared type node; `None` for `var`, inferred lambda parameters, union catch types,
    /// and enum constants.
    pub type_node: Option<Node<'t>>,
    /// Initializer (`= value`), or the iterable of an enhanced `for`.
    pub value: Option<Node<'t>>,
    /// Extra array dimensions (`int x[]`, varargs).
    pub(crate) dims: u8,
    /// Declared with `var`.
    pub is_var: bool,
}

impl<'t> VarDecl<'t> {
    /// Fields, enum constants, and record components (members rather than locals).
    pub fn is_field_like(&self) -> bool {
        matches!(
            self.kind,
            VarKind::Field | VarKind::EnumConstant | VarKind::RecordComponent
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Entry<'t> {
    Var(VarDecl<'t>),
    /// A local class/record/enum/interface declaration (shadows other type names).
    Class,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Scoped<'t> {
    pub name: &'t str,
    pub start: u32,
    pub end: u32,
    pub entry: Entry<'t>,
}

pub(crate) fn dims_of(node: Option<Node<'_>>, src: &str) -> u8 {
    node.map(|d| {
        node_text(d, src)
            .bytes()
            .filter(|b| *b == b'[')
            .count()
            .min(255) as u8
    })
    .unwrap_or(0)
}

fn is_var_type(t: Option<Node<'_>>, src: &str) -> bool {
    t.is_some_and(|t| kind(t) == "type_identifier" && node_text(t, src) == "var")
}

/// A `formal_parameter` / `spread_parameter` as a declaration.
pub(crate) fn param_decl<'t>(p: Node<'t>, kind_: VarKind, src: &str) -> Option<VarDecl<'t>> {
    match kind(p) {
        "formal_parameter" => {
            let name = p.field(Field::Name)?;
            let ty = p.field(Field::Type);
            Some(VarDecl {
                kind: kind_,
                name,
                decl: p,
                statement: p,
                type_node: ty.filter(|_| !is_var_type(ty, src)),
                value: None,
                dims: dims_of(p.field(Field::Dimensions), src),
                is_var: is_var_type(ty, src),
            })
        }
        "spread_parameter" => {
            let declarator = named_children(p).find(|c| kind(*c) == "variable_declarator")?;
            let name = declarator.field(Field::Name)?;
            let ty = named_children(p).find(|c| {
                !matches!(
                    kind(*c),
                    "modifiers" | "variable_declarator" | "annotation" | "marker_annotation"
                )
            });
            Some(VarDecl {
                kind: kind_,
                name,
                decl: p,
                statement: p,
                type_node: ty,
                value: None,
                dims: 1 + dims_of(declarator.field(Field::Dimensions), src),
                is_var: false,
            })
        }
        _ => None,
    }
}

/// Declarators of a field/local/constant declaration statement.
pub(crate) fn declarator_decls<'t>(stmt: Node<'t>, kind_: VarKind, src: &str) -> Vec<VarDecl<'t>> {
    let ty = stmt.field(Field::Type);
    let is_var = is_var_type(ty, src);
    named_children(stmt)
        .filter(|c| kind(*c) == "variable_declarator")
        .filter_map(|d| {
            let name = d.field(Field::Name)?;
            Some(VarDecl {
                kind: kind_,
                name,
                decl: d,
                statement: stmt,
                type_node: ty.filter(|_| !is_var),
                value: d.field(Field::Value),
                dims: dims_of(d.field(Field::Dimensions), src),
                is_var,
            })
        })
        .collect()
}

/// Statements and expressions that can only contain a declaration through a lambda, a pattern,
/// a switch expression, or an anonymous class body (see `build_scope`).
fn is_plain_expression(kk: K) -> bool {
    matches!(
        kk,
        K::ExpressionStatement
            | K::ReturnStatement
            | K::ThrowStatement
            | K::AssertStatement
            | K::YieldStatement
            | K::ExplicitConstructorInvocation
            | K::VariableDeclarator
            | K::ParenthesizedExpression
            | K::BinaryExpression
            | K::TernaryExpression
            | K::UnaryExpression
            | K::MethodInvocation
            | K::FieldAccess
            | K::ObjectCreation
    )
}

fn is_container(kk: K) -> bool {
    matches!(
        kk,
        K::Block
            | K::ConstructorBody
            | K::SwitchBlock
            | K::SwitchRule
            | K::ForStatement
            | K::EnhancedForStatement
            | K::CatchClause
            | K::TryWithResourcesStatement
            | K::Lambda
    )
}

/// Index every local declaration inside `member` (a method, constructor, initializer, field
/// declaration, or enum constant), skipping nested class bodies. One cursor walk.
pub(crate) fn build_scope<'t>(member: Node<'t>, src: &'t str) -> Vec<Scoped<'t>> {
    build_scope_with(member, src, true)
}

/// [`build_scope`]; `skip_plain` enables skipping expressions that cannot declare anything
/// (off only in the equivalence test).
fn build_scope_with<'t>(member: Node<'t>, src: &'t str, skip_plain: bool) -> Vec<Scoped<'t>> {
    let mut out: Vec<Scoped<'t>> = Vec::new();
    let push_var = |out: &mut Vec<Scoped<'t>>, d: VarDecl<'t>, start: usize, end: usize| {
        out.push(Scoped {
            name: node_text(d.name, src),
            start: start as u32,
            end: end as u32,
            entry: Entry::Var(d),
        });
    };

    // Parameters of the member itself.
    if matches!(
        kind(member),
        "method_declaration" | "constructor_declaration"
    ) {
        if let Some(params) = member.field(Field::Parameters) {
            for p in named_children(params) {
                if let Some(d) = param_decl(p, VarKind::Parameter, src) {
                    push_var(&mut out, d, member.start_byte(), member.end_byte());
                }
            }
        }
    }

    let mut cursor = member.walk();
    let mut depth = 0usize;
    // (depth of container node, end byte)
    let mut containers: Vec<(usize, usize)> = vec![(0, member.end_byte())];
    // Depth of the expression being walked because its text may declare something; the
    // expressions inside it are not checked again (keeps the text scans linear).
    let mut expr_depth: Option<usize> = None;
    'walk: loop {
        let n = cursor.node();
        let cend = containers.last().map(|c| c.1).unwrap_or(member.end_byte());
        let kn = k(n);
        let mut descend = true;
        if expr_depth.is_some_and(|d| depth <= d) {
            expr_depth = None;
        }
        match kn {
            K::LocalVarDecl => {
                for d in declarator_decls(n, VarKind::Local, src) {
                    let start = d.name.start_byte();
                    push_var(&mut out, d, start, cend);
                }
            }
            K::Lambda => {
                if let Some(params) = n.field(Field::Parameters) {
                    match kind(params) {
                        "identifier" => push_var(
                            &mut out,
                            VarDecl {
                                kind: VarKind::LambdaParameter,
                                name: params,
                                decl: n,
                                statement: n,
                                type_node: None,
                                value: None,
                                dims: 0,
                                is_var: false,
                            },
                            n.start_byte(),
                            n.end_byte(),
                        ),
                        "inferred_parameters" => {
                            for id in named_children(params).filter(|c| kind(*c) == "identifier") {
                                push_var(
                                    &mut out,
                                    VarDecl {
                                        kind: VarKind::LambdaParameter,
                                        name: id,
                                        decl: n,
                                        statement: n,
                                        type_node: None,
                                        value: None,
                                        dims: 0,
                                        is_var: false,
                                    },
                                    n.start_byte(),
                                    n.end_byte(),
                                );
                            }
                        }
                        _ => {
                            for p in named_children(params) {
                                if let Some(d) = param_decl(p, VarKind::LambdaParameter, src) {
                                    push_var(&mut out, d, n.start_byte(), n.end_byte());
                                }
                            }
                        }
                    }
                }
            }
            K::CatchClause => {
                if let Some(param) =
                    named_children(n).find(|c| kind(*c) == "catch_formal_parameter")
                {
                    if let Some(name) = param.field(Field::Name) {
                        let ctype = named_children(param).find(|c| kind(*c) == "catch_type");
                        let single = ctype.and_then(|ct| {
                            let mut it = named_children(ct);
                            let first = it.next();
                            if it.next().is_some() {
                                None
                            } else {
                                first
                            }
                        });
                        push_var(
                            &mut out,
                            VarDecl {
                                kind: VarKind::CatchParameter,
                                name,
                                decl: param,
                                statement: param,
                                type_node: single,
                                value: None,
                                dims: 0,
                                is_var: false,
                            },
                            n.start_byte(),
                            n.end_byte(),
                        );
                    }
                }
            }
            K::EnhancedForStatement => {
                if let Some(name) = n.field(Field::Name) {
                    let ty = n.field(Field::Type);
                    let is_var = is_var_type(ty, src);
                    push_var(
                        &mut out,
                        VarDecl {
                            kind: VarKind::ForEachVariable,
                            name,
                            decl: n,
                            statement: n,
                            type_node: ty.filter(|_| !is_var),
                            value: n.field(Field::Value),
                            dims: dims_of(n.field(Field::Dimensions), src),
                            is_var,
                        },
                        name.start_byte(),
                        n.end_byte(),
                    );
                }
            }
            K::Resource => {
                if let Some(name) = n.field(Field::Name) {
                    let ty = n.field(Field::Type);
                    let is_var = is_var_type(ty, src);
                    push_var(
                        &mut out,
                        VarDecl {
                            kind: VarKind::Resource,
                            name,
                            decl: n,
                            statement: n,
                            type_node: ty.filter(|_| !is_var),
                            value: n.field(Field::Value),
                            dims: dims_of(n.field(Field::Dimensions), src),
                            is_var,
                        },
                        name.start_byte(),
                        cend,
                    );
                }
            }
            K::InstanceofExpression => {
                if let Some(name) = n.field(Field::Name) {
                    push_var(
                        &mut out,
                        VarDecl {
                            kind: VarKind::PatternVariable,
                            name,
                            decl: n,
                            statement: n,
                            type_node: n.field(Field::Right),
                            value: None,
                            dims: 0,
                            is_var: false,
                        },
                        name.start_byte(),
                        cend,
                    );
                }
            }
            K::TypePattern | K::RecordPatternComponent => {
                let mut ty = None;
                let mut name = None;
                for c in named_children(n) {
                    match kind(c) {
                        "identifier" => name = Some(c),
                        "underscore_pattern" | "record_pattern" => {}
                        _ => ty = Some(c),
                    }
                }
                if let Some(name) = name {
                    let is_var = is_var_type(ty, src);
                    push_var(
                        &mut out,
                        VarDecl {
                            kind: VarKind::PatternVariable,
                            name,
                            decl: n,
                            statement: n,
                            type_node: ty.filter(|_| !is_var),
                            value: None,
                            dims: 0,
                            is_var,
                        },
                        name.start_byte(),
                        cend,
                    );
                }
            }
            kk if kk.is_type_decl() && n.id() != member.id() => {
                if let Some(name) = n.field(Field::Name) {
                    out.push(Scoped {
                        name: node_text(name, src),
                        start: n.start_byte() as u32,
                        end: cend as u32,
                        entry: Entry::Class,
                    });
                }
                descend = false;
            }
            K::ClassBody if n.id() != member.id() => descend = false,
            _ => {}
        }
        // Most of a method body is expressions that declare nothing: skip them. Inside an
        // expression, declarations only come with a lambda (`->`), a pattern (`instanceof`,
        // `switch` labels), or a switch expression's blocks; anonymous class bodies are skipped
        // anyway. Syntax errors may hide anything, so erroneous subtrees are always walked.
        if skip_plain && descend && depth > 0 && expr_depth.is_none() && is_plain_expression(kn) {
            let text = node_text(n, src);
            if n.has_error()
                || text.contains("->")
                || text.contains("instanceof")
                || text.contains("switch")
            {
                expr_depth = Some(depth);
            } else {
                descend = false;
            }
        }
        if is_container(kn) && n.id() != member.id() {
            containers.push((depth, n.end_byte()));
        }
        if descend && cursor.goto_first_child() {
            depth += 1;
            continue 'walk;
        }
        // Leave `n` and climb until a next sibling exists.
        loop {
            while containers.len() > 1 && containers.last().is_some_and(|c| c.0 >= depth) {
                containers.pop();
            }
            if depth == 0 {
                break 'walk;
            }
            if cursor.goto_next_sibling() {
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
            depth -= 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, ParseBudget};

    type Flat<'t> = Vec<(&'t str, u32, u32, Option<VarDecl<'t>>)>;

    fn flat<'t>(entries: &[Scoped<'t>]) -> Flat<'t> {
        entries
            .iter()
            .map(|s| {
                let d = match s.entry {
                    Entry::Var(d) => Some(d),
                    Entry::Class => None,
                };
                (s.name, s.start, s.end, d)
            })
            .collect()
    }

    /// Skipping expressions that cannot declare anything never changes a member's scope.
    fn assert_skip_is_exact(src: &str, what: &str) -> usize {
        let Some(tree) = parse(src, ParseBudget::DEFAULT, None) else {
            return 0;
        };
        let mut members = 0;
        let mut cursor = tree.walk();
        'walk: loop {
            let n = cursor.node();
            let member = matches!(
                k(n),
                K::MethodDecl
                    | K::ConstructorDecl
                    | K::CompactConstructorDecl
                    | K::StaticInitializer
                    | K::FieldDecl
                    | K::ConstantDecl
                    | K::EnumConstant
            ) || (k(n) == K::Block
                && n.parent()
                    .is_some_and(|p| matches!(k(p), K::ClassBody | K::EnumBodyDecls)));
            if member {
                members += 1;
                assert_eq!(
                    flat(&build_scope_with(n, src, true)),
                    flat(&build_scope_with(n, src, false)),
                    "{what}: {} at {:?}",
                    kind(n),
                    n.start_position()
                );
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    continue 'walk;
                }
                if !cursor.goto_parent() {
                    break 'walk;
                }
            }
        }
        members
    }

    fn java_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                java_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "java") {
                out.push(p);
            }
        }
    }

    #[test]
    fn skipping_plain_expressions_is_exact_on_fixtures() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let mut files = Vec::new();
        java_files(&dir, &mut files);
        assert!(!files.is_empty());
        let mut members = 0;
        for f in &files {
            let src = std::fs::read_to_string(f).unwrap_or_default();
            members += assert_skip_is_exact(&src, &f.display().to_string());
        }
        assert!(members > 500, "{members}");
    }

    /// Same over a Java corpus (`MOTE_JAVA_CORPUS`); skipped when unset. `cargo test --release -p mote-lang-java -- --ignored skipping_plain`
    #[test]
    #[ignore = "corpus check; run in release"]
    fn skipping_plain_expressions_is_exact_on_corpus() {
        let Ok(root) = std::env::var("MOTE_JAVA_CORPUS") else {
            eprintln!("MOTE_JAVA_CORPUS not set; skipping");
            return;
        };
        let mut files = Vec::new();
        java_files(std::path::Path::new(&root), &mut files);
        let mut members = 0;
        for f in &files {
            let bytes = std::fs::read(f).unwrap_or_default();
            let src = String::from_utf8_lossy(&bytes);
            members += assert_skip_is_exact(&src, &f.display().to_string());
        }
        eprintln!("{} files, {members} members", files.len());
    }
}
