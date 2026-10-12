//! The Java resolver: one strategy, file-local.
//!
//! Knows: declarations in this file (types, fields, methods, locals, parameters, patterns),
//! imports (single-type, wildcard, static), implicit `java.lang`, and a curated JDK table.
//! Everything else — types from other files of the repository or from libraries — is unknown,
//! and the public API answers `None`/`false` so type-dependent rules stay silent.

pub(crate) mod jdk;
mod overrides;
pub(crate) use overrides::OverrideIndex;
pub(crate) mod scope;
pub mod types;

pub use jdk::{is_enum_class, is_final_class, is_known_class};
pub use scope::{VarDecl, VarKind};
pub use types::{ClassType, JType, Prim};

use crate::ctx::Ctx;
use crate::kinds::{k, kind, K};
use crate::syntax::{self, named_children, node_text, operator_str, strip_parens};
use crate::syntax::{Field, NodeExt};
use jdk::{ClassKind, Env};
use rustc_hash::FxHashMap;
use scope::{Entry, Scoped};
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;
use tree_sitter::Node;

/// Recursion budget for expression typing (deep or cyclic inputs give up, never overflow).
const MAX_DEPTH: u32 = 40;

/// The declarations of a member ([`scope::build_scope`]), indexed by name: a member declaring
/// thousands of locals made every name lookup in it scan all of them.
pub(crate) struct MemberScope<'t> {
    entries: Vec<Scoped<'t>>,
    /// Entry indices by name, in entry order.
    by_name: FxHashMap<&'t str, Vec<u32>>,
}

impl<'t> MemberScope<'t> {
    fn new(entries: Vec<Scoped<'t>>) -> Self {
        let mut by_name: FxHashMap<&'t str, Vec<u32>> = FxHashMap::default();
        for (i, s) in entries.iter().enumerate() {
            by_name.entry(s.name).or_default().push(i as u32);
        }
        MemberScope { entries, by_name }
    }

    /// The entries named `name`, in entry order.
    fn named<'s>(&'s self, name: &str) -> impl Iterator<Item = &'s Scoped<'t>> + 's {
        self.by_name
            .get(name)
            .into_iter()
            .flatten()
            .filter_map(|&i| self.entries.get(i as usize))
    }
}

/// Per-file resolver caches (interior mutability; never borrowed across calls). Every memo
/// caches a pure function of the file and its key, so results do not depend on query order.
#[derive(Default)]
pub(crate) struct ResolveCache<'t> {
    scopes: RefCell<FxHashMap<usize, Rc<MemberScope<'t>>>>,
    classes: RefCell<FxHashMap<usize, Rc<ClassInfo<'t>>>>,
    local_types: OnceCell<FxHashMap<String, Option<Node<'t>>>>,
    memo: RefCell<FxHashMap<usize, Option<JType>>>,
    /// Type-name node → resolution of its (dotted) name at that node (see `named_type`).
    named: RefCell<FxHashMap<usize, TypeRes>>,
    /// Simple name → resolution among this file's top-level types, imports and `java.lang`.
    file_level: RefCell<FxHashMap<Box<str>, TypeRes>>,
    /// Expression → `type_ref` result.
    type_refs: RefCell<FxHashMap<usize, Option<TypeRes>>>,
    /// Identifier → `lookup_var(text, identifier)`.
    vars: RefCell<FxHashMap<usize, Lookup<'t>>>,
    /// Class declaration → `class_type`.
    class_types: RefCell<FxHashMap<usize, Option<JType>>>,
    /// Method/constructor → whether its text may declare a local type (see
    /// `scope_has_local_class`).
    may_declare_types: RefCell<FxHashMap<usize, bool>>,
    /// Times the recursion budget ([`MAX_DEPTH`]) cut a computation short. A type computed
    /// while this grew depends on how deep the query started, so it is not memoized: every
    /// memoized type is a function of the expression alone, whatever the order of queries.
    truncated: Cell<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LocalKind {
    Class,
    Interface,
    Enum,
    Record,
    Annotation,
    Anonymous,
}

/// Facts about one class-like declaration in this file.
pub(crate) struct ClassInfo<'t> {
    pub decl: Node<'t>,
    pub kind: LocalKind,
    pub tparams: Vec<&'t str>,
    pub fields: Vec<VarDecl<'t>>,
    /// The first of `fields` by name: a class of thousands of fields made every field lookup
    /// scan them all.
    field_index: FxHashMap<&'t str, usize>,
    pub methods: Vec<Node<'t>>,
    /// Names of `methods`, in the same order.
    pub method_names: Vec<&'t str>,
    /// Declared supertype nodes (superclass first). For anonymous classes: the instantiated type.
    pub super_nodes: Vec<Node<'t>>,
    /// For enum-constant bodies: the enum declaration.
    pub enum_owner: Option<Node<'t>>,
    /// Every `type_identifier` directly inside a node of `type_parameters` (what
    /// `has_type_param` looks for).
    pub tparam_idents: Vec<&'t str>,
    /// Member type declarations of the body, in source order: (name, declaration).
    pub member_types: Vec<(&'t str, Node<'t>)>,
    /// Cached [`Ctx::has_unknown_supertypes`].
    unknown_supers: OnceCell<bool>,
}

#[derive(Clone)]
enum TypeRes {
    Known(JType),
    TypeVar { method_level: bool, name: String },
    External(String),
    NotFound,
}

#[derive(Clone, Copy)]
enum Lookup<'t> {
    Found(VarDecl<'t>),
    /// Not found, and an unknown supertype might declare it.
    Blocked,
    NotFound,
}

/// Candidate method signature for overload resolution.
struct Cand {
    params: Vec<JType>,
    varargs: bool,
    ret: JType,
}

/// Whether `text` contains `class`, `interface`, `enum`, or `record` as a whole word that is not
/// preceded by `.` (so `Foo.class` and `x.record` don't count). A local type declaration always
/// contains one; the converse doesn't hold (`record` may be a variable), which is fine for a
/// pre-filter.
fn text_may_declare_types(text: &str) -> bool {
    let b = text.as_bytes();
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c >= 0x80;
    ["class", "interface", "enum", "record"].iter().any(|kw| {
        let mut from = 0;
        while let Some(p) = text.get(from..).and_then(|t| t.find(kw)) {
            let i = from + p;
            let j = i + kw.len();
            let before = i.checked_sub(1).and_then(|x| b.get(x)).copied();
            let after = b.get(j).copied();
            if !before.is_some_and(|c| ident(c) || c == b'.') && !after.is_some_and(ident) {
                return true;
            }
            from = j;
        }
        false
    })
}

fn known(t: JType) -> Option<JType> {
    if t.is_unknown() {
        None
    } else {
        Some(t)
    }
}

/// Assignability for overload selection: `Some(false)` only when provably not assignable.
fn assignable(ctx: &Ctx<'_>, arg: &JType, param: &JType, boxing: bool) -> Option<bool> {
    match (arg, param) {
        (JType::Unknown, _) | (_, JType::Unknown) => None,
        (JType::Prim(a), JType::Prim(p)) => Some(a.widens_to(*p)),
        (JType::Prim(a), JType::Class(_)) => {
            if !boxing {
                return Some(false);
            }
            let boxed = JType::class(a.boxed_fqn());
            ctx.subtype(&boxed, param.fqn().unwrap_or(""))
        }
        (JType::Class(_), JType::Prim(p)) => {
            if !boxing {
                return Some(false);
            }
            Some(arg.unboxed().is_some_and(|u| u.widens_to(*p)))
        }
        (JType::Null, JType::Prim(_)) => Some(false),
        (JType::Null, _) => Some(true),
        (JType::Class(_), JType::Class(pc)) | (JType::Array(_), JType::Class(pc)) => {
            ctx.subtype(arg, &pc.name)
        }
        (JType::Array(a), JType::Array(p)) => match (a.as_ref(), p.as_ref()) {
            (JType::Prim(x), JType::Prim(y)) => Some(x == y),
            (JType::Prim(_), _) | (_, JType::Prim(_)) => Some(false),
            (x, y) => assignable(ctx, x, y, false),
        },
        (JType::Class(_), JType::Array(_)) => Some(false),
        (JType::Void, _) | (_, JType::Void) => Some(false),
        _ => None,
    }
}

