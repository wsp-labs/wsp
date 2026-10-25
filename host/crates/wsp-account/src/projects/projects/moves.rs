// SPDX-License-Identifier: AGPL-3.0-only
//! A folder of this computer's onto a workspace's machine and a machine's folder back here, each saying its stages.
//! projects.ts:23-106, 482-574 at 65f6b89c7; spec/projects_move.md. The engine's roads these call are reached through
//! `MoveRoads`, so a test scripts them as the TS tests mock the engine.
use std::rc::Rc;

use async_trait::async_trait;
use wsp_core::types::internal::{ImportLanded, ImportReport};
use wsp_core::types::wiring::{LandRequest, LandedProject, PackedState, ProjectExportOptions, ProjectImportOptions};
use wsp_wire::views::project_bundle::{ProjectAgentResult, ProjectExportResult, ProjectImportResult};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::Projects;
use crate::projects::workspaces::seam::Entry;

/// A part of an upload or a download: `{bytes, total}`, with the part number where the road counts parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub part: u64,
    pub parts: u64,
    pub bytes: u64,
    pub total: u64,
}

/// What a landing of a bundle says as it goes.
pub struct LandHooks {
    pub on_part: Rc<dyn Fn(Progress)>,
    pub on_landing: Option<Rc<dyn Fn()>>,
}

/// The engine roads the moves call, on the entry's machine. Each is the TS engine function of the same name.
#[async_trait(?Send)]
pub trait MoveRoads {
    /// `agentsOnMachine(machine, ids)`: the ids whose agent is installed there.
    async fn agents_on_machine(&self, entry: &Entry, agents: &[String]) -> Result<Vec<String>, HostError>;
    /// `guestAgentHomes()`.
    fn guest_agent_homes(&self) -> indexmap::IndexMap<String, String>;
    /// `moduleOf(kind).scratch(entry)`.
    fn scratch(&self, entry: &Entry) -> String;
    /// `landBundle(machine, tar, dest, {replace?, tmpDir, timeoutMs, onPart, onLanding})`: the parts it took.
    async fn land_bundle(&self, entry: &Entry, tar: Vec<u8>, dest: &str, replace: Option<bool>, hooks: LandHooks) -> Result<u64, HostError>;
    /// `importInto(machine, tar, "/", {overlay: true, tmpDir, timeoutMs, onPart})`.
    async fn import_into(&self, entry: &Entry, tar: Vec<u8>, hooks: LandHooks) -> Result<(), HostError>;
    /// `mergeOnMachine(machine, script, agent)`.
    async fn merge_on_machine(&self, entry: &Entry, script: &str, agent: ProjectAgentResult) -> Result<ProjectAgentResult, HostError>;
    /// `ctx.projectOf(dest, bytes)`.
    fn project_of(&self, dest: &str, bytes: u64) -> Option<serde_json::Value>;
    /// `ctx.computerRows()` row of the record's place or provider is a provider place.
    async fn provider_row(&self, entry: &Entry) -> Result<bool, HostError>;
    /// `exportFolder(machine, source, caches, archive, {timeoutMs, onProgress})`: what it left out.
    async fn export_folder(&self, entry: &Entry, source: &str, caches: &[String], archive: &str, on_progress: Rc<dyn Fn(Progress)>) -> Result<Vec<String>, HostError>;
    /// `exportPathsInto(machine, groups, archive, {timeoutMs, onProgress})`.
    async fn export_paths_into(&self, entry: &Entry, groups: Vec<(String, Vec<String>)>, archive: &str, on_progress: Rc<dyn Fn(Progress)>) -> Result<(), HostError>;
    /// `machine.run(script, {deadlineMs})`.
    async fn machine_run(&self, entry: &Entry, script: &str, deadline_ms: u64) -> Result<ExecResult, HostError>;
    /// `machine.exec(cmd, {timeoutMs})`.
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError>;
    /// `moduleOf(kind).import(entry, o, report)`.
    async fn kind_import(&self, entry: &Entry, o: ProjectImportOptions, report: ImportReport) -> Result<ImportLanded, HostError>;
    /// `ctx.actionRefusal` inputs read off the record, then `ctx.copyBlocked`: done by the area itself.
    fn temp_dir(&self) -> String;
}

