// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts:374-631` at 65f6b89c7: the sign-ins step and the GitHub step of a run.
//! Spec: `spec/places_setup_sign_ins.md`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use wsp_catalog::catalog::{catalog_agents, login_sign_in, shared_on, sign_in_ways_of, AgentEntry};
use wsp_catalog::signin::{asks_the_person, key_env_of, login_there, mints_token};
use wsp_core::recipe_sync::RecipeChangeHow;
use wsp_engine::golden_seam::path_line;
use wsp_engine::machine::{ExecOptions, ExecResult};
use wsp_engine::provision::{EngineStep, ProvisionOn, ProvisionPlan};
use wsp_engine::run_env::{env_input, with_env_from_input, GITHUB_TOKEN_ENV};
use wsp_wire::types::agents_report::{AgentSignInState, AgentsSignInEvent, AgentsSignInEventState};
use wsp_wire::types::init_job::SignInFinish;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::recipe_file::{GitHubSignIn, RecipeFileAgentsValue, RecipeName, RecipeSignIn};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::agent_of_row;
use wsp_wire::views::place::{
    sign_in_row_id, PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceSetupEvent, PlaceSetupStep, PlaceWait, PlaceWaitState, GITHUB_CLI, GITHUB_ROW,
};
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
use wsp_words::exit::HostError;
use wsp_words::recipe_file::gh_status_of;
use wsp_words::views::place_setup_words::{
    copied_from_line, copied_not_signed_in_line, key_held_line, no_copy_line, no_key_line, sign_in_there_fix, sign_in_ways_fix, token_held_line,
    waits_for_install_line, SignInWay, SignInWayFacts, AT_ITS_TERMINAL, CODE_FROM_ROW, GITHUB_SKIPPED_LINE, NO_SIGN_IN_ROAD, SIGNED_IN_THERE,
    SIGN_IN_WAIT_MS, SKIPPED_FOR_NOW,
};
use wsp_words::words::base::last_line;

use super::super::helpers::{first_line_of, vault_held_line, vault_sign_in, GITHUB_MS, SIGNIN_STATUS_MS, SIGN_IN_SLACK_MS};
use super::super::types::{PlaceResult, SignInEmit, SignInHandle};
use super::run::{github_word_of, plain_row, SetupRun};
use super::{held_of, host_of, iso_at, signs_in_again, Skipper, SigningIn};

/// How a wait starts the sign-in it follows, every step it reaches going to the emit (setup.ts:80).
pub type SignInStart = Rc<dyn Fn(Rc<dyn Fn(AgentsSignInEvent)>) -> LocalBoxFuture<'static, Result<SignInHandle, HostError>>>;

/// How a wait lands: the road, the note a signed-in row carries, the fix a failed one carries (setup.ts:377).
#[derive(Clone, Default)]
pub struct SignInHow {
    pub start: Option<SignInStart>,
    pub note: Option<String>,
    pub fix: Option<String>,
}

const WAIT_MS: u64 = SIGN_IN_WAIT_MS as u64;

/// One sign-in a wait on the person follows: its page and code, its expiry, and whether a later run muted it.
struct SignInWait {
    run: Rc<SetupRun>,
    key: String,
    row: String,
    label: String,
    row_step: PlaceSetupStep,
    settled: Option<Rc<dyn Fn(bool)>>,
    note: Option<String>,
    fix: Option<String>,
    begun: u64,
    current: RefCell<PlaceWait>,
    shown: Cell<bool>,
    gone: Cell<bool>,
    finished: Cell<bool>,
    terminal: Cell<bool>,
    handle: RefCell<Option<Rc<SignInHandle>>>,
    signing: RefCell<Option<Rc<dyn Fn()>>>,
}

impl SignInWait {
    fn skipper_key(&self) -> String {
        format!("{}/{}", self.run.ask.place_id, self.row)
    }

