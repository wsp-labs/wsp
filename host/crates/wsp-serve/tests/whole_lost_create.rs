// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/lost-create.test.ts at 65f6b89c7, case for case. A create whose answer never came back: the
//! attempt keeps the bytes it sent, and the sweep asks again under its key and settles the machine the provider made,
//! or deletes the box named after one of this host's images that nothing names. The provider is the stub with the three
//! calls a key-keeping provider carries.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::types::api::{PrepareAsk, UpgradeAsk};
use wsp_core::types::wiring::{GoldenRecipe, SweepResult};
use wsp_engine::machine::{MachineSpec, NamedMachine};
use wsp_engine::no_provider_backend::NoProviderBackend;
use wsp_engine::orphans::date_parse;
use wsp_store::keys::new_place_key_pair;
use wsp_store::store::Store;

const HOST: &str = "boat-test";
/// 2026-10-11T10:00:00.000Z
const T0: i64 = 1_791_712_800_000;
/// Above any pid the kernel hands out, so no process holds it.
const DEAD_PID: u32 = 4_194_305;
/// How long the fake provider keeps a create's key: Boat's 24 hours, less the two minutes its own asks may take.
const KEY_KEPT_MS: i64 = 24 * 3_600_000 - 120_000;

#[derive(Clone, Debug, PartialEq)]
struct Sent {
    key: String,
    bytes: String,
}

/// The stub as a provider that keeps a key: the bytes a create sends carry the moment they were made, a create may
/// lose its answer after the machine is made, a replay answers the machine its key made and names it after the bytes,
/// and the listing by name sees a machine made from an image under that image's name until then.
struct KeyKeeping {
    fc: FakeClock,
    sent: Rc<RefCell<Vec<Sent>>>,
    replays: Rc<RefCell<Vec<Sent>>>,
    made_by: Rc<RefCell<IndexMap<String, Arc<StubMachine>>>>,
    names: Rc<RefCell<IndexMap<String, String>>>,
    created_at: Rc<RefCell<IndexMap<String, String>>>,
    asked: Rc<RefCell<Vec<Vec<String>>>>,
    lose: Rc<Cell<u32>>,
    hold: Rc<RefCell<Option<Rc<Gate>>>>,
    real: common::stub_backend::CreateFn,
}

fn iso(ms: i64) -> String {
    wsp_wire::views::activity::iso_time(ms)
}

impl KeyKeeping {
    fn minutes_ago(&self, n: i64) -> String {
        iso(self.fc.now() - n * 60_000)
    }

    /// A machine the provider lists under `name` and no labels, made `age` minutes ago.
    async fn stray(&self, name: &str, age: i64) -> Arc<StubMachine> {
        let m = (self.real)(sandbox_spec(), None, None).await.expect("a machine");
        self.names.borrow_mut().insert(m.id().to_string(), name.to_string());
        self.created_at.borrow_mut().insert(m.id().to_string(), self.minutes_ago(age));
        m
    }

    fn sent_last(&self) -> Sent {
        self.sent.borrow().last().cloned().expect("a create was sent")
    }
}

fn sandbox_spec() -> MachineSpec {
    serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec")
}

