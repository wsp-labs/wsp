// SPDX-License-Identifier: AGPL-3.0-only
//! agents_lists: what each agent offers on a machine, and thread titles. agents.ts:26-324, 682-703. Spec:
//! spec/agents_lists.md.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{join_all, FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use wsp_adapters::port::{
    Env, ExecFailure, ExecFailureKind, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe, HarnessExec,
    SessionRenameWrite, TitleTurn,
};
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::first_run::{installs_on_first_run, RunScript};
use wsp_core::console::warn;
use wsp_core::context::Joined;
use wsp_core::harness_catalog::{catalog_from_probe, harness_catalog, harness_catalogs, smallest_model};
use wsp_core::machine_exec::factory::exec_failure_of;
use wsp_core::types::harness::HarnessAdapter;
use wsp_core::types::internal::{no_made_title_log_line, no_name_write_log_line, no_title_log_line};
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_engine::run_env::env_exec;
use wsp_wire::views::harness::{HarnessCatalog, HarnessCatalogSource};
use wsp_wire::views::session::{thread_key_of, SessionView, TitleSource};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::exit::HostError;
use wsp_words::semver::compare_versions;
use wsp_words::words::base::marked;
use wsp_words::words::turn::stored_title_source;
use wsp_words::workspace_state::{workspace_state, WorkspaceState, WorkspaceStateInput};

use super::super::threads::{Entry, Row};
use super::launch::AdapterAsk;
use super::{ready, AccountRead, AgentsArea, CatalogHeld, TitleRead};

pub use wsp_core::types::events::{
    AGENT_VERSION_READ_MS, CATALOG_PROBE_TIMEOUT_MS, CATALOG_TTL_MS, FIRST_RUN_READ_MS, SESSION_TITLE_REFRESH_MAX,
    SESSION_TITLE_TIMEOUT_MS, SESSION_TITLE_TTL_MS, TITLE_MAKE_TIMEOUT_MS,
};
pub use wsp_core::types::internal::AGENT_LISTS;
/// `{ harnessTitle, titleSource }` (agents.ts:246).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CarriedTitle {
    pub harness_title: Option<String>,
    pub title_source: Option<TitleSource>,
}

/// agents.ts:60-76 `known`: L1.
pub fn known_catalog(table: &HarnessCatalog, adapter: &dyn HarnessAdapter) -> HarnessCatalog {
    let mut known = table.clone();
    known.steers = adapter.steers();
    known.renames = adapter.renames();
    known.images = adapter.attachments().is_some();
    known.asides = Some(adapter.asides());
    if adapter.mcp_servers() {
        known.mcp_servers = Some(true);
    }
    if adapter.moves_access() {
        known.moves_access = Some(true);
    }
    if adapter.steers_images() {
        known.steers_images = Some(true);
    }
    if let Some(compacts) = adapter.compacts() {
        known.compacts = Some(compacts);
    }
    if let Some(terminal) = adapter.terminal_resume() {
        known.terminal_resume = Some(terminal);
    }
    if adapter.forks_by_count() {
        known.forks_by_count = Some(true);
    }
    let screen = adapter.screen_commands();
    if !screen.is_empty() {
        known.screen_commands = Some(screen);
    }
    known
}

/// agents.ts:228 `sourceOf`: L6.
pub fn source_of(view: &SessionView) -> TitleSource {
    view.title_source.unwrap_or(TitleSource::Seed)
}

/// agents.ts:317 `titleRows`: L9.
pub fn title_rows(rows: &[SessionView]) -> Vec<SessionView> {
    let mut sorted: Vec<&SessionView> = rows.iter().collect();
    sorted.sort_by(|a, b| started_of(b).partial_cmp(&started_of(a)).unwrap_or(std::cmp::Ordering::Equal));
    let mut newest: IndexMap<String, SessionView> = IndexMap::new();
    for view in sorted {
        if let Some(session) = view.claude_session_id.as_ref() {
            newest.entry(session.clone()).or_insert_with(|| view.clone());
        }
    }
    newest.into_values().take(SESSION_TITLE_REFRESH_MAX).collect()
}

