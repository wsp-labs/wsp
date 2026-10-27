// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/reconcile.test.ts at 65f6b89c7, case for case. A record never leaves the store while its machine
//! exists at the provider, and the other way round: a workspace machine of this setup's that no record claims is recorded
//! again at the sweep, never killed; a running one whose workspace was deleted here is killed. A name names at most one
//! workspace; a rename takes the fork's own rule, keeps the record's id and machine, and leaves the threads alone.
#[macro_use]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::types::internal::{copy_key, DROPPED_WATCH_MS};
use wsp_core::types::wiring::CreateWorkspaceOptions;
use wsp_engine::errors::EngineError;
use wsp_engine::gone::KillConfirm;
use wsp_engine::machine::MachineSpec;
use wsp_store::store::Store;
use wsp_words::exit::{HostError, Kind};
use wsp_words::words::thread::{name_deleting_refusal, name_taken_refusal, BLANK_NAME_REFUSAL, RECORD_RESTORED};

const T0: i64 = 1_788_220_800_000;

fn ago(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(now_ms() - ms).expect("a time").strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn spec(doc: Value) -> MachineSpec {
    serde_json::from_value(doc).expect("a machine spec")
}

fn set_created(m: &StubMachine, ms: i64) {
    let mut labels = m.labels_now().expect("labels");
    labels.insert("createdAt".into(), ago(ms));
    m.set_labels(labels.clone());
    m.spec.borrow_mut().labels = Some(labels);
}

fn confirm() -> KillConfirm {
    KillConfirm { grace_ms: Some(50), poll_ms: Some(1) }
}

async fn owner_of(store: &Rc<dyn Store>) -> String {
    store.get("owner", "id").await.expect("a read").expect("an owner")["id"].as_str().expect("an id").to_string()
}

fn refused_with(e: &HostError, words: &str) {
    assert_eq!((e.message.as_str(), e.kind.clone()), (words, Some(Kind::Conflict)));
}

struct Lost {
    backend: Arc<StubBackend>,
    state: State,
    fx: Fx,
    ws: String,
    machine: Arc<StubMachine>,
    events: Events,
}

/// A workspace forked by one process whose record the store then lost, seen by a fresh runtime over the same store.
async fn lost_record(paused: bool) -> Lost {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let first = runtime(&backend).store(&state.store).build();
    let ws = create_on(&first.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
    first.rt.close().await.expect("a close");
    let machine = backend.machine(0);
    set_created(&machine, 2 * 60_000);
    machine.paused.set(paused);
    state.store.delete("workspaces", &ws).await.expect("a delete");
    let fx = runtime(&backend).store(&state.store).build();
    let events = fx.events();
    Lost { backend, state, fx, ws, machine, events }
}

fn lower<T: std::fmt::Debug>(v: &T) -> String {
    format!("{v:?}").to_lowercase()
}

/// A sweep as the JSON the TS result is: `adopted` and `failed` are keys only where there is something under them.
async fn reap(rt: &wsp_serve::AssembledRuntime) -> Value {
    let swept = rt.reap(None, None).await.expect("a sweep");
    let mut out = json!({
        "reaped": swept.reaped.reaped.iter().map(|r| json!({ "id": r.id, "reason": lower(&r.reason) })).collect::<Vec<_>>(),
        "spared": swept.reaped.spared.iter().map(|s| json!({ "id": s.id, "whose": lower(&s.whose) })).collect::<Vec<_>>(),
    });
    if let Some(failed) = &swept.reaped.failed {
        out["failed"] = json!(failed.iter().map(|f| json!({ "id": f.id, "message": f.message })).collect::<Vec<_>>());
    }
    if let Some(adopted) = &swept.adopted {
        out["adopted"] = json!(adopted.iter().map(j).collect::<Vec<_>>());
    }
    out
}

async fn names(rt: &wsp_serve::AssembledRuntime) -> Vec<Value> {
    rt.workspaces().list(None).await.expect("a list").iter().map(j).collect()
}

fn ids(list: &[Value]) -> Vec<Value> {
    list.iter().map(|w| w["id"].clone()).collect()
}

fn id_reads(backend: &Arc<StubBackend>) -> Rc<RefCell<Vec<String>>> {
    let read: Rc<RefCell<Vec<String>>> = Rc::default();
    let (kept, real) = (read.clone(), backend.get_fn());
    backend.set_get(Rc::new(move |id| {
        kept.borrow_mut().push(id.clone());
        real(id)
    }));
    read
}

// "the sweep records a machine of this setup that no record claims" > "stamps the record's id, name and golden on the fork, so a lost record can be rebuilt from the machine"
#[test]
fn stamps_the_records_id_name_and_golden_on_the_fork_so_a_lost_record_can_be_rebuilt_from_the_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let labels = j(&backend.machine(0).labels_now().expect("labels"));
        assert_matches(&labels, json!({ "wsp": "1", "wsp-workspace": ws, "wsp-name": "first", "wsp-golden": "snap_g" }));
        assert!(labels["wsp-owner"].as_str().is_some_and(|o| o.starts_with("h_")));
    });
}

