// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/places/views.ts` at 65f6b89c7: pending adds, what a remove takes off a computer, the
//! link's tunnels, and the rows a client reads. Spec: `spec/places_views.md` (rules V1-V22).
//!
//! What views reads off places-a's setup area and off `ctx.door` comes through `ViewsSetup` and `LinkReads`, so a test
//! hands fakes for both.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{select, Either};
use indexmap::{IndexMap, IndexSet};
use wsp_engine::link_backend::{LinkBackend, MachineLink};
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_engine::place_machine::PlaceMachine;
use wsp_engine::provision::{ProvisionStage, StageAt};
use wsp_engine::provision_files::{
    landed_at, machine_server_port, servers_out_lines, store_roots, unland_files, unmerge_servers,
    ServersOut,
};
use wsp_engine::target_line::stores_reached;
use wsp_store::keys::{node_base64, verify_place_bytes};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place::{
    mac_kind_of, PendingComputerFailed, PendingStep, PlacePendingEvent, PlaceProvisionRow,
    PlaceProvisionRowOutcome, PlaceViewBehind, PlaceViewBehindAct, PlaceViewForks,
    SETUP_STEP_WORDS,
};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::{PlaceJoinedEvent, PlaceRemovedEvent, PlaceView};
use wsp_wire::wire::capabilities::fork_room;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::daemon_version::{leave_asks, place_daemon_behind, place_update_line};
use wsp_wire::wire::machine_link::{place_blocked, place_no_tools_line, BackendFacts};
use wsp_wire::wire::place_link::{
    plugin_off_line, PlaceProveRequest, PLACE_CODE_REFUSAL, PLACE_KEY_REFUSAL,
};
use wsp_words::exit::HostError;
use wsp_words::views::place_setup_words::floor_failed_line;

use super::context::PlaceDoorContext;
use super::helpers::{
    bounded, first_line_of, pending_view, sign_ins_of, taken_report, ForwardConn, PendingRecord,
    ADD_FACTS_MS, ADD_NOT_TAKEN_BACK_LINE, ADD_STOPPED_FIX, ADD_STOPPED_LINE, CAPACITY_MS, PENDING,
    UNDO_MS, UNMERGE_MS,
};
use super::link::{Joining, LinkViews};
use super::records::{bare_view, engine_error, iso, js_int, store_doc, PlaceRecordsArea};
use super::types::{
    applied_view, made_by_setup, FolderComputer, MadeKind, PlaceEvent, PlaceLogin, PlaceRecord,
    PlaceRecordRoad, PlaceResult, Proved, ProvedDevice, RemovedRow, RowId, SetupEvent, CAPS,
    DEFAULT_COLLECTION, DEFAULT_ID, PLACES,
};

/// What views and manage read off `ctx.door` (views.ts:126,178,236-238,410): places-a's link reads. The door
/// assembly hands a forwarder over `ctx.door()`; a test hands a fake.
#[async_trait(?Send)]
pub trait LinkReads {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>>;
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>>;
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer>;
}

/// What views reads off places-a's setup area (views.ts:44).
pub trait ViewsSetup {
    /// `woken(placeId, back)` (setup.ts:131): wakes every wait on that computer's link.
    fn woken(&self, place_id: &str, back: bool);
    /// `closedAt.delete(placeId)` (setup.ts:128).
    fn forget_closed_at(&self, place_id: &str);
    /// `linkTo(placeId)` (setup.ts): the link a `PlaceMachine` or a `LinkBackend` rides.
    fn link_to(&self, place_id: &str) -> Arc<dyn MachineLink>;
}

/// What a remove took off over the link for the servers wsp merged (V7).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unmerged {
    pub took: Vec<String>,
    pub kept: Vec<String>,
}

/// What `plugins_off` took off and kept (V10).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PluginsOff {
    pub off: Vec<String>,
    pub kept: Vec<String>,
}

