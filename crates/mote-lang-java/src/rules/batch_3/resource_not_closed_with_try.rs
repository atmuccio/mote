//! `java/resource-not-closed-with-try` — a locally opened file/JDBC resource that isn't
//! managed by try-with-resources (or closed in a `finally` block).
//!
//! The analysis of a declared resource ([`tracked_resource`]) is shared with
//! `java/prefer-try-with-resources`, which reports the resources this rule accepts because they
//! are closed in a `finally` block.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::Medium,
    tags: &["resources", "leak"],
    status: RuleStatus::Stable,
    remediation_minutes: 10,
    description: r#"Detects local variables that open a file or database resource —
`new FileInputStream/FileOutputStream/FileReader/FileWriter/RandomAccessFile/ZipFile(…)`,
`new FileImageOutputStream(…)`, a JDK decorator (`BufferedReader`, `OutputStreamWriter`,
`BufferedInputStream`, `ZipOutputStream`, `DataOutputStream`, …) over a newly opened file,
`new Scanner(File|Path)`, `getResourceAsStream(…)` on a `Class` or `ClassLoader`,
`Files.newInputStream/newOutputStream/newBufferedReader/newBufferedWriter/lines/list/walk(…)`,
or a `Connection`/`Statement`/`ResultSet` obtained from `getConnection`, `prepareStatement`,
`prepareCall`, `createStatement`, `executeQuery`, or a `DatabaseMetaData` query such as
`getTables` — when the variable is not managed by a try-with-resources statement and is not
closed in a `finally` block. A variable declared as `null` (or without a value) and assigned the resource
later (`in = null; … in = new FileInputStream(f);`) is checked the same way. Resources closed in a
`finally` block are closed on every path and not reported here; `prefer-try-with-resources`
suggests converting them.

**Why it matters.** If any statement between opening and `close()` throws, the close is skipped
and the file handle or connection leaks. Leaks accumulate until the process runs out of file
descriptors or the connection pool is exhausted.

**How to fix.** Declare the resource in a try-with-resources header; it is closed on every
path, including exceptions.

```java
// Noncompliant
BufferedReader in = new BufferedReader(new FileReader(path));
String first = in.readLine();
in.close();

// Compliant
try (BufferedReader in = new BufferedReader(new FileReader(path))) {
    String first = in.readLine();
}
```

To stay precise, the rule is silent when ownership may move elsewhere: the variable is
returned (or an object derived from it is, as in `return stmt.executeQuery(sql)`), assigned to
another variable or field, passed as an argument (e.g. wrapped by another stream or closed by a
helper) — unless the method itself closes it afterwards, outside a `finally` block, which shows
it kept the ownership (a close only in a `catch` block does not show that) — or captured by a
lambda or inner class. JDBC results are only considered when the
variable's declared type is a known `java.sql` type (not `var`), and not in test code (where
they are usually mocks). A `ResultSet` obtained from a statement variable is closed with that
statement, so only the statement is checked. Wrappers over non-file streams (`System.in`,
`StringReader`, a servlet writer) are ignored.

Also not reported:

* A connection lent by an object that manages its lifecycle. `getConnection()` counts as
  opening a connection only on `DriverManager` or on a data source (a receiver whose static
  type is a `javax.sql.DataSource` or a class named `…DataSource`). A connection taken from a
  transaction, session or unit-of-work object, from a statement, from a static helper such
  as `DataSourceUtils.getConnection(ds)`, or from a receiver of unknown type belongs to that
  owner, and closing it would be the bug.
* A resource whose `close()` is the very next statement (`Connection c = ds.getConnection();
  c.close();`, also behind `if (c != null)`): nothing can throw in between.
* A statement or result set created from a connection variable (`conn.prepareStatement(..)`,
  `conn.createStatement().executeQuery(..)`, `conn.getMetaData().getTables(..)`) when that
  connection is closed on the failure path: it is a try-with-resources resource of an
  enclosing `try`, or a `catch` or `finally` clause of an enclosing `try` closes it
  (`conn.close()` or a `close…(conn)` helper). Closing a connection closes its statements and
  result sets."#,
    ..RuleMeta::new(
        "java/resource-not-closed-with-try",
        "java",
        "Resource not closed with try-with-resources",
    )
};

pub struct ResourceNotClosedWithTry;

impl<'t> JavaRule<'t> for ResourceNotClosedWithTry {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["local_variable_declaration"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        for d in syntax::declarators(node) {
            let Some(t) = tracked_resource(node, d, ctx) else {
                continue;
            };
            if t.closed_in_finally || t.lent {
                continue;
            }
            let var = ctx.text(t.name);
            let value = d
                .child_by_field_id(util::FIELDS.value)
                .map(syntax::strip_parens)
                .filter(|v| !syntax::is_null_literal(*v));
            if let Some(value) = value {
                if closed_by_next_statement(node, d, var, ctx)
                    || parent_connection_closed_on_failure(node, value, ctx)
                {
                    continue;
                }
            }
            ctx.emit(
                t.name,
                format!("Open `{var}` ({}) in a try-with-resources statement so it is closed even when an exception is thrown", t.what),
            );
        }
    }
}

