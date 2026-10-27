// SPDX-License-Identifier: AGPL-3.0-only
//! What pull-request.test.ts at 65f6b89c7 builds before its cases: two daemons behind one dial (this computer's and the
//! copy's, told apart by the url dialled), this computer wired with its own daemon at HERE, a workspace on a stub
//! backend, and the pull request each workspace's latest pushed status carried. Two test files read it.
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt as _;
use serde_json::{json, Map, Value};
use wsp_core::clock::FakeClock;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::copy_key;
use wsp_core::types::wiring::{CreateWorkspaceOptions, Local, LocalWiring, WorkspaceSpec};
use wsp_store::store::Store;
use wsp_wire::types::JsNumber;

use super::adapter::Held;
use super::*;

pub const DAEMON_TOKEN: &str = "cafef00dcafef00dcafef00d";
/// Where this computer's own daemon answers, and where the copy's does: every frame is told apart by the road it came on.
pub const HERE: &str = "ws://this-computer";
pub const COPY: &str = "http://127.0.0.1:7070";
pub const HEAD: &str = "ec5c10de663bd1860925ad42e9580bab4eb1d377";
/// What a read saw of the pull request, its last update, head and state, before and after a push to it.
pub const SEEN: &str = "2026-10-05T17:40:01Z ec5c10de663bd1860925ad42e9580bab4eb1d377 open";
pub const MOVED: &str = "2026-10-05T18:02:11Z c0ffee open";

pub fn status() -> Value {
    json!({ "id": 1, "ok": true, "branch": { "oid": "abc", "head": "fix/ci", "upstream": "origin/fix/ci", "ahead": 1, "behind": 0 }, "entries": [], "root": "/root/stub" })
}

/// `open(over)`: the open pull request every read answers unless a case says otherwise.
pub fn open(over: Value) -> Value {
    let mut pr = json!({
        "number": 12,
        "url": "https://github.com/wsp/pr-lab/pull/12",
        "state": "open",
        "host": "github.com",
        "draft": false,
        "base": "main",
        "branch": "fix/ci",
        "headOid": HEAD,
        "headSubject": "Set .ci-status to 1",
        "mergeable": "mergeable",
        "mergeState": "clean",
        "review": "none",
        "checks": [{ "name": "ci", "workflow": "ci", "state": "pass", "run": { "runId": 36, "jobId": 109 }, "link": "https://github.com/wsp/pr-lab/actions/runs/36/job/109" }],
        "additions": 2,
        "deletions": 1,
        "changedFiles": 1,
        "commits": 1,
        "behindBase": 0
    });
    if let (Some(pr), Some(over)) = (pr.as_object_mut(), over.as_object()) {
        pr.extend(over.clone());
    }
    pr
}

pub fn ok(more: Value) -> Value {
    let mut reply = json!({ "id": 1, "ok": true });
    if let (Some(reply), Some(more)) = (reply.as_object_mut(), more.as_object()) {
        reply.extend(more.clone());
    }
    reply
}

pub fn no_gh() -> Answer {
    Rc::new(
        |_| json!({ "id": 1, "ok": false, "code": "no-host-cli", "error": "no signed-in command line for github.com is on this computer; the branch is pushed and the pull request waits for one" }),
    )
}

pub fn rate_limited() -> Answer {
    Rc::new(
        |_| json!({ "id": 1, "ok": false, "code": "rate-limited", "error": "gh said: GraphQL: API rate limit already exceeded for user ID 1." }),
    )
}

pub type Answer = Rc<dyn Fn(&Map<String, Value>) -> Value>;

pub fn answer(f: impl Fn(&Map<String, Value>) -> Value + 'static) -> Answer {
    Rc::new(f)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Via {
    Here,
    Copy,
}

impl Via {
    pub fn word(self) -> &'static str {
        match self {
            Via::Here => "here",
            Via::Copy => "copy",
        }
    }
}

type Table = Rc<RefCell<Vec<(String, Answer)>>>;

/// `fakeDaemons({ here, copy })`: each records every frame with its road, and answers by op; a road with no answer for
/// an op refuses it as a daemon does.
#[derive(Clone)]
pub struct Daemons {
    pub frames: Rc<RefCell<Vec<(Via, Map<String, Value>)>>>,
    here: Table,
    copy: Table,
}

