// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/places/records.ts` at 65f6b89c7: the place records as the store keeps them, the
//! providers beside them, and the turns every write of one takes. Spec: `spec/places_records.md` (rules R1-R34).
//!
//! One thread: state is `RefCell`, no borrow is held across an `.await` or a call out (spec 03 rule 3.3), and every
//! map is an `IndexMap` with `shift_remove` (rule 3.11).

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;

use base64::Engine as _;
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use serde_json::{json, Value};
use wsp_core::ids::{Ids, OsIds};
use wsp_engine::child_exec::CHILD_TIMED_OUT;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::MachineBackend;
use wsp_store::keys::{
    fresh_ephemeral, make_seal, seal_keys, shared_secret, sign_place_bytes, PlaceKeyPair, Side,
};
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_state::{AGENTS_ON, DELETE_AFTER_MS, NAP_AFTER_MS};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place::{
    PlaceAddJob, PlaceAddJobState, PlaceBack, PlaceCapSet, PlaceSettingWord, PlaceSettingsSpawn,
    PlaceViewSize, PlaceViewSizeDefault,
};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::{CloudLimits, PlaceSettings, PlaceView};
use wsp_wire::views::workspace::{agents_from, WorkspaceAgentsPatch};
use wsp_wire::wire::capabilities::MachineSizeOffer;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_wire::wire::place_link::{
    held_place_file, place_file_names, place_link_transcript, place_refusal_transcript,
    LinkEphemerals, LinkRole, PlaceAuthRefusal, PlacePublicKey, PlaceSignature,
    PLACE_LINK_NONCE_BYTES,
};
use wsp_words::exit::{refusal_parts, HostError};
use wsp_words::place_state::{place_cap_of, place_takes, place_turn_limit, setting_for};
use wsp_words::project_path::held_place_script;
use wsp_words::words::base::last_line;
use wsp_words::workspace_state::{ssh_road_of, PlaceRoad as WordsRoad};

use async_trait::async_trait;

use super::context::PlaceDoorContext;
use super::helpers::{bounded, shared_login_file, ADDS_KEPT, UPDATE_POLL_MS};
use super::link::LinkRecords;
use super::setup::{RecordMove, SetupRecords};
use super::types::{
    is_place_login_refused, place_record_of, BackComputer, BackOn, PlaceChallenge, PlaceLogin,
    PlaceRecord, PlaceResult, RowId, RunOver, SudoVerb, CAPS, DEFAULT_COLLECTION, DEFAULT_ID,
    PLACES,
};

/// A boxed local future: what a chain runs.
pub type LocalFut<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// One install waiting on its computer to dial in (records.ts:163), keyed by the code it handed that computer. `join`
/// (views V17) writes `place_id`; `add` (manage A9) writes `login`, `back` and `woken`.
#[derive(Default)]
pub struct AwaitingInstall {
    pub place_id: Option<String>,
    pub login: Option<PlaceLogin>,
    pub back: Option<PlaceBack>,
    pub woken: Option<Woken>,
}

/// The `woken` call of one install's wait on its computer's link (records.ts:163).
pub type Woken = Rc<dyn Fn(&str)>;

/// What `reached_over` read (records.ts:223, R22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reached {
    Same,
    Other { other: Option<String> },
    Unread { unread: String, refused: bool },
}

/// A FIFO chain of the first form (spec 03 table, "chain"): each run starts after the one before it settles, its
/// caller gets its own result, and the tail swallows a failure so the next run still goes (records.ts:282-287).
pub struct Chain {
    turn: Arc<tokio::sync::Mutex<()>>,
    /// Runs queued or in flight on this chain, which a keyed chain counts to drop its entry.
    runs: Cell<usize>,
}

impl Chain {
    pub fn new() -> Self {
        Chain {
            turn: Arc::new(tokio::sync::Mutex::new(())),
            runs: Cell::new(0),
        }
    }

    /// Runs `write` in its turn and answers what it answered.
    pub async fn run<T: 'static>(&self, write: LocalFut<'_, T>) -> T {
        let _turn = Arc::clone(&self.turn).lock_owned().await;
        write.await
    }
}

impl Default for Chain {
    fn default() -> Self {
        Self::new()
    }
}

/// One `Chain` per key, each dropped once its last run settles and no later run joined (records.ts:336-345).
pub struct KeyedChain {
    turns: RefCell<IndexMap<String, Rc<Chain>>>,
}

/// A run queued or in flight on a key: dropping it counts the key down, and takes the key's entry out at zero.
struct Queued<'a> {
    keyed: &'a KeyedChain,
    key: String,
    chain: Rc<Chain>,
}