fn pick(ctx: &Ctx<'_>, cands: &[Cand], args: &[JType]) -> Option<JType> {
    let n = args.len();
    let fixed = |boxing: bool| -> Vec<&Cand> {
        cands
            .iter()
            .filter(|c| c.params.len() == n)
            .filter(|c| {
                c.params
                    .iter()
                    .zip(args)
                    .all(|(p, a)| assignable(ctx, a, p, boxing) != Some(false))
            })
            .collect()
    };
    let mut set = fixed(false);
    if set.is_empty() {
        set = fixed(true);
    }
    if set.is_empty() {
        set = cands
            .iter()
            .filter(|c| c.varargs && !c.params.is_empty() && n + 1 >= c.params.len())
            .filter(|c| {
                let last = c.params.len() - 1;
                let elem = match &c.params[last] {
                    JType::Array(e) => (**e).clone(),
                    other => other.clone(),
                };
                args.iter().enumerate().all(|(i, a)| {
                    let p = if i < last { &c.params[i] } else { &elem };
                    assignable(ctx, a, p, true) != Some(false)
                })
            })
            .collect();
    }
    let first = set.first()?;
    if set.iter().all(|c| c.ret == first.ret) {
        return Some(first.ret.clone());
    }
    let exact: Vec<&&Cand> = set
        .iter()
        .filter(|c| {
            c.params.len() == n
                && c.params
                    .iter()
                    .zip(args)
                    .all(|(p, a)| a.is_fully_known() && a == p)
        })
        .collect();
    if exact.len() == 1 {
        return Some(exact[0].ret.clone());
    }
    // Most specific: parameters assignable to every other candidate's parameters.
    let specific: Vec<&&Cand> = set
        .iter()
        .filter(|c| {
            set.iter().all(|d| {
                std::ptr::eq(**c, *d)
                    || (c.params.len() == d.params.len()
                        && c.params
                            .iter()
                            .zip(&d.params)
                            .all(|(x, y)| assignable(ctx, x, y, false) == Some(true)))
            })
        })
        .collect();
    if specific.len() == 1 {
        return Some(specific[0].ret.clone());
    }
    None
}

impl<'t> Ctx<'t> {
    // =================================================================== public API

    /// Static type of an expression, when it can be determined with certainty from this file,
    /// its imports and the JDK table. Handles identifiers (locals, parameters, fields, `var`),
    /// literals, `new`, casts, parentheses, string concatenation and arithmetic, array access,
    /// ternaries, `this`, field access, and method calls on known receivers (with generic
    /// substitution) or to methods declared in this file. Returns `None` otherwise.
    pub fn type_of(&self, expr: Node<'t>) -> Option<JType> {
        let _g = self.section(crate::timing::Section::Resolver);
        known(self.type_of_d(expr, 0))
    }

    /// Resolve a type node (`int`, `List<String>`, `Map.Entry<K, V>`, `String[]`). `None` for
    /// `var`, type variables, and types unknown to the resolver.
    pub fn resolve_type(&self, type_node: Node<'t>) -> Option<JType> {
        let _g = self.section(crate::timing::Section::Resolver);
        known(self.type_from_node(type_node, &[], 0))
    }

