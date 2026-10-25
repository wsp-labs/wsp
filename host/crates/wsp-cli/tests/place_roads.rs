// SPDX-License-Identifier: AGPL-3.0-only
//! A sign-in run on a computer you own and shown here, the pty road to that computer's daemon, one add or setup
//! followed on the host's socket, and the add tool. Ported from packages/host/test/place-signin.test.ts and the
//! setup-follow cases of places-add.test.ts at 65f6b89c7. The spec is spec/place_roads.md.
mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::*;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_cli::place_signin::*;
use wsp_cli::setup_follow::*;
use wsp_cli::verbs::client::HostClient;
use wsp_wire::types::agents_report::{AgentsTarget, SignInLine};
use wsp_words::exit::{HostError, Kind};

fn codex_line() -> SignInLine {
    serde_json::from_value(json!({"command": "codex login --device-auth", "env": {"CODEX_HOME": "/wsp/logins/codex"}, "prepare": "mkdir -p '/wsp/logins/codex'", "status": "codex login status"})).unwrap()
}

fn sign_in(link: &Arc<FakeLink>, line: SignInLine, agent: Option<&str>) -> BoxSignIn {
    BoxSignIn {
        r#where: "hetzner".into(),
        link: Arc::new(Link(link.clone())),
        agent: agent.map(String::from),
        line,
        open: Arc::new(|_| async { true }.boxed()),
        timeout_ms: Some(2_000),
    }
}

// ---- the sign-in the host planned

/// place-signin.test.ts:45: a refused or failing first step stops before the pty, in one sentence.
#[tokio::test]
async fn a_refused_or_failing_first_step_stops_before_the_pty_naming_the_computer_and_its_last_line() {
    let cases = [
        (json!({"ok": false, "error": "exec is not served on this channel", "kind": "refused"}), "exec is not served on this channel"),
        (
            json!({"ok": true, "exitCode": 1, "stdout": "", "stderr": "mkdir: cannot create directory '/wsp/logins/codex': Permission denied\n", "truncated": false}),
            "mkdir: cannot create directory '/wsp/logins/codex': Permission denied",
        ),
        (json!({"exitCode": 2, "stdout": "only stdout said it\n", "stderr": "", "truncated": false}), "only stdout said it"),
        (json!({"exitCode": 3, "stdout": "", "stderr": ""}), "it exited 3"),
        (json!({"ok": false, "error": ""}), "the computer refused it"),
        (json!({"ok": false}), "the computer refused it"),
    ];
    for (reply, said) in cases {
        let link = FakeLink::new();
        let r = reply.clone();
        *link.answer_exec.lock().unwrap() =
            Some(Arc::new(move |cmd| if cmd == "mkdir -p '/wsp/logins/codex'" { Some(frame(r.clone())) } else { None }));
        let (_t, mut rt) = terminal(false, 100, 30);
        let e = relay_sign_in(sign_in(&link, codex_line(), Some("codex")), &mut rt).await.unwrap_err();
        assert_eq!(e.message, format!("the sign-in on hetzner did not start, since the step before it failed: {said}"), "{reply}");
        assert_eq!(e.kind, None);
        assert!(link.ops().iter().all(|(op, _)| op != "pty.create"), "{reply}");
    }
    assert_eq!(sign_in_prepare_line("box", "x"), "the sign-in on box did not start, since the step before it failed: x");
}

/// place-signin.ts:67-74: a link that fails the op is the computer's refusal in its own words.
#[tokio::test]
async fn a_first_step_the_link_fails_is_read_as_the_link_s_own_words() {
    struct Failing;
    #[async_trait::async_trait]
    impl wsp_cli::signin_relay::PtyLink for Failing {
        async fn op(&self, _op: &str, _extra: wsp_cli::signin_relay::Frame) -> Result<wsp_cli::signin_relay::Frame, HostError> {
            Err(HostError::new("daemon connection closed 1006"))
        }
        fn on_event(&self, _sink: wsp_cli::signin_relay::EventSink) -> wsp_cli::signin_relay::Detach {
            wsp_cli::signin_relay::Detach(Box::new(|| {}))
        }
        fn closed(&self) -> Option<futures::future::BoxFuture<'static, ()>> {
            None
        }
        fn bare(&self) -> Option<bool> {
            None
        }
    }
    assert_eq!(prepare_failed(&Failing, "true").await.as_deref(), Some("daemon connection closed 1006"));
    let link = FakeLink::new();
    assert_eq!(prepare_failed(&Link(link.clone()), "true").await, None);
    *link.answer_exec.lock().unwrap() = Some(Arc::new(|_| Some(frame(json!({"exitCode": "1"})))));
    assert_eq!(prepare_failed(&Link(link), "true").await, None, "an exit code that is not a number is a step that ran");
}

