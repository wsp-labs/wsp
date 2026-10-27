// SPDX-License-Identifier: AGPL-3.0-only
//! `wsp-setup` packet 0b (#2477): the bridge from `RuntimeOptions` to the host's agents ports. The TS wires six ports
//! (`agentsReader`, `agentsActs`, `skillsActs`, `serversActs`, `pluginsActs`, `serverIcons`) and hands the callback
//! relay to `rt.agents.forwards`: 28 members in all (`AgentsReader` 6, `AgentsActs` 4, `SkillsActs` 6, `ServersActs` 3,
//! `PluginsActs` 1, `ServerIcons` 3, `SignInForward` 3, `CallbackForwards` 2; agents-read.ts:64-187). Rust's reader adds
//! `has_server` for the TS's optional `server`.
//!
//! A fake holder records each member it is asked, and the tests reach every one through the runtime `create_runtime`
//! builds, by the road the TS reaches it. A runtime with no holder answers each with the TS's missing-road refusal.
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_account::account::preferences::PreferencesCtx as P;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};
use wsp_core::types::api::{KeyWay, Runtime as _, SkillsAddAsk, ToolsAsk};
use wsp_core::types::pending::{SignInAsk, SkillAsk};
use wsp_core::types::wiring::{Local, LocalWiring, Platform, RuntimeOptions};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::MachineBackend;
use wsp_machines::agents_read::{
    AgentsActs, AgentsOn, AgentsPorts, AgentsRead, AgentsReader, CallbackForwards, ForwardsHolder, PluginsActs, ResolvedServer,
    ServerIcons, ServerToolsAsk, ServersActs, SignInAsk as ReadSignInAsk, SignInForward, SignInPlan, SignInRun, SignsIn, SkillAddAsk,
    SkillAsk as ReadSkillAsk, SkillsActs, NO_AGENTS_READER,
};
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_slates::account::slates::SlatesAreaCtx as S;
use wsp_store::store::{StateWriter, Store};
use wsp_threads::threads::agents::AgentsCtx;
use wsp_wire::types::agents_report::{
    AgentsSignInEvent, AgentsTarget, AgentsTargetV0, PluginAsk, PluginRow, ServerAdd, ServerAsk, ServerToolsAnswer, SignInLine, SkillAdded,
    SkillHit, SkillPreview,
};
use wsp_words::exit::HostError;

// ---------------------------------------------------------------------------------------------------------------
// The fake holder: every member says its name into one log.

type Log = Rc<RefCell<Vec<String>>>;

fn say(log: &Log, member: &str) {
    log.borrow_mut().push(member.to_string());
}

fn count(log: &Log, member: &str) -> usize {
    log.borrow().iter().filter(|m| *m == member).count()
}

/// The 29 members the holder answers: the TS's 28 and the reader's `has_server`.
const MEMBERS: [&str; 29] = [
    "reader.read",
    "reader.forget",
    "reader.tools",
    "reader.has_server",
    "reader.server",
    "reader.signs_in",
    "reader.close",
    "acts.sign_in_line",
    "acts.sign_in",
    "acts.key",
    "acts.add_tools",
    "skills.search",
    "skills.get",
    "skills.preview",
    "skills.add",
    "skills.remove",
    "skills.toggle",
    "servers.add",
    "servers.remove",
    "servers.toggle",
    "plugins.toggle",
    "icons.folder",
    "icons.icon",
    "icons.forget",
    "forward.arm",
    "forward.deliver",
    "forward.close",
    "relay.reaches",
    "relay.open",
];

struct Reader(Log);

