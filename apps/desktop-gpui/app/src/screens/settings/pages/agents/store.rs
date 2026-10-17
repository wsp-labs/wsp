// SPDX-License-Identifier: AGPL-3.0-only
//! What the Agents pages keep while one is drawn, after `settingsStore.ts`'s agents fields and the hooks under
//! `components/agents/`: the tab and the computer read, each computer's report and when it was read, the broker's word,
//! the sign-ins running and the acts on servers, skills and plugins with what is busy and what the host refused. It
//! follows the host's `agents.changed` and `agents.signIn` itself, so a report is read again and a sign-in steps on.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use futures::StreamExt as _;
use gpui_kit::{App, Context, Entity, Task, Window};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::ops::settings_agents::{Report as ReportReply, SignInStarted, SignIns};
use wsp_host_client::protocol::{
    AgentsAddToolsRequest, AgentsCloudSignInRequest, AgentsKeyRequest, AgentsReadRequest, AgentsSignInCodeRequest, AgentsSignInRequest,
    AgentsSignInStopRequest, AgentsSignInsRequest, BrokerStatusRequest, ServersSignInRequest, UsageAccountsRequest,
};
use wsp_host_client::{Event, HostClient, Op};

use super::model::{reshape, Broker, Report};
use crate::screens::settings::model::req;
use crate::screens::settings::Acts;

/// How long a reading stands before showing the page asks for another (`FRESH_MS`).
const FRESH: Duration = Duration::from_secs(5 * 60);

/// The tab the Agents page shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Agents,
    Servers,
    Skills,
    Plugins,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Agents, Tab::Servers, Tab::Skills, Tab::Plugins];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Agents => "agents",
            Tab::Servers => "servers",
            Tab::Skills => "skills",
            Tab::Plugins => "plugins",
        }
    }
}

/// One computer's read: the report, whether a read runs, the host's refusal while no report stands, and when it was
/// answered.
#[derive(Default)]
pub struct Read {
    pub report: Option<Report>,
    pub reading: bool,
    pub error: Option<String>,
    pub read_at: Option<SystemTime>,
}

/// How a sign-in goes where it was pressed (`SignInStart`).
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    Run {
        agent: String,
        server: Option<(String, String, Option<String>)>,
        finish: Option<&'static str>,
        pastes: bool,
        terminal: bool,
        cloud: bool,
    },
    Vault {
        agent: String,
        mint: Option<String>,
        word: &'static str,
    },
    Copy {
        line: String,
        why: Option<String>,
    },
}

/// A sign-in as its row draws it while it stands (`SignInFlow`).
#[derive(Clone, Debug, PartialEq)]
pub enum Flow {
    Run {
        id: Option<String>,
        state: String,
        finish: Option<&'static str>,
        pastes: bool,
        url: Option<String>,
        code: Option<String>,
        paste: bool,
        said: Option<String>,
        pty: bool,
    },
    Vault {
        agent: String,
        mint: Option<String>,
        word: &'static str,
        saving: bool,
        refused: Option<String>,
    },
    Copy {
        line: String,
        why: Option<String>,
    },
}

impl Flow {
    fn running(id: Option<String>) -> Flow {
        Flow::Run { id, state: "running".into(), finish: None, pastes: false, url: None, code: None, paste: false, said: None, pty: false }
    }

    /// Whether a watched sign-in is still going, so its act is Cancel.
    pub fn runs(&self) -> bool {
        matches!(self, Flow::Run { state, .. } if state != "failed")
    }

    /// Whether it waits on the person, which its row says in place of its state.
    pub fn waits(&self) -> bool {
        matches!(self, Flow::Run { state, .. } if state == "waiting")
    }
}

/// One step of a sign-in the host pushed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Step {
    sign_in_id: String,
    state: String,
    url: Option<String>,
    code: Option<String>,
    paste: Option<bool>,
    said: Option<String>,
    pty_id: Option<String>,
}

/// One server's tools as its last ask stands.
#[derive(Clone, Default)]
pub struct ToolsState {
    pub listing: bool,
    pub answer: Option<super::model::ToolsAnswer>,
    pub error: Option<String>,
}

/// A document read for a detail: reading, read, or refused.
#[derive(Clone, Default)]
pub struct Doc {
    pub text: Option<String>,
    pub size: u64,
    pub error: Option<String>,
}

/// A search of skills.sh for one query.
#[derive(Clone, Default)]
pub struct Search {
    pub reading: bool,
    pub hits: Option<Vec<super::model::SkillHit>>,
    pub error: Option<String>,
}

