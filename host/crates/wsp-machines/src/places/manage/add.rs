// SPDX-License-Identifier: AGPL-3.0-only
//! manage.ts:197-423 at 65f6b89c7: add, dial, road, exec, adds, reportOf, homeOf. Spec: `spec/places_manage_add.md`
//! (rules A1-A15, D1-D4, RD1-RD2, E1-E3, AD1-AD2).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use base64::Engine as _;
use futures::channel::oneshot;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use serde_json::{json, Map, Value};
use wsp_core::clock::Cancel as ClockCancel;
use wsp_core::ids::{Ids, OsIds};
use wsp_engine::machine::ExecResult;
use wsp_store::keys::key_fingerprint;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{
    PendingComputer, PendingComputerFailed, PendingStep, PlaceAddJob, PlaceAddJobState,
    PlaceAddStep, PlaceBack, PlaceDialled, PlaceStageEvent, PlaceStageEventState,
};
use wsp_wire::views::place_more::PlaceDial;
use wsp_wire::wire::devices::{join_token, PAIR_CODE_TTL_MS};
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_wire::wire::place_link::{
    place_no_daemon_port_line, place_no_link_line, PlaceReport, NO_PLACE_INSTALLER,
};
use wsp_words::exit::{
    kept_said, marked_cut, refusal_parts, usage_refusal, HostError, Kind, SAID_LINE_CHARS,
};
use wsp_words::views::place_setup_words::{pending_held_fix, pending_held_line};
use wsp_words::workspace_state::{
    absent_computer, linked_over, place_dial_line, place_dial_road, place_no_dial_line, Dialled,
    PlaceDialRoad, PlaceRoad,
};

use super::{ManageDoor, SetupPicks};
use crate::places::context::PlaceDoorContext;
use crate::places::helpers::{
    bounded, box_said, pending_view, PendingRecord, ADD_RUNNING_FIX, ADD_RUNNING_LINE, JOIN_WAIT_MS,
};
use crate::places::records::{AwaitingInstall, PlaceRecordsArea};
use crate::places::setup::iso_at;
use crate::places::types::{
    is_place_add_taken_back, AddAsk, BeforeDeploy, PlaceAdded, PlaceExecOpts, PlaceInstallRequest,
    PlaceInstalled, PlaceInstaller, PlaceLogin, PlaceRecord, PlaceResult, PlaceStaging, StageState,
};
use crate::places::views::PlaceViewsArea;

/// The pending adds the manage door writes, serialized in call order: each write waits for the one before it, and a
/// failed write is swallowed (manage.ts:222-227).
type PendingWrite = Shared<LocalBoxFuture<'static, ()>>;

/// What an add keeps while it runs, shared with the install's stage and deploy callbacks.
struct AddState {
    views: Rc<PlaceViewsArea>,
    records: Rc<PlaceRecordsArea>,
    ctx: Rc<PlaceDoorContext>,
    add_id: String,
    code: RefCell<String>,
    at: u64,
    waiting: Rc<RefCell<AwaitingInstall>>,
    installing: Cell<bool>,
    pending: RefCell<PendingRecord>,
    writes: RefCell<Option<PendingWrite>>,
    step: Cell<PlaceAddStep>,
    failed_said: Cell<Option<PlaceAddStep>>,
}

impl AddState {
    fn move_to(&self, next: PendingRecord) -> PendingWrite {
        *self.pending.borrow_mut() = next.clone();
        let prev = self.writes.borrow_mut().take();
        let views = Rc::clone(&self.views);
        let write: PendingWrite = async move {
            if let Some(prev) = prev {
                prev.await;
            }
            let _ = views.put_pending(&next).await;
        }
        .boxed_local()
        .shared();
        spawn_local_write(write.clone());
        *self.writes.borrow_mut() = Some(write.clone());
        write
    }

    async fn settled(&self) {
        let last = self.writes.borrow().clone();
        if let Some(last) = last {
            last.await;
        }
    }