/// A row's start, absent as zero (`startedAt ?? 0`).
fn started_of(view: &SessionView) -> f64 {
    view.started_at.as_ref().map_or(0.0, |at| at.0)
}

/// Whether the workspace stands running (`workspaceState({ phase }) === "running"`).
pub(super) fn running(entry: &Entry) -> bool {
    let phase: WorkspacePhase = entry.borrow().record.borrow().phase;
    workspace_state(&WorkspaceStateInput { phase, machine_state: None, reach: None, wake_refused: None }) == WorkspaceState::Running
}

/// The `Script` a first-run read runs: its exit 0 output, else none (agents.ts:52-57).
struct MachineScript(Arc<dyn Machine>);

impl RunScript for MachineScript {
    async fn run(&self, script: &str) -> Option<String> {
        let res =
            self.0.exec(script, ExecOptions { timeout_ms: Some(FIRST_RUN_READ_MS), idempotency_key: None, stdin: None }).await.ok()?;
        (res.exit_code.get() == 0).then_some(res.stdout)
    }
}

/// Whether the agent's command on the machine installs its agent on the first run (agents.ts:52).
pub(super) fn first_run_on(machine: Arc<dyn Machine>, harness: &str) -> LocalBoxFuture<'static, bool> {
    let bin = catalog_agents().into_iter().find(|a| a.id == harness).map(|a| a.bin.clone());
    async move {
        let Some(bin) = bin else {
            return false;
        };
        installs_on_first_run(&MachineScript(machine), &[bin]).await.first() == Some(&true)
    }
    .boxed_local()
}

/// Runs one command on the machine as the harness's exec road does: its stdout alone answers, whatever the exit code.
async fn run_exec(machine: Arc<dyn Machine>, command: String, env: Option<Env>, timeout_ms: u64) -> Result<String, ExecFailure> {
    let res = match env {
        None => machine.exec(&command, ExecOptions { timeout_ms: Some(timeout_ms), idempotency_key: None, stdin: None }).await,
        Some(env) => env_exec(machine.as_ref(), &command, &env, Some(timeout_ms)).await,
    };
    res.map(|r| r.stdout).map_err(|e| exec_failure_of(&e))
}

/// `harnessExec(machine, timeoutMs)` (engine run-env.ts:36) as an adapter takes it.
pub(super) fn harness_exec_on(machine: Arc<dyn Machine>, timeout_ms: u64) -> Arc<dyn HarnessExec> {
    Arc::new(move |command: &str, env: Option<&Env>| run_exec(machine.clone(), command.to_string(), env.cloned(), timeout_ms).boxed_local())
}

/// `harnessExec` held behind the first run: a command whose first run installs its agent is not run at all, and the
/// answer is the failure of a command that did not start (agents.ts:73-75 skips the probe there).
fn gated_exec(machine: Arc<dyn Machine>, timeout_ms: u64, installing: Shared<LocalBoxFuture<'static, bool>>) -> Arc<dyn HarnessExec> {
    Arc::new(move |command: &str, env: Option<&Env>| {
        let machine = machine.clone();
        let command = command.to_string();
        let env = env.cloned();
        let installing = installing.clone();
        async move {
            if installing.await {
                return Err(ExecFailure { kind: ExecFailureKind::Other, message: "the agent is installing on its first run".to_string() });
            }
            run_exec(machine, command, env, timeout_ms).await
        }
        .boxed_local()
    })
}

