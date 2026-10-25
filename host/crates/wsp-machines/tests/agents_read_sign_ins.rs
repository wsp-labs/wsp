// SPDX-License-Identifier: AGPL-3.0-only
//! spec/agents_read_sign_ins.md: one watched sign-in per agent or server on a target, ported from the sign-ins group of
//! packages/runtime/test/agents-read.test.ts (12 cases; the other 13 are in agents_read_targets.rs).
mod agents_read_support;
mod rest_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use serde_json::{json, Value};

use agents_read_support::*;
use wsp_machines::agents_read::{AgentsReads, Running, SignInAsk};
use wsp_wire::types::agents_report::{AgentsSignInEvent, McpScope, NO_SIGN_IN_REFUSAL, SIGN_IN_ENDED_KEPT_MS};
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_words::agents_report::{installing_first_line, napping_sign_in_refusal, tool_not_installed_fix, tool_not_installed_line};
use wsp_words::exit::{refusal, HostError};
use wsp_words::words::base::last_line;

/// The catalog's name of `gh` (`catalogEntry("gh").name` at 65f6b89c7), which wsp-catalog's stub cannot answer yet.
const GH: &str = "GitHub CLI";
const URL: &str = "https://auth.openai.com/codex/device";
const KEPT: i64 = SIGN_IN_ENDED_KEPT_MS as i64;

fn ok<T>(r: Result<T, HostError>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("refused: {}", e.message),
    }
}

fn machine_on_json() -> Value {
    json!({ "kind": "machine", "machine": "m_landing", "projects": [js(&landing())] })
}

fn event(v: Value) -> AgentsSignInEvent {
    serde_json::from_value(v).expect("an event")
}

fn gh_row(outcome: &str, note: Option<&str>) -> wsp_wire::views::place::PlaceProvisionRow {
    let mut v = json!({ "id": "gh", "label": GH, "outcome": outcome });
    if let Some(n) = note {
        v["note"] = json!(n);
    }
    row(v)
}

fn id_shape(id: &str) -> bool {
    id.strip_prefix("si_").is_some_and(|hex| hex.len() == 12 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')))
}

fn bare_run(t: wsp_wire::types::agents_report::AgentsTarget, a: SignInAsk, last: Option<AgentsSignInEvent>) -> Rc<Running> {
    Rc::new(Running { target: t, ask: a, r#type: RefCell::new(None), followers: RefCell::new(vec![]), last: RefCell::new(last), stopped_by: Cell::new(false), stop: Box::new(|| {}) })
}

// N1: agents-read.test.ts:328
#[test]
fn a_second_start_joins_the_running_sign_in() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let (a, b) = (Seen::new(), Seen::new());
        let (first, second) = futures::future::join(api.sign_in(ws("ws_1"), ask("codex"), a.f.clone(), None), api.sign_in(ws("ws_1"), ask("codex"), b.f.clone(), None)).await;
        let (first, second) = (ok(first), ok(second));
        assert_eq!(first.sign_in_id, sign_in_id(1));
        assert_eq!(second.sign_in_id, first.sign_in_id);
        assert_eq!(rig.planned.borrow().len(), 1);
        tick().await;
        rig.push_event(json!({ "url": URL }));
        assert_eq!(a.all(), vec![waiting(&first.sign_in_id, URL)]);
        assert_eq!(b.all(), vec![waiting(&first.sign_in_id, URL)]);

        // The caller putting the tool there itself is no other sign-in; another server, terminal or login is one of its own.
        let tool_there = ok(api.sign_in(ws("ws_1"), SignInAsk { tool_there: Some(true), ..ask("codex") }, no_follower(), None).await);
        assert_eq!(tool_there.sign_in_id, first.sign_in_id);
        let other = ok(api.sign_in(ws("ws_1"), ask_server("claude", "notion"), no_follower(), None).await);
        let terminal = ok(api.sign_in(ws("ws_1"), SignInAsk { terminal: Some(true), ..ask("codex") }, no_follower(), None).await);
        let login = ok(api.sign_in(ws("ws_1"), SignInAsk { login: Some(true), ..ask("codex") }, no_follower(), None).await);
        let elsewhere = ok(api.sign_in(pl("here"), ask("codex"), no_follower(), None).await);
        let ids = [&first, &other, &terminal, &login, &elsewhere].map(|f| f.sign_in_id.clone());
        assert_eq!(ids.to_vec(), vec![sign_in_id(1), sign_in_id(2), sign_in_id(3), sign_in_id(4), sign_in_id(5)]);
        assert_eq!(rig.planned.borrow().len(), 5);
        assert_eq!(api.running.borrow().len(), 5);

        let t = ws("ws_1");
        assert_eq!(AgentsReads::key_of(&t, &ask("codex")), "[{\"workspaceId\":\"ws_1\"},\"codex\",null,false,false]");
        let full = SignInAsk { agent: "claude".into(), server: Some("notion".into()), terminal: Some(true), tool_there: Some(true), login: Some(true), scope: Some(McpScope::Project), project: Some("pr_1".into()) };
        assert_eq!(AgentsReads::key_of(&pl("pl_1"), &full), "[{\"placeId\":\"pl_1\"},\"claude\",\"notion\",true,true]");
        assert_eq!(AgentsReads::key_of(&t, &SignInAsk { login: Some(false), terminal: Some(false), ..ask("codex") }), AgentsReads::key_of(&t, &ask("codex")));
    });
}

