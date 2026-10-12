//! Pathological nesting: the parser and the semantic builder recurse, so deeply nested input
//! must be reported as a syntax error instead of overflowing the stack (which aborts the
//! process). Every analysis here runs with all rules enabled on a thread with exactly the stack
//! the plugin was told about (the default 2 MiB unless a test says otherwise); run the tests in
//! debug and release builds (`cargo test -p mote-lang-js --release --test robustness`).

use mote_core::testing::ruleset_all;
use mote_core::{FileAnalysis, LanguagePlugin, SourceFile};
use mote_lang_js::{JsPlugin, DEFAULT_ANALYSIS_STACK};
use std::time::{Duration, Instant};

fn run(path: &str, text: &str) -> FileAnalysis {
    run_on(DEFAULT_ANALYSIS_STACK, JsPlugin::new(), path, text)
}

/// Analyse on a thread with `stack` bytes of stack (the size `plugin` was configured with).
fn run_on(stack: usize, plugin: JsPlugin, path: &str, text: &str) -> FileAnalysis {
    let rules = ruleset_all(&plugin);
    let file = SourceFile {
        path,
        text,
        is_test: false,
    };
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(stack)
            .spawn_scoped(s, || plugin.analyze(&file, &rules))
            .expect("spawn")
            .join()
            .expect("analysis panicked")
    })
}

fn rejected(path: &str, text: &str) {
    let a = run(path, text);
    assert_eq!(
        a.syntax_errors,
        1,
        "{path}: {:?}…",
        &text[..40.min(text.len())]
    );
    assert!(a.findings.is_empty() && a.functions.is_empty() && a.comments.is_empty());
}

fn analysed(path: &str, text: &str) -> FileAnalysis {
    let a = run(path, text);
    assert_eq!(
        a.syntax_errors,
        0,
        "{path}: {:?}…",
        &text[..40.min(text.len())]
    );
    assert!(!a.functions.is_empty(), "{path}: no functions");
    a
}

/// The inputs that used to crash the process.
#[test]
fn reported_inputs_are_rejected() {
    // 600 nested parentheses (~5 KB).
    rejected(
        "a.ts",
        &format!("x = {}1{};", "(".repeat(600), ")".repeat(600)),
    );
    // A 2000-call builder chain.
    rejected("a.ts", &format!("x = a{};", ".b().c()".repeat(1_000)));
    // A 4000-term `+` chain.
    rejected("a.js", &format!("x = a{};", " + a".repeat(3_999)));
}

/// Real code stays far below the limit; the largest inputs of each shape that the estimate
/// accepts are analysed normally, and fit a 2 MiB stack.
#[test]
fn nesting_below_the_limit_is_analysed() {
    let f = |body: String| format!("function f(a, b) {{\n  return {body};\n}}\n");
    analysed(
        "a.ts",
        &f(format!("{}a{}", "(".repeat(200), ")".repeat(200))),
    );
    analysed(
        "a.ts",
        &f(format!("{}a{}", "[".repeat(200), "]".repeat(200))),
    );
    analysed(
        "a.ts",
        &f(format!("{}1{}", "{a: ".repeat(150), "}".repeat(150))),
    );
    analysed(
        "a.ts",
        &f(format!("{}0{}", "f(".repeat(180), ")".repeat(180))),
    );
    analysed("a.ts", &f(format!("a{}", ".b().c()".repeat(400))));
    analysed("a.ts", &f(format!("a{}", ".b".repeat(1_600))));
    analysed("a.ts", &f(format!("a{}", " + a".repeat(1_600))));
    analysed("a.ts", &f(format!("a{}", " && b".repeat(1_600))));
    analysed("a.ts", &f(format!("{}0", "a ? b : ".repeat(300))));
    analysed("a.ts", &f(format!("{}a", "!".repeat(600))));
    analysed("a.ts", &f(format!("{}a", "typeof ".repeat(600))));
    analysed("a.ts", &f(format!("{}A", "new ".repeat(600))));
    analysed("a.ts", &f(format!("{}1", "() => ".repeat(600))));
    analysed("a.ts", &f(format!("{}a", "<A>".repeat(600))));
    analysed(
        "a.ts",
        &f(format!("{}1{}", "`${".repeat(200), "}`".repeat(200))),
    );
    analysed(
        "a.ts",
        &f(format!("{}a{}", "[...".repeat(200), "]".repeat(200))),
    );
    analysed(
        "a.tsx",
        &f(format!("{}{}", "<a><br/>".repeat(400), "</a>".repeat(400))),
    );
    analysed(
        "a.ts",
        &format!(
            "let x: {}X{};\nfunction f() {{}}\n",
            "Array<".repeat(200),
            ">".repeat(200)
        ),
    );
    analysed(
        "a.ts",
        &format!("{}x();{}", "function f() {".repeat(150), "}".repeat(150)),
    );
    analysed(
        "a.ts",
        &format!("{}x();{}", "f(() => {".repeat(90), "});".repeat(90)),
    );
    analysed(
        "a.ts",
        &format!("{}{}", "class A { m() {".repeat(90), "}}".repeat(90)),
    );
    analysed(
        "a.js",
        &format!("function f(a) {{ {} x(); }}", "if (a) ".repeat(380)),
    );
    // An `else if` chain of 300 branches.
    analysed(
        "a.js",
        &format!(
            "function f(a) {{ if (a === 0) {{}}{} }}",
            (1..300)
                .map(|i| format!(" else if (a === {i}) {{ g(); }}"))
                .collect::<String>()
        ),
    );
}

