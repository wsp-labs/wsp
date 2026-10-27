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
        let pong: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(pong, json!({ "id": 2, "ok": true }));
        slow.up();
        let auth: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
        assert_eq!(auth, json!({ "id": 1, "ok": true }));

        let late = late_daemon().await;
        let refused = tokio_tungstenite::connect_async(late.url()).await;
        assert!(refused.is_err(), "the edge answers a 502 before it is up");
        assert_eq!(late.dials(), 1);
        late.up();
        let (mut ws, _) = tokio_tungstenite::connect_async(late.url()).await.expect("a socket");
        ws.send(Message::text(json!({ "id": 3, "op": "auth", "token": "t" }).to_string())).await.expect("a send");
        let ok: serde_json::Value =
            serde_json::from_str(ws.next().await.expect("a frame").expect("ok").to_text().expect("text")).expect("json");
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

// box-nap.test.ts:902 `t.store.put = ...`: the first put of a record at one phase waits until the test lets it go.
#[test]
fn a_wrapped_store_holds_the_first_put_at_a_phase_until_the_test_lets_it_go() {
    drive(|| async {
        let state = sqlite_state();
        let wrapped = WrappedStore::over(&state.store);
        let gate = wrapped.hold_first_put_at_phase("workspaces", "waking");
        let store = wrapped.store();
        store.put("workspaces", "a", json!({ "phase": "running" })).await.expect("a put");
        assert!(!wrapped.held());
        let waiting = tokio::task::spawn_local({
            let store = store.clone();
            async move { store.put("workspaces", "a", json!({ "phase": "waking" })).await }
        });
        until(|| {
            let held = wrapped.held();
            async move { held }
        })
        .await;
        assert_eq!(state.store.get("workspaces", "a").await.expect("a read").expect("a record")["phase"], "running");
        gate.open();
        waiting.await.expect("a task").expect("a put");
        assert_eq!(state.store.get("workspaces", "a").await.expect("a read").expect("a record")["phase"], "waking");
        store.put("workspaces", "a", json!({ "phase": "waking", "again": true })).await.expect("a put goes through once the hold fired");
    });
}

// session-row.test.ts:51 `drivenAdapter`: a turn the test drives line by line, ends when it says, or replies at once while `quick` holds.
#[test]
fn a_driven_turn_prints_what_the_test_emits_ends_when_the_test_ends_it_and_replies_at_once_while_quick_holds() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let driven = Driven::new();
        let fx = runtime(&backend).adapter("claude", driven.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("w"), None).await.expect("a workspace").view.id;
        let run = fx.rt.sessions().start(&ws, StartOptions { prompt: "work".into(), ..Default::default() }, None).await.expect("a start");
        until(|| {
            let driven = driven.clone();
            async move { driven.len() == 1 }
        })
        .await;
        assert_eq!(driven.last().prompt, "work");
        driven.last().calls("toolu_1", "Bash", "{}");
        driven.last().end(json!({ "text": "fine" }));
        assert_eq!(j(&run.finished().await)["text"], "fine");
        let types: Vec<String> = fx
            .rt
            .sessions()
            .history(&ws, None)
            .await
            .expect("a history")
            .iter()
            .map(|e| j(e)["type"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(types, ["session.start", "session.delta", "session.done", "session.end"]);
        driven.quick.set(true);
        let quick =
            fx.rt.sessions().start(&ws, StartOptions { prompt: "again".into(), ..Default::default() }, None).await.expect("a start");
        assert_eq!(j(&quick.finished().await)["text"], "done");
        fx.rt.close().await.expect("a close");
    });
}

// ws-client.ts `WsClient`: auth first, then request and reply, every pushed event kept; `createOverWire` makes a project and a workspace.
#[test]
fn a_ws_client_authenticates_requests_keeps_every_pushed_event_and_sees_the_socket_close() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).build();
        let served = wsp_serve::serve::serve_runtime(
            fx.rt.clone(),
            wsp_serve::serve::ServeOptions { port: Some(0), auth_token: "secret".into(), ..Default::default() },
        )
        .await
        .expect("the runtime serves");
        let c = WsClient::connect(served.port(), "secret").await;
        let sub = c.request("events.subscribe", json!({})).await;
        assert_eq!(sub["ok"], json!(true));
        assert!(sub["seq"].is_number());
        let created = create_over_wire(&c, "x", json!({ "golden": "snap_g" })).await;
        assert!(created["workspace"]["id"].is_string(), "{created}");
        let events = c.received(1).await;
        assert!(events.iter().all(|e| e["type"].is_string()));
        c.close().await;
        c.closed().await;
        served.close().await.expect("a close");
    });
}