// N1: agents-read.ts:339, 570-576
#[test]
fn a_failed_start_is_not_joined_by_the_next() {
    run(async {
        let rig = Rig::new();
        rig.refuse_agents.borrow_mut().push("codex".to_string());
        let api = rig.reads();
        let (r1, r2) = futures::future::join(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None), api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None)).await;
        assert_eq!(r1.err(), Some(plain_error("codex asks you to pick")));
        assert_eq!(r2.err(), Some(plain_error("codex asks you to pick")));
        assert_eq!(rig.planned.borrow().len(), 1);
        assert_eq!(api.starting.borrow().len(), 0);

        rig.refuse_agents.borrow_mut().clear();
        let fresh = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        assert_eq!(fresh.sign_in_id, sign_in_id(1));
        assert_eq!(rig.planned.borrow().len(), 2);
        assert_eq!(rig.lines_of(&["random_bytes"]), vec!["random_bytes 6".to_string()]);
    });
}

// N2: agents-read.test.ts:236, 311
#[test]
fn a_refused_plan_opens_no_channel_and_logs_the_refusal() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let refused = api.sign_in(ws("ws_1"), ask("opencode"), no_follower(), None).await;
        assert_eq!(refused.err(), Some(plain_error("opencode asks you to pick")));
        assert_eq!(rig.frames.borrow().len(), 0);
        assert_eq!(rig.opened.borrow().len(), 0);
        assert_eq!(rig.shut.get(), 0);
        assert_eq!(rig.host_log.borrow().clone(), vec!["sign-in refused: opencode, on workspace ws_1: opencode asks you to pick".to_string()]);
        assert_eq!(api.sign_ins().len(), 0);
        assert_eq!(rig.lines_of(&["random_bytes", "channel."]), Vec::<String>::new());

        let rig = Rig::new();
        let mut o = rig.options();
        o.log = None;
        let api = AgentsReads::new(o);
        assert!(api.sign_in(ws("ws_1"), ask("opencode"), no_follower(), None).await.is_err());
        assert_eq!(rig.warned.lines(), vec!["sign-in refused: opencode, on workspace ws_1: opencode asks you to pick".to_string()]);
        assert_eq!(rig.host_log.borrow().len(), 0);
    });
}

// N2: agents-read.test.ts:311
#[test]
fn a_napping_workspace_is_refused_before_any_plan() {
    run(async {
        let rig = Rig::new();
        rig.phase.set(WorkspacePhase::Napping);
        let api = rig.reads();
        let refused = api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await;
        assert_eq!(refused.err(), Some(usage(&napping_sign_in_refusal("landing", None))));
        rig.pause_mode.set(Some(PauseMode::Memory));
        let refused = api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await;
        assert_eq!(refused.err(), Some(usage(&napping_sign_in_refusal("landing", Some(&PauseMode::Memory)))));
        assert_eq!(rig.planned.borrow().len(), 0);
        assert_eq!(rig.opened.borrow().len(), 0);
        assert_eq!(rig.host_log.borrow().len(), 0);
        assert_eq!(api.starting.borrow().len(), 0);
    });
}

// N2: agents-read.test.ts:236, 257
#[test]
fn a_start_logs_its_id_and_names_without_control_characters() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let forged = ask_server("opencode", "x\nsign-in si_forged ended: \u{1b}[2Kok\r");
        assert!(api.sign_in(ws("ws_1"), forged, no_follower(), None).await.is_err());
        assert_eq!(
            rig.host_log.borrow().clone(),
            vec!["sign-in refused: opencode, server xsign-in si_forged ended: [2Kok, on workspace ws_1: opencode asks you to pick".to_string()]
        );

        let rig = Rig::new();
        let api = rig.reads();
        let first = ok(api.sign_in(pl("here"), ask_server("co\u{1b}dex", "no\ttion"), no_follower(), None).await);
        assert!(id_shape(&first.sign_in_id), "{}", first.sign_in_id);
        assert_eq!(first.sign_in_id, sign_in_id(1));
        assert_eq!(rig.lines_of(&["random_bytes"]), vec!["random_bytes 6".to_string()]);
        let second = ok(api.sign_in(ws("ws_1"), ask("co\u{7}dex"), no_follower(), None).await);
        assert_eq!(
            rig.host_log.borrow().clone(),
            vec![
                format!("sign-in {} started: codex, server notion, on computer here", first.sign_in_id),
                format!("sign-in {} started: codex, on workspace ws_1", second.sign_in_id),
            ]
        );
    });
}