/// Every nesting shape, far beyond the limit: rejected quickly, never overflowing.
#[test]
fn deep_nesting_of_every_shape_is_rejected() {
    let n = 20_000;
    let cases: Vec<(&str, String)> = vec![
        ("a.ts", format!("x = {}1{};", "(".repeat(n), ")".repeat(n))),
        ("a.ts", format!("x = {}1{};", "(".repeat(n), "")),
        ("a.ts", format!("x = {}1{};", "[".repeat(n), "]".repeat(n))),
        (
            "a.ts",
            format!("x = {}1{};", "{a:".repeat(n), "}".repeat(n)),
        ),
        ("a.ts", format!("{}{}", "{".repeat(n), "}".repeat(n))),
        ("a.js", format!("{} x();", "if (a) ".repeat(n))),
        ("a.js", format!("{}x();", "for(;;) ".repeat(n))),
        ("a.js", format!("{}x();", "l: ".repeat(n))),
        (
            "a.ts",
            format!("{}{}", "function f() {".repeat(n), "}".repeat(n)),
        ),
        ("a.ts", format!("x = {}1;", "() => ".repeat(n))),
        ("a.ts", format!("x = {}1{};", "f(".repeat(n), ")".repeat(n))),
        ("a.ts", format!("x = a{};", ".b()".repeat(n))),
        ("a.ts", format!("x = a{};", "?.b".repeat(n))),
        ("a.ts", format!("x = a{};", " + a".repeat(n))),
        ("a.ts", format!("x = a{};", "!".repeat(n))),
        ("a.ts", format!("x = {}0;", "a ? b : ".repeat(n))),
        ("a.ts", format!("{}0;", "a = ".repeat(n))),
        ("a.ts", format!("x = {}a;", "- ".repeat(n))),
        ("a.ts", format!("x = {}A;", "new ".repeat(n))),
        // Prefix type assertions (regression: not charged, so 1 MB of them overflowed).
        ("a.ts", format!("x = {}a;", "<A>".repeat(n))),
        ("a.ts", format!("x = {}a;", "<A<B>>".repeat(n))),
        (
            "a.ts",
            format!("x = {}1{};", "`${".repeat(n), "}`".repeat(n)),
        ),
        (
            "a.tsx",
            format!("x = {}{};", "<a>".repeat(n), "</a>".repeat(n)),
        ),
        (
            "a.tsx",
            format!("x = {}{};", "<a><b/>".repeat(n), "</a>".repeat(n)),
        ),
        (
            "a.tsx",
            format!("x = {}{};", "<a>{".repeat(n), "}</a>".repeat(n)),
        ),
        (
            "a.ts",
            format!("let x: {}X{};", "Array<".repeat(n), ">".repeat(n)),
        ),
        (
            "a.ts",
            format!("let x: {}X{};", "{a:".repeat(n), "}".repeat(n)),
        ),
        (
            "a.ts",
            format!("type T = {}X;", "A extends B ? C : ".repeat(n)),
        ),
        (
            "a.ts",
            format!("{}{}", "class A { m() {".repeat(n), "}}".repeat(n)),
        ),
        (
            "a.ts",
            format!("{}{}", "f(() => {".repeat(n), "});".repeat(n)),
        ),
        (
            "a.ts",
            format!("const {}x{} = y;", "{a:".repeat(n), "}".repeat(n)),
        ),
    ];
    for (path, text) in &cases {
        let t = Instant::now();
        rejected(path, text);
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "{:?}… took {:?}",
            &text[..40],
            t.elapsed()
        );
    }
    // A whole megabyte of nesting is rejected by the linear pre-scan in no time.
    let t = Instant::now();
    rejected("a.js", &"(".repeat(1 << 20));
    rejected("a.js", &format!("x = a{};", ".b()".repeat(1 << 18)));
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
}

