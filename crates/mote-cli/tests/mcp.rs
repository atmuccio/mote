//! The MCP server: in-memory pipes for the tool flow, and the real binary over stdio.

mod common;

use common::*;
use mote_cli::mcp::{serve, Server};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

/// Feed newline-delimited messages through `serve` and return the decoded replies.
fn exchange(server: &mut Server, messages: &[Value]) -> Vec<Value> {
    let mut input = String::new();
    for m in messages {
        input.push_str(&m.to_string());
        input.push('\n');
    }
    let mut out = Vec::new();
    serve(Cursor::new(input.into_bytes()), &mut out, server).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn one(server: &mut Server, message: Value) -> Value {
    let mut r = exchange(server, &[message]);
    assert_eq!(r.len(), 1, "{r:?}");
    r.pop().unwrap()
}

fn call(server: &mut Server, id: u64, name: &str, args: Value) -> Value {
    let reply = one(
        server,
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}}),
    );
    assert_eq!(reply["id"], id);
    reply
}

/// The structured result of a successful tool call (checks the text mirror too).
fn ok(reply: &Value) -> Value {
    let result = &reply["result"];
    assert_eq!(result["isError"], false, "{reply}");
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(result["content"][0]["type"], "text");
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed, result["structuredContent"]);
    parsed
}

fn server(root: &std::path::Path) -> Server {
    Server::new(mote_cli::plugins::registry().unwrap(), root.to_path_buf())
}

