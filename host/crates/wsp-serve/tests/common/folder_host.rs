// SPDX-License-Identifier: AGPL-3.0-only
//! `here()`, `repo()` and `scratch()` of folder-threads.test.ts at 65f6b89c7: a host on this computer, over a folder of its own, whose
//! threads run in a project's folder or in a worktree of its repo. The copier does the worktree half with real git; the daemon is a
//! fake that takes checkpoints and restores and keeps every frame, where a test may answer any op.
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_core::daemon_channel::Frame;
use wsp_core::types::api::Runtime as _;
use wsp_core::types::api::{FolderFor, FolderForAsk, ProjectAddAsk, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::SessionHandle;
use wsp_core::types::wiring::{HereDaemon, LocalWiring};
use wsp_engine::copier::Copier;
use wsp_engine::machine::MachineBackend;
use wsp_serve::serve_start::serve_start;
use wsp_wire::wire::runtime_ops::parse_runtime_request;

use super::daemon::git;
use super::local::{git_copier, local_wiring, CopierLog};
use super::quick::{Quick, QuickOptions};
use super::recdaemon::RecDaemon;
use super::{j, runtime, sqlite_state, Fx, StubBackend};

/// Folders of the test's own, removed when it ends: `scratch()` of the TS file.
#[derive(Default)]
pub struct Scratch(RefCell<Vec<tempfile::TempDir>>);

impl Scratch {
    pub fn new() -> Rc<Scratch> {
        Rc::new(Scratch::default())
    }

    pub fn dir(&self) -> PathBuf {
        let dir = tempfile::Builder::new().prefix("wsp-folder-threads-").tempdir().expect("a folder");
        let path = std::fs::canonicalize(dir.path()).expect("a real path");
        self.0.borrow_mut().push(dir);
        path
    }

    /// A repo on main with one commit.
    pub fn repo(&self) -> PathBuf {
        let at = self.dir();
        git(&at, &["init", "-q", "-b", "main"]);
        git(&at, &["commit", "-q", "--allow-empty", "-m", "first"]);
        at
    }

    /// A repo on main whose one commit tracks these lockfiles, each in the folder its path names.
    pub fn repo_with(&self, lockfiles: &[&str]) -> PathBuf {
        let at = self.repo();
        for f in lockfiles {
            let file = at.join(f);
            std::fs::create_dir_all(file.parent().expect("a folder")).expect("a folder");
            std::fs::write(file, "lock\n").expect("a lockfile");
        }
        git(&at, &["add", "."]);
        git(&at, &["commit", "-q", "-m", "locks"]);
        at
    }
}

/// What differs between the TS test's calls of `here(wire, served, adapters, again)`.
#[derive(Default)]
pub struct HereOpts {
    /// Replaces the wiring's backend.
    pub backend: Option<Arc<dyn MachineBackend>>,
    /// Replaces the wiring's copier: `Some(None)` is a host that found no daemon binary.
    pub copier: Option<Option<Rc<dyn Copier>>>,
    pub here_daemon: Option<Rc<dyn HereDaemon>>,
    /// A runtime handed no state file.
    pub unserved: bool,
    pub adapters: Vec<(&'static str, HarnessAdapterFactory)>,
    /// A host started over the root and store of one before it, as a restart finds them.
    pub again: Option<Again>,
    /// The store the runtime reads and writes, where it is not the state's own.
    pub store: Option<Rc<dyn super::Store>>,
}

pub struct Again {
    pub root: PathBuf,
    pub state: Rc<super::State>,
}

pub struct Here {
    pub fx: Fx,
    pub root: PathBuf,
    pub state: Rc<super::State>,
    pub copier: CopierLog,
    pub agents: Rc<Quick>,
    pub daemon: Rc<RecDaemon>,
    /// The state folder of the host, where its worktrees live under `worktrees`.
    pub home: PathBuf,
    pub worktrees: PathBuf,
    pub roots: PathBuf,
    pub claude_home: PathBuf,
}

/// The daemon of `fakeDaemon()`: a checkpoint and a restore are answered, and any other op is refused until a test answers it.
pub fn fake_daemon() -> Rc<RecDaemon> {
    RecDaemon::new(|f: &Frame| {
        let text = |key: &str| f.get(key).and_then(Value::as_str).map_or_else(|| "undefined".to_string(), str::to_string);
        match text("op").as_str() {
            "git.checkpoint" => json!({
                "id": 1, "ok": true, "ref": format!("refs/wsp/checkpoints/{}/{}/{}", text("scope"), text("thread"), text("turn")),
                "commit": "c", "changed": true,
            }),
            "git.restore" => json!({ "id": 1, "ok": true, "before": format!("{}-before-1", text("checkpoint")), "files": 1 }),
            "git.checkpointDrop" => json!({ "id": 1, "ok": true }),
            other => json!({ "id": 1, "ok": false, "error": format!("no {other} here") }),
        }
    })
}

pub fn here(scratch: &Scratch, o: HereOpts) -> Here {
    let (root, state) = match o.again {
        Some(again) => (again.root, again.state),
        None => (scratch.dir(), Rc::new(sqlite_state())),
    };
    let home = root.join("state");
    std::fs::create_dir_all(&home).expect("a state folder");
    let (copier, log) = git_copier();
    let agents = Quick::new(QuickOptions { copies: true, inline: true, ..Default::default() });
    let daemon = fake_daemon();
    let mut local: LocalWiring = local_wiring(&root, copier);
    if let Some(backend) = o.backend {
        local.backend = backend;
    }
    if let Some(copier) = o.copier {
        local.copier = copier;
    }
    if let Some(here_daemon) = o.here_daemon {
        local.here_daemon = Some(here_daemon);
    }
    let state_path = (!o.unserved).then(|| home.join("state.json").display().to_string());
    let open = daemon.open();
    let backend = StubBackend::new(None);
    let mut wiring = runtime(&backend)
        .store(o.store.as_ref().unwrap_or(&state.store))
        .adapter("claude", agents.factory())
        .adapter("codex", agents.factory());
    for (name, factory) in o.adapters {
        wiring = wiring.adapter(name, factory);
    }
    let fx = wiring
        .with(move |opts| {
            opts.local = Some(local);
            opts.state_path = state_path;
            opts.daemon_channel = Some(open);
        })
        .build();
    Here {
        fx,
        worktrees: home.join("worktrees"),
        roots: root.join("roots"),
        claude_home: root.join(".claude"),
        home,
        root,
        state,
        copier: log,
        agents,
        daemon,
    }
}

impl Here {
    pub fn frames(&self) -> Vec<Value> {
        self.daemon.frames.borrow().iter().map(|f| Value::Object(f.clone())).collect()
    }

    pub fn ops(&self) -> Vec<String> {
        self.daemon.ops()
    }
}

pub fn path_of(p: &Path) -> String {
    p.display().to_string()
}

pub async fn add(h: &Here, source: &Path) -> Value {
    j(&h.fx.rt.projects().add(ProjectAddAsk { source: path_of(source), ..Default::default() }, None).await.expect("a project").project)
}

pub fn id_of(v: &Value) -> String {
    v["id"].as_str().expect("an id").to_string()
}

pub fn ask(project: &str, branch: Option<&str>, cwd: Option<&Path>) -> FolderForAsk {
    FolderForAsk { project: Some(project.into()), branch: branch.map(str::to_string), cwd: cwd.map(path_of), ..Default::default() }
}

pub async fn folder_for(h: &Here, ask: FolderForAsk) -> FolderFor {
    h.fx.rt.workspaces().folder_for(ask, None).await.expect("a folder")
}

pub fn prompt(text: &str) -> StartOptions {
    StartOptions { prompt: text.into(), ..Default::default() }
}

pub async fn start(h: &Here, ws: &str, text: &str) -> Rc<dyn SessionHandle> {
    h.fx.rt.sessions().start(ws, prompt(text), None).await.expect("a start")
}

pub async fn ran(h: &Here, ws: &str, text: &str) -> Rc<dyn SessionHandle> {
    let turn = start(h, ws, text).await;
    turn.finished().await;
    turn
}

pub async fn ran_in(h: &Here, ws: &str, o: StartOptions) -> Rc<dyn SessionHandle> {
    let turn = h.fx.rt.sessions().start(ws, o, None).await.expect("a start");
    turn.finished().await;
    turn
}

pub fn thread_of(turn: &Rc<dyn SessionHandle>) -> String {
    turn.view().thread_id.expect("a thread")
}

pub fn cwd_strings(h: &Here) -> Vec<Option<String>> {
    h.agents.cwds()
}

pub fn ws_json(f: &FolderFor) -> Value {
    j(&f.workspace)
}

pub async fn history_of(h: &Here, ws: &str) -> Vec<Value> {
    h.fx.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

pub async fn workspaces_of(h: &Here) -> Vec<Value> {
    h.fx.rt.workspaces().list(None).await.expect("a list").iter().map(j).collect()
}

pub fn asks(h: &Here) -> usize {
    h.copier.lock().unwrap().asks.len()
}

pub fn worktrees_asked(h: &Here) -> Vec<Value> {
    h.copier.lock().unwrap().worktrees.iter().map(j).collect()
}

pub fn prefs(doc: Value) -> wsp_wire::views::preferences::PreferencesPatch {
    serde_json::from_value(doc).expect("a patch")
}

pub fn start_request(fields: Value) -> wsp_wire::wire::runtime_ops::RuntimeRequest {
    let mut frame = fields.as_object().cloned().expect("fields");
    frame.insert("id".into(), json!(1));
    frame.insert("op".into(), json!("sessions.start"));
    parse_runtime_request(&Value::Object(frame)).unwrap_or_else(|e| panic!("the test's frame does not pass the schema: {e:?}"))
}

/// A `sessions.start` as the serve layer answers it: the thread it opened, read off the first answer.
pub async fn serve_started(h: &Here, fields: Value) -> String {
    let thread: Rc<RefCell<Option<String>>> = Rc::default();
    let kept = thread.clone();
    let reply: wsp_serve::serve_start::AnswerFn = Rc::new(move |r| *kept.borrow_mut() = r.session.thread_id.clone());
    serve_start(&*h.fx.rt, &start_request(fields), None, reply).await.expect("a start");
    let found = thread.borrow().clone();
    found.expect("a thread")
}

pub async fn behind_lines(h: &Here, ws: &str, thread: Option<&str>) -> Vec<String> {
    history_of(h, ws)
        .await
        .into_iter()
        .filter(|e| e["type"] == "session.behind" && thread.is_none_or(|t| e["threadId"] == t))
        .map(|e| e["text"].as_str().expect("a line").to_string())
        .collect()
}
