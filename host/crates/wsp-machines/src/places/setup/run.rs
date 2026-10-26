// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts:79-111, 246-372, 746-765, 808-922` at 65f6b89c7: one run of a computer's setup
//! or sync. Spec: `spec/places_setup_run.md`. The sign-ins and the folders are this run's other two files.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use wsp_core::clock::Cancel;
use wsp_core::setup_graph::{run_graph, GraphStep, Release, StepNext};
use wsp_core::types::pending::Clock;
use wsp_engine::exec_detached::put::{put_files, GuestWrite, PutFilesOptions, INLINE_EXEC_MS};
use wsp_engine::local_task::try_spawn_local as spawn_local;
use wsp_engine::machine::Machine;
use wsp_engine::place_machine::PlaceMachine;
use wsp_engine::provision::{
    hook_of, EngineStep, ProvisionOn, ProvisionPlan, ProvisionStage, SetupRun as EngineRun, StageAt,
};
use wsp_engine::run_env::GITHUB_TOKEN_ENV;
use wsp_wire::types::recipe_file::{GitHubSignIn, RecipeFile, NO_RECIPE};
use wsp_wire::views::golden_image::tool_row_id;
use wsp_wire::views::place::{
    sign_in_of_row, PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceSetup, PlaceSetupEvent,
    PlaceSetupLine, PlaceSetupLineState, PlaceSetupState, PlaceSetupStep, PlaceSync,
    PlaceSyncEvent, PlaceSyncState, PlaceWait, GITHUB_CLI, GITHUB_ROW, SETUP_STEP_WORDS,
};
use wsp_words::exit::HostError;
use wsp_words::project_path::place_provision_paths;
use wsp_words::views::place_setup_words::{
    floor_failed_line, no_agent_line, provision_log_line, setup_row_fix, setup_width,
};

use super::super::helpers::{
    cli_first, engine_row, first_line_of, picks_hash, setup_outcome, LandedRow, SyncJob, FILES,
    INSTALLS, PROVISION_LOG_EVERY_MS, PROVISION_LOG_LINES,
};
use super::super::types::{
    applied_view, HeldApplied, HeldRow, PlaceProvisioner, PlaceRecord, PlaceResult, SetupOn,
};
use super::{held_of, iso_at, signs_in_again, Lander, OnlyRow, PlaceSetupArea, SetupPatch};

/// What one run is handed (setup.ts:254).
pub struct RunAsk {
    pub place_id: String,
    pub add_id: String,
    pub picks: RecipeFile,
    pub done: IndexSet<PlaceSetupStep>,
    pub started: PlaceSetup,
    pub rerun: Vec<PlaceWait>,
    pub sync: Option<SyncJob>,
    pub only: Option<OnlyRow>,
}

/// Work chained one after another, each item starting once the one before it has ended, as the TS promise chain
/// does (setup.ts:297). Every item is spawned when it is added, so nothing waits on someone reading it.
pub struct Chain {
    last: RefCell<Shared<LocalBoxFuture<'static, ()>>>,
}

impl Chain {
    fn new() -> Chain {
        Chain {
            last: RefCell::new(futures::future::ready(()).boxed_local().shared()),
        }
    }

    fn push(&self, work: LocalBoxFuture<'static, ()>) {
        let prev = self.last.borrow().clone();
        let next = async move {
            prev.await;
            work.await;
        }
        .boxed_local()
        .shared();
        *self.last.borrow_mut() = next.clone();
        spawn_local(next);
    }

    pub async fn settled(&self) {
        let last = self.last.borrow().clone();
        last.await;
    }
}

/// The job's own log on the computer: the lines appended in batches, and the result file at the close (setup.ts:
/// 213). Nothing here fails the job.
pub struct ProvisionLog {
    clock: Rc<dyn Clock>,
    machine: Rc<dyn Machine>,
    paths: wsp_words::project_path::PlaceProvisionPaths,
    lines: RefCell<Vec<String>>,
    timer: RefCell<Option<Cancel>>,
    writing: Chain,
    me: std::rc::Weak<ProvisionLog>,
}

impl ProvisionLog {
    fn new(
        clock: Rc<dyn Clock>,
        machine: Rc<dyn Machine>,
        paths: wsp_words::project_path::PlaceProvisionPaths,
        header: String,
    ) -> Rc<ProvisionLog> {
        let first = provision_log_line(clock.now() as f64, &header);
        Rc::new_cyclic(|me| ProvisionLog {
            clock,
            machine,
            paths,
            lines: RefCell::new(vec![first]),
            timer: RefCell::new(None),
            writing: Chain::new(),
            me: me.clone(),
        })
    }

    /// `write(line)`: queued, a flush at once at the batch's size, else one flush two seconds on (setup.ts:225).
    pub fn write(&self, line: &str) {
        let queued = {
            let mut lines = self.lines.borrow_mut();
            lines.push(provision_log_line(self.clock.now() as f64, line));
            lines.len()
        };
        if queued >= PROVISION_LOG_LINES {
            self.flush();
            return;
        }
        let armed = self.timer.borrow().is_some();
        if !armed {
            let me = self.me.clone();
            let cancel = self.clock.schedule(
                Box::new(move || {
                    if let Some(log) = me.upgrade() {
                        log.flush();
                    }
                }),
                PROVISION_LOG_EVERY_MS as i64,
            );
            *self.timer.borrow_mut() = Some(cancel);
        }
    }

