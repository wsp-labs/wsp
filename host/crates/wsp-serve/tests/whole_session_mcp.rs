// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/session-mcp.test.ts at 65f6b89c7, case for case. What MCP servers named on a start come to: the
//! adapter that declares it renders them gets them, and the one that declares nothing is refused in that agent's name
//! before its machine is asked for anything.
mod common;

use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_wire::views::harness::no_mcp_servers_line;

const SESSION_ID: &str = "22222222-2222-4222-8222-222222222222";

fn wsp() -> Value {
    json!({ "command": "/usr/local/bin/node", "args": ["/opt/wsp/bin.js", "mcp", "--state", "/Users/me/.wsp/state.json"] })
}

fn servers(doc: Value) -> indexmap::IndexMap<String, wsp_wire::views::harness::McpServerSpec> {
    serde_json::from_value(doc).expect("servers")
}

/// `recording(takes)`: an adapter that records its starts, declaring the MCP road or not as the case is about.
fn recording(takes: bool) -> std::rc::Rc<Scripted> {
    let s = Scripted::new().session(SESSION_ID).when(When::Sync);
    if takes {
        s.takes_mcp()
    } else {
        s
    }
}

struct Workspace {
    fx: Fx,
    ws: String,
    backend: Arc<StubBackend>,
    mark: usize,
}

async fn workspace_on(harness: &str, adapter: &std::rc::Rc<Scripted>) -> Workspace {
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).adapter(harness, adapter.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("tools"), None).await.expect("a workspace").view.id;
    let mark = backend.machine(0).exec_log().len();
    Workspace { fx, ws, backend, mark }
}

fn start(harness: &str, prompt: &str, mcp: Option<Value>) -> StartOptions {
    StartOptions { harness: Some(harness.into()), prompt: prompt.into(), mcp_servers: mcp.map(servers), ..Default::default() }
}

// "MCP servers named on a start" > "reach the adapter that declares it renders them"
#[test]
fn reach_the_adapter_that_declares_it_renders_them() {
    drive(|| async {
        let claude = recording(true);
        let w = workspace_on("claude", &claude).await;
        w.fx.rt
            .sessions()
            .start(&w.ws, start("claude", "write the recipe", Some(json!({ "wsp": wsp() }))), None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(claude.mcp.borrow()[0], Some(json!({ "wsp": wsp() })));
    });
}

// "MCP servers named on a start" > "are refused in the agent's name on an adapter that declares nothing, before the machine is asked, and no thread opens"
#[test]
fn are_refused_in_the_agents_name_on_an_adapter_that_declares_nothing_before_the_machine_is_asked_and_no_thread_opens() {
    drive(|| async {
        let codex = recording(false);
        let w = workspace_on("codex", &codex).await;
        let refused =
            w.fx.rt
                .sessions()
                .start(&w.ws, start("codex", "write the recipe", Some(json!({ "wsp": wsp() }))), None)
                .await
                .err()
                .expect("a refusal");
        assert!(refused.message.contains(&no_mcp_servers_line("codex")), "{}", refused.message);
        assert!(codex.starts.borrow().is_empty());
        assert!(w.backend.machine(0).exec_log()[w.mark..].is_empty());
        assert!(w.fx.rt.sessions().list(Some(w.ws.clone()), None).await.expect("a list").is_empty());
    });
}

// "MCP servers named on a start" > "takes the same message without them, since only the servers were the trouble"
#[test]
fn takes_the_same_message_without_them_since_only_the_servers_were_the_trouble() {
    drive(|| async {
        let codex = recording(false);
        let w = workspace_on("codex", &codex).await;
        w.fx.rt.sessions().start(&w.ws, start("codex", "write the recipe", None), None).await.expect("a start").finished().await;
        assert_eq!(codex.starts.borrow().len(), 1);
        assert_eq!(codex.mcp.borrow()[0], None);
    });
}

// "MCP servers named on a start" > "an empty set is no servers at all, so it is not refused"
#[test]
fn an_empty_set_is_no_servers_at_all_so_it_is_not_refused() {
    drive(|| async {
        let codex = recording(false);
        let w = workspace_on("codex", &codex).await;
        w.fx.rt.sessions().start(&w.ws, start("codex", "hello", Some(json!({}))), None).await.expect("a start").finished().await;
        assert_eq!(codex.starts.borrow().len(), 1);
    });
}
