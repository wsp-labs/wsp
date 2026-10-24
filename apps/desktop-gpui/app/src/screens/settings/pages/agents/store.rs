// SPDX-License-Identifier: AGPL-3.0-only
//! What the Agents pages keep while the window is open, after `settingsStore.ts`'s agents fields and the hooks under
//! `components/agents/`: the tab and the computer read, each computer's report and when it was read, the broker's word,
//! the sign-ins running and the acts on servers, skills and plugins with what is busy and what the host refused. It
//! follows the host's `agents.changed` and `agents.signIn` itself, so a report is read again and a sign-in steps on,
//! and listens again once the socket went, reading everything again as the web does on a reconnect.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use base64::Engine as _;
use futures::StreamExt as _;
use gpui_kit::{App, Context, Entity, Global, Image, ImageFormat, Task, Window};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::ops::settings_agents::{Icon, Report as ReportReply, SignInStarted, SignIns};
use wsp_host_client::ops::thread::Harnesses;
use wsp_host_client::protocol::{
    AccountsAnswer, AccountsAnswerAccountsItem as Account, AgentsAddToolsRequest, AgentsCloudSignInRequest, AgentsKeyRequest,
    AgentsReadRequest, AgentsReport, AgentsSignInCodeRequest, AgentsSignInEvent as Step, AgentsSignInEventState, AgentsSignInRequest,
    AgentsSignInRunTarget, AgentsSignInStopRequest, AgentsSignInsRequest, BrokerStatus, BrokerStatusRequest, EventUnion, EventUnionTarget,
    HarnessCatalog, HarnessesListRequest, ServerToolsAnswer, ServersIconRequest, ServersSignInRequest, SkillHit, UsageAccountsRequest,
};
use wsp_host_client::{Event, HostClient, Op};

use crate::screens::settings::model::req;
use crate::screens::settings::Acts;

/// How long a reading stands before showing the page asks for another (`FRESH_MS`).
const FRESH: Duration = Duration::from_secs(5 * 60);
/// The skills.sh searches kept, the oldest let go past it.
const SEARCHES_KEPT: usize = 32;

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
    pub report: Option<AgentsReport>,
    pub reading: bool,
    pub error: Option<String>,
    pub read_at: Option<SystemTime>,
    /// The showing of the pages the host refused the last read in, which the next showing asks again.
    refused_in: Option<u64>,
}

impl Read {
    /// Whether showing the pages for the `shown`th time asks for this read: none stands, the last was refused before
    /// this showing, or the one standing is older than `FRESH` (`heldFresh`).
    fn due(read: Option<&Read>, shown: u64) -> bool {
        read.is_none_or(|r| {
            !r.reading
                && (r.refused_in.is_some_and(|at| at < shown) || r.read_at.is_some_and(|at| at.elapsed().unwrap_or_default() > FRESH))
        })
    }
}