// "..." > "a running machine with the owner label and no record becomes a running record under its own id and name, is listed, says so once, and is not killed"
#[test]
fn a_running_machine_with_the_owner_label_and_no_record_becomes_a_running_record_under_its_own_id_and_name_is_listed_says_so_once_and_is_not_killed(
) {
    drive(|| async {
        let l = lost_record(false).await;
        let result = reap(&l.fx.rt).await;
        assert_eq!(
            result,
            json!({ "reaped": [], "spared": [], "adopted": [{ "id": l.machine.id(), "workspaceId": l.ws, "name": "first", "phase": "running" }] })
        );
        assert!(!l.machine.killed.get());
        assert_matches(
            &json!(names(&l.fx.rt).await),
            json!([{ "id": l.ws, "name": "first", "machineId": l.machine.id(), "phase": "running", "golden": "snap_g" }]),
        );
        assert_matches(
            &l.state.store.get("workspaces", &l.ws).await.expect("a read").expect("a record"),
            json!({ "id": l.ws, "name": "first", "machineId": l.machine.id(), "phase": "running", "firstLife": false }),
        );
        assert_matches(&json!(l.events.of("workspace.created")), json!([{ "workspace": { "id": l.ws, "name": "first" } }]));
        assert_matches(&json!(l.events.statuses()), json!([{ "id": l.ws, "phase": "running", "reason": RECORD_RESTORED }]));

        // The second sweep finds it claimed; the delete road then works as for any workspace.
        assert_eq!(reap(&l.fx.rt).await, json!({ "reaped": [], "spared": [] }));
        l.fx.rt.workspaces().delete(&l.ws, None).await.expect("a delete");
        assert!(l.machine.killed.get());
        assert!(names(&l.fx.rt).await.is_empty());
    });
}

// "..." > "a paused one hydrates napping with the same words"
#[test]
fn a_paused_one_hydrates_napping_with_the_same_words() {
    drive(|| async {
        let l = lost_record(true).await;
        let result = reap(&l.fx.rt).await;
        assert_eq!(result["adopted"], json!([{ "id": l.machine.id(), "workspaceId": l.ws, "name": "first", "phase": "napping" }]));
        assert_matches(&json!(names(&l.fx.rt).await), json!([{ "id": l.ws, "phase": "napping" }]));
        assert_matches(
            &json!(l.events.statuses()),
            json!([{ "id": l.ws, "phase": "napping", "machineState": "paused", "reason": RECORD_RESTORED }]),
        );
    });
}

// "..." > "a machine forked before the stamps is reported, not recorded: this host can name no project for it"
#[test]
fn a_machine_forked_before_the_stamps_is_reported_not_recorded_this_host_can_name_no_project_for_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        state
            .store
            .put(
                "goldens",
                &copy_key("default", "default"),
                json!({ "head": 1, "versions": [{ "version": 1, "snapshotId": "snap_head", "baseTemplate": "base", "setupSha": "x", "createdAt": ago(3_600_000), "smoke": { "cmd": "true", "exitCode": 0 } }] }),
            )
            .await
            .expect("a put");
        let fx = runtime(&backend).store(&state.store).build();
        fx.rt.workspaces().list(None).await.expect("a list");
        let owner = owner_of(&state.store).await;
        let old = backend
            .make(spec(json!({ "kind": "sandbox", "labels": { "wsp": "1", "wsp-host": "1", "wsp-owner": owner, "createdAt": ago(2 * 60_000) } })), None, None)
            .await
            .expect("a machine");
        let result = reap(&fx.rt).await;
        // A workspace is one project's copy, so a machine this host can name no project for is reported and left
        // running rather than recorded: nothing here knows what work it holds.
        assert!(result.get("adopted").is_none());
        assert_eq!(result["failed"].as_array().map(Vec::len), Some(1));
        assert_eq!(result["failed"][0]["id"], old.id());
        assert!(result["failed"][0]["message"].as_str().unwrap_or_default().contains("this host holds no project for it"), "{result}");
        assert!(names(&fx.rt).await.is_empty());
    });
}

// "..." > "reports a machine it cannot name off the listing alone, sweep after sweep, since each read of a machine resets the provider's idle timer"
#[test]
fn reports_a_machine_it_cannot_name_off_the_listing_alone_sweep_after_sweep_since_each_read_of_a_machine_resets_the_providers_idle_timer() {
    drive(|| async {
        let l = lost_record(false).await;
        l.state.store.delete("workspace-names", &l.ws).await.expect("a delete");
        let owner = owner_of(&l.state.store).await;
        let unstamped = l
            .backend
            .make(spec(json!({ "kind": "sandbox", "labels": { "wsp": "1", "wsp-host": "1", "wsp-owner": owner, "createdAt": ago(2 * 60_000) } })), None, None)
            .await
            .expect("a machine");
        let read = id_reads(&l.backend);
        for _ in 0..2 {
            let mut failed: Vec<String> = reap(&l.fx.rt).await["failed"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|f| f["id"].as_str().unwrap_or_default().to_string())
                .collect();
            failed.sort();
            let mut want = vec![l.machine.id().to_string(), unstamped.id().to_string()];
            want.sort();
            assert_eq!(failed, want);
        }
        assert!(read.borrow().is_empty());
        assert_eq!(l.backend.machines().iter().map(|m| m.killed.get()).collect::<Vec<_>>(), [false, false]);
        l.fx.rt.close().await.expect("a close");
    });
}

