// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of spec/account_slates.md: account/slates.ts at 65f6b89c7, driven through a fake `SlatesAreaCtx`. The TS
//! has no test of its own for this file; these pin what the runtime-wide slate tests rely on it for.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::task::LocalSet;

use wsp_engine::machine::Machine;
use wsp_engine::target_line::{Platform, TargetLogin};
use wsp_slates::account::slates::*;
use wsp_slates::slate_mcp::{McpHttpAnswer, McpHttpRequest, McpPipeEvents, McpPost, McpServerSpec, McpSignIn, McpTransport, ToolAsks};
use wsp_slates::slate_mcp_on::StdioRoad;
use wsp_slates::slate_sources::context::{AccountRow, Checkout, PullRequestSeen, SessionView};

fn row(mut v: Value) -> SessionView {
    let o = v.as_object_mut().unwrap();
    o.entry("workspaceId").or_insert(json!("w1"));
    o.entry("harness").or_insert(json!("claude"));
    o.entry("status").or_insert(json!("completed"));
    serde_json::from_value(v).unwrap()
}

#[derive(Clone)]
struct Entry {
    id: String,
    name: String,
    place: Option<String>,
    project: Option<String>,
    folder_on_joined: bool,
    running: Rc<Cell<bool>>,
    local: bool,
    checkout: String,
    computer: String,
    kind_folder: Option<String>,
    rate: Option<f64>,
}

fn entry(id: &str) -> Entry {
    Entry { id: id.into(), name: format!("{id}-box"), place: None, project: None, folder_on_joined: false, running: Rc::new(Cell::new(true)), local: true, checkout: format!("/work/{id}"), computer: "studio".into(), kind_folder: None, rate: None }
}

type ServerAnswer = Rc<dyn Fn(&str) -> Result<ReadServer, ServerReadError>>;

/// A road that records every frame and answers `{}`, or the reply `http.post` gives.
struct Road {
    frames: Rc<RefCell<Vec<Value>>>,
    closed: Shared<LocalBoxFuture<'static, ()>>,
}

impl StdioRoad for Road {
    fn send(&self, frame: Value) -> LocalBoxFuture<'static, Result<Value, String>> {
        let post = frame["op"] == "http.post";
        self.frames.borrow_mut().push(frame);
        async move { Ok(if post { json!({ "status": 200, "body": "{}" }) } else { json!({}) }) }.boxed_local()
    }
    fn listen(&self, _f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()> {
        Box::new(|| {})
    }
    fn closed(&self) -> Shared<LocalBoxFuture<'static, ()>> {
        self.closed.clone()
    }
    fn close(&self) {}
}

#[derive(Default)]
struct Fake {
    latest: HashMap<String, SessionView>,
    records: HashMap<String, ThreadFacts>,
    running: HashMap<String, String>,
    live: HashMap<String, Entry>,
    boxes: HashMap<String, Entry>,
    folder_of: HashMap<(String, String), String>,
    exists: Vec<String>,
    at_provider: Vec<String>,
    asks_seen: RefCell<Vec<(Option<String>, Option<String>)>>,
    server: Option<ServerAnswer>,
    servers_asked: RefCell<Vec<(ServersTarget, String, String)>>,
    reader: Option<Rc<dyn Fn(&str) -> Result<McpServerSpec, String>>>,
    reader_asked: RefCell<Vec<Vec<HereProject>>>,
    roads: RefCell<Vec<(String, Option<i64>)>>,
    frames: Rc<RefCell<Vec<Value>>>,
    woken: RefCell<Vec<String>>,
    boot_waits: RefCell<Vec<String>>,
    settle: String,
    settled: RefCell<Vec<(String, Option<u64>)>>,
    rows: HashMap<String, Vec<SessionView>>,
    transcripts: HashMap<String, Vec<Value>>,
    accounts: Vec<Value>,
    accrued: HashMap<String, f64>,
    starts: RefCell<Vec<DeliverStart>>,
    start_answer: RefCell<Option<LocalBoxFuture<'static, Result<Started, String>>>>,
    queued: RefCell<HashMap<String, Box<dyn Fn()>>>,
    warned: RefCell<Vec<String>>,
    polled: RefCell<Vec<String>>,
    local_env: Option<IndexMap<String, String>>,
    state_folder: Option<String>,
    napped: RefCell<Option<Box<dyn Fn(&str)>>>,
}

impl SlatesAreaCtx for Fake {
    type Entry = Entry;

