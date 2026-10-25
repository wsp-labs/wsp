// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp-on.ts at 65f6b89c7. Spec: spec/slate_box.md (rules 1-8).
//!
//! A slate's command server on the computer its thread runs on, started there through that computer's daemon
//! (`stdio.open`) as the login its lines run as; its variables written first on its own stdin as NUL-separated pairs,
//! never on a command line. An http server asked from there too (`http.post`), the sign-in read there; or from here
//! with the sign-in the agent keeps here, read at each message.
#![allow(unused_variables)]

use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::Value;

pub use wsp_engine::target_line::TargetLogin;
pub use wsp_wire::wire::daemon::{HttpPostReply, EXEC_TIMEOUT_MAX_MS};

use wsp_core::ids::Ids;

use crate::slate_mcp::{McpPipeOpen, McpPost, McpPostRoad, McpSignIn, McpTransport};

/// One computer's daemon for one server: its frames, its events, and its end (slate-mcp-on.ts:278).
pub trait StdioRoad {
    /// Fails with the daemon's refusal.
    fn send(&self, frame: Value) -> LocalBoxFuture<'static, Result<Value, String>>;
    /// Every event the road carries; the answer stops them.
    fn listen(&self, f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()>;
    /// Settles when the road closes.
    fn closed(&self) -> Shared<LocalBoxFuture<'static, ()>>;
    fn close(&self);
}

/// Dials a road to the thread's computer's daemon.
pub type StdioRoadDial = Rc<dyn Fn() -> LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>>>;

/// Where a thread's slate keeps its files on its machine (slate-mcp-on.ts:290).
pub const BOX_SLATES: &str = "/tmp/wsp-slates";

// slate-mcp-on.ts:292 boxSlateDir(threadId)
pub fn box_slate_dir(thread_id: &str) -> String {
    todo!()
}

/// The loop that reads the server's variables off its stdin before it becomes the server (slate-mcp-on.ts:294).
pub const ENV_THEN_SERVER: &str = "while IFS= read -r -d '' k && [ -n \"$k\" ] && IFS= read -r -d '' v; do export \"$k=$v\"; done; ";

// slate-mcp-on.ts:297 serverLine(login, t, cwd)
/// The command `stdio.open` runs; `t` is the stdio transport (spec rule 2).
pub fn server_line(login: &TargetLogin, t: &McpTransport, cwd: &str) -> String {
    todo!()
}

// slate-mcp-on.ts:300 envPairs(env)
pub fn env_pairs(vars: &IndexMap<String, String>) -> String {
    todo!()
}

// slate-mcp-on.ts:308 pipeOn(computer, login, road, mark)
/// Starts a server through `road` (spec rule 4); `mark` rides the spawn's environment and goes last in the variables.
pub fn pipe_on(computer: &str, login: TargetLogin, road: StdioRoadDial, mark: IndexMap<String, String>, ids: Rc<dyn Ids>) -> McpPipeOpen {
    todo!()
}

// slate-mcp-on.ts:354 signInGoneLine(computer, line)
pub fn sign_in_gone_line(computer: &str, line: &str) -> String {
    todo!()
}

// slate-mcp-on.ts:360 renewOnRefusal(computer, signIn, send)
/// `send` once; a refusal for want of a sign-in renews once at a time and sends once more (spec rule 6).
pub fn renew_on_refusal(computer: &str, sign_in: Option<McpSignIn>, send: Rc<dyn McpPost>) -> Rc<dyn McpPost> {
    todo!()
}

// slate-mcp-on.ts:374 postOn(computer, road, signIn)
/// A maker of one post per connection through the daemon's `http.post` (spec rule 7).
pub fn post_on(computer: &str, road: StdioRoadDial, sign_in: Option<McpSignIn>) -> Rc<dyn Fn() -> Rc<dyn McpPost>> {
    todo!()
}

// slate-mcp-on.ts:398 postHere(computer, signIn)
/// This computer's own post with the bearer read at each message (spec rule 8).
pub fn post_here(computer: &str, sign_in: McpSignIn) -> McpPostRoad {
    todo!()
}
