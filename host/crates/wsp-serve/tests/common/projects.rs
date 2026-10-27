// SPDX-License-Identifier: AGPL-3.0-only
//! stub-backend.ts:399-460: `projectOn`, `tempRepo`, `createOn` and `ownedStore`. A workspace is one project's copy,
//! so a test about a nap, a status or a thread still needs one; these are the lines that make it.
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use wsp_core::types::api::{ProjectAddAsk, Runtime};
use wsp_core::types::wiring::{CreateWorkspaceOptions, CreatedWorkspace, IdleWindow, WorkspaceSpec};
use wsp_serve::AssembledRuntime;
use wsp_store::store::{memory_store, Store};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::JsNumber;
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_words::exit::HostError;

use super::events::json;

thread_local! {
    static STUBS: Cell<u32> = const { Cell::new(0) };
}

/// `tempRepo()`: a git repo in a folder of its own, for a project on this computer that has branches.
pub fn temp_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("wsp-project-{}-{}", std::process::id(), rand_suffix()));
    std::fs::create_dir_all(&dir).expect("a folder");
    let dir = std::fs::canonicalize(dir).expect("a real path");
    let ok = std::process::Command::new("git").arg("init").arg("-q").arg(&dir).status().expect("git").success();
    assert!(ok, "git init");
    dir
}

fn rand_suffix() -> String {
    format!("{:016x}", rand::random::<u64>())
}

/// The computer a test's workspaces land on when it names none: the first row that takes forks, else the first
/// provider, else the one a host wired without places forks at.
async fn forking_computer(rt: &AssembledRuntime) -> Result<String, HostError> {
    let rows = rt.projects().computers().await?;
    Ok(rows.iter().find(|r| r.id != HERE_PLACE_ID).or_else(|| rows.first()).expect("a computer").id.clone())
}

/// `projectOn(rt, computer, source, named)`.
pub async fn project_on(
    rt: &AssembledRuntime,
    computer: Option<&str>,
    source: Option<&str>,
    name: Option<&str>,
) -> Result<wsp_wire::views::workspace::ProjectView, HostError> {
    project_with(rt, computer, source, name, None).await
}

/// `projectOn(rt, computer, source, { name, base })`: as `project_on`, naming the base branch the project's pull requests
/// open against.
pub async fn project_with(
    rt: &AssembledRuntime,
    computer: Option<&str>,
    source: Option<&str>,
    name: Option<&str>,
    base: Option<&str>,
) -> Result<wsp_wire::views::workspace::ProjectView, HostError> {
    let on = match computer {
        Some(on) => on.to_string(),
        None => forking_computer(rt).await?,
    };
    let source = match source {
        Some(source) => source.to_string(),
        None if on == HERE_PLACE_ID => temp_repo().to_string_lossy().into_owned(),
        None => {
            let n = STUBS.with(|s| {
                s.set(s.get() + 1);
                s.get()
            });
            format!("https://github.com/wsp/stub-{n}.git")
        }
    };
    let ask = ProjectAddAsk { source, on: Some(on), name: name.map(str::to_string), base: base.map(str::to_string), ..Default::default() };
    Ok(rt.projects().add(ask, None).await?.project)
}

/// `CreateOn`: the options of `createOn`, with the spec flattened as the TS writes it.
#[derive(Clone, Default)]
pub struct CreateOn {
    pub name: String,
    pub golden: Option<String>,
    pub on: Option<String>,
    pub project: Option<String>,
    pub cpu: Option<f64>,
    pub mem_mb: Option<f64>,
    pub parent: Option<String>,
    pub idle_window_ms: IdleWindow,
    /// `agents: { maxMachines, maxDepth }`: the keys of `WorkspaceAgents` a test names.
    pub agents: Option<serde_json::Map<String, serde_json::Value>>,
    pub branch_of_record: Option<wsp_wire::views::workspace::BranchOfRecord>,
}

