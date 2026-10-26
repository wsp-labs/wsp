// SPDX-License-Identifier: AGPL-3.0-only
//! sub-spawn.ts:32-74, 283-341, 414-621: the machine of a spawn and of a resume. Spec: spec/sub_spawn_machine.md.
#![allow(clippy::result_large_err)]
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::json;

use wsp_catalog::git_hosts::{git_host_of, remote_host};
use wsp_core::types::wiring::{Joined, StageReport, StageSaid, WorkspaceRecord};
use wsp_engine::errors::EngineError;
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::sub_row::{
    SubSpawn, SubSpawnFailed, SubSpawnFailedStep, SubSpawnFiles, SubSpawnMachine,
};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace_events::WorkspaceCreateStage;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::{refusal_said, HostError};
use wsp_words::js::js_trim;
use wsp_words::js::utf16::tail;
use wsp_words::place_word::{names_place, provider_key_name, PlaceRef};
use wsp_words::sub_thread::{
    cloud_said, sub_cloud_gone_refusal, sub_copying_line, sub_next_size, sub_not_sent_line,
    sub_oom_line, sub_other_host_refusal, sub_replaced_line, sub_resume_preamble, sub_size_word,
    sub_stayed_line, sub_twice_refusal, BaseAt, CpuMem, SubFault, SubResumePreamble, SubSize,
};
use wsp_words::words::thread::{starting_line, CREATE_READY};

use super::checks::ImageAt;
use super::{Seed, SubSpawnArea};
use crate::machines::area_error::{AreaError, AreaResult};
use crate::machines::machines::Entry;
use crate::machines::sub_life::{sub_session_land_command, SessionFile, SUB_SESSIONS};

/// sub-spawn.ts:33 (rule 1).
pub fn fault_of(e: &AreaError) -> Option<SubFault> {
    match e {
        AreaError::Engine(EngineError::GuestUnusable {
            machine_id, detail, ..
        }) => Some(SubFault::Unusable {
            r#box: machine_id.clone(),
            why: detail.clone(),
            request_id: None,
        }),
        AreaError::Engine(EngineError::EnvNotLanded {
            machine_id,
            exit_code: 124,
            ..
        }) => Some(SubFault::Env {
            r#box: machine_id.clone(),
        }),
        AreaError::Engine(EngineError::CreateCancelled {
            machine_id, why, ..
        }) => Some(SubFault::Cancelled {
            r#box: machine_id.clone(),
            why: if why.is_empty() {
                "no reason given".to_string()
            } else {
                why.clone()
            },
        }),
        _ => None,
    }
}

/// What `resume_sub` answers (sub-spawn.ts:605): the live entry now, the head its send opens with, the notices.
pub struct Resumed {
    pub entry: Entry,
    pub preamble: Option<String>,
    pub notices: Vec<String>,
}

fn sub_spawn_machine(since: f64) -> SubSpawnMachine {
    SubSpawnMachine {
        since: JsNumber(since),
        id: None,
        states: Vec::new(),
        ms: None,
    }
}

fn kind_word(kind: WorkspaceKind) -> &'static str {
    match kind {
        WorkspaceKind::Cloud => "cloud",
        WorkspaceKind::Local => "local",
        WorkspaceKind::Place => "place",
    }
}

fn record_key(record: &Rc<RefCell<WorkspaceRecord>>) -> (String, String) {
    let record = record.borrow();
    (record.id.clone(), record.name.clone())
}

impl SubSpawnArea {
    /// sub-spawn.ts:47 (rule 2).
    pub(crate) fn told(&self, record: &WorkspaceRecord) {
        for (thread_id, workspace_id) in self.ctx.thread_records() {
            if workspace_id == record.id {
                self.ctx.push_head(&thread_id);
                return;
            }
        }
    }

