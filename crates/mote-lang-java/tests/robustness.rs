//! Pathological inputs: nothing may panic, hang, or overflow the stack.

use mote_core::testing::ruleset_all;
use mote_core::{FileAnalysis, LanguagePlugin, LineIndex, SourceFile};
use mote_lang_java::{JavaPlugin, ParseBudget};
use std::time::{Duration, Instant};

fn run(plugin: &JavaPlugin, text: &str) -> FileAnalysis {
    let rules = ruleset_all(plugin);
    let file = SourceFile {
        path: "src/P.java",
        text,
        is_test: false,
    };
    // Run on a thread with a small (1 MiB) stack to catch deep recursion.
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn_scoped(s, || plugin.analyze(&file, &rules))
            .expect("spawn")
            .join()
            .expect("analysis panicked")
    })
}

#[test]
fn degenerate_inputs() {
    let plugin = JavaPlugin::new();
    for text in [
        "",
        "   \n\n",
        "// only a comment",
        "/* unterminated",
        "class",
        "class A {",
        "}}}}{{{{",
        "\u{feff}class A {}",
        "class A { void m() { if (x) } }",
        "import ; package ; class { void () { try { } catch () { } finally { return; } } }",
        "@SuppressWarnings({\"a\", {\"b\", {\"mote\"}}}) class A {}",
        "record R(int x) { R { if (x < 0) throw new IllegalArgumentException(); } }",
        "enum E { A { void f() { try {} catch (Exception e) {} } }, B; private int unused; }",
        "class A { String s = \"\"\"\n  text\n  \"\"\"; }",
    ] {
        let a = run(&plugin, text);
        assert!(
            a.findings.iter().all(|f| f.range.start_line >= 1),
            "{text:?}"
        );
    }
    let garbage: String = (0u32..5000)
        .map(|i| char::from_u32(i * 7919 % 0x2FFF).unwrap_or('x'))
        .collect();
    run(&plugin, &garbage);
}

#[test]
fn deep_nesting_does_not_overflow() {
    let plugin = JavaPlugin::new();
    let n = 3000;
    let blocks = format!(
        "class A {{ void m() {{ {} {} }} }}",
        "{".repeat(n),
        "}".repeat(n)
    );
    run(&plugin, &blocks);
    let ifs = format!(
        "class A {{ void m(boolean b) {{ {} x(); {} }} }}",
        "if (b) {".repeat(n),
        "}".repeat(n)
    );
    let a = run(&plugin, &ifs);
    assert_eq!(a.functions.len(), 1);
    assert_eq!(a.functions[0].max_nesting, n as u32);
    let parens = format!(
        "class A {{ int x = {}1{}; boolean b = x == {}0; }}",
        "(".repeat(n),
        ")".repeat(n),
        "-".repeat(n)
    );
    run(&plugin, &parens);
    let lambdas = format!("class A {{ Object o = {}null; }}", "x -> ".repeat(n));
    run(&plugin, &lambdas);
    let concat = format!(
        "class A {{ boolean b = (\"a\"{}) == \"b\"; }}",
        " + s".repeat(50_000)
    );
    run(&plugin, &concat);
    let classes = format!(
        "class A {{ {} }}",
        "class B { ".repeat(500) + &"}".repeat(500)
    );
    run(&plugin, &classes);
    let annotations = format!(
        "@SuppressWarnings({}\"x\"{}) class A {{}}",
        "{".repeat(n),
        "}".repeat(n)
    );
    run(&plugin, &annotations);
}

#[test]
fn huge_single_line_stays_linear() {
    let plugin = JavaPlugin::new();
    let body = "try { f(); } catch (Exception e) { } /* c */ ".repeat(20_000);
    let text = format!("class A {{ void m() {{ {body} }} }}");
    let t = Instant::now();
    let a = run(&plugin, &text);
    assert_eq!(a.comments.len(), 20_000);
    assert!(a.findings.len() >= 20_000);
    assert!(
        t.elapsed() < Duration::from_secs(20),
        "took {:?}",
        t.elapsed()
    );
}

