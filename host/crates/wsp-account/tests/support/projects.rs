// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes the projects area's tests share on top of `support/base.rs`: `FakeProjects` implements `Base` (over
//! `BaseOf`), `MoveRoads` and `ProjectsCtx`; `FakeRoad`, `FakeSource`, `FakeSeed`, `FakeBundler` and `FakeLander` are the
//! landing road, the source module, the seed reader, the folder's bundler and this computer's lander. Every method logs
//! `self.base.call(<method name>, args)` and answers from the script (`base.answer_with(name, ..)`) or a default.
#![allow(dead_code, unused_imports)]
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_account::projects::projects::add::{
    ComputerRow, JoinedComputer, LandAsk, Landed, LandingDeps, LandingKind, LandingRoad, ProjectSourceModule, ResolveAsk, SourceResolved,
};
use wsp_account::projects::projects::moves::{LandHooks, MoveRoads, Progress};
use wsp_account::projects::projects::ProjectsCtx;
use wsp_account::projects::workspaces::seam::{Base, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::internal::{ImportLanded, ImportReport};
use wsp_core::types::pending::{CacheRule, Store};
use wsp_core::types::wiring::{
    LandRequest, LandedAgent, LandedProject, PackedProject, PackedState, ProjectBundler, ProjectImportOptions, ProjectLander, SeedPack,
    SeedWiring, StateRequest, WorkspaceRecord,
};
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow, MachineSpec,
    MachineState, RunOptions, SnapshotOptions,
};
use wsp_engine::EngineError;
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::project_bundle::{ProjectAgentResult, ProjectImportStage, ProjectPlan};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectSource, ProjectView, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ProjectBranch, WorkspaceView};
use wsp_words::exit::{HostError, Kind};

use crate::base::{kind_named, plain, rec, BaseOf, FakeBase};

pub const HOME: &str = "/home/me";

/// An `$error` answer read as the error it scripts.
pub fn error_in(v: &Value) -> Option<HostError> {
    let e = v.get("$error")?;
    let message = e.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
    let kind = e.get("kind").and_then(Value::as_str).map(kind_named);
    Some(HostError { kind, ..plain(&message) })
}

/// A failing answer for a script queue.
pub fn fails(message: &str, kind: Option<&str>) -> Value {
    crate::base::err_answer(message, kind)
}

fn scoped(origin: Option<&Caller>) -> bool {
    matches!(origin, Some(Caller::Scoped { .. }))
}

fn last_segment(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_owned()
}

fn source_json(source: &ProjectSource) -> Value {
    serde_json::to_value(source).unwrap()
}

