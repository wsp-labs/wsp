// SPDX-License-Identifier: AGPL-3.0-only
//! The whole-runtime fixture's own tests (#2476 join B): each proves the fixture answers as the TS `stubBackend` and
//! its helpers do (packages/runtime/test/stub-backend.ts, runtime-fixture.ts, held-port.ts, split-gateway.ts at
//! 65f6b89c7), with the TS line each one reads.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::stub_backend::{say_replayed, Put};
use common::*;
use wsp_core::types::api::StartOptions;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{ExecOptions, MachineLife, MachineSpec, MachineState, SnapshotOptions};

fn spec(doc: serde_json::Value) -> MachineSpec {
    serde_json::from_value(doc).expect("a machine spec")
}

async fn made(backend: &Arc<StubBackend>) -> Arc<StubMachine> {
    backend.make(spec(json!({ "kind": "sandbox" })), None, None).await.expect("a machine")
}

fn missing_404(e: &EngineError) -> bool {
    matches!(e, EngineError::Refused { kind: Some(wsp_engine::errors::ErrorKind::Missing), status: Some(404), .. })
}

// stub-backend.ts:270: a machine is `m<n>` in the order made, keeps its spec, and carries labels only where the spec names them.
#[test]
fn machines_are_named_in_the_order_they_are_made_and_carry_labels_only_where_the_spec_names_them() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let a = made(&backend).await;
        let b = backend
            .make(spec(json!({ "kind": "desktop", "labels": { "wsp": "1" }, "cpu": 4, "memMb": 8192 })), None, None)
            .await
            .expect("a machine");
        assert_eq!((a.id(), b.id()), ("m1", "m2"));
        assert!(a.labels().is_none());
        assert_eq!(j(&b.labels().expect("labels")), json!({ "wsp": "1" }));
        assert_eq!(a.stream_url(), None);
        assert_eq!(b.stream_url(), Some("wss://stub/stream/m2"));
        assert_eq!(j(&b.describe().await.expect("a shape"))["cpu"], 4);
        assert_eq!(j(&a.describe().await.expect("a shape"))["memMb"], 4096);
        assert_eq!(backend.made(), 2);
    });
}

// stub-backend.ts:290-301: exec and run log what the guest was given, and a killed machine answers `gone` as a 404.
#[test]
fn exec_and_run_are_logged_and_a_killed_machine_answers_gone() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        let said = m.exec("echo hi", ExecOptions::default()).await.expect("an exec");
        assert_eq!(said.stdout, "");
        let probe = m.exec("echo WSP_CTX", ExecOptions::default()).await.expect("an exec");
        assert_eq!(probe.stdout, "WSP_CTX\nWSP_CTX_END\n");
        let branch = m.exec("git rev-parse --abbrev-ref HEAD", ExecOptions::default()).await.expect("an exec");
        assert_eq!(branch.stdout, "work\norigin/work\n");
        m.run("make", wsp_engine::machine::RunOptions::default()).await.expect("a run");
        assert_eq!(m.exec_log(), ["echo hi", "echo WSP_CTX", "git rev-parse --abbrev-ref HEAD", "make"]);
        assert_eq!(m.run_log(), ["make"]);
        m.kill().await.expect("a kill");
        let refused = m.exec("echo again", ExecOptions::default()).await.expect_err("a refusal");
        assert!(missing_404(&refused) && refused.to_string() == "gone", "{refused}");
        assert!(backend.get("m1").await.err().is_some_and(|e| missing_404(&e)));
        assert_eq!(m.exec_log().len(), 4);
    });
}