    fn put_wait(self: &Rc<Self>, wait: PlaceWait) {
        let mut next = self.run.held.borrow().clone();
        next.waiting.retain(|x| x.row != self.row);
        next.waiting.push(wait.clone());
        self.run.push(next);
        self.run.frame_after_write(PlaceSetupEvent {
            add_id: self.run.ask.add_id.clone(),
            place_id: self.run.ask.place_id.clone(),
            line: None,
            wait: Some(wait),
            landed: None,
            running: None,
            end: None,
            said: None,
        });
    }

    /// The wait's first page is on the record before anything that follows it.
    fn ensure_shown(self: &Rc<Self>) {
        if self.shown.replace(true) {
            return;
        }
        let wait = self.current.borrow().clone();
        self.put_wait(wait);
    }

    fn mute(&self) {
        self.gone.set(true);
    }

    fn leave(&self) {
        self.gone.set(true);
        let handle = self.handle.borrow().clone();
        if let Some(handle) = handle {
            (handle.leave)();
        }
    }

    fn forget_signing(&self) {
        let token = self.signing.borrow().clone();
        let Some(token) = token else { return };
        let mut signing = self.run.area.signing_in.borrow_mut();
        if signing.get(&self.key).is_some_and(|entry| Rc::ptr_eq(&entry.leave, &token)) {
            signing.shift_remove(&self.key);
        }
    }

    /// What lands the row: the wait goes off the record, the settled hears it, and the row is said (setup.ts:384).
    fn landed(self: &Rc<Self>, mut row: PlaceProvisionRow) {
        self.finished.set(true);
        self.run.area.skippers.borrow_mut().shift_remove(&self.skipper_key());
        self.run.held.borrow_mut().waiting.retain(|x| x.row != self.row);
        if let Some(settled) = &self.settled {
            settled(row.outcome == PlaceProvisionRowOutcome::Installed);
        }
        if row.outcome == PlaceProvisionRowOutcome::Failed {
            if let Some(fix) = &self.fix {
                row.fix = Some(fix.clone());
            }
        }
        row.step = Some(self.row_step);
        self.run.land_row(held_of(row));
    }

    fn on_event(self: &Rc<Self>, e: AgentsSignInEvent) {
        if self.gone.get() {
            return;
        }
        let pending = matches!(e.state, AgentsSignInEventState::Waiting | AgentsSignInEventState::Running);
        if !pending {
            self.terminal.set(true);
            self.forget_signing();
        }
        let now = self.run.area.ctx.clock.now() as u64;
        match e.state {
            AgentsSignInEventState::Waiting | AgentsSignInEventState::Running => {
                if e.url.is_none() && e.code.is_none() {
                    return;
                }
                self.ensure_shown();
                {
                    let mut current = self.current.borrow_mut();
                    if let Some(url) = e.url.clone() {
                        current.url = Some(url);
                    }
                    if let Some(code) = e.code.clone() {
                        current.code = Some(code);
                    }
                }
                let wait = self.current.borrow().clone();
                self.put_wait(wait);
            }
            AgentsSignInEventState::SignedIn => {
                self.ensure_shown();
                let ms = (now as f64 - self.begun as f64).round();
                let row = PlaceProvisionRow {
                    note: Some(self.note.clone().unwrap_or_else(|| SIGNED_IN_THERE.to_string())),
                    ms: Some(JsInt(JsNumber(ms))),
                    ..plain_row(&self.row, &self.label, PlaceProvisionRowOutcome::Installed)
                };
                self.landed(row);
            }
            AgentsSignInEventState::Failed => {
                self.ensure_shown();
                if now >= self.begun + WAIT_MS - SIGN_IN_SLACK_MS {
                    let mut expired = self.current.borrow().clone();
                    expired.state = PlaceWaitState::Expired;
                    self.put_wait(expired);
                    if let Some(settled) = &self.settled {
                        settled(false);
                    }
                } else {
                    let said = e.said.clone().unwrap_or_else(|| "the sign-in did not finish".to_string());
                    let row = PlaceProvisionRow { note: Some(said), ..plain_row(&self.row, &self.label, PlaceProvisionRowOutcome::Failed) };
                    self.landed(row);
                }
            }
        }
    }

