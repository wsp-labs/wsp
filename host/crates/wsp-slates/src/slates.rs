// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slates.ts at 65f6b89c7: a thread's slate on the host, schema 2. Six units, each an
//! `impl SlatesInner` block in its own file: `model` (spec/slates_model.md), `access` (spec/slates_access.md),
//! `reads` (spec/slates_reads.md), `batch` (spec/slates_batch.md), `writes` (spec/slates_writes.md) and `acts`
//! (spec/slates_acts.md). Shared rules: spec/slates-a-README.md.
//!
//! The public items above `SlatesInner` are fixed. `SlatesInner`'s fields and every `pub(crate)` method are the TS
//! closure's state and functions written out; the implementer may reshape them. No `RefCell` borrow is held across an
//! `.await` or a call out (rule 3.3).
pub mod access;
pub mod acts;
pub mod batch;
pub mod model;
pub mod reads;
pub mod writes;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use tokio::task::spawn_local;
use wsp_core::clock::Clock;
use wsp_core::harness_catalog::ToolAsks;
use wsp_core::ids::OsIds;
use wsp_engine::machine::Machine;
use wsp_engine::place_machine::PlaceFolderMachine;
use wsp_store::store::Store;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::kept::Record;
use wsp_wire::types::slate_wire::{SessionSlateEvent, SlateRunEvent, SlateValuesEvent};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use crate::pending::wire::{SlateEventAnswer, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer};
use crate::slate_box::{box_ledger, box_road, launch_on_for, BoxLedger, Ready, SlateFilesNow};
use crate::slate_files::{box_pins, write_slate_files, BoxPins, HashOn};
use crate::slate_images::ImageOn;
use crate::slate_mcp::{create_slate_mcp, host_away, Away, McpSecrets, McpServerSpec, MissingOnFn, Release, SlateMcp, SlateMcpDeps};
use crate::slate_runs::{
    create_slate_runs, Reshape, ReshapeAnswer, RunApprovals, RunBy, RunRecord, RunRoad, SlateRuns, SlateRunsDeps, SlateSecrets, Stream,
    NO_FOLDER,
};
use crate::slate_sources::SlateSourceContext;
pub use crate::slate_sources::Views;
use crate::slate_writes::{write_budget, WriteBudget};
use access::RecordApprovals;
pub use model::{Approval, SlateRecord, SlateSnap, TurnMark};

// slates.ts:124
// /** What the runtime knows of a thread that the slate needs: where it runs, its tree, and the row its events ride. */
// export interface SlateThreadFacts {
//   workspaceId: string;
//   rootThreadId: string;
//   sessionId: string;
//   /** The running turn, when one runs. */
//   turnId?: string;
//   /** The folder a run `on` the thread starts in, on the thread's own computer. */
//   folder?: string;
//   /** The folder a run `on` the host starts in, on this computer: the thread's own here, else the project's folder
//    * here where the project is also here, else the host's own. */
//   hostFolder?: string;
//   /** The computer's own name, for the consent sheet. */
//   computer?: string;
//   /** When the thread's agent asks before an MCP tool, which its slate's tool runs follow; absent asks before each. */
//   toolAsks?: ToolAsks;
// }
/// What the runtime knows of a thread that the slate needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlateThreadFacts {
    pub workspace_id: String,
    pub root_thread_id: String,
    pub session_id: String,
    /// The running turn, when one runs.
    pub turn_id: Option<String>,
    /// The folder a run `on` the thread starts in, on the thread's own computer.
    pub folder: Option<String>,
    /// The folder a run `on` the host starts in, on this computer.
    pub host_folder: Option<String>,
    /// The computer's own name, for the consent sheet.
    pub computer: Option<String>,
    pub tool_asks: Option<ToolAsks>,
}

/// What `emit` pushes to the windows and nowhere else.
#[derive(Clone, Debug, PartialEq)]
pub enum SlateEmit {
    Values(SlateValuesEvent),
    Run(SlateRunEvent),
}

/// A send's message for the thread (slates.ts:172).
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    pub thread_id: String,
    pub workspace_id: String,
    pub prompt: String,
    pub request_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Started,
    Steered,
    Queued,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Delivered {
    pub outcome: DeliveryOutcome,
    pub turn_id: Option<String>,
}