// stub-backend.ts:302-317: snapshot ids, a repeated name, a named backend, the mark, and the life a snapshot may come from.
#[test]
fn snapshots_mint_ids_by_name_count_repeats_carry_the_mark_and_refuse_a_resumed_machine() {
    drive(|| async {
        let backend = StubBackend::new(Some("a"));
        let m = made(&backend).await;
        let first = MachineLife { first_life: true };
        assert_eq!(m.snapshot("img", first.clone(), SnapshotOptions::default()).await.expect("a snapshot"), "snap_a-img");
        assert_eq!(m.snapshot("img", first.clone(), SnapshotOptions::default()).await.expect("a snapshot"), "snap_a-img-2");
        assert_eq!(backend.snapshots.borrow().len(), 2);
        assert_eq!(j(&backend.snapshots.borrow()[0])["sizeBytes"], 8_000_000_000u64);
        let resumed = m.snapshot("late", MachineLife { first_life: false }, SnapshotOptions::default()).await.expect_err("a refusal");
        assert_eq!(
            resumed.to_string(),
            "snapshot late refused: machine m1 is not first-life (it was resumed); snapshots only come from fresh machines"
        );
        backend.named_snapshots.set(true);
        assert_eq!(m.snapshot("img", first, SnapshotOptions::default()).await.expect("a snapshot"), "img");
        let plain = StubBackend::new(None);
        let p = made(&plain).await;
        assert_eq!(p.snapshot("x", MachineLife { first_life: true }, SnapshotOptions::default()).await.expect("a snapshot"), "snap_x");
        assert_eq!(m.snapshot_lives.borrow().len(), 4);
    });
}

// stub-backend.ts:365-372, 376-394: Solari's 409s on a snapshot a fork or a template stands on, and a template a create names.
#[test]
fn a_snapshot_with_children_or_a_template_is_refused_and_an_unready_template_cannot_be_forked() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        m.snapshot("img", MachineLife { first_life: true }, SnapshotOptions::default()).await.expect("a snapshot");
        backend.make(spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_img" })), None, None).await.expect("a fork");
        let kept = backend.delete_snapshot("snap_img").await.expect_err("a refusal");
        assert_eq!(kept.to_string(), "SnapshotHasChildren");
        assert_eq!(kept.status(), Some(409));
        backend.machine(1).kill().await.expect("a kill");
        let tpl = backend.promote_snapshot("snap_img", "good").await.expect("a template");
        assert_eq!(tpl, "tpl_good");
        assert_eq!(backend.delete_snapshot("snap_img").await.expect_err("a refusal").to_string(), "SnapshotBacksTemplate");
        backend.delete_template(&tpl).await.expect("a delete");
        backend.delete_snapshot("snap_img").await.expect("a delete");
        assert!(backend.snapshots.borrow().is_empty());
        let unready = backend.make(spec(json!({ "kind": "sandbox", "template": "nope" })), None, None).await.expect_err("a refusal");
        assert!(missing_404(&unready) && unready.to_string() == "TemplateNotReady nope");
        backend.make(spec(json!({ "kind": "sandbox", "template": "base" })), None, None).await.expect("a builtin template");
        assert!(backend.promote_snapshot("snap_gone", "x").await.err().is_some_and(|e| missing_404(&e)));
    });
}

// stub-backend.ts:318-346: pause, resume and state follow each other; the provider's metrics 404 for a machine the host lost
// while the state read still says running.
#[test]
fn pause_resume_state_and_metrics_answer_as_the_stub_does() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        assert_eq!(m.state().await.expect("a state"), MachineState::Running);
        m.pause(None).await.expect("a pause");
        assert_eq!(m.state().await.expect("a state"), MachineState::Paused);
        assert_eq!(backend.list(None).await.expect("a list")[0].state, MachineState::Paused);
        m.resume(None).await.expect("a resume");
        assert_eq!((m.resumes.get(), m.state().await.expect("a state")), (1, MachineState::Running));
        m.metrics().await.expect("metrics");
        m.host_lost.set(true);
        assert!(m.metrics().await.err().is_some_and(|e| missing_404(&e) && e.to_string() == "host no longer knows this VM"));
        assert_eq!(m.state().await.expect("a state"), MachineState::Running);
        m.kill().await.expect("a kill");
        assert_eq!(m.state().await.expect("a state"), MachineState::Gone);
        assert!(m.resume(None).await.err().is_some_and(|e| missing_404(&e)));
        assert!(backend.list(None).await.expect("a list").is_empty());
        assert_eq!((m.calls.pause.get(), m.calls.resume.get(), m.calls.state.get()), (1, 2, 5));
    });
}