    /// The declaration an identifier refers to (local, parameter, field, enum constant, …),
    /// or `None` when it is not a variable of this file (or an unknown supertype could declare it).
    pub fn declaration_of(&self, ident: Node<'t>) -> Option<VarDecl<'t>> {
        let _g = self.section(crate::timing::Section::Resolver);
        if kind(ident) != "identifier" {
            return None;
        }
        match self.lookup_var(self.text(ident), ident) {
            Lookup::Found(d) => Some(d),
            _ => None,
        }
    }

    /// Fully-qualified name for a type name used at `at` (`List` → `java.util.List`,
    /// `Foo` → `com.acme.Foo` via an import or this file). Includes imported types that are
    /// otherwise unknown. `None` for type variables and unresolvable names.
    pub fn resolve_type_name(&self, name: &str, at: Node<'t>) -> Option<String> {
        let _g = self.section(crate::timing::Section::Resolver);
        let res = if name.contains('.') {
            self.resolve_dotted(name, at)
        } else {
            self.resolve_simple(name, at)
        };
        match res {
            TypeRes::Known(t) => t.fqn().map(str::to_string),
            TypeRes::External(f) => Some(f),
            _ => None,
        }
    }

    /// If `expr` (identifier or dotted name) names a type rather than a value — the receiver
    /// of a static call like `Math.max(..)` — its fully-qualified name.
    pub fn type_name_of(&self, expr: Node<'t>) -> Option<String> {
        let _g = self.section(crate::timing::Section::Resolver);
        match self.type_ref(expr)? {
            TypeRes::Known(t) => t.fqn().map(str::to_string),
            TypeRes::External(f) => Some(f),
            _ => None,
        }
    }

    /// Whether `t` is `fqn` or a subtype of it: `Some(true)`/`Some(false)` when provable,
    /// `None` when part of the hierarchy is unknown.
    pub fn subtype(&self, t: &JType, fqn: &str) -> Option<bool> {
        let _g = self.section(crate::timing::Section::Resolver);
        let _s = self.section(crate::timing::Section::Subtype);
        match t {
            JType::Null => Some(true),
            JType::Prim(_) | JType::Void => Some(false),
            JType::Unknown => None,
            JType::Array(_) => Some(matches!(
                fqn,
                "java.lang.Object" | "java.lang.Cloneable" | "java.io.Serializable"
            )),
            JType::Class(_) => {
                if fqn == "java.lang.Object" {
                    return Some(true);
                }
                let mut complete = true;
                let mut seen: Vec<String> = Vec::new();
                let mut queue: Vec<JType> = vec![t.clone()];
                let mut steps = 0;
                while let Some(cur) = queue.pop() {
                    steps += 1;
                    if steps > 256 {
                        return None;
                    }
                    let Some(name) = cur.fqn() else { continue };
                    if name == fqn {
                        return Some(true);
                    }
                    if seen.iter().any(|s| s == name) {
                        continue;
                    }
                    seen.push(name.to_string());
                    let (supers, ok) = self.direct_supertypes(&cur);
                    complete &= ok;
                    queue.extend(supers);
                }
                if complete {
                    Some(false)
                } else {
                    None
                }
            }
        }
    }

    /// `true` only when `t` is provably `fqn` or a subtype of it.
    pub fn is_subtype_of(&self, t: &JType, fqn: &str) -> bool {
        self.subtype(t, fqn) == Some(true)
    }

    /// `t` viewed as its supertype `fqn`, with type arguments substituted
    /// (`ArrayList<String>` as `java.lang.Iterable` → `Iterable<String>`).
    pub fn as_supertype(&self, t: &JType, fqn: &str) -> Option<JType> {
        let _g = self.section(crate::timing::Section::Resolver);
        let mut queue = vec![t.clone()];
        let mut steps = 0;
        while let Some(cur) = queue.pop() {
            steps += 1;
            if steps > 256 {
                return None;
            }
            if cur.fqn() == Some(fqn) {
                return Some(cur);
            }
            queue.extend(self.direct_supertypes(&cur).0);
        }
        None
    }

    /// Whether `call` (a `method_invocation`) calls method `method` of type `owner` (FQN) or of
    /// a subtype — through an instance receiver, a static type reference, a static import, or an
    /// inherited unqualified call. `true` only when provable.
    pub fn is_call_to(&self, call: Node<'t>, owner: &str, method: &str) -> bool {
        let _g = self.section(crate::timing::Section::Resolver);
        if kind(call) != "method_invocation" {
            return false;
        }
        let Some(name) = call.field(Field::Name) else {
            return false;
        };
        if self.text(name) != method {
            return false;
        }
        match call.field(Field::Object) {
            None => {
                if self
                    .enclosing_class_type(call)
                    .is_some_and(|t| self.is_subtype_of(&t, owner))
                {
                    return true;
                }
                if self.declares_method_in_chain(call, method) {
                    return false;
                }
                let full = format!("{owner}.{method}");
                self.imports.iter().any(|i| {
                    i.is_static
                        && ((!i.is_wildcard && i.path == full)
                            || (i.is_wildcard && i.path == owner))
                })
            }
            Some(obj) if kind(obj) == "super" => false,
            Some(obj) => {
                if let Some(res) = self.type_ref(obj) {
                    return match res {
                        TypeRes::Known(t) => self.is_subtype_of(&t, owner),
                        TypeRes::External(f) => f == owner,
                        _ => false,
                    };
                }
                self.type_of(obj)
                    .is_some_and(|t| self.is_subtype_of(&t, owner))
            }
        }
    }

    /// The type of a class declared in this file (`class_declaration`, `enum_declaration`, …)
    /// with its type parameters as unknown arguments. `None` for local and anonymous classes.
    pub fn class_type(&self, decl: Node<'t>) -> Option<JType> {
        let _g = self.section(crate::timing::Section::Resolver);
        if let Ok(map) = self.res.class_types.try_borrow() {
            if let Some(t) = map.get(&decl.id()) {
                return t.clone();
            }
        }
        let t = self.compute_class_type(decl);
        if let Ok(mut map) = self.res.class_types.try_borrow_mut() {
            map.insert(decl.id(), t.clone());
        }
        t
    }

    fn compute_class_type(&self, decl: Node<'t>) -> Option<JType> {
        let fqn = self.class_fqn(decl)?;
        let n = self.class_info(decl).map(|i| i.tparams.len()).unwrap_or(0);
        Some(JType::generic(fqn, vec![JType::Unknown; n]))
    }

    /// Fully-qualified name of a top-level or member type declared in this file.
    pub fn class_fqn(&self, decl: Node<'t>) -> Option<String> {
        let _g = self.section(crate::timing::Section::Resolver);
        if !k(decl).is_type_decl() {
            return None;
        }
        let mut names = vec![syntax::name_of(decl, self.src)];
        for a in self.ancestors(decl) {
            match k(a) {
                K::Program => break,
                kk if kk.is_type_body() => {}
                kk if kk.is_type_decl() => names.push(syntax::name_of(a, self.src)),
                _ => return None,
            }
        }
        let mut s = String::new();
        if let Some(p) = self.package {
            s.push_str(p);
            s.push('.');
        }
        for (i, n) in names.iter().rev().enumerate() {
            if i > 0 {
                s.push('.');
            }
            s.push_str(n);
        }
        Some(s)
    }

    // =================================================================== classes

    pub(crate) fn class_info(&self, decl: Node<'t>) -> Option<Rc<ClassInfo<'t>>> {
        if let Ok(map) = self.res.classes.try_borrow() {
            if let Some(i) = map.get(&decl.id()) {
                return Some(i.clone());
            }
        }
        let info = {
            let _g = self.section(crate::timing::Section::ClassInfo);
            Rc::new(self.build_class_info(decl)?)
        };
        if let Ok(mut map) = self.res.classes.try_borrow_mut() {
            map.insert(decl.id(), info.clone());
        }
        Some(info)
    }

    fn build_class_info(&self, decl: Node<'t>) -> Option<ClassInfo<'t>> {
        let src = self.src;
        let kind_ = match k(decl) {
            K::ClassDecl => LocalKind::Class,
            K::InterfaceDecl => LocalKind::Interface,
            K::EnumDecl => LocalKind::Enum,
            K::RecordDecl => LocalKind::Record,
            K::AnnotationTypeDecl => LocalKind::Annotation,
            K::ObjectCreation | K::EnumConstant => LocalKind::Anonymous,
            _ => return None,
        };
        let tparams: Vec<&'t str> = decl
            .field(Field::TypeParameters)
            .into_iter()
            .flat_map(named_children)
            .filter_map(|tp| named_children(tp).find(|c| kind(*c) == "type_identifier"))
            .map(|n| node_text(n, src))
            .collect();
        let tparam_idents: Vec<&'t str> = decl
            .field(Field::TypeParameters)
            .into_iter()
            .flat_map(named_children)
            .flat_map(named_children)
            .filter(|c| kind(*c) == "type_identifier")
            .map(|n| node_text(n, src))
            .collect();
        let mut member_types = Vec::new();
        let mut super_nodes = Vec::new();
        let mut enum_owner = None;
        match k(decl) {
            K::ClassDecl => {
                if let Some(sc) = decl.field(Field::Superclass) {
                    super_nodes.extend(named_children(sc));
                }
                if let Some(si) = decl.field(Field::Interfaces) {
                    super_nodes.extend(named_children(si).flat_map(named_children));
                }
            }
            K::EnumDecl | K::RecordDecl => {
                if let Some(si) = decl.field(Field::Interfaces) {
                    super_nodes.extend(named_children(si).flat_map(named_children));
                }
            }
            K::InterfaceDecl => {
                for c in named_children(decl).filter(|c| kind(*c) == "extends_interfaces") {
                    super_nodes.extend(named_children(c).flat_map(named_children));
                }
            }
            K::ObjectCreation => super_nodes.extend(decl.field(Field::Type)),
            K::EnumConstant => enum_owner = decl.parent().and_then(|b| b.parent()),
            _ => {}
        }
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        if k(decl) == K::RecordDecl {
            if let Some(params) = decl.field(Field::Parameters) {
                for p in named_children(params) {
                    if let Some(d) = scope::param_decl(p, VarKind::RecordComponent, src) {
                        fields.push(d);
                    }
                }
            }
        }
        if let Some(body) = syntax::type_body(decl) {
            for m in self.members(body) {
                match k(m) {
                    K::FieldDecl | K::ConstantDecl => {
                        fields.extend(scope::declarator_decls(m, VarKind::Field, src))
                    }
                    K::EnumConstant => {
                        if let Some(name) = m.field(Field::Name) {
                            fields.push(VarDecl {
                                kind: VarKind::EnumConstant,
                                name,
                                decl: m,
                                statement: m,
                                type_node: None,
                                value: None,
                                dims: 0,
                                is_var: false,
                            });
                        }
                    }
                    K::MethodDecl => methods.push(m),
                    kk if kk.is_type_decl() => member_types.push((syntax::name_of(m, src), m)),
                    _ => {}
                }
            }
        }
        let mut field_index: FxHashMap<&'t str, usize> = FxHashMap::default();
        for (i, f) in fields.iter().enumerate() {
            field_index.entry(node_text(f.name, src)).or_insert(i);
        }
        Some(ClassInfo {
            decl,
            kind: kind_,
            tparams,
            fields,
            field_index,
            method_names: methods.iter().map(|m| syntax::name_of(*m, src)).collect(),
            methods,
            super_nodes,
            enum_owner,
            tparam_idents,
            member_types,
            unknown_supers: OnceCell::new(),
        })
    }

    /// Map of FQN → declaration for every top-level and member type of this file (`None` marks
    /// a duplicated name).
    fn local_types(&self) -> &FxHashMap<String, Option<Node<'t>>> {
        self.res.local_types.get_or_init(|| {
            let mut map: FxHashMap<String, Option<Node<'t>>> = FxHashMap::default();
            let prefix = self.package.map(|p| format!("{p}.")).unwrap_or_default();
            let mut stack: Vec<(Node<'t>, String)> = self
                .top_types
                .iter()
                .map(|&c| (c, prefix.clone()))
                .collect();
            let mut guard = 0;
            while let Some((decl, pre)) = stack.pop() {
                guard += 1;
                if guard > 100_000 {
                    break;
                }
                let fqn = format!("{pre}{}", syntax::name_of(decl, self.src));
                if let Some(body) = syntax::type_body(decl) {
                    for m in self.members(body).filter(|m| k(*m).is_type_decl()) {
                        stack.push((m, format!("{fqn}.")));
                    }
                }
                match map.get_mut(&fqn) {
                    Some(slot) => *slot = None,
                    None => {
                        map.insert(fqn, Some(decl));
                    }
                }
            }
            map
        })
    }

    fn local_decl(&self, fqn: &str) -> Option<Node<'t>> {
        self.local_types().get(fqn).copied().flatten()
    }

    /// Direct supertypes of a type, and whether the list is complete.
    fn direct_supertypes(&self, t: &JType) -> (Vec<JType>, bool) {
        match t {
            JType::Array(_) => (
                vec![
                    JType::class("java.lang.Object"),
                    JType::class("java.lang.Cloneable"),
                    JType::class("java.io.Serializable"),
                ],
                true,
            ),
            JType::Class(ct) => {
                if let Some(decl) = self.local_decl(&ct.name) {
                    return self.local_supertypes(decl, &ct.args);
                }
                match jdk::class(&ct.name) {
                    Some(c) => (c.direct_supertypes(&ct.args), true),
                    None => (Vec::new(), false),
                }
            }
            _ => (Vec::new(), true),
        }
    }

    fn local_supertypes(&self, decl: Node<'t>, args: &[JType]) -> (Vec<JType>, bool) {
        let Some(info) = self.class_info(decl) else {
            return (Vec::new(), false);
        };
        let env: Vec<(&str, JType)> = info
            .tparams
            .iter()
            .enumerate()
            .map(|(i, n)| (*n, args.get(i).cloned().unwrap_or(JType::Unknown)))
            .collect();
        let mut out = Vec::new();
        let mut complete = true;
        for n in &info.super_nodes {
            let t = self.type_from_node(*n, &env, 0);
            if t.fqn()
                .is_some_and(|f| self.local_decl(f).is_some() || jdk::class(f).is_some())
            {
                out.push(t);
            } else {
                complete = false;
            }
        }
        match info.kind {
            LocalKind::Enum => {
                let me = self.class_type(decl).unwrap_or(JType::Unknown);
                out.push(JType::generic("java.lang.Enum", vec![me]));
            }
            LocalKind::Record => out.push(JType::class("java.lang.Record")),
            LocalKind::Annotation => out.push(JType::class("java.lang.annotation.Annotation")),
            _ => {}
        }
        let has_class_super = info.kind == LocalKind::Class
            && k(decl) == K::ClassDecl
            && decl.field(Field::Superclass).is_some();
        if !has_class_super && !matches!(info.kind, LocalKind::Enum) {
            out.push(JType::class("java.lang.Object"));
        }
        (out, complete)
    }

    /// Whether any declared supertype of the class is unknown (so it may contribute members).
    fn has_unknown_supertypes(&self, info: &ClassInfo<'t>) -> bool {
        if let Some(v) = info.unknown_supers.get() {
            return *v;
        }
        // `get` + `set` rather than `get_or_init`: never panics, even if reentered.
        let v = self.compute_unknown_supertypes(info);
        let _ = info.unknown_supers.set(v);
        v
    }

    fn compute_unknown_supertypes(&self, info: &ClassInfo<'t>) -> bool {
        if info.enum_owner.is_some() {
            return false;
        }
        info.super_nodes.iter().any(|n| {
            let t = self.type_from_node(*n, &[], 0);
            match t.fqn() {
                Some(f) => {
                    if let Some(d) = self.local_decl(f) {
                        // A local superclass may itself extend something unknown.
                        d.id() != info.decl.id()
                            && self
                                .class_info(d)
                                .is_some_and(|i| self.has_unknown_supertypes_d(&i, 1))
                    } else {
                        jdk::class(f).is_none()
                    }
                }
                None => true,
            }
        })
    }

    fn has_unknown_supertypes_d(&self, info: &ClassInfo<'t>, depth: u32) -> bool {
        if depth > 16 {
            return true;
        }
        info.super_nodes.iter().any(|n| {
            let t = self.type_from_node(*n, &[], 0);
            match t.fqn() {
                Some(f) => match self.local_decl(f) {
                    Some(d) if d.id() == info.decl.id() => true,
                    Some(d) => self
                        .class_info(d)
                        .is_some_and(|i| self.has_unknown_supertypes_d(&i, depth + 1)),
                    None => jdk::class(f).is_none(),
                },
                None => true,
            }
        })
    }

    // =================================================================== names → types

    /// Resolve a simple type name visible at `at`.
    fn resolve_simple(&self, name: &str, at: Node<'t>) -> TypeRes {
        let _g = self.section(crate::timing::Section::ResolveSimple);
        if name.is_empty() {
            return TypeRes::NotFound;
        }
        let mut prev: Option<Node<'t>> = None;
        for a in std::iter::once(at).chain(self.ancestors(at)) {
            match k(a) {
                K::MethodDecl | K::ConstructorDecl => {
                    if self.has_type_param(a, name) {
                        return TypeRes::TypeVar {
                            method_level: true,
                            name: name.to_string(),
                        };
                    }
                    if self.scope_has_local_class(a, name, at) {
                        return TypeRes::NotFound;
                    }
                }
                K::ClassDecl
                | K::InterfaceDecl
                | K::EnumDecl
                | K::RecordDecl
                | K::AnnotationTypeDecl => {
                    // Class type parameters are in scope in the body, the header, and record
                    // components — not in the class's own annotations.
                    let in_scope = prev.is_some_and(|p| {
                        k(p).is_type_body()
                            || matches!(
                                kind(p),
                                "type_parameters"
                                    | "superclass"
                                    | "super_interfaces"
                                    | "extends_interfaces"
                                    | "formal_parameters"
                            )
                    });
                    // Type parameters and member types come from the (cached) class facts.
                    let info = self.class_info(a);
                    let is_tparam = match &info {
                        Some(i) => i.tparam_idents.contains(&name),
                        None => self.has_type_param(a, name),
                    };
                    if in_scope && is_tparam {
                        return TypeRes::TypeVar {
                            method_level: false,
                            name: name.to_string(),
                        };
                    }
                    if syntax::name_of(a, self.src) == name {
                        if let Some(t) = self.class_type(a) {
                            return TypeRes::Known(t);
                        }
                        return TypeRes::NotFound;
                    }
                    let member = match &info {
                        Some(i) => i
                            .member_types
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, m)| *m),
                        None => syntax::type_body(a).and_then(|body| {
                            self.members(body).find(|m| {
                                k(*m).is_type_decl() && syntax::name_of(*m, self.src) == name
                            })
                        }),
                    };
                    if let Some(m) = member {
                        return match self.class_type(m) {
                            Some(t) => TypeRes::Known(t),
                            None => TypeRes::NotFound,
                        };
                    }
                }
                _ => {}
            }
            prev = Some(a);
        }
        self.resolve_file_level(name)
    }