#[async_trait(?Send)]
impl AgentsReader for Reader {
    async fn read(&self, _on: &AgentsOn, _latest: bool) -> Result<AgentsRead, HostError> {
        say(&self.0, "reader.read");
        Ok(AgentsRead {
            home: "/home/me".into(),
            user: "me".into(),
            agents: vec![],
            skills: vec![],
            servers: vec![],
            plugins: None,
            refused: vec![],
            projects: None,
            run_as: None,
        })
    }
    fn forget(&self, _key: &str) {
        say(&self.0, "reader.forget");
    }
    async fn tools(&self, _on: &AgentsOn, _ask: ServerToolsAsk) -> Result<ServerToolsAnswer, HostError> {
        say(&self.0, "reader.tools");
        Ok(serde_json::from_value(json!({ "auth": "open", "tools": [], "readAt": "2026-10-27T00:00:00.000Z" })).expect("an answer"))
    }
    fn has_server(&self) -> bool {
        say(&self.0, "reader.has_server");
        true
    }
    async fn server(&self, _on: &AgentsOn, _agent: &str, name: &str) -> Result<ResolvedServer, HostError> {
        say(&self.0, "reader.server");
        let spec = wsp_slates::slate_mcp::McpServerSpec {
            transport: wsp_catalog::mcp::McpTransport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), name.to_string()],
                env: IndexMap::new(),
                cwd: None,
                tool_timeout_sec: None,
            },
            cwd: "/work".into(),
            env: IndexMap::new(),
            secrets: vec![],
            open: None,
            post: None,
            sign_in: None,
        };
        Ok(ResolvedServer { spec: Rc::new(spec), login: None })
    }
    async fn signs_in(&self, urls: &[String]) -> Option<SignsIn> {
        say(&self.0, "reader.signs_in");
        let asked = urls.to_vec();
        Some(Rc::new(move |url: &str| asked.iter().any(|a| a == url).then_some(true)))
    }
    fn close(&self) {
        say(&self.0, "reader.close");
    }
}

struct Acts(Log);

#[async_trait(?Send)]
impl AgentsActs for Acts {
    async fn sign_in_line(&self, _on: &AgentsOn, _ask: &ReadSignInAsk) -> Result<SignInLine, HostError> {
        say(&self.0, "acts.sign_in_line");
        Ok(serde_json::from_value(json!({ "command": "codex login --device-auth" })).expect("a line"))
    }
    async fn sign_in(&self, _on: &AgentsOn, _ask: &ReadSignInAsk) -> Result<SignInPlan, HostError> {
        say(&self.0, "acts.sign_in");
        let log = self.0.clone();
        Ok(Box::new(move |run: SignInRun| {
            async move {
                let forward = run.forward.clone().expect("the relay's forward reaches the plan");
                assert!(forward.arm("https://mcp.notion.com/authorize").await?);
                forward.deliver("http://localhost:1/cb?code=1").await?;
                say(&log, "plan.done");
                Ok(())
            }
            .boxed_local()
        }))
    }
    async fn key(&self, _agent: &str, _key: &str, _way: Option<&str>) -> Result<(), HostError> {
        say(&self.0, "acts.key");
        Ok(())
    }
    async fn add_tools(&self, _on: &AgentsOn, _agent: &str) -> Result<String, HostError> {
        say(&self.0, "acts.add_tools");
        Ok("~/.codex/config.toml".to_string())
    }
}

struct Skills(Log);

#[async_trait(?Send)]
impl SkillsActs for Skills {
    async fn search(&self, _q: &str, _limit: u32) -> Result<Vec<SkillHit>, HostError> {
        say(&self.0, "skills.search");
        Ok(vec![serde_json::from_value(json!({ "id": "a/b/pdf", "source": "a/b", "skillId": "pdf", "name": "pdf", "installs": 3 }))
            .expect("a hit")])
    }
    async fn get(&self, _skill: &str) -> Result<SkillPreview, HostError> {
        say(&self.0, "skills.get");
        Ok(preview())
    }
    async fn preview(&self, _on: &AgentsOn, _ask: &ReadSkillAsk) -> Result<SkillPreview, HostError> {
        say(&self.0, "skills.preview");
        Ok(preview())
    }
    async fn add(&self, _on: &AgentsOn, _ask: &SkillAddAsk) -> Result<SkillAdded, HostError> {
        say(&self.0, "skills.add");
        Ok(serde_json::from_value(json!({ "path": "~/.agents/skills/pdf", "agents": [] })).expect("added"))
    }
    async fn remove(&self, _on: &AgentsOn, _ask: &ReadSkillAsk) -> Result<Vec<String>, HostError> {
        say(&self.0, "skills.remove");
        Ok(vec!["~/.agents/skills/pdf".to_string()])
    }
    async fn toggle(&self, _on: &AgentsOn, _ask: &ReadSkillAsk, _to: bool) -> Result<Vec<String>, HostError> {
        say(&self.0, "skills.toggle");
        Ok(vec!["~/.agents/skills/pdf".to_string()])
    }
}