/// What a skill's install is picked to take: the agents, and the project it goes in by id and name.
#[derive(Clone, Default)]
pub struct Picks {
    pub agents: Option<Vec<String>>,
    pub project: Option<(String, String)>,
}

/// The account an agent is signed in as on a computer, off the usage accounts.
#[derive(Clone, Deserialize)]
pub struct Account {
    pub agent: String,
    #[serde(default)]
    pub computers: Vec<String>,
    pub plan: Option<String>,
    pub address: Option<String>,
}

/// Rows and items by the computer they stand on and their key.
pub type Key = (String, String);

pub struct Store {
    acts: Acts,
    pub tab: Tab,
    pub place: Option<String>,
    pub reads: HashMap<String, Read>,
    pub broker: Option<Broker>,
    broker_asked: bool,
    pub accounts: Option<Vec<Account>>,
    accounts_asked: bool,
    pub flows: HashMap<Key, Flow>,
    /// Sign-in steps that came before the answer naming their run.
    early: HashMap<String, Step>,
    pub adding: HashSet<Key>,
    pub tools: HashMap<Key, ToolsState>,
    pub busy: HashSet<Key>,
    pub refused: HashMap<Key, String>,
    pub previews: HashMap<Key, Doc>,
    pub remote: HashMap<String, Doc>,
    pub searches: HashMap<String, Search>,
    pub picks: HashMap<String, Picks>,
    /// The report each computer's server checks were last asked for, by when it was read.
    pub shown: HashMap<String, String>,
    /// The add's search as last asked of skills.sh, and the words waiting out the pause before they are.
    pub add_asked: String,
    pub add_pending: Option<String>,
    /// The open sheet's write: whether it runs, and the host's refusal.
    pub sheet_saving: bool,
    pub sheet_refusal: Option<String>,
    /// The environment sheet's names kept and the variables staged, while it is open.
    pub env_kept: Vec<String>,
    pub env_added: Vec<(String, String)>,
    /// Calls asked before the host answered, sent once it does.
    pending: Vec<Pending>,
    _follow: Option<Following>,
}

type Pending = Box<dyn FnOnce(&mut Store, &mut Context<Store>)>;

/// The host's events this store follows, stopped with it.
struct Following {
    _task: Task<()>,
    abort: tokio::task::AbortHandle,
}

impl Drop for Following {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

/// A computer's read as the host takes it.
pub fn target(place: &str) -> Value {
    json!({ "placeId": place })
}

/// `agentRowId`.
pub fn agent_row(agent: &str) -> String {
    format!("agent-{agent}")
}

/// `serverRowId`.
pub fn server_row(agent: &str, scope: &str, project: Option<&str>, name: &str) -> String {
    match project {
        Some(project) => format!("server-{agent}-{scope}-{project}-{name}"),
        None => format!("server-{agent}-{scope}-{name}"),
    }
}

impl Store {
    pub fn new(acts: Acts) -> Store {
        Store {
            acts,
            tab: Tab::Agents,
            place: None,
            reads: HashMap::new(),
            broker: None,
            broker_asked: false,
            accounts: None,
            accounts_asked: false,
            flows: HashMap::new(),
            early: HashMap::new(),
            adding: HashSet::new(),
            tools: HashMap::new(),
            busy: HashSet::new(),
            refused: HashMap::new(),
            previews: HashMap::new(),
            remote: HashMap::new(),
            searches: HashMap::new(),
            picks: HashMap::new(),
            shown: HashMap::new(),
            add_asked: String::new(),
            add_pending: None,
            sheet_saving: false,
            sheet_refusal: None,
            env_kept: Vec::new(),
            env_added: Vec::new(),
            pending: Vec::new(),
            _follow: None,
        }
    }

    /// The page's one store, kept while an Agents page is drawn.
    pub fn of(acts: &Acts, window: &mut Window, cx: &mut App) -> Entity<Store> {
        let acts = acts.clone();
        let store = window.use_keyed_state("agents-store", cx, {
            let acts = acts.clone();
            move |_, _| Store::new(acts)
        });
        store.update(cx, |s, cx| {
            s.acts = acts;
            if s.acts.host(cx).is_some() {
                for call in std::mem::take(&mut s.pending) {
                    call(s, cx);
                }
            }
            s.follow(cx);
        });
        store
    }

    pub fn acts(&self) -> &Acts {
        &self.acts
    }