impl Drop for Queued<'_> {
    fn drop(&mut self) {
        let left = self.chain.runs.get().saturating_sub(1);
        self.chain.runs.set(left);
        if left == 0 {
            let mut turns = self.keyed.turns.borrow_mut();
            if turns
                .get(&self.key)
                .is_some_and(|held| Rc::ptr_eq(held, &self.chain))
            {
                turns.shift_remove(&self.key);
            }
        }
    }
}

/// The turn a key's run holds: the lock comes off before the count, so a run that queued behind it sees the key held.
pub struct KeyTurn<'a> {
    _turn: tokio::sync::OwnedMutexGuard<()>,
    _queued: Queued<'a>,
}

impl KeyedChain {
    pub fn new() -> Self {
        KeyedChain {
            turns: RefCell::new(IndexMap::new()),
        }
    }

    /// Queues behind the runs already on `key` and holds the key until the answer is dropped.
    pub async fn enter(&self, key: &str) -> KeyTurn<'_> {
        let chain = {
            let mut turns = self.turns.borrow_mut();
            let chain = Rc::clone(
                turns
                    .entry(key.to_string())
                    .or_insert_with(|| Rc::new(Chain::new())),
            );
            chain.runs.set(chain.runs.get() + 1);
            chain
        };
        let queued = Queued {
            keyed: self,
            key: key.to_string(),
            chain: Rc::clone(&chain),
        };
        let turn = Arc::clone(&chain.turn).lock_owned().await;
        KeyTurn {
            _turn: turn,
            _queued: queued,
        }
    }

    pub async fn run<T: 'static>(&self, key: &str, write: LocalFut<'_, T>) -> T {
        let _turn = self.enter(key).await;
        write.await
    }

    /// The keys with a run in flight or queued, for the tests that pin R25's cleanup.
    pub fn keys(&self) -> Vec<String> {
        self.turns.borrow().keys().cloned().collect()
    }
}

impl Default for KeyedChain {
    fn default() -> Self {
        Self::new()
    }
}

/// The records area (records.ts:39-373).
pub struct PlaceRecordsArea {
    pub ctx: Rc<PlaceDoorContext>,
    /// This area, for the work it starts behind a call (R12's read, R23's `moved`); set by `new` (`Rc::new_cyclic`).
    pub me: std::rc::Weak<PlaceRecordsArea>,
    /// R3: what `settingsAt` answers; `load`, `set` and `forget` write it.
    pub settings_held: Rc<RefCell<IndexMap<String, PlaceSettings>>>,
    /// R12: the last limits read per cloud, and the clouds with a read out.
    pub limits_kept: RefCell<IndexMap<String, CloudLimits>>,
    pub limits_asking: RefCell<IndexSet<String>>,
    /// R15.
    pub awaiting: RefCell<IndexMap<String, Rc<RefCell<AwaitingInstall>>>>,
    /// R16: the adds over ssh, oldest first.
    pub adds: RefCell<IndexMap<String, PlaceAddJob>>,
    /// R30: the mark's chain, which `set` and `forget` also take.
    pub marking: Chain,
    /// R25: the record turns.
    pub record_turns: KeyedChain,
}

/// A store document as the store takes it.
pub(super) fn store_doc<T: Serialize>(v: &T) -> serde_json::Result<Value> {
    serde_json::to_value(v)
}

/// An engine failure as the place door reports it: its own sentence, no kind.
pub(super) fn engine_error(e: EngineError) -> HostError {
    HostError::new(e.to_string())
}

pub(super) fn js_int(n: f64) -> JsInt {
    JsInt(JsNumber(n))
}