    /// sub-spawn.ts:51 (rule 2). `mv` changes the record's spawn; nothing without one.
    pub(crate) fn step(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        mv: &mut dyn FnMut(&mut SubSpawn),
    ) {
        let moved = {
            let mut held = record.borrow_mut();
            match held.sub.as_mut().and_then(|sub| sub.spawn.as_mut()) {
                Some(spawn) => {
                    mv(spawn);
                    true
                }
                None => false,
            }
        };
        if moved {
            self.told(&record.borrow());
        }
    }

    /// sub-spawn.ts:58 (rule 3).
    pub(crate) fn logged(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        report: StageReport,
    ) -> StageReport {
        let area = self
            .me
            .upgrade()
            .expect("a sub-spawn area is alive while its method runs");
        let record = Rc::clone(record);
        Rc::new(
            move |stage: WorkspaceCreateStage, message: &str, said: Option<StageSaid>| {
                report(stage, message, said.clone());
                area.step(&record, &mut |spawn: &mut SubSpawn| {
                    spawn.log.push(message.to_string());
                    let keep = spawn
                        .log
                        .len()
                        .saturating_sub(wsp_wire::views::sub_row::SUB_SPAWN_LOG_LINES as usize);
                    spawn.log.drain(..keep);
                    let Some(state) = said.as_ref().and_then(|said| said.r#box.as_ref()) else {
                        return;
                    };
                    let Some(machine) = spawn.machine.as_mut() else {
                        return;
                    };
                    machine.id = Some(state.id.clone());
                    if machine.states.last() != Some(&state.state) {
                        machine.states.push(state.state.clone());
                    }
                });
            },
        )
    }

    /// sub-spawn.ts:69 (rule 4).
    pub(crate) fn spawn_failed(&self, record: &Rc<RefCell<WorkspaceRecord>>, e: &AreaError) {
        let message = e.message();
        let fix = e.fix();
        self.step(record, &mut |spawn: &mut SubSpawn| {
            let step = if spawn.files.as_ref().is_some_and(|files| files.ms.is_none()) {
                SubSpawnFailedStep::Files
            } else {
                SubSpawnFailedStep::Machine
            };
            spawn.failed = Some(SubSpawnFailed {
                step,
                said: refusal_said(&message, fix.as_deref()),
                fix: fix.clone(),
            });
        });
    }