/// How a sign-in goes where it was pressed (`SignInStart`); `terminal` runs it in a pty there that a terminal drawn in
/// place attaches to.
#[derive(Clone, Debug, PartialEq)]
pub enum Start {
    Run {
        agent: String,
        server: Option<(String, String, Option<String>)>,
        finish: Option<&'static str>,
        pastes: bool,
        cloud: bool,
        terminal: bool,
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
        /// The shell on the row's computer the login runs in, which its terminal drawn in place attaches to.
        pty: Option<String>,
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
        Flow::Run { id, state: "running".into(), finish: None, pastes: false, url: None, code: None, paste: false, said: None, pty: None }
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

/// What the store hears from the host.
enum Heard {
    /// `agents.changed`, for one computer or for all.
    Changed(Option<String>),
    /// One step of a sign-in.
    Step(Box<Step>),
    /// The socket went and the host is heard again, so what changed meanwhile is read again.
    Back,
}

/// One server's tools as its last ask stands.
#[derive(Clone, Default)]
pub struct ToolsState {
    pub listing: bool,
    pub answer: Option<ServerToolsAnswer>,
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
    pub hits: Option<Vec<SkillHit>>,
    pub error: Option<String>,
}

/// What a skill's install is picked to take: the agents, and the project it goes in by id and name.
#[derive(Clone, Default)]
pub struct Picks {
    pub agents: Option<Vec<String>>,
    pub project: Option<(String, String)>,
}

/// Rows and items by the computer they stand on and their key.
pub type Key = (String, String);

pub struct Store {
    acts: Acts,
    pub tab: Tab,
    pub place: Option<String>,
    pub reads: HashMap<String, Read>,
    /// How many times the pages have been shown: Settings opening on them or moving to one.
    showings: u64,
    pub broker: Option<BrokerStatus>,
    broker_asked: bool,
    /// The usage accounts, for the plan and the address an agent's head names.
    pub accounts: Option<Vec<Account>>,
    accounts_asked: bool,
    /// Each computer's own agent lists, as its binaries there answer them (`useAskedCatalogs(computerKey(place))`).
    pub catalogs: HashMap<String, Vec<HarnessCatalog>>,
    catalogs_asked: HashSet<String>,
    pub flows: HashMap<Key, Flow>,
    /// Sign-in steps that came before the answer naming their run, kept only while a run waits for that answer.
    early: HashMap<String, Step>,
    /// The press each row's watched run answers, so a run whose answer lands after Cancel, or after a later press, is
    /// stopped on the host rather than left going there.
    starts: HashMap<Key, u64>,
    pressed: u64,
    pub adding: HashSet<Key>,
    pub tools: HashMap<Key, ToolsState>,
    pub busy: HashSet<Key>,
    pub refused: HashMap<Key, String>,
    pub previews: HashMap<Key, Doc>,
    pub remote: HashMap<String, Doc>,
    searches: HashMap<String, Search>,
    /// The searches' words, oldest first, so the oldest past `SEARCHES_KEPT` is let go.
    searched: VecDeque<String>,
    pub picks: HashMap<String, Picks>,
    /// The report each computer's server checks were last asked for, by when it was read.
    pub shown: HashMap<String, String>,
    /// The add's search as last asked of skills.sh, and the words waiting out the pause before they are.
    pub add_asked: String,
    pub add_pending: Option<String>,
    /// Each server address's icon as the host answered it, an image or none, absent until asked (`useServerIcon`); the
    /// ones being asked; and the ones a Read again dropped, whose next ask has the host fetch again.
    icons: HashMap<String, Option<Arc<Image>>>,
    icons_asking: HashSet<String>,
    icons_again: HashSet<String>,
    /// The open sheet's write: whether it runs, and the host's refusal.
    pub sheet_saving: bool,
    pub sheet_refusal: Option<String>,
    /// The environment sheet's names kept, while it is open; the values staged are the sheet's own.
    pub env_kept: Vec<String>,
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

/// A server icon's data url as an image, only of the kinds the host hands back (`ICON_DATA_URL`).
fn icon_of(url: &str) -> Option<Arc<Image>> {
    let (kind, data) = url.strip_prefix("data:image/")?.split_once(";base64,")?;
    let format = match kind {
        "png" => ImageFormat::Png,
        "x-icon" => ImageFormat::Ico,
        "gif" => ImageFormat::Gif,
        "webp" => ImageFormat::Webp,
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD.decode(data).ok()?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

/// Each host's store, made with the first Settings that host's window opens and kept until that window closes, so
/// a report read inside its hold is not asked again as Settings opens again, nor anything the pages hold lost.
#[derive(Default)]
struct Kept(HashMap<usize, (gpui_kit::WindowId, Entity<Store>)>);

impl Global for Kept {}

/// The store kept for `host`, or the one `make` makes, kept from now on.
pub fn kept(host: &Arc<HostClient>, window: &Window, cx: &mut App, make: impl FnOnce(&mut App) -> Entity<Store>) -> Entity<Store> {
    if !cx.has_global::<Kept>() {
        cx.set_global(Kept::default());
        cx.on_window_closed(|cx, closed| cx.global_mut::<Kept>().0.retain(|_, (window, _)| *window != closed)).detach();
    }
    let key = Arc::as_ptr(host) as usize;
    if let Some((_, store)) = cx.global::<Kept>().0.get(&key) {
        return store.clone();
    }
    let store = make(cx);
    cx.global_mut::<Kept>().0.insert(key, (window.window_handle().window_id(), store.clone()));
    store
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
            showings: 0,
            broker: None,
            broker_asked: false,
            accounts: None,
            accounts_asked: false,
            catalogs: HashMap::new(),
            catalogs_asked: HashSet::new(),
            flows: HashMap::new(),
            early: HashMap::new(),
            starts: HashMap::new(),
            pressed: 0,
            adding: HashSet::new(),
            tools: HashMap::new(),
            busy: HashSet::new(),
            refused: HashMap::new(),
            previews: HashMap::new(),
            remote: HashMap::new(),
            searches: HashMap::new(),
            searched: VecDeque::new(),
            picks: HashMap::new(),
            shown: HashMap::new(),
            add_asked: String::new(),
            add_pending: None,
            icons: HashMap::new(),
            icons_asking: HashSet::new(),
            icons_again: HashSet::new(),
            sheet_saving: false,
            sheet_refusal: None,
            env_kept: Vec::new(),
            pending: Vec::new(),
            _follow: None,
        }
    }

    /// The screen's store with this frame's acts laid on, and what waited on the host sent once it answers.
    pub fn of(store: &Entity<Store>, acts: &Acts, cx: &mut App) -> Entity<Store> {
        store.update(cx, |s, cx| {
            s.acts = acts.clone();
            if s.acts.host(cx).is_some() {
                for call in std::mem::take(&mut s.pending) {
                    call(s, cx);
                }
            }
            s.follow(cx);
        });
        store.clone()
    }

    /// `pickAgentsPlace`: the computer the pages read, picked from a computer's own page.
    pub fn pick_place(&mut self, place: String, cx: &mut Context<Self>) {
        self.place = Some(place);
        self.show();
        cx.notify();
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

    /// The computer's report, asked where none stands, the last was refused before this showing, or the one standing
    /// is older than `FRESH`.
    pub fn read(&mut self, place: &str, cx: &mut Context<Self>) {
        if Read::due(self.reads.get(place), self.showings) {
            self.refresh(place, cx);
        }
    }

    /// The pages are shown again, which asks again for any read the host refused before.
    pub fn show(&mut self) {
        self.showings += 1;
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
            let showing = s.showings;
            let read = s.reads.entry(place).or_default();
            read.reading = false;
            match answer {
                Ok(ReportReply { report }) => {
                    read.report = Some(report);
                    read.error = None;
                    read.read_at = Some(SystemTime::now());
                    read.refused_in = None;
                }
                Err(said) => {
                    read.error = Some(said);
                    read.refused_in = Some(showing);
                }
            }
        });
    }

    /// The broker's word for this computer's rows, while a provider is set up.
    pub fn ask_broker(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.broker_asked, true) {
            return;
        }
        self.call(req::<BrokerStatusRequest>(json!({})), cx, |s, answer, _| {
            s.broker = answer.ok();
        });
    }

    /// The usage accounts, for the plan and the address an agent's head names.
    pub fn ask_accounts(&mut self, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.accounts_asked, true) {
            return;
        }
        self.call(req::<UsageAccountsRequest>(json!({})), cx, |s, answer: Result<AccountsAnswer, String>, _| {
            s.accounts = Some(answer.map(|a| a.accounts).unwrap_or_default());
        });
    }

