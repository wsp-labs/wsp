// SPDX-License-Identifier: AGPL-3.0-only
//! Port of runtime/src/threads/launch.ts at 65f6b89c7. Spec: spec/launch.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, AgentLaunch, Env, HarnessAdapter, HarnessExec, HarnessWarmOptions, KeptAgent};
use wsp_catalog::signin_rows::server_values_of;
use wsp_core::console;
use wsp_core::ids::Ids;
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::api::WarmAsk;
use wsp_core::types::internal::{launches_as, lost_session_log_line, KeptLaunch, WarmKeep};
use wsp_wire::types::env::{NO_SLATE_MCP_ARG, SCOPED_MCP_ARG, THREAD_ENV, TURN_TOKEN_ENV};
use wsp_wire::types::harness_picks::{kept_picks, listed_pick, KeptPicks, RanPicks};
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::types::wsp_tools::MCP_SERVER_NAME;
use wsp_wire::views::harness::{start_picks, HarnessCatalog, HarnessCatalogSource, McpServerSpec, StartPicks};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::preferences::{default_preferences, Preferences};
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_wire::wire::runtime::{SessionWarmResult, SessionWarmResultWarm};
use wsp_words::exit::{HostError, Kind};
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::sub_thread::{cloud_added_line, cloud_removed_line};
use wsp_words::workspace_state::is_local_workspace;

use crate::threads::agents::launch::{AdapterAsk, BuiltAdapter, DefaultsOn, LaunchServers};
use crate::threads::threads::{Entry, KeptHand, ThreadRecords};

/// launch.ts:212. Rules L1, L2.
pub fn served_to(wsp: Option<&McpServerSpec>, root_thread_id: &str, thread_id: &str) -> Option<McpServerSpec> {
    let wsp = wsp?;
    if root_thread_id == thread_id {
        return Some(wsp.clone());
    }
    let mut served = wsp.clone();
    served.no_slate = Some(wsp_frames::True);
    if wsp.args.iter().any(|arg| arg == SCOPED_MCP_ARG) {
        served.args.push(NO_SLATE_MCP_ARG.to_string());
    }
    Some(served)
}

/// launch.ts:221. `rows` is the place door's rows (None where the host has no door); `prompt` is the send's, written in
/// place. Answers the clouds held. Rules L3-L6.
pub fn clouds_told(rows: Option<Vec<PlaceView>>, records: &ThreadRecords, thread_id: &str, prompt: &mut String) -> Vec<String> {
    let mut held: Vec<String> = rows
        .unwrap_or_default()
        .iter()
        .filter(|place| place.kind == PlaceKind::Provider)
        .map(|place| place_said(&PlaceRef { id: place.id.clone(), name: place.name.clone(), kind: Some("provider".to_string()) }))
        .collect();
    held.sort();
    let seen = records.borrow().get(thread_id).and_then(|record| record.clouds.clone());
    let lines: Vec<String> = match seen {
        None => Vec::new(),
        Some(seen) => held
            .iter()
            .filter(|cloud| !seen.contains(cloud))
            .map(|cloud| cloud_added_line(cloud))
            .chain(seen.iter().filter(|cloud| !held.contains(cloud)).map(|cloud| cloud_removed_line(cloud)))
            .collect(),
    };
    if !lines.is_empty() {
        *prompt = format!("{}\n\n{}", lines.join("\n"), prompt);
    }
    held
}

/// The picks a launch names (launch.ts:230).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchAsked {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    pub access: Option<AccessChoice>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
}

/// What `picks_for` answers: the start picks and the window kept (launch.ts:249).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaunchPicks {
    pub picks: StartPicks,
    pub context_window: Option<String>,
}

/// What a thread's launch is minted with (TS `threadLaunch(...)`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThreadLaunch {
    pub scoped_device_id: Option<String>,
    pub env: IndexMap<String, String>,
    pub wsp: Option<McpServerSpec>,
}