fn preview() -> SkillPreview {
    serde_json::from_value(json!({ "text": "# pdf", "size": 5 })).expect("a preview")
}

struct Servers(Log);

#[async_trait(?Send)]
impl ServersActs for Servers {
    async fn add(&self, _on: &AgentsOn, _ask: &ServerAdd) -> Result<String, HostError> {
        say(&self.0, "servers.add");
        Ok("~/.claude.json".to_string())
    }
    async fn remove(&self, _on: &AgentsOn, _ask: &ServerAsk) -> Result<String, HostError> {
        say(&self.0, "servers.remove");
        Ok("~/.claude.json".to_string())
    }
    async fn toggle(&self, _on: &AgentsOn, _ask: &ServerAsk, _to: bool) -> Result<String, HostError> {
        say(&self.0, "servers.toggle");
        Ok("~/.codex/config.toml".to_string())
    }
}

struct Plugins(Log);

#[async_trait(?Send)]
impl PluginsActs for Plugins {
    async fn toggle(&self, _on: &AgentsOn, _ask: &PluginAsk, _to: bool) -> Result<PluginRow, HostError> {
        say(&self.0, "plugins.toggle");
        Ok(serde_json::from_value(json!({
            "agent": "claude", "id": "pdf-tools@acme", "name": "pdf-tools", "marketplace": "acme", "scope": "user", "on": false,
            "brings": { "skills": ["pdf"], "commands": [], "subagents": [], "hooks": [], "servers": [], "lsp": [], "apps": [] },
        }))
        .expect("a plugin row"))
    }
}

struct Icons(Log, Cell<bool>);

#[async_trait(?Send)]
impl ServerIcons for Icons {
    fn folder(&self) -> String {
        say(&self.0, "icons.folder");
        "/root/.wsp/icons".to_string()
    }
    async fn icon(&self, _host: &str, _refresh: Option<bool>) -> Result<Option<String>, HostError> {
        say(&self.0, "icons.icon");
        Ok(Some("data:image/png;base64,AAAA".to_string()))
    }
    fn forget(&self) -> Result<(), String> {
        say(&self.0, "icons.forget");
        if self.1.get() {
            return Err("EACCES: permission denied, rmdir '/root/.wsp/icons'".to_string());
        }
        Ok(())
    }
}

struct Forward(Log);

#[async_trait(?Send)]
impl SignInForward for Forward {
    async fn arm(&self, _url: &str) -> Result<bool, HostError> {
        say(&self.0, "forward.arm");
        Ok(true)
    }
    async fn deliver(&self, _landed: &str) -> Result<(), HostError> {
        say(&self.0, "forward.deliver");
        Ok(())
    }
    fn close(&self) {
        say(&self.0, "forward.close");
    }
}

struct Relay(Log);

impl CallbackForwards for Relay {
    fn reaches(&self, _target: &AgentsTarget) -> bool {
        say(&self.0, "relay.reaches");
        true
    }
    fn open(&self, _target: &AgentsTarget) -> Option<Rc<dyn SignInForward>> {
        say(&self.0, "relay.open");
        Some(Rc::new(Forward(self.0.clone())))
    }
}

