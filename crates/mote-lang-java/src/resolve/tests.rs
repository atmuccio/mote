//! Resolver unit tests: type an expression found by its source text.

use crate::ctx::Ctx;
use crate::parse::{parse, ParseBudget};
use crate::resolve::VarKind;
use mote_core::{RuleIndex, SourceFile};
use tree_sitter::{Node, Tree};

struct Finder<'a> {
    src: &'a str,
    root: Node<'a>,
}

impl<'a> Finder<'a> {
    /// The last named node (pre-order) whose text is exactly .
    fn find(&self, text: &str) -> Node<'a> {
        let mut found = None;
        let mut cursor = self.root.walk();
        'walk: loop {
            let n = cursor.node();
            if n.is_named() && self.src.get(n.start_byte()..n.end_byte()) == Some(text) {
                found = Some(n);
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
        found.unwrap_or_else(|| panic!("no node with text `{text}`"))
    }
}

struct Fixture {
    src: String,
    tree: Tree,
    rules: RuleIndex,
}

impl Fixture {
    fn new(src: &str) -> Self {
        let tree = parse(src, ParseBudget::DEFAULT, None).expect("parse");
        Fixture {
            src: src.to_string(),
            tree,
            rules: RuleIndex::default(),
        }
    }

    fn with<R>(&self, f: impl for<'a> FnOnce(&Ctx<'a>, &Finder<'a>) -> R) -> R {
        let file = SourceFile {
            path: "src/T.java",
            text: &self.src,
            is_test: false,
        };
        let ctx = Ctx::new(&file, self.tree.root_node(), &self.rules, &[]);
        let finder = Finder {
            src: &self.src,
            root: self.tree.root_node(),
        };
        f(&ctx, &finder)
    }

    fn ty(&self, expr: &str) -> Option<String> {
        self.with(|ctx, f| ctx.type_of(f.find(expr)).map(|t| t.to_string()))
    }
}

fn check(src: &str, cases: &[(&str, Option<&str>)]) {
    let fx = Fixture::new(src);
    for (expr, want) in cases {
        assert_eq!(fx.ty(expr).as_deref(), *want, "type_of(`{expr}`)");
    }
}

#[test]
fn locals_params_literals_and_operators() {
    check(
        r#"class A { void m(int a, String s, boolean flag, Object o, String[] args, char ch) {
            long b = a; var c = s; float f = 1.0f;
            use(a + b); use(s + a); use(1L); use(1.5); use('c'); use(true); use(null);
            use(flag ? "x" : "y"); use(flag ? 1 : 2L); use(flag ? null : "z");
            use((String) o); use(args[0]); use(args.length); use(new int[3][]);
            use(c); use(-a); use(!flag); use(a << 2); use(ch + 1); use(a > b); use(a++);
            use("""
                block
                """);
        } }"#,
        &[
            ("a + b", Some("long")),
            ("s + a", Some("String")),
            ("1L", Some("long")),
            ("1.5", Some("double")),
            ("'c'", Some("char")),
            ("true", Some("boolean")),
            ("null", Some("null")),
            ("flag ? \"x\" : \"y\"", Some("String")),
            ("flag ? 1 : 2L", Some("long")),
            ("flag ? null : \"z\"", Some("String")),
            ("(String) o", Some("String")),
            ("args[0]", Some("String")),
            ("args.length", Some("int")),
            ("new int[3][]", Some("int[][]")),
            ("c", Some("String")),
            ("-a", Some("int")),
            ("!flag", Some("boolean")),
            ("a << 2", Some("int")),
            ("ch + 1", Some("int")),
            ("a > b", Some("boolean")),
            ("a++", Some("int")),
            ("f", Some("float")),
        ],
    );
}