/// place-signin.test.ts:62.
#[tokio::test]
async fn the_first_step_runs_then_the_command_with_its_environment_then_the_status_the_same_way() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if line.contains("WSP_STATUS") {
            l.data(pty, "Logged in using ChatGPT\r\nWSP_STATUS 0\r\n");
            l.exit(pty, 0);
            return;
        }
        if line.contains("; exec bash -c ") {
            l.data(pty, "Open https://auth.openai.com/device and enter CODE-1234\r\n");
            l.exit(pty, 0);
        }
    });
    let (_t, mut rt) = terminal(true, 100, 30);
    let answer = relay_sign_in(sign_in(&link, codex_line(), Some("codex")), &mut rt).await.unwrap();
    let execs: Vec<Value> = link.execs_beside().into_iter().map(|e| e["cmd"].clone()).collect();
    assert_eq!(execs, vec![json!("mkdir -p '/wsp/logins/codex'")]);
    assert!(link.staged().iter().all(|s| s.cleared));
    let ptys = link.ptys();
    assert_eq!(ptys[0].created.get("env"), Some(&json!({"CODEX_HOME": "/wsp/logins/codex"})));
    assert_eq!(ptys[0].ran.as_deref(), Some("codex login --device-auth"));
    assert_eq!(ptys[1].created.get("env"), Some(&json!({"PS1": "", "CODEX_HOME": "/wsp/logins/codex"})));
    assert_eq!(ptys[1].ran.as_deref(), Some("codex login status"));
    assert_eq!(answer, BoxSignedIn { signed_in: true, detail: None, said: None });
    assert_eq!(serde_json::to_string(&answer).unwrap(), r#"{"signedIn":true}"#);
}

/// place-signin.test.ts:82.
#[tokio::test]
async fn a_status_that_says_nothing_of_a_login_is_not_signed_in_and_carries_what_the_tool_said() {
    let link = FakeLink::new();
    link.set_script(|l, pty, line| {
        if line.contains("WSP_STATUS") {
            l.data(pty, "Not logged in\r\nWSP_STATUS 1\r\n");
            l.exit(pty, 0);
            return;
        }
        if line.contains("; exec bash -c ") {
            l.exit(pty, 1);
        }
    });
    let (_t, mut rt) = terminal(true, 100, 30);
    let answer = relay_sign_in(sign_in(&link, codex_line(), Some("codex")), &mut rt).await.unwrap();
    assert_eq!(answer, BoxSignedIn { signed_in: false, detail: None, said: Some("Not logged in".into()) });
}

/// place-signin.test.ts:95: one server's sign-in, read by its own exit.
#[tokio::test]
async fn a_line_with_nothing_first_and_no_status_is_read_by_its_own_exit() {
    for (code, signed) in [(0, true), (1, false)] {
        let link = FakeLink::new();
        link.set_script(move |l, pty, line| {
            if line.contains("; exec bash -c ") {
                l.exit(pty, code);
            }
        });
        let server: SignInLine =
            serde_json::from_value(json!({"command": "runuser -u 'ada' -- bash -c 'claude mcp login notion --no-browser'"})).unwrap();
        let (_t, mut rt) = terminal(true, 100, 30);
        let answer = relay_sign_in(sign_in(&link, server, None), &mut rt).await.unwrap();
        assert_eq!(link.execs_beside(), vec![]);
        assert_eq!(link.ptys()[0].created.get("env"), None);
        assert_eq!(link.ptys().len(), 1, "no status run");
        assert_eq!(answer, BoxSignedIn { signed_in: signed, detail: None, said: None });
    }
    assert_eq!(BOX_SIGN_IN_MS, 600_000);
    assert_eq!(STATUS_MS, 20_000);
}

// ---- the pty road to a computer you own