fn key_keeping(backend: &Arc<StubBackend>, fc: &FakeClock) -> Rc<KeyKeeping> {
    let real = backend.create_fn();
    let kk = Rc::new(KeyKeeping {
        fc: fc.clone(),
        sent: Rc::default(),
        replays: Rc::default(),
        made_by: Rc::default(),
        names: Rc::default(),
        created_at: Rc::default(),
        asked: Rc::default(),
        lose: Rc::default(),
        hold: Rc::default(),
        real: real.clone(),
    });
    let clock = fc.clone();
    backend.set_create_request(Some(Rc::new(move |spec| {
        let mut body = serde_json::Map::new();
        body.insert("labels".into(), json!(spec.labels.clone().unwrap_or_default()));
        if let Some(from) = spec.from_snapshot.clone().or(spec.template.clone()) {
            body.insert("from".into(), json!(from));
        }
        body.insert("made".into(), json!(format!("{}.{}", clock.now(), rand::random::<f64>())));
        let text = Value::Object(body).to_string();
        Box::pin(async move { Ok(text) })
    })));
    let (k, b) = (kk.clone(), backend.clone());
    backend.set_create(Rc::new(move |spec, waiting, bytes| {
        let (kk, backend, real) = (k.clone(), b.clone(), k.real.clone());
        Box::pin(async move {
            let body = match bytes {
                Some(bytes) => bytes,
                None => backend.create_request_fn().expect("a request")(spec.clone()).await?,
            };
            let key = spec.idempotency_key.clone().expect("a key");
            kk.sent.borrow_mut().push(Sent { key: key.clone(), bytes: body });
            let hold = kk.hold.borrow().clone();
            if let Some(hold) = hold {
                hold.wait().await;
            }
            if kk.lose.get() > 0 {
                kk.lose.set(kk.lose.get() - 1);
                let mut bare = spec.clone();
                bare.labels = Some(IndexMap::new());
                let m = real(bare, waiting, None).await?;
                kk.made_by.borrow_mut().insert(key, m.clone());
                kk.names.borrow_mut().insert(m.id().to_string(), spec.from_snapshot.clone().or(spec.template.clone()).unwrap_or_default());
                return Err(fetch_failed());
            }
            real(spec, waiting, None).await
        })
    }));
    let k = kk.clone();
    backend.set_replay_create(Some(Rc::new(move |key, bytes, made_at| {
        let kk = k.clone();
        Box::pin(async move {
            // Past the window the provider keeps a key, an ask would make a new box, so none is sent.
            if kk.fc.now() - date_parse(&made_at) as i64 >= KEY_KEPT_MS {
                return Ok(None);
            }
            kk.replays.borrow_mut().push(Sent { key: key.clone(), bytes: bytes.clone() });
            let held = kk.made_by.borrow().get(&key).cloned();
            let m = match held {
                Some(m) => m,
                None => (kk.real)(sandbox_spec(), None, None).await?,
            };
            let doc: Value = serde_json::from_str(&bytes).expect("the bytes are JSON");
            let labels: IndexMap<String, String> = serde_json::from_value(doc["labels"].clone()).expect("labels");
            m.spec.borrow_mut().labels = Some(labels.clone());
            m.set_labels(labels);
            kk.names.borrow_mut().shift_remove(m.id());
            Ok(Some(m))
        })
    })));
    let (k, b) = (kk.clone(), backend.clone());
    backend.set_named_after(Some(Rc::new(move |wanted: BTreeSet<String>| {
        let (kk, backend) = (k.clone(), b.clone());
        Box::pin(async move {
            kk.asked.borrow_mut().push(wanted.iter().cloned().collect());
            let names = kk.names.borrow().clone();
            let created = kk.created_at.borrow().clone();
            Ok(backend
                .machines()
                .into_iter()
                .filter(|m| !m.killed.get() && wanted.contains(names.get(m.id()).map_or("", String::as_str)))
                .map(|m| NamedMachine {
                    id: m.id().to_string(),
                    name: names[m.id()].clone(),
                    state: wsp_engine::machine::MachineState::Running,
                    created_at: created.get(m.id()).cloned(),
                })
                .collect())
        })
    })));
    kk
}

struct Setup {
    fx: Fx,
    store: Rc<dyn Store>,
    backend: Arc<StubBackend>,
    kk: Rc<KeyKeeping>,
    fc: FakeClock,
}

fn setup_over(store: Option<Rc<dyn Store>>, backend: Option<Arc<StubBackend>>, fc: &FakeClock) -> Setup {
    let backend = backend.unwrap_or_else(|| StubBackend::new(None));
    let kk = key_keeping(&backend, fc);
    let mut wiring = runtime(&backend).clock(fc).with(|o| o.host_id = Some(HOST.into()));
    let kept = store.unwrap_or_else(|| sqlite_state_kept());
    wiring = wiring.store(&kept);
    Setup { fx: wiring.build(), store: kept, backend, kk, fc: fc.clone() }
}