/// The views area (views.ts:38-430).
pub struct PlaceViewsArea {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<PlaceRecordsArea>,
    pub setup: Rc<dyn ViewsSetup>,
    pub links: Rc<dyn LinkReads>,
    /// V5: the pending adds whose floor runs now.
    pub flooring: Rc<RefCell<IndexSet<String>>>,
    /// V17: what each join told this host before its prove.
    pub joining: Rc<RefCell<IndexMap<String, Joining>>>,
}

/// A pending add with its place and step set, the rest kept as it stood (views.ts:66, 82).
fn moved_to(pending: &PendingRecord, place_id: &str, step: PendingStep) -> PendingRecord {
    let mut next = pending.clone();
    next.view.place_id = Some(place_id.to_string());
    next.view.step = step;
    next
}

/// The floor's stage on a computer, which drops the lines it is told (views.ts:73).
fn lines_dropped() -> ProvisionStage {
    Arc::new(|_: &str, _: Option<&StageAt>, _: Option<&PlaceProvisionRow>| {})
}

/// The row of a floor that threw, under the floor's own words (views.ts:77).
fn floor_error_row(e: &HostError) -> PlaceProvisionRow {
    let label = SETUP_STEP_WORDS
        .iter()
        .find(|(id, _)| *id == "floor")
        .map(|(_, word)| word.to_string())
        .unwrap_or_default();
    PlaceProvisionRow {
        id: "floor".to_string(),
        label,
        outcome: PlaceProvisionRowOutcome::Failed,
        kind: None,
        step: None,
        note: Some(first_line_of(e)),
        fix: None,
        keys: None,
        ms: None,
        earlier: None,
        project: None,
    }
}

impl PlaceViewsArea {
    pub fn new(
        ctx: Rc<PlaceDoorContext>,
        records: Rc<PlaceRecordsArea>,
        setup: Rc<dyn ViewsSetup>,
        links: Rc<dyn LinkReads>,
    ) -> Rc<Self> {
        Rc::new(PlaceViewsArea {
            ctx,
            records,
            setup,
            links,
            flooring: Rc::new(RefCell::new(IndexSet::new())),
            joining: Rc::new(RefCell::new(IndexMap::new())),
        })
    }

    /// V1 (:48-54).
    pub async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>> {
        let docs = self.ctx.store.list(PENDING).await?;
        let mut out: Vec<PendingRecord> = docs
            .into_iter()
            .filter_map(|doc| serde_json::from_value(doc).ok())
            .collect();
        out.sort_by(|a, b| a.view.started_at.cmp(&b.view.started_at));
        Ok(out)
    }

    /// V2 (:55-58).
    pub async fn put_pending(&self, p: &PendingRecord) -> PlaceResult<()> {
        self.ctx
            .store
            .put(
                PENDING,
                &p.view.id,
                store_doc(p).map_err(|e| HostError::new(e.to_string()))?,
            )
            .await?;
        if let Some(on_setup) = self.ctx.opts.on_setup.as_ref() {
            on_setup(&SetupEvent::Pending(PlacePendingEvent {
                id: p.view.id.clone(),
                pending: Some(pending_view(p)),
            }));
        }
        Ok(())
    }

    /// V3 (:59-62).
    pub async fn drop_pending(&self, id: &str) -> PlaceResult<()> {
        self.ctx.store.delete(PENDING, id).await?;
        if let Some(on_setup) = self.ctx.opts.on_setup.as_ref() {
            on_setup(&SetupEvent::Pending(PlacePendingEvent {
                id: id.to_string(),
                pending: None,
            }));
        }
        Ok(())
    }

