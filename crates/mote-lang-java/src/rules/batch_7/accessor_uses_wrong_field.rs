//! `java/accessor-uses-wrong-field` — `getWidth() { return height; }`: a getter or setter that
//! touches a different field than its name promises.

use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    status: RuleStatus::Incubating,
    tags: &["copy-paste", "suspicious"],
    remediation_minutes: 2,
    description: r#"Detects one-statement getters and setters that read or write a different field
than the one their name refers to, while that field exists in the same class with the same
type: `getWidth() { return height; }`, `setFirst(T v) { this.last = v; }`.

**Why it matters.** Accessors are written in batches by copying the previous one, and a
forgotten rename compiles fine when both fields have the same type. Callers then read the
wrong value, or their updates land in another field and the intended one keeps its old value.
Nothing fails until the data is visibly wrong.

**How to fix.** Use the field the accessor is named after.

```java
// Noncompliant
public int getHeight() { return width; }
public void setHeight(int h) { this.width = h; }

// Compliant
public int getHeight() { return height; }
public void setHeight(int h) { this.height = h; }
```

Only accessors whose body is a single `return field;` or `field = parameter;` are checked, and
only when the class declares a field with exactly the accessor's property name and the same
declared type as the field actually used, and the used field has a twin: its own accessor of
the same kind, itself a one-statement accessor that either uses the same field too (`getWidth`
next to the faulty `getHeight`, both returning `width`) or uses the field the faulty one is
named after (the two swapped their fields). Two accessors on one field, or a swapped pair, is
the copy-paste signature, while a getter deliberately exposing another field (a proxy, a
default) has no twin. An accessor whose body carries a comment is taken as deliberate, and so
is the getter of an alias property whose setter writes both fields (a misspelled or renamed
property kept for compatibility: `setColour(c) { color = c; colour = c; }` with
`getColour() { return color; }`)."#,
    ..RuleMeta::new(
        "java/accessor-uses-wrong-field",
        "java",
        "Accessor uses a different field than its name",
    )
};

/// Fields (name → declared type text) and methods (by name) of one class body, built once.
struct ClassIndex<'t> {
    fields: FxHashMap<&'t str, &'t str>,
    methods: FxHashMap<&'t str, Vec<Node<'t>>>,
}

impl<'t> ClassIndex<'t> {
    fn methods_named<'a>(&'a self, name: &str) -> impl Iterator<Item = Node<'t>> + 'a {
        self.methods.get(name).into_iter().flatten().copied()
    }
}

#[derive(Default)]
pub struct AccessorUsesWrongField<'t> {
    classes: FxHashMap<usize, ClassIndex<'t>>,
}

fn index_class<'t>(ctx: &Ctx<'t>, body: Node<'t>) -> ClassIndex<'t> {
    let mut fields = FxHashMap::default();
    let mut methods: FxHashMap<&'t str, Vec<Node<'t>>> = FxHashMap::default();
    for m in ctx.members(body) {
        match kind(m) {
            "field_declaration" => {
                let Some(ty) = m.field(Field::Type) else {
                    continue;
                };
                for d in syntax::declarators(m) {
                    if d.field(Field::Dimensions).is_none() {
                        fields.insert(syntax::name_of(d, ctx.source()), ctx.text(ty));
                    }
                }
            }
            "method_declaration" => {
                methods
                    .entry(syntax::name_of(m, ctx.source()))
                    .or_default()
                    .push(m);
            }
            _ => {}
        }
    }
    ClassIndex { fields, methods }
}

/// `Width` → `width`, `URL` → `uRL` is not produced: only the first char is lowered and names
/// whose second char is uppercase are kept as-is (`URL` stays `URL`).
fn property_name(rest: &str) -> Option<String> {
    let mut chars = rest.chars();
    let first = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    if chars.next().is_some_and(|c| c.is_ascii_uppercase()) {
        return Some(rest.to_string());
    }
    let mut s = String::with_capacity(rest.len());
    s.push(first.to_ascii_lowercase());
    s.push_str(&rest[1..]);
    Some(s)
}

/// `(getX, isX, setX)` for field `x`.
fn own_accessor_names(field: &str) -> (String, String, String) {
    let mut cap = String::with_capacity(field.len());
    let mut chars = field.chars();
    if let Some(c) = chars.next() {
        cap.push(c.to_ascii_uppercase());
        cap.push_str(chars.as_str());
    }
    (format!("get{cap}"), format!("is{cap}"), format!("set{cap}"))
}

/// The single statement of a block, if it has exactly one.
fn only_statement(block: Node<'_>) -> Option<Node<'_>> {
    let mut it = named_children(block);
    let s = it.next()?;
    it.next().is_none().then_some(s)
}

/// The field a target or value names: `x` or `this.x`.
fn field_ref<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<&'t str> {
    let expr = syntax::strip_parens(expr);
    match kind(expr) {
        "identifier" => Some(ctx.text(expr)),
        "field_access" => {
            let (o, f) = (expr.field(Field::Object)?, expr.field(Field::Field)?);
            (kind(o) == "this").then(|| ctx.text(f))
        }
        _ => None,
    }
}

