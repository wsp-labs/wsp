// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts` at 65f6b89c7: the links' waits, the setup and the sync of a computer to its
//! recipe, and the link a machine there is driven over. Specs: `spec/places_setup_area.md` (this file),
//! `places_setup_run.md` (`setup/run.rs`), `places_setup_sign_ins.md` (`setup/sign_ins.rs`),
//! `places_setup_folders.md` (`setup/folders.rs`).

pub mod folders;
pub mod run;
pub mod sign_ins;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use crate::local_task::spawn as spawn_local;
use async_trait::async_trait;
use futures::future::{select, Either, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_core::console::warn;
use wsp_core::recipe_sync::{recipe_changes, steps_for, RecipeChange, RecipeChangeHow};
use wsp_core::types::pending::Clock;
use wsp_engine::errors::{EngineError, Result as EngineResult};
use wsp_engine::link_backend::{LinkAsk, MachineLink};
use wsp_wire::types::recipe_file::{RecipeFile, NO_RECIPE};
use wsp_wire::views::place::{
    PlaceSetup, PlaceSetupEvent, PlaceSetupState, PlaceSetupStep, PlaceSync, PlaceSyncEvent,
    PlaceSyncState, PlaceWait, RecipesChangedEvent, GITHUB_ROW, SETUP_STEP_WORDS,
};
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::js_date::iso_string;
use wsp_words::views::place_setup_words::{
    no_setup_row_refusal, place_no_home_line, place_no_picks_line, place_provisioning_line,
    place_syncing_line,
};
use wsp_words::workspace_state::absent_computer;

use super::context::PlaceDoorContext;
use super::helpers::{first_line_of, refused_engine, Cancel, LandedRow, SyncJob};
use super::types::{
    ForwardAsk, HeldApplied, HeldRow, PlaceRecord, PlaceResult, Resolved, SetupEvent,
};
use run::{run_setup, RunAsk};

/// What the setup area reads off the records area (setup.ts:115). Slice b's `PlaceRecordsArea` implements it; the
/// tests drive a fake.
#[async_trait(?Send)]
pub trait SetupRecords {
    async fn record_of(&self, place_id: &str) -> PlaceResult<Option<PlaceRecord>>;
    /// One change of a computer's record as it stands, in its turn (records.ts:349): `None` where `mv` answered `None`
    /// or the record is gone.
    async fn change(&self, place_id: &str, mv: RecordMove) -> PlaceResult<Option<PlaceRecord>>;
    /// A login now standing on that computer, put on its daemon's last list (records.ts:359).
    async fn login_listed(&self, place_id: &str, agent: &str) -> PlaceResult<Option<PlaceRecord>>;
}

/// A move of a record from how it stands to what is written; `None` writes nothing.
pub type RecordMove = Box<dyn for<'a> FnOnce(&'a PlaceRecord) -> Option<PlaceRecord>>;

/// One row of a setup that ended, run again: the step it is under, and the row (setup.ts:84).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnlyRow {
    pub step: PlaceSetupStep,
    pub row: String,
}

/// The steps a run of one row takes with its own (setup.ts:92).
pub fn row_follows(step: PlaceSetupStep) -> &'static [PlaceSetupStep] {
    match step {
        PlaceSetupStep::Clis => &[PlaceSetupStep::Mcp, PlaceSetupStep::FolderServers],
        PlaceSetupStep::Folders => &[PlaceSetupStep::FolderServers],
        PlaceSetupStep::Github => &[PlaceSetupStep::Folders, PlaceSetupStep::FolderServers],
        PlaceSetupStep::Agents => &[PlaceSetupStep::Signins],
        _ => &[],
    }
}

/// The step a row of a setup is under (setup.ts:95).
pub fn row_step(record: &PlaceRecord, row: &str) -> Option<PlaceSetupStep> {
    if let Some(step) = step_of_word(row) {
        return Some(step);
    }
    let held = record
        .applied
        .as_ref()
        .and_then(|a| a.rows.iter().find(|r| r.row.id == row))
        .and_then(|r| r.row.step);
    if held.is_some() {
        return held;
    }
    if !record
        .setup
        .as_ref()
        .is_some_and(|s| s.waiting.iter().any(|w| w.row == row))
    {
        return None;
    }
    Some(if row == GITHUB_ROW {
        PlaceSetupStep::Github
    } else {
        PlaceSetupStep::Signins
    })
}