    /// sub-spawn.ts:285 (rule 5). Answers the replacement's lines.
    pub(crate) async fn machine_for(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        at: &Arc<dyn MachineBackend>,
        on: &str,
        seed: &Seed,
        report: StageReport,
        landed: Rc<dyn Fn()>,
    ) -> AreaResult<Vec<String>> {
        let (id, name) = record_key(record);
        let mut notices: Vec<String> = Vec::new();
        let mut first: Option<SubFault> = None;
        loop {
            let error = match self
                .stand_up(record, on, seed, report.clone(), landed.clone())
                .await
            {
                Ok(()) => return Ok(notices),
                Err(error) => error,
            };
            let fault = fault_of(&error);
            let cut = fault.as_ref().map(|fault| match fault {
                SubFault::Unusable { r#box, .. }
                | SubFault::Env { r#box }
                | SubFault::Cancelled { r#box, .. } => r#box.clone(),
            });
            self.drop_machine(at, record, cut.as_deref()).await;
            let Some(fault) = fault else {
                return Err(error);
            };
            if let Some(first) = &first {
                return Err(AreaError::Host(sub_twice_refusal(first, &fault, &name, on)));
            }
            let line = sub_replaced_line(&fault, &name, on);
            notices.push(line.clone());
            let replaced = line.clone();
            self.step(record, &mut |spawn: &mut SubSpawn| {
                spawn
                    .replaced
                    .get_or_insert_with(Vec::new)
                    .push(replaced.clone());
                spawn.files = None;
            });
            report(
                WorkspaceCreateStage::ForkRequested,
                &line,
                Some(StageSaid {
                    notice: Some(line.clone()),
                    ..StageSaid::default()
                }),
            );
            self.ctx.warn(&format!("workspace {id}: {line}"));
            self.ctx.delete_create(&format!("workspace/{id}")).await?;
            first = Some(fault);
        }
    }

    /// sub-spawn.ts:314 (rule 6).
    pub async fn stand_up_queued(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        posted: Rc<dyn Fn()>,
        waited: bool,
    ) -> AreaResult<()> {
        let (id, name, place_id, provider, lead, root_thread) = {
            let held = record.borrow();
            (
                held.id.clone(),
                held.name.clone(),
                self.ctx.place_id_of(&held),
                held.provider.clone(),
                held.lead
                    .clone()
                    .expect("a queued sub-thread names its lead"),
                held.root_thread_id.clone(),
            )
        };
        let rows = self.ctx.computer_rows().await?;
        let row = rows
            .into_iter()
            .find(|r| Some(&r.id) == place_id.as_ref() || Some(&r.id) == provider.as_ref());
        let on = match &row {
            Some(row) => provider_key_name(&row.name),
            None => record
                .borrow()
                .sub
                .as_ref()
                .map(|sub| sub.on.clone())
                .unwrap_or_default(),
        };
        let at = self.ctx.backend_for(&record.borrow())?;
        let project = self.ctx.project_held(&record.borrow().project);
        let seed = self.seed_of(&project).await?;
        let now = self.ctx.now();
        self.step(record, &mut |spawn: &mut SubSpawn| {
            if waited {
                spawn.started_at = JsNumber(now);
            }
            spawn.not_sent = seed.why.clone();
        });
        let asked_by = json!({
            "kind": "thread",
            "threadId": lead.thread_id,
            "workspaceId": lead.workspace_id,
            "rootThreadId": root_thread.unwrap_or_else(|| lead.thread_id.clone()),
        });
        let report = self.logged(
            record,
            self.ctx.stage_reporter(&id, &name, now, Some(asked_by)),
        );
        self.ctx.forking_add(&name);
        let run = self
            .machine_for(record, &at, &on, &seed, report.clone(), posted)
            .await;
        let outcome = match run {
            Ok(_) => Ok(()),
            Err(error) => {
                if !error.is_cap_refusal() {
                    self.spawn_failed(record, &error);
                    report(WorkspaceCreateStage::Failed, &error.message(), None);
                }
                Err(error)
            }
        };
        self.ctx.forking_remove(&name);
        outcome
    }

    /// sub-spawn.ts:416 (rule 7).
    pub(crate) async fn stand_up(
        &self,
        record: &Rc<RefCell<WorkspaceRecord>>,
        on: &str,
        seed: &Seed,
        report: StageReport,
        landed: Rc<dyn Fn()>,
    ) -> AreaResult<()> {
        let (id, name, folder) = {
            let held = record.borrow();
            (
                held.id.clone(),
                held.name.clone(),
                held.sub
                    .as_ref()
                    .map(|sub| sub.folder.clone())
                    .unwrap_or_default(),
            )
        };
        let now = self.ctx.now();
        self.step(record, &mut |spawn: &mut SubSpawn| {
            spawn.machine = Some(sub_spawn_machine(now));
        });
        report(
            WorkspaceCreateStage::ForkRequested,
            &starting_line(&name, on),
            None,
        );
        let ctx = Rc::clone(&self.ctx);
        let on_machine_record = Rc::clone(record);
        let on_machine: Box<dyn Fn(Arc<dyn Machine>)> =
            Box::new(move |machine: Arc<dyn Machine>| {
                {
                    let mut held = on_machine_record.borrow_mut();
                    held.machine_id = machine.id().to_string();
                    held.phase = WorkspacePhase::Running;
                }
                let entry = ctx.attach(&on_machine_record, machine);
                entry.borrow_mut().creating = true;
                landed();
            });
        self.ctx.fork(record, on_machine, report.clone()).await?;
        let entry = self
            .ctx
            .live(&id)
            .expect("a forked sub-thread stands in the live set");
        let (preview, daemon_answers) = {
            let offers = entry.borrow().machine.offers();
            (offers.preview_url, offers.daemon_answers)
        };
        if preview || daemon_answers {
            let fault = self.ctx.ping_daemon(&entry).await;
            let line = if fault.is_none() {
                "Daemon answered."
            } else {
                "Daemon did not answer."
            };
            report(
                WorkspaceCreateStage::DaemonAnswering,
                line,
                Some(StageSaid {
                    notice: fault,
                    ..StageSaid::default()
                }),
            );
            self.ctx.sync_daemon(&entry);
        }
        let machine = entry.borrow().machine.clone();
        let made = machine
            .exec(
                &format!("install -d -m 0755 {}", shell_quote(&folder)),
                ExecOptions::default(),
            )
            .await?;
        if made.exit_code != JsInt::from(0) {
            let said = if js_trim(&made.stderr).is_empty() {
                js_trim(&made.stdout)
            } else {
                js_trim(&made.stderr)
            };
            return Err(AreaError::Host(HostError::new(format!(
                "{folder} was not made on {} (exit {}): {said}",
                machine.id(),
                made.exit_code
            ))));
        }
        let made_at = self.ctx.now();
        self.step(record, &mut |spawn: &mut SubSpawn| {
            if let Some(machine) = spawn.machine.as_mut() {
                machine.ms = Some(JsInt(JsNumber(made_at - machine.since.0)));
            }
        });
        if let Some(tar) = seed.tar.as_ref() {
            let since = self.ctx.now();
            let paths = seed.paths.clone();
            let bytes = seed.bytes;
            self.step(record, &mut |spawn: &mut SubSpawn| {
                spawn.files = Some(SubSpawnFiles {
                    since: JsNumber(since),
                    paths: paths.clone(),
                    bytes: JsInt(JsNumber(bytes as f64)),
                    ms: None,
                });
            });
            if !machine.offers().put_bytes {
                return Err(AreaError::Host(HostError::new(format!(
                    "{on} takes no file onto {}, so the picked files cannot land",
                    machine.id()
                ))));
            }
            report(
                WorkspaceCreateStage::ProjectCloned,
                &sub_copying_line(seed.files as f64),
                None,
            );
            let at = format!("/tmp/wsp-seed-{id}.tgz");
            machine.put_bytes(&at, tar, None).await?;
            let unpacked = machine
                .exec(
                    &format!(
                        "tar -xzf {} -C {} && rm -f {}",
                        shell_quote(&at),
                        shell_quote(&folder),
                        shell_quote(&at)
                    ),
                    ExecOptions::default(),
                )
                .await?;
            if unpacked.exit_code != JsInt::from(0) {
                let said = if js_trim(&unpacked.stderr).is_empty() {
                    js_trim(&unpacked.stdout)
                } else {
                    js_trim(&unpacked.stderr)
                };
                return Err(AreaError::Host(HostError::new(format!(
                    "the picked files did not land in {folder} (exit {}): {said}",
                    unpacked.exit_code
                ))));
            }
            let landed_at = self.ctx.now();
            self.step(record, &mut |spawn: &mut SubSpawn| {
                if let Some(files) = spawn.files.as_mut() {
                    files.ms = Some(JsInt(JsNumber(landed_at - files.since.0)));
                }
            });
        }
        let ready = self.ctx.now();
        self.step(record, &mut |spawn: &mut SubSpawn| {
            spawn.ready_at = Some(JsNumber(ready));
            spawn.asked = None;
        });
        let snapshot = record.borrow().clone();
        self.ctx.persist(&snapshot).await?;
        entry.borrow_mut().creating = false;
        report(WorkspaceCreateStage::Ready, CREATE_READY, None);
        let released_at = record.borrow().sub.as_ref().and_then(|sub| sub.released_at);
        if released_at.is_none() {
            let view = self.ctx.view(&record.borrow());
            self.ctx
                .emit(json!({ "type": "workspace.created", "workspace": view }));
        } else {
            let (workspace_id, machine_id) = {
                let held = record.borrow();
                (held.id.clone(), held.machine_id.clone())
            };
            self.ctx.emit(json!({ "type": "workspace.woken", "workspaceId": workspace_id, "machineId": machine_id }));
            self.ctx.emit_status_reachable(&entry).await?;
        }
        Ok(())
    }

    /// sub-spawn.ts:476 (rule 8).
    pub(crate) async fn drop_machine(
        &self,
        at: &Arc<dyn MachineBackend>,
        record: &Rc<RefCell<WorkspaceRecord>>,
        r#box: Option<&str>,
    ) {
        let (id, _) = record_key(record);
        let entry = self.ctx.live(&id);
        let entry_machine = entry
            .as_ref()
            .map(|entry| entry.borrow().machine.id().to_string());
        if let Some(entry) = entry.as_ref() {
            if !self.ctx.is_absent_machine(entry) {
                if let Err(error) = self.ctx.unfork(entry).await {
                    if !error.is_missing() {
                        self.ctx.warn(&format!(
                            "workspace {id}: {} was not deleted: {}",
                            entry.borrow().machine.id(),
                            error.message()
                        ));
                    }
                }
                self.ctx.remove_live(&id);
            }
        }
        if let Some(box_id) = r#box {
            if entry_machine.as_deref() != Some(box_id) {
                let killed: Result<(), EngineError> = match at.get(box_id).await {
                    Ok(machine) => machine.kill().await,
                    Err(error) => Err(error),
                };
                if let Err(error) = killed {
                    let error = AreaError::Engine(error);
                    if !error.is_missing() {
                        self.ctx.warn(&format!(
                            "workspace {id}: {box_id} was not deleted: {}",
                            error.message()
                        ));
                    }
                }
            }
        }
        record.borrow_mut().machine_id = String::new();
    }

    /// sub-spawn.ts:501 (rule 9).
    pub async fn resume_sub(
        &self,
        asked: &Entry,
        thread_id: &str,
        harness: &str,
        prompt: Option<&str>,
    ) -> Result<Resumed, HostError> {
        let id = asked.borrow().record.borrow().id.clone();
        let ahead = self.resuming.borrow().get(&id).cloned();
        if let Some(ahead) = ahead {
            let _ = ahead.await;
            let entry = self
                .ctx
                .live(&id)
                .expect("a resumed sub-thread stands in the live set");
            return Ok(Resumed {
                entry,
                preamble: None,
                notices: Vec::new(),
            });
        }
        if asked.borrow().record.borrow().phase != WorkspacePhase::Released {
            return Ok(Resumed {
                entry: Rc::clone(asked),
                preamble: None,
                notices: Vec::new(),
            });
        }
        let (signal, done) = futures::channel::oneshot::channel::<()>();
        let finished: LocalBoxFuture<'static, Result<(), HostError>> = Box::pin(async move {
            let _ = done.await;
            Ok(())
        });
        let joined: Joined<()> = finished.shared();
        self.resuming.borrow_mut().insert(id.clone(), joined);
        let outcome = self.resume_once(asked, thread_id, harness, prompt).await;
        let _ = signal.send(());
        self.resuming.borrow_mut().shift_remove(&id);
        outcome
    }