    fn flush(&self) {
        let timer = self.timer.borrow_mut().take();
        if let Some(cancel) = timer {
            cancel();
        }
        let sending: Vec<String> = std::mem::take(&mut *self.lines.borrow_mut());
        if sending.is_empty() {
            return;
        }
        let text = format!("{}\n", sending.join("\n"));
        let write = self.file_write(&self.paths.log, true, text);
        self.writing.push(write);
    }

    /// `putFiles(machine, [{ path, text, append }])` with a failure ignored.
    fn file_write(&self, path: &str, append: bool, text: String) -> LocalBoxFuture<'static, ()> {
        let machine = Rc::clone(&self.machine);
        let file = GuestWrite {
            path: path.to_string(),
            text,
            append,
        };
        async move {
            let options = PutFilesOptions {
                timeout_ms: Some(INLINE_EXEC_MS),
                ..Default::default()
            };
            let _ = put_files(machine.as_ref(), std::slice::from_ref(&file), options).await;
        }
        .boxed_local()
    }

    /// `close(setup)`: a flush, then the result file beside the log (setup.ts:232).
    async fn close(&self, setup: &PlaceSetup) {
        self.flush();
        self.writing.settled().await;
        let text = format!(
            "{}\n",
            serde_json::to_string_pretty(setup).unwrap_or_default()
        );
        self.file_write(&self.paths.result, false, text).await;
    }
}

/// One run's state (the closures of `runSetup`). Memory only; built per run.
pub struct SetupRun {
    pub area: Rc<PlaceSetupArea>,
    pub ask: RunAsk,
    /// The rows this run writes, in order; a row is replaced by id.
    pub rows: RefCell<Vec<HeldRow>>,
    pub held: RefCell<PlaceSetup>,
    pub ended: Cell<bool>,
    pub stepped: Cell<bool>,
    pub running: RefCell<IndexSet<PlaceSetupStep>>,
    pub folders_waiting: Cell<u32>,
    pub agent_files_there: Cell<bool>,
    pub github_known: Cell<Option<bool>>,
    pub record: PlaceRecord,
    pub home: String,
    pub machine: Rc<dyn Machine>,
    pub log: Rc<ProvisionLog>,
    pub engine_run: EngineRun,
    pub hash: String,
    pub resolved_items: Option<IndexMap<String, String>>,
    pub put_ids: IndexSet<String>,
    pub made: Vec<HeldRow>,
    pub carried: Vec<HeldRow>,
    pub follows: bool,
    pub writing: Chain,
    pub github_gate: RefCell<Option<oneshot::Sender<bool>>>,
    pub github_ready: Shared<LocalBoxFuture<'static, bool>>,
    pub landed_folders: RefCell<IndexMap<String, HeldRow>>,
    pub carried_folders: RefCell<IndexSet<String>>,
    pub steps_gate: RefCell<Option<oneshot::Sender<()>>>,
    pub steps_done: Shared<LocalBoxFuture<'static, ()>>,
    pub own_lander: RefCell<Option<Lander>>,
}