fn road_client() -> Arc<FakeClient> {
    FakeClient::new(|op, _| match op {
        "daemon.open" => Ok(frame(json!({"ok": true, "channel": "ch1"}))),
        "daemon.send" => Ok(frame(json!({"ok": true, "reply": {"ok": true, "ptyId": "pty_1"}}))),
        _ => Ok(frame(json!({"ok": true}))),
    })
}

/// place-signin.test.ts:133.
#[tokio::test]
async fn one_channel_is_opened_the_ptys_frames_ride_it_and_only_its_own_events_come_back() {
    let c = road_client();
    let client: Arc<dyn HostClient> = Arc::new(Client(c.clone()));
    let road = place_link(client.clone(), "p1").await.unwrap();
    assert_eq!(c.sent(), vec![("daemon.open".to_string(), frame(json!({"placeId": "p1"})))]);
    let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
    let s = seen.clone();
    let _detach = road.link.on_event(Arc::new(move |e| s.lock().unwrap().push(Value::Object(e.clone()))));
    let reply = road.link.op("pty.create", frame(json!({"cols": 80, "rows": 24}))).await.unwrap();
    assert_eq!(Value::Object(reply), json!({"ok": true, "ptyId": "pty_1"}));
    let (op, params) = c.sent().last().unwrap().clone();
    assert_eq!(op, "daemon.send");
    assert_eq!(serde_json::to_string(&params).unwrap(), r#"{"channel":"ch1","frame":{"cols":80,"rows":24,"op":"pty.create"}}"#);
    c.push(json!({"type": "daemon.event", "channel": "ch2", "event": {"type": "pty.data", "data": "not mine"}}));
    c.push(json!({"type": "daemon.event", "channel": "ch1", "event": {"type": "pty.data", "data": "mine"}}));
    c.push(json!({"type": "daemon.event", "channel": "ch1", "event": "not an object"}));
    c.push(json!({"type": "daemon.event", "channel": "ch1", "event": null}));
    assert_eq!(*seen.lock().unwrap(), vec![json!({"type": "pty.data", "data": "mine"})]);
    let closed = road.link.closed().expect("the road says when it is gone");
    c.push(json!({"type": "daemon.closed", "channel": "ch2", "code": 1006}));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), road.link.closed().unwrap()).await.is_err(),
        "another channel's close is not this one's"
    );
    c.push(json!({"type": "daemon.closed", "channel": "ch1", "code": 1006, "reason": "the computer went"}));
    tokio::time::timeout(Duration::from_secs(1), closed).await.expect("the link going away settles closed");
    assert_eq!(road.link.bare(), Some(true), "a place id other than here is a computer somebody owns");
    let listeners = c.listeners();
    road.close().await;
    assert_eq!(c.sent().last().unwrap().clone(), ("daemon.close".to_string(), frame(json!({"channel": "ch1"}))));
    assert_eq!(c.listeners(), listeners - 1, "close takes the road's frame listener off");
    let ws = target_link(client, &serde_json::from_value::<AgentsTarget>(json!({"workspaceId": "ws_1"})).unwrap()).await.unwrap();
    assert_eq!(c.sent().last().unwrap().clone(), ("daemon.open".to_string(), frame(json!({"workspaceId": "ws_1"}))));
    assert_eq!(ws.link.bare(), Some(false));
}

/// place-signin.ts:58-60: a refused daemon.close is swallowed, and close settles closed for a waiting relay.
#[tokio::test]
async fn closing_the_road_swallows_a_refused_close_and_settles_closed() {
    let c = FakeClient::new(|op, _| match op {
        "daemon.open" => Ok(frame(json!({"ok": true, "channel": "ch9"}))),
        "daemon.close" => Err(HostError::new("no channel ch9")),
        _ => Ok(frame(json!({"ok": true}))),
    });
    let road = place_link(Arc::new(Client(c.clone())), "p1").await.unwrap();
    let closed = road.link.closed().unwrap();
    road.close().await;
    tokio::time::timeout(Duration::from_secs(1), closed).await.expect("close settles closed");
}

/// place-signin.ts:33: a daemon.open the host refuses is the host's refusal, unchanged.
#[tokio::test]
async fn a_refused_daemon_open_is_the_hosts_refusal() {
    let c = FakeClient::new(|_, _| Err(HostError::with_kind("spoo is not connected", Kind::Usage)));
    let e = place_link(Arc::new(Client(c)), "p1").await.err().unwrap();
    assert_eq!((e.message.as_str(), e.kind), ("spoo is not connected", Some(Kind::Usage)));
}