// runtime-comeback.test.ts:30 `machineRuns`: a run lives on the machine; a host that comes back attaches by its handle and the
// log is replayed from its first line; forgetting a run is the machine having swept it.
#[test]
fn a_machine_run_outlives_the_host_is_reattached_by_its_handle_replays_its_log_and_is_gone_once_swept() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt.sessions().start(&ws, StartOptions { prompt: "build it".into(), ..Default::default() }, None).await.expect("a start");
        let (rt, w) = (rt1.rt.clone(), ws.clone());
        until(|| {
            let (rt, w) = (rt.clone(), w.clone());
            async move { rt.sessions().history(&w, None).await.expect("a history").iter().any(|e| j(e)["type"] == "session.start") }
        })
        .await;
        let run = h.handles()[0].clone();
        assert!(run.starts_with("/tmp/wsp-run/"));
        h.emit(&run, json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "reading" }));
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let rows = rt2.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        assert_eq!(rows.iter().map(|r| j(r)["status"].clone()).collect::<Vec<_>>(), [json!("running")]);
        assert_eq!(h.reopened().iter().map(|r| r.prompt.clone()).collect::<Vec<_>>(), [Some("build it".to_string())]);
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (rt, w) = (rt2.rt.clone(), ws.clone());
        until(|| {
            let (rt, w) = (rt.clone(), w.clone());
            async move { j(&rt.sessions().list(Some(w), None).await.expect("a list")[0])["status"] == "completed" }
        })
        .await;
        h.sweep(&run);
        assert!(h.handles().is_empty());
        assert_eq!(h.attempts(), 1);
        rt2.rt.close().await.expect("a close");
    });
}

// runtime-comeback.test.ts:313, :812, :861, :203: a store whose listing answers last, whose puts a test counts and whose transcripts
// do not read; a harness whose attach a test holds and counts; a machine that offers a proof of its road; a workspace with agent limits.
#[test]
fn a_wrapped_store_holds_a_listing_counts_puts_and_fails_transcript_reads_and_a_holder_counts_the_attaches_it_holds() {
    drive(|| async {
        let state = sqlite_state();
        let wrapped = WrappedStore::over(&state.store);
        let gate = Gate::new();
        wrapped.hold_list("sessions", &gate);
        let store = wrapped.store();
        store.put("sessions", "w1", json!({ "sessions": [] })).await.expect("a put");
        store.put("workspaces", "w1", json!({})).await.expect("a put");
        assert_eq!((wrapped.puts_into("sessions"), wrapped.puts_into("workspaces")), (1, 1));
        let listing = tokio::task::spawn_local({
            let store = store.clone();
            async move { store.list("sessions").await.expect("a list").len() }
        });
        sleep_ms(30).await;
        assert!(!listing.is_finished());
        gate.open();
        assert_eq!(listing.await.expect("the listing"), 1);
        assert!(store.transcripts().expect("rows").all("w1").is_ok());
        wrapped.fail_transcripts("disk said EIO");
        assert_eq!(store.transcripts().expect("rows").all("w1").err().map(|e| e.message), Some("disk said EIO".to_string()));

        // session-images.test.ts: the host dies between two writes, so the bytes of a blob never land.
        wrapped.refuse_blob_puts("attachments", "the host went down");
        assert_eq!(store.put_blob("attachments", "k", vec![1]).await.err().map(|e| e.message), Some("the host went down".to_string()));
        store.put_blob("vaults", "k", vec![1]).await.expect("a put into another collection");

        // reconcile.test.ts `refusingOnce`: the next put into a collection fails, the ones after it do not.
        wrapped.refuse_next_put("workspaces", "ENOSPC");
        assert_eq!(store.put("workspaces", "w2", json!({})).await.err().map(|e| e.message), Some("ENOSPC".to_string()));
        store.put("workspaces", "w2", json!({})).await.expect("the second put lands");
        // transcript-index.test.ts: the next write of a transcript's index fails, the next lands.
        wrapped.fail_next_index_write("disk full");
        let rows = store.transcripts().expect("rows");
        assert_eq!(rows.put_index("w1", "{}").err().map(|e| e.message), Some("disk full".to_string()));
        rows.put_index("w1", "{}").expect("the second write lands");

        let holder = Holder::at(None);
        let held = Gate::new();
        holder.hold(&held);
        assert_eq!((holder.asked.get(), holder.gate.borrow().is_some()), (0, true));
    });
}

