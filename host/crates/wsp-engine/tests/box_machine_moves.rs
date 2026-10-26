// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.test.ts "BoxBackend against a fake Box API", the machine's own moves (lines 672-1420): pause, resume,
//! the size a resume names, the root disk a size reads as, the preview route and its firewall rule, bytes in and out,
//! snapshots, kill, state, describe, the desktop kind and the signed URL roads. Every body in src/ is `todo!()`, so
//! each test fails at the first call into it.
mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde_json::{json, Value};
use wsp_engine::box_backend::{
    stop_refusal_of, BoxBudgetsAsk, BoxClassType, BoxMachine, BoxView, BOX_CLASSES, FILE_GET_MAX, FILE_PUT_MAX, FIREWALL_HOLD_MS,
    FIREWALL_WATCH_MS, TRIAL_TTL_S,
};
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{aborted, is_missing, EngineError, ROAD_TRIES};
use wsp_engine::machine::{
    Machine, MachineBackend, MachineKind, MachineLife, MachineShape, MachineState, PreviewReach, RootDisk, Size, SnapshotOptions,
};
use wsp_wire::types::JsNumber;
use wsp_words::words::thread::stop_refused_line;
use wsp_words::words::units::BOAT_DISK_GONE_LINE;

use common::fake_box::{backend_on, box_view, command, error, info, inner, machine_on, named, with, Answer, FakeBox, Reply};

const ID: &str = "bx_tumrjngm";

fn reply(status: u16, body: Value) -> Answer {
    Answer::Reply(Reply { status, body })
}

fn idle(state: &str) -> Answer {
    info(ID, state, json!({}))
}

fn resuming() -> Answer {
    reply(202, json!({ "ok": true, "status": "resuming", "sandbox": box_view(ID, "provisioned", json!({})) }))
}

fn stopping() -> Answer {
    reply(202, json!({ "ok": true, "type": "sandbox.stopping", "status": "archiving", "sandbox": box_view(ID, "archiving", json!({})) }))
}

fn ok(stdout: &str) -> Answer {
    command(stdout, Some(0), json!({}))
}

fn exit(stdout: &str, code: i64) -> Answer {
    command(stdout, Some(code), json!({}))
}

fn path(suffix: &str) -> String {
    format!("/sandboxes/{ID}{suffix}")
}

