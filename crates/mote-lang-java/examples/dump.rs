//! Developer tool: analyze one Java file with every rule enabled and print the results.
//!
//! ```text
//! cargo run -p mote-lang-java --example dump -- path/to/File.java              # findings, metrics, comments
//! cargo run -p mote-lang-java --example dump -- path/to/File.java --tree       # also the syntax tree
//! ```
//!
//! The tree view prints each node as `kind [field] line:col` — the names rules use in
//! `kinds()` and `child_by_field_name(..)`. With `MOTE_RULE_TIMINGS=1` it also prints the
//! time spent in each analysis phase and in the most expensive rules.

use mote_core::testing::ruleset_all;
use mote_core::{LanguagePlugin, SourceFile};
use mote_lang_java::{timing, JavaPlugin};

fn print_tree(src: &str) {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .expect("language");
    let Some(tree) = parser.parse(src, None) else {
        return;
    };
    let mut cursor = tree.walk();
    let mut depth = 0usize;
    'walk: loop {
        let n = cursor.node();
        if n.is_named() {
            let field = cursor
                .field_name()
                .map(|f| format!(" [{f}]"))
                .unwrap_or_default();
            let leaf = if n.named_child_count() == 0 {
                let t = src.get(n.start_byte()..n.end_byte()).unwrap_or("");
                let t: String = t.chars().take(40).collect::<String>().replace('\n', "\\n");
                format!("  `{t}`")
            } else {
                String::new()
            };
            let p = n.start_position();
            println!(
                "{}{}{} {}:{}{}",
                "  ".repeat(depth),
                n.kind(),
                field,
                p.row + 1,
                p.column + 1,
                leaf
            );
        }
        if cursor.goto_first_child() {
            depth += 1;
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
            depth -= 1;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.iter().find(|a| !a.starts_with("--")) else {
        eprintln!("usage: dump <File.java> [--tree] [--test]");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(path).expect("read file");
    if args.iter().any(|a| a == "--tree") {
        print_tree(&text);
        println!();
    }
    let plugin = JavaPlugin::new();
    let rules = ruleset_all(&plugin);
    let is_test = args.iter().any(|a| a == "--test");
    let a = plugin.analyze(
        &SourceFile {
            path,
            text: &text,
            is_test,
        },
        &rules,
    );
    println!("findings ({}):", a.findings.len());
    for f in &a.findings {
        println!(
            "  {}:{} {} — {} [{}]{}",
            f.range.start_line,
            f.range.start_col,
            f.rule_id,
            f.message,
            f.symbol.as_deref().unwrap_or("-"),
            f.value.map(|v| format!(" value={v}")).unwrap_or_default()
        );
    }
    println!("functions ({}):", a.functions.len());
    for m in &a.functions {
        println!(
            "  {} lines {}-{} cyclomatic={} cognitive={} nesting={} params={} statements={}",
            m.name,
            m.start_line,
            m.end_line,
            m.cyclomatic,
            m.cognitive,
            m.max_nesting,
            m.params,
            m.statements
        );
    }
    println!("comments: {}", a.comments.len());
    for c in &a.comments {
        let first = c.text.lines().next().unwrap_or("");
        println!(
            "  {}-{}:{}{} {}",
            c.start_line,
            c.end_line,
            c.start_col,
            if c.trailing { " trailing" } else { "" },
            first
        );
    }
    println!("suppressions: {:?}", a.suppressions);
    println!(
        "classes={} statements={} syntax_errors={}",
        a.classes, a.statements, a.syntax_errors
    );
    if timing::enabled() {
        print!("{}", timing::report(20));
    }
}
