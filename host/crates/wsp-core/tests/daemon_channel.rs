// SPDX-License-Identifier: AGPL-3.0-only
//! spec/daemon_channel.md; ported from packages/runtime/test/daemon-channel.test.ts (8 cases) plus the cases the spec
//! pins. The TS ran a real daemon binary; here the fake daemon answers as its inbound door does.
mod support;

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use support::fake_daemon::FakeDaemon;
use support::net::{refusing_door, tcp_proxy};
use support::{run_local, settle, until};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use wsp_core::daemon_channel::{is_daemon_unanswered, open_daemon_channel, ChannelError, DaemonChannelOptions};

const TOKEN: &str = "channel-token";

fn options(url: String, token: &str, events: &Rc<RefCell<Vec<Value>>>) -> DaemonChannelOptions {
    let events = events.clone();
    DaemonChannelOptions { url, token: token.into(), on_event: Rc::new(move |e| events.borrow_mut().push(e)), connect_timeout_ms: None, clock: None }
}

fn frame(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

/// A one-socket websocket server on the test thread, driven by `script`.
async fn scripted<F, Fut>(script: F) -> u16
where
    F: FnOnce(WebSocketStream<tokio::net::TcpStream>) -> Fut + 'static,
    Fut: Future<Output = ()> + 'static,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::task::spawn_local(async move {
        let (sock, _) = listener.accept().await.unwrap();
        let ws = tokio_tungstenite::accept_async(sock).await.unwrap();
        script(ws).await;
    });
    port
}

async fn next_json(ws: &mut WebSocketStream<tokio::net::TcpStream>) -> Value {
    loop {
        if let Some(Ok(Message::Text(t))) = ws.next().await {
            return serde_json::from_str(&t).unwrap();
        }
    }
}

#[test]
fn dials_once_authenticates_and_round_trips_the_daemons_own_replies_with_the_hello_as_the_first_event() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let events = Rc::new(RefCell::new(Vec::new()));
        let channel = open_daemon_channel(options(daemon.url(), TOKEN, &events)).await.unwrap();
        until("the hello", 5_000, || !events.borrow().is_empty()).await;
        assert_eq!(events.borrow()[0]["type"], json!("daemon.hello"));
        let created = channel.send(frame(json!({"op": "pty.create", "cols": 80, "rows": 24, "shell": "/bin/sh"}))).await.unwrap();
        assert_eq!(created["ok"], json!(true));
        let pty = created["ptyId"].as_str().unwrap().to_string();
        assert_eq!(channel.send(frame(json!({"op": "pty.attach", "ptyId": pty}))).await.unwrap()["ok"], json!(true));
        assert_eq!(channel.send(frame(json!({"op": "pty.write", "ptyId": pty, "data": "channel-mark-42\r"}))).await.unwrap()["ok"], json!(true));
        until("the pty's data", 5_000, || events.borrow().iter().any(|e| e["type"] == "pty.data" && e["data"].as_str().unwrap_or("").contains("channel-mark-42"))).await;
        assert_eq!(daemon.frames().len(), 4, "one dial: the auth frame and three sends");
        channel.close();
    });
}

#[test]
fn the_auth_frame_is_op_token_id_in_that_order_and_takes_id_one() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let events = Rc::new(RefCell::new(Vec::new()));
        let channel = open_daemon_channel(options(format!("http://127.0.0.1:{}/", daemon.port), TOKEN, &events)).await.unwrap();
        assert_eq!(daemon.frames()[0].to_string(), r#"{"op":"auth","token":"channel-token","id":1}"#);
        channel.send(frame(json!({"op": "ping"}))).await.unwrap();
        assert_eq!(daemon.frames()[1].to_string(), r#"{"op":"ping","id":2}"#);
        channel.close();
    });
}

#[test]
fn a_callers_own_id_keeps_its_place_and_takes_the_channels_number() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let events = Rc::new(RefCell::new(Vec::new()));
        let channel = open_daemon_channel(options(daemon.url(), TOKEN, &events)).await.unwrap();
        let reply = channel.send(frame(json!({"id": 99, "op": "ping", "x": 1}))).await.unwrap();
        assert_eq!(daemon.frames()[1].to_string(), r#"{"id":2,"op":"ping","x":1}"#);
        assert_eq!(reply["id"], json!(2));
        channel.close();
    });
}

