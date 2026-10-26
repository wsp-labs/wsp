// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/boot.ts` at 65f6b89c7: the host's start and the doors every verb comes through, in
//! four units (specs under `spec/`): `load` (boot_load.md: builders and hydration), `ready` (boot_ready.md: the
//! start), `entries` (boot_entries.md: the verb doors) and `rows` (boot_rows.md: listings, the row push, marks).

pub mod entries;
pub mod load;
pub mod ready;
pub mod rows;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};

use wsp_core::types::internal::{KindModule, Reopened, ThreadRecord};
use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::{FoundMachine, GoneOutcome, PlaceBackends, WorkspaceRecord};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::types::usage::LimitWindow;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::sub_row::SubRow;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::HostError;

use super::machines::{Entry, Ids, Row};

/// The turn, workspace and thread key of a held row (boot.ts `threadKeyOf`).
fn row_keys(s: &Row) -> (String, String, String) {
    let e = s.borrow();
    (
        e.turn_id.clone(),
        e.view.workspace_id.clone(),
        wsp_wire::views::session::thread_key_of(&e.view),
    )
}

/// A computer a project can live on, by its row in the places table (boot.ts:1038).
#[derive(Clone, Debug, PartialEq)]
pub struct ComputerRow {
    pub id: String,
    pub name: String,
    pub kind: PlaceKind,
}

/// A scoped device as the start reads it: its id and the thread its token is scoped to (boot.ts:543, 597).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedDevice {
    pub id: String,
    pub thread_id: Option<String>,
}

/// One limit row of the usage ledger: its account key and windows (boot.ts:1009).
#[derive(Clone, Debug, PartialEq)]
pub struct LimitRow {
    pub key: String,
    pub windows: Vec<LimitWindow>,
}

/// What `readTurnChanges` is asked for an unread card (boot.ts:514).
#[derive(Clone, Debug, PartialEq)]
pub struct CardAsk {
    pub session_id: String,
    pub turn_id: String,
    pub thread_id: String,
    pub cwd: String,
    pub from: String,
    pub started_at: f64,
}

/// What `ctx.notifyEnd` tells a held start's targets (boot.ts:592).
#[derive(Clone, Debug, PartialEq)]
pub struct HeldEnd {
    pub view: SessionView,
    pub turn_id: String,
    pub notify: Vec<String>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub result: TurnResult,
}

/// The numbers the start reads once.
#[derive(Clone, Debug, PartialEq)]
pub struct BootConfig {
    /// `ctx.hostId`.
    pub host_id: String,
}

/// Every call the boot area makes on the rest of the runtime, each named as the TS member it stands for.
#[async_trait(?Send)]
pub trait BootRuntime {
    // The context's own handles.
    fn store(&self) -> Rc<dyn Store>;
    fn clock(&self) -> Rc<dyn Clock>;
    fn ids(&self) -> Rc<dyn Ids>;
    fn places(&self) -> Rc<dyn PlaceBackends>;
    fn emit(&self, event: EventUnion) -> Result<(), HostError>;
    /// `bus.pass`.
    fn pass(&self, event: EventUnion) -> Result<(), HostError>;
    fn warn(&self, line: &str);
    fn owner(&self) -> String;
    fn set_owner(&self, owner: &str);
    fn reads_since(&self) -> f64;
    fn set_reads_since(&self, at: f64);
    fn closed(&self) -> bool;
    fn closing(&self) -> bool;
    /// `os.hostname()`.
    fn hostname(&self) -> String;
    /// `ctx.local !== undefined`.
    fn local_wired(&self) -> bool;

    // The shared maps.
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>>;
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>>;
    fn thread_records(&self) -> Rc<RefCell<IndexMap<String, ThreadRecord>>>;
    fn builders(&self) -> crate::images::Builders;
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>>;
    /// `ctx.bootWork`: what a starting host still has out on each machine.
    fn boot_work(
        &self,
    ) -> Rc<
        RefCell<
            IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>,
        >,
    >;
    /// `ctx.rereading` (the machines area's set).
    fn rereading(&self) -> Rc<RefCell<IndexSet<String>>>;
    fn viewed_marks(&self) -> Rc<RefCell<IndexMap<String, IndexMap<String, String>>>>;
    fn has_transcript_index(&self, workspace_id: &str) -> bool;
    /// `transcriptIndex.get(ws)?.pos ?? 0`.
    fn index_pos(&self, workspace_id: &str) -> u64;
    /// The transcript index's children of a thread, each without `turnId` and `startRow` (boot.ts:697, 704).
    fn index_children(&self, workspace_id: &str, thread_id: &str) -> Vec<serde_json::Value>;