/// A local variable holding a resource opened in this method that stays in the method: not
/// returned, stored, passed on (except to a helper called in a `finally` block), captured, or
/// managed by a try-with-resources statement.
pub(super) struct Tracked<'t> {
    /// The variable's name in its declaration.
    pub(super) name: Node<'t>,
    /// What it opens (`FileInputStream`, `PreparedStatement`, …).
    pub(super) what: &'static str,
    /// Whether it is closed in a `finally` block (by `close()` or by a helper).
    pub(super) closed_in_finally: bool,
    /// Whether it is (at least once) a connection lent by an owning object
    /// ([`is_lent_connection`]).
    pub(super) lent: bool,
}

/// `getConnection()` returns a connection the caller owns only on `DriverManager` or on a data
/// source: a receiver whose static type is a `javax.sql.DataSource` or a class named
/// `…DataSource`. Anything else — a transaction, session or unit-of-work object, a statement, an
/// unqualified or static helper such as `DataSourceUtils.getConnection(ds)`, or a receiver of
/// unknown type — is taken to lend a connection whose lifecycle it manages.
fn is_lent_connection<'t>(value: Node<'t>, ctx: &Ctx<'t>) -> bool {
    if kind(value) != "method_invocation" || ctx.call_name(value) != Some("getConnection") {
        return false;
    }
    if ctx.is_call_to(value, "java.sql.DriverManager", "getConnection") {
        return false;
    }
    let Some(recv) = value.field(Field::Object) else {
        return true;
    };
    if ctx.type_name_of(recv).is_some() {
        // A static call on a class other than `DriverManager`.
        return true;
    }
    let is_data_source = |n: &str| n.ends_with("DataSource");
    let owned = match ctx.type_of(recv) {
        Some(t) => {
            ctx.is_subtype_of(&t, "javax.sql.DataSource")
                || t.simple_name().is_some_and(is_data_source)
        }
        // A library type the resolver does not know: its name as written.
        None => written_type_name(recv, ctx).is_some_and(is_data_source),
    };
    !owned
}

/// The simple type name written for `expr`: the declared type of a variable, or a cast's type.
fn written_type_name<'t>(expr: Node<'t>, ctx: &Ctx<'t>) -> Option<&'t str> {
    let e = syntax::strip_parens(expr);
    let type_node = match kind(e) {
        "identifier" => ctx.declaration_of(e)?.type_node?,
        "cast_expression" => e.field(Field::Type)?,
        _ => return None,
    };
    Some(util::type_simple_name(type_node, ctx))
}

/// Whether the statement right after the declaration `stmt` is `var.close();`: nothing can throw
/// between opening and closing. Only when `declarator` is the last one of `stmt` (a later
/// declarator's initializer runs in between).
fn closed_by_next_statement<'t>(
    stmt: Node<'t>,
    declarator: Node<'t>,
    var: &str,
    ctx: &Ctx<'t>,
) -> bool {
    if syntax::declarators(stmt).last().map(|d| d.id()) != Some(declarator.id()) {
        return false;
    }
    let Some(parent) = ctx.parent(stmt) else {
        return false;
    };
    let mut after = named_children(parent).skip_while(|c| c.id() != stmt.id());
    after.next();
    after
        .next()
        .is_some_and(|next| is_close_statement(next, var, ctx))
}

/// `var.close();`, or `if (var != null) { var.close(); }` (no `else`).
fn is_close_statement<'t>(stmt: Node<'t>, var: &str, ctx: &Ctx<'t>) -> bool {
    if kind(stmt) == "if_statement" {
        if stmt.field(Field::Alternative).is_some() {
            return false;
        }
        let guarded = stmt.field(Field::Condition).is_some_and(|c| {
            let t: String = ctx
                .text(c)
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            t == format!("({var}!=null)") || t == format!("(null!={var})")
        });
        let Some(cons) = stmt.field(Field::Consequence) else {
            return false;
        };
        let only = if kind(cons) == "block" {
            let mut it = named_children(cons);
            match (it.next(), it.next()) {
                (Some(one), None) => one,
                _ => return false,
            }
        } else {
            cons
        };
        return guarded && is_close_statement(only, var, ctx);
    }
    if kind(stmt) != "expression_statement" {
        return false;
    }
    let Some(call) = syntax::first_named_child(stmt).filter(|c| kind(*c) == "method_invocation")
    else {
        return false;
    };
    ctx.call_name(call) == Some("close")
        && syntax::argument_count(call) == 0
        && call
            .field(Field::Object)
            .map(syntax::strip_parens)
            .is_some_and(|o| kind(o) == "identifier" && ctx.text(o) == var)
}

/// JDBC calls creating an object that closing its connection also closes.
const CONNECTION_CHILDREN: &[&str] = &[
    "prepareStatement",
    "prepareCall",
    "createStatement",
    "executeQuery",
];

