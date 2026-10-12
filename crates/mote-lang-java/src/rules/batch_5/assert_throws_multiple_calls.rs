//! `java/assert-throws-multiple-calls` — `assertThrows(X.class, () -> { setup(); act(); })`.

use super::util::{self, Walk, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::CodeSmell,
    quality: Quality::Maintainability,
    severity: Severity::Low,
    tags: &["tests", "junit"],
    remediation_minutes: 5,
    description: r#"Detects `assertThrows`, `assertThrowsExactly`, and `assertDoesNotThrow` calls
whose lambda does more than the one call under test: it holds several statements, or it
passes the result of another call as an argument (`() -> parser.parse(load(file))`).

**Why it matters.** The assertion passes as soon as *any* code in the lambda throws the
expected exception. When setup code shares the lambda with the call under test, a failure in
the setup makes the test pass for the wrong reason, and readers can't tell which call is
expected to fail.

**How to fix.** Prepare everything before the assertion and keep a single call inside the
lambda.

```java
// Noncompliant
assertThrows(ParseException.class, () -> {
    Parser parser = new Parser(config);
    parser.parse("{");
});
assertThrows(ParseException.class, () -> parser.parse(readFixture("broken.json")));

// Compliant
Parser parser = new Parser(config);
assertThrows(ParseException.class, () -> parser.parse("{"));
String broken = readFixture("broken.json");
assertThrows(ParseException.class, () -> parser.parse(broken));
```

Only test files are checked. Arguments that cannot reasonably throw are ignored: calls
without arguments (getters), calls on JDK types and values (`List.of`, `Arrays.asList`,
`text.getBytes(UTF_8)`, also statically imported: `asList(..)`), `new` of JDK classes (`java.*`
and Swing values such as `new SortKey(0, ASCENDING)`), constructors without arguments, and the
arguments of assertions (`assertNotNull(new Parser(input))`). JDK calls that declare checked
exceptions do count, since they can fail: file and reflection APIs such as
`File.createTempFile(..)`, `Files.readString(..)`, `Class.forName(..)`, `Thread.sleep(..)`, and
`new FileInputStream(..)`, `new URL(..)`, `new URI(..)`."#,
    ..RuleMeta::new(
        "java/assert-throws-multiple-calls",
        "java",
        "Several calls inside an assertThrows lambda",
    )
};

const NAMES: &[&str] = &["assertThrows", "assertThrowsExactly", "assertDoesNotThrow"];

pub struct AssertThrowsMultipleCalls;

/// JDK methods (with arguments) that declare checked exceptions, by owner.
const THROWING_JDK_METHODS: &[(&str, &[&str])] = &[
    ("java.io.File", &["createTempFile"]),
    (
        "java.lang.Class",
        &[
            "forName",
            "getMethod",
            "getDeclaredMethod",
            "getField",
            "getDeclaredField",
            "getConstructor",
            "getDeclaredConstructor",
        ],
    ),
    ("java.lang.Thread", &["sleep", "join"]),
    ("java.lang.reflect.Method", &["invoke"]),
    (
        "java.util.Properties",
        &["load", "store", "loadFromXML", "storeToXML"],
    ),
];

/// `java.nio.file.Files` methods that declare no checked exception; every other one does.
const FILES_WITHOUT_CHECKED: &[&str] = &[
    "exists",
    "notExists",
    "isDirectory",
    "isRegularFile",
    "isReadable",
    "isWritable",
    "isExecutable",
    "isSymbolicLink",
];

/// JDK classes whose constructors (with arguments) declare checked exceptions.
const THROWING_JDK_CONSTRUCTORS: &[&str] = &[
    "java.io.FileInputStream",
    "java.io.FileOutputStream",
    "java.io.FileReader",
    "java.io.FileWriter",
    "java.io.RandomAccessFile",
    "java.io.ObjectInputStream",
    "java.io.ObjectOutputStream",
    "java.net.URL",
    "java.net.URI",
    "java.net.Socket",
    "java.net.ServerSocket",
    "java.util.zip.ZipFile",
    "java.util.jar.JarFile",
];

fn jdk_method_throws(owner: &str, method: &str) -> bool {
    if owner == "java.nio.file.Files" {
        return !FILES_WITHOUT_CHECKED.contains(&method);
    }
    THROWING_JDK_METHODS
        .iter()
        .any(|(o, ms)| *o == owner && ms.contains(&method))
}

/// A JDK class whose factories and constructors are taken as harmless (apart from the tables
/// above): `java.*`, and the Swing models and values (`javax.swing.RowSorter.SortKey`). Other
/// `javax` packages are left out on purpose: some are libraries (`javax.persistence`,
/// `javax.servlet`), and some JDK ones declare checked exceptions everywhere (XML parsers,
/// naming, image I/O).
fn is_jdk_owner(fqn: &str) -> bool {
    fqn.starts_with("java.") || fqn.starts_with("javax.swing.")
}