#[test]
fn hands_a_refusal_back_as_the_daemon_wrote_it_with_the_typed_code_and_no_throw() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let events = Rc::new(RefCell::new(Vec::new()));
        let channel = open_daemon_channel(options(daemon.url(), TOKEN, &events)).await.unwrap();
        let refused = channel.send(frame(json!({"op": "fs.list", "path": 7}))).await.expect("a refusal is a reply, never an error");
        assert_eq!(refused["ok"], json!(false));
        assert_eq!(refused["code"], json!("bad-request"));
        assert!(!refused["error"].as_str().unwrap().is_empty());
        channel.close();
    });
}

#[test]
fn a_frame_that_does_not_parse_is_dropped_and_the_next_one_still_lands() {
    run_local(async {
        let port = scripted(|mut ws| async move {
            let auth = next_json(&mut ws).await;
            ws.send(Message::text(json!({"id": auth["id"], "ok": true}).to_string())).await.unwrap();
            ws.send(Message::text("this is not json")).await.unwrap();
            ws.send(Message::text(json!({"type": "custom.event", "n": 1}).to_string())).await.unwrap();
            ws.send(Message::text(json!({"id": "1", "type": "string.id"}).to_string())).await.unwrap();
            ws.send(Message::text("[1,2]")).await.unwrap();
            let ping = next_json(&mut ws).await;
            ws.send(Message::text(json!({"id": 2.0, "ok": true, "pong": ping["op"]}).to_string())).await.unwrap();
            std::future::pending::<()>().await;
        })
        .await;
        let events = Rc::new(RefCell::new(Vec::new()));
        let channel = open_daemon_channel(options(format!("ws://127.0.0.1:{port}"), TOKEN, &events)).await.unwrap();
        let reply = channel.send(frame(json!({"op": "ping"}))).await.unwrap();
        assert_eq!(reply["pong"], json!("ping"), "a JSON 2.0 is the number 2");
        assert_eq!(*events.borrow(), vec![json!({"type": "custom.event", "n": 1}), json!({"id": "1", "type": "string.id"}), json!([1, 2])]);
        channel.close();
    });
}

#[test]
fn an_auth_reply_that_is_not_ok_rejects_plainly_with_the_daemons_words_or_the_default() {
    run_local(async {
        for (reply, words) in [(json!({"id": 1, "ok": false, "error": "not this host"}), "not this host"), (json!({"id": 1, "ok": false}), "the machine refused the daemon link")] {
            let port = scripted(move |mut ws| async move {
                next_json(&mut ws).await;
                ws.send(Message::text(reply.to_string())).await.unwrap();
                std::future::pending::<()>().await;
            })
            .await;
            let events = Rc::new(RefCell::new(Vec::new()));
            let failed = open_daemon_channel(options(format!("ws://127.0.0.1:{port}"), TOKEN, &events)).await.err().unwrap();
            assert_eq!(failed, ChannelError::Refused { message: words.to_string() });
            assert_eq!(failed.kind(), None);
            assert!(!is_daemon_unanswered(&failed));
        }
    });
}

#[test]
fn a_token_the_daemon_refuses_rejects_as_a_token_error_carrying_its_4401_sentence() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let events = Rc::new(RefCell::new(Vec::new()));
        let failed = open_daemon_channel(options(daemon.url(), "stale", &events)).await.err().unwrap();
        assert_eq!(failed.kind(), Some("reauth"));
        assert_eq!(failed.to_string(), "the machine refused this host's daemon token (4401 daemon token refused; the host holds the current one)");
        assert!(matches!(failed, ChannelError::Token(_)));
        assert!(!is_daemon_unanswered(&failed));
    });
}

#[test]
fn an_auth_deadline_that_passed_while_this_hosts_loop_stood_still_is_a_link_that_ended_never_a_refused_token() {
    run_local(async {
        let daemon = FakeDaemon::start_with(TOKEN, 100);
        let proxy = tcp_proxy(daemon.port, 0).await;
        proxy.freeze_after_next_reply(400);
        let events = Rc::new(RefCell::new(Vec::new()));
        let failed = open_daemon_channel(options(format!("ws://127.0.0.1:{}", proxy.port), TOKEN, &events)).await.err().unwrap();
        assert!(!matches!(failed, ChannelError::Token(_)));
        assert_ne!(failed.kind(), Some("reauth"));
        assert!(is_daemon_unanswered(&failed));
        // The same token on the next dial is let in: nothing about it was refused.
        let channel = open_daemon_channel(options(format!("ws://127.0.0.1:{}", proxy.port), TOKEN, &events)).await.unwrap();
        channel.close();
    });
}