/// The settings a place has when nobody set one.
pub(super) fn no_settings() -> PlaceSettings {
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

/// Whether a place's settings hold any key at all (records.ts:111).
pub(super) fn has_keys(settings: &PlaceSettings) -> bool {
    settings.threads.is_some()
        || settings.machines.is_some()
        || settings.spend_per_day_usd.is_some()
        || settings.nap_ms.is_some()
        || settings.turn_limit_ms.is_some()
        || settings.spawn.is_some()
        || settings.size.is_some()
        || settings.delete_after_ms.is_some()
}

/// The cap set `placeCapOf` reads off a place's settings.
pub(super) fn cap_set(settings: &PlaceSettings) -> PlaceCapSet {
    PlaceCapSet {
        threads: settings.threads,
        machines: settings.machines,
        spend_per_day_usd: settings.spend_per_day_usd,
    }
}

/// A row with only what every row carries: the rest is the caller's to add.
pub(super) fn bare_view(id: &str, kind: PlaceKind, name: &str, default: bool) -> PlaceView {
    PlaceView {
        id: id.to_string(),
        kind,
        name: name.to_string(),
        label: None,
        mac: None,
        default,
        os: None,
        shape: None,
        disk_free_bytes: None,
        copies: None,
        present: None,
        takes_runtime: None,
        joined_at: None,
        last_seen_at: None,
        daemon_version: None,
        behind: None,
        agents: None,
        agent_versions: None,
        sign_ins: None,
        logins: None,
        rate_usd_per_hour: None,
        sizes: None,
        forks: None,
        takes_forks: None,
        road: None,
        home: None,
        uptime_ms: None,
        reported_at: None,
        dialled: None,
        blocked: None,
        tools_blocked: None,
        build: None,
        build_stopped: None,
        builds_images: None,
        setup: None,
        applied: None,
        picks: None,
        recipe: None,
        sync: None,
        cap: None,
        cap_default: None,
        settings: None,
        nap_ms: None,
        nap_default: None,
        turn_limit_ms: None,
        turn_limit_default: None,
        spawn: None,
        spawn_default: None,
        running: None,
        plan: None,
        starts: None,
        size: None,
        size_default: None,
        delete_after_ms: None,
        credit_hours: None,
        spent_today_usd: None,
    }
}

/// A wire public key off its base64 text; a text of the wrong length reads as zeros, which no peer verifies.
pub(super) fn wire_public_key(text: &str) -> PlacePublicKey {
    PlacePublicKey::parse(text).unwrap_or_else(|| PlacePublicKey::from_bytes(&[0; 44]))
}

/// A wire signature off its base64 text; a text of the wrong length reads as zeros, which no peer verifies.
pub(super) fn wire_signature(text: &str) -> PlaceSignature {
    PlaceSignature::parse(text).unwrap_or_else(|| PlaceSignature::from_bytes(&[0; 64]))
}

/// `new Date(at).toISOString()` for a clock reading.
pub(super) fn iso(at: u64) -> String {
    wsp_words::js_date::iso_string(at as f64).unwrap_or_default()
}

/// The `spawn` patch a place's settings hold (records.ts:112).
fn spawn_patch(spawn: Option<&PlaceSettingsSpawn>) -> WorkspaceAgentsPatch {
    WorkspaceAgentsPatch {
        max_machines: spawn.and_then(|s| s.max_machines),
        max_depth: spawn.and_then(|s| s.max_depth),
    }
}

impl PlaceRecordsArea {
    pub fn new(ctx: Rc<PlaceDoorContext>) -> Rc<Self> {
        Rc::new_cyclic(|me| PlaceRecordsArea {
            ctx,
            me: me.clone(),
            settings_held: Rc::new(RefCell::new(IndexMap::new())),
            limits_kept: RefCell::new(IndexMap::new()),
            limits_asking: RefCell::new(IndexSet::new()),
            awaiting: RefCell::new(IndexMap::new()),
            adds: RefCell::new(IndexMap::new()),
            marking: Chain::new(),
            record_turns: KeyedChain::new(),
        })
    }

    /// R1 (:42).
    pub async fn records(&self) -> PlaceResult<Vec<PlaceRecord>> {
        let docs = self.ctx.store.list(PLACES).await?;
        let mut out: Vec<PlaceRecord> = docs.iter().filter_map(place_record_of).collect();
        out.sort_by(|a, b| a.joined_at.cmp(&b.joined_at));
        Ok(out)
    }

    /// R4 (:46).
    pub fn wired_provider(&self) -> Option<String> {
        self.ctx
            .wiring
            .provider
            .as_ref()
            .and_then(|provider| provider())
            .map(|wired| wired.id)
    }

    /// R5 (:50-56).
    pub fn provider_ids(&self) -> Vec<String> {
        let wired = self.wired_provider();
        let table = self.ctx.opts.providers.as_ref().map(|table| table());
        let listed = table.as_ref().map(|table| table.list()).unwrap_or_default();
        match wired {
            None => {
                let table_wired = table.as_ref().map(|table| table.wired());
                listed
                    .into_iter()
                    .filter(|id| Some(id) != table_wired.as_ref())
                    .collect()
            }
            Some(wired) if listed.contains(&wired) => listed,
            Some(wired) => vec![wired],
        }
    }

    /// R6 (:58).
    pub fn provider_backend(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.ctx
            .opts
            .providers
            .as_ref()
            .and_then(|table| table().backend(place_id))
    }

    /// R7 (:61-65).
    pub fn provider_rate(&self, place_id: &str) -> Option<f64> {
        if self.wired_provider().as_deref() == Some(place_id) {
            return self
                .ctx
                .wiring
                .provider
                .as_ref()
                .and_then(|provider| provider())
                .map(|wired| wired.rate_usd_per_hour);
        }
        self.provider_backend(place_id).map(|at| {
            let pricing = at.pricing();
            pricing.rate_usd_per_hour(pricing.default_size())
        })
    }

    /// R8 (:69).
    pub fn provider_sizes(&self, place_id: &str) -> Vec<MachineSizeOffer> {
        self.provider_backend(place_id)
            .map(|at| at.capabilities().sizes)
            .unwrap_or_default()
    }

    /// R9 (:72-78): sets `buildsImages`, `build` and `buildStopped` on `row` as the rule says, nothing else.
    pub fn image_facts(
        &self,
        row: &mut PlaceView,
        place_id: &str,
        backend: Option<&Arc<dyn MachineBackend>>,
    ) {
        if let Some(at) = backend {
            row.builds_images = Some(wsp_wire::wire::capabilities::builds_images(
                &at.capabilities(),
            ));
        }
        if let Some(build) = self
            .ctx
            .opts
            .copy_build
            .as_ref()
            .and_then(|copy| copy(place_id))
        {
            row.build = Some(build.line);
            if build.stopped {
                row.build_stopped = Some(true);
            }
        }
    }

    /// R2 (:79-82).
    pub async fn record_of(&self, place_id: &str) -> PlaceResult<Option<PlaceRecord>> {
        Ok(self
            .ctx
            .store
            .get(PLACES, place_id)
            .await?
            .as_ref()
            .and_then(place_record_of))
    }

    /// R3 (:83-86).
    pub async fn settings_of(&self, place_id: &str) -> PlaceResult<PlaceSettings> {
        let doc = self.ctx.store.get(CAPS, place_id).await?;
        Ok(doc
            .and_then(|doc| serde_json::from_value(doc).ok())
            .unwrap_or_else(no_settings))
    }

    /// R10 (:90-94).
    pub async fn row_ids(&self) -> PlaceResult<Vec<RowId>> {
        let mut ids = vec![RowId {
            id: HERE_PLACE_ID.to_string(),
            kind: PlaceKind::Computer,
        }];
        ids.extend(self.records().await?.into_iter().map(|r| RowId {
            id: r.id,
            kind: PlaceKind::Computer,
        }));
        ids.extend(self.provider_ids().into_iter().map(|id| RowId {
            id,
            kind: PlaceKind::Provider,
        }));
        Ok(ids)
    }

    /// R11 (:96-115).
    pub async fn with_cap(&self, row: PlaceView, ids: &[RowId]) -> PlaceResult<PlaceView> {
        let settings = self.settings_of(&row.id).await?;
        let mut out = row;
        if let Some(cap) = place_cap_of(&out, Some(&cap_set(&settings))) {
            out.cap = Some(cap);
        }
        if let Some(cap_default) = place_cap_of(&out, None) {
            out.cap_default = Some(cap_default);
        }
        if has_keys(&settings) {
            out.settings = Some(settings.clone());
        }
        let nap_default = self.ctx.opts.nap_ms.unwrap_or(NAP_AFTER_MS);
        if place_takes(&out, &PlaceSettingWord::Nap) {
            let fallback = Some(JsInt::from(nap_default as i64));
            out.nap_ms = Some(setting_for(None, settings.nap_ms.as_ref(), &fallback));
            out.nap_default = Some(JsInt::from(nap_default as i64));
        }
        out.turn_limit_ms =
            Some(place_turn_limit(&out.kind, &settings).map(|ms| JsInt::from(ms as i64)));
        out.turn_limit_default =
            Some(place_turn_limit(&out.kind, &no_settings()).map(|ms| JsInt::from(ms as i64)));
        out.spawn = Some(agents_from(None, &spawn_patch(settings.spawn.as_ref())));
        out.spawn_default = Some(AGENTS_ON);
        out.running = Some(JsInt::from(
            self.ctx.recording.running_on(&out.id, ids).await? as i64,
        ));
        if place_takes(&out, &PlaceSettingWord::Size) {
            let id = out.id.clone();
            self.size_facts(&mut out, &id, &settings);
        }
        if place_takes(&out, &PlaceSettingWord::DeleteAfter) {
            out.delete_after_ms = Some(
                settings
                    .delete_after_ms
                    .unwrap_or(JsInt::from(DELETE_AFTER_MS as i64)),
            );
        }
        Ok(out)
    }

    /// R12 (:122-138). Synchronous; the read it starts is a spawned local task.
    pub fn with_limits(&self, row: PlaceView) -> PlaceView {
        if matches!(row.kind, PlaceKind::Provider) {
            if let Some(backend) = self.provider_backend(&row.id) {
                if backend.offers().place_limits
                    && self.limits_asking.borrow_mut().insert(row.id.clone())
                {
                    if let Some(area) = self.me.upgrade() {
                        let id = row.id.clone();
                        crate::local_task::spawn(async move {
                            match backend.place_limits().await.ok().flatten() {
                                None => {
                                    area.limits_kept.borrow_mut().shift_remove(&id);
                                }
                                Some(limits) => {
                                    area.limits_kept.borrow_mut().insert(id.clone(), limits);
                                }
                            }
                            area.limits_asking.borrow_mut().shift_remove(&id);
                        });
                    }
                }
            }
        }
        let limits = self.limits_kept.borrow().get(&row.id).cloned();
        match limits {
            None => row,
            Some(limits) => PlaceView {
                plan: Some(limits.plan),
                starts: limits.starts,
                credit_hours: limits.credit_hours,
                ..row
            },
        }
    }

    /// R13 (:141-145): sets `size` and `sizeDefault` on `row`.
    pub fn size_facts(&self, row: &mut PlaceView, place_id: &str, settings: &PlaceSettings) {
        let fallback = self
            .provider_backend(place_id)
            .map(|at| at.pricing().default_size());
        let size = settings
            .size
            .as_ref()
            .map(|size| PlaceViewSize {
                cpu: size.cpu,
                mem_mb: size.mem_mb,
            })
            .or_else(|| {
                fallback.map(|size| PlaceViewSize {
                    cpu: js_int(size.cpu),
                    mem_mb: js_int(size.mem_mb),
                })
            });
        if let Some(size) = size {
            row.size = Some(size);
        }
        if let Some(default) = fallback {
            row.size_default = Some(PlaceViewSizeDefault {
                cpu: js_int(default.cpu),
                mem_mb: js_int(default.mem_mb),
            });
        }
    }

    /// R14 (:148-158).
    pub async fn until_daemon_version(
        &self,
        place_id: &str,
        from: u32,
        wait_ms: u64,
    ) -> PlaceResult<u32> {
        let clock = Rc::clone(&self.ctx.clock);
        let until = clock.now() + wait_ms as i64;
        loop {
            let version = match self.record_of(place_id).await? {
                Some(record) => record.report.daemon_version.get(),
                None => i64::from(from),
            };
            if version >= DAEMON_VERSION || clock.now() >= until {
                return Ok(u32::try_from(version).unwrap_or(0));
            }
            let (wake, woken) = futures::channel::oneshot::channel::<()>();
            let _cancel = clock.schedule(
                Box::new(move || {
                    let _ = wake.send(());
                }),
                UPDATE_POLL_MS as i64,
            );
            let _ = woken.await;
        }
    }

    /// R16 (:167-173).
    pub fn put_add(&self, add_id: &str, next: impl FnOnce(PlaceAddJob) -> PlaceAddJob) {
        let job = self.adds.borrow().get(add_id).cloned();
        let Some(job) = job else {
            return;
        };
        let job = next(job);
        self.adds.borrow_mut().insert(add_id.to_string(), job);
        let finished: Vec<String> = self
            .adds
            .borrow()
            .values()
            .filter(|job| !matches!(job.state, PlaceAddJobState::Running))
            .map(|job| job.add_id.clone())
            .collect();
        let gone = finished.len().saturating_sub(ADDS_KEPT);
        let mut adds = self.adds.borrow_mut();
        for id in finished.iter().take(gone) {
            adds.shift_remove(id);
        }
    }

    /// R17 (:177-189). `install` is the in-flight install's login and back, where one is.
    pub fn with_road(
        record: PlaceRecord,
        install: Option<(&Option<PlaceLogin>, &Option<PlaceBack>)>,
    ) -> PlaceRecord {
        let Some((Some(login), back)) = install else {
            return record;
        };
        let mut record = record;
        let mut road = record.road.take().unwrap_or_default();
        road.ssh = Some(login.ssh.clone());
        if let Some(key_path) = &login.key_path {
            road.key_path = Some(key_path.clone());
        }
        if let Some(host_key) = &login.host_key {
            road.host_key = Some(host_key.clone());
        }
        if let Some(back) = back {
            road.back = Some(back.clone());
        }
        record.road = Some(road);
        record
    }

    /// R19 (:194-197).
    pub fn login_of(record: &PlaceRecord) -> Option<PlaceLogin> {
        let road = record.road.as_ref().map(|road| WordsRoad {
            ssh: road.ssh.clone(),
            from: road.from.clone(),
            back: road.back.clone(),
        });
        let ssh = ssh_road_of(road.as_ref())?;
        Some(PlaceLogin {
            ssh,
            key_path: record.road.as_ref().and_then(|road| road.key_path.clone()),
            host_key: record.road.as_ref().and_then(|road| road.host_key.clone()),
        })
    }

    /// R20 (:201-202).
    pub async fn root_over(
        &self,
        login: &PlaceLogin,
        sudo_password: Option<&str>,
        verb: SudoVerb,
        name: &str,
    ) -> PlaceResult<Option<String>> {
        let Some(sudo) = self.ctx.wiring.sudo_over.as_ref() else {
            return Ok(None);
        };
        let said = sudo.sudo_over(login, sudo_password, verb, name).await?;
        Ok(if said == "taken" {
            sudo_password.map(str::to_string)
        } else {
            None
        })
    }

    /// R21 (:209-217).
    pub async fn login_answers(&self, login: &PlaceLogin) -> bool {
        let Some(dial) = self.ctx.wiring.dial.as_ref() else {
            return true;
        };
        bounded(
            &*self.ctx.clock,
            dial.dial(login),
            self.ctx.dial_wait_ms,
            &format!("ssh {}", login.ssh),
        )
        .await
        .is_ok()
    }

    /// R22 (:223-240).
    pub async fn reached_over(
        &self,
        record: &PlaceRecord,
        login: &PlaceLogin,
        sudo_password: Option<&str>,
    ) -> PlaceResult<Reached> {
        let Some(run_over) = self.ctx.wiring.run_over.clone() else {
            return Ok(Reached::Unread {
                unread: "this host holds no ssh road to read it over".into(),
                refused: false,
            });
        };
        let Some(home) = record.report.login.get("HOME") else {
            return Ok(Reached::Unread {
                unread: format!("{} never said where its home is", record.name),
                refused: false,
            });
        };
        let wait = self.ctx.dial_wait_ms;
        let script = held_place_script(home);
        let work = run_over.run_over(login, &script, wait, sudo_password);
        let read: RunOver =
            match bounded(&*self.ctx.clock, work, wait, &format!("ssh {}", login.ssh)).await {
                Ok(read) => read,
                Err(e) => {
                    return Ok(Reached::Unread {
                        unread: refusal_parts(&e).said,
                        refused: is_place_login_refused(&e),
                    })
                }
            };
        if read.exit_code == CHILD_TIMED_OUT {
            let seconds = (wait as f64 / 1000.0).round() as u64;
            return Ok(Reached::Unread {
                unread: format!("the read timed out after {seconds}s"),
                refused: false,
            });
        }
        if read.exit_code != 0 {
            let unread = last_line(&read.stderr)
                .unwrap_or_else(|| format!("the read exited {}", read.exit_code));
            return Ok(Reached::Unread {
                unread,
                refused: false,
            });
        }
        let file = held_place_file(&read.stdout);
        let host_public_key = self.ctx.wiring.host_key.public_key.clone();
        if place_file_names(
            file.as_ref(),
            std::slice::from_ref(&record.id),
            &host_public_key,
        ) {
            return Ok(Reached::Same);
        }
        let other = self.records().await?.into_iter().find(|r| {
            r.id != record.id
                && place_file_names(file.as_ref(), std::slice::from_ref(&r.id), &host_public_key)
        });
        Ok(Reached::Other {
            other: other.map(|r| r.name),
        })
    }

    /// R18 (:244-247): the login and back of the first install whose join became `place_id`.
    pub fn road_of_install(
        &self,
        place_id: &str,
    ) -> Option<(Option<PlaceLogin>, Option<PlaceBack>)> {
        self.awaiting.borrow().values().find_map(|waiting| {
            let waiting = waiting.borrow();
            (waiting.place_id.as_deref() == Some(place_id))
                .then(|| (waiting.login.clone(), waiting.back.clone()))
        })
    }

    /// R23 (:251-265).
    pub fn hold_back(&self, record: &PlaceRecord) {
        let Some(login) = Self::login_of(record) else {
            return;
        };
        let Some(back) = record.road.as_ref().and_then(|road| road.back.clone()) else {
            return;
        };
        let Some(home) = record.report.login.get("HOME").cloned() else {
            return;
        };
        let Some(holder) = self.ctx.wiring.back.clone() else {
            return;
        };
        let me = self.me.clone();
        let ssh = login.ssh.clone();
        let moved: Rc<dyn Fn(PlaceBack)> = Rc::new(move |to: PlaceBack| {
            let Some(area) = me.upgrade() else {
                return;
            };
            let ids: Vec<String> = area.ctx.kept.borrow().keys().cloned().collect();
            let ssh = ssh.clone();
            crate::local_task::spawn(async move {
                for id in ids {
                    let (ssh, to) = (ssh.clone(), to.clone());
                    let _ = area
                        .change(&id, move |now| {
                            let same = Self::login_of(&now).is_some_and(|held| held.ssh == ssh)
                                && now.road.is_some();
                            if !same {
                                return None;
                            }
                            let mut next = now;
                            if let Some(road) = next.road.as_mut() {
                                road.back = Some(to);
                            }
                            Some(next)
                        })
                        .await;
                }
            });
        });
        let on = BackOn {
            home,
            computer: Some(BackComputer {
                id: record.id.clone(),
                name: record.name.clone(),
            }),
        };
        let mut hold = async move {
            let _ = holder.hold(&login, back, on, Some(moved)).await;
        }
        .boxed_local();
        // The TS calls `hold` at once and runs its body up to the first await; the rest goes on as a task.
        let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
        if hold.as_mut().poll(&mut cx).is_pending() {
            crate::local_task::spawn(hold);
        }
    }

    /// R24 (:271-276).
    pub async fn keep(&self, record: PlaceRecord) -> PlaceResult<PlaceRecord> {
        let install = self.road_of_install(&record.id);
        let held = Self::with_road(record, install.as_ref().map(|(login, back)| (login, back)));
        self.ctx
            .kept
            .borrow_mut()
            .insert(held.id.clone(), held.clone());
        self.ctx
            .store
            .put(
                PLACES,
                &held.id,
                store_doc(&held).map_err(|e| HostError::new(e.to_string()))?,
            )
            .await?;
        Ok(held)
    }

    /// R29 (:277-280).
    pub async fn default_id(&self) -> PlaceResult<Option<String>> {
        let held = self.ctx.store.get(DEFAULT_COLLECTION, DEFAULT_ID).await?;
        Ok(held
            .as_ref()
            .and_then(|held| held.get("placeId"))
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    /// R30 (:282-287).
    pub async fn in_turn<T: 'static>(&self, write: LocalFut<'_, T>) -> T {
        self.marking.run(write).await
    }

    /// R31 (:288).
    pub async fn mark_default(&self, place_id: &str) -> PlaceResult<()> {
        let store = Rc::clone(&self.ctx.store);
        let place_id = place_id.to_string();
        self.in_turn(Box::pin(async move {
            store
                .put(
                    DEFAULT_COLLECTION,
                    DEFAULT_ID,
                    json!({ "placeId": place_id }),
                )
                .await
        }))
        .await
    }

    /// R32 (:290-294).
    pub async fn mark_held(&self) -> PlaceResult<Option<String>> {
        let Some(marked) = self.default_id().await? else {
            return Ok(None);
        };
        let held = marked == HERE_PLACE_ID
            || self.provider_ids().contains(&marked)
            || self.records().await?.iter().any(|r| r.id == marked);
        Ok(held.then_some(marked))
    }

    /// R33 (:303-321).
    pub fn challenge(
        host_key: &PlaceKeyPair,
        subject: &str,
        their_nonce: &str,
        their_ephemeral: Option<&str>,
    ) -> Option<PlaceChallenge> {
        let their_ephemeral = their_ephemeral?;
        let nonce_bytes = OsIds.bytes(PLACE_LINK_NONCE_BYTES as usize);
        let nonce = base64::engine::general_purpose::STANDARD.encode(nonce_bytes);
        let (mine, mine_public) = fresh_ephemeral();
        let secret = shared_secret(&mine, their_ephemeral).ok()?;
        let signature = sign_place_bytes(
            &host_key.private_key_pem,
            &place_link_transcript(
                LinkRole::Host,
                subject,
                their_nonce,
                &nonce,
                LinkEphemerals {
                    challenger: their_ephemeral,
                    answerer: &mine_public,
                },
            ),
        )
        .ok()?;
        let expect = place_link_transcript(
            LinkRole::Place,
            subject,
            &nonce,
            their_nonce,
            LinkEphemerals {
                challenger: &mine_public,
                answerer: their_ephemeral,
            },
        );
        Some(PlaceChallenge {
            nonce,
            host_public_key: host_key.public_key.clone(),
            signature,
            ephemeral: mine_public,
            expect,
            seal: make_seal(&seal_keys(&secret, subject), Side::Host),
        })
    }

    /// R34 (:325-328).
    pub fn signed_refusal(
        host_key: &PlaceKeyPair,
        place_id: &str,
        place_nonce: &str,
        sentence: &str,
    ) -> PlaceAuthRefusal {
        let signature = sign_place_bytes(
            &host_key.private_key_pem,
            &place_refusal_transcript(place_id, place_nonce, sentence),
        )
        .unwrap_or_default();
        PlaceAuthRefusal {
            host_public_key: wire_public_key(&host_key.public_key),
            signature: wire_signature(&signature),
        }
    }

    /// R27 (:330-332).
    pub async fn write_seen(&self, place_id: &str, at: u64) -> PlaceResult<()> {
        self.change(place_id, move |now| {
            Some(PlaceRecord {
                last_seen_at: iso(at),
                ..now
            })
        })
        .await?;
        Ok(())
    }

    /// R25 (:336-345).
    pub async fn in_record_turn<T: 'static>(&self, place_id: &str, write: LocalFut<'_, T>) -> T {
        let _turn = self.record_turns.enter(place_id).await;
        write.await
    }

    /// R26 (:349-354), runtime invariant 6: the one road a record changes by.
    pub async fn change(
        &self,
        place_id: &str,
        mv: impl FnOnce(PlaceRecord) -> Option<PlaceRecord>,
    ) -> PlaceResult<Option<PlaceRecord>> {
        let _turn = self.record_turns.enter(place_id).await;
        let Some(now) = self.record_of(place_id).await? else {
            return Ok(None);
        };
        match mv(now) {
            None => Ok(None),
            Some(next) => self.keep(next).await.map(Some),
        }
    }

    /// R28 (:359-365).
    pub async fn login_listed(
        &self,
        place_id: &str,
        agent: &str,
    ) -> PlaceResult<Option<PlaceRecord>> {
        let file = shared_login_file(agent);
        self.change(place_id, move |now| {
            let listed = now.report.logins.clone()?;
            let file = file?;
            if listed.contains(&file) {
                return None;
            }
            let mut logins = listed;
            logins.push(file);
            logins.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            Some(PlaceRecord {
                report: wsp_wire::wire::place_link::PlaceReport {
                    logins: Some(logins),
                    ..now.report.clone()
                },
                ..now
            })
        })
        .await
    }
}

/// places-a's setup reads off this area (setup.rs `SetupRecords`): forwards to the methods above.
#[async_trait(?Send)]
impl SetupRecords for PlaceRecordsArea {
    async fn record_of(&self, place_id: &str) -> PlaceResult<Option<PlaceRecord>> {
        PlaceRecordsArea::record_of(self, place_id).await
    }
    async fn change(&self, place_id: &str, mv: RecordMove) -> PlaceResult<Option<PlaceRecord>> {
        PlaceRecordsArea::change(self, place_id, move |now| mv(&now)).await
    }
    async fn login_listed(&self, place_id: &str, agent: &str) -> PlaceResult<Option<PlaceRecord>> {
        PlaceRecordsArea::login_listed(self, place_id, agent).await
    }
}

/// places-a's link reads off this area (link.rs `LinkRecords`): forwards to the methods above. `awaiting_for` answers
/// the `woken` of every install awaiting a join that became `place_id`, in order, none taken off.
#[async_trait(?Send)]
impl LinkRecords for PlaceRecordsArea {
    async fn records(&self) -> PlaceResult<Vec<PlaceRecord>> {
        PlaceRecordsArea::records(self).await
    }
    fn wired_provider(&self) -> Option<String> {
        PlaceRecordsArea::wired_provider(self)
    }
    fn provider_ids(&self) -> Vec<String> {
        PlaceRecordsArea::provider_ids(self)
    }
    fn provider_backend(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        PlaceRecordsArea::provider_backend(self, place_id)
    }
    async fn settings_of(&self, place_id: &str) -> PlaceResult<PlaceSettings> {
        PlaceRecordsArea::settings_of(self, place_id).await
    }
    fn settings_held(&self) -> Rc<RefCell<IndexMap<String, PlaceSettings>>> {
        Rc::clone(&self.settings_held)
    }
    fn awaiting_for(&self, place_id: &str) -> Vec<Rc<dyn Fn(&str)>> {
        self.awaiting
            .borrow()
            .values()
            .filter_map(|waiting| {
                let waiting = waiting.borrow();
                if waiting.place_id.as_deref() == Some(place_id) {
                    waiting.woken.clone()
                } else {
                    None
                }
            })
            .collect()
    }
    fn hold_back(&self, record: &PlaceRecord) {
        PlaceRecordsArea::hold_back(self, record)
    }
    async fn default_id(&self) -> PlaceResult<Option<String>> {
        PlaceRecordsArea::default_id(self).await
    }
    async fn in_turn(
        &self,
        write: Box<dyn FnOnce() -> futures::future::LocalBoxFuture<'static, PlaceResult<()>>>,
    ) -> PlaceResult<()> {
        PlaceRecordsArea::in_turn(self, write()).await
    }
    async fn mark_default(&self, place_id: &str) -> PlaceResult<()> {
        PlaceRecordsArea::mark_default(self, place_id).await
    }
    async fn mark_held(&self) -> PlaceResult<Option<String>> {
        PlaceRecordsArea::mark_held(self).await
    }
    fn challenge(
        &self,
        subject: &str,
        their_nonce: &str,
        their_ephemeral: Option<&str>,
    ) -> Option<PlaceChallenge> {
        PlaceRecordsArea::challenge(
            &self.ctx.wiring.host_key,
            subject,
            their_nonce,
            their_ephemeral,
        )
    }
    fn signed_refusal(
        &self,
        place_id: &str,
        place_nonce: &str,
        sentence: &str,
    ) -> PlaceAuthRefusal {
        PlaceRecordsArea::signed_refusal(&self.ctx.wiring.host_key, place_id, place_nonce, sentence)
    }
    async fn write_seen(&self, place_id: &str, at: u64) -> PlaceResult<()> {
        PlaceRecordsArea::write_seen(self, place_id, at).await
    }
    async fn with_cap(&self, row: PlaceView, ids: &[RowId]) -> PlaceResult<PlaceView> {
        PlaceRecordsArea::with_cap(self, row, ids).await
    }
    async fn row_ids(&self) -> PlaceResult<Vec<RowId>> {
        PlaceRecordsArea::row_ids(self).await
    }
}