    /// V4 (:66-83).
    pub async fn floor_while_choosing(
        &self,
        pending: &PendingRecord,
        place_id: &str,
    ) -> PlaceResult<()> {
        let provisioner = self.ctx.wiring.provision.clone();
        let record = self.records.record_of(place_id).await?;
        let home = record
            .as_ref()
            .and_then(|record| record.report.login.get("HOME").cloned());
        let (Some(provisioner), Some(record), Some(home)) = (provisioner, record, home) else {
            return self
                .put_pending(&moved_to(pending, place_id, PendingStep::Choosing))
                .await;
        };
        self.put_pending(&moved_to(pending, place_id, PendingStep::Floor))
            .await?;
        let machine: Rc<dyn Machine> = Rc::new(PlaceMachine::new(
            self.setup.link_to(place_id),
            &record.name,
            &home,
        ));
        let failed: Vec<PlaceProvisionRow> =
            match provisioner.floor(machine, &home, lines_dropped()).await {
                Ok(rows) => rows
                    .into_iter()
                    .filter(|row| matches!(row.outcome, PlaceProvisionRowOutcome::Failed))
                    .collect(),
                Err(e) => vec![floor_error_row(&e)],
            };
        if self
            .ctx
            .store
            .get(PENDING, &pending.view.id)
            .await?
            .is_none()
        {
            return Ok(());
        }
        let mut done = moved_to(pending, place_id, PendingStep::Choosing);
        if !failed.is_empty() {
            done.view.failed = Some(PendingComputerFailed {
                said: floor_failed_line(&failed),
                fix: None,
            });
        }
        self.put_pending(&done).await
    }

    /// V5 (:86-94).
    pub async fn floor_once(&self, pending: &PendingRecord, place_id: &str) -> PlaceResult<()> {
        self.flooring.borrow_mut().insert(pending.view.id.clone());
        let outcome = self.floor_while_choosing(pending, place_id).await;
        self.flooring.borrow_mut().shift_remove(&pending.view.id);
        outcome
    }

    /// V6 (:98-108).
    pub async fn taken_back(&self, pending: &PendingRecord) -> PlaceResult<()> {
        let said = match (
            pending.login.as_ref(),
            pending.undo.as_ref(),
            self.ctx.wiring.undo.clone(),
        ) {
            (Some(ssh), Some(script), Some(undo)) => {
                let login = PlaceLogin {
                    ssh: ssh.clone(),
                    key_path: pending.key_path.clone(),
                    host_key: pending.host_key.clone(),
                };
                match undo.undo(&login, script).await {
                    Ok(()) => ADD_STOPPED_LINE.to_string(),
                    Err(e) => format!("{ADD_NOT_TAKEN_BACK_LINE}: {}", first_line_of(&e)),
                }
            }
            _ => ADD_NOT_TAKEN_BACK_LINE.to_string(),
        };
        let mut next = pending.clone();
        next.view.failed = Some(PendingComputerFailed {
            said,
            fix: Some(ADD_STOPPED_FIX.to_string()),
        });
        self.put_pending(&next).await
    }

    /// V7 (:114-121).
    pub async fn unmerged_over(&self, place_id: &str, held: &PlaceRecord) -> Unmerged {
        let Some(home) = held.report.login.get("HOME").cloned() else {
            return Unmerged::default();
        };
        #[allow(clippy::arc_with_non_send_sync)]
        let machine: Arc<dyn Machine> = Arc::new(PlaceMachine::new(
            self.setup.link_to(place_id),
            &held.name,
            &home,
        ));
        let work = async {
            let stores = self.stores_over(place_id, &home).await?;
            unmerge_servers(&machine_server_port(Arc::clone(&machine)), &home, &stores)
                .await
                .map_err(engine_error)
        };
        let what = format!(
            "the servers wsp merged into the agents' files on {}",
            held.name
        );
        let outs: Vec<ServersOut> = bounded(&*self.ctx.clock, work, UNMERGE_MS, &what)
            .await
            .unwrap_or_default();
        Unmerged {
            took: outs
                .iter()
                .filter(|out| !matches!(out, ServersOut::Unread { .. }))
                .flat_map(servers_out_lines)
                .collect(),
            kept: outs
                .iter()
                .filter(|out| matches!(out, ServersOut::Unread { .. }))
                .flat_map(servers_out_lines)
                .collect(),
        }
    }

