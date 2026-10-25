// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/rules.ts at 65f6b89c7: what a caller may see and drive, and the guard every
//! act a thread's token asks for goes through. Spec: spec/rules.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{Map, Value};

use wsp_core::types::internal::WorkspaceLike;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::preferences::{Preferences, PreferencesPatch};
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents};
use wsp_words::exit::HostError;
use wsp_words::words::computer::{AcrossAct, SpawnAct};

use crate::account::core_types::{Live, Release};

/// What a thread's own record, or its first row, says of its place in a tree (`threadRecords.get(id)` and
/// `rowsOn(id)`): the workspace it runs on and the threads above it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeFacts {
    pub workspace_id: Option<String>,
    pub parent_thread_id: Option<String>,
    pub root_thread_id: Option<String>,
}

/// Where the caps a record runs under are held (rules.ts:240): the workspace by name, or the computer by name.
#[derive(Clone, Debug, PartialEq)]
pub enum CapsHeldAt {
    Workspace(String),
    Computer(String),
}

/// `{ parentThreadId?, rootThreadId? }` (rules.ts:327).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TreeOf {
    pub parent_thread_id: Option<String>,
    pub root_thread_id: Option<String>,
}

/// What the rules read off the runtime's context (rules.ts:8 and the `ctx.` calls below it).
#[async_trait(?Send)]
pub trait RulesCtx {
    /// `opts.noMachinesLine`.
    fn no_machines_line(&self) -> Option<String>;
    /// `store.put(PROJECTS, project.id, project)`.
    async fn store_put_project(&self, project: &ProjectView) -> Result<(), HostError>;
    /// `placeDoor?.nameOf(placeId)`: None where the host has no door.
    fn door_name_of(&self, place_id: &str) -> Option<String>;
    /// `landing`: the slots forks still landing hold, per root thread; shared with the create.
    fn landing(&self) -> Rc<RefCell<IndexMap<String, i64>>>;
    /// `live`, in map order.
    fn live(&self) -> Vec<Live>;
    /// `live.get(id)`.
    fn live_get(&self, id: &str) -> Option<Live>;
    /// `projectsHeld`: shared with every reader of the project map.
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>>;
    /// `sessions.size`.
    fn sessions_size(&self) -> usize;
    /// `threadRecords.size`.
    fn thread_records_size(&self) -> usize;
    /// `threadRecords.get(threadId)`.
    fn thread_record(&self, thread_id: &str) -> Option<TreeFacts>;
    /// `rowsOn(threadId)`, in row order.
    fn rows_on(&self, thread_id: &str) -> Vec<TreeFacts>;
    /// `ctx.preferences.get()`.
    async fn preferences(&self) -> Result<Preferences, HostError>;
    /// `ctx.preferences.set(patch)`.
    async fn set_preferences(&self, patch: PreferencesPatch) -> Result<(), HostError>;
    /// `backendFor(record).capabilities[can] === true`, `can` the TS key (`snapshot`, `fork`, `resize`, ...).
    fn capability(&self, record: &WorkspaceRecord, can: &str) -> bool;
    /// `backendFor(record).capabilities.pauseMode !== undefined`.
    fn pause_mode_declared(&self, record: &WorkspaceRecord) -> bool;
    /// `forksNoMachines(backendFor(record).capabilities)`: the backend offers no sizes.
    fn forks_no_machines(&self, record: &WorkspaceRecord) -> bool;
    /// `moduleOf(kind).relayed(machineId)`.
    fn relayed(&self, kind: WorkspaceKind, machine_id: Option<&str>) -> bool;
    /// `moduleOf(kind).turnReach(entry)` as the TS object.
    fn turn_reach(&self, entry: &Live) -> Option<Map<String, Value>>;
    /// `moduleOf(kind).wspMcp(entry)` as the TS object.
    fn wsp_mcp(&self, entry: &Live) -> Option<Value>;
    /// `ctx.sameRepository(a, b, caller)` (channels_copy C15).
    fn same_repository(&self, a: &ProjectView, b: &ProjectView, caller: Option<&Caller>) -> bool;
    /// `ctx.projectHeld(id)`: throws `noSuchProjectLine` for an id not held.
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError>;
    /// `ctx.kindOf(computer)`.
    fn kind_of(&self, computer: &str) -> WorkspaceKind;
    /// `ctx.placeName(id)`.
    fn place_name(&self, id: &str) -> String;
    /// `ctx.agentsHeld(record)` (channels_daemon D6).
    fn agents_held(&self, record: &WorkspaceRecord) -> WorkspaceAgents;
    /// `ctx.placeIdOf(record)` (channels_daemon D4).
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
}

/// A record read as the `WorkspaceLike` the rules take: its id, kind, name, machine, root, project and worktree, the
/// machine id kept as the record has it, `""` included, since the TS passes the record itself.
pub fn like_of(record: &WorkspaceRecord) -> WorkspaceLike {
    todo!()
}

/// rules.ts:7 `rulesArea(ctx)`.
pub struct RulesArea {
    pub(crate) ctx: Rc<dyn RulesCtx>,
}

impl RulesArea {
    pub fn new(ctx: Rc<dyn RulesCtx>) -> Rc<RulesArea> {
        todo!()
    }

