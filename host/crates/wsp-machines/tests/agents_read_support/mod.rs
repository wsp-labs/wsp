// SPDX-License-Identifier: AGPL-3.0-only
//! What the agents-read tests build on: a rig of fakes for every port `AgentsReadOptions` holds, all sharing one state
//! and one ordered log. Written after the options object of packages/runtime/test/agents-read.test.ts.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::{select, LocalBoxFuture};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::sync::Notify;

use wsp_core::clock::Clock;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{EngineError, Result as EResult};
use wsp_engine::machine::{
    ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineState, RunOptions,
    SnapshotOptions,
};
use wsp_machines::agents_read::{
    AgentsActs, AgentsOn, AgentsPlaces, AgentsRead, AgentsReadOptions, AgentsReader, AgentsReads,
    AgentsWorkspace, BoxLogin, CallbackForwards, CodeWriter, DaemonEvent, Follower, GhStep,
    PluginsActs, ResolvedServer, ServerIcons, ServerToolsAsk, ServersActs, SignInAsk,
    SignInForward, SignInPlan, SignInRun, SignInStep, SkillAddAsk, SkillAsk, SkillsActs,
};
use wsp_wire::types::agents_report::{
    AgentSignInState, AgentsProject, AgentsSignInEvent, AgentsSignInEventState, AgentsTarget,
    PluginAsk, PluginRow, ServerAdd, ServerAsk, ServerToolsAnswer, SignInLine, SkillAdded,
    SkillHit, SkillPreview,
};
use wsp_wire::types::thread_defaults::{AgentSetupSet, AgentSetupView};
use wsp_wire::views::place::PlaceProvisionRow;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::{HostError, Kind};

pub use crate::rest_support::{plain_error, run, TestClock, Warned};

/// 2026-09-24T12:00:00Z, the clock every TS case reads.
pub const T0: i64 = 1_790_251_200_000;
/// 2026-09-25T12:00:00Z, the clock of the projects group.
pub const T1: i64 = 1_790_337_600_000;
pub const READ_AT: &str = "2026-09-24T12:00:00.000Z";

/// Every catalog agent id, in catalog order (`CATALOG_AGENTS.map(a => a.id)` at 65f6b89c7); wsp-catalog's Rust catalog is
/// a stub, so the list is written out.
pub const CATALOG_IDS: [&str; 11] = [
    "claude", "codex", "gemini", "opencode", "pi", "hermes", "crush", "qwen", "goose", "amp",
    "cursor",
];

pub fn landing() -> AgentsProject {
    project("pr_landing", "landing", "/root/landing")
}

pub fn project(id: &str, name: &str, path: &str) -> AgentsProject {
    AgentsProject {
        id: id.to_string(),
        name: name.to_string(),
        path: path.to_string(),
    }
}

pub fn target(v: Value) -> AgentsTarget {
    serde_json::from_value(v).expect("a target")
}

pub fn ws(id: &str) -> AgentsTarget {
    target(json!({ "workspaceId": id }))
}

pub fn pl(id: &str) -> AgentsTarget {
    target(json!({ "placeId": id }))
}

pub fn js<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("serializes")
}

/// `Object.assign(new Error(sentence), { kind: "usage" })`.
pub fn usage(sentence: &str) -> HostError {
    HostError {
        kind: Some(Kind::Usage),
        ..HostError::new(sentence)
    }
}

/// The reader's answer of every TS case: `READ`.
pub fn read_answer() -> AgentsRead {
    AgentsRead {
        home: "/root".into(),
        user: "root".into(),
        agents: vec![],
        skills: vec![],
        servers: vec![],
        plugins: None,
        refused: vec![],
        projects: None,
        run_as: None,
    }
}

pub fn agent_row(id: &str) -> wsp_wire::types::agents_report::AgentRow {
    serde_json::from_value(json!({
        "id": id, "name": id, "installed": true, "road": "own", "signIn": "signed-in", "signInRoad": "device", "wspTools": false,
    }))
    .expect("an agent row")
}

pub fn place_row(id: &str, name: &str, kind: &str) -> PlaceView {
    serde_json::from_value(json!({ "id": id, "name": name, "kind": kind, "default": false }))
        .expect("a place row")
}

pub fn place_row_with(id: &str, name: &str, over: Value) -> PlaceView {
    let mut v = json!({ "id": id, "name": name, "kind": "computer", "default": false });
    for (k, x) in over.as_object().expect("an object").clone() {
        v[k] = x;
    }
    serde_json::from_value(v).expect("a place row")
}

pub fn report(over: Value) -> PlaceReport {
    let mut v = json!({
        "name": "spoo", "platform": "linux", "arch": "x64", "os": "Ubuntu 24.04", "shape": { "cpu": 4, "memMb": 4096 },
        "login": { "HOME": "/home/ada", "USER": "ada", "PATH": "/usr/bin" }, "runsWorkspaces": true, "engine": "none",
        "daemonVersion": 17, "agents": [], "wsp": [], "dialed": "http://192.168.1.20:14621",
    });
    for (k, x) in over.as_object().expect("an object").clone() {
        v[k] = x;
    }
    serde_json::from_value(v).expect("a report")
}

pub fn row(v: Value) -> PlaceProvisionRow {
    serde_json::from_value(v).expect("a provision row")
}

pub async fn tick() {
    tokio::time::sleep(Duration::from_millis(5)).await;
}

/// Polls until the condition holds, as test/until.ts does.
pub async fn until(f: impl Fn() -> bool) {
    for _ in 0..3000 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    panic!("until: the condition never held");
}

// ---------------------------------------------------------------------------------------------------------------
// The state every fake shares.

