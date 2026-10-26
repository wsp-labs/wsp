// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/link.ts` at 65f6b89c7: the door's half that joins, proves and holds each computer's
//! link, and answers what a link and a place say. Specs: `spec/places_link_door.md` (`answer_challenge` to `load`)
//! and `spec/places_link_reads.md` (`name_of` to the end, and `land_sign_in`).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use crate::local_task::spawn as spawn_local;
use async_trait::async_trait;
use futures::channel::mpsc;
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use tokio::net::TcpStream;
use wsp_core::console::warn;
use wsp_core::ids::{Ids, OsIds};
use wsp_core::types::pending::DaemonReach;
use wsp_engine::link_backend::{LinkAsk, LinkBackend};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_engine::place_machine::{PlaceFolderMachine, PlaceMachine};
use wsp_engine::provision::{EngineStep, ProvisionOn, ProvisionStage, SetupRun};
use wsp_store::keys::{key_fingerprint, verify_place_bytes, Seal};
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::types::recipe_file::{
    GitHubSignIn, RecipeFile, RecipeFileConfigs, RecipeFileConfigsGithub, NO_RECIPE,
};
use wsp_wire::views::place::{
    sign_in_row_id, PendingStep, PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceSetupState,
    PlaceSetupStep, PlaceSyncState, PlaceWaitState, GITHUB_CLI, GITHUB_ROW,
};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::{
    PlaceAbsentEvent, PlaceCap, PlaceCapSet, PlaceChangedEvent, PlacePresentEvent, PlaceSettings,
    PlaceView,
};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::daemon_version::place_daemon_behind;
use wsp_wire::wire::machine_link::{no_such_place_refusal, place_forks_nowhere_line, BackendFacts};
use wsp_wire::wire::place_link::{
    PlaceAuthRefusal, PlaceAuthReply, PlaceAuthRequest, PlaceJoinReply, PlaceJoinRequest,
    PlaceProveRequest, PlaceReport, PLACE_KEY_REFUSAL, PLACE_UNKNOWN_REFUSAL,
    PLACE_UNSEALED_JOIN_REFUSAL,
};
use wsp_words::exit::HostError;
use wsp_words::place_state::{place_cap_of, place_turn_limit};
use wsp_words::place_word::{cloud_named, names_place, provider_key_name, PlaceRef};

use super::context::PlaceDoorContext;
use super::helpers::{
    bounded, landed_on, picked_vault, reads_as_ed25519, relisted_logins, shared_logins_script,
    taken_report, vault_held_line, vault_sign_in, Forward, HeldForward, LandedRow, LinkSocket,
    Live, PendingRecord, BACKEND_FACTS_MS, JOIN_PROVE_MS, LOGINS_READ_MS, PLACE_BAD_KEY_REFUSAL,
    REPLACED,
};
use super::pane_ports::{PaneAsker, PaneOnConn, PanePorts, PanePortsDeps};
use super::readings::{PlaceReadings, Reader, Release};
use super::setup::{current_folders_of, host_of, iso_at, PlaceSetupArea, SetupRecords};
use super::types::{
    is_place_provisioning, place_forks_nowhere, place_provisioning, AuthAnswer, FolderComputer,
    ForwardAsk, HeldRow, JoinAnswer, PlaceChallenge, PlaceEvent, PlaceExecOpts, PlaceFor,
    PlaceRecord, PlaceRecordRoad, PlaceResult, Proved, RowId, SetupOn, CAPS, DEFAULT_COLLECTION,
    DEFAULT_ID,
};

