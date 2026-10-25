// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/orphans.test.ts, snapshot-storage.test.ts and snapshot-names.test.ts.
#[path = "rest_common/mod.rs"]
mod common;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::cancel::Cancel;
use wsp_wire::types::JsNumber;
use wsp_wire::views::golden_image::{GoldenBaseTool, GoldenManifest, GoldenVersion, GoldenVersionSmoke};
use wsp_wire::wire::capabilities::{Capabilities, WorkspaceSize};

use wsp_engine::errors::{EngineError, ErrorKind, Result, WspError};
use wsp_engine::golden_seam::BUILDER_IDLE_MS;
use wsp_engine::machine::{
    BackendPricing, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow, MachineSpec, MachineState,
    RunOptions, Size, SnapshotOptions, SnapshotRow, SnapshotStoragePricing, Waiting, WireSnapshotStoragePricing,
};
use wsp_engine::orphans::{
    describe_age, reap, ReapFailure, ReapOptions, ReapReason, ReapResult, ReapedMachine, SparedMachine, Whose, OWN_GRACE_MS,
};
use wsp_engine::snapshot_names::{golden_name, name_owner, project_snapshot_name};
use wsp_engine::snapshot_storage::{
    ownership_of, retention_plan, snapshot_storage, split_by_owner, GuardedVersion, OwnedRow, OwnerRead, Ownership,
};

fn iso(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).unwrap().to_string().replace('Z', ".000Z")
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 5e-7, "{a} is not close to {b}");
}

fn strs(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

// ---- orphans

const NOW: i64 = 1_788_264_000_000; // 2026-09-01T12:00:00Z
const ME: &str = "h_me";

fn old() -> String {
    iso(NOW - 11 * 60_000)
}
fn young() -> String {
    iso(NOW - 2 * 60_000)
}
fn past_backstop() -> String {
    iso(NOW - BUILDER_IDLE_MS as i64 - 60_000)
}
fn fresh() -> String {
    iso(NOW - 30_000)
}
fn at_grace() -> String {
    iso(NOW - 60_000)
}
fn rate() -> f64 {
    2.0 * 0.035 + 4.0 * 0.01
}

#[derive(Clone)]
struct Row {
    id: String,
    state: MachineState,
    labels: IndexMap<String, String>,
    size: Option<(f64, f64)>,
}

fn lab(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn row(id: &str, labels: IndexMap<String, String>) -> Row {
    Row { id: id.to_string(), state: MachineState::Running, labels, size: None }
}

fn builder(extra: &[(&str, &str)]) -> IndexMap<String, String> {
    let mut m = lab(&[("wsp", "1"), ("wsp-builder", "1")]);
    m.extend(lab(extra));
    m
}

fn workspace(extra: &[(&str, &str)]) -> IndexMap<String, String> {
    let mut m = lab(&[("wsp", "1")]);
    m.extend(lab(extra));
    m
}

fn listed(r: &Row) -> MachineListRow {
    MachineListRow {
        id: r.id.clone(),
        state: r.state,
        labels: r.labels.clone(),
        size: r.size.map(|(cpu, mem)| WorkspaceSize { cpu: JsNumber(cpu), mem_mb: JsNumber(mem) }),
    }
}

fn provider_missing() -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: "gone".into(),
        request_id: None,
        window: None,
        said: None,
    })
}

struct StubPricing;

impl BackendPricing for StubPricing {
    fn rate_usd_per_hour(&self, s: Size) -> f64 {
        s.cpu * 0.035 + (s.mem_mb / 1024.0) * 0.01
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        storage_pricing()
    }
}

struct StubMachine {
    id: String,
    killed: Arc<Mutex<Vec<String>>>,
    kill_fails: Option<EngineError>,
}

#[async_trait(?Send)]
impl Machine for StubMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        Ok(common::ok(""))
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        Ok(common::ok(""))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Ok("s".into())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        if let Some(e) = &self.kill_fails {
            return Err(e.clone());
        }
        self.killed.lock().unwrap().push(self.id.clone());
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Ok("u".into())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Ok("u".into())
    }
}

type Rows = Box<dyn Fn() -> Result<Vec<Row>>>;

/// A provider whose listing is `rows`, whose get() counts and hands a machine that records its kill, and whose
/// failures are set per id.
struct StubBackend {
    rows: Rows,
    killed: Arc<Mutex<Vec<String>>>,
    listed: Mutex<u32>,
    fetched: Mutex<u32>,
    get_fails: HashMap<String, EngineError>,
    kill_fails: HashMap<String, EngineError>,
}

impl StubBackend {
    fn new(rows: Vec<Row>) -> Arc<Self> {
        StubBackend::with(Box::new(move || Ok(rows.clone())), HashMap::new(), HashMap::new())
    }
    fn with(rows: Rows, get_fails: HashMap<String, EngineError>, kill_fails: HashMap<String, EngineError>) -> Arc<Self> {
        Arc::new(StubBackend {
            rows,
            killed: Arc::new(Mutex::new(Vec::new())),
            listed: Mutex::new(0),
            fetched: Mutex::new(0),
            get_fails,
            kill_fails,
        })
    }
    fn killed(&self) -> Vec<String> {
        self.killed.lock().unwrap().clone()
    }
    fn listed(&self) -> u32 {
        *self.listed.lock().unwrap()
    }
    fn fetched(&self) -> u32 {
        *self.fetched.lock().unwrap()
    }
}

#[async_trait(?Send)]
impl MachineBackend for StubBackend {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: true,
            pause_mode: Some(wsp_wire::wire::capabilities::PauseMode::Memory),
            replaces_machine: true,
            preview_urls: true,
            signed_urls: true,
            callback_relay: true,
            disk_snapshots: true,
            images: true,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            sizes: vec![],
            kept: false,
            copies: true,
            own_network: true,
            sizes_last: true,
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(StubPricing)
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Err(common::other("unused"))
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        *self.fetched.lock().unwrap() += 1;
        if let Some(e) = self.get_fails.get(id) {
            return Err(e.clone());
        }
        Ok(Arc::new(StubMachine { id: id.to_string(), killed: self.killed.clone(), kill_fails: self.kill_fails.get(id).cloned() }))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        *self.listed.lock().unwrap() += 1;
        Ok((self.rows)()?.iter().map(listed).collect())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}

fn stop() -> wsp_engine::orphans::StopMachine {
    Arc::new(|m: Arc<dyn Machine>| Box::pin(async move { m.kill().await }))
}

fn opts(backend: &Arc<StubBackend>, owner: &str, known: Vec<String>) -> ReapOptions {
    ReapOptions {
        backend: backend.clone(),
        listing: None,
        known_ids: Box::new(move || known.clone()),
        owner: owner.to_string(),
        stop: stop(),
        older_than_ms: None,
        now: Some(Box::new(|| NOW)),
    }
}