    /// The part of [`Ctx::resolve_simple`] that does not depend on the position: top-level
    /// types of this file, single-type imports, on-demand imports of JDK packages, `java.lang`.
    /// Memoized per name.
    fn resolve_file_level(&self, name: &str) -> TypeRes {
        if let Ok(map) = self.res.file_level.try_borrow() {
            if let Some(r) = map.get(name) {
                return r.clone();
            }
        }
        let r = self.compute_file_level(name);
        if let Ok(mut map) = self.res.file_level.try_borrow_mut() {
            map.insert(name.into(), r.clone());
        }
        r
    }

    fn compute_file_level(&self, name: &str) -> TypeRes {
        // Top-level types of this file.
        for &c in &self.top_types {
            if syntax::name_of(c, self.src) == name {
                return self
                    .class_type(c)
                    .map(TypeRes::Known)
                    .unwrap_or(TypeRes::NotFound);
            }
        }
        // Single-type imports.
        for imp in self
            .imports
            .iter()
            .filter(|i| !i.is_static && !i.is_wildcard)
        {
            if imp.simple_name() == name {
                return self.fqn_res(imp.path);
            }
        }
        // On-demand imports of JDK packages / classes.
        for imp in self
            .imports
            .iter()
            .filter(|i| !i.is_static && i.is_wildcard)
        {
            let cand = format!("{}.{name}", imp.path);
            if jdk::class(&cand).is_some() {
                return TypeRes::Known(JType::class(cand));
            }
        }
        let lang = format!("java.lang.{name}");
        if jdk::class(&lang).is_some() {
            return TypeRes::Known(JType::class(lang));
        }
        TypeRes::NotFound
    }

    fn fqn_res(&self, fqn: &str) -> TypeRes {
        if let Some(c) = jdk::class(fqn) {
            return TypeRes::Known(JType::class(c.fqn));
        }
        if self.local_decl(fqn).is_some() {
            return TypeRes::Known(JType::class(fqn.to_string()));
        }
        TypeRes::External(fqn.to_string())
    }

    fn resolve_dotted(&self, dotted: &str, at: Node<'t>) -> TypeRes {
        let (first, rest) = dotted.split_once('.').unwrap_or((dotted, ""));
        if rest.is_empty() {
            return self.resolve_simple(first, at);
        }
        match self.resolve_simple(first, at) {
            // A known class followed by a name that isn't one of its known nested types is a
            // field access (e.g. `TimeUnit.SECONDS`), not a type.
            TypeRes::Known(t) => match t.fqn() {
                Some(f) => match self.fqn_res(&format!("{f}.{rest}")) {
                    r @ TypeRes::Known(_) => r,
                    _ => TypeRes::NotFound,
                },
                None => TypeRes::NotFound,
            },
            TypeRes::External(f) => TypeRes::External(format!("{f}.{rest}")),
            TypeRes::TypeVar { .. } => TypeRes::NotFound,
            TypeRes::NotFound => {
                if first.starts_with(|c: char| c.is_lowercase()) {
                    self.fqn_res(dotted)
                } else {
                    TypeRes::NotFound
                }
            }
        }
    }

    fn has_type_param(&self, decl: Node<'t>, name: &str) -> bool {
        decl.field(Field::TypeParameters).is_some_and(|tps| {
            named_children(tps).any(|tp| {
                named_children(tp).any(|c| kind(c) == "type_identifier" && self.text(c) == name)
            })
        })
    }

    fn scope_has_local_class(&self, member: Node<'t>, name: &str, at: Node<'t>) -> bool {
        // Most members declare no local type: don't index a whole method body to find that out
        // (unless the index exists already).
        let indexed = self
            .res
            .scopes
            .try_borrow()
            .is_ok_and(|m| m.contains_key(&member.id()));
        if !indexed && !self.may_declare_types(member) {
            return false;
        }
        let pos = at.start_byte() as u32;
        self.member_scope(member)
            .named(name)
            .any(|s| matches!(s.entry, Entry::Class) && s.start <= pos && pos < s.end)
    }

    /// Whether the text of `member` contains a type-declaring keyword (`class`, `interface`,
    /// `enum`, `record`) as a word not preceded by `.` — a superset of "declares a local type".
    fn may_declare_types(&self, member: Node<'t>) -> bool {
        if let Ok(map) = self.res.may_declare_types.try_borrow() {
            if let Some(v) = map.get(&member.id()) {
                return *v;
            }
        }
        let v = text_may_declare_types(self.text(member));
        if let Ok(mut map) = self.res.may_declare_types.try_borrow_mut() {
            map.insert(member.id(), v);
        }
        v
    }

    /// A name at the head of a qualified expression is a variable if one is in scope, else a
    /// type or package. When an unknown supertype might declare a field of that name, only
    /// capitalized names (Java naming convention for types) are still treated as types.
    fn may_be_type_name(&self, name: &str, at: Node<'t>) -> bool {
        match self.lookup_var(name, at) {
            Lookup::Found(_) => false,
            Lookup::Blocked => name.starts_with(|c: char| c.is_uppercase()),
            Lookup::NotFound => true,
        }
    }