#[test]
fn a_machine_offers_a_proof_of_its_road_once_one_is_set_and_a_workspace_takes_the_agent_limits_it_is_created_with() {
    use common::projects::AgentsDoc;
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws =
            create_on(&fx.rt, CreateOn::golden("a").agents(json!({ "maxMachines": 3, "maxDepth": 1 })), None).await.expect("a workspace");
        assert_matches(&j(&ws.view)["agents"], json!({ "maxMachines": 3, "maxDepth": 1 }));
        let m = backend.machine(0);
        assert!(!m.offers().prove_road);
        let proofs = Rc::new(std::cell::Cell::new(0));
        let counted = proofs.clone();
        m.set_prove_road(Some(Rc::new(move || {
            counted.set(counted.get() + 1);
            Box::pin(async { Ok(()) })
        })));
        assert!(m.offers().prove_road);
        m.prove_road(None).await.expect("a proof");
        assert_eq!(proofs.get(), 1);
        fx.rt.close().await.expect("a close");
    });
}

// session-held.test.ts, session-mcp.test.ts, session-search.test.ts: a scripted harness prints its lines at once, a tick later or once a gate
// opens, streams the reply in pieces, declares the MCP road when told, and records what each start was asked.
#[test]
fn a_scripted_turn_prints_its_lines_at_once_a_tick_later_or_at_a_gate_streams_pieces_and_records_its_starts() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let gate = Gate::new();
        let script =
            Scripted::new().cwd("/root/app").pieces(2).reply(|p| format!("reply to {p}")).takes_mcp().when(When::Gate(gate.clone()));
        let fx = runtime(&backend).adapter("claude", script.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("w"), None).await.expect("a workspace").view.id;
        let run = fx.rt.sessions().start(&ws, StartOptions { prompt: "work".into(), ..Default::default() }, None).await.expect("a start");
        sleep_ms(30).await;
        assert!(fx.rt.sessions().history(&ws, None).await.expect("a history").is_empty(), "nothing is printed before the gate opens");
        gate.open();
        assert_eq!(j(&run.finished().await)["text"], "reply to work");
        let history = fx.rt.sessions().history(&ws, None).await.expect("a history");
        let types: Vec<String> = history.iter().map(|e| j(e)["type"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(types, ["session.start", "session.delta", "session.delta", "session.done", "session.end"]);
        assert_eq!(script.starts.borrow()[0].prompt, "work");
        // An adapter that declares the MCP road is handed the host's own server when the start names none.
        assert_eq!(script.mcp.borrow()[0], Some(json!({ "wsp": { "command": "wsp", "args": ["mcp"] } })));
        let sync = Scripted::new().session("22222222-2222-4222-8222-222222222222").when(When::Sync);
        let fx2 = runtime(&backend).adapter("claude", sync.factory()).build();
        let ws2 = create_on(&fx2.rt, CreateOn::golden("w2"), None).await.expect("a workspace").view.id;
        let quick = fx2.rt.sessions().start(&ws2, StartOptions { prompt: "x".into(), ..Default::default() }, None).await.expect("a start");
        assert_eq!(j(&quick.finished().await)["text"], "done");
        assert_eq!(j(&quick.view())["claudeSessionId"], "22222222-2222-4222-8222-222222222222");
        fx.rt.close().await.expect("a close");
        fx2.rt.close().await.expect("a close");
    });
}

