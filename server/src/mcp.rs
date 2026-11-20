//! Minimal MCP server: JSON-RPC 2.0, one message per line, over stdio.
//! Implements initialize, ping, tools/list and tools/call. Nothing is ever
//! written to stdout except protocol messages.

use crate::{store, tools};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::Path;

const FALLBACK_PROTOCOL: &str = "2025-06-18";

fn result(id: &Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn tool_result(text: String, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

fn handle(root: &Path, msg: &Value) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str)?;
    // Notifications carry no id and get no answer.
    let id = msg.get("id")?;
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let version = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(FALLBACK_PROTOCOL);
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "incipit", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Ticket store for an Incipit project. Each mutating tool belongs to one role; use only the tools of your own role. Tickets are never edited as files."
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => result(id, json!({"tools": tools::definitions()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if tools::allowed_roles(name).is_none() {
                return Some(error(id, -32602, &format!("unknown tool '{name}'")));
            }
            let outcome = store::open(root).and_then(|backend| tools::call(root, backend.as_ref(), name, &args));
            match outcome {
                Ok(text) => result(id, tool_result(text, false)),
                Err(text) => result(id, tool_result(format!("REFUSED: {text}"), true)),
            }
        }
        _ => error(id, -32601, &format!("method '{method}' not found")),
    })
}

pub fn serve(root: &Path) {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(root, &msg),
            Err(_) => Some(error(&Value::Null, -32700, "parse error")),
        };
        if let Some(reply) = reply {
            if writeln!(stdout, "{reply}").and_then(|_| stdout.flush()).is_err() {
                break;
            }
        }
    }
}