    /// One step of the add as it moves, folded into the job and handed to `on_stage` (manage.ts:231-238).
    fn stage(
        &self,
        which: PlaceAddStep,
        state: StageState,
        note: Option<String>,
        place_id: Option<String>,
        ms: Option<u64>,
    ) {
        if state == StageState::Running {
            self.step.set(which);
        }
        if state == StageState::Failed {
            self.failed_said.set(Some(which));
        }
        if state == StageState::Running && which == PlaceAddStep::Check {
            let mut next = self.pending.borrow().clone();
            next.view.step = PendingStep::Check;
            drop(self.move_to(next));
        }
        let event = PlaceStageEvent {
            add_id: self.add_id.clone(),
            step: which,
            state: match state {
                StageState::Running => PlaceStageEventState::Running,
                StageState::Done => PlaceStageEventState::Done,
                StageState::Failed => PlaceStageEventState::Failed,
            },
            note,
            place_id,
            ms: ms.map(|ms| JsInt(JsNumber(ms as f64))),
        };
        self.records.put_add(&self.add_id, |job| {
            wsp_wire::views::place::with_place_stage(&job, &event)
        });
        if let Some(on_stage) = &self.ctx.opts.on_stage {
            on_stage(&event);
        }
    }
}

fn spawn_local_write(write: PendingWrite) {
    crate::local_task::spawn(write);
}

/// The road back the install hands the add before any byte of wsp's is sent (manage.ts:255).
struct DeployMark {
    state: Rc<AddState>,
}

#[async_trait(?Send)]
impl BeforeDeploy for DeployMark {
    async fn before_deploy(
        &self,
        undo: String,
        ssh: String,
        host_key: Option<String>,
    ) -> PlaceResult<()> {
        let mut next = self.state.pending.borrow().clone();
        next.view.step = PendingStep::Wsp;
        next.undo = Some(undo);
        next.login = Some(ssh);
        if let Some(host_key) = host_key {
            next.host_key = Some(host_key);
        }
        self.state.move_to(next).await;
        Ok(())
    }
}

/// The picks an add with no choices of its own is planned from: its name, as `RecipeFile.parse({ name })` reads it.
fn picks_named(name: &str) -> Result<RecipeFile, HostError> {
    serde_json::from_value(json!({ "name": name })).map_err(|e| HostError::new(e.to_string()))
}

/// The one picks an add keeps for the rest of its life (manage.ts:218).
fn port_of(ssh_port: Option<u16>) -> Option<JsInt> {
    ssh_port.map(|port| JsInt(JsNumber(f64::from(port))))
}

fn job_running(
    add_id: &str,
    address: &str,
    ssh_port: Option<u16>,
    started_at: &str,
) -> Result<PlaceAddJob, HostError> {
    let mut job = json!({ "addId": add_id, "address": address, "startedAt": started_at, "state": "running", "steps": [] });
    if let (Some(port), Some(map)) = (ssh_port, job.as_object_mut()) {
        map.insert("sshPort".to_string(), json!(port));
    }
    serde_json::from_value(job).map_err(|e| HostError::new(e.to_string()))
}

