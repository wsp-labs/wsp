// SPDX-License-Identifier: AGPL-3.0-only
//! The agents area's fake context and fake adapter. The context keeps the runtime's rows and records in `StateCell`s,
//! answers each call out from what a test configured, and logs every call as `name args` in order. The adapter
//! answers each optional ability only when a test gave it one, and logs each call it gets.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AsideAnswer, AsideQuestion, AttachmentRoad, Env, ExecFailure, ExecFailureKind, ExecOptions, ExecStream,
    ExecStreamFactory, ForkTurn, ForkedSession, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessExec, HarnessSession,
    HarnessStartOptions, RevertAsk, Reverted, ScreenCommand, SessionRenameWrite, TitleTurn,
};
use wsp_core::clock::Clock;
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::events::EventBus;
use wsp_core::types::harness::{HarnessAdapter, HarnessAdapterContext, HarnessAdapterFactory};
use wsp_core::types::internal::ServerValuesRoad;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::{
    BytesLanded, ExecOptions as MachineExecOptions, Machine, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions,
    SnapshotOptions,
};
use wsp_machines::machines::kinds::BrokeredLaunch;
use wsp_store::store::Store;
use wsp_threads::threads::agents::{AgentsArea, AgentsCtx, SetupLaunch, SignsIn, TurnLaunch};
use wsp_threads::threads::threads::{Entry, Sessions, ThreadRecords};
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::{err, exec_result, CountingIds, FakeMachine, FakeStore, TestBus, TestClock};

// ---- a gate a test opens ---------------------------------------------------------------------------------------

/// A held answer: every call that reaches the gate waits until the test opens it.
#[derive(Clone)]
pub struct Gate {
    tx: Arc<tokio::sync::watch::Sender<bool>>,
}

impl Gate {
    pub fn new() -> Gate {
        let (tx, _rx) = tokio::sync::watch::channel(false);
        Gate { tx: Arc::new(tx) }
    }
    pub fn open(&self) {
        self.tx.send_replace(true);
    }
    fn wait(&self) -> LocalBoxFuture<'static, ()> {
        let mut rx = self.tx.subscribe();
        async move {
            loop {
                if *rx.borrow() {
                    break;
                }
                if rx.changed().await.is_err() {
                    break;
                }
            }
        }
        .boxed_local()
    }
}

impl Default for Gate {
    fn default() -> Self {
        Gate::new()
    }
}

// ---- one optional ability of the fake adapter ------------------------------------------------------------------

/// One answer of an ability, with a gate the call waits at before it gives it.
#[derive(Clone)]
struct Step<T> {
    result: Result<T, ExecFailure>,
    gate: Option<Gate>,
}

/// What an ability answers: its answers in order, the last one repeating; optionally a command it runs on the
/// handed exec first (so the machine's log shows its timeout), and a gate each answer may wait at.
pub struct Ability<T> {
    queue: Mutex<VecDeque<Step<T>>>,
    last: Step<T>,
    pub exec: Option<String>,
}

impl<T: Clone + Send + 'static> Ability<T> {
    pub fn answering(v: T) -> Ability<T> {
        Ability::sequence(vec![Ok(v)])
    }
    pub fn failing(message: &str) -> Ability<T> {
        Ability::sequence(vec![Err(ExecFailure { kind: ExecFailureKind::Other, message: message.to_string() })])
    }
    /// The answers in order; the last one answers every call after.
    pub fn sequence(answers: Vec<Result<T, ExecFailure>>) -> Ability<T> {
        Ability::steps(answers.into_iter().map(|a| (a, None)).collect())
    }
    /// The answers in order, each with the gate its call waits at, if any.
    pub fn steps(mut answers: Vec<(Result<T, ExecFailure>, Option<Gate>)>) -> Ability<T> {
        let (result, gate) = answers.pop().expect("an answer");
        let queue = answers.into_iter().map(|(result, gate)| Step { result, gate }).collect();
        Ability { queue: Mutex::new(queue), last: Step { result, gate }, exec: None }
    }
    pub fn execing(mut self, command: &str) -> Ability<T> {
        self.exec = Some(command.to_string());
        self
    }
    /// Holds the last answer at `gate`.
    pub fn gated(mut self, gate: &Gate) -> Ability<T> {
        self.last.gate = Some(gate.clone());
        self
    }
    fn next(&self) -> Step<T> {
        self.queue.lock().expect("queue").pop_front().unwrap_or_else(|| self.last.clone())
    }
    fn run(&self, exec: Arc<dyn HarnessExec>) -> LocalBoxFuture<'static, Result<T, ExecFailure>> {
        let step = self.next();
        let command = self.exec.clone();
        async move {
            if let Some(command) = command {
                exec.exec(&command, None).await?;
            }
            if let Some(gate) = step.gate {
                gate.wait().await;
            }
            step.result
        }
        .boxed_local()
    }
}

