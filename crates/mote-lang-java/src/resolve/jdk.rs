//! A curated, hand-written table of commonly used JDK classes: supertypes (with type
//! arguments), and the return types of their common methods and fields.
//!
//! The data lives in `jdk.txt` in a small line-based format (documented at its top) and is
//! parsed once per process. Anything missing from the table is simply unknown to the resolver,
//! which keeps rules silent.

use super::types::{JType, Prim};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::sync::LazyLock;

const DATA: &str = include_str!("jdk.txt");

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TExpr {
    Prim(Prim),
    Void,
    Class(&'static str, Vec<TExpr>),
    Var(&'static str),
    Array(Box<TExpr>),
    /// `? extends X` — read as `X`.
    Extends(Box<TExpr>),
    /// `?` / `? super X` — unknown.
    Wild,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClassKind {
    Class,
    Interface,
    Enum,
    Annotation,
}

#[derive(Debug)]
pub(crate) struct JdkMethod {
    pub name: &'static str,
    pub is_static: bool,
    pub tparams: Vec<&'static str>,
    pub params: Vec<TExpr>,
    pub varargs: bool,
    pub ret: TExpr,
}

#[derive(Debug)]
pub(crate) struct JdkField {
    pub name: &'static str,
    pub is_static: bool,
    pub ty: TExpr,
}

#[derive(Debug)]
pub(crate) struct JdkClass {
    pub fqn: &'static str,
    pub kind: ClassKind,
    pub is_final: bool,
    pub tparams: Vec<&'static str>,
    pub supers: Vec<TExpr>,
    pub methods: Vec<JdkMethod>,
    pub fields: Vec<JdkField>,
}

pub(crate) struct JdkTable {
    pub classes: FxHashMap<&'static str, JdkClass>,
    /// Problems found while parsing the table (asserted empty by tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub errors: Vec<String>,
}

pub(crate) static JDK: LazyLock<JdkTable> = LazyLock::new(|| parse_table(DATA));

/// Look up a class by fully-qualified name.
pub(crate) fn class(fqn: &str) -> Option<&'static JdkClass> {
    JDK.classes.get(fqn)
}

/// Whether `fqn` is in the curated JDK table.
pub fn is_known_class(fqn: &str) -> bool {
    JDK.classes.contains_key(fqn)
}

/// Whether `fqn` is an enum of the curated JDK table (`TimeUnit`, `RoundingMode`, …).
pub fn is_enum_class(fqn: &str) -> bool {
    class(fqn).is_some_and(|c| c.kind == ClassKind::Enum)
}

/// Whether `fqn` is a `final` class of the curated JDK table (`String`, wrappers, …).
pub fn is_final_class(fqn: &str) -> bool {
    class(fqn).is_some_and(|c| c.is_final)
}

// --------------------------------------------------------------------------------- parsing

/// Simple / nested names (`Entry`, `Map.Entry`) → FQN, from class headers.
fn simple_names(data: &'static str) -> FxHashMap<&'static str, &'static str> {
    let mut map = FxHashMap::default();
    for line in data.lines() {
        if line.starts_with([' ', '\t', '#']) || line.trim().is_empty() {
            continue;
        }
        let Some(fqn) = header_fqn(line) else {
            continue;
        };
        // From the first capitalized segment on: `Map.Entry`; and the last segment: `Entry`.
        let mut idx = 0;
        for seg in fqn.split('.') {
            if seg.chars().next().is_some_and(char::is_uppercase) {
                break;
            }
            idx += seg.len() + 1;
        }
        if let Some(nested) = fqn.get(idx..) {
            map.entry(nested).or_insert(fqn);
        }
        let last = fqn.rsplit('.').next().unwrap_or(fqn);
        map.entry(last).or_insert(fqn);
    }
    map
}

fn header_fqn(line: &'static str) -> Option<&'static str> {
    let head = line.split(':').next()?;
    let mut words = head.split_whitespace();
    let name = loop {
        let w = words.next()?;
        if !matches!(
            w,
            "final" | "abstract" | "class" | "interface" | "enum" | "annotation"
        ) {
            break w;
        }
    };
    Some(name.split('<').next().unwrap_or(name))
}

struct Parser<'m> {
    names: &'m FxHashMap<&'static str, &'static str>,
    errors: Vec<String>,
}

/// Split at top-level commas (outside `<>`).
fn split_top(s: &'static str) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        out.push(last);
    }
    out
}

impl Parser<'_> {
    fn ty(&mut self, s: &'static str, tvars: &[&'static str], ctx: &str) -> TExpr {
        let s = s.trim();
        if let Some(inner) = s.strip_suffix("[]") {
            return TExpr::Array(Box::new(self.ty(inner, tvars, ctx)));
        }
        if s == "?" {
            return TExpr::Wild;
        }
        if let Some(rest) = s.strip_prefix("? extends ") {
            return TExpr::Extends(Box::new(self.ty(rest, tvars, ctx)));
        }
        if s.starts_with("? super ") {
            return TExpr::Wild;
        }
        if s == "void" {
            return TExpr::Void;
        }
        if let Some(p) = Prim::from_name(s) {
            return TExpr::Prim(p);
        }
        let (base, args) = match s.find('<') {
            Some(i) if s.ends_with('>') => (&s[..i], Some(&s[i + 1..s.len() - 1])),
            _ => (s, None),
        };
        if args.is_none() && tvars.contains(&base) {
            return TExpr::Var(base);
        }
        let fqn = if base.contains('.') && base.starts_with(|c: char| c.is_lowercase()) {
            base
        } else if let Some(f) = self.names.get(base) {
            f
        } else {
            self.errors.push(format!("{ctx}: unknown type `{base}`"));
            "java.lang.Object"
        };
        let args = args
            .map(|a| {
                split_top(a)
                    .into_iter()
                    .map(|t| self.ty(t, tvars, ctx))
                    .collect()
            })
            .unwrap_or_default();
        TExpr::Class(fqn, args)
    }

    fn tparams(s: &'static str) -> Vec<&'static str> {
        split_top(s)
            .into_iter()
            .filter_map(|p| p.split_whitespace().next())
            .collect()
    }

    fn member(&mut self, class: &mut JdkClass, m: &'static str) {
        let ctx = format!("{}: `{m}`", class.fqn);
        let mut s = m.trim();
        let mut is_static = false;
        if let Some(rest) = s.strip_prefix("static ") {
            is_static = true;
            s = rest.trim();
        }
        let mut tparams = Vec::new();
        if s.starts_with('<') {
            let Some(end) = s.find('>') else {
                self.errors
                    .push(format!("{ctx}: unterminated type parameters"));
                return;
            };
            tparams = Self::tparams(&s[1..end]);
            s = s[end + 1..].trim();
        }
        let mut tvars: Vec<&'static str> = class.tparams.clone();
        tvars.extend(tparams.iter().copied());
        if let Some(open) = s.find('(') {
            let Some(close) = s.find(')') else {
                self.errors.push(format!("{ctx}: missing `)`"));
                return;
            };
            let name = s[..open].trim();
            let params_src = &s[open + 1..close];
            let ret_src = s[close + 1..].trim();
            if name.is_empty() || ret_src.is_empty() {
                self.errors
                    .push(format!("{ctx}: expected `name(params) Return`"));
                return;
            }
            let mut varargs = false;
            let mut params = Vec::new();
            for p in split_top(params_src) {
                if let Some(base) = p.strip_suffix("...") {
                    varargs = true;
                    params.push(TExpr::Array(Box::new(self.ty(base, &tvars, &ctx))));
                } else {
                    params.push(self.ty(p, &tvars, &ctx));
                }
            }
            let ret = self.ty(ret_src, &tvars, &ctx);
            class.methods.push(JdkMethod {
                name,
                is_static,
                tparams,
                params,
                varargs,
                ret,
            });
        } else if let Some((name, ty)) = s.split_once(':') {
            let ty = self.ty(ty, &tvars, &ctx);
            class.fields.push(JdkField {
                name: name.trim(),
                is_static,
                ty,
            });
        } else {
            self.errors.push(format!("{ctx}: not a method or field"));
        }
    }
}