// waiting-behind.test.ts:25 `drivenAdapter`: a turn raises a prompt, an answer closes it, a reply closes what is still
// open as cancelled, and each session id is its own.
#[test]
fn a_driven_turn_raises_a_prompt_takes_its_answer_and_closes_what_a_reply_leaves_open() {
    drive(|| async {
        use common::driven::{driven, Turns};
        use wsp_core::types::api::AnswerAsk;
        let backend = StubBackend::new(None);
        let turns = Turns::new();
        let fx = runtime(&backend).adapter("claude", driven(&turns)).build();
        let rt = &*fx.rt;
        let ws = create_on(rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = rt.sessions().start(&ws, StartOptions { prompt: "one".into(), ..Default::default() }, None).await.expect("a start");
        let second = rt.sessions().start(&ws, StartOptions { prompt: "two".into(), ..Default::default() }, None).await.expect("a start");
        until(|| async { turns.len() == 2 }).await;
        assert_ne!(first.view().id, second.view().id);
        turns.get(0).raise(json!({}));
        turns.get(1).raise(json!({ "askId": "ask_2" }));
        let answered = rt
            .sessions()
            .answer(&first.view().id, AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None }, None)
            .await
            .expect("an answer");
        assert_eq!(j(&answered), json!({ "outcome": "answered" }));
        let again = rt
            .sessions()
            .answer(&first.view().id, AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None }, None)
            .await;
        assert!(again.is_err() || j(&again.expect("a result")) != json!({ "outcome": "answered" }));
        turns.get(0).reply(json!({}));
        turns.get(1).reply(json!({ "text": "second" }));
        assert_eq!(first.finished().await.text.as_deref(), Some("done"));
        assert_eq!(second.finished().await.text.as_deref(), Some("second"));
        let history = rt.sessions().history(&ws, None).await.expect("a history");
        let closes: Vec<String> = history
            .iter()
            .map(j)
            .filter(|e| e["askId"] == "ask_2" && e["outcome"].is_string())
            .map(|e| e["outcome"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(closes.iter().filter(|o| *o == "cancelled").count(), 1, "{closes:?}");
        fx.rt.close().await.expect("a close");
    });
}

// stub-backend.ts:460-490 `fakeLocal` and `copyingFake`: this computer is a folder of the test's own, the copy road
// copies a folder for real and a removal takes it away, and every ask is logged.
#[test]
fn the_fake_local_computer_copies_a_folder_for_real_and_a_removal_takes_it_away() {
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let from = dir.path().join("from");
        std::fs::create_dir_all(from.join("src")).expect("a folder");
        std::fs::write(from.join("src/a.txt"), "a").expect("a file");
        let (copier, log) = copying_fake();
        let to = dir.path().join("to");
        let ask: wsp_engine::copier::CopyAsk = serde_json::from_value(json!({
            "from": from.display().to_string(), "to": to.display().to_string(), "exclude": ["node_modules"], "sizeLineBytes": 0
        }))
        .expect("an ask");
        use wsp_engine::copier::Copier as _;
        let report = copier.make(&ask).await.expect("a copy");
        assert_eq!(std::fs::read_to_string(to.join("src/a.txt")).expect("the copied file"), "a");
        assert_eq!(j(&report)["excluded"], json!(["node_modules"]));
        assert_eq!(log.lock().expect("a log").asks.len(), 1);
        copier.remove(&ask.from, &ask.to, wsp_engine::copier::CopyRoad::Clonefile).await.expect("a removal");
        assert!(!to.exists());
        assert_eq!(log.lock().expect("a log").removed.len(), 1);
        let local = fake_local(dir.path());
        assert_eq!(local.folder, dir.path().display().to_string());
        assert_eq!((local.home)("claude"), format!("{}/.claude", dir.path().display()));
    });
}

// forget.test.ts:81 and :173: a store whose transcript rows stop reading on the test's word, and whose puts into one
// collection wait at a gate; both otherwise answer as the store they wrap.
#[test]
fn a_wrapped_store_stops_reading_transcript_rows_and_holds_a_put_at_a_gate() {
    drive(|| async {
        let state = sqlite_state();
        let wrapped = WrappedStore::over(&state.store);
        let store = wrapped.store();
        let rows = store.transcripts().expect("rows");
        assert!(rows.all("ws_1").is_ok());
        wrapped.set_unreadable(true);
        assert!(rows.all("ws_1").is_err());
        assert!(rows.index("ws_1").is_err());
        wrapped.set_unreadable(false);
        assert!(rows.all("ws_1").is_ok());

        let (reached, open) = wrapped.gate_puts("sessions");
        let writing = {
            let store = store.clone();
            tokio::task::spawn_local(async move { store.put("sessions", "ws_1", json!({ "sessions": [] })).await })
        };
        reached.wait().await;
        assert!(state.store.get("sessions", "ws_1").await.expect("a read").is_none());
        open.open();
        writing.await.expect("the write").expect("written");
        assert!(state.store.get("sessions", "ws_1").await.expect("a read").is_some());
    });
}