/// For a statement or result set created from a connection variable (`conn.prepareStatement(..)`,
/// `conn.createStatement().executeQuery(..)`, `conn.getMetaData().getTables(..)`): whether that
/// connection is closed on the failure path of the code opening it — declared as a resource of
/// an enclosing try-with-resources, or closed (`conn.close()`, or passed to a method whose name
/// contains `close`) in a `catch` or `finally` clause of an enclosing `try` whose body holds
/// `stmt`. Closing a connection closes its statements and result sets.
fn parent_connection_closed_on_failure<'t>(stmt: Node<'t>, value: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let Some(method) = ctx.call_name(value) else {
        return false;
    };
    if !CONNECTION_CHILDREN.contains(&method) && !METADATA_RESULT_SETS.contains(&method) {
        return false;
    }
    // The connection: the variable at the root of the receiver chain.
    let mut root = value;
    let mut steps = 0;
    while kind(root) == "method_invocation" && steps < 8 {
        let Some(o) = root.field(Field::Object).map(syntax::strip_parens) else {
            return false;
        };
        root = o;
        steps += 1;
    }
    if kind(root) != "identifier" {
        return false;
    }
    let conn = ctx.text(root);
    for a in ctx.ancestors(stmt) {
        match kind(a) {
            "try_statement" | "try_with_resources_statement" => {
                let in_body = a.field(Field::Body).is_some_and(|b| {
                    b.start_byte() <= stmt.start_byte() && stmt.end_byte() <= b.end_byte()
                });
                if !in_body {
                    continue;
                }
                if kind(a) == "try_with_resources_statement" && is_resource_of(a, conn, ctx) {
                    return true;
                }
                let closes = syntax::children(a)
                    .filter(|c| matches!(kind(*c), "catch_clause" | "finally_clause"))
                    .any(|c| closes_variable(c, conn, ctx));
                if closes {
                    return true;
                }
            }
            "method_declaration"
            | "constructor_declaration"
            | "lambda_expression"
            | "class_body" => break,
            _ => {}
        }
    }
    false
}

/// Whether the try-with-resources statement `tws` manages the variable `name`.
fn is_resource_of<'t>(tws: Node<'t>, name: &str, ctx: &Ctx<'t>) -> bool {
    let Some(spec) = tws.field(Field::Resources) else {
        return false;
    };
    named_children(spec).any(|r| {
        let declared = util::name_field(r).map(|n| ctx.text(n));
        let named = syntax::first_named_child(r)
            .filter(|c| kind(*c) == "identifier")
            .map(|c| ctx.text(c));
        declared == Some(name) || (declared.is_none() && named == Some(name))
    })
}

/// Whether `clause` calls `name.close()` or passes `name` to a method whose name contains
/// `close` (`closeQuietly(conn)`, `JdbcUtils.closeConnection(conn)`).
fn closes_variable<'t>(clause: Node<'t>, name: &str, ctx: &Ctx<'t>) -> bool {
    let text = ctx.text(clause);
    if !text.contains(name) || !text.to_ascii_lowercase().contains("close") {
        return false;
    }
    let is_name = |n: Node<'t>| {
        let n = syntax::strip_parens(n);
        kind(n) == "identifier" && ctx.text(n) == name
    };
    util::any_descendant(clause, util::is_other_context, |n| {
        if kind(n) != "method_invocation" {
            return false;
        }
        let Some(m) = ctx.call_name(n) else {
            return false;
        };
        (m == "close" && n.field(Field::Object).is_some_and(is_name))
            || (m.to_ascii_lowercase().contains("close") && syntax::arguments(n).any(is_name))
    })
}

/// The resource declared by `declarator` (of the `local_variable_declaration` `stmt`), when it
/// is a tracked kind and stays in the method: initialized with it, or declared as `null` and
/// only ever assigned tracked resources (or `null`) later. Computed once per declarator for
/// both rules that use it.
pub(super) fn tracked_resource<'t>(
    stmt: Node<'t>,
    declarator: Node<'t>,
    ctx: &Ctx<'t>,
) -> Option<Tracked<'t>> {
    // Most declarations hold nothing tracked: reject them by name before the shared memo.
    if !may_be_tracked(stmt, declarator, ctx) {
        return None;
    }
    let facts = ctx.shared_memo("batch_3/tracked_resource", declarator.id(), || {
        analyze_resource(stmt, declarator, ctx).map(|t| (t.what, t.closed_in_finally, t.lent))
    })?;
    let (what, closed_in_finally, lent) = facts;
    Some(Tracked {
        // The name `analyze_resource` found.
        name: util::name_field(declarator)?,
        what,
        closed_in_finally,
        lent,
    })
}

/// A necessary condition for [`analyze_resource`] to find a resource, from names only: the
/// initializer creates one of the tracked classes or calls one of the tracked methods, or the
/// variable is declared as `null` (or without a value) with a tracked type.
/// Longest declarator text [`may_be_tracked`] scans.
const MAX_SCANNED_DECLARATOR: usize = 512;