    /// V8 (:124-128).
    pub async fn stores_over(
        &self,
        place_id: &str,
        home: &str,
    ) -> PlaceResult<IndexMap<String, String>> {
        let Some(stores) = self.ctx.recording.stores_on(place_id, home) else {
            return Ok(IndexMap::new());
        };
        let Some(computer) = self.links.folder_computer(place_id) else {
            return Ok(IndexMap::new());
        };
        let login = computer.machine.login_of().await.map_err(engine_error)?;
        Ok(stores_reached(&login, &stores))
    }

    /// V9 (:133-144).
    pub async fn store_files_off(&self, place_id: &str, held: &PlaceRecord) -> Vec<String> {
        let Some(home) = held.report.login.get("HOME").cloned() else {
            return Vec::new();
        };
        let machine = PlaceMachine::new(self.setup.link_to(place_id), &held.name, &home);
        let work = async {
            let stores = self.stores_over(place_id, &home).await?;
            let roots = store_roots(&home, &stores);
            let dests: Vec<String> = roots.iter().map(|(from, _)| from.clone()).collect();
            let gone = unland_files(&machine, &home, &dests, &stores)
                .await
                .map(|out| out.gone)
                .unwrap_or_default();
            Ok::<Vec<String>, HostError>(
                gone.iter()
                    .map(|rel| landed_at(&home, &roots, rel))
                    .collect(),
            )
        };
        let what = format!(
            "the files wsp landed in the agents' stores on {}",
            held.name
        );
        bounded(&*self.ctx.clock, work, UNMERGE_MS, &what)
            .await
            .unwrap_or_default()
    }

    /// V10 (:150-171).
    pub async fn plugins_off(
        &self,
        place_id: &str,
        held: &PlaceRecord,
        linked: bool,
        login: Option<&PlaceLogin>,
        sudo_password: Option<&str>,
    ) -> PluginsOff {
        let Some(home) = held.report.login.get("HOME").cloned() else {
            return PluginsOff::default();
        };
        let names = made_by_setup(held, MadeKind::Plugins);
        let provisioner = self
            .ctx
            .wiring
            .provision
            .clone()
            .filter(|provisioner| provisioner.has_undo());
        let (Some(provisioner), Some(picks)) = (provisioner, held.picks.as_ref()) else {
            return PluginsOff::default();
        };
        if names.is_empty() {
            return PluginsOff::default();
        }
        let removed: Vec<RemovedRow> = names
            .iter()
            .map(|name| RemovedRow {
                kind: "plugins".to_string(),
                name: name.clone(),
            })
            .collect();
        let planned = provisioner
            .undo(picks, &removed, &home, None)
            .await
            .unwrap_or_default();
        let machine =
            linked.then(|| PlaceMachine::new(self.setup.link_to(place_id), &held.name, &home));
        let mut kept: Vec<String> = names
            .iter()
            .filter(|name| {
                !planned
                    .iter()
                    .any(|u| u.key == format!("plugins/{name}") && u.cmd.is_some())
            })
            .cloned()
            .collect();
        let mut off = Vec::new();
        for undo in &planned {
            let Some(cmd) = undo.cmd.as_deref() else {
                continue;
            };
            let ran = match (&machine, login, self.ctx.wiring.run_over.clone()) {
                (Some(machine), _, _) => machine
                    .exec(
                        cmd,
                        ExecOptions {
                            timeout_ms: Some(UNDO_MS),
                            ..Default::default()
                        },
                    )
                    .await
                    .map(|res| res.exit_code.get() == 0)
                    .unwrap_or(false),
                (None, Some(login), Some(run_over)) => run_over
                    .run_over(login, cmd, UNDO_MS, sudo_password)
                    .await
                    .map(|res| res.exit_code == 0)
                    .unwrap_or(false),
                (None, _, _) => false,
            };
            if ran {
                off.push(plugin_off_line(&undo.label));
            } else {
                kept.push(undo.label.clone());
            }
        }
        PluginsOff { off, kept }
    }