// ---- the fake adapter ------------------------------------------------------------------------------------------

/// A harness adapter with every optional ability off until a test gives it one. Each call it gets is logged in
/// `calls` (`probeCatalog`, `probeVersion`, `sessionTitle <id>`, `renameSession <id> <title>`, `titleFor`,
/// `accountModels <version>`); a `title_for` also keeps its turn in `title_turns`.
pub struct FakeAdapter {
    pub steers: bool,
    pub probe_catalog: Option<Ability<Option<HarnessCatalogAnswer>>>,
    pub probe_version: Option<Ability<Option<String>>>,
    pub session_title: Option<Ability<Option<String>>>,
    pub rename_session: Option<Ability<SessionRenameWrite>>,
    pub title_for: Option<Ability<Option<String>>>,
    pub account_models: Option<Ability<Option<Vec<HarnessCatalogModelProbe>>>>,
    pub attachments: Option<AttachmentRoad>,
    pub mcp_servers: bool,
    pub denies_servers: bool,
    pub moves_access: bool,
    pub steers_images: bool,
    pub aside: bool,
    pub compacts: Option<String>,
    pub terminal_resume: Option<String>,
    pub forks: bool,
    pub reverts: bool,
    pub forks_by_count: bool,
    pub screen_commands: Vec<ScreenCommand>,
    pub conversations: Option<Arc<dyn wsp_adapters::port::ConversationStore>>,
    pub calls: Mutex<Vec<String>>,
    pub title_turns: Mutex<Vec<TitleTurn>>,
}

impl FakeAdapter {
    pub fn new() -> FakeAdapter {
        FakeAdapter {
            steers: false,
            probe_catalog: None,
            probe_version: None,
            session_title: None,
            rename_session: None,
            title_for: None,
            account_models: None,
            attachments: None,
            mcp_servers: false,
            denies_servers: false,
            moves_access: false,
            steers_images: false,
            aside: false,
            compacts: None,
            terminal_resume: None,
            forks: false,
            reverts: false,
            forks_by_count: false,
            screen_commands: vec![],
            conversations: None,
            calls: Mutex::new(vec![]),
            title_turns: Mutex::new(vec![]),
        }
    }
    pub fn steering(mut self) -> Self {
        self.steers = true;
        self
    }
    pub fn probing(mut self, a: Ability<Option<HarnessCatalogAnswer>>) -> Self {
        self.probe_catalog = Some(a);
        self
    }
    pub fn versioned(mut self, a: Ability<Option<String>>) -> Self {
        self.probe_version = Some(a);
        self
    }
    pub fn titled(mut self, a: Ability<Option<String>>) -> Self {
        self.session_title = Some(a);
        self
    }
    pub fn renaming(mut self, a: Ability<SessionRenameWrite>) -> Self {
        self.rename_session = Some(a);
        self
    }
    pub fn titling(mut self, a: Ability<Option<String>>) -> Self {
        self.title_for = Some(a);
        self
    }
    pub fn accounting(mut self, a: Ability<Option<Vec<HarnessCatalogModelProbe>>>) -> Self {
        self.account_models = Some(a);
        self
    }
    pub fn arc(self) -> Arc<FakeAdapter> {
        Arc::new(self)
    }
    pub fn called(&self) -> Vec<String> {
        self.calls.lock().expect("calls").clone()
    }
    fn log(&self, line: String) {
        self.calls.lock().expect("calls").push(line);
    }
}