// stub-backend.ts:341-346, 194-217: the upload and download URLs the stub mints are served by a loopback server that
// records every PUT and serves the empty archive, or what `downloads` gives for the path.
#[test]
fn the_vault_urls_take_a_put_and_serve_the_empty_archive_or_what_downloads_gives() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        let up = m.upload_url("/root/a b.tgz").await.expect("a url");
        assert!(up.starts_with("http://127.0.0.1:") && up.ends_with("/upload?machine=m1&path=%2Froot%2Fa%20b.tgz"), "{up}");
        let client = reqwest_free_put(&up, b"vault bytes").await;
        assert!(client.starts_with("HTTP/1.1 200"), "{client}");
        assert_eq!(backend.puts(), [Put { machine: "m1".into(), path: "/root/a b.tgz".into(), body: b"vault bytes".to_vec() }]);
        let down = m.download_url("/root/x.tgz").await.expect("a url");
        assert!(down.ends_with("/download/root/x.tgz"), "{down}");
        assert_eq!(http_get(&down).await, common::vault::EMPTY_TGZ.to_vec());
        backend.set_downloads(Rc::new(|path| path.as_bytes().to_vec()));
        assert_eq!(http_get(&down).await, b"/root/x.tgz".to_vec());
    });
}

async fn raw_http(url: &str, request: impl FnOnce(&str, &str) -> Vec<u8>) -> Vec<u8> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rest = url.strip_prefix("http://").expect("an http url");
    let (host, path) = rest.split_once('/').map_or((rest, "/".to_string()), |(h, p)| (h, format!("/{p}")));
    let mut stream = tokio::net::TcpStream::connect(host).await.expect("a connection");
    stream.write_all(&request(host, &path)).await.expect("a request");
    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.expect("a reply");
    out
}

async fn reqwest_free_put(url: &str, body: &[u8]) -> String {
    let body = body.to_vec();
    let reply = raw_http(url, move |host, path| {
        let mut req = format!("PUT {path} HTTP/1.1\r\nhost: {host}\r\ncontent-length: {}\r\n\r\n", body.len()).into_bytes();
        req.extend_from_slice(&body);
        req
    })
    .await;
    String::from_utf8_lossy(&reply).into_owned()
}

async fn http_get(url: &str) -> Vec<u8> {
    let reply = raw_http(url, |host, path| format!("GET {path} HTTP/1.1\r\nhost: {host}\r\n\r\n").into_bytes()).await;
    let at = reply.windows(4).position(|w| w == b"\r\n\r\n").expect("a head") + 4;
    reply[at..].to_vec()
}

// stub-backend.ts:266-269: a command on the computer is logged apart from every machine and answered by the same execImpl,
// through a machine that stands for the computer and is in no listing.
#[test]
fn a_command_on_the_computer_is_logged_apart_and_answered_by_the_same_exec_impl() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(exec_fn(|m, cmd| Ok(exec_ok(&format!("{}:{cmd}", m.id())))));
        let said = backend.on_computer("uname", None).await.expect("a command");
        assert_eq!(said.stdout, "computer:uname");
        assert_eq!(backend.computer_log(), ["uname"]);
        assert_eq!(backend.made(), 0);
        assert!(backend.list(None).await.expect("a list").is_empty());
    });
}

// stub-backend.ts:77-88 `withDaemonRoads`: every machine has a daemon road from the moment it exists and a token write lands.
#[test]
fn with_daemon_roads_gives_every_machine_a_route_an_answer_and_a_token_write() {
    drive(|| async {
        let backend = StubBackend::new(None).with_daemon_roads();
        backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.expect("a machine");
        let m = backend.machine(0);
        let reach = m.preview_url(7070).await.expect("a route");
        assert_eq!((reach.url.as_str(), reach.token.as_str()), ("http://127.0.0.1:9", "e"));
        assert!(m.daemon_answers(None).await.expect("an answer"));
        let wrote = m
            .exec(&format!("printf x > {}", wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH), ExecOptions::default())
            .await
            .expect("a write");
        assert_eq!(wrote.stdout, "WSP_DAEMON_TOKEN_SET\n");
        assert!(m.offers().preview_url && m.offers().daemon_answers);
        let bare = made(&StubBackend::new(None)).await;
        assert!(!bare.offers().preview_url && !bare.offers().daemon_answers);
    });
}

