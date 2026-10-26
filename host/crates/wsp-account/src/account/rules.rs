// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/rules.ts at 65f6b89c7: what a caller may see and drive, and the guard every
//! act a thread's token asks for goes through. Spec: spec/rules.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_core::types::internal::WorkspaceLike;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::workspace::{road_of, scope_of, Caller, ProjectView, WorkspacePhase};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents, WorkspaceOrigin};
use wsp_core::types::internal::ThreadRecord;
use wsp_words::exit::{not_found_refusal, refusal, HostError, Kind};
use wsp_words::projects::{child_on_another_computer_line, copies_folder, runs_in_folder};
use wsp_words::sub_thread::sub_starts_nothing_refusal;
use wsp_words::words::computer::{
    child_to_leads_computer_line, elsewhere_workspace_line, folder_gone_refusal, no_workspace_refusal, relayed_record_refusal,
    relayed_refusal, root_gone_refusal, spawn_act_refusal, spawn_cap_refusal, spawn_depth_refusal, spawn_folder_refusal,
    spawn_reach_refusal, spawn_repository_refusal, thread_word, tree_thread_out_of_reach_line, AcrossAct, DepthHeld,
    SpawnAct, SPAWN_ACTS_ALLOWED, SPAWN_FOLDER_FIX, SPAWN_REPOSITORY_FIX,
};
use wsp_words::words::computer::{provider_cannot_refusal, undriven_refusal};
use wsp_words::words::thread::NO_PROVIDER_LINE;
use wsp_words::workspace_state::{kind_words, machine_word, thread_place, workspace_state, WorkspaceState, WorkspaceStateInput};

use crate::account::core_types::{Entry, Release};

/// What a thread's first row says of its place in a tree (`rowsOn(id)`): the workspace it runs on and the threads
/// above it. A session row carries many fields no rule reads, so the row's three are read as this.
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
    fn live(&self) -> Vec<Entry>;
    /// `live.get(id)`.
    fn live_get(&self, id: &str) -> Option<Entry>;
    /// `projectsHeld`: shared with every reader of the project map.
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>>;
    /// `sessions.size`.
    fn sessions_size(&self) -> usize;
    /// `threadRecords.size`.
    fn thread_records_size(&self) -> usize;
    /// `threadRecords.get(threadId)`.
    fn thread_record(&self, thread_id: &str) -> Option<ThreadRecord>;
    /// `rowsOn(threadId)`, in row order.
    fn rows_on(&self, thread_id: &str) -> Vec<TreeFacts>;
    /// `(await ctx.preferences.get()).target?.workspace`.
    async fn target_workspace(&self) -> Result<Option<String>, HostError>;
    /// `ctx.preferences.set(patch)`, the patch as the TS object.
    async fn set_preferences(&self, patch: Value) -> Result<(), HostError>;
    /// `backendFor(record).capabilities[can] === true`, `can` the TS key (`snapshot`, `fork`, `resize`, ...).
    fn capability(&self, record: &WorkspaceRecord, can: &str) -> bool;
    /// `backendFor(record).capabilities.pauseMode !== undefined`.
    fn pause_mode_declared(&self, record: &WorkspaceRecord) -> bool;
    /// `forksNoMachines(backendFor(record).capabilities)`: the backend offers no sizes.
    fn forks_no_machines(&self, record: &WorkspaceRecord) -> bool;
    /// `moduleOf(kind).relayed(machineId)`.
    fn relayed(&self, kind: WorkspaceKind, machine_id: Option<&str>) -> bool;
    /// `moduleOf(kind).turnReach(entry)` as the TS object.
    fn turn_reach(&self, entry: &Entry) -> Option<Map<String, Value>>;
    /// `moduleOf(kind).wspMcp(entry)` as the TS object.
    fn wsp_mcp(&self, entry: &Entry) -> Option<Value>;
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
    WorkspaceLike {
        id: Some(record.id.clone()),
        kind: record.kind,
        name: record.name.clone(),
        machine_id: Some(record.machine_id.clone()),
        root_thread_id: record.root_thread_id.clone(),
        project: Some(record.project.clone()),
        worktree: record.worktree.clone(),
    }
}