impl Default for FakeAdapter {
    fn default() -> Self {
        FakeAdapter::new()
    }
}

impl HarnessAdapter for FakeAdapter {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("the fake adapter starts nothing".to_string()))
    }
    fn steers(&self) -> bool {
        self.steers
    }
    fn renames(&self) -> bool {
        self.rename_session.is_some()
    }
    fn asides(&self) -> bool {
        self.aside
    }
    fn account_models(
        &self,
        version: Option<String>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Vec<HarnessCatalogModelProbe>>, String>>> {
        let ability = self.account_models.as_ref()?;
        self.log(format!("accountModels {}", version.unwrap_or_default()));
        let step = ability.next();
        Some(
            async move {
                if let Some(gate) = step.gate {
                    gate.wait().await;
                }
                step.result.map_err(|e| e.message)
            }
            .boxed_local(),
        )
    }
    fn revert(&self, _ask: RevertAsk) -> Option<LocalBoxFuture<'static, Result<Reverted, String>>> {
        self.reverts.then(|| async { Err("the fake adapter reverts nothing".to_string()) }.boxed_local())
    }
    fn fork_session(&self, _session: String, _turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        self.forks.then(|| async { Err("the fake adapter forks nothing".to_string()) }.boxed_local())
    }
    fn reverts(&self) -> bool {
        self.reverts
    }
    fn forks(&self) -> bool {
        self.forks
    }
    fn forks_by_count(&self) -> bool {
        self.forks_by_count
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.attachments
    }
    fn mcp_servers(&self) -> bool {
        self.mcp_servers
    }
    fn denies_servers(&self) -> bool {
        self.denies_servers
    }
    fn screen_commands(&self) -> Vec<ScreenCommand> {
        self.screen_commands.clone()
    }
    fn moves_access(&self) -> bool {
        self.moves_access
    }
    fn steers_images(&self) -> bool {
        self.steers_images
    }
    fn probe_catalog(
        &self,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>> {
        let ability = self.probe_catalog.as_ref()?;
        self.log("probeCatalog".to_string());
        Some(ability.run(exec))
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let ability = self.probe_version.as_ref()?;
        self.log("probeVersion".to_string());
        Some(ability.run(exec))
    }
    fn session_title(
        &self,
        harness_session_id: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let ability = self.session_title.as_ref()?;
        self.log(format!("sessionTitle {harness_session_id}"));
        Some(ability.run(exec))
    }
    fn rename_session(
        &self,
        harness_session_id: String,
        title: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        let ability = self.rename_session.as_ref()?;
        self.log(format!("renameSession {harness_session_id} {title}"));
        Some(ability.run(exec))
    }
    fn title_for(
        &self,
        turn: TitleTurn,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let ability = self.title_for.as_ref()?;
        self.log("titleFor".to_string());
        self.title_turns.lock().expect("turns").push(turn);
        Some(ability.run(exec))
    }
    fn aside(&self, _question: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        self.aside.then(|| async { Ok(AsideAnswer { text: String::new(), usage: None }) }.boxed_local())
    }
    fn compacts(&self) -> Option<String> {
        self.compacts.clone()
    }
    fn terminal_resume(&self) -> Option<String> {
        self.terminal_resume.clone()
    }
    fn conversations(&self) -> Option<Arc<dyn wsp_adapters::port::ConversationStore>> {
        self.conversations.clone()
    }
}

/// A factory over one adapter that keeps every context it was handed, in order.
pub fn factory(adapter: Arc<FakeAdapter>) -> (HarnessAdapterFactory, Rc<RefCell<Vec<HarnessAdapterContext>>>) {
    let seen: Rc<RefCell<Vec<HarnessAdapterContext>>> = Rc::default();
    let keep = seen.clone();
    let f: HarnessAdapterFactory = Arc::new(move |ctx: HarnessAdapterContext| {
        keep.borrow_mut().push(ctx);
        let adapter: Arc<dyn HarnessAdapter> = adapter.clone();
        Ok(adapter)
    });
    (f, seen)
}