// N3: agents-read.test.ts:328
#[test]
fn a_follower_joining_late_is_handed_the_last_step() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let run = bare_run(ws("ws_1"), ask("codex"), None);
        let a = Seen::new();
        let following = api.follow("si_x", &run, a.f.clone());
        assert_eq!(following.sign_in_id, "si_x");
        assert_eq!(a.all(), Vec::<Value>::new());
        assert_eq!(run.followers.borrow().len(), 1);

        *run.last.borrow_mut() = Some(event(json!({ "signInId": "si_x", "state": "waiting", "url": URL })));
        let b = Seen::new();
        api.follow("si_x", &run, b.f.clone());
        assert_eq!(b.all(), vec![waiting("si_x", URL)]);
        api.follow("si_x", &run, b.f.clone());
        assert_eq!(b.all(), vec![waiting("si_x", URL)]);
        assert_eq!(run.followers.borrow().len(), 2);
        assert_eq!(a.all(), Vec::<Value>::new());
    });
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let a = Seen::new();
        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), a.f.clone(), None).await);
        tick().await;
        rig.push_event(json!({ "url": URL }));
        let c = Seen::new();
        let third = ok(api.sign_in(ws("ws_1"), ask("codex"), c.f.clone(), None).await);
        assert_eq!(third.sign_in_id, first.sign_in_id);
        assert_eq!(c.all(), vec![waiting(&first.sign_in_id, URL)]);
        assert_eq!(a.all().len(), 1);
    });
}

// N3: agents-read.test.ts:311, 328
#[test]
fn leaving_drops_one_follower_and_the_run_goes_on() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let (a, b) = (Seen::new(), Seen::new());
        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), a.f.clone(), None).await);
        let second = ok(api.sign_in(ws("ws_1"), ask("codex"), b.f.clone(), None).await);
        tick().await;
        (first.leave)();
        rig.push_event(json!({ "url": URL }));
        assert_eq!(a.all(), Vec::<Value>::new());
        assert_eq!(b.all(), vec![waiting(&second.sign_in_id, URL)]);
        assert_eq!(rig.shut.get(), 0);

        (second.leave)();
        tick().await;
        assert_eq!(rig.shut.get(), 0);
        assert_eq!(api.running.borrow().len(), 1);

        let late = Seen::new();
        let again = ok(api.sign_in(ws("ws_1"), ask("codex"), late.f.clone(), None).await);
        assert_eq!(again.sign_in_id, first.sign_in_id);
        assert_eq!(rig.planned.borrow().len(), 1);
        assert_eq!(late.all(), vec![waiting(&first.sign_in_id, URL)]);
        rig.finish_plan();
        tick().await;
        assert_eq!(rig.shut.get(), 1);
        assert_eq!(late.all().last(), Some(&signed_in(&first.sign_in_id)));
        assert_eq!(a.all(), Vec::<Value>::new());
    });
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let run = bare_run(ws("ws_1"), ask("codex"), None);
        let (a, b) = (Seen::new(), Seen::new());
        let left = api.follow("si_x", &run, a.f.clone());
        api.follow("si_x", &run, b.f.clone());
        (left.leave)();
        assert_eq!(run.followers.borrow().len(), 1);
        assert!(Rc::ptr_eq(&run.followers.borrow()[0], &b.f));
    });
}

// N4: agents-read.test.ts:263
#[test]
fn the_plan_runs_over_the_targets_channel_with_op_last() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        *rig.plan.borrow_mut() = Plan::Link;
        let api = rig.reads();
        let origin = Caller::Road(WorkspaceOrigin::Paired);
        let first = ok(api.sign_in(pl("pl_1"), ask("codex"), no_follower(), Some(origin.clone())).await);
        tick().await;
        assert_eq!(*rig.planned.borrow(), vec![(json!({ "kind": "box", "name": "spoo", "login": {} }), ask("codex"))]);
        assert_eq!(*rig.opened.borrow(), vec![(pl("pl_1"), Some(origin))]);
        let frames: Vec<String> = rig.frames.borrow().iter().map(|f| serde_json::to_string(f).unwrap()).collect();
        assert_eq!(frames, vec!["{\"cols\":80,\"rows\":24,\"op\":\"pty.create\"}".to_string()]);
        assert_eq!(*rig.op_replies.borrow(), vec![json!({ "ok": true, "ptyId": "pty_1" })]);

        rig.push_event(json!({ "x": 1 }));
        assert_eq!(*rig.readers_a.borrow(), vec![json!({ "x": 1 })]);
        assert_eq!(*rig.readers_b.borrow(), Vec::<Value>::new());
        assert_eq!(*rig.link_seen.borrow(), vec!["bare=Some(true)".to_string(), "closed=true".to_string()]);

        rig.end_channel_from_daemon();
        tick().await;
        assert_eq!(rig.link_seen.borrow().last(), Some(&"link closed".to_string()));
        assert_eq!(rig.host_log.borrow().last(), Some(&format!("sign-in {} ended: codex, on computer pl_1: failed", first.sign_in_id)));

        let rig = Rig::new();
        *rig.plan.borrow_mut() = Plan::Link;
        ok(rig.reads().sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        tick().await;
        assert_eq!(*rig.link_seen.borrow(), vec!["bare=Some(false)".to_string(), "closed=true".to_string()]);
    });
}