/// A join answered and waiting for its prove (views.ts:319).
mod channel;
use channel::{every, listen_loopback, run_tunnel, Conns, TunnelConn};
pub use channel::{ChannelGone, PlaceChannel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joining {
    pub public_key: String,
    pub at: u64,
}

/// What the link door reads off the records area (link.ts:49-52). Slice b's `PlaceRecordsArea` implements it.
#[async_trait(?Send)]
pub trait LinkRecords: SetupRecords {
    /// Every record, sorted by `joinedAt`.
    async fn records(&self) -> PlaceResult<Vec<PlaceRecord>>;
    fn wired_provider(&self) -> Option<String>;
    fn provider_ids(&self) -> Vec<String>;
    fn provider_backend(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>>;
    async fn settings_of(&self, place_id: &str) -> PlaceResult<PlaceSettings>;
    /// The settings held by place id, which `load` fills and the reads answer from.
    fn settings_held(&self) -> Rc<RefCell<IndexMap<String, PlaceSettings>>>;
    /// The `woken` call of every wait in `awaiting` for this place, in order (none taken off the list).
    fn awaiting_for(&self, place_id: &str) -> Vec<Rc<dyn Fn(&str)>>;
    fn hold_back(&self, record: &PlaceRecord);
    async fn default_id(&self) -> PlaceResult<Option<String>>;
    /// Runs `write` in the records' one turn for the default mark (records.ts:283).
    async fn in_turn(
        &self,
        write: Box<dyn FnOnce() -> futures::future::LocalBoxFuture<'static, PlaceResult<()>>>,
    ) -> PlaceResult<()>;
    async fn mark_default(&self, place_id: &str) -> PlaceResult<()>;
    async fn mark_held(&self) -> PlaceResult<Option<String>>;
    fn challenge(
        &self,
        subject: &str,
        their_nonce: &str,
        their_ephemeral: Option<&str>,
    ) -> Option<PlaceChallenge>;
    fn signed_refusal(&self, place_id: &str, place_nonce: &str, sentence: &str)
        -> PlaceAuthRefusal;
    async fn write_seen(&self, place_id: &str, at: u64) -> PlaceResult<()>;
    async fn with_cap(&self, row: PlaceView, ids: &[RowId]) -> PlaceResult<PlaceView>;
    async fn row_ids(&self) -> PlaceResult<Vec<RowId>>;
}

/// What the link door reads off the views area (link.ts:55). Slice b's `PlaceViewsArea` implements it.
#[async_trait(?Send)]
pub trait LinkViews {
    async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>>;
    async fn put_pending(&self, p: PendingRecord) -> PlaceResult<()>;
    /// The pending adds whose floor is running.
    fn flooring(&self) -> Rc<RefCell<IndexSet<String>>>;
    async fn floor_once(&self, pending: PendingRecord, place_id: &str);
    async fn taken_back(&self, pending: PendingRecord);
    fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend>;
    /// Whether the event was a tunnel's bytes this forward owns, taken.
    fn tunnelled(&self, place_id: &str, e: &DaemonEvent) -> bool;
    fn cut(&self, place_id: &str, reason: &str);
    fn joining(&self) -> Rc<RefCell<IndexMap<String, Joining>>>;
    async fn joined(
        &self,
        place_id: &str,
        pending: Joining,
        req: PlaceProveRequest,
        expect: &[u8],
        from: &str,
        at: u64,
    ) -> PlaceResult<Proved>;
    async fn joined_row(&self, record: &PlaceRecord, marked: &str) -> PlaceResult<PlaceView>;
}

/// The one picked-up cap of a row of `kind` on `shape`, read as `placeCapOf` reads it (place-state.ts:241).
fn cap_of(kind: &str, shape: Option<WorkspaceSize>, settings: &PlaceSettings) -> Option<PlaceCap> {
    let mut view: PlaceView =
        serde_json::from_value(json!({ "id": "", "kind": kind, "name": "", "default": false }))
            .ok()?;
    view.shape = shape;
    let set = PlaceCapSet {
        threads: settings.threads,
        machines: settings.machines,
        spend_per_day_usd: settings.spend_per_day_usd,
    };
    place_cap_of(&view, Some(&set))
}

fn empty_settings() -> PlaceSettings {
    PlaceSettings {
        threads: None,
        machines: None,
        spend_per_day_usd: None,
        nap_ms: None,
        turn_limit_ms: None,
        spawn: None,
        size: None,
        delete_after_ms: None,
    }
}

/// The picks a computer that has none is planned from: its name, as `RecipeFile.parse({ name })` reads it.
fn picks_named(name: &str) -> Option<RecipeFile> {
    serde_json::from_value(json!({ "name": name })).ok()
}

/// One read of a place's backend facts, kept on its record if the daemon that answered is still the one there
/// (link.ts:439-447).
async fn read_facts(
    door: &Rc<LinkDoor>,
    place_id: &str,
    record: &PlaceRecord,
) -> PlaceResult<Arc<dyn MachineBackend>> {
    let link = door.setup.link_to(place_id);
    let asked = {
        let link = Arc::clone(&link);
        async move {
            link.request("machine.backend", Map::new(), LinkAsk::default())
                .await
                .map_err(|e| host_of(&e))
        }
    };
    let answer = bounded(
        &*door.ctx.clock,
        asked,
        BACKEND_FACTS_MS,
        &format!("machine.backend on {}", record.name),
    )
    .await?;
    let facts: BackendFacts =
        serde_json::from_value(Value::Object(answer)).map_err(|e| HostError::new(e.to_string()))?;
    let stamp = facts.clone();
    let daemon = record.report.daemon_version;
    let wrote = door
        .records
        .change(
            place_id,
            Box::new(move |now: &PlaceRecord| {
                (now.report.daemon_version == daemon).then(|| PlaceRecord {
                    backend_facts: Some(stamp),
                    ..now.clone()
                })
            }),
        )
        .await?;
    Ok(match wrote {
        None => Arc::new(LinkBackend::of(link, facts)),
        Some(_) => door.views.backend_from(place_id, facts),
    })
}

/// The link door (link.ts:45). State beyond the context and the setup area is memory only.
pub struct LinkDoor {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<dyn LinkRecords>,
    pub setup: Rc<PlaceSetupArea>,
    pub views: Rc<dyn LinkViews>,
    pub panes: Rc<PanePorts>,
    pub readings: Rc<PlaceReadings>,
    /// One folder machine per place while its home and PATH stand, keyed `"{home}\0{path}"`.
    folder_machines:
        RefCell<IndexMap<String, (String, Arc<wsp_engine::place_machine::PlaceFolderMachine>)>>,
    /// The tunnel counter, one per door, from 1.
    tunnel_seq: Cell<u64>,
    /// The channel reader counter, one per door, from 1.
    reader_seq: Cell<u64>,
}

impl LinkDoor {
    pub fn new(
        ctx: Rc<PlaceDoorContext>,
        records: Rc<dyn LinkRecords>,
        setup: Rc<PlaceSetupArea>,
        views: Rc<dyn LinkViews>,
    ) -> Rc<LinkDoor> {
        let panes = PanePorts::new(PanePortsDeps {
            forwards: Rc::clone(&ctx.forwards),
            clock: Rc::clone(&ctx.clock),
        });
        let readings = PlaceReadings::new(Rc::clone(&ctx));
        Rc::new(LinkDoor {
            ctx,
            records,
            setup,
            views,
            panes,
            readings,
            folder_machines: RefCell::new(IndexMap::new()),
            tunnel_seq: Cell::new(0),
            reader_seq: Cell::new(0),
        })
    }

    /// A sign-in finished outside the setup lands the setup's own row for it (link.ts:61).
    pub async fn land_sign_in(self: &Rc<Self>, place_id: &str, row: LandedRow) -> PlaceResult<()> {
        let lander = self.setup.landers.borrow().get(place_id).cloned();
        let landed = Rc::new(Cell::new(false));
        let moved = match lander {
            Some(lander) => {
                let took = lander(row).await;
                landed.set(took);
                if took {
                    self.records.record_of(place_id).await?
                } else {
                    None
                }
            }
            None => {
                let (flag, row) = (Rc::clone(&landed), row);
                self.records
                    .change(
                        place_id,
                        Box::new(move |now: &PlaceRecord| {
                            let applied = now.applied.as_ref()?;
                            let plain: Vec<PlaceProvisionRow> =
                                applied.rows.iter().map(|r| r.row.clone()).collect();
                            let rows = landed_on(&plain, &row)?;
                            flag.set(true);
                            let held = rows
                                .into_iter()
                                .enumerate()
                                .map(|(at, landed_row)| match applied.rows.get(at) {
                                    Some(stood) if stood.row == landed_row => stood.clone(),
                                    _ => HeldRow {
                                        row: landed_row,
                                        pick: None,
                                        created_at: None,
                                    },
                                })
                                .collect();
                            let mut next = now.clone();
                            if let Some(applied) = next.applied.as_mut() {
                                applied.rows = held;
                            }
                            Some(next)
                        }),
                    )
                    .await?
            }
        };
        if let (Some(moved), true) = (moved, landed.get()) {
            let marked = self
                .records
                .mark_held()
                .await?
                .unwrap_or_else(|| HERE_PLACE_ID.to_string());
            let view = self.views.joined_row(&moved, &marked).await?;
            let ids = self.records.row_ids().await?;
            let place = self.records.with_cap(view, &ids).await?;
            self.ctx
                .emit(&PlaceEvent::Changed(PlaceChangedEvent { place }));
        }
        Ok(())
    }

    pub fn answer_challenge(
        &self,
        subject: &str,
        nonce: &str,
        ephemeral: Option<&str>,
    ) -> Option<PlaceChallenge> {
        self.records.challenge(subject, nonce, ephemeral)
    }

    /// link.ts:82. `id` draws 8 random bytes through the clock's ids seam.
    pub async fn join(
        self: &Rc<Self>,
        req: PlaceJoinRequest,
        from: &str,
        at: u64,
    ) -> PlaceResult<JoinAnswer> {
        let _ = from;
        let public_key = req.public_key.as_str().to_string();
        if !reads_as_ed25519(&public_key) {
            return Err(HostError::new(PLACE_BAD_KEY_REFUSAL));
        }
        let id = format!("p_{}", hex::encode(OsIds.bytes(8)));
        let nonce = req.nonce.as_str().to_string();
        let ephemeral = req.ephemeral.as_ref().map(|e| e.as_str().to_string());
        let opened = self
            .records
            .challenge(&id, &nonce, ephemeral.as_deref())
            .ok_or_else(|| HostError::new(PLACE_UNSEALED_JOIN_REFUSAL))?;
        let joining = self.views.joining();
        {
            let mut held = joining.borrow_mut();
            held.retain(|_, pending| (at as i64) - (pending.at as i64) <= JOIN_PROVE_MS as i64);
            held.insert(id.clone(), Joining { public_key, at });
        }
        let reply = serde_json::from_value::<PlaceJoinReply>(json!({
            "placeId": id,
            "hostPublicKey": opened.host_public_key,
            "nonce": opened.nonce,
            "signature": opened.signature,
            "ephemeral": opened.ephemeral,
            "hostName": (self.ctx.wiring.host_name)(),
        }))
        .map_err(|e| HostError::new(e.to_string()))?;
        Ok(JoinAnswer {
            reply,
            expect: opened.expect,
            seal: opened.seal,
            notice: None,
        })
    }

    /// link.ts:102.
    pub async fn auth(self: &Rc<Self>, req: PlaceAuthRequest) -> PlaceResult<AuthAnswer> {
        let place = req.place_id.clone();
        let nonce = req.nonce.as_str().to_string();
        let ephemeral = req.ephemeral.as_ref().map(|e| e.as_str().to_string());
        let Some(held) = self.records.record_of(&place).await? else {
            let signed = self
                .records
                .signed_refusal(&place, &nonce, PLACE_UNKNOWN_REFUSAL);
            return Ok(AuthAnswer::Refusal {
                refusal: PLACE_UNKNOWN_REFUSAL.to_string(),
                signed,
            });
        };
        let Some(opened) = self.records.challenge(&place, &nonce, ephemeral.as_deref()) else {
            let behind = place_daemon_behind(Some(held.report.daemon_version.as_f64() as i64))
                .unwrap_or_default();
            return Err(HostError::new(
                wsp_wire::wire::daemon_version::place_behind_line(&held.name, &behind),
            ));
        };
        let reply = serde_json::from_value::<PlaceAuthReply>(json!({
            "nonce": opened.nonce,
            "hostPublicKey": opened.host_public_key,
            "signature": opened.signature,
            "ephemeral": opened.ephemeral,
        }))
        .map_err(|e| HostError::new(e.to_string()))?;
        Ok(AuthAnswer::Reply {
            reply,
            expect: opened.expect,
            seal: opened.seal,
        })
    }

    /// link.ts:116.
    pub fn host_key(&self) -> String {
        key_fingerprint(&self.ctx.wiring.host_key.public_key)
    }

    /// link.ts:120.
    pub async fn prove(
        self: &Rc<Self>,
        place_id: &str,
        req: PlaceProveRequest,
        expect: &[u8],
        from: &str,
        at: u64,
    ) -> PlaceResult<Proved> {
        let pending = self.views.joining().borrow().get(place_id).cloned();
        if let Some(pending) = pending {
            return self
                .views
                .joined(place_id, pending, req, expect, from, at)
                .await;
        }
        let refused = Proved::Refusal {
            refusal: PLACE_KEY_REFUSAL.to_string(),
        };
        let Some(held) = self.records.record_of(place_id).await? else {
            return Ok(refused);
        };
        if !verify_place_bytes(&held.public_key, expect, req.signature.as_str()) {
            return Ok(refused);
        }
        match taken_report(&req.report) {
            Ok(report) => Ok(Proved::Report {
                report,
                device: None,
            }),
            Err(e) => Ok(Proved::Refusal { refusal: e.message }),
        }
    }

    /// link.ts:137.
    pub async fn attach(
        self: &Rc<Self>,
        place_id: &str,
        socket: Rc<dyn LinkSocket>,
        report: PlaceReport,
        from: &str,
        at: u64,
        seal: Option<Seal>,
    ) -> PlaceResult<()> {
        self.views.cut(place_id, REPLACED);
        if self.records.record_of(place_id).await?.is_none() {
            socket.close(1000, "this host no longer holds that place");
            return Ok(());
        }
        let stamp = iso_at(at);
        let newer = Rc::new(Cell::new(false));
        let moved = {
            let (newer, from) = (Rc::clone(&newer), from.to_string());
            let (stamp_seen, stamp_reported) = (stamp.clone(), stamp.clone());
            self.records
                .change(
                    place_id,
                    Box::new(move |now: &PlaceRecord| {
                        newer.set(now.report.daemon_version != report.daemon_version);
                        let road = PlaceRecordRoad {
                            from: Some(from),
                            ..now.road.clone().unwrap_or_default()
                        };
                        let mut next = PlaceRecord {
                            report,
                            last_seen_at: stamp_seen,
                            reported_at: Some(stamp_reported),
                            road: Some(road),
                            dialled: None,
                            ..now.clone()
                        };
                        if newer.get() {
                            next.backend_facts = None;
                        }
                        Some(next)
                    }),
                )
                .await?
        };
        let Some(moved) = moved else {
            socket.close(1000, "this host no longer holds that place");
            return Ok(());
        };
        if newer.get() {
            self.ctx.backends.borrow_mut().shift_remove(place_id);
            self.ctx.asking.borrow_mut().shift_remove(place_id);
        }
        let reach = Rc::clone(&socket).connect(seal, self.route_into(place_id));
        let seen = every(
            Rc::clone(&self.ctx.clock),
            self.ctx.seen_every_ms,
            self.seen_tick(place_id),
        );
        self.ctx.live.borrow_mut().insert(
            place_id.to_string(),
            Live {
                socket: Rc::clone(&socket),
                reach: Rc::clone(&reach),
                seen,
                forward: None,
            },
        );
        self.readings.linked(place_id);
        self.setup.woken(place_id, true);
        {
            let door = Rc::clone(self);
            let (place, name) = (place_id.to_string(), moved.name.clone());
            spawn_local(async move {
                if let Err(e) = door.forking_backend(&place).await {
                    if !is_place_provisioning(&e) {
                        warn(&format!(
                            "{name} did not say what it forks with: {}",
                            e.message
                        ));
                    }
                }
            });
        }
        let weak = Rc::downgrade(self);
        let (closing_place, closing_socket) = (place_id.to_string(), Rc::clone(&socket));
        socket.on_close(Box::new(move |_code, _reason| {
            if let Some(door) = weak.upgrade() {
                door.attach_closed(&closing_place, &closing_socket);
            }
        }));
        self.ctx.emit(&PlaceEvent::Present(PlacePresentEvent {
            place_id: place_id.to_string(),
            from: from.to_string(),
        }));
        for pending_wake in self.records.awaiting_for(place_id) {
            pending_wake(place_id);
        }
        let current = self.records.record_of(place_id).await?;
        match current {
            Some(current)
                if current
                    .setup
                    .as_ref()
                    .is_some_and(|s| s.state == PlaceSetupState::Running)
                    && !self.setup.setting.borrow().contains(place_id) =>
            {
                let add_id = current
                    .setup
                    .as_ref()
                    .map(|s| s.add_id.clone())
                    .unwrap_or_default();
                let (setup, place) = (Rc::clone(&self.setup), place_id.to_string());
                spawn_local(async move {
                    setup.started_or_said(&place, &add_id, None).await;
                });
            }
            Some(current)
                if current.recipe.as_deref().is_some_and(|r| r != NO_RECIPE)
                    && current
                        .setup
                        .as_ref()
                        .is_some_and(|s| s.state == PlaceSetupState::Done) =>
            {
                self.setup.sync_soon(place_id, 0);
            }
            _ => {}
        }
        for pending in self.views.pending_records().await? {
            if pending.view.place_id.as_deref() != Some(place_id)
                || pending.view.step != PendingStep::Floor
            {
                continue;
            }
            if self.views.flooring().borrow().contains(&pending.view.id) {
                continue;
            }
            let (views, place) = (Rc::clone(&self.views), place_id.to_string());
            spawn_local(async move {
                views.floor_once(pending, &place).await;
            });
        }
        Ok(())
    }

    /// What a daemon event on a place's link is routed to (link.ts:172-181).
    fn route_into(self: &Rc<Self>, place_id: &str) -> Rc<dyn Fn(&DaemonEvent)> {
        let door = Rc::downgrade(self);
        let place = place_id.to_string();
        Rc::new(move |e: &DaemonEvent| {
            if let Some(door) = door.upgrade() {
                door.route(&place, e);
            }
        })
    }

    fn route(&self, place_id: &str, e: &DaemonEvent) {
        if self.views.tunnelled(place_id, e) {
            return;
        }
        let forward: Option<HeldForward> = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .and_then(|held| held.forward.clone());
        if let Some(forward) = forward {
            let e = e.clone();
            spawn_local(async move {
                if let Ok(forward) = forward.await {
                    forward.event(&e);
                }
            });
        }
        self.readings.heard(place_id, e);
        let readers: Vec<Rc<dyn Fn(&Map<String, Value>)>> = self
            .setup
            .channels
            .borrow()
            .get(place_id)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        if !readers.is_empty() {
            if let Ok(Value::Object(as_map)) = serde_json::to_value(e) {
                for read in readers {
                    read(&as_map);
                }
            }
        }
        if let Some(on_event) = &self.ctx.opts.on_daemon_event {
            on_event(place_id, e);
        }
    }

    /// The last-seen write each interval the link is up (link.ts:184).
    fn seen_tick(&self, place_id: &str) -> Rc<dyn Fn()> {
        let records = Rc::clone(&self.records);
        let clock = Rc::clone(&self.ctx.clock);
        let place = place_id.to_string();
        Rc::new(move || {
            let (records, place) = (Rc::clone(&records), place.clone());
            let at = clock.now() as u64;
            spawn_local(async move {
                let _ = records.write_seen(&place, at).await;
            });
        })
    }

    /// The close of the socket an attach opened: the place reads absent, unless a newer link took its place already.
    fn attach_closed(&self, place_id: &str, socket: &Rc<dyn LinkSocket>) {
        let mine = {
            let mut live = self.ctx.live.borrow_mut();
            let current = live
                .get(place_id)
                .is_some_and(|held| Rc::ptr_eq(&held.socket, socket));
            if current {
                live.shift_remove(place_id)
            } else {
                None
            }
        };
        let Some(mine) = mine else {
            return;
        };
        self.setup
            .closed_at
            .borrow_mut()
            .insert(place_id.to_string(), self.ctx.clock.now() as u64);
        (mine.seen)();
        if let Some(forward) = mine.forward {
            spawn_local(async move {
                if let Ok(forward) = forward.await {
                    forward.close();
                }
            });
        }
        mine.reach.close();
        self.write_seen_later(place_id);
        self.ctx.emit(&PlaceEvent::Absent(PlaceAbsentEvent {
            place_id: place_id.to_string(),
            said: None,
        }));
    }

    fn write_seen_later(&self, place_id: &str) {
        let records = Rc::clone(&self.records);
        let at = self.ctx.clock.now() as u64;
        let place = place_id.to_string();
        spawn_local(async move {
            let _ = records.write_seen(&place, at).await;
        });
    }

    /// link.ts:228.
    pub fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        self.ctx
            .live
            .borrow()
            .get(place_id)
            .map(|held| Rc::clone(&held.reach))
    }

    /// link.ts:230.
    pub async fn watch_sys(self: &Rc<Self>, place_id: &str, f: Reader) -> PlaceResult<Release> {
        self.readings.watch(place_id, f).await
    }

    /// link.ts:232.
    pub fn channel(
        self: &Rc<Self>,
        place_id: &str,
        on_event: Rc<dyn Fn(&Map<String, Value>)>,
    ) -> Option<Rc<PlaceChannel>> {
        let (link, socket) = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .map(|held| (Rc::clone(&held.reach), Rc::clone(&held.socket)))?;
        let reader = self.reader_seq.get() + 1;
        self.reader_seq.set(reader);
        self.setup
            .channels
            .borrow_mut()
            .entry(place_id.to_string())
            .or_default()
            .insert(reader, on_event);
        Some(PlaceChannel::open(self, place_id, link, socket, reader))
    }

    /// link.ts:291.
    pub async fn load(self: &Rc<Self>) -> PlaceResult<()> {
        for place_id in self.ctx.store.keys(CAPS).await? {
            let settings = self.records.settings_of(&place_id).await?;
            self.records
                .settings_held()
                .borrow_mut()
                .insert(place_id, settings);
        }
        for record in self.records.records().await? {
            self.ctx
                .kept
                .borrow_mut()
                .insert(record.id.clone(), record.clone());
            if let Some(facts) = record.backend_facts.clone() {
                if !self.ctx.backends.borrow().contains_key(&record.id) {
                    self.views.backend_from(&record.id, facts);
                }
            }
            self.records.hold_back(&record);
            if record.setup.as_ref().is_some_and(|s| {
                s.state != PlaceSetupState::Running
                    && s.waiting.iter().any(|w| w.state == PlaceWaitState::Waiting)
            }) {
                self.records
                    .change(
                        &record.id,
                        Box::new(|now: &PlaceRecord| {
                            let setup = now.setup.clone()?;
                            let waiting = setup
                                .waiting
                                .iter()
                                .cloned()
                                .map(|w| wsp_wire::views::place::PlaceWait {
                                    state: PlaceWaitState::Expired,
                                    ..w
                                })
                                .collect();
                            Some(PlaceRecord {
                                setup: Some(wsp_wire::views::place::PlaceSetup {
                                    waiting,
                                    ..setup
                                }),
                                ..now.clone()
                            })
                        }),
                    )
                    .await?;
            }
            if record
                .sync
                .as_ref()
                .is_some_and(|s| s.state == PlaceSyncState::Running)
            {
                self.records
                    .change(
                        &record.id,
                        Box::new(|now: &PlaceRecord| {
                            let sync = now
                                .sync
                                .clone()
                                .filter(|s| s.state == PlaceSyncState::Running)?;
                            Some(PlaceRecord {
                                sync: Some(wsp_wire::views::place::PlaceSync {
                                    state: PlaceSyncState::Behind,
                                    ..sync
                                }),
                                ..now.clone()
                            })
                        }),
                    )
                    .await?;
            }
        }
        for pending in self.views.pending_records().await? {
            if pending.view.step != PendingStep::Wsp
                || pending.view.place_id.is_some()
                || pending.view.failed.is_some()
            {
                continue;
            }
            let joined = pending.login.as_ref().and_then(|login| {
                self.ctx
                    .kept
                    .borrow()
                    .values()
                    .find(|r| {
                        r.road.as_ref().and_then(|road| road.ssh.as_deref()) == Some(login.as_str())
                    })
                    .map(|r| r.id.clone())
            });
            if let Some(joined) = joined {
                let mut next = pending;
                next.view.place_id = Some(joined);
                next.view.step = PendingStep::Floor;
                self.views.put_pending(next).await?;
                continue;
            }
            let views = Rc::clone(&self.views);
            spawn_local(async move {
                views.taken_back(pending).await;
            });
        }
        Ok(())
    }

    /// link.ts:317.
    pub fn name_of(&self, place_id: &str) -> String {
        if place_id == HERE_PLACE_ID {
            return (self.ctx.wiring.here)().name;
        }
        match self.ctx.kept.borrow().get(place_id) {
            Some(record) => record.name.clone(),
            None => provider_key_name(place_id),
        }
    }

    /// link.ts:319.
    pub fn settings_at(&self, place_id: &str) -> PlaceSettings {
        self.records
            .settings_held()
            .borrow()
            .get(place_id)
            .cloned()
            .unwrap_or_else(empty_settings)
    }

    /// link.ts:321. `None` no row; `Some(None)` no limit.
    pub fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> {
        let kind = if place_id == HERE_PLACE_ID || self.ctx.kept.borrow().contains_key(place_id) {
            PlaceKind::Computer
        } else if self.records.provider_ids().iter().any(|id| id == place_id) {
            PlaceKind::Provider
        } else {
            return None;
        };
        Some(place_turn_limit(&kind, &self.settings_at(place_id)).map(|ms| ms as u64))
    }

    /// link.ts:326.
    pub fn threads_at(&self, place_id: &str) -> Option<u64> {
        if place_id != HERE_PLACE_ID && !self.ctx.kept.borrow().contains_key(place_id) {
            return None;
        }
        let shape = if place_id == HERE_PLACE_ID {
            (self.ctx.wiring.here)().shape
        } else {
            self.ctx
                .kept
                .borrow()
                .get(place_id)
                .map(|r| r.report.shape.clone())
        };
        match cap_of("computer", shape, &self.settings_at(place_id))? {
            PlaceCap::ComputerCap(cap) => Some(cap.threads.as_f64() as u64),
            PlaceCap::CloudCap(_) => None,
        }
    }

    /// link.ts:333.
    pub fn machines_at(&self, place_id: &str) -> Option<u64> {
        if !self.records.provider_ids().iter().any(|id| id == place_id) {
            return None;
        }
        match cap_of("provider", None, &self.settings_at(place_id))? {
            PlaceCap::CloudCap(cap) => Some(cap.machines.as_f64() as u64),
            PlaceCap::ComputerCap(_) => None,
        }
    }

    /// link.ts:339.
    pub fn sign_ins_at(
        &self,
        place_id: &str,
        agents: Option<&[String]>,
    ) -> Option<IndexMap<String, AgentSignInState>> {
        self.ctx.sign_ins_here(place_id, agents)
    }

    /// link.ts:341.
    pub fn vault_at(
        &self,
        place_id: &str,
        vault: &IndexMap<String, String>,
    ) -> IndexMap<String, String> {
        let picks = self
            .ctx
            .kept
            .borrow()
            .get(place_id)
            .and_then(|r| r.picks.clone());
        picked_vault(vault, picks.as_ref())
    }

    /// link.ts:345.
    pub fn github_from_vault(&self, place_id: &str) -> bool {
        let held = self.ctx.kept.borrow().get(place_id).cloned();
        let Some(held) = held else {
            return false;
        };
        let Some(github) = held.picks.as_ref().and_then(|p| p.configs.github.as_ref()) else {
            return false;
        };
        let vault_signed = github.signin.unwrap_or(GitHubSignIn::Vault) == GitHubSignIn::Vault;
        vault_signed
            && held.applied.as_ref().is_some_and(|a| {
                a.rows.iter().any(|r| {
                    r.row.id == GITHUB_ROW && r.row.outcome == PlaceProvisionRowOutcome::Present
                })
            })
    }

    /// link.ts:351.
    pub async fn login_landed(self: &Rc<Self>, place_id: &str, agent: &str) -> PlaceResult<()> {
        self.records.login_listed(place_id, agent).await?;
        let id = if agent == GITHUB_CLI {
            GITHUB_ROW.to_string()
        } else {
            sign_in_row_id(agent)
        };
        let row = PlaceProvisionRow {
            id,
            label: String::new(),
            outcome: PlaceProvisionRowOutcome::Installed,
            kind: None,
            step: None,
            note: Some(wsp_words::views::place_setup_words::SIGNED_IN_THERE.to_string()),
            fix: None,
            keys: None,
            ms: None,
            earlier: None,
            project: None,
        };
        self.land_sign_in(place_id, LandedRow { row, label: None })
            .await
    }

    /// link.ts:356.
    pub async fn logins_again(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        let logins = self
            .ctx
            .kept
            .borrow()
            .get(place_id)
            .and_then(|r| r.backend_facts.as_ref())
            .and_then(|f| f.logins.clone());
        let Some(logins) = logins else {
            return Ok(());
        };
        if !self.ctx.live.borrow().contains_key(place_id) {
            return Ok(());
        }
        let door = self.ctx.door();
        let exec = PlaceExecOpts {
            timeout_ms: Some(LOGINS_READ_MS),
            stdin: None,
        };
        let Ok(res) = door
            .exec(place_id, &shared_logins_script(&logins), exec)
            .await
        else {
            return Ok(());
        };
        if res.exit_code.as_f64() != 0.0 {
            return Ok(());
        }
        let found: Vec<String> = res
            .stdout
            .split('\n')
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect();
        self.records
            .change(
                place_id,
                Box::new(move |now: &PlaceRecord| {
                    let listed = now.report.logins.as_ref()?;
                    let next = relisted_logins(listed, &found)?;
                    Some(PlaceRecord {
                        report: PlaceReport {
                            logins: Some(next),
                            ..now.report.clone()
                        },
                        ..now.clone()
                    })
                }),
            )
            .await?;
        Ok(())
    }

    /// link.ts:368.
    pub async fn key_landed(self: &Rc<Self>, agent: &str) -> PlaceResult<()> {
        let vault = self
            .ctx
            .opts
            .vault
            .as_ref()
            .map(|vault| vault())
            .unwrap_or_default();
        if vault_sign_in(agent, &vault) != AgentSignInState::VaultKey {
            return Ok(());
        }
        let here = self.setup.here();
        for record in self.records.records().await? {
            let way = record.picks.as_ref().and_then(|p| {
                p.agents
                    .iter()
                    .find(|(name, _)| name.0 == agent)
                    .and_then(|(_, row)| row.signin)
            });
            let row = PlaceProvisionRow {
                id: sign_in_row_id(agent),
                label: String::new(),
                outcome: PlaceProvisionRowOutcome::Present,
                kind: None,
                step: None,
                note: Some(vault_held_line(agent, &vault, way, &here)),
                fix: None,
                keys: None,
                ms: None,
                earlier: None,
                project: None,
            };
            self.land_sign_in(&record.id, LandedRow { row, label: None })
                .await?;
        }
        Ok(())
    }

    /// link.ts:373.
    pub async fn gh_there(
        self: &Rc<Self>,
        place_id: &str,
        stage: ProvisionStage,
    ) -> PlaceResult<Vec<PlaceProvisionRow>> {
        let Some(provisioner) = self.ctx.wiring.provision.clone() else {
            return Ok(Vec::new());
        };
        let Some(record) = self.records.record_of(place_id).await? else {
            return Ok(Vec::new());
        };
        let Some(home) = record.report.login.get("HOME").cloned() else {
            return Ok(Vec::new());
        };
        let Some(mut picks) = record.picks.clone().or_else(|| picks_named(&record.name)) else {
            return Ok(Vec::new());
        };
        picks.clis = IndexMap::new();
        picks.configs = RecipeFileConfigs {
            git: None,
            shell: None,
            github: Some(RecipeFileConfigsGithub {
                signin: Some(GitHubSignIn::Machine),
            }),
        };
        let only: IndexSet<PlaceSetupStep> = IndexSet::from([PlaceSetupStep::Github]);
        let plan = provisioner
            .setup(
                &picks,
                SetupOn {
                    home: home.clone(),
                    stores: None,
                    recipe: None,
                },
                Some(&only),
            )
            .await?;
        let machine: Rc<dyn Machine> = Rc::new(PlaceMachine::new(
            self.setup.link_to(place_id),
            &record.name,
            &home,
        ));
        let on = ProvisionOn {
            home: home.clone(),
            held: None,
            stores: None,
        };
        let run = SetupRun::default();
        let rows = provisioner
            .step(machine, &plan, &EngineStep::Github, &run, stage, &on)
            .await?;
        for row in rows
            .iter()
            .filter(|r| r.outcome == PlaceProvisionRowOutcome::Installed)
        {
            let label = row.label.clone();
            let landed = PlaceProvisionRow {
                step: Some(PlaceSetupStep::Github),
                ..row.clone()
            };
            self.land_sign_in(
                place_id,
                LandedRow {
                    row: landed,
                    label: Some(label),
                },
            )
            .await?;
        }
        Ok(rows)
    }

    /// link.ts:388.
    pub fn offer_of(&self, place_id: &str) -> Option<String> {
        let offer = self
            .ctx
            .kept
            .borrow()
            .get(place_id)
            .and_then(|r| r.backend_facts.as_ref())
            .map(|f| f.offer.clone());
        offer.or_else(|| {
            self.records
                .provider_ids()
                .iter()
                .any(|id| id == place_id)
                .then(|| place_id.to_string())
        })
    }

    /// link.ts:390.
    pub fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        let made = self.ctx.backends.borrow().get(place_id).cloned();
        if made.is_some() {
            return made;
        }
        let facts = self
            .ctx
            .kept
            .borrow()
            .get(place_id)
            .and_then(|r| r.backend_facts.clone());
        if let Some(facts) = facts {
            return Some(self.views.backend_from(place_id, facts));
        }
        if self.ctx.kept.borrow().contains_key(place_id) {
            return None;
        }
        self.records.provider_backend(place_id)
    }

    /// link.ts:400.
    pub fn joined(&self, place_id: &str) -> bool {
        self.ctx.kept.borrow().contains_key(place_id)
    }

    /// link.ts:402.
    pub fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        let record = self.ctx.kept.borrow().get(place_id).cloned()?;
        let home = record.report.login.get("HOME").cloned()?;
        let path = record.report.login.get("PATH").cloned();
        let key = format!("{home}\0{}", path.as_deref().unwrap_or(""));
        let held = self.folder_machines.borrow().get(place_id).cloned();
        let machine = match held {
            Some((held_key, machine)) if held_key == key => machine,
            _ => PlaceFolderMachine::new(
                self.setup.link_to(place_id),
                place_id,
                &home,
                path.as_deref().filter(|p| !p.is_empty()),
            ),
        };
        self.folder_machines
            .borrow_mut()
            .insert(place_id.to_string(), (key, Arc::clone(&machine)));
        Some(FolderComputer {
            machine,
            home,
            cpu: record.report.shape.cpu.0 as u32,
            mem_mb: record.report.shape.mem_mb.0 as u64,
            tools: record.report.wsp_door == Some(true),
        })
    }

    /// link.ts:415.
    pub async fn forking_backend(
        self: &Rc<Self>,
        place_id: &str,
    ) -> PlaceResult<Arc<dyn MachineBackend>> {
        let record = match self.records.record_of(place_id).await? {
            Some(record) => Some(record),
            None => self.ctx.kept.borrow().get(place_id).cloned(),
        };
        let Some(record) = record else {
            if let Some(at) = self.records.provider_backend(place_id) {
                return Ok(at);
            }
            self.ctx.backends.borrow_mut().shift_remove(place_id);
            return Err(place_forks_nowhere(&place_forks_nowhere_line(place_id)));
        };
        let busy = if current_folders_of().as_deref() == Some(place_id) {
            None
        } else {
            self.setup.setting_now(&record)
        };
        if let Some(busy) = busy {
            return Err(place_provisioning(&busy));
        }
        if let Some(made) = self.backend_of(place_id) {
            return Ok(made);
        }
        let inflight = self.ctx.asking.borrow().get(place_id).cloned();
        if let Some(inflight) = inflight {
            return inflight.await;
        }
        let read = {
            let door = Rc::clone(self);
            let place = place_id.to_string();
            async move {
                let outcome = read_facts(&door, &place, &record).await;
                door.ctx.asking.borrow_mut().shift_remove(&place);
                outcome
            }
            .boxed_local()
            .shared()
        };
        self.ctx
            .asking
            .borrow_mut()
            .insert(place_id.to_string(), read.clone());
        read.await
    }

    /// link.ts:452.
    pub async fn forward(
        self: &Rc<Self>,
        place_id: &str,
        place_port: u16,
        o: ForwardAsk,
    ) -> PlaceResult<u16> {
        let key = match o.pane {
            Some(_) => format!("{place_id}:open:{place_port}"),
            None => format!("{place_id}:{place_port}"),
        };
        if let Some((workspace_id, name)) = o.pane.clone() {
            if o.standing {
                return self.panes.standing(&key, u32::from(place_port));
            }
            let (door, place, key_for_conn) =
                (Rc::downgrade(self), place_id.to_string(), key.clone());
            let on_conn: PaneOnConn = Rc::new(move |conn, used| {
                if let Some(door) = door.upgrade() {
                    door.tunnel(
                        &key_for_conn,
                        &place,
                        place_port,
                        conn,
                        Some(used),
                        Rc::new(RefCell::new(IndexMap::new())),
                    );
                }
            });
            return self
                .panes
                .reach(
                    &key,
                    u32::from(place_port),
                    PaneAsker { workspace_id, name },
                    on_conn,
                )
                .await;
        }
        if let Some(already) = self.ctx.forwards.borrow().get(&key).map(|f| f.local_port) {
            return Ok(already);
        }
        let conns: Conns = Rc::new(RefCell::new(IndexMap::new()));
        let on_conn: Rc<dyn Fn(TcpStream)> = {
            let (door, place, key_for_conn, fallback) = (
                Rc::downgrade(self),
                place_id.to_string(),
                key.clone(),
                Rc::clone(&conns),
            );
            Rc::new(move |conn| {
                if let Some(door) = door.upgrade() {
                    door.tunnel(
                        &key_for_conn,
                        &place,
                        place_port,
                        conn,
                        None,
                        Rc::clone(&fallback),
                    );
                }
            })
        };
        let (server, local_port) = listen_loopback(on_conn).await?;
        self.ctx.forwards.borrow_mut().insert(
            key,
            Forward {
                server,
                local_port,
                conns,
                stop: None,
            },
        );
        Ok(local_port)
    }

    /// One connection to a forward's port, carried as a tunnel to that computer's port (link.ts:456).
    fn tunnel(
        &self,
        key: &str,
        place_id: &str,
        place_port: u16,
        conn: TcpStream,
        used: Option<Rc<dyn Fn()>>,
        fallback: Conns,
    ) {
        let seq = self.tunnel_seq.get() + 1;
        self.tunnel_seq.set(seq);
        let id = format!("p{seq}");
        let reach = self
            .ctx
            .live
            .borrow()
            .get(place_id)
            .map(|held| Rc::clone(&held.reach));
        let Some(reach) = reach else {
            drop(conn);
            return;
        };
        if let Some(used) = &used {
            used();
        }
        let into = self
            .ctx
            .forwards
            .borrow()
            .get(key)
            .map(|f| Rc::clone(&f.conns))
            .unwrap_or(fallback);
        let (inbound, rx) = mpsc::unbounded();
        into.borrow_mut()
            .insert(id.clone(), Rc::new(TunnelConn { inbound }));
        spawn_local(run_tunnel(conn, rx, reach, into, id, place_port, used));
    }

    /// link.ts:502.
    pub fn pane_forwards(&self) -> Rc<PanePorts> {
        Rc::clone(&self.panes)
    }

    /// link.ts:504.
    pub async fn place_for(self: &Rc<Self>, word: &str) -> PlaceResult<PlaceFor> {
        let all = self.records.records().await?;
        let found: Vec<&PlaceRecord> = all
            .iter()
            .filter(|r| {
                names_place(
                    &PlaceRef {
                        id: r.id.clone(),
                        name: r.name.clone(),
                        kind: None,
                    },
                    word,
                )
            })
            .collect();
        if found.len() > 1 {
            let ids: Vec<String> = found.iter().map(|r| r.id.clone()).collect();
            return Err(HostError::new(
                wsp_wire::wire::place_link::two_places_refusal(word, &ids),
            ));
        }
        if let Some(record) = found.first() {
            return Ok(PlaceFor {
                place_id: Some(record.id.clone()),
            });
        }
        let here = (self.ctx.wiring.here)().name;
        let providers = self.records.provider_ids();
        let cloud = cloud_named(providers.as_slice(), word);
        if word == HERE_PLACE_ID
            || word == here
            || (cloud.is_some() && cloud == self.records.wired_provider())
        {
            return Ok(PlaceFor { place_id: None });
        }
        if let Some(cloud) = cloud {
            return Ok(PlaceFor {
                place_id: Some(cloud),
            });
        }
        let mut held = vec![here];
        held.extend(all.iter().map(|r| r.name.clone()));
        held.extend(providers.iter().map(|id| provider_key_name(id)));
        Err(HostError::new(no_such_place_refusal(word, &held)))
    }

    /// link.ts:522.
    pub async fn default_place(self: &Rc<Self>) -> PlaceResult<PlaceFor> {
        let Some(marked) = self.records.default_id().await? else {
            return Ok(PlaceFor { place_id: None });
        };
        if self.records.records().await?.iter().any(|r| r.id == marked) {
            return Ok(PlaceFor {
                place_id: Some(marked),
            });
        }
        let is_wired = self.records.wired_provider().as_deref() == Some(marked.as_str());
        if !is_wired && self.records.provider_ids().contains(&marked) {
            return Ok(PlaceFor {
                place_id: Some(marked),
            });
        }
        Ok(PlaceFor { place_id: None })
    }

    /// link.ts:531.
    pub async fn mark_used(self: &Rc<Self>, place_id: Option<&str>) -> PlaceResult<()> {
        let place = match place_id {
            Some(place) => place.to_string(),
            None => self
                .records
                .wired_provider()
                .unwrap_or_else(|| HERE_PLACE_ID.to_string()),
        };
        self.records.mark_default(&place).await
    }

    /// link.ts:535.
    pub async fn mark_default_if_none(self: &Rc<Self>, place_id: &str) -> PlaceResult<()> {
        let records = Rc::clone(&self.records);
        let store = Rc::clone(&self.ctx.store);
        let place = place_id.to_string();
        self.records
            .in_turn(Box::new(move || {
                async move {
                    if records.mark_held().await?.is_none() {
                        store
                            .put(DEFAULT_COLLECTION, DEFAULT_ID, json!({ "placeId": place }))
                            .await?;
                    }
                    Ok::<(), HostError>(())
                }
                .boxed_local()
            }))
            .await
    }
}
