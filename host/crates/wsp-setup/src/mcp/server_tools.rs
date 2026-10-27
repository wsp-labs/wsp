// SPDX-License-Identifier: AGPL-3.0-only
//! `server-tools.ts`: one MCP server's tools and state off one connect, kept a few minutes per target and shared by every agent that names the server.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_catalog::mcp::{McpServer, McpTransport};
use wsp_collect::host::Host;
use wsp_machines::agents_read::ServerToolsAsk;
use wsp_slates::slate_mcp::client::McpSignIn;
use wsp_wire::types::agents_report::ServerToolsAnswer;
use wsp_wire::wire::daemon_replies::HttpBearer;
use wsp_words::exit::HostError;

/// `TOOLS_DEADLINE_MS` (`server-tools.ts:31`).
pub const TOOLS_DEADLINE_MS: u64 = 20_000;

/// `TOOLS_KEPT_MS` (`server-tools.ts:33`), how long one server's answer stands before it is asked again.
pub const TOOLS_KEPT_MS: u64 = 3 * 60_000;

/// `INITIALIZE` (`server-tools.ts:54`), the initialize request as its JSON text.
pub fn initialize() -> String {
    todo!()
}

/// `exitedRefusal` (`server-tools.ts:218`).
pub fn exited_refusal(_command: &str, _exit: Option<&str>) -> String {
    todo!()
}

/// `variableNameRefusal` (`server-tools.ts:224`).
pub fn variable_name_refusal(_name: &str) -> String {
    todo!()
}

/// `signInOf` (`server-tools.ts:305`).
pub fn sign_in_of(_host: &Host, _agent_id: &str, _name: &str, _t: &McpTransport, _cwd: &str) -> Option<McpSignIn> {
    todo!()
}

/// `bearerIn` (`server-tools.ts:319`).
pub fn bearer_in(_text: Option<&str>, _b: &HttpBearer) -> Option<String> {
    todo!()
}

/// `unsetReferenceRefusal` (`server-tools.ts:349`).
pub fn unset_reference_refusal(_name: &str) -> String {
    todo!()
}

/// `noSuchServerRefusal` (`server-tools.ts:350`).
pub fn no_such_server_refusal(_name: &str, _agent: &str) -> String {
    todo!()
}

/// `noMcpAgentRefusal` (`server-tools.ts:351`).
pub fn no_mcp_agent_refusal(_agent: &str) -> String {
    todo!()
}

/// `ResolveServerOptions` (`server-tools.ts:372`), the inline `o` of `resolveServer`.
pub struct ResolveServerOptions {
    pub project: Option<String>,
    pub values: Option<IndexMap<String, String>>,
    pub off: Option<Rc<dyn Fn(McpServer) -> LocalBoxFuture<'static, bool>>>,
}

/// `ResolvedServer` (`server-tools.ts:372`), the `resolveServer` answer: the transport, the folder it runs in and the values to hide.
pub struct ResolvedServer {
    pub transport: McpTransport,
    pub cwd: String,
    pub secrets: Vec<String>,
}

/// `resolveServer` (`server-tools.ts:372`).
pub async fn resolve_server(_host: &Host, _agent_id: &str, _name: &str, _opts: ResolveServerOptions) -> Result<ResolvedServer, HostError> {
    todo!()
}

/// `ToolsOptions` (`server-tools.ts:415`), the optional `o` of `ServerTools.tools`.
pub struct ToolsOptions {
    pub project: Option<String>,
    pub env: Option<IndexMap<String, String>>,
    pub values: Option<IndexMap<String, String>>,
    pub off: Option<Rc<dyn Fn(McpServer) -> LocalBoxFuture<'static, bool>>>,
}

/// `ServerTools` (`server-tools.ts:410`), the tools of one MCP server and the forgetting of what was kept for a target.
#[async_trait::async_trait(?Send)]
pub trait ServerTools {
    /// `ServerTools.tools` (`server-tools.ts:415`).
    async fn tools(&self, host: &Host, ask: &ServerToolsAsk, opts: Option<ToolsOptions>) -> Result<ServerToolsAnswer, HostError>;
    /// `ServerTools.forget` (`server-tools.ts:417`).
    fn forget(&self, key: &str);
}

/// `ServerToolsOptions` (`server-tools.ts:420`), the inline `o` of `serverTools`.
pub struct ServerToolsOptions {
    pub now: Rc<dyn Fn() -> i64>,
    pub deadline_ms: Option<u64>,
    pub log: Rc<dyn Fn(&str)>,
}

/// `serverTools` (`server-tools.ts:420`).
pub fn server_tools(_opts: ServerToolsOptions) -> Rc<dyn ServerTools> {
    todo!()
}