// N4: agents-read.test.ts:263
#[test]
fn steps_go_to_every_follower_under_the_sign_in_id() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let (a, b) = (Seen::new(), Seen::new());
        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), a.f.clone(), None).await);
        let id = first.sign_in_id.clone();
        assert!(id_shape(&id));
        assert_eq!(*rig.planned.borrow(), vec![(machine_on_json(), ask("codex"))]);
        ok(api.sign_in(ws("ws_1"), ask("codex"), b.f.clone(), None).await);
        tick().await;
        let frames: Vec<Value> = rig.frames.borrow().iter().map(|f| Value::Object(f.clone())).collect();
        assert_eq!(frames, vec![json!({ "op": "pty.create" })]);
        rig.push_event(json!({ "url": URL }));
        assert_eq!(js(&api.sign_ins()[0])["last"], json!({ "type": "agents.signIn", "signInId": id, "state": "waiting", "url": URL }));
        rig.finish_plan();
        tick().await;
        assert_eq!(a.all(), vec![waiting(&id, URL), signed_in(&id)]);
        assert_eq!(b.all(), vec![waiting(&id, URL), signed_in(&id)]);
        assert_eq!(rig.shut.get(), 1);
        assert_eq!(rig.changed_json(), vec![json!({ "workspaceId": "ws_1" })]);
    });
}

// N4: agents-read.test.ts:179
#[test]
fn a_server_sign_in_gets_a_forward_of_its_own_closed_at_the_end() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let api = rig.reads();
        let t = pl("pl_1");
        let first = ok(api.sign_in(t.clone(), ask_server("codex", "notion"), no_follower(), None).await);
        tick().await;
        assert_eq!(rig.planned.borrow()[0].0.get("relayed"), None);
        assert_eq!(rig.handed.borrow().len(), 1);
        assert!(rig.handed.borrow()[0].is_none());
        assert_eq!(js(&api.read(&t, None).await.unwrap())["reach"], json!("none"));
        api.sign_in_stop(&first.sign_in_id).unwrap();
        tick().await;

        let relay = FakeRelay::new(&rig, "pl_1");
        let unregister = api.forwards(relay.clone());
        assert_eq!(js(&api.read(&t, None).await.unwrap())["reach"], json!("relay"));
        assert_eq!(relay.opened.borrow().len(), 0);
        let second = ok(api.sign_in(t.clone(), ask_server("codex", "notion"), no_follower(), None).await);
        tick().await;
        let planned = rig.planned.borrow()[1].0.clone();
        assert_eq!((planned["kind"].clone(), planned["name"].clone(), planned["relayed"].clone()), (json!("box"), json!("spoo"), json!(true)));
        assert_eq!(relay.opened.borrow().len(), 1);
        assert!(same_forward(&rig.handed.borrow()[1], &relay.opened.borrow()[0]));
        assert!(relay.asked.borrow().contains(&json!({ "placeId": "pl_1" })));
        assert_eq!(*relay.closed.borrow(), Vec::<usize>::new());
        api.sign_in_stop(&second.sign_in_id).unwrap();
        tick().await;
        assert_eq!(*relay.closed.borrow(), vec![1]);
        let tail: Vec<String> = rig.lines_of(&["forward.close", "channel.close", "log: sign-in"]);
        let from = tail.iter().position(|l| l.starts_with(&format!("log: sign-in {} started", second.sign_in_id))).unwrap();
        assert_eq!(
            tail[from + 1..].to_vec(),
            vec!["forward.close 1".to_string(), "channel.close".to_string(), format!("log: sign-in {} ended: codex, server notion, on computer pl_1: stopped", second.sign_in_id)]
        );

        // A tool's own login types its code on the terminal, so it is handed no forward to arm or to carry a pasted address.
        let login = ok(api.sign_in(t.clone(), ask("codex"), no_follower(), None).await);
        tick().await;
        assert!(rig.handed.borrow()[2].is_none());
        assert_eq!(relay.opened.borrow().len(), 1);
        api.sign_in_stop(&login.sign_in_id).unwrap();
        tick().await;
        unregister();
        ok(api.sign_in(t.clone(), ask_server("codex", "notion"), no_follower(), None).await);
        tick().await;
        assert_eq!(rig.planned.borrow()[3].0.get("relayed"), None);
        assert!(rig.handed.borrow()[3].is_none());
    });
}

// N4: agents-read.test.ts:170
#[test]
fn a_workspaces_callback_port_forwarded_from_here_reaches_the_plan_as_relayed() {
    run(async {
        for relayed in [Some(true), Some(false), None] {
            let rig = Rig::new();
            rig.relayed.set(relayed);
            let api = rig.reads();
            let first = ok(api.sign_in(ws("ws_1"), ask_server("claude", "notion"), no_follower(), None).await);
            let mut want = machine_on_json();
            if relayed == Some(true) {
                want["relayed"] = json!(true);
            }
            assert_eq!(rig.planned.borrow()[0].0, want, "relayed {relayed:?}");
            (first.leave)();
        }
    });
}

