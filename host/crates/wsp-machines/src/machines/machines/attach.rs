// SPDX-License-Identifier: AGPL-3.0-only
//! machines.ts:270-301,364-587 at 65f6b89c7: the live entry, its engine hooks, and drop. Spec: spec/machines_attach.md.

use std::cell::RefCell;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::FutureExt;
use indexmap::IndexSet;
use serde_json::json;

use wsp_core::types::internal::{
    SessionEntry, AGENT_LISTS, CREATES, SESSIONS, TRANSCRIPTS, TRANSCRIPT_HEADS, TRANSCRIPT_INDEX,
    WORKSPACES, WORKSPACE_NAMES,
};
use wsp_core::types::wiring::{shape_fault, LiveWorkspace, WorkspaceRecord, WorkspaceSpec, VAULTS};
use wsp_engine::disk_sync::sync_disk;
use wsp_engine::errors::EngineError;
use wsp_engine::lifecycle::{
    MachineRef, ProviderMove, Workspace, WorkspaceHooks, WorkspaceInitial,
    WorkspacePhase as EnginePhase,
};
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, RunOptions};
use wsp_engine::vault::{import_into, VaultOptions};
use wsp_wire::types::pull_request::{PullRequestKept, PullRequestSeen};
use wsp_wire::views::workspace::{ReachState, WorkspacePhase};
use wsp_wire::views::workspace_events::WorkspaceDeletedEvent;
use wsp_wire::wire::events_union::{EventUnion, Sequenced};
use wsp_words::exit::{HostError, Kind};
use wsp_words::words::computer::RESUME_UNANSWERED;

use super::super::area_error::AreaError;
use super::super::sub_life::SUB_SESSIONS;
use super::create::{host_to_engine, iso, Around, ExecHook};
use super::{Entry, MachinesArea, Unreached};

/// `enginePhaseOf` (machines.ts:367): the engine's three phases off the record's seven.
pub fn engine_phase_of(phase: WorkspacePhase) -> EnginePhase {
    match phase {
        WorkspacePhase::Running => EnginePhase::Running,
        WorkspacePhase::Napping => EnginePhase::Napping,
        WorkspacePhase::Waking => EnginePhase::Waking,
        WorkspacePhase::Pausing
        | WorkspacePhase::Gone
        | WorkspacePhase::Queued
        | WorkspacePhase::Released => EnginePhase::Napping,
    }
}

/// The record's kind as the kind modules are keyed (`kindWords(kind)`): `cloud` or `local`.
fn kind_key(kind: wsp_wire::views::workspace_kind::WorkspaceKind) -> &'static str {
    match kind {
        wsp_wire::views::workspace_kind::WorkspaceKind::Cloud => "cloud",
        wsp_wire::views::workspace_kind::WorkspaceKind::Local => "local",
        wsp_wire::views::workspace_kind::WorkspaceKind::Place => "place",
    }
}

/// A machine's exec and run as the watch sees them: the provider's word that it cannot be reached or is stopped marks the
/// record, and any answer takes the mark off (machines.ts:274-300).
struct WatchExec {
    area: Rc<MachinesArea>,
    entry: Weak<RefCell<LiveWorkspace>>,
    machine_id: String,
}

impl WatchExec {
    async fn heard<F: Future<Output = Result<ExecResult, EngineError>>>(
        &self,
        call: F,
    ) -> Result<ExecResult, EngineError> {
        let res = call.await;
        let Some(entry) = self.entry.upgrade() else {
            return res;
        };
        let (held_id, record_id) = {
            let e = entry.borrow();
            let id = e.record.borrow().id.clone();
            (e.machine.id().to_string(), id)
        };
        match res {
            Ok(answer) => {
                let standing = self
                    .area
                    .rt
                    .unreached()
                    .borrow()
                    .get(&record_id)
                    .is_some_and(|u| u.machine_id == self.machine_id);
                if standing {
                    self.area
                        .rt
                        .unreached()
                        .borrow_mut()
                        .shift_remove(&record_id);
                }
                Ok(answer)
            }
            Err(e) => {
                if let EngineError::MachineUnreachable { .. } = &e {
                    if held_id == self.machine_id {
                        let standing = self
                            .area
                            .rt
                            .unreached()
                            .borrow()
                            .get(&record_id)
                            .is_some_and(|u| u.machine_id == self.machine_id);
                        self.area.rt.unreached().borrow_mut().insert(
                            record_id.clone(),
                            Unreached {
                                machine_id: self.machine_id.clone(),
                                line: e.to_string(),
                            },
                        );
                        let running =
                            entry.borrow().record.borrow().phase == WorkspacePhase::Running;
                        if !standing && running {
                            self.area
                                .rt
                                .emit_status(&entry, ReachState::Unreachable, Some(&e.to_string()))
                                .await
                                .map_err(host_to_engine)?;
                        }
                    }
                }
                if let EngineError::MachineStopped { .. } = &e {
                    if held_id == self.machine_id {
                        self.area
                            .adopt_pause(&entry)
                            .await
                            .map_err(host_to_engine)?;
                    }
                }
                Err(e)
            }
        }
    }
}