fn may_be_tracked<'t>(stmt: Node<'t>, declarator: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let names = &*TRACKED_NAMES;
    // From the text first: a tracked creation or call has its class or method name as a word
    // of the declarator. Without one, only a variable declared as `null` or without a value is
    // left, and without a comment or an annotation (`/`, `@`) the text tells whether it is one.
    // (Only for short declarators: the tree test below gives the same answer, and scanning the
    // text of every declarator of nested lambdas or anonymous classes was quadratic.)
    let text = ctx.text(declarator);
    if text.len() <= MAX_SCANNED_DECLARATOR {
        let scan = scan_declarator(text.as_bytes(), names);
        if !scan.tracked_word && !scan.unsure {
            if !scan.equals {
                // No value.
                return kind(stmt) == "local_variable_declaration"
                    && stmt
                        .field(Field::Type)
                        .is_some_and(|t| names.declared.contains(util::type_simple_name(t, ctx)));
            }
            if !scan.null {
                // A value that is neither `null` nor a tracked creation or call.
                return false;
            }
        }
    }
    let value = declarator
        .child_by_field_id(util::FIELDS.value)
        .map(syntax::strip_parens);
    match value {
        Some(v) if !syntax::is_null_literal(v) => match kind(v) {
            "object_creation_expression" => v
                .field(Field::Type)
                .is_some_and(|t| names.created.contains(util::type_simple_name(t, ctx))),
            "method_invocation" => {
                util::name_field(v).is_some_and(|n| names.called.contains(ctx.text(n)))
            }
            _ => false,
        },
        // Declared as `null` or without a value.
        _ => {
            (value.is_some() || kind(stmt) == "local_variable_declaration")
                && stmt
                    .field(Field::Type)
                    .is_some_and(|t| names.declared.contains(util::type_simple_name(t, ctx)))
        }
    }
}

/// What one pass over the text of a declarator finds.
struct DeclaratorText {
    /// A word is the name of a tracked class or method (`TrackedNames::words`).
    tracked_word: bool,
    /// A word is `null` (only complete when `tracked_word` is false).
    null: bool,
    /// The text holds `=` (only complete when `tracked_word` is false).
    equals: bool,
    /// The text holds `/` or `@`: a comment or an annotation, whose text may hold `=` or
    /// `null` (only complete when `tracked_word` is false).
    unsure: bool,
}

/// Reads the words of `text` (runs of ASCII letters, digits, `_` and `$`: every name the
/// rule looks for is one, and a Java identifier holding such a name as a whole is that run)
/// until one is a tracked name.
fn scan_declarator(text: &[u8], names: &TrackedNames) -> DeclaratorText {
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
    let mut found = DeclaratorText {
        tracked_word: false,
        null: false,
        equals: false,
        unsure: false,
    };
    let mut i = 0;
    while let Some(&b) = text.get(i) {
        if !is_word(b) {
            found.equals |= b == b'=';
            found.unsure |= b == b'/' || b == b'@';
            i += 1;
            continue;
        }
        let start = i;
        while text.get(i).is_some_and(|b| is_word(*b)) {
            i += 1;
        }
        let word = &text[start..i];
        if names.might_be_word(word) && names.words.contains(word) {
            found.tracked_word = true;
            return found;
        }
        found.null |= word == b"null";
    }
    found
}

/// The names [`classify`] and [`analyze_resource`] look for, as sets.
struct TrackedNames {
    /// Classes whose creation may open a resource.
    created: FxHashSet<&'static str>,
    /// Methods whose call may return a resource.
    called: FxHashSet<&'static str>,
    /// Declared types of a variable that may later be assigned one.
    declared: FxHashSet<&'static str>,
    /// `created` and `called` together.
    words: FxHashSet<&'static [u8]>,
    /// By word length: a bit per first letter of a word of that length (a cheap test before
    /// hashing).
    first_letters: [u64; 64],
}

impl TrackedNames {
    fn letter_bit(b: u8) -> u64 {
        match b {
            b'A'..=b'Z' => 1 << (b - b'A'),
            b'a'..=b'z' => 1 << (26 + b - b'a'),
            _ => 0,
        }
    }

    /// Whether some word of `words` has the length and the first letter of `word`.
    fn might_be_word(&self, word: &[u8]) -> bool {
        let letters = self.first_letters.get(word.len()).copied().unwrap_or(0);
        word.first()
            .is_some_and(|b| letters & Self::letter_bit(*b) != 0)
    }
}

