// SPDX-License-Identifier: AGPL-3.0-only
//! The socket to the host (client.ts:95-452, spec/client-dial.md): a real websocket on loopback, a host scripted
//! frame by frame, and what the line sends, reads and fails with.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

use common::{boxed, RecordingIo, ScriptedClient, ScriptedHost, Script, STOPPING_CLOSE};
use wsp_cli::hosts::{HostAim, HostPick};
use wsp_cli::verbs::client::{
    dial_host, host_address, host_again, stopped_under, until_settled, DialOpts, Frame, HostAddress, HostClient, PushedFrames,
    HOST_RESTARTING_LINE,
};
use wsp_cli::version::VERSION;
use wsp_words::exit::{HostError, Kind};

const HOST_STOPPING_LINE: &str = "the host is restarting; the turn goes on and its reply lands in the thread";
const HOST_CLOSED_LINE: &str = "the host closed the connection";

/// What the scripted host does with one frame it read.
enum Act {
    Send(String),
    Close(Option<u16>),
}

/// What a scripted host saw: every text frame as it came, the path of each upgrade, and each close frame's code.
#[derive(Default)]
struct Seen {
    frames: Vec<String>,
    paths: Vec<String>,
    closes: Vec<Option<u16>>,
    connections: usize,
}

type Answer = Arc<dyn Fn(&Value) -> Vec<Act> + Send + Sync>;