// stub-backend.ts:92-100: `guestBranchAnswer` and `tokenGuest`.
#[test]
fn the_scripted_guests_answer_the_branch_read_and_the_token_write() {
    assert_eq!(
        common::stub_backend::guest_branch_answer("git rev-parse --abbrev-ref HEAD").expect("an answer").stdout,
        "work\norigin/work\n"
    );
    assert!(common::stub_backend::guest_branch_answer("ls").is_none());
    assert_eq!(token_guest_answer("cat /root/.wsp/daemon-token").stdout, "WSP_DAEMON_TOKEN_SET\n");
    assert_eq!(token_guest_answer("git rev-parse --abbrev-ref HEAD").stdout, "work\norigin/work\n");
    assert_eq!(token_guest_answer("anything").stdout, "");
    let df = common::images::df_ok_answer("df -Pk /");
    assert_eq!(df.stdout, "3072000\n");
    assert_eq!(common::images::df_ok_answer("echo ok").stdout, "ok\n");
}

// stub-backend.ts:126-147: the gateway's copy that never held the machine, in its two ways.
#[test]
fn a_gateway_copy_that_never_held_the_machine_misses_the_first_delete_and_answers_gone_once() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        m.misses_first_delete();
        m.kill().await.expect("a kill");
        assert!(!m.killed.get());
        m.kill().await.expect("a kill");
        assert!(m.killed.get());

        let other = made(&backend).await;
        backend.answers_gone_once(&other, Rc::new(|| true));
        assert!(backend.get("m2").await.err().is_some_and(|e| missing_404(&e)));
        assert_eq!(backend.get("m2").await.expect("held").id(), "m2");
        let third = made(&backend).await;
        backend.answers_gone_once(&third, Rc::new(|| true));
        assert_eq!(third.state().await.expect("a state"), MachineState::Gone);
        assert_eq!(third.state().await.expect("a state"), MachineState::Running);
    });
}

// stub-backend.ts:262: a create may be wrapped by the test as `backend.create = ...`, and a wrap sees the one before it.
#[test]
fn a_create_the_test_wraps_sees_the_one_before_it_and_the_notice_rides_the_handle() {
    drive(|| async {
        let backend = StubBackend::new(None);
        *backend.create_notice.borrow_mut() = Some("a smaller box".into());
        let before = backend.create_fn();
        backend.set_create(Rc::new(move |mut spec, waiting, sent| {
            say_replayed(&waiting, "m0");
            spec.labels = Some([("by".to_string(), "test".to_string())].into_iter().collect());
            before(spec, waiting, sent)
        }));
        let m = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.expect("a machine");
        assert_eq!(m.notice(), Some("a smaller box"));
        assert_eq!(j(&m.labels().expect("labels")), json!({ "by": "test" }));
        assert_eq!(backend.calls.create.get(), 1);
    });
}

// held-port.ts:15: a held port drops every connection unanswered.
#[test]
fn a_dropping_port_drops_every_connection_and_an_edge_holds_a_request_until_the_test_answers() {
    drive(|| async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let port = dropping_port().await;
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port.port)).await.expect("a connection");
        let _ = stream.write_all(b"GET / HTTP/1.1\r\n\r\n").await;
        let mut buf = Vec::new();
        let _ = stream.read_to_end(&mut buf).await;
        assert!(buf.is_empty());

        let edge = Edge::start().await;
        let url = format!("http://127.0.0.1:{}/?port=1", edge.port);
        let asked = tokio::task::spawn_local(async move {
            raw_http(&url, |host, path| format!("GET {path} HTTP/1.1\r\nhost: {host}\r\n\r\n").into_bytes()).await
        });
        until(|| {
            let held = edge.held();
            async move { held == 1 }
        })
        .await;
        edge.answer(edge.take().expect("a held request")).await;
        assert!(String::from_utf8_lossy(&asked.await.expect("the task")).starts_with("HTTP/1.1 404"));
    });
}