static TRACKED_NAMES: std::sync::LazyLock<TrackedNames> = std::sync::LazyLock::new(|| {
    let created: FxHashSet<&'static str> = FILE_OPENERS
        .iter()
        .chain(WRAPPERS)
        .map(|(s, _)| *s)
        .chain(["Scanner"])
        .collect();
    let called: FxHashSet<&'static str> = FILES_OPENERS
        .iter()
        .map(|(m, _)| *m)
        .chain(METADATA_RESULT_SETS.iter().copied())
        .chain(JDBC_METHODS.iter().copied())
        .chain(["getResourceAsStream"])
        .collect();
    let words: FxHashSet<&'static [u8]> = created.union(&called).map(|w| w.as_bytes()).collect();
    let mut first_letters = [0u64; 64];
    for w in &words {
        if let (Some(slot), Some(first)) = (first_letters.get_mut(w.len()), w.first()) {
            *slot |= TrackedNames::letter_bit(*first);
        }
    }
    TrackedNames {
        words,
        first_letters,
        created,
        called,
        declared: RESOURCE_TYPES.iter().copied().collect(),
    }
});

fn analyze_resource<'t>(
    stmt: Node<'t>,
    declarator: Node<'t>,
    ctx: &Ctx<'t>,
) -> Option<Tracked<'t>> {
    // Cheapest test first: only `new …`, method-call and `null` initializers (or none) matter.
    let value = declarator
        .child_by_field_id(util::FIELDS.value)
        .map(syntax::strip_parens);
    let null_init = value.is_none_or(syntax::is_null_literal);
    if value.is_none() && kind(stmt) != "local_variable_declaration" {
        return None;
    }
    let value = value.unwrap_or(declarator);
    if !null_init
        && !matches!(
            kind(value),
            "object_creation_expression" | "method_invocation"
        )
    {
        return None;
    }
    let name = util::name_field(declarator)?;
    let type_node = stmt.field(Field::Type)?;
    let initial = if null_init {
        if !RESOURCE_TYPES.contains(&util::type_simple_name(type_node, ctx)) {
            return None;
        }
        None
    } else {
        Some(classify(value, type_node, ctx)?)
    };
    let lent = !null_init && is_lent_connection(value, ctx);
    let uses = scan_uses(stmt, declarator, name, type_node, initial, ctx)?;
    Some(Tracked {
        name,
        what: uses.what,
        closed_in_finally: uses.closed_in_finally,
        lent: lent || uses.lent,
    })
}

/// What [`scan_uses`] learns about a variable.
struct Uses {
    what: &'static str,
    closed_in_finally: bool,
    /// A later assignment stores a lent connection.
    lent: bool,
}

/// Classes opening a file directly, with their packages.
const FILE_OPENERS: &[(&str, &str)] = &[
    ("FileInputStream", "java.io.FileInputStream"),
    ("FileOutputStream", "java.io.FileOutputStream"),
    ("FileReader", "java.io.FileReader"),
    ("FileWriter", "java.io.FileWriter"),
    ("RandomAccessFile", "java.io.RandomAccessFile"),
    ("ZipFile", "java.util.zip.ZipFile"),
    (
        "FileImageOutputStream",
        "javax.imageio.stream.FileImageOutputStream",
    ),
];

/// Decorators that own the stream they wrap.
const WRAPPERS: &[(&str, &str)] = &[
    ("BufferedReader", "java.io.BufferedReader"),
    ("BufferedWriter", "java.io.BufferedWriter"),
    ("PrintWriter", "java.io.PrintWriter"),
    ("InputStreamReader", "java.io.InputStreamReader"),
    ("OutputStreamWriter", "java.io.OutputStreamWriter"),
    ("BufferedInputStream", "java.io.BufferedInputStream"),
    ("BufferedOutputStream", "java.io.BufferedOutputStream"),
    ("DataInputStream", "java.io.DataInputStream"),
    ("DataOutputStream", "java.io.DataOutputStream"),
    ("PrintStream", "java.io.PrintStream"),
    ("ZipInputStream", "java.util.zip.ZipInputStream"),
    ("ZipOutputStream", "java.util.zip.ZipOutputStream"),
    ("JarInputStream", "java.util.jar.JarInputStream"),
    ("JarOutputStream", "java.util.jar.JarOutputStream"),
    ("GZIPInputStream", "java.util.zip.GZIPInputStream"),
    ("GZIPOutputStream", "java.util.zip.GZIPOutputStream"),
];

const JDBC_METHODS: &[&str] = &[
    "getConnection",
    "prepareStatement",
    "prepareCall",
    "createStatement",
    "executeQuery",
];

/// `DatabaseMetaData` methods returning a result set the caller must close.
const METADATA_RESULT_SETS: &[&str] = &[
    "getTables",
    "getColumns",
    "getPrimaryKeys",
    "getImportedKeys",
    "getExportedKeys",
    "getCrossReference",
    "getIndexInfo",
    "getSchemas",
    "getCatalogs",
    "getTableTypes",
    "getTypeInfo",
    "getProcedures",
    "getProcedureColumns",
    "getFunctions",
    "getFunctionColumns",
    "getBestRowIdentifier",
    "getVersionColumns",
    "getColumnPrivileges",
    "getTablePrivileges",
    "getUDTs",
    "getSuperTypes",
    "getSuperTables",
    "getAttributes",
    "getClientInfoProperties",
    "getPseudoColumns",
];