    /// sub-spawn.ts:518 (rule 10).
    pub(crate) async fn resume_once(
        &self,
        asked: &Entry,
        thread_id: &str,
        harness: &str,
        prompt: Option<&str>,
    ) -> Result<Resumed, HostError> {
        self.resume_checked(asked, thread_id, harness, prompt)
            .await
            .map_err(AreaError::into_host)
    }

    async fn resume_checked(
        &self,
        asked: &Entry,
        thread_id: &str,
        harness: &str,
        prompt: Option<&str>,
    ) -> AreaResult<Resumed> {
        let record_rc = Rc::clone(&asked.borrow().record);
        let sub = record_rc
            .borrow()
            .sub
            .clone()
            .expect("a released sub-thread carries its sub record");
        let rows = self.ctx.computer_rows().await?;
        let row = rows.into_iter().find(|r| {
            r.kind == "provider"
                && names_place(
                    &PlaceRef {
                        id: r.id.clone(),
                        name: r.name.clone(),
                        kind: Some(r.kind.clone()),
                    },
                    &sub.on,
                )
        });
        let at = row.as_ref().and_then(|row| self.provider_at(&row.id));
        let (row, at) = match (row, at) {
            (Some(row), Some(at)) => (row, at),
            _ => {
                return Err(AreaError::Host(sub_cloud_gone_refusal(&cloud_said(
                    &sub.on,
                ))))
            }
        };
        let on = provider_key_name(&row.name);
        let (record_place, record_name, record_project, record_kind) = {
            let held = record_rc.borrow();
            (
                held.place.clone(),
                held.name.clone(),
                held.project.clone(),
                held.kind,
            )
        };
        let place = record_place.clone().unwrap_or_else(|| row.id.clone());
        let project = self.ctx.project_held(&record_project);
        let Some(host) = git_host_of(&sub.repo) else {
            let host_word = remote_host(&sub.repo).unwrap_or_else(|| sub.repo.clone());
            return Err(AreaError::Host(sub_other_host_refusal(
                &project.name,
                &host_word,
                &on,
            )));
        };
        let brokered = self
            .ctx
            .brokered(kind_word(record_kind), record_place.as_deref());
        let ImageAt {
            head,
            record: image,
        } = self
            .image_at(&place, &on, host, &sub.repo, brokered)
            .await?;
        self.ctx.place_guard(&place).await?;
        self.agent_on(image.as_ref(), &on, harness, brokered)?;
        let seed = self.seed_of(&project).await?;
        let limits = if at.offers().limits {
            Some(at.limits().await?)
        } else {
            None
        };
        self.starts_left(limits.as_ref(), &record_name)?;
        let (cpu, mem_mb) = {
            let held = record_rc.borrow();
            (held.size.cpu.0, held.size.mem_mb.0)
        };
        let sold = self.sold_by(&*at, limits.as_ref());
        let size = sold
            .iter()
            .find(|s| s.cpu == cpu && s.mem_mb == mem_mb)
            .cloned()
            .unwrap_or_else(|| SubSize {
                name: sub_size_word(&CpuMem { cpu, mem_mb }),
                cpu,
                mem_mb,
                rate_usd_per_hour: 0.0,
            });
        let tried = sub.opening.clone().or_else(|| prompt.map(str::to_string));
        let now = self.ctx.now();
        let mut spawn_json = json!({ "startedAt": now, "log": [] });
        if let Some(asked) = tried {
            spawn_json["asked"] = json!(asked);
        }
        if let Some(why) = &seed.why {
            spawn_json["notSent"] = json!(why);
        }
        let spawn: SubSpawn = serde_json::from_value(spawn_json)
            .map_err(|e| AreaError::Host(HostError::new(e.to_string())))?;
        {
            let mut held = record_rc.borrow_mut();
            held.golden = head.snapshot_id.clone();
            held.phase = WorkspacePhase::Running;
            held.first_life = true;
            held.disk_gone = None;
            if let Some(sub) = held.sub.as_mut() {
                sub.lost = None;
                sub.spawn = Some(spawn);
            }
        }
        self.ctx.broker_lift(&asked.borrow().record.borrow().id);
        self.told(&record_rc.borrow());
        let (record_id, record_name_now) = record_key(&record_rc);
        let report = self.logged(
            &record_rc,
            self.ctx
                .stage_reporter(&record_id, &record_name_now, now, None),
        );
        self.ctx.forking_add(&record_name_now);
        let started = self
            .machine_for(&record_rc, &at, &on, &seed, report.clone(), Rc::new(|| {}))
            .await;
        let mut notices = match started {
            Ok(notices) => notices,
            Err(error) => {
                self.spawn_failed(&record_rc, &error);
                report(WorkspaceCreateStage::Failed, &error.message(), None);
                {
                    let mut held = record_rc.borrow_mut();
                    held.phase = WorkspacePhase::Released;
                    held.machine_id = String::new();
                }
                self.ctx.attach_dead(&record_rc);
                let snapshot = record_rc.borrow().clone();
                self.ctx.persist(&snapshot).await?;
                self.ctx.rows_sent().await;
                self.ctx.forking_remove(&record_name_now);
                return Err(error);
            }
        };
        self.ctx.forking_remove(&record_name_now);
        let entry = self
            .ctx
            .live(&record_id)
            .expect("a started sub-thread stands in the live set");
        if let Some(why) = &seed.why {
            notices.push(sub_not_sent_line(why, &project.name));
        }
        if !seed.stayed.is_empty() {
            notices.push(sub_stayed_line(&seed.stayed));
        }
        let opening = record_rc
            .borrow()
            .sub
            .as_ref()
            .and_then(|sub| sub.opening.clone());
        if let Some(opening) = opening {
            let kept_preamble = if prompt == Some(opening.as_str()) {
                None
            } else {
                Some(opening.clone())
            };
            {
                let mut held = record_rc.borrow_mut();
                if let Some(sub) = held.sub.as_mut() {
                    sub.opening = None;
                }
            }
            let snapshot = record_rc.borrow().clone();
            self.ctx.persist(&snapshot).await?;
            return Ok(Resumed {
                entry,
                preamble: kept_preamble,
                notices,
            });
        }
        self.land_session(&entry, thread_id).await;
        let delete_after = self
            .ctx
            .delete_after_ms(&place)
            .unwrap_or(wsp_wire::types::place_state::DELETE_AFTER_MS as f64);
        let delete_after_minutes = wsp_words::js::js_round(delete_after / 60_000.0);
        let pushed = record_rc
            .borrow()
            .sub
            .as_ref()
            .and_then(|sub| sub.result.as_ref())
            .and_then(|result| result.pushed_sha.clone())
            .is_some();
        let branch_of_record = record_rc.borrow().branch_of_record.is_some();
        let base = BaseAt {
            branch: sub.base.branch.clone(),
            sha: sub.base.sha.clone(),
        };
        let preamble = sub_resume_preamble(&SubResumePreamble {
            on,
            size,
            folder: sub.folder.clone(),
            sent: seed.paths.clone(),
            setup_git: host.setup_git.to_string(),
            repo: sub.repo.clone(),
            base,
            branch: sub.branch.clone(),
            pushed,
            delete_after_minutes,
            brokered: branch_of_record.then_some(true),
        });
        Ok(Resumed {
            entry,
            preamble: Some(preamble),
            notices,
        })
    }