// TS: packages/engine/test/orphans.test.ts:47
#[tokio::test]
async fn kills_only_unknown_wsp_labeled_running_machines_older_than_10_min() {
    let mut napping = row("napping", lab(&[("wsp", "1"), ("createdAt", &old())]));
    napping.state = MachineState::Paused;
    let backend = StubBackend::new(vec![
        row("claimed", lab(&[("wsp", "1"), ("createdAt", &old())])),
        row("orphan-old", lab(&[("wsp", "1"), ("createdAt", &old())])),
        row("orphan-young", lab(&[("wsp", "1"), ("createdAt", &young())])),
        row("foreign", lab(&[("poc", "ttl-test"), ("createdAt", &old())])),
        row("unlabeled", lab(&[])),
        napping,
        row("ageless", lab(&[("wsp", "1")])),
    ]);
    let result = reap(opts(&backend, ME, strs(&["claimed"]))).await.unwrap();
    assert_eq!(
        result.reaped,
        vec![ReapedMachine {
            id: "orphan-old".into(),
            labels: Some(lab(&[("wsp", "1"), ("createdAt", &old())])),
            builder: false,
            reason: ReapReason::Orphan,
            age_ms: Some(11 * 60_000),
            name: None,
        }]
    );
    assert_eq!(result.spared.iter().map(|s| s.id.clone()).collect::<Vec<_>>(), strs(&["orphan-young", "ageless"]));
    assert_eq!(backend.killed(), strs(&["orphan-old"]));
}

async fn verdict_case(row_in: Row, known: bool, verdict: &str) {
    let id = row_in.id.clone();
    let backend = StubBackend::new(vec![row_in]);
    let result = reap(opts(&backend, ME, if known { vec![id.clone()] } else { vec![] })).await.unwrap();
    let dies = verdict == "own" || verdict == "orphan";
    assert_eq!(backend.killed(), if dies { vec![id.clone()] } else { vec![] });
    let reason = if verdict == "own" { ReapReason::Own } else { ReapReason::Orphan };
    assert_eq!(
        result.reaped.iter().map(|r| (r.id.clone(), r.reason)).collect::<Vec<_>>(),
        if dies { vec![(id.clone(), reason)] } else { vec![] }
    );
    assert_eq!(result.spared.iter().map(|s| s.id.clone()).collect::<Vec<_>>(), if verdict == "spared" { vec![id] } else { vec![] });
}

macro_rules! verdict {
    ($name:ident, $row:expr, $known:expr, $verdict:expr) => {
        #[tokio::test]
        async fn $name() {
            verdict_case($row, $known, $verdict).await;
        }
    };
}

// builders: own dies at any age, none dies past the six-hour backstop, foreign never

// TS: packages/engine/test/orphans.test.ts:65
verdict!(a_builder_this_owner_recorded_kept, row("recorded", builder(&[("wsp-owner", ME), ("createdAt", &young())])), true, "kept");
// TS: packages/engine/test/orphans.test.ts:66
verdict!(a_builder_this_owner_is_still_creating_kept, row("inflight", builder(&[("wsp-owner", ME)])), true, "kept");
// TS: packages/engine/test/orphans.test.ts:67
verdict!(
    a_builder_this_owner_made_but_lost_the_record_of_own,
    row("owned", builder(&[("wsp-owner", ME), ("createdAt", &young())])),
    false,
    "own"
);
// TS: packages/engine/test/orphans.test.ts:68
verdict!(
    a_builder_this_owner_made_less_than_a_minute_ago_spared,
    row("owned-fresh", builder(&[("wsp-owner", ME), ("createdAt", &fresh())])),
    false,
    "spared"
);
// TS: packages/engine/test/orphans.test.ts:69
verdict!(
    a_builder_this_owner_made_exactly_a_minute_ago_own,
    row("owned-at-grace", builder(&[("wsp-owner", ME), ("createdAt", &at_grace())])),
    false,
    "own"
);
// TS: packages/engine/test/orphans.test.ts:70
verdict!(a_builder_this_owner_made_record_and_created_at_both_gone_own, row("owned-ageless", builder(&[("wsp-owner", ME)])), false, "own");
// TS: packages/engine/test/orphans.test.ts:71
verdict!(
    another_owners_young_builder_spared,
    row("foreign", builder(&[("wsp-owner", "h_other"), ("createdAt", &young())])),
    false,
    "spared"
);
// TS: packages/engine/test/orphans.test.ts:72
verdict!(
    another_owners_builder_past_the_backstop_spared,
    row("foreign-old", builder(&[("wsp-owner", "h_other"), ("createdAt", &past_backstop())])),
    false,
    "spared"
);
// TS: packages/engine/test/orphans.test.ts:73
verdict!(an_unowned_builder_past_the_backstop_orphan, row("orphan", builder(&[("createdAt", &past_backstop())])), false, "orphan");
// TS: packages/engine/test/orphans.test.ts:74
verdict!(an_unowned_builder_inside_the_backstop_spared, row("orphan-young", builder(&[("createdAt", &young())])), false, "spared");
// TS: packages/engine/test/orphans.test.ts:75
verdict!(an_unowned_builder_whose_age_cannot_be_read_spared, row("ageless", builder(&[])), false, "spared");
// TS: packages/engine/test/orphans.test.ts:76
verdict!(an_unowned_builder_whose_created_at_is_garbage_spared, row("garbage", builder(&[("createdAt", "yesterday")])), false, "spared");
// TS: packages/engine/test/orphans.test.ts:77
verdict!(
    a_paused_builder_of_unknown_owner_kept,
    {
        let mut r = row("paused", builder(&[("createdAt", &past_backstop())]));
        r.state = MachineState::Paused;
        r
    },
    false,
    "kept"
);
// TS: packages/engine/test/orphans.test.ts:78
verdict!(
    a_poc_machine_wearing_our_labels_owner_included_kept,
    row("experiment", builder(&[("poc", "p1"), ("wsp-owner", ME), ("createdAt", &past_backstop())])),
    false,
    "kept"
);

// non-builders: own and none die past ten minutes, foreign never

// TS: packages/engine/test/orphans.test.ts:80
verdict!(
    a_workspace_this_owner_made_past_ten_minutes_own,
    row("own-ws-old", workspace(&[("wsp-owner", ME), ("createdAt", &old())])),
    false,
    "own"
);
// TS: packages/engine/test/orphans.test.ts:81
verdict!(
    a_workspace_this_owner_made_two_minutes_ago_own,
    row("own-ws-young", workspace(&[("wsp-owner", ME), ("createdAt", &young())])),
    false,
    "own"
);
// TS: packages/engine/test/orphans.test.ts:82
verdict!(
    a_workspace_this_owner_made_exactly_a_minute_ago_own,
    row("own-ws-at-grace", workspace(&[("wsp-owner", ME), ("createdAt", &at_grace())])),
    false,
    "own"
);
// TS: packages/engine/test/orphans.test.ts:83
verdict!(
    a_workspace_this_owner_made_less_than_a_minute_ago_spared,
    row("own-ws-fresh", workspace(&[("wsp-owner", ME), ("createdAt", &fresh())])),
    false,
    "spared"
);
// TS: packages/engine/test/orphans.test.ts:84
verdict!(
    a_workspace_this_owner_made_with_a_garbage_created_at_own,
    row("own-ws-garbage", workspace(&[("wsp-owner", ME), ("createdAt", "yesterday")])),
    false,
    "own"
);
// TS: packages/engine/test/orphans.test.ts:85
verdict!(a_workspace_this_owner_made_with_no_created_at_own, row("own-ws-ageless", workspace(&[("wsp-owner", ME)])), false, "own");
// TS: packages/engine/test/orphans.test.ts:86
verdict!(
    another_owners_workspace_past_ten_minutes_spared,
    row("foreign-ws", workspace(&[("wsp-owner", "h_other"), ("createdAt", &past_backstop())])),
    false,
    "spared"
);
// TS: packages/engine/test/orphans.test.ts:87
verdict!(an_unowned_workspace_past_ten_minutes_orphan, row("orphan-ws", workspace(&[("createdAt", &old())])), false, "orphan");
// TS: packages/engine/test/orphans.test.ts:88
verdict!(an_unowned_workspace_inside_ten_minutes_spared, row("orphan-ws-young", workspace(&[("createdAt", &young())])), false, "spared");
// TS: packages/engine/test/orphans.test.ts:89
verdict!(
    an_unowned_workspace_with_a_garbage_created_at_spared,
    row("orphan-ws-garbage", workspace(&[("createdAt", "yesterday")])),
    false,
    "spared"
);
// TS: packages/engine/test/orphans.test.ts:90
verdict!(
    a_poc_workspace_wearing_our_owner_kept,
    row("experiment-ws", workspace(&[("poc", "p1"), ("wsp-owner", ME), ("createdAt", &old())])),
    false,
    "kept"
);