#[test]
fn protocol_basics() {
    let d = tempfile::tempdir().unwrap();
    let mut s = server(d.path());
    let replies = exchange(
        &mut s,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": "two", "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list"}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}),
        ],
    );
    assert_eq!(
        replies.len(),
        5,
        "no reply to the notification: {replies:?}"
    );
    let init = &replies[0]["result"];
    assert_eq!(init["protocolVersion"], "2025-03-26");
    assert_eq!(init["serverInfo"]["name"], "mote");
    assert!(init["capabilities"]["tools"].is_object());
    assert!(init["instructions"]
        .as_str()
        .unwrap()
        .contains("work_units"));
    assert_eq!(replies[1]["id"], "two");
    assert_eq!(replies[1]["result"], json!({}));
    let tools = replies[2]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "scan",
            "summary",
            "work_units",
            "findings",
            "rule",
            "verify",
            "explain_config"
        ]
    );
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object", "{t}");
        assert!(t["description"].as_str().is_some_and(|d| !d.is_empty()));
    }
    assert_eq!(replies[3]["error"]["code"], -32601);
    assert_eq!(replies[4]["result"]["protocolVersion"], "2025-06-18");

    // Parse errors, invalid requests, unknown tools, bad arguments.
    let r = s.handle_line("{not json").unwrap();
    assert_eq!(r["error"]["code"], -32700);
    assert!(r["id"].is_null());
    let r = s.handle_line("[1, 2]").unwrap();
    assert_eq!(r["error"]["code"], -32600);
    let r = s.handle_line(r#"{"jsonrpc":"2.0","id":9}"#).unwrap();
    assert_eq!(r["error"]["code"], -32600);
    let r = call(&mut s, 10, "frobnicate", json!({}));
    assert_eq!(r["error"]["code"], -32602);
    let r = call(&mut s, 11, "findings", json!({"limit": 3}));
    assert_eq!(r["error"]["code"], -32602, "missing scan_id: {r}");
    let r = call(
        &mut s,
        12,
        "summary",
        json!({"scan_id": "s1", "group_by": "planet"}),
    );
    assert_eq!(r["error"]["code"], -32602, "{r}");
    let r = call(&mut s, 13, "summary", json!({"scan_id": "s99"}));
    assert_eq!(
        r["result"]["isError"], true,
        "unknown scan is a tool error: {r}"
    );
    let r = call(&mut s, 14, "rule", json!({"id": "tl/high-bugg"}));
    assert_eq!(r["result"]["isError"], true);
    assert!(r["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("tl/high-bug"));
    let rule = ok(&call(&mut s, 15, "rule", json!({"id": "tl/high-bug"})));
    assert_eq!(rule["severity"], "HIGH");
    assert!(rule["description"].as_str().unwrap().contains("HIGHBUG"));
    // Responses from the client and unknown notifications get no reply.
    assert!(s
        .handle_line(r#"{"jsonrpc":"2.0","id":7,"result":{}}"#)
        .is_none());
    assert!(s
        .handle_line(
            r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
        )
        .is_none());
}

#[test]
fn scan_units_findings_verify() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    init_repo(root);
    write(root, "src/a.tl", "fn a\n  HIGHBUG\n  LOUD\nend\n");
    write(root, "src/b.tl", "fn b\n  LOUD\n  LOUD\n  MAGIC\nend\n");
    write(root, "lib/c.tl", "fn c\n  HIGHBUG\nend\n");
    write(root, "lib/d.tl", "MAGIC\n");
    commit_all(root, "init");
    let mut s = server(root);

    let scan = ok(&call(&mut s, 1, "scan", json!({})));
    let scan_id = scan["scan_id"].as_str().unwrap().to_string();
    assert_eq!(scan["findings"]["total"], 7, "{scan}");
    assert_eq!(scan["findings"]["high"], 2);
    assert_eq!(scan["gate"]["passed"], false);

    let sum = ok(&call(
        &mut s,
        2,
        "summary",
        json!({"scan_id": scan_id, "group_by": "directory"}),
    ));
    assert_eq!(
        sum["groups"][0],
        json!({"key": "src", "count": 5, "high": 1, "medium": 3, "low": 1})
    );
    let sum = ok(&call(&mut s, 3, "summary", json!({"scan_id": scan_id})));
    assert_eq!(sum["groups"][0], json!({"key": "HIGH", "count": 2}));

    // Units of at most 3 findings: files are never split or shared.
    let wu = ok(&call(
        &mut s,
        4,
        "work_units",
        json!({"scan_id": scan_id, "max_findings_per_unit": 3, "strategy": "by_file"}),
    ));
    let units = wu["units"].as_array().unwrap();
    let mut seen = HashSet::new();
    let mut total = 0;
    for u in units {
        for f in u["files"].as_array().unwrap() {
            assert!(
                seen.insert(f.as_str().unwrap().to_string()),
                "{f} in two units: {wu}"
            );
        }
        total += u["findings"].as_u64().unwrap();
    }
    assert_eq!(total, 7);
    assert_eq!(seen.len(), 4);
    let wu_dir = ok(&call(
        &mut s,
        5,
        "work_units",
        json!({"scan_id": scan_id, "strategy": "by_directory", "severity": "MEDIUM"}),
    ));
    assert_eq!(wu_dir["total_findings"], 5);
    let dir_units = wu_dir["units"].as_array().unwrap();
    assert_eq!(dir_units.len(), 1, "one unit fits all: {wu_dir}");

    // The unit holding src/b.tl, then its findings page by page.
    let unit = units
        .iter()
        .find(|u| {
            u["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f == "src/b.tl")
        })
        .unwrap();
    let unit_id = unit["id"].as_str().unwrap();
    let page1 = ok(&call(
        &mut s,
        6,
        "findings",
        json!({"scan_id": scan_id, "unit_id": unit_id, "limit": 1}),
    ));
    assert_eq!(page1["returned"], 1);
    let cursor = page1["next_cursor"].as_str().unwrap().to_string();
    let f0 = &page1["files"][0]["findings"][0];
    assert!(f0["id"].is_string() && f0["rule"].is_string() && f0["line"].is_number());
    let rest = ok(&call(
        &mut s,
        7,
        "findings",
        json!({"scan_id": scan_id, "unit_id": unit_id, "limit": 50, "cursor": cursor}),
    ));
    assert!(rest.get("next_cursor").is_none());
    assert_eq!(
        page1["returned"].as_u64().unwrap() + rest["returned"].as_u64().unwrap(),
        unit["findings"].as_u64().unwrap()
    );
    let bad = call(
        &mut s,
        8,
        "findings",
        json!({"scan_id": scan_id, "cursor": cursor}),
    );
    assert_eq!(
        bad["error"]["code"], -32602,
        "cursor of another query: {bad}"
    );

    // All findings of src/b.tl.
    let b = ok(&call(
        &mut s,
        9,
        "findings",
        json!({"scan_id": scan_id, "path": "src/b.tl"}),
    ));
    let b_ids: Vec<String> = b["files"][0]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(b_ids.len(), 3);

    // Fix one LOUD, introduce a HIGHBUG, keep the rest; verify the unit.
    write(
        root,
        "src/b.tl",
        "fn b\n  LOUD\n  quiet\n  MAGIC\n  HIGHBUG\nend\n",
    );
    let v = ok(&call(
        &mut s,
        10,
        "verify",
        json!({"scan_id": scan_id, "paths": ["src/b.tl"]}),
    ));
    assert_eq!(v["counts"]["fixed"], 1, "{v}");
    assert_eq!(v["counts"]["still_present"], 2, "{v}");
    assert_eq!(v["counts"]["introduced"], 1, "{v}");
    assert_eq!(v["introduced"][0]["rule"], "tl/high-bug");
    assert_eq!(v["introduced"][0]["path"], "src/b.tl");
    let v_unit = ok(&call(
        &mut s,
        11,
        "verify",
        json!({"scan_id": scan_id, "unit_id": unit_id}),
    ));
    assert_eq!(v_unit["counts"]["introduced"], 1, "{v_unit}");
    let bad = call(&mut s, 12, "verify", json!({"scan_id": scan_id}));
    assert_eq!(bad["error"]["code"], -32602);

    // A rescan picks the change up; ids of unchanged code are stable.
    let scan2 = ok(&call(&mut s, 13, "scan", json!({"paths": ["src"]})));
    let id2 = scan2["scan_id"].as_str().unwrap();
    assert_ne!(id2, scan_id);
    let a1 = ok(&call(
        &mut s,
        14,
        "findings",
        json!({"scan_id": scan_id, "path": "src/a.tl"}),
    ));
    let a2 = ok(&call(
        &mut s,
        15,
        "findings",
        json!({"scan_id": id2, "path": "src/a.tl"}),
    ));
    assert_eq!(a1["files"], a2["files"]);

    let ex = ok(&call(
        &mut s,
        16,
        "explain_config",
        json!({"path": "src/a.tl"}),
    ));
    assert_eq!(ex["language"], "tl");
    assert_eq!(ex["rules"]["tl/high-bug"], "HIGH");
}

#[test]
fn stdio_server_writes_only_protocol_to_stdout() {
    let d = tempfile::tempdir().unwrap();
    write(d.path(), "a.tl", "HIGHBUG\n");
    let mut cmd = Command::new(BIN);
    isolate(&mut cmd)
        .args(["mcp", "-vv", "--root"])
        .arg(d.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for m in [
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "scan", "arguments": {}}}),
        ] {
            writeln!(stdin, "{m}").unwrap();
        }
        stdin.write_all(b"\r\n{oops\r\n").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let replies: Vec<Value> = stdout
        .lines()
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| panic!("non-JSON on stdout ({e}): {l}"))
        })
        .collect();
    assert_eq!(replies.len(), 3, "{stdout}");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-11-25");
    assert_eq!(
        replies[1]["result"]["structuredContent"]["findings"]["high"],
        1
    );
    assert_eq!(replies[2]["error"]["code"], -32700);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("MCP server ready"),
        "logs go to stderr: {stderr}"
    );
}