    /// V11 (:177-189).
    pub async fn facts_on(&self, place_id: &str, held: &PlaceRecord) -> PlaceRecord {
        let links = Rc::clone(&self.links);
        let records = Rc::clone(&self.records);
        let id = place_id.to_string();
        let fallback = held.clone();
        let read = crate::local_task::started(async move {
            match links.forking_backend(&id).await {
                Ok(_) => records
                    .record_of(&id)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(fallback),
                Err(_) => fallback,
            }
        });
        let (fire, fired) = futures::channel::oneshot::channel::<()>();
        let cancel = self.ctx.clock.schedule(
            Box::new(move || {
                let _ = fire.send(());
            }),
            ADD_FACTS_MS as i64,
        );
        let answer = match select(read, fired).await {
            Either::Left((record, _)) => record,
            _ => held.clone(),
        };
        cancel();
        answer
    }

    /// V12 (:193-197).
    pub fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend> {
        #[allow(clippy::arc_with_non_send_sync)]
        let made: Arc<dyn MachineBackend> =
            Arc::new(LinkBackend::of(self.setup.link_to(place_id), facts));
        self.ctx
            .backends
            .borrow_mut()
            .insert(place_id.to_string(), Arc::clone(&made));
        made
    }

    /// V13 (:201-216).
    pub fn tunnelled(&self, place_id: &str, event: &DaemonEvent) -> bool {
        let (tunnel_id, data) = match event {
            DaemonEvent::TunnelData {
                tunnel_id, data, ..
            } => (tunnel_id, Some(data)),
            DaemonEvent::TunnelEnd { tunnel_id, .. } => (tunnel_id, None),
            _ => return false,
        };
        let Some(conn) = self.conn_of(place_id, tunnel_id) else {
            return false;
        };
        match data {
            Some(data) => conn.write(&node_base64(data)),
            None => conn.end(),
        }
        true
    }

    /// The connection a forward of this place holds under `tunnel_id`, the first forward that holds one.
    fn conn_of(&self, place_id: &str, tunnel_id: &str) -> Option<Rc<dyn ForwardConn>> {
        let prefix = format!("{place_id}:");
        let forwards = self.ctx.forwards.borrow();
        for (key, forward) in forwards.iter() {
            if !key.starts_with(&prefix) {
                continue;
            }
            let conn = forward.conns.borrow().get(tunnel_id).cloned();
            if conn.is_some() {
                return conn;
            }
        }
        None
    }

    /// V14 (:220-228).
    pub fn cut(&self, place_id: &str, reason: &str) {
        let held = self.ctx.live.borrow_mut().shift_remove(place_id);
        let Some(held) = held else {
            return;
        };
        (held.seen)();
        if let Some(forward) = held.forward {
            wsp_engine::local_task::try_spawn_local(async move {
                if let Ok(forward) = forward.await {
                    forward.close();
                }
            });
        }
        held.reach.close();
        held.socket.close(1000, reason);
    }

    /// V15 (:234-256).
    pub async fn forks_of(&self, record: &PlaceRecord) -> Option<PlaceViewForks> {
        let linked = self.ctx.live.borrow().contains_key(&record.id);
        let backend = if linked {
            self.links.backend_of(&record.id)
        } else {
            None
        };
        let Some(backend) = backend else {
            if linked {
                let links = Rc::clone(&self.links);
                let id = record.id.clone();
                wsp_engine::local_task::try_spawn_local(async move {
                    let _ = links.forking_backend(&id).await;
                });
            }
            return None;
        };
        if !backend.offers().capacity {
            return None;
        }
        let work = async { backend.capacity().await.map_err(engine_error) };
        let what = format!("machine.capacity on {}", record.name);
        let capacity = bounded(&*self.ctx.clock, work, CAPACITY_MS, &what)
            .await
            .ok()?;
        let image = capacity
            .images
            .iter()
            .fold(0.0_f64, |most, image| most.max(image.size_bytes.0));
        let mem_mb = backend
            .pricing()
            .default_size()
            .mem_mb
            .min(capacity.machine_mem_mb.0);
        let room = fork_room(&capacity, mem_mb, (image != 0.0).then_some(image));
        Some(PlaceViewForks {
            running: JsInt(capacity.machines.running),
            room: js_int(room),
        })
    }