/// Whether a run signs an agent in again (setup.ts:105).
pub fn signs_in_again(
    only: Option<&OnlyRow>,
    agent: &str,
    rows: &[wsp_wire::views::place::PlaceProvisionRow],
) -> bool {
    let Some(only) = only else { return true };
    if only.row == "signins" {
        return true;
    }
    if only.step == PlaceSetupStep::Signins {
        return only.row == wsp_wire::views::place::sign_in_row_id(agent);
    }
    let id = wsp_wire::views::place::sign_in_row_id(agent);
    match rows.iter().find(|r| r.id == id) {
        None => true,
        Some(row) => {
            row.note.as_deref()
                == Some(
                    wsp_words::views::place_setup_words::waits_for_install_line(&row.label)
                        .as_str(),
                )
        }
    }
}

/// A patch of the record's setup fields, written over the record as it stands (setup.ts:193). `sync: Some(None)`
/// removes the sync.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetupPatch {
    pub setup: Option<PlaceSetup>,
    pub applied: Option<HeldApplied>,
    pub picks: Option<RecipeFile>,
    pub recipe: Option<String>,
    pub sync: Option<Option<PlaceSync>>,
}

impl SetupPatch {
    fn apply(self, now: &PlaceRecord) -> PlaceRecord {
        let mut next = now.clone();
        if let Some(setup) = self.setup {
            next.setup = Some(setup);
        }
        if let Some(applied) = self.applied {
            next.applied = Some(applied);
        }
        if let Some(picks) = self.picks {
            next.picks = Some(picks);
        }
        if let Some(recipe) = self.recipe {
            next.recipe = Some(recipe);
        }
        match self.sync {
            Some(None) => next.sync = None,
            Some(Some(sync)) => next.sync = Some(sync),
            None => {}
        }
        next
    }
}

/// How a start stands the moment it is under way, or the sentence it said instead (setup.ts:927).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Started {
    pub setup: Option<PlaceSetup>,
    pub said: Option<String>,
}

/// Picks given to a start, with the recipe they came from.
#[derive(Clone, Debug, PartialEq)]
pub struct GivenPicks {
    pub picks: RecipeFile,
    pub recipe: Option<String>,
}

/// What moved between what a computer last applied and the recipe it follows (setup.ts:974).
#[derive(Clone, Debug, PartialEq)]
pub struct SyncRead {
    pub resolved: Resolved,
    pub changes: Vec<RecipeChange>,
}

/// A wait for one computer to open a socket again: woken with true for the socket it opened.
pub type Wake = Rc<dyn Fn(bool)>;
/// A reader of one computer's daemon events over a channel.
pub type ChannelReader = Rc<dyn Fn(&serde_json::Map<String, serde_json::Value>)>;
/// A skip of a row a running setup waits on: lands it skipped and stops what waited.
pub type Skipper = Rc<dyn Fn() -> LocalBoxFuture<'static, ()>>;
/// Lands a row from outside the run on the run still writing that computer's rows; true where it landed.
pub type Lander = Rc<dyn Fn(LandedRow) -> LocalBoxFuture<'static, bool>>;

/// A sign-in a setup is following on a computer, by `<place id>/<agent>`.
pub struct SigningIn {
    pub mute: Rc<dyn Fn()>,
    pub leave: Rc<dyn Fn()>,
}

/// The setup area (setup.ts:113). Every map is memory only.
pub struct PlaceSetupArea {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<dyn SetupRecords>,
    /// The channels a road on this host opened over each computer's link, by place id, then by reader id.
    pub channels: RefCell<IndexMap<String, IndexMap<u64, ChannelReader>>>,
    /// What is waiting for each computer to open a socket again, by place id, then by wake id.
    pub waiting: RefCell<IndexMap<String, IndexMap<u64, Wake>>>,
    /// When each computer's last socket closed under this host, in clock ms.
    pub closed_at: RefCell<IndexMap<String, u64>>,
    pub setting: RefCell<IndexSet<String>>,
    pub syncing: RefCell<IndexSet<String>>,
    pub signing_in: RefCell<IndexMap<String, SigningIn>>,
    pub skippers: RefCell<IndexMap<String, Skipper>>,
    pub landers: RefCell<IndexMap<String, Lander>>,
    /// The computers a change reached while a setup or a sync ran there.
    pub again: RefCell<IndexSet<String>>,
    pub sync_timers: RefCell<IndexMap<String, Cancel>>,
    next_id: std::cell::Cell<u64>,
}