/// A host on loopback answering each frame as `answer` says; `silent` accepts TCP and never upgrades.
async fn host(answer: Answer) -> (u16, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let noted = seen.clone();
    tokio::spawn(async move {
        while let Ok((tcp, _)) = listener.accept().await {
            let answer = answer.clone();
            let noted = noted.clone();
            noted.lock().unwrap().connections += 1;
            tokio::spawn(async move {
                let paths = noted.clone();
                let on_path = move |asked: &tokio_tungstenite::tungstenite::handshake::server::Request, reply| {
                    paths.lock().unwrap().paths.push(asked.uri().to_string());
                    Ok(reply)
                };
                let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(tcp, on_path).await else { return };
                while let Some(Ok(message)) = ws.next().await {
                    match message {
                        Message::Text(text) => {
                            noted.lock().unwrap().frames.push(text.to_string());
                            let frame: Value = serde_json::from_str(&text).unwrap();
                            for act in answer(&frame) {
                                match act {
                                    Act::Send(text) => {
                                        let _ = ws.send(Message::text(text)).await;
                                    }
                                    Act::Close(code) => {
                                        let frame = code.map(|c| CloseFrame { code: CloseCode::from(c), reason: "".into() });
                                        let _ = ws.close(frame).await;
                                        return;
                                    }
                                }
                            }
                        }
                        Message::Close(frame) => {
                            noted.lock().unwrap().closes.push(frame.map(|f| u16::from(f.code)));
                            return;
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    (port, seen)
}

/// A host that takes any token and answers every op `{ok: true}` with the extra fields given for that op.
fn answering(extra: &'static [(&'static str, &'static str)]) -> Answer {
    Arc::new(move |frame| {
        let id = frame["id"].clone();
        let op = frame["op"].as_str().unwrap();
        let fields = extra.iter().find(|(o, _)| *o == op).map(|(_, f)| *f).unwrap_or("");
        let rest = if fields.is_empty() { String::new() } else { format!(",{fields}") };
        vec![Act::Send(format!(r#"{{"id":{id},"ok":true{rest}}}"#))]
    })
}

fn url_aim(port: u16, token: Option<&str>) -> HostAim {
    HostAim::Url { url: format!("http://127.0.0.1:{port}"), token: token.map(str::to_owned), host_key: None }
}

fn opts(aim: HostAim) -> DialOpts {
    DialOpts { aim: Some(aim), deadline_ms: Some(2_000), ..DialOpts::default() }
}

async fn dial(port: u16) -> Arc<dyn HostClient> {
    dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.unwrap()
}

fn frame(v: Value) -> Frame {
    v.as_object().unwrap().clone()
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(50)).await;
}

mod the_first_frame {
    use super::*;

    #[tokio::test]
    async fn the_token_rides_the_first_frame_as_auth_with_id_1_and_never_in_the_url() {
        let (port, seen) = host(answering(&[])).await;
        let client = dial(port).await;
        client.close();
        settle().await;
        let seen = seen.lock().unwrap();
        assert_eq!(seen.frames[0], r#"{"token":"tok","id":1,"op":"auth"}"#);
        assert_eq!(seen.paths, vec!["/ws".to_owned()], "the socket is dialled on the runtime's path, with no query");
    }

    #[tokio::test]
    async fn a_url_aim_with_no_token_is_refused_before_any_socket_opens() {
        let (port, seen) = host(answering(&[])).await;
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, None))).await.err().expect("refused");
        assert_eq!(
            e.message,
            format!("--host takes the name of a host on your account; http://127.0.0.1:{port} is an address, and wsp hosts lists the names. Run wsp hosts for the list.")
        );
        assert_eq!(e.kind, Some(Kind::Usage));
        settle().await;
        assert_eq!(seen.lock().unwrap().connections, 0, "nothing was dialled");
    }

    #[tokio::test]
    async fn a_pinned_host_key_opens_the_seal_first_and_a_host_that_cannot_prove_it_gets_no_token() {
        // The seal's frame goes before the token; a host answering anything but its key is refused with one
        // sentence, and the token never crosses.
        let refuse: Answer = Arc::new(|frame| vec![Act::Send(format!(r#"{{"id":{},"ok":true,"hostPublicKey":"x"}}"#, frame["id"]))]);
        let (port, seen) = host(refuse).await;
        let aim = HostAim::Url { url: format!("http://127.0.0.1:{port}"), token: Some("tok".into()), host_key: Some("SHA256:pinned".into()) };
        let e = dial_host("/nowhere/state.json", opts(aim)).await.err().expect("refused");
        assert_eq!(e.message, format!("the host at http://127.0.0.1:{port} did not prove the key this computer holds for it, so it is not that host; nothing was sent to it"));
        assert_eq!(e.kind, Some(Kind::Auth));
        settle().await;
        let seen = seen.lock().unwrap();
        assert_eq!(seen.frames.len(), 1, "only the seal's frame went: {:?}", seen.frames);
        let first: Value = serde_json::from_str(&seen.frames[0]).unwrap();
        assert_eq!(first["op"], "seal.open");
        assert_eq!(first["id"], 1);
        assert!(first["nonce"].is_string() && first["ephemeral"].is_string());
        assert!(!seen.frames[0].contains("tok"), "the token never crossed");
    }
}

mod requests {
    use super::*;

    #[tokio::test]
    async fn a_request_sends_its_params_first_then_id_then_op_with_ids_counting_every_frame() {
        let (port, seen) = host(answering(&[])).await;
        let client = dial(port).await;
        client.request("workspaces.list", Frame::new()).await.unwrap();
        client.request("usage.used", frame(json!({ "range": "day", "by": "agent" }))).await.unwrap();
        client.close();
        settle().await;
        let seen = seen.lock().unwrap();
        assert_eq!(seen.frames[1..], [r#"{"id":2,"op":"workspaces.list"}"#.to_owned(), r#"{"range":"day","by":"agent","id":3,"op":"usage.used"}"#.to_owned()]);
    }

    #[tokio::test]
    async fn stamps_its_own_id_and_op_over_any_the_params_carry_where_they_stand() {
        // dial-envelope.test.ts: a request cannot take another's reply. JS spread keeps a key where it first stood,
        // so an `op` or `id` among the params is overwritten in place.
        let (port, seen) = host(answering(&[])).await;
        let client = dial(port).await;
        client.request("sessions.list", frame(json!({ "op": "nope", "x": 1, "id": 99 }))).await.unwrap();
        client.close();
        settle().await;
        assert_eq!(seen.lock().unwrap().frames[1], r#"{"op":"sessions.list","x":1,"id":2}"#);
    }

    #[tokio::test]
    async fn replies_are_matched_by_id_whatever_order_they_come_in() {
        let answer: Answer = Arc::new(|frame| match frame["op"].as_str().unwrap() {
            "auth" => vec![Act::Send(format!(r#"{{"id":{},"ok":true}}"#, frame["id"]))],
            // The first request's answer waits until the second's has gone.
            "first" => vec![],
            _ => vec![Act::Send(format!(r#"{{"id":{},"ok":true,"which":"second"}}"#, frame["id"])), Act::Send(r#"{"id":2,"ok":true,"which":"first"}"#.to_owned())],
        });
        let (port, _seen) = host(answer).await;
        let client = dial(port).await;
        let (a, b) = tokio::join!(client.request("first", Frame::new()), async {
            settle().await;
            client.request("second", Frame::new()).await
        });
        assert_eq!(a.unwrap()["which"], "first");
        assert_eq!(b.unwrap()["which"], "second");
    }

    #[tokio::test]
    async fn a_refused_reply_carries_its_error_its_kind_and_its_lists_but_never_left() {
        let answer: Answer = Arc::new(|frame| {
            let id = &frame["id"];
            if frame["op"] == "auth" {
                return vec![Act::Send(format!(r#"{{"id":{id},"ok":true}}"#))];
            }
            vec![Act::Send(format!(r#"{{"id":{id},"ok":false,"error":"V750 the document moved","kind":"usage","errors":[{{"code":"V750"}}],"warnings":[],"left":[{{"id":"t"}}]}}"#))]
        });
        let (port, _seen) = host(answer).await;
        let e = dial(port).await.request("slate.write", Frame::new()).await.err().expect("refused");
        assert_eq!(e.message, "V750 the document moved");
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(e.errors, Some(vec![json!({ "code": "V750" })]));
        assert_eq!(e.warnings, Some(vec![]));
        assert_eq!(e.left, None, "left rides the reply and never reaches the failure (client.ts:266)");
    }

    #[tokio::test]
    async fn a_refusal_with_no_error_string_reads_op_failed_and_a_kind_that_is_no_string_is_dropped() {
        let answer: Answer = Arc::new(|frame| {
            let id = &frame["id"];
            match frame["op"].as_str().unwrap() {
                "auth" => vec![Act::Send(format!(r#"{{"id":{id},"ok":true}}"#))],
                "a" => vec![Act::Send(format!(r#"{{"id":{id},"ok":false}}"#))],
                "b" => vec![Act::Send(format!(r#"{{"id":{id},"ok":false,"error":7,"kind":3,"errors":"x"}}"#))],
                _ => vec![Act::Send(format!(r#"{{"id":{id},"ok":"true","error":"ok is a string"}}"#))],
            }
        });
        let (port, _seen) = host(answer).await;
        let client = dial(port).await;
        let a = client.request("a", Frame::new()).await.err().unwrap();
        assert_eq!((a.message.as_str(), a.kind.clone(), a.errors.clone()), ("a failed", None, None));
        let b = client.request("b", Frame::new()).await.err().unwrap();
        assert_eq!((b.message.as_str(), b.kind.clone(), b.errors.clone()), ("b failed", None, None));
        let c = client.request("c", Frame::new()).await.err().unwrap();
        assert_eq!(c.message, "ok is a string", "only ok === true is an answer");
        assert_eq!(c.exit_code(), 1);
    }

    #[tokio::test]
    async fn every_frame_that_answers_no_waiting_request_goes_to_every_sink_in_order_and_off_removes_one() {
        let answer: Answer = Arc::new(|frame| {
            let id = &frame["id"];
            if frame["op"] == "auth" {
                return vec![Act::Send(format!(r#"{{"id":{id},"ok":true}}"#))];
            }
            vec![
                Act::Send(r#"{"type":"session.delta","n":1}"#.to_owned()),
                Act::Send(r#"{"id":"2","type":"string id"}"#.to_owned()),
                Act::Send(r#"{"id":77,"type":"nobody waits on 77"}"#.to_owned()),
                Act::Send(format!(r#"{{"id":{id},"ok":true}}"#)),
                Act::Send(r#"{"type":"after","n":2}"#.to_owned()),
            ]
        });
        let (port, _seen) = host(answer).await;
        let client = dial(port).await;
        let a = Arc::new(Mutex::new(Vec::<String>::new()));
        let b = Arc::new(Mutex::new(Vec::<String>::new()));
        let (ra, rb) = (a.clone(), b.clone());
        let _keep = client.on_frame(Arc::new(move |f| ra.lock().unwrap().push(f["type"].as_str().unwrap().to_owned())));
        let off = client.on_frame(Arc::new(move |f| rb.lock().unwrap().push(f["type"].as_str().unwrap().to_owned())));
        client.request("go", Frame::new()).await.unwrap();
        off.off();
        client.request("go", Frame::new()).await.unwrap();
        settle().await;
        let first = ["session.delta", "string id", "nobody waits on 77", "after"];
        assert_eq!(a.lock().unwrap()[..], [&first[..], &first[..]].concat()[..]);
        assert_eq!(b.lock().unwrap()[..], first[..], "the removed sink saw the first round alone");
    }

    #[tokio::test]
    async fn events_subscribes_at_most_once_per_socket() {
        let (port, seen) = host(answering(&[])).await;
        let client = dial(port).await;
        client.events().await.unwrap();
        client.events().await.unwrap();
        client.close();
        settle().await;
        let subscribes = seen.lock().unwrap().frames.iter().filter(|f| f.contains("events.subscribe")).cloned().collect::<Vec<_>>();
        assert_eq!(subscribes, vec![r#"{"id":2,"op":"events.subscribe"}"#.to_owned()]);
    }

    #[tokio::test]
    async fn close_ends_the_socket_with_no_close_code_of_its_own() {
        let (port, seen) = host(answering(&[])).await;
        let client = dial(port).await;
        client.close();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(seen.lock().unwrap().closes, vec![None], "the host sees 1005: a close frame with no code");
        assert!(client.gone().is_some(), "the socket reads gone once closed");
    }
}

mod the_socket_going {
    use super::*;

    fn closing_on(op: &'static str, code: Option<u16>) -> Answer {
        Arc::new(move |frame| {
            let id = &frame["id"];
            if frame["op"] == op {
                return vec![Act::Close(code)];
            }
            vec![Act::Send(format!(r#"{{"id":{id},"ok":true}}"#))]
        })
    }

    #[tokio::test]
    async fn a_close_with_4001_fails_what_waits_with_the_stopping_line_and_no_kind() {
        let (port, _seen) = host(closing_on("sessions.start", Some(4001))).await;
        let client = dial(port).await;
        let e = client.request("sessions.start", Frame::new()).await.err().expect("failed");
        assert_eq!(e.message, HOST_STOPPING_LINE);
        assert_eq!(e.kind, None, "a plain error: exit 1");
        assert_eq!(client.closed().await, Some(4001));
        assert!(stopped_under(client.as_ref()));
        assert_eq!(client.close_words(), HOST_STOPPING_LINE);
    }

    #[tokio::test]
    async fn every_other_close_fails_them_with_the_closed_line_4401_included() {
        for code in [Some(1000), Some(4401), None] {
            let (port, _seen) = host(closing_on("x", code)).await;
            let client = dial(port).await;
            let e = client.request("x", Frame::new()).await.err().expect("failed");
            assert_eq!(e.message, HOST_CLOSED_LINE, "close {code:?}");
            assert_eq!(e.kind, None);
            assert!(!stopped_under(client.as_ref()), "close {code:?}");
            assert_eq!(client.close_words(), HOST_CLOSED_LINE);
        }
    }

    #[tokio::test]
    async fn stopped_under_reads_an_open_socket_as_not_stopped_without_waiting() {
        let (port, _seen) = host(answering(&[])).await;
        let client = dial(port).await;
        let read = tokio::time::timeout(Duration::from_millis(100), async { stopped_under(client.as_ref()) }).await;
        assert_eq!(read, Ok(false));
        assert_eq!(client.gone(), None);
    }
}

mod the_token_refused {
    use super::*;

    fn refusing(frame_text: &'static str, then_close: Option<u16>) -> Answer {
        Arc::new(move |frame| {
            let mut acts = vec![Act::Send(frame_text.replace("ID", &frame["id"].to_string()))];
            if let Some(code) = then_close {
                acts.push(Act::Close(Some(code)));
            }
            acts
        })
    }

    #[tokio::test]
    async fn a_reply_with_kind_auth_is_an_auth_refusal_in_the_host_s_own_words() {
        let (port, _seen) = host(refusing(r#"{"id":ID,"ok":false,"error":"this token is not the host's","kind":"auth"}"#, None)).await;
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.err().expect("refused");
        assert_eq!(e.message, "this token is not the host's");
        assert_eq!(e.kind, Some(Kind::Auth));
        assert_eq!(e.exit_code(), 2);
    }

    #[tokio::test]
    async fn a_reply_with_no_kind_behind_a_4401_close_within_the_grace_is_auth() {
        let (port, _seen) = host(refusing(r#"{"id":ID,"ok":false,"error":"unauthorized"}"#, Some(4401))).await;
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.err().expect("refused");
        assert_eq!(e.message, "unauthorized");
        assert_eq!(e.kind, Some(Kind::Auth), "an older host says auth with the close code alone");
    }

    #[tokio::test]
    async fn a_reply_with_no_kind_and_no_close_stands_as_it_came_once_the_grace_is_out() {
        let (port, _seen) = host(refusing(r#"{"id":ID,"ok":false,"error":"busy"}"#, None)).await;
        let started = std::time::Instant::now();
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.err().expect("refused");
        assert_eq!(e.message, "busy");
        assert_eq!(e.kind, None);
        assert!(started.elapsed() >= Duration::from_millis(500), "it waited the 500 ms grace for a close: {:?}", started.elapsed());
    }

    #[tokio::test]
    async fn a_kind_other_than_auth_on_the_token_s_frame_is_the_refusal_as_it_came() {
        let (port, _seen) = host(refusing(r#"{"id":ID,"ok":false,"error":"no such op","kind":"usage"}"#, None)).await;
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.err().expect("refused");
        assert_eq!((e.message.as_str(), e.kind.clone()), ("no such op", Some(Kind::Usage)));
    }
}

mod the_release {
    use super::*;

    fn versioned(version: &'static str, road: &'static str) -> Answer {
        Arc::new(move |frame| vec![Act::Send(format!(r#"{{"id":{},"ok":true,"version":"{version}","road":"{road}"}}"#, frame["id"]))])
    }

    #[tokio::test]
    async fn an_older_host_on_loopback_is_refused_naming_both_releases_and_its_road_s_fix_before_anything_is_asked() {
        let (port, seen) = host(versioned("0.0.1", "up")).await;
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.err().expect("refused");
        assert_eq!(
            e.message,
            format!("this line runs wsp {VERSION} and the host at http://127.0.0.1:{port} runs wsp 0.0.1: Ctrl-C the terminal running wsp up and run it again.")
        );
        assert_eq!(e.kind, Some(Kind::Usage));
        settle().await;
        assert_eq!(seen.lock().unwrap().frames.len(), 1, "only the auth frame went");
    }

    #[tokio::test]
    async fn any_release_dials_past_another_release_and_a_host_naming_none_is_not_judged() {
        let (port, _seen) = host(versioned("0.0.1", "app")).await;
        let past = dial_host("/nowhere/state.json", DialOpts { any_release: true, ..opts(url_aim(port, Some("tok"))) }).await;
        assert!(past.is_ok(), "{:?}", past.err());
        let (port, _seen) = host(answering(&[])).await;
        assert!(dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.is_ok());
        let (port, _seen) = host(Arc::new(|f| vec![Act::Send(format!(r#"{{"id":{},"ok":true,"version":7}}"#, f["id"]))])).await;
        assert!(dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.is_ok(), "a version that is no string is not judged");
    }

    #[tokio::test]
    async fn the_same_release_dials_as_before() {
        let (port, _seen) = host(Arc::new(|f| vec![Act::Send(format!(r#"{{"id":{},"ok":true,"version":"{VERSION}"}}"#, f["id"]))])).await;
        let client = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.unwrap();
        assert!(client.request("x", Frame::new()).await.is_ok());
    }
}

mod the_deadline {
    use super::*;

    #[tokio::test]
    async fn a_port_that_accepts_and_never_upgrades_fails_within_the_deadline_naming_the_address() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((tcp, _)) = listener.accept().await {
                held.push(tcp);
            }
        });
        let started = std::time::Instant::now();
        let e = dial_host("/nowhere/state.json", DialOpts { deadline_ms: Some(300), ..opts(url_aim(port, Some("tok"))) }).await.err().expect("refused");
        assert_eq!(e.message, format!("the host at http://127.0.0.1:{port} did not answer: nothing came back within 300 ms"));
        assert_eq!(e.kind, Some(Kind::Unreachable));
        assert!(started.elapsed() < Duration::from_millis(1_500), "{:?}", started.elapsed());
    }

    #[tokio::test]
    async fn a_host_that_upgrades_and_never_answers_the_token_fails_within_the_same_deadline() {
        let (port, _seen) = host(Arc::new(|_| vec![])).await;
        let e = dial_host("/nowhere/state.json", DialOpts { deadline_ms: Some(300), ..opts(url_aim(port, Some("tok"))) }).await.err().expect("refused");
        assert_eq!(e.message, format!("the host at http://127.0.0.1:{port} did not answer: nothing came back within 300 ms"));
    }

    #[tokio::test]
    async fn a_refused_connection_is_a_host_that_did_not_answer_with_the_road_s_reason() {
        let port = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let e = dial_host("/nowhere/state.json", opts(url_aim(port, Some("tok")))).await.err().expect("refused");
        assert!(e.message.starts_with(&format!("the host at http://127.0.0.1:{port} did not answer: ")), "{}", e.message);
        assert_eq!(e.kind, Some(Kind::Unreachable));
    }
}

mod the_address {
    use super::*;

    fn served(dir: &std::path::Path, lock: Value, token: Option<&str>) -> String {
        std::fs::write(dir.join("host.lock"), lock.to_string()).unwrap();
        if let Some(token) = token {
            std::fs::write(dir.join("host-token"), token).unwrap();
        }
        dir.join("state.json").to_string_lossy().into_owned()
    }

    #[test]
    fn a_host_here_is_its_lock_s_address_on_loopback_and_the_trimmed_token_beside_the_state() {
        let dir = tempfile::tempdir().unwrap();
        let state = served(dir.path(), json!({ "pid": std::process::id(), "port": 7777, "startedAt": "2026-10-25T00:00:00.000Z" }), Some("tok\n"));
        let at = host_address(&state, &HostPick::default(), Some(&HostAim::Here)).unwrap();
        assert_eq!(at, HostAddress { url: "ws://127.0.0.1:7777/ws".to_owned(), token: "tok".to_owned() });
    }

    #[test]
    fn a_wildcard_bind_is_dialled_on_loopback_and_a_named_address_as_it_is() {
        let dir = tempfile::tempdir().unwrap();
        let state = served(dir.path(), json!({ "pid": std::process::id(), "port": 7777, "address": "0.0.0.0", "startedAt": "2026-10-25T00:00:00.000Z" }), Some("t"));
        assert_eq!(host_address(&state, &HostPick::default(), Some(&HostAim::Here)).unwrap().url, "ws://127.0.0.1:7777/ws");
        let state = served(dir.path(), json!({ "pid": std::process::id(), "port": 7777, "address": "10.0.0.5", "startedAt": "2026-10-25T00:00:00.000Z" }), Some("t"));
        assert_eq!(host_address(&state, &HostPick::default(), Some(&HostAim::Here)).unwrap().url, "ws://10.0.0.5:7777/ws");
    }

    #[test]
    fn no_lock_is_no_host_serving_the_state_a_plain_error_naming_the_line_that_serves_it() {
        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("state.json").to_string_lossy().into_owned();
        let e = host_address(&state, &HostPick::default(), Some(&HostAim::Here)).err().expect("refused");
        assert_eq!(e.message, format!("no wsp host is serving {state}; start one with wsp up --state {state}"));
        assert_eq!(e.kind, None);
    }

    #[test]
    fn a_lock_with_no_token_beside_it_is_an_auth_refusal_naming_the_token_file() {
        let dir = tempfile::tempdir().unwrap();
        let state = served(dir.path(), json!({ "pid": std::process::id(), "port": 7777, "startedAt": "2026-10-25T00:00:00.000Z" }), None);
        let e = host_address(&state, &HostPick::default(), Some(&HostAim::Here)).err().expect("refused");
        assert_eq!(e.message, format!("the host's token file is missing: {}", dir.path().join("host-token").display()));
        assert_eq!(e.kind, Some(Kind::Auth));
    }

    #[test]
    fn a_url_aim_is_its_ws_url_with_its_token_or_none_and_an_https_one_is_wss() {
        let pick = HostPick::default();
        let at = host_address("/s.json", &pick, Some(&HostAim::Url { url: "http://127.0.0.1:7777/".into(), token: Some("t".into()), host_key: None })).unwrap();
        assert_eq!(at, HostAddress { url: "ws://127.0.0.1:7777/ws".into(), token: "t".into() });
        let at = host_address("/s.json", &pick, Some(&HostAim::Url { url: "https://h.example/base".into(), token: None, host_key: None })).unwrap();
        assert_eq!(at, HostAddress { url: "wss://h.example/base/ws".into(), token: String::new() });
    }
}

mod waiting {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn host_again_dials_every_poll_handing_each_try_what_is_left_until_one_opens() {
        let host = ScriptedHost::new(Script::default());
        let tries = Arc::new(Mutex::new(Vec::<u64>::new()));
        let started = tokio::time::Instant::now();
        let noted = tries.clone();
        let host2 = host.clone();
        let dial = move |within: u64| {
            let noted = noted.clone();
            let host = host2.clone();
            boxed(async move {
                let n = {
                    let mut t = noted.lock().unwrap();
                    t.push(within);
                    t.len()
                };
                if n < 4 {
                    Err(HostError::new("no wsp host is serving it"))
                } else {
                    Ok(ScriptedClient::new(host.script.clone(), host.state.clone()) as Arc<dyn HostClient>)
                }
            })
        };
        let back = host_again(&dial, 20_000).await;
        assert!(back.is_ok());
        let tries = tries.lock().unwrap().clone();
        assert_eq!(tries, vec![20_000, 19_800, 19_600, 19_400], "each try is handed what is left of the window, 200 ms apart");
        assert_eq!(started.elapsed(), Duration::from_millis(600));
    }

    #[tokio::test(start_paused = true)]
    async fn a_wait_whose_host_never_comes_back_answers_the_last_dial_s_own_refusal_once_the_window_is_out() {
        let n = Arc::new(Mutex::new(0));
        let counted = n.clone();
        let dial = move |_within: u64| {
            let counted = counted.clone();
            boxed(async move {
                *counted.lock().unwrap() += 1;
                let k = *counted.lock().unwrap();
                tokio::time::sleep(Duration::from_millis(150)).await;
                Err::<Arc<dyn HostClient>, _>(HostError::new(format!("refusal {k}")))
            })
        };
        let started = tokio::time::Instant::now();
        let e = host_again(&dial, 1_000).await.err().expect("gave up");
        let k = *n.lock().unwrap();
        assert_eq!(e.message, format!("refusal {k}"), "the last dial's own refusal stands");
        assert!(started.elapsed() >= Duration::from_millis(1_000) && started.elapsed() < Duration::from_millis(1_400), "{:?}", started.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn a_try_is_never_handed_less_than_one_millisecond() {
        let seen = Arc::new(Mutex::new(Vec::<u64>::new()));
        let noted = seen.clone();
        let dial = move |within: u64| {
            let noted = noted.clone();
            boxed(async move {
                noted.lock().unwrap().push(within);
                tokio::time::sleep(Duration::from_millis(within)).await;
                Err::<Arc<dyn HostClient>, _>(HostError::new("late"))
            })
        };
        let _ = host_again(&dial, 250).await;
        let seen = seen.lock().unwrap().clone();
        assert!(seen.iter().all(|w| *w >= 1), "{seen:?}");
        assert_eq!(seen[0], 250);
    }

    #[tokio::test]
    async fn until_settled_answers_the_work_or_the_close_words_when_the_socket_goes_first() {
        let host = ScriptedHost::new(Script::default());
        let socket = ScriptedClient::new(host.script.clone(), host.state.clone());
        let done = until_settled(socket.as_ref(), boxed(async { Ok::<_, HostError>(5) })).await;
        assert_eq!(done.unwrap(), 5);
        let gone = socket.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            gone.end(Some(STOPPING_CLOSE));
        });
        let e = until_settled(socket.as_ref(), boxed(std::future::pending::<Result<u8, HostError>>())).await.err().expect("failed");
        assert_eq!(e.message, HOST_STOPPING_LINE);
        assert_eq!(e.kind, None);
    }

    #[test]
    fn the_restarting_line_is_the_one_a_wait_says_while_it_dials() {
        assert_eq!(HOST_RESTARTING_LINE, "the host is restarting; waiting for it to come back");
        let io = RecordingIo::new();
        assert!(io.errors().is_empty());
    }
}

mod pushed_frames {
    use super::*;

    fn types(seen: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        seen.lock().unwrap().clone()
    }

    #[tokio::test]
    async fn frames_pushed_before_follow_are_held_and_replayed_in_order_through_the_pick() {
        let host = ScriptedHost::new(Script::default());
        let socket = ScriptedClient::new(host.script.clone(), host.state.clone());
        let pushed = PushedFrames::new(socket.as_ref());
        socket.push(r#"{"type":"exec.output","execId":"e-0","text":"someone else's"}"#);
        socket.push(r#"{"type":"exec.output","execId":"e-1","text":"one"}"#);
        socket.push(r#"{"type":"exec.output","execId":"e-1","text":"two"}"#);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let noted = seen.clone();
        pushed.follow(Arc::new(|f| f["execId"] == "e-1"), Arc::new(move |f| noted.lock().unwrap().push(f["text"].as_str().unwrap().to_owned())));
        assert_eq!(types(&seen), vec!["one", "two"]);
        socket.push(r#"{"type":"exec.output","execId":"e-1","text":"three"}"#);
        socket.push(r#"{"type":"exec.output","execId":"e-2","text":"not mine"}"#);
        assert_eq!(types(&seen), vec!["one", "two", "three"]);
        pushed.stop();
        socket.push(r#"{"type":"exec.output","execId":"e-1","text":"after stop"}"#);
        assert_eq!(types(&seen), vec!["one", "two", "three"], "nothing reaches the sink after stop");
    }

    #[tokio::test]
    async fn a_stop_from_inside_the_sink_drops_the_held_frames_not_yet_replayed() {
        let host = ScriptedHost::new(Script::default());
        let socket = ScriptedClient::new(host.script.clone(), host.state.clone());
        let pushed = Arc::new(PushedFrames::new(socket.as_ref()));
        for n in 1..=4 {
            socket.push(&format!(r#"{{"type":"session.delta","n":{n}}}"#));
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let noted = seen.clone();
        let stopper = pushed.clone();
        pushed.follow(
            Arc::new(|_| true),
            Arc::new(move |f| {
                noted.lock().unwrap().push(f["n"].to_string());
                if f["n"] == 2 {
                    stopper.stop();
                }
            }),
        );
        assert_eq!(types(&seen), vec!["1", "2"], "a whole turn may sit in the held frames; a stop ends it there");
    }
}