// TS: packages/engine/test/orphans.test.ts:100
#[tokio::test]
async fn describes_a_spared_machine_with_whose_it_is_its_age_its_backstop_and_the_rate() {
    let mut foreign = row("foreign", builder(&[("wsp-owner", "h_other"), ("createdAt", &young())]));
    foreign.size = Some((4.0, 8192.0));
    let backend = StubBackend::new(vec![
        foreign,
        row("orphan-young", builder(&[("createdAt", &young())])),
        row("ageless", builder(&[])),
        row("own-ws-fresh", workspace(&[("wsp-owner", ME), ("createdAt", &fresh())])),
        row("own-fresh", builder(&[("wsp-owner", ME), ("createdAt", &fresh())])),
    ]);
    let spared = reap(opts(&backend, ME, vec![])).await.unwrap().spared;
    let backstop = BUILDER_IDLE_MS as i64;
    assert_eq!(
        spared,
        vec![
            SparedMachine {
                id: "foreign".into(),
                labels: builder(&[("wsp-owner", "h_other"), ("createdAt", &young())]),
                builder: true,
                whose: Whose::Foreign,
                owner: Some("h_other".into()),
                age_ms: Some(2 * 60_000),
                backstop_ms: backstop,
                rate_usd_per_hour: 4.0 * 0.035 + 8.0 * 0.01,
            },
            SparedMachine {
                id: "orphan-young".into(),
                labels: builder(&[("createdAt", &young())]),
                builder: true,
                whose: Whose::None,
                owner: None,
                age_ms: Some(2 * 60_000),
                backstop_ms: backstop,
                rate_usd_per_hour: rate(),
            },
            SparedMachine {
                id: "ageless".into(),
                labels: builder(&[]),
                builder: true,
                whose: Whose::None,
                owner: None,
                age_ms: None,
                backstop_ms: backstop,
                rate_usd_per_hour: rate(),
            },
            SparedMachine {
                id: "own-ws-fresh".into(),
                labels: workspace(&[("wsp-owner", ME), ("createdAt", &fresh())]),
                builder: false,
                whose: Whose::Own,
                owner: Some(ME.into()),
                age_ms: Some(30_000),
                backstop_ms: 60_000,
                rate_usd_per_hour: rate(),
            },
            SparedMachine {
                id: "own-fresh".into(),
                labels: builder(&[("wsp-owner", ME), ("createdAt", &fresh())]),
                builder: true,
                whose: Whose::Own,
                owner: Some(ME.into()),
                age_ms: Some(30_000),
                backstop_ms: 60_000,
                rate_usd_per_hour: rate(),
            },
        ]
    );
}

// TS: packages/engine/test/orphans.test.ts:118
#[tokio::test]
async fn fetches_no_machine_it_leaves_alone_a_spared_row_is_judged_from_the_listing() {
    let mut paused = row("paused", builder(&[("createdAt", &past_backstop())]));
    paused.state = MachineState::Paused;
    let left = StubBackend::new(vec![
        row("foreign", builder(&[("wsp-owner", "h_other"), ("createdAt", &past_backstop())])),
        row("foreign-ws", workspace(&[("wsp-owner", "h_other"), ("createdAt", &old())])),
        row("orphan-young", builder(&[("createdAt", &young())])),
        row("orphan-ws-young", workspace(&[("createdAt", &young())])),
        row("own-fresh", builder(&[("wsp-owner", ME), ("createdAt", &fresh())])),
        row("own-ws-fresh", workspace(&[("wsp-owner", ME), ("createdAt", &fresh())])),
        row("ageless", builder(&[])),
        paused,
        row("experiment", builder(&[("poc", "p1"), ("wsp-owner", ME), ("createdAt", &past_backstop())])),
        row("not-ours", lab(&[])),
    ]);
    let result = reap(opts(&left, ME, vec![])).await.unwrap();
    assert_eq!(result.reaped, Vec::<ReapedMachine>::new());
    assert_eq!(
        result.spared.iter().map(|s| s.id.clone()).collect::<Vec<_>>(),
        strs(&["foreign", "foreign-ws", "orphan-young", "orphan-ws-young", "own-fresh", "own-ws-fresh", "ageless"])
    );
    assert_eq!(left.fetched(), 0);
    assert_eq!(left.killed(), Vec::<String>::new());

    let dies = StubBackend::new(vec![row("orphan", builder(&[("createdAt", &past_backstop())]))]);
    reap(opts(&dies, ME, vec![])).await.unwrap();
    assert_eq!(dies.fetched(), 1);
    assert_eq!(dies.killed(), strs(&["orphan"]));
}

// TS: packages/engine/test/orphans.test.ts:143
#[tokio::test]
async fn honors_a_custom_age_threshold_for_workspaces_not_for_builders() {
    let backend = StubBackend::new(vec![
        row("young-orphan", lab(&[("wsp", "1"), ("createdAt", &young())])),
        row("young-builder", builder(&[("createdAt", &young())])),
    ]);
    let mut o = opts(&backend, ME, vec![]);
    o.older_than_ms = Some(60_000);
    let result = reap(o).await.unwrap();
    assert_eq!(result.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), strs(&["young-orphan"]));
    assert_eq!(
        result.spared.iter().map(|s| (s.id.clone(), s.backstop_ms)).collect::<Vec<_>>(),
        vec![("young-builder".to_string(), BUILDER_IDLE_MS as i64)]
    );
    assert_eq!(backend.killed(), strs(&["young-orphan"]));
}

// TS: packages/engine/test/orphans.test.ts:154
#[tokio::test]
async fn kills_nothing_when_the_listing_fails() {
    let backend = StubBackend::with(Box::new(|| Err(common::other("list 502"))), HashMap::new(), HashMap::new());
    let err = reap(opts(&backend, ME, vec![])).await.unwrap_err();
    assert!(err.to_string().contains("list 502"), "{err}");
    assert_eq!(backend.killed(), Vec::<String>::new());
}