/// Runs the setup or the sync behind whoever asked for it (setup.ts:254). Never fails: a failure ends the run
/// `failed`, a computer that went away leaves it running, a sync that went nowhere reads behind.
pub async fn run_setup(area: Rc<PlaceSetupArea>, ask: RunAsk) {
    let Some(provisioner) = area.ctx.wiring.provision.clone() else {
        return;
    };
    let place_id = ask.place_id.clone();
    let Ok(Some(record)) = area.records.record_of(&place_id).await else {
        return;
    };
    let Some(home) = record.report.login.get("HOME").cloned() else {
        return;
    };
    let machine: Rc<dyn Machine> = Rc::new(PlaceMachine::new(
        area.link_to(&place_id),
        &record.name,
        &home,
    ));
    let now = iso_at(area.ctx.clock.now() as u64);
    let (verb, direction, at) = match &ask.sync {
        None => ("set up", "from", ask.started.started_at.clone()),
        Some(_) => ("synced", "to", now),
    };
    let header = format!(
        "wsp {} {verb} {} {direction} {} at {at}",
        (area.ctx.wiring.host_name)(),
        record.name,
        ask.picks.name
    );
    let log = ProvisionLog::new(
        Rc::clone(&area.ctx.clock),
        Rc::clone(&machine),
        place_provision_paths(&home),
        header,
    );
    let shelf = area.ctx.opts.recipes.as_ref().and_then(|f| f());
    let (hash, resolved_items) = match &ask.sync {
        Some(job) => (job.hash.clone(), Some(job.items.clone())),
        None => {
            let resolved = match (record.recipe.as_deref(), &shelf) {
                (None | Some(NO_RECIPE), Some(shelf)) => shelf.resolve_file(&ask.picks).await.ok(),
                (Some(slug), Some(shelf)) => shelf.resolve(slug).await.ok(),
                _ => None,
            };
            match resolved {
                Some(resolved) => (resolved.hash, Some(resolved.items)),
                None => (picks_hash(&ask.picks), None),
            }
        }
    };
    let before: Vec<HeldRow> = record
        .applied
        .as_ref()
        .map(|a| a.rows.clone())
        .unwrap_or_default();
    let before_plain: Vec<PlaceProvisionRow> = before.iter().map(|r| r.row.clone()).collect();
    let done = ask.done.clone();
    let put_rows: Vec<HeldRow> = before
        .iter()
        .filter(|r| engine_row(&r.row) && r.row.outcome == PlaceProvisionRowOutcome::Installed)
        .cloned()
        .collect();
    let put_ids: IndexSet<String> = put_rows.iter().map(|r| r.row.id.clone()).collect();
    let made: Vec<HeldRow> = before
        .iter()
        .filter(|r| r.row.step == Some(PlaceSetupStep::Folders) && r.row.project.is_some())
        .cloned()
        .collect();
    let carried: Vec<HeldRow> = if ask.sync.is_some() {
        Vec::new()
    } else {
        put_rows
            .iter()
            .chain(made.iter())
            .filter(|r| match r.row.step {
                Some(step) => !done.contains(&step),
                None => true,
            })
            .map(|r| {
                let mut earlier = r.clone();
                earlier.row.earlier = Some(true);
                earlier
            })
            .collect()
    };
    let stands = |r: &HeldRow| match r.row.step {
        None => false,
        Some(step) => {
            done.contains(&step)
                || (step == PlaceSetupStep::Signins
                    && !signs_in_again(
                        ask.only.as_ref(),
                        &sign_in_of_row(&r.row.id).unwrap_or_default(),
                        &before_plain,
                    ))
        }
    };
    let rows: Vec<HeldRow> = if ask.sync.is_some() {
        before.clone()
    } else {
        before
            .iter()
            .filter(|r| stands(r))
            .cloned()
            .chain(carried.iter().cloned())
            .collect()
    };
    let (gh_tx, gh_rx) = oneshot::channel::<bool>();
    let github_ready = gh_rx.map(|ok| ok.unwrap_or(false)).boxed_local().shared();
    let (steps_tx, steps_rx) = oneshot::channel::<()>();
    let steps_done = steps_rx.map(|_| ()).boxed_local().shared();
    let agent_files_there = done.contains(&PlaceSetupStep::Mcp)
        && ask
            .started
            .steps
            .iter()
            .any(|l| l.step == PlaceSetupStep::Mcp && l.state == PlaceSetupLineState::Done);
    let follows = record.recipe.as_deref().is_some_and(|r| r != NO_RECIPE);
    let run = Rc::new(SetupRun {
        area: Rc::clone(&area),
        rows: RefCell::new(rows),
        held: RefCell::new(ask.started.clone()),
        ended: Cell::new(false),
        stepped: Cell::new(false),
        running: RefCell::new(IndexSet::new()),
        folders_waiting: Cell::new(0),
        agent_files_there: Cell::new(agent_files_there),
        github_known: Cell::new(None),
        record,
        home,
        machine,
        log,
        engine_run: EngineRun::default(),
        hash,
        resolved_items,
        put_ids,
        made,
        carried,
        follows,
        writing: Chain::new(),
        github_gate: RefCell::new(Some(gh_tx)),
        github_ready,
        landed_folders: RefCell::new(IndexMap::new()),
        carried_folders: RefCell::new(IndexSet::new()),
        steps_gate: RefCell::new(Some(steps_tx)),
        steps_done,
        own_lander: RefCell::new(None),
        ask,
    });
    let lander: Lander = {
        let weak = Rc::downgrade(&run);
        Rc::new(move |landed: LandedRow| {
            let weak = weak.clone();
            async move {
                match weak.upgrade() {
                    Some(run) => run.lander(landed).await,
                    None => false,
                }
            }
            .boxed_local()
        })
    };
    *run.own_lander.borrow_mut() = Some(Rc::clone(&lander));
    area.landers.borrow_mut().insert(place_id.clone(), lander);
    if run.ask.done.contains(&PlaceSetupStep::Folders) {
        let folder_rows: Vec<HeldRow> = run.rows.borrow().clone();
        for r in folder_rows {
            if r.row.step == Some(PlaceSetupStep::Folders) {
                if let Some(key) = r.row.id.strip_prefix("folders/").map(str::to_string) {
                    run.landed_folder(&key, r);
                }
            }
        }
    }
    if run.ask.picks.configs.github.is_none() {
        run.settle_github(true);
    } else if (run.ask.done.contains(&PlaceSetupStep::Github)
        || run
            .ask
            .sync
            .as_ref()
            .is_some_and(|s| !s.steps.contains(&PlaceSetupStep::Github)))
        && !run.ask.rerun.iter().any(|w| w.row == GITHUB_ROW)
    {
        let known = run.rows.borrow().iter().any(|r| {
            r.row.id == GITHUB_ROW
                && matches!(
                    r.row.outcome,
                    PlaceProvisionRowOutcome::Present | PlaceProvisionRowOutcome::Installed
                )
        });
        run.settle_github(known);
    }
    if let Err(e) = run.body(Rc::clone(&provisioner)).await {
        if is_absent(&e) || (run.ask.sync.is_some() && !run.stepped.get()) {
            run.writing.settled().await;
            if !is_absent(&e) {
                run.log.write(&first_line_of(&e));
            }
            if let Some(sync) = run.ask.sync.as_ref() {
                let behind = PlaceSync {
                    state: PlaceSyncState::Behind,
                    changes: sync.state.changes.clone(),
                    since: sync.state.since.clone(),
                };
                let _ = area
                    .write_setup(
                        &place_id,
                        SetupPatch {
                            sync: Some(Some(behind.clone())),
                            ..Default::default()
                        },
                    )
                    .await;
                area.sync_frame(PlaceSyncEvent {
                    place_id: place_id.clone(),
                    sync: Some(behind),
                    line: None,
                    applied: None,
                });
            }
        } else {
            run.end(Some(first_line_of(&e))).await;
        }
    }
    run.end_steps();
    if !run.ended.get() {
        run.let_go();
    }
    let held = run.held.borrow().clone();
    let log = Rc::clone(&run.log);
    spawn_local(async move { log.close(&held).await });
}