fn spec(kind: MachineKind) -> wsp_engine::machine::MachineSpec {
    wsp_engine::machine::MachineSpec {
        kind,
        template: None,
        from_snapshot: None,
        cpu: None,
        mem_mb: None,
        disk_gb: None,
        envs: None,
        labels: None,
        on_idle: None,
        idle_timeout_ms: None,
        idempotency_key: None,
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

fn mins(n: i64) -> i64 {
    n * 60_000
}

/// The last line of the script inside each command the fake was sent.
fn last_lines(api: &FakeBox) -> Vec<String> {
    api.commands_sent().iter().map(|c| inner(c).split('\n').next_back().unwrap_or_default().to_string()).collect()
}

/// `s` with each `<stem>-<12 hex>` read as `<stem>-X`, as the TS's `.replace(/stem-[0-9a-f]{12}/g, "stem-X")` does.
fn masked(s: &str, stem: &str) -> String {
    let needle = format!("{stem}-");
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find(&needle) {
        let (head, tail) = rest.split_at(at + needle.len());
        let hex: String = tail.chars().take(12).collect();
        if hex.len() == 12 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
            out.push_str(head);
            out.push('X');
            rest = &tail[12..];
        } else {
            out.push_str(head);
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

fn posted_commands(api: &FakeBox) -> Vec<String> {
    api.seen().iter().filter(|s| s.method == "POST").map(|s| s.command()).collect()
}

/// box-backend.test.ts:672
#[tokio::test]
async fn pauses_with_one_stop_and_reads_the_box_until_archived_never_a_second_stop_never_force() {
    let api = FakeBox::new().on("GET", &path(""), vec![idle("idle"), idle("archiving"), idle("archiving"), idle("archived")]).on(
        "POST",
        &path("/stop"),
        vec![stopping()],
    );
    let (_on, machine) = machine_on(&api, ID);
    machine.pause(None).await.unwrap();
    assert_eq!(
        api.calls(),
        vec![
            format!("GET {}", path("")),
            format!("POST {}", path("/stop")),
            format!("GET {}", path("")),
            format!("GET {}", path("")),
            format!("GET {}", path(""))
        ]
    );
    assert_eq!(api.took("POST", &path("/stop")).unwrap().body, json!({}));
}

/// box-backend.test.ts:682
#[tokio::test]
async fn resumes_a_stopped_box_at_a_size_with_one_resume_naming_the_class_and_answers_once_it_runs() {
    let api = FakeBox::new().on("POST", &path("/resume"), vec![resuming()]).on("GET", &path(""), vec![idle("ready")]).on(
        "POST",
        &path("/commands"),
        vec![ok("")],
    );
    let (_on, machine) = machine_on(&api, ID);
    machine.resume_at(4.0, 8192, None).await.unwrap();
    let posts: Vec<String> =
        api.calls().into_iter().filter(|c| c.starts_with(&format!("POST {}/", path(""))) && !c.ends_with("/commands")).collect();
    assert_eq!(posts, vec![format!("POST {}", path("/resume"))]);
    assert_eq!(api.took("POST", &path("/resume")).unwrap().body["type"], json!("default"));
    assert_eq!(api.calls().last().unwrap(), &format!("POST {}", path("/commands")));
}

/// box-backend.test.ts:693
#[tokio::test]
async fn names_the_root_disk_each_size_reads_as_measured_and_none_for_a_size_nobody_measured() {
    let on = backend_on(&FakeBox::new(), "trial", Default::default());
    let disks: Vec<(BoxClassType, Option<f64>)> =
        BOX_CLASSES.iter().map(|c| (c.kind, on.backend.root_disk_of(Size { cpu: c.cpu, mem_mb: c.mem_mb }).map(|d| d.disk_gb))).collect();
    assert_eq!(
        disks,
        vec![
            (BoxClassType::Small, Some(75.0)),
            (BoxClassType::Default, Some(75.0)),
            (BoxClassType::Large, Some(160.0)),
            (BoxClassType::Xlarge, None)
        ]
    );
    assert_eq!(on.backend.root_disk_of(Size { cpu: 8.0, mem_mb: 16384.0 }), Some(RootDisk { name: "large".into(), disk_gb: 160.0 }));
}

/// box-backend.test.ts:699
#[tokio::test]
async fn a_plain_resume_names_no_class_so_the_box_keeps_its_own() {
    let api = FakeBox::new().on("POST", &path("/resume"), vec![resuming()]).on("GET", &path(""), vec![idle("ready")]).on(
        "POST",
        &path("/commands"),
        vec![ok("")],
    );
    let (_on, machine) = machine_on(&api, ID);
    machine.resume(None).await.unwrap();
    let body = api.took("POST", &path("/resume")).unwrap().body;
    assert!(!body.as_object().expect("a JSON object").contains_key("type"), "{body}");
}

/// box-backend.test.ts:708
#[tokio::test]
async fn a_resume_boat_refuses_for_a_deleted_disk_ends_in_the_disk_gone_error_in_wsps_line_and_reads_nothing_more() {
    let said = "The sandbox stayed stopped for 14 days. Boat deleted its data.";
    let api = FakeBox::new().on("POST", &path("/resume"), vec![error(409, "snapshot_data_expired", said).into()]);
    let (_on, machine) = machine_on(&api, ID);
    let e = machine.resume(None).await.unwrap_err();
    let EngineError::DiskGone { machine_id, line, said: got } = &e else { panic!("not a DiskGone: {e:?}") };
    assert_eq!(machine_id, ID);
    assert_eq!(line, BOAT_DISK_GONE_LINE);
    assert_eq!(e.to_string(), BOAT_DISK_GONE_LINE);
    assert_eq!(got, said);
    let calls: Vec<String> = api.calls().into_iter().filter(|c| c != "GET /limits").collect();
    assert_eq!(calls, vec![format!("POST {}", path("/resume"))]);
}

/// box-backend.test.ts:727
#[tokio::test]
async fn a_box_already_archiving_or_archived_gets_no_stop_at_all() {
    let api = FakeBox::new().on("GET", &path(""), vec![idle("archiving"), idle("archived")]);
    let (_on, machine) = machine_on(&api, ID);
    machine.pause(None).await.unwrap();
    assert_eq!(api.calls(), vec![format!("GET {}", path("")), format!("GET {}", path(""))]);
    api.on("GET", &path(""), vec![idle("archived")]);
    machine.pause(None).await.unwrap();
    assert_eq!(api.calls().into_iter().filter(|c| c.starts_with("POST")).collect::<Vec<_>>(), Vec::<String>::new());
}

/// box-backend.test.ts:737
#[tokio::test]
async fn a_stop_the_provider_refuses_is_thrown_in_its_words_and_one_it_takes_back_leaves_the_pause_not_taken() {
    let refused = FakeBox::new().on("GET", &path(""), vec![idle("idle")]).on(
        "POST",
        &path("/stop"),
        vec![error(400, "stop_failed", "Snapshot failed; the box keeps running.").into()],
    );
    let (_on, machine) = machine_on(&refused, ID);
    let e = machine.pause(None).await.unwrap_err();
    let EngineError::Provider(w) = &e else { panic!("not a Provider refusal: {e:?}") };
    assert_eq!(w.code.as_deref(), Some("stop_failed"));
    assert_eq!(w.message, "Snapshot failed; the box keeps running.");
    assert_eq!(refused.calls().into_iter().filter(|c| c.ends_with("/stop")).count(), 1);

    // The box's own fields after a stop the provider took back: its latest snapshot failed while the last that
    // completed stays behind, which is how it says its snapshots keep failing.
    let failing = json!({ "lastSnapshotStatus": "failed", "lastSnapshotAttemptAt": "2026-10-01T00:40:00Z", "snapshotCompletedAt": "2026-09-30T21:02:00Z" });
    let undone = FakeBox::new().on("GET", &path(""), vec![idle("idle"), idle("archiving"), info(ID, "idle", failing.clone())]).on(
        "POST",
        &path("/stop"),
        vec![stopping()],
    );
    let (on, machine) = machine_on(&undone, ID);
    let e = machine.pause(None).await.unwrap_err();
    let said = "its latest snapshot failed at 2026-10-01T00:40:00Z, and the last that completed was at 2026-09-30T21:02:00Z";
    let EngineError::StopRefused { machine_id, said: got, line } = &e else { panic!("not a StopRefused: {e:?}") };
    assert_eq!(machine_id, ID);
    assert_eq!(got, said);
    assert_eq!(line, &stop_refused_line(said));
    assert_eq!(e.to_string(), stop_refused_line(said));
    // The line the host logs keeps the provider's reading, not a guess at it.
    assert!(
        on.warned.lines().contains(&format!("{ID}: pause did not take: the provider left it idle and refused the stop: {said}")),
        "{:?}",
        on.warned.lines()
    );
    assert_eq!(undone.calls().into_iter().filter(|c| c.ends_with("/stop")).count(), 1);

    // The box's own error, where it gives one, is the reading.
    let own = BoxView {
        error: Some("disk snapshot failed: no space left on device".into()),
        last_snapshot_status: Some("failed".into()),
        last_snapshot_attempt_at: Some("2026-10-01T00:40:00Z".into()),
        snapshot_completed_at: Some("2026-09-30T21:02:00Z".into()),
        ..Default::default()
    };
    assert_eq!(stop_refusal_of(&own), "disk snapshot failed: no space left on device");
    assert_eq!(stop_refusal_of(&BoxView::default()), "it reported no snapshot attempt, and the last that completed was none");
}

/// box-backend.test.ts:767
#[tokio::test]
async fn a_pause_given_a_longer_wait_outlasts_the_budget_while_a_full_disk_archives() {
    let mut reads = vec![idle("idle")];
    reads.extend((0..20).map(|_| idle("archiving")));
    reads.push(idle("archived"));
    let api = FakeBox::new().on("GET", &path(""), reads).on("POST", &path("/stop"), vec![stopping()]);
    let (on, machine) = machine_on(&api, ID);
    let before = on.clock.now_ms();
    machine.pause(Some(30 * 60_000)).await.unwrap();
    assert!(on.clock.now_ms() - before > 100);
    assert_eq!(api.calls().into_iter().filter(|c| c.ends_with("/stop")).count(), 1);
}

/// box-backend.test.ts:779
#[tokio::test]
async fn a_stop_that_never_lands_inside_the_budget_ends_the_pause_with_the_rows_words() {
    let api = FakeBox::new().on("GET", &path(""), vec![idle("idle"), idle("archiving")]).on("POST", &path("/stop"), vec![stopping()]);
    let (on, machine) = machine_on(&api, ID);
    let before = on.clock.now_ms();
    let e = machine.pause(None).await.unwrap_err();
    let EngineError::MoveUnanswered { message, .. } = &e else { panic!("not a MoveUnanswered: {e:?}") };
    assert!(message.starts_with("pause did not complete in "), "{message}");
    assert!(message.ends_with("; the provider did not answer and reads the machine paused; try again"), "{message}");
    assert!(on.clock.now_ms() - before >= 100);
}

/// box-backend.test.ts:796
#[tokio::test]
async fn resumes_with_the_accounts_auto_stop_and_reads_the_box_until_ready_one_attempt() {
    let api = FakeBox::new()
        .on("POST", &path("/resume"), vec![reply(202, json!({ "ok": true, "type": "sandbox.resuming", "id": ID, "status": "resuming", "sandbox": box_view(ID, "provisioned", json!({})) }))])
        .on("GET", &path(""), vec![idle("provisioned"), info(ID, "ready", json!({ "ip": "116.203.245.5" }))])
        .on("POST", &path("/commands"), vec![ok("")]);
    let (_on, machine) = machine_on(&api, ID);
    machine.resume(None).await.unwrap();
    // The road every command takes is proved once the box reads ready, before the handle is anyone's to use.
    assert_eq!(
        api.calls(),
        vec![
            "GET /limits".to_string(),
            format!("POST {}", path("/resume")),
            format!("GET {}", path("")),
            format!("GET {}", path("")),
            format!("POST {}", path("/commands"))
        ]
    );
    assert_eq!(api.took("POST", &path("/resume")).unwrap().body, json!({ "ttlSeconds": TRIAL_TTL_S }));
    // `JSON.stringify(machine)` not containing the address has no Rust form: BoxMachine's fields are the id, kind,
    // labels, what get() saw, replayed and the deletion, with nowhere to keep an address.
}

/// box-backend.test.ts:809
#[tokio::test]
async fn a_resume_arms_the_timer_the_box_last_had_again_from_now_and_the_cap_where_none_is_on_record() {
    let created =
        reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_r1", "provisioning", json!({ "name": "wsp" })) }));
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![created])
        .on("GET", "/sandboxes/bx_r1", vec![info("bx_r1", "ready", json!({ "name": "wsp" }))])
        .on(
            "POST",
            "/sandboxes/bx_r1/resume",
            vec![reply(202, json!({ "ok": true, "status": "resuming", "sandbox": box_view("bx_r1", "provisioned", json!({})) }))],
        )
        .on("PATCH", "/sandboxes/bx_r1", vec![info("bx_r1", "idle", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_r1/commands", vec![ok("")]);
    let on = backend_on(&api, "standard", Default::default());
    let mut asked = spec(MachineKind::Sandbox);
    asked.on_idle = Some(wsp_wire::wire::machine_link::MachineSpecOnIdle::Pause);
    asked.idle_timeout_ms = Some(JsNumber(40.0 * 60_000.0));
    let machine = on.backend.create(asked, None, None).await.unwrap();
    // Hours later, with no arming since the create: the create's window, from now, not the instant that has passed.
    on.clock.advance(5 * mins(60));
    machine.resume(None).await.unwrap();
    assert_eq!(api.took("POST", "/sandboxes/bx_r1/resume").unwrap().body, json!({ "ttlSeconds": 40 * 60 }));
    // A backstop armed since is the span a resume arms again.
    on.backend.box_backstop(machine.id(), on.clock.now_ms() + mins(90), false).await.unwrap();
    on.clock.advance(3 * mins(60));
    machine.resume(None).await.unwrap();
    let resumes: Vec<_> = api.seen().into_iter().filter(|s| s.path == "/sandboxes/bx_r1/resume").collect();
    assert_eq!(resumes.last().unwrap().body, json!({ "ttlSeconds": 90 * 60 }));
    // A handle from get() has nothing on record: the cap, which off the trial is no timer.
    api.on(
        "POST",
        "/sandboxes/bx_r2/resume",
        vec![reply(202, json!({ "ok": true, "status": "resuming", "sandbox": box_view("bx_r2", "provisioned", json!({})) }))],
    )
    .on("GET", "/sandboxes/bx_r2", vec![info("bx_r2", "ready", json!({}))])
    .on("POST", "/sandboxes/bx_r2/commands", vec![ok("")]);
    BoxMachine::new(on.backend.clone(), "bx_r2", MachineKind::Sandbox, None, None, false).resume(None).await.unwrap();
    assert_eq!(api.took("POST", "/sandboxes/bx_r2/resume").unwrap().body, json!({ "ttlSeconds": null }));
}

/// box-backend.test.ts:833
#[tokio::test]
async fn a_resume_of_a_box_the_provider_lost_is_missing_one_that_reads_error_names_it_and_a_stopped_one_ends_at_once() {
    let gone = FakeBox::new().on("POST", "/sandboxes/bx_gone/resume", vec![error(404, "not_found", "not_found").into()]);
    let (_on, machine) = machine_on(&gone, "bx_gone");
    let lost = machine.resume(None).await.unwrap_err();
    assert!(is_missing(&lost), "{lost:?}");

    let broken = FakeBox::new().on("POST", &path("/resume"), vec![resuming()]).on(
        "GET",
        &path(""),
        vec![info(ID, "error", json!({ "error": "restore failed" }))],
    );
    let (_on, machine) = machine_on(&broken, ID);
    let e = machine.resume(None).await.unwrap_err();
    assert!(e.to_string().contains("reads error during resume: restore failed"), "{e}");

    let slow = FakeBox::new().on("POST", &path("/resume"), vec![resuming()]).on("GET", &path(""), vec![idle("provisioned")]);
    let stop = Cancel::new();
    let (_on, machine) = machine_on(&slow, ID);
    let (waited, _) = tokio::join!(machine.resume(Some(&stop)), async {
        tokio::task::yield_now().await;
        stop.cancel(None);
    });
    // No reason was passed: the cancel ends the resume as every road ends on one, with the abort.
    assert_eq!(waited.unwrap_err(), aborted());
}

/// box-backend.test.ts:853
#[tokio::test]
async fn a_resume_the_caller_stopped_waiting_on_ends_where_the_stop_was_even_while_the_probe_is_refused() {
    let stop = Cancel::new();
    let refusals = Arc::new(AtomicU32::new(0));
    let api = FakeBox::new().on("POST", &path("/resume"), vec![resuming()]).on("GET", &path(""), vec![idle("ready")]).on(
        "POST",
        &path("/commands"),
        vec![with({
            let (stop, refusals) = (stop.clone(), refusals.clone());
            move |_| {
                if refusals.fetch_add(1, Ordering::SeqCst) + 1 == 1 {
                    stop.cancel(None);
                }
                Answer::Reply(error(502, "bad_gateway", "Bad Gateway"))
            }
        })],
    );
    let on = backend_on(&api, "trial", BoxBudgetsAsk { restore_ms: Some(10_000), ..Default::default() });
    let machine = BoxMachine::new(on.backend.clone(), ID, MachineKind::Sandbox, None, None, false);
    let e = machine.resume(Some(&stop)).await.unwrap_err();
    assert_eq!(e, aborted());
    // The stop ends the probe's waiting too: no second round of commands goes to a box nobody is waiting for.
    assert!(refusals.load(Ordering::SeqCst) <= ROAD_TRIES);
}

fn hosted(port: u16, url: &str, protected: bool, extra: Value) -> Answer {
    let mut body =
        json!({ "ok": true, "port": port, "url": url, "isProtected": protected, "access": if protected { "private" } else { "public" } });
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        body[k] = v;
    }
    reply(200, body)
}

const MAX_SAFE: f64 = 9007199254740991.0;

/// box-backend.test.ts:870
#[tokio::test]
async fn routes_the_daemons_port_public_with_no_firewall_call() {
    let api = FakeBox::new().on(
        "POST",
        &path("/host"),
        vec![hosted(7070, "https://box-node-f06a49ff29e4ed7a-7070.on.boat.dev", false, json!({ "type": "port.hosted", "success": true }))],
    );
    let (_on, machine) = machine_on(&api, ID);
    let reach = machine.preview_url(7070).await.unwrap();
    assert_eq!(
        reach,
        PreviewReach { url: "https://box-node-f06a49ff29e4ed7a-7070.on.boat.dev".into(), token: "".into(), expires_at: JsNumber(MAX_SAFE) }
    );
    assert_eq!(api.calls(), vec![format!("POST {}", path("/host"))]);
    assert_eq!(api.took("POST", &path("/host")).unwrap().body, json!({ "port": 7070, "public": true }));
}

/// box-backend.test.ts:879
#[tokio::test]
async fn mints_a_private_route_for_any_other_port_once_the_firewall_rule_stands_the_token_rides_in_the_query() {
    let api = FakeBox::new().on("POST", &path("/commands"), vec![ok("")]).on(
        "POST",
        &path("/host"),
        vec![hosted(
            3000,
            "https://box-node-f06a49ff29e4ed7a-3000.on.boat.dev?_token=tok_abc",
            true,
            json!({ "type": "port.hosted", "success": true }),
        )],
    );
    let (_on, machine) = machine_on(&api, ID);
    let reach = machine.preview_url(3000).await.unwrap();
    assert_eq!(
        reach,
        PreviewReach {
            url: "https://box-node-f06a49ff29e4ed7a-3000.on.boat.dev?_token=tok_abc".into(),
            token: "tok_abc".into(),
            expires_at: JsNumber(MAX_SAFE)
        }
    );
    assert_eq!(api.calls(), vec![format!("POST {}", path("/commands")), format!("POST {}", path("/host"))]);
    assert!(inner(&api.took("POST", &path("/commands")).unwrap().command()).contains("ufw status | grep -q '^3000/tcp '"));
    assert_eq!(api.took("POST", &path("/host")).unwrap().body, json!({ "port": 3000, "public": false }));
}

fn route_t(token: &str) -> Answer {
    hosted(3000, &format!("https://sulu-axioms-gelant-3000.on.boat.dev?_token={token}"), true, json!({}))
}

/// box-backend.test.ts:891
#[tokio::test]
async fn mints_the_route_with_one_read_when_the_firewall_rule_already_stands() {
    let api = FakeBox::new().on("POST", &path("/commands"), vec![ok("")]).on("POST", &path("/host"), vec![route_t("t2")]);
    let (_on, machine) = machine_on(&api, ID);
    assert_eq!(machine.preview_url(3000).await.unwrap().token, "t2");
    assert_eq!(last_lines(&api), vec!["ufw status | grep -q '^3000/tcp '".to_string()]);
}

/// box-backend.test.ts:901
#[tokio::test]
async fn a_rule_the_boxs_own_agent_wipes_after_boot_is_added_again_and_held_for_the_hold_window() {
    // The box answers, in order: no rule; the add; a read that finds it; a read that finds it gone (the agent's
    // rewrite); the add again; then reads that keep finding it until the hold window is over.
    let script = [1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let api = FakeBox::new().on("POST", &path("/commands"), script.iter().map(|c| exit("", *c)).collect()).on(
        "POST",
        &path("/host"),
        vec![route_t("t2")],
    );
    let (on, machine) = machine_on(&api, ID);
    let before = on.clock.now_ms();
    assert_eq!(machine.preview_url(3000).await.unwrap().token, "t2");
    // The hold was real: the loop returned because the second add stood for the whole window, not because a deadline
    // counted from the start ran out under it.
    assert!(on.warned.lines().is_empty(), "{:?}", on.warned.lines());
    let commands = last_lines(&api);
    assert_eq!(commands.iter().filter(|c| *c == "ufw allow 3000/tcp").count(), 2);
    assert_eq!(commands[0], "ufw status | grep -q '^3000/tcp '");
    assert_eq!(commands[1], "ufw allow 3000/tcp");
    assert_eq!(commands[3], "ufw status | grep -q '^3000/tcp '");
    assert_eq!(commands[4], "ufw allow 3000/tcp");
    // Two adds five seconds of watching apart, then a full hold after the second: the loop ran past the first add's
    // hold window by the whole of the second's.
    assert!(on.clock.now_ms() - before >= (2 * FIREWALL_WATCH_MS + FIREWALL_HOLD_MS) as i64);
    assert_eq!(api.calls().last().unwrap(), &format!("POST {}", path("/host")));
}

/// box-backend.test.ts:931
#[tokio::test]
async fn a_rule_the_guest_refuses_while_it_boots_is_asked_for_again_one_that_never_holds_is_said_once() {
    let ufw_init = || command("", Some(1), json!({ "stderr": "ERROR: problem running ufw-init" }));
    let early = FakeBox::new().on("POST", &path("/commands"), vec![exit("", 1), ufw_init(), ufw_init(), ok("Rule added\n"), ok("")]).on(
        "POST",
        &path("/host"),
        vec![route_t("t2")],
    );
    let (_on, machine) = machine_on(&early, ID);
    assert_eq!(machine.preview_url(3000).await.unwrap().token, "t2");
    let commands = last_lines(&early);
    assert_eq!(
        commands[..4].to_vec(),
        vec!["ufw status | grep -q '^3000/tcp '", "ufw allow 3000/tcp", "ufw allow 3000/tcp", "ufw allow 3000/tcp"]
    );

    let never = FakeBox::new()
        .on("POST", &path("/commands"), vec![reply(409, error(409, "machine_not_running", "Box machine is not running.").body)])
        .on("POST", &path("/host"), vec![route_t("t3")]);
    let (on, stuck) = machine_on(&never, ID);
    assert_eq!(stuck.preview_url(3000).await.unwrap().token, "t3");
    assert!(never.calls().into_iter().filter(|c| c.ends_with("/commands")).count() > 2);
    // The refusal is logged once however often the watch meets it, and the rule that never held is said once.
    let lines = on.warned.lines();
    let refused = format!("Boat refused POST {}: 409 code=machine_not_running", path("/commands"));
    assert_eq!(lines.iter().filter(|l| l.starts_with(&refused)).count(), 1, "{lines:?}");
    let others: Vec<&String> = lines.iter().filter(|l| !l.starts_with("Boat refused")).collect();
    assert_eq!(others.len(), 1, "{lines:?}");
    assert!(others[0].contains("firewall did not hold a rule for port 3000 (Box machine is not running.)"), "{}", others[0]);
}

/// box-backend.test.ts:958
#[tokio::test]
async fn puts_bytes_through_put_files_straight_into_tmp_and_by_way_of_tmp_into_roots_folders_under_sudo() {
    let api = FakeBox::new()
        .on(
            "PUT",
            &path("/files"),
            vec![reply(
                200,
                json!({ "ok": true, "type": "file.written", "success": true, "path": "../../tmp/x", "encoding": "base64", "size": 3 }),
            )],
        )
        .on("POST", &path("/commands"), vec![ok("")]);
    let (_on, machine) = machine_on(&api, ID);
    machine.put_bytes("/tmp/wsp-ping.sh", &[1, 2, 3], None).await.unwrap();
    assert_eq!(
        api.took("PUT", &path("/files")).unwrap().body,
        json!({ "path": "/tmp/wsp-ping.sh", "content": B64.encode([1u8, 2, 3]), "encoding": "base64" })
    );
    assert_eq!(api.calls(), vec![format!("PUT {}", path("/files"))]);

    machine.put_bytes("/root/wsp-daemon.tgz", &[4, 5], None).await.unwrap();
    let seen = api.seen();
    let staged = seen[seen.len() - 2].body["path"].as_str().unwrap().to_string();
    assert_eq!(masked(&staged, "wsp-put"), "/tmp/wsp-put-X.0", "{staged}");
    let joined = inner(&seen[seen.len() - 1].command());
    assert!(
        joined.contains(&format!("cat '{staged}' > '/root/wsp-daemon.tgz' && chmod 0644 '/root/wsp-daemon.tgz' && rm -f '{staged}'")),
        "{joined}"
    );
    assert!(joined.contains("mkdir -p '/root'"), "{joined}");
    assert!(joined.contains("[ \"$(stat -c %s '/root/wsp-daemon.tgz')\" = 2 ]"), "{joined}");
}

/// box-backend.test.ts:976
#[tokio::test]
async fn a_join_over_a_file_already_at_the_new_bytes_size_still_writes_them_and_one_sent_again_is_done() {
    use std::os::unix::fs::PermissionsExt;
    let api =
        FakeBox::new().on("PUT", &path("/files"), vec![reply(200, json!({ "ok": true }))]).on("POST", &path("/commands"), vec![ok("")]);
    let (_on, machine) = machine_on(&api, ID);
    machine.put_bytes("/root/.wsp/daemon-token", b"new-token", None).await.unwrap();
    let seen = api.seen();
    let staged = seen[seen.len() - 2].body["path"].as_str().unwrap().to_string();
    let script = inner(&seen[seen.len() - 1].command());
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let bin = root.join("bin");
    std::fs::create_dir(&bin).unwrap();
    // The machine's GNU stat: -c %s is a file's size. protocol/test/stub-script.ts `writeStub`.
    let stat = bin.join("stat");
    std::fs::write(&stat, "#!/bin/sh\n[ -e \"$3\" ] && wc -c < \"$3\" | tr -d \" \" || exit 1\n").unwrap();
    std::fs::set_permissions(&stat, std::fs::Permissions::from_mode(0o755)).unwrap();
    let at = |p: &str| root.join(p).to_string_lossy().into_owned();
    std::fs::create_dir_all(at("root/.wsp")).unwrap();
    std::fs::create_dir(at("tmp")).unwrap();
    std::fs::write(at("root/.wsp/daemon-token"), "old-token").unwrap();
    std::fs::write(at(staged.trim_start_matches('/')), "new-token").unwrap();
    let local = script
        .replace(&staged, &at(staged.trim_start_matches('/')))
        .replace("/root/.wsp/daemon-token", &at("root/.wsp/daemon-token"))
        .replace("'/root/.wsp'", &format!("'{}'", at("root/.wsp")));
    let run = || -> i32 {
        std::process::Command::new("sh")
            .args(["-c", &local])
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .status()
            .unwrap()
            .code()
            .unwrap_or(-1)
    };
    assert_eq!(run(), 0);
    assert_eq!(std::fs::read_to_string(at("root/.wsp/daemon-token")).unwrap(), "new-token");
    assert_eq!(run(), 0);
    assert_eq!(std::fs::read_to_string(at("root/.wsp/daemon-token")).unwrap(), "new-token");
}

/// box-backend.test.ts:1006
#[tokio::test]
async fn bytes_past_the_files_endpoints_cap_go_up_in_pieces_and_are_joined_into_place_the_size_read_back() {
    let api = FakeBox::new().on("PUT", &path("/files"), vec![reply(200, json!({ "ok": true }))]).on(
        "POST",
        &path("/commands"),
        vec![ok(""), exit("WSP_SHORT 5242880\n", 1)],
    );
    let (_on, machine) = machine_on(&api, ID);
    let cap = FILE_PUT_MAX as usize;
    let mut big = vec![0u8; 2 * cap + 7];
    big[cap] = 9;
    machine.put_bytes("/tmp/big.tgz", &big, None).await.unwrap();
    let puts: Vec<_> = api.seen().into_iter().filter(|s| s.method == "PUT").collect();
    let bytes: Vec<Vec<u8>> = puts.iter().map(|p| B64.decode(p.body["content"].as_str().unwrap()).unwrap()).collect();
    assert_eq!(bytes.iter().map(|b| b.len()).collect::<Vec<_>>(), vec![cap, cap, 7]);
    assert_eq!(bytes[1][0], 9);
    let paths: Vec<String> = puts.iter().map(|p| p.body["path"].as_str().unwrap().to_string()).collect();
    assert_eq!(
        paths.iter().map(|p| masked(p, "wsp-put")).collect::<Vec<_>>(),
        vec!["/tmp/wsp-put-X.0", "/tmp/wsp-put-X.1", "/tmp/wsp-put-X.2"]
    );
    let seen = api.seen();
    let joined = inner(&seen.last().unwrap().command());
    assert!(joined.contains(&format!("cat '{}' '{}' '{}' > '/tmp/big.tgz'", paths[0], paths[1], paths[2])), "{joined}");
    assert!(joined.contains(&format!("= {} ]", 2 * cap + 7)), "{joined}");
    // Sent again after a 502 that may have run it once already, the join finds its pieces gone and the target at its
    // size, and is done before it opens the target or misses the pieces.
    assert_eq!(
        joined.split('\n').nth(2).unwrap(),
        format!("[ ! -e '{}' ] && [ \"$(stat -c %s '/tmp/big.tgz' 2>/dev/null)\" = {} ] && exit 0", paths[0], 2 * cap + 7)
    );
    let e = machine.put_bytes("/root/x", &vec![0u8; cap + 1], None).await.unwrap_err();
    assert!(e.to_string().contains("/root/x did not land on bx_tumrjngm (exit 1): WSP_SHORT 5242880"), "{e}");
}

/// box-backend.test.ts:1027
#[tokio::test]
async fn reads_a_file_off_the_box_through_get_files_one_past_the_cap_in_pieces_the_whole_checked_against_its_size() {
    let small = b"gzipped session".to_vec();
    let cap = FILE_GET_MAX as usize;
    let head = vec![1u8; cap];
    let api = FakeBox::new()
        .on(
            "GET",
            &path("/files"),
            vec![
                reply(200, json!({ "ok": true, "type": "file.read", "path": "/tmp/s.gz", "encoding": "base64", "size": small.len(), "content": B64.encode(&small) })),
                reply(200, json!({ "ok": true, "content": B64.encode(&head) })),
                reply(200, json!({ "ok": true, "content": B64.encode(b"abc") })),
            ],
        )
        .on("POST", &path("/commands"), vec![ok(""), ok("")]);
    let (_on, machine) = machine_on(&api, ID);
    assert_eq!(machine.read_bytes("/tmp/s.gz", Some(small.len() as u64), None).await.unwrap(), small);
    assert_eq!(api.seen()[0].query, vec![("path".to_string(), "/tmp/s.gz".to_string()), ("encoding".to_string(), "base64".to_string())]);
    let big = machine.read_bytes("/root/big.gz", Some(FILE_GET_MAX + 3), None).await.unwrap();
    assert_eq!(big.len(), cap + 3);
    let reads: Vec<String> =
        api.seen().iter().filter(|s| s.method == "GET").skip(1).map(|s| masked(s.query_get("path").unwrap(), "wsp-get")).collect();
    assert_eq!(reads, vec!["/tmp/wsp-get-X.0000", "/tmp/wsp-get-X.0001"]);
    let commands: Vec<String> =
        posted_commands(&api).iter().map(|c| masked(inner(c).split('\n').next_back().unwrap(), "wsp-get")).collect();
    assert_eq!(
        commands,
        vec![
            format!("split -b {FILE_GET_MAX} -d -a 4 '/root/big.gz' '/tmp/wsp-get-X.' && chmod 0644 '/tmp/wsp-get-X.0000' '/tmp/wsp-get-X.0001'"),
            "rm -f '/tmp/wsp-get-X.0000' '/tmp/wsp-get-X.0001'".to_string(),
        ]
    );
}

fn saving(name: &str, status: &str, extra: Value) -> Answer {
    reply(200, json!({ "ok": true, "snapshot": named(name, status, extra) }))
}

/// box-backend.test.ts:1047
#[tokio::test]
async fn a_snapshot_is_a_named_snapshot_from_any_life_answered_with_its_name_once_the_provider_reads_it_ready() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/named-snapshots",
            vec![reply(202, json!({ "ok": true, "type": "snapshot.named.saving", "status": "saving", "snapshot": named("wsp-mac-default-v1", "saving", json!({})) }))],
        )
        .on(
            "GET",
            "/named-snapshots/wsp-mac-default-v1",
            vec![saving("wsp-mac-default-v1", "saving", json!({})), saving("wsp-mac-default-v1", "ready", json!({}))],
        );
    let (_on, machine) = machine_on(&api, ID);
    // goldenName("mac", "default", 1): snapshot-names.ts:23
    let name = "wsp-mac-default-v1";
    let got = machine.snapshot(name, MachineLife { first_life: false }, SnapshotOptions::default()).await.unwrap();
    assert_eq!(got, "wsp-mac-default-v1");
    assert_eq!(api.took("POST", "/named-snapshots").unwrap().body, json!({ "sandboxId": ID, "name": "wsp-mac-default-v1" }));
    assert_eq!(
        api.calls(),
        vec!["POST /named-snapshots", "GET /named-snapshots/wsp-mac-default-v1", "GET /named-snapshots/wsp-mac-default-v1"]
    );
}

/// box-backend.test.ts:1057
#[tokio::test]
async fn a_save_the_provider_fails_or_one_still_saving_at_the_budget_is_a_failure_in_its_words() {
    let started = |name: &str| reply(202, json!({ "ok": true, "status": "saving", "snapshot": named(name, "saving", json!({})) }));
    let failed = FakeBox::new().on("POST", "/named-snapshots", vec![started("wsp-mac-default-v2")]).on(
        "GET",
        "/named-snapshots/wsp-mac-default-v2",
        vec![saving("wsp-mac-default-v2", "failed", json!({ "error": "capture timed out" }))],
    );
    let (_on, machine) = machine_on(&failed, ID);
    let e = machine.snapshot("wsp-mac-default-v2", MachineLife { first_life: true }, SnapshotOptions::default()).await.unwrap_err();
    assert!(e.to_string().contains("wsp-mac-default-v2 of bx_tumrjngm failed: capture timed out"), "{e}");

    let slow = FakeBox::new().on("POST", "/named-snapshots", vec![started("wsp-mac-default-v3")]).on(
        "GET",
        "/named-snapshots/wsp-mac-default-v3",
        vec![saving("wsp-mac-default-v3", "saving", json!({}))],
    );
    let (_on, machine) = machine_on(&slow, ID);
    let e = machine.snapshot("wsp-mac-default-v3", MachineLife { first_life: true }, SnapshotOptions::default()).await.unwrap_err();
    assert!(e.to_string().contains("still reads saving"), "{e}");
}

/// box-backend.test.ts:1160
#[tokio::test]
async fn kills_with_the_confirm_header_keeps_the_operation_id_and_takes_a_box_already_gone_as_killed() {
    let api = FakeBox::new().on(
        "DELETE",
        &path(""),
        vec![reply(
            202,
            json!({ "ok": true, "type": "sandbox.deleting", "operation": { "id": "bdop_e35decb98f414320a0da76eb36fbf98d", "kind": "box", "targetId": ID, "reason": "explicit", "status": "pending", "attemptCount": 0 } }),
        )],
    );
    let (_on, machine) = machine_on(&api, ID);
    machine.kill().await.unwrap();
    assert_eq!(api.took("DELETE", &path("")).unwrap().headers.get("x-ascii-confirm-delete").map(String::as_str), Some(ID));
    assert_eq!(machine.deletion().as_deref(), Some("bdop_e35decb98f414320a0da76eb36fbf98d"));
    // Nothing waits on the operation: the box is gone from GET within seconds while the operation reads blocked.
    assert_eq!(api.calls(), vec![format!("DELETE {}", path(""))]);
    let gone = FakeBox::new().on("DELETE", "/sandboxes/bx_gone", vec![error(404, "not_found", "not_found").into()]);
    let (_on, machine) = machine_on(&gone, "bx_gone");
    machine.kill().await.unwrap();
}

/// box-backend.test.ts:1186
#[tokio::test]
async fn reads_the_providers_states_onto_the_machines_four_a_box_it_lost_as_gone() {
    let api = FakeBox::new();
    let (_on, machine) = machine_on(&api, ID);
    let table: [(&str, MachineState); 10] = [
        ("init", MachineState::Starting),
        ("provisioning", MachineState::Starting),
        ("provisioned", MachineState::Starting),
        ("cloning", MachineState::Starting),
        ("ready", MachineState::Running),
        ("idle", MachineState::Running),
        ("running", MachineState::Running),
        ("archiving", MachineState::Paused),
        ("archived", MachineState::Paused),
        ("error", MachineState::Gone),
    ];
    for (theirs, ours) in table {
        api.on("GET", &path(""), vec![idle(theirs)]);
        assert_eq!(machine.state().await.unwrap(), ours, "{theirs}");
    }
    api.on("GET", &path(""), vec![error(404, "not_found", "not_found").into()]);
    assert_eq!(machine.state().await.unwrap(), MachineState::Gone);
}

/// box-backend.test.ts:1209
#[tokio::test]
async fn describes_the_class_the_box_runs_as_its_disk_the_class_floor_and_its_creation_time() {
    let api = FakeBox::new().on("GET", &path(""), vec![info(ID, "idle", json!({ "type": "default", "vcpu": 4, "memoryGB": 8 }))]);
    let (_on, machine) = machine_on(&api, ID);
    assert_eq!(
        machine.describe().await.unwrap(),
        MachineShape {
            cpu: Some(JsNumber(4.0)),
            mem_mb: Some(JsNumber(8192.0)),
            disk_gb: Some(JsNumber(50.0)),
            created_at: Some("2026-09-11T06:28:06.053Z".into()),
            used_bytes: None,
        }
    );
}

/// box-backend.test.ts:1403
#[tokio::test]
async fn a_desktop_machine_boots_the_same_box_with_no_stream_the_desktop_stays_off_until_read_per_open() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![reply(
                202,
                json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_d1", "provisioning", json!({ "name": "wsp" })) }),
            )],
        )
        .on("GET", "/sandboxes/bx_d1", vec![info("bx_d1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_d1/commands", vec![ok("")]);
    let on = backend_on(&api, "trial", Default::default());
    let machine = on.backend.create(spec(MachineKind::Desktop), None, None).await.unwrap();
    assert_eq!(machine.kind(), MachineKind::Desktop);
    assert_eq!(machine.stream_url(), None);
    assert_eq!(machine.daemon_supervisor(), None);
}

/// box-backend.test.ts:1415
#[tokio::test]
async fn the_signed_url_roads_say_where_a_boxs_bytes_go_instead() {
    let (_on, machine) = machine_on(&FakeBox::new(), ID);
    let e = machine.download_url("/tmp/x").await.unwrap_err();
    assert!(e.to_string().contains("GET /files"), "{e}");
    let e = machine.upload_url("/tmp/x").await.unwrap_err();
    assert!(e.to_string().contains("PUT /files"), "{e}");
}