// N5: agents-read.ts:429-439 (no TS case)
#[test]
fn gh_goes_on_first_and_says_installing_once() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let installed = gh_row("installed", None);
        *rig.gh_steps.borrow_mut() = vec![
            ("resolving".into(), None, None),
            ("fetching".into(), Some("2026-09-24T12:00:01.000Z".into()), None),
            ("unpacking".into(), Some("2026-09-24T12:00:02.000Z".into()), None),
            ("done".into(), Some("2026-09-24T12:00:03.000Z".into()), Some(installed.clone())),
        ];
        *rig.gh_rows.borrow_mut() = vec![installed];
        let api = rig.reads();
        let a = Seen::new();
        let first = ok(api.sign_in(pl("pl_1"), ask("gh"), a.f.clone(), None).await);
        tick().await;
        let id = first.sign_in_id.clone();
        assert_eq!(a.all(), vec![json!({ "signInId": id, "state": "running", "said": installing_first_line(GH) })]);
        rig.push_event(json!({ "url": URL }));
        assert_eq!(a.all().len(), 2);
        assert_eq!(a.all()[1], waiting(&id, URL));
        assert_eq!(rig.lines_of(&["door.gh_there", "plan.start"]), vec!["door.gh_there pl_1".to_string(), "plan.start gh".to_string()]);

        let nothing_first: Vec<(wsp_wire::types::agents_report::AgentsTarget, SignInAsk)> = vec![
            (pl("pl_1"), SignInAsk { tool_there: Some(true), ..ask("gh") }),
            (pl("pl_1"), ask_server("gh", "notion")),
            (pl("pl_1"), ask("codex")),
            (ws("ws_1"), ask("gh")),
        ];
        for (t, a) in nothing_first {
            let rig = Rig::new();
            rig.with_spoo();
            ok(rig.reads().sign_in(t, a, no_follower(), None).await);
            tick().await;
            assert_eq!(rig.lines_of(&["door.gh_there"]), Vec::<String>::new());
        }
    });
}

// N5: agents-read.ts:437-438, 539 (no TS case)
#[test]
fn a_failed_gh_install_ends_the_sign_in_failed() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let note = "fetching gh\nE: could not fetch gh\n";
        *rig.gh_rows.borrow_mut() = vec![gh_row("installed", None), gh_row("failed", Some(note))];
        let api = rig.reads();
        let a = Seen::new();
        let first = ok(api.sign_in(pl("pl_1"), ask("gh"), a.f.clone(), None).await);
        let id = first.sign_in_id.clone();
        until(|| api.sign_ins().iter().any(|r| js(r).get("ended").is_some())).await;
        assert_eq!(a.all(), vec![json!({ "signInId": id, "state": "failed", "said": last_line(note).unwrap() })]);
        assert_eq!(a.all()[0]["said"], json!("E: could not fetch gh"));
        assert_eq!(rig.lines_of(&["plan.start"]), Vec::<String>::new());
        assert_eq!(rig.handed.borrow().len(), 0);
        assert_eq!(rig.shut.get(), 1);
        assert_eq!(rig.host_log.borrow().last(), Some(&format!("sign-in {id} ended: gh, on computer pl_1: failed")));
        assert_eq!(
            js(&api.sign_ins()),
            json!([{ "signInId": id, "target": { "placeId": "pl_1" }, "agent": "gh", "last": { "type": "agents.signIn", "signInId": id, "state": "failed", "said": "E: could not fetch gh" }, "ended": true }])
        );

        *rig.gh_rows.borrow_mut() = vec![gh_row("failed", None)];
        let b = Seen::new();
        let second = ok(api.sign_in(pl("pl_1"), ask("gh"), b.f.clone(), None).await);
        assert_ne!(second.sign_in_id, id);
        until(|| rig.shut.get() == 2).await;
        assert_eq!(b.all(), vec![json!({ "signInId": second.sign_in_id, "state": "failed", "said": format!("{GH} did not install") })]);
        assert_eq!(rig.lines_of(&["plan.start"]), Vec::<String>::new());
        assert_eq!(rig.lines_of(&["door.key_landed", "door.login_landed"]), Vec::<String>::new());
    });
}

// N6: agents-read.ts:543 (no TS case)
#[test]
fn a_plan_that_throws_ends_failed_with_its_message() {
    run(async {
        let rig = Rig::new();
        *rig.plan.borrow_mut() = Plan::Throws("the pty would not open".to_string());
        let api = rig.reads();
        let a = Seen::new();
        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), a.f.clone(), None).await);
        let id = first.sign_in_id.clone();
        until(|| rig.shut.get() == 1).await;
        assert_eq!(a.all(), vec![json!({ "signInId": id, "state": "failed", "said": "the pty would not open" })]);
        assert_eq!(rig.host_log.borrow().last(), Some(&format!("sign-in {id} ended: codex, on workspace ws_1: failed")));
        assert_eq!(rig.changed_json(), vec![json!({ "workspaceId": "ws_1" })]);
        assert_eq!(api.running.borrow().len(), 0);
    });
}