#[test]
fn generics_and_jdk_methods() {
    check(
        r#"import java.util.*;
        class A { void m(List<String> l, Map<String, List<Integer>> m, Optional<Double> od, Map.Entry<String, Integer> en) {
            use(l.get(0)); use(m.get("k")); use(m.keySet()); use(m.entrySet());
            for (var e : m.entrySet()) { use(e.getValue()); }
            for (String item : l) { use(item); }
            use(od.orElse(0.0)); use(en.getKey()); use(l.stream().filter(p -> true).count());
            use(l.iterator().next()); use(l.subList(0, 1)); use(l.isEmpty()); use(l.remove(0));
            use(Math.max(1, 2L)); use(String.valueOf(3)); use(Arrays.asList("a"));
            use(System.out); use(System.out.println("x")); use(Math.round(1.5f));
            use(Character.toUpperCase('a')); use(Integer.parseInt("1")); use(s().length());
            ArrayList<String> al = new ArrayList<>(); use(al); use(new ArrayList<>());
            use(List.of(1, 2)); use(Collections.emptyList()); use(l.toString());
        } String s() { return ""; } }"#,
        &[
            ("l.get(0)", Some("String")),
            ("m.get(\"k\")", Some("List<Integer>")),
            ("m.keySet()", Some("Set<String>")),
            (
                "m.entrySet()",
                Some("Set<Map.Entry<String, List<Integer>>>"),
            ),
            ("e.getValue()", Some("List<Integer>")),
            ("item", Some("String")),
            ("od.orElse(0.0)", Some("Double")),
            ("en.getKey()", Some("String")),
            ("l.stream().filter(p -> true).count()", Some("long")),
            ("l.iterator().next()", Some("String")),
            ("l.subList(0, 1)", Some("List<String>")),
            ("l.isEmpty()", Some("boolean")),
            ("l.remove(0)", Some("String")),
            ("Math.max(1, 2L)", Some("long")),
            ("String.valueOf(3)", Some("String")),
            ("Arrays.asList(\"a\")", Some("List<?>")),
            ("System.out", Some("PrintStream")),
            ("System.out.println(\"x\")", Some("void")),
            ("Math.round(1.5f)", Some("int")),
            ("Character.toUpperCase('a')", Some("char")),
            ("Integer.parseInt(\"1\")", Some("int")),
            ("s().length()", Some("int")),
            ("al", Some("ArrayList<String>")),
            ("new ArrayList<>()", Some("ArrayList")),
            ("List.of(1, 2)", Some("List<?>")),
            ("Collections.emptyList()", Some("List<?>")),
            ("l.toString()", Some("String")),
        ],
    );
}

#[test]
fn fields_this_and_local_methods() {
    check(
        r#"package p;
        class Outer {
            private String name; int count; static final long ID = 1;
            String helper() { return name; }
            int f(int x) { return x; }
            String f(String x) { return x; }
            void m() { use(this.name); use(helper()); use(f(1)); use(f("s")); use((this)); use(ID); use(this.helper()); }
            class Inner {
                void n() { use(Outer.this.count); use(name); use(count + 1); }
            }
            static class Box<T> { T value; T get() { return value; } }
            void box(Box<String> b) { use(b.get()); use(b.value); }
            enum Color { RED, GREEN; }
            void colors() { use(Color.RED); use(Color.values()); use(Color.RED.ordinal()); use(Color.valueOf("RED")); }
        }"#,
        &[
            ("this.name", Some("String")),
            ("helper()", Some("String")),
            ("f(1)", Some("int")),
            ("f(\"s\")", Some("String")),
            ("(this)", Some("Outer")),
            ("ID", Some("long")),
            ("this.helper()", Some("String")),
            ("Outer.this.count", Some("int")),
            ("name", Some("String")),
            ("count + 1", Some("int")),
            ("b.get()", Some("String")),
            ("b.value", Some("String")),
            ("Color.RED", Some("Outer.Color")),
            ("Color.values()", Some("Outer.Color[]")),
            ("Color.RED.ordinal()", Some("int")),
            ("Color.valueOf(\"RED\")", Some("Outer.Color")),
        ],
    );
}

#[test]
fn unknown_types_stay_unknown() {
    check(
        r#"import com.acme.Foo;
        class Outer {
            String shared;
            class Sub extends com.acme.Base {
                void m(Foo foo, Object o) {
                    use(shared);            // may be inherited from Base
                    use(foo.bar());
                    use(Foo.make());
                    use(unknownCall());
                    use(o.hashCode());
                    java.util.List<Foo> fs = null; use(fs.get(0)); use(fs.size());
                    Runnable r = () -> use(o);
                }
            }
            <T> void g(T t, java.util.List<T> ts) { use(t); use(ts.get(0)); }
        }"#,
        &[
            ("shared", None),
            ("foo.bar()", None),
            ("Foo.make()", None),
            ("unknownCall()", None),
            ("o.hashCode()", Some("int")),
            ("fs.get(0)", None),
            ("fs.size()", Some("int")),
            ("t", None),
            ("ts.get(0)", None),
        ],
    );
}

#[test]
fn scopes_patterns_and_shadowing() {
    check(
        r#"import java.util.concurrent.TimeUnit;
        import static java.util.concurrent.TimeUnit.SECONDS;
        class A {
            String name;
            void m(Object o) {
                int name = 3; use(name);
                if (o instanceof String s && s.isEmpty()) { use(s); }
                try (java.io.BufferedReader br = null) { use(br.readLine()); } catch (java.io.IOException ex) { use(ex); }
                use(SECONDS); use(TimeUnit.SECONDS.toMillis(1));
                switch (o) { case Integer i -> use(i); default -> {} }
            }
        }"#,
        &[
            ("name", Some("int")),
            ("s", Some("String")),
            ("br.readLine()", Some("String")),
            ("ex", Some("IOException")),
            ("SECONDS", Some("TimeUnit")),
            ("TimeUnit.SECONDS.toMillis(1)", Some("long")),
            ("i", Some("Integer")),
        ],
    );
}