impl Projects {
    // ts: projects.ts:23-106
    //|   /** The import road onto a fork: the plan, what was consented, the pack, the upload in parts, the landing at the
    //|    * path and the agents' state keyed to it there. */
    //|   const copyImport = async (entry: LiveWorkspace, o: ProjectImportOptions, report: ImportReport): Promise<ImportLanded> => {
    //|     const plan = await o.bundler.plan();
    //|     report("planned", `${plural(plan.files, "file")}, ${fmtBytes(plan.bytes)}${plan.repo ? " and the repository" : ""}; ${plural(plan.secrets.length, "secret-shaped file")}; ${plural(plan.excluded.length, "cache")} left behind.`);
    //|     const carry = new Set(o.carry ?? []);
    //|     const rewrite = new Set(o.rewrite ?? []);
    //|     const rewriting = plan.secrets.flatMap(s => (s.rewrite !== undefined && rewrite.has(s.path) ? [{ path: s.path, ...s.rewrite }] : []));
    //|     const rewritten = new Set(rewriting.map(r => r.path));
    //|     const row = (await ctx.computerRows()).find(r => r.id === ctx.placeIdOf(entry.record) || r.id === entry.record.provider);
    //|     const provider = row !== undefined && isProviderPlace(row);
    //|     // A provider place gets no file of values and no key, carried or flagged or neither, and the consented line says so.
    //|     const held = seedTravels({ provider }, { kind: "secret" }) ? [] : (plan.values ?? []);
    //|     const carried = plan.secrets.filter(s => carry.has(s.path) && !rewritten.has(s.path) && !held.includes(s.path)).map(s => s.path);
    //|     const cut = [...new Set([...plan.secrets.filter(s => !carry.has(s.path) && !rewritten.has(s.path)).map(s => s.path), ...held])].sort();
    //|     const clauses = [
    //|       ...(carried.length > 0 ? [`carrying ${carried.join(", ")}`] : []),
    //|       ...(rewriting.length > 0 ? [`rewriting ${rewriting.map(r => `${r.path}${r.urls.length > 0 ? ` to ${r.urls.join(", ")}` : ""}${r.drop.length > 0 ? ` without ${r.drop.join(", ")}` : ""}`).join(", ")}`] : []),
    //|       ...(carried.length === 0 && rewriting.length === 0 ? ["no secret-shaped file travels"] : []),
    //|       cut.length === 0 ? "nothing cut" : `cut ${cut.join(", ")}`,
    //|     ].join("; ");
    //|     const named = new Set(o.agents ?? []);
    //|     const readable = plan.agents.filter(a => a.error === undefined);
    //|     const unreadable = plan.agents.filter(a => a.error !== undefined);
    //|     const travelling = readable.filter(a => named.has(a.agent));
    //|     const staying = readable.filter(a => !named.has(a.agent));
    //|     const withCount = (a: ProjectPlan["agents"][number]): string => `${a.name} (${plural(a.sessions, "session")})`;
    //|     const notes = [
    //|       ...(plan.agents.length === 0 ? [] : travelling.length === 0 ? ["No agent sessions travel"] : [`Sessions travel for ${travelling.map(withCount).join(", ")}`]),
    //|       ...(travelling.length > 0 && staying.length > 0 ? [`${staying.map(a => a.name).join(", ")} ${staying.length === 1 ? "stays" : "stay"}`] : []),
    //|       ...unreadable.map(a => `${a.name} could not be read (${a.error})`),
    //|     ];
    //|     const agentsLine = notes.length === 0 ? "" : ` ${notes.join("; ")}.`;
    //|     report("consented", `${plan.secrets.length === 0 && cut.length === 0 ? "No secret-shaped files." : `${clauses.charAt(0).toUpperCase()}${clauses.slice(1)}.`}${agentsLine}`);
    //|     report("packing", `Packing ${plural(plan.files - cut.length, "file")}.`);
    //|     const packed = await o.bundler.pack(carry, rewrite, provider);
    //|     let state: PackedState | undefined;
    //|     if (travelling.length > 0) {
    //|       const present = await agentsOnMachine(entry.machine, travelling.map(a => a.agent));
    //|       const homes = guestAgentHomes();
    //|       state = await o.bundler.packState({
    //|         dest: o.dest,
    //|         agents: travelling.map(a => {
    //|           const home = homes[a.agent];
    //|           if (home === undefined) throw new Error(`${a.agent} is not an agent the catalog knows`);
    //|           return { agent: a.agent, home, present: present.has(a.agent) };
    //|         }),
    //|       });
    //|     }
    //|     report("uploading", `Uploading ${fmtBytes(packed.tar.length)}.`, { bytes: 0, total: packed.tar.length });
    //|     const { parts } = await landBundle(entry.machine, packed.tar, o.dest, {
    //|       ...(o.replace !== undefined ? { replace: o.replace } : {}),
    //|       tmpDir: ctx.moduleOf(entry.record.kind).scratch(entry),
    //|       timeoutMs: 600_000,
    //|       onPart: p => report("uploading", `Part ${p.part} of ${p.parts}, ${fmtBytes(p.bytes)} of ${fmtBytes(p.total)}.`, { bytes: p.bytes, total: p.total }),
    //|       onLanding: () => report("landing", `Landing at ${o.dest}.`),
    //|     });
    //|     const nameOf = (id: string): string => plan.agents.find(a => a.agent === id)?.name ?? id;
    //|     const agents = [...(state?.agents ?? [])];
    //|     const outcomes = (): string => agents.map(a => `${nameOf(a.agent)} ${outcomeWords(a)}`).join(", ");
    //|     if (state !== undefined && (agents.some(a => a.files > 0) || state.merges.length > 0)) {
    //|       const files = agents.reduce((n, a) => n + a.files, 0);
    //|       const what = [...(files > 0 ? [plural(files, "session file")] : []), ...(state.merges.length > 0 ? ["the rows to merge"] : [])].join(" and ");
    //|       report("uploading", `Uploading ${what}, ${fmtBytes(state.tar.length)}.`, { bytes: 0, total: state.tar.length });
    //|       await importInto(entry.machine, state.tar, "/", {
    //|         overlay: true,
    //|         tmpDir: ctx.moduleOf(entry.record.kind).scratch(entry),
    //|         timeoutMs: 600_000,
    //|         onPart: p => report("uploading", `Part ${p.part} of ${p.parts}, ${fmtBytes(p.bytes)} of ${fmtBytes(p.total)}.`, { bytes: p.bytes, total: p.total }),
    //|       });
    //|       if (state.merges.length > 0) {
    //|         report("landing", `Merging rows into ${state.merges.map(m => nameOf(m.agent)).join(", ")}.`);
    //|         for (const m of state.merges) {
    //|           const at = agents.findIndex(a => a.agent === m.agent);
    //|           if (at >= 0) agents[at] = await mergeOnMachine(entry.machine, m.script, agents[at]!);
    //|         }
    //|       }
    //|       report("landing", `Landing sessions: ${outcomes()}.`);
    //|     }
    //|     return {
    //|       result: { dest: o.dest, files: packed.files, bytes: packed.bytes, parts, cut: packed.cut, rewritten: packed.rewritten, agents, project: ctx.projectOf(o.dest, packed.bytes) },
    //|       done: `${plural(packed.files, "file")}, ${fmtBytes(packed.bytes)}, landed at ${o.dest}${parts > 1 ? ` in ${parts} parts` : ""}${agents.length > 0 ? `; sessions: ${outcomes()}` : ""}.`,
    //|     };
    //|   };
    /// The import road onto a fork (spec copy rules 1-11).
    pub async fn copy_import(&self, entry: &Entry, o: ProjectImportOptions, report: ImportReport) -> Result<ImportLanded, HostError> {
        todo!()
    }