    // The machines area.
    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine>;
    fn attach(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        machine: Arc<dyn Machine>,
    ) -> Result<Entry, HostError>;
    async fn mark_gone(
        &self,
        entry: &Entry,
        machine_id: &str,
        reason: &str,
    ) -> Result<GoneOutcome, HostError>;
    fn idle_touch(&self, workspace_id: &str);
    fn sync_daemon(&self, entry: &Entry);
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError>;
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError>;
    async fn shape_of(&self, machine: &Arc<dyn Machine>) -> Option<MachineShape>;
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize;
    fn arm_grace(&self, builder_id: &str, sealed_at: &str);
    /// The place door's `backendOf(place)`, where the places table has none.
    fn door_backend_of(&self, place: &str) -> Option<Arc<dyn MachineBackend>>;
    /// `imageOf(golden).version?.kind`.
    async fn image_kind(
        &self,
        golden: &str,
    ) -> Result<Option<wsp_wire::views::golden_image::MachineKind>, HostError>;
    fn attach_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Result<(), HostError>;

    // The start's other steps (boot.ts:343-618), in the order ready() calls them.
    async fn migrate_copies(&self) -> Result<(), HostError>;
    async fn place_door_load(&self) -> Result<(), HostError>;
    async fn git_top_of(&self, path: &str) -> Result<String, HostError>;
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError>;
    async fn setups_load(&self) -> Result<(), HostError>;
    async fn move_transcript(&self, record: serde_json::Value) -> Result<(), HostError>;
    fn move_blobs(&self);
    async fn load_index(&self, workspace_id: &str) -> Result<(), HostError>;
    fn wake_at(&self, thread_id: &str, until: f64);
    fn resume_on_reset(&self, thread_id: &str, at: f64);
    /// `ctx.settleCut(row, reason, line, stopped, unreached)`: `line` gets the end's time.
    fn settle_cut(
        &self,
        row: &Row,
        reason: &str,
        line: &dyn Fn(f64) -> String,
        stopped: Option<bool>,
        unreached: Option<bool>,
    );
    /// The per-workspace chain's write, which swallows its failure; `mark` awaits it, `ready` does not.
    async fn persist_sessions(&self, workspace_id: &str);
    /// `turnWritten(await ctx.openThread(ws, thread), turn).changes`.
    async fn turn_changes_written(
        &self,
        workspace_id: &str,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<bool, HostError>;
    async fn read_turn_changes(&self, entry: &Entry, ask: CardAsk) -> Result<bool, HostError>;
    async fn reattach(&self, row: &Row, again: bool) -> Reopened;
    async fn devices(&self) -> Result<Vec<ScopedDevice>, HostError>;
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError>;
    async fn revoke_asides(&self) -> Result<(), HostError>;
    async fn shell_held_devices(&self) -> Result<IndexSet<String>, HostError>;
    fn thread_runs(&self, thread_id: &str) -> bool;
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError>;
    async fn sweep_runs(&self, entry: &Entry) -> Result<(), HostError>;
    async fn resume_queue(&self) -> Result<(), HostError>;
    fn notify_end(&self, end: HeldEnd);
    /// `ctx.record(event)`: stamps and appends to the transcript, then emits.
    fn record(&self, event: EventUnion);
    fn arm_subs(&self);
    async fn deliver_owed(&self) -> Result<(), HostError>;
    async fn move_old_copies(&self) -> Result<(), HostError>;
    fn arm_sweep(&self);
    async fn slates_ready(&self) -> Result<(), HostError>;

    // The doors (boot.ts:628-672, 1017-1065).
    fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&Caller>) -> bool;
    fn acts_on(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> bool;
    fn talks_to_its_tree(&self, caller: Option<&Caller>) -> Result<bool, HostError>;
    fn refuse_relayed(
        &self,
        record: &WorkspaceRecord,
        origin: Option<&Caller>,
        act: Option<&str>,
        thread: Option<&str>,
    ) -> Result<(), HostError>;
    /// `ctx.workspaces.resolve(ref, origin)`: the id it names.
    async fn resolve_workspace(
        &self,
        reference: &str,
        origin: Option<&Caller>,
    ) -> Result<String, HostError>;
    /// `placeDoor.rows()`, or `None` with no place door wired.
    async fn place_rows(&self) -> Option<Vec<ComputerRow>>;
    /// `goldenHead(await ctx.copyOf(await ctx.imagePlace("default"), "default"))?.snapshotId`.
    async fn default_image_head(&self) -> Result<Option<String>, HostError>;
    /// `ctx.placeDoorOf().placeFor(place)`.
    async fn place_for(&self, place: &str) -> Result<Option<String>, HostError>;

    // The rows (boot.ts:673-1013).
    /// `ctx.restarts()`: thread to the thread that replaced it.
    fn restarts(&self) -> IndexMap<String, String>;
    fn stopped_behind(&self, row: &Row) -> Option<serde_json::Value>;
    /// `ctx.capHeld.get(turnId)?.waiting?.wait`.
    fn cap_wait(&self, turn_id: &str) -> Option<serde_json::Value>;
    fn agent_pid_of(&self, run: &str) -> Option<u32>;
    /// The refusal of the row's harness at its workspace's setup place (boot.ts:705-709).
    fn setup_refusal(&self, workspace_id: &str, harness: &str) -> Option<String>;
    /// `ctx.setupPlace(entry) === place`.
    fn setup_place(&self, entry: &Entry) -> Option<String>;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView>;
    fn sub_row_of(&self, record: &WorkspaceRecord) -> Option<SubRow>;
    fn disk_kept_until(&self, record: &WorkspaceRecord) -> Option<f64>;
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    async fn sub_read(&self, workspace_id: &str, since: f64) -> Result<(), HostError>;
    fn title_rows(&self, views: Vec<SessionView>) -> Vec<SessionView>;
    fn refresh_title(&self, view: SessionView);
    fn reap_kept(&self, thread_id: &str);
    /// `ctx.resumeTimers.get(thread)?.()`.
    fn cancel_resume(&self, thread_id: &str);
    fn lead_asks(&self, thread_id: &str) -> bool;
    async fn ledger_limits(&self) -> Vec<LimitRow>;
    /// `accountOnComputer({...}).key` over `vaultedFor`, `loginStands`, `vaultOn`, `usageComputerOf`, `computerOf`
    /// and the harness's catalog label (boot.ts:1010-1011).
    fn account_key_on(&self, entry: &Entry, harness: &str) -> String;
    /// `ctx.sessionsApi.start(ws, {prompt: LIMIT_RESUME_PROMPT, thread, afterLimit})`.
    async fn resume_turn(
        &self,
        workspace_id: &str,
        thread_id: &str,
        after_limit: f64,
    ) -> Result<(), HostError>;
}

/// The boot area: the state boot.ts keeps in its closure.
pub struct BootArea {
    pub(crate) rt: Rc<dyn BootRuntime>,
    pub(crate) config: BootConfig,
    /// `hydrated`: the one start.
    pub(crate) hydrated: RefCell<Option<ready::Ready>>,
    /// `admitting`: one read per stored builder at a time.
    pub(crate) admitting: RefCell<
        IndexMap<
            String,
            futures::future::Shared<
                futures::future::LocalBoxFuture<'static, Result<(), HostError>>,
            >,
        >,
    >,
    /// `passes` and `latest`: the newest builders listing wins.
    pub(crate) passes: std::cell::Cell<u64>,
    /// The state a held record's re-read sighted, handed to the hydration it starts (`FoundMachine.sighting.state`,
    /// which wsp-core's `Sighting` does not carry yet): a second read would reset the provider's idle timer.
    pub(crate) sighted: RefCell<Option<wsp_engine::machine::MachineState>>,
    /// `heldAnswers`: what the provider said to the load's read of a record now held.
    pub(crate) held_answers: RefCell<IndexMap<String, String>>,
    /// `toldRows`, `toldBehind`, `rowsMoved`, `rowsGone`, `rowsQueued`: the row push.
    pub(crate) told_rows: RefCell<IndexSet<String>>,
    pub(crate) told_behind: RefCell<IndexSet<String>>,
    pub(crate) rows_moved: RefCell<IndexMap<String, String>>,
    pub(crate) rows_gone: RefCell<IndexMap<String, (String, Option<String>)>>,
    pub(crate) rows_queued: std::cell::Cell<bool>,
    /// `rowsOut`: the last link of the rows chain, each push waits for the one before it.
    pub(crate) rows_out:
        RefCell<Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>>,
    /// `latest`: the newest builders pass, which an older pass that a newer one overtook hands its caller.
    pub(crate) latest: RefCell<
        Option<
            futures::future::Shared<
                futures::future::LocalBoxFuture<'static, Result<(), HostError>>,
            >,
        >,
    >,
}

impl BootArea {
    /// `bootArea(ctx)` (boot.ts:29).
    pub fn new(rt: Rc<dyn BootRuntime>, config: BootConfig) -> Rc<Self> {
        Rc::new(BootArea {
            rt,
            config,
            hydrated: RefCell::default(),
            admitting: RefCell::default(),
            passes: std::cell::Cell::new(0),
            sighted: RefCell::default(),
            held_answers: RefCell::default(),
            told_rows: RefCell::default(),
            told_behind: RefCell::default(),
            rows_moved: RefCell::default(),
            rows_gone: RefCell::default(),
            rows_queued: std::cell::Cell::new(false),
            rows_out: RefCell::default(),
            latest: RefCell::default(),
        })
    }

