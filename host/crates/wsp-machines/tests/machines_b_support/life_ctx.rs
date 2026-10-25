// SPDX-License-Identifier: AGPL-3.0-only
//! `SubLifeCtx` over the fake host: every call kept by name, every answer its default unless scripted.
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::{json, Value};

use wsp_engine::machine::MachineBackend;
use wsp_core::clock::Cancel;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_life::{BackendFor, SessionFile, SubLifeCtx};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::ReachState;
use wsp_words::exit::HostError;

use super::*;

impl BackendFor for FakeHost {
    fn backend_for(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        Ok(self.backend.clone())
    }
}

/// Claude Code's session file on a machine (sub-fixture.ts `CLAUDE_SESSION_FILE`).
pub fn claude_session_file(id: &str) -> SessionFile {
    SessionFile { folder: "/root/.claude-cfg/projects".to_string(), name: format!("{id}.jsonl"), depth: 1 }
}

#[async_trait(?Send)]
impl SubLifeCtx for FakeHost {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn live_all(&self) -> Vec<Entry> {
        self.live.borrow().values().cloned().collect()
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.thread_records.borrow().clone()
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.sessions.borrow().values().map(|s| s.view.clone()).collect()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.sessions.borrow().values().rev().find(|s| s.view.thread_id.as_deref() == Some(thread_id)).map(|s| s.view.clone())
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        self.running.borrow().contains(thread_id)
    }
    fn launching_on(&self, thread_id: &str) -> bool {
        self.launching.borrow().contains(thread_id)
    }
    fn push_head(&self, thread_id: &str) {
        self.call("push_head", &[thread_id]);
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        record.place.clone()
    }
    fn delete_after_ms(&self, place_id: Option<&str>) -> Option<f64> {
        place_id.and_then(|p| self.settings_delete_after.borrow().get(p).copied())
    }
    fn session_file(&self, _entry: &Entry, harness: &str, session_id: &str) -> Option<SessionFile> {
        (harness == "claude").then(|| claude_session_file(session_id))
    }
    async fn read_transcript(&self, workspace_id: &str, thread_id: &str, limit: usize, bytes: usize) -> AreaResult<Vec<Value>> {
        self.call("read_transcript", &[workspace_id, thread_id, &limit.to_string(), &bytes.to_string()]);
        match self.next("read_transcript") {
            Some(Ok(Value::Array(events))) => Ok(events),
            Some(Err(e)) => Err(e),
            _ => Ok(Vec::new()),
        }
    }
    async fn put_blob(&self, collection: &str, key: &str, bytes: Vec<u8>) -> AreaResult<()> {
        self.call("put_blob", &[collection, key]);
        self.blobs.borrow_mut().insert((collection.to_string(), key.to_string()), bytes);
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, key: &str) -> AreaResult<()> {
        self.call("delete_blob", &[collection, key]);
        self.blobs.borrow_mut().shift_remove(&(collection.to_string(), key.to_string()));
        Ok(())
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        self.call("persist", &[&record.id]);
        if let Some(Err(e)) = self.next("persist") {
            return Err(e);
        }
        self.persist_now(record);
        Ok(())
    }
    async fn nap_with(&self, workspace_id: &str, reason: &str) -> AreaResult<()> {
        self.call("nap_with", &[workspace_id, reason]);
        if let Some(Err(e)) = self.next("nap_with") {
            return Err(e);
        }
        let entry = self.live.borrow().get(workspace_id).cloned();
        if let Some(entry) = entry {
            let record = entry.borrow().record.clone();
            let mut r = record.borrow_mut();
            r.phase = serde_json::from_value(json!("napping")).unwrap();
            r.napped_at = Some(self.clock.now().into());
        }
        Ok(())
    }
    fn end_sessions(&self, workspace_id: &str, reason: &str) {
        self.call("end_sessions", &[workspace_id, reason]);
    }
    fn attach_dead(&self, entry: &Entry, machine_id: &str) -> Entry {
        self.call("attach_dead", &[&entry_id(entry), machine_id]);
        self.reattach(entry, FakeMachine::new(machine_id))
    }
    fn forget_marks(&self, workspace_id: &str) {
        self.call("forget_marks", &[workspace_id]);
    }
    fn idle_forget(&self, workspace_id: &str) {
        self.call("idle_forget", &[workspace_id]);
    }
    async fn emit_status(&self, entry: &Entry, reach: ReachState) -> AreaResult<()> {
        self.call("emit_status", &[&entry_id(entry), &serde_json::to_value(reach).unwrap().as_str().unwrap_or("").to_string()]);
        Ok(())
    }
    fn emit(&self, event: Value) {
        self.emit_now(event);
    }
    async fn unfork(&self, entry: &Entry) -> AreaResult<()> {
        let machine = entry.borrow().machine.clone();
        self.call("unfork", &[&entry_id(entry), machine.id()]);
        if let Some(Err(e)) = self.next("unfork") {
            return Err(e);
        }
        machine.kill().await.map_err(AreaError::Engine)
    }
    async fn cut_off(&self, entry: &Entry) {
        self.call("cut_off", &[&entry_id(entry)]);
    }
    fn broker_lift(&self, workspace_id: &str) {
        self.call("broker_lift", &[workspace_id]);
    }
    async fn delete_workspace(&self, workspace_id: &str) -> AreaResult<()> {
        self.call("delete_workspace", &[workspace_id]);
        self.live.borrow_mut().shift_remove(workspace_id);
        if let Some(c) = self.store.borrow_mut().get_mut("workspaces") {
            c.shift_remove(workspace_id);
        }
        Ok(())
    }
    fn now(&self) -> f64 {
        self.clock.now()
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(ms, f)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        self.spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
    fn on_event(&self, kind: &str, f: Rc<dyn Fn(&Value)>) {
        self.listeners.borrow_mut().push((kind.to_string(), f));
    }
}