// TS: packages/engine/test/orphans.test.ts:160
#[tokio::test]
async fn reads_the_claimed_ids_after_the_listing_returns_so_a_create_that_lands_during_it_is_claimed() {
    let claimed: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let during = claimed.clone();
    let backend = StubBackend::with(
        Box::new(move || {
            *during.lock().unwrap() = strs(&["late"]);
            Ok(vec![row("late", builder(&[("wsp-owner", ME), ("createdAt", &young())]))])
        }),
        HashMap::new(),
        HashMap::new(),
    );
    let read = claimed.clone();
    let mut o = opts(&backend, ME, vec![]);
    o.known_ids = Box::new(move || read.lock().unwrap().clone());
    let result = reap(o).await.unwrap();
    assert_eq!(result, ReapResult { reaped: vec![], spared: vec![], failed: None });
    assert_eq!(backend.killed(), Vec::<String>::new());
}

// TS: packages/engine/test/orphans.test.ts:171
#[tokio::test]
async fn skips_a_listed_machine_that_is_gone_by_the_time_it_is_fetched_or_killed_and_still_processes_the_rest() {
    let rows = vec![
        row("gone-before-get", builder(&[("wsp-owner", ME), ("createdAt", &young())])),
        row("gone-before-kill", builder(&[("wsp-owner", ME), ("createdAt", &young())])),
        row("orphan", builder(&[("createdAt", &past_backstop())])),
    ];
    let backend = StubBackend::with(
        Box::new(move || Ok(rows.clone())),
        HashMap::from([("gone-before-get".to_string(), provider_missing())]),
        HashMap::from([("gone-before-kill".to_string(), provider_missing())]),
    );
    let result = reap(opts(&backend, ME, vec![])).await.unwrap();
    assert_eq!(result.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), strs(&["orphan"]));
    assert_eq!(result.spared, Vec::<SparedMachine>::new());
    assert_eq!(backend.killed(), strs(&["orphan"]));
}

// TS: packages/engine/test/orphans.test.ts:190
#[tokio::test]
async fn a_row_whose_kill_fails_with_anything_else_is_reported_by_id_and_the_rest_of_the_pass_still_runs() {
    let rows = vec![
        row("orphan-before", workspace(&[("createdAt", &old())])),
        row("bad", builder(&[("wsp-owner", ME), ("createdAt", &young())])),
        row("orphan-after", builder(&[("createdAt", &past_backstop())])),
        row("foreign", builder(&[("wsp-owner", "h_other"), ("createdAt", &young())])),
    ];
    let backend = StubBackend::with(
        Box::new(move || Ok(rows.clone())),
        HashMap::new(),
        HashMap::from([("bad".to_string(), common::other("Bad Gateway"))]),
    );
    let result = reap(opts(&backend, ME, vec![])).await.unwrap();
    assert_eq!(result.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), strs(&["orphan-before", "orphan-after"]));
    assert_eq!(result.spared.iter().map(|s| s.id.clone()).collect::<Vec<_>>(), strs(&["foreign"]));
    assert_eq!(result.failed, Some(vec![ReapFailure { id: Some("bad".into()), message: "could not stop: Bad Gateway".into() }]));
    assert_eq!(backend.killed(), strs(&["orphan-before", "orphan-after"]));
}

// TS: packages/engine/test/orphans.test.ts:210
#[test]
fn describes_an_age_for_a_person_never_negative_floored_hours_with_one_decimal() {
    assert_eq!(describe_age(None), "age unknown");
    assert_eq!(describe_age(Some(-45_000)), "0 s old");
    assert_eq!(describe_age(Some(59_600)), "59 s old");
    assert_eq!(describe_age(Some(60_000)), "1 min old");
    assert_eq!(describe_age(Some(570_000)), "9 min old");
    assert_eq!(describe_age(Some(6 * 3_600_000)), "6.0 h old");
    assert_eq!(describe_age(Some(6 * 3_600_000 + 60_000)), "6.0 h old");
}

// TS: packages/engine/test/orphans.test.ts:220
#[tokio::test]
async fn refuses_an_empty_owner_before_listing_anything() {
    let backend = StubBackend::new(vec![row("orphan-young", builder(&[("createdAt", &young())]))]);
    let err = reap(opts(&backend, "", vec![])).await.unwrap_err();
    assert!(err.to_string().contains("owner id"), "{err}");
    assert_eq!(backend.listed(), 0);
    assert_eq!(backend.killed(), Vec::<String>::new());
}

// ---- snapshot-storage

const GB: f64 = 1e9;
const STORAGE_NOW: i64 = 1_788_868_800_000; // 2026-09-08T12:00:00.000Z

fn storage_pricing() -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: WireSnapshotStoragePricing { free_gb: JsNumber(10.0), usd_per_gb_month: JsNumber(0.05), billed_from: "2026-10-01".into() },
        free_named: None,
        usd_per_named_month: None,
    }
}

/// Past the grace, so only the mark and the record decide.
fn old_stamp() -> String {
    iso(STORAGE_NOW - 3_600_000)
}

fn snap(id: &str, gb: f64) -> SnapshotRow {
    SnapshotRow { id: id.into(), name: None, size_bytes: JsNumber(gb * GB), restored_bytes: None, created_at: None, parent: None }
}

fn named(id: &str, gb: f64, name: &str) -> SnapshotRow {
    SnapshotRow { name: Some(name.into()), created_at: Some(old_stamp()), ..snap(id, gb) }
}

fn unrecorded() -> OwnerRead {
    OwnerRead { host_id: "h1".into(), recorded: HashSet::new(), now: STORAGE_NOW }
}

/// A listed snapshot or template as the split reads it.
#[derive(Clone, Debug, PartialEq)]
struct Plain {
    id: String,
    name: Option<String>,
    created_at: Option<String>,
}

fn plain(id: &str, name: Option<&str>, created_at: Option<String>) -> Plain {
    Plain { id: id.into(), name: name.map(String::from), created_at }
}

impl OwnedRow for Plain {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }
}

// TS: packages/engine/test/snapshot-storage.test.ts:25
#[test]
fn counts_every_snapshot_on_the_account_and_bills_the_gb_past_the_free_tier_at_the_published_rate() {
    let s =
        snapshot_storage(&[snap("a", 7.8), snap("b", 8.5), snap("c", 20.0), snap("d", 3.84)], &storage_pricing(), &unrecorded()).unwrap();
    assert_eq!(s.count, JsNumber(4.0));
    assert_eq!(s.total_bytes, JsNumber(40.14 * GB));
    close(s.monthly_usd.0, (40.14 - 10.0) * 0.05);
    assert_eq!(s.free_gb, JsNumber(10.0));
    assert_eq!(s.usd_per_gb_month, JsNumber(0.05));
    assert_eq!(s.billed_from, "2026-10-01");
}

// TS: packages/engine/test/snapshot-storage.test.ts:33
#[test]
fn inside_the_free_tier_the_monthly_cost_is_zero_never_negative() {
    assert_eq!(snapshot_storage(&[snap("a", 7.8)], &storage_pricing(), &unrecorded()).unwrap().monthly_usd, JsNumber(0.0));
    let empty = snapshot_storage(&[], &storage_pricing(), &unrecorded()).unwrap();
    assert_eq!(empty.count, JsNumber(0.0));
    assert_eq!(empty.total_bytes, JsNumber(0.0));
    assert_eq!(empty.monthly_usd, JsNumber(0.0));
}