// N7: agents-read.test.ts:286
#[test]
fn the_end_notes_a_landed_login_before_saying_changed() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        rig.land_waits.set(true);
        let api = rig.reads();
        finished(&rig, &api, pl("pl_1"), ask("codex")).await;
        let id = sign_in_id(1);
        assert_eq!(*rig.landed.borrow(), vec!["pl_1 codex after 0 changes".to_string()]);
        assert_eq!(rig.changed_json(), vec![json!({ "placeId": "pl_1" })]);
        assert_eq!(
            rig.lines_of(&["log: sign-in", "door.login_landed", "reader.forget", "changed"]),
            vec![
                format!("log: sign-in {id} started: codex, on computer pl_1"),
                format!("log: sign-in {id} ended: codex, on computer pl_1: signed-in"),
                "door.login_landed pl_1 codex".to_string(),
                "reader.forget {\"placeId\":\"pl_1\"}".to_string(),
                "changed {\"placeId\":\"pl_1\"}".to_string(),
            ]
        );
    });
}

// N7: agents-read.ts:554-559 (no TS case)
#[test]
fn a_minting_agent_here_lands_as_a_key() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let api = rig.reads();
        finished(&rig, &api, pl("here"), ask("claude")).await;
        finished(&rig, &api, pl("here"), ask("codex")).await;
        finished(&rig, &api, pl("pl_1"), ask("claude")).await;
        assert_eq!(rig.lines_of(&["door.key_landed", "door.login_landed"]), vec!["door.key_landed claude", "door.login_landed here codex", "door.login_landed pl_1 claude"]);

        let rig = Rig::new();
        let api = rig.reads();
        finished(&rig, &api, pl("here"), ask("claude")).await;
        assert_eq!(rig.lines_of(&["door."]), Vec::<String>::new());
        assert!(rig.host_log.borrow().iter().all(|l| !l.contains("was not noted")));
    });
}

// N7: agents-read.test.ts:286
#[test]
fn a_server_or_workspace_sign_in_notes_no_login() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let api = rig.reads();
        finished(&rig, &api, pl("pl_1"), ask_server("claude", "linear")).await;
        finished(&rig, &api, ws("ws_1"), ask("codex")).await;
        assert_eq!(rig.lines_of(&["door.key_landed", "door.login_landed"]), Vec::<String>::new());
        assert_eq!(rig.landed.borrow().len(), 0);
        assert_eq!(rig.changed_json(), vec![json!({ "placeId": "pl_1" }), json!({ "workspaceId": "ws_1" })]);
    });
}

// N7: agents-read.test.ts:236
#[test]
fn the_end_forgets_the_readers_copy_then_says_changed() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let first = ok(api.sign_in(ws("ws_1"), ask_server("claude", "notion"), no_follower(), None).await);
        let id = first.sign_in_id.clone();
        tick().await;
        rig.push_event(json!({ "url": "https://mcp.notion.com/authorize?code=SECRETCODE&state=x" }));
        ok(api.sign_in_code(&id, "SECRETCODE").await);
        rig.finish_plan();
        tick().await;
        assert_eq!(*rig.forgot.borrow(), vec!["{\"workspaceId\":\"ws_1\"}".to_string()]);
        assert_eq!(rig.changed_json(), vec![json!({ "workspaceId": "ws_1" })]);
        assert_eq!(
            rig.host_log.borrow().clone(),
            vec![format!("sign-in {id} started: claude, server notion, on workspace ws_1"), format!("sign-in {id} ended: claude, server notion, on workspace ws_1: signed-in")]
        );
        assert_eq!(
            rig.lines_of(&["log: sign-in", "reader.forget", "changed"]),
            vec![
                format!("log: sign-in {id} started: claude, server notion, on workspace ws_1"),
                format!("log: sign-in {id} ended: claude, server notion, on workspace ws_1: signed-in"),
                "reader.forget {\"workspaceId\":\"ws_1\"}".to_string(),
                "changed {\"workspaceId\":\"ws_1\"}".to_string(),
            ]
        );

        let stopped = ok(api.sign_in(pl("here"), ask("codex"), no_follower(), None).await);
        api.sign_in_stop(&stopped.sign_in_id).unwrap();
        tick().await;
        assert_eq!(rig.host_log.borrow().last(), Some(&format!("sign-in {} ended: codex, on computer here: stopped", stopped.sign_in_id)));
        assert!(!serde_json::to_string(&*rig.host_log.borrow()).unwrap().contains("SECRETCODE"));
    });
}

// N7: agents-read.ts:560-562 (no TS case)
#[test]
fn a_failed_note_is_logged() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        rig.fail("door.landed", "the vault is locked");
        let api = rig.reads();
        finished(&rig, &api, pl("pl_1"), ask("codex")).await;
        let id = sign_in_id(1);
        assert_eq!(
            rig.lines_of(&["log: sign-in", "door.", "reader.forget", "changed"]),
            vec![
                format!("log: sign-in {id} started: codex, on computer pl_1"),
                format!("log: sign-in {id} ended: codex, on computer pl_1: signed-in"),
                "door.login_landed pl_1 codex".to_string(),
                format!("log: sign-in {id}: the landed login was not noted: the vault is locked"),
                "reader.forget {\"placeId\":\"pl_1\"}".to_string(),
                "changed {\"placeId\":\"pl_1\"}".to_string(),
            ]
        );
    });
}