// "..." > "leaves a machine inside its create minute for the next sweep, and one the provider no longer knows to the listing's lag"
#[test]
fn leaves_a_machine_inside_its_create_minute_for_the_next_sweep_and_one_the_provider_no_longer_knows_to_the_listings_lag() {
    drive(|| async {
        let l = lost_record(false).await;
        set_created(&l.machine, 30_000);
        let young = reap(&l.fx.rt).await;
        assert!(young.get("adopted").is_none());
        assert_matches(&young["spared"], json!([{ "id": l.machine.id(), "whose": "own" }]));
        assert!(names(&l.fx.rt).await.is_empty());

        set_created(&l.machine, 2 * 60_000);
        let (real, gone) = (l.backend.get_fn(), l.machine.id().to_string());
        l.backend.set_get(Rc::new(move |id| if id == gone { Box::pin(async { Err(missing("gone")) }) } else { real(id) }));
        assert_eq!(reap(&l.fx.rt).await, json!({ "reaped": [], "spared": [] }));
        assert!(!ids(&names(&l.fx.rt).await).contains(&json!(l.ws)));
    });
}

// "..." > "a body a rebuild or a wake replaced and failed to stop is not recorded twice: its record lives on the new machine, so the engine kills it"
#[test]
fn a_body_a_rebuild_or_a_wake_replaced_and_failed_to_stop_is_not_recorded_twice_its_record_lives_on_the_new_machine_so_the_engine_kills_it()
{
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let owner = owner_of(&state.store).await;
        let body = backend
            .make(
                spec(json!({ "kind": "sandbox", "labels": { "wsp": "1", "wsp-owner": owner, "wsp-workspace": ws, "wsp-name": "first", "wsp-golden": "snap_g", "createdAt": ago(2 * 60_000) } })),
                None,
                None,
            )
            .await
            .expect("a machine");
        let result = reap(&fx.rt).await;
        assert!(result.get("adopted").is_none());
        let held: Vec<Value> = names(&fx.rt).await.iter().map(|w| json!([w["id"], w["machineId"]])).collect();
        assert_eq!(held, [json!([ws, "m1"])]);
        assert_matches(&result["reaped"], json!([{ "id": body.id(), "reason": "own" }]));
        assert_eq!(backend.machines().iter().map(|m| m.killed.get()).collect::<Vec<_>>(), [false, true]);
    });
}

// "..." > "a name a live workspace already holds is not taken twice: the recovered record is named by its machine"
#[test]
fn a_name_a_live_workspace_already_holds_is_not_taken_twice_the_recovered_record_is_named_by_its_machine() {
    drive(|| async {
        let l = lost_record(false).await;
        let again = create_on(&l.fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        set_created(&l.backend.machine(1), 2 * 60_000);
        let result = reap(&l.fx.rt).await;
        assert_eq!(result["adopted"], json!([{ "id": l.machine.id(), "workspaceId": l.ws, "name": l.machine.id(), "phase": "running" }]));
        let mut held: Vec<Value> = names(&l.fx.rt).await.iter().map(|w| json!([w["id"], w["name"]])).collect();
        let mut want = vec![json!([again, "first"]), json!([l.ws, l.machine.id()])];
        held.sort_by_key(Value::to_string);
        want.sort_by_key(Value::to_string);
        assert_eq!(held, want);
    });
}

// "..." > "a confirming read the provider refuses leaves the machine for the next sweep: not recorded, not killed, its own words in failed"
#[test]
fn a_confirming_read_the_provider_refuses_leaves_the_machine_for_the_next_sweep_not_recorded_not_killed_its_own_words_in_failed() {
    drive(|| async {
        let l = lost_record(false).await;
        let (real, refused, target) = (l.backend.get_fn(), Rc::new(Cell::new(false)), l.machine.id().to_string());
        l.backend.set_get(Rc::new(move |id| {
            if id == target && !refused.get() {
                refused.set(true);
                return Box::pin(async {
                    Err(EngineError::Refused { message: "Bad Gateway".into(), kind: None, status: Some(502), code: None })
                });
            }
            real(id)
        }));
        let first = reap(&l.fx.rt).await;
        assert!(first.get("adopted").is_none());
        assert_eq!(first["reaped"], json!([]));
        assert_eq!(first["failed"], json!([{ "id": l.machine.id(), "message": "not recorded: Bad Gateway; retried next sweep" }]));
        assert!(!l.machine.killed.get());
        assert!(names(&l.fx.rt).await.is_empty());

        let second = reap(&l.fx.rt).await;
        assert_eq!(second["adopted"], json!([{ "id": l.machine.id(), "workspaceId": l.ws, "name": "first", "phase": "running" }]));
        assert!(!l.machine.killed.get());
    });
}

// "..." > "a machine that comes back after its workspace was deleted is killed by the next start's sweep, not reported and left billing"
#[test]
fn a_machine_that_comes_back_after_its_workspace_was_deleted_is_killed_by_the_next_starts_sweep_not_reported_and_left_billing() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let first = runtime(&backend).store(&state.store).with(|o| o.kill_confirm = Some(confirm())).build();
        let ws = create_on(&first.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let machine = backend.machine(0);
        first.rt.workspaces().delete(&ws, None).await.expect("a delete");
        first.rt.close().await.expect("a close");
        assert!(machine.killed.get());

        // Measured 2026-09-26: a Solari sandbox read gone after its delete listed running again an hour later.
        machine.killed.set(false);
        set_created(&machine, 60 * 60_000);
        let rt = runtime(&backend).store(&state.store).with(|o| o.kill_confirm = Some(confirm())).build();
        let result = reap(&rt.rt).await;
        assert!(result.get("failed").is_none());
        assert!(result.get("adopted").is_none());
        assert_matches(&result["reaped"], json!([{ "id": machine.id(), "reason": "own" }]));
        assert!(machine.killed.get());
        assert!(names(&rt.rt).await.is_empty());
        rt.rt.close().await.expect("a close");
    });
}