    /// sub-spawn.ts:587 (rule 11).
    pub(crate) async fn land_session(&self, entry: &Entry, thread_id: &str) {
        let (kept, machine) = {
            let held = entry.borrow();
            let record = held.record.borrow();
            (
                record.sub.as_ref().and_then(|sub| sub.session.clone()),
                held.machine.clone(),
            )
        };
        let Some(kept) = kept else {
            return;
        };
        let (Some(_), Some(path)) = (kept.kept, kept.path.clone()) else {
            return;
        };
        let Some(file): Option<SessionFile> = self.ctx.session_file(entry, &kept.agent, &kept.id)
        else {
            return;
        };
        let bytes = self
            .ctx
            .get_blob(SUB_SESSIONS, thread_id)
            .await
            .ok()
            .flatten();
        let put = machine.offers().put_bytes;
        let Some(bytes) = bytes else {
            return;
        };
        if !put || !path.starts_with(&format!("{}/", file.folder)) {
            return;
        }
        let at = format!("/tmp/wsp-session-{thread_id}.gz");
        let landed = async {
            machine.put_bytes(&at, &bytes, None).await?;
            let landed = machine
                .exec(
                    &sub_session_land_command(&file, &path, &at),
                    ExecOptions {
                        timeout_ms: Some(INLINE_EXEC_MS),
                        ..ExecOptions::default()
                    },
                )
                .await?;
            if landed.exit_code != JsInt::from(0) {
                let said = if js_trim(&landed.stderr).is_empty() {
                    js_trim(&landed.stdout)
                } else {
                    js_trim(&landed.stderr)
                };
                return Err(EngineError::Other {
                    message: format!("exit {}: {}", landed.exit_code, tail(said, 200)),
                    code: None,
                });
            }
            Ok(())
        }
        .await;
        if let Err(error) = landed {
            self.ctx.warn(&format!(
                "the kept session of sub-thread {thread_id} did not land on {}, so its turn goes on from the transcript: {}",
                machine.id(),
                AreaError::Engine(error).message()
            ));
        }
    }