/// A PlaceProvisionRow with nothing but its id, label and outcome.
pub fn plain_row(id: &str, label: &str, outcome: PlaceProvisionRowOutcome) -> PlaceProvisionRow {
    PlaceProvisionRow {
        id: id.into(),
        label: label.into(),
        outcome,
        kind: None,
        step: None,
        note: None,
        fix: None,
        keys: None,
        ms: None,
        earlier: None,
        project: None,
    }
}

/// The step's own word, as the rows and the log spell it.
pub fn step_word(step: PlaceSetupStep) -> &'static str {
    match step {
        PlaceSetupStep::Floor => "floor",
        PlaceSetupStep::Agents => "agents",
        PlaceSetupStep::Signins => "signins",
        PlaceSetupStep::Clis => "clis",
        PlaceSetupStep::Skills => "skills",
        PlaceSetupStep::Mcp => "mcp",
        PlaceSetupStep::Plugins => "plugins",
        PlaceSetupStep::Configs => "configs",
        PlaceSetupStep::Folders => "folders",
        PlaceSetupStep::FolderServers => "folderServers",
        PlaceSetupStep::Github => "github",
        PlaceSetupStep::Context => "context",
    }
}

fn step_label(step: PlaceSetupStep) -> &'static str {
    let word = step_word(step);
    SETUP_STEP_WORDS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, label)| *label)
        .unwrap_or(word)
}

fn state_word(state: PlaceSetupLineState) -> &'static str {
    match state {
        PlaceSetupLineState::Running => "running",
        PlaceSetupLineState::Done => "done",
        PlaceSetupLineState::Failed => "failed",
        PlaceSetupLineState::Skipped => "skipped",
    }
}

pub fn is_absent(e: &HostError) -> bool {
    e.kind.as_ref().is_some_and(|k| k.as_str() == "absent")
}

fn pick_named<'a, V>(
    map: &'a IndexMap<wsp_wire::types::recipe_file::RecipeName, V>,
    name: &str,
) -> Option<&'a V> {
    map.iter()
        .find(|(key, _)| key.0 == name)
        .map(|(_, value)| value)
}

pub fn github_word_of(picks: &RecipeFile) -> GitHubSignIn {
    picks
        .configs
        .github
        .as_ref()
        .and_then(|g| g.signin)
        .unwrap_or(GitHubSignIn::Vault)
}

fn graph_step(
    name: &'static str,
    after: Vec<&'static str>,
    marks: Vec<&'static str>,
    lanes: Vec<&'static str>,
    light: bool,
    run: impl FnOnce(Release<&'static str>) -> LocalBoxFuture<'static, PlaceResult<StepNext>> + 'static,
) -> GraphStep<&'static str> {
    GraphStep {
        name,
        after,
        marks,
        lanes: lanes.into_iter().map(str::to_string).collect(),
        light,
        run: Box::new(run),
    }
}

/// The engine's rows for one step, with the server names the vault holds handed to it (setup.ts:818).
async fn engine_rows(
    run: &Rc<SetupRun>,
    provisioner: &dyn PlaceProvisioner,
    plan: &ProvisionPlan,
    step: EngineStep,
) -> PlaceResult<Vec<PlaceProvisionRow>> {
    let stores = if step == EngineStep::Mcp {
        run.stores_checked().await?
    } else {
        None
    };
    let on = ProvisionOn {
        home: run.home.clone(),
        held: Some(run.server_names()),
        stores,
    };
    let got = provisioner
        .step(
            Rc::clone(&run.machine),
            plan,
            &step,
            &run.engine_run,
            run.stage_of(step.word()),
            &on,
        )
        .await?;
    Ok(run.ours(got))
}

impl SetupRun {
    pub fn vault(&self) -> IndexMap<String, String> {
        self.area
            .ctx
            .opts
            .vault
            .as_ref()
            .map(|vault| vault())
            .unwrap_or_default()
    }

    pub fn server_names(&self) -> IndexSet<String> {
        wsp_catalog::signin_rows::server_values_of(&self.vault())
            .keys()
            .cloned()
            .collect()
    }

    pub fn here(&self) -> String {
        self.area.here()
    }

    pub fn iso_now(&self) -> String {
        iso_at(self.area.ctx.clock.now() as u64)
    }

    /// `stageOf(s)`: the step's own lines, marked with the step (setup.ts:255).
    pub fn stage_of(&self, word: &'static str) -> ProvisionStage {
        let log = Rc::clone(&self.log);
        Arc::new(
            move |detail: &str, _at: Option<&StageAt>, _row: Option<&PlaceProvisionRow>| {
                log.write(&format!("[{word}] {detail}"))
            },
        )
    }

    /// The rows an engine step put on that a row installed earlier stays wsp's (setup.ts:300).
    pub fn ours(&self, got: Vec<PlaceProvisionRow>) -> Vec<PlaceProvisionRow> {
        got.into_iter()
            .map(|r| {
                if r.outcome == PlaceProvisionRowOutcome::Present && self.put_ids.contains(&r.id) {
                    PlaceProvisionRow {
                        outcome: PlaceProvisionRowOutcome::Installed,
                        earlier: Some(true),
                        ..r
                    }
                } else {
                    r
                }
            })
            .collect()
    }

    /// Settles the GitHub step once; the folders waiting on it are told (setup.ts:263).
    pub fn settle_github(&self, ok: bool) {
        let gate = self.github_gate.borrow_mut().take();
        if let Some(gate) = gate {
            self.github_known.set(Some(ok));
            let _ = gate.send(ok);
        }
    }