fn kind_word(kind: LandingKind) -> &'static str {
    match kind {
        LandingKind::Mac => "mac",
        LandingKind::Box => "box",
        LandingKind::Provider => "provider",
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A machine and a backend nobody calls

pub struct StubMachine;

#[async_trait(?Send)]
impl Machine for StubMachine {
    fn id(&self) -> &str {
        "m_stub"
    }
    fn kind(&self) -> MachineKind {
        unreachable!("the area never drives a machine itself")
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult, EngineError> {
        unreachable!()
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        unreachable!()
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        unreachable!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        unreachable!()
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<(), EngineError> {
        unreachable!()
    }
    async fn kill(&self) -> Result<(), EngineError> {
        unreachable!()
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        unreachable!()
    }
    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        unreachable!()
    }
    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        unreachable!()
    }
}

/// A backend the area only passes along: any call into it is a fault in the test.
pub struct StubBackend;

#[async_trait(?Send)]
impl MachineBackend for StubBackend {
    fn capabilities(&self) -> Capabilities {
        unreachable!("the area never reads a backend's capabilities")
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        unreachable!()
    }
    async fn create(
        &self,
        _spec: MachineSpec,
        _waiting: Option<wsp_engine::machine::Waiting>,
        _sent: Option<String>,
    ) -> Result<Arc<dyn Machine>, EngineError> {
        unreachable!()
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        unreachable!()
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>, EngineError> {
        unreachable!()
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<(), EngineError> {
        unreachable!()
    }
}

pub fn joined(home: &str) -> JoinedComputer {
    JoinedComputer { machine: Arc::new(StubMachine), home: home.to_owned() }
}

// ---------------------------------------------------------------------------------------------------------------
// The landing road

/// One kind's landing road. Reads answer from the script under `road_<method>` (the kind is in every logged call);
/// a road method that may be absent (`folder_stands`, `seed_into`, `unsaved`) is absent until its answer is scripted.
pub struct FakeRoad {
    pub kind: LandingKind,
    pub base: Rc<FakeBase>,
}

impl FakeRoad {
    fn key(&self, method: &str) -> String {
        format!("road_{method}:{}", kind_word(self.kind))
    }
    /// The answer scripted for this kind alone, else for the method on every kind.
    fn scripted(&self, method: &str) -> Option<Value> {
        self.base.answer(&self.key(method)).or_else(|| self.base.answer(&format!("road_{method}")))
    }
    fn deps_json(deps: &LandingDeps) -> Value {
        json!({
            "computerName": deps.computer_name,
            "macStateHome": deps.mac_state_home,
            "joinedHome": deps.computer.as_ref().map(|c| c.home.clone()),
            "cloneEnv": deps.clone_env,
        })
    }
}

#[async_trait(?Send)]
impl LandingRoad for FakeRoad {
    fn kind(&self) -> LandingKind {
        self.kind
    }
    fn path(&self, name: &str, source: &ProjectSource, deps: &LandingDeps) -> String {
        self.base.call(
            "road_path",
            json!({ "kind": kind_word(self.kind), "name": name, "source": source_json(source), "deps": Self::deps_json(deps) }),
        );
        if let Some(v) = self.scripted("path") {
            return v.as_str().unwrap().to_owned();
        }
        match (self.kind, source) {
            (_, ProjectSource::Folder(f)) if self.kind == LandingKind::Mac => f.path.clone(),
            (LandingKind::Box, _) => {
                format!("{}/{name}", deps.computer.as_ref().map(|c| c.home.clone()).unwrap_or_else(|| "/root".to_owned()))
            }
            _ => format!("/root/{name}"),
        }
    }
    fn memory_dir(&self, project: &ProjectView, memory_key: &str, deps: &LandingDeps) -> String {
        self.base.call(
            "road_memory_dir",
            json!({ "kind": kind_word(self.kind), "project": project.id, "name": project.name, "path": project.path, "memoryKey": memory_key, "deps": Self::deps_json(deps) }),
        );
        match self.scripted("memory_dir") {
            Some(v) => v.as_str().unwrap().to_owned(),
            None => format!("{HOME}/.claude/projects/{memory_key}/memory"),
        }
    }
    fn lands_at_add(&self, seeding: bool) -> bool {
        self.base.call("road_lands_at_add", json!({ "kind": kind_word(self.kind), "seeding": seeding }));
        match self.scripted("lands_at_add") {
            Some(v) => v.as_bool().unwrap(),
            None => match self.kind {
                LandingKind::Mac => false,
                LandingKind::Box => true,
                LandingKind::Provider => seeding,
            },
        }
    }
    async fn land(&self, ask: LandAsk, deps: &LandingDeps) -> Result<Landed, HostError> {
        let seed = ask.seed.as_ref().map(|(tar, choice, plan)| {
            json!({ "tar": String::from_utf8_lossy(tar), "choice": serde_json::to_value(choice).unwrap(), "plan": serde_json::to_value(plan).unwrap() })
        });
        self.base.call(
            "road_land",
            json!({ "kind": kind_word(self.kind), "project": serde_json::to_value(&ask.project).unwrap(), "seed": seed, "deps": Self::deps_json(deps) }),
        );
        let said = self.base.answer("road_land_reports").unwrap_or(json!([]));
        for line in said.as_array().unwrap() {
            let stage = serde_json::from_value(line[0].clone()).expect("a stage reads");
            if let Some(ms) = line.get(2).and_then(Value::as_i64) {
                self.base.clock.advance(ms);
            }
            (ask.report)(stage, line[1].as_str().unwrap());
        }
        match self.scripted("land") {
            None => Ok(Landed::default()),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(Landed {
                    fields: v.get("fields").and_then(Value::as_object).cloned().unwrap_or_default(),
                    notice: v.get("notice").and_then(Value::as_str).map(str::to_owned),
                }),
            },
        }
    }
    async fn remove(&self, project: &ProjectView, deps: &LandingDeps) -> Result<String, HostError> {
        self.base.call(
            "road_remove",
            json!({
                "kind": kind_word(self.kind), "project": project.id, "deps": Self::deps_json(deps),
                "held": self.base.projects.borrow().contains_key(&project.id),
                "writes": self.base.store.writes.borrow().clone(),
            }),
        );
        match self.scripted("remove") {
            None => Ok(format!("{} removed", project.name)),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(v.as_str().unwrap().to_owned()),
            },
        }
    }
    async fn folder_stands(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Result<bool, HostError>> {
        self.base.call("road_folder_stands", json!({ "kind": kind_word(self.kind), "project": project.id, "deps": Self::deps_json(deps) }));
        let v = self.scripted("folder_stands")?;
        Some(match error_in(&v) {
            Some(e) => Err(e),
            None => Ok(v.as_bool().unwrap()),
        })
    }
    fn takes_files(&self) -> bool {
        self.base.has_answer(&self.key("seed_into")) || self.base.has_answer("road_seed_into")
    }
    async fn seed_into(&self, project: &ProjectView, tar: Vec<u8>, deps: &LandingDeps) -> Option<Result<(), HostError>> {
        self.base.call(
            "road_seed_into",
            json!({ "kind": kind_word(self.kind), "project": project.id, "tar": String::from_utf8_lossy(&tar), "deps": Self::deps_json(deps) }),
        );
        let v = self.scripted("seed_into")?;
        Some(match error_in(&v) {
            Some(e) => Err(e),
            None => Ok(()),
        })
    }
    async fn unsaved(&self, project: &ProjectView, deps: &LandingDeps) -> Option<Result<Option<String>, HostError>> {
        self.base.call("road_unsaved", json!({ "kind": kind_word(self.kind), "project": project.id, "deps": Self::deps_json(deps) }));
        let v = self.scripted("unsaved")?;
        Some(match error_in(&v) {
            Some(e) => Err(e),
            None => Ok(v.as_str().map(str::to_owned)),
        })
    }
    async fn branch(&self, project: &ProjectView, deps: &LandingDeps) -> Result<ProjectBranch, HostError> {
        self.base.call("road_branch", json!({ "kind": kind_word(self.kind), "project": project.id, "deps": Self::deps_json(deps) }));
        match self.scripted("branch") {
            None => Ok(ProjectBranch { branch: Some(project.default_branch.clone()), folder: false }),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(serde_json::from_value(v).expect("a scripted branch reads")),
            },
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The source module

/// `projectSource(kind)`: scripted under `source_resolve` as `{name, remote, defaultBranch, seed?, seedMemoryKey?}`, else
/// read as project-sources.ts reads a source (a folder asks `folder_remote`, or the menu when seeding).
pub struct FakeSource {
    pub base: Rc<FakeBase>,
}

#[async_trait(?Send)]
impl ProjectSourceModule for FakeSource {
    async fn resolve(&self, source: &ProjectSource, ask: ResolveAsk) -> Result<SourceResolved, HostError> {
        self.base.call("source_resolve", json!({ "source": source_json(source), "seeding": ask.seeding }));
        if let Some(v) = self.base.answer("source_resolve") {
            if let Some(e) = error_in(&v) {
                return Err(e);
            }
            return Ok(SourceResolved {
                name: v["name"].as_str().unwrap().to_owned(),
                remote: v["remote"].as_str().unwrap().to_owned(),
                default_branch: v["defaultBranch"].as_str().unwrap().to_owned(),
                seed: v.get("seed").filter(|s| !s.is_null()).map(|s| serde_json::from_value(s.clone()).unwrap()),
                seed_memory_key: v.get("seedMemoryKey").and_then(Value::as_str).map(str::to_owned),
            });
        }
        match source {
            ProjectSource::Folder(f) if ask.seeding => {
                let plan = (ask.seed_plan)(f.path.clone()).await?;
                Ok(SourceResolved {
                    name: last_segment(&f.path),
                    remote: plan.remote.clone().unwrap_or_default(),
                    default_branch: plan.default_branch.clone().unwrap_or_else(|| plan.branch.clone()),
                    seed_memory_key: plan.memory.as_ref().map(|m| m.key.clone()),
                    seed: Some(plan),
                })
            }
            ProjectSource::Folder(f) => {
                let remote = (ask.folder_remote)(f.path.clone()).await?;
                Ok(SourceResolved {
                    name: last_segment(&f.path),
                    remote,
                    default_branch: "main".to_owned(),
                    seed: None,
                    seed_memory_key: None,
                })
            }
            ProjectSource::Git(g) => Ok(SourceResolved {
                name: last_segment(g.url.trim_end_matches(".git")),
                remote: g.url.clone(),
                default_branch: "main".to_owned(),
                seed: None,
                seed_memory_key: None,
            }),
            ProjectSource::Github(g) => Ok(SourceResolved {
                name: last_segment(&g.repo),
                remote: format!("https://github.com/{}.git", g.repo),
                default_branch: "main".to_owned(),
                seed: None,
                seed_memory_key: None,
            }),
            ProjectSource::Gitlab(g) => Ok(SourceResolved {
                name: last_segment(&g.repo),
                remote: format!("https://gitlab.com/{}.git", g.repo),
                default_branch: "main".to_owned(),
                seed: None,
                seed_memory_key: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The seed reader

/// `opts.seed`: `plan` answers the scripted `seed_wiring_plan` (a `SeedPlan` as JSON) or `menu(folder)`; `pack` answers
/// the scripted `seed_wiring_pack` (`{tar, files, bytes, commits, left}`) or a one-file pack of the choice.
pub struct FakeSeed {
    pub base: Rc<FakeBase>,
}

#[async_trait(?Send)]
impl SeedWiring for FakeSeed {
    async fn plan(&self, folder: &str, homes: &IndexMap<String, String>) -> Result<SeedPlan, HostError> {
        self.base.call("seed_wiring_plan", json!({ "folder": folder, "homes": homes }));
        match self.base.answer("seed_wiring_plan") {
            None => Ok(menu(folder)),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(serde_json::from_value(v).expect("a scripted menu reads")),
            },
        }
    }
    async fn pack(&self, plan: SeedPlan, choice: SeedChoice, homes: &IndexMap<String, String>) -> Result<SeedPack, HostError> {
        self.base.call(
            "seed_wiring_pack",
            json!({ "plan": serde_json::to_value(&plan).unwrap(), "choice": serde_json::to_value(&choice).unwrap(), "homes": homes }),
        );
        match self.base.answer("seed_wiring_pack") {
            None => Ok(SeedPack { tar: b"seed".to_vec(), files: choice.files.len() as u64, bytes: 1, commits: 0, left: vec![] }),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(SeedPack {
                    tar: v["tar"].as_str().unwrap().as_bytes().to_vec(),
                    files: v["files"].as_u64().unwrap(),
                    bytes: v["bytes"].as_u64().unwrap(),
                    commits: v["commits"].as_u64().unwrap(),
                    left: v["left"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect(),
                }),
            },
        }
    }
}

/// The menu the host's reader would answer for a folder: the catalogue's own ticks on, nothing remembered
/// (project-seed-picks.test.ts:43).
pub fn menu(folder: &str) -> SeedPlan {
    serde_json::from_value(json!({
        "source": folder,
        "remote": "https://github.com/acme/lab.git",
        "branch": "main",
        "defaultBranch": "main",
        "unpushed": null,
        "uncommitted": 0,
        "memory": null,
        "files": [
            { "path": ".env", "dir": false, "bytes": 1126, "kind": "config", "row": { "id": "dotenv", "name": "environment file" }, "ticked": true },
            { "path": ".env.local", "dir": false, "bytes": 132, "kind": "config", "row": { "id": "dotenv", "name": "environment file" }, "ticked": true },
            { "path": ".git-credentials", "dir": false, "bytes": 300, "kind": "never", "row": { "id": "logins", "name": "logins" }, "ticked": false },
            { "path": "node_modules", "dir": true, "bytes": 831520768, "kind": "rebuilt", "row": { "id": "node", "name": "Node" }, "ticked": false },
            { "path": "deploy.pem", "dir": false, "bytes": 4096, "kind": "secret", "row": { "id": "certificates", "name": "certificates" }, "ticked": true }
        ],
        "remembered": false
    }))
    .unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// The folder's bundler and this computer's lander (the move tests)

/// What the host would hand the runtime for a folder: `plan` is the plan as JSON, `pack` replays project-import.test.ts's
/// fakeBundler (the cut set is the secrets not carried or rewritten, with every file of values for a provider), and
/// `pack_state` answers the scripted `PackedState` in `state`.
pub struct FakeBundler {
    pub base: Rc<FakeBase>,
    pub plan: ProjectPlan,
    pub values: Vec<String>,
    pub tar_len: usize,
    pub state: RefCell<Option<PackedState>>,
}

impl FakeBundler {
    pub fn new(base: &Rc<FakeBase>, plan: Value) -> Rc<FakeBundler> {
        let plan: ProjectPlan = serde_json::from_value(plan).expect("a test plan reads");
        let values = plan.values.clone().unwrap_or_default();
        Rc::new(FakeBundler { base: base.clone(), plan, values, tar_len: 2048, state: RefCell::new(None) })
    }
}

#[async_trait(?Send)]
impl ProjectBundler for FakeBundler {
    async fn plan(&self) -> Result<ProjectPlan, HostError> {
        self.base.call("bundler_plan", json!(null));
        match self.base.answer("bundler_plan").and_then(|v| error_in(&v)) {
            Some(e) => Err(e),
            None => Ok(self.plan.clone()),
        }
    }
    async fn pack(&self, carry: &[String], rewrite: &[String], provider: bool) -> Result<PackedProject, HostError> {
        self.base.call("bundler_pack", json!({ "carry": carry, "rewrite": rewrite, "provider": provider }));
        if let Some(e) = self.base.answer("bundler_pack").and_then(|v| error_in(&v)) {
            return Err(e);
        }
        let rewritten: Vec<String> =
            self.plan.secrets.iter().filter(|s| s.rewrite.is_some() && rewrite.contains(&s.path)).map(|s| s.path.clone()).collect();
        let held: Vec<String> = if provider { self.values.clone() } else { vec![] };
        let mut cut: Vec<String> = self
            .plan
            .secrets
            .iter()
            .map(|s| s.path.clone())
            .filter(|p| (!carry.contains(p) && !rewritten.contains(p)) || held.contains(p))
            .chain(held.iter().cloned())
            .collect();
        cut.sort();
        cut.dedup();
        Ok(PackedProject { tar: vec![7u8; self.tar_len], files: self.plan.files.0 as u64 - cut.len() as u64, bytes: 4000, cut, rewritten })
    }
    async fn pack_state(&self, req: StateRequest) -> Result<PackedState, HostError> {
        self.base.call(
            "bundler_pack_state",
            json!({ "dest": req.dest, "agents": req.agents.iter().map(|a| json!({ "agent": a.agent, "home": a.home, "present": a.present })).collect::<Vec<_>>() }),
        );
        match self.base.answer("bundler_pack_state").and_then(|v| error_in(&v)) {
            Some(e) => Err(e),
            None => Ok(self.state.borrow().clone().expect("the test scripted the packed state")),
        }
    }
}

/// This computer's side of an export. `probe` answers the scripted `lander_probe` (a file count) or none; `land` reads
/// the archives off disk as they stand when it is asked, and answers `files`, `bytes` and `agents`.
pub struct FakeLander {
    pub base: Rc<FakeBase>,
    pub agents: RefCell<Vec<LandedAgent>>,
    pub files: Cell<u64>,
    pub bytes: Cell<u64>,
    /// (archive path, archive text, state archive text) as they stood at each landing.
    pub seen: RefCell<Vec<(String, String, Option<String>)>>,
}

impl FakeLander {
    pub fn new(base: &Rc<FakeBase>) -> Rc<FakeLander> {
        Rc::new(FakeLander {
            base: base.clone(),
            agents: RefCell::new(vec![]),
            files: Cell::new(3),
            bytes: Cell::new(4000),
            seen: RefCell::new(vec![]),
        })
    }
}

#[async_trait(?Send)]
impl ProjectLander for FakeLander {
    fn caches(&self) -> CacheRule {
        CacheRule::default()
    }
    async fn probe(&self, dest: &str) -> Result<Option<u64>, HostError> {
        self.base.call("lander_probe", json!({ "dest": dest }));
        match self.base.answer("lander_probe") {
            None => Ok(None),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(v.as_u64()),
            },
        }
    }
    async fn land(&self, req: LandRequest) -> Result<LandedProject, HostError> {
        let archive = std::fs::read_to_string(&req.archive).unwrap_or_default();
        let state = req.state.as_ref().map(|s| std::fs::read_to_string(&s.archive).unwrap_or_default());
        self.seen.borrow_mut().push((req.archive.clone(), archive, state));
        self.base.call(
            "lander_land",
            json!({
                "source": req.source, "dest": req.dest, "replace": req.replace, "archive": req.archive,
                "state": req.state.as_ref().map(|s| json!({ "archive": s.archive, "homes": s.homes, "agents": s.agents })),
                "unread": req.unread.as_ref().map(|u| u.iter().map(|x| json!({ "agent": x.agent, "store": x.store, "why": x.why })).collect::<Vec<_>>()),
            }),
        );
        if let Some(e) = self.base.answer("lander_land").and_then(|v| error_in(&v)) {
            return Err(e);
        }
        Ok(LandedProject { files: self.files.get(), bytes: self.bytes.get(), agents: self.agents.borrow().clone() })
    }
}

/// A landed agent row: the wire result and the catalogue's name.
pub fn landed_agent(name: &str, result: Value) -> LandedAgent {
    LandedAgent { result: serde_json::from_value(result).expect("a test agent result reads"), name: name.to_owned() }
}

/// An agent result as the wire reads it.
pub fn agent_result(v: Value) -> ProjectAgentResult {
    serde_json::from_value(v).expect("a test agent result reads")
}

// ---------------------------------------------------------------------------------------------------------------
// The context

pub struct FakeProjects {
    pub base: Rc<FakeBase>,
    pub rows: RefCell<Vec<ComputerRow>>,
    pub seed: RefCell<Option<Rc<FakeSeed>>>,
    pub mac: Rc<FakeRoad>,
    pub boxed: Rc<FakeRoad>,
    pub provider: Rc<FakeRoad>,
    pub source: Rc<FakeSource>,
    pub cloud_projects: Cell<Option<bool>>,
    pub vault_token: RefCell<Option<String>>,
    pub claude_home: RefCell<Option<String>>,
    pub state_path: Cell<bool>,
    pub copier: Cell<bool>,
    pub joined: RefCell<IndexMap<String, JoinedComputer>>,
    pub vault_places: RefCell<Vec<String>>,
    /// The project ids a thread's scope may start children on.
    pub reached: RefCell<Vec<String>>,
    /// The workspace ids a thread names (`holdsThread`).
    pub holding: RefCell<Vec<String>>,
    /// The agents an adapter is wired for.
    pub runs: RefCell<Vec<String>>,
    /// What each move road was told beside its arguments: the folder its parts are written in and its deadline.
    pub roads: RefCell<Vec<Value>>,
    pub tmp: tempfile::TempDir,
    /// The failures the context's lookups answer in place of their values (each throws where the TS does).
    pub state_fail: RefCell<Option<HostError>>,
    pub reached_fail: RefCell<Option<HostError>>,
    pub elsewhere_fail: RefCell<Option<HostError>>,
    pub threads_fail: RefCell<Option<HostError>>,
    pub scratch_fail: RefCell<Option<HostError>>,
}

/// The rows every test starts with: this computer, a joined box, and a provider named by its id.
pub fn rows() -> Vec<ComputerRow> {
    vec![
        ComputerRow { id: "here".to_owned(), name: "Mac".to_owned(), kind: PlaceKind::Computer },
        ComputerRow { id: "pl_box".to_owned(), name: "box".to_owned(), kind: PlaceKind::Computer },
        ComputerRow { id: "solari".to_owned(), name: "solari".to_owned(), kind: PlaceKind::Provider },
    ]
}

impl FakeProjects {
    pub fn new() -> Rc<FakeProjects> {
        Self::over(FakeBase::new())
    }

    pub fn over(base: Rc<FakeBase>) -> Rc<FakeProjects> {
        base.kinds.borrow_mut().insert("solari".to_owned(), WorkspaceKind::Cloud);
        *base.prefs.borrow_mut() = Some(prefs());
        Rc::new(FakeProjects {
            rows: RefCell::new(rows()),
            seed: RefCell::new(Some(Rc::new(FakeSeed { base: base.clone() }))),
            mac: Rc::new(FakeRoad { kind: LandingKind::Mac, base: base.clone() }),
            boxed: Rc::new(FakeRoad { kind: LandingKind::Box, base: base.clone() }),
            provider: Rc::new(FakeRoad { kind: LandingKind::Provider, base: base.clone() }),
            source: Rc::new(FakeSource { base: base.clone() }),
            cloud_projects: Cell::new(None),
            vault_token: RefCell::new(None),
            claude_home: RefCell::new(Some(format!("{HOME}/.claude"))),
            state_path: Cell::new(true),
            copier: Cell::new(false),
            joined: RefCell::new(IndexMap::new()),
            vault_places: RefCell::new(vec![]),
            reached: RefCell::new(vec![]),
            holding: RefCell::new(vec![]),
            runs: RefCell::new(vec!["claude".to_owned()]),
            roads: RefCell::new(Vec::new()),
            tmp: tempfile::tempdir().expect("a scratch folder"),
            state_fail: RefCell::new(None),
            reached_fail: RefCell::new(None),
            elsewhere_fail: RefCell::new(None),
            threads_fail: RefCell::new(None),
            scratch_fail: RefCell::new(None),
            base,
        })
    }

    pub fn area(self: &Rc<Self>) -> Rc<wsp_account::projects::projects::Projects> {
        wsp_account::projects::projects::projects_area(self.clone())
    }

    fn road_of(&self, kind: LandingKind) -> Rc<FakeRoad> {
        match kind {
            LandingKind::Mac => self.mac.clone(),
            LandingKind::Box => self.boxed.clone(),
            LandingKind::Provider => self.provider.clone(),
        }
    }

    fn me(&self) -> BaseOf {
        BaseOf(self.base.clone())
    }
}

#[async_trait(?Send)]
impl Base for FakeProjects {
    async fn ready(&self) -> Result<(), HostError> {
        self.me().ready().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.me().clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.me().ids()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.me().store()
    }
    fn live(&self) -> Live {
        self.me().live()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.me().projects_held()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.me().project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.me().session_views()
    }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        self.me().thread_records()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.me().persist(record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.me().view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.me().emit(event)
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.me().entry_of(id, origin, ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.me().spawn_guard(act, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.me().kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        self.me().place_name(place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.me().copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        self.me().with_daemon(entry, work).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        self.me().on_this_computer(work).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        self.me().git_here(ask).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.me().checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.me().preferences().await
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        let r = rec(entry);
        self.base.call("write_daemon_roots", json!({ "id": r.id, "strict": strict, "landed": serde_json::to_value(&r.landed).unwrap() }));
        // Another import landing its own folder while this write is out.
        if let Some(v) = self.base.answer("write_daemon_roots_meanwhile") {
            let record = entry.borrow().record.clone();
            record.borrow_mut().landed.get_or_insert_with(Vec::new).push(v.as_str().unwrap().to_owned());
        }
        self.base.answer_as("write_daemon_roots", || ())
    }
    fn home_dir(&self) -> String {
        self.me().home_dir()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        self.me().machine_id(entry)
    }
}

#[async_trait(?Send)]
impl MoveRoads for FakeProjects {
    async fn agents_on_machine(&self, entry: &Entry, agents: &[String]) -> Result<Vec<String>, HostError> {
        self.base.call("agents_on_machine", json!({ "id": rec(entry).id, "agents": agents }));
        self.base.answer_as("agents_on_machine", || agents.to_vec())
    }
    fn guest_agent_homes(&self) -> IndexMap<String, String> {
        self.base.call("guest_agent_homes", json!(null));
        self.base
            .answer_as("guest_agent_homes", || {
                [("claude", "/root/.claude-cfg"), ("codex", "/root/.codex"), ("pi", "/root/.pi/agent"), ("hermes", "/root/.hermes")]
                    .into_iter()
                    .map(|(a, h)| (a.to_owned(), h.to_owned()))
                    .collect()
            })
            .unwrap()
    }
    fn scratch(&self, entry: &Entry) -> Result<String, HostError> {
        self.base.call("scratch", json!({ "id": rec(entry).id }));
        if let Some(e) = self.scratch_fail.borrow().clone() {
            return Err(e);
        }
        Ok("/tmp".to_owned())
    }
    async fn land_bundle(
        &self,
        entry: &Entry,
        tar: Vec<u8>,
        dest: &str,
        replace: Option<bool>,
        hooks: LandHooks,
    ) -> Result<u64, HostError> {
        self.base.call("land_bundle", json!({ "id": rec(entry).id, "bytes": tar.len(), "dest": dest, "replace": replace }));
        self.roads.borrow_mut().push(json!({ "road": "land_bundle", "tmpDir": hooks.tmp_dir, "timeoutMs": hooks.timeout_ms }));
        let parts: Vec<Progress> = match self.base.answer("land_bundle_parts") {
            Some(v) => v
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Progress {
                    part: p[0].as_u64().unwrap(),
                    parts: p[1].as_u64().unwrap(),
                    bytes: p[2].as_u64().unwrap(),
                    total: p[3].as_u64().unwrap(),
                })
                .collect(),
            None => vec![Progress { part: 1, parts: 1, bytes: tar.len() as u64, total: tar.len() as u64 }],
        };
        if let Some(e) = self.base.answer("land_bundle").and_then(|v| error_in(&v)) {
            return Err(e);
        }
        for p in &parts {
            (hooks.on_part)(*p);
        }
        if let Some(landing) = &hooks.on_landing {
            landing();
        }
        Ok(parts.len() as u64)
    }
    async fn import_into(&self, entry: &Entry, tar: Vec<u8>, hooks: LandHooks) -> Result<(), HostError> {
        self.base.call("import_into", json!({ "id": rec(entry).id, "bytes": tar.len() }));
        self.roads.borrow_mut().push(json!({ "road": "import_into", "tmpDir": hooks.tmp_dir, "timeoutMs": hooks.timeout_ms }));
        if let Some(e) = self.base.answer("import_into").and_then(|v| error_in(&v)) {
            return Err(e);
        }
        (hooks.on_part)(Progress { part: 1, parts: 1, bytes: tar.len() as u64, total: tar.len() as u64 });
        Ok(())
    }
    async fn merge_on_machine(&self, entry: &Entry, script: &str, agent: ProjectAgentResult) -> Result<ProjectAgentResult, HostError> {
        self.base
            .call("merge_on_machine", json!({ "id": rec(entry).id, "script": script, "agent": serde_json::to_value(&agent).unwrap() }));
        match self.base.answer("merge_on_machine") {
            None => Ok(agent),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(serde_json::from_value(v).expect("a scripted merge result reads")),
            },
        }
    }
    fn project_of(&self, dest: &str, bytes: u64) -> Option<Value> {
        self.base.call("project_of", json!({ "dest": dest, "bytes": bytes }));
        Some(json!({ "name": last_segment(dest), "dest": dest, "importedAt": "2026-10-25T00:00:00.000Z", "size": bytes }))
    }
    async fn provider_row(&self, entry: &Entry) -> Result<bool, HostError> {
        self.base.call("provider_row", json!({ "id": rec(entry).id }));
        self.base.answer_as("provider_row", || true)
    }
    async fn export_folder(
        &self,
        entry: &Entry,
        source: &str,
        caches: &CacheRule,
        archive: &str,
        timeout_ms: u64,
        on_progress: Rc<dyn Fn(Progress)>,
    ) -> Result<Vec<String>, HostError> {
        let rule = json!({ "dirs": caches.dirs, "files": caches.files, "markers": caches.markers });
        self.base.call("export_folder", json!({ "id": rec(entry).id, "source": source, "caches": rule, "archive": archive }));
        self.roads.borrow_mut().push(json!({ "road": "export_folder", "timeoutMs": timeout_ms }));
        if let Some(e) = self.base.answer("export_folder_fails").and_then(|v| error_in(&v)) {
            return Err(e);
        }
        std::fs::write(archive, "FOLDER").expect("the scratch folder stands");
        on_progress(Progress { part: 0, parts: 0, bytes: 0, total: 6 });
        on_progress(Progress { part: 0, parts: 0, bytes: 6, total: 6 });
        self.base.answer_as("export_folder", || vec!["dist".to_owned(), "node_modules".to_owned()])
    }
    async fn export_paths_into(
        &self,
        entry: &Entry,
        groups: Vec<(String, Vec<String>)>,
        archive: &str,
        timeout_ms: u64,
        on_progress: Rc<dyn Fn(Progress)>,
    ) -> Result<(), HostError> {
        self.base.call("export_paths_into", json!({ "id": rec(entry).id, "groups": groups, "archive": archive }));
        self.roads.borrow_mut().push(json!({ "road": "export_paths_into", "timeoutMs": timeout_ms }));
        if let Some(e) = self.base.answer("export_paths_into").and_then(|v| error_in(&v)) {
            return Err(e);
        }
        std::fs::write(archive, "STATE").expect("the scratch folder stands");
        on_progress(Progress { part: 0, parts: 0, bytes: 0, total: 5 });
        on_progress(Progress { part: 0, parts: 0, bytes: 5, total: 5 });
        Ok(())
    }
    async fn machine_run(&self, entry: &Entry, script: &str, deadline_ms: u64) -> Result<ExecResult, HostError> {
        self.base.call("machine_run", json!({ "id": rec(entry).id, "script": script, "deadlineMs": deadline_ms }));
        let mut got: ExecResult =
            self.base.answer_as("machine_run", || ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })?;
        // A listing names the copies it wrote under the scratch root it was handed, which only the script knows.
        if got.stdout.contains("{SCRATCH}") {
            got.stdout = got.stdout.replace("{SCRATCH}", &scratch_root(script).expect("the listing names a scratch root"));
        }
        Ok(got)
    }
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
        self.base.call("machine_exec", json!({ "id": rec(entry).id, "cmd": cmd, "timeoutMs": timeout_ms }));
        self.base.answer_as("machine_exec", || ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
    async fn kind_import(&self, entry: &Entry, o: ProjectImportOptions, report: ImportReport) -> Result<ImportLanded, HostError> {
        self.base.call(
            "kind_import",
            json!({ "id": rec(entry).id, "workspaceId": o.workspace_id, "source": o.source, "dest": o.dest, "replace": o.replace }),
        );
        if let Some(ms) = self.base.answer("kind_import_advance").and_then(|v| v.as_i64()) {
            self.base.clock.advance(ms);
        }
        for line in self.base.answer("kind_import_reports").unwrap_or(json!([])).as_array().unwrap() {
            let stage: ProjectImportStage = serde_json::from_value(line[0].clone()).expect("a stage reads");
            let progress = line.get(2).filter(|p| !p.is_null()).map(|p| (p[0].as_u64().unwrap(), p[1].as_u64().unwrap()));
            report(stage, line[1].as_str().unwrap(), progress)?;
        }
        match self.base.answer("kind_import") {
            None => Ok(ImportLanded {
                result: serde_json::from_value(json!({
                    "dest": o.dest, "files": 4, "bytes": 4000, "parts": 1, "cut": [], "rewritten": [], "agents": [],
                    "project": { "name": last_segment(&o.dest), "dest": o.dest, "importedAt": "2026-10-25T00:00:00.000Z", "size": 4000 }
                }))
                .unwrap(),
                done: format!("4 files, 4 KB, landed at {}.", o.dest),
            }),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(ImportLanded {
                    result: serde_json::from_value(v["result"].clone()).expect("a scripted result reads"),
                    done: v["done"].as_str().unwrap().to_owned(),
                }),
            },
        }
    }
    fn temp_dir(&self) -> String {
        self.tmp.path().to_string_lossy().into_owned()
    }
}

#[async_trait(?Send)]
impl ProjectsCtx for FakeProjects {
    fn state_folder(&self) -> Result<String, HostError> {
        if let Some(e) = self.state_fail.borrow().clone() {
            return Err(e);
        }
        Ok(format!("{HOME}/.wsp"))
    }
    fn has_state_path(&self) -> bool {
        self.state_path.get()
    }
    fn seed_wiring(&self) -> Option<Rc<dyn SeedWiring>> {
        self.seed.borrow().clone().map(|s| s as Rc<dyn SeedWiring>)
    }
    fn cloud_projects(&self) -> Option<bool> {
        self.cloud_projects.get()
    }
    fn vault_github_token(&self) -> Option<String> {
        self.vault_token.borrow().clone()
    }
    fn local_claude_home(&self) -> Option<String> {
        self.claude_home.borrow().clone()
    }
    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> {
        self.base.call("homes_here", json!(null));
        self.base.answer_as("homes_here", || [("claude".to_owned(), format!("{HOME}/.claude"))].into_iter().collect())
    }
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.base.call("refuse_recording", json!({ "word": word, "scoped": scoped(origin) }));
        self.base.answer_as("refuse_recording", || ())
    }
    async fn computer_rows(&self) -> Result<Vec<ComputerRow>, HostError> {
        self.base.call("computer_rows", json!(null));
        Ok(self.rows.borrow().clone())
    }
    fn name_of_computer(&self, id: &str, rows: &[ComputerRow]) -> String {
        rows.iter().find(|r| r.id == id).map(|r| r.name.clone()).unwrap_or_else(|| id.to_owned())
    }
    async fn clone_here(&self, word: &str, source: &ProjectSource, into: &str) -> Result<String, HostError> {
        self.base.call("clone_here", json!({ "word": word, "source": source_json(source), "into": into }));
        self.base.answer_as("clone_here", || into.to_owned())
    }
    async fn git_top_of(&self, path: &str) -> Result<Option<String>, HostError> {
        self.base.call("git_top_of", json!({ "path": path }));
        self.base.answer_as("git_top_of", || None)
    }
    async fn remote_here(&self, folder: &str) -> Result<String, HostError> {
        self.base.call("remote_here", json!({ "folder": folder }));
        self.base.answer_as("remote_here", || format!("git@github.com:acme/{}.git", last_segment(folder)))
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.base.call("landing_place", json!({ "computer": computer }));
        self.base.answer_as("landing_place", || if computer == "here" { None } else { Some(computer.to_owned()) })
    }
    async fn forking_at(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.base.call("forking_at", json!({ "placeId": place_id }));
        self.base.answer_as("forking_at", || ())?;
        Ok(Arc::new(StubBackend))
    }
    async fn landing_backend(&self, place_id: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.base.call("landing_backend", json!({ "placeId": place_id }));
        self.base.answer_as("landing_backend", || ())?;
        Ok(Arc::new(StubBackend))
    }
    fn folder_computer(&self, place_id: &str) -> Option<JoinedComputer> {
        self.base.call("folder_computer", json!({ "placeId": place_id }));
        self.joined.borrow().get(place_id).cloned()
    }
    fn github_from_vault(&self, place_id: &str) -> bool {
        self.base.call("github_from_vault", json!({ "placeId": place_id }));
        self.vault_places.borrow().iter().any(|p| p == place_id)
    }
    fn landing_road(&self, kind: LandingKind) -> Rc<dyn LandingRoad> {
        self.base.call("landing_road", json!({ "kind": kind_word(kind) }));
        self.road_of(kind)
    }
    fn project_source(&self, kind: &ProjectSource) -> Rc<dyn ProjectSourceModule> {
        self.base.call("project_source", json!({ "kind": source_json(kind)["kind"] }));
        self.source.clone()
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        self.base.call("remember_project", json!({ "project": serde_json::to_value(&project).unwrap() }));
        self.base.answer_as("remember_project", || ())?;
        self.base.store.put("projects", &project.id, serde_json::to_value(&project).unwrap()).await?;
        self.base.projects.borrow_mut().insert(project.id.clone(), project);
        Ok(())
    }
    fn project_reached(&self, origin: Option<&Caller>, project_id: &str) -> Result<bool, HostError> {
        self.base.call("project_reached", json!({ "scoped": scoped(origin), "project": project_id }));
        if let Some(e) = self.reached_fail.borrow().clone() {
            return Err(e);
        }
        Ok(self.reached.borrow().iter().any(|p| p == project_id))
    }
    fn project_of_scope(&self, origin: Option<&Caller>) -> Option<String> {
        self.base.call("project_of_scope", json!({ "scoped": scoped(origin) }));
        self.base.answer_as::<Option<String>>("project_of_scope", || None).unwrap()
    }
    fn elsewhere_refusal(&self, origin: Option<&Caller>, project: &ProjectView, word: &str) -> Result<Option<HostError>, HostError> {
        self.base.call("elsewhere_refusal", json!({ "scoped": scoped(origin), "project": project.id, "word": word }));
        if let Some(e) = self.elsewhere_fail.borrow().clone() {
            return Err(e);
        }
        Ok(self.base.answer("elsewhere_refusal").and_then(|v| error_in(&v)))
    }
    fn of_threads_repository(&self, origin: Option<&Caller>, project_id: &str) -> Result<bool, HostError> {
        self.base.call("of_threads_repository", json!({ "scoped": scoped(origin), "project": project_id }));
        if let Some(e) = self.threads_fail.borrow().clone() {
            return Err(e);
        }
        self.base.answer_as("of_threads_repository", || false)
    }
    fn holds_thread(&self, workspace_id: &str) -> bool {
        self.base.call("holds_thread", json!({ "id": workspace_id }));
        self.holding.borrow().iter().any(|w| w == workspace_id)
    }
    async fn delete_workspace(&self, id: &str, origin: Option<Caller>) -> Result<(), HostError> {
        self.base.call(
            "delete_workspace",
            json!({ "id": id, "scoped": scoped(origin.as_ref()), "writes": self.base.store.writes.borrow().len() }),
        );
        self.base.answer_as("delete_workspace", || ())?;
        self.base.live.borrow_mut().shift_remove(id);
        Ok(())
    }
    async fn copier_worktree_forget(&self, from: &str, home: &str, project: &str) -> Option<Result<(), HostError>> {
        self.base.call("copier_worktree_forget", json!({ "from": from, "home": home, "project": project }));
        if !self.copier.get() {
            return None;
        }
        Some(self.base.answer_as("copier_worktree_forget", || ()))
    }
    async fn forget_icon(&self, project_id: &str) -> Result<(), HostError> {
        self.base.call(
            "forget_icon",
            json!({
                "project": project_id,
                "held": self.base.projects.borrow().contains_key(project_id),
                "writes": self.base.store.writes.borrow().clone(),
                "events": self.base.events.borrow().len(),
            }),
        );
        self.base.answer_as("forget_icon", || ())
    }
    async fn set_icon(&self, project_id: &str, png: Option<String>) -> Result<Option<String>, HostError> {
        self.base.call("set_icon", json!({ "project": project_id, "png": png }));
        self.base.answer_as("set_icon", || None)
    }
    async fn read_icons(&self, hashes: Vec<String>) -> Result<IndexMap<String, Option<String>>, HostError> {
        self.base.call("read_icons", json!({ "hashes": hashes }));
        self.base.answer_as("read_icons", IndexMap::new)
    }
    fn runs_agent(&self, agent: &str) -> bool {
        self.runs.borrow().iter().any(|a| a == agent)
    }
    async fn agent_lists(&self, computer: &str, origin: Option<&Caller>) -> Result<Vec<HarnessCatalog>, HostError> {
        self.base.call("agent_lists", json!({ "computer": computer, "scoped": scoped(origin) }));
        match self.base.answer(&format!("agent_lists:{computer}")).or_else(|| self.base.answer("agent_lists")) {
            None => Ok(vec![]),
            Some(v) => match error_in(&v) {
                Some(e) => Err(e),
                None => Ok(serde_json::from_value(v).expect("scripted catalogues read")),
            },
        }
    }
    fn thread_defaults(&self, project_id: &str, first_agent: &str, lists: &[HarnessCatalog]) -> ThreadDefaults {
        self.base.call("thread_defaults", json!({ "project": project_id, "firstAgent": first_agent, "lists": lists.iter().map(|c| c.harness.clone()).collect::<Vec<_>>() }));
        match self.base.answer(&format!("thread_defaults:{project_id}")) {
            Some(v) => serde_json::from_value(v).expect("scripted defaults read"),
            None => serde_json::from_value(json!({ "agent": { "value": first_agent, "from": "catalog" } })).unwrap(),
        }
    }
    fn first_agent(&self) -> String {
        self.base.call("first_agent", json!(null));
        self.base.answer_as("first_agent", || "claude".to_owned()).unwrap()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Helpers for the assertions

/// `Kind` of the error, for a test that pins only the kind.
pub fn kind_of_error(e: &HostError) -> Option<Kind> {
    e.kind.clone()
}

/// The `project.add` events as `[stage, message]` pairs, in order.
pub fn add_stages(base: &FakeBase) -> Vec<(String, String)> {
    base.events
        .borrow()
        .iter()
        .filter(|e| e["type"] == "project.add")
        .map(|e| (e["stage"].as_str().unwrap().to_owned(), e["message"].as_str().unwrap().to_owned()))
        .collect()
}

/// Every event's `type`, in order.
pub fn event_types(base: &FakeBase) -> Vec<String> {
    base.events.borrow().iter().map(|e| e["type"].as_str().unwrap().to_owned()).collect()
}

/// An object's keys in the order it holds them.
pub fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

/// The project as JSON, keys in the struct's order.
pub fn pj(p: &ProjectView) -> Value {
    serde_json::to_value(p).unwrap()
}

/// A scripted object made from pairs, for `Map`-typed answers.
pub fn obj(pairs: Vec<(&str, Value)>) -> Map<String, Value> {
    pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

/// The stored preferences every test starts with: the defaults, no project defaults named.
pub fn prefs() -> Preferences {
    serde_json::from_value(json!({"theme":"dark","lightTheme":"paper","darkTheme":"graphite","sidebarMode":"list","terminalSize":"app","terminalZoom":{},"access":{},"projectLook":{},"projectIcon":{},"computerLook":{},"serverIcons":true,"agentVersions":true,"usageLogs":true,"productUsage":true,"keepAwake":true,"transparency":true,"projectOrder":[],"keybindings":{},"appFont":"","codeFont":"","agentDefaults":{},"projectDefaults":{},"sendWith":"enter","midTurn":"queue","notifyNeeds":"notify-sound","notifyDone":"notify","planAlerts":true,"settleAfter":"2h","askDelete":true,"onQuit":"ask","newThreadIn":"current","labs":false}))
        .expect("preferences read")
}

/// A catalogue of one agent's models and options, as JSON an `agent_lists` answer scripts.
pub fn catalog(harness: &str) -> Value {
    json!({ "harness": harness, "label": harness, "source": "table", "version": null, "models": [], "efforts": [], "contextWindows": [],
            "permissionModes": [], "steers": false, "renames": false, "images": false })
}

/// The scratch root a state listing was told to write its copies under: the `/tmp/wsp-state-...` value among the
/// base64 JSON data the script carries (`data("<base64>")`).
pub fn scratch_root(script: &str) -> Option<String> {
    use base64::Engine;
    let mut rest = script;
    while let Some(at) = rest.find("data(\"") {
        rest = &rest[at + 6..];
        let end = rest.find('"')?;
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&rest[..end]) {
            if let Ok(Value::String(text)) = serde_json::from_slice::<Value>(&bytes) {
                if text.starts_with("/tmp/wsp-state-") {
                    return Some(text.splitn(4, '/').take(3).collect::<Vec<_>>().join("/"));
                }
            }
        }
    }
    None
}
