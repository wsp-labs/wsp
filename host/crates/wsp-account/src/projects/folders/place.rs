// SPDX-License-Identifier: AGPL-3.0-only
//! Where a thread on this computer runs: the project folder's record, a worktree's record, and `folderFor`.
//! folders.ts:16-200 at 65f6b89c7; spec/folders_place.md.
use std::rc::Rc;

use wsp_core::types::harness::StartPicksAsked;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_more::{CarryModule, WorktreeFolder, WorktreeReport};
use wsp_words::exit::HostError;

use super::Folders;
use crate::projects::workspaces::seam::Entry;

/// How long one command a new worktree runs may take. folders.ts:21.
pub const WORKTREE_COMMAND_MS: u64 = 3 * 60_000;

/// What `folderFor` is asked: the start's words. folders.ts:154.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FolderAsk {
    pub project: Option<String>,
    pub branch: Option<String>,
    pub cwd: Option<String>,
    pub picks: Option<StartPicksAsked>,
    /// `opens: true`: the start opens a thread, which takes the folder's setup line.
    pub opens: bool,
}

/// Where a thread runs: the folder's record, the folder inside it, and the line a fresh worktree left.
#[derive(Clone)]
pub struct FolderPlaced {
    pub entry: Entry,
    pub cwd: Option<String>,
    pub setup: Option<String>,
}

// ts: folders.ts:17-17
//| const CARRY_MODULES: CarryModule[] = ECOSYSTEM_MODULES.map(({ id, lockfiles, carry, never, installed }) => ({ id, lockfiles, carry, never, ...(installed !== undefined ? { installed } : {}) }));
/// The ecosystems as the worktree verb is handed them.
pub fn carry_modules() -> Vec<CarryModule> {
    todo!()
}

// ts: folders.ts:455-455
//|   const under = (path: string, folder: string): boolean => path === folder || path.startsWith(folder.endsWith("/") ? folder : `${folder}/`);
/// Whether a path is the folder or inside it.
pub fn under(path: &str, folder: &str) -> bool {
    todo!()
}

impl Folders {
    // ts: folders.ts:25-62
    //|   /** A folder's record, written once: the machine is the computer holding the project, this one or one the person
    //|    * joined, running, with auto-nap off, since a machine wsp does not run neither naps nor wakes. Its name is the
    //|    * project's, or the project's with the branch for a worktree, and nothing shows it to a person. */
    //|   const recordFolder = async (project: ProjectView, worktree?: WorktreeFolder, parent?: string): Promise<LiveWorkspace> => {
    //|     const kind = ctx.kindOf(project.computer);
    //|     const lands = workspaceLands(project.computer, undefined);
    //|     const place = lands.at === "place" ? lands.place : undefined;
    //|     await ctx.moduleOf(kind).admitFolder?.({ kind, name: project.name, ...(place !== undefined ? { place } : {}) });
    //|     const mine = ctx.backendOfKind(kind, place);
    //|     const machine = await mine.get(place ?? LOCAL_MACHINE_ID);
    //|     const id = `ws_${randomBytes(4).toString("hex")}`;
    //|     const taken = new Set([...live.values()].map(e => e.record.name));
    //|     const record: WorkspaceRecord = {
    //|       id,
    //|       name: takenNameAfter(worktree?.branch === undefined ? project.name : `${project.name}@${worktree.branch}`, taken),
    //|       kind,
    //|       ...(place !== undefined ? { place } : {}),
    //|       machineId: machine.id,
    //|       phase: "running",
    //|       golden: "",
    //|       createdAt: new Date(clock.now()).toISOString(),
    //|       project: project.id,
    //|       ...(worktree !== undefined ? { worktree } : {}),
    //|       ...(parent !== undefined ? { parentWorkspaceId: parent } : {}),
    //|       spec: {},
    //|       size: mine.pricing.defaultSize,
    //|       firstLife: false,
    //|       idleWindowMs: null,
    //|     };
    //|     ctx.attach(record, machine);
    //|     await ctx.persist(record);
    //|     // A project folder outside the person's home, or a worktree under the host's own folder, is outside the daemon's
    //|     // home root, and the host's start lists only the records it found, so the file is written before a turn's snapshot.
    //|     await ctx.writeDaemonRoots(live.get(id)!);
    //|     if (worktree?.made === true) armSweep();
    //|     bus.emit({ type: "workspace.created", workspace: ctx.view(record) });
    //|     return live.get(id)!;
    //|   };
    /// A folder's record, written once (spec rule 1).
    pub async fn record_folder(self: &Rc<Self>, project: &ProjectView, worktree: Option<WorktreeFolder>, parent: Option<String>) -> Result<Entry, HostError> {
        todo!()
    }