    pub fn end_steps(&self) {
        let gate = self.steps_gate.borrow_mut().take();
        if let Some(gate) = gate {
            let _ = gate.send(());
        }
    }

    /// A folder that landed as a project is remembered for its servers (setup.ts:689).
    pub fn landed_folder(&self, key: &str, row: HeldRow) -> HeldRow {
        if row.row.outcome == PlaceProvisionRowOutcome::Installed && row.row.project.is_some() {
            self.landed_folders
                .borrow_mut()
                .insert(key.to_string(), row.clone());
        }
        row
    }

    /// The applied rows as the record keeps them (setup.ts:284).
    pub fn applied(&self) -> HeldApplied {
        let (hash, items) = if self.ask.sync.is_some() && !self.ended.get() {
            (
                self.record
                    .applied
                    .as_ref()
                    .map(|a| a.hash.clone())
                    .unwrap_or_default(),
                self.record.applied.as_ref().and_then(|a| a.items.clone()),
            )
        } else {
            (self.hash.clone(), self.resolved_items.clone())
        };
        let mut by_id: IndexMap<String, HeldRow> = IndexMap::new();
        for r in self.rows.borrow().iter() {
            by_id.insert(r.row.id.clone(), r.clone());
        }
        let rows = by_id
            .into_values()
            .map(|mut r| {
                if r.row.outcome == PlaceProvisionRowOutcome::Failed && r.row.fix.is_none() {
                    r.row.fix = setup_row_fix(&r.row, &self.record.name);
                }
                r
            })
            .collect();
        HeldApplied {
            hash,
            at: self.iso_now(),
            rows,
            items,
        }
    }

    /// The setup as it stands, written after every earlier write (setup.ts:297).
    pub fn push(self: &Rc<Self>, next: PlaceSetup) {
        *self.held.borrow_mut() = next.clone();
        let run = Rc::clone(self);
        self.writing.push(
            async move {
                let applied = run.applied();
                let patch = SetupPatch {
                    setup: Some(next),
                    applied: Some(applied),
                    ..Default::default()
                };
                let _ = run.area.write_setup(&run.ask.place_id, patch).await;
            }
            .boxed_local(),
        );
    }

    /// A frame said once the writes before it are on the record (setup.ts:307).
    pub fn frame_after_write(&self, event: PlaceSetupEvent) {
        let area = Rc::clone(&self.area);
        self.writing.push(
            async move {
                area.setup_frame(event);
            }
            .boxed_local(),
        );
    }

    /// One line of a step (setup.ts:310).
    pub fn line(self: &Rc<Self>, l: PlaceSetupLine) {
        {
            let mut running = self.running.borrow_mut();
            if l.state == PlaceSetupLineState::Running {
                running.insert(l.step);
            } else {
                running.shift_remove(&l.step);
            }
        }
        let mut next = self.held.borrow().clone();
        if self.ask.sync.is_none() {
            match next.steps.iter_mut().find(|s| s.step == l.step) {
                Some(slot) => *slot = l.clone(),
                None => next.steps.push(l.clone()),
            }
        }
        self.push(next);
        let note = l
            .note
            .as_ref()
            .map(|n| format!(" ({n})"))
            .unwrap_or_default();
        self.log.write(&format!(
            "[{}] {}: {}{note}",
            step_word(l.step),
            step_label(l.step),
            state_word(l.state)
        ));
        match &self.ask.sync {
            Some(sync) => self.area.sync_frame(PlaceSyncEvent {
                place_id: self.ask.place_id.clone(),
                sync: Some(sync.state.clone()),
                line: Some(l),
                applied: None,
            }),
            None => {
                let running: Vec<PlaceSetupStep> = self.running.borrow().iter().copied().collect();
                self.frame_after_write(PlaceSetupEvent {
                    add_id: self.ask.add_id.clone(),
                    place_id: self.ask.place_id.clone(),
                    line: Some(l),
                    wait: None,
                    landed: None,
                    running: Some(running),
                    end: None,
                    said: None,
                })
            }
        }
    }

    /// One step: marked running, run, its rows kept with the step on each, and marked done or failed with what it
    /// took. A step that throws is one failed row, unless the computer stopped answering, which the job waits out
    /// (setup.ts:330). `None` where it does not run this time.
    pub async fn step_held(
        self: &Rc<Self>,
        name: PlaceSetupStep,
        work: impl Future<Output = PlaceResult<Vec<HeldRow>>>,
    ) -> PlaceResult<Option<(Vec<HeldRow>, bool)>> {
        if self.ask.done.contains(&name)
            || self
                .ask
                .sync
                .as_ref()
                .is_some_and(|s| !s.steps.contains(&name))
        {
            return Ok(None);
        }
        self.stepped.set(true);
        let began = self.area.ctx.clock.now() as u64;
        self.line(PlaceSetupLine {
            step: name,
            state: PlaceSetupLineState::Running,
            note: None,
            ms: None,
            started_at: Some(iso_at(began)),
        });
        let got: Vec<HeldRow> = match work.await {
            Ok(rows) => {
                self.rows
                    .borrow_mut()
                    .retain(|r| !(self.carried.contains(r) && r.row.step == Some(name)));
                rows.into_iter()
                    .map(|mut r| {
                        r.row.step = Some(name);
                        r
                    })
                    .collect()
            }
            Err(e) if is_absent(&e) => return Err(e),
            Err(e) => vec![held_of(PlaceProvisionRow {
                note: Some(first_line_of(&e)),
                step: Some(name),
                ..plain_row(
                    &format!("{}/stopped", step_word(name)),
                    step_label(name),
                    PlaceProvisionRowOutcome::Failed,
                )
            })],
        };
        self.rows.borrow_mut().extend(got.iter().cloned());
        let failed = got
            .iter()
            .filter(|r| r.row.outcome == PlaceProvisionRowOutcome::Failed)
            .count();
        let ms = (self.area.ctx.clock.now() as f64 - began as f64)
            .max(0.0)
            .round();
        let note = (failed > 0).then(|| format!("{failed} of {} failed", got.len()));
        let state = if failed > 0 {
            PlaceSetupLineState::Failed
        } else {
            PlaceSetupLineState::Done
        };
        self.line(PlaceSetupLine {
            step: name,
            state,
            note,
            ms: Some(wsp_wire::types::js_number::JsInt(
                wsp_wire::types::js_number::JsNumber(ms),
            )),
            started_at: None,
        });
        Ok(Some((got, failed > 0)))
    }