// ---- following one add or setup

/// setup-follow.ts:27-68: stage lines, setup lines, waits by row as they last stood, the end with what it said, and
/// a failed stage's note; frames of another stream are not this one's until also() takes it.
#[tokio::test]
async fn a_followed_add_hears_its_lines_waits_and_end_and_nothing_of_another_stream() {
    let c = FakeClient::new(|_, _| Ok(frame(json!({"ok": true}))));
    let lines: Arc<Mutex<Vec<Value>>> = Arc::default();
    let waits: Arc<Mutex<Vec<Value>>> = Arc::default();
    let (l, w) = (lines.clone(), waits.clone());
    let say = SetupSay {
        line: Arc::new(move |x| l.lock().unwrap().push(serde_json::to_value(x).unwrap())),
        wait: Arc::new(move |x| w.lock().unwrap().push(serde_json::to_value(x).unwrap())),
    };
    let mut watch = watch_setup(&Client(c.clone()), "a_x", Some(say));
    let wait = json!({"row": "signins/codex", "label": "Codex", "url": "https://auth.openai.com/codex/device", "code": "ABCD-EFGH", "expiresAt": "2026-10-03T10:15:00.000Z", "state": "waiting"});
    c.push(json!({"type": "place.stage", "addId": "a_x", "step": "connect", "state": "done", "note": "Ubuntu 24.04", "ms": 10}));
    c.push(json!({"type": "place.stage", "addId": "a_other", "step": "connect", "state": "done"}));
    c.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "line": {"step": "floor", "state": "done", "ms": 41000}}));
    c.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "wait": wait}));
    let mut landed = wait.clone();
    landed["state"] = json!("expired");
    c.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "wait": landed}));
    c.push(json!({"type": "place.stage", "addId": "a_x", "step": "join", "state": "failed", "note": "the join refused"}));
    c.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "end": "needs-you", "said": "a sign-in waits"}));
    c.push(json!({"type": "place.setup", "addId": "a_x"}));
    c.push(json!({"type": "something.else", "addId": "a_x"}));
    {
        let heard = watch.heard.lock().unwrap();
        let got: Vec<Value> = heard.lines.iter().map(|l| serde_json::to_value(l).unwrap()).collect();
        assert_eq!(
            got,
            vec![
                json!({"step": "connect", "state": "done", "note": "Ubuntu 24.04"}),
                json!({"step": "floor", "state": "done", "ms": 41000}),
                json!({"step": "join", "state": "failed", "note": "the join refused"})
            ]
        );
        assert_eq!(heard.waits.len(), 1);
        assert_eq!(serde_json::to_value(&heard.waits["signins/codex"]).unwrap(), landed);
        assert_eq!(heard.failed.as_deref(), Some("the join refused"));
        let end = heard.end.clone().unwrap();
        assert_eq!((serde_json::to_value(&end.end).unwrap(), end.said.as_deref()), (json!("needs-you"), Some("a sign-in waits")));
    }
    assert_eq!(lines.lock().unwrap().len(), 3);
    assert_eq!(*waits.lock().unwrap(), vec![wait.clone(), landed.clone()]);

    watch.also("a_other");
    c.push(json!({"type": "place.setup", "addId": "a_other", "placeId": "p_1", "end": "ready"}));
    assert_eq!(serde_json::to_value(&watch.heard.lock().unwrap().end.clone().unwrap().end).unwrap(), json!("ready"));
    assert_eq!(watch.heard.lock().unwrap().end.clone().unwrap().said, None, "a later end without said drops the earlier said");
    watch.off();
    c.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "line": {"step": "clis", "state": "done"}}));
    assert_eq!(watch.heard.lock().unwrap().lines.len(), 3);
    assert_eq!(c.listeners(), 0);
}