    // ts: projects.ts:482-517
    //|     async import(o, origin) {
    //|       ctx.spawnGuard("import", origin);
    //|       const entry = await ctx.entryOf(o.workspaceId, origin);
    //|       const refusal = actionRefusal(workspaceState({ phase: entry.record.phase }), "import", entry.record.gone, entry.record.name);
    //|       if (refusal !== null) throw new Error(refusal);
    //|       await ctx.copyBlocked(entry);
    //|       const began = clock.now();
    //|       const report: ImportReport = (stage, message, progress) => {
    //|         bus.emit({ type: "project.import", workspaceId: o.workspaceId, source: o.source, dest: o.dest, stage, message, elapsedMs: clock.now() - began, ...progress });
    //|       };
    //|       try {
    //|         const kind = ctx.moduleOf(entry.record.kind);
    //|         const landed = await kind.import(entry, o, report);
    //|         // A folder landed beside the workspace's own project is browsable too: the record keeps it beside the one
    //|         // project it names, so every later write of the roots file, a restarted host's included, lists it. Named on
    //|         // the record before the write, so a write queued behind this one lists it too, and taken back if the machine
    //|         // refuses the file: this import's folder alone, since another import may have added its own meanwhile.
    //|         const dest = landed.result.dest;
    //|         const had = entry.record.landed?.includes(dest) === true;
    //|         entry.record.landed = [...new Set([...(entry.record.landed ?? []), dest])];
    //|         try {
    //|           await ctx.writeDaemonRoots(entry, true);
    //|         } catch (e) {
    //|           const kept = had ? entry.record.landed : entry.record.landed.filter(d => d !== dest);
    //|           if (kept.length > 0) entry.record.landed = kept;
    //|           else delete entry.record.landed;
    //|           throw e;
    //|         }
    //|         await ctx.persist(entry.record);
    //|         report("done", landed.done);
    //|         return landed.result;
    //|       } catch (e) {
    //|         report("failed", e instanceof Error ? e.message : String(e));
    //|         throw e;
    //|       }
    //|     },
    pub async fn import(&self, o: ProjectImportOptions, origin: Option<Caller>) -> Result<ProjectImportResult, HostError> {
        todo!()
    }