// forget.test.ts:99 `dying` and `working`: an adapter literal is a start function, its events follow the start, and its session finishes with the result the events carried.
#[test]
fn a_scripted_adapter_sends_its_events_and_finishes_with_its_result() {
    drive(|| async {
        use common::scripted::{adapter_of, after_a_turn, done_and_end, Plain};
        let backend = StubBackend::new(None);
        let start: common::scripted::StartFn = Rc::new(|o| {
            let on_event = o.on_event.clone().expect("a sink");
            let doc = json!({ "status": "completed", "text": "all green" });
            let mut events = vec![json!({ "type": "session.start", "sessionId": "s-1", "cwd": "/root/app" })];
            events.extend(done_and_end("s-1", &doc, 0, true));
            Ok(Plain::of("s-1", after_a_turn(on_event, events, doc)))
        });
        let fx = runtime(&backend).adapter("claude", adapter_of(start)).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let handle = fx.rt.sessions().start(&ws, StartOptions { prompt: "go".into(), ..Default::default() }, None).await.expect("a start");
        assert_eq!(handle.finished().await.text.as_deref(), Some("all green"));
        let types: Vec<String> = fx
            .rt
            .sessions()
            .history(&ws, None)
            .await
            .expect("a history")
            .iter()
            .map(|e| j(e)["type"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(types, ["session.start", "session.done", "session.end"]);
        fx.rt.close().await.expect("a close");
    });
}

// kept-stubs.ts `claudeKeeper` and `codexKeeper`, strays.ts: each stand-in agent answers a message with its pid, its
// turn count and its turn token, and a test reads whether a process is there without finding it by name.
#[test]
fn the_stand_in_agents_name_their_process_and_turn_and_a_test_reads_whether_a_process_is_there() {
    drive(|| async {
        use common::stubs::{alive, claude_keeper, codex_keeper, gone, pid_of, token_of};
        use std::io::Write as _;
        use std::process::{Command, Stdio};
        let dir = tempfile::tempdir().expect("a folder");
        let (claude, codex) = (dir.path().join("claude"), dir.path().join("codex"));
        claude_keeper(&claude);
        codex_keeper(&codex);
        let mut child = Command::new(&claude)
            .args(["--session-id", "abc"])
            .env("WSP_TURN", "tok123")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the claude stand-in");
        let pid = child.id();
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(b"{\"type\":\"user\",\"message\":{\"content\":[{\"text\":\"hello\"}]}}\n")
            .expect("a message");
        drop(child.stdin.take());
        let out = child.wait_with_output().expect("its output");
        let said = String::from_utf8_lossy(&out.stdout).to_string();
        let reply = said
            .lines()
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find(|v| v["type"] == "result")
            .expect("a result");
        let text = reply["result"].as_str().map(str::to_string);
        assert_eq!(text.as_deref(), Some(format!("hello from {pid} turn 1 token tok123").as_str()));
        assert_eq!((pid_of(text.as_deref()), token_of(text.as_deref())), (Some(pid), "tok123".to_string()));
        assert!(!alive(pid));
        gone(pid).await;
        let version = Command::new(&claude).arg("--version").output().expect("a version");
        assert_eq!(String::from_utf8_lossy(&version.stdout).trim(), "2.1.289 (Claude Code)");
        assert_eq!(Command::new(&codex).arg("nothing").status().expect("codex").code(), Some(2));
    });
}

// pull-request.test.ts:84 `fakeDaemons` and :168 `withWorkspace`: two daemons behind one dial told apart by the url, each
// frame kept with its road, an op nobody answers refused as a daemon refuses it; and a workspace over them whose pull
// request reads over this computer's own daemon.
#[test]
fn two_daemons_behind_one_dial_are_told_apart_by_the_url_and_a_workspace_reads_its_pull_request_over_the_first() {
    drive(|| async {
        use common::pull_request::{answer, fake_daemons, ok, with_workspace, Via, With, COPY, HERE};
        use wsp_core::daemon_channel::DaemonChannelOptions;
        let d = fake_daemons(vec![("git.hello", answer(|_| ok(json!({ "who": "here" }))))], vec![]);
        let dial = |url: &str| DaemonChannelOptions {
            url: url.into(),
            token: "t".into(),
            on_event: Rc::new(|_| {}),
            connect_timeout_ms: None,
            clock: None,
        };
        let open = d.open();
        let here = open(dial(HERE)).await.expect("a channel");
        let copy = open(dial(COPY)).await.expect("a channel");
        let frame = |op: &str| match json!({ "op": op }) {
            serde_json::Value::Object(map) => map,
            _ => unreachable!(),
        };
        assert_eq!(here.send(frame("git.hello")).await.expect("a reply")["who"], "here");
        let refused = copy.send(frame("git.hello")).await.expect("a reply");
        assert_eq!((refused["ok"].clone(), refused["error"].clone()), (json!(false), json!("git.hello was not answered here")));
        assert_eq!(d.ops(Some(Via::Here)), ["git.hello"]);
        assert_eq!(d.ops(Some(Via::Copy)), ["git.hello"]);

        let d = fake_daemons(vec![], vec![]);
        let h = with_workspace(&d, With::default()).await;
        h.rt().workspaces().checkout(&h.id, None, None).await.expect("a checkout");
        until(|| async { h.pr_field("number") == Some(json!(12)) }).await;
        assert_eq!(d.ops(Some(Via::Copy)), ["git.status"]);
        assert_eq!(d.count(Via::Here, "git.prRead"), 1);
        assert!(h.remote.starts_with("https://github.com/wsp/stub-"));
    });
}

// runtime-steer.test.ts:191 `steerable` and steer-images.test.ts:15 `harness`: a hand-driven harness keeps what each start and
// steer was handed, answers a steer as told, and says the events the test gives it.
#[test]
fn a_live_harness_keeps_its_starts_and_steers_answers_as_told_and_says_what_the_test_emits() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let live = Live::new(LiveOptions {
            steers: true,
            announces: false,
            session: Some("66666666-6666-4666-8666-666666666666".into()),
            ..Default::default()
        });
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
        let steer =
            wsp_core::types::api::SteerAsk { prompt: "and say pineapple".into(), request_id: Some("req_2".into()), attachments: None };
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
    use common::projects::AgentsCaps;
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let made = create_on(&fx.rt, CreateOn::golden("a").agents(2, 3), None).await.expect("a workspace");
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
        let failed = channel.send(json!({ "op": "git.push" }).as_object().cloned().expect("an object")).await.expect_err("a failure");
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
        assert_eq!(rows.all("w").expect_err("a failure").message, "disk I/O error");
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

// fed-runs.ts: runs of the real adapter fed by hand: lines pushed reach the stream, a write that loses its answer lands or
// not as told, and an attach after a restart reads the run again from its first line with what its input took.
#[test]
fn a_fed_run_streams_the_lines_pushed_loses_a_write_as_told_and_reads_again_on_attach() {
    use futures::StreamExt as _;
    use wsp_adapters::port::{AttachRunOptions, Attached, ExecOptions};
    drive(|| async {
        let (factory, fed) = common::fed::fed_runs();
        let stream = factory.launch("claude", ExecOptions { input: Some(vec!["open the run".into()]), ..Default::default() });
        let run = fed.run_of("open the run").expect("the run");
        run.push(&["one".into(), "two".into()]);
        let mut lines = stream.lines();
        assert_eq!(lines.next().await.expect("a line").expect("ok"), "one");
        assert_eq!(lines.next().await.expect("a line").expect("ok"), "two");
        run.loses.set(Some(common::fed::Loses::Landed));
        assert!(stream.write("{\"uuid\":\"u1\"}".into()).await.is_err());
        run.loses.set(Some(common::fed::Loses::Lost));
        assert!(stream.write("{\"uuid\":\"u2\"}".into()).await.is_err());
        stream.write("{\"uuid\":\"u3\"}".into()).await.expect("a write");
        assert_eq!(run.writes(), ["{\"uuid\":\"u1\"}", "{\"uuid\":\"u3\"}"]);
        assert_eq!(fed.steered("open the run", 1), "u3");
        run.end(Some(0));
        assert!(lines.next().await.is_none());
        let id = run.id.clone();
        let attached =
            factory.attach(&id, AttachRunOptions { input: true, started_at: 0.0, from: None }).expect("an attach").await.expect("found");
        let Attached::Stream(again) = attached else { panic!("the run is gone") };
        let mut replay = again.lines();
        assert_eq!(replay.next().await.expect("a line").expect("ok"), "one");
        assert_eq!(again.taken().expect("what its input took"), ["open the run", "{\"uuid\":\"u1\"}", "{\"uuid\":\"u3\"}"]);
        let gone = factory
            .attach("/tmp/wsp-run/none", AttachRunOptions { input: true, started_at: 0.0, from: None })
            .expect("an attach")
            .await
            .expect("an answer");
        assert!(matches!(gone, Attached::Gone));
    });
}

// steer-unread.test.ts `host()` and turn-costs.test.ts `runtime()`: a wiring on this computer carries the launch environment, the run
// folder and the poll a test names.
#[test]
fn a_local_wiring_carries_the_launch_variables_and_the_run_folder_a_test_names() {
    use common::local::{git_copier, local_wiring_with, LocalOpts};
    let dir = tempfile::tempdir().expect("a folder");
    let (copier, _log) = git_copier();
    let vars = indexmap::IndexMap::from([("STUB_MODE".to_string(), "dies".to_string())]);
    let local = local_wiring_with(dir.path(), copier, LocalOpts { vars: Some(vars.clone()), poll_ms: Some(20), ..Default::default() });
    assert_eq!((local.env)(), vars);
    assert_eq!((local.home)("claude"), format!("{}/.claude", dir.path().display()));
    assert_eq!(local.folder, dir.path().display().to_string());
    assert!(local.copier.is_some());
}

// channels.test.ts:139 `brokeredFork`: where the broker is ready the record keeps the branch of record a create names, and where it is
// not the record keeps none.
#[test]
fn a_ready_broker_makes_the_record_keep_the_branch_of_record_a_create_names() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).with(|o| o.broker = Some(TestBroker::ready().rc())).build();
        let made =
            create_on(&fx.rt, CreateOn::golden("a").branch_of_record(4242, "acme", "lab", "wsp/a"), None).await.expect("a workspace");
        let record = fx.store.get("workspaces", &made.view.id).await.expect("a read").expect("a record");
        assert_matches(&record["branchOfRecord"], json!({ "repoId": 4242, "owner": "acme", "name": "lab", "branch": "wsp/a" }));
        let backend = StubBackend::new(None);
        let off = runtime(&backend).with(|o| o.broker = Some(TestBroker::gated(|| false).rc())).build();
        let made =
            create_on(&off.rt, CreateOn::golden("b").branch_of_record(4242, "acme", "lab", "wsp/b"), None).await.expect("a workspace");
        let record = off.store.get("workspaces", &made.view.id).await.expect("a read").expect("a record");
        assert!(record.get("branchOfRecord").is_none());
    });
}

