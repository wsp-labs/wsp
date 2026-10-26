// SPDX-License-Identifier: AGPL-3.0-only
//! Where a thread on this computer runs: the project folder's record, a worktree's record, and `folderFor`.
//! folders.ts:16-200 at 65f6b89c7; spec/folders_place.md.
use std::path::Path;
use std::rc::Rc;

use futures::FutureExt;
use indexmap::IndexSet;
use serde_json::json;
use wsp_catalog::git_hosts::ECOSYSTEM_MODULES;
use wsp_core::console::warn;
use wsp_core::types::harness::StartPicksAsked;
use wsp_core::types::wiring::{folder_named, last_line_of, WorkspaceRecord, NO_COPIER_HERE};
use wsp_engine::child_exec::CHILD_TIMED_OUT;
use wsp_wire::types::projects::{BRANCH_OR_CWD_LINE, NAME_A_PROJECT_LINE};
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::workspace::{scope_of, Caller, ProjectView};
use wsp_wire::views::workspace_more::{CarryModule, WorktreeAsk, WorktreeFolder, WorktreeReport};
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::js_date::iso_string;
use wsp_words::projects::{
    copies_folder, cwd_outside_line, home_shortened, no_branches_line, not_on_this_computer_line, place_branch_line, runs_in_folder,
    workspace_lands, worktree_command_failed_line, worktree_command_running_line, worktree_setup_line, worktree_step_words, WorkspaceLands,
    WorktreeCommandEnd,
};
use wsp_words::start::taken_name_after;
use wsp_words::words::computer::{GUEST_NAMES_FIX, GUEST_NAMES_WORKSPACE_LINE};
use wsp_words::words::thread::refusal_line;
use wsp_words::words::units::here_daemon_behind_line;

use super::Folders;
use crate::projects::workspaces::seam::{record_of, Entry};

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
    ECOSYSTEM_MODULES.iter().map(|module| module.carry.clone()).collect()
}

/// Node's `path.posix.normalize`: `.` and empty segments fold, `..` pops where it can, a trailing slash stays.
fn normalize(path: &str) -> String {
    let absolute = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(last) if *last != ".." => {
                    parts.pop();
                }
                _ if !absolute => parts.push(".."),
                _ => {}
            },
            kept => parts.push(kept),
        }
    }
    let mut out = parts.join("/");
    if absolute {
        out.insert(0, '/');
    } else if out.is_empty() {
        out.push('.');
    }
    if path.ends_with('/') && !out.ends_with('/') {
        out.push('/');
    }
    out
}

