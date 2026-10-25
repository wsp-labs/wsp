// SPDX-License-Identifier: AGPL-3.0-only
//! Listing, resolving, what a project's computer holds, removing a project, the start defaults and the branch.
//! projects.ts:343-481 at 65f6b89c7; spec/projects_door.md.
use indexmap::IndexMap;
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::workspace::{Caller, ProjectView, SeedPlan};
use wsp_wire::views::workspace_more::ProjectBranch;
use wsp_words::exit::HostError;

use super::add::ComputerRow;
use super::Projects;

/// `remove`'s options. projects.ts:405.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RemoveWords {
    pub force: bool,
    pub check: bool,
    pub keep_look: bool,
}

/// `{ said, unsaved? }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovedProject {
    pub said: String,
    pub unsaved: Option<String>,
}

impl Projects {
    // ts: projects.ts:343-348
    //|     async list(origin?: Caller): Promise<ProjectView[]> {
    //|       await ctx.ready();
    //|       // A thread is served the projects it may start children on, so the listing and the start read one rule.
    //|       const scope = scopeOf(origin);
    //|       return [...projectsHeld.values()].filter(p => scope === undefined || ctx.projectReached(origin, p.id)).sort((a, b) => a.createdAt.localeCompare(b.createdAt));
    //|     },
    /// Every project, for a thread those it may start children on, by `createdAt`.
    pub async fn list(&self, origin: Option<Caller>) -> Result<Vec<ProjectView>, HostError> {
        todo!()
    }

    // ts: projects.ts:350-353
    //|     async computers(): Promise<{ id: string; name: string }[]> {
    //|       await ctx.ready();
    //|       return ctx.computerRows();
    //|     },
    pub async fn computers(&self) -> Result<Vec<ComputerRow>, HostError> {
        todo!()
    }

    // ts: projects.ts:355-379
    //|     async resolve(ref: string, origin?: Caller): Promise<ProjectView> {
    //|       await ctx.ready();
    //|       const all = [...projectsHeld.values()];
    //|       const found = all.find(p => p.id === ref) ?? all.find(p => p.name === ref);
    //|       // A thread works on its own project and its repository's on computers that fork machines. A project the
    //|       // person holds that it may not use is refused by the rule that says why, another repository's or a folder
    //|       // the person keeps of its own, in words that carry the word it typed; a word naming nothing, and every word
    //|       // for a thread whose workspace this host no longer holds, read as absent.
    //|       const scope = scopeOf(origin);
    //|       if (scope !== undefined) {
    //|         // Its own project by id or by name first, so another computer's project of the same name never stands in.
    //|         const mine = ctx.projectOfScope(scope);
    //|         if (mine === undefined) throw notFoundRefusal(bareNoSuchProjectLine(ref));
    //|         const own = projectsHeld.get(mine);
    //|         if (own !== undefined && (own.id === ref || own.name === ref)) return own;
    //|         if (found === undefined) throw notFoundRefusal(bareNoSuchProjectLine(ref));
    //|         if (ctx.projectReached(origin, found.id)) return found;
    //|         const away = ctx.elsewhereRefusal(origin, found, ref);
    //|         if (away !== undefined) throw away;
    //|         if (ctx.ofThreadsRepository(origin, found.id)) throw refusal(spawnFolderRefusal(scope.threadId, ref), SPAWN_FOLDER_FIX, "usage");
    //|         throw refusal(spawnRepositoryRefusal(scope.threadId, ctx.projectHeld(mine).name, ref), SPAWN_REPOSITORY_FIX, "usage");
    //|       }
    //|       if (found === undefined) throw notFoundRefusal(noSuchProjectLine(ref, all.map(p => p.name)));
    //|       return found;
    //|     },
    /// A word to a project, with the thread's own rules (spec rule 3).
    pub async fn resolve(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        todo!()
    }

    // ts: projects.ts:381-387
    //|     /** Whether the folder a project lives in still stands on its computer; a road where wsp keeps the project for
    //|      * itself holds no folder of the person's, so none stands. */
    //|     async folderStands(id: string): Promise<boolean> {
    //|       const project = await projectsDoor.resolve(id);
    //|       const { deps, at } = await landingDeps(project.computer);
    //|       return (await projectLanding(landingKind(project.computer, at)).folderStands?.(project, deps)) ?? false;
    //|     },
    pub async fn folder_stands(&self, id: &str) -> Result<bool, HostError> {
        todo!()
    }

    // ts: projects.ts:389-397
    //|     /** Copies files off a folder's seed menu into the folder its project stands in on its computer. */
    //|     async seedInto(id: string, plan: SeedPlan, files: readonly string[]): Promise<void> {
    //|       const project = await projectsDoor.resolve(id);
    //|       const { deps, at } = await landingDeps(project.computer);
    //|       const road = projectLanding(landingKind(project.computer, at));
    //|       if (road.seedInto === undefined) throw new Error(`${project.name} keeps no folder of its own on ${deps.computerName} to copy files into`);
    //|       const packed = await seedWiring().pack({ plan, choice: { files: [...files], memory: false, commits: false }, homes: await ctx.homesHere() });
    //|       await road.seedInto(project, packed.tar, deps);
    //|     },
    pub async fn seed_into(&self, id: &str, plan: SeedPlan, files: Vec<String>) -> Result<(), HostError> {
        todo!()
    }