// conversations.test.ts `fakeLocal` over `new LocalBackend({ env })`: every exec of this computer's backend runs under the variables a
// test names.
#[test]
fn a_local_wiring_runs_the_backends_execs_under_the_variables_a_test_names() {
    use common::local::{git_copier, local_wiring_with, LocalOpts};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let (copier, _log) = git_copier();
        let vars =
            indexmap::IndexMap::from([("FIXTURE_MARK".to_string(), "kept".to_string()), ("PATH".to_string(), "/usr/bin:/bin".to_string())]);
        let local = local_wiring_with(dir.path(), copier, LocalOpts { backend_vars: Some(vars), ..Default::default() });
        let machine = local.backend.get("local").await.expect("a machine");
        let said = machine.exec("printf %s \"$FIXTURE_MARK\"", ExecOptions::default()).await.expect("an exec");
        assert_eq!(said.stdout, "kept");
    });
}

// ws-client.ts `WsClient`: a socket authenticated with a token answers a request by its id and keeps the frames the host pushes it.
#[test]
fn a_ws_client_answers_a_request_and_keeps_the_events_the_host_pushes() {
    use common::local::WsClient;
    use wsp_serve::serve::{serve_runtime, ServeOptions};
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let served = serve_runtime(fx.rt.clone(), ServeOptions { port: Some(0), auth_token: "secret".into(), ..Default::default() })
            .await
            .expect("the runtime serves");
        let mut client = WsClient::connect(served.port(), "secret").await;
        assert_eq!(client.request("events.subscribe", json!({})).await["ok"], true);
        let made = create_on(&fx.rt, CreateOn::golden("pushed"), None).await.expect("a workspace");
        let events = client.events.clone();
        until(|| async {
            events.borrow().iter().any(|e| e["type"] == "workspace.created" && e["workspace"]["id"] == made.view.id.as_str())
        })
        .await;
        client.close();
        served.close().await.expect("a close");
        fx.rt.close().await.expect("a close");
    });
}