// slates.ts:142
// export interface SlatesDeps {
//   store: Store;
//   now(): number;
//   /** Records an event in the thread's transcript, live to every window. */
//   record(event: SessionSlateEvent): void;
//   /** Pushes an event to the windows and nowhere else. */
//   emit(event: SlateValuesEvent | SlateRunEvent): void;
//   thread(threadId: string): SlateThreadFacts | undefined;
//   /** The host's workspaces loaded, which a thread's facts are read off; the slates recover only after. */
//   loaded?(): Promise<void>;
//   /** The machine a thread runs on where that is not this computer, which its slate's commands then run on too. */
//   machineOf?(threadId: string): Machine | undefined;
//   /** Where the thread runs on another computer: reads an image file there through its daemon, by its whole path. */
//   imageOn?(threadId: string): ImageOn | undefined;
//   /** Absent where the thread runs on this computer. */
//   hashOn?(threadId: string): HashOn | undefined;
//   /** That machine naps: a timer never wakes it, a press does, through wake. */
//   asleep?(threadId: string): boolean;
//   /** Readies that machine for a run a press starts: waits out the sweep a starting host has out there, which would end
//    * the run, then wakes it where it naps. */
//   wake?(threadId: string): Promise<void>;
//   /** Whether the thread is settled, by the sidebar's own rule: its slate's timers, always ones too, wait until a window
//    * shows the slate again. */
//   settled?(threadId: string): Promise<boolean>;
//   /** Every thread under the lead, the lead included. */
//   under(lead: string): string[];
//   threadOfToken(token: string): string;
//   /** What the source resolvers read for this thread. */
//   sources(threadId: string, workspaceId: string): SlateSourceContext;
//   /** Puts a send's message into the thread: a start that the runtime steers or queues as it would any send. */
//   deliver(o: { threadId: string; workspaceId: string; prompt: string; requestId: string }): Promise<{ outcome: "started" | "steered" | "queued"; turnId?: string }>;
//   /** A slate in this workspace now binds the pull request's checks, so the host re-reads it sooner while one is pending. */
//   watchPr?(workspaceId: string): void;
//   /** The person's login environment a run starts from. */
//   runEnv(): Readonly<Record<string, string>>;
//   /** Where secrets declared keep are written, beside the state file. */
//   secretsFile?: string;
//   /** Under the host's home: each thread's slate writes its files into a folder of its own here, named by the thread. */
//   slatesDir?: string;
//   /** The runs factory; the real one unless a test swaps it. */
//   runs?: (d: SlateRunsDeps) => SlateRuns;
//   /** One of the thread's agent's MCP servers, as that agent would start it on this computer. */
//   mcpServer?(threadId: string, name: string): Promise<McpServerSpec>;
//   ownProcess?(threadId: string, pid: number, kind: "slate" | "tool"): () => void;
//   /** The computer a thread runs on where that is not this one, whose own config and sign-ins its runs start with. */
//   away?(threadId: string): Away | undefined;
//   /** Of these servers, those the thread's agent's config on its computer does not name, and why any it names is off. */
//   missingOn?(threadId: string, servers: string[]): Promise<{ agent: { id: string; name: string }; thread: string; missing: string[]; off?: string[] }>;
// }
/// The runtime as the slate area reads it. Optional TS members answer `None`, `false` or `Ok(())` by default.
#[async_trait(?Send)]
pub trait SlatesDeps {
    fn store(&self) -> Rc<dyn Store>;
    /// Every `now()` and timer of the area and its runs.
    fn clock(&self) -> Rc<dyn Clock>;
    /// Records an event in the thread's transcript, live to every window.
    fn record(&self, event: SessionSlateEvent);
    /// Pushes an event to the windows and nowhere else.
    fn emit(&self, event: SlateEmit);
    fn thread(&self, thread_id: &str) -> Option<SlateThreadFacts>;
    /// The host's workspaces loaded; the slates recover only after.
    async fn loaded(&self) -> Result<(), HostError> {
        Ok(())
    }
    /// The machine a thread runs on where that is not this computer.
    fn machine_of(&self, _thread_id: &str) -> Option<Arc<dyn Machine>> {
        None
    }
    /// The same machine as the computer's own folder machine, where the thread runs on a joined computer.
    fn place_of(&self, _thread_id: &str) -> Option<Arc<PlaceFolderMachine>> {
        None
    }
    fn image_on(&self, _thread_id: &str) -> Option<ImageOn> {
        None
    }
    fn hash_on(&self, _thread_id: &str) -> Option<HashOn> {
        None
    }
    /// That machine naps (the TS `asleep?.() === true`).
    fn asleep(&self, _thread_id: &str) -> bool {
        false
    }
    /// Readies that machine for a run a press starts.
    async fn wake(&self, _thread_id: &str) -> Result<(), HostError> {
        Ok(())
    }
    /// Whether the thread is settled; none where the runtime gives no answer.
    async fn settled(&self, _thread_id: &str) -> Option<bool> {
        None
    }
    /// Every thread under the lead, the lead included.
    fn under(&self, lead: &str) -> Vec<String>;
    fn thread_of_token(&self, token: &str) -> Result<String, HostError>;
    fn sources(&self, thread_id: &str, workspace_id: &str) -> Rc<dyn SlateSourceContext>;
    /// Puts a send's message into the thread.
    async fn deliver(&self, o: Delivery) -> Result<Delivered, HostError>;
    fn watch_pr(&self, _workspace_id: &str) {}
    /// The person's login environment a run starts from.
    fn run_env(&self) -> IndexMap<String, String>;
    fn secrets_file(&self) -> Option<PathBuf> {
        None
    }
    fn slates_dir(&self) -> Option<PathBuf> {
        None
    }
    /// The runs factory; none uses `create_slate_runs`.
    fn runs(&self, _deps: Rc<dyn SlateRunsDeps>) -> Option<Rc<dyn SlateRuns>> {
        None
    }
    /// None: this host starts no MCP servers for a slate.
    async fn mcp_server(&self, _thread_id: &str, _name: &str) -> Option<Result<McpServerSpec, HostError>> {
        None
    }
    fn own_process(&self, _thread_id: &str, _pid: u32, _kind: &str) -> Option<Rc<dyn Fn()>> {
        None
    }
    fn away(&self, _thread_id: &str) -> Option<Away> {
        None
    }
    fn missing_on(&self, _thread_id: &str) -> Option<MissingOnFn> {
        None
    }
}