/// What the launch area reads of the runtime. The TS module-level helpers that read the runtime's own tables
/// (`harnessCatalog`, `notRunRefused`) are here too, since their files are not ported in a crate below this one.
#[async_trait(?Send)]
pub trait LaunchCtx {
    fn ids(&self) -> Rc<dyn Ids>;
    async fn ready(&self) -> Result<(), HostError>;
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Entry, HostError>;
    /// `ctx.moduleOf(entry.record.kind).keepsAgents`.
    fn keeps_agents(&self, entry: &Entry) -> bool;
    /// `ctx.keptBox(workspaceId)?.cap`.
    fn kept_box_cap(&self, workspace_id: &str) -> Option<u32>;
    fn closing(&self) -> bool;
    fn preferences_held(&self) -> Option<Preferences>;
    async fn preferences(&self) -> Result<Preferences, HostError>;
    fn default_agent_of(&self, prefs: &Preferences, entry: &Entry) -> String;
    fn agent_off(&self, entry: &Entry, harness: &str) -> bool;
    /// `ctx.adapterFor(entry, harness, env, waiting, servers, threadId, asksUntilStopped)`: `ask.named` is the harness.
    fn adapter_for(&self, entry: &Entry, ask: AdapterAsk) -> Result<BuiltAdapter, HostError>;
    /// Whether the adapter has a `warm` road (TS `adapter.warm !== undefined`; the Rust port answers it by calling).
    fn adapter_warms(&self, adapter: &dyn HarnessAdapter) -> bool;
    async fn confine_setup(&self, entry: &Entry, harness: &str) -> Result<(), HostError>;
    /// `harnessCatalog(harness) === undefined ? undefined : ctx.catalogOn(table, entry, adapter)`, with the table read.
    async fn catalog_on(&self, harness: &str, entry: &Entry, adapter: &dyn HarnessAdapter) -> Result<Option<HarnessCatalog>, HostError>;
    /// `ctx.defaultsOn(catalog, prefs, prefs.projectDefaults[project])`.
    fn defaults_on(&self, catalog: HarnessCatalog, prefs: &Preferences, project: &str) -> DefaultsOn;
    fn named_mode(&self, catalog: Option<&HarnessCatalog>, harness: &str, access: &AccessChoice) -> Option<String>;
    fn access_of(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> Option<String>;
    fn ran_on(&self, workspace_id: &str, thread_id: &str, session: Option<&str>) -> RanPicks;
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String>;
    /// `notRunRefused(catalog, model)`.
    fn not_run_refused(&self, catalog: Option<&HarnessCatalog>, model: Option<&str>) -> Option<HostError>;
    async fn thread_folder(&self, entry: &Entry, cwd: Option<&str>) -> Result<String, HostError>;
    /// `existsSync(path)` on this computer.
    fn folder_exists(&self, path: &str) -> bool;
    fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, folder: &str) -> String;
    fn setup_place(&self, entry: &Entry) -> Option<String>;
    /// `setups.launchOf(place, harness)`.
    fn launch_of(&self, place: &str, harness: &str) -> Option<AgentLaunch>;
    /// `ctx.warmOn(workspaceId, harness)`: the standing warm agent's thread and launch.
    fn warm_on(&self, workspace_id: &str, harness: &str) -> Option<(String, KeptLaunch)>;
    fn rewarm(&self, thread_id: &str);
    /// `ctx.reapKept(threadId)`.
    fn reap_kept(&self, thread_id: &str);
    async fn thread_launch(&self, entry: &Entry, thread_id: &str, root_thread_id: &str, aside: bool) -> Result<ThreadLaunch, HostError>;
    /// `opts.vault?.() ?? {}`; the area reads `server_values_of` off it.
    fn vault(&self) -> Env;
    async fn launch_servers_for(
        &self,
        entry: &Entry,
        harness: &str,
        adapter: &dyn HarnessAdapter,
        cwd: &str,
    ) -> Result<LaunchServers, HostError>;
    fn hold_warm(&self, thread_id: &str, hold: KeptHand);
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError>;
    /// TS `void promise`: work that outlives its caller. The host runs it on its `LocalSet`, so where it runs is its to say.
    fn spawn(&self, work: LocalBoxFuture<'static, ()>);
}

/// What `launch_picks` answers: the catalog read and the picks for a session (launch.ts:264).
pub struct PicksRead {
    pub catalog: Option<HarnessCatalog>,
    pub picks_for: Box<dyn Fn(Option<&str>) -> Result<LaunchPicks, HostError>>,
}

type WarmRun = Shared<LocalBoxFuture<'static, Result<SessionWarmResult, HostError>>>;

#[derive(Default)]
struct LaunchState {
    /// Each in-flight warm, with the number of the ask that started it, so only the newest one drops its key.
    warming: IndexMap<String, (u64, WarmRun)>,
    asks: u64,
}

/// launch.ts:232. Clones share one state.
#[derive(Clone)]
pub struct LaunchArea {
    ctx: Rc<dyn LaunchCtx>,
    state: Rc<RefCell<LaunchState>>,
}

impl LaunchArea {
    pub fn new(ctx: Rc<dyn LaunchCtx>) -> Self {
        LaunchArea { ctx, state: Rc::new(RefCell::new(LaunchState::default())) }
    }