// "..." > "a deleted workspace's machine that comes back paused is reported, not left unnamed"
#[test]
fn a_deleted_workspaces_machine_that_comes_back_paused_is_reported_not_left_unnamed() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let first = runtime(&backend).store(&state.store).with(|o| o.kill_confirm = Some(confirm())).build();
        let ws = create_on(&first.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let machine = backend.machine(0);
        first.rt.workspaces().delete(&ws, None).await.expect("a delete");
        first.rt.close().await.expect("a close");
        machine.killed.set(false);
        machine.paused.set(true);
        set_created(&machine, 60 * 60_000);
        let rt = runtime(&backend).store(&state.store).build();
        let result = reap(&rt.rt).await;
        assert_eq!(result["failed"].as_array().map(Vec::len), Some(1));
        assert_eq!(result["failed"][0]["id"], machine.id());
        assert!(result["failed"][0]["message"].as_str().unwrap_or_default().contains("not recorded"), "{result}");
        assert!(!machine.killed.get());
        rt.rt.close().await.expect("a close");
    });
}

// "..." > "forgets a deleted machine a week on by the runtime's clock, so the watch list does not grow with every workspace ever deleted"
#[test]
fn forgets_a_deleted_machine_a_week_on_by_the_runtimes_clock_so_the_watch_list_does_not_grow_with_every_workspace_ever_deleted() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let first = runtime(&backend).store(&state.store).clock(&FakeClock::new(T0)).with(|o| o.kill_confirm = Some(confirm())).build();
        let ws = create_on(&first.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        first.rt.workspaces().delete(&ws, None).await.expect("a delete");
        first.rt.close().await.expect("a close");
        let machine_id = backend.machine(0).id().to_string();

        let inside = runtime(&backend).store(&state.store).clock(&FakeClock::new(T0 + DROPPED_WATCH_MS as i64 - 1)).build();
        reap(&inside.rt).await;
        inside.rt.close().await.expect("a close");
        assert_eq!(state.store.keys("dropped-machines").await.expect("keys"), [machine_id]);

        let past = runtime(&backend).store(&state.store).clock(&FakeClock::new(T0 + DROPPED_WATCH_MS as i64)).build();
        reap(&past.rt).await;
        past.rt.close().await.expect("a close");
        assert!(state.store.keys("dropped-machines").await.expect("keys").is_empty());
    });
}

// "..." > "builders, smoke forks, another setup's machines and reserved experiments are never recorded as workspaces"
#[test]
fn builders_smoke_forks_another_setups_machines_and_reserved_experiments_are_never_recorded_as_workspaces() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        fx.rt.workspaces().list(None).await.expect("a list");
        let owner = owner_of(&state.store).await;
        let at = ago(2 * 60_000);
        for labels in [
            json!({ "wsp": "1", "wsp-builder": "1", "wsp-owner": owner, "createdAt": at }),
            json!({ "wsp": "1", "wsp-smoke": "1", "wsp-owner": owner, "createdAt": at }),
            json!({ "wsp": "1", "wsp-owner": "h_other", "createdAt": at }),
            json!({ "wsp": "1", "wsp-owner": owner, "poc": "ttl-test", "createdAt": at }),
        ] {
            backend.make(spec(json!({ "kind": "sandbox", "labels": labels })), None, None).await.expect("a machine");
        }
        let result = reap(&fx.rt).await;
        assert!(result.get("adopted").is_none());
        assert!(names(&fx.rt).await.is_empty());
        assert_eq!(result["reaped"].as_array().expect("reaped").iter().map(|r| r["id"].clone()).collect::<Vec<_>>(), ["m1", "m2"]);
    });
}