    fn latest_on(&self, thread: &str) -> Option<SessionView> {
        self.latest.get(thread).cloned()
    }
    fn thread_record(&self, thread: &str) -> Option<ThreadFacts> {
        self.records.get(thread).cloned()
    }
    fn running_turn(&self, thread: &str) -> Option<String> {
        self.running.get(thread).cloned()
    }
    fn live(&self, workspace: &str) -> Option<Entry> {
        self.live.get(workspace).cloned()
    }
    fn box_of(&self, thread: &str) -> Option<Entry> {
        self.boxes.get(thread).cloned()
    }
    fn folder_of(&self, workspace: &str, session: &str) -> Option<String> {
        self.folder_of.get(&(workspace.to_string(), session.to_string())).cloned()
    }
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, fallback: &str) -> String {
        format!("runs_in({}, {}, {fallback})", entry.id, ran_in.unwrap_or("-"))
    }
    fn kind_folder(&self, entry: &Entry) -> Option<String> {
        entry.kind_folder.clone()
    }
    fn checkout_of(&self, entry: &Entry) -> String {
        entry.checkout.clone()
    }
    fn is_local(&self, entry: &Entry) -> bool {
        entry.local
    }
    fn path_exists(&self, path: &str) -> bool {
        self.exists.iter().any(|p| p == path)
    }
    fn home_dir(&self) -> String {
        "/home/me".into()
    }
    fn computer_of(&self, entry: &Entry) -> String {
        entry.computer.clone()
    }
    fn root_of(&self, thread: &str) -> String {
        format!("root-of-{thread}")
    }
    fn tool_asks(&self, harness: Option<&str>, mode: Option<&str>) -> ToolAsks {
        self.asks_seen.borrow_mut().push((harness.map(String::from), mode.map(String::from)));
        if mode == Some("bypassPermissions") { ToolAsks::Never } else { ToolAsks::Every }
    }
    fn entry_id(&self, entry: &Entry) -> String {
        entry.id.clone()
    }
    fn entry_name(&self, entry: &Entry) -> String {
        entry.name.clone()
    }
    fn entry_place(&self, entry: &Entry) -> Option<String> {
        entry.place.clone()
    }
    fn entry_project(&self, entry: &Entry) -> Option<String> {
        entry.project.clone()
    }
    fn entry_folder_on_joined(&self, entry: &Entry) -> bool {
        entry.folder_on_joined
    }
    fn entry_running(&self, entry: &Entry) -> bool {
        entry.running.get()
    }
    fn entry_machine(&self, _entry: &Entry) -> Option<Arc<dyn Machine>> {
        None
    }
    fn entry_checkout(&self, _entry: &Entry) -> Option<Checkout> {
        Some(serde_json::from_value(json!({ "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 3 })).unwrap())
    }
    fn entry_pr(&self, _entry: &Entry) -> Option<PullRequestSeen> {
        None
    }
    fn entry_rate_usd_per_hour(&self, entry: &Entry) -> Option<f64> {
        entry.rate
    }
    fn at_provider(&self, place: &str) -> bool {
        self.at_provider.iter().any(|p| p == place)
    }
    fn boot_work(&self, workspace: &str) -> LocalBoxFuture<'static, ()> {
        self.boot_waits.borrow_mut().push(workspace.to_string());
        async {}.boxed_local()
    }
    fn wake(&self, workspace: &str) -> LocalBoxFuture<'static, Result<(), String>> {
        self.woken.borrow_mut().push(workspace.to_string());
        async { Ok(()) }.boxed_local()
    }
    fn ready(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn settled_now(&self, thread: &str, ms: Option<u64>) -> LocalBoxFuture<'static, ()> {
        self.settled.borrow_mut().push((thread.to_string(), ms));
        async {}.boxed_local()
    }
    fn settle_after(&self) -> LocalBoxFuture<'static, String> {
        let s = self.settle.clone();
        async move { s }.boxed_local()
    }
    fn tree_under(&self, lead: &str) -> Vec<String> {
        vec![lead.to_string(), format!("{lead}-child")]
    }
    fn thread_of_token(&self, token: &str) -> Option<String> {
        (token == "tok").then(|| "t1".to_string())
    }
    fn own_process(&self, _thread: &str, _pid: u32, _kind: &str) -> Box<dyn FnOnce()> {
        Box::new(|| {})
    }
    fn daemon_image(&self, _entry: &Entry, path: String) -> LocalBoxFuture<'static, Result<Value, String>> {
        async move { Ok(json!({ "size": 3, "mediaType": "image/png", "content": "AAAA", "path": path })) }.boxed_local()
    }
    fn daemon_hash(&self, _entry: &Entry, root: String, paths: Vec<String>) -> LocalBoxFuture<'static, Result<Value, String>> {
        async move { Ok(json!({ "files": { "a.sh": format!("{root}:{}", paths.join(",")) } })) }.boxed_local()
    }
    fn agents_server(&self, target: ServersTarget, agent: &str, name: &str) -> LocalBoxFuture<'static, Result<ReadServer, ServerReadError>> {
        self.servers_asked.borrow_mut().push((target, agent.to_string(), name.to_string()));
        let got = (self.server.as_ref().expect("a box server reader"))(name);
        async move { got }.boxed_local()
    }
    fn agents_reader_server(&self, projects: Vec<HereProject>, _agent: &str, name: &str) -> Option<LocalBoxFuture<'static, Result<McpServerSpec, String>>> {
        let reader = self.reader.clone()?;
        self.reader_asked.borrow_mut().push(projects);
        let got = reader(name);
        Some(async move { got }.boxed_local())
    }
    fn stdio_road(&self, entry: &Entry, min_version: Option<i64>) -> LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>> {
        self.roads.borrow_mut().push((entry.id.clone(), min_version));
        let frames = self.frames.clone();
        async move { Ok(Rc::new(Road { frames, closed: futures::future::pending::<()>().boxed_local().shared() }) as Rc<dyn StdioRoad>) }.boxed_local()
    }
    fn place_name_here(&self) -> String {
        "studio".into()
    }
    fn agent_label(&self, agent: &str) -> String {
        if agent == "claude" { "Claude Code".into() } else { agent.into() }
    }
    fn rows_on(&self, thread: &str) -> Vec<SessionView> {
        self.rows.get(thread).cloned().unwrap_or_default()
    }
    fn transcript(&self, workspace: &str) -> Vec<Value> {
        self.transcripts.get(workspace).cloned().unwrap_or_default()
    }
    fn usage_account_key(&self, _entry: &Entry, harness: &str) -> String {
        format!("{harness}|key")
    }
    fn usage_accounts(&self) -> LocalBoxFuture<'static, Vec<AccountRow>> {
        let rows: Vec<AccountRow> = self.accounts.iter().map(|v| serde_json::from_value(v.clone()).unwrap()).collect();
        async move { rows }.boxed_local()
    }
    fn project_name(&self, project: &str) -> String {
        format!("{project}-name")
    }
    fn accrued(&self, workspace: &str) -> Option<f64> {
        self.accrued.get(workspace).copied()
    }
    fn now(&self) -> i64 {
        1_760_000_000_000
    }
    fn start_session(&self, start: DeliverStart) -> LocalBoxFuture<'static, Result<Started, String>> {
        self.starts.borrow_mut().push(start);
        self.start_answer.borrow_mut().take().expect("a start answer")
    }
    fn on_queued(&self, request_id: &str, f: Box<dyn Fn()>) -> Box<dyn FnOnce()> {
        self.queued.borrow_mut().insert(request_id.to_string(), f);
        Box::new(|| {})
    }
    fn warn(&self, line: &str) {
        self.warned.borrow_mut().push(line.to_string());
    }
    fn poll_pull_request(&self, entry: &Entry) {
        self.polled.borrow_mut().push(entry.id.clone());
    }
    fn local_env(&self) -> Option<IndexMap<String, String>> {
        self.local_env.clone()
    }
    fn process_env(&self) -> IndexMap<String, String> {
        let mut m = IndexMap::new();
        m.insert("FROM".to_string(), "process".to_string());
        m
    }
    fn state_folder(&self) -> Option<String> {
        self.state_folder.clone()
    }
    fn on_napped(&self, f: Box<dyn Fn(&str)>) {
        *self.napped.borrow_mut() = Some(f);
    }
}

