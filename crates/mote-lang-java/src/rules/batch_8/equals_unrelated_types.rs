//! `java/equals-unrelated-types` — `equals(..)` between values that can never be equal.

use super::util;
use crate::prelude::*;
use crate::resolve;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["equality", "suspicious"],
    remediation_minutes: 5,
    description: r#"Detects `a.equals(b)`, `Objects.equals(a, b)` and test assertions of equality
(`assertEquals(a, b)`, `assertNotEquals(a, b)`, AssertJ `assertThat(a).isEqualTo(b)` /
`isNotEqualTo(b)`) where the two values have
unrelated types, so the comparison is always `false`: a `Long` compared with an `int`, an
enum constant compared with a `String`, a `String` compared with a `StringBuilder`, an
`Optional` compared with the value it wraps.

**Why it matters.** `equals` takes an `Object`, so the compiler accepts any argument. The value
classes of the JDK (`String`, the wrappers, `UUID`, the `java.time` types, `Optional`, …) are
only ever equal to instances of their own class, and an enum constant only to itself. When the
argument's type can never be that class, the test is constant: the `if` never runs, the
lookup never matches, and the code silently does the wrong thing. Typical slips are autoboxing
(`count.equals(0)` on a `Long` compares with an `Integer`), comparing an enum with its name,
and comparing a wrapper with the object that holds the value.

**How to fix.** Compare values of the same type: convert one side (`count.equals(0L)`,
`status == Status.ACTIVE`, `status.name().equals("ACTIVE")`, `sb.toString().equals(s)`), or
compare the field that was meant.

```java
// Noncompliant
Long id = order.getId();
if (id.equals(42)) { … }                // Integer vs Long: always false
if (status.equals("ACTIVE")) { … }      // enum vs String: always false

// Compliant
if (id.equals(42L)) { … }
if (status == Status.ACTIVE) { … }
```

Reported only when the resolver proves both types and the receiver's `equals` is known to
accept only its own class: a JDK value class, an enum, a record, or a `final` class of this
file that inherits `Object.equals`. The argument's type must be provably unrelated (no object
can be an instance of both). Receivers of other types, whose `equals` could be written to
accept anything, are never reported; `null` arguments are left to `java/equals-with-null`.
For assertions, both sides must be such strict types (test libraries differ in which side's
`equals` they call), numeric, character and boolean pairs are skipped (a primitive overload may
widen instead of boxing), and only the two-argument forms are checked; an always-failing
`assertEquals` hides what the test meant to check, and an always-passing `assertNotEquals`
checks nothing. Comparisons where neither side's type shows in its syntax (a literal, or a variable with a
declared type), such as `a.getX().equals(b.getY())`, are not checked."#,
    ..RuleMeta::new(
        "java/equals-unrelated-types",
        "java",
        "equals() between unrelated types",
    )
};

/// JDK classes whose `equals` only ever returns true for an instance of the same class (or
/// is identity). All final except the two big-number classes.
const STRICT_JDK: &[&str] = &[
    "java.lang.String",
    "java.lang.StringBuilder",
    "java.lang.StringBuffer",
    "java.lang.Integer",
    "java.lang.Long",
    "java.lang.Short",
    "java.lang.Byte",
    "java.lang.Double",
    "java.lang.Float",
    "java.lang.Boolean",
    "java.lang.Character",
    "java.util.UUID",
    "java.util.Optional",
    "java.util.OptionalInt",
    "java.util.OptionalLong",
    "java.util.OptionalDouble",
    "java.util.Locale",
    "java.net.URI",
    "java.time.LocalDate",
    "java.time.LocalTime",
    "java.time.LocalDateTime",
    "java.time.ZonedDateTime",
    "java.time.OffsetDateTime",
    "java.time.Instant",
    "java.time.Duration",
    "java.time.Period",
    "java.math.BigDecimal",
    "java.math.BigInteger",
];

/// JDK interfaces and classes that JDK value classes implement or extend but that the
/// curated table may not list on every class: never treat them as unrelated.
fn may_be_jdk_supertype(fqn: &str) -> bool {
    matches!(
        fqn,
        "java.lang.Object"
            | "java.io.Serializable"
            | "java.lang.Comparable"
            | "java.lang.CharSequence"
            | "java.lang.Number"
            | "java.lang.Cloneable"
            | "java.lang.Enum"
            | "java.lang.Record"
    ) || fqn.starts_with("java.lang.constant.")
        || fqn.starts_with("java.time.chrono.")
        || fqn.starts_with("java.time.temporal.")
}