/// Nesting hidden in strings, comments, regular expressions and template text does not count.
#[test]
fn nesting_in_literals_and_comments_is_ignored() {
    let deep = "(".repeat(5_000);
    analysed(
        "a.ts",
        &format!(
            "// {deep}\n/* {deep} */\nconst s = '{deep}';\nconst t = `{deep}`;\nconst r = /\\({deep}/;\nfunction f() {{ return s + t + r; }}\n"
        ),
    );
}

/// A host that declares a larger stack gets deeper nesting analysed (the limit scales with the
/// stack), still safely: the depth accepted stays well below what overflows that stack. (Up to
/// the nesting analysed on any stack, `MAX_NESTING`: see `bracket_nesting_has_a_cap`.)
#[test]
fn declared_stack_scales_the_limit() {
    let stack = 32 << 20;
    let parens = format!(
        "function f() {{ return {}1{}; }}",
        "(".repeat(1_500),
        ")".repeat(1_500)
    );
    let chain = format!("function f() {{ return a{}; }}", ".b().c()".repeat(5_000));
    for text in [&parens, &chain] {
        // Rejected on the default 2 MiB…
        let a = run("a.ts", text);
        assert_eq!(a.syntax_errors, 1);
        // …analysed on a declared 32 MiB.
        let a = run_on(
            stack,
            JsPlugin::new().with_analysis_stack(stack),
            "a.ts",
            text,
        );
        assert_eq!(a.syntax_errors, 0);
        assert_eq!(a.functions.len(), 1);
    }
    // Far beyond even that: rejected without overflowing.
    let deep = format!("x = {}1{};", "(".repeat(50_000), ")".repeat(50_000));
    let a = run_on(
        stack,
        JsPlugin::new().with_analysis_stack(stack),
        "a.ts",
        &deep,
    );
    assert_eq!(a.syntax_errors, 1);
}

/// Regular expressions are parsed by rules, not by the JavaScript parser, so the nesting estimate
/// does not see inside them (and must not: real patterns are long). Every regex parse is guarded
/// instead (regression: `/[[[…]]]/v` and `new RegExp("((…")` overflowed the stack).
#[test]
fn deeply_nested_regular_expressions_do_not_overflow() {
    let n = 200_000;
    let cases = [
        format!(
            "function f(a) {{ return /{}a{}/.test(a); }}",
            "(".repeat(n),
            ")".repeat(n)
        ),
        format!(
            "function f(a) {{ return /{}a|b{}/.test(a); }}",
            "(".repeat(n),
            ")".repeat(n)
        ),
        format!(
            "function f(a) {{ return /{}a{}/v.test(a); }}",
            "[".repeat(n),
            "]".repeat(n)
        ),
        format!(
            "function f(a) {{ return /[{}a{}]/v.test(a); }}",
            "[".repeat(n),
            "]".repeat(n)
        ),
        format!(
            "function f(a) {{ return /{}a{}/.test(a); }}",
            "(?:".repeat(n),
            ")*".repeat(n)
        ),
        format!(
            "function f(a) {{ return /{}a{}/.test(a); }}",
            "(?<=".repeat(n),
            ")".repeat(n)
        ),
        format!(
            "function f() {{ return new RegExp('{}a{}'); }}",
            "(".repeat(n),
            ")".repeat(n)
        ),
        format!(
            "function f() {{ return new RegExp('{}a{}', 'v'); }}",
            "[".repeat(n),
            "]".repeat(n)
        ),
        format!(
            "function f() {{ return RegExp(`{}a{}`); }}",
            "(".repeat(n),
            ")".repeat(n)
        ),
    ];
    for text in &cases {
        let t = Instant::now();
        let a = run("a.js", text);
        assert_eq!(a.syntax_errors, 0, "{:?}…", &text[..50]);
        assert_eq!(a.functions.len(), 1);
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "{:?}… took {:?}",
            &text[..50],
            t.elapsed()
        );
    }
}

/// Regression: nested parentheses holding an assignment (`(b = (b = …))`, `({a} = …)`,
/// `(a, b = …)`) may each be arrow function parameters, which the parser tries before parsing
/// them as an expression: quadratic in the nesting (20 000 levels took minutes and gigabytes).
/// Past a nesting no real code reaches they are rejected up front, even on a large stack.
#[test]
fn nested_parenthesized_assignments_are_rejected() {
    let stack = 256 << 20;
    let big = || JsPlugin::new().with_analysis_stack(stack);
    for unit in ["(b = ", "({a} = ", "([a] = ", "(a, b = ", "async (b = "] {
        let deep = format!("x = {}1{};\n", unit.repeat(20_000), ")".repeat(20_000));
        let t = Instant::now();
        let a = run_on(stack, big(), "a.ts", &deep);
        assert_eq!(a.syntax_errors, 1, "{unit}");
        assert!(
            t.elapsed() < Duration::from_secs(10),
            "{unit}: {:?}",
            t.elapsed()
        );
        // A realistic nesting is analysed.
        let shallow = format!(
            "function f() {{ return {}1{}; }}\n",
            unit.repeat(8),
            ")".repeat(8)
        );
        let a = run_on(stack, big(), "a.ts", &shallow);
        assert_eq!(a.syntax_errors, 0, "{unit}");
    }
}