/// `fromAccount` (agents.ts:80-93): the models the account lists under the table's other lists, else the table.
fn account_answer(
    area: Rc<AgentsArea>,
    table: HarnessCatalog,
    known: HarnessCatalog,
    adapter: Arc<dyn HarnessAdapter>,
) -> LocalBoxFuture<'static, HarnessCatalog> {
    async move {
        let version = area.heard_versions.borrow().get(&table.harness).cloned();
        let read_key = format!("{}:{}", table.harness, version.clone().unwrap_or_default());
        let now = area.ctx.clock().now();
        let held = area.account_reads.borrow().get(&read_key).filter(|read| now - read.at < CATALOG_TTL_MS as i64).cloned();
        let models = match held {
            Some(read) => read.models,
            None => match adapter.account_models(version.clone()) {
                None => return known,
                Some(read) => {
                    let models = async move { read.await.ok().flatten() }.boxed_local().shared();
                    area.account_reads.borrow_mut().insert(read_key, AccountRead { at: now, models: models.clone() });
                    models
                }
            },
        };
        let Some(models) = models.await.filter(|models| !models.is_empty()) else {
            return known;
        };
        let own = known.models.iter().find(|m| m.is_default == Some(true)).map(|m| m.value.clone());
        let marked_models: Vec<HarnessCatalogModelProbe> = if models.iter().any(|m| Some(&m.slug) == own.as_ref()) {
            models.into_iter().map(|m| HarnessCatalogModelProbe { is_default: Some(&m.slug) == own.as_ref(), ..m }).collect()
        } else {
            models
        };
        let probe = HarnessCatalogProbe {
            version: None,
            models: marked_models,
            efforts: known.efforts.iter().map(|e| e.value.clone()).collect(),
            permission_modes: known.permission_modes.iter().map(|p| p.value.clone()).collect(),
        };
        catalog_from_probe(&known, &probe)
    }
    .boxed_local()
}

/// `sameBinary` (agents.ts:118-130): kept lists stand while the binary answers the version they were read off.
async fn same_binary(
    area: Rc<AgentsArea>,
    adapter: Arc<dyn HarnessAdapter>,
    machine: Arc<dyn Machine>,
    installing: Shared<LocalBoxFuture<'static, bool>>,
    lists: HarnessCatalog,
    catalog: Shared<LocalBoxFuture<'static, HarnessCatalog>>,
) -> HarnessCatalog {
    if lists.source != HarnessCatalogSource::Harness {
        return lists;
    }
    let Some(was) = lists.version.clone() else {
        return catalog.await;
    };
    if installing.await {
        return catalog.await;
    }
    let Some(read) = adapter.probe_version(harness_exec_on(machine, AGENT_VERSION_READ_MS)) else {
        return catalog.await;
    };
    let answered = area.hold(async move { read.await.ok().flatten() }.boxed_local()).await;
    if answered.as_deref() == Some(was.as_str()) {
        lists
    } else {
        catalog.await
    }
}

/// The lists one probe answer makes, kept for the next host in the background (agents.ts:94-110).
fn keep_lists(area: &AgentsArea, key: String, machine_id: String, harness: String, probe: &HarnessCatalogProbe) {
    let Ok(value) = serde_json::to_value(probe) else {
        return;
    };
    let store = area.ctx.store();
    area.fire(
        async move {
            if let Err(e) = store.put(AGENT_LISTS, &key, value).await {
                warn(&format!(
                    "{harness} on {machine_id}: its lists were not kept for the next host ({}); its first send there waits on the probe",
                    e.message
                ));
            }
        }
        .boxed_local(),
    );
}

