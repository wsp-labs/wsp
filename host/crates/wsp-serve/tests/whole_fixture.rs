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

// stub-backend.ts:253 `lifecycle`: a test writes the whole budgets record (wake-keeps-machine.test.ts:21) or deletes and
// sets `resumeAsks` alone (wake-asks-again.test.ts:34).
#[test]
fn the_lifecycle_budgets_are_written_whole_and_resume_asks_alone() {
    use wsp_engine::machine::Lifecycle as _;
    drive(|| async {
        let backend = StubBackend::new(None);
        let life = backend.lifecycle();
        let seen = j(&life.budgets());
        assert_eq!(seen["wakeAttempts"], 2);
        assert_eq!(seen["daemonAnswersMs"], 30_000);
        assert_eq!(seen["resumeAsks"], json!({ "everyMs": 60_000, "forMs": 1_800_000 }));
        life.set_resume_asks(Some((60.0, 120.0)));
        assert_eq!(j(&life.budgets())["resumeAsks"], json!({ "everyMs": 60, "forMs": 120 }));
        life.set_resume_asks(None);
        assert!(j(&life.budgets()).get("resumeAsks").is_none());
        life.set_budgets(wsp_engine::box_backend::plan::box_budgets());
        life.set_daemon_answers_ms(300.0);
        let box_seen = j(&life.budgets());
        assert_eq!(box_seen["daemonAnswersMs"], 300);
        assert_eq!(box_seen["wakeAttempts"], 1);
    });
}

// stub-backend.ts:511 `copyingFake`: a copy is a folder copied for real and a remove takes it away.
#[test]
fn the_copying_fake_copies_a_folder_for_real_and_a_remove_takes_it_away() {
    use wsp_engine::copier::{Copier as _, CopyAsk, CopyRoad};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let from = dir.path().join("from");
        std::fs::create_dir_all(&from).expect("a source");
        std::fs::write(from.join("a.txt"), "a").expect("a file");
        let to = dir.path().join("to");
        let copier = common::copying::copying_fake();
        let ask = CopyAsk {
            from: from.display().to_string(),
            to: to.display().to_string(),
            base: None,
            exclude: vec!["node_modules".into()],
            size_line_bytes: 0.into(),
            road: None,
        };
        let report = copier.make(&ask).await.expect("a copy");
        assert_eq!(report.path, to.display().to_string());
        assert_eq!(j(&report)["excluded"], json!(["node_modules"]));
        assert_eq!(std::fs::read_to_string(to.join("a.txt")).expect("the copy"), "a");
        copier.remove(&ask.from, &ask.to, CopyRoad::Clonefile).await.expect("a remove");
        assert!(!to.exists());
        assert_eq!(copier.log().lock().expect("the log").asks.len(), 1);
    });
}

// wake-asks-again.test.ts:49 `deafResume`: a machine's resume that is handed the caller's cancel, wrapped as
// `machine.resume.bind(machine)` wraps it, and replaced whole by a plain reassignment.
#[test]
fn a_resume_that_reads_the_callers_cancel_is_wrapped_and_a_plain_reassignment_replaces_it() {
    use wsp_engine::cancel::Cancel;
    use wsp_engine::machine::Machine as _;
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        m.pause(None).await.expect("a pause");
        assert!(m.paused.get());
        let seen = Rc::new(std::cell::Cell::new(false));
        let was = m.resume_with_fn();
        let marked = seen.clone();
        m.set_resume_with(Rc::new(move |signal| {
            marked.set(signal.is_some());
            let was = was.clone();
            Box::pin(async move { was(None).await })
        }));
        let cancel = Cancel::new();
        m.resume(Some(&cancel)).await.expect("a resume");
        assert!(seen.get());
        assert!(!m.paused.get());
        assert_eq!(m.calls.resume.get(), 1);
        m.set_resume(Rc::new(|| Box::pin(async { Err(missing("Box not found")) })));
        assert!(m.resume(Some(&cancel)).await.is_err());
        assert_eq!(m.calls.resume.get(), 2);
    });
}