    /// V16 (:258-314).
    pub fn view_of(&self, record: &PlaceRecord, defaulted: Option<&str>) -> PlaceView {
        let report = &record.report;
        let mut row = bare_view(
            &record.id,
            PlaceKind::Computer,
            &record.name,
            defaulted == Some(record.id.as_str()),
        );
        row.os = Some(report.os.clone());
        row.mac = report.model.as_deref().and_then(mac_kind_of);
        row.shape = Some(report.shape.clone());
        row.disk_free_bytes = report.disk_free_bytes;
        row.copies = report.copies;
        row.present = Some(self.ctx.live.borrow().contains_key(&record.id));
        if report.takes_runtime == Some(true)
            && leave_asks(report.wsp_daemon_version.map(JsInt::get))
        {
            row.takes_runtime = Some(true);
        }
        row.joined_at = Some(record.joined_at.clone());
        row.last_seen_at = Some(record.last_seen_at.clone());
        row.daemon_version = Some(report.daemon_version);
        row.behind =
            place_daemon_behind(Some(report.daemon_version.get())).map(|word| PlaceViewBehind {
                word,
                fix: place_update_line(&record.name),
                act: PlaceViewBehindAct::Update,
            });
        row.agents = Some(report.agents.clone());
        row.agent_versions = report.agent_versions.clone();
        let rows: Vec<_> = record
            .applied
            .iter()
            .flat_map(|applied| applied.rows.iter().map(|held| held.row.clone()))
            .collect();
        let vault = self
            .ctx
            .opts
            .vault
            .as_ref()
            .map(|vault| vault())
            .unwrap_or_default();
        row.sign_ins = sign_ins_of(
            &report.agents,
            report.logins.as_deref(),
            &vault,
            &rows,
            None,
        );
        row.logins = record
            .backend_facts
            .as_ref()
            .and_then(|facts| facts.logins.clone());
        row.takes_forks = Some(true);
        row.road = record.road.as_ref().map(PlaceRecordRoad::view);
        row.home = report.login.get("HOME").cloned();
        row.uptime_ms = report.uptime_ms;
        row.reported_at = record.reported_at.clone();
        row.dialled = record.dialled.clone();
        row.blocked = place_blocked(
            &record.name,
            report.runs_workspaces,
            report.workspaces_blocked.as_deref(),
        );
        row.tools_blocked = place_no_tools_line(
            &record.name,
            report.wsp_door,
            report.wsp_door_blocked.as_deref(),
            report.daemon_unit.as_deref(),
        );
        row.setup = record.setup.clone();
        row.applied = record.applied.as_ref().map(applied_view);
        row.picks = record.picks.clone();
        row.recipe = record.recipe.clone();
        row.sync = record.sync.clone();
        row
    }