impl CreateOn {
    pub fn named(name: &str) -> CreateOn {
        CreateOn { name: name.into(), ..Default::default() }
    }
    /// `{ golden: "snap_g", name }`: nearly every create of the TS tests.
    pub fn golden(name: &str) -> CreateOn {
        CreateOn { name: name.into(), golden: Some("snap_g".into()), ..Default::default() }
    }
    pub fn on(mut self, computer: &str) -> CreateOn {
        self.on = Some(computer.into());
        self
    }
    pub fn cpu(mut self, cpu: f64) -> CreateOn {
        self.cpu = Some(cpu);
        self
    }
    pub fn mem(mut self, mem_mb: f64) -> CreateOn {
        self.mem_mb = Some(mem_mb);
        self
    }
    pub fn golden_is(mut self, golden: &str) -> CreateOn {
        self.golden = Some(golden.into());
        self
    }
    /// `branchOfRecord: { repoId, owner, name, branch }`.
    pub fn branch_of_record(mut self, repo_id: i64, owner: &str, name: &str, branch: &str) -> CreateOn {
        self.branch_of_record = Some(wsp_wire::views::workspace::BranchOfRecord {
            repo_id: repo_id.into(),
            owner: owner.into(),
            name: name.into(),
            branch: branch.into(),
            start_sha: None,
            public: None,
        });
        self
    }
}

/// `createOn({ agents })` with the object written out (`.agents(json!({ "maxMachines": 3, "maxDepth": 1 }))`). Two parts
/// wrote the one option with two call shapes, so each is a trait a test file imports.
pub trait AgentsDoc {
    fn agents(self, agents: serde_json::Value) -> CreateOn;
}

impl AgentsDoc for CreateOn {
    fn agents(mut self, agents: serde_json::Value) -> CreateOn {
        self.agents = agents.as_object().cloned();
        self
    }
}

/// `createOn({ agents: { maxMachines, maxDepth } })` as two numbers (`.agents(3, 2)`).
pub trait AgentsCaps {
    fn agents(self, max_machines: u32, max_depth: u32) -> CreateOn;
}

impl AgentsCaps for CreateOn {
    fn agents(self, max_machines: u32, max_depth: u32) -> CreateOn {
        AgentsDoc::agents(self, serde_json::json!({ "maxMachines": max_machines, "maxDepth": max_depth }))
    }
}

/// `createOn(rt, o, origin)`: a workspace of a fresh project, for every test whose subject is the workspace and not
/// the project.
pub async fn create_on(rt: &AssembledRuntime, o: CreateOn, origin: Option<Caller>) -> Result<CreatedWorkspace, HostError> {
    // A thread works on its own project alone, so a fork a thread asks for is of the project its own workspace holds.
    let own = match scope_of(origin.as_ref()) {
        Some(scope) => Some(rt.workspaces().get(&scope.workspace_id, origin.clone(), None).await?.project.id),
        None => None,
    };
    let id = match (o.project.clone(), own) {
        (Some(project), _) => project,
        (None, Some(own)) => own,
        (None, None) => {
            // A project on this computer is its folder, and the record of that folder carries the project's name.
            let name = (o.on.as_deref() == Some(HERE_PLACE_ID)).then_some(o.name.as_str());
            project_on(rt, o.on.as_deref(), None, name).await?.id
        }
    };
    let opts = CreateWorkspaceOptions {
        spec: WorkspaceSpec { cpu: o.cpu.map(JsNumber), mem_mb: o.mem_mb.map(JsNumber), ..Default::default() },
        project: id,
        golden: o.golden,
        name: o.name,
        parent: o.parent,
        idle_window_ms: o.idle_window_ms,
        agents: o.agents,
        branch_of_record: o.branch_of_record,
        ..Default::default()
    };
    rt.workspaces().create(opts, origin).await
}

/// `ownedStore(owner)`: a state file that already holds its owner id, so every image a runtime on it names carries a
/// mark a test can write down: the host's id, then this one.
pub async fn owned_store(owner: &str) -> Rc<dyn Store> {
    let store: Rc<dyn Store> = Rc::new(memory_store());
    store.put("owner", "id", json!({ "id": owner })).await.expect("the owner is written");
    store
}