/// A factory that hands out this adapter and keeps no context.
pub fn plain_factory(adapter: Arc<FakeAdapter>) -> HarnessAdapterFactory {
    factory(adapter).0
}

/// The exec factory the fake context hands every launch; nothing runs on it.
pub struct FakeExecFactory;

impl ExecStreamFactory for FakeExecFactory {
    fn launch(&self, _command: &str, _options: ExecOptions) -> Arc<dyn ExecStream> {
        unimplemented!("the agents tests start no process")
    }
}

// ---- the fake context -------------------------------------------------------------------------------------------

/// What a test sets for one agent on one computer.
#[derive(Clone, Default)]
pub struct FakeSetup {
    pub off: bool,
    pub config_dir: Option<String>,
    pub launch: SetupLaunch,
}

pub struct FakeAgentsCtx {
    pub clock: Rc<TestClock>,
    pub ids: Rc<CountingIds>,
    pub bus: Rc<TestBus>,
    pub store: Rc<FakeStore>,
    pub sessions: Sessions,
    pub thread_records: ThreadRecords,
    pub closing: Cell<bool>,
    pub entries: RefCell<IndexMap<String, Entry>>,
    pub adapters: RefCell<IndexMap<String, HarnessAdapterFactory>>,
    pub local_home_dir: RefCell<Option<String>>,
    pub local_homes: RefCell<IndexMap<String, String>>,
    pub process_home: RefCell<String>,
    pub state_path: RefCell<Option<String>>,
    /// By (place, agent).
    pub setups: RefCell<IndexMap<(String, String), FakeSetup>>,
    /// What `place_exec` answers: the first rule whose place and needle match; else exit 0 with no output.
    pub place_rules: RefCell<Vec<(String, String, Result<ExecResult, HostError>)>>,
    pub place_names: RefCell<IndexMap<String, String>>,
    /// The place of a workspace record, by record id.
    pub place_ids: RefCell<IndexMap<String, String>>,
    /// Places `at_provider` answers true for; with `provider_when_no_place`, a record on no place is at a provider too.
    pub provider_places: RefCell<IndexSet<String>>,
    pub provider_when_no_place: Cell<bool>,
    /// By (place, agent), for `place_agent_home`; else `<home>/.<agent>-of-<place>`.
    pub place_agent_homes: RefCell<IndexMap<(String, String), String>>,
    /// Makes `place_agent_home` throw with these words.
    pub place_home_fails: RefCell<Option<String>>,
    /// Makes `brokered_launch` throw with these words.
    pub brokered_fails: RefCell<Option<String>>,
    pub ready_fails: RefCell<Option<String>>,
    /// A harness whose `know_broker` fails, with its message.
    pub broker_failures: RefCell<IndexMap<String, String>>,
    /// A workspace `unreached_of` names, with the line.
    pub unreached: RefCell<IndexMap<String, String>>,
    /// By agent; else `/kind/<workspace>/<agent>`.
    pub kind_homes: RefCell<IndexMap<String, String>>,
    pub kind_env_base: RefCell<Env>,
    /// Agents whose home `kind_env` writes as `HOME_OF_<AGENT>`, through the `home` it was handed.
    pub kind_env_homes: RefCell<Vec<String>>,
    pub kind_server_values: Cell<Option<ServerValuesRoad>>,
    pub kind_ends_threads: Cell<bool>,
    pub kind_memory_keys: RefCell<IndexMap<String, String>>,
    pub kind_login_stands: RefCell<IndexSet<String>>,
    pub folder_computers: RefCell<IndexSet<String>>,
    pub brokered: RefCell<Option<BrokeredLaunch>>,
    pub vault: RefCell<Env>,
    pub vault_on: RefCell<Env>,
    pub exec_factory: Arc<FakeExecFactory>,
    /// Every `exec_factory_for`: the workspace, the launch limits and whether a waiting reader came.
    pub exec_factories: RefCell<Vec<(String, TurnLaunch, bool)>>,
    /// The addresses the agents reader says sign in; `None` is a host with no reader.
    pub signs: RefCell<Option<IndexMap<String, bool>>>,
    pub projects: RefCell<Vec<ProjectView>>,
    /// The folder `project_folder` holds, by project id; else it fails.
    pub folders: RefCell<IndexMap<String, Entry>>,
    pub calls: RefCell<Vec<String>>,
}