// disk-watch.test.ts:36 and :213 `movable`: a backend that offers a root disk per size, a machine that offers `resumeAt`
// once a test gives it one, and a pause that keeps the `waitMs` it was handed.
#[test]
fn a_backend_offers_a_root_disk_and_a_machine_a_resume_at_only_once_a_test_gives_one() {
    use wsp_engine::machine::{Machine as _, MachineBackend as _, RootDisk, Size};
    drive(|| async {
        let backend = StubBackend::new(None);
        let size = Size { cpu: 2.0, mem_mb: 4096.0 };
        assert!(!backend.offers().root_disk_of);
        assert!(backend.root_disk_of(size).is_none());
        backend.set_root_disk_of(Some(Rc::new(|s: Size| (s.cpu == 2.0).then(|| RootDisk { name: "small".into(), disk_gb: 75.0 }))));
        assert!(backend.offers().root_disk_of);
        assert_eq!(backend.root_disk_of(size).map(|d| (d.name, d.disk_gb)), Some(("small".to_string(), 75.0)));
        assert!(backend.root_disk_of(Size { cpu: 4.0, mem_mb: 8192.0 }).is_none());

        let m = made(&backend).await;
        assert!(!m.offers().resume_at);
        assert!(m.resume_at(4.0, 8192, None).await.is_err());
        let landed = Rc::new(std::cell::RefCell::new(vec![]));
        let seen = landed.clone();
        let machine = m.clone();
        m.set_resume_at(Some(Rc::new(move |cpu, mem_mb, _| {
            seen.borrow_mut().push((cpu, mem_mb));
            machine.paused.set(false);
            Box::pin(async { Ok(()) })
        })));
        assert!(m.offers().resume_at);
        m.pause(Some(1_800_000)).await.expect("a pause");
        m.pause(None).await.expect("a pause");
        assert_eq!(*m.pause_waits.borrow(), [Some(1_800_000), None]);
        m.resume_at(4.0, 8192, None).await.expect("a resume at a size");
        assert_eq!(*landed.borrow(), [(4.0, 8192)]);
        assert!(!m.paused.get());
    });
}

// stub-backend.ts:432 `createOn` with `{ cpu, memMb }`: the size a test names reaches the machine's spec.
#[test]
fn create_on_carries_the_cpu_and_memory_a_test_names_to_the_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        create_on(&fx.rt, CreateOn::golden("web").cpu(4.0).mem(8192.0), None).await.expect("a workspace");
        let spec = backend.machine(0).spec_now();
        assert_eq!((j(&spec)["cpu"].clone(), j(&spec)["memMb"].clone()), (json!(4), json!(8192)));
        fx.rt.close().await.expect("a close");
    });
}

// idle.test.ts:244 and :596 `quietForMs` and `backstop`: a lifecycle has neither until a test gives it one.
#[test]
fn a_lifecycle_has_a_quiet_figure_and_a_backstop_only_once_a_test_gives_them() {
    use wsp_engine::machine::Lifecycle as _;
    drive(|| async {
        let backend = StubBackend::new(None);
        let life = backend.lifecycle();
        let m = made(&backend).await;
        assert!(!life.has_quiet_for_ms() && !life.has_backstop());
        assert!(life.quiet_for_ms(&*m).await.is_err());
        life.set_quiet_for_ms(Some(Rc::new(|| Box::pin(async { Ok(Some(60_000)) }))));
        assert!(life.has_quiet_for_ms());
        assert_eq!(life.quiet_for_ms(&*m).await.expect("a figure"), Some(60_000));
        let heard = Rc::new(std::cell::RefCell::new(vec![]));
        let seen = heard.clone();
        life.set_backstop(Some(Rc::new(move |machine, until| {
            seen.borrow_mut().push((machine, until));
            Box::pin(async { Ok(()) })
        })));
        assert!(life.has_backstop());
        life.backstop(&*m, 1_000, false).await.expect("a backstop");
        assert_eq!(*heard.borrow(), [("m1".to_string(), 1_000)]);
        life.set_backstop(None);
        assert!(!life.has_backstop());
    });
}

