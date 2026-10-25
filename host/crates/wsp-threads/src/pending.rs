// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins this slice needs from wsp-core in a shape wsp-core does not have yet. Spec: spec/threads-a-README.md,
//! "How the areas reach the runtime". The wsp-core join takes each one into `types/internal.rs` or `types/wiring.rs`
//! and this file becomes re-exports.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use wsp_adapters::port::HarnessExec;
use wsp_core::types::internal::{CallSeen, EndRow, ThreadRecord, TurnAsked, TurnLive};
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_words::exit::HostError;

/// A future on the host's one thread that may fail.
pub type Local<'a, T> = LocalBoxFuture<'a, Result<T, HostError>>;
/// A future more than one waiter awaits; it never fails.
pub type Joined = Shared<LocalBoxFuture<'static, ()>>;

/// One live workspace as these areas read it (TS `LiveWorkspace`). The real one is wsp-core's shared entry.
pub trait EntryHandle {
    fn id(&self) -> String;
    /// The record as it stands now, a copy (TS `entry.record`, read at the moment of the read).
    fn record(&self) -> WorkspaceRecord;
    /// TS `entry.record.claudeSessionId = id` (turns.ts:303).
    fn set_session(&self, session_id: &str);
    /// TS `entry.unchecked === true`.
    fn unchecked(&self) -> bool;
    /// TS `delete entry.unchecked` (turns.ts:828).
    fn set_checked(&self);
    /// `entry.opensThread`: the thread a workspace made for its first send opens.
    fn opens_thread(&self) -> Option<String>;
    /// `harnessExec(entry.machine, ms)`: the machine's command road for a harness read bounded by `ms`.
    fn harness_exec(&self, ms: u64) -> Arc<dyn HarnessExec>;
}

pub type Entry = Rc<dyn EntryHandle>;

/// A row's view, shared by the start road, the row and the turn's closures (one object in the TS).
pub type SharedView = Rc<RefCell<SessionView>>;
pub type SharedLive = Rc<RefCell<TurnLive>>;
pub type SharedAsked = Rc<RefCell<TurnAsked>>;
/// The box a turn's exec stream reads to know it waits on something outside its process.
pub type Waiting = Rc<Cell<bool>>;

/// One row the runtime holds (TS `SessionEntry`, internal.ts:1259), with its view, live state and asked shared.
#[derive(Clone)]
pub struct SessionEntry {
    pub view: SharedView,
    pub turn_id: String,
    pub replaces: Option<String>,
    pub notify: Option<Vec<String>>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub turn_token: Option<String>,
    pub scope_device_id: Option<String>,
    pub handle: Option<Rc<dyn SessionHandle>>,
    pub end: Option<EndRow>,
    pub turn_live: Option<SharedLive>,
    pub run: Option<String>,
    pub from: Option<u64>,
    pub asked: Option<SharedAsked>,
    pub snapshot: Option<String>,
    pub pid: Option<u32>,
    pub launch: Option<Joined>,
    pub calls: Option<Rc<RefCell<IndexMap<String, CallSeen>>>>,
    pub unanswered: bool,
    pub queued: bool,
}

impl SessionEntry {
    /// A row with only its view and turn id, every other field absent.
    pub fn new(view: SessionView, turn_id: &str) -> Self {
        todo!()
    }
}

/// `ctx.sessions`: every row by its id, in JS Map order (remove with `shift_remove`).
pub type Rows = Rc<RefCell<IndexMap<String, SessionEntry>>>;

/// `ThreadRecord & { workspaceId }`, as `ctx.threadRecords` holds it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeldThread {
    pub workspace_id: String,
    pub record: ThreadRecord,
}

/// `ctx.threadRecords`: every thread's record by its id, in JS Map order.
pub type Records = Rc<RefCell<IndexMap<String, HeldThread>>>;
