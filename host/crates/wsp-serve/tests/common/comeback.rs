// SPDX-License-Identifier: AGPL-3.0-only
//! The helpers runtime-comeback.test.ts keeps inside its `describe`: `hostWentDown`, `guestRuns`, `slowListing`,
//! `threadOn`, and the reads of a workspace's rows and history every case makes as JSON.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::views::workspace::Caller;

use super::*;

/// `rt.sessions.list(ws)` as the JSON the wire carries.
pub async fn rows(rt: &AssembledRuntime, ws: &str) -> Vec<Value> {
    rt.sessions().list(Some(ws.to_string()), None).await.expect("a list").iter().map(j).collect()
}

/// `(await rt.sessions.list(ws)).map(s => s.status)`.
pub async fn statuses(rt: &AssembledRuntime, ws: &str) -> Vec<String> {
    rows(rt, ws).await.iter().map(|r| r["status"].as_str().unwrap_or_default().to_string()).collect()
}

/// `rt.sessions.history(ws)` as the JSON the wire carries.
pub async fn history(rt: &AssembledRuntime, ws: &str) -> Vec<Value> {
    rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

pub async fn history_types(rt: &AssembledRuntime, ws: &str) -> Vec<String> {
    history(rt, ws).await.iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect()
}

/// `(await rt.sessions.history(ws)).some(e => e.type === kind)`.
pub async fn has_event(rt: &AssembledRuntime, ws: &str, kind: &str) -> bool {
    history(rt, ws).await.iter().any(|e| e["type"] == kind)
}

pub fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

/// `turn.done` then `session.end` of a run, the way a harness that finished prints them.
pub fn finish_run(h: &MachineRuns, run: &str, session: &str, text: &str) {
    h.emit(run, json!({ "type": "turn.done", "sessionId": session, "result": { "status": "completed", "text": text } }));
    h.emit(run, json!({ "type": "session.end", "sessionId": session, "exitCode": 0, "sawResult": true }));
}

pub struct Down {
    pub workspace_id: String,
    pub run: String,
}

/// `hostWentDown`: a workspace with one turn running on the machine, the host stopped under it, and what that turn's run is.
pub async fn host_went_down(h: &Rc<MachineRuns>, store: &Rc<dyn Store>, backend: &Arc<StubBackend>) -> Down {
    let fx = runtime(backend).store(store).adapter("claude", h.factory()).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    fx.rt.sessions().start(&ws, opts("build it"), None).await.expect("a start");
    let (rt, w) = (fx.rt.clone(), ws.clone());
    wait!([rt, w] has_event(&rt, &w, "session.start").await);
    let run = h.handles()[0].clone();
    h.emit(&run, json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "reading the ticket" }));
    fx.rt.close().await.expect("a close");
    Down { workspace_id: ws, run }
}

/// `guestRuns`: what the machine answers when a connecting host asks which runs it holds, and what it was told to end.
pub fn guest_runs(backend: &Arc<StubBackend>, claims: &[&str]) -> Rc<RefCell<Vec<String>>> {
    let reaps: Rc<RefCell<Vec<String>>> = Rc::default();
    let listed: String = claims.iter().map(|base| format!("{base}.d")).collect::<Vec<_>>().join("\n");
    let kept = reaps.clone();
    backend.set_exec_impl(exec_fn(move |_, cmd| {
        if cmd.contains("printf '%s\\n' \"$d\"") {
            return Ok(exec_ok(&format!("{listed}\n")));
        }
        if cmd.contains("kill -TERM") {
            kept.borrow_mut().push(cmd.to_string());
        }
        Ok(exec_ok(if cmd.contains("echo WSP_CTX") { "WSP_CTX\nWSP_CTX_END\n" } else { "" }))
    }));
    reaps
}

/// What `slowListing` hands back.
pub struct Slow {
    pub slow: String,
    pub slow_machine: String,
    pub other: String,
    /// Lets out the machine's answer to the question of which runs it holds.
    pub answer: Rc<Gate>,
    pub thread: Option<(String, Caller)>,
    pub backend: Arc<StubBackend>,
}

pub const ORPHAN: &str = "/tmp/wsp-run/aabbccddeeff";

impl Slow {
    pub fn log(&self) -> Vec<String> {
        self.backend.machines().iter().find(|m| m.id() == self.slow_machine).expect("the slow machine").exec_log()
    }
    pub fn reap_at(&self) -> Option<usize> {
        self.log().iter().position(|cmd| cmd.contains("kill -TERM") && cmd.contains(ORPHAN))
    }
    pub fn machine(&self) -> Arc<StubMachine> {
        self.backend.machines().into_iter().find(|m| m.id() == self.slow_machine).expect("the slow machine")
    }
}