    /// launch.ts:237. Rules L7-L11.
    pub async fn launch_picks(
        &self,
        entry: &Entry,
        harness: &str,
        adapter: &dyn HarnessAdapter,
        prefs: &Preferences,
        thread_id: &str,
        asked: &LaunchAsked,
    ) -> Result<PicksRead, HostError> {
        let (workspace_id, project) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (record.id.clone(), record.project.clone())
        };
        let resolved = self.ctx.catalog_on(harness, entry, adapter).await?.map(|catalog| self.ctx.defaults_on(catalog, prefs, &project));
        let catalog = resolved.as_ref().map(|r| r.catalog.clone());
        let (open_model, open_effort) = resolved.map(|r| (r.open_model, r.open_effort)).unwrap_or_default();
        let named = asked
            .permission_mode
            .clone()
            .or_else(|| asked.access.and_then(|access| self.ctx.named_mode(catalog.as_ref(), harness, &access)));
        let read_catalog = catalog.clone();
        let ctx = self.ctx.clone();
        let (asked, thread_id) = (asked.clone(), thread_id.to_string());
        let picks_for: Box<dyn Fn(Option<&str>) -> Result<LaunchPicks, HostError>> = Box::new(move |session: Option<&str>| {
            let access = named.clone().or_else(|| {
                catalog.as_ref().and_then(|catalog| {
                    listed_pick(&catalog.permission_modes, ctx.access_of(&workspace_id, &thread_id, session).as_deref())
                })
            });
            let ran = ctx.ran_on(&workspace_id, &thread_id, session);
            let kept = kept_picks(
                catalog.as_ref(),
                &ran,
                &KeptPicks { model: asked.model.clone(), effort: asked.effort.clone(), context_window: asked.context_window.clone() },
            );
            let fresh = session.is_none() && ran.model.is_none() && ran.effort.is_none();
            let (open_model, open_effort) = if fresh { (open_model.clone(), open_effort.clone()) } else { (None, None) };
            let model = kept.model.clone().or(open_model);
            let effort = kept.effort.clone().or(open_effort);
            let runs_on = match session {
                Some(session) => ctx.resumed_fact(&workspace_id, session, "model"),
                None => None,
            };
            let input = StartPicks {
                model: model.clone().or_else(|| asked.model.clone()),
                effort: effort.or_else(|| asked.effort.clone()),
                permission_mode: access,
                fast: asked.fast,
            };
            let picks = match start_picks(catalog.as_ref(), &input, session.is_none(), runs_on.as_deref()) {
                Ok(picks) => picks,
                Err(refusal) => {
                    let own = HostError::with_kind(refusal.message.clone(), Kind::parse(refusal.kind));
                    return Err(ctx.not_run_refused(catalog.as_ref(), model.as_deref().or(asked.model.as_deref())).unwrap_or(own));
                }
            };
            Ok(LaunchPicks { picks, context_window: kept.context_window })
        });
        Ok(PicksRead { catalog: read_catalog, picks_for })
    }