/// The class a statically imported method comes from (`java.util.Arrays` for `asList(..)` after
/// `import static java.util.Arrays.asList;`), unless the enclosing class declares a method of
/// that name, which would take precedence.
fn static_import_owner<'t>(ctx: &Ctx<'t>, call: Node<'t>) -> Option<&'t str> {
    let name = util::call_name(ctx, call);
    let import = ctx
        .imports()
        .iter()
        .find(|i| i.is_static && !i.is_wildcard && i.simple_name() == name)?;
    let declared_here = ctx
        .enclosing_class(call)
        .and_then(syntax::type_body)
        .is_some_and(|b| util::declares_method(ctx, b, name));
    (!declared_here).then(|| import.qualifier())
}

/// A call or object creation used as an argument that could itself throw.
fn risky_argument_call<'t>(ctx: &Ctx<'t>, n: Node<'t>) -> bool {
    match kind(n) {
        "method_invocation" => {
            if util::arg_count(n) == 0 {
                return false;
            }
            // Static factories and methods of JDK types (`List.of(..)`, `Arrays.asList(..)`,
            // `text.getBytes(UTF_8)`), also through a static import (`asList(..)`).
            let owner = match util::fld(n, F.object) {
                Some(obj) => ctx
                    .type_name_of(obj)
                    .or_else(|| ctx.type_of(obj).and_then(|t| t.fqn().map(str::to_string))),
                None => static_import_owner(ctx, n).map(str::to_string),
            };
            match owner {
                Some(f) if is_jdk_owner(&f) => jdk_method_throws(&f, util::call_name(ctx, n)),
                _ => true,
            }
        }
        "object_creation_expression" => {
            util::arg_count(n) > 0
                && util::fld(n, F.ty)
                    .and_then(|t| util::type_fqn(ctx, t))
                    .is_none_or(|f| {
                        !is_jdk_owner(&f) || THROWING_JDK_CONSTRUCTORS.contains(&f.as_str())
                    })
        }
        _ => false,
    }
}

/// A `return;` / `return <literal>;` that only satisfies the functional interface
/// (`ThrowingSupplier`, `Callable`).
fn is_trivial_return(stmt: Node<'_>) -> bool {
    kind(stmt) == "return_statement" && syntax::first_named_child(stmt).is_none_or(util::is_literal)
}

impl<'t> JavaRule<'t> for AssertThrowsMultipleCalls {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        NAMES
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !ctx.is_test() {
            return;
        }
        let Some(name) = util::call_named(ctx, node, NAMES) else {
            return;
        };
        let Some(lambda) = util::args(node).find(|a| kind(*a) == "lambda_expression") else {
            return;
        };
        let Some(body) = util::fld(lambda, F.body) else {
            return;
        };
        let t = util::test_imports(ctx);
        if !(t.jupiter || t.junit4) {
            return;
        }
        if util::fld(node, F.object).is_none() {
            if let Some(cls) = ctx.enclosing_class(node).and_then(syntax::type_body) {
                if util::declares_method(ctx, cls, name) {
                    return;
                }
            }
        }
        let scope = if kind(body) == "block" {
            let mut stmts = named_children(body).filter(|s| !is_trivial_return(*s));
            if stmts.next().is_none() {
                return;
            }
            let extra = stmts.count();
            if extra > 0 {
                ctx.emit(
                    lambda,
                    format!(
                        "Move the setup out of this `{name}` lambda: it holds {} statements \
                         instead of the single call under test",
                        extra + 1
                    ),
                )
                .value((extra + 1) as f64);
                return;
            }
            body
        } else {
            lambda
        };
        let mut risky = false;
        util::visit(scope, |n| {
            if util::is_other_context(n) {
                return Walk::Skip;
            }
            if kind(n) == "method_invocation" && util::call_name(ctx, n).starts_with("assert") {
                // `assertNotNull(new Foo(x))`: the assertion itself is not setup.
                return match util::fld(n, F.arguments) {
                    Some(args) => {
                        for a in named_children(args) {
                            if util::visit(a, |m| {
                                if util::is_other_context(m) {
                                    Walk::Skip
                                } else if kind(m) == "argument_list"
                                    && named_children(m)
                                        .any(|x| risky_argument_call(ctx, syntax::strip_parens(x)))
                                {
                                    Walk::Stop
                                } else {
                                    Walk::Continue
                                }
                            }) {
                                risky = true;
                                return Walk::Stop;
                            }
                        }
                        Walk::Skip
                    }
                    None => Walk::Skip,
                };
            }
            if kind(n) == "argument_list" {
                for a in named_children(n) {
                    let a = syntax::strip_parens(a);
                    if risky_argument_call(ctx, a) {
                        risky = true;
                        return Walk::Stop;
                    }
                }
            }
            Walk::Continue
        });
        if risky {
            ctx.emit(
                lambda,
                format!(
                    "Compute the arguments before `{name}`: only the call under test belongs in \
                     the lambda"
                ),
            );
        }
    }
}