#[test]
fn parse_deadline_reports_a_syntax_error() {
    let plugin = JavaPlugin::new().with_parse_timeout(Duration::from_nanos(1));
    let text = "class A { void m() { int x = 1; } }\n".repeat(50_000);
    let a = run(&plugin, &text);
    assert_eq!(a.syntax_errors, 1);
    assert!(a.findings.is_empty() && a.functions.is_empty());
    // The thread-local parser recovers for the next file.
    let ok = run(
        &JavaPlugin::new(),
        "class B { void m() { try { } catch (Exception e) { } } }",
    );
    assert_eq!(ok.syntax_errors, 0);
    assert_eq!(
        ok.findings
            .iter()
            .filter(|f| f.rule_id == "java/empty-catch-block")
            .count(),
        1
    );
}

#[test]
fn syntax_errors_are_counted() {
    let a = run(&JavaPlugin::new(), "class A { void m() { int x = ; } }");
    assert!(a.syntax_errors >= 1);
}

#[test]
fn columns_match_core_line_index() {
    let text = "class Ä {\n  // ünïcödé comment\n  void m() { try { } catch (Exception é) { } /* ✓ */ }\n}\n";
    let a = run(&JavaPlugin::new(), text);
    let idx = LineIndex::new(text);
    for c in &a.comments {
        let offset = text.match_indices(&c.text).next().unwrap().0 as u32;
        assert_eq!(
            (c.start_line, c.start_col),
            idx.line_col(offset),
            "{}",
            c.text
        );
    }
    let f = a
        .findings
        .iter()
        .find(|f| f.rule_id == "java/empty-catch-block")
        .expect("empty-catch-block finding");
    let offset = text.find("catch").unwrap() as u32;
    assert_eq!(
        (f.range.start_line, f.range.start_col),
        idx.line_col(offset)
    );
    assert_eq!(
        a.comments.iter().map(|c| c.trailing).collect::<Vec<_>>(),
        vec![false, true]
    );
}

#[test]
fn parse_budget_reports_a_syntax_error() {
    let text = "class A { void m() { int x = 1; } }\n".repeat(2_000);
    let tight = JavaPlugin::new().with_parse_budget(ParseBudget {
        base: 10,
        per_kib: 0,
    });
    for _ in 0..3 {
        let a = run(&tight, &text);
        assert_eq!(a.syntax_errors, 1);
        assert!(a.findings.is_empty() && a.functions.is_empty() && a.comments.is_empty());
    }
    let a = run(&JavaPlugin::new(), &text);
    assert_eq!((a.syntax_errors, a.functions.len()), (0, 2_000));
}

/// A realistic, error-free Java source of roughly `bytes` bytes.
fn big_source(bytes: usize) -> String {
    let mut s = String::from(
        "package p;\n\nimport java.util.*;\n\npublic class Big {\n    private final Map<String, List<Integer>> index = new HashMap<>();\n",
    );
    let mut i = 0;
    while s.len() < bytes {
        s.push_str(&format!(
            r#"
    /** Method {i}. */
    public int method{i}(String name, List<Integer> values, int limit) {{
        int total = 0;
        for (int v : values) {{
            if (v > limit && name != null) {{
                total += v * 2 - limit;
            }} else if (name == "x{i}") {{
                total -= 1;
            }}
        }}
        try {{
            index.computeIfAbsent(name, k -> new ArrayList<>()).add(total);
        }} catch (Exception e) {{
        }}
        String label = "item-" + name + ":" + total + (limit > 3 ? "!" : "?");
        System.out.println(label);
        return total > 1000 ? 1000 : total;
    }}
"#
        ));
        i += 1;
    }
    s.push_str("}\n");
    s
}