    /// The computer's own agent lists, asked once a store; until they answer, and where the host refuses them, the
    /// host-wide lists stand in.
    pub fn ask_catalogs(&mut self, place: &str, cx: &mut Context<Self>) {
        if !self.catalogs_asked.insert(place.to_owned()) {
            return;
        }
        let place = place.to_owned();
        self.call(req::<HarnessesListRequest>(json!({ "computer": &place })), cx, move |s, answer: Result<Harnesses, String>, _| {
            if let Ok(Harnesses { harnesses }) = answer {
                s.catalogs.insert(place, harnesses);
            }
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

    /// Listens on the host's socket, and once it went, or the host would not take the listen, listens again after the
    /// Activity link's backoff.
    fn listen(&mut self, host: Arc<HostClient>, runtime: Arc<Runtime>, cx: &mut Context<Self>) {
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Heard>();
        let task = runtime.spawn(async move {
            let (mut back, mut attempt) = (false, 0u32);
            loop {
                if let Ok(mut events) = host.events().await {
                    if back && tx.unbounded_send(Heard::Back).is_err() {
                        return;
                    }
                    attempt = 0;
                    while let Some(next) = events.next().await {
                        let heard = match next {
                            Ok(Event::SignIn(step)) => Heard::Step(step),
                            Ok(Event::Host(event)) => match *event {
                                EventUnion::AgentsChanged { target, .. } => Heard::Changed(match target {
                                    Some(EventUnionTarget::Variant0 { place_id, .. }) => Some(place_id),
                                    _ => None,
                                }),
                                _ => continue,
                            },
                            _ => continue,
                        };
                        if tx.unbounded_send(heard).is_err() {
                            return;
                        }
                    }
                }
                if tx.is_closed() {
                    return;
                }
                back = true;
                attempt += 1;
                crate::screens::settings::pages::activity::redial(attempt).await;
            }
        });
        let me = cx.entity().downgrade();
        let hear = cx.spawn(async move |_, cx| {
            while let Some(heard) = rx.next().await {
                if me.update(cx, |s, cx| s.hear(heard, cx)).is_err() {
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
            for run in runs {
                if run.ended == Some(true) {
                    continue;
                }
                let AgentsSignInRunTarget::Variant0 { place_id: place, .. } = run.target else { continue };
                let row = match (&run.server, run.scope) {
                    (None, _) => agent_row(&run.agent),
                    (Some(server), Some(scope)) => server_row(&run.agent, &scope.to_string(), run.project.as_deref(), server),
                    (Some(_), None) => continue,
                };
                let key = (place, row);
                if s.flows.get(&key).is_some_and(|f| !matches!(f, Flow::Run { state, .. } if state == "failed")) {
                    continue;
                }
                s.flows.insert(key, Flow::running(Some(run.sign_in_id)));
                if let Some(last) = run.last {
                    s.hear(Heard::Step(Box::new(last)), cx);
                }
            }
        });
    }

    /// Whether a sign-in pressed here still waits for the answer that names its run.
    fn awaiting(&self) -> bool {
        self.flows.values().any(|f| matches!(f, Flow::Run { id: None, state, .. } if state != "failed"))
    }

    fn hear(&mut self, heard: Heard, cx: &mut Context<Self>) {
        match heard {
            Heard::Changed(place) => {
                let places: Vec<String> = match place {
                    Some(place) => vec![place],
                    None => self.reads.keys().cloned().collect(),
                };
                let read: Vec<String> = places.into_iter().filter(|p| self.reads.contains_key(p)).collect();
                for place in read {
                    self.refresh(&place, cx);
                }
                self.ask_again(cx);
            }
            Heard::Back => {
                let places: Vec<String> = self.reads.keys().cloned().collect();
                for place in places {
                    self.refresh(&place, cx);
                }
                self.ask_again(cx);
                self.rejoin(cx);
            }
            Heard::Step(step) => {
                let key = self
                    .flows
                    .iter()
                    .find(|(_, f)| matches!(f, Flow::Run { id: Some(id), .. } if *id == step.sign_in_id))
                    .map(|(k, _)| k.clone());
                match key {
                    Some(key) => self.step(key, *step),
                    None if self.awaiting() => {
                        self.early.insert(step.sign_in_id.clone(), *step);
                    }
                    None => {}
                }
                cx.notify();
            }
        }
    }

    /// The broker's word and the accounts asked again where they were asked, since a sign-in moves both.
    fn ask_again(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.broker_asked) {
            self.ask_broker(cx);
        }
        if std::mem::take(&mut self.accounts_asked) {
            self.ask_accounts(cx);
        }
    }

    /// The search for these words, if it was asked.
    pub fn search(&self, q: &str) -> Option<&Search> {
        self.searches.get(q)
    }

    /// A search asked of skills.sh, the oldest let go past `SEARCHES_KEPT`.
    pub fn search_asked(&mut self, q: String) {
        self.searched.retain(|w| *w != q);
        self.searched.push_back(q.clone());
        self.searches.insert(q, Search { reading: true, ..Search::default() });
        while self.searched.len() > SEARCHES_KEPT {
            if let Some(gone) = self.searched.pop_front() {
                self.searches.remove(&gone);
            }
        }
    }

    /// Its answer laid on it, unless it was let go meanwhile.
    pub fn search_answered(&mut self, q: &str, answer: Result<Vec<SkillHit>, String>) {
        let Some(search) = self.searches.get_mut(q) else { return };
        search.reading = false;
        match answer {
            Ok(hits) => search.hits = Some(hits),
            Err(said) => search.error = Some(said),
        }
    }

    /// A server's own icon as the host answered it, none where it has none or while server icons are off.
    pub fn icon(&self, host: &str, on: bool) -> Option<Arc<Image>> {
        self.icons.get(host).filter(|_| on).cloned().flatten()
    }

    /// Asks the host for a server's own icon, once per address, never while server icons are off (`useServerIcon`).
    pub fn ask_icon(&mut self, host: &str, on: bool, cx: &mut Context<Self>) {
        let Some(ask) = self.icon_ask(host, on) else { return };
        let host = host.to_owned();
        // A refusal or a lost socket draws the glyph until a Read again asks once more.
        self.call(req::<ServersIconRequest>(ask), cx, move |s, answer: Result<Icon, String>, _| {
            s.icon_answered(host, answer.ok().and_then(|a| a.icon));
        });
    }

    /// The ask for a server's icon where one is due, marked as asked; a host a Read again dropped is fetched afresh.
    fn icon_ask(&mut self, host: &str, on: bool) -> Option<Value> {
        if !on || self.icons.contains_key(host) || !self.icons_asking.insert(host.to_owned()) {
            return None;
        }
        let mut ask = json!({ "host": host });
        if self.icons_again.remove(host) {
            ask["refresh"] = json!(true);
        }
        Some(ask)
    }

    pub(super) fn icon_answered(&mut self, host: String, url: Option<String>) {
        self.icons_asking.remove(&host);
        self.icons.insert(host, url.as_deref().and_then(icon_of));
    }

    /// Every icon asked again at its next draw, the host fetching it afresh (`forgetServerIcons`).
    pub fn forget_icons(&mut self) {
        self.icons_again.extend(self.icons.drain().map(|(host, _)| host));
    }

    /// One step onto its run: a signed-in run leaves its row, a failed one stays drawn.
    fn step(&mut self, key: Key, step: Step) {
        if step.state == AgentsSignInEventState::SignedIn {
            self.flows.remove(&key);
            return;
        }
        let Some(Flow::Run { url, paste, pty, finish, pastes, id, .. }) = self.flows.get(&key).cloned() else { return };
        let failed = step.state == AgentsSignInEventState::Failed;
        self.flows.insert(
            key,
            Flow::Run {
                id,
                state: step.state.to_string(),
                finish,
                pastes,
                url: step.url.or(url),
                code: step.code,
                paste: step.paste.unwrap_or(paste),
                said: step.said,
                pty: step.pty_id.or(pty.filter(|_| !failed)),
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
            Start::Run { agent, server, finish, pastes, cloud, terminal } => {
                self.stop(&key, cx);
                let mut flow = Flow::running(None);
                if let Flow::Run { finish: f, pastes: p, .. } = &mut flow {
                    *f = finish;
                    *p = pastes;
                }
                self.flows.insert(key.clone(), flow);
                self.pressed += 1;
                let mine = self.pressed;
                self.starts.insert(key.clone(), mine);
                let started = move |s: &mut Store, answer: Result<SignInStarted, String>, cx: &mut Context<Store>| match answer {
                    Ok(SignInStarted { sign_in_id }) if s.starts.get(&key) != Some(&mine) => {
                        s.early.remove(&sign_in_id);
                        s.call(req::<AgentsSignInStopRequest>(json!({ "signInId": sign_in_id })), cx, |_, _, _| {});
                    }
                    Ok(SignInStarted { sign_in_id }) => {
                        if let Some(Flow::Run { id, .. }) = s.flows.get_mut(&key) {
                            *id = Some(sign_in_id.clone());
                        }
                        if let Some(step) = s.early.remove(&sign_in_id) {
                            s.step(key, step);
                        }
                        if !s.awaiting() {
                            s.early.clear();
                        }
                    }
                    Err(said) => {
                        if s.starts.get(&key) != Some(&mine) {
                            return;
                        }
                        if let Some(Flow::Run { state, said: was, .. }) = s.flows.get_mut(&key) {
                            *state = "failed".into();
                            *was = Some(said);
                        }
                        if !s.awaiting() {
                            s.early.clear();
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
        self.starts.remove(key);
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
    use gpui_kit::AppContext as _;

    use super::*;

    fn store(cx: &mut gpui_kit::TestAppContext) -> Entity<Store> {
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        cx.update(|cx| cx.new(|_| Store::new(Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime))))
    }

    fn step(id: &str) -> Heard {
        Heard::Step(Box::new(serde_json::from_value(json!({ "type": "agents.signIn", "signInId": id, "state": "waiting" })).unwrap()))
    }

    #[gpui_kit::test]
    fn a_read_the_host_refused_is_asked_again_at_the_next_showing(cx: &mut gpui_kit::TestAppContext) {
        let store = store(cx);
        store.update(cx, |s, cx| {
            let refused = Read { error: Some("box1 did not answer".into()), refused_in: Some(s.showings), ..Read::default() };
            s.reads.insert("box1".into(), refused);
            s.reads.insert("here".into(), Read { read_at: Some(SystemTime::now()), ..Read::default() });
            s.read("box1", cx);
            assert!(!s.reads["box1"].reading, "nothing asked again in the showing the host refused it in");
            s.show();
            s.read("box1", cx);
            s.read("here", cx);
            assert!(s.reads["box1"].reading, "the next showing asks for the refused read again");
            assert!(!s.reads["here"].reading, "and holds a report read inside FRESH");
        });
    }

    #[gpui_kit::test]
    fn a_server_icon_is_asked_once_per_address_while_icons_are_on(cx: &mut gpui_kit::TestAppContext) {
        let store = store(cx);
        let png = "data:image/png;base64,iVBORw0KGgo=";
        store.update(cx, |s, _| {
            assert_eq!(s.icon_ask("mcp.notion.com", false), None, "nothing is asked while server icons are off");
            assert_eq!(s.icon_ask("mcp.notion.com", true), Some(json!({ "host": "mcp.notion.com" })));
            assert_eq!(s.icon_ask("mcp.notion.com", true), None, "an ask on its way is not asked again");
            s.icon_answered("mcp.notion.com".into(), Some(png.into()));
            assert!(s.icon("mcp.notion.com", true).is_some(), "the answer is drawn");
            assert!(s.icon("mcp.notion.com", false).is_none(), "and not once icons are off");
            assert_eq!(s.icon_ask("mcp.notion.com", true), None, "an address answered is held for the window");
            s.icon_answered("example.com".into(), Some("data:text/html;base64,PGI+".into()));
            assert!(s.icon("example.com", true).is_none(), "an answer that is no icon draws the glyph");
            s.forget_icons();
            assert!(s.icon("mcp.notion.com", true).is_none(), "a Read again drops every icon");
            assert_eq!(
                s.icon_ask("mcp.notion.com", true),
                Some(json!({ "host": "mcp.notion.com", "refresh": true })),
                "and asks it afresh"
            );
        });
    }

    #[gpui_kit::test]
    fn a_change_asks_the_accounts_again_so_a_sign_in_shows_its_plan(cx: &mut gpui_kit::TestAppContext) {
        let store = store(cx);
        store.update(cx, |s, cx| {
            s.ask_accounts(cx);
            s.ask_accounts(cx);
            assert_eq!(s.pending.len(), 1, "asked once while the answer stands");
            s.hear(Heard::Changed(None), cx);
            assert_eq!(s.pending.len(), 2, "a change asks them again");
        });
    }

    #[gpui_kit::test]
    fn a_step_no_press_waits_for_is_not_kept(cx: &mut gpui_kit::TestAppContext) {
        let store = store(cx);
        store.update(cx, |s, cx| {
            s.hear(step("other-window"), cx);
            assert!(s.early.is_empty(), "nothing pressed here waits for a run's answer");
            let start = Start::Run { agent: "claude".into(), server: None, finish: None, pastes: false, cloud: false, terminal: false };
            s.start("here", &agent_row("claude"), start, cx);
            s.hear(step("mine"), cx);
            assert_eq!(s.early.len(), 1, "a press waits for its answer, so a step that came first is kept for it");
        });
    }

    #[gpui_kit::test]
    fn the_searches_kept_stop_at_their_bound(cx: &mut gpui_kit::TestAppContext) {
        let store = store(cx);
        store.update(cx, |s, _| {
            for n in 0..SEARCHES_KEPT + 8 {
                s.search_asked(format!("q{n}"));
            }
            assert_eq!(s.searches.len(), SEARCHES_KEPT);
            assert!(s.search("q0").is_none() && s.search(&format!("q{}", SEARCHES_KEPT + 7)).is_some(), "the oldest went first");
            s.search_answered("q0", Ok(Vec::new()));
            assert!(s.search("q0").is_none(), "an answer to a search let go does not bring it back");
        });
    }

    /// A store whose acts reach `door`, as the screen's do once its host answered.
    fn store_on(door: &crate::screens::settings::door::Door, cx: &mut gpui_kit::TestAppContext) -> Entity<Store> {
        cx.executor().allow_parking();
        let runtime = crate::screens::settings::door::runtime();
        let host = door.client(&runtime);
        cx.update(|cx| cx.new(|_| Store::new(Acts::new(gpui_kit::WeakEntity::new_invalid(), Some(host), runtime))))
    }

    /// Runs the app in real time until `done` holds.
    fn waits(cx: &mut gpui_kit::TestAppContext, what: &str, mut done: impl FnMut(&mut gpui_kit::TestAppContext) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(std::time::Instant::now() < deadline, "never came to: {what}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    #[gpui_kit::test]
    fn the_agent_lists_are_asked_of_the_picked_computer(cx: &mut gpui_kit::TestAppContext) {
        let own = json!({ "harness": "opencode", "label": "OpenCode", "source": "table", "version": null, "efforts": [],
            "models": [{ "value": "zeta/one", "label": "Zeta One" }], "contextWindows": [], "permissionModes": [],
            "steers": false, "renames": false, "images": false });
        let door = crate::screens::settings::door::Door::start(json!({ "harnesses.list": { "reply": { "harnesses": [own] } } }));
        let store = store_on(&door, cx);
        store.update(cx, |s, cx| s.ask_catalogs("box1", cx));
        waits(cx, "box1's lists", |cx| store.read_with(cx, |s, _| s.catalogs.contains_key("box1")));
        let asked = door.asked_of("harnesses.list");
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0]["computer"], "box1", "the read names the computer, so its own OpenCode answers");
        assert_eq!(store.read_with(cx, |s, _| s.catalogs["box1"][0].models.len()), 1);
    }

    #[gpui_kit::test]
    fn cancel_before_the_sign_in_answers_stops_its_run_on_the_host(cx: &mut gpui_kit::TestAppContext) {
        let door = crate::screens::settings::door::Door::start(json!({
            "agents.signIn": { "reply": { "signInId": "si_agent" }, "delayMs": 200 },
            "servers.signIn": { "reply": { "signInId": "si_server" }, "delayMs": 200 },
            "agents.signInStop": { "reply": {} },
        }));
        let store = store_on(&door, cx);
        let server = Some(("notion".to_owned(), "user".to_owned(), None));
        let rows = [
            (
                agent_row("claude"),
                Start::Run { agent: "claude".into(), server: None, finish: None, pastes: false, cloud: false, terminal: false },
            ),
            (
                server_row("claude", "user", None, "notion"),
                Start::Run { agent: "claude".into(), server, finish: None, pastes: false, cloud: false, terminal: false },
            ),
        ];
        store.update(cx, |s, cx| {
            for (row, start) in rows {
                s.start("here", &row, start, cx);
                s.cancel("here", &row, cx);
            }
        });
        waits(cx, "both runs stopped", |_| door.asked_of("agents.signInStop").len() == 2);
        let stopped: Vec<Value> = door.asked_of("agents.signInStop").iter().map(|f| f["signInId"].clone()).collect();
        assert!(stopped.contains(&json!("si_agent")) && stopped.contains(&json!("si_server")), "{stopped:?}");
        assert!(store.read_with(cx, |s, _| s.flows.is_empty()), "nothing of either is drawn");
    }

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