// TS: packages/engine/test/snapshot-storage.test.ts:38
#[test]
fn splits_the_sum_by_who_made_each_snapshot_so_the_count_on_the_line_says_why_the_account_holds_them() {
    let rows = [
        named("s1", 12.0, "wsp-h1-default-v1"),
        named("s2", 20.0, "wsp-h1-default-v9"),
        named("s3", 21.0, "golden-v1"),
        named("s4", 7.0, "wsp-zz9-default-v1"),
    ];
    let read = OwnerRead { host_id: "h1".into(), recorded: HashSet::from(["s1".to_string()]), now: STORAGE_NOW };
    let s = snapshot_storage(&rows, &storage_pricing(), &read).unwrap();
    assert_eq!((s.kept.count, s.kept.bytes), (JsNumber(1.0), JsNumber(12.0 * GB)));
    assert_eq!((s.orphans.count, s.orphans.bytes), (JsNumber(1.0), JsNumber(20.0 * GB)));
    assert_eq!((s.others.count, s.others.bytes), (JsNumber(2.0), JsNumber(28.0 * GB)));
    assert_eq!(s.kept.bytes.0 + s.orphans.bytes.0 + s.others.bytes.0, s.total_bytes.0);
}

fn read_with_recorded() -> OwnerRead {
    OwnerRead { host_id: "h1".into(), recorded: HashSet::from(["s1".to_string(), "tpl_recorded".to_string()]), now: STORAGE_NOW }
}

// TS: packages/engine/test/snapshot-storage.test.ts:53
#[test]
fn a_row_anything_records_is_kept_whatever_its_name() {
    let read = read_with_recorded();
    assert_eq!(ownership_of(&plain("s1", Some("golden-v1"), None), &read), Ownership::Kept);
    assert_eq!(ownership_of(&plain("s1", None, None), &read), Ownership::Kept);
}

// TS: packages/engine/test/snapshot-storage.test.ts:58
#[test]
fn this_hosts_mark_with_nothing_recording_the_id_is_an_orphan() {
    let read = read_with_recorded();
    assert_eq!(ownership_of(&plain("s9", Some("wsp-h1-default-v9"), Some(old_stamp())), &read), Ownership::Orphan);
    assert_eq!(ownership_of(&plain("s9", Some("wsp-h1-project-spoo-2026"), Some(old_stamp())), &read), Ownership::Orphan);
}

// TS: packages/engine/test/snapshot-storage.test.ts:63
#[test]
fn no_mark_of_this_host_is_foreign_whether_another_hosts_a_persons_own_or_a_name_from_before_the_mark() {
    let read = read_with_recorded();
    assert_eq!(ownership_of(&plain("s9", Some("wsp-zz9-default-v1"), Some(old_stamp())), &read), Ownership::Foreign);
    assert_eq!(ownership_of(&plain("s9", Some("golden-v1"), Some(old_stamp())), &read), Ownership::Foreign);
    assert_eq!(ownership_of(&plain("s9", Some("base"), Some(old_stamp())), &read), Ownership::Foreign);
    assert_eq!(ownership_of(&plain("s9", None, Some(old_stamp())), &read), Ownership::Foreign);
}

// TS: packages/engine/test/snapshot-storage.test.ts:70
#[test]
fn the_one_split_serves_snapshots_and_templates_alike() {
    let read = read_with_recorded();
    let rows = [
        plain("tpl_recorded", Some("wsp-h1-default-v1"), None),
        plain("tpl_old", Some("wsp-h1-old-v1"), Some(old_stamp())),
        plain("base", Some("base"), None),
    ];
    let split = split_by_owner(&rows, &read).unwrap();
    assert_eq!(split.kept, vec![rows[0].clone()]);
    assert_eq!(split.orphans, vec![rows[1].clone()]);
    assert_eq!(split.foreign, vec![rows[2].clone()]);
}

// TS: packages/engine/test/snapshot-storage.test.ts:77
#[test]
fn a_marked_row_the_provider_listed_inside_the_grace_is_kept() {
    let read = read_with_recorded();
    let at = |ms_ago: i64| iso(STORAGE_NOW - ms_ago);
    let marked = |created_at: Option<String>| plain("s9", Some("wsp-h1-default-v9"), created_at);
    assert_eq!(ownership_of(&marked(Some(at(OWN_GRACE_MS - 1))), &read), Ownership::Kept);
    assert_eq!(ownership_of(&marked(Some(at(0))), &read), Ownership::Kept);
    assert_eq!(ownership_of(&marked(Some(at(OWN_GRACE_MS + 1))), &read), Ownership::Orphan);
    // Deleting a snapshot cannot be undone, so a row whose age the provider did not give is never offered.
    assert_eq!(ownership_of(&marked(None), &read), Ownership::Kept);
    assert_eq!(ownership_of(&marked(Some("not a date".into())), &read), Ownership::Kept);
    // A clock behind the provider's reads as age zero, never as a row old enough to delete.
    assert_eq!(ownership_of(&marked(Some(iso(STORAGE_NOW + 60_000))), &read), Ownership::Kept);
}

// TS: packages/engine/test/snapshot-storage.test.ts:91
#[test]
fn without_this_hosts_mark_there_is_no_split_at_all_rather_than_one_that_claims_every_unmarked_row() {
    let read = OwnerRead { host_id: String::new(), ..read_with_recorded() };
    let err = split_by_owner(&[plain("s1", Some("wsp-h1-default-v1"), None)], &read).unwrap_err();
    assert!(err.to_string().contains("mark"), "{err}");
}

// ---- retention plan

fn version(n: i64, parent: Option<i64>) -> GoldenVersion {
    GoldenVersion {
        version: JsNumber(n as f64),
        snapshot_id: format!("snap_v{n}"),
        template_id: None,
        base_template: "base".into(),
        kind: None,
        setup_sha: "s".into(),
        created_at: "2026-09-01T00:00:00Z".into(),
        smoke: GoldenVersionSmoke { cmd: "true".into(), exit_code: JsNumber(0.0) },
        size: None,
        browser_shim: None,
        npm_bin: None,
        logins: None,
        missing_tools: None,
        silenced: None,
        shell_noise: None,
        left_behind: None,
        parent_snapshot_id: parent.map(|p| format!("snap_v{p}")),
        base: None,
        retired: None,
        used_bytes: None,
        image_hash: None,
        no_secrets: None,
    }
}

/// Versions sealed before parents were recorded: a linear chain by number.
fn manifest(head: i64, versions: &[i64]) -> GoldenManifest {
    GoldenManifest { head: JsNumber(head as f64), versions: versions.iter().map(|n| version(*n, None)).collect() }
}

/// Versions with their recorded parents, (version, parent).
fn chained(head: i64, links: &[(i64, Option<i64>)]) -> GoldenManifest {
    GoldenManifest { head: JsNumber(head as f64), versions: links.iter().map(|(n, p)| version(*n, *p)).collect() }
}

fn nums(vs: &[GoldenVersion]) -> Vec<i64> {
    vs.iter().map(|v| v.version.0 as i64).collect()
}

fn nobody(_: &str) -> Vec<String> {
    vec![]
}