#[async_trait::async_trait(?Send)]
impl ExecHook for WatchExec {
    async fn exec(
        &self,
        inner: &Arc<dyn Machine>,
        cmd: &str,
        opts: ExecOptions,
    ) -> Result<ExecResult, EngineError> {
        self.heard(inner.exec(cmd, opts)).await
    }
    async fn run(
        &self,
        inner: &Arc<dyn Machine>,
        script: &str,
        opts: RunOptions,
    ) -> Result<ExecResult, EngineError> {
        self.heard(inner.run(script, opts)).await
    }
}

/// The live entry's `pr` as the record kept it, read as merged or closed and never read again (machines.ts:413).
fn kept_pull_request(
    kept: &wsp_wire::types::pull_request::PullRequestRecord,
) -> Option<PullRequestSeen> {
    let mut value = serde_json::to_value(kept).ok()?;
    let read_at = kept
        .merged_at
        .or(kept.closed_at)
        .map(|t| t.0)
        .unwrap_or(0.0);
    value
        .as_object_mut()?
        .insert("readAt".into(), json!(read_at));
    serde_json::from_value::<PullRequestKept>(value)
        .ok()
        .map(PullRequestSeen::PullRequestKept)
}

/// The engine's hooks for one entry, each closing over the entry by its weak handle, so an entry that is dropped is
/// never kept alive by its own hooks (machines.ts:428-470).
struct Hooks {
    area: Rc<MachinesArea>,
    entry: Weak<RefCell<LiveWorkspace>>,
    record: Rc<RefCell<WorkspaceRecord>>,
    at: Arc<dyn wsp_engine::machine::MachineBackend>,
}

impl MachinesArea {
    /// `leave` (machines.ts:412): the sign-in paths stay out of a vault where the record's place is a provider.
    fn vault_leave(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Option<Vec<String>> {
        let snapshot = record.borrow().clone();
        let place = self.rt.place_id_of(&snapshot);
        self.rt
            .at_provider(place.as_deref())
            .then(|| (*wsp_catalog::signin_rows::LOGIN_PATHS).clone())
    }
}

/// The `leave` option of an engine import, as the TS `{ leave }` spread gives it (machines.ts:437-450).
fn import_options(leave: Option<Vec<String>>) -> VaultOptions {
    VaultOptions {
        leave: leave.unwrap_or_default(),
        ..Default::default()
    }
}

/// `wakeCheck`'s "both" line: the shape the record was created as and the provider's view of it (machines.ts:500).
fn both_line(expected: &impl serde::Serialize, actual: &impl serde::Serialize) -> String {
    format!(
        "created as {}, provider view {}",
        serde_json::to_string(expected).unwrap_or_default(),
        serde_json::to_string(actual).unwrap_or_default()
    )
}

impl MachinesArea {
    /// `watched` (machines.ts:274): the machine with `exec` and `run` read for the provider's unreachable and stopped
    /// answers, on behalf of `entry`.
    pub fn watched(self: &Rc<Self>, entry: &Entry, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        self.watched_on(Rc::downgrade(entry), machine)
    }

    /// `watched` for an entry that is still being made: its weak handle, read at each call.
    fn watched_on(
        self: &Rc<Self>,
        entry: Weak<RefCell<LiveWorkspace>>,
        machine: Arc<dyn Machine>,
    ) -> Arc<dyn Machine> {
        let machine_id = machine.id().to_string();
        Arc::new(Around {
            inner: machine,
            hook: Rc::new(WatchExec {
                area: self.clone(),
                entry,
                machine_id,
            }),
        })
    }