// runtime-fixture.ts:23 `helloingDaemon` and :87 `withDaemon`.
#[test]
fn the_helloing_daemon_answers_every_op_announces_its_version_after_auth_and_answers_a_plain_get_with_426() {
    drive(|| async {
        use futures::{SinkExt as _, StreamExt as _};
        use tokio_tungstenite::tungstenite::Message;
        let daemon = helloing_daemon(17, false, None).await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}", daemon.port)).await.expect("a socket");
        ws.send(Message::text(json!({ "id": 1, "op": "auth", "token": "t" }).to_string())).await.expect("a send");
        let reply: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(reply, json!({ "id": 1, "ok": true }));
        let hello: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(hello, json!({ "type": "daemon.hello", "root": "/root", "version": 17 }));
        assert_eq!(daemon.dials(), 1);
        let plain = raw_http(&format!("http://127.0.0.1:{}/", daemon.port), |host, path| {
            format!("GET {path} HTTP/1.1\r\nhost: {host}\r\n\r\n").into_bytes()
        })
        .await;
        assert!(String::from_utf8_lossy(&plain).starts_with("HTTP/1.1 426"));
        assert_eq!(daemon.hits(), 1);

        let held = helloing_daemon(3, true, Some("right".into())).await;
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}", held.port)).await.expect("a socket");
        ws.send(Message::text(json!({ "id": 2, "op": "auth", "token": "wrong" }).to_string())).await.expect("a send");
        let refused: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(refused["ok"], false);
        ws.send(Message::text(json!({ "id": 3, "op": "auth", "token": "right" }).to_string())).await.expect("a send");
        let ok: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(ok["ok"], true);
        held.announce(9);
        held.release();
        let hello: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(hello["version"], 9);
    });
}

// split-gateway.ts: a create lands on the holder; a read lands on whichever copy `route` names; the empty copy takes a
// delete with the same 200 and does nothing.
#[test]
fn the_split_gateway_answers_a_read_from_the_copy_the_route_names_and_a_delete_only_ends_it_on_the_holder() {
    drive(|| async {
        use wsp_engine::fetch::FetchRequest;
        let gw = common::gateway::SplitGateway::new(|n, method| {
            if method == "GET" && n == 0 {
                common::gateway::Side::Empty
            } else {
                common::gateway::Side::Holder
            }
        });
        let f = gw.fetch();
        let ask =
            |method: &str, url: &str| FetchRequest { method: method.into(), url: url.into(), headers: vec![], body: None, abort: None };
        let made = f.fetch(ask("POST", "https://gw/sandboxes")).await.expect("a create");
        assert_eq!(
            (made.status, String::from_utf8_lossy(&made.body).into_owned()),
            (201, "{\"sandboxId\":\"sb1\",\"kind\":\"sandbox\"}".to_string())
        );
        assert_eq!(f.fetch(ask("GET", "https://gw/sandboxes/sb1")).await.expect("a read").status, 404);
        let held = f.fetch(ask("GET", "https://gw/sandboxes/sb1")).await.expect("a read");
        assert_eq!(
            (held.status, String::from_utf8_lossy(&held.body).into_owned()),
            (200, "{\"sandboxId\":\"sb1\",\"kind\":\"sandbox\",\"state\":\"running\"}".to_string())
        );
        assert_eq!(f.fetch(ask("DELETE", "https://gw/sandboxes/sb1")).await.expect("a delete").status, 200);
        assert!(gw.holder.borrow().is_empty());
        assert_eq!((gw.deletes(), gw.gets()), (1, 2));
    });
}

// stub-backend.ts:445 `ownedStore`, 405-411 `projectOn` and 432 `createOn`: a workspace of a fresh project on the
// computer a test never names, and a project on this computer that is a real git folder.
#[test]
fn create_on_makes_a_fresh_project_each_call_and_a_project_here_is_a_real_repo() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let a = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let b = create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace");
        assert_ne!(j(&a.view)["project"], j(&b.view)["project"]);
        assert_eq!(backend.made(), 2);
        let repo = temp_repo();
        assert!(repo.join(".git").is_dir());
        let owned = owned_store("s1").await;
        assert_eq!(owned.get("owner", "id").await.expect("a read"), Some(json!({ "id": "s1" })));
    });
}