    /// sub-spawn.ts:606 (rule 12).
    pub async fn sub_killed_why(&self, entry: &Entry, started_at: f64) -> Option<String> {
        let (record, machine) = {
            let held = entry.borrow();
            let record = held.record.borrow().clone();
            (record, held.machine.clone())
        };
        let command = format!(
            "journalctl -k --since @{} --no-pager | grep -ci 'out of memory'",
            (started_at / 1000.0).floor() as i64
        );
        let read = match machine
            .exec(
                &command,
                ExecOptions {
                    timeout_ms: Some(INLINE_EXEC_MS),
                    ..ExecOptions::default()
                },
            )
            .await
        {
            Ok(read) => read,
            Err(error) => {
                self.ctx.warn(&format!(
                    "the kernel log of {} was not read after its agent was killed: {}",
                    record.name, error
                ));
                return None;
            }
        };
        let counted: String = read
            .stdout
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if counted.parse::<i64>().unwrap_or(0) <= 0 {
            return None;
        }
        let at = self.ctx.backend_for(&record).ok()?;
        let limits = if at.offers().limits {
            at.limits().await.ok()
        } else {
            None
        };
        let sold = self.sold_by(&*at, limits.as_ref());
        let sub = record.sub.clone()?;
        let size = sold
            .iter()
            .find(|s| s.cpu == record.size.cpu.0 && s.mem_mb == record.size.mem_mb.0)
            .cloned()
            .unwrap_or_else(|| SubSize {
                name: sub_size_word(&CpuMem {
                    cpu: record.size.cpu.0,
                    mem_mb: record.size.mem_mb.0,
                }),
                cpu: record.size.cpu.0,
                mem_mb: record.size.mem_mb.0,
                rate_usd_per_hour: 0.0,
            });
        let bigger = sub_next_size(record.size.mem_mb.0, &sold);
        let row = self
            .ctx
            .computer_rows()
            .await
            .unwrap_or_default()
            .into_iter()
            .find(|r| {
                r.kind == "provider"
                    && names_place(
                        &PlaceRef {
                            id: r.id.clone(),
                            name: r.name.clone(),
                            kind: Some(r.kind.clone()),
                        },
                        &sub.on,
                    )
            });
        let on = match row {
            Some(row) => provider_key_name(&row.name),
            None => sub.on.clone(),
        };
        let bigger_word = bigger.map(|b| {
            sub_size_word(&CpuMem {
                cpu: b.cpu,
                mem_mb: b.mem_mb,
            })
        });
        Some(sub_oom_line(
            &record.name,
            &on,
            &size,
            bigger_word.as_deref(),
        ))
    }
}