/// Regression: parsing used a 2 s wall-clock deadline, so large files analysed on an oversubscribed
/// machine came out as a bare syntax error (and the engine cached that). The work budget is
/// deterministic: the result is the same on 1 thread and on 48 threads competing for the CPU.
/// `MOTE_LOAD_KIB` sets the file size (default 256; the original report used 1024 in release).
#[test]
fn results_do_not_depend_on_thread_count_or_load() {
    use rayon::prelude::*;
    let kib: usize = std::env::var("MOTE_LOAD_KIB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);
    let text = big_source(kib * 1024);
    let plugin = JavaPlugin::new();
    let rules = ruleset_all(&plugin);
    let file = SourceFile {
        path: "src/Big.java",
        text: &text,
        is_test: false,
    };
    let one = plugin.analyze(&file, &rules);
    assert_eq!(one.syntax_errors, 0);
    assert!(one.functions.len() > 100 && !one.findings.is_empty());
    let expected = format!("{one:?}");
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(48)
        .build()
        .expect("pool");
    let many: Vec<String> = pool.install(|| {
        (0..48)
            .into_par_iter()
            .map(|_| format!("{:?}", plugin.analyze(&file, &rules)))
            .collect()
    });
    for got in &many {
        assert!(*got == expected, "result differs under load");
    }
}

/// tree-sitter parses with a heap-allocated stack and the plugin walks iteratively: 20 000 levels
/// of nesting fit a 1 MiB stack (debug and release builds). Rules run down to the depth cap
/// (10 000 tree levels); the rest of such a file is cut off and reported as a syntax error.
#[test]
fn very_deep_nesting_does_not_overflow() {
    let plugin = JavaPlugin::new();
    let n = 20_000;
    let cases = [
        format!(
            "class A {{ void m() {{ {} {} }} }}",
            "{".repeat(n),
            "}".repeat(n)
        ),
        format!(
            "class A {{ void m(boolean b) {{ {} x(); {} }} }}",
            "if (b) {".repeat(n),
            "}".repeat(n)
        ),
        format!("class A {{ int x = {}1{}; }}", "(".repeat(n), ")".repeat(n)),
        format!(
            "class A {{ int x = f({}1{}); }}",
            "f(".repeat(n),
            ")".repeat(n)
        ),
        format!("class A {{ Object o = {}null; }}", "x -> ".repeat(n)),
        format!("class A {{ int x = {}0; }}", "a ? b : ".repeat(n)),
        format!("class A {{ Object o = {}x; }}", "(A) ".repeat(n)),
        format!("class A {{ int x = {}1; }}", "- ~ !".repeat(n)),
        format!("class A {{ Object o = a{}; }}", ".b().c".repeat(n)),
        format!("class A {{ int x = a{}; }}", " + a * b".repeat(n)),
        format!(
            "class A {{ int[][] x = {}1{}; }}",
            "{".repeat(n),
            "}".repeat(n)
        ),
        format!("class A {{ {}X{} x; }}", "List<".repeat(n), ">".repeat(n)),
        format!("class A {{ {} }}", "class B { ".repeat(n) + &"}".repeat(n)),
        format!(
            "class A {{ Object o = {}null{}; }}",
            "new Object() { Object f() { return ".repeat(2_000),
            "; } }".repeat(2_000)
        ),
    ];
    for text in &cases {
        let t = Instant::now();
        let a = run(&plugin, text);
        // Deeper than the walk accepts: cut off, deterministically.
        assert_eq!(a.syntax_errors, 1, "{:?}", &text[..60]);
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "{:?}… took {:?}",
            &text[..60],
            t.elapsed()
        );
    }
}

/// Regression: rules and symbols look at ancestors, so a file of nothing but nesting made the
/// walk quadratic (20 000 nested `if`s took ~2 minutes in a debug build). Trees deeper than the
/// cap are reported as one syntax error; everything below it is analysed as usual.
#[test]
fn pathological_depth_stays_linear() {
    let plugin = JavaPlugin::new();
    let n = 125_000; // ~1 MB
    let text = format!(
        "class A {{ void m(boolean b) {{ {} x(); {} }} }}",
        "if (b) {".repeat(n),
        "}".repeat(n)
    );
    let t = Instant::now();
    let a = run(&plugin, &text);
    assert_eq!(a.syntax_errors, 1);
    assert!(a.findings.is_empty() && a.functions.is_empty());
    assert!(
        t.elapsed() < Duration::from_secs(30),
        "took {:?}",
        t.elapsed()
    );
    // Well below the cap: analysed.
    let ok = format!(
        "class A {{ void m(boolean b) {{ {} x(); {} }} }}",
        "if (b) {".repeat(2_000),
        "}".repeat(2_000)
    );
    let a = run(&plugin, &ok);
    assert_eq!(a.syntax_errors, 0);
    assert_eq!(a.functions[0].max_nesting, 2_000);
}