impl ManageDoor {
    /// A1-A15 (:198-342).
    pub async fn add(self: &Rc<Self>, req: AddAsk, at: u64) -> PlaceResult<PlaceAdded> {
        let Some(install) = self.ctx.wiring.install.clone() else {
            return Err(HostError::new(NO_PLACE_INSTALLER));
        };
        let add_id = req
            .add_id
            .clone()
            .unwrap_or_else(|| format!("a_{}", hex::encode(OsIds.bytes(6))));
        if self
            .records
            .adds
            .borrow()
            .get(&add_id)
            .is_some_and(|job| job.state == PlaceAddJobState::Running)
        {
            return Err(usage_refusal(ADD_RUNNING_LINE, ADD_RUNNING_FIX));
        }
        let before: Vec<PendingRecord> = self
            .views
            .pending_records()
            .await?
            .into_iter()
            .filter(|p| p.view.address == req.address)
            .collect();
        if let Some(standing) = before.iter().find(|p| p.view.failed.is_none()) {
            let name = standing
                .view
                .name
                .clone()
                .unwrap_or_else(|| req.address.clone());
            return Err(usage_refusal(
                &pending_held_line(&req.address, &standing.view.step),
                &pending_held_fix(&name),
            ));
        }
        for p in &before {
            self.views.drop_pending(&p.view.id).await?;
        }
        let started_at = iso_at(at);
        let choices = match &req.choices {
            Some(choices) => choices.clone(),
            None => picks_named(req.name.as_deref().unwrap_or(&req.address))?,
        };
        let first = PendingRecord {
            view: PendingComputer {
                id: add_id.clone(),
                address: req.address.clone(),
                ssh_port: port_of(req.ssh_port),
                name: req.name.clone(),
                place_id: None,
                step: PendingStep::Connect,
                choices,
                recipe: req.recipe.clone(),
                started_at: started_at.clone(),
                failed: None,
            },
            key_path: req.key_path.clone(),
            host_key: req.host_key.clone(),
            undo: None,
            login: None,
        };
        let state = Rc::new(AddState {
            views: Rc::clone(&self.views),
            records: Rc::clone(&self.records),
            ctx: Rc::clone(&self.ctx),
            add_id: add_id.clone(),
            code: RefCell::new(String::new()),
            at,
            waiting: Rc::new(RefCell::new(AwaitingInstall::default())),
            installing: Cell::new(true),
            pending: RefCell::new(first.clone()),
            writes: RefCell::new(None),
            step: Cell::new(PlaceAddStep::Connect),
            failed_said: Cell::new(None),
        });
        state.move_to(first).await;
        let issued = self
            .ctx
            .devices
            .issue(at as i64, PAIR_CODE_TTL_MS as u64, false)
            .await?;
        let code = issued.code;
        *state.code.borrow_mut() = code.clone();
        self.records.adds.borrow_mut().shift_remove(&add_id);
        let job = job_running(&add_id, &req.address, req.ssh_port, &started_at)?;
        self.records.adds.borrow_mut().insert(add_id.clone(), job);
        self.records
            .awaiting
            .borrow_mut()
            .insert(code.clone(), Rc::clone(&state.waiting));
        let outcome = self.add_steps(&req, install, &state).await;
        let answer = match outcome {
            Ok(answer) => Ok(answer),
            Err(e) => self.add_failed(&state, e).await,
        };
        self.records.awaiting.borrow_mut().shift_remove(&code);
        answer
    }

