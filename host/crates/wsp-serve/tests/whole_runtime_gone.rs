// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-gone.test.ts at 65f6b89c7, case for case.
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::console::capture;
use wsp_core::status::StatusWatchOptions;
use wsp_core::types::api::StartOptions;
use wsp_engine::errors::EngineError;
use wsp_store::store::Store;

/// `^machine <id> is gone at the provider: the record load found it gone at \S+Z \(404 gone\)$`
fn load_saw(text: &str, id: &str) -> bool {
    text.strip_prefix(&format!("machine {id} is gone at the provider: the record load found it gone at "))
        .and_then(|rest| rest.strip_suffix(" (404 gone)"))
        .is_some_and(|at| !at.is_empty() && !at.contains(char::is_whitespace) && at.ends_with('Z'))
}

/// `^machine m1 is gone at the provider: the status poll found it gone at \S+Z$`
fn poll_saw(text: &str) -> bool {
    text.strip_prefix("machine m1 is gone at the provider: the status poll found it gone at ")
        .is_some_and(|at| !at.is_empty() && !at.contains(char::is_whitespace) && at.ends_with('Z'))
}

struct Hydrated {
    backend: Arc<StubBackend>,
    store: Rc<dyn Store>,
    fx: Fx,
    a: String,
    b: String,
    _state: State,
}

