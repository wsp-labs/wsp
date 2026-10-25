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

use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use wsp_core::clock::Clock;
use wsp_engine::machine::Machine;
use wsp_store::store::Store;
use wsp_wire::types::slate_wire::{SessionSlateEvent, SlateRunEvent, SlateValuesEvent};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use crate::pending::wire::{SlateEventAnswer, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer};
use crate::slate_box::BoxLedger;
use crate::slate_files::{BoxPins, HashOn};
use crate::slate_images::ImageOn;
use crate::slate_mcp::{Away, McpServerSpec, MissingOnFn, SlateMcp, ToolAsks};
use crate::slate_runs::{RunBy, SlateRuns, SlateRunsDeps};
use crate::slate_sources::SlateSourceContext;
pub use crate::slate_sources::Views;
use crate::slate_writes::WriteBudget;
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
    pub(crate) queues: RefCell<HashMap<String, Tail>>,
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
}

/// The area as the runtime holds it.
pub struct SlatesArea {
    pub(crate) inner: Rc<SlatesInner>,
}

// slates.ts:294
// export function createSlates(deps: SlatesDeps): Slates {
//   const records = new Map<string, SlateRecord>();
//   const queues = new Map<string, Promise<unknown>>();
//   /** One change at a time per thread: a batch, a write and a turn's end never interleave on one record. */
//   const serial = <T>(threadId: string, run: () => Promise<T>): Promise<T> => {
//     const next = (queues.get(threadId) ?? Promise.resolve()).then(run, run);
//     queues.set(threadId, next.catch(() => {}));
//     return next;
//   };
//   const save = (r: SlateRecord): Promise<void> => {
//     r.updatedAt = deps.now();
//     return deps.store.put(SLATES, r.threadId, r);
//   };
//
//   /** The run a batch is starting: the runs module's first record of it is the batch's to write, not a batch of its own. */
//   let capturing: { threadId: string; run: string } | undefined;
//   /** Who started each run, for the transcript's event when its record goes running. */
//   const startedBy = new Map<string, RunBy>();
//   const byKey = (threadId: string, run: string): string => `${threadId}\u0000${run}`;
//
//   const approvals: RunApprovals = {
//       has: (threadId, key) => records.get(threadId)?.approvals[key]?.state === "allowed",
//       allow: (threadId, key) => {
//         const r = records.get(threadId);
//         if (r === undefined) return;
//         r.approvals[key] = { state: "allowed", at: deps.now(), ...approvalNames(r, key) };
//         void save(r);
//       },
//       revoke: (threadId, key) => {
//         const r = records.get(threadId);
//         if (r?.approvals[key] === undefined) return;
//         delete r.approvals[key];
//         void save(r);
//       },
//       list: threadId => Object.entries(records.get(threadId)?.approvals ?? {}).flatMap(([k, a]) => (a.state === "allowed" ? [k] : [])),
//   };
//   const folderOf = (threadId: string): string | undefined => (deps.slatesDir === undefined ? undefined : join(deps.slatesDir, threadId));
//   const writeFiles = (threadId: string): string => writeSlateFiles(folderOf(threadId)!, records.get(threadId)?.document?.files ?? {});
//   const moved = (threadId: string, run: string, record: RunRecord): void => {
//     if (capturing?.threadId === threadId && capturing.run === run) return;
//     void serial(threadId, () => runMoved(threadId, run, record)).catch((e: unknown) => console.warn(`the slate of thread ${threadWord(threadId)} lost a record of $${run}: ${e instanceof Error ? e.message : String(e)}`));
//   };
//   const ledger = boxLedger();
//   const pins = boxPins(threadId => deps.hashOn?.(threadId));
//   const runs: SlateRuns = (deps.runs ?? createSlateRuns)({
//     env: deps.runEnv,
//     now: deps.now,
//     ...(deps.secretsFile !== undefined ? { secretsFile: deps.secretsFile } : {}),
//     ...(deps.slatesDir !== undefined ? { dir: writeFiles } : {}),
//     hostRefused: (threadId, run) => hostAway(deps.away?.(threadId), run),
//     road: (threadId, on) => {
//       if (on === "host") return undefined;
//       const machine = deps.machineOf?.(threadId);
//       return machine === undefined
//         ? undefined
//         : boxRoad(
//             machine,
//             threadId,
//             () => records.get(threadId)?.document?.files ?? {},
//             async () => deps.wake?.(threadId),
//             ledger,
//           );
//     },
//     approvals,
//     own: deps.ownProcess,
//     onRecord: moved,
//     onLine: (threadId, run, line) => {
//       const r = records.get(threadId);
//       if (r !== undefined) deps.emit({ type: "slate.run", workspaceId: r.workspaceId, threadId, run, lines: [line] });
//     },
//     onTimer: (threadId, run) => timed(threadId, run),
//   });
//
//   /** Runs a settled thread's timer skipped while no window showed the slate, started the moment one does. */
//   const skipped = new Map<string, Set<string>>();
//   /** The windows showing each thread's slate, whatever sources they hold. */
//   const windows = new Map<string, number>();
//   function timed(threadId: string, run: string): void {
//     void serial(threadId, async () => {
//       const r = records.get(threadId);
//       if (r?.document?.runs[run] === undefined) return;
//       if ((windows.get(threadId) ?? 0) === 0 && (await deps.settled?.(threadId)) === true) {
//         skipped.set(threadId, (skipped.get(threadId) ?? new Set()).add(run));
//         return;
//       }
//       await batch(r, [], "timer", { starts: [{ run, by: "timer" }] });
//     }).catch(() => {});
//   }
//
//   const refusedOn = async (threadId: string, before: SlateDoc | null, doc: SlateDoc | null | undefined): Promise<SlateProblem[]> =>
//     mcp.refusedRuns(threadId, before, doc, await refusedAway(before, doc, deps.away?.(threadId), deps.missingOn?.bind(deps, threadId)));
//
//   const mcp = createSlateMcp({
//     server: (threadId, name) => (deps.mcpServer === undefined ? Promise.reject(new Error("this host starts no MCP servers for a slate")) : deps.mcpServer(threadId, name)),
//     onRecord: moved,
//     onAsks: threadId => {
//       const r = records.get(threadId);
//       if (r !== undefined) deps.emit({ type: "slate.values", workspaceId: r.workspaceId, threadId, version: r.version, revision: r.revision, values: {} });
//     },
//     approvals,
//     secrets: runs.secrets,
//     reshape: (threadId, cmd, input) => {
//       const folder = deps.thread(threadId)?.folder;
//       return folder === undefined ? { done: Promise.resolve({ why: NO_FOLDER, exit: null, err: "" }), kill: () => {} } : runs.reshape(threadId, cmd, input, folder);
//     },
//     computer: threadId => deps.thread(threadId)?.computer ?? "this computer",
//     asks: threadId => deps.thread(threadId)?.toolAsks ?? "every",
//     own: deps.ownProcess,
//     now: deps.now,
//   });
//
/// Builds the area: the runs table (or `deps.runs`'s), the MCP table, the ledger and the pins, wired to it.
pub fn create_slates(deps: Rc<dyn SlatesDeps>) -> Rc<dyn Slates> {
    todo!()
}