fn plan_rows() -> Vec<SnapshotRow> {
    vec![snap("snap_v1", 7.8), snap("snap_v2", 8.0), snap("snap_v3", 8.2), snap("snap_v4", 8.5)]
}

// TS: packages/engine/test/snapshot-storage.test.ts:99
#[test]
fn keeps_the_head_and_its_recorded_parent_and_offers_the_older_ancestors_oldest_first_with_what_they_hold_and_save() {
    let m = chained(4, &[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3))]);
    let plan = retention_plan(&m, &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![4, 3]);
    assert_eq!(nums(&plan.drop), vec![1, 2]);
    assert_eq!(plan.guarded, Vec::<GuardedVersion>::new());
    assert!(!plan.parent_assumed);
    assert_eq!(plan.freed_bytes, 15.8 * GB);
    // 32.5 GB billed at 22.5 over the free tier; 16.7 GB left bills 6.7 over it.
    close(plan.saves_usd_per_month, 15.8 * 0.05);
}

// TS: packages/engine/test/snapshot-storage.test.ts:110
#[test]
fn once_an_update_seals_a_new_head_from_the_rolled_back_version_the_branch_rolled_back_from_is_abandoned() {
    // v1 to v4, head rolled back to v2, then an update seals v5 from v2.
    let m = chained(5, &[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3)), (5, Some(2))]);
    let mut rows = plan_rows();
    rows.push(snap("snap_v5", 9.0));
    let plan = retention_plan(&m, &rows, &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![5, 2]);
    assert_eq!(nums(&plan.drop), vec![1, 3, 4]);
    assert_eq!(nums(&plan.abandoned), vec![3, 4]);
    assert_eq!(plan.guarded, Vec::<GuardedVersion>::new());
    assert!(!plan.parent_assumed);
    assert_eq!(plan.freed_bytes, (7.8 + 8.2 + 8.5) * GB);
    close(plan.saves_usd_per_month, (7.8 + 8.2 + 8.5) * 0.05);
}

// TS: packages/engine/test/snapshot-storage.test.ts:122
#[test]
fn an_abandoned_branch_a_workspace_was_forked_from_is_guarded_like_an_ancestor_and_stays_out_of_the_offer() {
    let forked_from = |id: &str| if id == "snap_v4" { strs(&["alpha"]) } else { vec![] };
    let m = chained(5, &[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3)), (5, Some(2))]);
    let mut rows = plan_rows();
    rows.push(snap("snap_v5", 9.0));
    let plan = retention_plan(&m, &rows, &forked_from, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![5, 2]);
    assert_eq!(nums(&plan.drop), vec![1, 3]);
    assert_eq!(nums(&plan.abandoned), vec![3]);
    assert_eq!(plan.guarded, vec![GuardedVersion { version: version(4, Some(3)), workspaces: strs(&["alpha"]) }]);
    assert_eq!(plan.freed_bytes, (7.8 + 8.2) * GB);
}

// TS: packages/engine/test/snapshot-storage.test.ts:132
#[test]
fn a_manifest_whose_head_is_not_among_its_versions_keeps_nothing_and_offers_nothing() {
    let m = chained(9, &[(1, None), (2, Some(1))]);
    let plan = retention_plan(&m, &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(plan.keep, Vec::<GoldenVersion>::new());
    assert_eq!(plan.drop, Vec::<GoldenVersion>::new());
    assert_eq!(plan.abandoned, Vec::<GoldenVersion>::new());
}

// TS: packages/engine/test/snapshot-storage.test.ts:139
#[test]
fn versions_sealed_before_parents_were_recorded_chain_by_number_and_the_offer_knows_the_kept_parent_was_assumed() {
    let plan = retention_plan(&manifest(4, &[1, 2, 3, 4]), &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![4, 3]);
    assert_eq!(nums(&plan.drop), vec![1, 2]);
    assert!(plan.parent_assumed);
    // A recorded parent on the head alone settles the kept pair; the older hops still chain by number.
    let mixed_manifest = chained(5, &[(1, None), (2, None), (3, None), (4, None), (5, Some(2))]);
    let mixed = retention_plan(&mixed_manifest, &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&mixed.keep), vec![5, 2]);
    assert_eq!(nums(&mixed.drop), vec![1, 3, 4]);
    assert_eq!(nums(&mixed.abandoned), vec![3, 4]);
    assert!(!mixed.parent_assumed);
}

// TS: packages/engine/test/snapshot-storage.test.ts:152
#[test]
fn a_head_built_from_a_fresh_machine_keeps_the_next_version_down_as_its_step_back_and_nothing_says_a_parent_was_assumed() {
    let fresh = |n: i64, parent: Option<i64>| GoldenVersion {
        base: Some(vec![GoldenBaseTool { name: "git".into(), version: "2.43.0".into() }]),
        ..version(n, parent)
    };
    let m = GoldenManifest { head: JsNumber(3.0), versions: vec![fresh(1, None), fresh(2, Some(1)), fresh(3, None)] };
    let plan = retention_plan(&m, &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![3, 2]);
    assert_eq!(nums(&plan.drop), vec![1]);
    assert_eq!(plan.abandoned, Vec::<GoldenVersion>::new());
    assert!(!plan.parent_assumed);
}

// TS: packages/engine/test/snapshot-storage.test.ts:161
#[test]
fn a_recorded_parent_no_longer_in_the_manifest_ends_the_chain_the_head_alone_is_kept_and_the_rest_is_abandoned() {
    let m = chained(3, &[(1, None), (3, Some(9))]);
    let plan = retention_plan(&m, &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![3]);
    assert_eq!(nums(&plan.drop), vec![1]);
    assert_eq!(nums(&plan.abandoned), vec![1]);
    assert!(!plan.parent_assumed);
}

// TS: packages/engine/test/snapshot-storage.test.ts:169
#[test]
fn a_drop_that_lands_inside_the_free_gb_saves_nothing_and_the_parent_is_the_next_version_down_whatever_its_number() {
    let rows = [snap("snap_v1", 3.0), snap("snap_v2", 3.0), snap("snap_v4", 3.0)];
    let plan = retention_plan(&manifest(4, &[1, 2, 4]), &rows, &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![4, 2]);
    assert_eq!(nums(&plan.drop), vec![1]);
    assert_eq!(plan.freed_bytes, 3.0 * GB);
    assert_eq!(plan.saves_usd_per_month, 0.0);
}

// TS: packages/engine/test/snapshot-storage.test.ts:177
#[test]
fn never_offers_a_version_a_workspace_was_forked_from_it_is_named_with_the_workspaces_on_it() {
    let forked_from = |id: &str| if id == "snap_v1" { strs(&["alpha", "beta"]) } else { vec![] };
    let plan = retention_plan(&manifest(4, &[1, 2, 3, 4]), &plan_rows(), &forked_from, &storage_pricing(), None);
    assert_eq!(nums(&plan.drop), vec![2]);
    assert_eq!(plan.guarded, vec![GuardedVersion { version: version(1, None), workspaces: strs(&["alpha", "beta"]) }]);
    assert_eq!(plan.freed_bytes, 8.0 * GB);
}

// TS: packages/engine/test/snapshot-storage.test.ts:185
#[test]
fn with_two_versions_or_fewer_there_is_nothing_to_offer() {
    let plan = retention_plan(&manifest(2, &[1, 2]), &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![2, 1]);
    assert_eq!(plan.drop, Vec::<GoldenVersion>::new());
    assert_eq!(plan.saves_usd_per_month, 0.0);
}