thread_local! {
    static STATES: RefCell<Vec<State>> = const { RefCell::new(Vec::new()) };
}

/// A sqlite store whose folder lives to the end of the test thread.
fn sqlite_state_kept() -> Rc<dyn Store> {
    let state = sqlite_state();
    let store = state.store.clone();
    STATES.with(|s| s.borrow_mut().push(state));
    store
}

fn setup() -> Setup {
    setup_over(None, None, &FakeClock::new(T0))
}

fn set_time(fc: &FakeClock, ms: i64) {
    fc.advance(ms - fc.now());
}

fn creates_of(rows: &[Value]) -> Vec<(String, String)> {
    rows.iter()
        .map(|r| (r["key"].as_str().unwrap_or_default().to_string(), r["request"].as_str().unwrap_or_default().to_string()))
        .collect()
}

fn reaped_has(swept: &SweepResult, id: &str, reason: &str, name: Option<&str>) -> bool {
    swept
        .reaped
        .reaped
        .iter()
        .any(|r| r.id == id && format!("{:?}", r.reason).eq_ignore_ascii_case(reason) && name.is_none_or(|n| r.name.as_deref() == Some(n)))
}

// "a create whose answer is lost" > "keeps the exact bytes it sent beside its key, and a create asked again for the record sends those bytes"
#[test]
fn keeps_the_exact_bytes_it_sent_beside_its_key_and_a_create_asked_again_for_the_record_sends_those_bytes() {
    drive(|| async {
        let s = setup();
        let _w = capture();
        let ws = create_on(&s.fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        s.kk.lose.set(1);
        let refused = s.fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let first = s.kk.sent_last();
        let rows = s.store.list("creates").await.expect("a list");
        assert_eq!(creates_of(&rows), [(first.key.clone(), first.bytes.clone())]);
        set_time(&s.fc, T0 + 30_000);
        s.fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        let again = s.kk.sent_last();
        assert_eq!(again, first);
        assert!(s.store.list("creates").await.expect("a list").is_empty());
    });
}

// "a workspace whose create lost its answer leaves the attempt for the sweep, which replays it after two minutes and deletes the box no record waits for"
#[test]
fn a_workspace_whose_create_lost_its_answer_leaves_the_attempt_for_the_sweep_which_replays_it_after_two_minutes_and_deletes_the_box_no_record_waits_for(
) {
    drive(|| async {
        let s = setup();
        let _w = capture();
        s.kk.lose.set(1);
        let refused = create_on(&s.fx.rt, CreateOn::golden("x"), None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let lost = s.kk.made_by.borrow().values().next().cloned().expect("a lost machine");
        let sent = s.kk.sent_last();
        assert_eq!(creates_of(&s.store.list("creates").await.expect("a list")), [(sent.key.clone(), sent.bytes.clone())]);

        set_time(&s.fc, T0 + 119_000);
        s.fx.rt.reap(None, None).await.expect("a sweep");
        assert!(s.kk.replays.borrow().is_empty());
        assert!(!lost.killed.get());

        set_time(&s.fc, T0 + 120_000);
        let swept = s.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(*s.kk.replays.borrow(), [sent]);
        assert!(lost.killed.get());
        assert!(reaped_has(&swept, lost.id(), "lost", None), "{:?}", swept.reaped.reaped);
        assert!(s.store.list("creates").await.expect("a list").is_empty());
    });
}

// "the sweep adopts the box when its record still waits for it: the name was kept and no record holds the workspace"
#[test]
fn the_sweep_adopts_the_box_when_its_record_still_waits_for_it_the_name_was_kept_and_no_record_holds_the_workspace() {
    drive(|| async {
        let fc = FakeClock::new(T0);
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let first = setup_over(Some(state.store.clone()), Some(backend.clone()), &fc);
        let _w = capture();
        let ws = create_on(&first.fx.rt, CreateOn::golden("kept"), None).await.expect("a workspace");
        let mut labels = backend.machines().last().expect("a machine").spec_now().labels.expect("labels");
        labels.insert("createdAt".into(), first.kk.minutes_ago(3));
        first.fx.rt.close().await.expect("a close");
        backend.machines().last().expect("a machine").killed.set(true);
        state.store.delete("workspaces", &ws.view.id).await.expect("a delete");
        // The earlier process asked for a machine for this workspace and died before the answer came.
        let key = format!("workspace/{}:00112233aabbccdd", ws.view.id);
        let bytes = json!({ "labels": labels, "from": "snap_g", "made": "earlier" }).to_string();
        let lost = first.kk.stray("snap_g", 3).await;
        first.kk.made_by.borrow_mut().insert(key.clone(), lost.clone());
        state
            .store
            .put(
                "creates",
                &format!("workspace/{}", ws.view.id),
                json!({ "key": key, "createdAt": first.kk.minutes_ago(3), "body": "earlier", "request": bytes, "image": "snap_g", "host": HOST, "pid": DEAD_PID }),
            )
            .await
            .expect("a put");

        let second = runtime(&backend).clock(&fc).store(&state.store).with(|o| o.host_id = Some(HOST.into())).build();
        let swept = second.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(*first.kk.replays.borrow(), [Sent { key, bytes }]);
        assert!(!lost.killed.get());
        let adopted = swept.adopted.expect("an adopted list");
        assert!(adopted.iter().any(|a| a.id == lost.id() && a.workspace_id == ws.view.id), "{adopted:?}");
        assert_eq!(second.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id, lost.id());
        assert!(state.store.list("creates").await.expect("a list").is_empty());
        second.rt.close().await.expect("a close");
    });
}

// "a create still in flight in this process is never replayed, however old its attempt"
#[test]
fn a_create_still_in_flight_in_this_process_is_never_replayed_however_old_its_attempt() {
    drive(|| async {
        let s = setup();
        let _w = capture();
        let gate = Gate::new();
        *s.kk.hold.borrow_mut() = Some(gate.clone());
        let rt = s.fx.rt.clone();
        let making = tokio::task::spawn_local(async move { create_on(&rt, CreateOn::golden("slow"), None).await });
        let store = s.store.clone();
        until(|| {
            let store = store.clone();
            async move { store.list("creates").await.expect("a list").len() == 1 }
        })
        .await;
        set_time(&s.fc, T0 + 5 * 60_000);
        s.fx.rt.reap(None, None).await.expect("a sweep");
        assert!(s.kk.replays.borrow().is_empty());
        gate.open();
        making.await.expect("the task").expect("a workspace");
        assert!(s.store.list("creates").await.expect("a list").is_empty());
    });
}

/// A host that forks on none, as every public build does, with Boat a place beside it under its own backend.
struct Beside {
    fx: Fx,
    store: Rc<dyn Store>,
    boxed: Arc<StubBackend>,
    kk: Rc<KeyKeeping>,
    fc: FakeClock,
}

fn boat_beside(store: Option<Rc<dyn Store>>) -> Beside {
    let fc = FakeClock::new(T0);
    let none: Arc<dyn wsp_engine::machine::MachineBackend> = Arc::new(NoProviderBackend::new(None));
    let boxed = StubBackend::new(None);
    let kk = key_keeping(&boxed, &fc);
    let places = Rc::new(Places {
        wired: "none".into(),
        backends: vec![("none".into(), none.clone()), ("box".into(), boxed.clone() as Arc<dyn wsp_engine::machine::MachineBackend>)],
        list: vec!["box".into()],
    });
    let kept = store.unwrap_or_else(sqlite_state_kept);
    let fx = over(none)
        .clock(&fc)
        .store(&kept)
        .with(move |o| {
            o.places = Some(places);
            o.host_id = Some(HOST.into());
            o.place_links = Some(place_wiring(new_place_key_pair()));
        })
        .build();
    Beside { fx, store: kept, boxed, kk, fc }
}

fn recipe() -> GoldenRecipe {
    GoldenRecipe { setup: "true".into(), smoke: "true".into(), ..Default::default() }
}

fn prepare_at_box() -> PrepareAsk {
    PrepareAsk { place: Some("box".into()), recipe: Some(recipe()), ..Default::default() }
}

fn has_creates(rows: &[Value], key: &str, place: &str) -> bool {
    rows.len() == 1 && rows[0]["key"] == key && rows[0]["place"] == place
}

// "a create at a place this host does not fork on, whose answer is lost" > "is asked again at that place, which deletes the box it made and clears the attempt"
#[test]
fn is_asked_again_at_that_place_which_deletes_the_box_it_made_and_clears_the_attempt() {
    drive(|| async {
        let b = boat_beside(None);
        let _w = capture();
        b.kk.lose.set(1);
        let refused = create_on(&b.fx.rt, CreateOn::golden("x").on("box"), None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let lost = b.kk.made_by.borrow().values().next().cloned().expect("a lost machine");
        let sent = b.kk.sent_last();
        assert_eq!(creates_of(&b.store.list("creates").await.expect("a list")), [(sent.key.clone(), sent.bytes.clone())]);

        set_time(&b.fc, T0 + 120_000);
        let swept = b.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(*b.kk.replays.borrow(), [sent]);
        assert!(lost.killed.get());
        assert!(reaped_has(&swept, lost.id(), "lost", None), "{:?}", swept.reaped.reaped);
        assert!(b.store.list("creates").await.expect("a list").is_empty());
        b.fx.rt.close().await.expect("a close");
    });
}

// "a builder's create at that place is asked again there too"
#[test]
fn a_builders_create_at_that_place_is_asked_again_there_too() {
    drive(|| async {
        let b = boat_beside(None);
        let _w = capture();
        b.kk.lose.set(1);
        let refused = b.fx.rt.golden().prepare(prepare_at_box()).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let lost = b.kk.made_by.borrow().values().next().cloned().expect("a lost machine");
        assert_eq!(b.store.list("creates").await.expect("a list").len(), 1);

        set_time(&b.fc, T0 + 120_000);
        b.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(*b.kk.replays.borrow(), [b.kk.sent_last()]);
        assert!(lost.killed.get());
        assert!(b.store.list("creates").await.expect("a list").is_empty());
        b.fx.rt.close().await.expect("a close");
    });
}

// "a seal's smoke create at that place saves the place, and is asked again there"
#[test]
fn a_seals_smoke_create_at_that_place_saves_the_place_and_is_asked_again_there() {
    drive(|| async {
        let b = boat_beside(None);
        let _w = capture();
        b.boxed.set_exec_impl(images::df_ok());
        let built = b.fx.rt.golden().prepare(prepare_at_box()).await.expect("a builder");
        b.kk.lose.set(1);
        let refused = b.fx.rt.golden().seal(&built.id, None, None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let rows = b.store.list("creates").await.expect("a list");
        assert!(has_creates(&rows, &b.kk.sent_last().key, "box"), "{rows:?}");

        set_time(&b.fc, T0 + 120_000);
        b.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(*b.kk.replays.borrow(), [b.kk.sent_last()]);
        assert!(b.store.list("creates").await.expect("a list").is_empty());
        b.fx.rt.close().await.expect("a close");
    });
}

// "an update's builder created at that place saves the place, and is asked again there"
#[test]
fn an_updates_builder_created_at_that_place_saves_the_place_and_is_asked_again_there() {
    drive(|| async {
        let b = boat_beside(None);
        let _w = capture();
        b.boxed.set_exec_impl(images::df_ok());
        let built = b.fx.rt.golden().prepare(prepare_at_box()).await.expect("a builder");
        b.fx.rt.golden().seal(&built.id, None, Some(false)).await.expect("a seal");
        b.kk.lose.set(1);
        // The TS hands `upgrade` a delta of an import of recipe "h2" with no files, tools or agents. The Rust
        // `UpgradeAsk.delta` is wsp-core's empty `GoldenDelta {}` marker (types/pending.rs:57), which carries none.
        let ask = UpgradeAsk { recipe: Some(recipe()), ..Default::default() };
        let refused = b.fx.rt.golden().upgrade(ask).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let rows = b.store.list("creates").await.expect("a list");
        assert!(has_creates(&rows, &b.kk.sent_last().key, "box"), "{rows:?}");

        set_time(&b.fc, T0 + 120_000);
        b.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(*b.kk.replays.borrow(), [b.kk.sent_last()]);
        assert!(b.store.list("creates").await.expect("a list").is_empty());
        b.fx.rt.close().await.expect("a close");
    });
}

// "a rebuild under the same key while the sweep asks it again waits for the replay and keeps a box of its own" (with `wired` false and true)
#[test]
fn a_rebuild_under_the_same_key_while_the_sweep_asks_it_again_waits_for_the_replay_and_keeps_a_box_of_its_own() {
    rebuild_beside_a_replay(false);
}

#[test]
fn a_rebuild_under_the_same_key_while_the_sweep_asks_it_again_waits_for_the_replay_and_keeps_a_box_of_its_own_where_boat_is_the_provider_this_host_forks_on(
) {
    rebuild_beside_a_replay(true);
}

fn rebuild_beside_a_replay(wired: bool) {
    drive(move || async move {
        let _w = capture();
        let (rt, store, boxed, kk, fc, on) = if wired {
            let s = setup();
            (s.fx.rt.clone(), s.store.clone(), s.backend.clone(), s.kk.clone(), s.fc.clone(), None)
        } else {
            let b = boat_beside(None);
            (b.fx.rt.clone(), b.store.clone(), b.boxed.clone(), b.kk.clone(), b.fc.clone(), Some("box"))
        };
        let ws = create_on(&rt, CreateOn { on: on.map(str::to_string), ..CreateOn::golden("x") }, None).await.expect("a workspace");
        kk.lose.set(1);
        let refused = rt.workspaces().rebuild(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let lost = kk.made_by.borrow().values().next().cloned().expect("a lost machine");
        // Boat answers a key it already made with that box, replayed.
        let create = boxed.create_fn();
        let made = kk.made_by.clone();
        let hold = kk.hold.clone();
        boxed.set_create(Rc::new(move |spec, waiting, bytes| {
            let (create, made, hold) = (create.clone(), made.clone(), hold.clone());
            Box::pin(async move {
                let gate = hold.borrow().clone();
                if let Some(gate) = gate {
                    gate.wait().await;
                }
                let had = made.borrow().get(spec.idempotency_key.as_deref().unwrap_or_default()).cloned();
                let Some(had) = had else { return create(spec, waiting, bytes).await };
                common::stub_backend::say_replayed(&waiting, had.id());
                had.replayed.set(true);
                Ok(had)
            })
        }));
        let asked = Rc::new(Cell::new(false));
        let answer_replay = Gate::new();
        let replay = boxed.replay_create_fn().expect("a replay");
        let (seen, gate) = (asked.clone(), answer_replay.clone());
        boxed.set_replay_create(Some(Rc::new(move |key, bytes, made_at| {
            seen.set(true);
            let (gate, replay) = (gate.clone(), replay.clone());
            Box::pin(async move {
                gate.wait().await;
                replay(key, bytes, made_at).await
            })
        })));
        let answer_create = Gate::new();
        *kk.hold.borrow_mut() = Some(answer_create.clone());
        let land_kill = Gate::new();
        let kill = lost.kill_fn();
        let landed = land_kill.clone();
        lost.set_kill(Rc::new(move || {
            let (landed, kill) = (landed.clone(), kill.clone());
            Box::pin(async move {
                landed.wait().await;
                kill().await
            })
        }));

        set_time(&fc, T0 + 120_000);
        let sweeping = rt.clone();
        let sweep = tokio::task::spawn_local(async move { sweeping.reap(None, None).await });
        let flag = asked.clone();
        until(|| {
            let flag = flag.clone();
            async move { flag.get() }
        })
        .await;
        let again_rt = rt.clone();
        let id = ws.view.id.clone();
        let again = tokio::task::spawn_local(async move { again_rt.workspaces().rebuild(&id, None).await });
        // The replay's answer comes back first, then the rebuild's under the same key, then the kill lands.
        answer_replay.open();
        sleep_ms(50).await;
        answer_create.open();
        sleep_ms(50).await;
        land_kill.open();
        again.await.expect("the task").expect("a rebuild");
        sweep.await.expect("the task").expect("a sweep");

        let held = rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id;
        assert!(lost.killed.get());
        assert_ne!(held, lost.id());
        assert!(!boxed.machines().iter().find(|m| m.id() == held).expect("the held machine").killed.get());
        assert!(store.list("creates").await.expect("a list").is_empty());
        rt.close().await.expect("a close");
    });
}

// "clears the attempt when that place says its key made nothing"
#[test]
fn clears_the_attempt_when_that_place_says_its_key_made_nothing() {
    drive(|| async {
        let b = boat_beside(None);
        let _w = capture();
        b.kk.lose.set(1);
        let refused = create_on(&b.fx.rt, CreateOn::golden("x").on("box"), None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let replays = b.kk.replays.clone();
        b.boxed.set_replay_create(Some(Rc::new(move |key, bytes, _| {
            replays.borrow_mut().push(Sent { key: key.clone(), bytes });
            Box::pin(async move { Err(common::stub_backend::missing(&format!("no create under {key}"))) })
        })));

        set_time(&b.fc, T0 + 120_000);
        b.fx.rt.reap(None, None).await.expect("a sweep");
        assert_eq!(b.kk.replays.borrow().len(), 1);
        assert!(b.store.list("creates").await.expect("a list").is_empty());
        b.fx.rt.close().await.expect("a close");
    });
}

fn image_of(owner: &str) -> String {
    let host = format!("{}{}", wsp_core::host_id::template_host(HOST), wsp_core::host_id::template_host(owner));
    wsp_engine::snapshot_names::golden_name(&host, "default", 4).expect("a golden name")
}

// "an attempt older than the window the provider keeps its key" > "is cleared without an ask, and the box its first create made is deleted as one named after this host's image"
#[test]
fn is_cleared_without_an_ask_and_the_box_its_first_create_made_is_deleted_as_one_named_after_this_hosts_image() {
    past_the_window(false);
}

#[test]
fn is_cleared_without_an_ask_and_the_box_its_first_create_made_is_deleted_as_one_named_after_this_hosts_image_at_a_place_this_host_does_not_fork_on(
) {
    past_the_window(true);
}

fn past_the_window(at_place: bool) {
    drive(move || async move {
        let _w = capture();
        let owned = owned_store("s1").await;
        let (rt, store, boxed, kk, fc) = if at_place {
            let b = boat_beside(Some(owned));
            (b.fx.rt.clone(), b.store.clone(), b.boxed.clone(), b.kk.clone(), b.fc.clone())
        } else {
            let s = setup_over(Some(owned), None, &FakeClock::new(T0));
            (s.fx.rt.clone(), s.store.clone(), s.backend.clone(), s.kk.clone(), s.fc.clone())
        };
        let image = image_of(&rt.owner().await.expect("an owner"));
        let on = at_place.then(|| "box".to_string());
        create_on(&rt, CreateOn { on: on.clone(), ..CreateOn::named("kept").golden_is(&image) }, None).await.expect("a workspace");
        kk.lose.set(1);
        let refused = create_on(&rt, CreateOn { on, ..CreateOn::named("x").golden_is(&image) }, None).await.expect_err("a refusal");
        assert!(refused.message.contains("fetch failed"), "{}", refused.message);
        let lost = kk.made_by.borrow().values().next().cloned().expect("a lost machine");
        kk.created_at.borrow_mut().insert(lost.id().to_string(), kk.minutes_ago(0));
        let made = boxed.made();

        // A day on the provider has forgotten the key, and an ask under it would make a box anew.
        set_time(&fc, T0 + 24 * 3_600_000);
        kk.created_at.borrow_mut().insert(lost.id().to_string(), kk.minutes_ago(24 * 60));
        kk.made_by.borrow_mut().clear();
        let swept = rt.reap(None, None).await.expect("a sweep");
        assert!(kk.replays.borrow().is_empty());
        assert_eq!(boxed.made(), made);
        assert!(store.list("creates").await.expect("a list").is_empty());
        assert!(lost.killed.get());
        assert!(reaped_has(&swept, lost.id(), "stray", Some(&image)), "{:?}", swept.reaped.reaped);
        rt.close().await.expect("a close");
    });
}

// "a box named after one of this host's images" > "is deleted once it is ten minutes old and no record or attempt names it; a box under any other name is never touched"
#[test]
fn is_deleted_once_it_is_ten_minutes_old_and_no_record_or_attempt_names_it_a_box_under_any_other_name_is_never_touched() {
    named_after_an_image(false);
}

#[test]
fn is_deleted_once_it_is_ten_minutes_old_and_no_record_or_attempt_names_it_a_box_under_any_other_name_is_never_touched_at_a_place_this_host_does_not_fork_on(
) {
    named_after_an_image(true);
}

fn named_after_an_image(at_place: bool) {
    drive(move || async move {
        let _w = capture();
        let (rt, store, kk) = if at_place {
            let b = boat_beside(None);
            (b.fx.rt.clone(), b.store.clone(), b.kk.clone())
        } else {
            let s = setup();
            (s.fx.rt.clone(), s.store.clone(), s.kk.clone())
        };
        let on = at_place.then(|| "box".to_string());
        let image = image_of(&rt.owner().await.expect("an owner"));
        let ws = create_on(&rt, CreateOn { on, ..CreateOn::named("w").golden_is(&image) }, None).await.expect("a workspace");
        let own = rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id;
        kk.names.borrow_mut().insert(own.clone(), image.clone());
        kk.created_at.borrow_mut().insert(own.clone(), kk.minutes_ago(30));

        let stray = kk.stray(&image, 11).await;
        let young = kk.stray(&image, 9).await;
        let other_host = wsp_engine::snapshot_names::golden_name("otherhost", "default", 4).expect("a golden name");
        let mut untouched = vec![];
        for (name, age) in [(format!("{image}-copy"), 60), (other_host, 60), ("my box".to_string(), 60), (String::new(), 60)] {
            untouched.push(kk.stray(&name, age).await);
        }
        // An attempt of this host that asked for the same image, not yet two minutes old: the box may be its.
        let mut pending = json!({ "key": "workspace/ws_pending:00", "createdAt": kk.minutes_ago(1), "body": "b", "request": "{}", "image": image, "host": HOST, "pid": DEAD_PID });
        if at_place {
            pending["place"] = json!("box");
        }
        store.put("creates", "workspace/ws_pending", pending).await.expect("a put");
        rt.reap(None, None).await.expect("a sweep");
        assert!(!stray.killed.get());
        store.delete("creates", "workspace/ws_pending").await.expect("a delete");

        let swept = rt.reap(None, None).await.expect("a sweep");
        assert!(stray.killed.get());
        assert!(reaped_has(&swept, stray.id(), "stray", Some(&image)), "{:?}", swept.reaped.reaped);
        assert!(!young.killed.get());
        assert_eq!(rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id, own);
        assert!(kk.names.borrow().contains_key(&own));
        assert_eq!(untouched.iter().map(|m| m.killed.get()).collect::<Vec<_>>(), [false, false, false, false]);
        // The listing is asked for this host's image names alone, so no other box is even read.
        assert_eq!(kk.asked.borrow().last().cloned(), Some(vec![image]));
    });
}