/// What kind of `equals` the receiver's static type has.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recv {
    /// A JDK value class; `final` for all but `BigDecimal` / `BigInteger`.
    Jdk { is_final: bool },
    /// An enum (of the JDK table or of this file): identity, no subclasses outside it.
    Enum,
    /// A record or `final` class of this file with the generated / inherited `equals`.
    LocalFinal,
}

/// A declared type of this file, as the rule sees it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Local {
    Enum,
    Interface,
    /// A class (or record) and whether it is final with a class-strict `equals`.
    Class {
        strict_final: bool,
    },
}

#[derive(Default)]
pub struct EqualsUnrelatedTypes<'t> {
    calls: Vec<Candidate<'t>>,
    types: Vec<Node<'t>>,
}

/// Which comparison a candidate is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Form {
    /// `a.equals(b)`
    Equals,
    /// `Objects.equals(a, b)`
    Objects,
    /// `assertEquals(a, b)`, `assertThat(a).isEqualTo(b)`; `negated` for the `Not` forms.
    Assert { negated: bool },
}

#[derive(Clone, Copy)]
struct Candidate<'t> {
    call: Node<'t>,
    at: Node<'t>,
    recv: Node<'t>,
    arg: Node<'t>,
    form: Form,
}

/// The declared type of a variable whose class lives in another file (a project or library
/// type the resolver does not model), by its imported fully-qualified name. JDK names are
/// left out: a JDK type missing from the table could be a supertype of a value class.
fn external_type<'t>(ctx: &Ctx<'t>, expr: Node<'t>) -> Option<JType> {
    let expr = syntax::strip_parens(expr);
    if kind(expr) != "identifier" {
        return None;
    }
    let decl = ctx.declaration_of(expr)?;
    if decl.dims > 0 || kind(decl.decl) == "spread_parameter" {
        return None;
    }
    let ty = decl.type_node?;
    let base = match kind(ty) {
        "type_identifier" | "scoped_type_identifier" => ty,
        "generic_type" => syntax::first_named_child(ty)?,
        _ => return None,
    };
    let fqn = ctx.resolve_type_name(ctx.text(base), base)?;
    if fqn.starts_with("java.") || fqn.starts_with("javax.") || !fqn.contains('.') {
        return None;
    }
    Some(JType::class(fqn))
}

/// Simple names of the enums of the JDK table.
const JDK_ENUMS: &[&str] = &[
    "TimeUnit",
    "LinkOption",
    "FileVisitOption",
    "StandardOpenOption",
    "StandardCopyOption",
    "RoundingMode",
    "ChronoUnit",
];