// folder-threads.test.ts `tools()` and `machine.exec = ...`: a backend whose one machine runs every command through a hook of the test's own
// first, which may change the command or answer in its place.
#[test]
fn an_around_backend_runs_each_command_through_the_hook_that_may_change_or_answer_it() {
    use common::local::AroundBackend;
    use wsp_engine::machine::{ExecResult, MachineBackend as _};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let seen: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
        let kept = seen.clone();
        let around: common::local::Around = Rc::new(move |cmd: String| {
            kept.borrow_mut().push(cmd.clone());
            Box::pin(async move {
                if cmd.contains("refuse") {
                    return Err(ExecResult { exit_code: 1.into(), stdout: String::new(), stderr: "refused".into() });
                }
                Ok(cmd.replace("one", "two"))
            })
        });
        let backend = AroundBackend::new(dir.path(), None, around);
        let machine = backend.get("local").await.expect("a machine");
        assert_eq!(machine.exec("printf one", ExecOptions::default()).await.expect("an exec").stdout, "two");
        let refused = machine.exec("refuse", ExecOptions::default()).await.expect("an answer");
        assert_eq!((refused.exit_code.0 .0 as i64, refused.stderr.as_str()), (1, "refused"));
        assert_eq!(*seen.borrow(), ["printf one", "refuse"]);
    });
}