    /// R1, rules.ts:15.
    pub async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        todo!()
    }

    /// R2, rules.ts:20.
    pub async fn remember_target(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        todo!()
    }

    /// R3, R4, rules.ts:43. `can` is the TS capability key.
    pub fn refuse_cannot(&self, entry: &Live, can: &str, action: &str) -> Result<(), HostError> {
        todo!()
    }

    /// R4, rules.ts:48.
    pub fn pauses(&self, record: &WorkspaceRecord) -> bool {
        todo!()
    }

    /// R4, rules.ts:49.
    pub fn refuse_pauseless(&self, entry: &Live, action: &str) -> Result<(), HostError> {
        todo!()
    }

    /// R4, rules.ts:56.
    pub fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        todo!()
    }

    /// R5, rules.ts:61.
    pub fn drives(&self, kind: WorkspaceKind, machine_id: Option<&str>, caller: Option<&Caller>) -> bool {
        todo!()
    }

    /// R12, rules.ts:147.
    pub fn acts_on(&self, record: &WorkspaceLike, caller: Option<&Caller>) -> bool {
        todo!()
    }

    /// R9, rules.ts:102.
    pub fn opens_in(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> Result<bool, HostError> {
        todo!()
    }

    /// rules.ts:76.
    pub fn project_of_scope(&self, scope: &ThreadScope) -> Option<String> {
        todo!()
    }

    /// R7, rules.ts:80. Fails where `projectHeld` does.
    pub fn of_threads_repository(&self, caller: Option<&Caller>, project: &str) -> Result<bool, HostError> {
        todo!()
    }

    /// R8, rules.ts:91.
    pub fn project_reached(&self, caller: Option<&Caller>, project: &str) -> Result<bool, HostError> {
        todo!()
    }

    /// R11, rules.ts:127. The outer error is `projectHeld`'s; the inner is the refusal.
    pub fn elsewhere_refusal(&self, caller: Option<&Caller>, project: &ProjectView, word: &str) -> Result<Option<HostError>, HostError> {
        todo!()
    }

    /// R12, rules.ts:157.
    pub fn away_for(&self, record: &WorkspaceLike, caller: Option<&Caller>, act: AcrossAct, word: &str) -> Result<Option<HostError>, HostError> {
        todo!()
    }

    /// R12, rules.ts:150.
    pub fn lead_acts_on(&self, record: &WorkspaceLike, caller: Option<&Caller>) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// R13, rules.ts:164.
    pub fn talks_to_its_tree(&self, caller: Option<&Caller>) -> Result<bool, HostError> {
        todo!()
    }

    /// R14, rules.ts:174: the refusal's line, or None where the caller may drive it.
    pub fn refusal_for(&self, record: Option<&WorkspaceLike>, caller: Option<&Caller>) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// R15, rules.ts:194. `act` defaults to `work` at the call sites that name none.
    pub fn refuse_relayed(&self, record: Option<&WorkspaceLike>, caller: Option<&Caller>, act: AcrossAct, thread: Option<&str>) -> Result<(), HostError> {
        todo!()
    }

    /// R16, rules.ts:206.
    pub fn refuse_named(&self, workspace_id: &str, caller: Option<&Caller>) -> Result<(), HostError> {
        todo!()
    }

    /// R17, rules.ts:212.
    pub fn held(&self) -> Vec<Live> {
        todo!()
    }

    /// R17, rules.ts:216.
    pub fn listed_for(&self, caller: Option<&Caller>) -> Result<Vec<Live>, HostError> {
        todo!()
    }

    /// R18, rules.ts:220.
    pub fn refuse_recording(&self, named: &str, caller: Option<&Caller>) -> Result<(), HostError> {
        todo!()
    }

    /// R20, rules.ts:235.
    pub fn agents_of(&self, record: &WorkspaceRecord) -> Option<WorkspaceAgents> {
        todo!()
    }

    /// R21, rules.ts:240.
    pub fn agents_held_at(&self, record: &WorkspaceRecord) -> CapsHeldAt {
        todo!()
    }

    /// R19, rules.ts:249.
    pub fn parent_of(&self, thread_id: &str) -> Option<String> {
        todo!()
    }

    /// R22, rules.ts:250.
    pub fn depth_under_root(&self, thread_id: &str) -> usize {
        todo!()
    }

    /// R19, rules.ts:262.
    pub fn root_of(&self, thread_id: &str) -> String {
        todo!()
    }

    /// R19, rules.ts:231.
    pub fn record_of_thread(&self, thread_id: &str) -> Option<WorkspaceRecord> {
        todo!()
    }

    /// R25, rules.ts:265: the reach's own keys, then `wsp`.
    pub fn agents_reach(&self, entry: &Live) -> Option<Map<String, Value>> {
        todo!()
    }

    /// R23, R24, rules.ts:283. The release frees a landing slot once; a no-op for every act but a fork that took one.
    pub fn spawn_guard(&self, act: SpawnAct, caller: Option<&Caller>) -> Result<Release, HostError> {
        todo!()
    }

    /// R26, rules.ts:327.
    pub fn tree_of(&self, scope: Option<&ThreadScope>) -> TreeOf {
        todo!()
    }

    /// R26, rules.ts:273.
    pub fn place_of_thread(&self, scope: &ThreadScope) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// R26, rules.ts:279.
    pub fn act_refusal(&self, scope: &ThreadScope, act: SpawnAct) -> Result<HostError, HostError> {
        todo!()
    }
}
