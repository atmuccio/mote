//! `java/confusing-member-name` — a method named after its class, and members whose names
//! differ only in letter case.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["naming", "confusing"],
    remediation_minutes: 5,
    description: r#"Detects member names that are easy to misread:

* a method with the same name as its class (`public void Invoice() { … }`): it looks like a
  constructor but is an ordinary method — usually a constructor that got a return type by
  mistake, so it never runs when an object is created;
* two methods of one class whose names differ only in letter case (`getUrl` and `getURL`);
* two fields of one class whose names differ only in letter case (`count` and `Count`).

**Why it matters.** Readers (and code completion) treat such names as the same word. Calling
or assigning the wrong one compiles and fails silently, and a "constructor" with a return type
leaves every field it was meant to initialize at its default value.

**How to fix.** Remove the return type if a constructor was meant; otherwise rename one of the
members so the names differ in more than case.

```java
// Noncompliant
class Invoice {
    private String Number;
    private String number;
    public void Invoice() { number = "0"; }
}

// Compliant
class Invoice {
    private String externalNumber;
    private String number;
    public Invoice() { number = "0"; }
}
```

Not reported: a `static` method named like its class (it can't be a mistyped constructor, e.g.
the `main` entry point of a class named `main`), a field and a method sharing a name (a
record-style accessor), overloads, a
constant (`static final`, or a `static` field named in capitals) paired with a field (`NAME`
and `name` is a common convention for the name of a property), pairs where one method is
`@Deprecated` (a renamed method kept for compatibility) or `@Override` (its name is imposed by
a supertype), and test code (tests of case-sensitive property handling need such names)."#,
    ..RuleMeta::new(
        "java/confusing-member-name",
        "java",
        "Confusing member name",
    )
};

pub struct ConfusingMemberName;

impl<'t> JavaRule<'t> for ConfusingMemberName {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if ctx.is_test() {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        let src = ctx.source();
        let class_name = syntax::name_of(node, src);
        // (declaration, name node, name) of the methods and fields.
        let mut methods: Vec<(Node<'t>, Node<'t>, &'t str)> = Vec::new();
        let mut fields: Vec<(Node<'t>, Node<'t>, &'t str)> = Vec::new();
        for m in ctx.members(body) {
            match kind(m) {
                "method_declaration" => {
                    let Some(n) = m.field(Field::Name) else {
                        continue;
                    };
                    let name = ctx.text(n);
                    // A static method cannot be a constructor that got a return type by
                    // mistake (`public static void main` of a class named `main`).
                    if name == class_name && !syntax::has_modifier(m, "static") {
                        ctx.emit(
                            n,
                            format!("Rename this method or remove its return type: a method named `{name}` looks like a constructor but is not one"),
                        );
                    }
                    methods.push((m, n, name));
                }
                "field_declaration" | "constant_declaration" => {
                    for d in syntax::declarators(m) {
                        if let Some(n) = d.field(Field::Name) {
                            fields.push((m, n, ctx.text(n)));
                        }
                    }
                }
                _ => {}
            }
        }
        report_case_pairs(ctx, &methods, "method", excluded_method);
        report_case_pairs(ctx, &fields, "field", constant_field);
    }
}

/// A deprecated alias, or a name imposed by a supertype, is not a choice.
fn excluded_method<'t>(ctx: &Ctx<'t>, decl: Node<'t>, _name: &str) -> bool {
    let src = ctx.source();
    syntax::has_annotation(decl, src, "Deprecated") || syntax::has_annotation(decl, src, "Override")
}

/// Constants, and static fields named like one (`FACTORY` beside `factory`), follow the
/// NAME / name convention.
fn constant_field<'t>(_ctx: &Ctx<'t>, decl: Node<'t>, name: &str) -> bool {
    let is_static = kind(decl) == "constant_declaration" || syntax::has_modifier(decl, "static");
    let is_final = kind(decl) == "constant_declaration" || syntax::has_modifier(decl, "final");
    is_static && (is_final || name.chars().all(|c| !c.is_lowercase()))
}