    /// Skip for now: the row reads skipped, nothing more of this login is heard, and the login there stops (setup.ts:406).
    async fn skip(self: &Rc<Self>) {
        if self.gone.get() {
            return;
        }
        self.gone.set(true);
        let handle = self.handle.borrow().clone();
        if let Some(handle) = handle {
            if let Some(stop) = &handle.stop {
                stop();
            }
            (handle.leave)();
        }
        self.forget_signing();
        let row = PlaceProvisionRow { note: Some(SKIPPED_FOR_NOW.to_string()), ..plain_row(&self.row, &self.label, PlaceProvisionRowOutcome::Skipped) };
        self.landed(row);
        self.run.writing.settled().await;
    }
}

fn catalog_agent(agent: &str) -> Option<&'static AgentEntry> {
    catalog_agents().into_iter().find(|entry| entry.id == agent)
}

fn agent_label(agent: &str) -> String {
    catalog_agent(agent).map(|entry| entry.name.clone()).unwrap_or_else(|| agent.to_string())
}

fn agent_pick<'a>(agents: &'a IndexMap<RecipeName, RecipeFileAgentsValue>, agent: &str) -> Option<&'a RecipeFileAgentsValue> {
    agents.iter().find(|(name, _)| name.0 == agent).map(|(_, value)| value)
}

impl SetupRun {
    fn before_rows(&self) -> Vec<PlaceProvisionRow> {
        self.record.applied.as_ref().map(|a| a.rows.iter().map(|r| r.row.clone()).collect()).unwrap_or_default()
    }

