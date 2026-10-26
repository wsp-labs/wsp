// SPDX-License-Identifier: AGPL-3.0-only
//! agents-read.ts:312-350, 426-439, 486-605 at 65f6b89c7: one watched sign-in per agent or server on a target. Spec:
//! `spec/agents_read_sign_ins.md`.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use serde_json::{json, Map, Value};

use wsp_catalog::catalog::{catalog_entry, CatalogEntry};
use wsp_catalog::signin::mints_token;
use wsp_core::daemon_channel::Frame;
use wsp_frames::True;
use wsp_wire::types::agents_report::{
    AgentsSignInEvent, AgentsSignInEventState, AgentsSignInEventType, AgentsSignInRun,
    AgentsSignInRunLast, AgentsTarget, SignInLine, NO_SIGN_IN_REFUSAL, SIGN_IN_ENDED_KEPT_MS,
};
use wsp_wire::views::place::{PlaceProvisionRow, PlaceProvisionRowOutcome, GITHUB_CLI};
use wsp_wire::views::workspace::Caller;
use wsp_words::agents_report::{
    installing_first_line, napping_sign_in_refusal, pty_bare_on, tool_not_installed_fix,
    tool_not_installed_line, without_control_chars,
};
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::words::base::last_line;

use super::{
    AgentsReads, CodeWriter, DaemonEvent, Follower, Following, OnOrNapping, PtyLink, Running,
    SignInAsk, SignInForward, SignInPlan, SignInRun, SignInStep,
};

/// The refusal a start, a follow, a code or a stop gives an id that is not running (agents-read.ts:584).
fn no_sign_in() -> HostError {
    HostError::with_kind(NO_SIGN_IN_REFUSAL, Kind::Usage)
}

fn usage(sentence: String) -> HostError {
    HostError::with_kind(sentence, Kind::Usage)
}

/// `listed` (agents-read.ts:325-333): a sign-in as the list answers it.
fn listed_run(sign_in_id: &str, run: &Running) -> AgentsSignInRun {
    AgentsSignInRun {
        sign_in_id: sign_in_id.to_string(),
        target: run.target.clone(),
        agent: run.ask.agent.clone(),
        server: run.ask.server.clone(),
        scope: run.ask.scope,
        project: run.ask.project.clone(),
        last: run.last.borrow().clone().map(|event| AgentsSignInRunLast {
            kind: AgentsSignInEventType,
            event,
        }),
        ended: None,
    }
}

fn state_word(state: AgentsSignInEventState) -> &'static str {
    match state {
        AgentsSignInEventState::Running => "running",
        AgentsSignInEventState::Waiting => "waiting",
        AgentsSignInEventState::SignedIn => "signed-in",
        AgentsSignInEventState::Failed => "failed",
    }
}

impl AgentsReads {
    /// agents-read.ts:339 (N1): the JSON of `[target, agent, server or null, terminal, login]`.
    pub fn key_of(target: &AgentsTarget, ask: &SignInAsk) -> String {
        let target = serde_json::to_value(target).unwrap_or(Value::Null);
        wsp_wire::text::json_stringify(&json!([
            target,
            ask.agent,
            ask.server,
            ask.terminal == Some(true),
            ask.login == Some(true)
        ]))
    }

    /// agents-read.ts:340-350 (N3).
    pub fn follow(&self, sign_in_id: &str, run: &Rc<Running>, emit: Follower) -> Following {
        let fresh = {
            let mut followers = run.followers.borrow_mut();
            if followers.iter().any(|held| Rc::ptr_eq(held, &emit)) {
                false
            } else {
                followers.push(emit.clone());
                true
            }
        };
        if fresh {
            let last = run.last.borrow().clone();
            if let Some(last) = last {
                emit(last);
            }
        }
        let held_run = Rc::clone(run);
        Following {
            sign_in_id: sign_in_id.to_string(),
            leave: Box::new(move || {
                held_run
                    .followers
                    .borrow_mut()
                    .retain(|held| !Rc::ptr_eq(held, &emit))
            }),
        }
    }