fn holder(log: &Log, icons_refuse: bool) -> Rc<dyn wsp_core::types::pending::AgentsPorts> {
    Rc::new(AgentsPorts {
        reader: Some(Rc::new(Reader(log.clone()))),
        acts: Some(Rc::new(Acts(log.clone()))),
        skills: Some(Rc::new(Skills(log.clone()))),
        servers: Some(Rc::new(Servers(log.clone()))),
        plugins: Some(Rc::new(Plugins(log.clone()))),
        icons: Some(Rc::new(Icons(log.clone(), Cell::new(icons_refuse)))),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The runtime: this computer's daemon road is a channel that answers each frame, which a sign-in's pty runs over.

struct Echo;

#[async_trait(?Send)]
impl DaemonChannel for Echo {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let mut reply = frame;
        reply.insert("ok".to_string(), json!(true));
        Ok(reply)
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending::<ChannelEnd>().await
    }
}

fn backend() -> Arc<dyn MachineBackend> {
    Arc::new(FakeBackend::new(FakeBackendOptions::default()))
}

fn local_wiring() -> LocalWiring {
    LocalWiring {
        backend: backend(),
        folder: "/tmp/wsp-local".into(),
        exec_stream: Rc::new(|_, _| unimplemented!("no turn runs in these tests")),
        home: Rc::new(|_| "/home/me/.claude".to_string()),
        home_dir: "/home/me".into(),
        roots_path: "/home/me/.wsp/roots".into(),
        env: Rc::new(|| IndexMap::from([("PATH".to_string(), "/usr/bin".to_string())])),
        daemon_road: Some({
            let held: Rc<dyn Fn() -> Local<'static, wsp_wire::views::workspace::DaemonReachView>> = Rc::new(|| {
                Box::pin(async {
                    Ok(wsp_wire::views::workspace::DaemonReachView {
                        url: "ws://127.0.0.1:7".into(),
                        expires_at: wsp_wire::types::js_number::JsNumber(9.0e15),
                        daemon_token: Some("tok-7".into()),
                    })
                })
            });
            held
        }),
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: None,
        platform: Platform::Linux,
        close: None,
    }
}

fn sqlite(dir: &std::path::Path) -> Rc<dyn Store> {
    let writer = StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() };
    Rc::new(wsp_store::sqlite_store::sqlite_store(&dir.join("state.json"), writer))
}

fn options(dir: &std::path::Path, ports: Option<Rc<dyn wsp_core::types::pending::AgentsPorts>>) -> RuntimeOptions {
    RuntimeOptions {
        backend: Some(backend()),
        store: Some(sqlite(dir)),
        env: Some(IndexMap::new()),
        local: Some(local_wiring()),
        daemon_channel: Some(Rc::new(|_: DaemonChannelOptions| Box::pin(async { Ok(Rc::new(Echo) as Rc<dyn DaemonChannel>) }))),
        agents_ports: ports,
        ..Default::default()
    }
}

/// The runtime over a fake holder, inside the task set the owner thread runs, and the log the holder writes.
fn with_holder<Fut: Future<Output = ()>>(icons_refuse: bool, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>, Log) -> Fut) {
    let log: Log = Rc::default();
    let dir = tempfile::tempdir().expect("a folder");
    let opts = options(dir.path(), Some(holder(&log, icons_refuse)));
    run(opts, |rt, c| f(rt, c, log));
}

fn without_holder<Fut: Future<Output = ()>>(f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let dir = tempfile::tempdir().expect("a folder");
    run(options(dir.path(), None), f);
}

fn run<Fut: Future<Output = ()>>(opts: RuntimeOptions, f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx = runtime.context();
        f(runtime, ctx).await
    });
}

fn here() -> AgentsTarget {
    AgentsTarget::V0(AgentsTargetV0 { place_id: "here".into(), project: None })
}

fn sign_in_ask(server: Option<&str>) -> SignInAsk {
    SignInAsk { agent: "claude".into(), server: server.map(str::to_string), ..Default::default() }
}

