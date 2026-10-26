// SPDX-License-Identifier: AGPL-3.0-only
//! A folder of this computer's onto a workspace's machine and a machine's folder back here, each saying its stages.
//! projects.ts:23-106, 482-574 at 65f6b89c7; spec/projects_move.md. The engine's roads these call are reached through
//! `MoveRoads`, so a test scripts them as the TS tests mock the engine.
use std::cell::{Cell, RefCell};
use std::os::unix::fs::DirBuilderExt;
use std::rc::Rc;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_core::types::internal::{ImportLanded, ImportReport};
use wsp_core::types::pending::CacheRule;
use wsp_core::types::wiring::{
    home_outcome, outcome_words, LandRequest, LandState, LandedProject, PackedState, ProjectExportOptions, ProjectImportOptions,
    StateAgent, StateRequest, LISTING_DEADLINE_MS,
};
use wsp_engine::exec_detached::{GUEST_TMP, INLINE_EXEC_MS};
use wsp_engine::project_state::{parse_state_listing, state_listing};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::project_bundle::{
    ProjectAgent, ProjectAgentResult, ProjectExportResult, ProjectExportStage, ProjectImportResult, ProjectImportStage, ProjectRewrite,
};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::{HostError, Kind};
use wsp_words::project_path::under_project;
use wsp_words::words::base::plural;
use wsp_words::words::units::fmt_bytes;
use wsp_words::workspace_state::{action_refusal, workspace_state, WorkspaceStateInput};

use super::Projects;
use crate::projects::workspaces::seam::{record_of, Entry, EntryAsk};

/// What an export says as it goes, with the bytes read so far and the total where the road counts them.
pub type ExportReport = Rc<dyn Fn(ProjectExportStage, &str, Option<(u64, u64)>) -> Result<(), HostError>>;

/// The deadline every upload, landing and download of a move is given (projects.ts:76, 90, 540, 681).
const MOVE_DEADLINE_MS: u64 = 600_000;

/// A part of an upload or a download: `{bytes, total}`, with the part number where the road counts parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub part: u64,
    pub parts: u64,
    pub bytes: u64,
    pub total: u64,
}

/// What a landing of a bundle says as it goes, and where and for how long it works: the machine's kind names the
/// folder its parts are written in (`tmpDir`), and `timeoutMs` is 600 s.
pub struct LandHooks {
    pub on_part: Rc<dyn Fn(Progress)>,
    pub on_landing: Option<Rc<dyn Fn()>>,
    pub tmp_dir: String,
    pub timeout_ms: u64,
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
    async fn land_bundle(&self, entry: &Entry, tar: Vec<u8>, dest: &str, replace: Option<bool>, hooks: LandHooks)
        -> Result<u64, HostError>;
    /// `importInto(machine, tar, "/", {overlay: true, tmpDir, timeoutMs, onPart})`.
    async fn import_into(&self, entry: &Entry, tar: Vec<u8>, hooks: LandHooks) -> Result<(), HostError>;
    /// `mergeOnMachine(machine, script, agent)`.
    async fn merge_on_machine(&self, entry: &Entry, script: &str, agent: ProjectAgentResult) -> Result<ProjectAgentResult, HostError>;
    /// `ctx.projectOf(dest, bytes)`.
    fn project_of(&self, dest: &str, bytes: u64) -> Option<serde_json::Value>;
    /// `ctx.computerRows()` row of the record's place or provider is a provider place.
    async fn provider_row(&self, entry: &Entry) -> Result<bool, HostError>;
    /// `exportFolder(machine, source, caches, archive, {timeoutMs, onProgress})`: what it left out. The rule is the
    /// lander's own, folders, files and markers apart.
    async fn export_folder(
        &self,
        entry: &Entry,
        source: &str,
        caches: &CacheRule,
        archive: &str,
        timeout_ms: u64,
        on_progress: Rc<dyn Fn(Progress)>,
    ) -> Result<Vec<String>, HostError>;
    /// `exportPathsInto(machine, groups, archive, {timeoutMs, onProgress})`.
    async fn export_paths_into(
        &self,
        entry: &Entry,
        groups: Vec<(String, Vec<String>)>,
        archive: &str,
        timeout_ms: u64,
        on_progress: Rc<dyn Fn(Progress)>,
    ) -> Result<(), HostError>;
    /// `machine.run(script, {deadlineMs})`.
    async fn machine_run(&self, entry: &Entry, script: &str, deadline_ms: u64) -> Result<ExecResult, HostError>;
    /// `machine.exec(cmd, {timeoutMs})`.
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError>;
    /// `moduleOf(kind).import(entry, o, report)`.
    async fn kind_import(&self, entry: &Entry, o: ProjectImportOptions, report: ImportReport) -> Result<ImportLanded, HostError>;
    /// `ctx.actionRefusal` inputs read off the record, then `ctx.copyBlocked`: done by the area itself.
    fn temp_dir(&self) -> String;
}

