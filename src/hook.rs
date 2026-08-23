use anyhow::Result;
use serde_json::json;

/// SessionStart hook: injects one-time guidance about stow's tools.
/// Non-blocking by design — SessionStart hooks can only add context,
/// they cannot intercept or redirect other tool calls (unlike a
/// PreToolUse hook, which is deliberately not used here).
pub fn session_start() -> Result<()> {
    let msg = "stow is available (MCP tools: capture, search, show) — a local SQLite FTS5 \
store for large text you don't need to read in full right away. Call capture(content, source, tool) \
proactively before returning/pasting something you expect to be large (a long fetched page, a big \
file you already have in hand, a large API response with no compression option of its own) — content \
over 2KB is stored and you get back a short reference; call search(query) later to find relevant \
portions across everything ever captured, or show(id) for the exact original. Not a replacement for \
RTK (shell output) or the sqz MCP proxy (github) — use those first where they apply; stow covers what \
they don't.";

    let output = json!({
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": msg
        }
    });
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