    /// One step's rows as the step answers them (setup.ts:330).
    pub async fn step(
        self: &Rc<Self>,
        name: PlaceSetupStep,
        work: impl Future<Output = PlaceResult<Vec<PlaceProvisionRow>>>,
    ) -> PlaceResult<Option<(Vec<PlaceProvisionRow>, bool)>> {
        let answered = self
            .step_held(name, async {
                work.await
                    .map(|rows| rows.into_iter().map(held_of).collect())
            })
            .await?;
        Ok(answered.map(|(rows, failed)| (rows.into_iter().map(|r| r.row).collect(), failed)))
    }

    /// A row that lands after its step ended (setup.ts:352).
    pub fn land_row(self: &Rc<Self>, r: HeldRow) {
        {
            let mut rows = self.rows.borrow_mut();
            rows.retain(|x| x.row.id != r.row.id);
            rows.push(r.clone());
        }
        let held = self.held.borrow().clone();
        self.push(held.clone());
        let through =
            self.ended.get() && held.waiting.is_empty() && self.folders_waiting.get() == 0;
        let (end, said) = if through {
            self.outcome()
        } else {
            (None, None)
        };
        self.frame_after_write(PlaceSetupEvent {
            add_id: self.ask.add_id.clone(),
            place_id: self.ask.place_id.clone(),
            line: None,
            wait: None,
            landed: Some(r.row.id.clone()),
            running: None,
            end,
            said,
        });
        if through && self.ask.sync.is_none() {
            self.let_go();
        }
    }

    /// How a setup came out, as the frame says it (helpers.ts:359).
    pub fn outcome(&self) -> (Option<wsp_wire::views::place::SetupEnd>, Option<String>) {
        let held = self.held.borrow().clone();
        let applied = applied_view(&self.applied());
        let (end, said) = setup_outcome(&held, Some(&applied));
        (Some(end), said)
    }

    /// Lands a row from outside the run; true once its write is on the record (setup.ts:361).
    pub async fn lander(self: &Rc<Self>, r: LandedRow) -> bool {
        let plain: Vec<PlaceProvisionRow> =
            self.rows.borrow().iter().map(|h| h.row.clone()).collect();
        let Some(rows) = super::super::helpers::landed_on(&plain, &r) else {
            return false;
        };
        let Some(landed) = rows.into_iter().find(|x| x.id == r.row.id) else {
            return false;
        };
        self.land_row(held_of(landed));
        self.writing.settled().await;
        true
    }

    /// This run writes no more rows (setup.ts:370).
    pub fn let_go(&self) {
        let own = self.own_lander.borrow().clone();
        let Some(own) = own else { return };
        let mut landers = self.area.landers.borrow_mut();
        let current = landers.get(&self.ask.place_id).cloned();
        if current.is_some_and(|current| Rc::ptr_eq(&current, &own)) {
            landers.shift_remove(&self.ask.place_id);
        }
    }

    /// The end of the run (setup.ts:746).
    pub async fn end(self: &Rc<Self>, failed: Option<String>) {
        self.ended.set(true);
        self.end_steps();
        let place_id = self.ask.place_id.clone();
        if self.ask.sync.is_some() {
            self.writing.settled().await;
            if let Some(line) = &failed {
                self.log.write(line);
            }
            let applied = self.applied();
            let patch = SetupPatch {
                applied: Some(applied.clone()),
                picks: Some(self.ask.picks.clone()),
                sync: Some(None),
                ..Default::default()
            };
            let _ = self.area.write_setup(&place_id, patch).await;
            self.let_go();
            self.area.sync_frame(PlaceSyncEvent {
                place_id,
                sync: None,
                line: None,
                applied: Some(applied_view(&applied)),
            });
            return;
        }
        let mut next = self.held.borrow().clone();
        next.state = if failed.is_none() {
            PlaceSetupState::Done
        } else {
            PlaceSetupState::Failed
        };
        next.finished_at = Some(self.iso_now());
        if let Some(said) = &failed {
            next.said = Some(said.clone());
        }
        self.push(next);
        self.writing.settled().await;
        if self.held.borrow().waiting.is_empty() && self.folders_waiting.get() == 0 {
            self.let_go();
        }
        if failed.is_none() && self.folders_waiting.get() > 0 {
            return;
        }
        let (end, said) = match &failed {
            None => self.outcome(),
            Some(said) => (
                Some(wsp_wire::views::place::SetupEnd::Failed),
                Some(said.clone()),
            ),
        };
        self.area.setup_frame(PlaceSetupEvent {
            add_id: self.ask.add_id.clone(),
            place_id,
            line: None,
            wait: None,
            landed: None,
            running: None,
            end,
            said,
        });
    }