#[test]
fn verify_checks_duplication_against_the_whole_scan() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    init_repo(root);
    let block: String = (0..14)
        .map(|i| format!("  total = compute(value{i}, \"text\", {i}) + other.call(x, y);\n"))
        .collect();
    write(root, "src/a.tl", &format!("fn a\n{block}end\n"));
    write(root, "lib/b.tl", &format!("fn b\n{block}end\n"));
    write(root, "lib/c.tl", "LOUD\n");
    commit_all(root, "init");
    let mut s = server(root);
    let scan = ok(&call(&mut s, 1, "scan", json!({})));
    let scan_id = scan["scan_id"].as_str().unwrap().to_string();
    assert_eq!(
        scan["top_rules"][0]["rule"], "tl/duplicated-block",
        "{scan}"
    );

    // Unchanged: the copy in lib/b.tl (not verified) still counts.
    let v = ok(&call(
        &mut s,
        2,
        "verify",
        json!({"scan_id": scan_id, "paths": ["src/a.tl"]}),
    ));
    assert_eq!(v["counts"]["still_present"], 1, "{v}");
    assert_eq!(v["counts"]["fixed"], 0, "{v}");
    assert_eq!(v["counts"]["introduced"], 0, "{v}");
    assert!(v.get("note").is_none(), "{v}");

    // Remove the copy in lib/b.tl: its duplicated-block finding is fixed.
    write(root, "lib/b.tl", "fn b\n  small = 1;\nend\n");
    let v = ok(&call(
        &mut s,
        3,
        "verify",
        json!({"scan_id": scan_id, "paths": ["lib/b.tl", "lib/c.tl"]}),
    ));
    assert_eq!(v["counts"]["fixed"], 1, "{v}");
    assert_eq!(v["counts"]["still_present"], 1, "LOUD in c.tl: {v}");
    assert_eq!(v["counts"]["introduced"], 0, "{v}");

    // Both files: a.tl no longer has a copy anywhere either.
    let v = ok(&call(
        &mut s,
        4,
        "verify",
        json!({"scan_id": scan_id, "paths": ["src/a.tl", "lib/b.tl", "gone.tl"]}),
    ));
    assert_eq!(v["counts"]["fixed"], 2, "{v}");
    assert!(
        v["warnings"][0]
            .as_str()
            .is_some_and(|w| w.starts_with("gone.tl: not part of this scan")),
        "{v}"
    );
}