/// `java.nio.file.Files` methods opening a file (or a directory stream), with a label.
const FILES_OPENERS: &[(&str, &str)] = &[
    ("newInputStream", "file InputStream"),
    ("newOutputStream", "file OutputStream"),
    ("newBufferedReader", "BufferedReader"),
    ("newBufferedWriter", "BufferedWriter"),
    ("newByteChannel", "file channel"),
    ("newDirectoryStream", "DirectoryStream"),
    ("lines", "Stream of file lines"),
    ("list", "Stream of directory entries"),
    ("walk", "Stream of directory entries"),
    ("find", "Stream of directory entries"),
];

/// Simple names of declared types a resource may be stored in (a cheap filter before looking at
/// the assignments of a variable declared as `null`).
const RESOURCE_TYPES: &[&str] = &[
    "InputStream",
    "OutputStream",
    "Reader",
    "Writer",
    "FileInputStream",
    "FileOutputStream",
    "FileReader",
    "FileWriter",
    "RandomAccessFile",
    "ZipFile",
    "BufferedReader",
    "BufferedWriter",
    "PrintWriter",
    "InputStreamReader",
    "OutputStreamWriter",
    "BufferedInputStream",
    "BufferedOutputStream",
    "DataInputStream",
    "DataOutputStream",
    "PrintStream",
    "ZipInputStream",
    "ZipOutputStream",
    "JarInputStream",
    "JarOutputStream",
    "GZIPInputStream",
    "GZIPOutputStream",
    "FileImageOutputStream",
    "ImageOutputStream",
    "Scanner",
    "Connection",
    "Statement",
    "PreparedStatement",
    "CallableStatement",
    "ResultSet",
    "Stream",
    "DirectoryStream",
    "Closeable",
    "AutoCloseable",
];

/// A short description of the resource `value` opens, if it is one of the tracked kinds.
fn classify<'t>(value: Node<'t>, type_node: Node<'t>, ctx: &Ctx<'t>) -> Option<&'static str> {
    match kind(value) {
        "object_creation_expression" => {
            if syntax::child_of_kind(value, "class_body").is_some() {
                return None;
            }
            let t = value.field(Field::Type)?;
            let simple = util::type_simple_name(t, ctx);
            if let Some((s, fqn)) = FILE_OPENERS.iter().find(|(s, _)| *s == simple) {
                return (util::type_is(ctx, t, fqn)).then_some(*s);
            }
            if simple == "Scanner" {
                let is_scanner = util::type_is(ctx, t, "java.util.Scanner");
                let first = syntax::arguments(value).next()?;
                let on_file = ctx.type_of(first).is_some_and(|a| {
                    a.is_class("java.io.File") || ctx.is_subtype_of(&a, "java.nio.file.Path")
                });
                return (is_scanner && on_file).then_some("Scanner");
            }
            let (s, fqn) = WRAPPERS.iter().find(|(s, _)| *s == simple)?;
            if !util::type_is(ctx, t, fqn) {
                return None;
            }
            let first = syntax::arguments(value).next()?;
            let file_name_arg = *s == "PrintWriter"
                && ctx
                    .type_of(first)
                    .is_some_and(|a| a.is_string() || a.is_class("java.io.File"));
            (file_name_arg || opens_file(first, ctx, 0)).then_some(*s)
        }
        "method_invocation" => {
            let name = util::name_field(value)?;
            let method = ctx.text(name);
            if method == "getResourceAsStream" {
                let opens = ctx.is_call_to(value, "java.lang.Class", method)
                    || ctx.is_call_to(value, "java.lang.ClassLoader", method)
                    || value
                        .field(Field::Object)
                        .is_some_and(|r| is_class_or_loader_of_get_class(r, ctx));
                return opens.then_some("resource InputStream");
            }
            if let Some((_, label)) = FILES_OPENERS.iter().find(|(m, _)| *m == method) {
                return ctx
                    .is_call_to(value, "java.nio.file.Files", method)
                    .then_some(*label);
            }
            let metadata = METADATA_RESULT_SETS.contains(&method);
            if !metadata && !JDBC_METHODS.contains(&method) {
                return None;
            }
            // JDBC objects in tests are usually mocks.
            if ctx.is_test() || ctx.text(type_node) == "var" {
                return None;
            }
            // A result set is closed together with its statement: when the statement is a
            // variable, whether it is managed decides (and is checked on its own declaration).
            if method == "executeQuery"
                && value
                    .field(Field::Object)
                    .map(syntax::strip_parens)
                    .is_some_and(|o| matches!(kind(o), "identifier" | "field_access"))
            {
                return None;
            }
            if metadata {
                // Only on a receiver known to be a `DatabaseMetaData`.
                let recv = value.field(Field::Object)?;
                let rt = ctx.type_of(recv)?;
                if !ctx.is_subtype_of(&rt, "java.sql.DatabaseMetaData") {
                    return None;
                }
            }
            let t = ctx.resolve_type(type_node)?;
            let known_jdbc = t.fqn().is_some_and(|f| f.starts_with("java.sql."));
            (known_jdbc && ctx.is_subtype_of(&t, "java.lang.AutoCloseable"))
                .then(|| t.simple_name().map_or("JDBC resource", jdbc_label))
        }
        _ => None,
    }
}