fn table(of: Vec<(&str, Answer)>) -> Table {
    Rc::new(RefCell::new(of.into_iter().map(|(op, a)| (op.to_string(), a)).collect()))
}

pub fn fake_daemons(here: Vec<(&str, Answer)>, copy: Vec<(&str, Answer)>) -> Daemons {
    let mut here_all: Vec<(&str, Answer)> = vec![("git.prRead", answer(|_| ok(json!({ "pr": open(json!({})) }))))];
    here_all.extend(here);
    let mut copy_all: Vec<(&str, Answer)> = vec![
        ("git.status", answer(|_| status())),
        ("git.startOn", answer(|f| ok(json!({ "branch": f.get("branch").and_then(Value::as_str).unwrap_or_default(), "oid": "c0ffee" })))),
    ];
    copy_all.extend(copy);
    Daemons { frames: Rc::default(), here: table(here_all), copy: table(copy_all) }
}

fn lookup(table: &Table, op: &str) -> Option<Answer> {
    table.borrow().iter().rev().find(|(o, _)| o == op).map(|(_, a)| a.clone())
}

impl Daemons {
    pub fn ops(&self, road: Option<Via>) -> Vec<String> {
        self.frames
            .borrow()
            .iter()
            .filter(|(r, _)| road.is_none_or(|want| *r == want))
            .map(|(_, f)| f.get("op").and_then(Value::as_str).unwrap_or_default().to_string())
            .collect()
    }

    /// How many frames of this op `road` has had.
    pub fn count(&self, road: Via, op: &str) -> usize {
        self.ops(Some(road)).iter().filter(|o| *o == op).count()
    }

    /// The frames of this op, with their roads.
    pub fn of(&self, op: &str) -> Vec<(Via, Map<String, Value>)> {
        self.frames.borrow().iter().filter(|(_, f)| f.get("op").and_then(Value::as_str) == Some(op)).cloned().collect()
    }

    /// Changes what `road` answers an op with from now on.
    pub fn set(&self, road: Via, op: &str, a: Answer) {
        let table = if road == Via::Here { &self.here } else { &self.copy };
        table.borrow_mut().push((op.to_string(), a));
    }

    pub fn open(&self) -> Rc<dyn Fn(DaemonChannelOptions) -> Local<'static, Rc<dyn DaemonChannel>>> {
        let daemons = self.clone();
        Rc::new(move |opts: DaemonChannelOptions| {
            let road = if opts.url == HERE { Via::Here } else { Via::Copy };
            let channel = Channel { road, daemons: daemons.clone() };
            async move { Ok(Rc::new(channel) as Rc<dyn DaemonChannel>) }.boxed_local()
        })
    }
}

struct Channel {
    road: Via,
    daemons: Daemons,
}

#[async_trait(?Send)]
impl DaemonChannel for Channel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.daemons.frames.borrow_mut().push((self.road, frame.clone()));
        let op = frame.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
        let table = if self.road == Via::Here { &self.daemons.here } else { &self.daemons.copy };
        let reply = match lookup(table, &op) {
            Some(answer) => answer(&frame),
            None => json!({ "id": 1, "ok": false, "error": format!("{op} was not answered here") }),
        };
        match reply {
            Value::Object(map) => Ok(map),
            other => panic!("not an object: {other}"),
        }
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending().await
    }
}

/// An agent whose turns are held until the test ends them, recording every prompt it was given.
pub struct HeldAgent {
    pub held: Rc<Held>,
}

impl HeldAgent {
    pub fn new() -> HeldAgent {
        HeldAgent { held: Held::new(false) }
    }
    pub fn factory(&self) -> HarnessAdapterFactory {
        self.held.factory()
    }
    pub fn prompts(&self) -> Vec<String> {
        self.held.starts.borrow().iter().map(|s| s.prompt.clone()).collect()
    }
    pub fn end(&self, nth: usize) {
        self.held.end(nth, "done", json!({}));
    }
}

pub fn image() -> Value {
    json!({
        "head": 1,
        "versions": [{ "version": 1, "snapshotId": "snap_g", "kind": "desktop", "baseTemplate": "base", "setupSha": "abc", "createdAt": "2026-09-01T00:00:00.000Z", "smoke": { "cmd": "true", "exitCode": 0 } }]
    })
}

