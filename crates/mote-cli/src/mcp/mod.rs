//! Model Context Protocol server: JSON-RPC 2.0 over stdio, one message per line.
//!
//! Only protocol messages are written to the output stream; logs go to stderr. Requests are
//! handled one at a time. Protocol errors use JSON-RPC codes (-32700 parse error, -32600 invalid
//! request, -32601 unknown method, -32602 invalid params — including arguments that don't match
//! a tool's input schema); failures of a well-formed tool call (unknown `scan_id`, a scan that
//! could not run, …) are tool results with `isError: true`.

mod tools;

pub use tools::Server;

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

/// Protocol revisions this server speaks (newest first).
pub const PROTOCOL_VERSIONS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
/// Answer for clients asking for a revision we don't know.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

const INSTRUCTIONS: &str = "Mote finds bugs, code smells, complexity and duplication. \
Flow: `scan` (returns scan_id + counts) → `summary` or `work_units` (disjoint file sets, one per \
parallel fixer; no file is in two units) → `findings` (per unit, paginated) → edit → `verify` \
(re-analyzes only those files: each prior finding is fixed/still_present, plus introduced ones). \
Findings with a `fix` have a mechanical, behaviour-preserving rewrite: `fixes` returns the edits and \
a unified diff per file (nothing is written; apply it, then `verify`). \
Finding ids are stable while the flagged code is unchanged. `rule` explains a rule and how to fix \
it; `explain_config` shows the effective rules for a file. With `base` (e.g. origin/main) findings \
are classified new/unchanged/worsened and new_only focuses on new code.";

fn response(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// A JSON-RPC error response.
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

impl Server {
    /// Handle one line of input; `None` when nothing must be sent back (notifications,
    /// responses from the client).
    pub fn handle_line(&mut self, line: &str) -> Option<Value> {
        match serde_json::from_str::<Value>(line) {
            Ok(v) => self.handle_message(v),
            Err(e) => Some(error(
                Value::Null,
                PARSE_ERROR,
                &format!("Parse error: {e}"),
            )),
        }
    }

    /// Handle one decoded message.
    pub fn handle_message(&mut self, msg: Value) -> Option<Value> {
        let Value::Object(obj) = msg else {
            let what = if msg.is_array() {
                "Invalid Request: batches are not supported"
            } else {
                "Invalid Request: expected a JSON object"
            };
            return Some(error(Value::Null, INVALID_REQUEST, what));
        };
        let id = obj.get("id").cloned();
        let method = obj.get("method").and_then(Value::as_str);
        let valid_id = matches!(id, None | Some(Value::String(_)) | Some(Value::Number(_)));
        let Some(method) = method else {
            // A response to something we never send, or garbage.
            if obj.contains_key("result") || obj.contains_key("error") {
                return None;
            }
            return Some(error(
                id.filter(|_| valid_id).unwrap_or(Value::Null),
                INVALID_REQUEST,
                "Invalid Request: missing `method`",
            ));
        };
        if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") || !valid_id {
            return id.map(|id| {
                error(
                    if valid_id { id } else { Value::Null },
                    INVALID_REQUEST,
                    "Invalid Request: expected `\"jsonrpc\": \"2.0\"` and a string or number id",
                )
            });
        }
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            self.notification(method, &params);
            return None;
        };
        Some(match method {
            "initialize" => response(id, self.initialize(&params)),
            "ping" => response(id, json!({})),
            "tools/list" => response(id, json!({"tools": tools::definitions()})),
            "tools/call" => match self.call_tool(&params) {
                Ok(result) => response(id, result),
                Err((code, message)) => error(id, code, &message),
            },
            _ => error(id, METHOD_NOT_FOUND, &format!("Method not found: {method}")),
        })
    }

    fn notification(&mut self, method: &str, _params: &Value) {
        match method {
            "notifications/initialized" => tracing::debug!("client initialized"),
            // Requests are handled synchronously; there is nothing to cancel.
            "notifications/cancelled" => {}
            other => tracing::debug!("ignoring notification {other}"),
        }
    }

    fn initialize(&mut self, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = requested
            .filter(|v| PROTOCOL_VERSIONS.contains(v))
            .unwrap_or(DEFAULT_PROTOCOL_VERSION);
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "mote", "version": env!("CARGO_PKG_VERSION")},
            "instructions": INSTRUCTIONS,
        })
    }
}

/// Serve until the input ends. Each response is one line, flushed immediately.
pub fn serve(
    mut input: impl BufRead,
    output: &mut dyn Write,
    server: &mut Server,
) -> io::Result<()> {
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if input.read_until(b'\n', &mut buf)? == 0 {
            return Ok(());
        }
        let reply = match std::str::from_utf8(&buf) {
            Ok(line) if line.trim().is_empty() => continue,
            Ok(line) => server.handle_line(line.trim_end_matches(['\r', '\n'])),
            Err(_) => Some(error(
                Value::Null,
                PARSE_ERROR,
                "Parse error: invalid UTF-8",
            )),
        };
        if let Some(reply) = reply {
            serde_json::to_writer(&mut *output, &reply)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}