/// The server only holds the cache while a tool runs: a CLI scan next to an idle server gets the
/// cache, and after a config edit the rebuilt scanner can open it (the old one used to keep the
/// lock, so every later scan ran uncached).
#[test]
fn server_releases_the_cache_between_calls() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    init_repo(root);
    write(root, "src/a.tl", "fn a\n  HIGHBUG\nend\n");
    write(root, "src/b.tl", "LOUD\n");
    commit_all(root, "init");
    let mut s = server(root);
    let first = ok(&call(&mut s, 1, "scan", json!({})));
    assert!(first.get("warnings").is_none(), "{first}");

    // Idle server: the CLI uses the same cache.
    let r = mote(root, &["scan", "--no-gate", "--format", "json"]);
    assert_eq!(r.code, 0, "{r:?}");
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["stats"]["cache_enabled"], true, "{}", r.stderr);
    assert_eq!(v["stats"]["cache_hits"], 2, "warm from the server's scan");
    assert!(
        v["diagnostics"].as_array().unwrap().is_empty(),
        "{}",
        v["diagnostics"]
    );

    // Config edit: a new scanner, which must be able to open the cache.
    write(root, ".mote.yaml", "rules:\n  tl/loud: HIGH\n");
    let second = ok(&call(&mut s, 2, "scan", json!({})));
    assert!(second.get("warnings").is_none(), "{second}");
    assert_eq!(second["findings"]["high"], 2, "{second}");
    let third = ok(&call(&mut s, 3, "scan", json!({})));
    assert_eq!(third["files"]["cached"], 2, "{third}");
    assert!(third.get("warnings").is_none(), "{third}");

    // `verify` on the first scan (its scanner was replaced) also leaves the cache free.
    let first_id = first["scan_id"].as_str().unwrap();
    let v = ok(&call(
        &mut s,
        4,
        "verify",
        json!({"scan_id": first_id, "paths": ["src/b.tl"]}),
    ));
    assert_eq!(v["counts"]["still_present"], 1, "{v}");
    let r = mote(root, &["scan", "--no-gate", "--format", "json"]);
    let v: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["stats"]["cache_enabled"], true, "{}", r.stderr);
}

/// `scan` takes a baseline results file and reports the triage file's counts.
#[test]
fn scan_with_a_baseline_and_a_triage_file() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    write(root, "a.tl", "HIGHBUG\nLOUD\n");
    let r = mote(
        root,
        &["scan", "--no-cache", "--no-gate", "--json", "base.json"],
    );
    assert_eq!(r.code, 0, "{r:?}");
    let base: Value = serde_json::from_str(&read(root, "base.json")).unwrap();
    let loud = base["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rule_id"] == "tl/loud")
        .unwrap()
        .clone();
    write(
        root,
        ".mote-triage.yaml",
        &format!(
            "entries:\n  - fingerprint: {}\n    rule: tl/loud\n    path: a.tl\n    status: accepted\n    reason: kept\n",
            loud["fingerprint"].as_str().unwrap()
        ),
    );
    write(root, "b.tl", "MAGIC\n");
    let mut s = server(root);
    let v = ok(&call(&mut s, 1, "scan", json!({"baseline": "base.json"})));
    assert_eq!(v["new_code"]["baseline"], true, "{v}");
    assert_eq!(v["new_code"]["new"]["low"], 1, "only b.tl is new: {v}");
    assert_eq!(v["new_code"]["new"]["high"], 0, "{v}");
    assert_eq!(v["triage"]["matched"], 1, "{v}");
    assert_eq!(v["suppressed"], 1, "{v}");

    let r = call(
        &mut s,
        2,
        "scan",
        json!({"baseline": "base.json", "base": "main"}),
    );
    assert!(r.get("error").is_some(), "base and baseline: {r}");
    let r = call(&mut s, 3, "scan", json!({"baseline": "missing.json"}));
    assert_eq!(r["result"]["isError"], true, "{r}");
    let tools = one(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}),
    );
    let scan = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "scan")
        .unwrap()
        .clone();
    assert!(scan["inputSchema"]["properties"]["baseline"].is_object());
}