    /// V17 (:323-357).
    pub async fn joined(
        &self,
        place_id: &str,
        pending: Joining,
        req: PlaceProveRequest,
        expect: &[u8],
        from: &str,
        at: u64,
    ) -> PlaceResult<Proved> {
        self.joining.borrow_mut().shift_remove(place_id);
        let refused = |refusal: &str| Proved::Refusal {
            refusal: refusal.to_string(),
        };
        if !verify_place_bytes(&pending.public_key, expect, req.signature.as_str()) {
            return Ok(refused(PLACE_KEY_REFUSAL));
        }
        let taken = match taken_report(&req.report) {
            Ok(taken) => taken,
            Err(e) => return Ok(refused(&e.message)),
        };
        if let Some(blocked) = place_blocked(
            &taken.name,
            taken.runs_workspaces,
            taken.workspaces_blocked.as_deref(),
        ) {
            return Ok(refused(&blocked));
        }
        let Some(code) = req.code.clone() else {
            return Ok(refused(PLACE_CODE_REFUSAL));
        };
        if !self.ctx.devices.spend(&code, at as i64).await? {
            return Ok(refused(PLACE_CODE_REFUSAL));
        }
        let stamp = iso(at);
        let waiting = self.records.awaiting.borrow().get(&code).cloned();
        if let Some(waiting) = waiting {
            waiting.borrow_mut().place_id = Some(place_id.to_string());
        }
        let record = PlaceRecord {
            id: place_id.to_string(),
            name: taken.name.clone(),
            public_key: pending.public_key.clone(),
            joined_at: stamp.clone(),
            last_seen_at: stamp.clone(),
            report: taken.clone(),
            backend_facts: None,
            road: None,
            dialled: None,
            setup: None,
            applied: None,
            picks: None,
            recipe: None,
            sync: None,
            reported_at: Some(stamp),
        };
        let record = self.records.keep(record).await?;
        self.records.mark_default(place_id).await?;
        let client = match req.client.as_ref() {
            Some(client) => Some(self.ctx.devices.admit(&client.name, at as i64).await?),
            None => None,
        };
        let place = self.view_of(&record, Some(place_id));
        self.ctx.emit(&PlaceEvent::Joined(PlaceJoinedEvent {
            place,
            from: from.to_string(),
        }));
        Ok(Proved::Report {
            report: taken,
            device: client.map(|client| ProvedDevice {
                device_id: client.device_id,
                device_token: client.device_token,
            }),
        })
    }

    /// V18 (:360-375).
    pub async fn forget(&self, place_id: &str) -> PlaceResult<()> {
        if self.ctx.live.borrow().contains_key(place_id) {
            self.cut(place_id, "removed from this host");
        }
        self.ctx.backends.borrow_mut().shift_remove(place_id);
        let ctx = &self.ctx;
        self.records
            .in_record_turn(
                place_id,
                Box::pin(async move {
                    ctx.kept.borrow_mut().shift_remove(place_id);
                    ctx.store.delete(PLACES, place_id).await
                }),
            )
            .await?;
        self.ctx.store.delete(CAPS, place_id).await?;
        self.records
            .settings_held
            .borrow_mut()
            .shift_remove(place_id);
        let records = &self.records;
        self.records
            .in_turn(Box::pin(async move {
                if records.default_id().await?.as_deref() == Some(place_id) {
                    ctx.store.delete(DEFAULT_COLLECTION, DEFAULT_ID).await?;
                }
                Ok::<(), HostError>(())
            }))
            .await?;
        self.setup.woken(place_id, false);
        self.setup.forget_closed_at(place_id);
        self.ctx.emit(&PlaceEvent::Removed(PlaceRemovedEvent {
            place_id: place_id.to_string(),
        }));
        Ok(())
    }

    /// V19 (:378-401).
    pub fn here_row(&self, marked: &str) -> PlaceView {
        let here = (self.ctx.wiring.here)();
        let daemon_version = self
            .ctx
            .opts
            .here_daemon
            .as_ref()
            .and_then(|daemon| daemon.held())
            .flatten();
        let behind =
            daemon_version.and_then(|version| place_daemon_behind(Some(i64::from(version))));
        let mut row = bare_view(
            HERE_PLACE_ID,
            PlaceKind::Computer,
            &here.name,
            marked == HERE_PLACE_ID,
        );
        row.label = here.label;
        row.mac = here.mac;
        row.os = here.os;
        row.shape = here.shape;
        row.disk_free_bytes = here.disk_free_bytes.map(|bytes| js_int(bytes as f64));
        row.home = here.home;
        row.daemon_version = daemon_version.map(|version| JsInt::from(i64::from(version)));
        if let (Some(word), Some(daemon)) = (behind, self.ctx.opts.here_daemon.as_ref()) {
            row.behind = Some(PlaceViewBehind {
                word,
                fix: daemon.fix(),
                act: PlaceViewBehindAct::Install,
            });
        }
        row.present = Some(true);
        row.takes_forks = Some(false);
        row.builds_images = Some(false);
        row
    }

