// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider as the dialog walks it (`cloudFlow.ts`, `useRecipeJob.ts`): the provider's key checked live,
//! the image's picks off the host's own setup screens, the summary with the sign-ins on the computer the host runs on,
//! then the image building and the ready page. Every host answer lands here through a method named for it, so a test
//! moves the flow with fixture answers; the view's thin layer sends the calls and hands their answers back.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use wsp_host_client::Failure;

use super::sign_in::{self, SignIns};
use super::words::{self, FIRST_CLOUD, FIRST_LAUNCH_SCREEN, KEY_REFUSED, SIGNING_IN, SIGN_IN_STAGE_ID};
use crate::ui::catalog;

/// The host's screens the dialog draws as pick steps, in its order; nothing signs in on a cloud machine and its build
/// writes nothing on this computer, so the sign-ins and wsp tools screens are not among them.
pub const PICKS: [&str; 3] = ["agents", "tools", "also"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Provider,
    Key,
    /// One of [`PICKS`], by its index.
    Pick(usize),
    Summary,
    Building,
    Ready,
}

impl Step {
    pub fn pick(id: &str) -> Option<Step> {
        PICKS.iter().position(|p| *p == id).map(Step::Pick)
    }

    pub fn pick_id(self) -> Option<&'static str> {
        match self {
            Step::Pick(at) => PICKS.get(at).copied(),
            _ => None,
        }
    }
}

/// What a key that works read off the provider.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct Facts {
    pub account: String,
    pub plan: String,
    pub running: u64,
    pub limit: u64,
}

/// A refusal as the dialog shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub said: String,
    pub kind: Option<String>,
}

impl From<Failure> for Refused {
    fn from(f: Failure) -> Self {
        Refused { said: f.message, kind: f.kind }
    }
}

/// The live check of the key in the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Check {
    Idle,
    Checking,
    Works(Facts),
    /// The provider answered and refused the key.
    Refused(Refused),
    /// Nothing came back about the key at all: worth pressing again.
    Unreachable(Refused),
}

