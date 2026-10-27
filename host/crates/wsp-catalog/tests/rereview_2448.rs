// SPDX-License-Identifier: AGPL-3.0-only
//! Re-review of round 2 of wsp-labs/wsp-map#2448: refer_codex's new TOML writer against node. Each expected text is
//! node's: CODEX_TOML.refer from packages/catalog/src/mcp.ts with smol-toml 1.9.0, node 22.23.4.
use indexmap::IndexSet;
use wsp_catalog::mcp::McpFormat;

fn refer(text: &str) -> Result<String, String> {
    McpFormat::CodexToml.refer(text, None, &[], &IndexSet::new()).map(|r| r.text).map_err(|e| e.message)
}

// toml_write.rs:14 `quoted` writes a string through jsonc's `js_string`, which writes a char in U+10F800..U+10FFFF as the
// JSON.parse port's stand-in for a lone surrogate (`\udffd` for U+10FFFD). TOML has no such escape, so refer_codex's own
// read of what it wrote (mcp_toml_refer.rs:320) fails. smol-toml's formatString writes the char as it is.
#[test]
fn refer_codex_keeps_a_plane_16_char_in_a_string_as_node_does() {
    let text = "[mcp_servers.x]\ncommand = \"c\"\nenv.TOKEN = \"secret-value\"\nnote = \"a\u{10FFFD}b\"\n";
    let node = "[mcp_servers.x]\ncommand = \"c\"\nnote = \"a\u{10FFFD}b\"\nenv_vars = [ \"TOKEN\" ]\n";
    assert_eq!(refer(text).as_deref(), Ok(node));
}

// toml_write.rs:53-54: an offset of -00:00 is the toml crate's `Custom { minutes: 0 }` and is written `+00:00`. TomlDate
// (smol-toml date.js toISOString) restores the offset as authored, `-00:00`.
#[test]
fn refer_codex_keeps_a_minus_zero_offset_as_node_does() {
    let text = "when = 1979-05-27T07:32:00-00:00\n[mcp_servers.x]\ncommand = \"c\"\nenv.TOKEN = \"secret-value\"\n";
    let node = "when = 1979-05-27T07:32:00.000-00:00\n\n[mcp_servers.x]\ncommand = \"c\"\nenv_vars = [ \"TOKEN\" ]\n";
    assert_eq!(refer(text).as_deref(), Ok(node));
}
