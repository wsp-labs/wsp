// SPDX-License-Identifier: AGPL-3.0-only
//! reach.ts:1-197, 374-379, 476-482: the status pushed outside the poll, the refused nap and stop, and the disk of a
//! machine whose stop snapshots it. Spec: spec/reach_disk.md.
use futures::future::{join_all, select, Either, LocalBoxFuture};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use wsp_core::types::internal::SessionEntry;
use wsp_engine::disk_sync::{disk_use_of, DISK_USE_CMD};
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, Size};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::workspace::{ReachState, WorkspacePhase};
use wsp_words::words::thread::{
    disk_full_line, disk_mark_line, nap_refused_line, stop_refused_line, DiskMark, DiskMove,
    DISK_FULL_PCT, DISK_REARM_PCT,
};

use super::{DiskHeld, ReachArea};
use crate::machines::area_error::AreaResult;
use crate::machines::machines::Entry;
use crate::unwound::unwound;
use wsp_core::context::Joined;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_words::place_word::provider_key_name;

/// reach.ts:15. How often a running machine whose stop snapshots its disk has it read.
pub const DISK_READ_MS: f64 = 120_000.0;
/// reach.ts:19. How long a host closing waits on a disk read still out.
pub const DISK_CLOSE_WAIT_MS: f64 = 2_000.0;

/// What one disk reading answers (reach.ts:63): the reading and whether it crossed the line.
#[derive(Clone, Debug, PartialEq)]
pub struct DiskReading {
    pub pct: f64,
    pub used_bytes: f64,
    pub free_bytes: f64,
    pub crossed: bool,
}

fn entry_ids(entry: &Entry) -> (String, String) {
    let entry = entry.borrow();
    let record = entry.record.borrow();
    (record.id.clone(), entry.machine.id().to_string())
}

fn started_of(entry: &SessionEntry) -> f64 {
    entry
        .view
        .started_at
        .as_ref()
        .map_or(0.0, |started| started.0)
}

fn sized(record: &WorkspaceRecord) -> Size {
    Size {
        cpu: record.size.cpu.0,
        mem_mb: record.size.mem_mb.0,
    }
}

impl ReachArea {
    /// reach.ts:35 (rule 2).
    pub fn nap_refused_of(&self, entry: &Entry) -> Option<String> {
        let (id, machine_id) = entry_ids(entry);
        let refused = self.nap_refusals.borrow().get(&id).cloned()?;
        if refused.machine_id == machine_id {
            return Some(refused.value);
        }
        self.nap_refusals.borrow_mut().shift_remove(&id);
        None
    }

    /// reach.ts:42 (rule 2).
    pub fn nap_refused_reason(&self, entry: &Entry) -> Option<String> {
        self.nap_refused_of(entry)
            .map(|said| nap_refused_line(&said))
    }

    /// reach.ts:49 (rule 3).
    pub fn stop_refused_reason(&self, entry: &Entry) -> Option<String> {
        let (id, machine_id) = entry_ids(entry);
        let refused = self.stop_refusals.borrow().get(&id).cloned()?;
        (refused.machine_id == machine_id).then(|| stop_refused_line(&refused.value))
    }

    /// reach.ts:56 (rule 4).
    pub fn disk_reason(&self, entry: &Entry) -> Option<String> {
        let (id, machine_id) = entry_ids(entry);
        let read = self.disks.borrow().get(&id).cloned()?;
        (read.machine_id == machine_id && read.pct > DISK_FULL_PCT)
            .then(|| disk_full_line(read.pct))
    }

    /// reach.ts:60 (rule 5).
    pub(crate) fn watches_disk(&self, entry: &Entry) -> bool {
        let record = entry.borrow().record.borrow().clone();
        let Ok(backend) = self.ctx.backend_for(&record) else {
            return false;
        };
        self.ctx.pause_keeps_disk(&*backend) && record.phase == WorkspacePhase::Running
    }