/// Regression: rules and helpers that walked to the root, scanned the whole source or compared
/// every pair per node were quadratic in nesting depth or in the number of members (each case
/// took from seconds to minutes in a release build). Analysed in a debug build, every case now
/// takes well under the budget.
#[test]
fn machine_made_shapes_stay_linear() {
    let plugin = JavaPlugin::new();
    let method = |body: String| {
        format!("class A {{\n  int m(boolean b, int a, Object o, String s) {{\n{body}\n    return 0;\n  }}\n}}\n")
    };
    let cases: Vec<(&str, String)> = vec![
        (
            // synchronized-on-reassigned-field visited each block's body; symbols walked up.
            "nested synchronized",
            method(format!(
                "{} a++; {}",
                "synchronized (o) {".repeat(3_000),
                "}".repeat(3_000)
            )),
        ),
        (
            // for-loop-counter-misuse collected the counters of every enclosing loop.
            "nested for loops",
            method(format!(
                "{} a++; {}",
                "for (int i = 0; i < a; i++) {".repeat(1_500),
                "}".repeat(1_500)
            )),
        ),
        (
            // redundant-parentheses walked to the statement from every pair.
            "nested parentheses",
            format!(
                "class A {{ int x = {}1{}; }}",
                "(".repeat(9_000),
                ")".repeat(9_000)
            ),
        ),
        (
            // misleading-indentation followed the chain from every `if` of it.
            "else-if chain",
            method(format!(
                "    if (a == 0) {{ a++; }}{}",
                (1..=6_000)
                    .map(|i| format!(" else if (a == {i}) {{ a--; }}"))
                    .collect::<String>()
            )),
        ),
        (
            // confusing-member-name compared every member with every earlier one.
            "class body of statements",
            format!("class A {{\n  {}\n}}\n", "a;".repeat(60_000)),
        ),
        (
            // field-could-be-local scanned the source (and the method) once per field; the
            // resolver searched the fields one by one.
            "fields used in one method",
            format!(
                "class A {{\n{}  void m() {{\n{}  }}\n}}\n",
                (0..20_000)
                    .map(|i| format!("  private int f{i};\n"))
                    .collect::<String>(),
                (0..20_000)
                    .map(|i| format!("    f{i} = f{i} + 1;\n"))
                    .collect::<String>()
            ),
        ),
        (
            // overloads-not-adjacent searched a list of the names seen per method.
            "many methods",
            format!(
                "class A {{\n{}}}\n",
                (0..40_000)
                    .map(|i| format!("  void m{i}() {{ }}\n"))
                    .collect::<String>()
            ),
        ),
        (
            // missing-override searched a list of the classes seen per method.
            "enum constants with bodies",
            format!(
                "enum E {{ {} }}\n",
                (0..40_000)
                    .map(|i| format!("C{i} {{ void f() {{}} }}, "))
                    .collect::<String>()
            ),
        ),
        (
            // mocks-not-initialized scanned the whole source once per class.
            "classes with mocks",
            format!(
                "import org.mockito.Mock;\n{}",
                (0..20_000)
                    .map(|i| format!("class C{i} {{ @Mock Object m; void t() {{ }} }}\n"))
                    .collect::<String>()
            ),
        ),
    ];
    for (name, text) in &cases {
        let t = Instant::now();
        run(&plugin, text);
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "{name}: {:?}",
            t.elapsed()
        );
    }
}

/// Regression (found by mutation fuzzing): names the parser could not find are empty, and
/// searching the text for an empty name never advanced — `<>` in a method declaration hung the
/// analysis forever. Analysed with a deadline.
#[test]
fn missing_names_do_not_hang() {
    let cases = [
        "class A {\n  public static <>File f(String s) { return null; }\n}\n",
        "class A {\n  <T, > void g() { }\n  static <> void h() { }\n}\n",
        "class A {\n  private void (int a) { log.debug(\"x\" + a); }\n  void m() { (1); }\n}\n",
        "class ATest {\n  @Test void () { try { f(); } catch (Exception e) { fail(); } }\n}\n",
    ];
    for text in cases {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .stack_size(8 << 20)
            .spawn(move || {
                let plugin = JavaPlugin::new();
                let rules = ruleset_all(&plugin);
                let file = SourceFile {
                    path: "src/p/ATest.java",
                    text,
                    is_test: true,
                };
                plugin.analyze(&file, &rules);
                let _ = tx.send(());
            })
            .expect("spawn");
        assert!(
            rx.recv_timeout(Duration::from_secs(30)).is_ok(),
            "analysis hung on {text:?}"
        );
    }
}