// folder-threads.test.ts `here()`: a host over a folder of its own where a thread runs in the project's folder, with no copy made.
#[test]
fn a_folder_host_runs_a_thread_in_the_projects_folder_and_keeps_every_daemon_frame() {
    use common::folder_host::{here, HereOpts, Scratch};
    use wsp_core::types::api::{FolderForAsk, ProjectAddAsk};
    drive(|| async {
        let scratch = Scratch::new();
        let h = here(&scratch, HereOpts::default());
        let folder = scratch.repo();
        let project =
            h.fx.rt
                .projects()
                .add(ProjectAddAsk { source: folder.display().to_string(), ..Default::default() }, None)
                .await
                .expect("a project");
        let at =
            h.fx.rt
                .workspaces()
                .folder_for(FolderForAsk { project: Some(project.project.id), ..Default::default() }, None)
                .await
                .expect("a folder");
        h.fx.rt
            .sessions()
            .start(&at.workspace.id, StartOptions { prompt: "hello".into(), ..Default::default() }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(h.agents.cwds(), [Some(folder.display().to_string())]);
        until(|| async { h.ops().iter().any(|o| o == "git.checkpoint") }).await;
        assert!(h.copier.lock().unwrap().asks.is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// folder-threads.test.ts `here(..., again)` and `sqliteStore(...)`: a folder host writes to the store a test hands it, and not to its own.
#[test]
fn a_folder_host_writes_to_the_store_a_test_hands_it() {
    use common::folder_host::{add, ask, folder_for, here, HereOpts, Scratch};
    drive(|| async {
        let scratch = Scratch::new();
        let mine = sqlite_state();
        let h = here(&scratch, HereOpts { store: Some(mine.store.clone()), ..Default::default() });
        let project = add(&h, &scratch.repo()).await;
        folder_for(&h, ask(project["id"].as_str().expect("an id"), None, None)).await;
        assert_eq!(mine.store.list("workspaces").await.expect("a list").len(), 1);
        assert!(h.state.store.list("workspaces").await.expect("a list").is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}