    /// The install and the join behind it, from the install's own ask to the answer (manage.ts:238-310).
    async fn add_steps(
        self: &Rc<Self>,
        req: &AddAsk,
        install: Rc<dyn PlaceInstaller>,
        state: &Rc<AddState>,
    ) -> PlaceResult<PlaceAdded> {
        state.settled().await;
        let held: Vec<String> = self
            .records
            .records()
            .await?
            .into_iter()
            .map(|r| r.id)
            .collect();
        let deploy: Rc<dyn BeforeDeploy> = Rc::new(DeployMark {
            state: Rc::clone(state),
        });
        let staging: PlaceStaging = {
            let state = Rc::clone(state);
            Rc::new(move |which, st, note, place, ms| state.stage(which, st, note, place, ms))
        };
        let request = PlaceInstallRequest {
            address: req.address.clone(),
            name: req.name.clone(),
            ssh_port: req.ssh_port,
            key_path: req.key_path.clone(),
            host_key: req.host_key.clone(),
            sudo_password: req.sudo_password.clone(),
            code: join_token(
                &state.code.borrow(),
                &key_fingerprint(&self.ctx.wiring.host_key.public_key),
            ),
            host_urls: req.host_urls.clone(),
            door_port: req.door_port,
            relay: req.relay.clone(),
            held: Some(held),
            before_deploy: Some(deploy),
        };
        let installed = install.install(request, staging).await?;
        state.installing.set(false);
        let login = installed.ssh.clone().map(|ssh| PlaceLogin {
            ssh,
            key_path: installed.ssh_key_path.clone(),
            host_key: installed.host_key.clone(),
        });
        {
            let mut entry = state.waiting.borrow_mut();
            entry.login = login.clone();
            if let Some(back) = installed.back.clone() {
                entry.back = Some(back);
            }
        }
        let known_place = state.waiting.borrow().place_id.clone();
        if let (Some(_), Some(place_id)) = (login.as_ref(), known_place.as_ref()) {
            self.records.change(place_id, Some).await?;
        }
        let joining = self.ctx.clock.now() as u64;
        state.stage(PlaceAddStep::Join, StageState::Running, None, None, None);
        let place_id = self.await_join(&state.waiting, &installed).await?;
        let Some(held) = self.records.record_of(&place_id).await? else {
            return Err(HostError::new(place_no_link_line(&installed.name)));
        };
        self.records.hold_back(&held);
        let back: Option<PlaceBack> = held.road.as_ref().and_then(|r| r.back.clone());
        let note =
            linked_over(&held.report.dialed, back.as_ref(), &req.host_urls).unwrap_or_default();
        let took = (self.ctx.clock.now() as u64).saturating_sub(joining);
        state.stage(
            PlaceAddStep::Join,
            StageState::Done,
            Some(note),
            Some(place_id.clone()),
            Some(took),
        );
        let facts = self.views.facts_on(&place_id, &held).await;
        state.settled().await;
        let host_key = installed.host_key.clone();
        let default = self.records.default_id().await?;
        let Some(choices) = req.choices.clone() else {
            let mut joined = state.pending.borrow().clone();
            joined.view.place_id = Some(place_id.clone());
            joined.view.step = PendingStep::Floor;
            state.move_to(joined.clone()).await;
            let views = Rc::clone(&self.views);
            let (place, floor) = (place_id.clone(), joined.clone());
            crate::local_task::spawn(async move {
                let _ = views.floor_once(&floor, &place).await;
            });
            return Ok(PlaceAdded {
                add_id: state.add_id.clone(),
                place: self.views.view_of(&facts, default.as_deref()),
                host_key,
                said: None,
                pending: Some(pending_view(&joined)),
            });
        };
        self.views.drop_pending(&state.add_id).await?;
        let picks = SetupPicks {
            picks: choices,
            recipe: req.recipe.clone(),
        };
        let started = self
            .setup
            .started_or_said(&place_id, &state.add_id, picks)
            .await?;
        let row = self.records.record_of(&place_id).await?.unwrap_or(facts);
        Ok(PlaceAdded {
            add_id: state.add_id.clone(),
            place: self.views.view_of(&row, default.as_deref()),
            host_key,
            said: started.said,
            pending: None,
        })
    }

    /// A15 (:311-341): what a failure after the install's ask leaves behind, then the failure itself.
    async fn add_failed(
        self: &Rc<Self>,
        state: &Rc<AddState>,
        e: HostError,
    ) -> PlaceResult<PlaceAdded> {
        let parts = refusal_parts(&e);
        let (said, fix, kind) = (
            kept_said(&parts.said),
            parts.fix.clone(),
            parts.kind.clone(),
        );
        self.records.put_add(&state.add_id, |job| PlaceAddJob {
            said: Some(said.clone()),
            fix: fix.clone(),
            kind: kind.clone(),
            ..job
        });
        if state.pending.borrow().view.place_id.is_none() {
            let mut next = state.pending.borrow().clone();
            next.view.failed = Some(PendingComputerFailed {
                said: kept_said(&parts.said),
                fix: parts.fix.clone(),
            });
            state.move_to(next).await;
        }
        if state.failed_said.get() != Some(state.step.get()) {
            let line = e.message.split('\n').next().unwrap_or_default().to_string();
            state.stage(
                state.step.get(),
                StageState::Failed,
                Some(marked_cut(&line, SAID_LINE_CHARS)),
                None,
                None,
            );
        }
        let code = state.code.borrow().clone();
        let _ = self.ctx.devices.spend(&code, state.at as i64).await;
        let joined = state.waiting.borrow().place_id.clone();
        if let (true, Some(place_id)) = (state.installing.get(), joined.as_ref()) {
            if is_place_add_taken_back(&e) {
                self.views.forget(place_id).await?;
            }
        }
        let (login, back) = (
            state.waiting.borrow().login.clone(),
            state.waiting.borrow().back.clone(),
        );
        if let (Some(login), Some(_)) = (login, back) {
            let landed = match joined.as_ref() {
                Some(place_id) => self.records.record_of(place_id).await?,
                None => None,
            };
            match landed {
                Some(landed) if landed.road.as_ref().and_then(|r| r.back.clone()).is_some() => {
                    self.records.hold_back(&landed)
                }
                _ => {
                    if let Some(holder) = self.ctx.wiring.back.clone() {
                        holder.release(&login);
                    }
                }
            }
        }
        Err(e)
    }

