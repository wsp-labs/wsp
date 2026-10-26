// SPDX-License-Identifier: AGPL-3.0-only
//! `SubQueueCtx` over the fake host. `stand_up_queued` and `start_turn` answer as scripted: by default the machine
//! stands at once (a record running on a new fake machine) and the turn starts.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::{json, Value};

use wsp_engine::box_backend::BoxLimits;
use wsp_core::clock::Cancel;
use wsp_core::types::internal::{SessionEntry, SubStartRecord, SUB_STARTS};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::sub_queue::{StartedTurn, SubQueueCtx};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::ThreadScope;

use super::*;

/// A session row with only its view and turn id set, as the fakes hold one (every other field absent).
pub fn session_entry(view: SessionView, turn_id: String) -> SessionEntry {
    SessionEntry {
        view,
        turn_id,
        replaces: None,
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        handle: None,
        end: None,
        turn_live: None,
        run: None,
        from: None,
        asked: None,
        snapshot: None,
        pid: None,
        launch: None,
        calls: None,
        unanswered: false,
        queued: false,
    }
}

/// A copy of a row, field by field: `SessionEntry` is not `Clone`, and the table keeps the one row it is changed in.
pub fn snapshot_of(e: &SessionEntry) -> SessionEntry {
    SessionEntry {
        view: e.view.clone(),
        turn_id: e.turn_id.clone(),
        replaces: e.replaces.clone(),
        notify: e.notify.clone(),
        notify_by: e.notify_by.clone(),
        notify_road: e.notify_road,
        turn_token: e.turn_token.clone(),
        scope_device_id: e.scope_device_id.clone(),
        handle: e.handle.clone(),
        end: e.end.clone(),
        turn_live: e.turn_live.clone(),
        run: e.run.clone(),
        from: e.from,
        asked: e.asked.clone(),
        snapshot: e.snapshot.clone(),
        pid: e.pid,
        launch: e.launch.clone(),
        calls: e.calls.clone(),
        unanswered: e.unanswered,
        queued: e.queued,
    }
}

/// The first row of a workspace in the fake's table, copied.
pub fn row_on(host: &FakeHost, workspace_id: &str) -> Option<SessionEntry> {
    host.sessions.borrow().values().find(|s| s.view.workspace_id == workspace_id).map(snapshot_of)
}