/// `process.cwd()`, the folder a relative cwd is read against.
fn process_cwd() -> String {
    std::env::current_dir().map(|dir| dir.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".to_owned())
}

// ts: folders.ts:455-455
//|   const under = (path: string, folder: string): boolean => path === folder || path.startsWith(folder.endsWith("/") ? folder : `${folder}/`);
/// Whether a path is the folder or inside it.
pub fn under(path: &str, folder: &str) -> bool {
    let prefix = if folder.ends_with('/') { folder.to_owned() } else { format!("{folder}/") };
    path == folder || path.starts_with(&prefix)
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
    pub async fn record_folder(
        self: &Rc<Self>,
        project: &ProjectView,
        worktree: Option<WorktreeFolder>,
        parent: Option<String>,
    ) -> Result<Entry, HostError> {
        let ctx = self.ctx.clone();
        let kind = ctx.kind_of(&project.computer);
        let place = match workspace_lands(&project.computer, None) {
            WorkspaceLands::Place { place } => Some(place),
            WorkspaceLands::Here | WorkspaceLands::Wired => None,
        };
        ctx.admit_folder(kind, &project.name, place.as_deref()).await?;
        let (machine, size) = ctx.folder_machine(kind, place.as_deref()).await?;
        let id = format!("ws_{}", hex::encode(ctx.ids().bytes(4)));
        let taken: IndexSet<String> = ctx.live().borrow().values().map(|entry| record_of(entry).name).collect();
        let named = match worktree.as_ref().and_then(|tree| tree.branch.as_deref()) {
            Some(branch) => format!("{}@{branch}", project.name),
            None => project.name.clone(),
        };
        let made = worktree.as_ref().is_some_and(|tree| tree.made);
        let created_at = iso_string(ctx.clock().now() as f64).unwrap_or_default();
        let mut value = json!({
            "id": id,
            "name": taken_name_after(&named, &taken),
            "kind": kind,
            "machineId": machine.id(),
            "phase": "running",
            "golden": "",
            "createdAt": created_at,
            "project": project.id,
            "spec": {},
            "size": size,
            "firstLife": false,
            "idleWindowMs": null,
        });
        if let Some(place) = &place {
            value["place"] = json!(place);
        }
        if let Some(tree) = &worktree {
            value["worktree"] = serde_json::to_value(tree).map_err(|e| HostError::new(e.to_string()))?;
        }
        if let Some(parent) = &parent {
            value["parentWorkspaceId"] = json!(parent);
        }
        let record: WorkspaceRecord = serde_json::from_value(value).map_err(|e| HostError::new(e.to_string()))?;
        let entry = ctx.attach(record, machine);
        ctx.persist(record_of(&entry)).await?;
        ctx.write_daemon_roots(&entry, false).await?;
        if made {
            self.arm_sweep();
        }
        let workspace = serde_json::to_value(ctx.view(&record_of(&entry))).map_err(|e| HostError::new(e.to_string()))?;
        ctx.emit(json!({ "type": "workspace.created", "workspace": workspace }))?;
        Ok(entry)
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
        if let Some(held) = self.ctx.folders_of(&project.id).into_iter().find(|entry| record_of(entry).worktree.is_none()) {
            return Ok(held);
        }
        let me = self.clone();
        let owned = project.clone();
        let make = async move { me.record_folder(&owned, None, None).await }.boxed_local();
        self.ctx.one_folder(&format!("{}\0", project.id), make).await
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
        let key = format!("{}\0{}", project.id, branch);
        let me = self.clone();
        let project = project.clone();
        let (top, branch) = (top.to_owned(), branch.to_owned());
        let make = async move {
            if !me.ctx.has_copier() {
                return Err(HostError::with_kind(NO_COPIER_HERE, Kind::Invalid));
            }
            if let Some(here) = me.ctx.here_daemon() {
                let version = here.version().await?;
                if i64::from(version) < DAEMON_VERSION {
                    let behind = here_daemon_behind_line(f64::from(version), DAEMON_VERSION as f64, &here.fix());
                    return Err(HostError::new(behind));
                }
            }
            let ask = WorktreeAsk {
                from: top.clone(),
                home: me.ctx.state_folder(),
                project: project.id.clone(),
                branch: branch.clone(),
                modules: carry_modules(),
                checkpoint,
            };
            let made = me.ctx.copier_worktree(ask).await?;
            if made.path == top {
                return me.project_folder(&project).await;
            }
            let tree = WorktreeFolder {
                path: made.path.clone(),
                branch: Some(made.branch.clone()),
                made: made.made,
                made_for,
                settled: None,
                kept: None,
                remove_failed: None,
                gone: None,
            };
            let entry = me.worktree_record_at(&project, tree, parent).await?;
            if made.fresh {
                if let Some(said) = me.settle_worktree(&project, &made).await? {
                    me.setup_lines.borrow_mut().insert(record_of(&entry).id, said);
                }
            }
            Ok(entry)
        }
        .boxed_local();
        self.ctx.one_folder(&key, make).await
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
        self.setup_lines.borrow_mut().shift_remove(workspace_id)
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
        let own =
            self.ctx.preferences().await?.project_defaults.get(&project.id).and_then(|fields| fields.clone().into_inner().after_worktree);
        let mut commands: Vec<(String, String)> = Vec::new();
        for module in made.modules.iter().filter(|module| module.rebuild) {
            for ecosystem in ECOSYSTEM_MODULES.iter().filter(|ecosystem| ecosystem.carry.id == module.id) {
                commands.push((ecosystem.rebuild.clone(), module.folder.clone()));
            }
        }
        if let Some(own) = own {
            commands.push((own, ".".to_owned()));
        }
        if commands.is_empty() {
            return Ok(None);
        }
        let home = self.ctx.home_dir();
        let mut steps = Vec::new();
        for (command, folder) in commands {
            let at = if folder == "." { made.path.clone() } else { normalize(&format!("{}/{folder}", made.path)) };
            let shown = home_shortened(&at, Some(home.as_str()));
            self.ctx.emit(json!({ "type": "host.notice", "message": worktree_command_running_line(&command, &shown) }))?;
            let cmd = format!("cd {} && {command}", shell_line(&[at.as_str()]));
            let ran = self.ctx.local_exec(&cmd, WORKTREE_COMMAND_MS, Some(Vec::new())).await?;
            let code = ran.exit_code.get();
            if code == 0 {
                steps.push(worktree_step_words(&command, &folder, None));
                continue;
            }
            let said_in = if js_trim(&ran.stderr).is_empty() { js_trim(&ran.stdout) } else { js_trim(&ran.stderr) };
            let end = if code == CHILD_TIMED_OUT {
                WorktreeCommandEnd::Stopped { stopped_after_min: (WORKTREE_COMMAND_MS / 60_000) as f64 }
            } else {
                WorktreeCommandEnd::Exited { exit_code: code as i32, said: last_line_of(said_in) }
            };
            warn(&format!("{command} in {at} exited {code}: {said_in}"));
            self.ctx.emit(json!({ "type": "host.notice", "message": worktree_command_failed_line(&command, &shown, &end) }))?;
            steps.push(worktree_step_words(&command, &folder, Some(&end)));
        }
        Ok(Some(worktree_setup_line(&steps)))
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
        let record = record_of(entry);
        let top = self.ctx.project_held(&record.project)?.git.map(|git| git.top);
        let (Some(tree), Some(top)) = (record.worktree.as_ref(), top) else {
            return Ok(());
        };
        if !tree.made || tree.gone.is_some() || !self.ctx.has_copier() || !Path::new(&tree.path).exists() {
            return Ok(());
        }
        let home = self.ctx.state_folder();
        if let Err(e) = self.ctx.copier_worktree_mount(&top, &home, &tree.path).await {
            warn(&format!("the worktree at {} was not mounted again: {}", tree.path, e.message));
        }
        Ok(())
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
    pub async fn worktree_record_at(
        self: &Rc<Self>,
        project: &ProjectView,
        tree: WorktreeFolder,
        parent: Option<String>,
    ) -> Result<Entry, HostError> {
        let found = self.ctx.folders_of(&project.id).into_iter().find_map(|entry| {
            let was = record_of(&entry).worktree?;
            (was.path == tree.path).then_some((entry, was))
        });
        let Some((held, was)) = found else {
            return self.record_folder(project, Some(tree), parent).await;
        };
        let worktree = WorktreeFolder {
            path: tree.path.clone(),
            branch: tree.branch.clone(),
            made: was.made || tree.made,
            made_for: was.made_for.clone(),
            settled: None,
            kept: None,
            remove_failed: None,
            gone: None,
        };
        held.borrow().record.borrow_mut().worktree = Some(worktree);
        self.ctx.persist(record_of(&held)).await?;
        self.arm_sweep();
        Ok(held)
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
        let opens = ask.opens;
        let at = self.place_for(&ask, origin).await?;
        if !opens {
            return Ok(at);
        }
        let id = record_of(&at.entry).id;
        match self.take_setup_line(&id) {
            Some(setup) => Ok(FolderPlaced { setup: Some(setup), ..at }),
            None => Ok(at),
        }
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
        let ctx = self.ctx.clone();
        let scope = scope_of(origin.as_ref());
        let scope_root = scope.as_ref().map(|scope| scope.root_thread_id.clone());
        let live = ctx.live();
        let asking = scope.as_ref().and_then(|scope| live.borrow().get(&scope.workspace_id).cloned());
        let asking_record = asking.as_ref().map(record_of);
        if ask.project.is_none() && asking_record.as_ref().is_some_and(|record| !runs_in_folder(&record.kind)) {
            return Err(HostError::with_kind(refusal_line(GUEST_NAMES_WORKSPACE_LINE, GUEST_NAMES_FIX), Kind::Usage));
        }
        let own = match &asking_record {
            Some(record) if record.place.is_some() => Some(ctx.project_held(&record.project)?),
            _ => None,
        };
        let elsewhere = match (&own, &ask.project) {
            (Some(own), Some(word)) if *word != own.id && *word != own.name => {
                let held = ctx.projects_held();
                let found = held.borrow().values().find(|p| (p.id == *word || p.name == *word) && p.computer != own.computer).cloned();
                found
            }
            _ => None,
        };
        if let (Some(elsewhere), Some(word)) = (&elsewhere, &ask.project) {
            if let Some(away) = ctx.elsewhere_refusal(origin.as_ref(), elsewhere, word) {
                return Err(away);
            }
        }
        let project = match &ask.project {
            Some(word) => ctx.resolve_project(word, origin.clone()).await?,
            None => match &asking_record {
                Some(record) => ctx.project_held(&record.project)?,
                None => return Err(HostError::with_kind(NAME_A_PROJECT_LINE, Kind::Usage)),
            },
        };
        let kind = ctx.kind_of(&project.computer);
        if !runs_in_folder(&kind) {
            return Err(HostError::with_kind(not_on_this_computer_line(&project.name), Kind::Usage));
        }
        if let Some(picks) = &ask.picks {
            ctx.picks_hold(&project, picks).await?;
        }
        let beside = asking.filter(|entry| {
            let record = record_of(entry);
            runs_in_folder(&record.kind) && record.project == project.id
        });
        if ask.branch.is_some() && ask.cwd.is_some() {
            return Err(HostError::with_kind(BRANCH_OR_CWD_LINE, Kind::Usage));
        }
        if !copies_folder(&kind) {
            if ask.branch.is_some() {
                return Err(HostError::with_kind(place_branch_line(&ctx.place_name(&project.computer)), Kind::Usage));
            }
            let cwd = ask.cwd.as_deref().map(normalize);
            if let Some(cwd) = &cwd {
                if !cwd.starts_with('/') || !under(cwd, &project.path) {
                    return Err(HostError::with_kind(cwd_outside_line(cwd, &project.name), Kind::Usage));
                }
            }
            let entry = match beside.filter(|entry| record_of(entry).worktree.is_none()) {
                Some(entry) => entry,
                None => self.project_folder(&project).await?,
            };
            return Ok(FolderPlaced { entry, cwd, setup: None });
        }
        let top = project.git.as_ref().map(|git| git.top.clone());
        if let Some(raw) = &ask.cwd {
            let home = ctx.home_dir();
            let cwd = folder_named(raw, &home, &process_cwd());
            if under(&cwd, &project.path) {
                let entry = match beside.filter(|entry| record_of(entry).worktree.is_none()) {
                    Some(entry) => entry,
                    None => self.project_folder(&project).await?,
                };
                return Ok(FolderPlaced { entry, cwd: Some(cwd), setup: None });
            }
            let outside = HostError::with_kind(cwd_outside_line(&home_shortened(&cwd, Some(home.as_str())), &project.name), Kind::Usage);
            let Some(top_path) = top else {
                return Err(outside);
            };
            let Some(held) = self.worktrees_of(&top_path).await?.into_iter().find(|tree| under(&cwd, &tree.path)) else {
                return Err(outside);
            };
            if held.path == top_path {
                let entry = self.project_folder(&project).await?;
                return Ok(FolderPlaced { entry, cwd: Some(cwd), setup: None });
            }
            let tree = WorktreeFolder {
                path: held.path,
                branch: held.branch,
                made: false,
                made_for: scope_root,
                settled: None,
                kept: None,
                remove_failed: None,
                gone: None,
            };
            let entry = self.worktree_record_at(&project, tree, None).await?;
            return Ok(FolderPlaced { entry, cwd: Some(cwd), setup: None });
        }
        let Some(branch) = ask.branch.clone() else {
            let entry = match beside {
                Some(entry) => entry,
                None => self.project_folder(&project).await?,
            };
            return Ok(FolderPlaced { entry, cwd: None, setup: None });
        };
        let Some(top_path) = top else {
            return Err(HostError::with_kind(no_branches_line(&project.name), Kind::Usage));
        };
        let checkout = match &beside {
            Some(entry) => ctx.checkout_of(&record_of(entry)),
            None => project.path.clone(),
        };
        if ctx.branch_at(&checkout).await?.as_deref() == Some(branch.as_str()) {
            let entry = match beside {
                Some(entry) => entry,
                None => self.project_folder(&project).await?,
            };
            return Ok(FolderPlaced { entry, cwd: None, setup: None });
        }
        let parent = beside.as_ref().map(|entry| record_of(entry).id);
        let entry = self.worktree_folder(&project, &top_path, &branch, parent, scope_root, None).await?;
        Ok(FolderPlaced { entry, cwd: None, setup: None })
    }
}