fn json_word<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn engine_err<E: std::fmt::Display>(error: E) -> HostError {
    HostError::new(error.to_string())
}

/// `destExists(dest, files)`: the refusal an export of a standing destination without `replace` is.
fn dest_exists(dest: &str, files: u64) -> HostError {
    HostError::with_kind(
        format!("{dest} already exists on this computer with {}; export with replace to overwrite it", plural(files as f64, "file")),
        Kind::Exists,
    )
}

/// `guestTmpPath(what, dir = GUEST_TMP)`: the directory with its trailing slashes cut, then `<what>-<ms>-<six base-36 chars>`.
fn guest_tmp_path(what: &str, dir: &str, now_ms: i64, random: f64) -> String {
    let dir = dir.trim_end_matches('/');
    let mut x = random.fract().abs();
    let mut suffix = String::new();
    for _ in 0..6 {
        x *= 36.0;
        let digit = x.floor() as u32;
        suffix.push(char::from_digit(digit.min(35), 36).unwrap_or('0'));
        x -= f64::from(digit);
    }
    format!("{dir}/{what}-{now_ms}-{suffix}")
}

/// The last 200 UTF-16 units of a command's stderr, as the TS slices it.
fn tail_of(text: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    String::from_utf16_lossy(&units[units.len().saturating_sub(200)..])
}

/// The upload hooks: each part says `uploading` with its progress; `landing` names the path when one is given. The
/// parts are written in `tmp_dir`, the machine's kind's scratch folder.
fn upload_hooks(report: &ImportReport, landing: Option<String>, tmp_dir: String) -> LandHooks {
    let part_report = report.clone();
    let on_part: Rc<dyn Fn(Progress)> = Rc::new(move |p: Progress| {
        let message = format!("Part {} of {}, {} of {}.", p.part, p.parts, fmt_bytes(p.bytes as f64), fmt_bytes(p.total as f64));
        part_report(ProjectImportStage::Uploading, &message, Some((p.bytes, p.total)));
    });
    let on_landing: Option<Rc<dyn Fn()>> = landing.map(|dest| {
        let landing_report = report.clone();
        Rc::new(move || landing_report(ProjectImportStage::Landing, &format!("Landing at {dest}."), None)) as Rc<dyn Fn()>
    });
    LandHooks { on_part, on_landing, tmp_dir, timeout_ms: MOVE_DEADLINE_MS }
}

impl Projects {
    /// Refuses an import or an export the workspace's state does not allow, as the TS's `actionRefusal` reads it.
    fn refuse_action(&self, entry: &Entry, action: &str) -> Result<(), HostError> {
        let record = record_of(entry);
        let state = workspace_state(&WorkspaceStateInput { phase: record.phase, machine_state: None, reach: None, wake_refused: None });
        match action_refusal(&state, action, record.gone.as_deref(), Some(&record.name)) {
            Some(line) => Err(HostError::new(line)),
            None => Ok(()),
        }
    }