    /// The whole run from its first step to its end (setup.ts:808).
    async fn body(self: &Rc<Self>, provisioner: Rc<dyn PlaceProvisioner>) -> PlaceResult<()> {
        let picks = self.ask.picks.clone();
        if picks.configs.github.is_some()
            && github_word_of(&picks) == GitHubSignIn::Vault
            && self.vault().get(GITHUB_TOKEN_ENV).is_none()
        {
            if let Some(token) = self.area.ctx.wiring.github_token.clone() {
                let _ = token.github_token().await;
            }
        }
        self.undo().await?;
        let stores = self.stores_here().await;
        let recipe = self.follows.then(|| picks.name.clone());
        let planned = provisioner
            .setup(
                &picks,
                SetupOn {
                    home: self.home.clone(),
                    stores,
                    recipe,
                },
                self.ask.sync.as_ref().map(|s| &s.steps),
            )
            .await?;
        let mut kept = planned;
        if let (Some(sync), Some(plugins)) = (self.ask.sync.as_ref(), kept.plugins.clone()) {
            kept.plugins = Some(
                plugins
                    .into_iter()
                    .filter(|p| sync.moved.contains(&p.id))
                    .collect(),
            );
        }
        let gh_row =
            pick_named(&picks.clis, GITHUB_CLI).map(|cli| tool_row_id(&cli.via, GITHUB_CLI));
        let hooks: Vec<String> = kept
            .steps
            .iter()
            .skip(kept.agents)
            .filter(|s| hook_of(&s.id).is_some())
            .map(|s| s.id.clone())
            .collect();
        let mut first: Vec<String> = gh_row.iter().cloned().collect();
        first.extend(hooks.iter().cloned());
        first.extend(kept.server_tools.clone().unwrap_or_default());
        let plan = Rc::new(cli_first(&kept, &first));
        for w in self.ask.rerun.clone() {
            if w.row == GITHUB_ROW {
                self.github_sign_in(&plan).await;
            } else if let Some(agent) = sign_in_of_row(&w.row) {
                self.again_sign_in(&agent, &plan).await;
            }
        }
        let floor = self
            .step(
                PlaceSetupStep::Floor,
                engine_rows(self, &*provisioner, &plan, EngineStep::Floor),
            )
            .await?;
        if let Some((rows, true)) = floor {
            self.end(Some(floor_failed_line(&rows))).await;
            return Ok(());
        }
        let stopped: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let installs: IndexSet<String> = plan
            .steps
            .iter()
            .take(plan.agents)
            .map(|s| s.id.clone())
            .collect();
        let graph = self.graph(&provisioner, &plan, &stopped, installs, gh_row, hooks);
        let steps: Vec<GraphStep<&'static str>> = graph
            .into_iter()
            .filter(|g| !self.ask.done.iter().any(|s| step_word(*s) == g.name))
            .collect();
        let width = setup_width(self.record.report.shape.mem_mb.0) as usize;
        run_graph(steps, width, None).await?;
        let stopped_line = stopped.borrow().clone();
        if let Some(line) = stopped_line {
            self.end(Some(line)).await;
            return Ok(());
        }
        self.step(
            PlaceSetupStep::Context,
            engine_rows(self, &*provisioner, &plan, EngineStep::Context),
        )
        .await?;
        self.end(None).await;
        Ok(())
    }

