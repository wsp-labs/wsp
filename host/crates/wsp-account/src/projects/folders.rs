// SPDX-License-Identifier: AGPL-3.0-only
//! Where a thread on this computer or a joined box runs, and the worktrees wsp makes for it: the folder records, the
//! host's own git writes one at a time, removal and the sweep. Port of runtime/src/projects/folders.ts at 65f6b89c7
//! less `moveOldCopies` (Proposal 3); spec/folders_place.md and spec/folders_tree.md.
pub mod place;
pub mod tree;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use wsp_core::clock::Cancel;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::Machine;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorktreeReport;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use crate::projects::workspaces::seam::{Base, Entry, Queued};

/// `copier.worktree({...})`'s ask. folders.ts:82.
pub use wsp_wire::views::workspace_more::WorktreeAsk;
/// `copier.worktreeRemove({from, home, path, force})`'s ask. folders.ts:228.
pub use wsp_engine::copier::WorktreeRemove;
/// `local.hereDaemon`: this computer's daemon binary's version and the fix a behind one names. folders.ts:77-81.
pub use wsp_core::types::wiring::HereDaemon;

/// `ctx.state`'s sweep fields: the armed timer, the sweep in flight, and the close's stop. folders.ts:280-296.
#[derive(Default)]
pub struct SweepState {
    pub timer: Option<Cancel>,
    pub sweeping: Option<Shared<LocalBoxFuture<'static, ()>>>,
    pub stopped: bool,
}

/// What the folders area reads off the runtime context, past `Base`.
#[async_trait(?Send)]
pub trait FoldersCtx: Base {
    fn state_folder(&self) -> String;
    /// `ctx.opts.statePath !== undefined`.
    fn has_state_path(&self) -> bool;
    /// `local?.copier !== undefined`.
    fn has_copier(&self) -> bool;
    async fn copier_worktree(&self, ask: WorktreeAsk) -> Result<WorktreeReport, HostError>;
    async fn copier_worktree_mount(&self, from: &str, home: &str, path: &str) -> Result<(), HostError>;
    async fn copier_worktree_remove(&self, ask: WorktreeRemove) -> Result<(), HostError>;
    /// `local.hereDaemon`, its version read now; none where the host wires none.
    fn here_daemon(&self) -> Option<Rc<dyn HereDaemon>>;
    /// `moduleOf(kind).admitFolder?.({kind, name, place?})`.
    async fn admit_folder(&self, kind: WorkspaceKind, name: &str, place: Option<&str>) -> Result<(), HostError>;
    /// `backendOfKind(kind, place).get(place ?? LOCAL_MACHINE_ID)` and that backend's `pricing.defaultSize`.
    async fn folder_machine(&self, kind: WorkspaceKind, place: Option<&str>) -> Result<(Arc<dyn Machine>, WorkspaceSize), HostError>;
    /// `ctx.attach(record, machine)`: the record becomes a live entry.
    fn attach(&self, record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry;
    /// `ctx.foldersOf(projectId)`: the live folder records of a project, in insertion order.
    fn folders_of(&self, project_id: &str) -> Vec<Entry>;
    /// `ctx.oneFolder(key, make)`: a second caller of one key joins the first's make.
    async fn one_folder(&self, key: &str, make: Queued<Entry>) -> Result<Entry, HostError>;
    /// `ctx.projectsDoor.resolve(word, origin)`.
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError>;
    /// `ctx.elsewhereRefusal(origin, project, word)`.
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Option<HostError>;
    /// `ctx.picksHold(project, picks)` (start_from's).
    async fn picks_hold(&self, project: &ProjectView, picks: &StartPicksAsked) -> Result<(), HostError>;
    /// `ctx.branchAt(path)`.
    async fn branch_at(&self, path: &str) -> Result<Option<String>, HostError>;
    /// `ctx.turnRuns(workspaceId)`.
    fn turn_runs(&self, workspace_id: &str) -> bool;
    /// `ctx.drop(workspaceId)`.
    async fn drop_workspace(&self, workspace_id: &str) -> Result<(), HostError>;
    /// `ctx.checkpointsLanding.get(threadId)` awaited, its failure dropped.
    async fn checkpoints_landed(&self, thread_id: &str);
    /// `ctx.notARepo(record)`.
    fn not_a_repo(&self, record: &WorkspaceRecord) -> bool;
    /// `ctx.readPullRequest(entry, force)`; the fact lands on the record and the entry.
    async fn read_pull_request(&self, entry: &Entry, force: bool) -> Result<(), HostError>;
    /// `ctx.statusNow(entry)`.
    async fn status_now(&self, entry: &Entry) -> Result<(), HostError>;
    /// `machine.exec(cmd, {timeoutMs, stdin?})` on this computer's local machine.
    async fn local_exec(&self, cmd: &str, timeout_ms: u64, stdin: Option<Vec<u8>>) -> Result<ExecResult, HostError>;
    /// `ctx.state`'s sweep fields.
    fn sweep_state(&self) -> Rc<RefCell<SweepState>>;
}

/// The folders area: its context, the host's git write queue and the setup lines kept for a first start.
pub struct Folders {
    pub(crate) ctx: Rc<dyn FoldersCtx>,
    /// `gitWrites`: each key's tail, its failure swallowed. folders.ts:203.
    pub(crate) git_writes: Rc<RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, ()>>>>>,
    /// `setupLines`: one line per worktree made and not yet named by a start. folders.ts:93.
    pub(crate) setup_lines: Rc<RefCell<IndexMap<String, String>>>,
}

// export function foldersArea(ctx: RuntimeContext): FoldersArea {
pub fn folders_area(ctx: Rc<dyn FoldersCtx>) -> Rc<Folders> {
    Rc::new(Folders {
        ctx,
        git_writes: Rc::new(RefCell::new(IndexMap::new())),
        setup_lines: Rc::new(RefCell::new(IndexMap::new())),
    })
}

impl wsp_core::context::FoldersArea for Folders {}