impl FakeAgentsCtx {
    pub fn new() -> Rc<FakeAgentsCtx> {
        Rc::new(FakeAgentsCtx {
            clock: TestClock::new(1_000_000),
            ids: Rc::new(CountingIds::default()),
            bus: TestBus::new(),
            store: FakeStore::new(),
            sessions: Sessions::default(),
            thread_records: ThreadRecords::default(),
            closing: Cell::new(false),
            entries: RefCell::default(),
            adapters: RefCell::default(),
            local_home_dir: RefCell::new(None),
            local_homes: RefCell::default(),
            process_home: RefCell::new("/process-home".to_string()),
            state_path: RefCell::new(None),
            setups: RefCell::default(),
            place_rules: RefCell::default(),
            place_names: RefCell::default(),
            place_ids: RefCell::default(),
            provider_places: RefCell::default(),
            provider_when_no_place: Cell::new(false),
            place_agent_homes: RefCell::default(),
            place_home_fails: RefCell::default(),
            brokered_fails: RefCell::default(),
            ready_fails: RefCell::new(None),
            broker_failures: RefCell::default(),
            unreached: RefCell::default(),
            kind_homes: RefCell::default(),
            kind_env_base: RefCell::default(),
            kind_env_homes: RefCell::new(vec!["claude".to_string()]),
            kind_server_values: Cell::new(None),
            kind_ends_threads: Cell::new(false),
            kind_memory_keys: RefCell::default(),
            kind_login_stands: RefCell::default(),
            folder_computers: RefCell::default(),
            brokered: RefCell::new(None),
            vault: RefCell::default(),
            vault_on: RefCell::default(),
            exec_factory: Arc::new(FakeExecFactory),
            exec_factories: RefCell::default(),
            signs: RefCell::new(None),
            projects: RefCell::default(),
            folders: RefCell::default(),
            calls: RefCell::default(),
        })
    }
    /// The area over this context.
    pub fn area(self: &Rc<Self>) -> Rc<AgentsArea> {
        AgentsArea::new(self.clone())
    }
    pub fn call(&self, line: String) {
        self.calls.borrow_mut().push(line);
    }
    pub fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    /// The calls whose line starts with `prefix`.
    pub fn called_with(&self, prefix: &str) -> Vec<String> {
        self.calls.borrow().iter().filter(|c| c.starts_with(prefix)).cloned().collect()
    }
    pub fn add_entry(&self, entry: Entry) {
        let id = entry.borrow().record.borrow().id.clone();
        self.entries.borrow_mut().insert(id, entry);
    }
    pub fn add_adapter(&self, harness: &str, factory: HarnessAdapterFactory) {
        self.adapters.borrow_mut().insert(harness.to_string(), factory);
    }
    pub fn set_setup(&self, place: &str, agent: &str, setup: FakeSetup) {
        self.setups.borrow_mut().insert((place.to_string(), agent.to_string()), setup);
    }
    pub fn turn_off(&self, place: &str, agent: &str) {
        let mut setups = self.setups.borrow_mut();
        setups.entry((place.to_string(), agent.to_string())).or_default().off = true;
    }
    pub fn keep_config_dir(&self, place: &str, agent: &str, folder: &str) {
        let mut setups = self.setups.borrow_mut();
        setups.entry((place.to_string(), agent.to_string())).or_default().config_dir = Some(folder.to_string());
    }
    /// `place_exec` on `place` answers a script containing `needle` with this exit, output and error.
    pub fn place_answers(&self, place: &str, needle: &str, exit_code: i64, stdout: &str, stderr: &str) {
        self.place_rules.borrow_mut().push((place.to_string(), needle.to_string(), Ok(exec_result(exit_code, stdout, stderr))));
    }
    pub fn place_fails(&self, place: &str, needle: &str, message: &str) {
        self.place_rules.borrow_mut().push((place.to_string(), needle.to_string(), Err(err(message))));
    }
    pub fn emit(&self, v: Value) {
        self.bus.emit_json(v);
    }
}