    // ts: projects.ts:399-403
    //|     /** What the checkout the project's computer holds has that no remote does, one line, or nothing. */
    //|     async unsaved(project: ProjectView): Promise<string | undefined> {
    //|       const { deps, at } = await landingDeps(project.computer);
    //|       return projectLanding(landingKind(project.computer, at)).unsaved?.(project, deps);
    //|     },
    pub async fn unsaved(&self, project: &ProjectView) -> Result<Option<String>, HostError> {
        todo!()
    }

    // ts: projects.ts:405-437
    //|     async remove(id: string, origin?: Caller, o: { force?: boolean; check?: boolean; keepLook?: boolean } = {}): Promise<{ said: string; unsaved?: string }> {
    //|       await ctx.ready();
    //|       ctx.spawnGuard("delete", origin);
    //|       const project = await projectsDoor.resolve(id);
    //|       const held = [...live.values()].filter(e => e.record.project === project.id);
    //|       const standing = held.filter(e => !bareFolder(e.record, ctx.holdsThread(e.record.id))).map(e => e.record.name);
    //|       if (standing.length > 0) throw Object.assign(new Error(projectInUseRefusal(project.name, standing)), { kind: "conflict" });
    //|       const { deps, at, placeId } = await landingDeps(project.computer);
    //|       await readableComputer(at, placeId);
    //|       const road = projectLanding(landingKind(project.computer, at));
    //|       // The checkout on a computer of the person's goes with the record, and what it holds that no remote has with it.
    //|       const unsaved = o.force === true && o.check !== true ? undefined : await road.unsaved?.(project, deps);
    //|       if (unsaved !== undefined && o.force !== true) {
    //|         const refused = projectUnsavedRefusal(project.name, unsaved);
    //|         throw usageRefusal(refused.said, refused.fix);
    //|       }
    //|       if (o.check === true) return { said: "", ...(unsaved === undefined ? {} : { unsaved }) };
    //|       for (const entry of held) await ctx.workspaces.delete(entry.record.id, origin);
    //|       // The frozen copies its worktrees' overlays sat on go with it: no later worktree of this id reuses or drops them.
    //|       const top = project.git?.top;
    //|       if (top !== undefined && opts.statePath !== undefined && local?.copier !== undefined && copiesFolder(kindForComputer(project.computer))) {
    //|         await local.copier.worktreeForget({ from: top, home: ctx.stateFolder(), project: project.id }).catch((e: unknown) => console.warn(`the frozen copies of ${project.name}'s worktrees stayed: ${e instanceof Error ? e.message : String(e)}`));
    //|       }
    //|       // What the add made on that computer goes before the record does, so a computer that cannot be reached
    //|       // keeps both and the person can say it again when it is back. The sentence is the road's: it is true
    //|       // differently on a computer of theirs, at a provider and here.
    //|       const said = await road.remove(project, deps);
    //|       projectsHeld.delete(project.id);
    //|       await store.delete(PROJECTS, project.id);
    //|       bus.emit({ type: "project.removed", projectId: project.id });
    //|       if (o.keepLook !== true) await ctx.projectIcons.forget(project.id);
    //|       return { said };
    //|     },
    /// Removes a project with everything it made (spec rule 7).
    pub async fn remove_with(&self, id: &str, origin: Option<Caller>, o: RemoveWords) -> Result<RemovedProject, HostError> {
        todo!()
    }

    // ts: projects.ts:447-473
    //|     // Each project's against the lists its computer's agents answer, read once per computer, so what is said a new
    //|     // thread starts on is what the host will start it on there; wsp's table where that computer holds no folder.
    //|     async defaults(origin) {
    //|       const prefs = await ctx.preferences.get();
    //|       const runs = (id: string): boolean => adapters[id] !== undefined;
    //|       const firstAgent = CATALOG_AGENTS.find(a => runs(a.id))?.id ?? DEFAULT_AGENT.id;
    //|       const held = await projectsDoor.list(origin);
    //|       const read = new Map<string, Promise<HarnessCatalog[]>>();
    //|       const listsOn = (computer: string): Promise<HarnessCatalog[]> => {
    //|         const known = read.get(computer);
    //|         if (known !== undefined) return known;
    //|         const lists = ctx.computerHome(computer, origin).then(ctx.agentLists);
    //|         read.set(computer, lists);
    //|         return lists;
    //|       };
    //|       const resolved = await Promise.all(
    //|         held.map(async p => {
    //|           const lists = await listsOn(p.computer);
    //|           const catalogOf = (id: string): HarnessCatalog | undefined => {
    //|             const found = runs(id) ? lists.find(c => c.harness === id) : undefined;
    //|             return found === undefined ? undefined : withCustomModels(found, prefs.agentDefaults[id]?.models);
    //|           };
    //|           return [p.id, resolveThreadDefaults({ firstAgent, catalogOf, runs, prefs, ...(prefs.projectDefaults[p.id] !== undefined ? { project: prefs.projectDefaults[p.id] } : {}) })] as const;
    //|         }),
    //|       );
    //|       return Object.fromEntries(resolved);
    //|     },
    /// Each project's start defaults, each computer's agent lists read once.
    pub async fn defaults(&self, origin: Option<Caller>) -> Result<IndexMap<String, ThreadDefaults>, HostError> {
        todo!()
    }

    // ts: projects.ts:476-480
    //|     async branch(ref, origin) {
    //|       const project = await projectsDoor.resolve(ref, origin);
    //|       const { deps } = await landingDeps(project.computer);
    //|       return projectLanding(landingKind(project.computer)).branch(project, deps);
    //|     },
    pub async fn branch(&self, reference: &str, origin: Option<Caller>) -> Result<ProjectBranch, HostError> {
        todo!()
    }
}