// "a name names one workspace" > "a fork of a name whose workspace is being deleted is refused in words, mints no machine, and lands once the delete is done"
#[test]
fn a_fork_of_a_name_whose_workspace_is_being_deleted_is_refused_in_words_mints_no_machine_and_lands_once_the_delete_is_done() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let machine = backend.machine(0);
        let (reached, release) = (Rc::new(Cell::new(false)), Gate::new());
        let (flag, wait, m) = (reached.clone(), release.clone(), machine.clone());
        machine.set_kill(Rc::new(move || {
            flag.set(true);
            let (wait, m) = (wait.clone(), m.clone());
            Box::pin(async move {
                wait.wait().await;
                m.killed.set(true);
                Ok(())
            })
        }));
        let (rt, id) = (fx.rt.clone(), ws.clone());
        let deleting = tokio::task::spawn_local(async move { rt.workspaces().delete(&id, None).await });
        // The delete holds the name once it has reached the machine; a fork asked for before that meets the plain refusal.
        let flag = reached.clone();
        wait!([flag] flag.get());
        let refused = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect_err("a refusal");
        refused_with(&refused, &name_deleting_refusal("first"));
        assert_eq!(backend.machines().len(), 1);
        assert_eq!(ids(&names(&fx.rt).await), [json!(ws)]);

        release.open();
        deleting.await.expect("the delete").expect("a delete");
        let again = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        assert_ne!(again, ws);
        assert_eq!(ids(&names(&fx.rt).await), [json!(again)]);
        assert_eq!(backend.machines().iter().map(|m| m.killed.get()).collect::<Vec<_>>(), [true, false]);
    });
}

// "a name names one workspace" > "a second delete of the same workspace joins the first: one kill, one drop"
#[test]
fn a_second_delete_of_the_same_workspace_joins_the_first_one_kill_one_drop() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let (kills, m) = (Rc::new(Cell::new(0)), backend.machine(0));
        let (counted, machine) = (kills.clone(), m.clone());
        m.set_kill(Rc::new(move || {
            counted.set(counted.get() + 1);
            let machine = machine.clone();
            // The TS kill is an async function: the first delete is still in flight when the second asks.
            Box::pin(async move {
                tokio::task::yield_now().await;
                machine.killed.set(true);
                Ok(())
            })
        }));
        let (a, b) = futures::join!(fx.rt.workspaces().delete(&ws, None), fx.rt.workspaces().delete(&ws, None));
        a.expect("a delete");
        b.expect("a delete");
        assert_eq!(kills.get(), 1, "{} workspace.deleted events", events.of("workspace.deleted").len());
        assert_eq!(events.of("workspace.deleted").len(), 1);
    });
}

// "a name names one workspace" > "two forks of one name asked for together: one lands, the other is refused in words, one machine"
#[test]
fn two_forks_of_one_name_asked_for_together_one_lands_the_other_is_refused_in_words_one_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let (a, b) = futures::join!(create_on(&fx.rt, CreateOn::golden("first"), None), create_on(&fx.rt, CreateOn::golden("first"), None));
        assert_eq!([a.is_ok(), b.is_ok()].iter().filter(|ok| **ok).count(), 1);
        let refused = a.err().or(b.err()).expect("a refusal");
        refused_with(&refused, &name_taken_refusal("first"));
        assert_eq!(backend.machines().len(), 1);
        assert_eq!(names(&fx.rt).await.iter().map(|w| w["name"].clone()).collect::<Vec<_>>(), ["first"]);
    });
}

// "a name names one workspace" > "a fork of a name another workspace holds is refused in words and mints no machine"
#[test]
fn a_fork_of_a_name_another_workspace_holds_is_refused_in_words_and_mints_no_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        refused_with(&create_on(&fx.rt, CreateOn::golden("a"), None).await.expect_err("a refusal"), &name_taken_refusal("a"));
        assert_eq!(backend.machines().len(), 1);
        assert_eq!(j(&create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace").view)["name"], "b");
    });
}

fn theme() -> Value {
    json!({ "dots": [{ "angle": 200, "radius": 0.5 }, { "angle": 20, "radius": 0.5 }], "harmony": "complementary", "grain": 0.25, "opacity": 0.6, "mode": "auto" })
}

fn look(doc: Value) -> wsp_wire::types::workspace_look::WorkspaceLook {
    serde_json::from_value(doc).expect("a look")
}