    /// launch.ts:268. `mcp_servers` is the JSON of the servers a caller named; `version` is `None` for undefined and
    /// `Some(None)` for null. Rule L12.
    pub fn launch_fixed(
        &self,
        entry: &Entry,
        harness: &str,
        cwd: &str,
        mcp_servers: Option<&serde_json::Value>,
        version: Option<Option<&str>>,
    ) -> String {
        let mut doc = serde_json::Map::new();
        doc.insert("harness".to_string(), json!(harness));
        doc.insert("cwd".to_string(), json!(cwd));
        if let Some(servers) = mcp_servers {
            doc.insert("mcpServers".to_string(), servers.clone());
        }
        if let Some(version) = version {
            doc.insert("version".to_string(), json!(version));
        }
        let setup = self.ctx.setup_place(entry).and_then(|place| self.ctx.launch_of(&place, harness));
        if let Some(setup) = setup {
            if let Ok(setup) = serde_json::to_value(setup) {
                doc.insert("setup".to_string(), setup);
            }
        }
        Value::Object(doc).to_string()
    }

    /// launch.ts:314. Rules L13-L20.
    pub async fn warm(&self, workspace_id: &str, asked: WarmAsk, origin: Option<Caller>) -> Result<SessionWarmResult, HostError> {
        self.ctx.ready().await?;
        if scope_of(origin.as_ref()).is_some() {
            return Ok(warm_result(SessionWarmResultWarm::None));
        }
        let entry = self.ctx.entry_of(workspace_id, origin).await?;
        if !self.ctx.keeps_agents(&entry) || self.ctx.kept_box_cap(workspace_id) == Some(0) || self.ctx.closing() {
            return Ok(warm_result(SessionWarmResultWarm::None));
        }
        let held = self.ctx.preferences_held();
        let (harness, prefs) = match asked.harness.clone() {
            Some(harness) => (harness, held.unwrap_or_else(default_preferences)),
            None => {
                let prefs = match held {
                    Some(prefs) => prefs,
                    None => self.ctx.preferences().await?,
                };
                (self.ctx.default_agent_of(&prefs, &entry), prefs)
            }
        };
        if self.ctx.agent_off(&entry, &harness) {
            return Ok(warm_result(SessionWarmResultWarm::None));
        }
        let key = format!("{workspace_id}\u{0}{harness}");
        let (ask, prev) = {
            let mut st = self.state.borrow_mut();
            st.asks += 1;
            (st.asks, st.warming.get(&key).map(|(_, run)| run.clone()))
        };
        let me = self.clone();
        let (run_key, run_entry, run_harness, run_prefs, run_asked) =
            (key.clone(), entry.clone(), harness.clone(), prefs.clone(), asked.clone());
        let run: WarmRun = async move {
            if let Some(prev) = prev {
                let _ = prev.await;
            }
            let result = me.warm_now(&run_entry, &run_harness, &run_prefs, &run_asked).await;
            me.forget_warm(&run_key, ask);
            result
        }
        .boxed_local()
        .shared();
        self.state.borrow_mut().warming.insert(key, (ask, run.clone()));
        run.await
    }

    fn forget_warm(&self, key: &str, ask: u64) {
        let mut st = self.state.borrow_mut();
        if st.warming.get(key).is_some_and(|(newest, _)| *newest == ask) {
            st.warming.shift_remove(key);
        }
    }