/// The simple type name of an expression judged from its syntax and the declared types of
/// the file (primitives boxed), without the resolver; `None` when that is not enough.
fn text_type<'t>(ctx: &Ctx<'t>, e: Node<'t>) -> Option<&'t str> {
    let e = syntax::strip_parens(e);
    let declared = |name: &str| -> Option<&'t str> {
        let t = ctx.declared_types().type_of_name(name)?;
        let t = match kind(t) {
            "generic_type" => syntax::first_named_child(t)?,
            "type_identifier"
            | "scoped_type_identifier"
            | "integral_type"
            | "floating_point_type"
            | "boolean_type" => t,
            // Arrays and anything unusual: let the resolver decide.
            _ => return None,
        };
        let text = ctx.text(t);
        Some(match text {
            "int" => "Integer",
            "long" => "Long",
            "short" => "Short",
            "byte" => "Byte",
            "char" => "Character",
            "boolean" => "Boolean",
            "double" => "Double",
            "float" => "Float",
            "var" => return None,
            other => other.rsplit('.').next().unwrap_or(other),
        })
    };
    match kind(e) {
        "string_literal" => Some("String"),
        "character_literal" => Some("Character"),
        "true" | "false" => Some("Boolean"),
        "decimal_integer_literal"
        | "hex_integer_literal"
        | "octal_integer_literal"
        | "binary_integer_literal" => Some(if ctx.text(e).ends_with(['l', 'L']) {
            "Long"
        } else {
            "Integer"
        }),
        "decimal_floating_point_literal" => Some(if ctx.text(e).ends_with(['f', 'F']) {
            "Float"
        } else {
            "Double"
        }),
        "identifier" => declared(ctx.text(e)),
        "field_access" => {
            let (o, f) = (e.field(Field::Object)?, e.field(Field::Field)?);
            if kind(o) == "this" {
                declared(ctx.text(f))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn boxed(t: JType) -> JType {
    match t {
        JType::Prim(p) => JType::class(p.boxed_fqn()),
        other => other,
    }
}

impl<'t> EqualsUnrelatedTypes<'t> {
    /// The declared types of this file by fully-qualified name.
    fn local_types(&self, ctx: &Ctx<'t>) -> Vec<(String, Node<'t>)> {
        self.types
            .iter()
            .take(256)
            .filter_map(|d| ctx.class_fqn(*d).map(|f| (f, *d)))
            .collect()
    }

    fn classify_local(
        ctx: &Ctx<'t>,
        locals: &[(String, Node<'t>)],
        fqn: &str,
        depth: u32,
    ) -> Option<Local> {
        let decl = locals.iter().find(|(f, _)| f == fqn)?.1;
        Some(match kind(decl) {
            "enum_declaration" => Local::Enum,
            "interface_declaration" | "annotation_type_declaration" => Local::Interface,
            "record_declaration" => Local::Class {
                strict_final: !Self::declares_equals(ctx, decl),
            },
            "class_declaration" => {
                let is_final = syntax::has_modifier(decl, "final");
                let strict = is_final
                    && !Self::declares_equals(ctx, decl)
                    && Self::inherits_object_equals(ctx, locals, decl, depth);
                Local::Class {
                    strict_final: strict,
                }
            }
            _ => return None,
        })
    }

    fn declares_equals(ctx: &Ctx<'t>, decl: Node<'t>) -> bool {
        syntax::type_body(decl)
            .is_some_and(|b| ctx.members(b).any(|m| util::is_equals_decl(ctx, m)))
    }

    /// Whether every superclass of `decl` is a class of this file without its own `equals`.
    fn inherits_object_equals(
        ctx: &Ctx<'t>,
        locals: &[(String, Node<'t>)],
        decl: Node<'t>,
        depth: u32,
    ) -> bool {
        if depth > 8 {
            return false;
        }
        let Some(sup) = decl.field(Field::Superclass) else {
            return true;
        };
        let Some(ty) = syntax::first_named_child(sup) else {
            return false;
        };
        let Some(t) = ctx.resolve_type(ty) else {
            return false;
        };
        let Some(fqn) = t.fqn() else { return false };
        if fqn == "java.lang.Object" {
            return true;
        }
        let Some(parent) = locals.iter().find(|(f, _)| f == fqn).map(|(_, d)| *d) else {
            return false;
        };
        kind(parent) == "class_declaration"
            && !Self::declares_equals(ctx, parent)
            && Self::inherits_object_equals(ctx, locals, parent, depth + 1)
    }

    fn recv_kind(ctx: &Ctx<'t>, locals: &[(String, Node<'t>)], t: &JType) -> Option<Recv> {
        let fqn = t.fqn()?;
        if STRICT_JDK.contains(&fqn) {
            return Some(Recv::Jdk {
                is_final: resolve::is_final_class(fqn),
            });
        }
        if resolve::is_enum_class(fqn) {
            return Some(Recv::Enum);
        }
        match Self::classify_local(ctx, locals, fqn, 0)? {
            Local::Enum => Some(Recv::Enum),
            Local::Class { strict_final: true } => Some(Recv::LocalFinal),
            _ => None,
        }
    }

    /// Whether no object can be an instance of both the receiver type `r` and the argument
    /// type `a` (so the receiver's strict `equals` must return false).
    fn unrelated(
        ctx: &Ctx<'t>,
        locals: &[(String, Node<'t>)],
        r: &JType,
        rk: Recv,
        a: &JType,
    ) -> bool {
        let (Some(rf), Some(af)) = (r.fqn(), a.fqn()) else {
            return false;
        };
        if rf == af || may_be_jdk_supertype(af) {
            return false;
        }
        // A JDK type missing from the curated table could be a supertype we don't know about.
        let jdk_name = af.starts_with("java.") || af.starts_with("javax.");
        if jdk_name && !resolve::is_known_class(af) {
            return false;
        }
        let local = Self::classify_local(ctx, locals, af, 0);
        // The argument's own "no subclass outside what we can see" property.
        let arg_final = resolve::is_final_class(af)
            || resolve::is_enum_class(af)
            || matches!(
                local,
                Some(Local::Enum) | Some(Local::Class { strict_final: true })
            );
        match rk {
            Recv::Jdk { is_final: true } | Recv::Enum | Recv::LocalFinal => {
                // The receiver's class has no subclasses we cannot see: unrelated when it is
                // not a subtype of the argument's type.
                if ctx.subtype(r, af) == Some(false) {
                    return true;
                }
                // A JDK value class, an enum, or a final class is never a supertype of a
                // different final class or enum.
                arg_final && ctx.subtype(a, rf) == Some(false)
            }
            Recv::Jdk { is_final: false } => arg_final && ctx.subtype(a, rf) == Some(false),
        }
    }

    fn check_candidate(&self, ctx: &Ctx<'t>, locals: &[(String, Node<'t>)], c: Candidate<'t>) {
        let Some(r) = ctx.type_of(c.recv).map(boxed) else {
            return;
        };
        let Some(rk) = Self::recv_kind(ctx, locals, &r) else {
            return;
        };
        let assertion = matches!(c.form, Form::Assert { .. });
        let arg_type = if assertion {
            // Both sides must be known exactly: test libraries differ in which side's
            // `equals` they call.
            ctx.type_of(c.arg)
        } else {
            ctx.type_of(c.arg).or_else(|| external_type(ctx, c.arg))
        };
        let Some(a) = arg_type else {
            return;
        };
        if a.is_null() || a.is_array() || r.is_array() {
            return;
        }
        let a = boxed(a);
        if !Self::unrelated(ctx, locals, &r, rk, &a) {
            return;
        }
        if assertion {
            // Numeric, character and boolean pairs may pick a primitive overload
            // (`assertEquals(long, long)`, `isEqualTo(long)`) that widens instead of boxing.
            let primitive_like = |t: &JType| t.unboxed().is_some();
            if primitive_like(&r) && primitive_like(&a) {
                return;
            }
            let Some(ak) = Self::recv_kind(ctx, locals, &a) else {
                return;
            };
            if !Self::unrelated(ctx, locals, &a, ak, &r) {
                return;
            }
        }
        let msg = match c.form {
            Form::Objects => {
                format!("`Objects.equals` of a `{r}` and a `{a}` is false unless both are null")
            }
            Form::Equals => {
                format!("This `equals` is always false: a `{r}` is never equal to a `{a}`")
            }
            Form::Assert { negated: false } => {
                format!("This assertion always fails: a `{r}` is never equal to a `{a}`")
            }
            Form::Assert { negated: true } => {
                format!("This assertion always passes: a `{r}` is never equal to a `{a}`")
            }
        };
        ctx.emit(c.at, msg);
    }

    /// `assertEquals(x, y)` / `assertNotEquals(x, y)` of JUnit or TestNG, and AssertJ's
    /// `assertThat(x).isEqualTo(y)` / `isNotEqualTo(y)`.
    fn assertion_candidate(ctx: &Ctx<'t>, node: Node<'t>, name: &str) -> Option<Candidate<'t>> {
        let at = node.field(Field::Name)?;
        let mut args = util::args(node);
        let lib = |o: Option<Node<'t>>| {
            o.is_none_or(|o| {
                matches!(
                    ctx.text(o).rsplit('.').next().unwrap_or(""),
                    "Assert" | "Assertions" | "AssertJUnit" | "TestCase"
                )
            })
        };
        let (recv, arg, negated) = match name {
            "assertEquals" | "assertNotEquals" => {
                let (Some(x), Some(y), None) = (args.next(), args.next(), args.next()) else {
                    return None;
                };
                if !lib(node.field(Field::Object)) {
                    return None;
                }
                (x, y, name == "assertNotEquals")
            }
            _ => {
                let (Some(y), None) = (args.next(), args.next()) else {
                    return None;
                };
                let inner = node.field(Field::Object)?;
                if kind(inner) != "method_invocation"
                    || ctx.call_name(inner) != Some("assertThat")
                    || !lib(inner.field(Field::Object))
                {
                    return None;
                }
                let mut inner_args = util::args(inner);
                let (Some(x), None) = (inner_args.next(), inner_args.next()) else {
                    return None;
                };
                (x, y, name == "isNotEqualTo")
            }
        };
        if syntax::is_null_literal(recv) || syntax::is_null_literal(arg) {
            return None;
        }
        Some(Candidate {
            call: node,
            at,
            recv,
            arg,
            form: Form::Assert { negated },
        })
    }
}

impl<'t> JavaRule<'t> for EqualsUnrelatedTypes<'t> {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &[
            "method_invocation",
            "class_declaration",
            "enum_declaration",
            "record_declaration",
            "interface_declaration",
        ]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &[
            "equals",
            "assertEquals",
            "assertNotEquals",
            "isEqualTo",
            "isNotEqualTo",
        ]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) != "method_invocation" {
            self.types.push(node);
            return;
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if name != "equals" {
            if matches!(
                name,
                "assertEquals" | "assertNotEquals" | "isEqualTo" | "isNotEqualTo"
            ) {
                if let Some(c) = Self::assertion_candidate(ctx, node, name) {
                    self.calls.push(c);
                }
            }
            return;
        }
        let Some(obj) = node.field(Field::Object) else {
            return;
        };
        let Some(at) = node.field(Field::Name) else {
            return;
        };
        let mut a = util::args(node);
        let (first, second, third) = (a.next(), a.next(), a.next());
        match (first, second, third) {
            (Some(arg), None, None) => {
                if matches!(kind(obj), "this" | "super") || syntax::is_null_literal(arg) {
                    return;
                }
                self.calls.push(Candidate {
                    call: node,
                    at,
                    recv: obj,
                    arg,
                    form: Form::Equals,
                });
            }
            (Some(x), Some(y), None) => {
                if !matches!(ctx.text(obj), "Objects" | "java.util.Objects")
                    || syntax::is_null_literal(x)
                    || syntax::is_null_literal(y)
                {
                    return;
                }
                self.calls.push(Candidate {
                    call: node,
                    at,
                    recv: x,
                    arg: y,
                    form: Form::Objects,
                });
            }
            _ => {}
        }
    }

    fn finish(&mut self, ctx: &Ctx<'t>) {
        if self.calls.is_empty() {
            return;
        }
        // Computed only when a candidate survives the textual pre-filter.
        let mut locals: Option<Vec<(String, Node<'t>)>> = None;
        let local_names: Vec<&str> = self
            .types
            .iter()
            .map(|d| syntax::name_of(*d, ctx.source()))
            .collect();
        let strict_name = |t: &str| {
            STRICT_JDK.iter().any(|f| f.rsplit('.').next() == Some(t))
                || JDK_ENUMS.contains(&t)
                || local_names.contains(&t)
        };
        let calls = std::mem::take(&mut self.calls);
        let mut budget = 512u32;
        for c in calls {
            // Textual pre-filter: most comparisons are between values of one declared type,
            // or have a receiver whose declared type cannot qualify.
            let rt = text_type(ctx, c.recv);
            let at = text_type(ctx, c.arg);
            if rt.is_some_and(|r| !strict_name(r)) {
                continue;
            }
            // For assertions (very common in tests) both sides must be known from their syntax
            // and qualify as the receiver; numeric, character and boolean pairs are left to
            // the primitive overloads.
            if matches!(c.form, Form::Assert { .. }) {
                let (Some(r), Some(a)) = (rt, at) else {
                    continue;
                };
                let primitive_like = |t: &str| {
                    matches!(
                        t,
                        "Integer"
                            | "Long"
                            | "Short"
                            | "Byte"
                            | "Double"
                            | "Float"
                            | "Character"
                            | "Boolean"
                    )
                };
                if !strict_name(a) || (primitive_like(r) && primitive_like(a)) {
                    continue;
                }
            }
            // Both sides need the resolver (`a.getX().equals(b.getY())`): too costly for the
            // few bugs of that shape; one side must be known from its syntax.
            if rt.is_none() && at.is_none() {
                continue;
            }
            if let Some(a) = at {
                if rt == Some(a) || may_be_jdk_supertype(&format!("java.lang.{a}")) {
                    continue;
                }
            }
            if budget == 0 {
                break;
            }
            budget -= 1;
            if c.form == Form::Objects {
                let Some(obj) = c.call.field(Field::Object) else {
                    continue;
                };
                if !util::names_type(ctx, obj, "java.util.Objects") {
                    continue;
                }
            }
            let locals = locals.get_or_insert_with(|| self.local_types(ctx));
            self.check_candidate(ctx, locals, c);
        }
    }
}
