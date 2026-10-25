// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/activity.ts at 65f6b89c7. Spec: spec/activity.md.
//!
//! The door's three methods are declared here; the wsp-core join copies them into `types::pending::ActivityDoor`
//! (ruling 2 of 6400658264), which the runtime's `activity` member names.
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::{Map, Value};
use wsp_account::account::core_types::AccountError;
use wsp_core::clock::Clock;
use wsp_core::types::api::DaemonEventListener;
use wsp_core::types::internal::EndThread;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::activity::{ActivityEnded, ActivityView, ProcessesAsk};
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::preferences::ThreadFacts;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::daemon::ProcScope;
use wsp_words::exit::HostError;

use crate::threads::threads::{Entry, Sessions, ThreadRecords};

/// activity.ts:63.
pub const BOX_END_POLL_MS: i64 = 200;
/// activity.ts:66.
pub const SNAPSHOT_WAIT_MS: i64 = 10_000;
/// activity.ts:68.
pub const NO_DAEMON_HERE: &str = "this host runs no daemon on this computer, so it reads no processes";

/// What a read or an End is asked (activity.ts:73).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActivityAsk {
    pub computer: Option<String>,
    pub thread: Option<String>,
    pub thread_ids: Option<Vec<String>>,
}

/// An End's target (activity.ts:74).
#[derive(Clone, Debug, PartialEq)]
pub struct EndAsk {
    pub computer: Option<String>,
    pub pid: u32,
    pub started_at: f64,
}

/// The verb a meet runs before (activity.ts:79).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeetVerb {
    Settle,
    Delete,
}

/// activity.ts:70-80.
#[async_trait(?Send)]
pub trait ActivityDoor {
    async fn read(&self, ask: ActivityAsk, caller: Option<Caller>) -> Result<ActivityView, HostError>;
    async fn end(&self, ask: EndAsk, caller: Option<Caller>) -> Result<ActivityEnded, HostError>;
    async fn meet(&self, thread_ids: &[String], processes: ProcessesAsk, verb: MeetVerb, caller: Option<Caller>) -> Result<Vec<ActivityEnded>, HostError>;
}

/// One daemon channel with its asks (TS `DaemonChannel` read through `overChannel`'s `ask`, which throws a refusal).
pub trait DaemonAsk {
    fn ask(&self, frame: Value) -> futures::future::LocalBoxFuture<'static, Result<Map<String, Value>, AccountError>>;
    fn close(&self);
}

/// What the activity door reads of the runtime.
#[async_trait(?Send)]
pub trait ActivityCtx {
    fn clock(&self) -> Rc<dyn Clock>;
    async fn ready(&self) -> Result<(), HostError>;
    /// Whether the host has a place door (TS `ctx.placeDoor !== undefined`).
    fn has_door(&self) -> bool;
    /// `ctx.placeDoor?.nameOf(place)`.
    fn place_name(&self, place: &str) -> Option<String>;
    async fn place_rows(&self) -> Result<Vec<PlaceView>, HostError>;
    fn joined(&self, place: &str) -> bool;
    /// `door.link(place) !== undefined`.
    fn has_link(&self, place: &str) -> bool;
    /// `ctx.opts.local?.daemonRoad !== undefined`.
    fn has_local_daemon(&self) -> bool;
    /// `ctx.channelOver(await ctx.localRoad(), THIS_COMPUTER, onEvent)`, through `ctx.killGuarded(.., true)` where
    /// `guarded`.
    async fn local_channel(&self, on_event: DaemonEventListener, guarded: bool) -> Result<Rc<dyn DaemonAsk>, HostError>;
    /// `door.channel(place, onEvent)`; None where the door has none for it.
    fn box_channel(&self, place: &str, on_event: DaemonEventListener) -> Option<futures::future::LocalBoxFuture<'static, Result<Rc<dyn DaemonAsk>, HostError>>>;
    fn sessions(&self) -> Sessions;
    fn records(&self) -> ThreadRecords;
    fn live_entry(&self, workspace_id: &str) -> Option<Entry>;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn thread_facts(&self, thread_id: &str) -> Option<ThreadFacts>;
    /// `ctx.scopesHere()`: the roots the daemon is told, the one rule.
    fn scopes_here(&self) -> Vec<ProcScope>;
    /// `ctx.keptAgents.get(threadId)?.pid`.
    fn kept_pid(&self, thread_id: &str) -> Option<u32>;
    /// `ctx.reapKept(threadId, { now: true })`.
    fn reap_kept_now(&self, thread_id: &str);
    /// `process.pid`.
    fn host_pid(&self) -> u32;
    fn drives(&self, kind: WorkspaceKind, caller: Option<&Caller>) -> bool;
    fn refusal_for(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> Option<HostError>;
    fn settles_thread(&self, thread_id: &str, caller: Option<&Caller>) -> bool;
    /// `ctx.moduleOf(entry.record.kind).endThread?.(entry, threadId, o)`; None where the kind has none.
    fn end_thread(&self, entry: &Entry, thread_id: &str, o: EndThread) -> Option<futures::future::LocalBoxFuture<'static, Result<Option<String>, HostError>>>;
    /// `ctx.withDaemon(entry, ask => ask(frame))`.
    async fn with_daemon(&self, entry: &Entry, frame: Value) -> Result<Map<String, Value>, HostError>;
    /// `ctx.slates.closeServers(threadId)`.
    fn close_servers(&self, thread_id: &str);
}

/// activity.ts:97.
#[derive(Clone)]
pub struct Activity {
    ctx: Rc<dyn ActivityCtx>,
}

impl Activity {
    pub fn new(ctx: Rc<dyn ActivityCtx>) -> Self {
        todo!()
    }
}

#[async_trait(?Send)]
impl ActivityDoor for Activity {
    /// activity.ts:358. Rules AC1, AC2, AC4-AC20, AC22-AC25.
    async fn read(&self, ask: ActivityAsk, caller: Option<Caller>) -> Result<ActivityView, HostError> {
        todo!()
    }

    /// activity.ts:362. Rules AC3, AC26, AC27.
    async fn end(&self, ask: EndAsk, caller: Option<Caller>) -> Result<ActivityEnded, HostError> {
        todo!()
    }

    /// activity.ts:367. Rules AC28-AC32, AC21.
    async fn meet(&self, thread_ids: &[String], processes: ProcessesAsk, verb: MeetVerb, caller: Option<Caller>) -> Result<Vec<ActivityEnded>, HostError> {
        todo!()
    }
}