    /// launch.ts:274-312: one warm process, held for the thread's first send.
    async fn warm_now(&self, entry: &Entry, harness: &str, prefs: &Preferences, asked: &WarmAsk) -> Result<SessionWarmResult, HostError> {
        let (workspace_id, local) = {
            let live = entry.borrow();
            let record = live.record.borrow();
            (record.id.clone(), is_local_workspace(Some(&record.kind)))
        };
        let thread_id = self.ctx.ids().uuid();
        let never: TurnWaiting = Arc::new(|| false);
        let probe = self
            .ctx
            .adapter_for(
                entry,
                AdapterAsk {
                    named: Some(harness.to_string()),
                    turn_env: Some(Env::new()),
                    waiting: Some(never),
                    servers: Env::new(),
                    thread: Some(thread_id.clone()),
                    asks_until_stopped: false,
                },
            )?
            .adapter;
        if !self.ctx.adapter_warms(&*probe) {
            return Ok(warm_result(SessionWarmResultWarm::None));
        }
        self.ctx.confine_setup(entry, harness).await?;
        let read = self.launch_picks(entry, harness, &*probe, prefs, &thread_id, &launch_asked(asked)).await?;
        let picks = (read.picks_for)(None)?;
        let folder = self.ctx.thread_folder(entry, asked.cwd.as_deref()).await?;
        if local && !self.ctx.folder_exists(&folder) {
            return Ok(warm_result(SessionWarmResultWarm::None));
        }
        let cwd = self.ctx.runs_in(entry, None, &folder);
        let fixed = self.launch_fixed(entry, harness, &cwd, None, read.catalog.as_ref().map(|c| c.version.as_deref()));
        let launch = KeptLaunch { fixed, picks: picks_map(&picks) };
        let standing = self.ctx.warm_on(&workspace_id, harness);
        if let Some((standing_thread, kept)) = &standing {
            if launches_as(kept, &launch) && launches_as(&launch, kept) {
                self.ctx.rewarm(standing_thread);
                return Ok(warm_result(SessionWarmResultWarm::Standing));
            }
            self.ctx.reap_kept(standing_thread);
        }
        let turn_token = hex(self.ctx.ids().bytes(16));
        let launched = self.ctx.thread_launch(entry, &thread_id, &thread_id, false).await?;
        let waiting = Rc::new(Cell::new(false));
        let warmed = self.start_warm(entry, harness, &cwd, &thread_id, &turn_token, &launched, &read, &picks, waiting.clone()).await;
        let agent = match warmed {
            Ok(Some(agent)) => agent,
            Ok(None) => {
                self.revoke_scope(&launched).await;
                return Ok(warm_result(SessionWarmResultWarm::None));
            }
            Err(e) => {
                self.revoke_scope(&launched).await;
                return Err(e);
            }
        };
        self.ctx.hold_warm(
            &thread_id,
            KeptHand {
                workspace_id,
                agent,
                pid: None,
                launch,
                session: None,
                warm: Some(WarmKeep { harness: harness.to_string(), claimed: false }),
                turn_token,
                scope_device_id: launched.scoped_device_id.clone(),
                waiting,
            },
        );
        Ok(warm_result(SessionWarmResultWarm::Started))
    }

    /// The adapter built for the thread and its warm road (launch.ts:296-306). `Ok(None)` where the adapter has none.
    #[allow(clippy::too_many_arguments)]
    async fn start_warm(
        &self,
        entry: &Entry,
        harness: &str,
        cwd: &str,
        thread_id: &str,
        turn_token: &str,
        launched: &ThreadLaunch,
        read: &PicksRead,
        picks: &LaunchPicks,
        waiting: Rc<Cell<bool>>,
    ) -> Result<Option<Box<dyn KeptAgent>>, HostError> {
        let mut env = Env::new();
        env.insert(TURN_TOKEN_ENV.to_string(), turn_token.to_string());
        env.insert(THREAD_ENV.to_string(), thread_id.to_string());
        env.extend(launched.env.clone());
        let turn_waiting: TurnWaiting = Arc::new(move || waiting.get());
        let built = self.ctx.adapter_for(
            entry,
            AdapterAsk {
                named: Some(harness.to_string()),
                turn_env: Some(env),
                waiting: Some(turn_waiting),
                servers: server_values_of(&self.ctx.vault()),
                thread: Some(thread_id.to_string()),
                asks_until_stopped: false,
            },
        )?;
        let adapter = built.adapter;
        let served = if adapter.mcp_servers() { launched.wsp.clone() } else { None };
        let servers = if adapter.mcp_servers() || adapter.denies_servers() {
            self.ctx.launch_servers_for(entry, harness, &*adapter, cwd).await?
        } else {
            LaunchServers::default()
        };
        let options = HarnessWarmOptions {
            cwd: Some(cwd.to_string()),
            model: picks.picks.model.clone(),
            effort: picks.picks.effort.clone(),
            permission_mode: picks.picks.permission_mode.clone(),
            context_window: picks.context_window.clone(),
            fast: picks.picks.fast,
            mcp_servers: served.map(|s| IndexMap::from([(MCP_SERVER_NAME.to_string(), port_server(s))])),
            server_values: servers.server_values,
            version: read.catalog.as_ref().filter(|c| c.source == HarnessCatalogSource::Harness).and_then(|c| c.version.clone()),
        };
        match adapter.warm(options) {
            Some(Ok(agent)) => Ok(Some(agent)),
            Some(Err(AdapterError(why))) => Err(HostError::new(why)),
            None => Ok(None),
        }
    }