// slates.ts:192
// /** Where a slate op names its thread: a window always, a thread's token by itself, a person's shell inside a turn by
//  * that turn's token. */
// export interface SlateTarget {
//   threadId?: string;
//   turnToken?: string;
// }
/// Where a slate op names its thread.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlateTarget {
    pub thread_id: Option<String>,
    pub turn_token: Option<String>,
}

/// A rehearsed press.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Press {
    pub piece: String,
    pub index: Option<i64>,
    pub action: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WriteParams {
    pub target: SlateTarget,
    pub text: Option<String>,
    pub document: Option<Value>,
    pub check: Option<bool>,
    pub if_version: Option<i64>,
    pub values: Option<IndexMap<String, Value>>,
    pub press: Option<Press>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateParams {
    pub target: SlateTarget,
    pub values: Option<IndexMap<String, Value>>,
    pub start: Option<Vec<String>>,
    pub if_version: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadParams {
    pub target: SlateTarget,
    pub values: Option<Vec<String>>,
    pub text: Option<bool>,
    pub sketch: Option<bool>,
    pub document: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CatalogParams {
    pub target: SlateTarget,
    pub name: Option<String>,
}

/// A row's scope on an event.
#[derive(Clone, Debug, PartialEq)]
pub struct EventScope {
    pub item: Option<Value>,
    pub index: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventParams {
    pub thread_id: String,
    pub version: i64,
    pub piece: String,
    /// `press`, `submit` or `change`.
    pub event: String,
    pub request_id: String,
    pub scope: Option<EventScope>,
    pub row_action: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApprovalScope {
    Once,
    Thread,
    Refuse,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApproveParams {
    pub thread_id: String,
    pub key: String,
    pub scope: ApprovalScope,
}

// slates.ts:257
// export interface Slates {
//   get(threadId: string): Promise<SlateView | null>;
//   write(p: SlateTarget & { text?: string; document?: Record<string, unknown>; check?: boolean; ifVersion?: number; values?: Record<string, unknown>; press?: { piece: string; index?: number; action?: number } }, caller?: Caller): Promise<SlateWriteAnswer>;
//   state(p: SlateTarget & { values?: Record<string, unknown>; start?: string[]; ifVersion?: number }, caller?: Caller): Promise<SlateStateAnswer>;
//   read(p: SlateTarget & { values?: string[]; text?: boolean; sketch?: boolean; document?: boolean }, caller?: Caller): Promise<SlateReadAnswer>;
//   /** With a thread, a name that is one of its agent's MCP servers answers that server's tools. */
//   catalog(p: SlateTarget & { name?: string }, caller?: Caller): Promise<{ text: string }>;
//   shown(threadId: string): Promise<void>;
//   /** The person withdraws one standing approval: the runs it covered stop, leave the queue and their timers, and the
//    * next start asks again. */
//   revoke(p: { threadId: string; key: string }): Promise<void>;
//   event(p: { threadId: string; version: number; piece: string; event: "press" | "submit" | "change"; requestId: string; scope?: { item?: unknown; index: number }; rowAction?: number }): Promise<SlateEventAnswer>;
//   approve(p: { threadId: string; key: string; scope: "once" | "thread" | "refuse" }): Promise<void>;
//   cancel(p: { threadId: string; run: string }): Promise<void>;
//   /** A window's hold on sources for a thread's slate; the release goes when the window lets go or its socket closes. */
//   subscribe(p: { threadId: string; sources: string[] }): () => void;
//   resolve(p: { threadId: string; paths: string[] }): Promise<{ values: Record<string, SlateJson> }>;
//   image(p: { threadId: string; src: string; have?: string }): Promise<SlatesImageAnswer>;
//   /** A turn ended: its snapshot is written. */
//   turnEnded(o: { threadId: string; turnId: string }): Promise<void>;
//   /** The runtime rewound a thread to the end of turnId, cutting the turns named. */
//   rewound(o: { threadId: string; turnId: string; cut: readonly string[] }): Promise<void>;
//   /** With `keep`, the thread's running runs and kept servers are left running. */
//   forget(threadId: string, o?: { keep?: boolean }): Promise<void>;
//   /** The thread's kept MCP servers closed, as a settle that stops what the thread left running asks. */
//   closeServers(threadId: string): void;
//   /** Whether a slate in the workspace watches the pull request's checks. */
//   watchesPr(workspaceId: string): boolean;
//   /** The workspace's machine napped: its threads' MCP servers there are closed, and the next run starts them again. */
//   napped(workspaceId: string): void;
//   /** The stored slates loaded, runs that were running when the host stopped marked failed and their done fired. */
//   ready(): Promise<void>;
//   /** Every batch queued so far has run. */
//   settled(): Promise<void>;
//   close(): void;
// }
#[async_trait(?Send)]
pub trait Slates {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError>;
    async fn write(&self, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError>;
    async fn state(&self, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError>;
    async fn read(&self, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError>;
    async fn catalog(&self, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError>;
    async fn shown(&self, thread_id: &str) -> Result<(), HostError>;
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError>;
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError>;
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError>;
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError>;
    /// The release, which may be called more than once.
    fn subscribe(&self, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()>;
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError>;
    async fn image(&self, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError>;
    async fn turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError>;
    async fn rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError>;
    async fn forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError>;
    fn close_servers(&self, thread_id: &str);
    fn watches_pr(&self, workspace_id: &str) -> bool;
    fn napped(&self, workspace_id: &str);
    async fn ready(&self) -> Result<(), HostError>;
    async fn settled(&self);
    fn close(&self);
}

/// The source views a batch and a read hold: a source with no view is `None`.
/// One thread's record, shared by every task on its chain, as the TS shares one object.
pub type RecordRef = Rc<RefCell<SlateRecord>>;
/// A chain's tail and a held answer.
pub type Tail = Shared<LocalBoxFuture<'static, ()>>;
pub type HeldAnswer = Shared<LocalBoxFuture<'static, Result<SlateEventAnswer, HostError>>>;

/// `createSlates`' closure state (slates.ts:295-474).
pub struct SlatesInner {
    pub(crate) deps: Rc<dyn SlatesDeps>,
    pub(crate) records: RefCell<IndexMap<String, RecordRef>>,
    /// Each thread's chain tail with its sequence number, which `settled` compares to see a chain unchanged.
    pub(crate) queues: RefCell<IndexMap<String, (u64, Tail)>>,
    pub(crate) chain_seq: Cell<u64>,
    pub(crate) capturing: RefCell<Option<(String, String)>>,
    pub(crate) started_by: RefCell<IndexMap<String, RunBy>>,
    pub(crate) runs: OnceCell<Rc<dyn SlateRuns>>,
    pub(crate) mcp: OnceCell<Rc<SlateMcp>>,
    pub(crate) ledger: OnceCell<Rc<BoxLedger>>,
    pub(crate) pins: OnceCell<BoxPins>,
    pub(crate) skipped: RefCell<HashMap<String, IndexSet<String>>>,
    pub(crate) windows: RefCell<HashMap<String, i64>>,
    pub(crate) loaded: RefCell<Option<Shared<LocalBoxFuture<'static, Result<(), HostError>>>>>,
    pub(crate) writes: WriteBudget,
    pub(crate) press_sent_at: RefCell<HashMap<String, i64>>,
    pub(crate) reaction_sent_at: RefCell<HashMap<String, i64>>,
    pub(crate) requests: RefCell<IndexMap<String, (i64, HeldAnswer)>>,
    pub(crate) event_buckets: RefCell<HashMap<String, (f64, i64)>>,
    pub(crate) holds: RefCell<HashMap<String, IndexMap<String, i64>>>,
    pub(crate) views_now: RefCell<HashMap<String, Views>>,
    /// The sends a state write handed on, which `settled` waits for.
    pub(crate) deliveries: RefCell<Vec<Tail>>,
}

/// The area as the runtime holds it.
pub struct SlatesArea {
    pub(crate) inner: Rc<SlatesInner>,
}

impl SlatesInner {
    /// The runs table, set by `create_slates` before the area is handed out.
    pub(crate) fn runs(&self) -> Rc<dyn SlateRuns> {
        self.runs.get().expect("create_slates sets the runs table").clone()
    }

    pub(crate) fn mcp(&self) -> Rc<SlateMcp> {
        self.mcp.get().expect("create_slates sets the MCP table").clone()
    }

    pub(crate) fn ledger(&self) -> Rc<BoxLedger> {
        self.ledger.get().expect("create_slates sets the ledger").clone()
    }

    pub(crate) fn pins(&self) -> &BoxPins {
        self.pins.get().expect("create_slates sets the pins")
    }

    /// The files a thread's slate declares, as they are now; none for a thread with no record.
    pub(crate) fn files_of(&self, thread_id: &str) -> IndexMap<String, String> {
        let found = self.records.borrow().get(thread_id).cloned();
        let Some(r) = found else { return IndexMap::new() };
        let rec = r.borrow();
        let files = rec.document.as_ref().and_then(|doc| doc.files.clone());
        files.unwrap_or_default()
    }

    /// The approval sheet changed for a thread: its slate hears it as a push with no values.
    pub(crate) fn asks_changed(&self, thread_id: &str) {
        let found = self.records.borrow().get(thread_id).cloned();
        let Some(r) = found else { return };
        let (workspace_id, version, revision) = {
            let rec = r.borrow();
            (rec.workspace_id.clone(), rec.version, rec.rev())
        };
        self.deps.emit(SlateEmit::Values(SlateValuesEvent {
            workspace_id,
            thread_id: thread_id.to_string(),
            version: JsInt::from(version),
            revision: JsInt::from(revision),
            values: Record::default(),
        }));
    }
}

/// The runs table's callbacks into the area: a run's record and line, a timer, the approvals and the boxes.
struct AreaRunsDeps {
    area: Weak<SlatesInner>,
    deps: Rc<dyn SlatesDeps>,
}

impl SlateRunsDeps for AreaRunsDeps {
    fn env(&self) -> IndexMap<String, String> {
        self.deps.run_env()
    }

    fn on_record(&self, thread_id: &str, run: &str, record: RunRecord) {
        if let Some(area) = self.area.upgrade() {
            area.moved(thread_id, run, record);
        }
    }

    fn on_line(&self, thread_id: &str, run: &str, line: &str, _stream: Stream) {
        let Some(area) = self.area.upgrade() else { return };
        let found = area.records.borrow().get(thread_id).cloned();
        let Some(r) = found else { return };
        let workspace_id = r.borrow().workspace_id.clone();
        self.deps.emit(SlateEmit::Run(SlateRunEvent {
            workspace_id,
            thread_id: thread_id.to_string(),
            run: run.to_string(),
            lines: vec![line.to_string()],
        }));
    }

    fn on_timer(&self, thread_id: &str, run: &str) {
        if let Some(area) = self.area.upgrade() {
            area.timed(thread_id, run);
        }
    }

    fn approvals(&self) -> Option<Rc<dyn RunApprovals>> {
        Some(Rc::new(RecordApprovals { area: self.area.clone() }))
    }

    fn secrets_file(&self) -> Option<PathBuf> {
        self.deps.secrets_file()
    }

    fn dir(&self, thread_id: &str) -> Option<Result<String, String>> {
        let dir = self.deps.slates_dir()?;
        let files = self.area.upgrade().map(|area| area.files_of(thread_id)).unwrap_or_default();
        Some(write_slate_files(&dir.join(thread_id), &files).map(|path| path.display().to_string()).map_err(|e| e.to_string()))
    }

    fn road(&self, thread_id: &str, on: &str) -> Option<Rc<dyn RunRoad>> {
        if on == "host" {
            return None;
        }
        let machine = self.deps.machine_of(thread_id)?;
        let area = self.area.clone();
        let owner = thread_id.to_string();
        let files: SlateFilesNow = Rc::new(move || area.upgrade().map(|a| a.files_of(&owner)).unwrap_or_default());
        let deps = self.deps.clone();
        let awake = thread_id.to_string();
        let ready: Ready = Rc::new(move || {
            let deps = deps.clone();
            let id = awake.clone();
            async move {
                let _ = deps.wake(&id).await;
            }
            .boxed_local()
        });
        let place = self.deps.place_of(thread_id);
        let launch_on = launch_on_for(&machine, place.as_ref(), thread_id);
        let ledger = self.area.upgrade()?.ledger();
        let road: Rc<dyn RunRoad> = box_road(machine, thread_id, files, ready, ledger, launch_on, Rc::new(OsIds));
        Some(road)
    }

    fn own(&self, thread_id: &str, pid: u32, kind: &str) -> Option<Rc<dyn Fn()>> {
        self.deps.own_process(thread_id, pid, kind)
    }

    fn host_refused(&self, thread_id: &str, run: &str) -> Option<String> {
        host_away(self.deps.away(thread_id).as_ref(), run)
    }

    fn clock(&self) -> Rc<dyn Clock> {
        self.deps.clock()
    }
}

/// The MCP table reads its secrets through the same names the runs table keeps them under.
struct SecretsView(Rc<dyn SlateSecrets>);

impl McpSecrets for SecretsView {
    fn plaintext(&self, thread_id: &str, name: &str) -> Option<String> {
        self.0.plaintext(thread_id, name)
    }

    fn scrub(&self, thread_id: &str, text: &str) -> String {
        self.0.scrub(thread_id, text)
    }
}

/// Builds the area: the runs table (or `deps.runs`'s), the MCP table, the ledger and the pins, wired to it.
pub fn create_slates(deps: Rc<dyn SlatesDeps>) -> Rc<dyn Slates> {
    let clock = deps.clock();
    let inner = Rc::new(SlatesInner {
        deps: deps.clone(),
        records: RefCell::new(IndexMap::new()),
        queues: RefCell::new(IndexMap::new()),
        chain_seq: Cell::new(0),
        capturing: RefCell::new(None),
        started_by: RefCell::new(IndexMap::new()),
        runs: OnceCell::new(),
        mcp: OnceCell::new(),
        ledger: OnceCell::new(),
        pins: OnceCell::new(),
        skipped: RefCell::new(HashMap::new()),
        windows: RefCell::new(HashMap::new()),
        loaded: RefCell::new(None),
        writes: write_budget(clock.clone()),
        press_sent_at: RefCell::new(HashMap::new()),
        reaction_sent_at: RefCell::new(HashMap::new()),
        requests: RefCell::new(IndexMap::new()),
        event_buckets: RefCell::new(HashMap::new()),
        holds: RefCell::new(HashMap::new()),
        views_now: RefCell::new(HashMap::new()),
        deliveries: RefCell::new(Vec::new()),
    });
    let weak = Rc::downgrade(&inner);

    let run_deps: Rc<dyn SlateRunsDeps> = Rc::new(AreaRunsDeps { area: weak.clone(), deps: deps.clone() });
    let runs: Rc<dyn SlateRuns> = deps.runs(run_deps.clone()).unwrap_or_else(|| create_slate_runs(run_deps));

    let approvals: Rc<dyn RunApprovals> = Rc::new(RecordApprovals { area: weak.clone() });
    let server_deps = deps.clone();
    let record_weak = weak.clone();
    let asks_weak = weak.clone();
    let asks_deps = deps.clone();
    let reshape_deps = deps.clone();
    let reshape_runs = runs.clone();
    let computer_deps = deps.clone();
    let own_deps = deps.clone();
    let mcp = create_slate_mcp(SlateMcpDeps {
        server: Rc::new(move |thread_id: &str, name: &str| {
            let d = server_deps.clone();
            let (t, n) = (thread_id.to_string(), name.to_string());
            async move {
                match d.mcp_server(&t, &n).await {
                    None => Err("this host starts no MCP servers for a slate".to_string()),
                    Some(Ok(spec)) => Ok(spec),
                    Some(Err(e)) => Err(e.message),
                }
            }
            .boxed_local()
        }),
        on_record: Rc::new(move |thread_id: &str, run: &str, record: &RunRecord| {
            if let Some(area) = record_weak.upgrade() {
                area.moved(thread_id, run, record.clone());
            }
        }),
        on_asks: Some(Rc::new(move |thread_id: &str| {
            let (area, thread) = (asks_weak.clone(), thread_id.to_string());
            spawn_local(async move {
                if let Some(area) = area.upgrade() {
                    area.asks_changed(&thread);
                }
            });
        })),
        asks: Rc::new(move |thread_id: &str| asks_deps.thread(thread_id).and_then(|facts| facts.tool_asks).unwrap_or(ToolAsks::Every)),
        approvals,
        secrets: Rc::new(SecretsView(runs.secrets())),
        reshape: Rc::new(move |thread_id: &str, cmd: &str, input: &str| {
            match reshape_deps.thread(thread_id).and_then(|facts| facts.folder) {
                Some(folder) => reshape_runs.reshape(thread_id, cmd, input, &folder, None),
                None => Reshape {
                    done: Box::pin(async { ReshapeAnswer::Failed { why: NO_FOLDER.to_string(), exit: None, err: String::new() } }),
                    kill: Rc::new(|| {}),
                },
            }
        }),
        computer: Rc::new(move |thread_id: &str| {
            computer_deps.thread(thread_id).and_then(|facts| facts.computer).unwrap_or_else(|| "this computer".to_string())
        }),
        own: Some(Rc::new(move |thread_id: &str, pid: u32, kind: &str| -> Release {
            match own_deps.own_process(thread_id, pid, kind) {
                Some(release) => Box::new(move || release()),
                None => Box::new(|| {}),
            }
        })),
        clock: clock.clone(),
        idle_ms: None,
        call_ms: None,
    });

    let hash_deps = deps.clone();
    let pins = box_pins(Box::new(move |thread_id: &str| hash_deps.hash_on(thread_id)), None);
    let ledger = box_ledger(None, clock);
    let _ = inner.runs.set(runs);
    let _ = inner.mcp.set(mcp);
    let _ = inner.ledger.set(ledger);
    let _ = inner.pins.set(pins);
    Rc::new(SlatesArea { inner })
}

#[async_trait(?Send)]
impl Slates for SlatesArea {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        self.inner.clone().get(thread_id).await
    }
    async fn write(&self, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        self.inner.clone().write(p, caller).await
    }
    async fn state(&self, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        self.inner.clone().state(p, caller).await
    }
    async fn read(&self, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        self.inner.clone().read(p, caller).await
    }
    async fn catalog(&self, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        self.inner.clone().catalog(p, caller).await
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        self.inner.clone().shown(thread_id).await
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        self.inner.clone().revoke(thread_id, key).await
    }
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        self.inner.clone().event(p).await
    }
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError> {
        self.inner.clone().approve(p).await
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        self.inner.clone().cancel(thread_id, run).await
    }
    fn subscribe(&self, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        self.inner.subscribe(thread_id, sources)
    }
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        self.inner.clone().resolve(thread_id, paths).await
    }
    async fn image(&self, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        self.inner.clone().image(thread_id, src, have).await
    }
    async fn turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        self.inner.clone().turn_ended(thread_id, turn_id).await
    }
    async fn rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        self.inner.clone().rewound(thread_id, turn_id, cut).await
    }
    async fn forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError> {
        self.inner.clone().forget(thread_id, keep).await
    }
    fn close_servers(&self, thread_id: &str) {
        self.inner.close_servers(thread_id);
    }
    fn watches_pr(&self, workspace_id: &str) -> bool {
        self.inner.watches_pr(workspace_id)
    }
    fn napped(&self, workspace_id: &str) {
        self.inner.napped(workspace_id);
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.inner.clone().ready().await
    }
    async fn settled(&self) {
        self.inner.clone().settled().await;
    }
    fn close(&self) {
        self.inner.close();
    }
}