    /// The `*` listener (boot.ts:873), boot_rows.md rule 8: the wiring calls it for every event in emit order.
    pub fn on_event(self: &Rc<Self>, event: &EventUnion) {
        let kind = wsp_core::types::events::event_type(event);
        let Ok(e) = serde_json::to_value(event) else {
            return;
        };
        let text = |key: &str| {
            e.get(key)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        };
        match kind {
            "thread.head" => {
                let (Some(ws), Some(thread)) = (text("workspaceId"), text("threadId")) else {
                    return;
                };
                self.row_moved(&ws, &thread);
                for s in self.held_rows() {
                    let (turn, row_ws, key) = row_keys(&s);
                    let behind = self.rt.stopped_behind(&s).and_then(|b| {
                        b.get("threadId")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    });
                    let told = self.told_behind.borrow().contains(&turn);
                    if told && behind.as_deref() == Some(thread.as_str()) {
                        self.row_moved(&row_ws, &key);
                    }
                }
            }
            "session.held" | "session.capped" | "session.uncapped" | "session.subagent" => {
                if let (Some(ws), Some(thread)) = (text("workspaceId"), text("threadId")) {
                    self.row_moved(&ws, &thread);
                }
            }
            "thread.marked" => {
                let Some(ws) = text("workspaceId") else {
                    return;
                };
                for thread in e
                    .get("threadIds")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                {
                    self.row_moved(&ws, thread);
                }
            }
            "workspace.napped" | "workspace.woken" | "workspace.upgraded" => {
                let Some(ws) = text("workspaceId") else {
                    return;
                };
                for s in self.held_rows() {
                    let (_, row_ws, key) = row_keys(&s);
                    if row_ws == ws {
                        self.row_moved(&row_ws, &key);
                    }
                }
            }
            "session.start" => {
                let (Some(turn), Some(session)) = (text("turnId"), text("sessionId")) else {
                    return;
                };
                if turn != session {
                    if let Some(ws) = text("workspaceId") {
                        self.row_gone(&turn, &ws, text("threadId").as_deref());
                    }
                }
            }
            "session.end" => {
                let (Some(turn), Some(session)) = (text("turnId"), text("sessionId")) else {
                    return;
                };
                let held = self.held_rows().iter().any(|s| row_keys(s).0 == turn);
                if !held {
                    if let Some(ws) = text("workspaceId") {
                        self.row_gone(&session, &ws, text("threadId").as_deref());
                    }
                }
            }
            "session.permission" | "session.permission.closed" => {
                if let (Some(ws), Some(thread)) = (text("workspaceId"), text("threadId")) {
                    self.row_moved(&ws, &thread);
                }
                for s in self.held_rows() {
                    let (turn, row_ws, key) = row_keys(&s);
                    let running =
                        s.borrow().view.status == wsp_wire::views::session::SessionStatus::Running;
                    let told = self.told_behind.borrow().contains(&turn);
                    if running && (told || self.rt.stopped_behind(&s).is_some()) {
                        self.row_moved(&row_ws, &key);
                    }
                }
            }
            "session.delta" => {
                let Some(thread) = text("threadId") else {
                    return;
                };
                let Some(ws) = text("workspaceId") else {
                    return;
                };
                let moves = if text("kind").as_deref() == Some("tool_use") {
                    text("toolName").is_some_and(|tool| {
                        wsp_words::wsp_tools::threads_followed(
                            &tool,
                            &text("text").unwrap_or_default(),
                        )
                        .is_some()
                    })
                } else {
                    text("kind").as_deref() == Some("tool_result")
                        && text("turnId")
                            .is_some_and(|turn| self.told_behind.borrow().contains(&turn))
                };
                if moves {
                    self.row_moved(&ws, &thread);
                }
            }
            _ => {}
        }
    }
}

/// A sighting handed to the load in place of its own read (wsp-core's `FoundMachine`), re-exported for the units.
pub type Found = FoundMachine;