/// setup-follow.ts:20 and :28-34: next() settles at the next line, wait or end heard after it was asked.
#[tokio::test]
async fn next_settles_at_the_next_thing_heard_and_not_before() {
    let c = FakeClient::new(|_, _| Ok(frame(json!({"ok": true}))));
    let watch = watch_setup(&Client(c.clone()), "a_x", None);
    c.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "line": {"step": "floor", "state": "running"}}));
    assert!(
        tokio::time::timeout(Duration::from_millis(20), watch.next()).await.is_err(),
        "what was heard before the ask does not settle it"
    );
    let c2 = c.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        c2.push(json!({"type": "place.setup", "addId": "a_x", "placeId": "p_1", "end": "ready"}));
    });
    tokio::time::timeout(Duration::from_secs(1), watch.next()).await.expect("the end settles it");
    c.push(json!({"type": "place.stage", "addId": "a_other", "step": "connect", "state": "done"}));
    assert!(tokio::time::timeout(Duration::from_millis(20), watch.next()).await.is_err(), "another stream's frame settles nothing");
}

/// setup-follow.ts:71.
#[test]
fn the_open_waits_are_the_ones_still_waiting() {
    let waits: Vec<wsp_wire::views::place::PlaceWait> = serde_json::from_value(json!([
        {"row": "a", "label": "A", "expiresAt": "x", "state": "waiting"},
        {"row": "b", "label": "B", "expiresAt": "x", "state": "expired"},
        {"row": "c", "label": "C", "expiresAt": "x", "state": "waiting"}
    ]))
    .unwrap();
    let rows: Vec<String> = open_waits(&waits).into_iter().map(|w| w.row).collect();
    assert_eq!(rows, vec!["a", "c"]);
}

/// setup-follow.ts:90: user@host or an ssh alias; never a path, a repo, a URL or a provider's word.
#[test]
fn the_tool_takes_a_computer_word_and_nothing_else() {
    for word in ["spoo", "root@10.0.0.9", "dev@box.local", "my-box"] {
        assert!(is_computer_word(word), "{word}");
    }
    for word in ["/Users/dev/code/app", "https://github.com/dev/app.git", "app.git", "~/code", "./app", ".hidden", "box", "host:22", "a/b"]
    {
        assert!(!is_computer_word(word), "{word}");
    }
}

// ---- the add tool

const WAIT: &str = r#"{"row":"signins/codex","label":"Codex","url":"https://auth.openai.com/codex/device","code":"ABCD-EFGH","expiresAt":"2026-10-03T10:15:00.000Z","state":"waiting"}"#;

fn place() -> Value {
    json!({"id": "p_1", "kind": "computer", "name": "spoo", "default": true, "present": true, "takesForks": true})
}

#[derive(Default, Clone, Copy)]
struct Plays {
    sign_in: bool,
    failed: bool,
    running: bool,
}

/// places-add.test.ts:39-95: a host that plays one add after its reply; the row it lists moves with what it said.
fn fake_host(o: Plays) -> Arc<FakeClient> {
    let row: Arc<Mutex<Value>> = Arc::new(Mutex::new(place()));
    let c = FakeClient::new(|_, _| Ok(frame(json!({}))));
    let weak = Arc::downgrade(&c);
    let row2 = row.clone();
    *c.answer.lock().unwrap() = Box::new(move |op, params| {
        if op == "places.list" {
            return Ok(frame(json!({"ok": true, "places": [row2.lock().unwrap().clone()]})));
        }
        if op != "places.add" && op != "places.setup" {
            return Err(HostError::new(format!("unexpected op {op}")));
        }
        let add_id =
            if o.running { "a_running".to_string() } else { params.get("addId").and_then(Value::as_str).unwrap_or("a_tool").to_string() };
        let running = json!({"state": "running", "addId": add_id, "startedAt": "2026-10-03T10:01:00.000Z", "steps": [], "waiting": []});
        let mut r = place();
        r["setup"] = running.clone();
        *row2.lock().unwrap() = r.clone();
        let (weak, row3, add) = (weak.clone(), row2.clone(), add_id.clone());
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let Some(c) = weak.upgrade() else { return };
            c.push(json!({"type": "place.stage", "addId": add, "step": "connect", "state": "done", "note": "Ubuntu 24.04"}));
            c.push(json!({"type": "place.setup", "addId": add, "placeId": "p_1", "line": {"step": "floor", "state": "done", "ms": 41000}}));
            if o.sign_in {
                c.push(
                    json!({"type": "place.setup", "addId": add, "placeId": "p_1", "wait": serde_json::from_str::<Value>(WAIT).unwrap()}),
                );
            }
            c.push(json!({"type": "place.setup", "addId": add, "placeId": "p_1", "line": {"step": "clis", "state": "done", "ms": 9000}}));
            let state = if o.failed { "failed" } else { "done" };
            let waiting = if o.sign_in { json!([serde_json::from_str::<Value>(WAIT).unwrap()]) } else { json!([]) };
            let mut done = running.clone();
            done["state"] = json!(state);
            done["finishedAt"] = json!("x");
            done["steps"] = json!([{"step": "floor", "state": "done", "ms": 41000}, {"step": "clis", "state": "done", "ms": 9000}]);
            done["waiting"] = waiting;
            let mut r = place();
            r["setup"] = done;
            *row3.lock().unwrap() = r;
            let end = if o.failed {
                "failed"
            } else if o.sign_in {
                "needs-you"
            } else {
                "ready"
            };
            c.push(json!({"type": "place.setup", "addId": add, "placeId": "p_1", "end": end}));
        });
        Ok(frame(json!({"ok": true, "addId": add_id, "place": r})))
    });
    c
}

