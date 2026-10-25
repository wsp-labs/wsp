// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/mcp-check.test.ts.
use std::collections::BTreeMap;

use wsp_catalog::catalog::mcp_agents;
use wsp_catalog::mcp_check::{McpCheck, McpCheckWord};
use wsp_wire::types::agents_report::McpAuth;

const CLAUDE: McpCheck = McpCheck::Claude;
const CODEX: McpCheck = McpCheck::Codex;

// `claude mcp get <name>` as Claude Code 2.1.281 printed it; the first is this computer's own answer for a server.
fn transcript(status: &str) -> String {
    format!("notion:\n  Scope: User config (available in all your projects)\n  Status: {status}\n\nTo remove this server, run: claude mcp remove notion -s user\n")
}

// `codex mcp list --json` as Codex 0.162.1 printed it, its warning on stderr first, with one server of each state.
const LISTED: &str = r#"WARNING: proceeding, even though we could not create PATH aliases: Refusing to create helper binaries under temporary dir "/tmp"
[
  { "name": "axiom", "enabled": true, "transport": { "type": "streamable_http", "url": "https://mcp.axiom.co/mcp" }, "auth_status": "o_auth" },
  { "name": "linear", "enabled": true, "transport": { "type": "streamable_http", "url": "https://mcp.linear.app/mcp" }, "auth_status": "not_logged_in" },
  { "name": "tok", "enabled": true, "transport": { "type": "streamable_http", "url": "https://example.com/mcp", "bearer_token_env_var": "TOK" }, "auth_status": "bearer_token" },
  { "name": "plain", "enabled": true, "transport": { "type": "stdio", "command": "echo" }, "auth_status": "unsupported" }
]
"#;

mod the_harness_s_own_word_on_one_server_s_sign_in {
    use super::*;

    #[test]
    fn reads_claude_code_s_status_line_as_the_sign_in_it_stands_for_and_nothing_where_t() {
        assert_eq!(CLAUDE.line("notion"), "claude mcp get 'notion'");
        assert_eq!(CLAUDE.auth(&transcript("\u{2714} Connected"), "notion"), Some(McpCheckWord::Auth(McpAuth::SignedIn)));
        assert_eq!(CLAUDE.auth(&transcript("\u{26a0} Needs authentication"), "notion"), Some(McpCheckWord::Auth(McpAuth::NeedsSignIn)));
        assert_eq!(CLAUDE.auth(&transcript("! Needs authentication"), "notion"), Some(McpCheckWord::Auth(McpAuth::NeedsSignIn)));
        assert_eq!(CLAUDE.auth(&transcript("\u{2718} Failed to connect"), "notion"), Some(McpCheckWord::Auth(McpAuth::Failed)));
        assert_eq!(CLAUDE.auth(&transcript("\u{23f8} Pending approval"), "notion"), None);
    }

    #[test]
    fn reads_claude_code_s_words_for_a_name_its_config_does_not_hold_as_not_set_up_in_b() {
        assert_eq!(CLAUDE.auth("No MCP server named \"cloudflare\". Configured servers: plugin:context7:context7\n", "cloudflare"), Some(McpCheckWord::NotSetUp));
        assert_eq!(CLAUDE.auth("No MCP server named \"cloudflare\". Run `claude mcp add` to add one.\n", "cloudflare"), Some(McpCheckWord::NotSetUp));
        assert_eq!(CLAUDE.auth("No MCP server found with name: notion\n", "notion"), Some(McpCheckWord::NotSetUp));
    }

    #[test]
    fn reads_the_server_s_auth_status_off_codex_s_list_a_name_it_does_not_list_as_not_s() {
        assert_eq!(CODEX.line("axiom"), "codex mcp list --json");
        assert_eq!(CODEX.auth(LISTED, "axiom"), Some(McpCheckWord::Auth(McpAuth::SignedIn)));
        assert_eq!(CODEX.auth(LISTED, "linear"), Some(McpCheckWord::Auth(McpAuth::NeedsSignIn)));
        assert_eq!(CODEX.auth(LISTED, "tok"), None);
        assert_eq!(CODEX.auth(LISTED, "plain"), None);
        assert_eq!(CODEX.auth(LISTED, "cloudflare_observability"), Some(McpCheckWord::NotSetUp));
        assert_eq!(CODEX.auth("[]\n", "axiom"), Some(McpCheckWord::NotSetUp));
        assert_eq!(CODEX.auth("error: unexpected argument '--json' found\n", "axiom"), None);
        assert_eq!(CODEX.auth("[\n  { \"name\": \"axiom\"", "axiom"), None);
    }

    #[test]
    fn is_registered_on_the_agents_whose_check_was_measured_and_no_other_agent_guesses() {
        let checks: BTreeMap<String, Option<McpCheck>> = mcp_agents().into_iter().map(|a| (a.id.clone(), a.mcp.as_ref().unwrap().check)).collect();
        let want = BTreeMap::from([
            ("claude".to_string(), Some(McpCheck::Claude)),
            ("codex".to_string(), Some(McpCheck::Codex)),
            ("gemini".to_string(), None),
            ("opencode".to_string(), None),
        ]);
        assert_eq!(checks, want);
    }
}
