// SPDX-License-Identifier: AGPL-3.0-only
//! What every unit of the account slice shares: the error a unit answers, the frame and road shapes, and a stand-in
//! for the live entry. `LiveEntry` holds only the fields these areas read and write off `LiveWorkspace`
//! (types/wiring.ts:416-477) and `EntryMachine` the machine calls they make; once the wsp-core join shares one record
//! per workspace (ruling 6401022701, item 2) both become re-exports of wsp-core's and nothing importing from here
//! changes.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use serde_json::{Map, Value};

use wsp_core::types::internal::DaemonRefusal;
use wsp_core::types::wiring::{DeleteSaid, WakeAsk, WorkspaceRecord};
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::pull_request::PullRequestSeen;
use wsp_words::exit::HostError;

/// A daemon frame or reply as the TS object, keys in the order they were set (README A8).
pub type Frame = Map<String, Value>;

/// A timer's or a hold's undo, callable more than once (`wsp_core::clock::Cancel`).
pub type Cancel = wsp_core::clock::Cancel;

/// What a guard or an owned process hands back: calling it gives the place back.
pub type Release = Rc<dyn Fn()>;

/// The error an account area answers: a host error as the TS `Error` (README A4), or a daemon's refusal with the code
/// it put on it (`DaemonRefusal`, types/internal.ts:1074), which a caller tells apart by its code.
#[derive(Clone, Debug)]
pub enum AccountError {
    Host(HostError),
    Refusal(DaemonRefusal),
}

impl AccountError {
    /// `e instanceof Error ? e.message : String(e)`.
    pub fn message(&self) -> String {
        todo!()
    }

    /// The daemon's code where this is a refusal that carried one.
    pub fn code(&self) -> Option<&str> {
        todo!()
    }

    /// `isNoHostCli(e)`: a refusal coded `no-host-cli`.
    pub fn is_no_host_cli(&self) -> bool {
        todo!()
    }

    /// The host error, where this is one.
    pub fn host(&self) -> Option<&HostError> {
        todo!()
    }
}

impl From<HostError> for AccountError {
    fn from(e: HostError) -> Self {
        AccountError::Host(e)
    }
}

impl From<DaemonRefusal> for AccountError {
    fn from(e: DaemonRefusal) -> Self {
        AccountError::Refusal(e)
    }
}

pub type AccountResult<T> = Result<T, AccountError>;

/// One frame to a daemon and its reply passed through `replyOf` (channels.ts:208): the `ask` a road hands its work.
pub type Ask = Rc<dyn Fn(Frame) -> LocalBoxFuture<'static, AccountResult<Frame>>>;

/// A machine that answers its own daemon frames over its computer's link (`machine.daemonFrame`): the reply as the
/// computer wrote it, `ok: false` included.
pub type Served = Rc<dyn Fn(Frame) -> LocalBoxFuture<'static, AccountResult<Frame>>>;

/// The work a road runs with its `ask`.
pub type Work<T> = Box<dyn FnOnce(Ask) -> LocalBoxFuture<'static, AccountResult<T>>>;

/// An exec's answer as these areas read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExecAnswer {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

/// The machine calls the account areas make on `entry.machine`.
#[async_trait(?Send)]
pub trait EntryMachine {
    fn id(&self) -> String;
    /// `machine.daemonFrame`, where the machine answers its daemon's frames itself.
    fn daemon_frame(&self) -> Option<Served>;
    /// `machine.state()`: the provider's word for it (`running`, `paused`, ...).
    async fn state(&self) -> AccountResult<String>;
    /// `machine.exec(cmd, { timeoutMs })`.
    async fn exec(&self, cmd: &str, timeout_ms: i64) -> AccountResult<ExecAnswer>;
}

/// The in-flight pull request read an entry holds (`entry.prReading`), joined by a second reader.
pub type PrReading = Shared<LocalBoxFuture<'static, Option<PullRequestSeen>>>;

/// A workspace this host holds in memory, as the account areas see it (types/wiring.ts:416-477). Never stored.
pub struct LiveEntry {
    pub record: RefCell<WorkspaceRecord>,
    pub machine: Rc<dyn EntryMachine>,
    pub creating: Cell<bool>,
    pub waking: Cell<bool>,
    pub deleting: Cell<bool>,
    pub napping: Cell<bool>,
    pub generation: Cell<u64>,
    pub wake_ask: Cell<Option<WakeAsk>>,
    pub wake_said: RefCell<Option<String>>,
    pub delete_said: RefCell<Option<DeleteSaid>>,
    pub checkout: RefCell<Option<Checkout>>,
    pub pr: RefCell<Option<PullRequestSeen>>,
    pub pr_reading: RefCell<Option<PrReading>>,
    pub pr_poll: RefCell<Option<Cancel>>,
    pub pr_poll_ms: Cell<Option<i64>>,
    pub pr_none_at: RefCell<Option<String>>,
}

/// The live map's handle on one entry; `Rc::ptr_eq` is the TS's `live.get(id) === entry`.
pub type Live = Rc<LiveEntry>;