thread_local! {
    /// The computer whose setup's own folder step the running call belongs to (setup.ts:190).
    static FOLDERS_OF: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// The place the current call runs under as its setup's own folder add, if any.
pub fn current_folders_of() -> Option<String> {
    FOLDERS_OF.with(|place| place.borrow().clone())
}

/// Puts the mark it found back when it drops, whether the poll returned or unwound.
struct Restore(Option<String>);

impl Drop for Restore {
    fn drop(&mut self) {
        FOLDERS_OF.with(|held| *held.borrow_mut() = self.0.take());
    }
}

/// Polls `work` with `place` marked as the setup whose folder add it belongs to.
fn polled_under<R>(place: &str, work: impl FnOnce() -> R) -> R {
    let _restore = Restore(FOLDERS_OF.with(|held| held.replace(Some(place.to_string()))));
    work()
}

/// Runs `work` as the folder add of `place_id`'s own setup. The TS's `AsyncLocalStorage` follows every continuation of
/// the add, so each task the add starts is under it as well; a task-local would not follow a spawned task. The add runs
/// on a `LocalSet` of its own, which the mark is set around whenever it is polled, so the tasks the add starts land on
/// that set and carry the mark, and go on carrying it after the add has answered.
pub async fn under_folders_of<T>(place_id: &str, work: impl Future<Output = T>) -> T {
    let set = tokio::task::LocalSet::new();
    let out = {
        let mut run = Box::pin(set.run_until(work));
        std::future::poll_fn(|cx| polled_under(place_id, || run.as_mut().poll(cx))).await
    };
    let place = place_id.to_string();
    let mut rest = Box::pin(set);
    crate::local_task::spawn(std::future::poll_fn(move |cx| {
        polled_under(&place, || rest.as_mut().poll(cx))
    }));
    out
}

fn step_of_word(word: &str) -> Option<PlaceSetupStep> {
    if !SETUP_STEP_WORDS.iter().any(|(name, _)| *name == word) {
        return None;
    }
    serde_json::from_value(Value::String(word.to_string())).ok()
}

/// The ISO stamp of a clock reading, as `new Date(ms).toISOString()` writes it.
pub fn iso_at(ms: u64) -> String {
    iso_string(ms as f64).unwrap_or_default()
}

/// `wsp_engine`'s errors as the area's own host errors, the absent one kept by its kind.
pub fn host_of(e: &EngineError) -> HostError {
    let mut host = HostError::new(e.to_string());
    if matches!(e, EngineError::PlaceAbsent { .. }) {
        host.kind = Some(Kind::parse("absent"));
    }
    host
}

/// `bounded` over an engine road: its answer, or after `ms` on `clock` the error `"{what} was not answered in
/// {round(ms / 1000)}s"`; the timer is cancelled once the road settles (helpers.ts:56).
async fn bounded_engine<T>(
    clock: &dyn Clock,
    work: impl Future<Output = EngineResult<T>>,
    ms: u64,
    what: &str,
) -> EngineResult<T> {
    let (fire, fired) = futures::channel::oneshot::channel::<()>();
    let cancel = clock.schedule(
        Box::new(move || {
            let _ = fire.send(());
        }),
        ms as i64,
    );
    let work = std::pin::pin!(work);
    let outcome = select(work, fired).await;
    cancel();
    match outcome {
        Either::Left((answer, _)) => answer,
        Either::Right(_) => Err(EngineError::Other {
            message: format!("{what} was not answered in {}s", (ms + 500) / 1000),
            code: None,
        }),
    }
}

impl PlaceSetupArea {
    pub fn new(ctx: Rc<PlaceDoorContext>, records: Rc<dyn SetupRecords>) -> Rc<PlaceSetupArea> {
        Rc::new(PlaceSetupArea {
            ctx,
            records,
            channels: RefCell::new(IndexMap::new()),
            waiting: RefCell::new(IndexMap::new()),
            closed_at: RefCell::new(IndexMap::new()),
            setting: RefCell::new(IndexSet::new()),
            syncing: RefCell::new(IndexSet::new()),
            signing_in: RefCell::new(IndexMap::new()),
            skippers: RefCell::new(IndexMap::new()),
            landers: RefCell::new(IndexMap::new()),
            again: RefCell::new(IndexSet::new()),
            sync_timers: RefCell::new(IndexMap::new()),
            next_id: Cell::new(0),
        })
    }

    fn take_id(&self) -> u64 {
        let id = self.next_id.get() + 1;
        self.next_id.set(id);
        id
    }

    /// Ends every wait on one computer (setup.ts:131).
    pub fn woken(&self, place_id: &str, back: bool) {
        let held = self.waiting.borrow_mut().shift_remove(place_id);
        for wake in held.unwrap_or_default().into_values() {
            wake(back);
        }
    }

    /// Waits for that computer to dial in again, up to `until` on the clock (setup.ts:140).
    pub async fn dials_back(self: &Rc<Self>, place_id: &str, until: u64) -> bool {
        let up = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .map(|l| l.socket.is_open());
        if up == Some(true) {
            return false;
        }
        let now = self.ctx.clock.now() as u64;
        if until <= now {
            return false;
        }
        let left = until - now;
        let id = self.take_id();
        let (tx, rx) = futures::channel::oneshot::channel::<bool>();
        let sender = Rc::new(RefCell::new(Some(tx)));
        let timer: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let place = place_id.to_string();
        let wake: Wake = {
            let (sender, timer, place, weak) = (
                Rc::clone(&sender),
                Rc::clone(&timer),
                place.clone(),
                Rc::downgrade(self),
            );
            Rc::new(move |back: bool| {
                let Some(tx) = sender.borrow_mut().take() else {
                    return;
                };
                if let Some(area) = weak.upgrade() {
                    let mut waiting = area.waiting.borrow_mut();
                    let empty = match waiting.get_mut(&place) {
                        Some(held) => {
                            held.shift_remove(&id);
                            held.is_empty()
                        }
                        None => false,
                    };
                    if empty {
                        waiting.shift_remove(&place);
                    }
                }
                let cancel = timer.borrow_mut().take();
                if let Some(cancel) = cancel {
                    cancel();
                }
                let _ = tx.send(back);
            })
        };
        self.waiting
            .borrow_mut()
            .entry(place.clone())
            .or_default()
            .insert(id, Rc::clone(&wake));
        let fire = Rc::clone(&wake);
        let cancel = self
            .ctx
            .clock
            .schedule(Box::new(move || fire(false)), left as i64);
        *timer.borrow_mut() = Some(cancel);
        rx.await.unwrap_or(false)
    }

    /// The computer the host runs on, by the name the app reads it by (setup.ts:177).
    pub fn here(&self) -> String {
        let at = (self.ctx.wiring.here)();
        at.label.unwrap_or(at.name)
    }

    /// The one busy reading: the sentence a fork or a second setup there is refused with (setup.ts:185).
    pub fn setting_now(&self, record: &PlaceRecord) -> Option<String> {
        let busy = self.setting.borrow().contains(&record.id)
            || record
                .setup
                .as_ref()
                .is_some_and(|s| s.state == PlaceSetupState::Running);
        if !busy {
            return None;
        }
        let running = record.setup.as_ref().and_then(|s| {
            s.steps
                .iter()
                .find(|l| l.state == wsp_wire::views::place::PlaceSetupLineState::Running)
                .map(|l| l.step)
        });
        Some(place_provisioning_line(&record.name, running.as_ref()))
    }

    /// One write of a setup's state onto the record as it stands (setup.ts:193).
    pub async fn write_setup(&self, place_id: &str, patch: SetupPatch) -> PlaceResult<()> {
        self.records
            .change(
                place_id,
                Box::new(move |now: &PlaceRecord| Some(patch.apply(now))),
            )
            .await
            .map(|_| ())
    }

    /// `recipes.changed { slug }` on the stream (setup.ts:202).
    pub fn recipes_moved(&self, slug: &str) {
        if let Some(on_setup) = &self.ctx.opts.on_setup {
            on_setup(&SetupEvent::RecipesChanged(RecipesChangedEvent {
                slug: slug.to_string(),
            }));
        }
    }

    /// One move of a computer's sync on the stream (setup.ts:205).
    pub fn sync_frame(&self, event: PlaceSyncEvent) {
        if let Some(on_setup) = &self.ctx.opts.on_setup {
            on_setup(&SetupEvent::Sync(event));
        }
    }

    /// One frame of a setup on the stream (setup.ts:208).
    pub fn setup_frame(&self, event: PlaceSetupEvent) {
        if let Some(on_setup) = &self.ctx.opts.on_setup {
            on_setup(&SetupEvent::Setup(event));
        }
    }

    /// Starts the setup on one computer from its picks (setup.ts:927).
    pub async fn start_setup(
        self: &Rc<Self>,
        place_id: &str,
        add_id: &str,
        given: Option<GivenPicks>,
        row: Option<&str>,
    ) -> PlaceResult<Started> {
        if self.ctx.wiring.provision.is_none() {
            return Ok(Started::default());
        }
        let Some(record) = self.records.record_of(place_id).await? else {
            return Ok(Started::default());
        };
        let running = record.setup.as_ref().and_then(|s| {
            s.steps
                .iter()
                .find(|l| l.state == wsp_wire::views::place::PlaceSetupLineState::Running)
                .map(|l| l.step)
        });
        if self.setting.borrow().contains(place_id) {
            return Err(HostError::with_kind(
                place_provisioning_line(&record.name, running.as_ref()),
                Kind::Conflict,
            ));
        }
        if self.syncing.borrow().contains(place_id) {
            return Err(HostError::with_kind(
                place_syncing_line(&record.name),
                Kind::Conflict,
            ));
        }
        let Some(_home) = record.report.login.get("HOME").cloned() else {
            return Ok(Started {
                setup: None,
                said: Some(place_no_home_line(&record.name)),
            });
        };
        let Some(picks) = given
            .as_ref()
            .map(|g| g.picks.clone())
            .or_else(|| record.picks.clone())
        else {
            return Ok(Started {
                setup: None,
                said: Some(place_no_picks_line(&record.name)),
            });
        };
        let ended = match &record.setup {
            Some(setup) if row.is_some() && setup.state != PlaceSetupState::Running => {
                Some(setup.clone())
            }
            _ => None,
        };
        let only = match (&ended, row) {
            (Some(_), Some(row)) => match row_step(&record, row) {
                Some(step) => Some(OnlyRow {
                    step,
                    row: row.to_string(),
                }),
                None => {
                    return Err(usage_refusal(
                        &no_setup_row_refusal(row, &record.name),
                        "Run again a row the computer's page shows.",
                    ))
                }
            },
            _ => None,
        };
        let mut runs: IndexSet<PlaceSetupStep> = IndexSet::new();
        if let Some(only) = &only {
            runs.insert(only.step);
            runs.extend(row_follows(only.step).iter().copied());
            runs.insert(PlaceSetupStep::Context);
        }
        self.setting.borrow_mut().insert(place_id.to_string());
        let facts = StartFacts {
            record: &record,
            given: given.as_ref(),
            ended,
            only,
            runs: &runs,
        };
        let started = self.begin_setup(place_id, add_id, picks, facts).await;
        match started {
            Ok(setup) => Ok(Started {
                setup: Some(setup),
                said: None,
            }),
            Err(e) => {
                self.setting.borrow_mut().shift_remove(place_id);
                Err(e)
            }
        }
    }

    /// The part of a start after the computer is held: the setup written, then the run begun behind it.
    async fn begin_setup(
        self: &Rc<Self>,
        place_id: &str,
        add_id: &str,
        picks: RecipeFile,
        facts: StartFacts<'_>,
    ) -> PlaceResult<PlaceSetup> {
        let StartFacts {
            record,
            given,
            ended,
            only,
            runs,
        } = facts;
        let resuming = match &record.setup {
            Some(setup) if given.is_none() && setup.state == PlaceSetupState::Running => {
                Some(setup.clone())
            }
            _ => None,
        };
        let kept: Vec<wsp_wire::views::place::PlaceSetupLine> = match (&ended, &resuming) {
            (Some(ended), _) => ended
                .steps
                .iter()
                .filter(|l| !runs.contains(&l.step))
                .cloned()
                .collect(),
            (None, Some(resumed)) => resumed
                .steps
                .iter()
                .filter(|l| l.state == wsp_wire::views::place::PlaceSetupLineState::Done)
                .cloned()
                .collect(),
            (None, None) => Vec::new(),
        };
        let done: IndexSet<PlaceSetupStep> = kept.iter().map(|l| l.step).collect();
        let applied_rows: Vec<wsp_wire::views::place::PlaceProvisionRow> = record
            .applied
            .as_ref()
            .map(|a| a.rows.iter().map(|r| r.row.clone()).collect())
            .unwrap_or_default();
        let rerun: Vec<PlaceWait> = record
            .setup
            .as_ref()
            .map(|s| s.waiting.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|w| {
                if w.row == GITHUB_ROW {
                    return done.contains(&PlaceSetupStep::Github);
                }
                match wsp_wire::views::place::sign_in_of_row(&w.row) {
                    Some(agent) => {
                        done.contains(&PlaceSetupStep::Signins)
                            || !signs_in_again(only.as_ref(), &agent, &applied_rows)
                    }
                    None => false,
                }
            })
            .collect();
        let setup = PlaceSetup {
            state: PlaceSetupState::Running,
            add_id: add_id.to_string(),
            started_at: resuming
                .as_ref()
                .map(|r| r.started_at.clone())
                .unwrap_or_else(|| iso_at(self.ctx.clock.now() as u64)),
            finished_at: None,
            steps: kept,
            waiting: Vec::new(),
            said: None,
        };
        let recipe = given
            .as_ref()
            .map(|g| g.recipe.clone().unwrap_or_else(|| NO_RECIPE.to_string()));
        let patch = SetupPatch {
            setup: Some(setup.clone()),
            picks: Some(picks.clone()),
            recipe,
            ..Default::default()
        };
        self.write_setup(place_id, patch).await?;
        let ask = RunAsk {
            place_id: place_id.to_string(),
            add_id: add_id.to_string(),
            picks,
            done,
            started: setup.clone(),
            rerun,
            sync: None,
            only,
        };
        let area = Rc::clone(self);
        let place = place_id.to_string();
        spawn_local(async move {
            run_setup(Rc::clone(&area), ask).await;
            area.setting.borrow_mut().shift_remove(&place);
            let again_now = area.again.borrow_mut().shift_remove(&place);
            if again_now {
                area.sync_soon(&place, 0);
            }
        });
        Ok(setup)
    }

    /// What moved between what a computer last applied and the recipe it follows as this computer has it now, or
    /// nothing where it follows none, is mid-setup, or that recipe will not resolve. (setup.ts:974).
    pub async fn sync_of(&self, record: &PlaceRecord, given: Option<Resolved>) -> Option<SyncRead> {
        let slug = record.recipe.as_deref()?;
        let resolver = self.ctx.opts.recipes.as_ref().and_then(|f| f())?;
        if slug == NO_RECIPE
            || record.picks.is_none()
            || record.setup.as_ref()?.state != PlaceSetupState::Done
        {
            return None;
        }
        let resolved = match given {
            Some(given) => given,
            None => resolver.resolve(slug).await.ok()?,
        };
        if record
            .applied
            .as_ref()
            .is_some_and(|a| a.hash == resolved.hash)
        {
            return Some(SyncRead {
                resolved,
                changes: Vec::new(),
            });
        }
        let picks = record.picks.as_ref()?;
        let changes = recipe_changes(
            Some(picks),
            record.applied.as_ref().and_then(|a| a.items.as_ref()),
            &resolved.file,
            &resolved.items,
        );
        Some(SyncRead { resolved, changes })
    }

    /// A computer out of step reads Behind with what moved; one in step drops the word (setup.ts:985).
    pub async fn mark_sync(&self, place_id: &str, sync: Option<PlaceSync>) -> PlaceResult<()> {
        let Some(held) = self.records.record_of(place_id).await? else {
            return Ok(());
        };
        if held.sync.is_none() && sync.is_none() {
            return Ok(());
        }
        self.write_setup(
            place_id,
            SetupPatch {
                sync: Some(sync.clone()),
                ..Default::default()
            },
        )
        .await?;
        self.sync_frame(PlaceSyncEvent {
            place_id: place_id.to_string(),
            sync,
            line: None,
            applied: None,
        });
        Ok(())
    }

    /// One sync of a computer to the recipe it follows, one at a time per computer: a change that lands meanwhile runs
    /// it again once it ends. Only the difference goes: the rows the recipe took out come off, the rows it added or
    /// that changed here go on, and a sync with nothing moved runs nothing there. A computer that is not linked reads
    /// Behind and catches up when it dials back. (setup.ts:996).
    pub async fn sync_place(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        if self.setting.borrow().contains(place_id) || self.syncing.borrow().contains(place_id) {
            self.again.borrow_mut().insert(place_id.to_string());
            tokio::task::yield_now().await;
            return Ok(());
        }
        self.syncing.borrow_mut().insert(place_id.to_string());
        let result = self.sync_once(place_id).await;
        self.syncing.borrow_mut().shift_remove(place_id);
        let again_now = self.again.borrow_mut().shift_remove(place_id);
        if again_now {
            self.sync_soon(place_id, 0);
        }
        result
    }

    async fn sync_once(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        let Some(record) = self.records.record_of(place_id).await? else {
            return Ok(());
        };
        if self.ctx.wiring.provision.is_none() {
            return Ok(());
        }
        let Some(read) = self.sync_of(&record, None).await else {
            return Ok(());
        };
        let SyncRead { resolved, changes } = read;
        if changes.is_empty() {
            if record
                .applied
                .as_ref()
                .is_some_and(|a| a.hash != resolved.hash)
            {
                let (hash, items, file) = (
                    resolved.hash.clone(),
                    resolved.items.clone(),
                    resolved.file.clone(),
                );
                self.records
                    .change(
                        place_id,
                        Box::new(move |now: &PlaceRecord| {
                            now.applied.as_ref()?;
                            let mut next = now.clone();
                            if let Some(applied) = next.applied.as_mut() {
                                applied.hash = hash;
                                applied.items = Some(items);
                            }
                            next.picks = Some(file);
                            Some(next)
                        }),
                    )
                    .await?;
            }
            return self.mark_sync(place_id, None).await;
        }
        let state = PlaceSync {
            state: PlaceSyncState::Running,
            changes: changes.iter().map(|c| c.key.clone()).collect(),
            since: record
                .sync
                .as_ref()
                .map(|s| s.since.clone())
                .unwrap_or_else(|| iso_at(self.ctx.clock.now() as u64)),
        };
        let linked = self.ctx.live.borrow().contains_key(place_id);
        if !linked || record.report.login.get("HOME").is_none() {
            return self
                .mark_sync(
                    place_id,
                    Some(PlaceSync {
                        state: PlaceSyncState::Behind,
                        ..state
                    }),
                )
                .await;
        }
        self.mark_sync(place_id, Some(state.clone())).await?;
        let moved: IndexSet<String> = changes
            .iter()
            .filter(|c| c.how != RecipeChangeHow::Removed)
            .map(|c| c.key.clone())
            .collect();
        let (Some(started), Some(before)) = (record.setup.clone(), record.picks.clone()) else {
            return Ok(());
        };
        let steps = steps_for(&changes, &resolved.file);
        let job = SyncJob {
            file: resolved.file.clone(),
            items: resolved.items.clone(),
            hash: resolved.hash.clone(),
            changes,
            steps,
            moved,
            before,
            state,
        };
        let ask = RunAsk {
            place_id: place_id.to_string(),
            add_id: started.add_id.clone(),
            picks: resolved.file.clone(),
            done: IndexSet::new(),
            started,
            rerun: Vec::new(),
            sync: Some(job),
            only: None,
        };
        run_setup(Rc::clone(self), ask).await;
        Ok(())
    }

    /// A sync of one computer after `after_ms`, the clock starting again at every ask (setup.ts:1030).
    pub fn sync_soon(self: &Rc<Self>, place_id: &str, after_ms: u64) {
        let pending = self.sync_timers.borrow_mut().shift_remove(place_id);
        if let Some(cancel) = pending {
            cancel();
        }
        let area = Rc::clone(self);
        let place = place_id.to_string();
        let cancel = self.ctx.clock.schedule(
            Box::new(move || {
                area.sync_timers.borrow_mut().shift_remove(&place);
                let (fire, name_of) = (Rc::clone(&area), place.clone());
                spawn_local(async move {
                    if let Err(e) = fire.sync_place(&name_of).await {
                        let name = fire
                            .ctx
                            .kept
                            .borrow()
                            .get(&name_of)
                            .map(|r| r.name.clone())
                            .unwrap_or_else(|| name_of.clone());
                        warn(&format!(
                            "the sync of {name} stopped: {}",
                            first_line_of(&e)
                        ));
                    }
                });
            }),
            after_ms as i64,
        );
        self.sync_timers
            .borrow_mut()
            .insert(place_id.to_string(), Box::new(move || cancel()));
    }

    /// The start with its refusal as a sentence (setup.ts:1042).
    pub async fn started_or_said(
        self: &Rc<Self>,
        place_id: &str,
        add_id: &str,
        given: Option<GivenPicks>,
    ) -> Started {
        match self.start_setup(place_id, add_id, given, None).await {
            Ok(started) => started,
            Err(e) => Started {
                setup: None,
                said: Some(first_line_of(&e)),
            },
        }
    }

    /// The road the engine drives one place's machines over (setup.ts:1053). The engine's `MachineLink` is not `Send`
    /// (`#[async_trait(?Send)]`) and `LinkBackend` takes it as an `Arc`, so the link holds its state plainly.
    pub fn link_to(self: &Rc<Self>, place_id: &str) -> Arc<dyn MachineLink> {
        Arc::new(PlaceLink {
            area: Rc::clone(self),
            place: place_id.to_string(),
        })
    }
}

/// The road one place's machines are driven over: frames to its live link, waited out across a gap where a frame
/// carries a key (setup.ts:1053).
struct PlaceLink {
    area: Rc<PlaceSetupArea>,
    place: String,
}

impl PlaceLink {
    fn name(&self) -> String {
        self.area
            .ctx
            .kept
            .borrow()
            .get(&self.place)
            .map(|r| r.name.clone())
            .unwrap_or_else(|| self.place.clone())
    }

    fn now(&self) -> u64 {
        self.area.ctx.clock.now() as u64
    }
}

#[async_trait(?Send)]
impl MachineLink for PlaceLink {
    async fn request(
        &self,
        op: &str,
        params: Map<String, Value>,
        ask: LinkAsk,
    ) -> EngineResult<Map<String, Value>> {
        let area = Rc::clone(&self.area);
        let place = self.place.as_str();
        let keyed = ask.idempotency_key.is_some();
        let relink = area.ctx.relink_wait_ms;
        let mut until: Option<u64> = None;
        loop {
            let held = area
                .ctx
                .live
                .borrow()
                .get(place)
                .map(|l| (Rc::clone(&l.reach), Rc::clone(&l.socket)));
            let Some((reach, _socket)) = held else {
                let absent = EngineError::PlaceAbsent {
                    message: absent_computer(&self.name(), None).sentence,
                };
                let closed = area.closed_at.borrow().get(place).copied();
                let Some(closed) = closed.filter(|_| keyed) else {
                    return Err(absent);
                };
                let wait_until = *until.get_or_insert(closed + relink);
                if !area.dials_back(place, wait_until).await {
                    return Err(absent);
                }
                continue;
            };
            let sent_at = self.now();
            let bound = ask.timeout_ms.unwrap_or(area.ctx.frame_wait_ms);
            let what = format!("{op} on {}", self.name());
            let frame = {
                let reach = Rc::clone(&reach);
                let params = params.clone();
                let op = op.to_string();
                async move { reach.request(&op, params).await.map_err(refused_engine) }
            };
            match bounded_engine(&*area.ctx.clock, frame, bound, &what).await {
                Ok(answer) => return Ok(answer),
                Err(e) => {
                    let now_held = area
                        .ctx
                        .live
                        .borrow()
                        .get(place)
                        .map(|l| (Rc::clone(&l.reach), l.socket.is_open()));
                    if !keyed
                        || matches!(&now_held, Some((r, open)) if Rc::ptr_eq(r, &reach) && *open)
                    {
                        return Err(e);
                    }
                    if let Some((r, _)) = &now_held {
                        if !Rc::ptr_eq(r, &reach) {
                            let t = *until.get_or_insert(self.now() + relink);
                            if self.now() < t {
                                continue;
                            }
                        }
                    }
                    let closed = area.closed_at.borrow().get(place).copied();
                    let gap = closed
                        .filter(|at| *at >= sent_at)
                        .unwrap_or_else(|| self.now());
                    let t = *until.get_or_insert(gap + relink);
                    if !area.dials_back(place, t).await {
                        return Err(e);
                    }
                }
            }
        }
    }

    fn has_dials_back(&self) -> bool {
        true
    }

    async fn dials_back(&self) -> bool {
        let until = self.now() + self.area.ctx.relink_wait_ms;
        self.area.dials_back(&self.place, until).await
    }

    async fn forward(&self, place_port: u16) -> EngineResult<u16> {
        let ask = ForwardAsk {
            pane: None,
            standing: false,
        };
        self.area
            .ctx
            .door()
            .forward(&self.place, place_port, ask)
            .await
            .map_err(|e| EngineError::Other {
                message: e.message,
                code: e.code,
            })
    }
}

/// What a start reads off the record it is begun on.
struct StartFacts<'a> {
    record: &'a PlaceRecord,
    given: Option<&'a GivenPicks>,
    ended: Option<PlaceSetup>,
    only: Option<OnlyRow>,
    runs: &'a IndexSet<PlaceSetupStep>,
}

/// What a sign-in's outcome waits on: a `Result` the run turns into a failed row.
pub type SetupResult<T> = Result<T, HostError>;

/// The row a run lands with no step of its own yet, as the run holds rows.
pub(crate) fn held_of(row: wsp_wire::views::place::PlaceProvisionRow) -> HeldRow {
    HeldRow {
        row,
        pick: None,
        created_at: None,
    }
}