    /// agents-read.ts:325-333: a sign-in as listed.
    pub fn listed(sign_in_id: &str, run: &Running) -> AgentsSignInRun {
        listed_run(sign_in_id, run)
    }

    /// agents-read.ts:334-336 (N7, N9): drops ended sign-ins strictly older than `SIGN_IN_ENDED_KEPT_MS`.
    pub fn prune_ended(&self) {
        let now = self.o.clock.now();
        self.ended
            .borrow_mut()
            .retain(|_, (_, at)| now - *at <= SIGN_IN_ENDED_KEPT_MS as i64);
    }

    /// agents-read.ts:429-439 (N5): the failed install's words, or none.
    pub async fn gh_first(
        &self,
        target: &AgentsTarget,
        ask: &SignInAsk,
        installing: Rc<dyn Fn()>,
    ) -> Result<Option<String>, HostError> {
        let AgentsTarget::V0(place) = target else {
            return Ok(None);
        };
        if ask.server.is_some() || ask.tool_there == Some(true) || ask.agent != GITHUB_CLI {
            return Ok(None);
        }
        let Some(door) = (self.o.places)() else {
            return Ok(None);
        };
        let told = Rc::new(Cell::new(false));
        let rows = door
            .gh_there(
                &place.place_id,
                Rc::new(
                    move |_detail: String, at: Option<String>, row: Option<PlaceProvisionRow>| {
                        if told.get() || at.is_none() || row.is_some() {
                            return;
                        }
                        told.set(true);
                        installing();
                    },
                ),
            )
            .await?;
        let failed = rows
            .iter()
            .find(|r| r.outcome == PlaceProvisionRowOutcome::Failed);
        Ok(failed.map(|r| {
            last_line(r.note.as_deref().unwrap_or(""))
                .unwrap_or_else(|| format!("{} did not install", r.label))
        }))
    }

