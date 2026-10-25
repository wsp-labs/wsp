// SPDX-License-Identifier: AGPL-3.0-only
//! The live entry the account tests build and the machine it holds: one fake over the engine's `Machine`, and
//! `entry_over`, which makes the real `LiveWorkspace` the way `attach` does. The engine's `Workspace::new` is a stub
//! today, so building an entry stops there until it lands; the test is red either way.
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use wsp_core::types::wiring::{LiveWorkspace, WorkspaceRecord};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{
    ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions,
};
use wsp_machines::machines::machines::Entry;

/// A machine that answers nothing a slate asks of it: its id and its kind only.
pub struct FakeMachine {
    pub id: String,
}

impl FakeMachine {
    pub fn new(id: &str) -> Arc<FakeMachine> {
        Arc::new(FakeMachine { id: id.to_string() })
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers::default()
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EngineResult<String> {
        unimplemented!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EngineResult<()> {
        unimplemented!()
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> EngineResult<()> {
        unimplemented!()
    }
    async fn kill(&self) -> EngineResult<()> {
        unimplemented!()
    }
    async fn state(&self) -> EngineResult<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
    async fn upload_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
    async fn daemon_frame(&self, _frame: Map<String, Value>) -> EngineResult<Map<String, Value>> {
        unimplemented!()
    }
}

/// A workspace record from the keys a test names, the rest at what a running local workspace holds.
pub fn record(given: Value) -> WorkspaceRecord {
    let mut full = json!({ "id": "w1", "name": "w1", "kind": "local", "machineId": "m1", "phase": "running", "golden": "", "createdAt": "2026-10-25T00:00:00.000Z", "project": "p0", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false });
    for (k, v) in given.as_object().expect("an object") {
        full[k] = v.clone();
    }
    serde_json::from_value(full).unwrap_or_else(|e| panic!("record {given}: {e}"))
}

/// A live entry over a record and a machine, every in-memory field at its start: what `attach` makes.
pub fn entry_over(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
    let hooks = WorkspaceHooks {
        golden_snapshot: String::new(),
        wake_attempts: 1,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    };
    let ws = Rc::new(Workspace::new(machine.clone(), hooks, WorkspaceInitial::default()));
    Rc::new(RefCell::new(LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
        opens_thread: None,
        of_record_held: false,
        ws,
        machine,
        generation: 0,
        waking: None,
        unchecked: false,
        napping: None,
        adopting: None,
        deleting: None,
        late_read: None,
        wake_stop: None,
        resize_to: None,
        nap_wait_ms: None,
        wake_said: None,
        delete_said: None,
        wake_ask: None,
        creating: false,
        checkout: None,
        checkout_reading: None,
        tree: None,
        tree_reading: None,
        pr: None,
        pr_reading: None,
        pr_poll: None,
        pr_poll_ms: None,
        pr_none_at: None,
    }))
}

/// A live entry over a record, on a fake machine named by the record's `machineId`.
pub fn live(record: WorkspaceRecord) -> Entry {
    let machine = FakeMachine::new(&record.machine_id);
    entry_over(record, machine)
}