/// The field a one-statement accessor uses: `return x;` for a getter, `this.x = param;` for a
/// setter (exactly one parameter, none for a getter).
fn accessed_field<'t>(ctx: &Ctx<'t>, method: Node<'t>, is_setter: bool) -> Option<&'t str> {
    let body = method.field(Field::Body)?;
    let stmt = only_statement(body)?;
    let mut params = syntax::parameters(method);
    let param = params.next();
    if params.next().is_some() || param.is_some() != is_setter {
        return None;
    }
    if !is_setter {
        if kind(stmt) != "return_statement" {
            return None;
        }
        return field_ref(ctx, syntax::first_named_child(stmt)?);
    }
    if kind(stmt) != "expression_statement" {
        return None;
    }
    let assign = syntax::first_named_child(stmt)?;
    if kind(assign) != "assignment_expression" || syntax::operator_str(assign) != "=" {
        return None;
    }
    let (left, right, p) = (
        assign.field(Field::Left)?,
        assign.field(Field::Right)?,
        param?,
    );
    let pname = syntax::name_of(p, ctx.source());
    let right = syntax::strip_parens(right);
    if kind(right) != "identifier" || ctx.text(right) != pname {
        return None;
    }
    let target = field_ref(ctx, left)?;
    // `x = x` without `this.` assigns the parameter: another rule's business.
    if target == pname && kind(syntax::strip_parens(left)) == "identifier" {
        return None;
    }
    Some(target)
}

/// The fields a method assigns with top-level `x = ..;` / `this.x = ..;` statements.
fn assigned_fields<'t>(ctx: &Ctx<'t>, method: Node<'t>) -> Vec<&'t str> {
    let Some(body) = method.field(Field::Body) else {
        return Vec::new();
    };
    named_children(body)
        .filter(|s| kind(*s) == "expression_statement")
        .filter_map(syntax::first_named_child)
        .filter(|a| kind(*a) == "assignment_expression" && syntax::operator_str(*a) == "=")
        .filter_map(|a| a.field(Field::Left).and_then(|l| field_ref(ctx, l)))
        .collect()
}

impl<'t> JavaRule<'t> for AccessorUsesWrongField<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(name_node) = ctx.name_node(node) else {
            return;
        };
        let name = ctx.text(name_node);
        let (is_setter, rest) = if let Some(r) = name.strip_prefix("get") {
            (false, r)
        } else if let Some(r) = name.strip_prefix("is") {
            (false, r)
        } else if let Some(r) = name.strip_prefix("set") {
            (true, r)
        } else {
            return;
        };
        if !rest.starts_with(|c: char| c.is_ascii_uppercase()) {
            return;
        }
        let Some(body) = node.field(Field::Body) else {
            return;
        };
        // Text fast path: accessors are short, and nearly all use their own field.
        let text = ctx.text(body);
        if text.len() > 240 {
            return;
        }
        let inner = text.trim_start_matches('{').trim_end_matches('}').trim();
        let target = if is_setter {
            inner.split_once('=').map(|(l, _)| l)
        } else {
            inner.strip_prefix("return")
        };
        if let Some(t) = target {
            let t = t.trim().trim_end_matches(';').trim();
            if t.strip_prefix("this.")
                .unwrap_or(t)
                .eq_ignore_ascii_case(rest)
            {
                return;
            }
        }
        // One statement (plus possibly a comment): cheap count before iterating.
        if !(1..=2).contains(&body.named_child_count()) {
            return;
        }
        let Some(used) = accessed_field(ctx, node, is_setter) else {
            return;
        };
        // The usual case (the accessor uses its own field) without allocating.
        if used.eq_ignore_ascii_case(rest) {
            return;
        }
        let Some(prop) = property_name(rest) else {
            return;
        };
        if used == prop {
            return;
        }
        let Some(class_body) = ctx.parent(node) else {
            return;
        };
        if kind(class_body) != "class_body" {
            return;
        }
        let index = self
            .classes
            .entry(class_body.id())
            .or_insert_with(|| index_class(ctx, class_body));
        let (Some(expected_ty), Some(used_ty)) =
            (index.fields.get(prop.as_str()), index.fields.get(used))
        else {
            return;
        };
        if expected_ty != used_ty || syntax::children(body).any(syntax::is_comment) {
            return;
        }
        // A deprecated alias property: its setter writes both fields, so the getter returning
        // the real field is deliberate (`isColour() { return color; }` next to a
        // `setColour(c) { color = c; colour = c; }`).
        if !is_setter {
            let setter = format!("set{rest}");
            let writes_both = index.methods_named(&setter).any(|s| {
                let assigned = assigned_fields(ctx, s);
                assigned.contains(&used) && assigned.contains(&prop.as_str())
            });
            if writes_both {
                return;
            }
        }
        // A deliberate mapping (a getter exposing a proxy, a setter feeding a default) is common;
        // a copy-paste slip leaves two accessors on the same field, or two accessors that swapped
        // their fields. Require such a twin, recognized by what its body does.
        let own = own_accessor_names(used);
        let twin_names: &[&str] = if is_setter {
            &[own.2.as_str()]
        } else {
            &[own.0.as_str(), own.1.as_str()]
        };
        let twin = twin_names.iter().find_map(|t| {
            index
                .methods_named(t)
                .find_map(|m| accessed_field(ctx, m, is_setter).map(|f| (*t, f)))
        });
        let Some((twin, twin_field)) = twin else {
            return;
        };
        let verb = if is_setter { "writes" } else { "returns" };
        let message = if twin_field == used {
            format!("Check this accessor: `{name}` {verb} field `{used}` like `{twin}` does, while field `{prop}` exists with the same type")
        } else if twin_field == prop {
            format!("Check this accessor: `{name}` {verb} field `{used}` while `{twin}` {verb} field `{prop}`; the two look swapped")
        } else {
            return;
        };
        ctx.emit(name_node, message);
    }
}