    /// reach.ts:63 (rule 6).
    pub(crate) async fn disk_read(&self, entry: &Entry) -> Option<DiskReading> {
        let machine = entry.borrow().machine.clone();
        let options = ExecOptions {
            timeout_ms: Some(INLINE_EXEC_MS),
            ..ExecOptions::default()
        };
        let read = machine
            .exec(DISK_USE_CMD, options)
            .await
            .ok()
            .filter(|read| read.exit_code == JsInt::from(0))?;
        let usage = disk_use_of(&read.stdout)?;
        if !self.watches_disk(entry) {
            return None;
        }
        let (id, machine_id) = entry_ids(entry);
        let was = self.disk_reason(entry);
        let held = self.disks.borrow().get(&id).cloned();
        let told = held.as_ref().is_some_and(|held| {
            held.machine_id == machine_id && held.told && usage.pct > DISK_REARM_PCT
        });
        let crossed = !told && usage.pct > DISK_FULL_PCT;
        self.disks.borrow_mut().insert(
            id,
            DiskHeld {
                machine_id,
                pct: usage.pct,
                told: told || crossed,
            },
        );
        if self.disk_reason(entry) != was {
            let reach = self.reach_of(entry);
            let reason = self.ctx.row_reason(entry);
            if let Err(e) = self.emit_status(entry, reach, reason.as_deref()).await {
                self.ctx.warn(&e.message());
            }
        }
        Some(DiskReading {
            pct: usage.pct,
            used_bytes: usage.used_bytes,
            free_bytes: usage.free_bytes,
            crossed,
        })
    }