// "a workspace's look" > "holds the theme and the glyph beside the name, one fact at a time, and a fresh runtime reads them back"
#[test]
fn holds_the_theme_and_the_glyph_beside_the_name_one_fact_at_a_time_and_a_fresh_runtime_reads_them_back() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let events = fx.events();
        let a = j(&fx.rt.workspaces().look(&ws, look(json!({ "theme": theme() })), None).await.expect("a look"));
        assert_matches(&a, json!({ "id": ws, "theme": theme() }));
        // The glyph alone: the theme it was not sent stays as it is.
        let b = j(&fx.rt.workspaces().look(&ws, look(json!({ "glyph": "flask" })), None).await.expect("a look"));
        assert_matches(&b, json!({ "theme": theme(), "glyph": "flask" }));
        assert_matches(
            &state.store.get("workspaces", &ws).await.expect("a read").expect("a record"),
            json!({ "theme": theme(), "glyph": "flask" }),
        );
        // Its own event, both facts whole, and never workspace.created: the machine was not touched.
        assert_matches(
            &json!(events.of("workspace.look")),
            json!([{ "workspaceId": ws, "theme": theme(), "glyph": null }, { "workspaceId": ws, "theme": theme(), "glyph": "flask" }]),
        );
        assert!(events.of("workspace.created").is_empty());
        assert!(!backend.machine(0).killed.get());

        fx.rt.close().await.expect("a close");
        let next = runtime(&backend).store(&state.store).build();
        assert_matches(
            &j(&next.rt.workspaces().get(&ws, None, None).await.expect("a workspace")),
            json!({ "theme": theme(), "glyph": "flask" }),
        );
        next.rt.close().await.expect("a close");
    });
}

// "a workspace's look" > "null clears one fact back to none and leaves the other, and the record loses the key rather than holding an empty one"
#[test]
fn null_clears_one_fact_back_to_none_and_leaves_the_other_and_the_record_loses_the_key_rather_than_holding_an_empty_one() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        fx.rt.workspaces().look(&ws, look(json!({ "theme": theme(), "glyph": "rocket" })), None).await.expect("a look");

        assert_matches(
            &j(&fx.rt.workspaces().look(&ws, look(json!({ "theme": null })), None).await.expect("a look")),
            json!({ "glyph": "rocket" }),
        );
        assert!(j(&fx.rt.workspaces().get(&ws, None, None).await.expect("a workspace")).get("theme").is_none());
        assert!(state.store.get("workspaces", &ws).await.expect("a read").expect("a record").get("theme").is_none());
        assert!(j(&fx.rt.workspaces().look(&ws, look(json!({ "glyph": null })), None).await.expect("a look")).get("glyph").is_none());
        // A default workspace has neither, so nothing is themed until a person picks.
        assert!(j(&create_on(&fx.rt, CreateOn::golden("second"), None).await.expect("a workspace").view).get("theme").is_none());
    });
}

// "a rename of a workspace" > "names the record, keeps its id and machine, says so once, and the next listing carries it"
#[test]
fn names_the_record_keeps_its_id_and_machine_says_so_once_and_the_next_listing_carries_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace");
        let events = fx.events();
        let renamed = j(&fx.rt.workspaces().rename(&ws.view.id, "the name he typed", None).await.expect("a rename"));
        assert_matches(&renamed, json!({ "id": ws.view.id, "name": "the name he typed", "machineId": j(&ws.view)["machineId"] }));
        assert_eq!(names(&fx.rt).await.iter().map(|w| w["name"].clone()).collect::<Vec<_>>(), ["the name he typed"]);
        assert_matches(
            &state.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record"),
            json!({ "id": ws.view.id, "name": "the name he typed" }),
        );
        // Its own event: the record changed, so nothing that meters the machine reads it as one coming up.
        assert_matches(&json!(events.of("workspace.renamed")), json!([{ "workspaceId": ws.view.id, "name": "the name he typed" }]));
        assert!(events.of("workspace.created").is_empty());
        // The machine is untouched: the provider takes metadata at create and offers no update, so the name it was
        // forked under stands there until the next fork stamps the record's.
        assert_eq!(backend.machines().len(), 1);
        assert_matches(&j(&backend.machine(0).labels_now().expect("labels")), json!({ "wsp-name": "first" }));
        assert!(!backend.machine(0).killed.get());
    });
}

// "a rename of a workspace" > "takes the name the fork's own rule takes: one another workspace holds, one a fork is landing under, and a blank one are refused and nothing is renamed"
#[test]
fn takes_the_name_the_forks_own_rule_takes_one_another_workspace_holds_one_a_fork_is_landing_under_and_a_blank_one_are_refused_and_nothing_is_renamed(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let a = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace");

        refused_with(&fx.rt.workspaces().rename(&a, "b", None).await.expect_err("a refusal"), &name_taken_refusal("b"));
        refused_with(&fx.rt.workspaces().rename(&a, "   ", None).await.expect_err("a refusal"), BLANK_NAME_REFUSAL);
        assert_matches(&j(&fx.rt.workspaces().get(&a, None, None).await.expect("a workspace")), json!({ "name": "a" }));
        // Its own name is no duplicate: the record comes back untouched.
        assert_matches(&j(&fx.rt.workspaces().rename(&a, "a", None).await.expect("a rename")), json!({ "id": a, "name": "a" }));
        // A fork of the name is landing this moment, so the rename waits for it as a second fork would.
        let project = project_on(&fx.rt, None, None, None).await.expect("a project").id;
        let (rt, p) = (fx.rt.clone(), project);
        let landing = tokio::task::spawn_local(async move {
            rt.workspaces()
                .create(CreateWorkspaceOptions { project: p, golden: Some("snap_g".into()), name: "c".into(), ..Default::default() }, None)
                .await
                .map(|_| ())
        });
        // The create holds the name once it has resolved its project; the rename below is the second asker.
        tokio::task::yield_now().await;
        refused_with(&fx.rt.workspaces().rename(&a, "c", None).await.expect_err("a refusal"), &name_taken_refusal("c"));
        landing.await.expect("the create").expect("a create");
        // A fork of the name it gave up lands after it.
        fx.rt.workspaces().rename(&a, "d", None).await.expect("a rename");
        assert_eq!(j(&create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view)["name"], "a");
    });
}

