//! Which known supertype declares a method that a method declaration overrides.
//!
//! File-local like the rest of the resolver: only supertypes declared in this file or in the
//! curated JDK table are searched. A method that overrides something in an unknown supertype
//! is simply not found.
//!
//! Parameter types are compared after erasure. Types the resolver cannot see (declared in
//! other files or libraries) and the type variables of the class being checked become
//! *placeholders* — class types named `?Simple` — so that `Comparator<Order>` and
//! `compare(Order a, Order b)` still match when `Order` lives elsewhere, while
//! `ArrayList<Order>` and `add(String)` do not. Raw supertypes bind their type variables to
//! `Object`; only method-level type variables stay unknown.

use super::jdk::{self, Env};
use super::scope;
use super::types::JType;
use super::VarKind;
use crate::ctx::Ctx;
use crate::kinds::{k, kind, K};
use crate::syntax::{self, named_children};
use crate::syntax::{Field, NodeExt};
use rustc_hash::FxHashSet;
use tree_sitter::Node;

/// Budget for the supertype walk (deep or cyclic hierarchies give up).
const MAX_TYPES: usize = 64;

/// Name of the placeholder for a type argument left to inference (`new Comparator<>() { … }`).
const INFERRED: &str = "?<>";

/// Whether a declared parameter type matches a supertype's parameter type after erasure.
/// `Unknown` on either side is a method-level type variable (or something the resolver could
/// not type at all): it only matches another `Unknown`. An inferred type argument matches any
/// reference type.
fn param_matches(declared: &JType, sup: &JType) -> bool {
    match (declared, sup) {
        (JType::Class(_) | JType::Array(_) | JType::Unknown, JType::Class(b))
            if b.name == INFERRED =>
        {
            true
        }
        (JType::Prim(a), JType::Prim(b)) => a == b,
        (JType::Array(a), JType::Array(b)) => param_matches(a, b),
        (JType::Class(a), JType::Class(b)) => a.name == b.name,
        (JType::Unknown, JType::Unknown) => true,
        _ => false,
    }
}

/// A class type standing for a type the resolver cannot see, keyed by its simple name.
fn placeholder(text: &str) -> JType {
    let base = text.split('<').next().unwrap_or(text).trim();
    let simple = base.rsplit('.').next().unwrap_or(base);
    JType::class(format!("?{simple}"))
}

/// `args` for a class with `n` type parameters: a raw reference binds them all to `Object`.
fn bind_raw(args: &[JType], n: usize) -> Vec<JType> {
    if args.is_empty() && n > 0 {
        vec![JType::class("java.lang.Object"); n]
    } else {
        args.to_vec()
    }
}

/// A known supertype, with its type arguments.
enum Source<'t> {
    Jdk(&'static jdk::JdkClass, Vec<JType>),
    Local(Node<'t>, Vec<JType>),
}

/// The known supertypes of one class, built once per class with [`Ctx::override_index`] and
/// queried per method with [`Ctx::overrides_known_method`] (which only instantiates the
/// parameter types of methods with a matching name and arity).
pub(crate) struct OverrideIndex<'t> {
    sources: Vec<Source<'t>>,
    /// Names of the methods of every source: most methods are rejected by name alone.
    names: FxHashSet<&'t str>,
}