impl AgentsArea {
    /// agents.ts:58: L2, L3.
    pub fn catalog_on(
        &self,
        table: &HarnessCatalog,
        entry: &Entry,
        adapter: Arc<dyn HarnessAdapter>,
    ) -> LocalBoxFuture<'static, HarnessCatalog> {
        let known = known_catalog(table, adapter.as_ref());
        let Some(area) = self.me.upgrade() else {
            return ready(known);
        };
        let (machine, machine_id) = {
            let live = entry.borrow();
            (live.machine.clone(), live.machine.id().to_string())
        };
        let key = format!("{machine_id}:{}", table.harness);
        let now = self.ctx.clock().now();
        let hit = self.catalogs.borrow().get(&key).cloned();
        if let Some(hit) = hit.as_ref() {
            if now - hit.at < CATALOG_TTL_MS as i64 {
                return hit.catalog.clone().boxed_local();
            }
        }
        if self.ctx.closing() {
            return match hit {
                Some(hit) => hit.catalog.boxed_local(),
                None => ready(known),
            };
        }
        let installing = first_run_on(machine.clone(), &table.harness).shared();
        let probe_exec = gated_exec(machine.clone(), CATALOG_PROBE_TIMEOUT_MS, installing.clone());
        let Some(probe) = adapter.probe_catalog(probe_exec) else {
            return ready(known);
        };
        let skipped = Rc::new(std::cell::Cell::new(false));
        let chain = {
            let area = area.clone();
            let table = table.clone();
            let known = known.clone();
            let adapter = adapter.clone();
            let machine_id = machine_id.clone();
            let key = key.clone();
            let installing = installing.clone();
            let skipped = skipped.clone();
            async move {
                let answer = if installing.await {
                    skipped.set(true);
                    None
                } else {
                    Some(probe.await)
                };
                match answer {
                    None | Some(Ok(None)) => account_answer(area, table, known, adapter).await,
                    Some(Ok(Some(HarnessCatalogAnswer::Refused(refused)))) => HarnessCatalog { refusal: Some(refused), ..known },
                    Some(Ok(Some(HarnessCatalogAnswer::Probe(lists)))) => {
                        if let Some(version) = lists.version.clone() {
                            let mut heard = area.heard_versions.borrow_mut();
                            let newer =
                                heard.get(&table.harness).is_none_or(|was| compare_versions(&version, was) == std::cmp::Ordering::Greater);
                            if newer {
                                heard.insert(table.harness.clone(), version);
                            }
                        }
                        keep_lists(&area, key, machine_id, table.harness.clone(), &lists);
                        catalog_from_probe(&known, &lists)
                    }
                    Some(Err(e)) => {
                        warn(&format!(
                            "{} on {machine_id}: the probe of the agent failed ({}); wsp's built-in list answers until the next probe",
                            table.harness, e.message
                        ));
                        account_answer(area, table, known, adapter).await
                    }
                }
            }
            .boxed_local()
        };
        let before: Shared<LocalBoxFuture<'static, Option<HarnessCatalog>>> = match hit {
            Some(hit) => {
                let held = hit.catalog.clone();
                async move { Some(held.await) }.boxed_local()
            }
            None => {
                let store = self.ctx.store();
                let known = known.clone();
                let key = key.clone();
                async move {
                    match store.get(AGENT_LISTS, &key).await {
                        Ok(Some(value)) => {
                            serde_json::from_value::<HarnessCatalogProbe>(value).ok().map(|kept| catalog_from_probe(&known, &kept))
                        }
                        _ => None,
                    }
                }
                .boxed_local()
            }
        }
        .shared();
        self.fire(before.clone().map(|_| ()).boxed_local());
        let catalog = self.hold(chain);
        {
            let area = area.clone();
            let key = key.clone();
            let catalog = catalog.clone();
            let skipped = skipped.clone();
            let ctx = area.ctx.clone();
            ctx.spawn(Box::pin(async move {
                catalog.clone().await;
                if skipped.get() {
                    area.catalogs.borrow_mut().shift_remove(&key);
                } else if let Some(asking) = area.catalogs.borrow_mut().get_mut(&key) {
                    if asking.at == now {
                        asking.catalog = catalog;
                    }
                }
            }));
        }
        let held = {
            let catalog = catalog.clone();
            async move {
                match before.await {
                    None => catalog.await,
                    Some(lists) => same_binary(area, adapter, machine, installing, lists, catalog).await,
                }
            }
        };
        let held = held.boxed_local().shared();
        self.catalogs.borrow_mut().insert(key, CatalogHeld { at: now, catalog: held.clone() });
        held.boxed_local()
    }

    /// agents.ts:52: L4.
    pub fn first_run_here(&self, entry: &Entry, harness: &str) -> LocalBoxFuture<'static, bool> {
        first_run_on(entry.borrow().machine.clone(), harness)
    }

    /// agents.ts:167: L5. Never fails.
    pub fn refresh_title(&self, view: &SessionView, force: bool) -> LocalBoxFuture<'static, ()> {
        let Some(session) = view.claude_session_id.clone() else {
            return ready(());
        };
        let Some(entry) = self.ctx.entry(&view.workspace_id) else {
            return ready(());
        };
        if !running(&entry) || self.ctx.unreached_of(&entry).is_some() || !self.ctx.adapters().contains_key(&view.harness) {
            return ready(());
        }
        let machine = entry.borrow().machine.clone();
        let key = format!("{}:{session}", machine.id());
        let now = self.ctx.clock().now();
        let hit = self.title_reads.borrow().get(&key).cloned();
        let failed = match &hit {
            Some(hit) => {
                let read = hit.borrow();
                if read.live || (!force && now - read.at < SESSION_TITLE_TTL_MS as i64) {
                    return read.done.clone().boxed_local();
                }
                read.failed
            }
            None => false,
        };
        let Some(area) = self.me.upgrade() else {
            return ready(());
        };
        let pending = Rc::new(RefCell::new(TitleRead { at: now, done: ready(()).shared(), live: true, failed }));
        let body = {
            let pending = pending.clone();
            let harness = view.harness.clone();
            async move {
                let outcome = area.read_title(&entry, &harness, &machine, &session).await;
                {
                    let mut read = pending.borrow_mut();
                    read.at = area.ctx.clock().now();
                    read.live = false;
                }
                match outcome {
                    Ok(None) => {}
                    Ok(Some(title)) => {
                        pending.borrow_mut().failed = false;
                        if let Some(title) = title {
                            area.apply_title(&entry, &session, &title, force).await;
                        }
                    }
                    Err(words) => {
                        let mut read = pending.borrow_mut();
                        if !read.failed {
                            warn(&no_title_log_line(&session, &entry.borrow().record.borrow().id, &words));
                        }
                        read.failed = true;
                    }
                }
            }
        };
        let done: Joined = body.boxed_local().shared();
        pending.borrow_mut().done = done.clone();
        self.title_reads.borrow_mut().insert(key, pending);
        self.fire(done.clone().boxed_local());
        done.boxed_local()
    }

    /// The harness's own name for a session, read under the setup check (agents.ts:177-184). `Ok(None)` where the
    /// adapter reads no title, `Ok(Some(None))` where the store had none.
    async fn read_title(
        &self,
        entry: &Entry,
        harness: &str,
        machine: &Arc<dyn Machine>,
        session: &str,
    ) -> Result<Option<Option<String>>, String> {
        self.confine_setup(entry, Some(harness)).await.map_err(|e| e.message)?;
        let built =
            self.adapter_for(entry, AdapterAsk { named: Some(harness.to_string()), ..Default::default() }).map_err(|e| e.message)?;
        let Some(read) = built.adapter.session_title(session.to_string(), harness_exec_on(machine.clone(), SESSION_TITLE_TIMEOUT_MS))
        else {
            return Ok(None);
        };
        read.await.map(Some).map_err(|e| e.message)
    }

    /// Keeps a title the harness read on every row of its session, and writes the row's own name back where the
    /// harness is asked to (agents.ts:191-219).
    async fn apply_title(&self, entry: &Entry, session: &str, title: &str, force: bool) {
        let workspace = entry.borrow().record.borrow().id.clone();
        let sessions = self.ctx.sessions();
        let rows: Vec<Row> = sessions
            .borrow()
            .values()
            .filter(|row| {
                let view = &row.borrow().view;
                view.workspace_id == workspace && view.claude_session_id.as_deref() == Some(session)
            })
            .cloned()
            .collect();
        let mut unnamed: Option<SessionView> = None;
        let mut moved: IndexSet<String> = IndexSet::new();
        for row in &rows {
            let view = row.borrow().view.clone();
            let opening = view.thread_id.as_deref().and_then(|thread| self.opening_of(thread));
            let source = if stored_title_source(title, view.prompt.as_deref()) == TitleSource::Seed {
                TitleSource::Seed
            } else {
                stored_title_source(title, opening.as_deref())
            };
            if source == TitleSource::Seed && source_of(&view) != TitleSource::Seed {
                if view.harness_title.is_some() && view.harness_title.as_deref() != Some(title) {
                    unnamed = Some(view);
                }
                continue;
            }
            let mut live = row.borrow_mut();
            if live.view.harness_title.as_deref() != Some(title) {
                moved.insert(thread_key_of(&live.view));
            }
            live.view.harness_title = Some(title.to_string());
            live.view.title_source = Some(source);
        }
        self.ctx.persist_sessions(&workspace).await;
        if let Some(view) = unnamed.filter(|_| force) {
            if let Some(name) = view.harness_title.clone() {
                self.fire(self.name_in_harness(&view, &name));
            }
        }
        for thread in moved {
            self.ctx.push_head(&thread);
        }
    }

    /// agents.ts:230: L6.
    pub fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.ctx
            .sessions()
            .borrow()
            .values()
            .map(|row| row.borrow().view.clone())
            .filter(|view| view.thread_id.as_deref() == Some(thread_id))
            .collect()
    }

    /// agents.ts:233: L6.
    pub fn opening_of(&self, thread_id: &str) -> Option<String> {
        self.rows_on(thread_id)
            .into_iter()
            .fold(None::<SessionView>, |first, view| match first {
                Some(first) if started_of(&view) >= started_of(&first) => Some(first),
                _ => Some(view),
            })?
            .prompt
    }

    /// agents.ts:236: L6.
    pub fn thread_source(&self, thread_id: &str) -> TitleSource {
        let mut source = TitleSource::Seed;
        for view in self.rows_on(thread_id) {
            match source_of(&view) {
                TitleSource::Person => return TitleSource::Person,
                TitleSource::Auto => source = TitleSource::Auto,
                TitleSource::Seed => {}
            }
        }
        source
    }

    /// agents.ts:246: L6.
    pub fn carried_title(&self, thread_id: &str) -> CarriedTitle {
        let mut titled: Vec<SessionView> = self.rows_on(thread_id).into_iter().filter(|v| v.harness_title.is_some()).collect();
        titled.sort_by(|a, b| started_of(b).partial_cmp(&started_of(a)).unwrap_or(std::cmp::Ordering::Equal));
        match titled.into_iter().next() {
            Some(newest) => CarriedTitle { harness_title: newest.harness_title.clone(), title_source: Some(source_of(&newest)) },
            None => CarriedTitle::default(),
        }
    }

    /// agents.ts:257: L7. Never fails.
    pub fn name_in_harness(&self, view: &SessionView, title: &str) -> LocalBoxFuture<'static, ()> {
        let Some(session) = view.claude_session_id.clone() else {
            return ready(());
        };
        let Some(entry) = self.ctx.entry(&view.workspace_id) else {
            return ready(());
        };
        if !running(&entry) || !self.ctx.adapters().contains_key(&view.harness) {
            return ready(());
        }
        let ask = AdapterAsk { named: Some(view.harness.clone()), ..Default::default() };
        match self.adapter_for(&entry, ask.clone()) {
            Ok(built) if built.adapter.renames() => {}
            _ => return ready(()),
        }
        let Some(area) = self.me.upgrade() else {
            return ready(());
        };
        let title = title.to_string();
        async move {
            let workspace = entry.borrow().record.borrow().id.clone();
            let built = match area.launch_adapter_for(&entry, ask).await {
                Ok(built) => built,
                Err(e) => {
                    warn(&no_name_write_log_line(&session, &workspace, &e.message));
                    return;
                }
            };
            let machine = entry.borrow().machine.clone();
            let Some(write) = built.adapter.rename_session(session.clone(), title, harness_exec_on(machine, SESSION_TITLE_TIMEOUT_MS))
            else {
                return;
            };
            let write = async move { write.await.map_err(|e| HostError::new(e.message)) }.boxed_local();
            match area.ctx.write_session(&session, write).await {
                Ok(SessionRenameWrite::Failed { error }) => warn(&no_name_write_log_line(&session, &workspace, &error)),
                Ok(_) => {}
                Err(e) => warn(&no_name_write_log_line(&session, &workspace, &e.message)),
            }
        }
        .boxed_local()
    }

    /// agents.ts:284: L8.
    pub async fn make_title(&self, view: &SessionView) -> Result<(), HostError> {
        let Some(thread) = view.thread_id.clone() else {
            return Ok(());
        };
        let Some(entry) = self.ctx.entry(&view.workspace_id) else {
            return Ok(());
        };
        if self.titles_asked.borrow().contains(&thread) {
            return Ok(());
        }
        let Some(prompt) = view.prompt.clone() else {
            return Ok(());
        };
        if self.thread_source(&thread) != TitleSource::Seed || !running(&entry) || !self.ctx.adapters().contains_key(&view.harness) {
            return Ok(());
        }
        let built = self.launch_adapter_for(&entry, AdapterAsk { named: Some(view.harness.clone()), ..Default::default() }).await?;
        self.titles_asked.borrow_mut().insert(thread.clone());
        let model = match harness_catalog(&built.harness) {
            None => None,
            Some(table) => smallest_model(Some(&self.catalog_on(&table, &entry, built.adapter.clone()).await)),
        };
        if self.ctx.closing() {
            return Ok(());
        }
        let machine = entry.borrow().machine.clone();
        let workspace = entry.borrow().record.borrow().id.clone();
        let turn = TitleTurn { opening: prompt, reply: None, model };
        let Some(answer) = built.adapter.title_for(turn, harness_exec_on(machine, TITLE_MAKE_TIMEOUT_MS)) else {
            return Ok(());
        };
        let title = answer.await.map_err(|e| HostError::new(e.message))?;
        let Some(title) = title else {
            warn(&no_made_title_log_line(&thread, &workspace, "the harness answered with no title"));
            return Ok(());
        };
        if self.thread_source(&thread) == TitleSource::Person {
            return Ok(());
        }
        let rows: Vec<Row> = self
            .ctx
            .sessions()
            .borrow()
            .values()
            .filter(|row| row.borrow().view.thread_id.as_deref() == Some(thread.as_str()))
            .cloned()
            .collect();
        for row in rows {
            let mut live = row.borrow_mut();
            live.view.harness_title = Some(title.clone());
            live.view.title_source = Some(TitleSource::Auto);
        }
        self.ctx.persist_sessions(&workspace).await;
        self.ctx.push_head(&thread);
        self.name_in_harness(view, &title).await;
        Ok(())
    }

    /// agents.ts:689: L10.
    pub async fn agent_lists(&self, entry: Option<&Entry>) -> Vec<HarnessCatalog> {
        let adapters = self.ctx.adapters();
        let table: Vec<HarnessCatalog> = harness_catalogs()
            .into_iter()
            .filter(|c| adapters.contains_key(&c.harness) && entry.is_none_or(|e| !self.agent_off(e, &c.harness)))
            .collect();
        let Some(entry) = entry else {
            return table;
        };
        if entry.borrow().record.borrow().phase != WorkspacePhase::Running {
            return table;
        }
        join_all(table.into_iter().map(|c| self.list_on(c, entry.clone()))).await
    }

    /// One agent's list on a running workspace (agents.ts:700-703).
    async fn list_on(&self, table: HarnessCatalog, on: Entry) -> HarnessCatalog {
        match self.setup_refusal(&on, Some(&table.harness)).await {
            Ok(None) => {}
            Ok(Some(refusal)) => return HarnessCatalog { refusal: Some(refusal), ..table },
            Err(e) if marked(&e, "PlaceAbsent") => return table,
            Err(e) => return HarnessCatalog { refusal: Some(e.message), ..table },
        }
        if let Err(e) = self.ctx.know_broker(&on, &table.harness).await {
            return HarnessCatalog { refusal: Some(e.message), ..table };
        }
        match self.adapter_for(&on, AdapterAsk { named: Some(table.harness.clone()), ..Default::default() }) {
            Ok(built) => self.catalog_on(&table, &on, built.adapter).await,
            Err(_) => table,
        }
    }
}