/// `slowListing`: two workspaces left by a host that went down; `slow` stands for a machine still restoring, whose answer
/// to the question of which runs it holds the test lets out by hand, naming one run nobody here keeps.
pub async fn slow_listing(backend: &Arc<StubBackend>, store: &Rc<dyn Store>, h: &Rc<MachineRuns>, with_thread: bool) -> Slow {
    let rt1 = runtime(backend).store(store).adapter("claude", h.factory()).build();
    let slow = create_on(&rt1.rt, CreateOn::golden("slow"), None).await.expect("a workspace").view;
    let other = create_on(&rt1.rt, CreateOn::golden("other"), None).await.expect("a workspace").view.id;
    let thread = if with_thread { Some(thread_on(&rt1.rt, &slow.id, h).await) } else { None };
    rt1.rt.close().await.expect("a close");
    let answer = Gate::new();
    let (waited, machine_id) = (answer.clone(), j(&slow)["machineId"].as_str().expect("a machine").to_string());
    let slow_machine = machine_id.clone();
    backend.set_exec_impl(exec_async(move |m, cmd| {
        let (waited, machine_id) = (waited.clone(), machine_id.clone());
        Box::pin(async move {
            if m.id() == machine_id && cmd.contains("printf '%s\\n' \"$d\"") {
                waited.wait().await;
                return Ok(exec_ok(&format!("{ORPHAN}.d\n")));
            }
            Ok(exec_ok(if cmd.contains("echo WSP_CTX") {
                "WSP_CTX\nWSP_CTX_END\n"
            } else if cmd.contains("mkdir") {
                "WSP_LAUNCHED\n"
            } else if cmd.contains("echo yes || echo no") {
                "yes\n"
            } else {
                ""
            }))
        })
    }));
    Slow { slow: slow.id.clone(), slow_machine, other, answer, thread, backend: backend.clone() }
}

/// `threadOn`: a finished thread on a workspace, and that thread as a caller.
pub async fn thread_on(rt: &AssembledRuntime, workspace_id: &str, h: &Rc<MachineRuns>) -> (String, Caller) {
    let started = rt.sessions().start(workspace_id, opts("build it"), None).await.expect("a start");
    let thread_id = j(&started.view())["threadId"].as_str().expect("a thread").to_string();
    let run = h.handles().last().cloned().expect("a run");
    let w = workspace_id.to_string();
    until(|| {
        let w = w.clone();
        async move { has_event(rt, &w, "session.start").await }
    })
    .await;
    finish_run(h, &run, &format!("sess-{}", h.handles().len()), "done");
    started.finished().await;
    let by: wsp_wire::views::workspace_more::ThreadScope = serde_json::from_value(json!({
        "kind": "thread", "threadId": thread_id, "workspaceId": workspace_id, "rootThreadId": thread_id,
    }))
    .expect("a scope");
    let caller = Caller::Scoped { origin: wsp_wire::views::workspace_more::WorkspaceOrigin::Here, by };
    (thread_id, caller)
}

/// `notifies(rt, ws)`: the text of every `session.notify` of the workspace's history.
pub async fn notifies(rt: &AssembledRuntime, ws: &str) -> Vec<String> {
    history(rt, ws)
        .await
        .iter()
        .filter(|e| e["type"] == "session.notify")
        .map(|e| e["text"].as_str().unwrap_or_default().to_string())
        .collect()
}

/// `until(cond, ms).catch(() => undefined)`: whether `cond` held within `ms`, which never fails the test.
pub async fn within<Fut: std::future::Future<Output = bool>>(ms: u64, mut cond: impl FnMut() -> Fut) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    loop {
        if cond().await {
            return true;
        }
        if std::time::Instant::now() > deadline {
            return false;
        }
        sleep_ms(10).await;
    }
}

/// `text.replace(/^thread \w+ finished \(completed\): /, "")` and its kin: the line without the thread's word, and the
/// line unchanged where it does not open as that regex does.
pub fn without_thread(text: &str, rest_of_prefix: &str) -> String {
    let Some(after) = text.strip_prefix("thread ") else { return text.to_string() };
    let word = after.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').count();
    if word == 0 {
        return text.to_string();
    }
    match after[word..].strip_prefix(' ').and_then(|s| s.strip_prefix(rest_of_prefix)) {
        Some(rest) => rest.to_string(),
        None => text.to_string(),
    }
}