fn kept_at(setups: &RefCell<IndexMap<(String, String), FakeSetup>>, place: &str, agent: &str) -> Option<FakeSetup> {
    setups.borrow().get(&(place.to_string(), agent.to_string())).cloned()
}

impl AgentsCtx for FakeAgentsCtx {
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        super::spawn_detached(work);
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn bus(&self) -> Rc<dyn EventBus> {
        Rc::new(super::BusHandle(self.bus.clone()))
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> ThreadRecords {
        self.thread_records.clone()
    }
    fn entry(&self, workspace_id: &str) -> Option<Entry> {
        self.entries.borrow().get(workspace_id).cloned()
    }
    fn closing(&self) -> bool {
        self.closing.get()
    }
    fn adapters(&self) -> IndexMap<String, HarnessAdapterFactory> {
        self.adapters.borrow().clone()
    }
    fn local_home_dir(&self) -> Option<String> {
        self.local_home_dir.borrow().clone()
    }
    fn local_home(&self, agent: &str) -> Option<String> {
        self.local_homes.borrow().get(agent).cloned()
    }
    fn process_home(&self) -> String {
        self.process_home.borrow().clone()
    }
    fn state_path(&self) -> Option<String> {
        self.state_path.borrow().clone()
    }
    fn setup_off(&self, place: &str, agent: &str) -> bool {
        kept_at(&self.setups, place, agent).is_some_and(|s| s.off)
    }
    fn setup_config_dir(&self, place: &str, agent: &str) -> Option<String> {
        kept_at(&self.setups, place, agent).and_then(|s| s.config_dir)
    }
    fn setup_launch(&self, place: &str, agent: &str) -> SetupLaunch {
        kept_at(&self.setups, place, agent).map(|s| s.launch).unwrap_or_default()
    }
    fn setup_refusal_moved(&self, place: &str, harness: &str) {
        self.call(format!("setupRefusalMoved {place} {harness}"));
    }
    fn place_exec(&self, place: &str, script: &str, timeout_ms: u64) -> LocalBoxFuture<'static, Result<ExecResult, HostError>> {
        self.call(format!("placeExec {place} {timeout_ms} {script}"));
        let rule = self
            .place_rules
            .borrow()
            .iter()
            .find(|(p, needle, _)| p == place && script.contains(needle.as_str()))
            .map(|(_, _, r)| r.clone());
        let answer = rule.unwrap_or_else(|| Ok(exec_result(0, "", "")));
        async move { answer }.boxed_local()
    }
    fn place_name(&self, place: &str) -> String {
        self.place_names.borrow().get(place).cloned().unwrap_or_else(|| format!("{place}-name"))
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.place_ids.borrow().get(&record.id).cloned()
    }
    fn at_provider(&self, place_id: Option<&str>) -> bool {
        match place_id {
            Some(p) => self.provider_places.borrow().contains(p),
            None => self.provider_when_no_place.get(),
        }
    }
    fn place_agent_home(&self, place: &str, home: &str, agent: &str) -> Result<String, HostError> {
        if let Some(why) = self.place_home_fails.borrow().clone() {
            return Err(HostError::new(why));
        }
        Ok(self
            .place_agent_homes
            .borrow()
            .get(&(place.to_string(), agent.to_string()))
            .cloned()
            .unwrap_or_else(|| format!("{home}/.{agent}-of-{place}")))
    }
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call("ready".to_string());
        let answer = match self.ready_fails.borrow().clone() {
            Some(m) => Err(err(&m)),
            None => Ok(()),
        };
        async move { answer }.boxed_local()
    }
    fn know_broker(&self, entry: &Entry, harness: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call(format!("knowBroker {} {harness}", entry.borrow().record.borrow().id));
        let answer = match self.broker_failures.borrow().get(harness).cloned() {
            Some(m) => Err(err(&m)),
            None => Ok(()),
        };
        async move { answer }.boxed_local()
    }
    fn unreached_of(&self, entry: &Entry) -> Option<String> {
        self.unreached.borrow().get(&entry.borrow().record.borrow().id).cloned()
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.call(format!("persist {workspace_id}"));
        async {}.boxed_local()
    }
    fn push_head(&self, thread_id: &str) {
        self.call(format!("pushHead {thread_id}"));
    }
    fn write_session(
        &self,
        session: &str,
        write: LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>,
    ) -> LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>> {
        self.call(format!("writeSession {session}"));
        write
    }
    fn kind_home(&self, entry: &Entry, agent: &str) -> String {
        self.kind_homes.borrow().get(agent).cloned().unwrap_or_else(|| format!("/kind/{}/{agent}", entry.borrow().record.borrow().id))
    }
    fn kind_env(&self, entry: &Entry, harness: Option<&str>, home: &dyn Fn(&str) -> String) -> Env {
        self.call(format!("kindEnv {} {}", entry.borrow().record.borrow().id, harness.unwrap_or("-")));
        let mut env = self.kind_env_base.borrow().clone();
        for id in self.kind_env_homes.borrow().iter() {
            env.insert(format!("HOME_OF_{}", id.to_uppercase()), home(id));
        }
        env
    }
    fn kind_server_values(&self, _entry: &Entry) -> Option<ServerValuesRoad> {
        self.kind_server_values.get()
    }
    fn kind_ends_threads(&self, _entry: &Entry) -> bool {
        self.kind_ends_threads.get()
    }
    fn kind_memory_key(&self, _entry: &Entry, harness: &str) -> Option<String> {
        self.kind_memory_keys.borrow().get(harness).cloned()
    }
    fn kind_login_stands(&self, _entry: &Entry, agent: &str) -> bool {
        self.kind_login_stands.borrow().contains(agent)
    }
    fn computer_runs_in_folder(&self, computer: &str) -> bool {
        self.folder_computers.borrow().contains(computer)
    }
    fn brokered_launch(&self, entry: &Entry, harness: &str, token: Option<&str>) -> Result<Option<BrokeredLaunch>, HostError> {
        self.call(format!("brokeredLaunch {} {harness} {}", entry.borrow().record.borrow().id, token.unwrap_or("-")));
        if let Some(why) = self.brokered_fails.borrow().clone() {
            return Err(HostError::new(why));
        }
        Ok(self.brokered.borrow().clone())
    }
    fn vault(&self) -> Env {
        self.vault.borrow().clone()
    }
    fn vault_on(&self, entry: &Entry) -> Env {
        self.call(format!("vaultOn {}", entry.borrow().record.borrow().id));
        self.vault_on.borrow().clone()
    }
    fn exec_factory_for(&self, entry: &Entry, turn: TurnLaunch, waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory> {
        self.exec_factories.borrow_mut().push((entry.borrow().record.borrow().id.clone(), turn, waiting.is_some()));
        self.exec_factory.clone()
    }
    fn signs_in(&self, addresses: Vec<String>) -> LocalBoxFuture<'static, Option<SignsIn>> {
        self.call(format!("signsIn {}", addresses.join(",")));
        let answer: Option<SignsIn> = self.signs.borrow().clone().map(|map| {
            let read: SignsIn = Rc::new(move |address: &str| map.get(address).copied());
            read
        });
        async move { answer }.boxed_local()
    }
    fn resolve_project(&self, reference: &str, _origin: Option<Caller>) -> LocalBoxFuture<'static, Result<ProjectView, HostError>> {
        let found = self.projects.borrow().iter().find(|p| p.id == reference).cloned();
        let answer = found.ok_or_else(|| err(&format!("no such project {reference}")));
        async move { answer }.boxed_local()
    }
    fn list_projects(&self, _origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Vec<ProjectView>, HostError>> {
        let all = self.projects.borrow().clone();
        async move { Ok(all) }.boxed_local()
    }
    fn project_folder(&self, project: &ProjectView) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        let held = self.folders.borrow().get(&project.id).cloned();
        let answer = held.ok_or_else(|| err("the folder could not be held now"));
        async move { answer }.boxed_local()
    }
}