// N8: agents-read.test.ts:387
#[test]
fn stop_ends_the_run_for_everyone_and_the_next_start_runs_fresh() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let seen = Seen::new();
        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), seen.f.clone(), None).await);
        let joined = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        assert_eq!(joined.sign_in_id, first.sign_in_id);
        api.sign_in_stop(&first.sign_in_id).unwrap();
        let fresh = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        assert_ne!(fresh.sign_in_id, first.sign_in_id);
        assert_eq!(rig.planned.borrow().len(), 2);
        tick().await;
        assert_eq!(rig.shut.get(), 1);
        assert_eq!(seen.all().last(), Some(&signed_in(&first.sign_in_id)));
        assert_eq!(api.sign_in_stop(&first.sign_in_id).err(), Some(usage(NO_SIGN_IN_REFUSAL)));
        assert_eq!(rig.host_log.borrow().last(), Some(&format!("sign-in {} ended: codex, on workspace ws_1: stopped", first.sign_in_id)));
    });
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let api = rig.reads();
        let first = ok(api.sign_in(pl("pl_1"), ask("codex"), no_follower(), None).await);
        tick().await;
        api.sign_in_stop(&first.sign_in_id).unwrap();
        tick().await;
        assert_eq!(rig.lines_of(&["door.key_landed", "door.login_landed"]), Vec::<String>::new());
        assert_eq!(rig.lines_of(&["reader.forget", "changed"]), vec!["reader.forget {\"placeId\":\"pl_1\"}".to_string(), "changed {\"placeId\":\"pl_1\"}".to_string()]);
    });
}

fn run_json(id: &str, last: Option<Value>, ended: bool) -> Value {
    let mut v = json!({ "signInId": id, "target": { "workspaceId": "ws_1" }, "agent": "claude", "server": "notion", "scope": "project", "project": "pr_1" });
    if let Some(l) = last {
        v["last"] = l;
    }
    if ended {
        v["ended"] = json!(true);
    }
    v
}

// N9: agents-read.test.ts:364
#[test]
fn sign_ins_list_running_then_ended_and_drop_ended_past_ten_minutes() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let a = SignInAsk { scope: Some(McpScope::Project), project: Some("pr_1".into()), ..ask_server("claude", "notion") };
        let first = ok(api.sign_in(ws("ws_1"), a, no_follower(), None).await);
        let id = first.sign_in_id.clone();
        tick().await;
        rig.push_event(json!({ "url": "https://mcp.notion.com/authorize" }));
        let wait = json!({ "type": "agents.signIn", "signInId": id, "state": "waiting", "url": "https://mcp.notion.com/authorize" });
        let heard = Seen::new();
        let followed = ok(api.sign_in_follow(&id, heard.f.clone()));
        assert_eq!(heard.all(), vec![waiting(&id, "https://mcp.notion.com/authorize")]);
        assert_eq!(js(&api.sign_ins()), json!([run_json(&id, Some(wait), false)]));
        rig.finish_plan();
        tick().await;
        assert_eq!(heard.all().last(), Some(&signed_in(&id)));
        let done = json!({ "type": "agents.signIn", "signInId": id, "state": "signed-in" });
        assert_eq!(js(&api.sign_ins()), json!([run_json(&id, Some(done), true)]));
        assert_eq!(api.sign_in_follow(&id, no_follower()).err(), Some(usage(NO_SIGN_IN_REFUSAL)));
        rig.clock.advance(KEPT);
        assert_eq!(api.sign_ins().len(), 1);
        rig.clock.advance(1);
        assert_eq!(js(&api.sign_ins()), json!([]));
        (followed.leave)();
    });
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        finished(&rig, &api, ws("ws_1"), ask("gemini")).await;
        let done_id = sign_in_id(1);
        let a = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        let b = ok(api.sign_in(ws("ws_1"), ask_server("claude", "notion"), no_follower(), None).await);
        tick().await;
        let listed: Vec<Value> = api.sign_ins().iter().map(js).collect();
        let ids: Vec<&str> = listed.iter().map(|r| r["signInId"].as_str().unwrap()).collect();
        assert_eq!(ids, vec![a.sign_in_id.as_str(), b.sign_in_id.as_str(), done_id.as_str()]);
        assert_eq!(listed[0], json!({ "signInId": a.sign_in_id, "target": { "workspaceId": "ws_1" }, "agent": "codex" }));
        assert_eq!(listed[2]["ended"], json!(true));

        // Ended runs are pruned as another run ends: strictly older than the kept time goes, exactly that old stays.
        rig.clock.advance(KEPT);
        api.sign_in_stop(&b.sign_in_id).unwrap();
        tick().await;
        let kept: Vec<String> = api.ended.borrow().keys().cloned().collect();
        assert_eq!(kept, vec![done_id.clone(), b.sign_in_id.clone()]);
        rig.clock.advance(1);
        api.sign_in_stop(&a.sign_in_id).unwrap();
        tick().await;
        let kept: Vec<String> = api.ended.borrow().keys().cloned().collect();
        assert_eq!(kept, vec![b.sign_in_id.clone(), a.sign_in_id.clone()]);
        assert_eq!(api.ended.borrow().get(&a.sign_in_id).unwrap().1, T0 + KEPT + 1);
    });
}