    /// The steps after the floor, their waits and marks (setup.ts:876-921).
    fn graph(
        self: &Rc<Self>,
        provisioner: &Rc<dyn PlaceProvisioner>,
        plan: &Rc<ProvisionPlan>,
        stopped: &Rc<RefCell<Option<String>>>,
        installs: IndexSet<String>,
        gh_row: Option<String>,
        hooks: Vec<String>,
    ) -> Vec<GraphStep<&'static str>> {
        let mut graph = Vec::new();
        {
            let (run, p, plan, stopped, installs) = (
                Rc::clone(self),
                Rc::clone(provisioner),
                Rc::clone(plan),
                Rc::clone(stopped),
                installs.clone(),
            );
            graph.push(graph_step(
                "agents",
                vec![],
                vec![],
                vec![INSTALLS],
                false,
                move |_release| {
                    async move {
                        let agents = run
                            .step(
                                PlaceSetupStep::Agents,
                                engine_rows(&run, &*p, &plan, EngineStep::Agents),
                            )
                            .await?;
                        let agent_rows: Vec<PlaceProvisionRow> = agents
                            .map(|(rows, _)| rows)
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|r| installs.contains(&r.id))
                            .collect();
                        if agent_rows.is_empty()
                            || !agent_rows
                                .iter()
                                .all(|r| r.outcome == PlaceProvisionRowOutcome::Failed)
                        {
                            return Ok(StepNext::Go);
                        }
                        *stopped.borrow_mut() = Some(no_agent_line(&agent_rows));
                        Ok(StepNext::Stop)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let (run, plan) = (Rc::clone(self), Rc::clone(plan));
            graph.push(graph_step(
                "signins",
                vec!["agents"],
                vec![],
                vec![],
                true,
                move |_release| {
                    async move {
                        run.step(PlaceSetupStep::Signins, async {
                            Ok(run.sign_ins(&plan).await)
                        })
                        .await
                        .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let (run, p, plan) = (Rc::clone(self), Rc::clone(provisioner), Rc::clone(plan));
            graph.push(graph_step(
                "skills",
                vec![],
                vec![],
                vec![FILES],
                false,
                move |_release| {
                    async move {
                        run.step(
                            PlaceSetupStep::Skills,
                            engine_rows(&run, &*p, &plan, EngineStep::Skills),
                        )
                        .await
                        .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let (run, plan) = (Rc::clone(self), Rc::clone(plan));
            let after = if gh_row.is_some() { vec!["gh"] } else { vec![] };
            let lanes = if plan.github.is_none() {
                vec![]
            } else {
                vec![INSTALLS]
            };
            graph.push(graph_step(
                "github",
                after,
                vec![],
                lanes,
                false,
                move |_release| {
                    async move {
                        run.step(PlaceSetupStep::Github, async { run.github(&plan).await })
                            .await
                            .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let (run, p, plan) = (Rc::clone(self), Rc::clone(provisioner), Rc::clone(plan));
            let gh: IndexSet<String> = gh_row.iter().cloned().collect();
            let hook_set: IndexSet<String> = hooks.iter().cloned().collect();
            let servers: IndexSet<String> = plan
                .server_tools
                .clone()
                .unwrap_or_default()
                .into_iter()
                .collect();
            graph.push(graph_step("clis", vec!["agents"], vec!["gh", "hooks", "servers"], vec![INSTALLS], false, move |release: Release<&'static str>| {
                let waits: Rc<Vec<(&'static str, RefCell<IndexSet<String>>)>> =
                    Rc::new(vec![("gh", RefCell::new(gh)), ("hooks", RefCell::new(hook_set)), ("servers", RefCell::new(servers))]);
                for (mark, rows) in waits.iter() {
                    if rows.borrow().is_empty() {
                        release(mark);
                    }
                }
                async move {
                    let stage: ProvisionStage = {
                        let (log, waits, release) = (Rc::clone(&run.log), Rc::clone(&waits), Rc::clone(&release));
                        Arc::new(move |detail: &str, _at: Option<&StageAt>, row: Option<&PlaceProvisionRow>| {
                            log.write(&format!("[clis] {detail}"));
                            let Some(row) = row else { return };
                            for (mark, rows) in waits.iter() {
                                let removed = rows.borrow_mut().shift_remove(&row.id);
                                if removed && rows.borrow().is_empty() {
                                    release(mark);
                                }
                            }
                        })
                    };
                    let on = ProvisionOn { home: run.home.clone(), held: None, stores: None };
                    run.step(PlaceSetupStep::Clis, async {
                        let got = p.step(Rc::clone(&run.machine), &plan, &EngineStep::Clis, &run.engine_run, stage, &on).await?;
                        Ok(run.ours(got))
                    })
                    .await
                    .map(|_| StepNext::Go)
                }
                .boxed_local()
            }));
        }
        {
            let (run, p, plan) = (Rc::clone(self), Rc::clone(provisioner), Rc::clone(plan));
            graph.push(graph_step(
                "mcp",
                vec!["agents", "servers"],
                vec![],
                vec![FILES],
                false,
                move |_release| {
                    async move {
                        let done = run
                            .step(
                                PlaceSetupStep::Mcp,
                                engine_rows(&run, &*p, &plan, EngineStep::Mcp),
                            )
                            .await?;
                        let there = match &done {
                            None => true,
                            Some((rows, _)) => !rows.iter().any(|r| {
                                r.outcome == PlaceProvisionRowOutcome::Failed
                                    && (r.id.starts_with("files/") || r.id == "mcp/stopped")
                            }),
                        };
                        run.agent_files_there.set(there);
                        Ok(StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let (run, p, plan) = (Rc::clone(self), Rc::clone(provisioner), Rc::clone(plan));
            let lanes = if plan.config_tools.as_ref().is_some_and(|t| !t.is_empty()) {
                vec![INSTALLS, FILES]
            } else {
                vec![FILES]
            };
            graph.push(graph_step(
                "configs",
                vec![],
                vec![],
                lanes,
                false,
                move |_release| {
                    async move {
                        run.step(
                            PlaceSetupStep::Configs,
                            engine_rows(&run, &*p, &plan, EngineStep::Configs),
                        )
                        .await
                        .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let (run, p, plan) = (Rc::clone(self), Rc::clone(provisioner), Rc::clone(plan));
            graph.push(graph_step(
                "plugins",
                vec!["mcp"],
                vec![],
                vec![],
                false,
                move |_release| {
                    async move {
                        run.step(
                            PlaceSetupStep::Plugins,
                            engine_rows(&run, &*p, &plan, EngineStep::Plugins),
                        )
                        .await
                        .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let run = Rc::clone(self);
            graph.push(graph_step(
                "folders",
                vec!["github", "hooks"],
                vec![],
                vec![],
                false,
                move |_release| {
                    async move {
                        run.step_held(PlaceSetupStep::Folders, run.folders())
                            .await
                            .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        {
            let run = Rc::clone(self);
            graph.push(graph_step(
                "folderServers",
                vec!["mcp", "plugins", "folders"],
                vec![],
                vec![FILES],
                false,
                move |_release| {
                    async move {
                        run.step(PlaceSetupStep::FolderServers, async {
                            run.folder_servers().await
                        })
                        .await
                        .map(|_| StepNext::Go)
                    }
                    .boxed_local()
                },
            ));
        }
        graph
    }
}