    /// An empty folder under the temp root for one export's archives, private to the person as `mkdtempSync` makes it.
    fn scratch_dir(&self) -> Result<String, HostError> {
        let path = format!("{}/wsp-exported-{}", self.ctx.temp_dir(), hex::encode(self.ctx.ids().bytes(6)));
        std::fs::DirBuilder::new().mode(0o700).create(&path).map_err(engine_err)?;
        Ok(path)
    }

    async fn export_steps(
        &self,
        entry: &Entry,
        o: &ProjectExportOptions,
        scratch: &str,
        report: &ExportReport,
        on_machine: &str,
        listed: &Cell<bool>,
    ) -> Result<ProjectExportResult, HostError> {
        let progress_failed: Rc<RefCell<Option<HostError>>> = Rc::default();
        let downloading = |what: &'static str| -> Rc<dyn Fn(Progress)> {
            let report = report.clone();
            let failed = progress_failed.clone();
            Rc::new(move |p: Progress| {
                let said = format!("{what}: {} of {}.", fmt_bytes(p.bytes as f64), fmt_bytes(p.total as f64));
                if let Err(e) = report(ProjectExportStage::Downloading, &said, Some((p.bytes, p.total))) {
                    failed.borrow_mut().get_or_insert(e);
                }
            })
        };
        let homes = self.ctx.guest_agent_homes();
        let listing = state_listing(&homes, &o.source, on_machine, o.agents.as_deref()).map_err(engine_err)?;
        if let Some(files) = o.lander.probe(&o.dest).await? {
            if o.replace != Some(true) {
                return Err(dest_exists(&o.dest, files));
            }
        }
        report(ProjectExportStage::Packing, &format!("Packing {} on the machine.", o.source), None)?;
        let archive = format!("{scratch}/folder.tgz");
        let excluded =
            self.ctx.export_folder(entry, &o.source, &o.lander.caches(), &archive, MOVE_DEADLINE_MS, downloading("The folder")).await?;
        if let Some(e) = progress_failed.borrow_mut().take() {
            return Err(e);
        }
        let mut found = ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() };
        if !listing.is_empty() {
            listed.set(true);
            found = self.ctx.machine_run(entry, &listing, LISTING_DEADLINE_MS).await?;
        }
        if found.exit_code.get() != 0 {
            return Err(HostError::new(format!("could not look for agent state on the machine: {}", tail_of(&found.stderr))));
        }
        let listed_state = parse_state_listing(&found.stdout).map_err(engine_err)?;
        let mut state: Option<LandState> = None;
        if !listed_state.paths.is_empty() {
            report(ProjectExportStage::Packing, "Packing the agents' state for it on the machine.", None)?;
            let state_archive = format!("{scratch}/state.tgz");
            let outside: Vec<String> = listed_state.paths.iter().filter(|p| !under_project(p, on_machine)).cloned().collect();
            let inside: Vec<String> = listed_state.paths.iter().filter(|p| under_project(p, on_machine)).cloned().collect();
            let mut groups: Vec<(String, Vec<String>)> = Vec::new();
            if !outside.is_empty() {
                groups.push(("/".to_owned(), outside));
            }
            if !inside.is_empty() {
                groups.push((on_machine.to_owned(), inside));
            }
            self.ctx.export_paths_into(entry, groups, &state_archive, MOVE_DEADLINE_MS, downloading("Agent state")).await?;
            if let Some(e) = progress_failed.borrow_mut().take() {
                return Err(e);
            }
            state = Some(LandState { archive: state_archive, homes: homes.clone(), agents: o.agents.clone() });
        }
        report(ProjectExportStage::Landing, &format!("Landing at {}.", o.dest), None)?;
        let landed: LandedProject = o
            .lander
            .land(LandRequest {
                source: o.source.clone(),
                dest: o.dest.clone(),
                replace: o.replace == Some(true),
                archive,
                state,
                unread: if listed_state.unread.is_empty() { None } else { Some(listed_state.unread.clone()) },
            })
            .await?;
        let outcomes: Vec<String> = landed.agents.iter().map(home_outcome).collect();
        let caches_text =
            if excluded.is_empty() { String::new() } else { format!("; {} left behind", plural(excluded.len() as f64, "cache")) };
        let sessions = if outcomes.is_empty() {
            "no agent sessions for it on the machine".to_owned()
        } else {
            format!("sessions: {}", outcomes.join(", "))
        };
        report(
            ProjectExportStage::Done,
            &format!(
                "{}, {}, landed at {}{}; {}.",
                plural(landed.files as f64, "file"),
                fmt_bytes(landed.bytes as f64),
                o.dest,
                caches_text,
                sessions
            ),
            None,
        )?;
        Ok(ProjectExportResult {
            dest: o.dest.clone(),
            files: JsNumber(landed.files as f64),
            bytes: JsNumber(landed.bytes as f64),
            excluded,
            agents: landed.agents.iter().map(|agent| agent.result.clone()).collect(),
        })
    }

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
        let plan = o.bundler.plan().await?;
        let plan_files = plan.files.0;
        let plan_bytes = plan.bytes.0;
        report(
            ProjectImportStage::Planned,
            &format!(
                "{}, {}{}; {}; {} left behind.",
                plural(plan_files, "file"),
                fmt_bytes(plan_bytes),
                if plan.repo { " and the repository" } else { "" },
                plural(plan.secrets.len() as f64, "secret-shaped file"),
                plural(plan.excluded.len() as f64, "cache"),
            ),
            None,
        );
        let carry: Vec<String> = o.carry.clone().unwrap_or_default();
        let rewrite: Vec<String> = o.rewrite.clone().unwrap_or_default();
        let rewriting: Vec<(String, ProjectRewrite)> = plan
            .secrets
            .iter()
            .filter_map(|secret| match &secret.rewrite {
                Some(words) if rewrite.contains(&secret.path) => Some((secret.path.clone(), words.clone())),
                _ => None,
            })
            .collect();
        let rewritten: Vec<String> = rewriting.iter().map(|(path, _)| path.clone()).collect();
        let provider = self.ctx.provider_row(entry).await?;
        let held: Vec<String> = if provider { plan.values.clone().unwrap_or_default() } else { Vec::new() };
        let carried: Vec<String> = plan
            .secrets
            .iter()
            .filter(|s| carry.contains(&s.path) && !rewritten.contains(&s.path) && !held.contains(&s.path))
            .map(|s| s.path.clone())
            .collect();
        let mut cut: Vec<String> =
            plan.secrets.iter().filter(|s| !carry.contains(&s.path) && !rewritten.contains(&s.path)).map(|s| s.path.clone()).collect();
        for path in &held {
            if !cut.contains(path) {
                cut.push(path.clone());
            }
        }
        cut.sort();
        let mut clauses: Vec<String> = Vec::new();
        if !carried.is_empty() {
            clauses.push(format!("carrying {}", carried.join(", ")));
        }
        if !rewriting.is_empty() {
            let parts: Vec<String> = rewriting
                .iter()
                .map(|(path, words)| {
                    let to = if words.urls.is_empty() { String::new() } else { format!(" to {}", words.urls.join(", ")) };
                    let without = if words.drop.is_empty() { String::new() } else { format!(" without {}", words.drop.join(", ")) };
                    format!("{path}{to}{without}")
                })
                .collect();
            clauses.push(format!("rewriting {}", parts.join(", ")));
        }
        if carried.is_empty() && rewriting.is_empty() {
            clauses.push("no secret-shaped file travels".to_owned());
        }
        clauses.push(if cut.is_empty() { "nothing cut".to_owned() } else { format!("cut {}", cut.join(", ")) });
        let clause_text = clauses.join("; ");
        let named: Vec<String> = o.agents.clone().unwrap_or_default();
        let readable: Vec<&ProjectAgent> = plan.agents.iter().filter(|a| a.error.is_none()).collect();
        let unreadable: Vec<&ProjectAgent> = plan.agents.iter().filter(|a| a.error.is_some()).collect();
        let travelling: Vec<&ProjectAgent> = readable.iter().copied().filter(|a| named.contains(&a.agent)).collect();
        let staying: Vec<&ProjectAgent> = readable.iter().copied().filter(|a| !named.contains(&a.agent)).collect();
        let with_count = |a: &ProjectAgent| format!("{} ({})", a.name, plural(a.sessions.0, "session"));
        let mut notes: Vec<String> = Vec::new();
        if !plan.agents.is_empty() {
            if travelling.is_empty() {
                notes.push("No agent sessions travel".to_owned());
            } else {
                notes.push(format!("Sessions travel for {}", travelling.iter().map(|a| with_count(a)).collect::<Vec<_>>().join(", ")));
            }
        }
        if !travelling.is_empty() && !staying.is_empty() {
            let names: Vec<&str> = staying.iter().map(|a| a.name.as_str()).collect();
            notes.push(format!("{} {}", names.join(", "), if staying.len() == 1 { "stays" } else { "stay" }));
        }
        for a in &unreadable {
            notes.push(format!("{} could not be read ({})", a.name, a.error.clone().unwrap_or_default()));
        }
        let agents_line = if notes.is_empty() { String::new() } else { format!(" {}.", notes.join("; ")) };
        let consented = if plan.secrets.is_empty() && cut.is_empty() {
            "No secret-shaped files.".to_owned()
        } else {
            let mut chars = clause_text.chars();
            let head = match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            };
            format!("{head}.")
        };
        report(ProjectImportStage::Consented, &format!("{consented}{agents_line}"), None);
        report(ProjectImportStage::Packing, &format!("Packing {}.", plural(plan_files - cut.len() as f64, "file")), None);
        let packed = o.bundler.pack(&carry, &rewrite, provider).await?;
        let mut state: Option<PackedState> = None;
        if !travelling.is_empty() {
            let ids: Vec<String> = travelling.iter().map(|a| a.agent.clone()).collect();
            let present = self.ctx.agents_on_machine(entry, &ids).await?;
            let homes = self.ctx.guest_agent_homes();
            let mut agents: Vec<StateAgent> = Vec::new();
            for a in &travelling {
                let Some(home) = homes.get(&a.agent) else {
                    return Err(HostError::new(format!("{} is not an agent the catalog knows", a.agent)));
                };
                agents.push(StateAgent { agent: a.agent.clone(), home: home.clone(), present: present.contains(&a.agent) });
            }
            state = Some(o.bundler.pack_state(StateRequest { dest: o.dest.clone(), agents }).await?);
        }
        let tar_len = packed.tar.len() as u64;
        report(ProjectImportStage::Uploading, &format!("Uploading {}.", fmt_bytes(tar_len as f64)), Some((0, tar_len)));
        let hooks = upload_hooks(&report, Some(o.dest.clone()), self.ctx.scratch(entry));
        let parts = self.ctx.land_bundle(entry, packed.tar.clone(), &o.dest, o.replace, hooks).await?;
        let name_of = |id: &str| plan.agents.iter().find(|a| a.agent == id).map(|a| a.name.clone()).unwrap_or_else(|| id.to_owned());
        let mut agents: Vec<ProjectAgentResult> = state.as_ref().map(|s| s.agents.clone()).unwrap_or_default();
        let outcomes = |agents: &[ProjectAgentResult]| {
            agents.iter().map(|a| format!("{} {}", name_of(&a.agent), outcome_words(a))).collect::<Vec<_>>().join(", ")
        };
        if let Some(state) = &state {
            let files: f64 = agents.iter().map(|a| a.files.0).sum();
            if files > 0.0 || !state.merges.is_empty() {
                let mut what: Vec<String> = Vec::new();
                if files > 0.0 {
                    what.push(plural(files, "session file"));
                }
                if !state.merges.is_empty() {
                    what.push("the rows to merge".to_owned());
                }
                let state_len = state.tar.len() as u64;
                report(
                    ProjectImportStage::Uploading,
                    &format!("Uploading {}, {}.", what.join(" and "), fmt_bytes(state_len as f64)),
                    Some((0, state_len)),
                );
                let hooks = upload_hooks(&report, None, self.ctx.scratch(entry));
                self.ctx.import_into(entry, state.tar.clone(), hooks).await?;
                if !state.merges.is_empty() {
                    let names: Vec<String> = state.merges.iter().map(|(agent, _)| name_of(agent)).collect();
                    report(ProjectImportStage::Landing, &format!("Merging rows into {}.", names.join(", ")), None);
                    for (agent, script) in &state.merges {
                        if let Some(at) = agents.iter().position(|a| &a.agent == agent) {
                            let current = agents[at].clone();
                            agents[at] = self.ctx.merge_on_machine(entry, script, current).await?;
                        }
                    }
                }
                report(ProjectImportStage::Landing, &format!("Landing sessions: {}.", outcomes(&agents)), None);
            }
        }
        let project = match self.ctx.project_of(&o.dest, packed.bytes) {
            Some(value) => serde_json::from_value(value).map_err(|e| HostError::new(e.to_string()))?,
            None => return Err(HostError::new(format!("{} did not land as a project", o.dest))),
        };
        let parts_text = if parts > 1 { format!(" in {parts} parts") } else { String::new() };
        let sessions_text = if agents.is_empty() { String::new() } else { format!("; sessions: {}", outcomes(&agents)) };
        let done = format!(
            "{}, {}, landed at {}{}{}.",
            plural(packed.files as f64, "file"),
            fmt_bytes(packed.bytes as f64),
            o.dest,
            parts_text,
            sessions_text
        );
        let result = ProjectImportResult {
            dest: o.dest.clone(),
            files: JsNumber(packed.files as f64),
            bytes: JsNumber(packed.bytes as f64),
            parts: JsNumber(parts as f64),
            cut: packed.cut.clone(),
            rewritten: packed.rewritten.clone(),
            agents,
            project,
        };
        Ok(ImportLanded { result, done })
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
        self.ctx.spawn_guard("import", origin.as_ref())?;
        let entry = self.ctx.entry_of(&o.workspace_id, origin.clone(), EntryAsk::default()).await?;
        self.refuse_action(&entry, "import")?;
        self.ctx.copy_blocked(&entry).await?;
        let began = self.ctx.clock().now();
        let emitter = self.ctx.clone();
        let (workspace_id, source, dest) = (o.workspace_id.clone(), o.source.clone(), o.dest.clone());
        let emit_failed: Rc<RefCell<Option<HostError>>> = Rc::default();
        let failed = emit_failed.clone();
        let report: ImportReport = Rc::new(move |stage: ProjectImportStage, message: &str, progress: Option<(u64, u64)>| {
            if failed.borrow().is_some() {
                return;
            }
            let mut event = json!({
                "type": "project.import",
                "workspaceId": workspace_id,
                "source": source,
                "dest": dest,
                "stage": json_word(&stage),
                "message": message,
                "elapsedMs": emitter.clock().now() - began,
            });
            if let Some((bytes, total)) = progress {
                event["bytes"] = json!(bytes);
                event["total"] = json!(total);
            }
            if let Err(e) = emitter.emit(event) {
                *failed.borrow_mut() = Some(e);
            }
        });
        let mut outcome = self.import_run(&entry, o, &report, &emit_failed).await;
        if let Err(e) = &outcome {
            report(ProjectImportStage::Failed, &e.message, None);
            if let Some(said) = emit_failed.borrow_mut().take() {
                outcome = Err(said);
            }
        }
        outcome
    }

    /// `emit_failed` holds a bus listener's failure: the report's own return is `()`, so the road reads it where the TS's
    /// `report(...)` would have thrown.
    async fn import_run(
        &self,
        entry: &Entry,
        o: ProjectImportOptions,
        report: &ImportReport,
        emit_failed: &RefCell<Option<HostError>>,
    ) -> Result<ProjectImportResult, HostError> {
        let landed = self.ctx.kind_import(entry, o, report.clone()).await;
        if let Some(e) = emit_failed.borrow_mut().take() {
            return Err(e);
        }
        let landed = landed?;
        let landed_dest = landed.result.dest.clone();
        let had = record_of(entry).landed.is_some_and(|dirs| dirs.contains(&landed_dest));
        {
            let held = entry.borrow();
            let mut record = held.record.borrow_mut();
            let mut dirs = record.landed.clone().unwrap_or_default();
            if !dirs.contains(&landed_dest) {
                dirs.push(landed_dest.clone());
            }
            record.landed = Some(dirs);
        }
        if let Err(e) = self.ctx.write_daemon_roots(entry, true).await {
            let kept: Vec<String> = match record_of(entry).landed {
                Some(dirs) if had => dirs,
                Some(dirs) => dirs.into_iter().filter(|d| *d != landed_dest).collect(),
                None => Vec::new(),
            };
            entry.borrow().record.borrow_mut().landed = if kept.is_empty() { None } else { Some(kept) };
            return Err(e);
        }
        self.ctx.persist(record_of(entry)).await?;
        report(ProjectImportStage::Done, &landed.done, None);
        if let Some(e) = emit_failed.borrow_mut().take() {
            return Err(e);
        }
        Ok(landed.result)
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
        self.ctx.spawn_guard("export", origin.as_ref())?;
        let entry = self.ctx.entry_of(&o.workspace_id, origin.clone(), EntryAsk::default()).await?;
        self.refuse_action(&entry, "export")?;
        self.ctx.copy_blocked(&entry).await?;
        let began = self.ctx.clock().now();
        let emitter = self.ctx.clone();
        let (workspace_id, source, dest) = (o.workspace_id.clone(), o.source.clone(), o.dest.clone());
        let report: ExportReport = Rc::new(move |stage: ProjectExportStage, message: &str, progress: Option<(u64, u64)>| {
            let mut event = json!({
                "type": "project.export",
                "workspaceId": workspace_id,
                "source": source,
                "dest": dest,
                "stage": json_word(&stage),
                "message": message,
                "elapsedMs": emitter.clock().now() - began,
            });
            if let Some((bytes, total)) = progress {
                event["bytes"] = json!(bytes);
                event["total"] = json!(total);
            }
            emitter.emit(event)
        });
        let scratch = self.scratch_dir()?;
        let on_machine = guest_tmp_path("wsp-state", GUEST_TMP, self.ctx.clock().now(), self.ctx.ids().random());
        let listed = Cell::new(false);
        let mut outcome = self.export_steps(&entry, &o, &scratch, &report, &on_machine, &listed).await;
        if let Err(e) = &outcome {
            if let Err(said) = report(ProjectExportStage::Failed, &e.message, None) {
                outcome = Err(said);
            }
        }
        if listed.get() {
            let cleanup = format!("rm -rf {}", shell_quote(&on_machine));
            let _ = self.ctx.machine_exec(&entry, &cleanup, INLINE_EXEC_MS).await;
        }
        std::fs::remove_dir_all(&scratch).ok();
        outcome
    }
}

/// The unused-shape guard for the packed state the import reads (`PackedState`) and the lander's answers.
pub type MoveShapes = (PackedState, LandRequest, LandedProject);