// "a rename of a workspace" > "a blank name is refused a fork too, so the rule the rename takes is the fork's own"
#[test]
fn a_blank_name_is_refused_a_fork_too_so_the_rule_the_rename_takes_is_the_forks_own() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        refused_with(&create_on(&fx.rt, CreateOn::golden(" "), None).await.expect_err("a refusal"), BLANK_NAME_REFUSAL);
        assert!(backend.machines().is_empty());
    });
}

// "a rename of a workspace" > "the machine takes the new name at its next fork, so a rebuilt machine carries it"
#[test]
fn the_machine_takes_the_new_name_at_its_next_fork_so_a_rebuilt_machine_carries_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        fx.rt.workspaces().rename(&ws, "the name he typed", None).await.expect("a rename");
        fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        let last = backend.machines().pop().expect("a machine");
        assert_matches(&j(&last.labels_now().expect("labels")), json!({ "wsp-name": "the name he typed", "wsp-workspace": ws }));
    });
}

// "a rename of a workspace" > "drops the space around the name, as a fork's own name rule does, so nothing has to be typed back for --in"
#[test]
fn drops_the_space_around_the_name_as_a_forks_own_name_rule_does_so_nothing_has_to_be_typed_back_for_in() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("  alpha  "), None).await.expect("a workspace").view;
        assert_eq!(j(&ws)["name"], "alpha");
        assert_matches(&j(&backend.machine(0).labels_now().expect("labels")), json!({ "wsp-name": "alpha" }));
        assert_matches(
            &j(&fx.rt.workspaces().rename(&ws.id, "  the name he typed  ", None).await.expect("a rename")),
            json!({ "name": "the name he typed" }),
        );
        assert_eq!(names(&fx.rt).await.iter().map(|w| w["name"].clone()).collect::<Vec<_>>(), ["the name he typed"]);
        // The trimmed name is the one it holds, so renaming to it again is the no-op, not a duplicate.
        assert_matches(
            &j(&fx.rt.workspaces().rename(&ws.id, "the name he typed", None).await.expect("a rename")),
            json!({ "name": "the name he typed" }),
        );
    });
}

// "a rename of a workspace" > "a sweep that records this machine after the store lost its workspace document restores the name a person gave, not the fork's"
#[test]
fn a_sweep_that_records_this_machine_after_the_store_lost_its_workspace_document_restores_the_name_a_person_gave_not_the_forks() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let first = runtime(&backend).store(&state.store).build();
        let ws = create_on(&first.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        first.rt.workspaces().rename(&ws, "the name he typed", None).await.expect("a rename");
        first.rt.close().await.expect("a close");
        // The machine still wears the name it was forked under: the provider takes metadata at create and offers no update.
        assert_matches(&j(&backend.machine(0).labels_now().expect("labels")), json!({ "wsp-name": "first" }));
        set_created(&backend.machine(0), 2 * 60_000);
        state.store.delete("workspaces", &ws).await.expect("a delete");

        let rt = runtime(&backend).store(&state.store).build();
        assert_matches(&reap(&rt.rt).await, json!({ "adopted": [{ "workspaceId": ws, "name": "the name he typed" }] }));
        assert_eq!(names(&rt.rt).await.iter().map(|w| w["name"].clone()).collect::<Vec<_>>(), ["the name he typed"]);
        assert_matches(
            &state.store.get("workspaces", &ws).await.expect("a read").expect("a record"),
            json!({ "name": "the name he typed" }),
        );
        rt.rt.close().await.expect("a close");
    });
}

// "a rename of a workspace" > "the name is kept by workspace id, so a rebuild leaves no second copy behind and a delete takes it with the record"
#[test]
fn the_name_is_kept_by_workspace_id_so_a_rebuild_leaves_no_second_copy_behind_and_a_delete_takes_it_with_the_record() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        fx.rt.workspaces().rename(&ws, "the name he typed", None).await.expect("a rename");
        assert_matches(
            &state.store.get("workspace-names", &ws).await.expect("a read").expect("a row"),
            json!({ "workspaceId": ws, "name": "the name he typed" }),
        );
        // The machine is replaced but the workspace is not: one row, still under the id its machines carry as a label.
        fx.rt.workspaces().rebuild(&ws, None).await.expect("a rebuild");
        assert_eq!(state.store.list("workspace-names").await.expect("a list").len(), 1);
        fx.rt.workspaces().delete(&ws, None).await.expect("a delete");
        assert_eq!(state.store.get("workspace-names", &ws).await.expect("a read"), None);
        fx.rt.close().await.expect("a close");
    });
}