    /// Waits for the computer's own join to name its place: the link may be up already, else the attach wakes this,
    /// and the wait running out fails with the box's own words beside the line (manage.ts:268-285).
    async fn await_join(
        self: &Rc<Self>,
        waiting: &Rc<RefCell<AwaitingInstall>>,
        installed: &PlaceInstalled,
    ) -> PlaceResult<String> {
        let standing = waiting.borrow().place_id.clone();
        if let Some(place_id) = standing.filter(|id| self.ctx.live.borrow().contains_key(id)) {
            return Ok(place_id);
        }
        let (tx, rx) = oneshot::channel::<Result<String, HostError>>();
        let tx = Rc::new(RefCell::new(Some(tx)));
        let timer: Rc<RefCell<Option<ClockCancel>>> = Rc::new(RefCell::new(None));
        let timer_tx = Rc::clone(&tx);
        let timer_name = installed.name.clone();
        let cancel = self.ctx.clock.schedule(
            Box::new(move || {
                let sender = timer_tx.borrow_mut().take();
                if let Some(sender) = sender {
                    let _ = sender.send(Err(HostError::new(place_no_link_line(&timer_name))));
                }
            }),
            self.ctx.opts.join_wait_ms.unwrap_or(JOIN_WAIT_MS) as i64,
        );
        *timer.borrow_mut() = Some(cancel);
        let woken_tx = Rc::clone(&tx);
        let woken_timer = Rc::clone(&timer);
        waiting.borrow_mut().woken = Some(Rc::new(move |id: &str| {
            let stop = woken_timer.borrow_mut().take();
            if let Some(stop) = stop {
                stop();
            }
            let sender = woken_tx.borrow_mut().take();
            if let Some(sender) = sender {
                let _ = sender.send(Ok(id.to_string()));
            }
        }));
        let answer = rx
            .await
            .unwrap_or_else(|_| Err(HostError::new(place_no_link_line(&installed.name))));
        match answer {
            Ok(place_id) => Ok(place_id),
            Err(e) => {
                let mut lines = vec![e.message.clone()];
                lines.extend(box_said(&self.ctx.wiring, installed).await);
                Err(HostError::new(lines.join("\n")))
            }
        }
    }