#[test]
fn a_door_that_answers_the_upgrade_with_a_status_rejects_as_a_door_error_carrying_the_status_and_the_bodys_first_line() {
    run_local(async {
        let door = refusing_door(403, "cross-origin websocket denied\nthe edge said so").await;
        let events = Rc::new(RefCell::new(Vec::new()));
        let failed = open_daemon_channel(options(format!("http://127.0.0.1:{}/", door.port), TOKEN, &events)).await.err().unwrap();
        let ChannelError::Door(d) = &failed else { panic!("not a door error: {failed:?}") };
        assert_eq!(d.status, 403);
        assert_eq!(failed.kind(), Some("refused"));
        // One line of the body, so a door that answers an HTML page does not become the pane's sentence.
        assert_eq!(failed.to_string(), "the connection was refused with 403: cross-origin websocket denied");
        assert!(!is_daemon_unanswered(&failed));
    });
}

#[test]
fn a_door_with_an_empty_body_says_only_its_status() {
    run_local(async {
        let door = refusing_door(502, "  \nsecond").await;
        let events = Rc::new(RefCell::new(Vec::new()));
        let failed = open_daemon_channel(options(format!("ws://127.0.0.1:{}", door.port), TOKEN, &events)).await.err().unwrap();
        assert_eq!(failed.to_string(), "the connection was refused with 502");
    });
}

#[test]
fn a_door_body_past_the_cap_is_cut_at_512_units() {
    run_local(async {
        let door = refusing_door(500, &"é".repeat(700)).await;
        let events = Rc::new(RefCell::new(Vec::new()));
        let failed = open_daemon_channel(options(format!("ws://127.0.0.1:{}", door.port), TOKEN, &events)).await.err().unwrap();
        assert_eq!(failed.to_string(), format!("the connection was refused with 500: {}", "é".repeat(512)));
    });
}

#[test]
fn a_port_nothing_listens_on_and_a_door_that_never_answers_both_reject_plainly_with_no_kind_to_read() {
    run_local(async {
        let mut daemon = FakeDaemon::start(TOKEN);
        let dead = daemon.port;
        daemon.close();
        let events = Rc::new(RefCell::new(Vec::new()));
        let refused = open_daemon_channel(options(format!("ws://127.0.0.1:{dead}"), TOKEN, &events)).await.err().unwrap();
        assert_eq!(refused.kind(), None);
        assert!(!matches!(refused, ChannelError::Door(_)));
        assert_eq!(refused.to_string(), "the link to the computer ended before it opened");
        assert!(is_daemon_unanswered(&refused));

        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        proxy.stall();
        let mut o = options(format!("ws://127.0.0.1:{}", proxy.port), TOKEN, &events);
        o.connect_timeout_ms = Some(150);
        let timed_out = open_daemon_channel(o).await.err().unwrap();
        assert_eq!(timed_out.to_string(), "daemon connect timed out after 150ms");
        assert_eq!(timed_out.kind(), None);
        assert!(is_daemon_unanswered(&timed_out));
    });
}

#[test]
fn closed_settles_once_with_the_code_and_reason_and_a_send_in_flight_when_the_socket_goes_rejects() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let proxy = tcp_proxy(daemon.port, 0).await;
        let events = Rc::new(RefCell::new(Vec::new()));
        let ch = open_daemon_channel(options(format!("ws://127.0.0.1:{}", proxy.port), TOKEN, &events)).await.unwrap();
        proxy.stall();
        let sending = ch.clone();
        let in_flight = tokio::task::spawn_local(async move { sending.send(frame(json!({"op": "ping"}))).await });
        settle().await;
        proxy.cut_all();
        let lost = in_flight.await.unwrap().err().unwrap();
        assert_eq!(lost.to_string(), "connection lost");
        assert!(is_daemon_unanswered(&lost));
        let ended = ch.closed().await;
        assert_eq!(ch.closed().await, ended, "closed settles once");
        // A dead channel answers a send rather than hanging on it.
        let after = ch.send(frame(json!({"op": "ping"}))).await.err().unwrap();
        assert_eq!(after.to_string(), "daemon unreachable");
    });
}

#[test]
fn a_close_the_caller_asked_for_settles_closed_too() {
    run_local(async {
        let daemon = FakeDaemon::start(TOKEN);
        let events = Rc::new(RefCell::new(Vec::new()));
        let ch = open_daemon_channel(options(daemon.url(), TOKEN, &events)).await.unwrap();
        ch.close();
        let ended = tokio::time::timeout(std::time::Duration::from_secs(5), ch.closed()).await.expect("closed settles after close()");
        assert!(ended.code >= 1000);
    });
}