fn wire(fake: Fake) -> (Rc<Fake>, SlatesWiring<Fake>, Rc<RefCell<Vec<String>>>) {
    let fake = Rc::new(fake);
    let naps = Rc::new(RefCell::new(Vec::new()));
    let n = naps.clone();
    let w = slates_area(fake.clone(), Rc::new(move |ws: &str| n.borrow_mut().push(ws.to_string())));
    (fake, w, naps)
}

fn login() -> TargetLogin {
    TargetLogin { platform: Platform::Linux, home: "/root".into(), path: None, user: "root".into(), run_as: None }
}

fn stdio_spec() -> McpServerSpec {
    let t = McpTransport::Stdio { command: "npx".into(), args: vec!["zones".into()], env: IndexMap::new(), cwd: None, tool_timeout_sec: None };
    McpServerSpec { transport: t, cwd: "/root".into(), env: IndexMap::new(), secrets: vec![], open: None, post: None, sign_in: None }
}

fn sign_in(read: bool) -> McpSignIn {
    McpSignIn {
        bearer: serde_json::from_value(json!({ "file": "~/.claude/.credentials.json", "at": ["mcpOAuth", "k"], "field": "accessToken" })).unwrap(),
        renew: Rc::new(|| async { false }.boxed_local()),
        line: "claude mcp login cloudflare".into(),
        read: if read { Some(Rc::new(|| async { Some("tok".to_string()) }.boxed_local())) } else { None },
    }
}