// ---- builders --------------------------------------------------------------------------------------------------

/// A live entry over a fake machine, its whole record read from JSON (`id`, `name`, `kind`, `project`, `phase` and
/// the fields a record must carry default).
pub fn entry(fields: Value) -> (Entry, Arc<FakeMachine>) {
    let mut v = json!({
        "id": "ws1", "name": "ws1", "machineId": "m-ws1", "kind": "local", "golden": "snap_g", "createdAt": "2026-10-01T12:00:00.000Z",
        "project": "p1", "phase": "running", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    if fields.get("machineId").is_none() {
        v["machineId"] = json!(format!("m-{}", v["id"].as_str().unwrap_or("ws1")));
    }
    let machine = FakeMachine::new(v["machineId"].as_str().expect("a machine id"));
    let record: WorkspaceRecord = serde_json::from_value(v).expect("a WorkspaceRecord");
    (super::entry::entry_over(record, machine.clone()), machine)
}

/// A project as the host lists it: an id on a computer, a folder source.
pub fn project(id: &str, computer: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": id, "name": id, "computer": computer, "source": { "kind": "folder", "path": "/p" }, "path": "/p", "remote": "",
        "defaultBranch": "main", "memoryKey": id, "memoryDir": "/m", "createdAt": "2026-10-01T12:00:00.000Z",
    }))
    .expect("a ProjectView")
}