    /// `followMachine` (machines.ts:386).
    pub fn follow_machine(&self, entry: &Entry) {
        let (machine_id, first_life) = {
            let e = entry.borrow();
            (e.ws.machine_id(), e.ws.is_first_life())
        };
        {
            let e = entry.borrow();
            let mut record = e.record.borrow_mut();
            record.phase = WorkspacePhase::Running;
            record.machine_id = machine_id;
            record.first_life = first_life;
            record.gone = None;
        }
        entry.borrow_mut().unchecked = false;
        self.rt.sync_daemon(entry);
    }

    /// `attach` (machines.ts:395): the new live entry, set in `live`, its engine workspace given the hooks of spec
    /// rules 4-7.
    pub fn attach(
        self: &Rc<Self>,
        record: &Rc<RefCell<WorkspaceRecord>>,
        machine: Arc<dyn Machine>,
    ) -> Entry {
        let (id, golden, phase, first_life, kept, checkout) = {
            let r = record.borrow();
            (
                r.id.clone(),
                r.golden.clone(),
                r.phase,
                r.first_life,
                r.pr.clone(),
                r.checkout.clone(),
            )
        };
        let generation = self
            .rt
            .live()
            .borrow()
            .get(&id)
            .map_or(0, |e| e.borrow().generation + 1);
        let at = self.rt.backend_for(&record.borrow().clone());
        let vaulted = self.rt.keeps_images(&at);
        if !vaulted {
            let mut r = record.borrow_mut();
            r.vaulted_at = None;
            r.vault_refused = None;
        }
        let wake_attempts = at
            .lifecycle()
            .map(|l| l.budgets().wake_attempts.0 .0 as u32)
            .unwrap_or(0);
        let pr = kept
            .as_ref()
            .filter(|k| {
                serde_json::to_value(k.state)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .as_deref()
                    != Some("open")
            })
            .and_then(kept_pull_request);

        let entry: Entry = Rc::new_cyclic(|weak: &Weak<RefCell<LiveWorkspace>>| {
            let hooks = Hooks {
                area: self.clone(),
                entry: weak.clone(),
                record: record.clone(),
                at: at.clone(),
            };
            let machine = self.watched_on(weak.clone(), machine);
            let workspace = Workspace::new(
                machine.clone(),
                build_hooks(&hooks, golden.clone(), wake_attempts, vaulted),
                WorkspaceInitial {
                    phase: Some(engine_phase_of(phase)),
                    first_life: Some(first_life),
                },
            );
            RefCell::new(LiveWorkspace {
                record: record.clone(),
                opens_thread: None,
                of_record_held: false,
                ws: Rc::new(workspace),
                machine,
                generation,
                waking: None,
                unchecked: false,
                napping: None,
                adopting: None,
                deleting: None,
                late_read: None,
                wake_stop: None,
                resize_to: None,
                nap_wait_ms: None,
                wake_said: None,
                delete_said: None,
                wake_ask: None,
                creating: false,
                checkout,
                checkout_reading: None,
                tree: None,
                tree_reading: None,
                pr,
                pr_reading: None,
                pr_poll: None,
                pr_poll_ms: None,
                pr_none_at: None,
            })
        });
        self.rt.live().borrow_mut().insert(id, entry.clone());
        entry
    }

    /// `idleWindowOf` (machines.ts:517): `None` is off.
    pub fn idle_window_of(&self, record: &WorkspaceRecord) -> Option<f64> {
        let own = record
            .idle_window_ms
            .as_ref()
            .map(|w| w.as_ref().map(|j| j.0));
        match own {
            Some(window) => window,
            None => match self.rt.place_nap_ms(self.rt.place_id_of(record).as_deref()) {
                Some(window) => window,
                None => self.config.default_idle_window_ms,
            },
        }
    }

    /// `endSessions` (machines.ts:520).
    pub fn end_sessions(&self, workspace_id: &str, reason: &str) {
        let ends: Vec<_> = self
            .rt
            .sessions()
            .borrow()
            .values()
            .filter(|s| s.borrow().view.workspace_id == workspace_id)
            .filter_map(|s| s.borrow().end.clone())
            .collect();
        for end in ends {
            end(reason, None, None);
        }
        self.rt.end_execs(workspace_id, reason);
    }