    /// V20 (:404-408).
    pub async fn rows_of(&self, held: &[PlaceRecord]) -> PlaceResult<Vec<PlaceView>> {
        let marked = self
            .records
            .mark_held()
            .await?
            .unwrap_or_else(|| HERE_PLACE_ID.to_string());
        let mut rows = vec![self.here_row(&marked)];
        rows.extend(held.iter().map(|record| self.joined_row(record, &marked)));
        rows.extend(
            self.records
                .provider_ids()
                .iter()
                .map(|id| self.provider_row(id, &marked)),
        );
        let ids: Vec<RowId> = rows
            .iter()
            .map(|row| RowId {
                id: row.id.clone(),
                kind: row.kind,
            })
            .collect();
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(self.records.with_cap(row, &ids).await?);
        }
        Ok(out)
    }

    /// V21 (:410).
    pub fn joined_row(&self, record: &PlaceRecord, marked: &str) -> PlaceView {
        let mut row = self.view_of(record, Some(marked));
        let backend = self.links.backend_of(&record.id);
        self.records
            .image_facts(&mut row, &record.id, backend.as_ref());
        row
    }

    /// V22 (:411-424).
    pub fn provider_row(&self, id: &str, marked: &str) -> PlaceView {
        let mut row = bare_view(id, PlaceKind::Provider, id, marked == id);
        row.takes_forks = Some(true);
        row.rate_usd_per_hour = self.records.provider_rate(id).map(JsNumber);
        let sizes = self.records.provider_sizes(id);
        if !sizes.is_empty() {
            row.sizes = Some(sizes);
        }
        let backend = self.records.provider_backend(id);
        self.records.image_facts(&mut row, id, backend.as_ref());
        row
    }
}

/// places-a's link reads off this area (link.rs `LinkViews`): each a forward to the method above of the same name.
#[async_trait(?Send)]
impl LinkViews for PlaceViewsArea {
    async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>> {
        PlaceViewsArea::pending_records(self).await
    }
    async fn put_pending(&self, p: PendingRecord) -> PlaceResult<()> {
        PlaceViewsArea::put_pending(self, &p).await
    }
    fn flooring(&self) -> Rc<RefCell<IndexSet<String>>> {
        Rc::clone(&self.flooring)
    }
    async fn floor_once(&self, pending: PendingRecord, place_id: &str) {
        let _ = PlaceViewsArea::floor_once(self, &pending, place_id).await;
    }
    async fn taken_back(&self, pending: PendingRecord) {
        let _ = PlaceViewsArea::taken_back(self, &pending).await;
    }
    fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend> {
        PlaceViewsArea::backend_from(self, place_id, facts)
    }
    fn tunnelled(&self, place_id: &str, e: &DaemonEvent) -> bool {
        PlaceViewsArea::tunnelled(self, place_id, e)
    }
    fn cut(&self, place_id: &str, reason: &str) {
        PlaceViewsArea::cut(self, place_id, reason)
    }
    fn joining(&self) -> Rc<RefCell<IndexMap<String, Joining>>> {
        Rc::clone(&self.joining)
    }
    async fn joined(
        &self,
        place_id: &str,
        pending: Joining,
        req: PlaceProveRequest,
        expect: &[u8],
        from: &str,
        at: u64,
    ) -> PlaceResult<Proved> {
        PlaceViewsArea::joined(self, place_id, pending, req, expect, from, at).await
    }
    async fn joined_row(&self, record: &PlaceRecord, marked: &str) -> PlaceResult<PlaceView> {
        Ok(PlaceViewsArea::joined_row(self, record, marked))
    }
}