// TS: packages/engine/test/snapshot-storage.test.ts:192
#[test]
fn after_a_rollback_alone_the_versions_ahead_of_the_head_may_still_be_rolled_forward_to_neither_kept_nor_offered() {
    let m = chained(2, &[(1, None), (2, Some(1)), (3, Some(2)), (4, Some(3))]);
    let plan = retention_plan(&m, &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.keep), vec![2, 1]);
    assert_eq!(plan.drop, Vec::<GoldenVersion>::new());
    assert_eq!(plan.abandoned, Vec::<GoldenVersion>::new());
    // The same over a manifest sealed before parents were recorded.
    let before = retention_plan(&manifest(2, &[1, 2, 3, 4]), &plan_rows(), &nobody, &storage_pricing(), None);
    assert_eq!(before.drop, Vec::<GoldenVersion>::new());
}

// TS: packages/engine/test/snapshot-storage.test.ts:201
#[test]
fn a_snapshot_the_listing_does_not_carry_counts_as_holding_nothing() {
    let rows = [snap("snap_v2", 8.0), snap("snap_v3", 8.0)];
    let plan = retention_plan(&manifest(3, &[1, 2, 3]), &rows, &nobody, &storage_pricing(), None);
    assert_eq!(nums(&plan.drop), vec![1]);
    assert_eq!(plan.freed_bytes, 0.0);
    assert_eq!(plan.saves_usd_per_month, 0.0);
}

// ---- snapshot-names

// TS: packages/engine/test/snapshot-names.test.ts:6
#[test]
fn a_golden_versions_snapshot_and_its_template_read_the_same_name_and_the_host_reads_back_off_it() {
    assert_eq!(golden_name("9f3a1c2b", "default", 1).unwrap(), "wsp-9f3a1c2b-default-v1");
    assert_eq!(golden_name("9f3a1c2b", "default", 12).unwrap(), "wsp-9f3a1c2b-default-v12");
    assert_eq!(name_owner(Some(&golden_name("9f3a1c2b", "default", 12).unwrap())), Some("9f3a1c2b".to_string()));
}

// TS: packages/engine/test/snapshot-names.test.ts:12
#[test]
fn a_project_golden_carries_the_project_and_the_stamp_behind_the_same_mark() {
    let name = project_snapshot_name("h1", "spoo", "2026-09-08T10-00-00-000Z").unwrap();
    assert_eq!(name, "wsp-h1-project-spoo-2026-09-08T10-00-00-000Z");
    assert_eq!(name_owner(Some(&name)), Some("h1".to_string()));
}

// TS: packages/engine/test/snapshot-names.test.ts:17
#[test]
fn a_golden_or_a_project_whose_own_name_carries_hyphens_still_reads_back_to_the_host_that_made_it() {
    assert_eq!(name_owner(Some(&golden_name("h1", "work-mac-2", 3).unwrap())), Some("h1".to_string()));
    assert_eq!(name_owner(Some(&project_snapshot_name("h1", "spoo-me", "s").unwrap())), Some("h1".to_string()));
}

// TS: packages/engine/test/snapshot-names.test.ts:22
#[test]
fn a_name_without_the_mark_has_no_owner_so_nothing_may_claim_it() {
    assert_eq!(name_owner(Some("golden-v1")), None);
    assert_eq!(name_owner(Some("project-spoo-latest")), None);
    assert_eq!(name_owner(Some("base")), None);
    assert_ne!(name_owner(Some("wsp-default-v1")), Some("h1".to_string()));
    assert_eq!(name_owner(None), None);
}

// TS: packages/engine/test/snapshot-names.test.ts:30
#[test]
fn another_hosts_name_reads_as_that_host_never_as_this_one() {
    assert_eq!(name_owner(Some("wsp-zz9-default-v1")), Some("zz9".to_string()));
    assert_ne!(name_owner(Some(&golden_name("zz9", "default", 1).unwrap())), Some("h1".to_string()));
}

// TS: packages/engine/test/snapshot-names.test.ts:35
#[test]
fn a_host_outside_the_class_the_provider_takes_is_refused_rather_than_written_into_a_name_no_parser_reads_back() {
    let words = "lowercase letters and digits";
    for err in [
        golden_name("Mac.local", "default", 1).unwrap_err(),
        golden_name("", "default", 1).unwrap_err(),
        project_snapshot_name("mac local", "spoo", "s").unwrap_err(),
    ] {
        assert!(err.to_string().contains(words), "{err}");
    }
}

// ---- K-rules (spec/sweeps.md K1 to K4), values from running orphans.ts and snapshot-names.ts at a4033dc34

fn listed_row(state: MachineState, labels: IndexMap<String, String>) -> MachineListRow {
    MachineListRow { id: "m".into(), state, labels, size: None }
}

/// K1: a lost workspace is running or paused, `wsp` exactly `1`, owned by this owner, not a builder or smoke fork by the
/// value `1`, and of no readable age or at least a minute old (60000 in, 59999 out, a future stamp out).
#[test]
fn a_lost_workspace_is_read_label_by_label() {
    let own = |extra: &[(&str, &str)], at: Option<String>| {
        let mut l = lab(&[("wsp", "1"), ("wsp-owner", "me")]);
        l.extend(lab(extra));
        if let Some(a) = at {
            l.insert("createdAt".into(), a);
        }
        l
    };
    let lost =
        |state: MachineState, labels: IndexMap<String, String>| wsp_engine::orphans::lost_workspace(&listed_row(state, labels), "me", NOW);
    for (state, labels, want) in [
        (MachineState::Running, own(&[], None), true),
        (MachineState::Paused, own(&[], None), true),
        (MachineState::Starting, own(&[], None), false),
        (MachineState::Gone, own(&[], None), false),
        (MachineState::Running, own(&[], Some(iso(NOW - 60_000))), true),
        (MachineState::Running, own(&[], Some(iso(NOW - 59_999))), false),
        (MachineState::Running, own(&[], Some(iso(NOW + 1_000))), false),
        (MachineState::Running, own(&[], Some("yesterday".into())), true),
        (MachineState::Running, own(&[], Some(String::new())), true),
        (MachineState::Running, own(&[("wsp-owner", "other")], None), false),
        (MachineState::Running, lab(&[("wsp", "1")]), false),
        (MachineState::Running, own(&[("wsp-owner", "")], None), false),
        (MachineState::Running, own(&[("wsp", "0")], None), false),
        (MachineState::Running, own(&[("wsp-builder", "1")], None), false),
        (MachineState::Running, own(&[("wsp-builder", "true")], None), true),
        (MachineState::Running, own(&[("wsp-builder", "0")], None), true),
        (MachineState::Running, own(&[("wsp-smoke", "1")], None), false),
        (MachineState::Running, own(&[("wsp-smoke", "")], None), true),
        (MachineState::Running, own(&[("poc", "")], None), false),
    ] {
        assert_eq!(lost(state, labels.clone()), want, "{state:?} {labels:?}");
    }
}