/// `getClass()` (on no receiver, `this`, or any object: `Object.getClass()` cannot be
/// overloaded without arguments) or `….getClass().getClassLoader()`: a `Class` or its
/// `ClassLoader`, which the resolver does not type through the inherited `getClass()`.
fn is_class_or_loader_of_get_class<'t>(recv: Node<'t>, ctx: &Ctx<'t>) -> bool {
    let recv = syntax::strip_parens(recv);
    if kind(recv) != "method_invocation" || syntax::argument_count(recv) != 0 {
        return false;
    }
    match ctx.call_name(recv) {
        Some("getClass") => true,
        Some("getClassLoader") => recv.field(Field::Object).is_some_and(|o| {
            let o = syntax::strip_parens(o);
            kind(o) == "method_invocation"
                && ctx.call_name(o) == Some("getClass")
                && syntax::argument_count(o) == 0
        }),
        _ => false,
    }
}

fn jdbc_label(simple: &str) -> &'static str {
    match simple {
        "Connection" => "Connection",
        "Statement" => "Statement",
        "PreparedStatement" => "PreparedStatement",
        "CallableStatement" => "CallableStatement",
        "ResultSet" => "ResultSet",
        _ => "JDBC resource",
    }
}

/// Whether `expr` newly opens a file (directly or through wrapping decorators).
fn opens_file<'t>(expr: Node<'t>, ctx: &Ctx<'t>, depth: u32) -> bool {
    let expr = syntax::strip_parens(expr);
    if depth > 4 || kind(expr) != "object_creation_expression" {
        return false;
    }
    let Some(t) = expr.field(Field::Type) else {
        return false;
    };
    let simple = util::type_simple_name(t, ctx);
    if let Some((_, fqn)) = FILE_OPENERS.iter().find(|(s, _)| *s == simple) {
        return util::type_is(ctx, t, fqn);
    }
    if let Some((_, fqn)) = WRAPPERS.iter().find(|(s, _)| *s == simple) {
        return util::type_is(ctx, t, fqn)
            && syntax::arguments(expr)
                .next()
                .is_some_and(|a| opens_file(a, ctx, depth + 1));
    }
    false
}

/// Scans the enclosing function body once for the uses of the variable declared by
/// `declarator`. Returns what it holds and whether it is closed in a `finally` block, or `None`
/// when it may escape (returned, stored, passed on outside a `finally` block, captured, used as a
/// try-with-resources resource). `initial` is what the initializer opens; `None` for a variable
/// declared as `null`, whose later assignments must then all be tracked resources (or `null`).
fn scan_uses<'t>(
    stmt: Node<'t>,
    declarator: Node<'t>,
    name: Node<'t>,
    type_node: Node<'t>,
    initial: Option<&'static str>,
    ctx: &Ctx<'t>,
) -> Option<Uses> {
    let func = ctx.enclosing_function(stmt)?;
    let body = func
        .field(Field::Body)
        .or_else(|| syntax::first_named_child(func).filter(|b| kind(*b) == "block"))?;
    let null_init = initial.is_none();
    let var = ctx.text(name);
    let after = declarator.end_byte();
    // Where the name occurs as text after the declarator: only subtrees holding one of these
    // can hold a use, so the walk skips the others.
    let from = after.max(body.start_byte());
    let occurrences: Vec<usize> = ctx
        .source()
        .get(from..body.end_byte())
        .map(|text| text.match_indices(var).map(|(i, _)| from + i).collect())
        .unwrap_or_default();
    let may_use = |n: Node<'t>| {
        let i = occurrences.partition_point(|&o| o < n.start_byte());
        occurrences
            .get(i)
            .is_some_and(|&o| o + var.len() <= n.end_byte())
    };
    let mut what = initial;
    let mut closed_in_finally = false;
    let mut lent = false;
    // Passed to a call outside `finally` (the last such use), and closed by this method outside
    // `finally` (the last such close): when the close comes after, the method kept ownership.
    let mut last_argument: Option<usize> = None;
    let mut last_close: Option<usize> = None;
    // Assignments of a resource (to a variable declared `null`), and those closed right after.
    let mut opened = 0usize;
    let mut closed_right_after = 0usize;
    // (node, parent, grandparent, inside finally, inside catch, inside lambda/inner class)
    type Entry<'t> = (
        Node<'t>,
        Option<Node<'t>>,
        Option<Node<'t>>,
        bool,
        bool,
        bool,
    );
    let mut stack: Vec<Entry<'t>> = vec![(body, None, None, false, false, false)];
    while let Some((n, parent, grand, in_finally, in_catch, captured)) = stack.pop() {
        let k = kind(n);
        if k == "identifier" {
            if n.start_byte() < after || ctx.text(n) != var {
                continue;
            }
            let same = ctx
                .declaration_of(n)
                .is_some_and(|d| d.decl.id() == declarator.id());
            if !same {
                continue;
            }
            if captured {
                return None;
            }
            let p = parent?;
            if null_init {
                let assigned = kind(p) == "assignment_expression"
                    && syntax::operator_str(p) == "="
                    && p.field(Field::Left).is_some_and(|l| l.id() == n.id());
                if assigned {
                    let rhs = p.field(Field::Right).map(syntax::strip_parens)?;
                    if !syntax::is_null_literal(rhs) {
                        // Holding something else at times makes ownership unclear.
                        what = Some(classify(rhs, type_node, ctx)?);
                        lent |= is_lent_connection(rhs, ctx);
                        opened += 1;
                        // `in = open(..); if (in != null) { in.close(); }`: nothing in between.
                        let closed_next = ctx
                            .parent(p)
                            .filter(|st| kind(*st) == "expression_statement")
                            .and_then(|st| ctx.next_named_sibling(st))
                            .is_some_and(|next| is_close_statement(next, var, ctx));
                        if closed_next {
                            closed_right_after += 1;
                        }
                    }
                    continue;
                }
            }
            match use_kind(n, p, grand, ctx) {
                Use::Benign => {}
                Use::Close => {
                    closed_in_finally |= in_finally;
                    // A close on the normal path (not only when something failed).
                    if !in_finally && !in_catch {
                        last_close = last_close.max(Some(n.start_byte()));
                    }
                }
                // `finally { closeQuietly(in); }` closes it; elsewhere ownership may move.
                Use::Argument if in_finally => closed_in_finally = true,
                Use::Argument => last_argument = last_argument.max(Some(n.start_byte())),
                Use::Escape => return None,
            }
            continue;
        }
        let in_finally = in_finally || k == "finally_clause";
        let in_catch = in_catch || k == "catch_clause";
        let captured = captured || util::is_other_context(n);
        // Comments are skipped too: they hold no identifier.
        for ch in syntax::named_children(n).filter(|ch| may_use(*ch)) {
            stack.push((ch, Some(n), parent, in_finally, in_catch, captured));
        }
    }
    if opened > 0 && closed_right_after == opened {
        return None;
    }
    if let Some(arg) = last_argument {
        // Passed on and never closed here, or closed in `finally` too: ownership may have moved.
        if closed_in_finally || last_close.is_none_or(|c| c < arg) {
            return None;
        }
    }
    Some(Uses {
        what: what?,
        closed_in_finally,
        lent,
    })
}