// runtime-fixture.ts:109 `held`: a turn runs until the test ends it; a resumed start keeps the session id.
#[test]
fn a_held_turn_runs_until_the_test_ends_it_and_the_fixture_runtime_runs_on_the_sqlite_store_of_a_temp_folder() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let held = Held::new(false);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", held.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("w"), None).await.expect("a workspace");
        let run =
            fx.rt.sessions().start(&ws.view.id, StartOptions { prompt: "work".into(), ..Default::default() }, None).await.expect("a start");
        until(|| {
            let held = held.clone();
            async move { held.turns() == 1 }
        })
        .await;
        assert_eq!(held.starts.borrow()[0].prompt, "work");
        held.end(0, "all done", json!({}));
        let result = run.finished().await;
        assert_eq!(j(&result)["text"], "all done");
        assert_eq!(held.envs.borrow().len(), 1);
        let dir = state.dir.path().to_path_buf();
        assert!(std::fs::read_dir(&dir).expect("the folder").next().is_some(), "the sqlite store wrote under {}", dir.display());
        fx.rt.close().await.expect("a close");
    });
}

// fake-clock.ts, until.ts, the gate a test settles by hand.
#[test]
fn a_gate_holds_a_waiter_until_it_is_opened_and_until_polls_a_condition() {
    drive(|| async {
        let gate = Gate::new();
        let waiter = gate.clone();
        let waited = tokio::task::spawn_local(async move { waiter.wait().await });
        sleep_ms(30).await;
        assert!(!waited.is_finished());
        gate.open();
        waited.await.expect("the waiter");
        gate.wait().await;
        let ticks = Rc::new(std::cell::Cell::new(0));
        let counted = ticks.clone();
        until(|| {
            counted.set(counted.get() + 1);
            let n = counted.get();
            async move { n >= 3 }
        })
        .await;
        assert_eq!(ticks.get(), 3);
    });
}

