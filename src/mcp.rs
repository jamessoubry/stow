use crate::db;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

const THRESHOLD_BYTES: usize = 2048;

pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle(&req)? {
            writeln!(out, "{}", serde_json::to_string(&resp)?)?;
            out.flush()?;
        }
    }
    Ok(())
}

fn handle(req: &Value) -> Result<Option<Value>> {
    let id = req.get("id").cloned();
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "stow", "version": env!("CARGO_PKG_VERSION") }
        }),
        "notifications/initialized" => return Ok(None),
        "tools/list" => json!({ "tools": tool_defs() }),
        "tools/call" => return Ok(Some(handle_call(req, id)?)),
        _ => return Ok(None),
    };

    Ok(Some(json!({ "jsonrpc": "2.0", "id": id, "result": result })))
}

fn tool_defs() -> Value {
    json!([
        {
            "name": "capture",
            "description": "Store large text content locally (SQLite FTS5) instead of returning it in full. Use for any output you expect to be large and only need to search/reference later, not read verbatim right away. Returns a short stub with an ID; content over 2KB is always stored, smaller content is returned unchanged.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "The text to store" },
                    "source": { "type": "string", "description": "Where this came from, e.g. a file path, command, or URL" },
                    "tool": { "type": "string", "description": "What produced it, e.g. 'bash', 'read', 'fetch'" }
                },
                "required": ["content", "source", "tool"]
            }
        },
        {
            "name": "search",
            "description": "BM25 full-text search over everything previously captured via the capture tool. Returns matching snippets with IDs — call show(id) to get the full original content for any result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search terms (FTS5 query syntax — plain words, \"phrases\", OR, NOT, etc.)" },
                    "limit": { "type": "integer", "description": "Max results, default 10" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "show",
            "description": "Retrieve the full, original, byte-exact content for a capture by its ID (from capture or search results).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer", "description": "The capture ID" }
                },
                "required": ["id"]
            }
        }
    ])
}

fn handle_call(req: &Value, id: Option<Value>) -> Result<Value> {
    let params = req.get("params").cloned().unwrap_or(json!({}));
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let text = match name {
        "capture" => {
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("unknown");
            let tool = args.get("tool").and_then(|v| v.as_str()).unwrap_or("unknown");

            if content.len() < THRESHOLD_BYTES {
                content.to_string()
            } else {
                let conn = db::open()?;
                let capture_id = db::insert(&conn, content, source, tool)?;
                format!(
                    "[stowed #{} — {} bytes from {} ({})]\nUse the search tool to find relevant portions, or show({}) for the full content.",
                    capture_id, content.len(), source, tool, capture_id
                )
            }
        }
        "search" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(10);
            let conn = db::open()?;
            let results = db::search(&conn, query, limit)?;
            if results.is_empty() {
                "No matches found.".to_string()
            } else {
                results
                    .iter()
                    .map(|c| {
                        format!(
                            "#{} [{}, {} bytes, {}] {}: {}",
                            c.id, c.tool, c.byte_len, c.created_at, c.source, c.snippet
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
        }
        "show" => {
            let capture_id = args.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
            let conn = db::open()?;
            match db::show(&conn, capture_id)? {
                Some((content, source, tool, created_at)) => {
                    format!("[#{} — {} ({}), captured {}]\n\n{}", capture_id, source, tool, created_at, content)
                }
                None => format!("No capture found with id {}", capture_id),
            }
        }
        _ => format!("Unknown tool: {}", name),
    };

    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }] }
    }))
}