fn box_fake(thread: &str) -> Fake {
    let mut b = entry("w1");
    b.place = Some("hetzner-id".into());
    b.project = Some("p1".into());
    b.folder_on_joined = true;
    b.local = false;
    b.computer = "hetzner".into();
    let mut f = Fake::default();
    f.latest.insert(thread.into(), row(json!({ "id": "s1", "workspaceId": "w1", "harness": "claude" })));
    f.live.insert("w1".into(), b.clone());
    f.boxes.insert(thread.into(), b);
    f
}

// ---- the thread ----

#[test]
fn thread_without_a_live_entry_names_the_session() {
    // spec rule 4: no live entry, the record's workspace; the session id falls back to the latest row's id, then the
    // thread.
    let mut f = Fake::default();
    f.records.insert("t1".into(), ThreadFacts { workspace_id: Some("w9".into()), harness: Some("codex".into()), permission_mode: Some("bypassPermissions".into()) });
    f.running.insert("t1".into(), "turn-7".into());
    let (fake, w, _) = wire(f);
    assert_eq!(
        w.thread("t1"),
        Some(SlateThread { workspace_id: "w9".into(), root_thread_id: "root-of-t1".into(), session_id: "t1".into(), turn_id: Some("turn-7".into()), folder: None, host_folder: None, computer: None, tool_asks: ToolAsks::Never })
    );
    assert_eq!(*fake.asks_seen.borrow(), vec![(Some("codex".to_string()), Some("bypassPermissions".to_string()))]);
    assert_eq!(w.thread("nobody"), None);
}

#[test]
fn thread_on_a_box_runs_in_its_folder_there_and_here_in_the_checkout_or_home() {
    // spec rule 4: ran_in from the latest row's cwd, else the session's folder; the host folder a local workspace's
    // own folder, else the checkout where it exists here, else the home folder.
    let mut f = box_fake("t1");
    f.latest.insert("t1".into(), row(json!({ "id": "s1", "workspaceId": "w1", "harness": "claude", "claudeSessionId": "c-1", "permissionMode": "default" })));
    f.folder_of.insert(("w1".into(), "c-1".into()), "/root/acme/sub".into());
    let (_fake, w, _) = wire(f);
    let t = w.thread("t1").unwrap();
    assert_eq!(t.session_id, "c-1");
    assert_eq!(t.folder.as_deref(), Some("runs_in(w1, /root/acme/sub, /work/w1)"));
    assert_eq!(t.host_folder.as_deref(), Some("/home/me"));
    assert_eq!(t.computer.as_deref(), Some("hetzner"));
    assert_eq!(t.tool_asks, ToolAsks::Every);

    let mut f = box_fake("t1");
    f.exists.push("/work/w1".into());
    f.latest.insert("t1".into(), row(json!({ "id": "s1", "workspaceId": "w1", "harness": "claude", "cwd": "/root/acme" })));
    let (_fake, w, _) = wire(f);
    let t = w.thread("t1").unwrap();
    assert_eq!(t.folder.as_deref(), Some("runs_in(w1, /root/acme, /work/w1)"));
    assert_eq!(t.host_folder.as_deref(), Some("/work/w1"));

    let mut f = Fake::default();
    let mut here = entry("w2");
    here.kind_folder = Some("/kind/folder".into());
    f.live.insert("w2".into(), here);
    f.latest.insert("t2".into(), row(json!({ "id": "s2", "workspaceId": "w2", "harness": "claude" })));
    let (_fake, w, _) = wire(f);
    let t = w.thread("t2").unwrap();
    assert_eq!(t.folder.as_deref(), Some("runs_in(w2, -, /kind/folder)"));
    assert_eq!(t.host_folder, t.folder, "a local workspace's host folder is its own folder");
}

// ---- the MCP server ----