// runtime-reach.test.ts:542 `daemonFrame`: a machine whose computer serves its daemon answers a frame and offers that road.
#[test]
fn a_machine_serves_its_daemon_over_its_own_road_only_once_a_test_says_so() {
    use wsp_engine::machine::Machine as _;
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        assert!(!m.offers().daemon_frame);
        assert!(m.daemon_frame(serde_json::Map::new()).await.is_err());
        m.set_served(true);
        assert!(m.offers().daemon_frame);
        assert_eq!(serde_json::Value::Object(m.daemon_frame(serde_json::Map::new()).await.expect("an answer")), json!({ "ok": true }));
    });
}

// runtime-reach.test.ts:401 `daemonPort`, and the route a test hands a machine's `previewUrl`.
#[test]
fn the_daemon_port_answers_502_then_426_and_a_route_leads_a_machine_to_it() {
    use common::reach::{daemon_port, route_through, token_written};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    drive(|| async {
        let up = Rc::new(std::cell::Cell::new(false));
        let edge = daemon_port({
            let up = up.clone();
            move || up.get()
        })
        .await;
        let status = |port: u16| async move {
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.expect("a socket");
            stream.write_all(b"GET / HTTP/1.1\r\nhost: x\r\n\r\n").await.expect("a request");
            let mut said = String::new();
            let _ = stream.read_to_string(&mut said).await;
            said.split_whitespace().nth(1).unwrap_or_default().to_string()
        };
        assert_eq!(status(edge.port).await, "502");
        up.set(true);
        assert_eq!(status(edge.port).await, "426");
        assert_eq!(edge.hits(), 2);

        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        route_through(&m, edge.port, None);
        let reach = m.preview_url(7070).await.expect("a route");
        assert_eq!(reach.url, format!("http://127.0.0.1:{}/?port=7070", edge.port));
        assert_eq!(reach.token, "e");
        assert_eq!(token_written("a\nWSP_DAEMON_TOKEN='abc'\nb").as_deref(), Some("abc"));
        assert_eq!(token_written("WSP_DAEMON_TOKEN='a'b'"), None);
    });
}

// runtime-reach.test.ts:164 `deployDaemon: async () => daemon.announce(v)`: a deploy holds the announce past the daemon's borrow.
#[test]
fn a_deploy_holds_the_daemons_announce_as_a_closure() {
    use futures::{SinkExt as _, StreamExt as _};
    use tokio_tungstenite::tungstenite::Message;
    drive(|| async {
        let daemon = helloing_daemon(1, false, None).await;
        let announce = daemon.announcer();
        announce(42);
        let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}", daemon.port)).await.expect("a socket");
        ws.send(Message::text(json!({ "id": 1, "op": "auth", "token": "t" }).to_string())).await.expect("a send");
        let _ok = ws.next().await.expect("a frame").expect("ok");
        let hello: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(hello["version"], 42);
    });
}

// box-nap.test.ts:49 `slowDaemon` and :78 `lateDaemon`: one holds its answer to the auth frame until `up()`, the other
// refuses every upgrade with a 502 until `up()`.
#[test]
fn the_slow_daemon_holds_its_auth_answer_and_the_late_daemon_refuses_upgrades_until_up() {
    use common::box_daemons::{late_daemon, slow_daemon};
    use futures::{SinkExt as _, StreamExt as _};
    use tokio_tungstenite::tungstenite::Message;
    drive(|| async {
        let slow = slow_daemon().await;
        let (mut ws, _) = tokio_tungstenite::connect_async(slow.url()).await.expect("a socket");
        ws.send(Message::text(json!({ "id": 1, "op": "auth", "token": "t" }).to_string())).await.expect("a send");
        assert!(tokio::time::timeout(std::time::Duration::from_millis(100), ws.next()).await.is_err());
        ws.send(Message::text(json!({ "id": 2, "op": "ping" }).to_string())).await.expect("a send");
        let pong: serde_json::Value = serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(pong, json!({ "id": 2, "ok": true }));
        slow.up();
        let auth: serde_json::Value = serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(auth, json!({ "id": 1, "ok": true }));

        let late = late_daemon().await;
        let refused = tokio_tungstenite::connect_async(late.url()).await;
        assert!(refused.is_err(), "the edge answers a 502 before it is up");
        assert_eq!(late.dials(), 1);
        late.up();
        let (mut ws, _) = tokio_tungstenite::connect_async(late.url()).await.expect("a socket");
        ws.send(Message::text(json!({ "id": 3, "op": "auth", "token": "t" }).to_string())).await.expect("a send");
        let ok: serde_json::Value = serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(ok, json!({ "id": 3, "ok": true }));
        assert_eq!(late.dials(), 2);
    });
}