/// K2: a stray is not gone, not known, not asked, and at least ten minutes old by a stamp `Date.parse` reads (600000 in,
/// 599999 out; paused counts; a date-only stamp is UTC midnight; a future or empty stamp is out), in row order.
#[test]
fn strays_are_ten_minutes_old_by_a_readable_stamp() {
    let m = |id: &str, state: MachineState, at: Option<String>, name: &str| wsp_engine::machine::NamedMachine {
        id: id.into(),
        name: name.into(),
        state,
        created_at: at,
    };
    let rows = vec![
        m("a", MachineState::Running, Some(iso(NOW - 600_000)), "wsp-h1-default"),
        m("b", MachineState::Running, Some(iso(NOW - 599_999)), "wsp-h1-default"),
        m("c", MachineState::Gone, Some(iso(NOW - 6_000_000)), "wsp-h1-default"),
        m("d", MachineState::Running, None, "wsp-h1-default"),
        m("e", MachineState::Running, Some("garbage".into()), "wsp-h1-default"),
        m("f", MachineState::Running, Some(iso(NOW - 6_000_000)), "asked-name"),
        m("g", MachineState::Running, Some(iso(NOW - 6_000_000)), "wsp-h1-default"),
        m("h", MachineState::Paused, Some(iso(NOW - 7_000_000)), "wsp-h1-default"),
        m("i", MachineState::Running, Some(iso(NOW + 5_000)), "wsp-h1-default"),
        m("j", MachineState::Running, Some(String::new()), "wsp-h1-default"),
        m("k", MachineState::Running, Some("2026-09-01".into()), "wsp-h1-default"),
    ];
    let read = wsp_engine::orphans::StrayRead { known: ["g".to_string()].into(), asked: ["asked-name".to_string()].into(), now: NOW };
    let got: Vec<(String, i64)> = wsp_engine::orphans::strays(&rows, &read).into_iter().map(|(r, age)| (r.id, age)).collect();
    assert_eq!(got, vec![("a".to_string(), 600_000), ("h".to_string(), 7_000_000), ("k".to_string(), 43_200_000)]);
}

/// K3: reap compares label values exactly: an empty owner is a foreign owner (spared, never killed); `wsp` other than
/// `1` is passed over; `wsp-builder` other than `1` is not a builder (ten-minute backstop, killed as an orphan).
#[tokio::test]
async fn reap_reads_label_values_exactly() {
    let hour = iso(NOW - 3_600_000);
    let backend = StubBackend::new(vec![
        row("empty-owner", lab(&[("wsp", "1"), ("wsp-owner", ""), ("createdAt", &hour)])),
        row("wsp-true", lab(&[("wsp", "true"), ("createdAt", &hour)])),
        row("wsp-empty", lab(&[("wsp", ""), ("createdAt", &hour)])),
        row("builder-true", lab(&[("wsp", "1"), ("wsp-builder", "true"), ("createdAt", &hour)])),
        row("builder-0", lab(&[("wsp", "1"), ("wsp-builder", "0"), ("createdAt", &hour)])),
        row("builder-1", lab(&[("wsp", "1"), ("wsp-builder", "1"), ("createdAt", &hour)])),
    ]);
    let result = reap(opts(&backend, "me", vec![])).await.unwrap();
    assert_eq!(backend.killed(), strs(&["builder-true", "builder-0"]));
    assert_eq!(
        result.reaped.iter().map(|r| (r.id.clone(), r.reason, r.builder)).collect::<Vec<_>>(),
        vec![("builder-true".to_string(), ReapReason::Orphan, false), ("builder-0".to_string(), ReapReason::Orphan, false),]
    );
    let spared: Vec<(String, Whose, Option<String>, i64, Option<i64>)> =
        result.spared.iter().map(|s| (s.id.clone(), s.whose, s.owner.clone(), s.backstop_ms, s.age_ms)).collect();
    assert_eq!(
        spared,
        vec![
            ("empty-owner".to_string(), Whose::Foreign, Some(String::new()), 600_000, Some(3_600_000)),
            ("builder-1".to_string(), Whose::None, None, 21_600_000, Some(3_600_000)),
        ]
    );
}

/// K4: an age is never clamped (a future own stamp is spared at a negative age); `older_than_ms: Some(0)` is a value
/// (age 0 dies, a future stamp lives with backstop 0); a given empty listing is never re-read, and the known ids are
/// read exactly once.
#[tokio::test]
async fn reap_keeps_negative_ages_takes_zero_as_a_value_and_reads_known_ids_once() {
    let backend = StubBackend::new(vec![row("future-own", lab(&[("wsp", "1"), ("wsp-owner", "me"), ("createdAt", &iso(NOW + 300_000))]))]);
    let result = reap(opts(&backend, "me", vec![])).await.unwrap();
    assert_eq!(result.spared.iter().map(|s| (s.age_ms, s.backstop_ms)).collect::<Vec<_>>(), vec![(Some(-300_000), 60_000)]);
    let backend = StubBackend::new(vec![
        row("aged-1s", lab(&[("wsp", "1"), ("createdAt", &iso(NOW - 1_000))])),
        row("aged-0", lab(&[("wsp", "1"), ("createdAt", &iso(NOW))])),
        row("future", lab(&[("wsp", "1"), ("createdAt", &iso(NOW + 1_000))])),
    ]);
    let result = reap(ReapOptions { older_than_ms: Some(0), ..opts(&backend, "me", vec![]) }).await.unwrap();
    assert_eq!(backend.killed(), strs(&["aged-1s", "aged-0"]));
    assert_eq!(
        result.spared.iter().map(|s| (s.id.clone(), s.age_ms, s.backstop_ms)).collect::<Vec<_>>(),
        vec![("future".to_string(), Some(-1_000), 0)]
    );
    let backend = StubBackend::new(vec![row("x", lab(&[("wsp", "1"), ("createdAt", &old())]))]);
    let reads = Arc::new(Mutex::new(0));
    let counted = reads.clone();
    let result = reap(ReapOptions {
        listing: Some(vec![]),
        known_ids: Box::new(move || {
            *counted.lock().unwrap() += 1;
            vec![]
        }),
        ..opts(&backend, "me", vec![])
    })
    .await
    .unwrap();
    assert!(result.reaped.is_empty() && result.spared.is_empty());
    assert_eq!((backend.listed(), *reads.lock().unwrap()), (0, 1));
}

/// K5: JS `.` refuses `\n`, `\r`, U+2028 and U+2029, so a name holding any of them has no owner (foreign, never an
/// orphan to delete); the owner mark is `[a-z0-9]+` and nothing is trimmed.
#[test]
fn a_name_with_any_js_line_break_has_no_owner() {
    for (name, want) in [
        ("wsp-h1-a\n", None),
        ("wsp-h1-a\rb", None),
        ("wsp-h1-a\u{2028}b", None),
        ("wsp-h1-a\u{2029}b", None),
        ("wsp-h1-\r", None),
        ("wsp-h1-é", Some("h1")),
        ("wsp-h1--v1", Some("h1")),
        ("wsp-0-x", Some("0")),
        ("wsp-h1_2-x", None),
        ("WSP-h1-x", None),
        (" wsp-h1-x", None),
        ("wsp-h1-x ", Some("h1")),
    ] {
        assert_eq!(name_owner(Some(name)).as_deref(), want, "{name:?}");
    }
}
