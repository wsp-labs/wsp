// SPDX-License-Identifier: AGPL-3.0-only
//! The live entry every test builds and the machine it holds: one fake over the engine's `Machine`, scripted by what a
//! test sets, and `entry_over`, which makes the real `LiveWorkspace` the way `attach` does. The engine's
//! `Workspace::new` is a stub today, so building an entry stops there until it lands; the test is red either way.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{Map, Value};

use wsp_account::account::core_types::{AccountError, Entry, Served};
use wsp_core::types::wiring::{Joined, LiveWorkspace, WorkspaceRecord};
use wsp_engine::errors::{EngineError, Result as EngineResult};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{
    ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions,
};
use wsp_words::exit::HostError;

/// What a test scripts an exec with: the command and its timeout in, the answer or a failure sentence out.
pub type ExecScript = Rc<dyn Fn(String, i64) -> LocalBoxFuture<'static, Result<ExecResult, String>>>;

/// A machine that answers what the test set: its state, its exec, and the frames its computer serves.
pub struct FakeMachine {
    pub id: String,
    pub served: RefCell<Option<Served>>,
    pub state: RefCell<Result<MachineState, String>>,
    pub state_calls: Cell<usize>,
    pub execs: RefCell<Vec<String>>,
    pub exec_answer: RefCell<Result<ExecResult, String>>,
    pub exec_script: RefCell<Option<ExecScript>>,
}

impl FakeMachine {
    pub fn new(id: &str) -> Arc<FakeMachine> {
        Arc::new(FakeMachine {
            id: id.to_string(),
            served: RefCell::new(None),
            state: RefCell::new(Ok(MachineState::Running)),
            state_calls: Cell::new(0),
            execs: RefCell::new(Vec::new()),
            exec_answer: RefCell::new(Ok(ok_exec())),
            exec_script: RefCell::new(None),
        })
    }
}

pub fn ok_exec() -> ExecResult {
    ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() }
}

fn refused(message: String) -> EngineError {
    EngineError::Other { message, code: None }
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
        MachineOffers { daemon_frame: self.served.borrow().is_some(), ..MachineOffers::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> EngineResult<ExecResult> {
        self.execs.borrow_mut().push(cmd.to_string());
        let script = self.exec_script.borrow().clone();
        let answer = match script {
            Some(run) => run(cmd.to_string(), opts.timeout_ms.unwrap_or(0) as i64).await,
            None => self.exec_answer.borrow().clone(),
        };
        answer.map_err(refused)
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
        self.state_calls.set(self.state_calls.get() + 1);
        self.state.borrow().clone().map_err(refused)
    }
    async fn download_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
    async fn upload_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
    async fn daemon_frame(&self, frame: Map<String, Value>) -> EngineResult<Map<String, Value>> {
        let served = self.served.borrow().clone();
        match served {
            Some(serve) => serve(frame).await.map_err(|e| {
                refused(match e {
                    AccountError::Host(h) => h.message,
                    AccountError::Refusal(r) => r.message,
                })
            }),
            None => Err(refused("not on this road".into())),
        }
    }
}

/// A join that is in flight: the move it stands for never lands, so a reader sees the flag set.
pub fn in_flight<T: Clone + 'static>() -> Joined<T> {
    futures::future::pending::<Result<T, HostError>>().boxed_local().shared()
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
