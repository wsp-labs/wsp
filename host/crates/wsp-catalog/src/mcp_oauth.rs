// SPDX-License-Identifier: AGPL-3.0-only
//! Where an agent's harness keeps the sign-in it holds for an http MCP server. Port of
//! packages/catalog/src/mcp-oauth.ts at a3b870a6e; the spec is spec/mcp_modules.md.
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::jsonc::js_stringify;
use wsp_wire::wire::daemon::HttpBearer;

/// One module per harness (mcp-oauth.ts:202).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpOAuthStore {
    Claude,
}

impl McpOAuthStore {
    pub fn measured(self) -> &'static str {
        match self {
            McpOAuthStore::Claude => "claude 2.1.296",
        }
    }

    /// The store folder where wsp names none, `~/`-relative: "~/.claude".
    pub fn home(self) -> &'static str {
        match self {
            McpOAuthStore::Claude => "~/.claude",
        }
    }

    /// `{home}/.credentials.json`, at `["mcpOAuth", "<name>|<first 16 hex of sha256(JSON of {type:"http",url,headers})>"]`,
    /// field `accessToken`; `sha256` answers hex (mcp-oauth.ts:205). The JSON is `JSON.stringify` of the three keys in
    /// that order, the headers in their own order.
    pub fn bearer(
        self,
        name: &str,
        url: &str,
        headers: &IndexMap<String, String>,
        home: &str,
        sha256: &dyn Fn(&str) -> String,
    ) -> HttpBearer {
        let mut header_map = Map::new();
        for (k, v) in headers {
            header_map.insert(k.clone(), Value::String(v.clone()));
        }
        let identity = js_stringify(&json!({ "type": "http", "url": url, "headers": Value::Object(header_map) }));
        let digest: String = sha256(&identity).chars().take(16).collect();
        HttpBearer {
            file: format!("{home}/.credentials.json"),
            at: vec!["mcpOAuth".to_string(), format!("{name}|{digest}")],
            field: "accessToken".to_string(),
        }
    }
}