/// Two workspaces, one napping, both machines deleted at the provider while no host ran; the same store hydrated again.
async fn hydrated_gone() -> Hydrated {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let rt1 = runtime(&backend).store(&state.store).build();
    let a = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let b = create_on(&rt1.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
    rt1.rt.workspaces().nap(&b, None).await.expect("a nap");
    rt1.rt.close().await.expect("a close");
    for m in backend.machines() {
        m.killed.set(true);
    }
    let fx = runtime(&backend).store(&state.store).build();
    // The load holds each record on its one 404 and confirms it once the host serves.
    until(|| {
        let rt = fx.rt.clone();
        async move { rt.workspaces().list(None).await.expect("a list").iter().all(|w| j(w)["phase"] == "gone") }
    })
    .await;
    Hydrated { backend, store: state.store.clone(), fx, a, b, _state: state }
}

// "gone machines" > "a stored workspace whose machine the provider no longer knows hydrates as gone with the provider's words, whatever phase it was left at"
#[test]
fn a_stored_workspace_whose_machine_the_provider_no_longer_knows_hydrates_as_gone_with_the_providers_words_whatever_phase_it_was_left_at() {
    drive(|| async {
        let h = hydrated_gone().await;
        let listed = h.fx.rt.workspaces().list(None).await.expect("a list");
        // The words name the load that met the 404 and quote the provider's answer to it.
        let rows: Vec<(String, String, String)> = listed
            .iter()
            .map(|w| {
                (
                    w.name.clone(),
                    j(w)["phase"].as_str().unwrap_or_default().to_string(),
                    j(w)["gone"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        assert_eq!(rows.iter().map(|r| (r.0.as_str(), r.1.as_str())).collect::<Vec<_>>(), [("a", "gone"), ("b", "gone")]);
        assert!(load_saw(&rows[0].2, "m1"), "{}", rows[0].2);
        assert!(load_saw(&rows[1].2, "m2"), "{}", rows[1].2);
        // Written back once confirmed: a second host over the store reads gone without asking the provider.
        let stored_a = h.store.get("workspaces", &h.a).await.expect("a read").expect("a record");
        assert_eq!(stored_a["phase"], "gone");
        assert!(load_saw(stored_a["gone"].as_str().expect("words"), "m1"));
        assert_eq!(h.store.get("workspaces", &h.b).await.expect("a read").expect("a record")["phase"], "gone");
        let statuses = status_rows(&h.fx.rt).await;
        let sa = statuses.iter().find(|s| s["id"] == h.a.as_str()).expect("a's row");
        assert_matches(sa, json!({ "phase": "gone", "machineState": "gone", "reach": { "state": "gone" } }));
        assert!(load_saw(sa["reason"].as_str().expect("a reason"), "m1"));
        assert!(sa.get("idleAt").is_none());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a gone workspace refuses wake and sends with the provider's words; nap is a no-op; rebuild is the road out"
#[test]
fn a_gone_workspace_refuses_wake_and_sends_with_the_providers_words_nap_is_a_no_op_rebuild_is_the_road_out() {
    drive(|| async {
        let h = hydrated_gone().await;
        let words =
            j(&h.fx.rt.workspaces().get(&h.a, None, None).await.expect("the workspace"))["gone"].as_str().expect("words").to_string();
        assert!(words.starts_with("machine m1 is gone at the provider: the record load found it gone at "), "{words}");
        let wake = h.fx.rt.workspaces().wake(&h.a, None).await.expect_err("a refusal");
        assert!(wake.message.contains(&format!("a's machine is gone with its disk, so work that was not pushed is lost; rebuild it to wake, which brings back its home folder from the last saved nap ({words})")), "{}", wake.message);
        let send =
            h.fx.rt
                .sessions()
                .start(&h.a, StartOptions { prompt: "hi".into(), ..Default::default() }, None)
                .await
                .err()
                .expect("a refusal");
        assert!(send.message.contains(&format!("a's machine is gone with its disk, so work that was not pushed is lost; rebuild it to send, which brings back its home folder from the last saved nap ({words})")), "{}", send.message);
        assert_matches(&j(&h.fx.rt.workspaces().nap(&h.a, None).await.expect("a nap")), json!({ "phase": "gone" }));
        assert_eq!(h.backend.made(), 2);

        let events = h.fx.events();
        let rebuilt = h.fx.rt.workspaces().rebuild(&h.a, None).await.expect("a rebuild");
        assert_matches(&j(&rebuilt), json!({ "phase": "running", "machineId": "m3" }));
        assert!(j(&rebuilt).get("gone").is_none());
        assert_eq!(h.backend.machine(2).spec_now().from_snapshot.as_deref(), Some("snap_g"));
        assert_matches(&events.of("workspace.upgraded")[0], json!({ "workspaceId": h.a, "machineId": "m3" }));
        let rows = status_rows(&h.fx.rt).await;
        let row = rows.iter().find(|s| s["id"] == h.a.as_str()).expect("a's row");
        assert_matches(row, json!({ "phase": "running", "machineState": "running", "machineId": "m3" }));
        let record = h.store.get("workspaces", &h.a).await.expect("a read").expect("a record");
        assert_matches(&record, json!({ "phase": "running", "machineId": "m3" }));
        assert!(record.get("gone").is_none());
        assert_matches(&j(&h.fx.rt.workspaces().wake(&h.a, None).await.expect("a wake")), json!({ "phase": "running" }));
        h.fx.rt.close().await.expect("a close");
    });
}

// "the reach poll finds a running machine gone: the workspace moves to gone, its turn ends, the idle window drops and the bill stops"
#[test]
fn the_reach_poll_finds_a_running_machine_gone_the_workspace_moves_to_gone_its_turn_ends_the_idle_window_drops_and_the_bill_stops() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend)
            .store(&state.store)
            .adapter("claude", never_ends())
            .with(|o| {
                o.status = Some(StatusWatchOptions {
                    poll_interval_ms: Some(5),
                    cost_interval_ms: Some(5),
                    reconcile_min_ms: Some(0),
                    ..Default::default()
                });
                o.gone_confirm_ms = Some(5);
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let port = dropping_port().await;
        backend.machine(0).set_preview_port_url(&format!("http://127.0.0.1:{}", port.port));
        fx.rt.sessions().start(&ws.view.id, StartOptions { prompt: "work".into(), ..Default::default() }, None).await.expect("a start");
        let events = fx.events();
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        // Unreachable alone is weather: the provider still says running, so the workspace does.
        until(|| {
            let events = events.clone();
            async move { events.of("workspace.status").iter().any(|e| e["status"]["reach"]["state"] == "unreachable") }
        })
        .await;
        assert_matches(&j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace")), json!({ "phase": "running" }));
        assert!(!events.has("workspace.gone"));

        backend.machine(0).killed.set(true); // deleted through the provider's API under a running host
        until(|| {
            let events = events.clone();
            async move { events.has("workspace.gone") }
        })
        .await;
        let gone = events.of("workspace.gone").remove(0);
        assert_matches(&gone, json!({ "workspaceId": ws.view.id, "machineId": "m1" }));
        assert!(poll_saw(gone["reason"].as_str().expect("a reason")), "{gone}");
        let held = j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"));
        assert_eq!(held["phase"], "gone");
        assert!(poll_saw(held["gone"].as_str().expect("words")));
        let stored = state.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record");
        assert_eq!(stored["phase"], "gone");
        assert!(poll_saw(stored["gone"].as_str().expect("words")));
        assert_matches(
            &events.of("session.end")[0],
            json!({ "workspaceId": ws.view.id, "reason": "machine gone at the provider while the agent was working" }),
        );
        let listed = fx.rt.sessions().list(Some(ws.view.id.clone()), None).await.expect("a list");
        assert_eq!(listed.iter().map(|s| j(s)["status"].clone()).collect::<Vec<_>>(), [json!("failed")]);

        // The bill stops where the machine did: rate 0 and the awake time frozen from one tick to the next.
        until(|| {
            let events = events.clone();
            async move { events.of("workspace.cost").iter().filter(|e| e["phase"] == "gone").count() >= 2 }
        })
        .await;
        let ticks: Vec<Value> = events.of("workspace.cost").into_iter().filter(|e| e["phase"] == "gone").collect();
        assert_eq!(ticks[0]["rateUsdPerHour"], 0);
        assert_eq!(ticks[1]["awakeMs"], ticks[0]["awakeMs"]);
        let last = events.statuses().last().cloned().expect("a status");
        assert_matches(&last, json!({ "phase": "gone", "machineState": "gone", "reach": { "state": "gone" } }));
        assert!(poll_saw(last["reason"].as_str().expect("a reason")));
        assert!(last.get("idleAt").is_none());
        stop();
        fx.rt.close().await.expect("a close");
    });
}

// "the host's metrics answer 404 while the state read says running: the workspace stays running and its turn works on"
#[test]
fn the_hosts_metrics_answer_404_while_the_state_read_says_running_the_workspace_stays_running_and_its_turn_works_on() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend)
            .store(&state.store)
            .adapter("claude", never_ends())
            .with(|o| {
                o.status = Some(StatusWatchOptions {
                    poll_interval_ms: Some(5),
                    cost_interval_ms: Some(5),
                    reconcile_min_ms: Some(0),
                    ..Default::default()
                });
                o.gone_confirm_ms = Some(5);
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let m = backend.machine(0);
        let port = dropping_port().await;
        m.set_preview_port_url(&format!("http://127.0.0.1:{}", port.port));
        fx.rt.sessions().start(&ws.view.id, StartOptions { prompt: "work".into(), ..Default::default() }, None).await.expect("a start");
        let events = fx.events();
        let warned = capture();
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        // The guest misses the probe and the host has lost the VM; the state read by id is the only word on gone.
        m.host_lost.set(true);
        // Several passes read the metrics 404 with the guest still missing; none of them is a verdict.
        until(|| {
            let m = m.clone();
            async move { m.calls.metrics.get() >= 3 }
        })
        .await;

        assert!(!m.killed.get());
        assert_matches(&j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace")), json!({ "phase": "running" }));
        assert!(!events.has("workspace.gone"));
        assert!(!events.has("session.end"));
        let listed = fx.rt.sessions().list(Some(ws.view.id.clone()), None).await.expect("a list");
        assert_eq!(listed.iter().map(|s| j(s)["status"].clone()).collect::<Vec<_>>(), [json!("running")]);
        let rows = events.statuses();
        assert!(rows.iter().all(|r| r["machineState"] != "gone"));
        assert_matches(
            rows.last().expect("a row"),
            json!({ "phase": "running", "machineState": "running", "reach": { "state": "unreachable" } }),
        );
        // The gap is one line per spell, however many passes read it.
        assert_eq!(warned.lines().iter().filter(|l| l.starts_with("host metrics for")).count(), 1);
        stop();
        fx.rt.close().await.expect("a close");
    });
}

/// One stored workspace whose next host's reads of its machine fail, the first landing 2 s on, past LOAD_READS_MS.
async fn late_failure(
    fails: impl Fn(&Arc<StubBackend>, common::stub_backend::GetFn) -> common::stub_backend::Fut<Arc<StubMachine>> + 'static,
) -> (Fx, String, State) {
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let rt1 = runtime(&backend).store(&state.store).build();
    let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace");
    rt1.rt.close().await.expect("a close");
    let get = backend.get_fn();
    let reads = Rc::new(std::cell::Cell::new(0));
    let b = backend.clone();
    let fails = Rc::new(fails);
    backend.set_get(Rc::new(move |_| {
        let first = reads.get() == 0;
        reads.set(reads.get() + 1);
        let (b, get, fails) = (b.clone(), get.clone(), fails.clone());
        Box::pin(async move {
            // The first read waits 2 s, and what fails the read is decided when it lands, as the TS's `await` has it.
            if first {
                sleep_ms(2_000).await;
            }
            fails(&b, get).await
        })
    }));
    let fx = runtime(&backend).store(&state.store).build();
    (fx, ws.view.id, state)
}

// "a machine its provider answers for past the load's deadline" > "holds the record on what the provider said, as a read that fails in time does"
#[test]
fn holds_the_record_on_what_the_provider_said_as_a_read_that_fails_in_time_does() {
    drive(|| async {
        let (fx, id, _state) =
            late_failure(|_, _| Box::pin(async { Err(EngineError::Other { message: "provider: key refused (401)".into(), code: None }) }))
                .await;
        let refused = fx.rt.workspaces().exec(&id, "true", None, None).await.expect_err("a refusal");
        assert!(refused.message.contains("provider: key refused (401)"), "{}", refused.message);
        fx.rt.close().await.expect("a close");
    });
}

// "settles a machine it answers gone for with the provider's words, as a load that met the 404 in time does"
#[test]
fn settles_a_machine_it_answers_gone_for_with_the_providers_words_as_a_load_that_met_the_404_in_time_does() {
    drive(|| async {
        let backend_of: Rc<RefCell<Option<Arc<StubBackend>>>> = Rc::default();
        let _ = &backend_of;
        let (fx, id, _state) = late_failure(|backend, get| {
            for m in backend.machines() {
                m.killed.set(true);
            }
            get("m1".into())
        })
        .await;
        until_for(6_000, &mut || {
            let rt = fx.rt.clone();
            let id = id.clone();
            async move { j(&rt.workspaces().get(&id, None, None).await.expect("the workspace"))["phase"] == "gone" }
        })
        .await;
        let words = j(&fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"))["gone"].as_str().expect("words").to_string();
        assert!(load_saw(&words, "m1"), "{words}");
        fx.rt.close().await.expect("a close");
    });
}

// "a create on a computer that names its own machines" > "runs no hostname command inside the workspace, and a provider's fork is still named"
#[test]
fn runs_no_hostname_command_inside_the_workspace_and_a_providers_fork_is_still_named() {
    drive(|| async {
        let named = StubBackend::new(None);
        named.set_names_workspace(true);
        let fx = runtime(&named).build();
        let ws = create_on(&fx.rt, CreateOn::golden("on-a-box"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_eq!(named.machine(0).resumes.get(), 1);
        assert!(named.machine(0).exec_starting("hostname ").is_empty());

        // The provider's own fork boots as localhost, so that road still names it: the flag is the backend's, not a
        // rule about creates.
        let provider = StubBackend::new(None);
        let forking = runtime(&provider).build();
        create_on(&forking.rt, CreateOn::golden("at-a-provider"), None).await.expect("a workspace");
        assert_eq!(provider.machine(0).exec_starting("hostname "), ["hostname at-a-provider && echo at-a-provider > /etc/hostname"]);
    });
}