    // ts: folders.ts:63-68
    //|   /** The project folder's record, made at its first thread. */
    //|   const projectFolder = (project: ProjectView): Promise<LiveWorkspace> => {
    //|     const held = ctx.foldersOf(project.id).find(e => e.record.worktree === undefined);
    //|     if (held !== undefined) return Promise.resolve(held);
    //|     return ctx.oneFolder(`${project.id}\0`, () => recordFolder(project));
    //|   };
    /// The project folder's record, made at its first thread (spec rule 2).
    pub async fn project_folder(self: &Rc<Self>, project: &ProjectView) -> Result<Entry, HostError> {
        todo!()
    }

    // ts: folders.ts:69-90
    //|   /** The record of the worktree holding a branch of the project's repo: the one git already has the branch checked
    //|    * out in, wherever it is, or one the daemon binary makes under this host's folder. The project folder itself
    //|    * answers when it is the one holding the branch. */
    //|   const worktreeFolder = (project: ProjectView, top: string, branch: string, parent?: string, madeFor?: string, checkpoint?: string): Promise<LiveWorkspace> =>
    //|     ctx.oneFolder(`${project.id}\0${branch}`, async () => {
    //|       const copier = local?.copier;
    //|       if (copier === undefined) throw Object.assign(new Error(NO_COPIER_HERE), { kind: "invalid" });
    //|       // Before the binary is run at all: one older than this wsp answers a verb it never heard of with its usage text.
    //|       const here = local?.hereDaemon;
    //|       if (here !== undefined) {
    //|         const version = await here.version();
    //|         if (version < DAEMON_VERSION) throw new Error(hereDaemonBehindLine(version, DAEMON_VERSION, here.fix));
    //|       }
    //|       const made = await copier.worktree({ from: top, home: ctx.stateFolder(), project: project.id, branch, modules: CARRY_MODULES, ...(checkpoint !== undefined ? { checkpoint } : {}) });
    //|       if (made.path === top) return projectFolder(project);
    //|       const entry = await worktreeRecordAt(project, { path: made.path, branch: made.branch, made: made.made, ...(madeFor !== undefined ? { madeFor } : {}) }, parent);
    //|       if (made.fresh) {
    //|         const said = await settleWorktree(project, made);
    //|         if (said !== undefined) setupLines.set(entry.record.id, said);
    //|       }
    //|       return entry;
    //|     });
    /// The record of the worktree holding a branch of the project's repo (spec rule 3).
    pub async fn worktree_folder(
        self: &Rc<Self>,
        project: &ProjectView,
        top: &str,
        branch: &str,
        parent: Option<String>,
        made_for: Option<String>,
        checkpoint: Option<String>,
    ) -> Result<Entry, HostError> {
        todo!()
    }