/// `usage(line)`: the usage class the TS builds as `Object.assign(new Error(line), { kind: "usage" })`.
fn usage(line: &str) -> HostError {
    HostError::with_kind(line, Kind::Usage)
}

/// The TypeError a root's caps read as `policy!.maxDepth` leaves when its own record names a root that is gone.
const CAPS_UNREAD: &str = "Cannot read properties of undefined (reading 'maxDepth')";

fn record_of_entry(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

/// `phaseHoldsSlot(record)` (place-state.ts:87), read off the record's phase and queue alone.
fn phase_holds_slot_of(record: &WorkspaceRecord) -> bool {
    let state = workspace_state(&WorkspaceStateInput { phase: record.phase, machine_state: None, reach: None, wake_refused: None });
    state != WorkspaceState::Paused && state != WorkspaceState::Gone && state != WorkspaceState::Released && !(record.phase == WorkspacePhase::Queued && record.queue.is_some())
}

/// `inTree` (rules.ts:65).
fn in_tree(record: &WorkspaceLike, scope: &ThreadScope) -> bool {
    let Some(id) = &record.id else { return true };
    id == &scope.workspace_id
        || record.root_thread_id.as_deref() == Some(scope.root_thread_id.as_str())
        || record.worktree.as_ref().and_then(|w| w.made_for.as_deref()) == Some(scope.root_thread_id.as_str())
}

fn free_release() -> Release {
    Rc::new(|| {})
}

/// The box a thread on a joined computer acts for (`boxOf`, rules.ts:106).
struct BoxOf {
    scope: ThreadScope,
    asking: WorkspaceRecord,
    computer: String,
}

/// The lead of a thread's tree as a caller (`leadOf`, rules.ts:114).
struct TreeLead {
    thread_id: String,
    caller: Caller,
    computer: String,
}

/// Where a workspace on another computer than the box sits (`awayOn`, rules.ts:140).
struct Away {
    box_computer: String,
    on: String,
}

/// rules.ts:7 `rulesArea(ctx)`.
pub struct RulesArea {
    pub(crate) ctx: Rc<dyn RulesCtx>,
}

impl RulesArea {
    pub fn new(ctx: Rc<dyn RulesCtx>) -> Rc<RulesArea> {
        Rc::new(RulesArea { ctx })
    }

    /// R1, rules.ts:15.
    pub async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        self.ctx.projects_held().borrow_mut().insert(project.id.clone(), project.clone());
        self.ctx.store_put_project(&project).await
    }

    /// R2, rules.ts:20.
    pub async fn remember_target(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        if self.ctx.target_workspace().await?.as_deref() == Some(record.id.as_str()) {
            return Ok(());
        }
        self.ctx.set_preferences(json!({ "target": { "workspace": record.id } })).await
    }

    /// `cannotLine` (rules.ts:30).
    fn cannot_line(&self, record: &WorkspaceRecord, action: &str) -> String {
        let kind = record.kind;
        if !kind_words(&kind).driven {
            return undriven_refusal(&record.name, &machine_word(&kind), action);
        }
        if self.ctx.forks_no_machines(record) {
            return self.ctx.no_machines_line().unwrap_or_else(|| NO_PROVIDER_LINE.to_string());
        }
        provider_cannot_refusal(&record.name, &machine_word(&kind), action)
    }

    /// `refuseUnless` (rules.ts:37).
    fn refuse_unless(&self, record: &WorkspaceRecord, able: bool, action: &str) -> Result<(), HostError> {
        if able {
            return Ok(());
        }
        Err(HostError::new(self.cannot_line(record, action)))
    }

    /// R3, R4, rules.ts:43. `can` is the TS capability key.
    pub fn refuse_cannot(&self, entry: &Entry, can: &str, action: &str) -> Result<(), HostError> {
        let record = record_of_entry(entry);
        let able = self.ctx.capability(&record, can);
        self.refuse_unless(&record, able, action)
    }

    /// R4, rules.ts:48.
    pub fn pauses(&self, record: &WorkspaceRecord) -> bool {
        self.ctx.pause_mode_declared(record)
    }

    /// R4, rules.ts:49.
    pub fn refuse_pauseless(&self, entry: &Entry, action: &str) -> Result<(), HostError> {
        let record = record_of_entry(entry);
        let able = self.pauses(&record);
        self.refuse_unless(&record, able, action)
    }

    /// R4, rules.ts:56.
    pub fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        self.pauses(record) && phase_holds_slot_of(record)
    }

    /// R5, rules.ts:61.
    pub fn drives(&self, kind: WorkspaceKind, machine_id: Option<&str>, caller: Option<&Caller>) -> bool {
        road_of(caller) != Some(WorkspaceOrigin::Relayed) || self.ctx.relayed(kind, machine_id)
    }

    /// `ctx.live.get(workspaceId)?.record`, read as a record.
    fn live_record(&self, workspace_id: &str) -> Option<WorkspaceRecord> {
        self.ctx.live_get(workspace_id).map(|entry| record_of_entry(&entry))
    }

    /// R12, rules.ts:147.
    pub fn acts_on(&self, record: &WorkspaceLike, caller: Option<&Caller>) -> bool {
        // A throw from the box read (a project this host does not hold) leaves the thread not acting there.
        self.drives(record.kind, record.machine_id.as_deref(), caller) && matches!(self.away_on(record, caller), Ok(None))
    }

    /// R9, rules.ts:102.
    pub fn opens_in(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> Result<bool, HostError> {
        if scope_of(caller).is_none() || !runs_in_folder(&record.kind) || record.worktree.is_some() {
            return Ok(false);
        }
        self.project_reached(caller, &record.project)
    }

    /// rules.ts:76.
    pub fn project_of_scope(&self, scope: &ThreadScope) -> Option<String> {
        self.live_record(&scope.workspace_id).map(|record| record.project)
    }

    /// R7, rules.ts:80. Fails where `projectHeld` does.
    pub fn of_threads_repository(&self, caller: Option<&Caller>, project: &str) -> Result<bool, HostError> {
        let Some(mine) = scope_of(caller).and_then(|scope| self.project_of_scope(&scope)) else {
            return Ok(false);
        };
        if project == mine {
            return Ok(true);
        }
        let mine_view = self.ctx.project_held(&mine)?;
        let project_view = self.ctx.project_held(project)?;
        Ok(self.ctx.same_repository(&mine_view, &project_view, caller))
    }

    /// R8, rules.ts:91.
    pub fn project_reached(&self, caller: Option<&Caller>, project: &str) -> Result<bool, HostError> {
        let Some(scope) = scope_of(caller) else {
            return Ok(false);
        };
        if !self.of_threads_repository(caller, project)? {
            return Ok(false);
        }
        if self.project_of_scope(&scope).as_deref() == Some(project) {
            return Ok(true);
        }
        let there = self.ctx.kind_of(&self.ctx.project_held(project)?.computer);
        if copies_folder(&there) {
            return Ok(false);
        }
        let Some(asking) = self.live_record(&scope.workspace_id) else {
            return Ok(false);
        };
        Ok(copies_folder(&asking.kind) || (!runs_in_folder(&asking.kind) && !runs_in_folder(&there)))
    }

    /// `boxOf` (rules.ts:106).
    fn box_of(&self, caller: Option<&Caller>) -> Result<Option<BoxOf>, HostError> {
        let Some(scope) = scope_of(caller) else {
            return Ok(None);
        };
        let Some(asking) = self.live_record(&scope.workspace_id) else {
            return Ok(None);
        };
        if !runs_in_folder(&asking.kind) || copies_folder(&asking.kind) {
            return Ok(None);
        }
        let computer = self.ctx.project_held(&asking.project)?.computer;
        Ok(Some(BoxOf { scope, asking, computer }))
    }

    /// `leadOf` (rules.ts:114).
    fn lead_of(&self, scope: &ThreadScope) -> Result<Option<TreeLead>, HostError> {
        let lead = scope.root_thread_id.clone();
        if lead == scope.thread_id {
            return Ok(None);
        }
        let Some(record) = self.record_of_thread(&lead) else {
            return Ok(None);
        };
        let computer = self.ctx.project_held(&record.project)?.computer;
        let by = ThreadScope {
            kind: Default::default(),
            thread_id: lead.clone(),
            workspace_id: record.id.clone(),
            root_thread_id: lead.clone(),
        };
        Ok(Some(TreeLead { thread_id: lead, caller: Caller::Scoped { origin: WorkspaceOrigin::Here, by }, computer }))
    }

    /// R11, rules.ts:127. The outer error is `projectHeld`'s; the inner is the refusal.
    pub fn elsewhere_refusal(&self, caller: Option<&Caller>, project: &ProjectView, word: &str) -> Result<Option<HostError>, HostError> {
        let Some(box_) = self.box_of(caller)? else {
            return Ok(None);
        };
        if project.computer == box_.computer {
            return Ok(None);
        }
        if !self.of_threads_repository(caller, &project.id)? {
            let asking_project = self.ctx.project_held(&box_.asking.project)?;
            let line = spawn_repository_refusal(&box_.scope.thread_id, &asking_project.name, word);
            return Ok(Some(refusal(&line, SPAWN_REPOSITORY_FIX, Some(Kind::Usage))));
        }
        let from = self.ctx.place_name(&box_.computer);
        let to = self.ctx.place_name(&project.computer);
        if let Some(lead) = self.lead_of(&box_.scope)? {
            if self.project_reached(Some(&lead.caller), &project.id)? {
                return Ok(Some(usage(&child_to_leads_computer_line(&from, &to, &lead.thread_id))));
            }
            if lead.computer == project.computer {
                let line = spawn_folder_refusal(&box_.scope.thread_id, word);
                return Ok(Some(refusal(&line, SPAWN_FOLDER_FIX, Some(Kind::Usage))));
            }
        }
        Ok(Some(usage(&child_on_another_computer_line(&from, &to))))
    }

    /// `awayOn` (rules.ts:140): the computer rule's box and the computer the workspace is on, where the rule keeps it out.
    fn away_on(&self, record: &WorkspaceLike, caller: Option<&Caller>) -> Result<Option<Away>, HostError> {
        let Some(box_) = self.box_of(caller)? else {
            return Ok(None);
        };
        let Some(project_id) = &record.project else {
            return Ok(None);
        };
        let project = self.ctx.projects_held().borrow().get(project_id).cloned();
        Ok(match project {
            Some(project) if project.computer != box_.computer => Some(Away { box_computer: box_.computer, on: project.computer }),
            _ => None,
        })
    }

    /// R12, rules.ts:157.
    pub fn away_for(&self, record: &WorkspaceLike, caller: Option<&Caller>, act: AcrossAct, word: &str) -> Result<Option<HostError>, HostError> {
        let Some(away) = self.away_on(record, caller)? else {
            return Ok(None);
        };
        let lead = self.lead_acts_on(record, caller)?;
        let line = elsewhere_workspace_line(
            word,
            &self.ctx.place_name(&away.on),
            &self.ctx.place_name(&away.box_computer),
            act,
            lead.as_deref(),
        );
        Ok(Some(usage(&line)))
    }

    /// R12, rules.ts:150.
    pub fn lead_acts_on(&self, record: &WorkspaceLike, caller: Option<&Caller>) -> Result<Option<String>, HostError> {
        let Some(box_) = self.box_of(caller)? else {
            return Ok(None);
        };
        let Some(lead) = self.lead_of(&box_.scope)? else {
            return Ok(None);
        };
        let lead_refusal = self.refusal_for(Some(record), Some(&lead.caller))?;
        Ok(if lead_refusal.is_none() { Some(lead.thread_id) } else { None })
    }

    /// R13, rules.ts:164.
    pub fn talks_to_its_tree(&self, caller: Option<&Caller>) -> Result<bool, HostError> {
        if self.box_of(caller)?.is_some() {
            return Ok(true);
        }
        Ok(scope_of(caller).and_then(|scope| self.live_record(&scope.workspace_id)).is_some_and(|record| record.lead.is_some()))
    }

    /// R14, rules.ts:174: the refusal's line, or None where the caller may drive it.
    pub fn refusal_for(&self, record: Option<&WorkspaceLike>, caller: Option<&Caller>) -> Result<Option<String>, HostError> {
        let Some(record) = record else {
            return Ok(None);
        };
        if !self.drives(record.kind, record.machine_id.as_deref(), caller) {
            return Ok(Some(relayed_refusal(&record.name)));
        }
        let Some(scope) = scope_of(caller) else {
            return Ok(None);
        };
        if let (Some(mine), Some(project)) = (self.project_of_scope(&scope), record.project.as_deref()) {
            if !self.of_threads_repository(caller, project)? {
                let mine_name = self.ctx.project_held(&mine)?.name;
                let project_name = self.ctx.project_held(project)?.name;
                return Ok(Some(spawn_repository_refusal(&scope.thread_id, &mine_name, &project_name)));
            }
        }
        if let Some(away) = self.away_for(record, caller, AcrossAct::Work, &record.name)? {
            return Ok(Some(away.message));
        }
        if !in_tree(record, &scope) {
            return Ok(Some(spawn_reach_refusal(&scope.thread_id, &record.name)));
        }
        Ok(None)
    }

    /// R15, rules.ts:194. `act` defaults to `work` at the call sites that name none.
    pub fn refuse_relayed(&self, record: Option<&WorkspaceLike>, caller: Option<&Caller>, act: AcrossAct, thread: Option<&str>) -> Result<(), HostError> {
        let Some(line) = self.refusal_for(record, caller)? else {
            return Ok(());
        };
        let Some(scope) = scope_of(caller) else {
            return Err(HostError::new(line));
        };
        if let (Some(record), Some(thread)) = (record, thread) {
            let word = format!("thread {}", thread_word(thread));
            return Err(match self.away_for(record, caller, act, &word)? {
                Some(away) => away,
                None => usage(&tree_thread_out_of_reach_line(thread, &scope.thread_id)),
            });
        }
        if let Some(record) = record {
            if self.lead_acts_on(record, caller)?.is_some() {
                if let Some(away) = self.away_for(record, caller, act, &record.name)? {
                    return Err(away);
                }
            }
        }
        Err(not_found_refusal(&no_workspace_refusal(None)))
    }

    /// R16, rules.ts:206.
    pub fn refuse_named(&self, workspace_id: &str, caller: Option<&Caller>) -> Result<(), HostError> {
        let record = self.live_record(workspace_id);
        if record.is_none() && scope_of(caller).is_some() {
            return Err(not_found_refusal(&no_workspace_refusal(None)));
        }
        let like = record.as_ref().map(like_of);
        self.refuse_relayed(like.as_ref(), caller, AcrossAct::Work, None)
    }

    /// R17, rules.ts:212.
    pub fn held(&self) -> Vec<Entry> {
        self.ctx.live().into_iter().filter(|entry| !entry.borrow().creating).collect()
    }

    /// R17, rules.ts:216.
    pub fn listed_for(&self, caller: Option<&Caller>) -> Result<Vec<Entry>, HostError> {
        let mut listed = vec![];
        for entry in self.held() {
            let like = like_of(&record_of_entry(&entry));
            if self.refusal_for(Some(&like), caller)?.is_none() {
                listed.push(entry);
            }
        }
        Ok(listed)
    }

    /// R18, rules.ts:220.
    pub fn refuse_recording(&self, named: &str, caller: Option<&Caller>) -> Result<(), HostError> {
        if road_of(caller) == Some(WorkspaceOrigin::Relayed) {
            return Err(HostError::new(relayed_record_refusal(named)));
        }
        Ok(())
    }

    /// `agentsRecordOf` (rules.ts:228).
    fn agents_record_of(&self, record: &WorkspaceRecord) -> Option<WorkspaceRecord> {
        match &record.root_thread_id {
            None => Some(record.clone()),
            Some(root) => self.record_of_thread(root),
        }
    }

    /// R20, rules.ts:235.
    pub fn agents_of(&self, record: &WorkspaceRecord) -> Option<WorkspaceAgents> {
        self.agents_record_of(record).map(|held| self.ctx.agents_held(&held))
    }

    /// R21, rules.ts:240.
    pub fn agents_held_at(&self, record: &WorkspaceRecord) -> CapsHeldAt {
        let held = self.agents_record_of(record).unwrap_or_else(|| record.clone());
        if held.agents.is_some() {
            return CapsHeldAt::Workspace(held.name);
        }
        let place_id = self.ctx.place_id_of(&held).unwrap_or_else(|| HERE_PLACE_ID.to_string());
        CapsHeldAt::Computer(self.ctx.door_name_of(&place_id).unwrap_or(place_id))
    }

    /// R19, rules.ts:249.
    pub fn parent_of(&self, thread_id: &str) -> Option<String> {
        self.ctx
            .thread_record(thread_id)
            .and_then(|thread| thread.parent_thread_id)
            .or_else(|| self.ctx.rows_on(thread_id).into_iter().find_map(|row| row.parent_thread_id))
    }

    /// R22, rules.ts:250.
    pub fn depth_under_root(&self, thread_id: &str) -> usize {
        let mut depth = 0;
        let mut at = thread_id.to_string();
        let steps = self.ctx.sessions_size() + self.ctx.thread_records_size() + 1;
        for _ in 0..steps {
            let Some(parent) = self.parent_of(&at) else {
                return depth;
            };
            depth += 1;
            at = parent;
        }
        depth
    }

    /// R19, rules.ts:262.
    pub fn root_of(&self, thread_id: &str) -> String {
        self.ctx
            .thread_record(thread_id)
            .and_then(|thread| thread.root_thread_id)
            .or_else(|| self.ctx.rows_on(thread_id).into_iter().find_map(|row| row.root_thread_id))
            .unwrap_or_else(|| thread_id.to_string())
    }

    /// R19, rules.ts:231.
    pub fn record_of_thread(&self, thread_id: &str) -> Option<WorkspaceRecord> {
        let at = self
            .ctx
            .thread_record(thread_id)
            .map(|thread| thread.workspace_id)
            .filter(|id| !id.is_empty())
            .or_else(|| self.ctx.rows_on(thread_id).first().and_then(|row| row.workspace_id.clone()));
        self.live_record(&at?)
    }

    /// R25, rules.ts:265: the reach's own keys, then `wsp`.
    pub fn agents_reach(&self, entry: &Entry) -> Option<Map<String, Value>> {
        let mut reach = self.ctx.turn_reach(entry)?;
        if let Some(wsp) = self.ctx.wsp_mcp(entry) {
            reach.insert("wsp".to_string(), wsp);
        }
        Some(reach)
    }

    /// R23, R24, rules.ts:283. The release frees a landing slot once; a no-op for every act but a fork that took one.
    pub fn spawn_guard(&self, act: SpawnAct, caller: Option<&Caller>) -> Result<Release, HostError> {
        let Some(scope) = scope_of(caller) else {
            return Ok(free_release());
        };
        let Some(own) = self.live_record(&scope.workspace_id) else {
            return Err(HostError::new(folder_gone_refusal(&scope.thread_id, act)));
        };
        let rooted = if scope.root_thread_id == scope.thread_id { Some(own.clone()) } else { self.record_of_thread(&scope.root_thread_id) };
        let Some(rooted) = rooted else {
            return Err(HostError::new(root_gone_refusal(&scope.thread_id, act)));
        };
        let mut over = vec![(rooted.clone(), self.agents_of(&rooted))];
        if rooted.id != own.id && runs_in_folder(&own.kind) {
            over.push((own.clone(), self.agents_of(&own)));
        }
        if !SPAWN_ACTS_ALLOWED.contains(&act) {
            return Err(self.act_refusal(&scope, act)?);
        }
        if let Some(lead) = &own.lead {
            if matches!(act, SpawnAct::ThreadNew | SpawnAct::Fork) {
                return Err(sub_starts_nothing_refusal(&scope.thread_id, &lead.thread_id));
            }
        }
        if matches!(act, SpawnAct::Send | SpawnAct::BringBack | SpawnAct::DeleteSub) {
            return Ok(free_release());
        }
        let depth = self.depth_under_root(&scope.thread_id);
        let mut caps = Vec::with_capacity(over.len());
        for (record, policy) in over {
            match policy {
                Some(policy) => caps.push((record, policy)),
                None => return Err(HostError::new(CAPS_UNREAD)),
            }
        }
        let Some(((first, first_policy), rest)) = caps.split_first() else {
            return Err(HostError::new(CAPS_UNREAD));
        };
        let (mut deepest, mut deepest_policy) = (first, first_policy);
        for (record, policy) in rest {
            if policy.max_depth.as_f64() < deepest_policy.max_depth.as_f64() {
                deepest = record;
                deepest_policy = policy;
            }
        }
        let max_depth = deepest_policy.max_depth.as_f64();
        let max_machines = caps.iter().map(|(_, policy)| policy.max_machines.as_f64()).fold(f64::INFINITY, f64::min);
        if depth as f64 >= max_depth {
            let held = match self.agents_held_at(deepest) {
                CapsHeldAt::Workspace(name) => DepthHeld::Workspace(name),
                CapsHeldAt::Computer(name) => DepthHeld::Computer(name),
            };
            return Err(HostError::new(spawn_depth_refusal(&scope.thread_id, depth as f64, max_depth, &held)));
        }
        if act != SpawnAct::Fork {
            return Ok(free_release());
        }
        let root = scope.root_thread_id.clone();
        let standing = self
            .ctx
            .live()
            .iter()
            .map(record_of_entry)
            .filter(|record| {
                record.root_thread_id.as_deref() == Some(root.as_str())
                    && !matches!(
                        workspace_state(&WorkspaceStateInput { phase: record.phase, machine_state: None, reach: None, wake_refused: None }),
                        WorkspaceState::Gone | WorkspaceState::Released
                    )
            })
            .count();
        let landing = self.ctx.landing();
        let landed = landing.borrow().get(&root).copied().unwrap_or(0);
        let counted = standing as i64 + landed;
        if counted as f64 >= max_machines {
            return Err(HostError::new(spawn_cap_refusal(&root, counted as f64, max_machines)));
        }
        landing.borrow_mut().insert(root.clone(), landed + 1);
        let freed = Cell::new(false);
        let release: Release = Rc::new(move || {
            if freed.get() {
                return;
            }
            freed.set(true);
            let now = landing.borrow().get(&root).copied().unwrap_or(1) - 1;
            if now <= 0 {
                landing.borrow_mut().shift_remove(&root);
            } else {
                landing.borrow_mut().insert(root.clone(), now);
            }
        });
        Ok(release)
    }

    /// R26, rules.ts:327.
    pub fn tree_of(&self, scope: Option<&ThreadScope>) -> TreeOf {
        match scope {
            None => TreeOf::default(),
            Some(scope) => TreeOf { parent_thread_id: Some(scope.thread_id.clone()), root_thread_id: Some(scope.root_thread_id.clone()) },
        }
    }

    /// R26, rules.ts:273.
    pub fn place_of_thread(&self, scope: &ThreadScope) -> Result<Option<String>, HostError> {
        let Some(own) = self.live_record(&scope.workspace_id) else {
            return Ok(None);
        };
        let computer = self.ctx.project_held(&own.project)?.computer;
        Ok(Some(thread_place(&own.kind, &self.ctx.place_name(&computer))))
    }

    /// R26, rules.ts:279.
    pub fn act_refusal(&self, scope: &ThreadScope, act: SpawnAct) -> Result<HostError, HostError> {
        let on = self.place_of_thread(scope)?;
        Ok(usage(&spawn_act_refusal(&scope.thread_id, act, on.as_deref())))
    }
}