// N10: agents-read.test.ts:283, 379, 399
#[test]
fn follow_code_and_stop_refuse_an_id_not_running() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let want = Some(usage(NO_SIGN_IN_REFUSAL));
        assert_eq!(api.sign_in_follow("si_nope", no_follower()).err(), want);
        assert_eq!(api.sign_in_code("si_nope", "x").await.err(), want);
        assert_eq!(api.sign_in_stop("si_nope").err(), want);

        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        tick().await;
        rig.finish_plan();
        tick().await;
        assert_eq!(api.sign_in_code(&first.sign_in_id, "ABCD-1234").await.err(), want);
        assert_eq!(api.sign_in_follow(&first.sign_in_id, no_follower()).err(), want);
        assert_eq!(api.sign_in_stop(&first.sign_in_id).err(), want);

        *rig.plan.borrow_mut() = Plan::Quiet;
        let quiet = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        tick().await;
        assert_eq!(api.sign_in_code(&quiet.sign_in_id, "ABCD-1234").await.err(), want);
        assert_eq!(rig.codes.borrow().len(), 0);
        api.sign_in_stop(&quiet.sign_in_id).unwrap();
    });
}

// N10: agents-read.test.ts:263
#[test]
fn a_code_goes_to_the_writer_the_run_handed() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let first = ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        tick().await;
        ok(api.sign_in_code(&first.sign_in_id, "ABCD-1234").await);
        ok(api.sign_in_code(&first.sign_in_id, "EFGH-5678").await);
        assert_eq!(*rig.codes.borrow(), vec!["ABCD-1234".to_string(), "EFGH-5678".to_string()]);
        rig.finish_plan();
        tick().await;
        assert_eq!(api.sign_in_code(&first.sign_in_id, "ABCD-1234").await.err(), Some(usage(NO_SIGN_IN_REFUSAL)));
        assert_eq!(rig.codes.borrow().len(), 2);
    });
}

// N11: agents-read.ts:597-605 (no TS case)
#[test]
fn sign_in_line_refuses_a_failed_gh_install_with_its_fix() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        *rig.gh_steps.borrow_mut() = vec![("fetching".into(), Some("2026-09-24T12:00:01.000Z".into()), None)];
        *rig.gh_rows.borrow_mut() = vec![gh_row("failed", Some("fetching gh\nE: nope\n"))];
        let api = rig.reads();
        let refused = api.sign_in_line(&pl("pl_1"), &ask("gh"), None).await;
        assert_eq!(refused.err(), Some(refusal(&tool_not_installed_line(GH, "E: nope"), &tool_not_installed_fix(GH), None)));
        assert_eq!(rig.lines_of(&["door.gh_there", "acts.sign_in_line"]), vec!["door.gh_there pl_1".to_string()]);

        let rig = Rig::new();
        rig.with_spoo();
        *rig.gh_rows.borrow_mut() = vec![gh_row("installed", None)];
        let line = rig.reads().sign_in_line(&pl("pl_1"), &ask("gh"), None).await.unwrap();
        assert_eq!(js(&line), json!({ "command": "codex login --device-auth" }));
        assert_eq!(rig.lines_of(&["door.gh_there", "acts.sign_in_line"]), vec!["door.gh_there pl_1".to_string(), "acts.sign_in_line box gh".to_string()]);
    });
}

// N11: agents-read.test.ts:404
#[test]
fn sign_in_line_answers_the_acts_line() {
    run(async {
        let rig = Rig::new();
        let api = rig.reads();
        let line = api.sign_in_line(&pl("here"), &ask("codex"), None).await.unwrap();
        assert_eq!(js(&line), json!({ "command": "codex login --device-auth" }));
        assert_eq!(rig.lines_of(&["acts.sign_in_line"]), vec!["acts.sign_in_line here codex".to_string()]);
        assert_eq!(rig.lines_of(&["door."]), Vec::<String>::new());

        rig.phase.set(WorkspacePhase::Napping);
        assert_eq!(api.sign_in_line(&ws("ws_1"), &ask("codex"), None).await.err(), Some(usage(&napping_sign_in_refusal("landing", None))));
        assert_eq!(rig.lines_of(&["acts.sign_in_line"]).len(), 1);
    });
}

// Restart: agents-read.ts:258-338 (no TS case)
#[test]
fn a_new_reads_lists_no_sign_in() {
    run(async {
        let rig = Rig::new();
        rig.with_spoo();
        let api = rig.reads();
        finished(&rig, &api, ws("ws_1"), ask("gemini")).await;
        ok(api.sign_in(ws("ws_1"), ask("codex"), no_follower(), None).await);
        api.read(&ws("ws_1"), None).await.unwrap();
        tick().await;
        assert_eq!(api.sign_ins().len(), 2);

        let again = rig.reads();
        assert_eq!(again.sign_ins().len(), 0);
        assert_eq!(again.running.borrow().len(), 0);
        assert_eq!(again.ended.borrow().len(), 0);
        assert_eq!(again.starting.borrow().len(), 0);
        assert_eq!(again.last.borrow().len(), 0);
        assert_eq!(again.relays.borrow().len(), 0);
        assert_eq!(api.sign_ins().len(), 2);
    });
}