    /// D1-D4 (:344-382).
    pub async fn dial(&self, place_id: &str, at: u64) -> PlaceResult<PlaceDial> {
        let Some(held) = self.records.record_of(place_id).await? else {
            let names: Vec<String> = self
                .ctx
                .kept
                .borrow()
                .values()
                .map(|r| r.name.clone())
                .collect();
            return Err(HostError::new(no_such_place_refusal(place_id, &names)));
        };
        let stamp = iso_at(at);
        let linked = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .map(|held| Rc::clone(&held.reach));
        let started = self.ctx.clock.now();
        let words_road = held.road.as_ref().map(|r| PlaceRoad {
            ssh: r.ssh.clone(),
            from: r.from.clone(),
            back: r.back.clone(),
        });
        let road = place_dial_road(Some(linked.is_some()), words_road.as_ref());
        let outcome: Result<(), String> = match (road, linked.clone()) {
            (Some(PlaceDialRoad::Link), Some(reach)) => {
                let ping = async move {
                    reach
                        .request("ping", Map::new())
                        .await
                        .map(|_| ())
                        .map_err(|e| HostError::new(e.message))
                };
                bounded(
                    &*self.ctx.clock,
                    ping,
                    self.ctx.dial_wait_ms,
                    &format!("ping on {}", held.name),
                )
                .await
                .map_err(|e| e.message)
            }
            (Some(PlaceDialRoad::Ssh), _) => match (
                PlaceRecordsArea::login_of(&held),
                self.ctx.wiring.dial.clone(),
            ) {
                (Some(login), Some(dialer)) => {
                    let what = format!("ssh {}", login.ssh);
                    let dial = async move { dialer.dial(&login).await };
                    bounded(&*self.ctx.clock, dial, self.ctx.dial_wait_ms, &what)
                        .await
                        .map_err(|e| e.message)
                }
                _ => Err(place_no_dial_line(&held.name)),
            },
            _ => Err(place_no_dial_line(&held.name)),
        };
        let round_trip = (self.ctx.clock.now() - started).max(0) as u64;
        let dialled = match outcome {
            Ok(()) => PlaceDialled {
                at: stamp.clone(),
                answered: true,
                round_trip_ms: Some(JsInt(JsNumber(round_trip as f64))),
                said: None,
            },
            Err(said) => PlaceDialled {
                at: stamp.clone(),
                answered: false,
                round_trip_ms: None,
                said: Some(said),
            },
        };
        let seen = dialled.answered && linked.is_some();
        let (write_dialled, write_stamp) = (dialled.clone(), stamp.clone());
        let moved = self
            .records
            .change(place_id, move |now: PlaceRecord| {
                let last_seen_at = if seen {
                    write_stamp
                } else {
                    now.last_seen_at.clone()
                };
                Some(PlaceRecord {
                    dialled: Some(write_dialled),
                    last_seen_at,
                    ..now
                })
            })
            .await?;
        let written = moved.unwrap_or_else(|| PlaceRecord {
            dialled: Some(dialled.clone()),
            last_seen_at: if seen {
                stamp.clone()
            } else {
                held.last_seen_at.clone()
            },
            ..held.clone()
        });
        let words_dialled = Dialled {
            answered: dialled.answered,
            round_trip_ms: dialled.round_trip_ms.map(|ms| ms.as_f64()),
            said: dialled.said.clone(),
        };
        let line = place_dial_line(
            &held.name,
            words_road.as_ref(),
            linked.is_some(),
            &words_dialled,
        );
        let default = self.records.default_id().await?;
        let view = self.views.view_of(&written, default.as_deref());
        let ids = self.records.row_ids().await?;
        let place = self.records.with_cap(view, &ids).await?;
        Ok(PlaceDial {
            dialled,
            line,
            place,
        })
    }

    /// RD1-RD2 (:384-398).
    pub async fn road(&self, place_id: &str) -> PlaceResult<u16> {
        let held = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .map(|held| (Rc::clone(&held.reach), Rc::clone(&held.socket)));
        let name = self
            .records
            .record_of(place_id)
            .await?
            .map(|r| r.name)
            .unwrap_or_else(|| place_id.to_string());
        let Some((reach, _socket)) = held else {
            return Err(HostError::new(absent_computer(&name, None).sentence));
        };
        let port = self
            .records
            .record_of(place_id)
            .await?
            .and_then(|r| r.report.daemon_port.map(|p| p.as_f64() as u16));
        let Some(port) = port else {
            return Err(HostError::new(place_no_daemon_port_line(&name)));
        };
        let opened = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .filter(|live| Rc::ptr_eq(&live.reach, &reach))
            .and_then(|live| live.forward.clone());
        let forward = match opened {
            Some(forward) => forward,
            None => {
                let forward = wsp_core::place_forward::open_place_forward(Rc::clone(&reach), port)
                    .map(|opened| opened.map(Rc::new))
                    .boxed_local()
                    .shared();
                if let Some(live) = self
                    .ctx
                    .live
                    .borrow_mut()
                    .get_mut(place_id)
                    .filter(|live| Rc::ptr_eq(&live.reach, &reach))
                {
                    live.forward = Some(forward.clone());
                }
                forward
            }
        };
        match forward.await {
            Ok(forward) => Ok(forward.port),
            Err(e) => {
                let same = self
                    .ctx
                    .live
                    .borrow()
                    .get(place_id)
                    .is_some_and(|live| Rc::ptr_eq(&live.reach, &reach));
                if same {
                    if let Some(live) = self.ctx.live.borrow_mut().get_mut(place_id) {
                        live.forward = None;
                    }
                }
                Err(e)
            }
        }
    }