/// What a sign-in's plan does once it is handed its run.
#[derive(Clone, Debug)]
pub enum Plan {
    /// The TS test's plan: a frame down the link, an event up, the code writer handed over, wait for finish or stop,
    /// the writer taken back, then signed-in.
    Standard,
    /// Waits for finish or stop and says nothing, handing no code writer.
    Quiet,
    /// Returns this error at once.
    Throws(String),
    /// Sends `{cols, rows}` with the op, adds two readers and removes one, records the link's `bare` and `closed`, waits
    /// for the link to close.
    Link,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SkillCall {
    Search(String, u32),
    Get(String),
    Preview(Value, SkillAsk),
    Add(Value, SkillAddAsk),
    Remove(Value, SkillAsk),
    Toggle(Value, SkillAsk, bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerCall {
    Add(Value, Value),
    Remove(Value, Value),
    Toggle(Value, Value, bool),
}

pub struct State {
    pub log: RefCell<Vec<String>>,
    pub host_log: RefCell<Vec<String>>,
    pub warned: Warned,
    pub clock: Rc<TestClock>,
    pub fails: RefCell<IndexMap<String, HostError>>,

    pub phase: Cell<WorkspacePhase>,
    pub local: Cell<bool>,
    pub pause_mode: Cell<Option<PauseMode>>,
    pub brokered: Cell<Option<bool>>,
    pub ws_stores: RefCell<Option<IndexMap<String, String>>>,
    pub ws_seeds: RefCell<Option<IndexMap<String, String>>>,
    pub machine: Arc<FakeMachine>,
    pub workspace_origins: RefCell<Vec<Option<Caller>>>,
    pub relayed: Cell<Option<bool>>,
    pub latest: Cell<Option<bool>>,
    pub stores_here: RefCell<Option<Result<IndexMap<String, String>, HostError>>>,
    pub place_stores: RefCell<Option<IndexMap<String, String>>>,
    pub held: RefCell<Option<IndexMap<String, Vec<AgentsProject>>>>,
    pub setups: RefCell<Option<IndexMap<(String, String), AgentSetupView>>>,
    pub setup_writes: RefCell<Vec<(String, String, Value)>>,
    pub bytes: Cell<u8>,

    pub reader_answer: RefCell<AgentsRead>,
    pub read_calls: RefCell<Vec<(AgentsOn, bool)>>,
    pub tools_calls: RefCell<Vec<(AgentsOn, ServerToolsAsk)>>,
    pub server_calls: RefCell<Vec<(AgentsOn, String, String)>>,
    pub forgot: RefCell<Vec<String>>,
    pub has_server: Cell<bool>,

    pub door_on: Cell<bool>,
    pub rows: RefCell<Vec<PlaceView>>,
    pub report: RefCell<Option<PlaceReport>>,
    pub sign_ins: RefCell<Option<IndexMap<String, AgentSignInState>>>,
    pub gh_steps: RefCell<Vec<(String, Option<String>, Option<PlaceProvisionRow>)>>,
    pub gh_rows: RefCell<Vec<PlaceProvisionRow>>,
    pub list_now: RefCell<Vec<i64>>,
    pub land_waits: Cell<bool>,
    pub landed: RefCell<Vec<String>>,

    pub planned: RefCell<Vec<(Value, SignInAsk)>>,
    pub plan: RefCell<Plan>,
    pub refuse_agents: RefCell<Vec<String>>,
    pub handed: RefCell<Vec<Option<Rc<dyn SignInForward>>>>,
    pub codes: RefCell<Vec<String>>,
    pub finish: RefCell<Rc<Notify>>,
    pub op_replies: RefCell<Vec<Value>>,
    pub link_seen: RefCell<Vec<String>>,
    pub readers_a: RefCell<Vec<Value>>,
    pub readers_b: RefCell<Vec<Value>>,

    pub frames: RefCell<Vec<Frame>>,
    pub shut: Cell<usize>,
    pub opened: RefCell<Vec<(AgentsTarget, Option<Caller>)>>,
    pub push: RefCell<Option<Rc<dyn Fn(DaemonEvent)>>>,
    pub ends: RefCell<Vec<Rc<End>>>,

    pub changed: RefCell<Vec<Option<AgentsTarget>>>,
    pub skill_calls: RefCell<Vec<SkillCall>>,
    pub server_writes: RefCell<Vec<ServerCall>>,
    pub plugin_calls: RefCell<Vec<(Value, PluginAsk, bool)>>,
    pub acted: RefCell<Vec<AgentsOn>>,
}

impl State {
    pub fn say(&self, line: impl Into<String>) {
        self.log.borrow_mut().push(line.into());
    }

    pub fn failure(&self, name: &str) -> Option<HostError> {
        self.fails.borrow().get(name).cloned()
    }
}

/// One channel's end: set where the channel closes, awaited by `closed`.
#[derive(Default)]
pub struct End {
    pub done: Cell<bool>,
    pub note: Notify,
}

impl End {
    pub fn finish(&self) {
        self.done.set(true);
        self.note.notify_waiters();
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The rig.

pub struct Rig {
    pub s: Rc<State>,
}

impl std::ops::Deref for Rig {
    type Target = State;
    fn deref(&self) -> &State {
        &self.s
    }
}

impl Rig {
    pub fn new() -> Rig {
        Rig::at(T0)
    }

    pub fn at(now: i64) -> Rig {
        let s = Rc::new(State {
            log: RefCell::new(vec![]),
            host_log: RefCell::new(vec![]),
            warned: Warned::default(),
            clock: TestClock::at(now),
            fails: RefCell::new(IndexMap::new()),
            phase: Cell::new(WorkspacePhase::Running),
            local: Cell::new(false),
            pause_mode: Cell::new(None),
            brokered: Cell::new(None),
            ws_stores: RefCell::new(None),
            ws_seeds: RefCell::new(None),
            machine: Arc::new(FakeMachine {
                id: "m_landing".into(),
            }),
            workspace_origins: RefCell::new(vec![]),
            relayed: Cell::new(None),
            latest: Cell::new(None),
            stores_here: RefCell::new(None),
            place_stores: RefCell::new(None),
            held: RefCell::new(None),
            setups: RefCell::new(None),
            setup_writes: RefCell::new(vec![]),
            bytes: Cell::new(0),
            reader_answer: RefCell::new(read_answer()),
            read_calls: RefCell::new(vec![]),
            tools_calls: RefCell::new(vec![]),
            server_calls: RefCell::new(vec![]),
            forgot: RefCell::new(vec![]),
            has_server: Cell::new(true),
            door_on: Cell::new(false),
            rows: RefCell::new(vec![]),
            report: RefCell::new(None),
            sign_ins: RefCell::new(None),
            gh_steps: RefCell::new(vec![]),
            gh_rows: RefCell::new(vec![]),
            list_now: RefCell::new(vec![]),
            land_waits: Cell::new(false),
            landed: RefCell::new(vec![]),
            planned: RefCell::new(vec![]),
            plan: RefCell::new(Plan::Standard),
            refuse_agents: RefCell::new(vec!["opencode".to_string()]),
            handed: RefCell::new(vec![]),
            codes: RefCell::new(vec![]),
            finish: RefCell::new(Rc::new(Notify::new())),
            op_replies: RefCell::new(vec![]),
            link_seen: RefCell::new(vec![]),
            readers_a: RefCell::new(vec![]),
            readers_b: RefCell::new(vec![]),
            frames: RefCell::new(vec![]),
            shut: Cell::new(0),
            opened: RefCell::new(vec![]),
            push: RefCell::new(None),
            ends: RefCell::new(vec![]),
            changed: RefCell::new(vec![]),
            skill_calls: RefCell::new(vec![]),
            server_writes: RefCell::new(vec![]),
            plugin_calls: RefCell::new(vec![]),
            acted: RefCell::new(vec![]),
        });
        Rig { s }
    }

    /// One joined computer, spoo, as the place door lists it, with the door standing.
    pub fn with_spoo(&self) {
        self.door_on.set(true);
        self.rows
            .borrow_mut()
            .push(place_row("pl_1", "spoo", "computer"));
    }

    /// The projects the host holds on a computer; wires `projects` in the options.
    pub fn hold(&self, place: &str, projects: Vec<AgentsProject>) {
        self.held
            .borrow_mut()
            .get_or_insert_with(IndexMap::new)
            .insert(place.to_string(), projects);
    }

    pub fn fail(&self, name: &str, message: &str) {
        self.fails
            .borrow_mut()
            .insert(name.to_string(), HostError::new(message));
    }

    pub fn lines(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    /// The log lines that start with one of the prefixes, in order.
    pub fn lines_of(&self, prefixes: &[&str]) -> Vec<String> {
        self.log
            .borrow()
            .iter()
            .filter(|l| prefixes.iter().any(|p| l.starts_with(p)))
            .cloned()
            .collect()
    }

    pub fn clear_log(&self) {
        self.log.borrow_mut().clear();
    }

    pub fn changed_json(&self) -> Vec<Value> {
        self.changed
            .borrow()
            .iter()
            .map(|t| t.as_ref().map(js).unwrap_or(Value::Null))
            .collect()
    }

    pub fn asked_on(&self) -> Vec<Value> {
        self.read_calls
            .borrow()
            .iter()
            .map(|(on, _)| on_json(on))
            .collect()
    }

    pub fn push_event(&self, e: Value) {
        let f = self.push.borrow().clone().expect("a channel is open");
        f(e.as_object().expect("an object").clone());
    }

    pub fn finish_plan(&self) {
        self.finish.borrow().notify_one();
    }

    pub fn end_channel_from_daemon(&self) {
        self.ends
            .borrow()
            .last()
            .expect("a channel is open")
            .finish();
    }

    pub fn follower(&self) -> Seen {
        Seen::new()
    }

    pub fn reads(&self) -> Rc<AgentsReads> {
        AgentsReads::new(self.options())
    }

    pub fn options(&self) -> AgentsReadOptions {
        let s = self.s.clone();
        let reader: Rc<dyn AgentsReader> = Rc::new(Reader(s.clone()));
        let acts: Rc<dyn AgentsActs> = Rc::new(Acts(s.clone()));
        let skills: Rc<dyn SkillsActs> = Rc::new(Skills(s.clone()));
        let servers: Rc<dyn ServersActs> = Rc::new(Servers(s.clone()));
        let plugins: Rc<dyn PluginsActs> = Rc::new(Plugins(s.clone()));
        let icons: Rc<dyn ServerIcons> = Rc::new(Icons(s.clone()));
        let places = {
            let s = s.clone();
            Rc::new(move || {
                if s.door_on.get() {
                    Some(Rc::new(Door(s.clone())) as Rc<dyn AgentsPlaces>)
                } else {
                    None
                }
            })
        };
        let workspace = {
            let s = s.clone();
            Rc::new(move |id: String, origin: Option<Caller>| {
                let s = s.clone();
                async move {
                    s.say(format!("workspace {id}"));
                    s.workspace_origins.borrow_mut().push(origin);
                    Ok(AgentsWorkspace {
                        name: "landing".into(),
                        phase: s.phase.get(),
                        local: s.local.get(),
                        machine: s.machine.clone(),
                        project: landing(),
                        stores: s.ws_stores.borrow().clone(),
                        seeds: s.ws_seeds.borrow().clone(),
                        brokered: s.brokered.get(),
                        pause_mode: s.pause_mode.get(),
                    })
                }
                .boxed_local()
            })
        };
        let place_stores: Option<Rc<dyn Fn(&str, &str) -> IndexMap<String, String>>> =
            if s.place_stores.borrow().is_some() {
                let s = s.clone();
                Some(Rc::new(move |place: &str, home: &str| {
                    s.say(format!("place_stores {place} {home}"));
                    s.place_stores.borrow().clone().unwrap_or_default()
                }))
            } else {
                None
            };
        let stores_here: Option<
            Rc<dyn Fn() -> LocalBoxFuture<'static, Result<IndexMap<String, String>, HostError>>>,
        > = if s.stores_here.borrow().is_some() {
            let s = s.clone();
            Some(Rc::new(move || {
                let s = s.clone();
                async move {
                    s.say("stores_here");
                    s.stores_here.borrow().clone().expect("wired")
                }
                .boxed_local()
            }))
        } else {
            None
        };
        let projects: Option<
            Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<Vec<AgentsProject>, HostError>>>,
        > = if s.held.borrow().is_some() {
            let s = s.clone();
            Some(Rc::new(move |place: String| {
                let s = s.clone();
                async move {
                    s.say(format!("projects.list {place}"));
                    Ok(s.held
                        .borrow()
                        .as_ref()
                        .and_then(|h| h.get(&place).cloned())
                        .unwrap_or_default())
                }
                .boxed_local()
            }))
        } else {
            None
        };
        let latest_on: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, bool>>> =
            s.latest.get().map(|v| {
                Rc::new(move || async move { v }.boxed_local())
                    as Rc<dyn Fn() -> LocalBoxFuture<'static, bool>>
            });
        let channel = {
            let s = s.clone();
            Rc::new(
                move |t: AgentsTarget,
                      on_event: Rc<dyn Fn(DaemonEvent)>,
                      origin: Option<Caller>| {
                    let s = s.clone();
                    async move {
                        s.say(format!("channel.open {}", js(&t)));
                        s.opened.borrow_mut().push((t, origin));
                        *s.push.borrow_mut() = Some(on_event);
                        let end = Rc::new(End::default());
                        s.ends.borrow_mut().push(end.clone());
                        Ok(Rc::new(Channel { s: s.clone(), end }) as Rc<dyn DaemonChannel>)
                    }
                    .boxed_local()
                },
            )
        };
        let changed = {
            let s = s.clone();
            Rc::new(move |t: Option<AgentsTarget>| {
                s.say(format!(
                    "changed {}",
                    t.as_ref()
                        .map(|t| js(t).to_string())
                        .unwrap_or_else(|| "all".into())
                ));
                s.changed.borrow_mut().push(t);
            })
        };
        let relayed: Option<Rc<dyn Fn() -> bool>> = s
            .relayed
            .get()
            .map(|v| Rc::new(move || v) as Rc<dyn Fn() -> bool>);
        let log = {
            let s = s.clone();
            Rc::new(move |line: &str| {
                s.say(format!("log: {line}"));
                s.host_log.borrow_mut().push(line.to_string());
            })
        };
        let setup_of: Option<Rc<dyn Fn(&str, &str) -> Option<AgentSetupView>>> =
            if s.setups.borrow().is_some() {
                let s = s.clone();
                Some(Rc::new(move |place: &str, agent: &str| {
                    s.say(format!("setup_of {place} {agent}"));
                    s.setups
                        .borrow()
                        .as_ref()
                        .and_then(|m| m.get(&(place.to_string(), agent.to_string())).cloned())
                }))
            } else {
                None
            };
        let setup_write = {
            let s = s.clone();
            Rc::new(move |place: String, agent: String, change: AgentSetupSet| {
                let s = s.clone();
                async move {
                    s.say(format!("setup_write {place} {agent} {}", js(&change)));
                    s.setup_writes
                        .borrow_mut()
                        .push((place, agent, js(&change)));
                    match s.failure("setup_write") {
                        Some(e) => Err(e),
                        None => Ok(()),
                    }
                }
                .boxed_local()
            })
        };
        let random_bytes = {
            let s = s.clone();
            Rc::new(move |n: usize| {
                s.say(format!("random_bytes {n}"));
                let k = s.bytes.get() + 1;
                s.bytes.set(k);
                let mut b = vec![k, 0xbe, 0xef, 0x00, 0x10, 0xff];
                b.truncate(n);
                b
            })
        };
        let warn = {
            let s = s.clone();
            Rc::new(move |line: &str| {
                s.say(format!("warn: {line}"));
                s.warned.push(line);
            })
        };
        let clock: Rc<dyn Clock> = s.clock.clone();
        AgentsReadOptions {
            reader: Some(reader),
            places,
            workspace,
            place_stores,
            stores_here,
            projects,
            acts: Some(acts),
            skills: Some(skills),
            servers: Some(servers),
            plugins: Some(plugins),
            icons: Some(icons),
            latest_on,
            channel,
            changed,
            clock,
            relayed,
            log: Some(log),
            setup_of,
            setup_write: Some(setup_write),
            random_bytes,
            warn,
        }
    }
}

/// The id the fake's `random_bytes` gives the n-th sign-in (1-based): `si_` and the six bytes in hex.
pub fn sign_in_id(n: u8) -> String {
    format!("si_{n:02x}beef0010ff")
}

/// A follower that keeps every event it is handed, as JSON.
pub struct Seen {
    pub events: Rc<RefCell<Vec<Value>>>,
    pub f: Follower,
}

impl Seen {
    pub fn new() -> Seen {
        let events: Rc<RefCell<Vec<Value>>> = Rc::new(RefCell::new(vec![]));
        let keep = events.clone();
        let f: Follower = Rc::new(move |e: AgentsSignInEvent| keep.borrow_mut().push(js(&e)));
        Seen { events, f }
    }

    pub fn all(&self) -> Vec<Value> {
        self.events.borrow().clone()
    }
}

pub fn waiting(id: &str, url: &str) -> Value {
    json!({ "signInId": id, "state": "waiting", "url": url })
}

pub fn signed_in(id: &str) -> Value {
    json!({ "signInId": id, "state": "signed-in" })
}

// ---------------------------------------------------------------------------------------------------------------
// Where a read runs, as JSON.

pub fn on_json(on: &AgentsOn) -> Value {
    let mut m = Map::new();
    let put = |m: &mut Map<String, Value>, k: &str, v: Value| {
        m.insert(k.to_string(), v);
    };
    match on {
        AgentsOn::Here { projects, stores } => {
            put(&mut m, "kind", json!("here"));
            if let Some(s) = stores {
                put(&mut m, "stores", js(s));
            }
            if let Some(p) = projects {
                put(&mut m, "projects", js(p));
            }
        }
        AgentsOn::Box {
            name,
            login,
            sign_ins,
            versions,
            setup_rows,
            logins,
            relayed,
            projects,
            stores,
            ..
        } => {
            put(&mut m, "kind", json!("box"));
            if let Some(n) = name {
                put(&mut m, "name", json!(n));
            }
            let mut l = Map::new();
            if let Some(h) = &login.home {
                l.insert("HOME".into(), json!(h));
            }
            if let Some(p) = &login.path {
                l.insert("PATH".into(), json!(p));
            }
            put(&mut m, "login", Value::Object(l));
            if let Some(v) = sign_ins {
                put(&mut m, "signIns", js(v));
            }
            if let Some(v) = versions {
                put(&mut m, "versions", js(v));
            }
            if let Some(v) = setup_rows {
                put(&mut m, "setupRows", js(v));
            }
            if let Some(v) = logins {
                put(&mut m, "logins", json!(v));
            }
            if let Some(v) = relayed {
                put(&mut m, "relayed", json!(v));
            }
            if let Some(p) = projects {
                put(&mut m, "projects", js(p));
            }
            if let Some(s) = stores {
                put(&mut m, "stores", js(s));
            }
        }
        AgentsOn::Machine {
            machine,
            projects,
            relayed,
            brokered,
            stores,
            seeds,
        } => {
            put(&mut m, "kind", json!("machine"));
            put(&mut m, "machine", json!(machine.id()));
            if let Some(p) = projects {
                put(&mut m, "projects", js(p));
            }
            if let Some(v) = relayed {
                put(&mut m, "relayed", json!(v));
            }
            if let Some(v) = brokered {
                put(&mut m, "brokered", json!(v));
            }
            if let Some(s) = stores {
                put(&mut m, "stores", js(s));
            }
            if let Some(s) = seeds {
                put(&mut m, "seeds", js(s));
            }
        }
    }
    Value::Object(m)
}

pub fn on_kind(on: &AgentsOn) -> &'static str {
    match on {
        AgentsOn::Here { .. } => "here",
        AgentsOn::Box { .. } => "box",
        AgentsOn::Machine { .. } => "machine",
    }
}

pub fn on_projects(on: &AgentsOn) -> Option<Vec<AgentsProject>> {
    match on {
        AgentsOn::Here { projects, .. }
        | AgentsOn::Box { projects, .. }
        | AgentsOn::Machine { projects, .. } => projects.clone(),
    }
}

pub fn machine_on(rig: &Rig, relayed: Option<bool>) -> AgentsOn {
    AgentsOn::Machine {
        machine: rig.machine.clone(),
        projects: None,
        relayed,
        brokered: None,
        stores: None,
        seeds: None,
    }
}

pub fn box_on(relayed: Option<bool>) -> AgentsOn {
    AgentsOn::Box {
        name: None,
        exec: Rc::new(|_cmd, _timeout, _stdin| {
            async { Err(HostError::new("no exec on this road")) }.boxed_local()
        }),
        login: BoxLogin::default(),
        sign_ins: None,
        versions: None,
        setup_rows: None,
        logins: None,
        relayed,
        projects: None,
        stores: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The workspace's machine.

pub struct FakeMachine {
    pub id: String,
}

fn nothing() -> EngineError {
    EngineError::Other {
        message: "the fake machine runs nothing".to_string(),
        code: None,
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> EResult<ExecResult> {
        Ok(
            serde_json::from_value(json!({ "exitCode": 0, "stdout": "", "stderr": "" }))
                .expect("an exec result"),
        )
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> EResult<ExecResult> {
        Err(nothing())
    }
    async fn snapshot(
        &self,
        _name: &str,
        _life: MachineLife,
        _opts: SnapshotOptions,
    ) -> EResult<String> {
        Err(nothing())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EResult<()> {
        Err(nothing())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> EResult<()> {
        Err(nothing())
    }
    async fn kill(&self) -> EResult<()> {
        Err(nothing())
    }
    async fn state(&self) -> EResult<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> EResult<String> {
        Err(nothing())
    }
    async fn upload_url(&self, _path: &str) -> EResult<String> {
        Err(nothing())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The reader.

pub struct Reader(pub Rc<State>);

#[async_trait(?Send)]
impl AgentsReader for Reader {
    async fn read(&self, on: &AgentsOn, latest: bool) -> Result<AgentsRead, HostError> {
        self.0
            .say(format!("reader.read {} latest={latest}", on_kind(on)));
        self.0.read_calls.borrow_mut().push((on.clone(), latest));
        match self.0.failure("reader.read") {
            Some(e) => Err(e),
            None => Ok(self.0.reader_answer.borrow().clone()),
        }
    }

    fn forget(&self, key: &str) {
        self.0.say(format!("reader.forget {key}"));
        self.0.forgot.borrow_mut().push(key.to_string());
    }

    async fn tools(
        &self,
        on: &AgentsOn,
        ask: ServerToolsAsk,
    ) -> Result<ServerToolsAnswer, HostError> {
        self.0
            .say(format!("reader.tools {} {}", on_kind(on), ask.key));
        self.0.acted.borrow_mut().push(on.clone());
        self.0.tools_calls.borrow_mut().push((on.clone(), ask));
        Ok(
            serde_json::from_value(json!({ "auth": "open", "tools": [], "readAt": READ_AT }))
                .expect("an answer"),
        )
    }

    fn has_server(&self) -> bool {
        self.0.has_server.get()
    }

    async fn server(
        &self,
        on: &AgentsOn,
        agent: &str,
        name: &str,
    ) -> Result<ResolvedServer, HostError> {
        self.0
            .say(format!("reader.server {} {agent} {name}", on_kind(on)));
        self.0
            .server_calls
            .borrow_mut()
            .push((on.clone(), agent.to_string(), name.to_string()));
        Ok(ResolvedServer {
            spec: Rc::new(
                serde_json::from_value::<wsp_wire::views::harness::McpServerSpec>(
                    json!({ "command": "npx", "args": ["-y", name] }),
                )
                .expect("a spec"),
            ),
            login: None,
        })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The place door.

pub struct Door(pub Rc<State>);

#[async_trait(?Send)]
impl AgentsPlaces for Door {
    async fn list(&self, now: i64) -> Result<Vec<PlaceView>, HostError> {
        self.0.say(format!("door.list now={now}"));
        self.0.list_now.borrow_mut().push(now);
        Ok(self.0.rows.borrow().clone())
    }

    async fn report_of(&self, place_id: &str) -> Result<Option<PlaceReport>, HostError> {
        self.0.say(format!("door.report_of {place_id}"));
        Ok(self.0.report.borrow().clone())
    }

    async fn logins_again(&self, place_id: &str) -> Result<(), HostError> {
        self.0.say(format!("door.logins_again {place_id}"));
        Ok(())
    }

    fn sign_ins_at(
        &self,
        place_id: &str,
        agents: &[String],
    ) -> Option<IndexMap<String, AgentSignInState>> {
        self.0
            .say(format!("door.sign_ins_at {place_id} {}", agents.join(",")));
        self.0.sign_ins.borrow().clone()
    }

    async fn exec(
        &self,
        place_id: &str,
        cmd: &str,
        timeout_ms: Option<u64>,
        stdin: Option<Vec<u8>>,
    ) -> Result<ExecResult, HostError> {
        self.0.say(format!(
            "door.exec {place_id} {cmd} timeout={timeout_ms:?} stdin={stdin:?}"
        ));
        Ok(
            serde_json::from_value(json!({ "exitCode": 0, "stdout": "", "stderr": "" }))
                .expect("an exec result"),
        )
    }

    async fn gh_there(
        &self,
        place_id: &str,
        on_step: GhStep,
    ) -> Result<Vec<PlaceProvisionRow>, HostError> {
        self.0.say(format!("door.gh_there {place_id}"));
        let steps = self.0.gh_steps.borrow().clone();
        for (detail, at, row) in steps {
            on_step(detail, at, row);
        }
        Ok(self.0.gh_rows.borrow().clone())
    }

    async fn key_landed(&self, agent: &str) -> Result<(), HostError> {
        self.land("door.key_landed", agent.to_string(), agent.to_string())
            .await
    }

    async fn login_landed(&self, place_id: &str, agent: &str) -> Result<(), HostError> {
        self.land(
            "door.login_landed",
            format!("{place_id} {agent}"),
            format!("{place_id} {agent}"),
        )
        .await
    }
}

impl Door {
    async fn land(&self, call: &str, said: String, kept: String) -> Result<(), HostError> {
        if self.0.land_waits.get() {
            tick().await;
        }
        let after = self.0.changed.borrow().len();
        self.0.say(format!("{call} {said}"));
        self.0
            .landed
            .borrow_mut()
            .push(format!("{kept} after {after} changes"));
        match self.0.failure("door.landed") {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The channel.

pub struct Channel {
    pub s: Rc<State>,
    pub end: Rc<End>,
}

#[async_trait(?Send)]
impl DaemonChannel for Channel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.s
            .say(format!("channel.send {}", Value::Object(frame.clone())));
        self.s.frames.borrow_mut().push(frame);
        let mut reply = Frame::new();
        reply.insert("ok".into(), json!(true));
        reply.insert("ptyId".into(), json!("pty_1"));
        Ok(reply)
    }

    fn close(&self) {
        self.s.say("channel.close");
        self.s.shut.set(self.s.shut.get() + 1);
        self.end.finish();
    }

    async fn closed(&self) -> ChannelEnd {
        loop {
            if self.end.done.get() {
                return ChannelEnd {
                    code: 1000,
                    reason: String::new(),
                };
            }
            self.end.note.notified().await;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The acts, and the plan a sign-in runs.

pub struct Acts(pub Rc<State>);

#[async_trait(?Send)]
impl AgentsActs for Acts {
    async fn sign_in_line(&self, on: &AgentsOn, ask: &SignInAsk) -> Result<SignInLine, HostError> {
        self.0
            .say(format!("acts.sign_in_line {} {}", on_kind(on), ask.agent));
        Ok(
            serde_json::from_value(json!({ "command": "codex login --device-auth" }))
                .expect("a line"),
        )
    }

    async fn sign_in(&self, on: &AgentsOn, ask: &SignInAsk) -> Result<SignInPlan, HostError> {
        let s = self.0.clone();
        s.say(format!("acts.sign_in {} {}", on_kind(on), ask.agent));
        s.planned.borrow_mut().push((on_json(on), ask.clone()));
        if s.refuse_agents.borrow().contains(&ask.agent) {
            return Err(HostError::new(format!("{} asks you to pick", ask.agent)));
        }
        let agent = ask.agent.clone();
        let plan = s.plan.borrow().clone();
        Ok(Box::new(move |run: SignInRun| {
            run_plan(s, agent, plan, run).boxed_local()
        }))
    }

    async fn key(&self, agent: &str, _key: &str, way: Option<&str>) -> Result<(), HostError> {
        self.0.say(format!("acts.key {agent} way={way:?}"));
        match self.0.failure("acts.key") {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn add_tools(&self, on: &AgentsOn, agent: &str) -> Result<String, HostError> {
        self.0
            .say(format!("acts.add_tools {} {agent}", on_kind(on)));
        match self.0.failure("acts.add_tools") {
            Some(e) => Err(e),
            None => Ok("~/.codex/config.toml".to_string()),
        }
    }
}

async fn run_plan(
    s: Rc<State>,
    agent: String,
    plan: Plan,
    run: SignInRun,
) -> Result<(), HostError> {
    s.say(format!("plan.start {agent}"));
    s.handed.borrow_mut().push(run.forward.clone());
    let finish = Rc::new(Notify::new());
    *s.finish.borrow_mut() = finish.clone();
    let wait = |run: &SignInRun| {
        let notified = Box::pin({
            let finish = finish.clone();
            async move { finish.notified().await }
        });
        select(notified, run.stop.clone())
    };
    let step = |state: AgentsSignInEventState| SignInStep {
        state,
        url: None,
        code: None,
        paste: None,
        said: None,
        pty_id: None,
    };
    match plan {
        Plan::Throws(m) => Err(HostError::new(m)),
        Plan::Quiet => {
            wait(&run).await;
            Ok(())
        }
        Plan::Standard => {
            let reply = (run.link.op)("pty.create".to_string(), Map::new()).await?;
            s.op_replies.borrow_mut().push(Value::Object(reply));
            let emit = run.emit.clone();
            let _off = (run.link.on_event)(Rc::new(move |e: &Map<String, Value>| {
                let url = e.get("url").map(|u| {
                    u.as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| u.to_string())
                });
                emit(SignInStep {
                    url,
                    ..step(AgentsSignInEventState::Waiting)
                });
            }));
            let codes = s.clone();
            let writer: CodeWriter = Rc::new(move |code: String| {
                let codes = codes.clone();
                async move {
                    codes.codes.borrow_mut().push(code);
                    Ok(())
                }
                .boxed_local()
            });
            (run.typing)(Some(writer));
            wait(&run).await;
            (run.typing)(None);
            (run.emit)(step(AgentsSignInEventState::SignedIn));
            Ok(())
        }
        Plan::Link => {
            let mut extra = Map::new();
            extra.insert("cols".into(), json!(80));
            extra.insert("rows".into(), json!(24));
            let reply = (run.link.op)("pty.create".to_string(), extra).await?;
            s.op_replies.borrow_mut().push(Value::Object(reply));
            let a = s.clone();
            let _keep_a = (run.link.on_event)(Rc::new(move |e: &Map<String, Value>| {
                a.readers_a.borrow_mut().push(Value::Object(e.clone()))
            }));
            let b = s.clone();
            let off_b = (run.link.on_event)(Rc::new(move |e: &Map<String, Value>| {
                b.readers_b.borrow_mut().push(Value::Object(e.clone()))
            }));
            off_b();
            s.link_seen
                .borrow_mut()
                .push(format!("bare={:?}", run.link.bare));
            s.link_seen
                .borrow_mut()
                .push(format!("closed={}", run.link.closed.is_some()));
            if let Some(closed) = run.link.closed.clone() {
                closed.await;
                s.link_seen.borrow_mut().push("link closed".to_string());
            }
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Skills, servers, plugins, icons.

pub struct Skills(pub Rc<State>);

fn skill_ask_json(a: &SkillAsk) -> Value {
    let mut v = json!({ "name": a.name });
    if let Some(p) = a.project {
        v["project"] = json!(p);
    }
    v
}

#[async_trait(?Send)]
impl SkillsActs for Skills {
    async fn search(&self, q: &str, limit: u32) -> Result<Vec<SkillHit>, HostError> {
        self.0.say(format!("skills.search {q} {limit}"));
        self.0
            .skill_calls
            .borrow_mut()
            .push(SkillCall::Search(q.into(), limit));
        Ok(vec![serde_json::from_value(json!({ "id": "a/b/pdf", "source": "a/b", "skillId": "pdf", "name": "pdf", "installs": 3 })).expect("a hit")])
    }

    async fn get(&self, skill: &str) -> Result<SkillPreview, HostError> {
        self.0.say(format!("skills.get {skill}"));
        self.0
            .skill_calls
            .borrow_mut()
            .push(SkillCall::Get(skill.into()));
        Ok(serde_json::from_value(json!({ "text": "# pdf", "size": 5 })).expect("a preview"))
    }

    async fn preview(&self, on: &AgentsOn, ask: &SkillAsk) -> Result<SkillPreview, HostError> {
        self.0.say(format!(
            "skills.preview {} {}",
            on_kind(on),
            skill_ask_json(ask)
        ));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .skill_calls
            .borrow_mut()
            .push(SkillCall::Preview(on_json(on), ask.clone()));
        Ok(serde_json::from_value(json!({ "text": "# pdf", "size": 5 })).expect("a preview"))
    }

    async fn add(&self, on: &AgentsOn, ask: &SkillAddAsk) -> Result<SkillAdded, HostError> {
        self.0
            .say(format!("skills.add {} {}", on_kind(on), ask.skill));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .skill_calls
            .borrow_mut()
            .push(SkillCall::Add(on_json(on), ask.clone()));
        match self.0.failure("skills.add") {
            Some(e) => Err(e),
            None => Ok(serde_json::from_value(
                json!({ "path": "~/.agents/skills/pdf", "agents": [] }),
            )
            .expect("added")),
        }
    }

    async fn remove(&self, on: &AgentsOn, ask: &SkillAsk) -> Result<Vec<String>, HostError> {
        self.0.say(format!(
            "skills.remove {} {}",
            on_kind(on),
            skill_ask_json(ask)
        ));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .skill_calls
            .borrow_mut()
            .push(SkillCall::Remove(on_json(on), ask.clone()));
        match self.0.failure("skills.remove") {
            Some(e) => Err(e),
            None => Ok(vec!["~/.agents/skills/pdf".to_string()]),
        }
    }

    async fn toggle(
        &self,
        on: &AgentsOn,
        ask: &SkillAsk,
        to: bool,
    ) -> Result<Vec<String>, HostError> {
        self.0.say(format!(
            "skills.toggle {} {} to={to}",
            on_kind(on),
            skill_ask_json(ask)
        ));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .skill_calls
            .borrow_mut()
            .push(SkillCall::Toggle(on_json(on), ask.clone(), to));
        match self.0.failure("skills.toggle") {
            Some(e) => Err(e),
            None => Ok(vec!["~/.agents/skills/pdf".to_string()]),
        }
    }
}

pub struct Servers(pub Rc<State>);

#[async_trait(?Send)]
impl ServersActs for Servers {
    async fn add(&self, on: &AgentsOn, ask: &ServerAdd) -> Result<String, HostError> {
        self.0
            .say(format!("servers.add {} {}", on_kind(on), ask.name));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .server_writes
            .borrow_mut()
            .push(ServerCall::Add(on_json(on), js(ask)));
        match self.0.failure("servers.add") {
            Some(e) => Err(e),
            None => Ok("~/.claude.json".to_string()),
        }
    }

    async fn remove(&self, on: &AgentsOn, ask: &ServerAsk) -> Result<String, HostError> {
        self.0
            .say(format!("servers.remove {} {}", on_kind(on), ask.name));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .server_writes
            .borrow_mut()
            .push(ServerCall::Remove(on_json(on), js(ask)));
        match self.0.failure("servers.remove") {
            Some(e) => Err(e),
            None => Ok("~/.claude.json".to_string()),
        }
    }

    async fn toggle(&self, on: &AgentsOn, ask: &ServerAsk, to: bool) -> Result<String, HostError> {
        self.0.say(format!(
            "servers.toggle {} {} to={to}",
            on_kind(on),
            ask.name
        ));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .server_writes
            .borrow_mut()
            .push(ServerCall::Toggle(on_json(on), js(ask), to));
        match self.0.failure("servers.toggle") {
            Some(e) => Err(e),
            None => Ok("~/.codex/config.toml".to_string()),
        }
    }
}

pub fn plugin_row() -> PluginRow {
    serde_json::from_value(json!({
        "agent": "claude", "id": "pdf-tools@acme", "name": "pdf-tools", "marketplace": "acme", "scope": "user", "on": false,
        "brings": { "skills": ["pdf"], "commands": [], "subagents": [], "hooks": [], "servers": [], "lsp": [], "apps": [] },
    }))
    .expect("a plugin row")
}

pub struct Plugins(pub Rc<State>);

#[async_trait(?Send)]
impl PluginsActs for Plugins {
    async fn toggle(
        &self,
        on: &AgentsOn,
        ask: &PluginAsk,
        to: bool,
    ) -> Result<PluginRow, HostError> {
        self.0.say(format!(
            "plugins.toggle {} {} to={to}",
            on_kind(on),
            ask.plugin
        ));
        self.0.acted.borrow_mut().push(on.clone());
        self.0
            .plugin_calls
            .borrow_mut()
            .push((on_json(on), ask.clone(), to));
        match self.0.failure("plugins.toggle") {
            Some(e) => Err(e),
            None => Ok(plugin_row()),
        }
    }
}

pub struct Icons(pub Rc<State>);

pub const ICON: &str = "data:image/png;base64,AAAA";

#[async_trait(?Send)]
impl ServerIcons for Icons {
    fn folder(&self) -> String {
        "/root/.wsp/icons".to_string()
    }

    async fn icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError> {
        self.0.say(format!("icons.icon {host} refresh={refresh:?}"));
        Ok(Some(ICON.to_string()))
    }

    fn forget(&self) {
        self.0.say("icons.forget");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The relay and its forwards.

pub struct FakeForward {
    pub n: usize,
    pub s: Rc<State>,
    pub closed: Rc<RefCell<Vec<usize>>>,
}

#[async_trait(?Send)]
impl SignInForward for FakeForward {
    async fn arm(&self, _url: &str) -> Result<bool, HostError> {
        Ok(true)
    }
    async fn deliver(&self, _landed: &str) -> Result<(), HostError> {
        Ok(())
    }
    fn close(&self) {
        self.s.say(format!("forward.close {}", self.n));
        self.closed.borrow_mut().push(self.n);
    }
}

/// A relay that holds a link to one computer.
pub struct FakeRelay {
    pub place: String,
    pub s: Rc<State>,
    pub asked: RefCell<Vec<Value>>,
    pub opened: RefCell<Vec<Rc<FakeForward>>>,
    pub closed: Rc<RefCell<Vec<usize>>>,
}

impl FakeRelay {
    pub fn new(rig: &Rig, place: &str) -> Rc<FakeRelay> {
        Rc::new(FakeRelay {
            place: place.to_string(),
            s: rig.s.clone(),
            asked: RefCell::new(vec![]),
            opened: RefCell::new(vec![]),
            closed: Rc::new(RefCell::new(vec![])),
        })
    }

    fn ours(&self, t: &AgentsTarget) -> bool {
        self.asked.borrow_mut().push(js(t));
        matches!(t, AgentsTarget::V0(v) if v.place_id == self.place)
    }
}

impl CallbackForwards for FakeRelay {
    fn reaches(&self, target: &AgentsTarget) -> bool {
        self.ours(target)
    }

    fn open(&self, target: &AgentsTarget) -> Option<Rc<dyn SignInForward>> {
        if !self.ours(target) {
            return None;
        }
        let forward = Rc::new(FakeForward {
            n: self.opened.borrow().len() + 1,
            s: self.s.clone(),
            closed: self.closed.clone(),
        });
        self.opened.borrow_mut().push(forward.clone());
        Some(forward)
    }
}

/// Two forwards are the same where they are one allocation.
pub fn same_forward(a: &Option<Rc<dyn SignInForward>>, b: &Rc<FakeForward>) -> bool {
    match a {
        Some(a) => Rc::as_ptr(a) as *const () == Rc::as_ptr(b) as *const (),
        None => false,
    }
}

pub fn no_follower() -> Follower {
    Rc::new(|_e: AgentsSignInEvent| {})
}

/// The TS `finished` helper: start a sign-in, let its plan run, finish it, and wait until the end said changed.
pub async fn finished(rig: &Rig, api: &Rc<AgentsReads>, t: AgentsTarget, ask: SignInAsk) {
    let before = rig.changed.borrow().len();
    api.sign_in(t, ask, no_follower(), None)
        .await
        .expect("a sign-in");
    tick().await;
    rig.finish_plan();
    until(|| rig.changed.borrow().len() > before).await;
}

pub fn ask(agent: &str) -> SignInAsk {
    SignInAsk {
        agent: agent.to_string(),
        ..SignInAsk::default()
    }
}

pub fn ask_server(agent: &str, server: &str) -> SignInAsk {
    SignInAsk {
        agent: agent.to_string(),
        server: Some(server.to_string()),
        ..SignInAsk::default()
    }
}