    /// If `expr` is a (possibly qualified) name that denotes a type rather than a variable.
    /// Memoized per node.
    fn type_ref(&self, expr: Node<'t>) -> Option<TypeRes> {
        if let Ok(map) = self.res.type_refs.try_borrow() {
            if let Some(r) = map.get(&expr.id()) {
                return r.clone();
            }
        }
        let r = self.compute_type_ref(expr);
        if let Ok(mut map) = self.res.type_refs.try_borrow_mut() {
            map.insert(expr.id(), r.clone());
        }
        r
    }

    fn compute_type_ref(&self, expr: Node<'t>) -> Option<TypeRes> {
        let expr = strip_parens(expr);
        match kind(expr) {
            "identifier" => {
                let name = self.text(expr);
                if !self.may_be_type_name(name, expr) {
                    return None;
                }
                match self.resolve_simple(name, expr) {
                    r @ (TypeRes::Known(_) | TypeRes::External(_)) => Some(r),
                    _ => None,
                }
            }
            "field_access" | "scoped_identifier" => {
                let mut parts: Vec<&'t str> = Vec::new();
                let mut cur = expr;
                for _ in 0..64 {
                    match kind(cur) {
                        "field_access" => {
                            let f = cur.field(Field::Field)?;
                            if kind(f) != "identifier" {
                                return None;
                            }
                            parts.push(self.text(f));
                            cur = cur.field(Field::Object)?;
                        }
                        "scoped_identifier" => {
                            parts.push(self.text(cur.field(Field::Name)?));
                            cur = cur.field(Field::Scope)?;
                        }
                        "identifier" => {
                            let name = self.text(cur);
                            if !self.may_be_type_name(name, cur) {
                                return None;
                            }
                            parts.push(name);
                            break;
                        }
                        _ => return None,
                    }
                }
                parts.reverse();
                let dotted = parts.join(".");
                match self.resolve_dotted(&dotted, expr) {
                    r @ (TypeRes::Known(_) | TypeRes::External(_)) => Some(r),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Resolve a type node. `env` maps class type parameters to arguments.
    pub(crate) fn type_from_node(
        &self,
        node: Node<'t>,
        env: &[(&str, JType)],
        depth: u32,
    ) -> JType {
        if depth > MAX_DEPTH {
            self.res.truncated.set(self.res.truncated.get() + 1);
            return JType::Unknown;
        }
        let _g = self.section(crate::timing::Section::TypeFromNode);
        match kind(node) {
            "integral_type" | "floating_point_type" | "boolean_type" => {
                Prim::from_name(self.text(node).trim())
                    .map(JType::Prim)
                    .unwrap_or(JType::Unknown)
            }
            "void_type" => JType::Void,
            "type_identifier" | "scoped_type_identifier" | "identifier" | "scoped_identifier" => {
                self.named_type(node, Vec::new(), env)
            }
            "generic_type" => {
                let mut base = None;
                let mut args = Vec::new();
                for c in named_children(node) {
                    match kind(c) {
                        "type_arguments" => {
                            for a in named_children(c) {
                                args.push(if kind(a) == "wildcard" {
                                    // `? extends X` reads as X; `?` and `? super X` are unknown.
                                    let is_extends =
                                        syntax::children(a).any(|t| kind(t) == "extends");
                                    match named_children(a)
                                        .filter(|t| {
                                            !matches!(kind(*t), "annotation" | "marker_annotation")
                                        })
                                        .last()
                                    {
                                        Some(b) if is_extends => {
                                            self.type_from_node(b, env, depth + 1)
                                        }
                                        _ => JType::Unknown,
                                    }
                                } else {
                                    self.type_from_node(a, env, depth + 1)
                                });
                            }
                        }
                        _ => base = Some(c),
                    }
                }
                match base {
                    Some(b) => self.named_type(b, args, env),
                    None => JType::Unknown,
                }
            }
            "array_type" => {
                let Some(elem) = node.field(Field::Element) else {
                    return JType::Unknown;
                };
                let dims = scope::dims_of(node.field(Field::Dimensions), self.src).max(1);
                let mut t = self.type_from_node(elem, env, depth + 1);
                for _ in 0..dims {
                    t = JType::Array(Box::new(t));
                }
                t
            }
            "annotated_type" => named_children(node)
                .find(|c| !matches!(kind(*c), "annotation" | "marker_annotation"))
                .map(|c| self.type_from_node(c, env, depth + 1))
                .unwrap_or(JType::Unknown),
            "catch_type" => {
                let mut it = named_children(node);
                match (it.next(), it.next()) {
                    (Some(one), None) => self.type_from_node(one, env, depth + 1),
                    _ => JType::Unknown,
                }
            }
            _ => JType::Unknown,
        }
    }

    fn dotted_type_text(&self, node: Node<'t>) -> String {
        match kind(node) {
            "type_identifier" | "identifier" => self.text(node).to_string(),
            _ => {
                // scoped_type_identifier / scoped_identifier: join identifier parts, skipping
                // annotations and type arguments.
                let mut parts: Vec<&'t str> = Vec::new();
                let mut stack = vec![node];
                let mut guard = 0;
                while let Some(n) = stack.pop() {
                    guard += 1;
                    if guard > 256 {
                        break;
                    }
                    match kind(n) {
                        "type_identifier" | "identifier" => parts.push(self.text(n)),
                        "scoped_type_identifier" | "scoped_identifier" | "generic_type" => {
                            let kids: Vec<Node<'t>> = named_children(n)
                                .filter(|c| {
                                    matches!(
                                        kind(*c),
                                        "type_identifier"
                                            | "identifier"
                                            | "scoped_type_identifier"
                                            | "scoped_identifier"
                                            | "generic_type"
                                    )
                                })
                                .collect();
                            stack.extend(kids.into_iter().rev());
                        }
                        _ => {}
                    }
                }
                parts.join(".")
            }
        }
    }

    /// Resolution of the (dotted) name written at a type-name node; memoized per node (it
    /// depends on nothing else). `var` resolves to nothing.
    fn named_res(&self, node: Node<'t>) -> TypeRes {
        if let Ok(map) = self.res.named.try_borrow() {
            if let Some(r) = map.get(&node.id()) {
                return r.clone();
            }
        }
        let text = self.dotted_type_text(node);
        let res = if text == "var" {
            TypeRes::NotFound
        } else if text.contains('.') {
            self.resolve_dotted(&text, node)
        } else {
            self.resolve_simple(&text, node)
        };
        if let Ok(mut map) = self.res.named.try_borrow_mut() {
            map.insert(node.id(), res.clone());
        }
        res
    }

    fn named_type(&self, node: Node<'t>, args: Vec<JType>, env: &[(&str, JType)]) -> JType {
        match self.named_res(node) {
            TypeRes::Known(JType::Class(ct)) => JType::Class(ClassType {
                name: ct.name,
                args,
            }),
            TypeRes::Known(t) => t,
            TypeRes::TypeVar {
                method_level: false,
                name,
            } => env
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, t)| t.clone())
                .unwrap_or(JType::Unknown),
            _ => JType::Unknown,
        }
    }

    // =================================================================== variables

    fn member_scope(&self, member: Node<'t>) -> Rc<MemberScope<'t>> {
        if let Ok(map) = self.res.scopes.try_borrow() {
            if let Some(s) = map.get(&member.id()) {
                return s.clone();
            }
        }
        let s = {
            let _g = self.section(crate::timing::Section::BuildScope);
            Rc::new(MemberScope::new(scope::build_scope(member, self.src)))
        };
        if let Ok(mut map) = self.res.scopes.try_borrow_mut() {
            map.insert(member.id(), s.clone());
        }
        s
    }

    fn lookup_in_member(
        &self,
        member: Node<'t>,
        name: &str,
        pos: u32,
    ) -> Option<Option<VarDecl<'t>>> {
        let scope = self.member_scope(member);
        let mut found: Option<VarDecl<'t>> = None;
        let mut ambiguous = false;
        for s in scope.named(name).filter(|s| s.start <= pos && pos < s.end) {
            match s.entry {
                Entry::Var(d) => match found {
                    None => found = Some(d),
                    Some(prev) => {
                        let same_type = match (prev.type_node, d.type_node) {
                            (Some(a), Some(b)) => self.text(a) == self.text(b),
                            _ => false,
                        };
                        if !same_type {
                            ambiguous = true;
                        }
                    }
                },
                Entry::Class => {}
            }
        }
        if ambiguous {
            return Some(None);
        }
        found.map(Some)
    }

    fn lookup_field_in_class(&self, info: &ClassInfo<'t>, name: &str, depth: u32) -> Lookup<'t> {
        if depth > 16 {
            return Lookup::Blocked;
        }
        if let Some(f) = info.field_index.get(name).and_then(|&i| info.fields.get(i)) {
            return Lookup::Found(*f);
        }
        if let Some(owner) = info.enum_owner {
            if let Some(oi) = self.class_info(owner) {
                return self.lookup_field_in_class(&oi, name, depth + 1);
            }
        }
        let mut blocked = false;
        for n in &info.super_nodes {
            let t = self.type_from_node(*n, &[], 0);
            match t.fqn() {
                Some(f) => {
                    if let Some(d) = self.local_decl(f) {
                        if d.id() == info.decl.id() {
                            continue;
                        }
                        if let Some(si) = self.class_info(d) {
                            match self.lookup_field_in_class(&si, name, depth + 1) {
                                Lookup::Found(v) => return Lookup::Found(v),
                                Lookup::Blocked => blocked = true,
                                Lookup::NotFound => {}
                            }
                        }
                    } else if jdk::class(f).is_none() {
                        blocked = true;
                    }
                }
                None => blocked = true,
            }
        }
        if blocked {
            Lookup::Blocked
        } else {
            Lookup::NotFound
        }
    }

    /// Find the variable `name` visible at `at`. Memoized when `name` is the text of `at` (an
    /// identifier), which is how it is always called.
    fn lookup_var(&self, name: &str, at: Node<'t>) -> Lookup<'t> {
        let memo = name == self.text(at);
        if memo {
            if let Ok(map) = self.res.vars.try_borrow() {
                if let Some(r) = map.get(&at.id()) {
                    return *r;
                }
            }
        }
        let r = self.compute_lookup_var(name, at);
        if memo {
            if let Ok(mut map) = self.res.vars.try_borrow_mut() {
                map.insert(at.id(), r);
            }
        }
        r
    }

    fn compute_lookup_var(&self, name: &str, at: Node<'t>) -> Lookup<'t> {
        let _g = self.section(crate::timing::Section::LookupVar);
        let pos = at.start_byte() as u32;
        // Only structural ancestors matter here: skip the rest (deeply nested code made every
        // lookup cost the whole depth).
        for (a, child) in self.structural_ancestors(at) {
            // Each ancestor is compared with the one below it (none for the parent).
            let prev = (child.id() != at.id()).then_some(child);
            let ka = k(a);
            match ka {
                K::MethodDecl
                | K::ConstructorDecl
                | K::CompactConstructorDecl
                | K::StaticInitializer
                | K::FieldDecl
                | K::ConstantDecl => {
                    if let Some(r) = self.lookup_in_member(a, name, pos) {
                        return r.map(Lookup::Found).unwrap_or(Lookup::Blocked);
                    }
                }
                K::EnumConstant if prev.is_some_and(|p| k(p) != K::ClassBody) => {
                    if let Some(r) = self.lookup_in_member(a, name, pos) {
                        return r.map(Lookup::Found).unwrap_or(Lookup::Blocked);
                    }
                }
                K::ClassBody | K::EnumBodyDecls if prev.is_some_and(|p| k(p) == K::Block) => {
                    if let Some(block) = prev {
                        if let Some(r) = self.lookup_in_member(block, name, pos) {
                            return r.map(Lookup::Found).unwrap_or(Lookup::Blocked);
                        }
                    }
                }
                _ => {}
            }
            let is_owner = (ka.is_type_decl()
                && prev.is_some_and(|p| k(p).is_type_body() || kind(p) == "formal_parameters"))
                || (matches!(ka, K::ObjectCreation | K::EnumConstant)
                    && prev.is_some_and(|p| k(p) == K::ClassBody));
            if is_owner {
                if let Some(info) = self.class_info(a) {
                    match self.lookup_field_in_class(&info, name, 0) {
                        Lookup::Found(d) => return Lookup::Found(d),
                        Lookup::Blocked => return Lookup::Blocked,
                        Lookup::NotFound => {}
                    }
                }
            }
        }
        Lookup::NotFound
    }

    fn decl_type(&self, d: &VarDecl<'t>, depth: u32) -> JType {
        let mut t = if let Some(tn) = d.type_node {
            self.type_from_node(tn, &[], depth + 1)
        } else if d.kind == VarKind::EnumConstant {
            let owner = d.decl.parent().and_then(|b| b.parent());
            owner
                .and_then(|o| self.class_type(o))
                .unwrap_or(JType::Unknown)
        } else if d.is_var {
            match (d.kind, d.value) {
                (VarKind::ForEachVariable, Some(iterable)) => {
                    let it = self.type_of_d(iterable, depth + 1);
                    self.element_type_of(&it)
                }
                (_, Some(v)) => self.type_of_d(v, depth + 1),
                _ => JType::Unknown,
            }
        } else {
            JType::Unknown
        };
        for _ in 0..d.dims {
            t = JType::Array(Box::new(t));
        }
        t
    }

    fn element_type_of(&self, iterable: &JType) -> JType {
        match iterable {
            JType::Array(e) => (**e).clone(),
            JType::Class(_) => self
                .as_supertype(iterable, "java.lang.Iterable")
                .and_then(|t| t.type_arg(0).cloned())
                .unwrap_or(JType::Unknown),
            _ => JType::Unknown,
        }
    }

    // =================================================================== expressions

    pub(crate) fn type_of_d(&self, expr: Node<'t>, depth: u32) -> JType {
        if depth > MAX_DEPTH {
            self.res.truncated.set(self.res.truncated.get() + 1);
            return JType::Unknown;
        }
        let id = expr.id();
        if let Ok(memo) = self.res.memo.try_borrow() {
            if let Some(t) = memo.get(&id) {
                return t.clone().unwrap_or(JType::Unknown);
            }
        }
        let before = self.res.truncated.get();
        let t = self.compute_type(expr, depth);
        // Only a complete computation is a property of `expr` alone.
        if self.res.truncated.get() == before {
            if let Ok(mut memo) = self.res.memo.try_borrow_mut() {
                memo.insert(id, known(t.clone()));
            }
        }
        t
    }

    fn compute_type(&self, expr: Node<'t>, depth: u32) -> JType {
        let d = depth + 1;
        match kind(expr) {
            "parenthesized_expression" => syntax::first_named_child(expr)
                .map(|e| self.type_of_d(e, d))
                .unwrap_or(JType::Unknown),
            "string_literal" => JType::string(),
            "character_literal" => JType::Prim(Prim::Char),
            "true" | "false" => JType::Prim(Prim::Boolean),
            "null_literal" => JType::Null,
            "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal" => {
                if self.text(expr).ends_with(['l', 'L']) {
                    JType::Prim(Prim::Long)
                } else {
                    JType::Prim(Prim::Int)
                }
            }
            "decimal_floating_point_literal" | "hex_floating_point_literal" => {
                if self.text(expr).ends_with(['f', 'F']) {
                    JType::Prim(Prim::Float)
                } else {
                    JType::Prim(Prim::Double)
                }
            }
            "class_literal" => {
                let inner = syntax::first_named_child(expr)
                    .map(|t| self.type_from_node(t, &[], d))
                    .unwrap_or(JType::Unknown);
                let inner = match inner {
                    JType::Prim(p) => JType::class(p.boxed_fqn()),
                    other => other,
                };
                JType::generic("java.lang.Class", vec![inner])
            }
            "this" => self.enclosing_class_type(expr).unwrap_or(JType::Unknown),
            "identifier" => self.identifier_type(expr, d),
            "object_creation_expression" => expr
                .field(Field::Type)
                .map(|t| self.type_from_node(t, &[], d))
                .unwrap_or(JType::Unknown),
            "array_creation_expression" => {
                let Some(t) = expr.field(Field::Type) else {
                    return JType::Unknown;
                };
                let mut dims = 0usize;
                for c in syntax::children(expr) {
                    match kind(c) {
                        "dimensions_expr" => dims += 1,
                        "dimensions" => dims += scope::dims_of(Some(c), self.src) as usize,
                        _ => {}
                    }
                }
                let mut ty = self.type_from_node(t, &[], d);
                for _ in 0..dims.max(1) {
                    ty = JType::Array(Box::new(ty));
                }
                ty
            }
            "array_access" => {
                let arr = expr
                    .field(Field::Array)
                    .map(|a| self.type_of_d(a, d))
                    .unwrap_or(JType::Unknown);
                match arr {
                    JType::Array(e) => *e,
                    _ => JType::Unknown,
                }
            }
            "cast_expression" => {
                // Children: type(s) then value; only single-type casts are resolved.
                let value = expr.field(Field::Value);
                let tys: Vec<Node<'t>> =
                    named_children(expr).filter(|c| Some(*c) != value).collect();
                if tys.len() == 1 {
                    self.type_from_node(tys[0], &[], d)
                } else {
                    JType::Unknown
                }
            }
            "ternary_expression" => {
                let a = expr
                    .field(Field::Consequence)
                    .map(|n| self.type_of_d(n, d))
                    .unwrap_or(JType::Unknown);
                let b = expr
                    .field(Field::Alternative)
                    .map(|n| self.type_of_d(n, d))
                    .unwrap_or(JType::Unknown);
                match (&a, &b) {
                    _ if a.is_unknown() || b.is_unknown() => JType::Unknown,
                    _ if a == b => a,
                    (JType::Null, x) | (x, JType::Null) if x.is_reference() => x.clone(),
                    (JType::Prim(x), JType::Prim(y)) if x.is_numeric() && y.is_numeric() => {
                        JType::Prim(Prim::promote(*x, *y))
                    }
                    _ => JType::Unknown,
                }
            }
            "binary_expression" => self.binary_type(expr, d),
            "unary_expression" => {
                let op = operator_str(expr);
                if op == "!" {
                    return JType::Prim(Prim::Boolean);
                }
                let t = expr
                    .field(Field::Operand)
                    .map(|o| self.type_of_d(o, d))
                    .unwrap_or(JType::Unknown);
                match t.unboxed() {
                    Some(p) if p.is_numeric() => JType::Prim(Prim::promote(p, Prim::Int)),
                    _ => JType::Unknown,
                }
            }
            "update_expression" => syntax::first_named_child(expr)
                .map(|o| self.type_of_d(o, d))
                .unwrap_or(JType::Unknown),
            "assignment_expression" => expr
                .field(Field::Left)
                .map(|l| self.type_of_d(l, d))
                .unwrap_or(JType::Unknown),
            "instanceof_expression" => JType::Prim(Prim::Boolean),
            "method_invocation" => self.call_type(expr, d),
            "field_access" => self.field_access_type(expr, d),
            _ => JType::Unknown,
        }
    }

    fn binary_type(&self, expr: Node<'t>, d: u32) -> JType {
        let op = operator_str(expr);
        match op {
            "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" => JType::Prim(Prim::Boolean),
            "+" => {
                // Walk the left spine iteratively: `a + b + c + …` can be very long.
                let mut operands: Vec<Node<'t>> = Vec::new();
                let mut cur = expr;
                let mut guard = 0;
                loop {
                    guard += 1;
                    if guard > 1_000_000 {
                        return JType::Unknown;
                    }
                    let (Some(l), Some(r)) = (cur.field(Field::Left), cur.field(Field::Right))
                    else {
                        return JType::Unknown;
                    };
                    operands.push(r);
                    let l = strip_parens(l);
                    if kind(l) == "binary_expression" && operator_str(l) == "+" {
                        cur = l;
                    } else {
                        operands.push(l);
                        break;
                    }
                }
                let mut numeric: Option<Prim> = None;
                let mut all_numeric = true;
                for o in operands.iter().rev() {
                    let t = self.type_of_d(*o, d);
                    if t.is_string() {
                        return JType::string();
                    }
                    match t.unboxed() {
                        Some(p) if p.is_numeric() => {
                            numeric = Some(
                                numeric
                                    .map_or(Prim::promote(p, Prim::Int), |n| Prim::promote(n, p)),
                            )
                        }
                        _ => all_numeric = false,
                    }
                }
                match numeric {
                    Some(p) if all_numeric => JType::Prim(p),
                    _ => JType::Unknown,
                }
            }
            "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>" | ">>>" => {
                let l = expr
                    .field(Field::Left)
                    .map(|n| self.type_of_d(n, d))
                    .unwrap_or(JType::Unknown);
                if matches!(op, "<<" | ">>" | ">>>") {
                    return match l.unboxed() {
                        Some(p) if p.is_integral() => JType::Prim(Prim::promote(p, Prim::Int)),
                        _ => JType::Unknown,
                    };
                }
                let r = expr
                    .field(Field::Right)
                    .map(|n| self.type_of_d(n, d))
                    .unwrap_or(JType::Unknown);
                match (l.unboxed(), r.unboxed()) {
                    (Some(Prim::Boolean), Some(Prim::Boolean)) if matches!(op, "&" | "|" | "^") => {
                        JType::Prim(Prim::Boolean)
                    }
                    (Some(a), Some(b)) if a.is_numeric() && b.is_numeric() => {
                        JType::Prim(Prim::promote(a, b))
                    }
                    _ => JType::Unknown,
                }
            }
            _ => JType::Unknown,
        }
    }

    fn identifier_type(&self, ident: Node<'t>, d: u32) -> JType {
        let name = self.text(ident);
        match self.lookup_var(name, ident) {
            Lookup::Found(decl) => self.decl_type(&decl, d),
            Lookup::Blocked => JType::Unknown,
            Lookup::NotFound => {
                // Statically imported constant?
                for imp in self.imports.iter().filter(|i| i.is_static) {
                    let owner = if imp.is_wildcard {
                        imp.path
                    } else if imp.simple_name() == name {
                        imp.qualifier()
                    } else {
                        continue;
                    };
                    if let Some(t) = self.static_field_type(owner, name) {
                        return t;
                    }
                }
                JType::Unknown
            }
        }
    }

    fn static_field_type(&self, owner_fqn: &str, name: &str) -> Option<JType> {
        let c = jdk::class(owner_fqn)?;
        let f = c.fields.iter().find(|f| f.name == name && f.is_static)?;
        Some(jdk::inst(
            &f.ty,
            &Env {
                names: &c.tparams,
                args: &[],
                method_tparams: &[],
            },
        ))
    }

    /// Type of `this` at `node`: the nearest named enclosing class (anonymous → unknown).
    fn enclosing_class_type(&self, node: Node<'t>) -> Option<JType> {
        let c = self.enclosing_class(node)?;
        self.class_type(c)
    }

    fn field_access_type(&self, expr: Node<'t>, d: u32) -> JType {
        let (Some(obj), Some(field)) = (expr.field(Field::Object), expr.field(Field::Field)) else {
            return JType::Unknown;
        };
        // `Outer.this`
        if kind(field) == "this" {
            return match self.type_ref(obj) {
                Some(TypeRes::Known(t)) => t,
                _ => JType::Unknown,
            };
        }
        let name = self.text(field);
        match kind(obj) {
            "this" => {
                let Some(c) = self.enclosing_class(expr) else {
                    return JType::Unknown;
                };
                let Some(info) = self.class_info(c) else {
                    return JType::Unknown;
                };
                return match self.lookup_field_in_class(&info, name, 0) {
                    Lookup::Found(v) => self.decl_type(&v, d),
                    _ => JType::Unknown,
                };
            }
            "super" => return JType::Unknown,
            "field_access" if obj.field(Field::Field).is_some_and(|f| kind(f) == "this") => {
                // Outer.this.x
                let Some(TypeRes::Known(t)) =
                    obj.field(Field::Object).and_then(|o| self.type_ref(o))
                else {
                    return JType::Unknown;
                };
                return self.member_field_type(&t, name, d, false);
            }
            _ => {}
        }
        if let Some(res) = self.type_ref(obj) {
            return match res {
                TypeRes::Known(t) => self.member_field_type(&t, name, d, true),
                _ => JType::Unknown,
            };
        }
        let ot = self.type_of_d(obj, d);
        if name == "length" && ot.is_array() {
            return JType::Prim(Prim::Int);
        }
        self.member_field_type(&ot, name, d, false)
    }

    fn member_field_type(&self, owner: &JType, name: &str, d: u32, static_only: bool) -> JType {
        let JType::Class(ct) = owner else {
            return JType::Unknown;
        };
        if let Some(decl) = self.local_decl(&ct.name) {
            let Some(info) = self.class_info(decl) else {
                return JType::Unknown;
            };
            return match self.lookup_field_in_class(&info, name, 0) {
                Lookup::Found(v) => {
                    if static_only
                        && v.kind == VarKind::Field
                        && !syntax::has_modifier(v.statement, "static")
                    {
                        // Interface constants are implicitly static.
                        if k(decl) != K::InterfaceDecl {
                            return JType::Unknown;
                        }
                    }
                    let env: Vec<(&str, JType)> = info
                        .tparams
                        .iter()
                        .enumerate()
                        .map(|(i, n)| (*n, ct.args.get(i).cloned().unwrap_or(JType::Unknown)))
                        .collect();
                    match v.type_node {
                        Some(tn) => {
                            let mut t = self.type_from_node(tn, &env, d);
                            for _ in 0..v.dims {
                                t = JType::Array(Box::new(t));
                            }
                            t
                        }
                        None => self.decl_type(&v, d),
                    }
                }
                _ => JType::Unknown,
            };
        }
        // JDK: search the class and its supertypes for the field.
        let mut queue = vec![owner.clone()];
        let mut steps = 0;
        while let Some(cur) = queue.pop() {
            steps += 1;
            if steps > 64 {
                break;
            }
            let JType::Class(cc) = &cur else { continue };
            let Some(c) = jdk::class(&cc.name) else {
                return JType::Unknown;
            };
            if let Some(f) = c
                .fields
                .iter()
                .find(|f| f.name == name && (!static_only || f.is_static))
            {
                return jdk::inst(
                    &f.ty,
                    &Env {
                        names: &c.tparams,
                        args: &cc.args,
                        method_tparams: &[],
                    },
                );
            }
            queue.extend(c.direct_supertypes(&cc.args));
        }
        JType::Unknown
    }

    // =================================================================== calls

    fn arg_types(&self, call: Node<'t>, d: u32) -> Vec<JType> {
        syntax::arguments(call)
            .map(|a| self.type_of_d(a, d))
            .collect()
    }

    fn call_type(&self, call: Node<'t>, d: u32) -> JType {
        let Some(name_node) = call.field(Field::Name) else {
            return JType::Unknown;
        };
        let name = self.text(name_node);
        let args = self.arg_types(call, d);
        match call.field(Field::Object) {
            None => self.unqualified_call_type(call, name, &args, d),
            Some(obj) if kind(obj) == "super" => JType::Unknown,
            Some(obj) => {
                if let Some(res) = self.type_ref(obj) {
                    return match res {
                        TypeRes::Known(t) => self.method_type(&t, name, &args, true, d),
                        _ => JType::Unknown,
                    };
                }
                let recv = self.type_of_d(obj, d);
                self.method_type(&recv, name, &args, false, d)
            }
        }
    }

    /// Candidates for `name` declared by a local class, with class type parameters bound by `env`.
    fn local_cands(
        &self,
        info: &ClassInfo<'t>,
        name: &str,
        env: &[(&str, JType)],
        static_only: bool,
        d: u32,
    ) -> Vec<Cand> {
        let mut out = Vec::new();
        for (m, m_name) in info.methods.iter().zip(&info.method_names) {
            if *m_name != name {
                continue;
            }
            if static_only && !syntax::has_modifier(*m, "static") {
                continue;
            }
            let mut params = Vec::new();
            let mut varargs = false;
            for p in syntax::parameters(*m) {
                if kind(p) == "spread_parameter" {
                    varargs = true;
                }
                let t = scope::param_decl(p, VarKind::Parameter, self.src)
                    .map(|pd| {
                        let mut t = pd
                            .type_node
                            .map(|tn| self.type_from_node(tn, env, d))
                            .unwrap_or(JType::Unknown);
                        for _ in 0..pd.dims {
                            t = JType::Array(Box::new(t));
                        }
                        t
                    })
                    .unwrap_or(JType::Unknown);
                params.push(t);
            }
            let ret = m
                .field(Field::Type)
                .map(|t| self.type_from_node(t, env, d))
                .unwrap_or(JType::Unknown);
            out.push(Cand {
                params,
                varargs,
                ret,
            });
        }
        if info.kind == LocalKind::Enum {
            let me = self.class_type(info.decl).unwrap_or(JType::Unknown);
            if name == "values" {
                out.push(Cand {
                    params: vec![],
                    varargs: false,
                    ret: JType::Array(Box::new(me)),
                });
            } else if name == "valueOf" {
                out.push(Cand {
                    params: vec![JType::string()],
                    varargs: false,
                    ret: me,
                });
            }
        }
        out
    }

    fn jdk_cands(
        &self,
        c: &'static jdk::JdkClass,
        args: &[JType],
        name: &str,
        static_only: bool,
    ) -> Vec<Cand> {
        let mut out = Vec::new();
        for m in c
            .methods
            .iter()
            .filter(|m| m.name == name && (!static_only || m.is_static))
        {
            let env = Env {
                names: &c.tparams,
                args,
                method_tparams: &m.tparams,
            };
            out.push(Cand {
                params: m.params.iter().map(|p| jdk::inst(p, &env)).collect(),
                varargs: m.varargs,
                ret: jdk::inst(&m.ret, &env),
            });
        }
        if c.kind == ClassKind::Enum {
            let me = JType::class(c.fqn);
            if name == "values" {
                out.push(Cand {
                    params: vec![],
                    varargs: false,
                    ret: JType::Array(Box::new(me)),
                });
            } else if name == "valueOf" {
                out.push(Cand {
                    params: vec![JType::string()],
                    varargs: false,
                    ret: me,
                });
            }
        }
        out
    }

    /// Return type of `recv.name(args)`; `static_only` for type receivers.
    fn method_type(
        &self,
        recv: &JType,
        name: &str,
        args: &[JType],
        static_only: bool,
        d: u32,
    ) -> JType {
        let _g = self.section(crate::timing::Section::MethodType);
        if let JType::Array(_) = recv {
            if name == "clone" && args.is_empty() {
                return recv.clone();
            }
            return self.method_type(&JType::class("java.lang.Object"), name, args, false, d);
        }
        if !matches!(recv, JType::Class(_)) {
            return JType::Unknown;
        }
        let mut cands: Vec<Cand> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        let mut queue: std::collections::VecDeque<JType> = std::collections::VecDeque::new();
        queue.push_back(recv.clone());
        let mut steps = 0;
        while let Some(cur) = queue.pop_front() {
            steps += 1;
            if steps > 128 {
                return JType::Unknown;
            }
            let JType::Class(ct) = &cur else { continue };
            if seen.iter().any(|s| *s == *ct.name) {
                continue;
            }
            seen.push(ct.name.to_string());
            let mut found: Vec<Cand> = if let Some(decl) = self.local_decl(&ct.name) {
                let Some(info) = self.class_info(decl) else {
                    return JType::Unknown;
                };
                let env: Vec<(&str, JType)> = info
                    .tparams
                    .iter()
                    .enumerate()
                    .map(|(i, n)| (*n, ct.args.get(i).cloned().unwrap_or(JType::Unknown)))
                    .collect();
                self.local_cands(&info, name, &env, static_only, d)
            } else if let Some(c) = jdk::class(&ct.name) {
                self.jdk_cands(c, &ct.args, name, static_only)
            } else {
                // Unknown part of the hierarchy: it cannot add candidates we know of; an
                // inapplicable overload there is harmless, an applicable one is rare enough
                // that argument checking below keeps us honest.
                Vec::new()
            };
            // Overrides in subclasses win: drop candidates whose parameters were already seen.
            found.retain(|f| !cands.iter().any(|c| c.params == f.params));
            cands.extend(found);
            queue.extend(self.direct_supertypes(&cur).0);
        }
        if cands.is_empty() {
            return JType::Unknown;
        }
        pick(self, &cands, args).unwrap_or(JType::Unknown)
    }

    fn declares_method_in_chain(&self, at: Node<'t>, name: &str) -> bool {
        let mut prev: Option<Node<'t>> = None;
        for a in self.ancestors(at) {
            let owner = (k(a).is_type_decl() && prev.is_some_and(|p| k(p).is_type_body()))
                || (matches!(k(a), K::ObjectCreation | K::EnumConstant)
                    && prev.is_some_and(|p| k(p) == K::ClassBody));
            if owner {
                if let Some(info) = self.class_info(a) {
                    if info.method_names.contains(&name) {
                        return true;
                    }
                }
            }
            prev = Some(a);
        }
        false
    }

    fn unqualified_call_type(&self, call: Node<'t>, name: &str, args: &[JType], d: u32) -> JType {
        let mut prev: Option<Node<'t>> = None;
        for a in self.ancestors(call) {
            let owner = (k(a).is_type_decl() && prev.is_some_and(|p| k(p).is_type_body()))
                || (matches!(k(a), K::ObjectCreation | K::EnumConstant)
                    && prev.is_some_and(|p| k(p) == K::ClassBody));
            prev = Some(a);
            if !owner {
                continue;
            }
            let Some(info) = self.class_info(a) else {
                return JType::Unknown;
            };
            let local = self.local_cands(&info, name, &[], false, d);
            let mut cands = local;
            let mut blocked = false;
            // Inherited methods from known supertypes.
            for n in &info.super_nodes {
                let t = self.type_from_node(*n, &[], d);
                match t.fqn() {
                    Some(f) if jdk::class(f).is_some() || self.local_decl(f).is_some() => {
                        let r = self.method_type(&t, name, args, false, d);
                        if !r.is_unknown() {
                            cands.push(Cand {
                                params: args.to_vec(),
                                varargs: false,
                                ret: r,
                            });
                        }
                    }
                    _ => blocked = true,
                }
            }
            if let Some(owner_enum) = info.enum_owner {
                if let Some(oi) = self.class_info(owner_enum) {
                    cands.extend(self.local_cands(&oi, name, &[], false, d));
                }
            }
            if !cands.is_empty() {
                return pick(self, &cands, args).unwrap_or(JType::Unknown);
            }
            if blocked || self.has_unknown_supertypes(&info) {
                return JType::Unknown;
            }
        }
        // Static imports.
        for imp in self.imports.iter().filter(|i| i.is_static) {
            let owner = if imp.is_wildcard {
                imp.path
            } else if imp.simple_name() == name {
                imp.qualifier()
            } else {
                continue;
            };
            if let Some(c) = jdk::class(owner) {
                let cands = self.jdk_cands(c, &[], name, true);
                if !cands.is_empty() {
                    return pick(self, &cands, args).unwrap_or(JType::Unknown);
                }
            }
        }
        JType::Unknown
    }
}

#[cfg(test)]
mod tests;