#[test]
fn subtypes_and_calls() {
    let fx = Fixture::new(
        r#"import java.util.*;
        import static java.util.Collections.sort;
        class MyList extends ArrayList<String> {}
        class Weird extends com.acme.Base {}
        class A {
            void m(MyList ml, Weird w, ArrayList<String> al, List<String> l) throws Exception {
                use(ml); use(w); use(al);
                Thread.sleep(1); l.add("x"); sort(l); al.clear();
            }
        }"#,
    );
    fx.with(|ctx, f| {
        let find = |t: &str| f.find(t);
        let ml = ctx.type_of(find("ml")).unwrap();
        assert!(ctx.is_subtype_of(&ml, "java.util.Collection"));
        assert!(ctx.is_subtype_of(&ml, "java.lang.Iterable"));
        assert_eq!(ctx.subtype(&ml, "java.util.Map"), Some(false));
        let it = ctx.as_supertype(&ml, "java.lang.Iterable").unwrap();
        assert_eq!(it.to_string(), "Iterable<String>");
        let w = ctx.type_of(find("w")).unwrap();
        assert_eq!(ctx.subtype(&w, "java.util.Collection"), None);
        assert!(ctx.is_subtype_of(&w, "java.lang.Object"));
        let al = ctx.type_of(find("al")).unwrap();
        assert!(ctx.is_subtype_of(&al, "java.util.RandomAccess"));
        assert!(ctx.is_call_to(find("Thread.sleep(1)"), "java.lang.Thread", "sleep"));
        assert!(ctx.is_call_to(find("l.add(\"x\")"), "java.util.Collection", "add"));
        assert!(!ctx.is_call_to(find("l.add(\"x\")"), "java.util.Map", "add"));
        assert!(ctx.is_call_to(find("sort(l)"), "java.util.Collections", "sort"));
        assert!(ctx.is_call_to(find("al.clear()"), "java.util.List", "clear"));
    });
}

#[test]
fn declarations_and_names() {
    let fx = Fixture::new(
        r#"import java.util.List;
        import com.acme.Foo;
        class A {
            private int field;
            void m(int param) {
                int local = 1;
                try { run(); } catch (RuntimeException caught) { use(caught); }
                use(field); use(param); use(local);
                for (String each : List.of("a")) { use(each); }
            }
        }"#,
    );
    fx.with(|ctx, f| {
        let find = |t: &str| f.find(t);
        let kinds: Vec<VarKind> = ["caught", "field", "param", "local", "each"]
            .iter()
            .map(|n| ctx.declaration_of(find(n)).map(|d| d.kind).unwrap())
            .collect();
        assert_eq!(
            kinds,
            vec![
                VarKind::CatchParameter,
                VarKind::Field,
                VarKind::Parameter,
                VarKind::Local,
                VarKind::ForEachVariable
            ]
        );
        let at = find("local");
        assert_eq!(
            ctx.resolve_type_name("List", at).as_deref(),
            Some("java.util.List")
        );
        assert_eq!(
            ctx.resolve_type_name("Foo", at).as_deref(),
            Some("com.acme.Foo")
        );
        assert_eq!(
            ctx.resolve_type_name("String", at).as_deref(),
            Some("java.lang.String")
        );
        assert_eq!(ctx.resolve_type_name("Bar", at), None);
        assert_eq!(ctx.resolve_type_name("A", at).as_deref(), Some("A"));
    });
}

#[test]
fn pathological_inputs_do_not_overflow() {
    let long_concat = format!("class A {{ String s = \"a\"{}; }}", " + 1".repeat(20_000));
    let fx = Fixture::new(&long_concat);
    let expr = format!("\"a\"{}", " + 1".repeat(20_000));
    assert_eq!(fx.ty(&expr).as_deref(), Some("String"));

    let deep = format!(
        "class A {{ int x = {}1{}; }}",
        "(".repeat(3_000),
        ")".repeat(3_000)
    );
    let fx = Fixture::new(&deep);
    let expr = format!("{}1{}", "(".repeat(3_000), ")".repeat(3_000));
    assert_eq!(fx.ty(&expr), None);

    // Cyclic inheritance and self-referential `var` must terminate.
    check(
        "class A extends B {} class B extends A {} class C { void m(A a) { var v = v; use(v); use(a.foo()); } }",
        &[("v", None), ("a.foo()", None)],
    );
}