/// The members of one exact spelling within a group of names equal but for case.
struct Spelling<'t> {
    name: &'t str,
    /// Member indices, in order.
    members: Vec<usize>,
    /// The first member not excluded from pairs, once asked (`Some(None)`: all excluded).
    first_kept: Option<Option<usize>>,
}

/// Hash of a name with ASCII letters folded to lower case.
fn folded_hash(name: &str) -> u64 {
    use std::hash::Hasher;
    let mut h = rustc_hash::FxHasher::default();
    for b in name.bytes() {
        h.write_u8(b.to_ascii_lowercase());
    }
    h.finish()
}

/// Reports, once per name, a member whose name equals an earlier one's except for case.
/// `excluded` keeps a member out of any pair (deprecated method, constant field); it is only
/// evaluated for members whose folded names collide.
fn report_case_pairs<'t>(
    ctx: &Ctx<'t>,
    members: &[(Node<'t>, Node<'t>, &'t str)],
    what: &str,
    excluded: fn(&Ctx<'t>, Node<'t>, &str) -> bool,
) {
    if members.len() < 2 {
        return;
    }
    // Cheap pass: most classes have no two names that fold to the same hash.
    let mut seen: FxHashSet<u64> = FxHashSet::default();
    let mut candidates = false;
    for &(_, _, name) in members {
        if !seen.insert(folded_hash(name)) {
            candidates = true;
        }
    }
    if !candidates {
        return;
    }
    // The members by name folded to lower case, then by exact spelling (in order of first
    // appearance), with their indices in order: comparing every member with every earlier one
    // was quadratic (a class body of 260 000 members took minutes).
    let mut group_of_name: FxHashMap<String, usize> = FxHashMap::default();
    let mut groups: Vec<Vec<Spelling<'t>>> = Vec::new();
    let mut group_of: Vec<usize> = Vec::with_capacity(members.len());
    for (i, &(_, _, name)) in members.iter().enumerate() {
        let g = *group_of_name
            .entry(name.to_ascii_lowercase())
            .or_insert_with(|| {
                groups.push(Vec::new());
                groups.len() - 1
            });
        let spellings = &mut groups[g];
        match spellings.iter_mut().find(|s| s.name == name) {
            Some(s) => s.members.push(i),
            None => spellings.push(Spelling {
                name,
                members: vec![i],
                first_kept: None,
            }),
        }
        group_of.push(g);
    }
    let mut reported: FxHashSet<&str> = FxHashSet::default();
    for (i, &(decl, node, name)) in members.iter().enumerate() {
        if reported.contains(name) {
            continue;
        }
        let spellings = &mut groups[group_of[i]];
        // Members of another spelling (same name but for case) before this one?
        let earlier = spellings
            .iter()
            .any(|s| s.name != name && s.members.first().is_some_and(|&j| j < i));
        if !earlier || excluded(ctx, decl, name) {
            continue;
        }
        // The first of them that is not excluded: the first kept member of each other spelling,
        // the earliest of those.
        let mut first: Option<usize> = None;
        for s in spellings.iter_mut().filter(|s| s.name != name) {
            let kept = *s.first_kept.get_or_insert_with(|| {
                s.members
                    .iter()
                    .copied()
                    .find(|&j| !excluded(ctx, members[j].0, s.name))
            });
            if let Some(j) = kept.filter(|&j| j < i) {
                first = Some(first.map_or(j, |f| f.min(j)));
            }
        }
        if let Some(&(_, first, other)) = first.map(|j| &members[j]) {
            reported.insert(name);
            ctx.emit(
                node,
                format!("Rename this {what}: `{name}` differs from `{other}` only in letter case"),
            )
            .secondary(first, format!("`{other}` declared here"));
        }
    }
}