    /// launch.ts:309-311: a scoped device minted for a warm that did not start is revoked, its own failure swallowed.
    async fn revoke_scope(&self, launched: &ThreadLaunch) {
        if let Some(device) = &launched.scoped_device_id {
            let _ = self.ctx.revoke_device(device).await;
        }
    }
}

fn warm_result(warm: SessionWarmResultWarm) -> SessionWarmResult {
    SessionWarmResult { warm }
}

fn launch_asked(asked: &WarmAsk) -> LaunchAsked {
    LaunchAsked {
        model: asked.model.clone(),
        effort: asked.effort.clone(),
        permission_mode: asked.permission_mode.clone(),
        access: asked.access,
        context_window: asked.context_window.clone(),
        fast: asked.fast,
    }
}

/// The picks a launch keeps as its key: the start picks in the order `start_picks` writes them, then the window.
fn picks_map(picks: &LaunchPicks) -> IndexMap<String, Value> {
    let mut map: IndexMap<String, Value> = match serde_json::to_value(&picks.picks) {
        Ok(Value::Object(doc)) => doc.into_iter().collect(),
        _ => IndexMap::new(),
    };
    if let Some(window) = &picks.context_window {
        map.insert("contextWindow".to_string(), json!(window));
    }
    map
}

fn port_server(spec: McpServerSpec) -> wsp_adapters::port::McpServerSpec {
    wsp_adapters::port::McpServerSpec { command: spec.command, args: spec.args, no_slate: spec.no_slate.map(|_| true) }
}

fn hex(bytes: Vec<u8>) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The adapter's `sessionHeld` road (TS `HarnessAdapter["sessionHeld"]`, not in wsp-adapters' port yet): what the
/// machine holds for a session, in words.
pub type SessionHeld = Rc<dyn Fn(String, Arc<dyn HarnessExec>) -> futures::future::LocalBoxFuture<'static, Result<String, String>>>;

/// The seed a resuming start hands its agent.
pub type Seed = Rc<dyn Fn() -> futures::future::LocalBoxFuture<'static, Result<String, HostError>>>;

/// Where a lost session is named in the log line.
#[derive(Clone, Debug, PartialEq)]
pub struct LostAt {
    pub session_id: String,
    pub thread_id: String,
    pub workspace_id: String,
}

/// launch.ts:340. `exec` is the machine's harness exec bounded by `SESSION_TITLE_TIMEOUT_MS`. Rules L21-L23.
pub fn seed_after_loss(session_held: Option<SessionHeld>, exec: Arc<dyn HarnessExec>, at: LostAt, seed: Seed) -> Seed {
    let read = Rc::new(Cell::new(false));
    Rc::new(move || {
        if let Some(held) = session_held.as_ref() {
            if !read.get() {
                read.set(true);
                let (held, exec, at) = (held.clone(), exec.clone(), at.clone());
                tokio::task::spawn_local(async move {
                    let words = match held(at.session_id.clone(), exec).await {
                        Ok(words) => words,
                        Err(why) => format!("not read: {why}"),
                    };
                    console::warn(&lost_session_log_line(&at.session_id, &at.thread_id, &at.workspace_id, &words));
                });
            }
        }
        seed()
    })
}