#[tokio::test(flavor = "current_thread")]
async fn mcp_server_on_a_box_reads_that_computers_config_and_marks_the_thread() {
    // Invariant and spec rule 7: read on the box through its config there, the spec opened there with the thread's
    // mark, its http asked through http.post at the daemon version that has it.
    LocalSet::new()
        .run_until(async {
            let mut f = box_fake("t1");
            let mut with_sign_in = stdio_spec();
            with_sign_in.sign_in = Some(sign_in(false));
            f.server = Some(Rc::new(move |_name: &str| Ok(ReadServer { spec: with_sign_in.clone(), login: Some(login()) })));
            f.reader = Some(Rc::new(|_n: &str| panic!("this computer's config was read for a thread on hetzner")));
            let (fake, w, _) = wire(f);
            let spec = w.mcp_server("t1", "cloudflare").await.unwrap();
            assert_eq!(*fake.servers_asked.borrow(), vec![(ServersTarget::Place { place_id: "hetzner-id".into(), project: Some("p1".into()) }, "claude".to_string(), "cloudflare".to_string())]);
            assert!(fake.reader_asked.borrow().is_empty());
            let open = spec.open.clone().expect("opened there");
            let events = McpPipeEvents { out: Rc::new(|_: &[u8]| {}), err: Rc::new(|_: &str| {}), exit: Rc::new(|_: &str| {}) };
            let _pipe = open(&spec.transport, &spec, events);
            for _ in 0..50 {
                if !fake.frames.borrow().is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            let first = fake.frames.borrow()[0].clone();
            assert_eq!(first["op"], "stdio.open");
            assert_eq!(first["env"], json!({ "WSP_THREAD": "t1", "SLATE_DIR": "/tmp/wsp-slates/t1" }));
            assert!(first["cmd"].as_str().unwrap().starts_with("export HOME='/root'"));
            let post = (spec.post.clone().expect("posted there"))(Rc::new(Unused));
            post.send(&McpTransport::Http { url: "http://127.0.0.1:9/mcp".into(), headers: IndexMap::new() }, McpHttpRequest { headers: IndexMap::new(), body: "{}".into(), ms: 1000 }).await.unwrap();
            assert_eq!(*fake.roads.borrow(), vec![("w1".to_string(), None), ("w1".to_string(), Some(155))]);
            let posted = fake.frames.borrow().iter().find(|f| f["op"] == "http.post").cloned().unwrap();
            assert_eq!(posted["bearer"], json!({ "file": "~/.claude/.credentials.json", "at": ["mcpOAuth", "k"], "field": "accessToken" }));
        })
        .await;
}

/// This computer's post, never reached through a box's road.
struct Unused;

impl McpPost for Unused {
    fn send(&self, _t: &McpTransport, _r: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        panic!("a box thread's server was asked from this computer")
    }
    fn close(&self) {}
}

#[tokio::test(flavor = "current_thread")]
async fn mcp_server_on_a_box_marks_the_thread_and_slate_dir() {
    // Invariant: the mark rides as the last variables too, over the config's own.
    LocalSet::new()
        .run_until(async {
            let mut f = box_fake("t9");
            let mut spec = stdio_spec();
            spec.env.insert("SLATE_DIR".into(), "/elsewhere".into());
            f.server = Some(Rc::new(move |_n: &str| Ok(ReadServer { spec: spec.clone(), login: Some(login()) })));
            let (fake, w, _) = wire(f);
            let spec = w.mcp_server("t9", "zones").await.unwrap();
            let events = McpPipeEvents { out: Rc::new(|_: &[u8]| {}), err: Rc::new(|_: &str| {}), exit: Rc::new(|_: &str| {}) };
            let _pipe = (spec.open.clone().unwrap())(&spec.transport, &spec, events);
            for _ in 0..50 {
                if fake.frames.borrow().len() >= 2 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            use base64::Engine as _;
            let vars = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(fake.frames.borrow()[1]["data"].as_str().unwrap()).unwrap()).unwrap();
            assert_eq!(vars, "SLATE_DIR\u{0}/tmp/wsp-slates/t9\u{0}WSP_THREAD\u{0}t9\u{0}\u{0}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn mcp_server_refuses_a_machine_on_a_box() {
    // Invariant: a machine wsp made on a joined computer (a place, not a folder kind, not at a provider) gets no
    // tool run, and its config is never read.
    LocalSet::new()
        .run_until(async {
            let mut f = box_fake("t1");
            for e in [f.live.get_mut("w1").unwrap(), f.boxes.get_mut("t1").unwrap()] {
                e.folder_on_joined = false;
                e.name = "m1".into();
            }
            f.server = Some(Rc::new(|_n: &str| panic!("asked a machine on a box")));
            let (fake, w, _) = wire(f);
            assert_eq!(
                w.mcp_server("t1", "zones").await.err(),
                Some("$zones calls an MCP server, and this thread runs on m1, a machine wsp made on hetzner, whose server would start on hetzner outside it: a slate's tool run goes only to a thread in a folder on hetzner or on a cloud machine, so start the thread in the project's folder on hetzner".to_string())
            );
            assert!(fake.servers_asked.borrow().is_empty());
            assert_eq!(w.away("t1"), Some(wsp_slates::slate_mcp::Away { computer: "hetzner".into(), host: "studio".into(), machine: Some("m1".into()) }));
            // At a provider it is a cloud machine, which may.
            let mut f = box_fake("t1");
            for e in [f.live.get_mut("w1").unwrap(), f.boxes.get_mut("t1").unwrap()] {
                e.folder_on_joined = false;
            }
            f.at_provider.push("hetzner-id".into());
            let (_fake, w, _) = wire(f);
            assert!(!w.machine_on_box(&w_entry()));
            assert_eq!(w.servers_target(&w_entry()), ServersTarget::Workspace { workspace_id: "w1".into() });
        })
        .await;
}

fn w_entry() -> Entry {
    let mut e = entry("w1");
    e.place = Some("hetzner-id".into());
    e
}

#[tokio::test(flavor = "current_thread")]
async fn mcp_server_on_a_box_without_a_login_fails() {
    // spec rule 7.
    LocalSet::new()
        .run_until(async {
            let mut f = box_fake("t1");
            f.server = Some(Rc::new(|_n: &str| Ok(ReadServer { spec: stdio_spec(), login: None })));
            let (_fake, w, _) = wire(f);
            assert_eq!(w.mcp_server("t1", "zones").await.err(), Some("hetzner did not say who its lines run as".to_string()));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn mcp_server_here_adds_post_here_only_with_a_sign_in_read() {
    // spec rule 7: this computer's reader asked about the thread's checkout; a sign-in with a read gains post_here.
    LocalSet::new()
        .run_until(async {
            for (read, posts) in [(true, true), (false, false)] {
                let mut f = Fake::default();
                f.live.insert("w2".into(), entry("w2"));
                f.latest.insert("t2".into(), row(json!({ "id": "s2", "workspaceId": "w2", "harness": "claude" })));
                f.reader = Some(Rc::new(move |_n: &str| {
                    let mut s = stdio_spec();
                    s.sign_in = Some(sign_in(read));
                    Ok(s)
                }));
                let (fake, w, _) = wire(f);
                let spec = w.mcp_server("t2", "zones").await.unwrap();
                assert_eq!(spec.post.is_some(), posts);
                assert!(spec.open.is_none());
                assert_eq!(*fake.reader_asked.borrow(), vec![vec![HereProject { id: "w2".into(), name: "thread".into(), path: "/work/w2".into() }]]);
            }
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn mcp_server_with_no_folder_or_no_reader_fails() {
    // spec rule 7.
    LocalSet::new()
        .run_until(async {
            let (_f, w, _) = wire(Fake::default());
            assert_eq!(w.mcp_server("t1", "zones").await.err(), Some("this host holds no folder for thread t1".to_string()));
            let mut f = Fake::default();
            f.live.insert("w2".into(), entry("w2"));
            f.latest.insert("t2".into(), row(json!({ "id": "s2", "workspaceId": "w2", "harness": "claude" })));
            let (_f, w, _) = wire(f);
            assert_eq!(w.mcp_server("t2", "zones").await.err(), Some("this host reads no agent's MCP config".to_string()));
        })
        .await;
}

// ---- box roads, wake and settle ----

#[tokio::test(flavor = "current_thread")]
async fn box_roads_answer_none_for_a_thread_here() {
    // spec rule 5.
    LocalSet::new()
        .run_until(async {
            let (_f, w, _) = wire(Fake::default());
            assert!(w.image_on("t1").is_none());
            assert!(w.hash_on("t1").is_none());
            assert!(w.machine_of("t1").is_none());
            assert!(!w.asleep("t1"));
            assert_eq!(w.away("t1"), None);
            let (_f, w, _) = wire(box_fake("t1"));
            let image = w.image_on("t1").expect("read there");
            assert_eq!(image.name, "hetzner");
            let got = (image.read)("/tmp/a.png".into()).await.unwrap();
            assert_eq!(got.media_type.as_deref(), Some("image/png"));
            let hash = w.hash_on("t1").expect("hashed there");
            let read = hash("/root/acme".into(), vec!["/root/acme/a.sh".into()]).await.unwrap();
            assert_eq!(read.files.get("a.sh").map(String::as_str), Some("/root/acme:/root/acme/a.sh"));
            assert_eq!(w.away("t1"), Some(wsp_slates::slate_mcp::Away { computer: "hetzner".into(), host: "studio".into(), machine: None }));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn asleep_and_wake_follow_the_phase() {
    // spec rule 5: a napping box is asleep; wake waits its boot work, then wakes it only while still not running.
    LocalSet::new()
        .run_until(async {
            let f = box_fake("t1");
            let running = f.boxes["t1"].running.clone();
            running.set(false);
            let (fake, w, _) = wire(f);
            assert!(w.asleep("t1"));
            w.wake("t1").await.unwrap();
            assert_eq!((fake.boot_waits.borrow().clone(), fake.woken.borrow().clone()), (vec!["w1".to_string()], vec!["w1".to_string()]));
            running.set(true);
            assert!(!w.asleep("t1"));
            w.wake("t1").await.unwrap();
            assert_eq!(fake.woken.borrow().len(), 1);
            w.wake("nobody").await.unwrap();
            assert_eq!(fake.boot_waits.borrow().len(), 2);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn settled_reads_the_settle_preference() {
    // spec rule 6.
    LocalSet::new()
        .run_until(async {
            for (key, ms) in [("15m", Some(900_000)), ("1h", Some(3_600_000)), ("2h", Some(7_200_000)), ("1d", Some(86_400_000)), ("never", None)] {
                let mut f = Fake::default();
                f.settle = key.into();
                let (fake, w, _) = wire(f);
                w.settled("t1").await;
                assert_eq!(*fake.settled.borrow(), vec![("t1".to_string(), ms)], "{key}");
                assert_eq!(settle_ms(key), ms);
            }
            let (_f, w, _) = wire(Fake::default());
            assert_eq!(w.under("lead"), vec!["lead".to_string(), "lead-child".into()]);
            assert_eq!(w.thread_of_token("tok").as_deref(), Some("t1"));
        })
        .await;
}

// ---- away and missing ----

#[tokio::test(flavor = "current_thread")]
async fn away_names_both_computers_and_the_machine_on_a_box() {
    // spec rule 8.
    LocalSet::new()
        .run_until(async {
            let (_f, w, _) = wire(box_fake("t1"));
            assert_eq!(w.away("t1"), Some(wsp_slates::slate_mcp::Away { computer: "hetzner".into(), host: "studio".into(), machine: None }));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn missing_on_splits_missing_and_off_and_ignores_other_failures() {
    // spec rule 9.
    LocalSet::new()
        .run_until(async {
            let mut f = box_fake("t1");
            f.server = Some(Rc::new(|name: &str| match name {
                "github" => Ok(ReadServer { spec: stdio_spec(), login: Some(login()) }),
                "cloudflare" => Err(ServerReadError::NotSetUp("no cloudflare there".into())),
                "zoho" => Err(ServerReadError::Off("zoho is off in Claude Code's config on hetzner".into())),
                _ => Err(ServerReadError::Other("hetzner did not answer".into())),
            }));
            let (fake, w, _) = wire(f);
            let got = w.missing_on("t1", vec!["cloudflare".into(), "github".into(), "zoho".into(), "flaky".into()]).await;
            assert_eq!(got, wsp_slates::slate_mcp::MissingOn { agent_id: "claude".into(), agent_name: "Claude Code".into(), thread: "t1".into(), missing: vec!["cloudflare".into()], off: vec!["zoho is off in Claude Code's config on hetzner".into()] });
            assert_eq!(fake.servers_asked.borrow().iter().map(|(_, _, n)| n.clone()).collect::<Vec<_>>(), vec!["cloudflare", "github", "zoho", "flaky"]);
            let (_f, w, _) = wire(Fake::default());
            assert_eq!(w.missing_on("t1", vec!["x".into()]).await, wsp_slates::slate_mcp::MissingOn { agent_id: "".into(), agent_name: "".into(), thread: "t1".into(), missing: vec![], off: vec![] });
        })
        .await;
}

// ---- sources ----

#[tokio::test(flavor = "current_thread")]
async fn sources_sort_rows_and_read_this_threads_results() {
    // spec rule 10.
    LocalSet::new()
        .run_until(async {
            let mut f = Fake::default();
            let mut e = entry("w1");
            e.project = Some("p1".into());
            e.rate = Some(0.25);
            f.live.insert("w1".into(), e);
            f.latest.insert("t1".into(), row(json!({ "id": "s2", "workspaceId": "w1", "harness": "claude" })));
            f.rows.insert("t1".into(), vec![row(json!({ "id": "b", "startedAt": 5 })), row(json!({ "id": "none" })), row(json!({ "id": "a", "startedAt": 1 }))]);
            f.transcripts.insert("w1".into(), vec![
                json!({ "type": "session.done", "threadId": "t1", "result": { "status": "completed", "text": "one" } }),
                json!({ "type": "session.done", "threadId": "t2", "result": { "status": "completed", "text": "other" } }),
                json!({ "type": "session.start", "threadId": "t1" }),
                json!({ "type": "session.done", "threadId": "t1", "result": { "status": "failed", "text": "two" } }),
            ]);
            f.accounts = vec![
                json!({ "key": "codex|key", "label": "Other", "agent": "codex", "computers": [] }),
                json!({ "key": "claude|key", "label": "Me", "agent": "claude", "computers": ["here"] }),
            ];
            f.accrued.insert("w1".into(), 2.5);
            let (_f, w, _) = wire(f);
            let ctx = w.sources("t1", "w1");
            assert_eq!((ctx.thread_id(), ctx.workspace_id(), ctx.now()), ("t1", "w1", 1_760_000_000_000));
            assert_eq!(ctx.rows().iter().map(|r| serde_json::to_value(r).unwrap()["id"].as_str().unwrap().to_string()).collect::<Vec<_>>(), vec!["none", "a", "b"]);
            assert_eq!(ctx.results().iter().map(|r| serde_json::to_value(r).unwrap()["text"].as_str().unwrap().to_string()).collect::<Vec<_>>(), vec!["one", "two"]);
            assert_eq!(serde_json::to_value(ctx.account().await.unwrap()).unwrap()["label"], "Me");
            let facts = ctx.workspace().unwrap();
            assert_eq!((facts.computer.as_deref(), facts.project.as_deref(), facts.folder.as_deref()), (Some("studio"), Some("p1-name"), Some("/work/w1")));
            assert_eq!((facts.rate_usd_per_hour, facts.accrued_usd), (Some(0.25), Some(2.5)));
            assert!(facts.checkout.is_some() && facts.pr.is_none());
            let other = w.sources("t1", "w-gone");
            assert!(other.workspace().is_none());
            assert!(other.account().await.is_none());
        })
        .await;
    assert_eq!(
        thread_results(&[json!({ "type": "session.done", "threadId": "t1", "result": { "status": "completed" } }), json!({ "type": "session.done", "threadId": "t2", "result": { "status": "completed" } })], "t1").len(),
        1
    );
}

// ---- deliver ----

#[tokio::test(flavor = "current_thread")]
async fn deliver_answers_queued_when_the_press_waits() {
    // spec rule 11: the queued event wins; the start's later failure is a warning and never surfaces.
    LocalSet::new()
        .run_until(async {
            let f = Fake::default();
            let (tx, rx) = tokio::sync::oneshot::channel::<Result<Started, String>>();
            *f.start_answer.borrow_mut() = Some(async move { rx.await.unwrap() }.boxed_local());
            let (fake, w, _) = wire(f);
            let start = DeliverStart { workspace_id: "w1".into(), prompt: "go".into(), thread: "t1".into(), request_id: "r-1".into() };
            let delivering = w.deliver(start.clone());
            let fired = async {
                for _ in 0..50 {
                    if let Some(f) = fake.queued.borrow().get("r-1") {
                        f();
                        return;
                    }
                    tokio::task::yield_now().await;
                }
                panic!("never listened for the queued event");
            };
            let (got, ()) = futures::join!(delivering, fired);
            assert_eq!(got, Ok(Delivered::Queued));
            assert_eq!(*fake.starts.borrow(), vec![start]);
            tx.send(Err("the thread was deleted".into())).unwrap();
            for _ in 0..20 {
                tokio::task::yield_now().await;
            }
            assert_eq!(*fake.warned.borrow(), vec!["a press waiting in thread t1 was not sent: the thread was deleted".to_string()]);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn deliver_answers_the_started_turn() {
    // spec rule 11.
    LocalSet::new()
        .run_until(async {
            let f = Fake::default();
            *f.start_answer.borrow_mut() = Some(async { Ok(Started { outcome: "started".into(), turn_id: Some("turn-1".into()) }) }.boxed_local());
            let (_fake, w, _) = wire(f);
            let got = w.deliver(DeliverStart { workspace_id: "w1".into(), prompt: "go".into(), thread: "t1".into(), request_id: "r-2".into() }).await;
            assert_eq!(got, Ok(Delivered::Started { outcome: "started".into(), turn_id: Some("turn-1".into()) }));
        })
        .await;
}

// ---- files and naps ----

#[test]
fn secrets_and_slates_dir_sit_in_the_state_folder() {
    // spec rules 12, 13.
    let mut f = Fake::default();
    f.state_folder = Some("/state".into());
    let mut env = IndexMap::new();
    env.insert("FROM".to_string(), "wiring".to_string());
    f.local_env = Some(env);
    let (_f, w, _) = wire(f);
    assert_eq!((w.secrets_file().as_deref(), w.slates_dir().as_deref()), (Some("/state/slates.secrets.json"), Some("/state/slates")));
    assert_eq!(w.run_env().get("FROM").map(String::as_str), Some("wiring"));
    let (_f, w, _) = wire(Fake::default());
    assert_eq!((w.secrets_file(), w.slates_dir()), (None, None));
    assert_eq!(w.run_env().get("FROM").map(String::as_str), Some("process"));
}

#[test]
fn a_nap_reaches_the_slates() {
    // spec rules 12, 14.
    let mut f = Fake::default();
    f.live.insert("w1".into(), entry("w1"));
    let (fake, w, naps) = wire(f);
    (fake.napped.borrow().as_ref().expect("listening for naps"))("w1");
    assert_eq!(*naps.borrow(), vec!["w1".to_string()]);
    w.watch_pr("w1");
    w.watch_pr("w-gone");
    assert_eq!(*fake.polled.borrow(), vec!["w1".to_string()]);
}

#[test]
fn parse_hash_reply_reads_the_daemons_reply() {
    // slates.ts:66: the reply parsed as FsHashReply.
    let read = parse_hash_reply(json!({ "files": { "a.sh": "0123456789abcdef0123" }, "outside": ["/x"] })).unwrap();
    assert_eq!(read.outside, vec!["/x".to_string()]);
    assert!(parse_hash_reply(json!({ "nope": 1 })).is_err());
}

#[allow(dead_code)]
fn _unused(_: Rc<RefCell<Vec<String>>>) {}