// "a rename of a workspace" > "a workspace the runtime does not know is refused, and nothing is recorded"
#[test]
fn a_workspace_the_runtime_does_not_know_is_refused_and_nothing_is_recorded() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).build();
        assert!(fx.rt.workspaces().rename("ws_nope", "a", None).await.is_err());
    });
}

fn enospc() -> &'static str {
    "ENOSPC: no space left on device, write"
}

// "a create that fails after its fork" > "kills the machine and forgets the record when the provider parts with it"
#[test]
fn kills_the_machine_and_forgets_the_record_when_the_provider_parts_with_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let refusing = WrappedStore::over(&state.store);
        refusing.refuse_next_put("workspaces", enospc());
        let fx = runtime(&backend).store(&refusing.store()).build();
        let failed = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect_err("a failed create");
        assert!(failed.message.contains("ENOSPC"), "{}", failed.message);
        assert!(backend.machine(0).killed.get());
        assert!(names(&fx.rt).await.is_empty());
    });
}

// "a create that fails after its fork" > "marks the machine as a delete does, so one that comes back is killed by the next sweep, not reported and left billing"
#[test]
fn marks_the_machine_as_a_delete_does_so_one_that_comes_back_is_killed_by_the_next_sweep_not_reported_and_left_billing() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let refusing = WrappedStore::over(&state.store);
        refusing.refuse_next_put("workspaces", enospc());
        let first = runtime(&backend).store(&refusing.store()).with(|o| o.kill_confirm = Some(confirm())).build();
        let failed = create_on(&first.rt, CreateOn::golden("first"), None).await.expect_err("a failed create");
        assert!(failed.message.contains("ENOSPC"), "{}", failed.message);
        first.rt.close().await.expect("a close");
        let machine = backend.machine(0);
        machine.killed.set(false);
        set_created(&machine, 60 * 60_000);
        let rt = runtime(&backend).store(&state.store).with(|o| o.kill_confirm = Some(confirm())).build();
        let result = reap(&rt.rt).await;
        assert!(result.get("failed").is_none());
        assert!(result.get("adopted").is_none());
        assert_matches(&result["reaped"], json!([{ "id": machine.id(), "reason": "own" }]));
        assert!(machine.killed.get());
        rt.rt.close().await.expect("a close");
    });
}

// "a create that fails after its fork" > "a mark the store will not take is said in its own words, and the machine that went goes with its record"
#[test]
fn a_mark_the_store_will_not_take_is_said_in_its_own_words_and_the_machine_that_went_goes_with_its_record() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let refusing = WrappedStore::over(&state.store);
        refusing.refuse_next_put("workspaces", enospc());
        refusing.refuse_puts("dropped-machines", "EIO: i/o error, write");
        let warned = capture();
        let fx = runtime(&backend).store(&refusing.store()).with(|o| o.kill_confirm = Some(confirm())).build();
        let failed = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect_err("a failed create");
        assert!(failed.message.contains("ENOSPC"), "{}", failed.message);
        assert!(backend.machine(0).killed.get());
        assert!(names(&fx.rt).await.is_empty());
        let lines = warned.lines();
        assert!(lines.iter().filter(|l| l.contains("would not stop")).count() == 0);
        assert_eq!(lines.iter().filter(|l| l.contains("m1") && l.contains("EIO: i/o error")).count(), 1, "{lines:?}");
    });
}

// "a create that fails after its fork" > "keeps the record, listed and deletable, when the machine will not die"
#[test]
fn keeps_the_record_listed_and_deletable_when_the_machine_will_not_die() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let refusing = WrappedStore::over(&state.store);
        refusing.refuse_next_put("workspaces", enospc());
        let fx = runtime(&backend).store(&refusing.store()).build();
        let events = fx.events();
        let real = backend.create_fn();
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            let real = real.clone();
            Box::pin(async move {
                let m = real(spec, waiting, sent).await?;
                m.set_kill(Rc::new(|| Box::pin(async { Err(EngineError::Other { message: "Bad Gateway".into(), code: None }) })));
                Ok(m)
            })
        }));
        let failed = create_on(&fx.rt, CreateOn::golden("first"), None).await.expect_err("a failed create");
        assert!(failed.message.contains("ENOSPC"), "{}", failed.message);

        let listed = names(&fx.rt).await;
        assert_matches(&json!(listed), json!([{ "name": "first", "machineId": "m1", "phase": "running" }]));
        let id = listed[0]["id"].as_str().expect("an id").to_string();
        assert_matches(
            &state.store.get("workspaces", &id).await.expect("a read").expect("a record"),
            json!({ "name": "first", "machineId": "m1" }),
        );
        assert_matches(&json!(events.of("workspace.created")), json!([{ "workspace": { "name": "first" } }]));
        assert!(!backend.machine(0).killed.get());

        let m = backend.machine(0);
        let machine = m.clone();
        m.set_kill(Rc::new(move || {
            machine.killed.set(true);
            Box::pin(async { Ok(()) })
        }));
        fx.rt.workspaces().delete(&id, None).await.expect("a delete");
        assert!(backend.machine(0).killed.get());
        assert!(names(&fx.rt).await.is_empty());
    });
}