#[async_trait(?Send)]
impl SubQueueCtx for FakeHost {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn live_all(&self) -> Vec<Entry> {
        self.live.borrow().values().cloned().collect()
    }
    fn session(&self, turn_id: &str) -> Option<SessionEntry> {
        self.sessions.borrow().get(turn_id).map(snapshot_of)
    }
    fn set_session(&self, turn_id: &str, entry: SessionEntry) {
        self.sessions.borrow_mut().insert(turn_id.to_string(), entry);
    }
    fn update_session(&self, turn_id: &str, f: &mut dyn FnMut(&mut SessionEntry)) {
        if let Some(e) = self.sessions.borrow_mut().get_mut(turn_id) {
            f(e);
        }
    }
    fn remove_session(&self, turn_id: &str) {
        self.sessions.borrow_mut().shift_remove(turn_id);
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        record.place.clone()
    }
    /// phaseHoldsSlot (place-state.ts:87): every phase but napping, gone and released, and a queued record still waiting with `queue`.
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        let phase = serde_json::to_value(&record.phase).unwrap();
        let free = ["napping", "gone", "released"].iter().any(|p| phase == json!(p));
        !free && !(phase == json!("queued") && record.queue.is_some())
    }
    fn place_name_of(&self, place_id: &str) -> Option<String> {
        (place_id == "box").then(|| "Boat".to_string())
    }
    fn machines_at(&self, place_id: &str) -> Option<f64> {
        self.machines_at.borrow().get(place_id).copied()
    }
    async fn limits_of(&self, _record: &WorkspaceRecord) -> AreaResult<Option<BoxLimits>> {
        self.call("limits_of", &[]);
        let l = self.backend.limits.lock().unwrap().clone();
        Ok(l)
    }
    fn view(&self, record: &WorkspaceRecord) -> Value {
        json!({ "id": record.id, "name": record.name, "phase": record.phase })
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        self.call("persist", &[&record.id]);
        if let Some(Err(e)) = self.next("persist") {
            return Err(e);
        }
        self.persist_now(record);
        Ok(())
    }
    async fn persist_sessions(&self, workspace_id: &str) -> AreaResult<()> {
        self.call("persist_sessions", &[workspace_id]);
        Ok(())
    }
    fn emit(&self, event: Value) {
        self.emit_now(event);
    }
    fn record(&self, event: Value) {
        self.recorded.borrow_mut().push(event.clone());
        self.emit_now(event);
    }
    fn attach_absent(&self, record: &Rc<RefCell<WorkspaceRecord>>, said: &str) -> Entry {
        let id = record.borrow().id.clone();
        self.call("attach_absent", &[&id, said]);
        let entry = live_entry(record.clone(), FakeMachine::new(""));
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Entry {
        let id = record.borrow().id.clone();
        self.call("attach_dead", &[&id]);
        let entry = live_entry(record.clone(), FakeMachine::new(""));
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    fn record_cell(&self, workspace_id: &str) -> Option<Rc<RefCell<WorkspaceRecord>>> {
        self.live.borrow().get(workspace_id).map(|e| e.borrow().record.clone())
    }
    async fn drop_workspace(&self, workspace_id: &str) -> AreaResult<()> {
        self.call("drop_workspace", &[workspace_id]);
        self.live.borrow_mut().shift_remove(workspace_id);
        if let Some(c) = self.store.borrow_mut().get_mut("workspaces") {
            c.shift_remove(workspace_id);
        }
        Ok(())
    }
    fn notify_failed(&self, view: &SessionView, turn_id: &str, notify: &[String], _by: Option<&ThreadScope>, _road: Option<&Value>, error: &str) {
        let targets = notify.join(",");
        self.call("notify_failed", &[&view.workspace_id, turn_id, &targets, error]);
    }
    fn push_head(&self, thread_id: &str) {
        self.call("push_head", &[thread_id]);
    }
    async fn rows_sent(&self) {
        self.call("rows_sent", &[]);
    }
    async fn put_start(&self, record: &SubStartRecord) -> AreaResult<()> {
        self.call("put_start", &[&record.workspace_id]);
        if let Some(Err(e)) = self.next("put_start") {
            return Err(e);
        }
        self.store.borrow_mut().entry(SUB_STARTS.to_string()).or_default().insert(record.workspace_id.clone(), serde_json::to_value(record).unwrap());
        Ok(())
    }
    async fn delete_start(&self, workspace_id: &str) -> AreaResult<()> {
        self.call("delete_start", &[workspace_id]);
        if let Some(c) = self.store.borrow_mut().get_mut(SUB_STARTS) {
            c.shift_remove(workspace_id);
        }
        Ok(())
    }
    async fn list_starts(&self) -> AreaResult<Vec<SubStartRecord>> {
        Ok(self.store.borrow().get(SUB_STARTS).map(|c| c.values().map(|v| serde_json::from_value(v.clone()).unwrap()).collect()).unwrap_or_default())
    }
    /// By default the machine stands at once: the record runs on a new machine of the backend's, and the start is
    /// posted. A scripted error is the stand-up's failure; a scripted `"never"` holds it until the gate in
    /// `stand_up_gate` opens.
    async fn stand_up_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, posted: Rc<dyn Fn()>, waited: bool) -> AreaResult<()> {
        let id = record.borrow().id.clone();
        self.call("stand_up_queued", &[&id, if waited { "waited" } else { "at once" }]);
        posted();
        match self.next("stand_up_queued") {
            Some(Err(e)) => return Err(e),
            Some(Ok(Value::String(s))) if s == "gate" => {
                let g = self.stand_up_gate.borrow().clone().expect("a gate to wait on");
                let _ = g.acquire().await;
            }
            _ => {}
        }
        let n = self.backend.machines.lock().unwrap().len() + 1;
        let m = FakeMachine::new(&format!("bx_{n}"));
        self.backend.add(m.clone());
        {
            let mut r = record.borrow_mut();
            r.machine_id = m.id.clone();
            r.phase = serde_json::from_value(json!("running")).unwrap();
        }
        let entry = live_entry(record.clone(), m);
        self.live.borrow_mut().insert(id, entry);
        Ok(())
    }
    async fn start_turn(&self, workspace_id: &str, o: serde_json::Map<String, Value>, origin: Option<Caller>) -> AreaResult<StartedTurn> {
        let as_ = o.get("as").cloned().unwrap_or(Value::Null);
        self.call("start_turn", &[workspace_id, &serde_json::to_string(&Value::Object(o.clone())).unwrap(), &caller_words(origin.as_ref())]);
        if let Some(Err(e)) = self.next("start_turn") {
            return Err(e);
        }
        Ok(StartedTurn { id: as_["turnId"].as_str().unwrap_or("").to_string(), workspace_id: workspace_id.to_string(), turn_id: as_["turnId"].as_str().unwrap_or("").to_string() })
    }
    async fn cut_off(&self, entry: &Entry) {
        self.call("cut_off", &[&entry_id(entry)]);
    }
    async fn unfork(&self, entry: &Entry) -> AreaResult<()> {
        let machine = entry.borrow().machine.clone();
        self.call("unfork", &[&entry_id(entry), machine.id()]);
        machine.kill().await.map_err(AreaError::Engine)
    }
    fn now(&self) -> f64 {
        self.clock.now()
    }
    fn uuid(&self) -> String {
        let n = self.calls_of("uuid").len() + 1;
        self.call("uuid", &[]);
        format!("00000000-0000-4000-8000-{n:012}")
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