// runtime-steer.test.ts:191 `steerable` and steer-images.test.ts:15 `harness`: a hand-driven harness keeps what each start and
// steer was handed, answers a steer as told, and says the events the test gives it.
#[test]
fn a_live_harness_keeps_its_starts_and_steers_answers_as_told_and_says_what_the_test_emits() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let live = Live::new(LiveOptions { steers: true, announces: false, session: Some("66666666-6666-4666-8666-666666666666".into()), ..Default::default() });
        let fx = runtime(&backend).adapter("claude", live.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let run = fx
            .rt
            .sessions()
            .start(&ws.view.id, StartOptions { prompt: "go".into(), request_id: Some("req_1".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        assert_eq!(live.turns(), 1);
        live.init(0);
        let steer = wsp_core::types::api::SteerAsk { prompt: "and say pineapple".into(), request_id: Some("req_2".into()), attachments: None };
        let taken = fx.rt.sessions().steer(&run.id(), steer.clone(), None).await.expect("a steer");
        assert_eq!(j(&taken), json!({ "outcome": "accepted" }));
        live.answer.set(false);
        let missed = fx.rt.sessions().steer(&run.id(), steer, None).await.expect("a steer");
        assert_eq!(j(&missed)["outcome"], "not-running");
        assert_eq!(live.steered.borrow().iter().map(|s| s.prompt.as_str()).collect::<Vec<_>>(), ["and say pineapple", "and say pineapple"]);
        assert_eq!(live.starts.borrow()[0].prompt, "go");
        live.end(0, "ok");
        assert_eq!(j(&run.finished().await)["text"], "ok");
        fx.rt.close().await.expect("a close");
    });
}

// bring-back.test.ts:18 `fakeDaemon`: every frame is kept with the token of its dial, an op is answered as the test says
// or as the default does, the dials and closes are counted, and a push reaches every open channel.
#[test]
fn a_recording_daemon_keeps_frames_with_their_dial_answers_per_op_counts_closes_and_pushes() {
    use wsp_core::daemon_channel::DaemonChannelOptions;
    drive(|| async {
        let daemon = RecDaemon::new(|_| json!({ "ok": true, "default": true }));
        daemon.answer("git.pr", |f| json!({ "ok": false, "error": format!("no pr for {}", f["cwd"].as_str().unwrap_or_default()) }));
        daemon.hello.set(Some(7));
        let heard: Rc<std::cell::RefCell<Vec<serde_json::Value>>> = Rc::default();
        let sink = heard.clone();
        let open = daemon.open();
        let channel = open(DaemonChannelOptions {
            url: "http://127.0.0.1:9".into(),
            token: "tok".into(),
            on_event: Rc::new(move |e| sink.borrow_mut().push(e)),
            connect_timeout_ms: None,
            clock: None,
        })
        .await
        .expect("a channel");
        let frame = |doc: serde_json::Value| doc.as_object().cloned().expect("an object");
        let pushed = channel.send(frame(json!({ "op": "git.push", "cwd": "/a" }))).await.expect("a reply");
        assert_eq!(serde_json::Value::Object(pushed), json!({ "id": 1, "ok": true, "default": true }));
        let pr = channel.send(frame(json!({ "op": "git.pr", "cwd": "/a" }))).await.expect("a reply");
        assert_eq!(serde_json::Value::Object(pr), json!({ "id": 1, "ok": false, "error": "no pr for /a" }));
        assert_eq!(daemon.ops(), ["git.push", "git.pr"]);
        assert_eq!(*daemon.tokens.borrow(), ["tok", "tok"]);
        assert_eq!(*daemon.dials.borrow(), [("http://127.0.0.1:9".to_string(), "tok".to_string())]);
        sleep_ms(10).await;
        daemon.push(json!({ "type": "pty.exit", "ptyId": "p1" }));
        assert_eq!(*heard.borrow(), [json!({ "type": "daemon.hello", "version": 7 }), json!({ "type": "pty.exit", "ptyId": "p1" })]);
        channel.close();
        daemon.push(json!({ "type": "after close" }));
        assert_eq!(heard.borrow().len(), 2);
        assert_eq!(daemon.closed.get(), 1);
    });
}

// stub-backend.ts:399 `projectOn(rt, computer, source, { base })`: a project that names the base its pull requests open against.
#[test]
fn project_with_names_the_base_a_project_opens_its_pull_requests_against() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let named = project_with(&fx.rt, None, None, None, Some("main")).await.expect("a project");
        let plain = project_on(&fx.rt, None, None, None).await.expect("a project");
        assert_eq!(j(&named)["base"], "main");
        assert!(j(&plain).get("base").is_none());
    });
}

// stub-backend.ts:437 `createOn(rt, { agents, ... })`: the keys of `agents` a test names reach the create.
#[test]
fn create_on_hands_the_agents_to_the_create() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let made = create_on(&fx.rt, CreateOn::golden("a").agents(2, 3), None)
            .await
            .expect("a workspace");
        let view = j(&made.view);
        assert_eq!(view["agents"]["maxMachines"], 2);
        assert_eq!(view["agents"]["maxDepth"], 3);
    });
}