    /// E1-E3 (:400-418).
    pub async fn exec(
        &self,
        place_id: &str,
        cmd: &str,
        opts: PlaceExecOpts,
    ) -> PlaceResult<ExecResult> {
        exec_over(&self.ctx, &self.records, place_id, cmd, opts).await
    }

    /// AD1 (:420).
    pub fn adds(&self) -> Vec<PlaceAddJob> {
        self.records.adds.borrow().values().cloned().collect()
    }

    /// AD2 (:422).
    pub async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> {
        Ok(self.records.record_of(place_id).await?.map(|r| r.report))
    }

    /// AD2 (:423).
    pub async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> {
        Ok(self
            .records
            .record_of(place_id)
            .await?
            .and_then(|r| r.report.login.get("HOME").cloned()))
    }
}

/// One command on a place over its live link (manage.ts:400-418), read off the context and the records alone so a
/// reader that outlives the door can run it.
pub(super) async fn exec_over(
    ctx: &PlaceDoorContext,
    records: &PlaceRecordsArea,
    place_id: &str,
    cmd: &str,
    opts: PlaceExecOpts,
) -> PlaceResult<ExecResult> {
    let held = ctx
        .live
        .borrow()
        .get(place_id)
        .map(|held| Rc::clone(&held.reach));
    let absent = |name: String| {
        HostError::with_kind(absent_computer(&name, None).sentence, Kind::parse("absent"))
    };
    let Some(reach) = held else {
        return Err(absent(name_or_id(records, place_id).await?));
    };
    let mut frame = Map::new();
    frame.insert("cmd".to_string(), Value::String(cmd.to_string()));
    if let Some(timeout) = opts.timeout_ms {
        frame.insert("timeoutMs".to_string(), json!(timeout));
    }
    if let Some(stdin) = &opts.stdin {
        frame.insert(
            "stdin".to_string(),
            Value::String(base64::engine::general_purpose::STANDARD.encode(stdin)),
        );
    }
    match reach.request("exec", frame).await {
        Ok(answer) => {
            let code = answer
                .get("exitCode")
                .and_then(Value::as_f64)
                .unwrap_or(-1.0);
            let stdout = answer
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let stderr = answer
                .get("stderr")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            serde_json::from_value(json!({ "exitCode": code, "stdout": stdout, "stderr": stderr }))
                .map_err(|e| HostError::new(e.to_string()))
        }
        Err(e) => {
            let still_open = ctx
                .live
                .borrow()
                .get(place_id)
                .map(|now| (Rc::clone(&now.reach), now.socket.is_open()));
            match still_open {
                Some((now, true)) if Rc::ptr_eq(&now, &reach) => Err(HostError::new(e.message)),
                _ => Err(absent(name_or_id(records, place_id).await?)),
            }
        }
    }
}

/// A place's name as the record holds it, or its id where no record does.
pub(super) async fn name_or_id(records: &PlaceRecordsArea, place_id: &str) -> PlaceResult<String> {
    Ok(records
        .record_of(place_id)
        .await?
        .map(|r| r.name)
        .unwrap_or_else(|| place_id.to_string()))
}