fn parse_table(data: &'static str) -> JdkTable {
    let names = simple_names(data);
    let mut p = Parser {
        names: &names,
        errors: Vec::new(),
    };
    let mut classes: FxHashMap<&'static str, JdkClass> = FxHashMap::default();
    let mut current: Option<JdkClass> = None;
    for (no, line) in data.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with([' ', '\t']) {
            let Some(class) = current.as_mut() else {
                p.errors
                    .push(format!("line {}: member outside a class", no + 1));
                continue;
            };
            for m in trimmed.split(';').map(str::trim).filter(|m| !m.is_empty()) {
                p.member(class, m);
            }
            continue;
        }
        if let Some(c) = current.take() {
            if classes.insert(c.fqn, c).is_some() {
                p.errors.push(format!("line {}: duplicate class", no + 1));
            }
        }
        let (head, supers_src) = match line.split_once(':') {
            Some((h, s)) => (h, Some(s)),
            None => (line, None),
        };
        let mut kind = ClassKind::Class;
        let mut is_final = false;
        let mut name = "";
        for w in head.split_whitespace() {
            match w {
                "final" => is_final = true,
                "abstract" => {}
                "class" => kind = ClassKind::Class,
                "interface" => kind = ClassKind::Interface,
                "enum" => kind = ClassKind::Enum,
                "annotation" => kind = ClassKind::Annotation,
                _ => {
                    // The name may contain `<A, B>` with spaces: take the rest of the head.
                    let start = head.find(w).unwrap_or(0);
                    name = head[start..].trim();
                    break;
                }
            }
        }
        let (fqn, tparams) = match name.find('<') {
            Some(i) if name.ends_with('>') => {
                (&name[..i], Parser::tparams(&name[i + 1..name.len() - 1]))
            }
            _ => (name, Vec::new()),
        };
        if fqn.is_empty() {
            p.errors.push(format!("line {}: bad class header", no + 1));
            continue;
        }
        let ctx = format!("{fqn} header");
        let supers = supers_src
            .map(|s| {
                split_top(s)
                    .into_iter()
                    .map(|t| p.ty(t, &tparams, &ctx))
                    .collect()
            })
            .unwrap_or_default();
        current = Some(JdkClass {
            fqn,
            kind,
            is_final,
            tparams,
            supers,
            methods: Vec::new(),
            fields: Vec::new(),
        });
    }
    if let Some(c) = current.take() {
        classes.insert(c.fqn, c);
    }
    JdkTable {
        classes,
        errors: p.errors,
    }
}