    /// reach.ts:78 (rule 7).
    pub async fn read_disk(&self, entry: &Entry) {
        if !self.watches_disk(entry) {
            return;
        }
        self.arm_disk_watch();
        let (id, _) = entry_ids(entry);
        let held = self.disk_reads.borrow().get(&id).cloned();
        let read: Joined = match held {
            Some(held) => held,
            None => {
                let area = self
                    .me
                    .upgrade()
                    .expect("a reach area is alive while its method runs");
                let entry = entry.clone();
                let key = id.clone();
                let reading: LocalBoxFuture<'static, ()> = Box::pin(async move {
                    let _ = unwound(area.read_disk_now(&entry)).await;
                    area.disk_reads.borrow_mut().shift_remove(&key);
                });
                let shared = reading.shared();
                self.disk_reads.borrow_mut().insert(id, shared.clone());
                shared
            }
        };
        read.await;
    }

    /// reach.ts:88 (rule 8).
    pub(crate) async fn read_disk_now(&self, entry: &Entry) {
        let first = self.disk_read(entry).await;
        let Some(first) = first.filter(|reading| reading.crossed) else {
            return;
        };
        if self.ctx.closing() {
            return;
        }
        let (record, machine) = {
            let entry = entry.borrow();
            let record = entry.record.borrow().clone();
            (record, entry.machine.clone())
        };
        if let Err(e) = self.ctx.sweep_leftovers(&machine).await {
            self.ctx.warn(&format!(
                "what wsp made on {} was not cleared: {}",
                record.name,
                e.message()
            ));
        }
        let after = self.disk_read(entry).await.unwrap_or_else(|| first.clone());
        let to = self
            .move_for(entry, after.used_bytes + after.free_bytes)
            .await;
        let words = DiskMark {
            machine: record.name.clone(),
            on: provider_key_name(
                &self
                    .ctx
                    .provider_of(&record)
                    .unwrap_or_else(|| "the provider".to_string()),
            ),
            pct: after.pct,
            used_bytes: after.used_bytes,
            size_bytes: after.used_bytes + after.free_bytes,
            freed_bytes: (first.used_bytes - after.used_bytes).max(0.0),
            machine_goes: record.sub.is_some(),
            r#move: None,
        };
        self.ctx
            .warn(&format!("{}: {}", record.name, disk_mark_line(&words)));
        self.tell_threads(&record.id, &|thread: &str| {
            let mut line = words.clone();
            line.r#move = to.as_ref().map(|to| DiskMove {
                thread: thread.to_string(),
                ..to.clone()
            });
            disk_mark_line(&line)
        });
    }

    /// reach.ts:107 (rule 12).
    pub fn tell_threads(&self, workspace_id: &str, line: &dyn Fn(&str) -> String) {
        for entry in self.last_turns(workspace_id) {
            let thread = entry
                .view
                .thread_id
                .clone()
                .unwrap_or_else(|| entry.view.id.clone());
            let session = entry
                .view
                .claude_session_id
                .clone()
                .unwrap_or_else(|| entry.turn_id.clone());
            self.ctx.record(json!({
                "type": "session.notice",
                "workspaceId": workspace_id,
                "sessionId": session,
                "turnId": entry.turn_id,
                "threadId": thread,
                "text": line(&thread),
            }));
        }
    }

    /// reach.ts:116 (rule 11). The move less its thread.
    pub(crate) async fn move_for(&self, entry: &Entry, read_bytes: f64) -> Option<DiskMove> {
        let record = entry.borrow().record.borrow().clone();
        let at = self.ctx.backend_for(&record).ok()?;
        let offers = at.offers();
        if !offers.root_disk_of {
            return None;
        }
        if offers.limits {
            let _ = at.limits().await;
        }
        let mut up: Vec<_> = at
            .capabilities()
            .sizes
            .into_iter()
            .filter(|s| s.mem_mb.0 > record.size.mem_mb.0)
            .collect();
        up.sort_by(|a, b| {
            a.mem_mb
                .0
                .partial_cmp(&b.mem_mb.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for size in up {
            let cpu = size.cpu.0;
            let mem_mb = size.mem_mb.0;
            if let Some(disk) = at.root_disk_of(Size { cpu, mem_mb }) {
                if disk.disk_gb * 1e9 > read_bytes * 1.1 {
                    return Some(DiskMove {
                        thread: String::new(),
                        name: disk.name,
                        cpu,
                        mem_mb,
                        disk_gb: disk.disk_gb,
                    });
                }
            }
        }
        None
    }

    /// reach.ts:128 (rule 13).
    pub(crate) fn last_turns(&self, workspace_id: &str) -> Vec<SessionEntry> {
        let mut latest: IndexMap<String, SessionEntry> = IndexMap::new();
        for session in self.ctx.session_entries() {
            if session.view.workspace_id != workspace_id {
                continue;
            }
            let key = session
                .view
                .thread_id
                .clone()
                .unwrap_or_else(|| session.view.id.clone());
            let started = started_of(&session);
            let replace = match latest.get(&key) {
                None => true,
                Some(held) => started >= started_of(held),
            };
            if replace {
                latest.insert(key, session);
            }
        }
        latest.into_values().collect()
    }

    /// reach.ts:143 (rule 9).
    pub fn arm_disk_watch(&self) {
        if self.disk_timer.borrow().is_some()
            || self.ctx.closing()
            || !self
                .ctx
                .live_all()
                .iter()
                .any(|entry| self.watches_disk(entry))
        {
            return;
        }
        let me = self.me.clone();
        let cancel = self.ctx.schedule(
            DISK_READ_MS,
            Box::new(move || {
                if let Some(area) = me.upgrade() {
                    area.beat();
                }
            }),
        );
        *self.disk_timer.borrow_mut() = Some(cancel);
    }

    fn beat(&self) {
        *self.disk_timer.borrow_mut() = None;
        for entry in self.ctx.live_all() {
            let Some(area) = self.me.upgrade() else {
                return;
            };
            self.ctx.spawn(Box::pin(async move {
                area.read_disk(&entry).await;
            }));
        }
        self.arm_disk_watch();
    }

    /// reach.ts:155 (rule 10).
    pub async fn stop_disk_watch(&self) {
        let cancel = self.disk_timer.borrow_mut().take();
        if let Some(cancel) = cancel {
            cancel();
        }
        let reads: Vec<Joined> = self.disk_reads.borrow().values().cloned().collect();
        let settled = Box::pin(join_all(reads));
        let bound = self.ctx.sleep(DISK_CLOSE_WAIT_MS);
        match select(settled, bound).await {
            Either::Left(_) | Either::Right(_) => {}
        }
    }

    /// reach.ts:165 (rule 14).
    pub fn reach_of(&self, entry: &Entry) -> ReachState {
        if self.ctx.unreached_of(entry).is_some() {
            return ReachState::Unreachable;
        }
        let (id, machine_id) = entry_ids(entry);
        let seen = self.polled_reach.borrow().get(&id).cloned();
        if let Some(seen) = seen.filter(|seen| seen.machine_id == machine_id) {
            return seen.value;
        }
        let answers = entry.borrow().machine.offers().daemon_answers;
        if self.ctx.kind_has_daemon(entry) || answers {
            ReachState::Reachable
        } else {
            ReachState::Unsupported
        }
    }

    /// reach.ts:172 (rule 15).
    pub async fn emit_status(
        &self,
        entry: &Entry,
        reach: ReachState,
        reason: Option<&str>,
    ) -> AreaResult<()> {
        let (record, wake_ask, checkout, pr, tree) = {
            let entry = entry.borrow();
            let record = entry.record.borrow().clone();
            (
                record,
                entry
                    .wake_ask
                    .as_ref()
                    .map(|ask| json!({ "ask": ask.ask, "of": ask.of })),
                entry
                    .checkout
                    .as_ref()
                    .and_then(|checkout| serde_json::to_value(checkout).ok()),
                entry
                    .pr
                    .as_ref()
                    .and_then(|pr| serde_json::to_value(pr).ok()),
                entry
                    .tree
                    .as_ref()
                    .and_then(|tree| serde_json::to_value(tree).ok()),
            )
        };
        let idle_at = if record.phase == WorkspacePhase::Running {
            self.ctx.idle_at(&record.id)
        } else {
            None
        };
        let rate = self
            .ctx
            .backend_for(&record)?
            .pricing()
            .rate_usd_per_hour(sized(&record));
        let mut status: Map<String, Value> = match self.ctx.view(&record) {
            Value::Object(view) => view,
            _ => Map::new(),
        };
        status.insert(
            "machineState".to_string(),
            json!(self.ctx.machine_state_of(&record.phase)),
        );
        status.insert("reach".to_string(), json!({ "state": reach }));
        status.insert(
            "size".to_string(),
            json!({ "cpu": record.size.cpu, "memMb": record.size.mem_mb }),
        );
        status.insert("rateUsdPerHour".to_string(), json!(rate));
        if let Some(reason) = reason {
            status.insert("reason".to_string(), json!(reason));
        }
        if let Some(wake) = wake_ask {
            status.insert("wakeAsk".to_string(), wake);
        }
        if let Some(idle) = idle_at {
            status.insert("idleAt".to_string(), json!(idle));
        }
        if let Some(checkout) = checkout {
            status.insert("checkout".to_string(), checkout);
        }
        if let Some(pr) = pr {
            status.insert("pr".to_string(), pr);
        }
        if let Some(tree) = tree {
            status.insert("tree".to_string(), tree);
        }
        self.ctx
            .emit(json!({ "type": "workspace.status", "status": status }));
        Ok(())
    }

    /// reach.ts:194 (rule 16).
    pub async fn says_waking(&self, entry: &Entry, words: &str) -> AreaResult<()> {
        entry.borrow_mut().wake_said = Some(words.to_string());
        self.emit_status(entry, ReachState::Napping, Some(words))
            .await
    }

    /// reach.ts:376 (rule 17).
    pub async fn push_status(&self, entry: &Entry) -> AreaResult<()> {
        let running = entry.borrow().record.borrow().phase == WorkspacePhase::Running;
        if !running {
            return Ok(());
        }
        let reach = self.reach_of(entry);
        self.emit_status(entry, reach, None).await
    }

    /// reach.ts:477 (rule 17).
    pub async fn status_now(&self, entry: &Entry) -> AreaResult<()> {
        let running = entry.borrow().record.borrow().phase == WorkspacePhase::Running;
        let reach = if running {
            self.reach_of(entry)
        } else {
            ReachState::Napping
        };
        self.emit_status(entry, reach, None).await
    }
}
