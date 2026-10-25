// SPDX-License-Identifier: AGPL-3.0-only
//! Where a project's workspaces land, making one, and reading one or all back with the caller's rules.
//! workspaces.ts:66-288 at 65f6b89c7; spec/workspaces_create.md.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::api::Landing;
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, StageReport, WorkspaceRecord};
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_events::WorkspaceCreatingEvent;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceView};
use wsp_wire::wire::capabilities::Capabilities;
use wsp_words::exit::HostError;

use super::seam::{Base, Entry, FreeSlot};

/// What a lead's start tells a child's create. machines' `leadStart`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LeadStart {
    pub base: Option<String>,
    pub on_leads: bool,
    pub lines: Vec<String>,
}

/// What the create and lookup half reads off the runtime context, past `Base`.
#[async_trait(?Send)]
pub trait CreateCtx: Base {
    fn places_wired(&self) -> String;
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError>;
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError>;
    /// `ctx.landingBackend(placeId).capabilities`.
    async fn landing_capabilities(&self, place_id: Option<&str>) -> Result<Capabilities, HostError>;
    /// `ctx.backendOfKind(kind, place).capabilities`.
    fn backend_capabilities(&self, kind: WorkspaceKind, place: Option<&str>) -> Capabilities;
    /// `ctx.actRefusal(scope, act)`.
    fn act_refusal(&self, scope: &ThreadScope, act: &str) -> HostError;
    /// `ctx.spawnSub(opts, origin)`.
    async fn spawn_sub(&self, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError>;
    fn place_of_thread(&self, scope: &ThreadScope) -> Option<String>;
    /// `ctx.nameGiven(name)`.
    fn name_given(&self, name: &str) -> String;
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    /// `ctx.refuseRelayed({kind, name, project}, origin)` or of a record.
    fn refuse_relayed(&self, kind: WorkspaceKind, name: &str, project: &str, origin: Option<&Caller>) -> Result<(), HostError>;
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError>;
    /// `agentsFrom(ctx.spawnAt(place), agents)`.
    fn agents_from(&self, place: &str, agents: &serde_json::Map<String, serde_json::Value>) -> wsp_wire::views::workspace::WorkspaceAgents;
    fn same_repository(&self, a: &ProjectView, b: &ProjectView, origin: Option<&Caller>) -> bool;
    async fn place_guard(&self, place_id: &str) -> Result<(), HostError>;
    fn name_refusal(&self, name: &str) -> Option<String>;
    /// `ctx.forking.add/delete`.
    fn forking_add(&self, name: &str);
    fn forking_delete(&self, name: &str);
    fn supersede_failed(&self, name: &str);
    fn stage_reporter(&self, id: &str, name: &str, began: i64, scope: Option<&ThreadScope>) -> StageReport;
    async fn lead_start(&self, parent: &Entry, name: &str, how: &str) -> Result<LeadStart, HostError>;
    /// `ctx.showCreate(view)`.
    fn show_create(&self, view: WorkspaceView);
    /// `ctx.refOf(project)` and `ctx.treeOf(scope)`, as the shown view's fields.
    fn shown_view(&self, id: &str, name: &str, kind: WorkspaceKind, golden: &str, created_at: &str, project: &ProjectView, scope: &ThreadScope, parent: Option<&str>) -> WorkspaceView;
    async fn create_staged(
        &self,
        opts: CreateWorkspaceOptions,
        project: &ProjectView,
        id: &str,
        report: StageReport,
        scope: Option<ThreadScope>,
        free: FreeSlot,
        base: Option<String>,
    ) -> Result<CreatedWorkspace, HostError>;
    fn child_started_line(&self, name: &str, branch: &str) -> String;
    async fn start_child_on(&self, child: &Entry, base: &str, parent_name: &str) -> Result<String, HostError>;
    /// `ctx.unfork(entry)`; an `Err` that `is_missing` counts as gone.
    async fn unfork(&self, entry: &Entry) -> Result<(), wsp_engine::errors::EngineError>;
    /// `ctx.shownCreates`.
    fn shown_creates(&self) -> Rc<std::cell::RefCell<indexmap::IndexMap<String, WorkspaceView>>>;
    /// `ctx.failedCreates`.
    fn failed_creates(&self) -> Rc<std::cell::RefCell<indexmap::IndexMap<String, WorkspaceView>>>;
    fn failed_view(&self, id: &str, name: &str, kind: WorkspaceKind, golden: &str, began: i64, project: &ProjectView, said: &str) -> WorkspaceView;
    /// `await ctx.bootWork.get(id)`.
    async fn boot_work(&self, id: &str);
    fn listed_for(&self, origin: Option<&Caller>) -> Vec<Entry>;
    fn held(&self) -> Vec<Entry>;
    fn drives(&self, view: &WorkspaceView, origin: Option<&Caller>) -> bool;
    fn talks_to_tree_on(&self, id: &str, origin: Option<&Caller>) -> bool;
    fn project_of_scope(&self, origin: Option<&Caller>) -> Option<String>;
    fn refusal_for(&self, record: Option<&WorkspaceRecord>, origin: Option<&Caller>) -> Option<String>;
    fn of_threads_repository(&self, origin: Option<&Caller>, project_id: &str) -> bool;
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Option<HostError>;
    fn away_for(&self, record: &WorkspaceRecord, origin: Option<&Caller>, act: &str, word: &str) -> Option<HostError>;
    fn project_reached(&self, origin: Option<&Caller>, project_id: &str) -> bool;
    /// `placeDoor?.find(word)`: the names of the places it names.
    async fn find_places(&self, word: &str) -> Result<Vec<String>, HostError>;
    /// `ctx.createStages`, in insertion order.
    fn create_stages(&self) -> Vec<WorkspaceCreatingEvent>;
}

// ts: workspaces.ts:67-82
//|     async landing(o, origin) {
//|       await ctx.ready();
//|       const asked = scopeOf(origin);
//|       if (asked !== undefined && o.sized === true) throw ctx.actRefusal(asked, "size");
//|       const project = await ctx.projectsDoor.resolve(o.project, origin);
//|       const computer = project.computer;
//|       const kind = ctx.kindOf(computer);
//|       if (runsInFolder(kind)) {
//|         const lands = workspaceLands(computer, undefined);
//|         const place = lands.at === "place" ? lands.place : undefined;
//|         return { ...(place !== undefined ? { place } : {}), name: ctx.placeName(place ?? HERE_PLACE_ID), capabilities: ctx.backendOfKind(kind, place).capabilities, kind };
//|       }
//|       const { placeId } = await ctx.landingPlace(computer);
//|       const at = await ctx.landingBackend(placeId);
//|       return { ...(placeId !== undefined ? { place: placeId } : {}), name: ctx.placeName(placeId ?? places.wired), capabilities: at.capabilities, kind };
//|     },
pub async fn landing(ctx: &Rc<dyn CreateCtx>, project: &str, sized: bool, origin: Option<Caller>) -> Result<Landing, HostError> {
    todo!()
}

// ts: workspaces.ts:84-214
//|     async create(opts, origin) {
//|       await ctx.ready();
//|       if (opts.sub !== undefined) return ctx.spawnSub(opts, origin);
//|       const asked = scopeOf(origin);
//|       // A thread forks the image its own workspace's project runs and names none, at the size that image runs, as
//|       // the fork a run makes for it does: both are the person's to pick.
//|       if (asked !== undefined && opts.golden !== undefined) throw Object.assign(new Error(spawnGoldenRefusal(asked.threadId, ctx.placeOfThread(asked))), { kind: "usage" });
//|       if (asked !== undefined && (opts.cpu !== undefined || opts.memMb !== undefined)) throw ctx.actRefusal(asked, "size");
//|       // A create a thread asked for is a child of a workspace of its own tree: the one it names, which a fork does so
//|       // the new machine starts where the forked one's work is, else the one the thread runs on. Either way it sits
//|       // under the asking thread, so nothing a thread makes stands beside it as a sibling of the person's.
//|       if (asked !== undefined && opts.parent !== undefined) await ctx.entryOf(opts.parent, origin);
//|       const bornOf = asked === undefined ? opts.parent : (opts.parent ?? asked.workspaceId);
//|       const o = { ...opts, name: ctx.nameGiven(opts.name), ...(bornOf !== undefined ? { parent: bornOf } : {}) };
//|       const project = await ctx.projectsDoor.resolve(o.project, origin);
//|       // The project's computer decides which road this create takes, off the one table that says whether a thread
//|       // of a kind runs in its project's folder or on a fork of an image; the origin rule is then read on that kind.
//|       const kind = ctx.kindOf(project.computer);
//|       // A create on a project whose threads run in its folder makes nothing, so one naming a parent is a fork of that
//|       // folder, which is no machine: refused rather than answered with the folder's own record, which would read as a
//|       // machine made. A child on another computer under a lead in a folder names the folder as its parent and stands.
//|       if (opts.parent !== undefined && runsInFolder(kind)) throw refusal(folderForkRefusal(project.name), folderForkFix(project.name), "usage");
//|       // The same rule and the same sentence the verb that sets the switch on a workspace that exists reads, so a
//|       // create on a computer whose agents could not drive this host is refused rather than given a dead switch.
//|       // Read before anything is asked of a machine: the project rule refuses a thread naming another project here,
//|       // as the same reading refuses it every workspace of one. A computer that copies its folders takes no relayed
//|       // request at all and says so in its own words below.
//|       // A project on this computer or on one the person joined is its folder: a create there names the folder's
//|       // record, the one every thread in that folder shares, and makes nothing.
//|       if (runsInFolder(kind)) {
//|         ctx.refuseRecording(o.name, origin);
//|         // The folder is not forked, so the words a fork takes have nothing to act on: refused rather than ignored.
//|         const forkWords = [o.golden !== undefined ? "--from" : "", o.cpu !== undefined || o.memMb !== undefined ? "--size" : ""].filter(w => w !== "");
//|         if (forkWords.length > 0) throw Object.assign(new Error(copyTakesNone(project.name, forkWords, ...(copiesFolder(kind) ? [] : [ctx.placeName(project.computer)]))), { kind: "invalid" });
//|         if (asked !== undefined && o.agents !== undefined) throw ctx.actRefusal(asked, "agents");
//|         const folder = await ctx.projectFolder(project);
//|         if (o.agents !== undefined) {
//|           folder.record.agents = agentsFrom(ctx.spawnAt(folder.record.place ?? HERE_PLACE_ID), o.agents);
//|           await ctx.persist(folder.record);
//|         }
//|         return ctx.view(folder.record);
//|       }
//|       ctx.refuseRelayed({ kind, name: o.name, project: project.id }, origin);
//|       // A child of another workspace starts where that workspace is now, not where the project starts. A parent
//|       // this host does not hold, and one whose own create has not finished, are refused rather than dropped, since
//|       // a create that dropped it would land as somebody's root; the branch itself is read off the parent's machine
//|       // below, once this create is allowed.
//|       const parent = o.parent === undefined ? undefined : live.get(o.parent);
//|       if (o.parent !== undefined && (parent === undefined || parent.creating === true)) throw Object.assign(new Error(noParentWorkspaceLine(o.parent)), { kind: "invalid" });
//|       // A child is a second checkout of its parent's repository on the branch that parent is on, so a parent holding
//|       // another repository has no branch this child could start from and land its work back in. The same repository
//|       // added on another computer is the same code, which is how a lead on this computer starts a child on a box.
//|       if (parent !== undefined && parent.record.project !== project.id && !ctx.sameRepository(ctx.projectHeld(parent.record.project), project, origin)) {
//|         throw Object.assign(new Error(parentProjectRefusal(parent.record.name, ctx.projectHeld(parent.record.project).name, project.name)), { kind: "invalid" });
//|       }
//|       await ctx.placeGuard((await ctx.landingPlace(project.computer)).placeId ?? places.wired);
//|       // The place under the root is taken here, with no await between the count and the taking, and handed back in
//|       // the finally below however this create ends: the record it becomes is what holds it from then on. A copy on
//|       // this computer is a child in the tree as a fork is, so it takes the same place.
//|       const freePlace = ctx.spawnGuard("fork", origin);
//|       const spawned = scopeOf(origin);
//|       // A thread's fork carries the switch of the workspace it was asked from, and nothing the caller says: the
//|       // caps are the person's, and a fork naming its own would be the agents act by another road.
//|       if (spawned !== undefined && o.agents !== undefined) {
//|         freePlace();
//|         throw ctx.actRefusal(spawned, "agents");
//|       }
//|       const nameTaken = ctx.nameRefusal(o.name);
//|       if (nameTaken !== undefined) {
//|         freePlace();
//|         throw Object.assign(new Error(nameTaken), { kind: "conflict" });
//|       }
//|       ctx.forking.add(o.name);
//|       ctx.supersedeFailed(o.name);
//|       const id = `ws_${randomBytes(4).toString("hex")}`;
//|       const began = clock.now();
//|       const report = ctx.stageReporter(id, o.name, began, spawned);
//|       try {
//|         // Read inside the try, so a parent that did not answer gives the name and the slot back the way every
//|         // other end of this create does, and after the guard, so what a thread may do is decided before anything
//|         // is asked of a machine.
//|         // Only a fork names its parent; a create that names none is a thread's start, a child of its own workspace.
//|         const start = parent === undefined ? undefined : await ctx.leadStart(parent, o.name, opts.parent !== undefined ? "fork" : "run");
//|         // A fork a thread asked for stands under that thread before its first stage, as a sub-thread's record does.
//|         if (spawned !== undefined) {
//|           ctx.showCreate({ id, name: o.name, machineId: "", phase: "queued", kind, golden: o.golden ?? "", createdAt: new Date(began).toISOString(), project: ctx.refOf(project), ...ctx.treeOf(spawned), ...(o.parent !== undefined ? { parentWorkspaceId: o.parent } : {}) });
//|         }
//|         const made = await ctx.createStaged(o, project, id, report, spawned, freePlace, start?.base);
//|         if (start === undefined) return made;
//|         // A brokered child's clone already stands on its parent's commit, on its own branch of record.
//|         const child = live.get(id)!;
//|         const ofRecord = child.record.branchOfRecord;
//|         const onLead = start.onLeads && start.base !== undefined ? [ofRecord !== undefined ? childStartedLine(child.record.name, ofRecord.branch) : await ctx.startChildOn(child, start.base, parent!.record.name)] : [];
//|         const lines = [...start.lines, ...onLead];
//|         return lines.length === 0 ? made : { ...made, notice: [made.notice, ...lines].filter(l => l !== undefined).join("\n") };
//|       } catch (e) {
//|         // A machine already forked goes with the failed create, so the retry forks a fresh one; one the provider
//|         // will not part with keeps its record instead, since a machine nobody records bills unseen.
//|         const entry = live.get(id);
//|         let kept: LiveWorkspace | undefined;
//|         if (entry !== undefined) {
//|           const gone = await ctx.unfork(entry).then(() => true, (k: unknown) => isMissing(k));
//|           if (gone) live.delete(id);
//|           else {
//|             kept = entry;
//|             delete entry.creating;
//|             await ctx.persist(entry.record).catch((p: unknown) => console.warn(`workspace ${id} not stored: ${p instanceof Error ? p.message : String(p)}`));
//|             console.warn(`workspace ${id} failed to create and its machine ${entry.machine.id} would not stop; the record stays for wsp delete`);
//|           }
//|         }
//|         // The id dies with a failed create, so nothing retries under its key but the sweep, which asks again an
//|         // attempt whose answer was lost and whose bytes are kept, and deletes the machine it made.
//|         if (((await store.get(CREATES, `workspace/${id}`)) as PendingCreate | undefined)?.request === undefined) await store.delete(CREATES, `workspace/${id}`);
//|         const said = e instanceof Error ? e.message : String(e);
//|         const shown = ctx.shownCreates.get(id);
//|         ctx.shownCreates.delete(id);
//|         if (kept === undefined) {
//|           // A fork a thread asked for stays under that thread, failed with why, until the person deletes it.
//|           const failed = shown === undefined ? ctx.failedView(id, o.name, kind, o.golden ?? "", began, project, said) : { ...shown, phase: "gone" as const, gone: said };
//|           ctx.failedCreates.set(id, failed);
//|           if (shown !== undefined) bus.emit({ type: "workspace.created", workspace: failed });
//|         }
//|         report("failed", said);
//|         if (kept !== undefined) bus.emit({ type: "workspace.created", workspace: ctx.view(kept.record) });
//|         throw e;
//|       } finally {
//|         ctx.shownCreates.delete(id);
//|         ctx.forking.delete(o.name);
//|         freePlace();
//|       }
//|     },
/// Makes a workspace (spec rule 2).
pub async fn create(ctx: &Rc<dyn CreateCtx>, opts: CreateWorkspaceOptions, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
    todo!()
}

// ts: workspaces.ts:216-218
//|     async get(id, origin, threadId) {
//|       return ctx.view((await ctx.entryOf(id, origin, threadId === undefined ? {} : { thread: threadId })).record);
//|     },
pub async fn get(ctx: &Rc<dyn CreateCtx>, id: &str, origin: Option<Caller>, thread_id: Option<&str>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:220-225
//|     async heldOrCreating(id) {
//|       await ctx.ready();
//|       await ctx.bootWork.get(id);
//|       const entry = live.get(id);
//|       return entry === undefined ? undefined : ctx.view(entry.record);
//|     },
pub async fn held_or_creating(ctx: &Rc<dyn CreateCtx>, id: &str) -> Result<Option<WorkspaceView>, HostError> {
    todo!()
}

// ts: workspaces.ts:229-235
//|     async list(origin) {
//|       await ctx.ready();
//|       const listed = ctx.listedFor(origin).map(e => ctx.view(e.record));
//|       if (scopeOf(origin) !== undefined) return listed;
//|       const forks = [...ctx.shownCreates.values(), ...[...ctx.failedCreates.values()].filter(v => v.parentThreadId !== undefined)];
//|       return [...listed, ...forks.filter(v => ctx.drives(v, origin) && !listed.some(l => l.id === v.id))];
//|     },
pub async fn list(ctx: &Rc<dyn CreateCtx>, origin: Option<Caller>) -> Result<Vec<WorkspaceView>, HostError> {
    todo!()
}

// ts: workspaces.ts:237-287
//|     async resolve(ref, origin) {
//|       await ctx.ready();
//|       const scope = scopeOf(origin);
//|       const rows = scope === undefined ? ctx.held() : ctx.listedFor(origin);
//|       // The whole of an id, then the whole of a name, as a thread's own reference does: a name names one workspace at
//|       // most, since the create and the rename both refuse a name another already holds, and a word that is one is
//|       // that workspace whatever else it starts. Only a word that is neither reaches the prefix, where enough of an
//|       // id is the way round quoting a name with spaces and a word that starts two is refused with both ids.
//|       // A thread on a computer the person joined names by its id the workspace its lead's message goes to.
//|       const tree = scope === undefined ? undefined : ctx.held().find(e => e.record.id === ref && ctx.talksToTreeOn(ref, origin));
//|       const exact = rows.find(e => e.record.id === ref) ?? tree ?? rows.find(e => e.record.name === ref);
//|       const started = exact === undefined && ref.length >= ID_PREFIX_MIN ? rows.filter(e => e.record.id.startsWith(ref)) : [];
//|       if (started.length > 1) throw new Error(idPrefixRefusal(ref, started.map(e => e.record.id)));
//|       const entry = exact ?? started[0];
//|       if (entry === undefined) {
//|         if (scope !== undefined) {
//|           // A whole id or name the person holds outside the thread's tree, a workspace's or a project's, is refused
//|           // by the rule that keeps it out, so a lead learns what to do instead, in words that carry the word the
//|           // thread typed and nothing of the workspace it may not see. A word that names nothing, or only starts an
//|           // id, reads as absent. A run names a project here when the thread's listing did not carry it, and the
//|           // project door refuses one the thread may not use in its own words. A thread on a box naming a workspace of
//|           // another computer reads the computer rule: a folder as its project reads on the run road; otherwise a
//|           // message to its lead where the lead acts there itself, in the act's words, and else the person.
//|           const mine = ctx.projectOfScope(scope);
//|           const theirs = ctx.held().find(e => e.record.id === ref) ?? ctx.held().find(e => e.record.name === ref);
//|           if (theirs !== undefined && mine !== undefined && ctx.refusalFor(theirs.record, origin) !== undefined) {
//|             const project = ctx.projectHeld(theirs.record.project);
//|             if (!ctx.ofThreadsRepository(origin, project.id)) throw refusal(spawnRepositoryWorkspaceRefusal(scope.threadId, ctx.projectHeld(mine).name, ref), SPAWN_REPOSITORY_WORKSPACE_FIX, "usage");
//|             const away = runsInFolder(theirs.record.kind) ? ctx.elsewhereRefusal(origin, project, ref) : ctx.awayFor(theirs.record, origin, "work", ref);
//|             if (away !== undefined) throw away;
//|             const folder = runsInFolder(ctx.kindOf(project.computer));
//|             if (!ctx.projectReached(origin, project.id) && folder) throw refusal(spawnFolderRefusal(scope.threadId, ref), SPAWN_FOLDER_FIX, "usage");
//|             throw refusal(spawnReachRefusal(scope.threadId, ref), spawnReachFix(project.name, !folder), "usage");
//|           }
//|           await ctx.projectsDoor.resolve(ref, origin).catch((e: unknown) => {
//|             if ((e as { kind?: unknown }).kind !== "not-found") throw e;
//|           });
//|           throw notFoundRefusal(noWorkspaceRefusal(ref));
//|         }
//|         const failed = [...ctx.failedCreates.values()].find(v => v.id === ref || v.name === ref);
//|         if (failed !== undefined) return failed;
//|         // A computer somebody joined is a place, and a place is no workspace: the word is answered with the road to
//|         // one there rather than with absence, since the person typed the name of something this host does hold.
//|         const place = (await placeDoor?.find(ref)) ?? [];
//|         if (place.length > 0) throw notFoundRefusal(refusalLine(placeNotAWorkspaceLine(place[0]!.name), placeNotAWorkspaceFix(place[0]!.name)));
//|         throw notFoundRefusal(noWorkspaceRefusal(ref));
//|       }
//|       if (entry === tree) return WorkspaceTalked.parse(ctx.view(entry.record));
//|       ctx.refuseRelayed(entry.record, origin);
//|       return ctx.view(entry.record);
//|     },
/// A word to a workspace with the caller's rules (spec rule 7).
pub async fn resolve(ctx: &Rc<dyn CreateCtx>, reference: &str, origin: Option<Caller>) -> Result<WorkspaceView, HostError> {
    todo!()
}

// ts: workspaces.ts:1211-1215
//|     creating() {
//|       // A sub-thread's record and a thread's fork stand under their lead before their machine is made, so a stage of
//|       // one replayed ahead of a window's list would be filed there as a create at the root.
//|       return [...ctx.createStages.values()].filter(s => live.get(s.workspaceId)?.record.lead === undefined && !ctx.shownCreates.has(s.workspaceId));
//|     },
/// Every create stage of a root create in flight.
pub fn creating(ctx: &Rc<dyn CreateCtx>) -> Vec<WorkspaceCreatingEvent> {
    todo!()
}