/// This computer wired as a host wires it, its own daemon at HERE.
pub fn local_on(root: &std::path::Path, daemon_road: bool) -> LocalWiring {
    let mut local = fake_local(root);
    if daemon_road {
        local.daemon_road = Some(Rc::new(|| {
            Box::pin(async {
                Ok(wsp_wire::views::workspace::DaemonReachView {
                    url: HERE.into(),
                    expires_at: JsNumber(9_007_199_254_740_991.0),
                    daemon_token: Some(DAEMON_TOKEN.into()),
                })
            })
        }));
    }
    local
}

/// A runtime over a stub backend and a workspace on it, as `withWorkspace` makes them.
pub struct Held2 {
    pub fx: Fx,
    pub backend: Arc<StubBackend>,
    pub id: String,
    pub name: String,
    pub remote: String,
    pub events: Events,
    pub root: PathBuf,
    pub store: Rc<dyn Store>,
    _dir: tempfile::TempDir,
    _state: State,
}

pub struct With {
    pub adapters: Vec<(&'static str, HarnessAdapterFactory)>,
    pub clock: Option<FakeClock>,
    pub daemon_road: bool,
}

impl Default for With {
    fn default() -> Self {
        With { adapters: vec![], clock: None, daemon_road: true }
    }
}

pub async fn with_workspace(daemons: &Daemons, o: With) -> Held2 {
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    let state = sqlite_state();
    state.store.put("goldens", &copy_key("default", "default"), image()).await.expect("an image");
    let backend = StubBackend::new(None);
    backend.with_daemon_roads();
    backend.set_exec_impl(token_guest());
    let (channel, local) = (daemons.open(), local_on(&root, o.daemon_road));
    let mut wired = runtime(&backend).store(&state.store).with(move |opts| {
        opts.daemon_token = Some(DAEMON_TOKEN.into());
        opts.daemon_channel = Some(channel);
        opts.local = Some(local);
    });
    for (name, factory) in o.adapters {
        wired = wired.adapter(name, factory);
    }
    if let Some(clock) = &o.clock {
        wired = wired.clock(clock);
    }
    let fx = wired.build();
    let events = fx.events();
    let project = fx
        .rt
        .projects()
        .add(
            wsp_core::types::api::ProjectAddAsk {
                source: "https://github.com/wsp/stub-9.git".into(),
                on: Some(first_provider(&fx.rt).await),
                base: Some("main".into()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("a project");
    let agents = json!({ "spawn": true, "maxMachines": 3, "maxDepth": 3 });
    let ws = fx
        .rt
        .workspaces()
        .create(
            CreateWorkspaceOptions {
                spec: WorkspaceSpec::default(),
                project: project.project.id.clone(),
                golden: Some("snap_g".into()),
                name: "pr-lab".into(),
                agents: agents.as_object().cloned(),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("a workspace");
    backend.machine(0).set_preview_url(COPY);
    Held2 {
        fx,
        backend,
        id: ws.view.id.clone(),
        name: ws.view.name.clone(),
        remote: project.project.remote.clone(),
        events,
        root,
        store: state.store.clone(),
        _dir: dir,
        _state: state,
    }
}

async fn first_provider(rt: &wsp_serve::AssembledRuntime) -> String {
    let rows = rt.projects().computers().await.expect("computers");
    rows.iter().find(|r| r.id != wsp_wire::types::place_word::HERE_PLACE_ID).or_else(|| rows.first()).expect("a computer").id.clone()
}

impl Held2 {
    /// The pull request the workspace's latest pushed status carried.
    pub fn pr(&self) -> Option<Value> {
        self.events
            .of("workspace.status")
            .iter()
            .rev()
            .find(|e| e["status"]["id"] == self.id.as_str())
            .and_then(|e| e["status"].get("pr").cloned())
    }

    /// `(await prOf(id))?.pr?.[key]`.
    pub fn pr_field(&self, key: &str) -> Option<Value> {
        self.pr().and_then(|pr| pr.get(key).cloned())
    }

    pub fn rt(&self) -> &wsp_serve::AssembledRuntime {
        &self.fx.rt
    }
}

pub fn flag() -> Rc<Cell<bool>> {
    Rc::default()
}