// --------------------------------------------------------------------------------- instantiation

/// Substitution environment: class type parameters → arguments. Method type parameters and
/// unbound variables become `Unknown`.
pub(crate) struct Env<'a> {
    pub names: &'a [&'static str],
    pub args: &'a [JType],
    pub method_tparams: &'a [&'static str],
}

pub(crate) fn inst(t: &TExpr, env: &Env<'_>) -> JType {
    match t {
        TExpr::Prim(p) => JType::Prim(*p),
        TExpr::Void => JType::Void,
        TExpr::Wild => JType::Unknown,
        TExpr::Extends(x) => inst(x, env),
        TExpr::Array(e) => JType::Array(Box::new(inst(e, env))),
        TExpr::Var(v) => {
            if env.method_tparams.contains(v) {
                return JType::Unknown;
            }
            env.names
                .iter()
                .position(|n| n == v)
                .and_then(|i| env.args.get(i))
                .cloned()
                .unwrap_or(JType::Unknown)
        }
        TExpr::Class(fqn, args) => JType::generic(
            Cow::Borrowed(*fqn),
            args.iter().map(|a| inst(a, env)).collect(),
        ),
    }
}

impl JdkClass {
    /// Direct supertypes of `self<args>`; interfaces and root classes get `Object`.
    pub(crate) fn direct_supertypes(&self, args: &[JType]) -> Vec<JType> {
        let env = Env {
            names: &self.tparams,
            args,
            method_tparams: &[],
        };
        let mut out: Vec<JType> = self.supers.iter().map(|s| inst(s, &env)).collect();
        if self.fqn != "java.lang.Object" && !out.iter().any(|t| t.is_class("java.lang.Object")) {
            let has_class_super = out.iter().any(|t| {
                t.fqn()
                    .and_then(class)
                    .is_some_and(|c| c.kind == ClassKind::Class || c.kind == ClassKind::Enum)
            });
            if !has_class_super {
                out.push(JType::class("java.lang.Object"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_parses_cleanly() {
        let t = &*JDK;
        assert!(
            t.errors.is_empty(),
            "jdk.txt errors:\n{}",
            t.errors.join("\n")
        );
        assert!(t.classes.len() >= 100, "only {} classes", t.classes.len());
        let list = class("java.util.List").unwrap();
        assert_eq!(list.tparams, vec!["E"]);
        assert!(list.methods.iter().any(|m| m.name == "get"));
        let entry = class("java.util.Map.Entry").unwrap();
        assert_eq!(entry.kind, ClassKind::Interface);
    }

    #[test]
    fn every_referenced_class_exists() {
        fn walk(t: &TExpr, missing: &mut Vec<String>) {
            match t {
                TExpr::Class(f, args) => {
                    if class(f).is_none() {
                        missing.push(f.to_string());
                    }
                    args.iter().for_each(|a| walk(a, missing));
                }
                TExpr::Array(e) | TExpr::Extends(e) => walk(e, missing),
                _ => {}
            }
        }
        let mut missing = Vec::new();
        for c in JDK.classes.values() {
            c.supers.iter().for_each(|s| walk(s, &mut missing));
            for m in &c.methods {
                m.params.iter().for_each(|p| walk(p, &mut missing));
                walk(&m.ret, &mut missing);
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "referenced but not declared: {missing:?}"
        );
    }
}