    /// agents-read.ts:486-577 (N1, N2, N4, N6, N7, N8).
    pub async fn sign_in(
        self: &Rc<Self>,
        target: AgentsTarget,
        ask: SignInAsk,
        emit: Follower,
        origin: Option<Caller>,
    ) -> Result<Following, HostError> {
        let key = Self::key_of(&target, &ask);
        let held = self.starting.borrow().get(&key).cloned();
        if let Some(held) = held {
            let (sign_in_id, run) = held.await?;
            return Ok(self.follow(&sign_in_id, &run, emit));
        }
        let begun: Shared<LocalBoxFuture<'static, Result<(String, Rc<Running>), HostError>>> = {
            let me = self.clone();
            let (key, target, ask, origin, first) = (
                key.clone(),
                target.clone(),
                ask.clone(),
                origin.clone(),
                emit.clone(),
            );
            async move { me.begin(&key, target, ask, origin, first).await }
                .boxed_local()
                .shared()
        };
        self.starting
            .borrow_mut()
            .insert(key.clone(), begun.clone());
        tokio::task::yield_now().await;
        match begun.await {
            Ok((sign_in_id, run)) => Ok(self.follow(&sign_in_id, &run, emit)),
            Err(e) => {
                self.starting.borrow_mut().shift_remove(&key);
                Err(e)
            }
        }
    }

    /// agents-read.ts:486-549: the plan is refused or started, and the run is listed and handed to its plan.
    async fn begin(
        self: &Rc<Self>,
        key: &str,
        target: AgentsTarget,
        ask: SignInAsk,
        origin: Option<Caller>,
        emit: Follower,
    ) -> Result<(String, Rc<Running>), HostError> {
        let acts = self.acts()?;
        let on = match self.on_of(&target, origin.as_ref(), false).await? {
            OnOrNapping::On(on) => on,
            OnOrNapping::Napping(n) => {
                return Err(usage(napping_sign_in_refusal(
                    &n.napping,
                    n.pause_mode.as_ref(),
                )))
            }
        };
        let what = format!(
            "{}{}, on {}",
            without_control_chars(&ask.agent),
            ask.server
                .as_ref()
                .map(|s| format!(", server {}", without_control_chars(s)))
                .unwrap_or_default(),
            match &target {
                AgentsTarget::V1(ws) => format!("workspace {}", ws.workspace_id),
                AgentsTarget::V0(place) => format!("computer {}", place.place_id),
            }
        );
        let plan: SignInPlan = match acts.sign_in(&on, &ask).await {
            Ok(plan) => plan,
            Err(e) => {
                self.log_line(format!("sign-in refused: {what}: {}", e.message));
                return Err(e);
            }
        };
        let bytes = (self.o.random_bytes)(6);
        let sign_in_id = format!(
            "si_{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        self.log_line(format!("sign-in {sign_in_id} started: {what}"));
        let readers: Rc<RefCell<Vec<Rc<dyn Fn(&DaemonEvent)>>>> = Rc::new(RefCell::new(Vec::new()));
        let fan: Rc<dyn Fn(DaemonEvent)> = {
            let readers = readers.clone();
            Rc::new(move |event: DaemonEvent| {
                let held = readers.borrow().clone();
                for read in held {
                    read(&event);
                }
            })
        };
        let channel = (self.o.channel)(target.clone(), fan, origin.clone()).await?;
        let (settle, stopped) = oneshot::channel::<()>();
        let settle = Rc::new(RefCell::new(Some(settle)));
        let stop_future: Shared<LocalBoxFuture<'static, ()>> = async move {
            let _ = stopped.await;
        }
        .boxed_local()
        .shared();
        let run = Rc::new(Running {
            target: target.clone(),
            ask: ask.clone(),
            r#type: RefCell::new(None),
            followers: RefCell::new(vec![emit.clone()]),
            last: RefCell::new(None),
            stopped_by: Cell::new(false),
            stop: {
                let me = self.clone();
                let (key, sign_in_id) = (key.to_string(), sign_in_id.clone());
                let settle = settle.clone();
                Box::new(move || {
                    let ours = me
                        .starting
                        .borrow()
                        .get(&key)
                        .and_then(|held| held.clone().now_or_never())
                        .and_then(|r| r.ok())
                        .is_some_and(|(id, _)| id == sign_in_id);
                    if ours {
                        me.starting.borrow_mut().shift_remove(&key);
                    }
                    if let Some(tx) = settle.borrow_mut().take() {
                        let _ = tx.send(());
                    }
                })
            },
        });
        self.running
            .borrow_mut()
            .insert(sign_in_id.clone(), run.clone());
        let forward: Option<Rc<dyn SignInForward>> = if ask.server.is_some() {
            self.forward_of(&target)
        } else {
            None
        };
        let link = self.pty_link(&channel, &readers, &target);
        let step = {
            let (run, id) = (run.clone(), sign_in_id.clone());
            Rc::new(move |s: SignInStep| {
                let event = AgentsSignInEvent {
                    sign_in_id: id.clone(),
                    state: s.state,
                    url: s.url,
                    code: s.code,
                    paste: s.paste,
                    said: s.said,
                    pty_id: s.pty_id,
                };
                *run.last.borrow_mut() = Some(event.clone());
                let followers = run.followers.borrow().clone();
                for tell in followers {
                    tell(event.clone());
                }
            })
        };
        let sign_in_run = SignInRun {
            link,
            emit: {
                let step = step.clone();
                Rc::new(move |s: SignInStep| step(s))
            },
            typing: {
                let run = run.clone();
                Rc::new(move |write: Option<CodeWriter>| *run.r#type.borrow_mut() = write)
            },
            stop: stop_future,
            forward: forward.clone(),
        };
        let me = self.clone();
        let (sign_in_id_task, key_task, what_task, target_task, ask_task, run_task) = (
            sign_in_id.clone(),
            key.to_string(),
            what.clone(),
            target.clone(),
            ask.clone(),
            run.clone(),
        );
        wsp_engine::local_task::try_spawn_local(async move {
            let gh_step = {
                let step = step.clone();
                let gh_name = catalog_entry(GITHUB_CLI)
                    .map(|e| e.name().to_string())
                    .unwrap_or_else(|| GITHUB_CLI.to_string());
                Rc::new(move || {
                    step(SignInStep {
                        state: AgentsSignInEventState::Running,
                        url: None,
                        code: None,
                        paste: None,
                        said: Some(installing_first_line(&gh_name)),
                        pty_id: None,
                    })
                }) as Rc<dyn Fn()>
            };
            let outcome: Result<(), HostError> = async {
                if let Some(said) = me.gh_first(&target_task, &ask_task, gh_step).await? {
                    step(SignInStep {
                        state: AgentsSignInEventState::Failed,
                        url: None,
                        code: None,
                        paste: None,
                        said: Some(said),
                        pty_id: None,
                    });
                    return Ok(());
                }
                plan(sign_in_run).await
            }
            .await;
            if let Err(e) = outcome {
                step(SignInStep {
                    state: AgentsSignInEventState::Failed,
                    url: None,
                    code: None,
                    paste: None,
                    said: Some(e.message.clone()),
                    pty_id: None,
                });
            }
            me.finish_sign_in(
                &sign_in_id_task,
                &key_task,
                &what_task,
                &target_task,
                &ask_task,
                &run_task,
                forward,
                &channel,
            )
            .await;
        });
        Ok((sign_in_id, run))
    }

    fn pty_link(
        &self,
        channel: &Rc<dyn wsp_core::daemon_channel::DaemonChannel>,
        readers: &Rc<RefCell<Vec<Rc<dyn Fn(&DaemonEvent)>>>>,
        target: &AgentsTarget,
    ) -> PtyLink {
        let op_channel = channel.clone();
        let closed_channel = channel.clone();
        let readers = readers.clone();
        PtyLink {
            op: Rc::new(move |op: String, mut extra: Map<String, Value>| {
                let channel = op_channel.clone();
                async move {
                    extra.insert("op".to_string(), Value::String(op));
                    let frame: Frame = extra;
                    channel
                        .send(frame)
                        .await
                        .map_err(|e| HostError::new(e.to_string()))
                }
                .boxed_local()
            }),
            on_event: Rc::new(move |read: Rc<dyn Fn(&DaemonEvent)>| {
                readers.borrow_mut().push(read.clone());
                let readers = readers.clone();
                Box::new(move || readers.borrow_mut().retain(|held| !Rc::ptr_eq(held, &read)))
            }),
            closed: Some(
                async move {
                    closed_channel.closed().await;
                }
                .boxed_local()
                .shared(),
            ),
            bare: Some(pty_bare_on(target)),
        }
    }

    /// agents-read.ts:544-566 (N7): the end of a run, in its order.
    #[allow(clippy::too_many_arguments)]
    async fn finish_sign_in(
        &self,
        sign_in_id: &str,
        key: &str,
        what: &str,
        target: &AgentsTarget,
        ask: &SignInAsk,
        run: &Rc<Running>,
        forward: Option<Rc<dyn SignInForward>>,
        channel: &Rc<dyn wsp_core::daemon_channel::DaemonChannel>,
    ) {
        if let Some(forward) = &forward {
            forward.close();
        }
        self.running.borrow_mut().shift_remove(sign_in_id);
        self.prune_ended();
        let listed = listed_run(sign_in_id, run);
        let now = self.o.clock.now();
        self.ended.borrow_mut().insert(
            sign_in_id.to_string(),
            (
                AgentsSignInRun {
                    ended: Some(True),
                    ..listed
                },
                now,
            ),
        );
        let is_ours = self
            .starting
            .borrow()
            .get(key)
            .and_then(|held| held.clone().now_or_never())
            .and_then(|r| r.ok())
            .is_some_and(|(id, _)| id == sign_in_id);
        if is_ours {
            self.starting.borrow_mut().shift_remove(key);
        }
        channel.close();
        let stopped = run.stopped_by.get();
        let last = run.last.borrow().clone();
        let outcome = if stopped {
            "stopped".to_string()
        } else {
            last.as_ref()
                .map(|e| state_word(e.state).to_string())
                .unwrap_or_else(|| "failed".to_string())
        };
        self.log_line(format!("sign-in {sign_in_id} ended: {what}: {outcome}"));
        if let AgentsTarget::V0(place) = target {
            if ask.server.is_none()
                && !stopped
                && last
                    .as_ref()
                    .is_some_and(|e| e.state == AgentsSignInEventState::SignedIn)
            {
                if let Some(door) = (self.o.places)() {
                    let landed = if place.place_id == wsp_wire::types::place_word::HERE_PLACE_ID
                        && mints_here(&ask.agent)
                    {
                        door.key_landed(&ask.agent).await
                    } else {
                        door.login_landed(&place.place_id, &ask.agent).await
                    };
                    if let Err(e) = landed {
                        self.log_line(format!(
                            "sign-in {sign_in_id}: the landed login was not noted: {}",
                            e.message
                        ));
                    }
                }
            }
        }
        if let Some(reader) = self.o.reader.clone() {
            reader.forget(&serde_json::to_string(target).unwrap_or_default());
        }
        self.changed(Some(target));
    }

    fn log_line(&self, line: String) {
        match &self.o.log {
            Some(log) => log(&line),
            None => (self.o.warn)(&line),
        }
    }

    /// agents-read.ts:314-321 and 528-533: the forward of the first relay that holds one for the target.
    fn forward_of(&self, target: &AgentsTarget) -> Option<Rc<dyn SignInForward>> {
        let relays = self.relays.borrow().clone();
        relays.iter().find_map(|relay| relay.open(target))
    }

    /// agents-read.ts:578-581 (N9).
    pub fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        self.prune_ended();
        let running: Vec<AgentsSignInRun> = self
            .running
            .borrow()
            .iter()
            .map(|(id, run)| listed_run(id, run))
            .collect();
        let ended: Vec<AgentsSignInRun> = self
            .ended
            .borrow()
            .values()
            .map(|(run, _)| run.clone())
            .collect();
        running.into_iter().chain(ended).collect()
    }

    /// agents-read.ts:582-586 (N10).
    pub fn sign_in_follow(&self, sign_in_id: &str, emit: Follower) -> Result<Following, HostError> {
        let run = self
            .running
            .borrow()
            .get(sign_in_id)
            .cloned()
            .ok_or_else(no_sign_in)?;
        Ok(self.follow(sign_in_id, &run, emit))
    }

    /// agents-read.ts:587-591 (N10).
    pub async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        let writer: Option<CodeWriter> = self
            .running
            .borrow()
            .get(sign_in_id)
            .and_then(|run| run.r#type.borrow().clone());
        let Some(writer) = writer else {
            return Err(no_sign_in());
        };
        writer(code.to_string()).await
    }

    /// agents-read.ts:592-596 (N8, N10).
    pub fn sign_in_stop(&self, sign_in_id: &str) -> Result<(), HostError> {
        let run = self
            .running
            .borrow()
            .get(sign_in_id)
            .cloned()
            .ok_or_else(no_sign_in)?;
        run.stopped_by.set(true);
        (run.stop)();
        Ok(())
    }

    /// agents-read.ts:597-605 (N11).
    pub async fn sign_in_line(
        &self,
        target: &AgentsTarget,
        ask: &SignInAsk,
        origin: Option<&Caller>,
    ) -> Result<SignInLine, HostError> {
        let acts = self.acts()?;
        let on = match self.on_of(target, origin, false).await? {
            OnOrNapping::On(on) => on,
            OnOrNapping::Napping(n) => {
                return Err(usage(napping_sign_in_refusal(
                    &n.napping,
                    n.pause_mode.as_ref(),
                )))
            }
        };
        let failed = self.gh_first(target, ask, Rc::new(|| {})).await?;
        let gh = catalog_entry(GITHUB_CLI)
            .map(|e| e.name().to_string())
            .unwrap_or_else(|| GITHUB_CLI.to_string());
        if let Some(failed) = failed {
            return Err(refusal(
                &tool_not_installed_line(&gh, &failed),
                &tool_not_installed_fix(&gh),
                None,
            ));
        }
        acts.sign_in_line(&on, ask).await
    }
}

/// agents-read.ts:756: whether the agent's catalog sign-in mints a token the vault keeps.
fn mints_here(agent: &str) -> bool {
    match catalog_entry(agent) {
        Some(CatalogEntry::Agent(entry)) => mints_token(&entry.sign_in).is_some(),
        _ => false,
    }
}