/// Regression: rules that walked up from every node of a long chain (`no-loop-func` from every
/// callback of a `.then(…)` chain, `no-magic-numbers` from every literal of an `else if`
/// chain) or compared every link with every earlier one (`no-dupe-else-if`) were quadratic.
#[test]
fn machine_made_chains_stay_linear() {
    let stack = 256 << 20;
    let cases = [
        format!(
            "function f(a) {{ return a{}; }}\n",
            ".then(x => x)".repeat(20_000)
        ),
        // `no-self-compare` checked every operand of a comparison chain for purity. (`<=`: a
        // `<` may open type arguments, which the nesting pre-scan counts as levels.)
        format!(
            "function f(a, b) {{ return a{}; }}\n",
            " <= b".repeat(50_000)
        ),
        // `no-return-assign` walked up from every link of an assignment chain.
        format!(
            "function f(a) {{ a = {}1; return a; }}\n",
            "a = ".repeat(40_000)
        ),
        // `array-type` looked through every level of `T[][]…` from every level.
        format!(
            "function f(a: string{}) {{ return a; }}\n",
            "[]".repeat(50_000)
        ),
        format!(
            "function f(a) {{ if (a === 0) {{ g(); }}{} }}\n",
            (1..=3_500)
                .map(|i| format!(" else if (a === {i}) {{ g(); }}"))
                .collect::<String>()
        ),
    ];
    for text in &cases {
        let t = Instant::now();
        let a = run_on(
            stack,
            JsPlugin::new().with_analysis_stack(stack),
            "a.ts",
            text,
        );
        assert_eq!(a.syntax_errors, 0, "{:?}…", &text[..40]);
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "{:?}… took {:?}",
            &text[..40],
            t.elapsed()
        );
    }
}

/// Past 4 000 levels of nesting (brackets, and statements nested without braces) a file is
/// rejected whatever the stack: there, time grows with the square of the depth (the semantic
/// builder looks names up scope by scope; 20 000 nested `for … of` blocks took a second, so a
/// file of such nests took minutes).
#[test]
fn bracket_nesting_has_a_cap() {
    let stack = 256 << 20;
    let big = || JsPlugin::new().with_analysis_stack(stack);
    let nest = |n: usize| {
        format!(
            "function f(a) {{ {} g(x); {} }}\n",
            "for (const x of a) {".repeat(n),
            "}".repeat(n)
        )
    };
    let t = Instant::now();
    let a = run_on(stack, big(), "a.js", &nest(20_000));
    assert_eq!(a.syntax_errors, 1);
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    let a = run_on(stack, big(), "a.js", &nest(1_500));
    assert_eq!(a.syntax_errors, 0);
    // Small files are checked too (the stack alone would not look at them).
    let a = run_on(
        stack,
        big(),
        "a.js",
        &format!("x = {}1{};", "[".repeat(4_500), "]".repeat(4_500)),
    );
    assert_eq!(a.syntax_errors, 1);
    // Statements nested without braces count.
    let t = Instant::now();
    let a = run_on(
        stack,
        big(),
        "a.js",
        &format!("function f(a) {{ {} g(); }}\n", "with (a) ".repeat(20_000)),
    );
    assert_eq!(a.syntax_errors, 1);
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
}

/// Regex rules parse a pattern of at most `REGEX_MAX_LEN` bytes (regression: a 4 MB alternation
/// took gigabytes, a megabyte of named groups minutes).
#[test]
fn huge_regular_expressions_are_not_parsed() {
    let stack = 256 << 20;
    let cases = [
        format!(
            "function f(a) {{ return /a{}/.test(a); }}\n",
            "|a".repeat(2_000_000)
        ),
        format!(
            "function f(a) {{ return /{}/.test(a); }}\n",
            (0..100_000)
                .map(|i| format!("(?<g{i}>a)"))
                .collect::<String>()
        ),
    ];
    for text in &cases {
        let t = Instant::now();
        let a = run_on(
            stack,
            JsPlugin::new().with_analysis_stack(stack),
            "a.js",
            text,
        );
        assert_eq!(a.syntax_errors, 0);
        assert!(t.elapsed() < Duration::from_secs(20), "{:?}", t.elapsed());
    }
}