    /// A sign-in on that computer that waits on the person: its row waits with the page and the code as the relay
    /// reads them, and when the wait runs out it reads expired and a retry asks for a fresh one. It never holds the
    /// job; its row lands whenever the person is through, and `settled` hears whether it signed in (setup.ts:377).
    pub async fn sign_in_there(self: &Rc<Self>, ask: &str, row: &str, label: &str, row_step: PlaceSetupStep, settled: Option<Rc<dyn Fn(bool)>>, how: SignInHow) {
        let SignInHow { start, note, fix } = how;
        let place = self.ask.place_id.clone();
        let begun = self.area.ctx.clock.now() as u64;
        let key = format!("{place}/{ask}");
        let wait = Rc::new(SignInWait {
            run: Rc::clone(self),
            key: key.clone(),
            row: row.to_string(),
            label: label.to_string(),
            row_step,
            settled,
            note,
            fix,
            begun,
            current: RefCell::new(PlaceWait {
                row: row.to_string(),
                label: label.to_string(),
                url: None,
                code: None,
                expires_at: iso_at(begun + WAIT_MS),
                state: PlaceWaitState::Waiting,
            }),
            shown: Cell::new(false),
            gone: Cell::new(false),
            finished: Cell::new(false),
            terminal: Cell::new(false),
            handle: RefCell::new(None),
            signing: RefCell::new(None),
        });
        let emit: SignInEmit = {
            let wait = Rc::clone(&wait);
            Rc::new(move |e: AgentsSignInEvent| wait.on_event(e))
        };
        let road: LocalBoxFuture<'static, PlaceResult<Option<SignInHandle>>> = match start {
            Some(start) => {
                let emit = Rc::clone(&emit);
                async move { start(emit).await.map(Some) }.boxed_local()
            }
            None => {
                let recording = Rc::clone(&self.area.ctx.recording);
                let (place, agent, emit) = (place.clone(), ask.to_string(), Rc::clone(&emit));
                async move {
                    match recording.sign_in(&place, &agent, emit).await {
                        None => Ok(None),
                        Some(answer) => answer.map(Some),
                    }
                }
                .boxed_local()
            }
        };
        let before = self.area.signing_in.borrow().get(&key).map(|entry| (Rc::clone(&entry.mute), Rc::clone(&entry.leave)));
        let handle = match road.await {
            Ok(Some(handle)) => handle,
            Ok(None) => {
                wait.landed(PlaceProvisionRow { note: Some(NO_SIGN_IN_ROAD.to_string()), ..plain_row(row, label, PlaceProvisionRowOutcome::Failed) });
                return;
            }
            Err(e) => {
                if !wait.gone.get() {
                    wait.landed(PlaceProvisionRow { note: Some(first_line_of(&e)), ..plain_row(row, label, PlaceProvisionRowOutcome::Failed) });
                }
                return;
            }
        };
        wait.ensure_shown();
        let handle = Rc::new(handle);
        *wait.handle.borrow_mut() = Some(Rc::clone(&handle));
        if let Some((mute, _)) = &before {
            mute();
        }
        if !wait.finished.get() && !wait.terminal.get() {
            let leave: Rc<dyn Fn()> = {
                let wait = Rc::clone(&wait);
                Rc::new(move || wait.leave())
            };
            let mute: Rc<dyn Fn()> = {
                let wait = Rc::clone(&wait);
                Rc::new(move || wait.mute())
            };
            *wait.signing.borrow_mut() = Some(Rc::clone(&leave));
            self.area.signing_in.borrow_mut().insert(key, SigningIn { mute, leave });
        }
        if !wait.finished.get() {
            let skip: Skipper = {
                let wait = Rc::clone(&wait);
                Rc::new(move || {
                    let wait = Rc::clone(&wait);
                    async move { wait.skip().await }.boxed_local()
                })
            };
            self.area.skippers.borrow_mut().insert(format!("{place}/{row}"), skip);
        }
        if let Some((_, leave)) = &before {
            leave();
        }
        if wait.gone.get() {
            (handle.leave)();
        }
    }

    /// Whether an agent is signed in on that computer, by its own status command run there as the hand sign-in plans
    /// it; the logins that computer's daemon last listed only where that status cannot be read (setup.ts:453).
    pub async fn agent_signed_in(self: &Rc<Self>, agent: &str, plan: &ProvisionPlan) -> bool {
        let place = self.ask.place_id.clone();
        if let Some(check) = login_sign_in(agent).and_then(|sign_in| sign_in.status()) {
            let line = match self.area.ctx.recording.sign_in_line(&place, agent).await {
                Some(Ok(line)) => Some(line),
                _ => None,
            };
            if let Some(status) = line.as_ref().and_then(|line| line.status.clone()) {
                let cmd = format!("{}\n{status} 2>&1", path_line(&plan.path, plan.prefix.as_deref()));
                let vars = line.as_ref().and_then(|line| line.env.clone()).unwrap_or_default();
                let options = ExecOptions { timeout_ms: Some(SIGNIN_STATUS_MS), stdin: Some(env_input(&vars)), ..Default::default() };
                let res = self.machine.exec(&with_env_from_input(&cmd), options).await.ok().filter(|res| res.exit_code.get() != EXEC_DEADLINE_EXIT);
                if let Some(res) = res {
                    let signed_in = check.signed_in(&res.stdout, res.exit_code.get() as i32);
                    if signed_in {
                        let _ = self.area.records.login_listed(&place, agent).await;
                    }
                    return signed_in;
                }
            }
        }
        self.area.ctx.sign_ins_here(&place, None).and_then(|states| states.get(agent).copied()) == Some(AgentSignInState::SignedIn)
    }

    /// An agent's sign-in on that computer, started only where it is not signed in there already (setup.ts:469).
    pub async fn agent_sign_in(self: &Rc<Self>, agent: &str, plan: &ProvisionPlan) {
        let label = agent_label(agent);
        let id = sign_in_row_id(agent);
        if self.agent_signed_in(agent, plan).await {
            let row = PlaceProvisionRow { note: Some(SIGNED_IN_THERE.to_string()), step: Some(PlaceSetupStep::Signins), ..plain_row(&id, &label, PlaceProvisionRowOutcome::Present) };
            return self.land_row(held_of(row));
        }
        let sign_in = catalog_agent(agent).map(|entry| &entry.sign_in);
        if sign_in.is_some_and(|s| asks_the_person(s)) {
            let row = PlaceProvisionRow { note: Some(AT_ITS_TERMINAL.to_string()), step: Some(PlaceSetupStep::Signins), ..plain_row(&id, &label, PlaceProvisionRowOutcome::Skipped) };
            return self.land_row(held_of(row));
        }
        if sign_in.and_then(|s| login_there(s)).is_some_and(|login| matches!(login.finish, SignInFinish::Code)) {
            let fix = self.ways_fix(agent, &label, Some(SignInWay::Machine)).unwrap_or_else(|| sign_in_there_fix(&self.record.name));
            let row = PlaceProvisionRow { note: Some(CODE_FROM_ROW.to_string()), fix: Some(fix), step: Some(PlaceSetupStep::Signins), ..plain_row(&id, &label, PlaceProvisionRowOutcome::Skipped) };
            return self.land_row(held_of(row));
        }
        self.sign_in_there(agent, &id, &label, PlaceSetupStep::Signins, None, SignInHow::default()).await;
    }

    /// What a row of an agent with several ways says to do, the way picked first; nothing for an agent with one (setup.ts:483).
    pub fn ways_fix(&self, agent: &str, label: &str, first: Option<SignInWay>) -> Option<String> {
        let ways = sign_in_ways_of(agent);
        let sign_in = match catalog_agent(agent).map(|entry| &entry.sign_in) {
            Some(sign_in) if !ways.is_empty() => sign_in,
            _ => return None,
        };
        let ordered: Vec<SignInWay> = match first {
            Some(first) if ways.contains(&first) => std::iter::once(first).chain(ways.iter().copied().filter(|w| *w != first)).collect(),
            _ => ways.clone(),
        };
        let facts = SignInWayFacts {
            name: label.to_string(),
            here: self.area.here(),
            r#box: self.record.name.clone(),
            mint: mints_token(sign_in).map(|token| token.mint.clone()),
            key_env: key_env_of(sign_in).map(str::to_string),
            field: None,
        };
        Some(sign_in_ways_fix(&ordered, &facts))
    }

    /// A sign-in picked to take a token or a key from this host's vault: present where the vault holds it; a token it
    /// lacks is made on this computer, waiting on the person's browser, and lands in the vault for every turn there
    /// (setup.ts:494). `None` where a wait started or the row landed through it.
    pub async fn vault_way(self: &Rc<Self>, agent: &str, label: &str, way: SignInWay) -> Option<PlaceProvisionRow> {
        let id = sign_in_row_id(agent);
        let sign_in = catalog_agent(agent).map(|entry| &entry.sign_in);
        let name: Option<String> = sign_in.and_then(|s| match way {
            SignInWay::Token => mints_token(s).map(|token| token.token_env.clone()),
            _ => key_env_of(s).map(str::to_string),
        });
        let here = self.area.here();
        if let Some(name) = &name {
            if self.vault().contains_key(name) {
                let note = if way == SignInWay::Token { token_held_line(&here) } else { key_held_line(&here) };
                return Some(PlaceProvisionRow { note: Some(note), step: Some(PlaceSetupStep::Signins), ..plain_row(&id, label, PlaceProvisionRowOutcome::Present) });
            }
        }
        if way == SignInWay::Key {
            let fix = self.ways_fix(agent, label, Some(SignInWay::Key)).unwrap_or_else(|| sign_in_there_fix(&self.record.name));
            let note = no_key_line(name.as_deref().unwrap_or("its key"), &here);
            return Some(PlaceProvisionRow { note: Some(note), fix: Some(fix), step: Some(PlaceSetupStep::Signins), ..plain_row(&id, label, PlaceProvisionRowOutcome::Failed) });
        }
        let fix = self.ways_fix(agent, label, Some(SignInWay::Token)).unwrap_or_else(|| sign_in_there_fix(&self.record.name));
        let recording = Rc::clone(&self.area.ctx.recording);
        let mint_agent = agent.to_string();
        let start: SignInStart = Rc::new(move |emit: SignInEmit| {
            let (recording, mint_agent) = (Rc::clone(&recording), mint_agent.clone());
            async move {
                match recording.mint_here(&mint_agent, emit).await {
                    None => Err(HostError::new(NO_SIGN_IN_ROAD)),
                    Some(answer) => answer,
                }
            }
            .boxed_local()
        });
        let how = SignInHow { start: Some(start), note: Some(token_held_line(&here)), fix: Some(fix) };
        self.sign_in_there(agent, &id, label, PlaceSetupStep::Signins, None, how).await;
        None
    }

    /// A sign-in a retry runs again for a fresh page: the token made here where the row was picked to take one, else
    /// the agent's own sign-in on that computer (setup.ts:508).
    pub async fn again_sign_in(self: &Rc<Self>, agent: &str, plan: &ProvisionPlan) {
        let token = agent_pick(&self.ask.picks.agents, agent).and_then(|pick| pick.signin.as_ref()) == Some(&RecipeSignIn::Token);
        if !token {
            return self.agent_sign_in(agent, plan).await;
        }
        let label = agent_label(agent);
        if let Some(row) = self.vault_way(agent, &label, SignInWay::Token).await {
            self.land_row(held_of(PlaceProvisionRow { step: Some(PlaceSetupStep::Signins), ..row }));
        }
    }

    /// Whether an agent did not install (setup.ts:517).
    pub fn not_installed(&self, agent: &str) -> bool {
        self.rows.borrow().iter().any(|r| agent_of_row(&r.row.id).as_deref() == Some(agent) && r.row.outcome == PlaceProvisionRowOutcome::Failed)
    }

    /// The sign-ins step (setup.ts:522).
    pub async fn sign_ins(self: &Rc<Self>, plan: &ProvisionPlan) -> Vec<PlaceProvisionRow> {
        let mut out = Vec::new();
        let before = self.before_rows();
        let here = self.area.here();
        for (name, pick) in self.ask.picks.agents.iter() {
            let agent = name.0.clone();
            if let Some(sync) = &self.ask.sync {
                let key = format!("agents/{agent}");
                if !sync.changes.iter().any(|c| c.key == key && matches!(c.how, RecipeChangeHow::Added | RecipeChangeHow::Changed)) {
                    continue;
                }
            }
            if !signs_in_again(self.ask.only.as_ref(), &agent, &before) {
                continue;
            }
            let label = agent_label(&agent);
            let id = sign_in_row_id(&agent);
            match pick.signin.clone().unwrap_or(RecipeSignIn::Vault) {
                RecipeSignIn::Token => {
                    if let Some(row) = self.vault_way(&agent, &label, SignInWay::Token).await {
                        out.push(row);
                    }
                }
                RecipeSignIn::Key => {
                    if let Some(row) = self.vault_way(&agent, &label, SignInWay::Key).await {
                        out.push(row);
                    }
                }
                RecipeSignIn::Machine => {
                    if self.not_installed(&agent) {
                        let note = waits_for_install_line(&label);
                        out.push(PlaceProvisionRow { note: Some(note), ..plain_row(&id, &label, PlaceProvisionRowOutcome::Skipped) });
                    } else {
                        self.agent_sign_in(&agent, plan).await;
                    }
                }
                RecipeSignIn::Vault => {
                    let vault = self.vault();
                    if vault_sign_in(&agent, &vault) == AgentSignInState::VaultKey {
                        let note = vault_held_line(&agent, &vault, Some(RecipeSignIn::Vault), &here);
                        out.push(PlaceProvisionRow { note: Some(note), ..plain_row(&id, &label, PlaceProvisionRowOutcome::Present) });
                        continue;
                    }
                    if let Some(copied) = self.login_copied(&agent, &label, plan).await {
                        out.push(copied);
                        continue;
                    }
                    let fix = self.ways_fix(&agent, &label, None).unwrap_or_else(|| sign_in_there_fix(&self.record.name));
                    let note = no_copy_line(&label, &here);
                    out.push(PlaceProvisionRow { note: Some(note), fix: Some(fix), ..plain_row(&id, &label, PlaceProvisionRowOutcome::Failed) });
                }
            }
        }
        out
    }

    /// An agent's login as this computer holds it, put where every thread there reads it, on the run's input alone
    /// (setup.ts:559).
    pub async fn login_copied(self: &Rc<Self>, agent: &str, label: &str, plan: &ProvisionPlan) -> Option<PlaceProvisionRow> {
        let shared = shared_on(agent)?;
        let place = self.ask.place_id.clone();
        let held: Vec<u8> = match self.area.ctx.recording.login_here(agent).await {
            Some(Ok(Some(bytes))) => bytes,
            _ => return None,
        };
        let store = self.stores_here().await.and_then(|stores| stores.get(agent).cloned())?;
        let id = sign_in_row_id(agent);
        if self.not_installed(agent) {
            let note = waits_for_install_line(label);
            return Some(PlaceProvisionRow { note: Some(note), ..plain_row(&id, label, PlaceProvisionRowOutcome::Skipped) });
        }
        if self.agent_signed_in(agent, plan).await {
            return Some(PlaceProvisionRow { note: Some(SIGNED_IN_THERE.to_string()), ..plain_row(&id, label, PlaceProvisionRowOutcome::Present) });
        }
        let at = shell_quote(&format!("{store}/{}", shared.file));
        let cmd = format!("umask 077 && mkdir -p {} && cat > {at}.wsp && mv -f {at}.wsp {at}", shell_quote(&store));
        let options = ExecOptions { timeout_ms: Some(SIGNIN_STATUS_MS), stdin: Some(held), ..Default::default() };
        let (exit, stderr) = match self.machine.exec(&cmd, options).await {
            Ok(res) => (res.exit_code.get(), res.stderr),
            Err(e) => (-1, first_line_of(&host_of(&e))),
        };
        let here = self.area.here();
        let _ = place;
        if exit != 0 {
            let note = last_line(&stderr).unwrap_or_else(|| format!("the copy exited {exit}"));
            let fix = sign_in_there_fix(&self.record.name);
            return Some(PlaceProvisionRow { note: Some(note), fix: Some(fix), ..plain_row(&id, label, PlaceProvisionRowOutcome::Failed) });
        }
        if self.agent_signed_in(agent, plan).await {
            Some(PlaceProvisionRow { note: Some(copied_from_line(&here)), ..plain_row(&id, label, PlaceProvisionRowOutcome::Installed) })
        } else {
            let note = copied_not_signed_in_line(label, &here);
            let fix = sign_in_there_fix(&self.record.name);
            Some(PlaceProvisionRow { note: Some(note), fix: Some(fix), ..plain_row(&id, label, PlaceProvisionRowOutcome::Failed) })
        }
    }

    /// gh's own status there, over the vault's token on the run's input where one is given (setup.ts:588).
    pub async fn gh_status(self: &Rc<Self>, plan: &ProvisionPlan, token: Option<&str>) -> Result<ExecResult, HostError> {
        let cmd = format!("{}\ngh auth status --hostname github.com 2>&1", path_line(&plan.path, plan.prefix.as_deref()));
        let answer = match token {
            None => self.machine.exec(&cmd, ExecOptions { timeout_ms: Some(GITHUB_MS), ..Default::default() }).await,
            Some(token) => {
                let vars = IndexMap::from([(GITHUB_TOKEN_ENV.to_string(), token.to_string())]);
                let options = ExecOptions { timeout_ms: Some(GITHUB_MS), stdin: Some(env_input(&vars)), ..Default::default() };
                self.machine.exec(&with_env_from_input(&cmd), options).await
            }
        };
        answer.map_err(|e| host_of(&e))
    }

    /// gh's login on that computer through the sign-in relay, started only where gh is not signed in there already (setup.ts:593).
    pub async fn github_sign_in(self: &Rc<Self>, plan: &ProvisionPlan) {
        if let Ok(res) = self.gh_status(plan, None).await {
            if res.exit_code.get() == 0 {
                self.settle_github(true);
                let row = PlaceProvisionRow { note: Some(SIGNED_IN_THERE.to_string()), step: Some(PlaceSetupStep::Github), ..plain_row(GITHUB_ROW, "GitHub", PlaceProvisionRowOutcome::Present) };
                return self.land_row(held_of(row));
            }
        }
        let run = Rc::clone(self);
        let settled: Rc<dyn Fn(bool)> = Rc::new(move |ok: bool| run.settle_github(ok));
        self.sign_in_there(GITHUB_CLI, GITHUB_ROW, "GitHub", PlaceSetupStep::Github, Some(settled), SignInHow::default()).await;
    }

    /// The GitHub step: gh put on where nothing else puts it, then signed in by the row's word (setup.ts:603).
    pub async fn github(self: &Rc<Self>, plan: &ProvisionPlan) -> Result<Vec<PlaceProvisionRow>, HostError> {
        if self.ask.picks.configs.github.is_none() {
            return Ok(Vec::new());
        }
        let word = github_word_of(&self.ask.picks);
        if word == GitHubSignIn::Skip {
            let kept: Vec<PlaceProvisionRow> = self
                .record
                .applied
                .as_ref()
                .map(|a| {
                    a.rows
                        .iter()
                        .filter(|r| r.row.step == Some(PlaceSetupStep::Github) && (r.row.outcome == PlaceProvisionRowOutcome::Installed || (r.row.id == GITHUB_ROW && r.row.outcome == PlaceProvisionRowOutcome::Present)))
                        .map(|r| r.row.clone())
                        .collect()
                })
                .unwrap_or_default();
            let github_kept = kept.iter().any(|r| r.id == GITHUB_ROW);
            self.settle_github(github_kept);
            if github_kept {
                return Ok(kept);
            }
            let mut out = kept;
            out.push(PlaceProvisionRow { note: Some(GITHUB_SKIPPED_LINE.to_string()), ..plain_row(GITHUB_ROW, "GitHub", PlaceProvisionRowOutcome::Skipped) });
            return Ok(out);
        }
        let Some(provisioner) = self.area.ctx.wiring.provision.clone() else { return Ok(Vec::new()) };
        let on = ProvisionOn { home: self.home.clone(), held: None, stores: None };
        let got = provisioner.step(Rc::clone(&self.machine), plan, &EngineStep::Github, &self.engine_run, self.stage_of("github"), &on).await?;
        let gh = self.ours(got);
        if gh.iter().any(|r| r.outcome == PlaceProvisionRowOutcome::Failed) {
            self.settle_github(false);
            return Ok(gh);
        }
        if word == GitHubSignIn::Machine {
            self.github_sign_in(plan).await;
            return Ok(gh);
        }
        let here = self.area.here();
        let Some(token) = self.vault().get(GITHUB_TOKEN_ENV).cloned() else {
            self.settle_github(false);
            let fix = sign_in_there_fix(&self.record.name);
            let mut out = gh;
            out.push(PlaceProvisionRow { note: Some(no_copy_line("GitHub", &here)), fix: Some(fix), ..plain_row(GITHUB_ROW, "GitHub", PlaceProvisionRowOutcome::Failed) });
            return Ok(out);
        };
        let res = self.gh_status(plan, Some(&token)).await?;
        let scopes = gh_status_of(&res.stdout).scopes.map(|scopes| scopes.join(", ")).filter(|scopes| !scopes.is_empty());
        let ok = res.exit_code.get() == 0;
        self.settle_github(ok);
        let mut out = gh;
        let row = if ok {
            let suffix = scopes.map(|scopes| format!("; token scopes: {scopes}")).unwrap_or_default();
            PlaceProvisionRow { note: Some(format!("{}{suffix}", copied_from_line(&here))), ..plain_row(GITHUB_ROW, "GitHub", PlaceProvisionRowOutcome::Present) }
        } else {
            let note = last_line(&res.stdout).unwrap_or_else(|| format!("gh auth status exited {}", res.exit_code.get()));
            PlaceProvisionRow { note: Some(note), ..plain_row(GITHUB_ROW, "GitHub", PlaceProvisionRowOutcome::Failed) }
        };
        out.push(row);
        Ok(out)
    }
}