    /// `drop` (machines.ts:526), spec rule 10, in that order.
    pub async fn drop(self: &Rc<Self>, id: &str) -> Result<(), HostError> {
        let going = self.rt.live().borrow().get(id).cloned();
        if let Some(entry) = &going {
            let late = entry.borrow_mut().late_read.take();
            if let Some(late) = late {
                late();
            }
        }
        if let Some(entry) = &going {
            let (kind, name, machine_id) = {
                let e = entry.borrow();
                let r = e.record.borrow();
                (r.kind, r.name.clone(), e.machine.id().to_string())
            };
            let module = self.rt.module_of(kind_key(kind));
            let dropped = {
                let e = entry.borrow();
                module.dropped(&e)
            };
            if let Err(e) = dropped.await {
                self.rt.warn(&format!(
                    "{name}'s machine {machine_id} kept something of this host's: {}",
                    e.message
                ));
            }
        }
        self.rt.live().borrow_mut().shift_remove(id);
        if let Some(entry) = &going {
            let kind = entry.borrow().record.borrow().kind;
            if wsp_words::projects::runs_in_folder(&kind) {
                self.rt.write_daemon_roots(entry).await?;
            }
        }
        self.rt.forget_workspace_maps(id);

        let mut dropped: IndexSet<String> = IndexSet::new();
        for (thread_id, held) in self.rt.thread_records().borrow().iter() {
            if held.workspace_id == id {
                dropped.insert(thread_id.clone());
            }
        }
        for s in self.rt.sessions().borrow().values() {
            let s = s.borrow();
            if s.view.workspace_id == id {
                if let Some(thread_id) = &s.view.thread_id {
                    dropped.insert(thread_id.clone());
                }
            }
        }
        self.rt
            .sessions()
            .borrow_mut()
            .retain(|_, s: &mut Row| s.borrow().view.workspace_id != id);
        for thread_id in self.rt.kept_agents_on(id) {
            self.rt.reap_kept(&thread_id);
        }
        self.rt
            .thread_records()
            .borrow_mut()
            .retain(|_, held| held.workspace_id != id);
        for thread_id in &dropped {
            self.rt.forget_slate(thread_id).await?;
        }
        let is_sub = match &going {
            Some(entry) => entry.borrow().record.borrow().sub.is_some(),
            None => false,
        };
        if is_sub {
            for thread_id in &dropped {
                self.rt.store().delete_blob(SUB_SESSIONS, thread_id).await?;
            }
        }
        self.rt.forget_viewed(id);
        self.rt.cancel_flush(id);
        self.rt.settle_flushes(id).await;

        let store = self.rt.store();
        store.delete(WORKSPACES, id).await?;
        store.delete_blob(TRANSCRIPTS, id).await?;
        self.rt.clear_rows(id);
        store.delete_blob(TRANSCRIPT_HEADS, id).await?;
        store.delete_blob(TRANSCRIPT_INDEX, id).await?;
        self.rt.drop_sent_images(id).await?;
        self.rt.forget_kept_writes(id);
        store.delete(WORKSPACE_NAMES, id).await?;
        store.delete(SESSIONS, id).await?;
        store.delete(CREATES, &format!("workspace/{id}")).await?;
        store.delete_blob(VAULTS, id).await?;

        if let Some(entry) = &going {
            let machine_id = entry.borrow().machine.id().to_string();
            let still_held = self
                .rt
                .live()
                .borrow()
                .values()
                .any(|e| e.borrow().machine.id() == machine_id);
            if !still_held {
                for key in store.keys(AGENT_LISTS).await? {
                    if key.starts_with(&format!("{machine_id}:")) {
                        store.delete(AGENT_LISTS, &key).await?;
                    }
                }
            }
        }
        self.rt.emit(EventUnion::WorkspaceDeleted(Sequenced {
            event: WorkspaceDeletedEvent {
                workspace_id: id.to_string(),
            },
            seq: None,
        }))
    }
}

type Row = Rc<RefCell<SessionEntry>>;

/// The engine's hooks of one entry (machines.ts:428-486): the four vault hooks only where the backend keeps images.
fn build_hooks(h: &Hooks, golden: String, wake_attempts: u32, vaulted: bool) -> WorkspaceHooks {
    let mut hooks = WorkspaceHooks {
        golden_snapshot: golden,
        wake_attempts,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    };
    let (area, entry, record, at) = (
        h.area.clone(),
        h.entry.clone(),
        h.record.clone(),
        h.at.clone(),
    );

    {
        let (area, at) = (area.clone(), at.clone());
        hooks.retire = Some(Arc::new(move |m: MachineRef| {
            let (area, at) = (area.clone(), at.clone());
            async move { area.rt.gone_stop(&at, &m).await.map_err(host_to_engine) }.boxed_local()
        }));
    }
    {
        let (area, entry, record) = (area.clone(), entry.clone(), record.clone());
        hooks.resurrect = Some(Arc::new(
            move |over: Option<wsp_engine::machine::MachineSpec>| {
                let (area, entry, record) = (area.clone(), entry.clone(), record.clone());
                async move {
                    let bind_entry = entry.clone();
                    let bind_area = area.clone();
                    let bind = move |m: &MachineRef, _r: &WorkspaceRecord| {
                        if let Some(live) = bind_entry.upgrade() {
                            live.borrow_mut().machine =
                                bind_area.watched_on(bind_entry.clone(), m.clone());
                        }
                    };
                    let over = over.map(|o| WorkspaceSpec {
                        cpu: o.cpu,
                        mem_mb: o.mem_mb,
                        envs: o.envs,
                        labels: o.labels,
                        binds: o.binds,
                        copy: o.copy,
                    });
                    area.fork(&record, &bind, over.as_ref(), None, true)
                        .await
                        .map_err(AreaError::into_engine)
                }
                .boxed_local()
            },
        ));
    }

    if vaulted {
        {
            let (area, record) = (area.clone(), record.clone());
            hooks.vault_export = Some(Arc::new(move |m: MachineRef| {
                let (area, record) = (area.clone(), record.clone());
                async move {
                    let leave = area.vault_leave(&record);
                    let cap = area.config.vault_cap_bytes;
                    match area.rt.vault_export(&m, cap, leave).await {
                        Ok(bytes) => Ok(Some(bytes)),
                        Err(e) if e.kind == Some(Kind::VaultTooLarge) => {
                            let id = record.borrow().id.clone();
                            area.rt.warn(&format!("vault for {id} not taken before the replacement: {}; replacing with no backup", e.message));
                            Ok(None)
                        }
                        Err(e) => Err(host_to_engine(e)),
                    }
                }
                .boxed_local()
            }));
        }
        {
            let (area, record) = (area.clone(), record.clone());
            hooks.vault_import = Some(Arc::new(move |m: MachineRef, payload: Vec<u8>| {
                let (area, record) = (area.clone(), record.clone());
                async move {
                    let leave = area.vault_leave(&record);
                    import_into(m, &payload, "/", &import_options(leave))
                        .await
                        .map(|_| ())
                }
                .boxed_local()
            }));
        }
        {
            let (area, entry, record, at) =
                (area.clone(), entry.clone(), record.clone(), at.clone());
            hooks.stash_vault = Some(Arc::new(move |m: MachineRef| {
                let (area, entry, record, at) =
                    (area.clone(), entry.clone(), record.clone(), at.clone());
                async move {
                    let Some(live) = entry.upgrade() else {
                        return Ok(());
                    };
                    if area.rt.nap_refused_of(&live).is_some() {
                        return Ok(());
                    }
                    let id = record.borrow().id.clone();
                    let machine = live.borrow().machine.clone();
                    if let Err(e) = sync_disk(machine.as_ref()).await {
                        let answer = match &e {
                            wsp_engine::disk_sync::SyncDiskError::Sync { answer, .. } => {
                                answer.clone()
                            }
                            other => other.to_string(),
                        };
                        area.rt.warn(&format!(
                            "disk sync before the nap of {id} failed: {answer}; napping anyway"
                        ));
                    }
                    if area.rt.pause_keeps_disk(&at) {
                        return Ok(());
                    }
                    let stored = match area
                        .rt
                        .vault_export(&m, area.config.vault_cap_bytes, area.vault_leave(&record))
                        .await
                    {
                        Ok(bytes) => area.rt.store().put_blob(VAULTS, &id, bytes).await,
                        Err(e) => Err(e),
                    };
                    match stored {
                        Ok(()) => {
                            let mut r = record.borrow_mut();
                            r.vaulted_at = Some(iso(area.rt.clock().now()));
                            r.vault_refused = None;
                        }
                        Err(e) => {
                            record.borrow_mut().vault_refused = Some(e.message.clone());
                            area.rt.warn(&format!(
                                "nap vault for {id} not stored, previous kept: {}",
                                e.message
                            ));
                        }
                    }
                    Ok(())
                }
                .boxed_local()
            }));
        }
        {
            let (area, record) = (area.clone(), record.clone());
            hooks.restore_vault = Some(Arc::new(move |m: MachineRef| {
                let (area, record) = (area.clone(), record.clone());
                async move {
                    let id = record.borrow().id.clone();
                    let payload = area
                        .rt
                        .store()
                        .get_blob(VAULTS, &id)
                        .await
                        .map_err(host_to_engine)?;
                    match payload {
                        Some(bytes) => {
                            let leave = area.vault_leave(&record);
                            import_into(m, &bytes, "/", &import_options(leave))
                                .await
                                .map(|_| ())
                        }
                        None => Ok(()),
                    }
                }
                .boxed_local()
            }));
        }
    }

    {
        let (area, entry, record, at) = (area.clone(), entry.clone(), record.clone(), at.clone());
        hooks.move_hook = Some(Arc::new(move |m: MachineRef, mv: ProviderMove| {
            let (area, entry, record, at) =
                (area.clone(), entry.clone(), record.clone(), at.clone());
            async move {
                let Some(live) = entry.upgrade() else {
                    return Err(EngineError::Other {
                        message: "workspace is gone".into(),
                        code: None,
                    });
                };
                match mv {
                    ProviderMove::Resume => {
                        let (resize, signal) = {
                            let e = live.borrow();
                            (e.resize_to, e.wake_stop.clone())
                        };
                        let name = record.borrow().name.clone();
                        let resumed = match resize {
                            Some(size) if m.offers().resume_at => {
                                m.resume_at(size.cpu.0, size.mem_mb.0 as u64, signal.as_ref())
                                    .await
                            }
                            _ => m.resume(signal.as_ref()).await,
                        };
                        match resumed {
                            Ok(()) => {
                                if !area.rt.names_workspace(&at) {
                                    let _ =
                                        wsp_core::types::wiring::set_hostname(m.as_ref(), &name)
                                            .await;
                                }
                                Ok(())
                            }
                            Err(e) => {
                                if matches!(e, EngineError::ResumeUnanswered { .. }) {
                                    area.rt
                                        .says_waking(&live, RESUME_UNANSWERED)
                                        .await
                                        .map_err(host_to_engine)?;
                                }
                                Err(e)
                            }
                        }
                    }
                    ProviderMove::Pause => {
                        let wait = live.borrow().nap_wait_ms;
                        m.pause(wait).await
                    }
                }
            }
            .boxed_local()
        }));
    }

    {
        let (area, entry, record) = (area.clone(), entry.clone(), record.clone());
        hooks.wake_check = Some(Arc::new(move |m: MachineRef| {
            let (area, entry, record, _at) =
                (area.clone(), entry.clone(), record.clone(), at.clone());
            async move {
                let expected = record.borrow().shape.clone();
                let mut both = String::new();
                if let (Some(expected), true) = (expected, m.offers().describe) {
                    let actual = match m.describe().await {
                        Ok(actual) => actual,
                        Err(e) => {
                            return Ok(Some(format!(
                                "provider view of {} unavailable ({})",
                                m.id(),
                                e
                            )))
                        }
                    };
                    both = both_line(&expected, &actual);
                    if let Some(fault) = shape_fault(&expected, &actual) {
                        area.rt
                            .warn(&format!("wake check on {}: {fault}; {both}", m.id()));
                        return Ok(Some(format!("{fault} on {} ({both})", m.id())));
                    }
                }
                let Some(live) = entry.upgrade() else {
                    return Ok(None);
                };
                let fault = area.rt.ping_daemon(&live).await;
                Ok(match fault {
                    None => None,
                    Some(fault) if both.is_empty() => Some(fault),
                    Some(fault) => Some(format!("{fault} ({both})")),
                })
            }
            .boxed_local()
        }));
    }

    hooks
}