/// A table row for a harness, as the runtime's table builds one: source table, no pin version unless named, the
/// given models, efforts and permission modes. Values only; labels are the value capitalised.
pub fn table(harness: &str, label: &str, extra: Value) -> wsp_wire::views::harness::HarnessCatalog {
    let mut v = json!({
        "harness": harness, "label": label, "source": "table", "version": null, "models": [], "efforts": [], "contextWindows": [],
        "permissionModes": [], "steers": false, "renames": false, "images": false,
    });
    for (k, x) in extra.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    serde_json::from_value(v).expect("a HarnessCatalog")
}

/// A machine that lands bytes by the backend's own road: wraps a `FakeMachine`, whose `put` keeps each file.
pub struct PutMachine(pub Arc<FakeMachine>);

#[async_trait::async_trait(?Send)]
impl Machine for PutMachine {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn kind(&self) -> MachineKind {
        self.0.kind()
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { put_bytes: true, ..MachineOffers::default() }
    }
    async fn exec(&self, cmd: &str, opts: MachineExecOptions) -> wsp_engine::errors::Result<ExecResult> {
        self.0.exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> wsp_engine::errors::Result<ExecResult> {
        self.0.run(script, opts).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> wsp_engine::errors::Result<String> {
        self.0.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> wsp_engine::errors::Result<()> {
        self.0.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&wsp_engine::Cancel>) -> wsp_engine::errors::Result<()> {
        self.0.resume(signal).await
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        self.0.kill().await
    }
    async fn state(&self) -> wsp_engine::errors::Result<MachineState> {
        self.0.state().await
    }
    async fn download_url(&self, path: &str) -> wsp_engine::errors::Result<String> {
        self.0.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> wsp_engine::errors::Result<String> {
        self.0.upload_url(path).await
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> wsp_engine::errors::Result<Option<BytesLanded>> {
        self.0.put_bytes(path, bytes, timeout_ms).await
    }
}

/// A live entry whose machine lands bytes by its own road; `put` on the returned machine keeps each file in order.
pub fn landing_entry(fields: Value) -> (Entry, Arc<FakeMachine>) {
    let (e, machine) = entry(fields);
    let record = e.borrow().record.borrow().clone();
    (super::entry::entry_over(record, Arc::new(PutMachine(machine.clone()))), machine)
}