/// The words a key that works reads as: the account, the plan and the machines running of the most it may, each
/// left out where the provider did not say it.
pub fn key_works_line(facts: &Facts) -> String {
    let parts: Vec<String> = [
        (!facts.account.is_empty()).then(|| format!("Account {}", facts.account)),
        (!facts.plan.is_empty()).then(|| facts.plan.clone()),
        (facts.limit != 0).then(|| format!("{} of {} running at once", facts.running, facts.limit)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if parts.is_empty() {
        "Works.".into()
    } else {
        format!("Works. {}.", parts.join(", "))
    }
}

/// `InitSetup`, as much of it as the dialog reads.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Setup {
    #[serde(default)]
    pub keys: BTreeMap<String, bool>,
    #[serde(default)]
    pub agents: Vec<SetupAgent>,
    #[serde(default)]
    pub job: Option<Job>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct SetupAgent {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub configured: bool,
}

/// `InitJob`, as much of it as the dialog reads.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub phase: String,
    #[serde(default)]
    pub step: usize,
    #[serde(default)]
    pub screens: Vec<Screen>,
    #[serde(default)]
    pub rows: Vec<Row>,
    #[serde(default)]
    pub drafts: Vec<Draft>,
    #[serde(default)]
    pub disk: Option<Disk>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub golden: Option<Golden>,
    #[serde(default)]
    pub place: Option<JobPlace>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Screen {
    pub id: String,
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub ticks: Vec<String>,
    #[serde(default)]
    pub answers: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Item {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub detail: Vec<String>,
    #[serde(default)]
    pub lock: Option<String>,
    #[serde(default)]
    pub choices: Option<serde_json::Value>,
    #[serde(default)]
    pub mark: Option<String>,
}

impl Item {
    pub fn locked_on(&self) -> bool {
        self.lock.as_deref() == Some("on")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Row {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub state: String,
    #[serde(default)]
    pub login: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub ms: Option<f64>,
    #[serde(default)]
    pub since: Option<f64>,
    #[serde(default)]
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Draft {
    pub at: String,
    #[serde(default)]
    pub ticks: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Disk {
    pub fixed: f64,
    pub total: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct Golden {
    pub version: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct JobPlace {
    pub id: String,
}

/// `initJobOver`.
pub fn job_over(phase: &str) -> bool {
    matches!(phase, "done" | "failed" | "cancelled")
}

/// `initJobBuilding`.
pub fn job_building(phase: &str) -> bool {
    matches!(phase, "building" | "signing-in" | "sealing" | "finishing")
}

/// The setup a job is, for this provider: the job a card for it started, and no other.
pub fn job_for<'a>(job: Option<&'a Job>, provider: &str) -> Option<&'a Job> {
    job.filter(|j| j.place.as_ref().is_some_and(|p| p.id == provider))
}

/// The pick screens a job shows, in the dialog's order: a screen with nothing to pick is not shown.
pub fn pick_screens(job: &Job) -> Vec<&Screen> {
    job.screens.iter().filter(|s| !s.items.is_empty() && PICKS.contains(&s.id.as_str())).collect()
}

/// The steps the dialog counts: the provider where more than one is offered, the key, the pick screens the job shows
/// (all three until it has read), the summary.
pub fn cloud_steps(job: Option<&Job>, offered: usize) -> Vec<Step> {
    let shown: Vec<Step> = match job {
        Some(job) if job.phase == "answering" => pick_screens(job).iter().filter_map(|s| Step::pick(&s.id)).collect(),
        _ => (0..PICKS.len()).map(Step::Pick).collect(),
    };
    let mut steps = Vec::new();
    if offered > 1 {
        steps.push(Step::Provider);
    }
    steps.push(Step::Key);
    steps.extend(shown);
    steps.push(Step::Summary);
    steps
}

/// Where a provider's setup stands, as the step the dialog opens on: its building view while the image builds or once
/// a build failed, the step the host stands at while the picks are being made, and the key with no setup at all.
pub fn cloud_step_of(job: Option<&Job>, provider: &str) -> Step {
    let Some(mine) = job_for(job, provider) else { return Step::Key };
    if job_building(&mine.phase) || mine.phase == "failed" {
        return Step::Building;
    }
    if job_over(&mine.phase) {
        return Step::Key;
    }
    let at = mine.screens.get(mine.step).map(|s| s.id.as_str());
    match at {
        Some(at) if mine.phase == "answering" => Step::pick(at).unwrap_or(Step::Summary),
        _ if mine.phase == "answering" => Step::Summary,
        _ => Step::Pick(0),
    }
}

/// Whether a setup for this provider is under way: picks being made, an image building, or a build that failed and
/// waits on the person.
pub fn setting_up(job: Option<&Job>, provider: &str) -> bool {
    job_for(job, provider).is_some_and(|j| !job_over(&j.phase) || j.phase == "failed")
}

/// The clouds this host could be given a key for: every cloud its setup keys a row by, none before it has said.
pub fn clouds_offered(setup: Option<&Setup>) -> Vec<&'static str> {
    words::clouds().into_iter().map(|c| c.id).filter(|id| setup.is_some_and(|s| s.keys.contains_key(*id))).collect()
}

/// A build row's state as a step row reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Waiting,
    Working,
    NeedsYou,
    Done,
    Failed,
}

/// One row of the build as the card draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildRow {
    pub id: String,
    pub name: String,
    pub state: RowState,
    pub ms: Option<f64>,
    pub since: Option<f64>,
    pub said: Option<String>,
    pub lines: Vec<String>,
}

const ROW_OVER: [&str; 10] = ["done", "failed", "stopped", "forked", "imported", "key set", "MCP added", "skipped", "not made", "gone"];
const ROW_UNRUN: [&str; 3] = ["skipped", "not made", "stopped"];
const OPEN: &str = "waiting for you";

fn row_failed(state: &str, login: Option<&str>) -> bool {
    state == "failed" || login == Some("not-signed-in")
}

fn row_over(state: &str, login: Option<&str>) -> bool {
    login.is_some() || ROW_OVER.contains(&state)
}

/// What the stage the sign-ins fold into stands at, from the sign-ins themselves.
fn sign_in_stage(sign_ins: &[&Row]) -> (String, Option<String>) {
    if sign_ins.iter().any(|r| r.state == OPEN) {
        return (OPEN.into(), None);
    }
    if sign_ins.iter().any(|r| r.state == "running") {
        return ("running".into(), None);
    }
    if !sign_ins.iter().all(|r| row_over(&r.state, r.login.as_deref())) {
        let all_waiting = sign_ins.iter().all(|r| r.state == "waiting");
        return ((if all_waiting { "waiting" } else { "running" }).into(), None);
    }
    if let Some(out) = sign_ins.iter().find(|r| row_failed(&r.state, r.login.as_deref())) {
        return (out.state.clone(), out.login.clone());
    }
    if sign_ins.iter().all(|r| r.state == "skipped") {
        return ("skipped".into(), None);
    }
    ("done".into(), None)
}

/// `initBuildRows`: stages only, in the job's order, the sign-ins folded into one stage row where the first of them
/// sits; agent rows leave. Each as (row, its state's word, its login).
fn stage_rows(rows: &[Row]) -> Vec<Row> {
    let sign_ins: Vec<&Row> = rows.iter().filter(|r| r.kind == "sign-in").collect();
    let mut out = Vec::new();
    let mut folded = false;
    for row in rows {
        match row.kind.as_str() {
            "agent" => {}
            "sign-in" if folded => {}
            "sign-in" => {
                folded = true;
                let (state, login) = sign_in_stage(&sign_ins);
                out.push(Row {
                    id: SIGN_IN_STAGE_ID.into(),
                    kind: "stage".into(),
                    label: SIGNING_IN.into(),
                    state,
                    login,
                    ..Row::default()
                });
            }
            _ => out.push(row.clone()),
        }
    }
    out
}

fn row_state(row: &Row) -> RowState {
    let login = row.login.as_deref();
    if row_failed(&row.state, login) {
        RowState::Failed
    } else if row.state == "running" || row.state == "waiting for a machine slot" {
        RowState::Working
    } else if row.state == OPEN {
        RowState::NeedsYou
    } else if ROW_UNRUN.contains(&row.state.as_str()) {
        RowState::Waiting
    } else if row_over(&row.state, login) {
        RowState::Done
    } else {
        RowState::Waiting
    }
}

/// The image's build as a box's setup draws it: one step row per stage, a failed one with its reason.
pub fn build_rows(job: &Job) -> Vec<BuildRow> {
    stage_rows(&job.rows)
        .into_iter()
        .map(|row| {
            let state = row_state(&row);
            let lines = job.rows.iter().find(|r| r.id == row.id).map(|r| r.lines.clone()).unwrap_or_default();
            BuildRow {
                id: row.id.clone(),
                name: row.label.clone(),
                state,
                ms: row.ms,
                since: if state == RowState::Working { row.since } else { None },
                said: if state == RowState::Failed { row.detail.clone() } else { None },
                lines,
            }
        })
        .collect()
}

/// `initStageCount`: how many of the build's stages ended well, of all of them.
pub fn stage_count(job: &Job) -> (usize, usize) {
    let stages: Vec<Row> = stage_rows(&job.rows).into_iter().filter(|r| r.kind == "stage").collect();
    let done = stages
        .iter()
        .filter(|r| {
            row_over(&r.state, r.login.as_deref()) && !row_failed(&r.state, r.login.as_deref()) && !ROW_UNRUN.contains(&r.state.as_str())
        })
        .count();
    (done, stages.len())
}

/// The ticks of a screen as the host holds them: the draft it kept, else what it last answered.
fn kept_ticks(job: &Job, screen: &Screen) -> BTreeSet<String> {
    job.drafts
        .iter()
        .find(|d| d.at == screen.id)
        .map_or_else(|| screen.ticks.iter().cloned().collect(), |d| d.ticks.iter().cloned().collect())
}

/// `initImageBytes`: what the disk holds before any tick plus every screen's ticked rows, drafts over answers.
fn image_bytes(job: &Job, current: Option<(&str, &BTreeSet<String>)>) -> f64 {
    let fixed = job.disk.as_ref().map_or(0., |d| d.fixed);
    job.screens.iter().fold(fixed, |sum, s| {
        let ticks = match current {
            Some((at, ticks)) if at == s.id => ticks.clone(),
            _ => kept_ticks(job, s),
        };
        sum + s.items.iter().filter(|i| i.choices.is_none() && (i.locked_on() || ticks.contains(&i.id))).filter_map(|i| i.size).sum::<f64>()
    })
}

/// A call the flow asks the view's thin layer to send.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// `providers.checkKey`.
    Check { provider: String, key: String },
    /// `init.draft`: a screen's ticks kept on the host as they change.
    Draft { at: String, ticks: Vec<String> },
    /// `init.step`: the host's step moved to a screen the dialog shows, by its index among the job's screens.
    StepTo { at: usize },
    /// `init.answer` for a pick screen, then the step after it.
    Answer { screen: String, ticks: Vec<String>, answers: BTreeMap<String, String>, then: Step },
    /// `init.keys` where a key was typed, then `init.start` unless a setup stands, then the step after the key.
    KeyOn { provider: String, key: Option<String>, start: bool, then: Step },
    /// The first launch's screen answered where the job has one, then `init.build`.
    Build { first_launch: Option<Vec<String>>, on: String },
    /// `init.start` again after a failed build, then the step after the key.
    StartOver { on: String, then: Step },
}

/// One sign-in on the computer the host runs on, as the summary lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct SignInLine {
    pub id: String,
    pub name: String,
    pub ready: bool,
    pub word: String,
    /// How its Sign in goes, for one not ready that has one.
    pub start: Option<sign_in::Start>,
}

/// One agent on this computer as the agents report reads it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sign_in: String,
    #[serde(default)]
    pub sign_in_road: String,
    #[serde(default)]
    pub sign_in_kind: Option<String>,
}

impl Agent {
    /// Signed in here, by the agent's own login or a key in the vault.
    pub fn ready(&self) -> bool {
        self.sign_in == "signed-in" || self.sign_in == "vault-key"
    }
}

/// The dialog's whole state, for one open.
#[derive(Clone, Debug)]
pub struct Flow {
    pub provider: String,
    pub step: Step,
    pub check: Check,
    /// What the key field holds.
    pub key: String,
    pub setup: Option<Setup>,
    pub setup_refused: Option<String>,
    /// The host's one setup job as it last said it, whoever it is for.
    pub job: Option<Job>,
    /// The agents on the computer the host runs on; none until read.
    pub report: Option<Vec<Agent>>,
    /// The GitHub login the setup read here.
    pub github: Option<String>,
    /// The computer the host runs on, by its name.
    pub here: String,
    /// The key step's refusal.
    pub refused: Option<Refused>,
    /// Continue from the key is with the host.
    pub busy: bool,
    /// A pick, the build or Start over is with the host.
    pub recipe_busy: bool,
    pub recipe_refusal: Option<String>,
    /// The unsent ticks of one screen, by its id.
    pub draft: Option<(String, BTreeSet<String>)>,
    pub sign_ins: SignIns,
    /// The step the flow opened on is decided once the setup is read.
    placed: bool,
}

impl Flow {
    /// A flow on `provider`'s key, waiting for the setup; none named, it opens where the setup says.
    pub fn new(provider: Option<&str>) -> Flow {
        Flow {
            provider: provider.unwrap_or(FIRST_CLOUD).to_owned(),
            step: Step::Key,
            check: Check::Idle,
            key: String::new(),
            setup: None,
            setup_refused: None,
            job: None,
            report: None,
            github: None,
            here: String::new(),
            refused: None,
            busy: false,
            recipe_busy: false,
            recipe_refusal: None,
            draft: None,
            sign_ins: SignIns::default(),
            placed: provider.is_some(),
        }
    }

    pub fn cloud(&self) -> String {
        words::cloud_name(&self.provider)
    }

    /// This provider's job, and no other.
    pub fn mine(&self) -> Option<&Job> {
        job_for(self.job.as_ref(), &self.provider)
    }

    pub fn offered(&self) -> Vec<&'static str> {
        clouds_offered(self.setup.as_ref())
    }

    pub fn steps(&self) -> Vec<Step> {
        cloud_steps(self.mine(), self.offered().len())
    }

    /// Where the step stands among the counted steps; none on the build and the ready page.
    pub fn at(&self) -> Option<usize> {
        self.steps().iter().position(|s| *s == self.step)
    }

    fn after_key(&self) -> Step {
        let steps = self.steps();
        steps.iter().position(|s| *s == Step::Key).and_then(|at| steps.get(at + 1).copied()).unwrap_or(Step::Summary)
    }

    /// The host holds a key for this provider.
    pub fn held(&self) -> bool {
        self.setup.as_ref().is_some_and(|s| s.keys.get(&self.provider) == Some(&true))
    }

    /// The pick screen the step shows, once the job has it.
    pub fn screen(&self) -> Option<&Screen> {
        let id = self.step.pick_id()?;
        self.mine()?.screens.iter().find(|s| s.id == id)
    }

    /// The job is still reading this computer, so the picks wait on it.
    pub fn reading(&self) -> bool {
        self.mine().is_none_or(|j| j.phase == "reading" || j.phase == "agent")
    }

    /// The setup read (`init.get`). The first one places a flow named by nobody: on the setup that stands for any cloud
    /// the host offers, else on the provider step where it offers more than one, else at the one cloud's key.
    pub fn on_setup(&mut self, setup: Setup) {
        if let Some(job) = setup.job.clone() {
            self.on_job(job);
        }
        self.setup = Some(setup);
        self.setup_refused = None;
        if !self.placed {
            self.placed = true;
            let offered = self.offered();
            let standing = offered.iter().find(|p| setting_up(self.job.as_ref(), p)).copied();
            self.provider = standing.or(offered.first().copied()).unwrap_or(FIRST_CLOUD).to_owned();
            self.step =
                if standing.is_none() && offered.len() > 1 { Step::Provider } else { cloud_step_of(self.job.as_ref(), &self.provider) };
        }
    }

    pub fn on_setup_refused(&mut self, failure: Failure) {
        self.setup_refused = Some(failure.message);
    }

    /// A job the host said, read or pushed (`init.job`). A new job starts from what the host holds for it; a build
    /// started from another window moves a flow left on the summary on to the build.
    pub fn on_job(&mut self, job: Job) {
        if self.job.as_ref().map(|j| &j.id) != Some(&job.id) {
            self.draft = None;
        }
        self.job = Some(job);
        if self.mine().is_some_and(|j| job_building(&j.phase)) && self.step == Step::Summary {
            self.step = Step::Building;
        }
    }

    pub fn pick_provider(&mut self, id: &str) {
        self.provider = id.to_owned();
        self.check = Check::Idle;
    }

    pub fn go(&mut self, step: Step) {
        self.step = step;
    }

    /// The field changed: checked at once on a paste or Enter, else back to nothing until then.
    pub fn type_key(&mut self, text: &str, now: bool) -> Option<Ask> {
        self.key = text.to_owned();
        self.refused = None;
        if !now {
            self.check = Check::Idle;
            return None;
        }
        let typed = text.trim();
        if typed.is_empty() {
            self.check = Check::Idle;
            return None;
        }
        self.check = Check::Checking;
        Some(Ask::Check { provider: self.provider.clone(), key: typed.to_owned() })
    }

    /// The provider's answer for `typed`; dropped where the field has changed since, so a slow check never stands over
    /// a newer one.
    pub fn on_check(&mut self, typed: &str, answer: Result<Facts, Failure>) {
        if self.key.trim() != typed {
            return;
        }
        self.check = match answer {
            Ok(facts) => Check::Works(facts),
            Err(f) if f.kind.as_deref() == Some(KEY_REFUSED) => Check::Refused(f.into()),
            Err(f) => Check::Unreachable(f.into()),
        };
    }

    /// Continue from the key may be pressed: a key the provider took, or none typed and one held.
    pub fn key_ready(&self) -> bool {
        if self.key.trim().is_empty() {
            self.held()
        } else {
            matches!(self.check, Check::Works(_))
        }
    }

    /// Continue from the key: the key saved where one was typed, then the setup started for this cloud unless one
    /// stands.
    pub fn key_on(&mut self) -> Option<Ask> {
        if !self.key_ready() || self.busy {
            return None;
        }
        self.busy = true;
        self.refused = None;
        let key = Some(self.key.trim().to_owned()).filter(|k| !k.is_empty());
        let start = self.mine().is_none_or(|j| !job_building(&j.phase) && j.phase != "answering" && j.phase != "reading");
        Some(Ask::KeyOn { provider: self.provider.clone(), key, start, then: self.after_key() })
    }

    /// The key was saved: the setup it answered with.
    pub fn on_key_saved(&mut self, setup: Setup) {
        self.on_setup(setup);
        self.key.clear();
    }

    /// Continue from the key ended: on to `then`, or the refusal under the field.
    pub fn on_key_on(&mut self, then: Step, answer: Result<(), Failure>) {
        self.busy = false;
        match answer {
            Ok(()) => {
                self.key.clear();
                self.check = Check::Idle;
                self.step = then;
            }
            Err(f) => self.refused = Some(f.into()),
        }
    }

    /// A screen as the person has it: the unsent draft, else what the host kept of it, else what stands.
    pub fn ticks_at(&self, screen: &Screen) -> BTreeSet<String> {
        match &self.draft {
            Some((at, ticks)) if *at == screen.id => ticks.clone(),
            _ => self.mine().map_or_else(|| screen.ticks.iter().cloned().collect(), |job| kept_ticks(job, screen)),
        }
    }

    /// Ticks changed on the step's screen; a locked row keeps its tick. The draft goes to the host as it changes.
    pub fn set_ticks(&mut self, changes: &[(String, bool)]) -> Option<Ask> {
        let screen = self.screen()?.clone();
        let mut ticks = self.ticks_at(&screen);
        for (id, on) in changes {
            if screen.items.iter().any(|i| i.id == *id && i.lock.is_some()) {
                continue;
            }
            if *on {
                ticks.insert(id.clone());
            } else {
                ticks.remove(id);
            }
        }
        if ticks == self.ticks_at(&screen) {
            return None;
        }
        self.draft = Some((screen.id.clone(), ticks.clone()));
        Some(Ask::Draft { at: screen.id, ticks: ticks.into_iter().collect() })
    }

    /// Back: the step before, and the host's step with it where that is a screen it shows; held while a pick, the build or
    /// Start over is with the host.
    pub fn back(&mut self) -> Option<Ask> {
        // An answer still with the host would move the step on after this one moved it back.
        if self.recipe_busy {
            return None;
        }
        let steps = self.steps();
        let at = self.at()?;
        let to = *steps.get(at.checked_sub(1)?)?;
        self.step = to;
        let id = to.pick_id()?;
        let at = self.mine()?.screens.iter().position(|s| s.id == id)?;
        Some(Ask::StepTo { at })
    }

    /// One press at a time: a second press while the first is with the host sends nothing.
    fn attempt(&mut self) -> bool {
        if self.recipe_busy {
            return false;
        }
        self.recipe_busy = true;
        self.recipe_refusal = None;
        true
    }

    /// Continue from a pick: refused over the disk, else the answer to the host and then the next step.
    pub fn go_on(&mut self) -> Option<Ask> {
        let steps = self.steps();
        let to = self.at().and_then(|at| steps.get(at + 1).copied()).unwrap_or(Step::Summary);
        let (Some(screen), Some(job)) = (self.screen().cloned(), self.mine()) else {
            self.step = to;
            return None;
        };
        let ticks = self.ticks_at(&screen);
        if let Some(disk) = &job.disk {
            let over = image_bytes(job, Some((&screen.id, &ticks))) - disk.total;
            if over > 0. {
                self.recipe_refusal = Some(format!("over by {}", words::fmt_bytes(over as u64)));
                return None;
            }
        }
        self.attempt().then(|| Ask::Answer {
            screen: screen.id.clone(),
            ticks: ticks.into_iter().collect(),
            answers: screen.answers.clone(),
            then: to,
        })
    }

    /// The image build: the first launch's screen answered from the agents configured here where the job has one.
    pub fn build(&mut self) -> Option<Ask> {
        let setup = self.setup.as_ref()?;
        let first = self.mine()?.screens.iter().find(|s| s.id == FIRST_LAUNCH_SCREEN).map(|screen| {
            setup
                .agents
                .iter()
                .filter(|a| a.configured)
                .map(|a| format!("wsp-tools/{}", a.id))
                .filter(|id| screen.items.iter().any(|i| i.id == *id))
                .collect::<Vec<_>>()
        });
        let on = self.provider.clone();
        self.attempt().then_some(Ask::Build { first_launch: first, on })
    }

    pub fn start_over(&mut self) -> Option<Ask> {
        let (on, then) = (self.provider.clone(), self.after_key());
        self.attempt().then_some(Ask::StartOver { on, then })
    }

    /// A pick, the build or Start over came back: on to `then`, or the refusal.
    pub fn on_recipe(&mut self, then: Option<Step>, answer: Result<(), Failure>) {
        self.recipe_busy = false;
        match answer {
            Ok(()) => {
                if matches!(self.step, Step::Pick(_)) {
                    self.draft = None;
                }
                if let Some(then) = then {
                    self.step = then;
                }
            }
            Err(f) => self.recipe_refusal = Some(f.message),
        }
    }

    /// The agents ticked for the image, locked ones among them.
    pub fn ticked(&self) -> Vec<(String, String)> {
        let Some(screen) = self.mine().and_then(|j| j.screens.iter().find(|s| s.id == "agents")) else { return Vec::new() };
        screen.items.iter().filter(|i| i.locked_on() || screen.ticks.contains(&i.id)).map(|i| (i.id.clone(), i.label.clone())).collect()
    }

    /// The sign-ins cloud machines use through wsp: one per ticked agent that signs in, and GitHub.
    pub fn sign_in_lines(&self) -> Vec<SignInLine> {
        let here = &self.here;
        let mut lines: Vec<SignInLine> = self
            .ticked()
            .into_iter()
            .filter_map(|(id, name)| {
                let Some(report) = &self.report else {
                    return Some(SignInLine { id, name, ready: false, word: format!("Reading the sign-ins on {here}"), start: None });
                };
                let row = report.iter().find(|a| a.id == id);
                let road = row.map(|r| r.sign_in_road.clone()).or_else(|| catalog::sign_in(&id).map(|s| s.road.to_owned()))?;
                if road == "none" {
                    return None;
                }
                if let Some(row) = row.filter(|r| r.ready()) {
                    return Some(SignInLine { id, name, ready: true, word: signed_in_word(row, here), start: None });
                }
                let start = sign_in::start_of(&id, &road);
                Some(SignInLine { id, name, ready: false, word: format!("Not signed in on {here}"), start })
            })
            .collect();
        lines.push(SignInLine {
            id: "github".into(),
            name: "GitHub".into(),
            ready: self.github.is_some(),
            word: self.github.as_ref().map_or_else(|| format!("Run it in a terminal on {here}"), |a| format!("Signed in as {a}")),
            start: None,
        });
        lines
    }

    /// The summary's lines: the agents by name, every other screen as "n of m".
    pub fn summary_lines(&self) -> Vec<(String, String)> {
        let Some(job) = self.mine() else { return Vec::new() };
        let cloud = self.cloud();
        pick_screens(job)
            .into_iter()
            .map(|screen| {
                let on: Vec<&Item> = screen.items.iter().filter(|i| i.locked_on() || screen.ticks.contains(&i.id)).collect();
                let label = words::pick_words(&screen.id, &cloud, &self.here).0;
                let value = if screen.id == "agents" {
                    on.iter().map(|i| i.label.as_str()).collect::<Vec<_>>().join(", ")
                } else {
                    format!("{} of {}", on.len(), screen.items.len())
                };
                (label, if value.is_empty() { "None".into() } else { value })
            })
            .collect()
    }

    /// Build image is held: no setup read, a press with the host, or a sign-in not ready.
    pub fn build_held(&self) -> bool {
        self.setup.is_none() || self.recipe_busy || self.sign_in_lines().iter().any(|l| !l.ready)
    }

    pub fn sealed(&self) -> bool {
        self.mine().is_some_and(|j| j.phase == "done")
    }

    pub fn failed(&self) -> bool {
        self.mine().is_some_and(|j| j.phase == "failed")
    }

    /// The ready page's line: "Image vN sealed in T." where the seal said its version.
    pub fn ready_line(&self) -> Option<String> {
        let job = self.mine()?;
        let version = job.golden.as_ref()?.version;
        let took: f64 = stage_rows(&job.rows).iter().filter_map(|r| r.ms).sum();
        Some(if took > 0. {
            format!("Image v{version} sealed in {}.", crate::screens::dialogs::steps::fmt_duration(took))
        } else {
            format!("Image v{version} sealed.")
        })
    }

    /// The head: the step's title and its one line.
    pub fn head(&self) -> (String, Option<String>) {
        let cloud = self.cloud();
        match self.step {
            Step::Provider => ("Add a cloud provider".into(), Some("Where your sub-threads run.".into())),
            Step::Key => (format!("Add {cloud}"), Some(format!("wsp runs your sub-threads on your {cloud} account with this key."))),
            Step::Pick(_) => {
                let (title, line) = words::pick_words(self.step.pick_id().unwrap_or_default(), &cloud, &self.here);
                (title, Some(line))
            }
            Step::Summary => ("Summary".into(), Some(format!("What goes on {cloud}'s image, before it builds."))),
            Step::Building | Step::Ready if self.failed() => (format!("Setup on {cloud} failed"), None),
            Step::Building | Step::Ready if self.sealed() => (format!("{cloud} is ready"), None),
            Step::Building | Step::Ready => (format!("Setting up {cloud}"), None),
        }
    }

    /// The count at the head's right: the step among the steps, or the stages done on the build.
    pub fn figure(&self) -> String {
        if self.step == Step::Building {
            let (done, total) = self.mine().map_or((0, 0), stage_count);
            return format!("{done} of {total} done");
        }
        let at = self.at().map_or(0, |a| a + 1);
        format!("{at} of {}", self.steps().len())
    }
}

/// How a ready agent is signed in, in the Agents page's words; a plan sold under a brand other than the agent's own
/// name says that brand, and an OAuth login is a token's.
fn signed_in_word(row: &Agent, here: &str) -> String {
    match row.sign_in_kind.as_deref() {
        _ if row.sign_in == "vault-key" => words::SIGNED_IN_KEY.into(),
        Some("api-key") => words::SIGNED_IN_KEY.into(),
        Some("subscription") => match catalog::plan_brand(&row.id) {
            Some(brand) if !row.name.contains(brand) => format!("{}{brand}", words::SIGNED_IN_WITH),
            _ => words::SIGNED_IN_SUBSCRIPTION.into(),
        },
        Some("oauth") => format!("{}a token", words::SIGNED_IN_WITH),
        _ => format!("Signed in on {here}"),
    }
}