    // ts: projects.ts:518-573
    //|     async export(o, origin) {
    //|       ctx.spawnGuard("export", origin);
    //|       const entry = await ctx.entryOf(o.workspaceId, origin);
    //|       const refusal = actionRefusal(workspaceState({ phase: entry.record.phase }), "export", entry.record.gone, entry.record.name);
    //|       if (refusal !== null) throw new Error(refusal);
    //|       await ctx.copyBlocked(entry);
    //|       const began = clock.now();
    //|       const report = (stage: ProjectExportStage, message: string, progress?: { bytes: number; total: number }): void => {
    //|         bus.emit({ type: "project.export", workspaceId: o.workspaceId, source: o.source, dest: o.dest, stage, message, elapsedMs: clock.now() - began, ...progress });
    //|       };
    //|       const downloading = (what: string) => (p: { bytes: number; total: number }): void => report("downloading", `${what}: ${fmtBytes(p.bytes)} of ${fmtBytes(p.total)}.`, p);
    //|       const scratch = mkdtempSync(join(tmpdir(), "wsp-exported-"));
    //|       // Where the listing writes the filtered copy of a store an agent keeps for every project, mirroring the homes.
    //|       const onMachine = guestTmpPath("wsp-state");
    //|       let listed = false;
    //|       try {
    //|         const homes = guestAgentHomes();
    //|         const listing = stateListing(homes, o.source, onMachine, o.agents);
    //|         const at = await o.lander.probe(o.dest);
    //|         if (at !== undefined && o.replace !== true) throw destExists(o.dest, at.files);
    //|         report("packing", `Packing ${o.source} on the machine.`);
    //|         const archive = join(scratch, "folder.tgz");
    //|         const folder = await exportFolder(entry.machine, o.source, o.lander.caches, archive, { timeoutMs: 600_000, onProgress: downloading("The folder") });
    //|         let found = { exitCode: 0, stdout: "", stderr: "" };
    //|         if (listing !== "") {
    //|           listed = true;
    //|           found = await entry.machine.run(listing, { deadlineMs: LISTING_DEADLINE_MS });
    //|         }
    //|         if (found.exitCode !== 0) throw new Error(`could not look for agent state on the machine: ${found.stderr.slice(-200)}`);
    //|         const { paths: present, unread } = parseStateListing(found.stdout);
    //|         let state: LandRequest["state"];
    //|         if (present.length > 0) {
    //|           report("packing", `Packing the agents' state for it on the machine.`);
    //|           const stateArchive = join(scratch, "state.tgz");
    //|           // A copy travels at the path it mirrors under the scratch root, so the archive is the homes as the project alone left them.
    //|           const groups = [
    //|             { root: "/", paths: present.filter(p => !underProject(p, onMachine)) },
    //|             { root: onMachine, paths: present.filter(p => underProject(p, onMachine)) },
    //|           ].filter(g => g.paths.length > 0);
    //|           await exportPathsInto(entry.machine, groups, stateArchive, { timeoutMs: 600_000, onProgress: downloading("Agent state") });
    //|           state = { archive: stateArchive, homes, ...(o.agents !== undefined ? { agents: o.agents } : {}) };
    //|         }
    //|         report("landing", `Landing at ${o.dest}.`);
    //|         const landed = await o.lander.land({ source: o.source, dest: o.dest, replace: o.replace === true, archive, ...(state !== undefined ? { state } : {}), ...(unread.length > 0 ? { unread } : {}) });
    //|         const outcomes = landed.agents.map(homeOutcome);
    //|         const caches = folder.excluded.length === 0 ? "" : `; ${plural(folder.excluded.length, "cache")} left behind`;
    //|         report("done", `${plural(landed.files, "file")}, ${fmtBytes(landed.bytes)}, landed at ${o.dest}${caches}; ${outcomes.length === 0 ? "no agent sessions for it on the machine" : `sessions: ${outcomes.join(", ")}`}.`);
    //|         return { dest: o.dest, files: landed.files, bytes: landed.bytes, excluded: folder.excluded, agents: landed.agents.map(({ name: _name, ...a }) => a) };
    //|       } catch (e) {
    //|         report("failed", e instanceof Error ? e.message : String(e));
    //|         throw e;
    //|       } finally {
    //|         if (listed) await entry.machine.exec(`rm -rf ${shellQuote(onMachine)}`, { timeoutMs: INLINE_EXEC_MS }).catch(() => {});
    //|         rmSync(scratch, { recursive: true, force: true });
    //|       }
    //|     },
    pub async fn export(&self, o: ProjectExportOptions, origin: Option<Caller>) -> Result<ProjectExportResult, HostError> {
        todo!()
    }
}

/// The unused-shape guard for the packed state the import reads (`PackedState`) and the lander's answers.
pub type MoveShapes = (PackedState, LandRequest, LandedProject);