fn ask(address: &str) -> AddAsk {
    AddAsk { address: address.into(), ..AddAsk::default() }
}

fn wait_value() -> Value {
    serde_json::from_str(WAIT).unwrap()
}

/// places-add.test.ts:256: answers at the first sign-in waiting on the person, never waiting on them.
#[tokio::test]
async fn the_add_tool_answers_at_the_first_sign_in_waiting_on_the_person() {
    let host = fake_host(Plays { sign_in: true, ..Plays::default() });
    let answer = add_computer(Arc::new(Client(host.clone())), &AddAsk { recipe: Some("laptop".into()), ..ask("root@10.0.0.9") }, None)
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(&answer.waiting).unwrap(), json!([wait_value()]));
    let steps: Vec<Value> = answer.setup.iter().map(|l| serde_json::to_value(&l.step).unwrap()).collect();
    assert_eq!(steps, vec![json!("floor"), json!("clis")]);
    assert_eq!(answer.computer.id, "p_1");
    let asked: Vec<(String, String)> =
        host.sent().into_iter().filter(|(op, _)| op != "places.list").map(|(op, p)| (op, serde_json::to_string(&p).unwrap())).collect();
    assert_eq!(asked, vec![("places.add".to_string(), r#"{"address":"root@10.0.0.9","recipe":"laptop"}"#.to_string())]);
    assert_eq!(*host.events_called.lock().unwrap(), 1, "events are subscribed before the add");
    assert_eq!(host.listeners(), 0, "the tool takes its frame listener off");
    let first = host.sent().into_iter().position(|(op, _)| op == "places.add").unwrap();
    assert_eq!(first, 0);
}

/// places-add.test.ts:264.
#[tokio::test]
async fn the_add_tool_answers_at_the_end_once_nothing_waits_and_goes_past_a_wait_it_was_told_to_leave() {
    let ready = add_computer(Arc::new(Client(fake_host(Plays::default()))), &AddAsk { recipe: Some("laptop".into()), ..ask("spoo") }, None)
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(&ready.computer.setup.as_ref().unwrap().state).unwrap(), json!("done"));
    assert_eq!(ready.waiting.len(), 0);
    let later = add_computer(
        Arc::new(Client(fake_host(Plays { sign_in: true, ..Plays::default() }))),
        &AddAsk { recipe: Some("laptop".into()), later: Some(true), ..ask("spoo") },
        None,
    )
    .await
    .unwrap();
    assert_eq!(serde_json::to_value(&later.computer.setup.as_ref().unwrap().state).unwrap(), json!("done"));
    assert_eq!(serde_json::to_value(&later.waiting).unwrap(), json!([wait_value()]));
    let failed = add_computer(Arc::new(Client(fake_host(Plays { failed: true, ..Plays::default() }))), &ask("spoo"), None).await.unwrap();
    assert_eq!(serde_json::to_value(&failed.computer.setup.as_ref().unwrap().state).unwrap(), json!("failed"));
}

/// places-add.test.ts:272: a project's source and a provider are refused, the host asked nothing.
#[tokio::test]
async fn the_add_tool_refuses_a_projects_source_and_a_provider_asking_the_host_nothing() {
    let host = fake_host(Plays::default());
    let client: Arc<dyn HostClient> = Arc::new(Client(host.clone()));
    let e = add_computer(client.clone(), &ask("/Users/dev/code/app"), None).await.unwrap_err();
    assert_eq!(e.message, format!("{} {ADD_TOOL_FIX}", add_tool_refusal("/Users/dev/code/app")));
    assert_eq!((e.kind, e.fix.as_deref()), (Some(Kind::Usage), Some(ADD_TOOL_FIX)));
    let e = add_computer(client.clone(), &ask("https://github.com/dev/app.git"), None).await.unwrap_err();
    assert_eq!(e.kind, Some(Kind::Usage));
    let e = add_computer(client, &ask("box"), None).await.unwrap_err();
    assert!(e.message.contains("box is not one"));
    assert_eq!(
        add_tool_refusal("box"),
        "add takes a computer over ssh, by user@host or by an alias from the ssh config, and box is not one"
    );
    assert_eq!(host.sent(), vec![]);
}

/// setup-follow.ts:97: resume skips the word check and asks places.setup with ref and recipe.
#[tokio::test]
async fn the_add_tool_resumes_with_places_setup_by_ref_and_takes_the_runs_next_wait() {
    let host = fake_host(Plays { sign_in: true, running: true, ..Plays::default() });
    let answer =
        add_computer(Arc::new(Client(host.clone())), &AddAsk { resume: Some(true), recipe: Some("laptop".into()), ..ask("box") }, None)
            .await
            .unwrap();
    assert_eq!(serde_json::to_value(&answer.waiting).unwrap(), json!([wait_value()]));
    let (op, params) = host.sent()[0].clone();
    assert_eq!((op.as_str(), serde_json::to_string(&params).unwrap().as_str()), ("places.setup", r#"{"ref":"box","recipe":"laptop"}"#));
}

/// setup-follow.ts:108-115: the add's params in the TS order, absent ones left out.
#[tokio::test]
async fn the_add_tool_sends_the_adds_params_in_order() {
    let host = fake_host(Plays::default());
    let a = AddAsk {
        address: "dev@10.0.0.9".into(),
        recipe: Some("laptop".into()),
        later: None,
        resume: None,
        name: Some("spoo".into()),
        ssh_port: Some(2222),
        key_path: Some("/k".into()),
        host_key: Some("SHA256:x".into()),
    };
    add_computer(Arc::new(Client(host.clone())), &a, None).await.unwrap();
    let (op, params) = host.sent()[0].clone();
    assert_eq!(op, "places.add");
    assert_eq!(
        serde_json::to_string(&params).unwrap(),
        r#"{"address":"dev@10.0.0.9","name":"spoo","sshPort":2222,"keyPath":"/k","hostKey":"SHA256:x","recipe":"laptop"}"#
    );
}

/// places-add.test.ts:286: at the ceiling it answers how far the add got.
#[tokio::test]
async fn the_add_tool_answers_at_its_ceiling_with_how_far_the_add_got() {
    let host = fake_host(Plays { sign_in: true, ..Plays::default() });
    let answer = tokio::time::timeout(
        Duration::from_secs(2),
        add_computer(Arc::new(Client(host)), &AddAsk { recipe: Some("laptop".into()), later: Some(true), ..ask("root@10.0.0.9") }, Some(0)),
    )
    .await
    .expect("a ceiling of 0 answers at once")
    .unwrap();
    assert_eq!(serde_json::to_value(&answer.computer.setup.as_ref().unwrap().state).unwrap(), json!("running"));
    assert_eq!(answer.setup.len(), 0);
    assert_eq!(ADD_TOOL_MS, 600_000);
}

/// setup-follow.ts:119: a row gone from the listing reads as the place the add answered.
#[tokio::test]
async fn a_row_missing_from_the_listing_reads_as_the_place_the_add_answered() {
    let c = FakeClient::new(|op, _| match op {
        "places.add" => Ok(frame(json!({"ok": true, "addId": "a_1", "place": place()}))),
        "places.list" => Ok(frame(json!({"ok": true, "places": []}))),
        _ => Ok(frame(json!({"ok": true}))),
    });
    let answer = add_computer(Arc::new(Client(c)), &ask("spoo"), None).await.unwrap();
    assert_eq!(answer.computer.id, "p_1");
    assert!(answer.computer.setup.is_none());
    assert_eq!((answer.setup.len(), answer.waiting.len()), (0, 0));
    assert_eq!(serde_json::to_value(&answer).unwrap()["setup"], json!([]));
}