    // ts: folders.ts:91-98
    //|   /** What a new worktree ran, kept for the start that names it first, whose thread opens with it; one short line per
    //|    * worktree made and not yet named by a start, dropped when it is taken or the worktree is removed. */
    //|   const setupLines = new Map<string, string>();
    //|   const takeSetupLine = (workspaceId: string): string | undefined => {
    //|     const said = setupLines.get(workspaceId);
    //|     setupLines.delete(workspaceId);
    //|     return said;
    //|   };
    /// Answers and forgets the line a fresh worktree left for its first start.
    pub fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        todo!()
    }

    // ts: folders.ts:99-126
    //|   /** What a worktree this call made runs once before any thread starts there, in order: the install of each module
    //|    * the verb found in a folder of it and not already installed for its lockfile, in that folder, then the project's
    //|    * own after-worktree command at its top. Each is said as it starts, runs with nothing on its stdin and no longer
    //|    * than WORKTREE_COMMAND_MS, and one that fails is said; the thread starts all the same, since the agent can run it
    //|    * again and read why. Answers the line the first thread there opens with. */
    //|   const settleWorktree = async (project: ProjectView, made: WorktreeReport): Promise<string | undefined> => {
    //|     const own = (await ctx.preferences.get()).projectDefaults[project.id]?.afterWorktree;
    //|     const installs = made.modules.filter(m => m.rebuild).flatMap(m => ECOSYSTEM_MODULES.filter(e => e.id === m.id).map(e => ({ command: e.rebuild, folder: m.folder })));
    //|     const commands = [...installs, ...(own !== undefined ? [{ command: own, folder: "." }] : [])];
    //|     if (commands.length === 0) return undefined;
    //|     const machine = await ctx.backendOfKind("local").get(LOCAL_MACHINE_ID);
    //|     const steps: string[] = [];
    //|     for (const { command, folder } of commands) {
    //|       const at = folder === "." ? made.path : join(made.path, folder);
    //|       const shown = homeShortened(at, homedir());
    //|       bus.emit({ type: "host.notice", message: worktreeCommandRunningLine(command, shown) });
    //|       const ran = await machine.exec(`cd ${shellLine([at])} && ${command}`, { timeoutMs: WORKTREE_COMMAND_MS, stdin: new Uint8Array() });
    //|       if (ran.exitCode === 0) {
    //|         steps.push(worktreeStepWords(command, folder));
    //|         continue;
    //|       }
    //|       const end = ran.exitCode === CHILD_TIMED_OUT ? { stoppedAfterMin: WORKTREE_COMMAND_MS / 60_000 } : { exitCode: ran.exitCode, said: lastLineOf(ran.stderr.trim() || ran.stdout.trim()) };
    //|       console.warn(`${command} in ${at} exited ${ran.exitCode}: ${ran.stderr.trim() || ran.stdout.trim()}`);
    //|       bus.emit({ type: "host.notice", message: worktreeCommandFailedLine(command, shown, end) });
    //|       steps.push(worktreeStepWords(command, folder, end));
    //|     }
    //|     return worktreeSetupLine(steps);
    //|   };
    /// What a worktree this call made runs once before any thread starts there; the line the first thread opens with.
    pub async fn settle_worktree(&self, project: &ProjectView, made: &WorktreeReport) -> Result<Option<String>, HostError> {
        todo!()
    }

    // ts: folders.ts:127-136
    //|   /** The dependency overlays of a worktree wsp made mounted again before a turn or a command runs there, where a
    //|    * restart of this computer took them; anything else asks nothing. One that cannot be mounted is said in the log and
    //|    * the turn runs all the same. */
    //|   const worktreeMounted = async (entry: LiveWorkspace): Promise<void> => {
    //|     const tree = entry.record.worktree;
    //|     const top = ctx.projectHeld(entry.record.project).git?.top;
    //|     const copier = local?.copier;
    //|     if (tree?.made !== true || tree.gone === true || top === undefined || copier === undefined || !existsSync(tree.path)) return;
    //|     await copier.worktreeMount({ from: top, home: ctx.stateFolder(), path: tree.path }).catch((e: unknown) => console.warn(`the worktree at ${tree.path} was not mounted again: ${e instanceof Error ? e.message : String(e)}`));
    //|   };
    /// The dependency overlays of a worktree wsp made mounted again before a turn or a command runs there.
    pub async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }

    // ts: folders.ts:137-149
    //|   /** The record naming a worktree at this path, written again where one stands from before (a worktree removed and
    //|    * made again comes back to its threads, and to the tree it was made for), else a new one, a child of the folder
    //|    * whose thread asked for it so its branch merges back there. */
    //|   const worktreeRecordAt = async (project: ProjectView, tree: WorktreeFolder, parent?: string): Promise<LiveWorkspace> => {
    //|     const held = ctx.foldersOf(project.id).find(e => e.record.worktree?.path === tree.path);
    //|     if (held === undefined) return recordFolder(project, tree, parent);
    //|     const was = held.record.worktree!;
    //|     const madeFor = was.madeFor;
    //|     held.record.worktree = { path: tree.path, made: was.made || tree.made, ...(tree.branch !== undefined ? { branch: tree.branch } : {}), ...(madeFor !== undefined ? { madeFor } : {}) };
    //|     await ctx.persist(held.record);
    //|     armSweep();
    //|     return held;
    //|   };
    /// The record naming a worktree at this path, written again where one stands, else a new child record.
    pub async fn worktree_record_at(self: &Rc<Self>, project: &ProjectView, tree: WorktreeFolder, parent: Option<String>) -> Result<Entry, HostError> {
        todo!()
    }

    // ts: folders.ts:150-158
    //|   /** Where a thread asked for on this computer runs: the record of its folder and the folder itself where the start
    //|    * named one. Nothing named runs in the project folder, or beside the thread asking; a branch runs in the worktree
    //|    * holding it, the project folder when it is that folder's own branch; a cwd runs where it is, inside the project
    //|    * folder or a worktree of its repo and nowhere else. */
    //|   const folderFor = async (o: { project?: string; branch?: string; cwd?: string; picks?: StartPicksAsked; opens?: true }, origin: Caller | undefined): Promise<{ entry: LiveWorkspace; cwd?: string; setup?: string }> => {
    //|     const at = await placeFor(o, origin);
    //|     const setup = o.opens === true ? takeSetupLine(at.entry.record.id) : undefined;
    //|     return setup === undefined ? at : { ...at, setup };
    //|   };
    /// Where a thread asked for on this computer runs, with the setup line where it opens.
    pub async fn folder_for(self: &Rc<Self>, ask: FolderAsk, origin: Option<Caller>) -> Result<FolderPlaced, HostError> {
        todo!()
    }

    // ts: folders.ts:159-200
    //|   const placeFor = async (o: { project?: string; branch?: string; cwd?: string; picks?: StartPicksAsked }, origin: Caller | undefined): Promise<{ entry: LiveWorkspace; cwd?: string }> => {
    //|     const scope = scopeOf(origin);
    //|     const asking = scope === undefined ? undefined : live.get(scope.workspaceId);
    //|     // A thread on a machine wsp forked has no folder to run beside: it names the workspace it means.
    //|     if (o.project === undefined && asking !== undefined && !runsInFolder(asking.record.kind)) throw Object.assign(new Error(refusalLine(guestNamesWorkspaceLine, GUEST_NAMES_FIX)), { kind: "usage" });
    //|     // A thread on a computer the person joined starts threads on that computer alone, said before the project rule
    //|     // reads the word as absent. A word its own project answers is its own, whatever another computer's is called.
    //|     const own = asking === undefined || asking.record.place === undefined ? undefined : ctx.projectHeld(asking.record.project);
    //|     const from = own?.computer;
    //|     const elsewhere = own === undefined || o.project === undefined || o.project === own.id || o.project === own.name ? undefined : [...projectsHeld.values()].find(p => (p.id === o.project || p.name === o.project) && p.computer !== from);
    //|     const away = elsewhere === undefined || o.project === undefined ? undefined : ctx.elsewhereRefusal(origin, elsewhere, o.project);
    //|     if (away !== undefined) throw away;
    //|     const project = o.project !== undefined ? await ctx.projectsDoor.resolve(o.project, origin) : asking !== undefined ? ctx.projectHeld(asking.record.project) : undefined;
    //|     if (project === undefined) throw Object.assign(new Error(NAME_A_PROJECT_LINE), { kind: "usage" });
    //|     const kind = ctx.kindOf(project.computer);
    //|     if (!runsInFolder(kind)) throw Object.assign(new Error(notOnThisComputerLine(project.name)), { kind: "usage" });
    //|     if (o.picks !== undefined) await ctx.picksHold(project, o.picks);
    //|     const beside = asking !== undefined && runsInFolder(asking.record.kind) && asking.record.project === project.id ? asking : undefined;
    //|     if (o.branch !== undefined && o.cwd !== undefined) throw Object.assign(new Error(BRANCH_OR_CWD_LINE), { kind: "usage" });
    //|     // A folder on a computer the person joined runs in the project folder or a folder inside it: its worktrees are
    //|     // read and made by this computer's own git and copier, which reach no other.
    //|     if (!copiesFolder(kind)) {
    //|       if (o.branch !== undefined) throw Object.assign(new Error(placeBranchLine(ctx.placeName(project.computer))), { kind: "usage" });
    //|       const cwd = o.cwd === undefined ? undefined : posix.normalize(o.cwd);
    //|       if (cwd !== undefined && (!posix.isAbsolute(cwd) || !under(cwd, project.path))) throw Object.assign(new Error(cwdOutsideLine(cwd, project.name)), { kind: "usage" });
    //|       return { entry: beside !== undefined && beside.record.worktree === undefined ? beside : await projectFolder(project), ...(cwd !== undefined ? { cwd } : {}) };
    //|     }
    //|     const top = project.git?.top;
    //|     if (o.cwd !== undefined) {
    //|       const cwd = folderNamed(o.cwd);
    //|       if (under(cwd, project.path)) return { entry: beside !== undefined && beside.record.worktree === undefined ? beside : await projectFolder(project), cwd };
    //|       const held = top === undefined ? undefined : (await worktreesOf(top)).find(w => under(cwd, w.path));
    //|       if (held === undefined || top === undefined) throw Object.assign(new Error(cwdOutsideLine(homeShortened(cwd, homedir()), project.name)), { kind: "usage" });
    //|       if (held.path === top) return { entry: await projectFolder(project), cwd };
    //|       // A worktree wsp holds no record of, one made with plain git worktree add, is the asking thread's tree's from here.
    //|       return { entry: await worktreeRecordAt(project, { path: held.path, ...(held.branch !== undefined ? { branch: held.branch } : {}), made: false, ...(scope !== undefined ? { madeFor: scope.rootThreadId } : {}) }), cwd };
    //|     }
    //|     if (o.branch === undefined) return { entry: beside ?? (await projectFolder(project)) };
    //|     if (top === undefined) throw Object.assign(new Error(noBranchesLine(project.name)), { kind: "usage" });
    //|     if ((await ctx.branchAt(beside !== undefined ? ctx.checkoutOf(beside.record) : project.path)) === o.branch) return { entry: beside ?? (await projectFolder(project)) };
    //|     return { entry: await worktreeFolder(project, top, o.branch, beside?.record.id, scope?.rootThreadId) };
    //|   };
    /// The start's words read against the asking thread (spec rule 10); `setup` is always none here.
    pub async fn place_for(self: &Rc<Self>, ask: &FolderAsk, origin: Option<Caller>) -> Result<FolderPlaced, HostError> {
        todo!()
    }
}