    /// One op on the host after this frame, its answer laid on the store and drawn.
    pub fn call<O>(
        &mut self,
        op: O,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Store, Result<O::Reply, String>, &mut Context<Store>) + 'static,
    ) where
        O: Op + Send + 'static,
        O::Reply: Send + 'static,
    {
        if self.acts.host(cx).is_none() {
            self.pending.push(Box::new(move |s, cx| s.call(op, cx, then)));
            return;
        }
        let me = cx.entity().downgrade();
        self.acts.call(op, cx, move |answer, cx| {
            _ = me.update(cx, |s, cx| {
                then(s, answer, cx);
                cx.notify();
            });
        });
    }

    pub fn failed(&self, said: String, cx: &mut App) {
        self.acts.failed(said, cx);
    }

    pub fn done(&self, line: String, cx: &mut App) {
        self.acts.done(line, cx);
    }

    pub fn close_dialog(&self, cx: &mut App) {
        self.acts.close_dialog(cx);
    }

    /// `agents.setup` for one agent on one computer, then the report read again so the row draws what the host keeps.
    pub fn setup(
        &mut self,
        place: &str,
        agent: &str,
        change: Value,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Store, Result<(), String>, &mut Context<Store>) + 'static,
    ) {
        let mut params = json!({ "placeId": place, "agent": agent });
        if let (Some(params), Value::Object(change)) = (params.as_object_mut(), change) {
            params.extend(change);
        }
        let place = place.to_owned();
        self.call(RawSetup(params), cx, move |s, answer, cx| {
            let ok = answer.is_ok();
            then(s, answer.map(|_| ()), cx);
            if ok {
                s.reads.entry(place.clone()).or_default();
                s.refresh(&place, cx);
            }
        });
    }

    /// The computer's report, asked where none stands or the one standing is older than `FRESH`.
    pub fn read(&mut self, place: &str, cx: &mut Context<Self>) {
        let stale =
            self.reads.get(place).is_none_or(|r| !r.reading && r.read_at.is_some_and(|at| at.elapsed().unwrap_or_default() > FRESH));
        if stale {
            self.refresh(place, cx);
        }
    }

    /// Read again, at once.
    pub fn refresh(&mut self, place: &str, cx: &mut Context<Self>) {
        let read = self.reads.entry(place.to_owned()).or_default();
        if read.reading {
            return;
        }
        read.reading = true;
        let place = place.to_owned();
        self.call(req::<AgentsReadRequest>(json!({ "target": target(&place) })), cx, move |s, answer: Result<ReportReply, String>, _| {
            let read = s.reads.entry(place).or_default();
            read.reading = false;
            match answer.map(|r| reshape::<Report>(&r.report)) {
                Ok(Some(report)) => {
                    read.report = Some(report);
                    read.error = None;
                    read.read_at = Some(SystemTime::now());
                }
                Ok(None) => read.error = Some("The agents report did not read.".to_owned()),
                Err(said) => read.error = Some(said),
            }
        });
    }

    /// The broker's word for this computer's rows, while a provider is set up.
    pub fn ask_broker(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.broker_asked, true) {
            return;
        }
        self.call(req::<BrokerStatusRequest>(json!({})), cx, |s, answer, _| {
            s.broker = answer.ok().and_then(|b| reshape::<Broker>(&b));
        });
    }

    /// The usage accounts, for the plan and the address an agent's head names.
    pub fn ask_accounts(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.accounts_asked, true) {
            return;
        }
        self.call(req::<UsageAccountsRequest>(json!({})), cx, |s, answer, _| {
            let accounts = answer.ok().and_then(|a| serde_json::to_value(&a).ok()).and_then(|v| v.get("accounts").cloned());
            s.accounts = Some(accounts.and_then(|a| serde_json::from_value(a).ok()).unwrap_or_default());
        });
    }

    /// Follows the host's agents events once the host answers; until then each frame asks again.
    fn follow(&mut self, cx: &mut Context<Self>) {
        if self._follow.is_some() {
            return;
        }
        if let Some((host, runtime)) = self.acts.host(cx) {
            self.listen(host, runtime, cx);
        }
    }

    fn listen(&mut self, host: Arc<HostClient>, runtime: Arc<Runtime>, cx: &mut Context<Self>) {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Value>();
        let task = runtime.spawn(async move {
            let Ok(mut events) = host.events().await else { return };
            while let Some(next) = events.next().await {
                let value = match next {
                    Ok(Event::SignIn(step)) => serde_json::to_value(&*step).ok(),
                    Ok(Event::Host(event)) => {
                        serde_json::to_value(&*event).ok().filter(|v| v.get("type").and_then(Value::as_str) == Some("agents.changed"))
                    }
                    _ => None,
                };
                if let Some(value) = value {
                    if tx.unbounded_send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let me = cx.entity().downgrade();
        let hear = cx.spawn(async move |_, cx| {
            while let Some(event) = rx.next().await {
                if me.update(cx, |s, cx| s.hear(event, cx)).is_err() {
                    break;
                }
            }
        });
        self._follow = Some(Following { _task: hear, abort: task.abort_handle() });
        self.rejoin(cx);
    }

    /// The host's runs, drawn on their rows: a sign-in started in another window, or before this one opened.
    fn rejoin(&mut self, cx: &mut Context<Self>) {
        self.call(req::<AgentsSignInsRequest>(json!({})), cx, |s, answer: Result<SignIns, String>, cx| {
            let Ok(SignIns { runs }) = answer else { return };
            for run in runs.iter().filter_map(|r| serde_json::to_value(r).ok()) {
                if run.get("ended").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                let Some(place) = run.pointer("/target/placeId").and_then(Value::as_str) else { continue };
                let Some(agent) = run.get("agent").and_then(Value::as_str) else { continue };
                let Some(id) = run.get("signInId").and_then(Value::as_str) else { continue };
                let row = match run.get("server").and_then(Value::as_str) {
                    None => agent_row(agent),
                    Some(server) => {
                        let Some(scope) = run.get("scope").and_then(Value::as_str) else { continue };
                        server_row(agent, scope, run.get("project").and_then(Value::as_str), server)
                    }
                };
                let key = (place.to_owned(), row);
                if s.flows.get(&key).is_some_and(|f| !matches!(f, Flow::Run { state, .. } if state == "failed")) {
                    continue;
                }
                s.flows.insert(key, Flow::running(Some(id.to_owned())));
                if let Some(last) = run.get("last").cloned() {
                    s.hear(last, cx);
                }
            }
        });
    }

    fn hear(&mut self, event: Value, cx: &mut Context<Self>) {
        match event.get("type").and_then(Value::as_str) {
            Some("agents.changed") => {
                let place = event.pointer("/target/placeId").and_then(Value::as_str).map(str::to_owned);
                let places: Vec<String> = match place {
                    Some(place) => vec![place],
                    None => self.reads.keys().cloned().collect(),
                };
                let read: Vec<String> = places.into_iter().filter(|p| self.reads.contains_key(p)).collect();
                for place in read {
                    self.refresh(&place, cx);
                }
                if self.broker_asked {
                    self.broker_asked = false;
                    self.ask_broker(cx);
                }
            }
            Some("agents.signIn") => {
                let Ok(step) = serde_json::from_value::<Step>(event) else { return };
                let key = self
                    .flows
                    .iter()
                    .find(|(_, f)| matches!(f, Flow::Run { id: Some(id), .. } if *id == step.sign_in_id))
                    .map(|(k, _)| k.clone());
                match key {
                    Some(key) => self.step(key, step),
                    None => {
                        self.early.insert(step.sign_in_id.clone(), step);
                    }
                }
                cx.notify();
            }
            _ => {}
        }
    }

    /// One step onto its run: a signed-in run leaves its row, a failed one stays drawn.
    fn step(&mut self, key: Key, step: Step) {
        if step.state == "signed-in" {
            self.flows.remove(&key);
            return;
        }
        let Some(Flow::Run { url, paste, pty, finish, pastes, id, .. }) = self.flows.get(&key).cloned() else { return };
        let failed = step.state == "failed";
        self.flows.insert(
            key,
            Flow::Run {
                id,
                state: step.state,
                finish,
                pastes,
                url: step.url.or(url),
                code: step.code,
                paste: step.paste.unwrap_or(paste),
                said: step.said,
                pty: step.pty_id.is_some() || (pty && !failed),
            },
        );
    }

    /// Starts a sign-in for one row on one computer.
    pub fn start(&mut self, place: &str, row: &str, start: Start, cx: &mut Context<Self>) {
        let key = (place.to_owned(), row.to_owned());
        match start {
            Start::Copy { line, why } => {
                self.flows.insert(key, Flow::Copy { line, why });
            }
            Start::Vault { agent, mint, word } => {
                self.stop(&key, cx);
                self.flows.insert(key, Flow::Vault { agent, mint, word, saving: false, refused: None });
            }
            Start::Run { agent, server, finish, pastes, terminal, cloud } => {
                self.stop(&key, cx);
                let mut flow = Flow::running(None);
                if let Flow::Run { finish: f, pastes: p, .. } = &mut flow {
                    *f = finish;
                    *p = pastes;
                }
                self.flows.insert(key.clone(), flow);
                let started = move |s: &mut Store, answer: Result<SignInStarted, String>, _: &mut Context<Store>| match answer {
                    Ok(SignInStarted { sign_in_id }) => {
                        if let Some(Flow::Run { id, .. }) = s.flows.get_mut(&key) {
                            *id = Some(sign_in_id.clone());
                        }
                        if let Some(step) = s.early.remove(&sign_in_id) {
                            s.step(key, step);
                        }
                    }
                    Err(said) => {
                        if let Some(Flow::Run { state, said: was, .. }) = s.flows.get_mut(&key) {
                            *state = "failed".into();
                            *was = Some(said);
                        }
                    }
                };
                if cloud {
                    self.call(req::<AgentsCloudSignInRequest>(json!({ "agent": agent })), cx, started);
                } else if let Some((name, scope, project)) = server {
                    let mut params = json!({ "target": target(place), "agent": agent, "name": name, "scope": scope });
                    if let Some(project) = project {
                        params["project"] = json!(project);
                    }
                    self.call(req::<ServersSignInRequest>(params), cx, started);
                } else {
                    let mut params = json!({ "target": target(place), "agent": agent });
                    if terminal {
                        params["terminal"] = json!(true);
                    }
                    self.call(req::<AgentsSignInRequest>(params), cx, started);
                }
            }
        }
        cx.notify();
    }

    /// Ends a running sign-in on the host and drops what it drew.
    fn stop(&mut self, key: &Key, cx: &mut Context<Self>) {
        if let Some(Flow::Run { id: Some(id), state, .. }) = self.flows.remove(key) {
            if state != "failed" {
                self.call(req::<AgentsSignInStopRequest>(json!({ "signInId": id })), cx, |_, _, _| {});
            }
        }
    }

    pub fn cancel(&mut self, place: &str, row: &str, cx: &mut Context<Self>) {
        self.stop(&(place.to_owned(), row.to_owned()), cx);
        cx.notify();
    }

    /// The code or the address a sign-in's page handed back.
    pub fn code(&mut self, place: &str, row: &str, code: String, cx: &mut Context<Self>) {
        let key = (place.to_owned(), row.to_owned());
        let Some(Flow::Run { id: Some(id), .. }) = self.flows.get(&key) else { return };
        let id = id.clone();
        self.call(req::<AgentsSignInCodeRequest>(json!({ "signInId": id, "code": code })), cx, move |s, answer, _| {
            if let (Err(said), Some(Flow::Run { state, said: was, .. })) = (answer, s.flows.get_mut(&key)) {
                *state = "failed".into();
                *was = Some(said);
            }
        });
    }

    /// The token or key pasted for an agent, into this host's vault.
    pub fn save(&mut self, place: &str, row: &str, key_text: String, cx: &mut Context<Self>) {
        let key = (place.to_owned(), row.to_owned());
        let Some(Flow::Vault { agent, word, saving, .. }) = self.flows.get_mut(&key) else { return };
        *saving = true;
        let params = json!({ "agent": agent, "key": key_text, "way": word });
        self.call(req::<AgentsKeyRequest>(params), cx, move |s, answer, _| match answer {
            Ok(_) => {
                s.flows.remove(&key);
            }
            Err(said) => {
                if let Some(Flow::Vault { saving, refused, .. }) = s.flows.get_mut(&key) {
                    *saving = false;
                    *refused = Some(said);
                }
            }
        });
        cx.notify();
    }

    /// The wsp tools written into an agent's config.
    pub fn add_tools(&mut self, place: &str, agent: &str, cx: &mut Context<Self>) {
        let key = (place.to_owned(), agent.to_owned());
        self.adding.insert(key.clone());
        let params = json!({ "target": target(place), "agent": agent });
        let row = (place.to_owned(), agent_row(agent));
        self.call(req::<AgentsAddToolsRequest>(params), cx, move |s, answer, _| {
            s.adding.remove(&key);
            if let Err(said) = answer {
                let mut failed = Flow::running(None);
                if let Flow::Run { state, said: was, .. } = &mut failed {
                    *state = "failed".into();
                    *was = Some(said);
                }
                s.flows.insert(row, failed);
            }
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rows_key_is_the_webs() {
        assert_eq!(agent_row("claude"), "agent-claude");
        assert_eq!(server_row("codex", "project", Some("p1"), "notion"), "server-codex-project-p1-notion");
        assert!(Flow::running(None).runs());
        assert!(!Flow::Copy { line: String::new(), why: None }.runs());
    }
}

/// `agents.setup` with its params sent as written: the typed request drops a null, and a null is how Reset and Put back
/// clear the program, the config folder and the launch words.
#[derive(serde::Serialize)]
struct RawSetup(Value);

impl Op for RawSetup {
    const NAME: &'static str = "agents.setup";
    type Reply = wsp_host_client::ops::settings_agents::AgentSetUp;
}