// box-nap.test.ts:278 `m.startDaemon` and :381 `m.daemonExits`: a machine starts or looks at its daemon's unit only once a
// test gives it the road; and `void promise` starts a future at once, as a JS call runs its synchronous half.
#[test]
fn a_machine_starts_its_daemon_and_reads_its_exit_only_once_a_test_gives_it_the_roads_and_fire_starts_a_future_at_once() {
    use wsp_engine::machine::Machine as _;
    drive(|| async {
        let backend = StubBackend::new(None);
        let m = made(&backend).await;
        assert!(!m.offers().start_daemon && !m.offers().daemon_exits);
        assert!(m.start_daemon().await.is_err() && m.daemon_exits().await.is_err());
        m.set_start_daemon(Some(Rc::new(|| Box::pin(async { Ok(exec_ok("started")) }))));
        m.set_daemon_exits(Some(Rc::new(|| Box::pin(async { Ok(Some("it exits".to_string())) }))));
        assert!(m.offers().start_daemon && m.offers().daemon_exits);
        assert_eq!(m.start_daemon().await.expect("a start").stdout, "started");
        assert_eq!(m.daemon_exits().await.expect("a look").as_deref(), Some("it exits"));

        let ran = Rc::new(std::cell::Cell::new(false));
        let seen = ran.clone();
        fire(async move {
            seen.set(true);
            sleep_ms(1).await;
        });
        assert!(ran.get(), "the part before the first wait ran inside the call");
    });
}

// box-nap.test.ts:687 `backend.get = async id => (id === ws.machineId ? box.machine : get(id))`: a read of one id answers a
// machine the stub did not make, and every other read is the stub's own.
#[test]
fn a_read_of_one_id_can_answer_a_machine_the_stub_did_not_make() {
    use wsp_engine::machine::{Machine as _, MachineBackend as _};
    drive(|| async {
        let backend = StubBackend::new(None);
        let a = made(&backend).await;
        let b = made(&backend).await;
        backend.answer_get_with(a.id(), b.clone());
        assert_eq!(backend.get(a.id()).await.expect("a machine").id(), b.id());
        assert_eq!(backend.get(b.id()).await.expect("a machine").id(), b.id());
        assert_eq!(backend.calls.get.get(), 2);
    });
}

// box-nap.test.ts:583 `restoringBox`: the real Boat machine over a fake gateway refuses commands while it restores, reads its
// state as it is told, counts the proofs it is asked and can hold a read.
#[test]
fn the_restoring_box_refuses_commands_until_its_clock_reaches_the_restore_and_the_machine_counts_its_proofs() {
    use common::box_fake::restoring_box;
    use wsp_engine::machine::{Machine as _, MachineState};
    drive(|| async {
        let state = Rc::new(std::cell::RefCell::new("ready".to_string()));
        let reads = state.clone();
        let b = restoring_box("bx_1", move || reads.borrow().clone(), 30.0, || {});
        assert_eq!(b.machine.state().await.expect("a state"), MachineState::Running);
        *state.borrow_mut() = "archived".into();
        assert_eq!(b.machine.state().await.expect("a state"), MachineState::Paused);
        *state.borrow_mut() = "ready".into();
        b.machine.prove_road(None).await.expect("a proof once restored");
        assert_eq!(b.machine.proofs.get(), 1);
        assert!(b.state.commands.borrow().contains(&true), "the box answered sandbox_restoring while its clock ran");
        assert_eq!(b.state.last_command_restoring(), Some(false));
        let held = b.state.hold_next();
        let proving = tokio::task::spawn_local({
            let machine = b.machine.clone();
            async move { machine.prove_road(None).await }
        });
        sleep_ms(50).await;
        assert!(!proving.is_finished());
        held.open();
        proving.await.expect("a task").expect("a proof");
        assert_eq!(b.machine.proofs.get(), 2);
        b.state.deleted.set(true);
        assert!(b.machine.prove_road(None).await.is_err());
    });
}