// rewind.test.ts:53 `fakeDaemon({ unreached })` and the `open` that rejects with a door's refusal: a dial refused, a send that
// never reaches the daemon and so keeps no frame.
#[test]
fn a_recording_daemon_refuses_dials_and_fails_sends_where_a_test_says() {
    use wsp_core::daemon_channel::{ChannelError, DaemonChannelOptions, DaemonDoorError};
    drive(|| async {
        let daemon = RecDaemon::new(|_| json!({ "ok": true }));
        let door = Rc::new(std::cell::Cell::new(true));
        let shut = door.clone();
        daemon.refusing_dials(move || {
            shut.get().then(|| {
                let mut door = wsp_words::exit::HostError::with_kind(DaemonDoorError::new(302, "").message, wsp_words::exit::Kind::Refused);
                door.status = Some(302.0);
                door
            })
        });
        let dial = || DaemonChannelOptions {
            url: "http://127.0.0.1:9".into(),
            token: "tok".into(),
            on_event: Rc::new(|_| {}),
            connect_timeout_ms: None,
            clock: None,
        };
        let open = daemon.open();
        let refused = open(dial()).await.err().expect("a refusal");
        assert_eq!(refused.message, "the connection was refused with 302");
        assert_eq!(refused.status, Some(302.0));
        assert!(daemon.dials.borrow().is_empty());
        door.set(false);
        let channel = open(dial()).await.expect("a channel");
        let down = Rc::new(std::cell::Cell::new(true));
        let stopped = down.clone();
        daemon.failing_sends(move || stopped.get().then(|| ChannelError::Refused { message: "Sandbox machine is not running.".into() }));
        let failed = channel.send(json!({ "op": "git.push" }).as_object().cloned().expect("an object")).await.err().expect("a failure");
        assert_eq!(failed.to_string(), "Sandbox machine is not running.");
        assert!(daemon.frames.borrow().is_empty());
        down.set(false);
        channel.send(json!({ "op": "git.push" }).as_object().cloned().expect("an object")).await.expect("a reply");
        assert_eq!(daemon.ops(), ["git.push"]);
    });
}

// sqlite-transcripts.test.ts:233, :341 and :462: a store that keeps the collection of each blob written, counts the reads of a
// folder's rows and of a thread's words, and fails the folder read where a test says.
#[test]
fn a_watched_store_counts_the_reads_of_its_rows_and_keeps_each_blob_collection() {
    drive(|| async {
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let store = watched.store();
        store.put_blob("vaults", "a", vec![1]).await.expect("a blob");
        assert_eq!(*watched.blobs.borrow(), ["vaults"]);
        let rows = store.transcripts().expect("rows");
        rows.all("w").expect("rows");
        rows.word_events("w", "t").expect("rows");
        assert_eq!((watched.rows.folder_reads.get(), watched.rows.word_reads.get()), (1, 1));
        watched.rows.fail_all.set(true);
        assert_eq!(rows.all("w").err().expect("a failure").message, "disk I/O error");
        assert_eq!(watched.rows.folder_reads.get(), 2);
    });
}

// worktree-sweep.test.ts:136 `host()` and folder-threads.test.ts:128 `here()`: a host stood on this computer reads a real folder,
// and `Quick` answers each turn at once with the folder it was started in.
#[test]
fn a_host_on_this_computer_runs_a_quick_turn_in_the_project_folder_and_makes_no_copy() {
    use common::local::{git_copier, local_wiring};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(dir.path()).expect("a real path");
        let repo = temp_repo();
        let (copier, log) = git_copier();
        let quick = Quick::new(QuickOptions::default());
        let backend = StubBackend::new(None);
        let local = local_wiring(&root, copier);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", quick.factory()).with(move |o| o.local = Some(local)).build();
        let project = fx
            .rt
            .projects()
            .add(wsp_core::types::api::ProjectAddAsk { source: repo.display().to_string(), ..Default::default() }, None)
            .await
            .expect("a project");
        let at = fx
            .rt
            .workspaces()
            .folder_for(wsp_core::types::api::FolderForAsk { project: Some(project.project.id.clone()), ..Default::default() }, None)
            .await
            .expect("a folder");
        let run = fx
            .rt
            .sessions()
            .start(&at.workspace.id, StartOptions { prompt: "hello".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        run.finished().await;
        assert_eq!(quick.cwds(), [Some(repo.display().to_string())]);
        assert!(log.lock().unwrap().asks.is_empty());
        assert!(log.lock().unwrap().worktrees.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// stub-script.ts `writeStub`: a script the shell can run, made executable.
#[test]
fn write_stub_makes_a_script_the_shell_can_run() {
    let dir = tempfile::tempdir().expect("a folder");
    let path = dir.path().join("codex");
    common::local::write_stub(&path, "#!/bin/sh\necho stubbed\n");
    let out = std::process::Command::new(&path).output().expect("the stub runs");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "stubbed\n");
}