enum Use {
    Benign,
    Close,
    Argument,
    Escape,
}

fn use_kind<'t>(ident: Node<'t>, parent: Node<'t>, grand: Option<Node<'t>>, ctx: &Ctx<'t>) -> Use {
    match kind(parent) {
        "method_invocation" => {
            let is_receiver = parent
                .field(Field::Object)
                .is_some_and(|o| o.id() == ident.id());
            if !is_receiver {
                return Use::Escape;
            }
            let closing = parent
                .field(Field::Name)
                .is_some_and(|m| ctx.text(m) == "close");
            if closing {
                Use::Close
            } else if grand.is_some_and(|g| kind(g) == "return_statement") {
                // `return stmt.executeQuery(sql);`: the caller takes over the derived object.
                Use::Escape
            } else {
                Use::Benign
            }
        }
        "argument_list" => Use::Argument,
        "assignment_expression" => {
            let is_target = parent
                .field(Field::Left)
                .is_some_and(|l| l.id() == ident.id());
            if is_target {
                Use::Benign
            } else {
                Use::Escape
            }
        }
        "binary_expression" | "field_access" | "instanceof_expression" | "expression_statement" => {
            Use::Benign
        }
        // return, variable_declarator (alias), resource (`try (in)`), casts, ternaries, arrays…
        _ => Use::Escape,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tracked_name_passes_the_cheap_word_test() {
        let names = &*TRACKED_NAMES;
        assert!(!names.words.is_empty());
        for w in &names.words {
            assert!(names.might_be_word(w), "{}", String::from_utf8_lossy(w));
        }
        for w in names.created.iter().chain(&names.called) {
            let scan = scan_declarator(format!("x = a.{w}(b)").as_bytes(), names);
            assert!(scan.tracked_word, "{w}");
        }
    }

    #[test]
    fn the_declarator_scan_reads_words_and_signs() {
        let names = &*TRACKED_NAMES;
        let scan = scan_declarator(b"in = null", names);
        assert!(!scan.tracked_word && scan.null && scan.equals && !scan.unsure);
        let scan = scan_declarator(b"in /* note */", names);
        assert!(!scan.tracked_word && !scan.null && !scan.equals && scan.unsure);
        // Part of a longer name is not the name.
        let scan = scan_declarator(b"r = new MyFileReader(f)", names);
        assert!(!scan.tracked_word);
        let scan = scan_declarator(b"r = new java.io.FileReader(f)", names);
        assert!(scan.tracked_word);
    }
}