#[async_trait(?Send)]
impl Slates for SlatesArea {
    async fn get(&self, thread_id: &str) -> Result<Option<SlateView>, HostError> {
        todo!()
    }
    async fn write(&self, p: WriteParams, caller: Option<&Caller>) -> Result<SlateWriteAnswer, HostError> {
        todo!()
    }
    async fn state(&self, p: StateParams, caller: Option<&Caller>) -> Result<SlateStateAnswer, HostError> {
        todo!()
    }
    async fn read(&self, p: ReadParams, caller: Option<&Caller>) -> Result<SlateReadAnswer, HostError> {
        todo!()
    }
    async fn catalog(&self, p: CatalogParams, caller: Option<&Caller>) -> Result<String, HostError> {
        todo!()
    }
    async fn shown(&self, thread_id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn event(&self, p: EventParams) -> Result<SlateEventAnswer, HostError> {
        todo!()
    }
    async fn approve(&self, p: ApproveParams) -> Result<(), HostError> {
        todo!()
    }
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError> {
        todo!()
    }
    fn subscribe(&self, thread_id: &str, sources: &[String]) -> Rc<dyn Fn()> {
        todo!()
    }
    async fn resolve(&self, thread_id: &str, paths: &[String]) -> Result<IndexMap<String, Value>, HostError> {
        todo!()
    }
    async fn image(&self, thread_id: &str, src: &str, have: Option<&str>) -> Result<SlatesImageAnswer, HostError> {
        todo!()
    }
    async fn turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        todo!()
    }
    async fn rewound(&self, thread_id: &str, turn_id: &str, cut: &[String]) -> Result<(), HostError> {
        todo!()
    }
    async fn forget(&self, thread_id: &str, keep: bool) -> Result<(), HostError> {
        todo!()
    }
    fn close_servers(&self, thread_id: &str) {
        todo!()
    }
    fn watches_pr(&self, workspace_id: &str) -> bool {
        todo!()
    }
    fn napped(&self, workspace_id: &str) {
        todo!()
    }
    async fn ready(&self) -> Result<(), HostError> {
        todo!()
    }
    async fn settled(&self) {
        todo!()
    }
    fn close(&self) {
        todo!()
    }
}
