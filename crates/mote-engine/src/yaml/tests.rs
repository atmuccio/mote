//! Tests of the YAML reader: the scalar rules, positions, error messages and limits.

use super::*;
use serde::Deserialize;
use serde_json::{json, Value as Json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn json(text: &str) -> Json {
    from_str::<Json>(text).unwrap_or_else(|e| panic!("{text:?}: {e}"))
}

fn err<T: serde::de::DeserializeOwned + std::fmt::Debug>(text: &str) -> Error {
    from_str::<T>(text).expect_err(text)
}

#[test]
fn plain_scalars_follow_the_core_schema() {
    for (text, want) in [
        ("a: true", json!(true)),
        ("a: False", json!(false)),
        ("a: TRUE", json!(true)),
        ("a: 12", json!(12)),
        ("a: -12", json!(-12)),
        ("a: +12", json!(12)),
        ("a: 0x1F", json!(31)),
        ("a: 0o17", json!(15)),
        ("a: 0b101", json!(5)),
        ("a: 1.5", json!(1.5)),
        ("a: 1e3", json!(1000.0)),
        ("a: ~", Json::Null),
        ("a: null", Json::Null),
        ("a:", Json::Null),
        // YAML 1.1 forms are strings.
        ("a: on", json!("on")),
        ("a: off", json!("off")),
        ("a: yes", json!("yes")),
        ("a: no", json!("no")),
        ("a: y", json!("y")),
        ("a: tRUE", json!("tRUE")),
        ("a: 012", json!("012")),
        ("a: 1_000", json!("1_000")),
        ("a: 2026-10-13", json!("2026-10-13")),
        ("a: 1:20", json!("1:20")),
        // Quoted scalars are strings.
        ("a: '12'", json!("12")),
        ("a: \"true\"", json!("true")),
        ("a: ''", json!("")),
        // Core schema tags.
        ("a: !!str 12", json!("12")),
        ("a: !!int '12'", json!(12)),
        ("a: !!float 1", json!(1.0)),
    ] {
        assert_eq!(json(text)["a"], want, "{text}");
    }
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Typed {
    #[serde(default)]
    s: Option<String>,
    #[serde(default)]
    n: Option<u32>,
    #[serde(default)]
    b: Option<bool>,
    #[serde(default)]
    f: Option<f64>,
    #[serde(default)]
    list: Vec<String>,
}

#[test]
fn typed_targets_read_scalars_as_before() {
    let t: Typed = from_str("s: 0.1\nn: 0x10\nb: True\nf: 3\nlist: [2024, yes, ~, 1.10]").unwrap();
    assert_eq!(
        t.s.as_deref(),
        Some("0.1"),
        "a plain scalar is read as written"
    );
    assert_eq!(t.n, Some(16));
    assert_eq!(t.b, Some(true));
    assert_eq!(t.f, Some(3.0));
    assert_eq!(t.list, ["2024", "yes", "~", "1.10"]);
    let t: Typed = from_str("s: ~\nn: null\nlist:").unwrap();
    assert_eq!((t.s, t.n, t.list.len()), (None, None, 0));
    // A quoted number is not a number; `yes` is not a boolean.
    assert_eq!(
        err::<Typed>("n: \"17\"").to_string(),
        "n: invalid type: string \"17\", expected u32 at line 1 column 4"
    );
    assert_eq!(
        err::<Typed>("b: yes").to_string(),
        "b: invalid type: string \"yes\", expected a boolean at line 1 column 4"
    );
    assert_eq!(
        err::<Typed>("n: 012").to_string(),
        "n: invalid type: string \"012\", expected u32 at line 1 column 4"
    );
    assert_eq!(
        err::<Typed>("list: [a, {b: 1}]").to_string(),
        "list[1]: invalid type: map, expected a string at line 1 column 11"
    );
    let e = err::<Typed>("x: 1");
    assert_eq!(
        e.to_string(),
        "unknown field `x`, expected one of `s`, `n`, `b`, `f`, `list`"
    );
    assert_eq!(e.location(), Some((1, 1)));
}

#[test]
fn errors_name_the_path_and_position_of_the_value() {
    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Outer {
        inner: BTreeMap<String, Vec<u8>>,
    }
    let e = err::<Outer>("inner:\n  a: [1, 2]\n  b: [3, x]\n");
    assert_eq!(
        e.to_string(),
        "inner.b[1]: invalid type: string \"x\", expected u8 at line 3 column 10"
    );
    assert_eq!(e.location(), Some((3, 10)));
    let e = err::<[u8; 2]>("[1, 2, 3]");
    assert_eq!(
        e.to_string(),
        "invalid length 3, expected sequence of 2 elements"
    );
}

#[test]
fn documents() {
    // No document: the target's empty value.
    assert_eq!(json(""), Json::Null);
    assert_eq!(json("# only a comment\n"), Json::Null);
    assert_eq!(from_str::<BTreeMap<String, u8>>("").unwrap().len(), 0);
    let e = err::<Json>("a: 1\n---\nb: 2\n");
    assert_eq!(
        e.to_string(),
        "deserializing from YAML containing more than one document is not supported"
    );
    // One document with markers is fine.
    assert_eq!(json("---\na: 1\n...\n"), json!({"a": 1}));
    // A byte order mark is not part of the text.
    let e = err::<Typed>("\u{feff}s: a\nx: 1\n");
    assert_eq!(e.location(), Some((2, 1)));
    assert_eq!(json("\u{feff}a: 1\nb: 2\n"), json!({"a": 1, "b": 2}));
}

#[test]
fn anchors_aliases_and_merge_keys() {
    assert_eq!(
        json("a: &x [1, 2]\nb: *x\n"),
        json!({"a": [1, 2], "b": [1, 2]})
    );
    // `<<` is an ordinary key.
    assert_eq!(
        json("a: &x {k: 1}\nb:\n  <<: *x\n  j: 2\n"),
        json!({"a": {"k": 1}, "b": {"<<": {"k": 1}, "j": 2}})
    );
    let e = err::<Json>("a: *nope\n");
    assert_eq!(e.to_string(), "unknown anchor at line 1 column 4");
}

#[test]
fn duplicate_keys() {
    // A typed map keeps the last value, as before; a generic value refuses them.
    let m: BTreeMap<String, u8> = from_str("a: 1\na: 2\n").unwrap();
    assert_eq!(m["a"], 2);
    let doc = load("x:\n  a: 1\n  b: 2\n  a: 3\n").unwrap();
    let e = check_generic(&doc).unwrap_err();
    assert_eq!(
        e.to_string(),
        "x: duplicate entry with key \"a\" at line 2 column 3"
    );
    // By value: `1` and `0x1` are the same key, `1` and `"1"` are not.
    assert!(check_generic(&load("1: a\n0x1: b\n").unwrap()).is_err());
    assert!(check_generic(&load("1: a\n'1': b\n").unwrap()).is_ok());
    // Integers beyond 64 bits are not generic values either.
    let e = check_generic(&load("a: 18446744073709551616\n").unwrap()).unwrap_err();
    assert!(e.to_string().contains("as u128"), "{e}");
}

#[test]
fn local_tags_select_enum_variants() {
    #[derive(Debug, Deserialize, PartialEq)]
    enum E {
        Unit,
        New(u8),
        Pair(u8, u8),
    }
    assert_eq!(from_str::<E>("Unit").unwrap(), E::Unit);
    assert_eq!(from_str::<E>("!New 3").unwrap(), E::New(3));
    assert_eq!(from_str::<E>("!Pair [1, 2]").unwrap(), E::Pair(1, 2));
    let e = err::<E>("Other");
    assert!(e.to_string().starts_with("unknown variant `Other`"), "{e}");
}

/// The line and column of the node at `path` (`a.0.b`; `^` reads a key).
fn at(text: &str, path: &str) -> (usize, usize) {
    let mut node = &load(text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        let (want_key, seg) = match seg.strip_prefix('^') {
            Some(s) => (true, s),
            None => (false, seg),
        };
        node = match &node.value {
            Value::Seq(items) => &items[seg.parse::<usize>().unwrap()],
            Value::Map(entries) => {
                let (k, v) = entries
                    .iter()
                    .find(|(k, _)| key_text(k) == seg)
                    .unwrap_or_else(|| panic!("no {seg} in {text:?}"));
                if want_key {
                    k
                } else {
                    v
                }
            }
            _ => panic!("{path} in {text:?}"),
        };
    }
    (node.mark.line, node.mark.column)
}

#[test]
fn nodes_start_where_libyaml_put_them() {
    // Keys, values and flow collections.
    let doc = "a: 1\nb: { c: x, d: [y] }\n";
    assert_eq!(at(doc, "^a"), (1, 1));
    assert_eq!(at(doc, "a"), (1, 4));
    assert_eq!(at(doc, "b"), (2, 4));
    assert_eq!(at(doc, "b.^d"), (2, 12));
    assert_eq!(at(doc, "b.d.0"), (2, 16));
    // A block mapping at its first key, a block sequence at its first `-`.
    let doc = "a:\n  b: 1\nc:\n  - x\nd:\n- y\n";
    assert_eq!(at(doc, "a"), (2, 3));
    assert_eq!(at(doc, "c"), (4, 3));
    assert_eq!(at(doc, "d"), (6, 1));
    assert_eq!(at(doc, "d.0"), (6, 3));
    // Properties first.
    let doc = "a: &x { b: 1 }\nc: !!str 2\nd: &y\n  e: 3\nf: *x\n";
    assert_eq!(at(doc, "a"), (1, 4));
    assert_eq!(at(doc, "c"), (2, 4));
    assert_eq!(at(doc, "d"), (3, 4));
    // An alias reads as its anchored node, where that is.
    assert_eq!(at(doc, "f"), (1, 4));
    assert_eq!(at(doc, "f.b"), (1, 12));
    // Block scalars at their indicator.
    let doc = "a: |\n  text\nb: >-  # folded\n\n  more\n";
    assert_eq!(at(doc, "a"), (1, 4));
    assert_eq!(at(doc, "b"), (3, 4));
    // Empty values: after `:` or `-` in block style, at the next token in flow style.
    let doc = "a:\n  b:\nc: { d: , e: 1 }\nf:\n  -\n  - g\n";
    assert_eq!(at(doc, "a.b"), (2, 5));
    assert_eq!(at(doc, "c.d"), (3, 9));
    assert_eq!(at(doc, "f.0"), (5, 4));
    // Columns count characters; CRLF is one line break; a byte order mark is not a column.
    assert_eq!(at("x: é€😀\nb: { c: é, d: 2 }\n", "b.^d"), (2, 12));
    assert_eq!(at("a: 1\r\nb:\r\n  c: 2\r\n", "b.c"), (3, 6));
    assert_eq!(at("\u{feff}a: 1\n", "^a"), (1, 1));
}

#[test]
fn text_that_is_not_yaml() {
    // An unclosed flow collection is reported where the text ends, as libyaml did.
    let e = load("rules: [a, b\n").unwrap_err();
    assert_eq!(
        e.to_string(),
        "did not find expected ',' or ']' at line 2 column 1, while parsing a flow sequence at line 1 column 8"
    );
    assert_eq!(e.location(), Some((2, 1)));
    let e = load("a: [1, 2").unwrap_err();
    assert_eq!(e.location(), Some((2, 1)));
    let e = load("gate:\n\tfail_on: LOW\n").unwrap_err();
    assert_eq!(
        e.to_string(),
        "found character that cannot start any token at line 2 column 1, while scanning for the next token"
    );
    let e = load("exclude: [a]\n\0").unwrap_err();
    assert_eq!(
        e.to_string(),
        "control characters are not allowed at position 13"
    );
    // What comes before the error is still read.
    let (partial, e) = load_partial("a: 1\nb: [1, 2\n");
    assert!(e.is_some());
    assert!(
        key_text(&partial).starts_with("{a: 1, b: [1"),
        "{partial:?}"
    );
    // Tabs as separators and under-indented flow collections are read, as libyaml did.
    assert_eq!(json("a:\tx\nb: [1,\n2]\n"), json!({"a": "x", "b": [1, 2]}));
}

fn quickly<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    assert!(
        start.elapsed() < Duration::from_secs(20),
        "{what} took {:?}",
        start.elapsed()
    );
    out
}

#[test]
fn hostile_documents_are_refused_quickly() {
    // "Billion laughs": nine levels of nine aliases.
    let mut doc = String::from("a0: &a0 [lol, lol, lol, lol, lol, lol, lol, lol, lol]\n");
    for i in 1..10 {
        let p = i - 1;
        doc.push_str(&format!(
            "a{i}: &a{i} [*a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}]\n"
        ));
    }
    let e = quickly("billion laughs", || load(&doc).unwrap_err());
    assert!(
        e.to_string().starts_with(
            "repetition limit exceeded (aliases expand to more than 100000 values) at line "
        ),
        "{e}"
    );
    // An alias inside its own anchor.
    let e = load("a: &a [*a]\n").unwrap_err();
    assert!(e.to_string().starts_with("recursion limit exceeded"), "{e}");
    // Deep nesting, flow and block.
    let flow = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    let e = quickly("deep flow", || load(&flow).unwrap_err());
    assert!(e.to_string().contains("recursion limit exceeded"), "{e}");
    let block = format!("{}x\n", "- ".repeat(100_000));
    let e = quickly("deep block", || load(&block).unwrap_err());
    assert!(e.to_string().contains("recursion limit exceeded"), "{e}");
    // Nesting the parser allows but deserialization does not go into.
    let nested = format!("{}{}", "[".repeat(200), "]".repeat(200));
    let e = from_str::<Json>(&nested).unwrap_err();
    assert!(e.to_string().starts_with("recursion limit exceeded"), "{e}");
    // A text over the size limit is refused before parsing.
    let huge = format!("a: {}\n", "x".repeat(MAX_BYTES));
    let e = quickly("huge scalar", || load(&huge).unwrap_err());
    assert!(
        e.to_string()
            .starts_with("the text is too large to read as YAML"),
        "{e}"
    );
    // As are more values than the limit.
    let many = format!("[{}]", "0,".repeat(MAX_NODES));
    let e = quickly("many values", || load(&many).unwrap_err());
    assert!(
        e.to_string()
            .starts_with("the document is too large to read as YAML"),
        "{e}"
    );
    // A long key is not a key (YAML's 1024-character limit on implicit keys).
    assert!(load(&format!("{}: 1\n", "k".repeat(2000))).is_err());
}