impl<'t> Ctx<'t> {
    /// The supertypes whose methods members of `owner` (a class, enum, record, or anonymous
    /// class — `None` for interfaces and annotation types) may override: those known to the
    /// resolver (declared in this file or in the JDK table), with type arguments substituted.
    /// `Object` always counts, also when the superclass is unknown.
    pub(crate) fn override_index(&self, owner: Node<'t>) -> Option<OverrideIndex<'t>> {
        let _g = self.section(crate::timing::Section::Overrides);
        if !matches!(
            k(owner),
            K::ClassDecl | K::EnumDecl | K::RecordDecl | K::ObjectCreation | K::EnumConstant
        ) {
            return None;
        }
        let mut sources: Vec<Source<'t>> = Vec::new();
        let mut seen_jdk: Vec<&'static str> = Vec::new();
        let mut seen_local: Vec<usize> = vec![owner.id()];
        // `Object` is processed last (the queue is a stack).
        let mut queue: Vec<JType> = vec![JType::class("java.lang.Object")];
        queue.extend(self.owner_supertypes(owner));
        while let Some(t) = queue.pop() {
            if sources.len() > MAX_TYPES {
                break;
            }
            let JType::Class(ct) = t else { continue };
            if let Some(decl) = self.local_decl(&ct.name) {
                if seen_local.contains(&decl.id()) {
                    continue;
                }
                seen_local.push(decl.id());
                let Some(info) = self.class_info(decl) else {
                    continue;
                };
                let args = bind_raw(&ct.args, info.tparams.len());
                queue.extend(self.local_supertypes(decl, &args).0);
                sources.push(Source::Local(decl, args));
            } else if let Some(c) = jdk::class(&ct.name) {
                if seen_jdk.contains(&c.fqn) {
                    continue;
                }
                seen_jdk.push(c.fqn);
                let args = bind_raw(&ct.args, c.tparams.len());
                queue.extend(c.direct_supertypes(&args));
                sources.push(Source::Jdk(c, args));
            }
        }
        let mut names: FxHashSet<&'t str> = FxHashSet::default();
        for s in &sources {
            match s {
                Source::Jdk(c, _) => names.extend(c.methods.iter().map(|m| m.name)),
                Source::Local(decl, _) => {
                    if let Some(info) = self.class_info(*decl) {
                        names.extend(info.method_names.iter().copied());
                    }
                }
            }
        }
        Some(OverrideIndex { sources, names })
    }

    /// Whether `method` (a `method_declaration` of the class `index` was built for) overrides or
    /// implements a method of a supertype in the index: same name, same arity, same erased
    /// parameter types. Static and private methods override nothing.
    pub(crate) fn overrides_known_method(
        &self,
        method: Node<'t>,
        index: &OverrideIndex<'t>,
    ) -> bool {
        let _g = self.section(crate::timing::Section::Overrides);
        if k(method) != K::MethodDecl
            || syntax::has_modifier(method, "static")
            || syntax::has_modifier(method, "private")
        {
            return false;
        }
        let name = syntax::name_of(method, self.src);
        if !index.names.contains(name) {
            return false;
        }
        let arity = syntax::parameters(method).count();
        // The declared parameter types, computed on the first candidate.
        let mut declared: Option<Vec<JType>> = None;
        let mut matches = |sup: &mut dyn Iterator<Item = JType>| -> bool {
            let declared = declared.get_or_insert_with(|| {
                syntax::parameters(method)
                    .map(|p| self.signature_type(p, method, &[]))
                    .collect()
            });
            declared
                .iter()
                .all(|d| sup.next().is_some_and(|s| param_matches(d, &s)))
        };
        for source in &index.sources {
            match source {
                Source::Jdk(c, args) => {
                    for m in c
                        .methods
                        .iter()
                        .filter(|m| !m.is_static && m.name == name && m.params.len() == arity)
                    {
                        let env = Env {
                            names: &c.tparams,
                            args,
                            method_tparams: &m.tparams,
                        };
                        if matches(&mut m.params.iter().map(|p| jdk::inst(p, &env))) {
                            return true;
                        }
                    }
                }
                Source::Local(decl, args) => {
                    let Some(info) = self.class_info(*decl) else {
                        continue;
                    };
                    for m in info
                        .methods
                        .iter()
                        .zip(&info.method_names)
                        .filter(|(m, m_name)| {
                            **m_name == name
                                && !syntax::has_modifier(**m, "static")
                                && !syntax::has_modifier(**m, "private")
                                && syntax::parameters(**m).count() == arity
                        })
                        .map(|(m, _)| m)
                    {
                        let env: Vec<(&str, JType)> = info
                            .tparams
                            .iter()
                            .enumerate()
                            .map(|(i, n)| (*n, args.get(i).cloned().unwrap_or(JType::Unknown)))
                            .collect();
                        if matches(
                            &mut syntax::parameters(*m).map(|p| self.signature_type(p, *m, &env)),
                        ) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Direct supertypes of the class owning the method, with the type arguments written in its
    /// header (placeholders for types the resolver cannot see). Unknown supertypes are left out.
    fn owner_supertypes(&self, owner: Node<'t>) -> Vec<JType> {
        let Some(info) = self.class_info(owner) else {
            return Vec::new();
        };
        if let Some(enum_decl) = info.enum_owner {
            return self.class_type(enum_decl).into_iter().collect();
        }
        let mut out = Vec::new();
        for n in &info.super_nodes {
            let JType::Class(mut ct) = self.type_from_node(*n, &[], 0) else {
                continue;
            };
            if self.local_decl(&ct.name).is_none() && jdk::class(&ct.name).is_none() {
                continue;
            }
            if kind(*n) == "generic_type" {
                if let Some(targs) = named_children(*n).find(|c| kind(*c) == "type_arguments") {
                    ct.args = named_children(targs)
                        .map(|a| self.signature_type(a, owner, &[]))
                        .collect();
                    if ct.args.is_empty() {
                        // Diamond: the arguments are inferred from the context.
                        let n = self
                            .local_decl(&ct.name)
                            .and_then(|d| self.class_info(d))
                            .map(|i| i.tparams.len())
                            .or_else(|| jdk::class(&ct.name).map(|c| c.tparams.len()))
                            .unwrap_or(0);
                        ct.args = vec![JType::class(INFERRED); n];
                    }
                }
            }
            out.push(JType::Class(ct));
        }
        match k(owner) {
            K::EnumDecl => {
                let me = self.class_type(owner).unwrap_or(JType::Unknown);
                out.push(JType::generic("java.lang.Enum", vec![me]));
            }
            K::RecordDecl => out.push(JType::class("java.lang.Record")),
            _ => {}
        }
        out
    }

    /// The erased-comparison type of a parameter (`formal_parameter`, `spread_parameter`) or of a
    /// type node, seen from `member` (the method or class it belongs to): resolved when
    /// possible, `Unknown` for a type variable of `member` when it is a method, a placeholder
    /// for anything else the resolver cannot see.
    fn signature_type(&self, node: Node<'t>, member: Node<'t>, env: &[(&str, JType)]) -> JType {
        let (type_node, dims) = match kind(node) {
            "formal_parameter" | "spread_parameter" => {
                match scope::param_decl(node, VarKind::Parameter, self.src) {
                    Some(d) => match d.type_node {
                        Some(t) => (t, d.dims),
                        None => return JType::Unknown,
                    },
                    None => return JType::Unknown,
                }
            }
            _ => (node, 0),
        };
        let mut t = self.erasable_type(type_node, member, env, 0);
        for _ in 0..dims {
            t = JType::Array(Box::new(t));
        }
        t
    }

    fn erasable_type(
        &self,
        node: Node<'t>,
        member: Node<'t>,
        env: &[(&str, JType)],
        depth: u32,
    ) -> JType {
        if depth > 8 {
            return JType::Unknown;
        }
        let t = self.type_from_node(node, env, 0);
        if !t.is_unknown() {
            return t;
        }
        match kind(node) {
            "type_identifier" => {
                let name = self.text(node);
                if k(member) == K::MethodDecl && self.has_type_param(member, name) {
                    JType::Unknown
                } else {
                    placeholder(name)
                }
            }
            "scoped_type_identifier" | "generic_type" => placeholder(self.text(node)),
            "array_type" => match node.field(Field::Element) {
                Some(e) => {
                    let dims = scope::dims_of(node.field(Field::Dimensions), self.src).max(1);
                    let mut t = self.erasable_type(e, member, env, depth + 1);
                    for _ in 0..dims {
                        t = JType::Array(Box::new(t));
                    }
                    t
                }
                None => JType::Unknown,
            },
            "annotated_type" => named_children(node)
                .find(|c| !matches!(kind(*c), "annotation" | "marker_annotation"))
                .map(|c| self.erasable_type(c, member, env, depth + 1))
                .unwrap_or(JType::Unknown),
            _ => JType::Unknown,
        }
    }
}