fn server_add() -> ServerAdd {
    serde_json::from_value(json!({ "agent": "claude", "name": "notion", "url": "https://mcp.notion.com/mcp" })).expect("a server")
}

fn server_ask() -> ServerAsk {
    serde_json::from_value(json!({ "agent": "claude", "name": "notion" })).expect("a server ask")
}

fn plugin_ask() -> PluginAsk {
    PluginAsk { agent: "claude".into(), plugin: "pdf-tools@acme".into() }
}

fn skill_ask() -> SkillAsk {
    SkillAsk { name: "pdf".into(), project: None }
}

fn patch(value: Value) -> wsp_wire::views::preferences::PreferencesPatch {
    serde_json::from_value(value).expect("a preferences patch")
}

/// Waits for a sign-in's detached run to reach its end, which says `reader.forget` last.
async fn ended(log: &Log) {
    for _ in 0..200 {
        if count(log, "reader.forget") > 0 {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("the sign-in never ended: {:?}", log.borrow());
}

// ---------------------------------------------------------------------------------------------------------------
// With a holder: each member answers through the runtime, by the road the TS reaches it.

type Rig = (Rc<AssembledRuntime>, Rc<Ctx>, Log);

async fn reader_reads_and_asks_for_tools_and_servers((rt, c, log): Rig) {
    let report = rt.agents().read(here(), None).await.expect("a read");
    assert_eq!(report.home, "/home/me");
    assert_eq!(count(&log, "reader.read"), 1);
    let ask = ToolsAsk { agent: "claude".into(), name: "notion".into(), refresh: None };
    rt.agents().tools(here(), ask, None).await.expect("tools");
    assert_eq!(count(&log, "reader.tools"), 1);
    let held = S::agents_server(&*c, here(), "claude", "wsp").await.expect("a server");
    assert!(matches!(held.spec.transport, wsp_catalog::mcp::McpTransport::Stdio { ref command, .. } if command == "npx"));
    let spec = S::agents_reader_server(&*c, vec![], "claude", "wsp").expect("this host reads agents' MCP config").await.expect("a spec");
    assert_eq!(spec.cwd, "/work");
    assert!(count(&log, "reader.has_server") >= 2, "both roads ask the reader whether it resolves servers");
    assert_eq!(count(&log, "reader.server"), 2);
}

async fn reader_answers_which_addresses_ask_for_a_sign_in_where_a_launch_asks((_, c, log): Rig) {
    let asked = AgentsCtx::signs_in(&*c, vec!["https://mcp.notion.com/mcp".to_string()]).await.expect("the reader's answer");
    assert_eq!(count(&log, "reader.signs_in"), 1);
    assert_eq!(asked("https://mcp.notion.com/mcp"), Some(true));
    assert_eq!(asked("https://elsewhere.example"), None);
}

async fn this_computers_sign_ins_read_through_the_reader_and_the_runtime_closes_it((rt, c, log): Rig) {
    let before = count(&log, "reader.read");
    let signed_in = wsp_account::account::usage::UsageCtx::read_sign_ins_here(&*c).await.expect("a read");
    assert_eq!(signed_in, Some(vec![]), "a reader that finds no agent signed in lists none");
    assert_eq!(count(&log, "reader.read"), before + 1);
    assert_eq!(count(&log, "reader.close"), 0);
    rt.close().await.expect("close");
    assert_eq!(count(&log, "reader.close"), 1);
}

async fn acts_write_the_sign_in_line_the_key_and_the_tools_config((rt, _, log): Rig) {
    rt.agents().sign_in_line(here(), sign_in_ask(None), None).await.expect("a line");
    rt.agents().key("claude", "sk-ant-1", Some(KeyWay::Key)).await.expect("a key");
    let written = rt.agents().add_tools(here(), "claude", None).await.expect("the tools config");
    assert_eq!(written, "~/.codex/config.toml");
    for member in ["acts.sign_in_line", "acts.key", "acts.add_tools"] {
        assert_eq!(count(&log, member), 1, "{member}");
    }
}

async fn sign_in_runs_the_acts_plan_over_the_relays_forward_and_the_reader_forgets_at_its_end((rt, c, log): Rig) {
    let relay = Rc::new(Relay(log.clone()));
    let detach = rt.agents().forwards(Rc::new(ForwardsHolder(relay)));
    assert!(c.preferences_seams().agents_read.reached(&here()), "the relay reaches the target it was asked about");
    let events: Rc<RefCell<Vec<AgentsSignInEvent>>> = Rc::default();
    let held = events.clone();
    rt.agents().sign_in(here(), sign_in_ask(Some("notion")), Rc::new(move |e| held.borrow_mut().push(e)), None).await.expect("a sign-in");
    ended(&log).await;
    for member in ["acts.sign_in", "relay.open", "forward.arm", "forward.deliver", "forward.close", "reader.forget"] {
        assert_eq!(count(&log, member), 1, "{member}");
    }
    assert!(count(&log, "relay.reaches") >= 1);
    detach();
    assert!(!c.preferences_seams().agents_read.reached(&here()), "a detached relay reaches nothing");
}

async fn skills_acts_search_get_preview_add_remove_and_toggle((rt, _, log): Rig) {
    assert_eq!(rt.agents().skills_search("pdf", None).await.expect("hits").len(), 1);
    rt.agents().skills_get("a/b/pdf").await.expect("a preview");
    rt.agents().skills_preview(here(), skill_ask(), None).await.expect("a preview");
    let add = SkillsAddAsk { skill: "a/b/pdf".into(), agents: None, project: None };
    rt.agents().skills_add(here(), add, None).await.expect("added");
    rt.agents().skills_remove(here(), skill_ask(), None).await.expect("removed");
    rt.agents().skills_toggle(here(), skill_ask(), false, None).await.expect("toggled");
    for member in ["skills.search", "skills.get", "skills.preview", "skills.add", "skills.remove", "skills.toggle"] {
        assert_eq!(count(&log, member), 1, "{member}");
    }
}

async fn servers_and_plugins_acts_write_their_configs((rt, _, log): Rig) {
    rt.agents().servers_add(here(), server_add(), None).await.expect("added");
    rt.agents().servers_remove(here(), server_ask(), None).await.expect("removed");
    rt.agents().servers_toggle(here(), server_ask(), false, None).await.expect("toggled");
    let row = rt.agents().plugins_toggle(here(), plugin_ask(), false, None).await.expect("toggled");
    assert_eq!(row.id, "pdf-tools@acme");
    for member in ["servers.add", "servers.remove", "servers.toggle", "plugins.toggle"] {
        assert_eq!(count(&log, member), 1, "{member}");
    }
}

async fn icons_answer_an_icon_and_the_preferences_name_their_folder_and_forget_them_when_turned_off((rt, c, log): Rig) {
    let icon = rt.agents().servers_icon("mcp.notion.com", Some(true)).await.expect("an icon");
    assert_eq!(icon.as_deref(), Some("data:image/png;base64,AAAA"));
    assert_eq!(count(&log, "icons.icon"), 1);
    let held = P::server_icons(&*c).expect("the host's icons are held");
    assert_eq!(wsp_account::account::preferences::ServerIcons::folder(&*held), "/root/.wsp/icons");
    assert!(wsp_account::account::preferences::ServerIcons::forget(&*held).is_ok());
    assert_eq!((count(&log, "icons.folder"), count(&log, "icons.forget")), (1, 1));
    let set = rt.preferences().set(patch(json!({ "serverIcons": false }))).await.expect("set");
    assert_eq!(set.notice, None, "a folder that went leaves nothing to say");
    assert_eq!(count(&log, "icons.forget"), 2);
    let none = rt.agents().servers_icon("mcp.notion.com", None).await.expect("no icon");
    assert_eq!(none, None, "icons the person turned off are asked of no one");
    assert_eq!(count(&log, "icons.icon"), 1);
}

#[test]
fn the_reader_reads_and_asks_for_tools_and_servers_through_the_runtime() {
    with_holder(false, |rt, c, log| reader_reads_and_asks_for_tools_and_servers((rt, c, log)));
}

#[test]
fn the_reader_answers_which_addresses_ask_for_a_sign_in_where_a_launch_asks() {
    with_holder(false, |rt, c, log| reader_answers_which_addresses_ask_for_a_sign_in_where_a_launch_asks((rt, c, log)));
}

#[test]
fn the_reader_is_closed_where_the_runtime_closes_and_this_computers_sign_ins_read_through_it() {
    with_holder(false, |rt, c, log| this_computers_sign_ins_read_through_the_reader_and_the_runtime_closes_it((rt, c, log)));
}

#[test]
fn the_acts_write_the_sign_in_line_the_key_and_the_tools_config() {
    with_holder(false, |rt, c, log| acts_write_the_sign_in_line_the_key_and_the_tools_config((rt, c, log)));
}

#[test]
fn a_sign_in_runs_the_acts_plan_over_the_relays_forward_and_the_reader_forgets_at_its_end() {
    with_holder(false, |rt, c, log| sign_in_runs_the_acts_plan_over_the_relays_forward_and_the_reader_forgets_at_its_end((rt, c, log)));
}

#[test]
fn the_skills_acts_search_get_preview_add_remove_and_toggle() {
    with_holder(false, |rt, c, log| skills_acts_search_get_preview_add_remove_and_toggle((rt, c, log)));
}

#[test]
fn the_servers_and_plugins_acts_write_their_configs() {
    with_holder(false, |rt, c, log| servers_and_plugins_acts_write_their_configs((rt, c, log)));
}

#[test]
fn the_icons_answer_an_icon_and_the_preferences_name_their_folder_and_forget_them_when_turned_off() {
    with_holder(false, |rt, c, log| {
        icons_answer_an_icon_and_the_preferences_name_their_folder_and_forget_them_when_turned_off((rt, c, log))
    });
}

#[test]
fn icons_whose_folder_cannot_be_deleted_still_save_and_the_notice_says_so_in_nodes_words_cut_short() {
    with_holder(true, |rt, _, _| async move {
        let set = rt.preferences().set(patch(json!({ "serverIcons": false }))).await.expect("set");
        let notice = set.notice.expect("a folder that stays is said");
        assert!(notice.contains("permission denied"), "{notice}");
        assert!(!notice.contains("EACCES"), "{notice}");
    });
}

#[test]
fn the_runtime_reaches_each_of_the_28_members_of_one_holder_and_the_readers_has_server() {
    with_holder(false, |rt, c, log| async move {
        let rig = || (rt.clone(), c.clone(), log.clone());
        reader_reads_and_asks_for_tools_and_servers(rig()).await;
        reader_answers_which_addresses_ask_for_a_sign_in_where_a_launch_asks(rig()).await;
        acts_write_the_sign_in_line_the_key_and_the_tools_config(rig()).await;
        sign_in_runs_the_acts_plan_over_the_relays_forward_and_the_reader_forgets_at_its_end(rig()).await;
        skills_acts_search_get_preview_add_remove_and_toggle(rig()).await;
        servers_and_plugins_acts_write_their_configs(rig()).await;
        icons_answer_an_icon_and_the_preferences_name_their_folder_and_forget_them_when_turned_off(rig()).await;
        this_computers_sign_ins_read_through_the_reader_and_the_runtime_closes_it(rig()).await;
        let missed: Vec<&str> = MEMBERS.iter().copied().filter(|member| count(&log, member) == 0).collect();
        assert!(missed.is_empty(), "members no road reached: {missed:?}");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// With no holder: each answers as the runtime does today, which is the TS wired without the ports.

#[test]
fn a_runtime_with_no_holder_refuses_every_read_and_write_in_the_missing_reader_sentence() {
    without_holder(|rt, c| async move {
        let refused = |e: HostError| assert_eq!(e.message, NO_AGENTS_READER);
        refused(rt.agents().read(here(), None).await.map(|_| ()).expect_err("read"));
        let ask = ToolsAsk { agent: "claude".into(), name: "notion".into(), refresh: None };
        refused(rt.agents().tools(here(), ask, None).await.map(|_| ()).expect_err("tools"));
        refused(rt.agents().sign_in_line(here(), sign_in_ask(None), None).await.map(|_| ()).expect_err("sign_in_line"));
        refused(rt.agents().key("claude", "sk-ant-1", None).await.expect_err("key"));
        refused(rt.agents().add_tools(here(), "claude", None).await.map(|_| ()).expect_err("add_tools"));
        refused(rt.agents().skills_search("pdf", None).await.map(|_| ()).expect_err("skills_search"));
        refused(rt.agents().skills_get("a/b/pdf").await.map(|_| ()).expect_err("skills_get"));
        refused(rt.agents().skills_preview(here(), skill_ask(), None).await.map(|_| ()).expect_err("skills_preview"));
        let add = SkillsAddAsk { skill: "a/b/pdf".into(), agents: None, project: None };
        refused(rt.agents().skills_add(here(), add, None).await.map(|_| ()).expect_err("skills_add"));
        refused(rt.agents().skills_remove(here(), skill_ask(), None).await.map(|_| ()).expect_err("skills_remove"));
        refused(rt.agents().skills_toggle(here(), skill_ask(), true, None).await.map(|_| ()).expect_err("skills_toggle"));
        refused(rt.agents().servers_add(here(), server_add(), None).await.map(|_| ()).expect_err("servers_add"));
        refused(rt.agents().servers_remove(here(), server_ask(), None).await.map(|_| ()).expect_err("servers_remove"));
        refused(rt.agents().servers_toggle(here(), server_ask(), true, None).await.map(|_| ()).expect_err("servers_toggle"));
        refused(rt.agents().plugins_toggle(here(), plugin_ask(), true, None).await.map(|_| ()).expect_err("plugins_toggle"));
        let said = S::agents_server(&*c, here(), "claude", "wsp").await.map(|_| ()).expect_err("no reader wired");
        assert_eq!(said, wsp_slates::account::slates::ServerReadError::Other(NO_AGENTS_READER.to_string()));
    });
}

#[test]
fn a_runtime_with_no_holder_has_no_icons_no_signs_in_no_sign_ins_read_and_nothing_to_close() {
    without_holder(|rt, c| async move {
        assert_eq!(rt.agents().servers_icon("mcp.notion.com", None).await.expect("no icons is no icon"), None);
        assert!(P::server_icons(&*c).is_none());
        assert_eq!(wsp_account::account::preferences::ServerIcons::folder(&*c), "");
        assert!(wsp_account::account::preferences::ServerIcons::forget(&*c).is_ok());
        assert!(S::agents_reader_server(&*c, vec![], "claude", "wsp").is_none(), "this host reads no agent's MCP config");
        assert!(AgentsCtx::signs_in(&*c, vec!["https://mcp.notion.com/mcp".to_string()]).await.is_none());
        assert_eq!(wsp_account::account::usage::UsageCtx::read_sign_ins_here(&*c).await.expect("no reader is no read"), None);
        assert!(!c.preferences_seams().agents_read.reached(&here()), "no relay is held");
        rt.close().await.expect("a runtime with no reader closes");
    });
}

struct Stranger;

impl wsp_core::types::pending::CallbackForwards for Stranger {}

#[test]
fn a_relay_the_runtime_cannot_read_is_held_as_no_relay_and_its_detach_does_nothing() {
    without_holder(|rt, c| async move {
        let detach = rt.agents().forwards(Rc::new(Stranger));
        assert!(!c.preferences_seams().agents_read.reached(&here()));
        detach();
    });
}
