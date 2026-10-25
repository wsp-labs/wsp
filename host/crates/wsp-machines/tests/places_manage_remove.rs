// SPDX-License-Identifier: AGPL-3.0-only
//! places/manage, unit remove: holds, remove, update (spec `places_manage_remove.md`, rules H1-H6, HL1, RM1-RM15,
//! U1-U6). Ported from `places-remove.test.ts`, `places-update.test.ts`, `places-unable.test.ts`, the provider cases of
//! `places-remove-provider.test.ts` and the remove and update cases of `places-setup.test.ts`. Sentences, scripts and
//! digests come from `tests/fixtures/oracle/places_b_remove.json` (written by `oracle/places_b_remove.oracle.ts`) and
//! from `places_b.json`, never from the wsp-words functions, whose bodies are `todo!()`.
mod places_b_common;
use places_b_common::*;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_engine::link_backend::MachineLink;
use wsp_engine::machine::MachineBackend;
use wsp_engine::place_machine::PlaceFolderMachine;
use wsp_machines::places::context::PlaceDoorContext;
use wsp_machines::places::records::LocalFut;
use wsp_machines::places::manage::remove::{runtime_checkouts, runtime_checkouts_script, runtime_project_of, ReachesItself, THREADS_END_MS};
use wsp_machines::places::manage::ManageDoor;
use wsp_machines::places::types::*;
use wsp_machines::places::views::{LinkReads, PlaceViewsArea};
use wsp_store::keys::PlaceKeyPair;
use wsp_words::exit::{HostError, Kind};

// ---------------------------------------------------------------- the fixture and its words

fn fx() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/places_b_remove.json")).expect("the remove oracle fixture");
    serde_json::from_str(&text).expect("the oracle parses")
}

fn word(path: &str) -> String {
    fx().pointer(path).and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| panic!("no string in the remove oracle at {path}"))
}

fn w(name: &str) -> String {
    word(&format!("/words/{name}"))
}

/// `{said, fix}` or `{happened, fix, said}` of a refusal, as the oracle holds it.
fn halves(name: &str) -> (String, String) {
    (word(&format!("/words/{name}/said")), word(&format!("/words/{name}/fix")))
}

const NAME: &str = "old-macbook";
const SSH: &str = "maya@10.0.0.5";
const HOME: &str = "/home/maya";
const LISTING: &str = "for d in '/wsp/projects'/*/checkout; do [ -d \"$d\" ] && printf '%s\\n' \"$d\"; done; true";

fn login() -> PlaceLogin {
    PlaceLogin { ssh: SSH.into(), key_path: Some("/home/dev/.ssh/id".into()), host_key: Some("ssh-ed25519 AAAA".into()) }
}

fn usage(e: &HostError, said: &str, fix: &str) {
    assert_eq!(e.message, said);
    assert_eq!(e.fix.as_deref(), Some(fix));
    assert_eq!(e.kind, Some(Kind::Usage));
}

fn ask() -> RemoveAsk {
    RemoveAsk::default()
}

fn forget() -> RemoveAsk {
    RemoveAsk { forget: Some(true), ..RemoveAsk::default() }
}

fn force() -> RemoveAsk {
    RemoveAsk { force: Some(true), ..RemoveAsk::default() }
}

fn sudo_pw(p: &str) -> RemoveAsk {
    RemoveAsk { sudo_password: Some(p.into()), ..RemoveAsk::default() }
}

fn took(forks: Value, projects: Value) -> Value {
    json!({ "forks": forks, "projects": projects })
}

fn holds_of_json(v: Value) -> wsp_wire::views::place_more::PlaceHolds {
    serde_json::from_value(v).expect("holds")
}

fn took_of_json(v: Value) -> wsp_wire::views::place_more::PlaceRemovedTook {
    serde_json::from_value(v).expect("took")
}

fn removed_ids(d: &Door) -> Vec<String> {
    d.events.borrow().iter().filter_map(|e| if let PlaceEvent::Removed(r) = e { Some(r.place_id.clone()) } else { None }).collect()
}

/// A record, its caps, its link and its place gone and the one `place.removed` said (V18, RM15).
fn assert_gone(d: &Door) {
    assert!(d.stored(PLACE).is_none(), "the record is deleted from the store");
    assert!(d.caps(PLACE).is_none());
    assert!(!d.ctx.kept.borrow().contains_key(PLACE));
    assert!(!d.ctx.live.borrow().contains_key(PLACE));
    assert_eq!(removed_ids(d), vec![PLACE.to_string()]);
}

fn assert_kept(d: &Door) {
    assert!(d.stored(PLACE).is_some(), "the record is still stored");
    assert!(d.ctx.kept.borrow().contains_key(PLACE));
    assert!(removed_ids(d).is_empty());
    assert!(d.store.writes.borrow().is_empty(), "nothing was written: {:?}", d.store.writes.borrow());
}

fn pending_ids(d: &Door) -> Vec<String> {
    d.pending_docs().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect()
}

fn report_json(over: Value) -> Value {
    let mut base = at("/record/report");
    merge(&mut base, over);
    base
}

// ---------------------------------------------------------------- the roads

type Ran = Rc<RefCell<Vec<(String, String, Option<String>)>>>;
type Asked = Rc<RefCell<Vec<PlaceLeaveRequest>>>;
type Order = Arc<Mutex<Vec<String>>>;

fn order() -> Order {
    Arc::new(Mutex::new(Vec::new()))
}

fn at_order(o: &Order, f: impl Fn(&str) -> bool) -> Option<usize> {
    o.lock().unwrap().iter().position(|s| f(s))
}

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

#[derive(Default)]
struct Wires {
    leave: Option<Rc<dyn PlaceLeaver>>,
    run_over: Option<Rc<dyn PlaceRunOver>>,
    sudo: Option<Rc<dyn PlaceSudo>>,
    dial: Option<Rc<dyn PlaceDialler>>,
    update: Option<Rc<dyn PlaceUpdater>>,
    forget_key: Option<Rc<dyn PlaceKeyForgetter>>,
    back: Option<Rc<dyn PlaceBackHolder>>,
    provision: Option<Rc<dyn PlaceProvisioner>>,
}

impl Wires {
    fn put(self) -> Box<dyn FnOnce(&mut PlaceWiring)> {
        Box::new(move |w| {
            w.leave = self.leave;
            w.run_over = self.run_over;
            w.sudo_over = self.sudo;
            w.dial = self.dial;
            w.update = self.update;
            w.forget_key = self.forget_key;
            w.back = self.back;
            w.provision = self.provision;
        })
    }
}

fn build(key: &PlaceKeyPair, wires: Wires, f: impl FnOnce(&mut Seed)) -> Door {
    let key = key.clone();
    door(move |s| {
        s.host_key = key;
        s.wiring = wires.put();
        s.opts = Box::new(|o| o.dial_wait_ms = Some(15_000));
        f(s);
    })
}

fn leaver(asked: &Asked, answer: impl Fn(&PlaceLeaveRequest) -> PlaceResult<Vec<String>> + 'static) -> Rc<dyn PlaceLeaver> {
    let asked = asked.clone();
    Rc::new(Leave(Box::new(move |req| {
        asked.borrow_mut().push(req.clone());
        answer(&req)
    })))
}

/// The place file a box holds, naming the computer `place_id` and the host `key`.
fn place_file(place_id: &str, key: &str) -> String {
    let file = json!({ "placeId": place_id, "name": NAME, "hostName": "zingzys-mac", "hostUrls": ["http://192.168.1.10:14621"], "hostPublicKey": key, "keyPath": "/home/maya/.wsp/place.key", "joinedAt": "2026-10-07T00:00:00.000Z" });
    format!("{}\n", serde_json::to_string_pretty(&file).unwrap())
}

/// What the held read prints: the login's rc output, then its own line, then the file where one stands.
fn held_stdout(file: Option<String>, rc: Option<&str>) -> String {
    let before = rc.map(|r| format!("{r}\n")).unwrap_or_default();
    format!("{before}\nWSP_PLACE_HELD\n{}", file.unwrap_or_default())
}

/// A login that reaches a box whose place file `file` answers (none where it answers `None`), every script it was
/// asked to run written down with the login and the password it rode.
fn box_road(file: impl Fn() -> Option<String> + 'static, rc: Option<&'static str>, ran: &Ran) -> Rc<dyn PlaceRunOver> {
    let ran = ran.clone();
    let held = word("/scripts/held");
    Rc::new(RunOverRoad(
        Box::new(move |login, script, _timeout, password| {
            ran.borrow_mut().push((login.ssh.clone(), script.to_string(), password.map(String::from)));
            let stdout = if script == held { held_stdout(file(), rc) } else { String::new() };
            Ok(RunOver { exit_code: 0, stdout, stderr: String::new() })
        }),
        RefCell::new(Vec::new()),
    ))
}

/// A box that is this computer: its place file names `PLACE` and the host's key.
fn same_box(key: &PlaceKeyPair, ran: &Ran) -> Rc<dyn PlaceRunOver> {
    let key = key.public_key.clone();
    box_road(move || Some(place_file(PLACE, &key)), None, ran)
}

fn held_only(ran: &Ran) -> Vec<(String, String, Option<String>)> {
    ran.borrow().iter().filter(|(_, s, _)| *s == word("/scripts/held")).cloned().collect()
}

fn sudo_road(f: impl Fn(&PlaceLogin, Option<&str>, SudoVerb, &str) -> PlaceResult<String> + 'static) -> Rc<dyn PlaceSudo> {
    Rc::new(Sudo(Box::new(f)))
}

fn sudo_kind_error(message: &str) -> HostError {
    HostError::with_kind(message, Kind::parse("sudo"))
}

fn exec_out(code: i64, stdout: &str, stderr: &str) -> Map<String, Value> {
    obj(json!({ "exitCode": code, "stdout": stdout, "stderr": stderr }))
}

/// A link whose daemon answers `place.leave` with `swept` and every exec with nothing, each recorded in `order` as
/// `reach:place.leave`, `reach:cgroups` or `reach:exec`. `over(op, cmd)` answers instead where it says something.
fn reach_with(order: &Order, swept: Vec<&'static str>, over: impl Fn(&str, &str) -> Option<Result<Map<String, Value>, HostError>> + 'static) -> Rc<FakeReach> {
    let order = order.clone();
    let cgroups = word("/scripts/cgroupsEnd");
    FakeReach::new(move |op, params| {
        let cmd = params.get("cmd").and_then(|c| c.as_str()).unwrap_or("").to_string();
        let tag = match op {
            "exec" if cmd == cgroups => "reach:cgroups".to_string(),
            "exec" => "reach:exec".to_string(),
            other => format!("reach:{other}"),
        };
        order.lock().unwrap().push(tag);
        if let Some(said) = over(op, &cmd) {
            return said;
        }
        match op {
            "ping" => Ok(Map::new()),
            "exec" => Ok(exec_out(0, "", "")),
            "place.leave" => Ok(obj(json!({ "swept": swept }))),
            other => Err(HostError::new(format!("unknown op {other}"))),
        }
    })
}

fn reach_plain(order: &Order, swept: Vec<&'static str>) -> Rc<FakeReach> {
    reach_with(order, swept, |_, _| None)
}

/// The link a `PlaceMachine` rides: each exec recorded in `order` as `link:<cmd>`; `f(cmd)` answers `(exit, stdout)`
/// where it says something, else exit 0 and nothing.
fn guest(order: &Order, f: impl Fn(&str) -> Option<(i64, String)> + Send + Sync + 'static) -> Arc<FakeMachineLink> {
    let order = order.clone();
    FakeMachineLink::new(move |op, params| {
        if op != "exec" {
            return Err(format!("unknown op {op}"));
        }
        let cmd = params.get("cmd").and_then(|c| c.as_str()).unwrap_or("").to_string();
        order.lock().unwrap().push(format!("link:{cmd}"));
        let (code, stdout) = f(&cmd).unwrap_or((0, String::new()));
        Ok(obj(json!({ "exitCode": code, "stdout": stdout, "stderr": "", "truncated": false })))
    })
}

fn setup_over(g: Arc<FakeMachineLink>) -> FakeSetup {
    let s = FakeSetup::default();
    *s.link.borrow_mut() = Some(g as Arc<dyn MachineLink>);
    s
}

/// A detached run's one poll, as a guest answers it: exit 0, the output so far, and the run gone.
fn polled(out: &str) -> String {
    ["WSP_POLL".to_string(), "0".to_string(), b64(&format!("{out}\n")), String::new(), "down".to_string(), "WSP_POLL_END".to_string()].join("\n")
}

/// A guest that answers the two long reads of the unmerge with `reads` in the order they are made.
fn unmerging(order: &Order, reads: Vec<String>) -> Arc<FakeMachineLink> {
    let reads = Mutex::new(VecDeque::from(reads));
    guest(order, move |cmd| {
        if cmd.contains("WSP_LAUNCHED") {
            Some((0, "WSP_LAUNCHED\n".to_string()))
        } else if cmd.contains("WSP_POLL") {
            Some((0, polled(&reads.lock().unwrap().pop_front().unwrap_or_default())))
        } else {
            None
        }
    })
}

fn project_on(id: &str, name: &str, checkout: Option<&str>) -> ProjectOn {
    ProjectOn { id: id.into(), name: name.into(), path: format!("/home/dev/{name}"), checkout: checkout.map(String::from) }
}

fn f1() -> Value {
    json!({ "name": "f1", "threads": 0 })
}

fn record_with_plugins(extra: Value) -> Value {
    let mut over = json!({ "applied": { "hash": "h1", "at": "2026-10-02T00:00:00.000Z", "rows": [
        { "id": "plugins/lint", "label": "PLUGINS/LINT", "outcome": "installed", "step": "plugins" },
        { "id": "plugins/stuck", "label": "PLUGINS/STUCK", "outcome": "installed", "step": "plugins" },
        { "id": "plugins/fmt", "label": "PLUGINS/FMT", "outcome": "present", "step": "plugins" }
    ] }, "picks": { "name": "web", "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": { "lint": {}, "stuck": {}, "fmt": {} }, "folders": {}, "configs": {} } });
    merge(&mut over, extra);
    over
}

// ---------------------------------------------------------------- helpers: H1, H2, H3

#[test]
fn runtime_checkouts_script_text() {
    // spec: H1
    assert_eq!(runtime_checkouts(), "/wsp/projects");
    assert_eq!(runtime_checkouts_script(), LISTING);
    assert_eq!(THREADS_END_MS, 60_000);
}

#[test]
fn runtime_project_of_cases() {
    // spec: H2
    let cases: [(Option<&str>, Option<&str>); 12] = [
        (Some("/wsp/projects/app/checkout"), Some("app")),
        (Some("/wsp/projects/x-y.z/checkout"), Some("x-y.z")),
        (None, None),
        (Some("/wsp/projects/a/b/checkout"), None),
        (Some("/wsp/projects//checkout"), None),
        (Some("/wsp/projects/checkout"), None),
        (Some("/wsp/projects/app"), None),
        (Some("/wsp/projects/app/checkout/"), None),
        (Some("/other/projects/app/checkout"), None),
        (Some("/wsp/projectsx/app/checkout"), None),
        (Some("/home/maya/app/checkout"), None),
        (Some(""), None),
    ];
    for (checkout, want) in cases {
        assert_eq!(runtime_project_of(checkout).as_deref(), want, "{checkout:?}");
    }
}

#[test]
fn reaches_itself_unchecked_is_usage() {
    // spec: H3, R22
    run(async {
        let key = new_key();
        for (name, run_over, said) in [
            ("noRoad", None, "uncheckedNoRoad"),
            ("exit", Some(RunOver { exit_code: 3, stdout: String::new(), stderr: String::new() }), "uncheckedExit"),
            ("sudo", Some(RunOver { exit_code: 1, stdout: String::new(), stderr: "sudo: a password is required\n".into() }), "uncheckedSudo"),
            ("timeout", Some(RunOver { exit_code: 124, stdout: String::new(), stderr: String::new() }), "uncheckedTimeout"),
        ] {
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let road = run_over.map(|r| {
                let ran = ran.clone();
                Rc::new(RunOverRoad(
                    Box::new(move |l: &PlaceLogin, s: &str, _t: u64, _p: Option<&str>| {
                        ran.borrow_mut().push((l.ssh.clone(), s.to_string(), None));
                        Ok(r.clone())
                    }),
                    RefCell::new(vec![]),
                )) as Rc<dyn PlaceRunOver>
            });
            let d = build(&key, Wires { run_over: road, ..Wires::default() }, |_| {});
            let e = d.manage.reaches_itself(&record(json!({})), &login(), None).await.expect_err(name);
            let (said, fix) = (word(&format!("/words/{said}/said")), word(&format!("/words/{said}/fix")));
            usage(&e, &said, &fix);
            assert_eq!(held_only(&ran).len(), usize::from(name != "noRoad"), "{name}: the held read ran once, or never with no road");
        }
        // no home recorded: nothing is read
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |_| {});
        let mut r = report_json(json!({}));
        r["login"] = json!({ "USER": "maya" });
        let e = d.manage.reaches_itself(&record(json!({ "report": r })), &login(), None).await.expect_err("no home");
        let (said, fix) = halves("uncheckedNoHome");
        usage(&e, &said, &fix);
        assert!(ran.borrow().is_empty());
    });
}

fn new_key() -> PlaceKeyPair {
    wsp_store::keys::new_place_key_pair()
}

#[test]
fn reaches_itself_answers_stands_elsewhere_and_refused() {
    // TS: places-remove.test.ts:493 "runs nothing over a login that reaches another computer's place file, or none"; spec: H3, R22
    run(async {
        let key = new_key();
        let file = Rc::new(RefCell::new(Some(place_file(PLACE, &key.public_key))));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let f = file.clone();
        let road = box_road(move || f.borrow().clone(), Some("Welcome to box"), &ran);
        let d = build(&key, Wires { run_over: Some(road), ..Wires::default() }, |s| {
            s.records = vec![record_json(json!({})), record_json(json!({ "id": PLACE2, "name": "vps" }))];
        });
        let rec = record(json!({}));
        assert_eq!(d.manage.reaches_itself(&rec, &login(), Some("pw")).await.unwrap(), ReachesItself::Stands);
        assert_eq!(ran.borrow().last().unwrap().2.as_deref(), Some("pw"), "the password rides the read");
        *file.borrow_mut() = Some(place_file(PLACE2, &key.public_key));
        assert_eq!(d.manage.reaches_itself(&rec, &login(), None).await.unwrap(), ReachesItself::Elsewhere { other: Some("vps".into()) });
        *file.borrow_mut() = Some(place_file("p_0123456789abcdef", &key.public_key));
        assert_eq!(d.manage.reaches_itself(&rec, &login(), None).await.unwrap(), ReachesItself::Elsewhere { other: None });
        *file.borrow_mut() = Some(place_file(PLACE, &new_key().public_key));
        assert_eq!(d.manage.reaches_itself(&rec, &login(), None).await.unwrap(), ReachesItself::Elsewhere { other: None }, "another host's key is another computer");
        *file.borrow_mut() = None;
        assert_eq!(d.manage.reaches_itself(&rec, &login(), None).await.unwrap(), ReachesItself::Elsewhere { other: None });
        // a login that would not stand reads as refused, with the road's own sentence
        let refusing: Rc<dyn PlaceRunOver> = Rc::new(RunOverRoad(Box::new(|_, _, _, _| Err(place_login_refused("ssh: connect to host 10.0.0.5 port 22: Connection refused"))), RefCell::new(vec![])));
        let d = build(&key, Wires { run_over: Some(refusing), ..Wires::default() }, |_| {});
        assert_eq!(d.manage.reaches_itself(&rec, &login(), None).await.unwrap(), ReachesItself::Refused { refused: "ssh: connect to host 10.0.0.5 port 22: Connection refused".into() });
    });
}

// ---------------------------------------------------------------- holds: H4, H5, H6, HL1

#[test]
fn holds_away_reads_nothing() {
    // TS: oracle remove.holdsAway; spec: H4
    run(async {
        let key = new_key();
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [f1()], "projects": [], "unsaved": ["site has 2 commits no remote has"] }));
            *s.recording.projects.borrow_mut() = vec![project_on("pr_1", "site", Some("/wsp/projects/site/checkout"))];
        });
        let holds = d.manage.holds(PLACE).await.unwrap();
        assert_eq!(js(&holds), json!({ "forks": [f1()], "projects": [], "unsaved": ["site has 2 commits no remote has"], "away": true }));
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false")]);
        assert!(ran.borrow().is_empty());
        let plain = build(&key, Wires::default(), |_| {});
        assert_eq!(outcome(&plain.manage.holds(PLACE).await), at("/remove/holdsAway"));
    });
}

#[test]
fn holds_answering_link_reads_over_it() {
    // spec: H4 (a live link reads over it, away is not said)
    run(async {
        let key = new_key();
        let o = order();
        let d = build(&key, Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [f1()], "projects": [], "unsaved": [] }));
        });
        link(&d, PLACE, reach_plain(&o, vec![]));
        let holds = d.manage.holds(PLACE).await.unwrap();
        assert_eq!(js(&holds), json!({ "forks": [f1()], "projects": [], "unsaved": [] }));
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=true")]);
        assert!(o.lock().unwrap().is_empty(), "a computer that does not say takesRuntime is asked no listing");
        let report_false = build(&key, Wires::default(), |s| s.records = vec![record_json(json!({ "report": report_json(json!({ "takesRuntime": false })) }))]);
        link(&report_false, PLACE, reach_plain(&o, vec![]));
        report_false.manage.holds(PLACE).await.unwrap();
        assert!(o.lock().unwrap().is_empty(), "takesRuntime false is not true: no listing");
    });
}

#[test]
fn holds_takes_runtime_adds_unrecorded_checkouts() {
    // spec: H4, H5; TS: places-setup.test.ts:598 "takes a project the person recorded there by hand with it too"
    run(async {
        let key = new_key();
        let o = order();
        let seen = Rc::new(RefCell::new(Vec::<(String, Value)>::new()));
        let reach = {
            let seen = seen.clone();
            reach_with(&o, vec![], move |_, cmd| {
                seen.borrow_mut().push((cmd.to_string(), Value::Null));
                if cmd == LISTING {
                    return Some(Ok(exec_out(0, "/wsp/projects/app/checkout\n\n/wsp/projects/old/checkout\n/wsp/projects/fresh/checkout\n", "")));
                }
                if cmd.contains("'/wsp/projects/old/checkout'") {
                    return Some(Ok(exec_out(0, "2 1 0\n", "")));
                }
                if cmd.contains("'/wsp/projects/fresh/checkout'") {
                    return Some(Ok(exec_out(1, "", "git failed\n")));
                }
                None
            })
        };
        let d = build(&key, Wires::default(), |s| {
            s.records = vec![record_json(json!({ "report": report_json(json!({ "takesRuntime": true })) }))];
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [], "projects": [{ "name": "app", "threads": 0 }], "unsaved": ["site has 2 commits no remote has"] }));
            *s.recording.projects.borrow_mut() = vec![project_on("pr_1", "app", Some("/wsp/projects/app/checkout")), project_on("pr_2", "mine", None)];
        });
        link(&d, PLACE, reach.clone());
        let held = d.manage.holds(PLACE).await.unwrap();
        assert_eq!(
            js(&held),
            json!({ "forks": [], "projects": [{ "name": "app", "threads": 0 }], "unsaved": ["site has 2 commits no remote has", w("unsavedOld"), w("unreadFresh")] })
        );
        // the listing is asked first with the threads' bound; the recorded checkout is never read again
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=true"), format!("projectsOn {PLACE}")]);
        let asked = reach.asked.borrow();
        assert_eq!(asked[0].0, "exec");
        assert_eq!(asked[0].1["cmd"], json!(LISTING));
        assert_eq!(asked[0].1["timeoutMs"], json!(60_000));
        assert_eq!(asked.len(), 3);
        assert!(!seen.borrow().iter().any(|(c, _)| c.contains("'/wsp/projects/app/checkout'") && c != LISTING));
    });
}

#[test]
fn holds_listing_failure_one_line() {
    // spec: H5
    run(async {
        let key = new_key();
        let o = order();
        for (what, reply, line) in [
            ("exits non-zero with stderr", Ok(exec_out(2, "", "x\nPermission denied\n")), w("unpushedUnreadStderr")),
            ("exits non-zero silently", Ok(exec_out(2, "", "")), w("unpushedUnread")),
            ("fails to ask", Err(HostError::new("boom")), w("unreadBoom")),
        ] {
            let d = build(&key, Wires::default(), |s| s.records = vec![record_json(json!({ "report": report_json(json!({ "takesRuntime": true })) }))]);
            let answer = Rc::new(RefCell::new(Some(reply)));
            let a = answer.clone();
            link(&d, PLACE, reach_with(&o, vec![], move |_, _| a.borrow_mut().take()));
            let held = d.manage.holds(PLACE).await.unwrap();
            assert_eq!(js(&held)["unsaved"], json!([line]), "{what}");
        }
    });
}

#[test]
fn unrecorded_unsaved_reads_each_listed_checkout_not_recorded() {
    // spec: H5
    run(async {
        let key = new_key();
        let o = order();
        let d = build(&key, Wires::default(), |s| *s.recording.projects.borrow_mut() = vec![project_on("pr_1", "app", Some("/wsp/projects/app/checkout"))]);
        link(
            &d,
            PLACE,
            reach_with(&o, vec![], |_, cmd| (cmd == LISTING).then(|| Ok(exec_out(0, "/wsp/projects/app/checkout\n", "")))),
        );
        assert_eq!(d.manage.unrecorded_unsaved(PLACE).await.unwrap(), Vec::<String>::new());
        assert_eq!(*o.lock().unwrap(), vec!["reach:exec".to_string()]);
    });
}

#[test]
fn holds_unknown_id_empty() {
    // TS: oracle remove.holdsUnknown; spec: HL1
    run(async {
        let d = build(&new_key(), Wires::default(), |_| {});
        assert_eq!(outcome(&d.manage.holds("p_nope").await), at("/remove/holdsUnknown"));
        assert!(d.recording.calls().is_empty());
    });
}

fn stub(listing_capability: bool, offers_listing: bool) -> Arc<StubBackend> {
    let mut b = StubBackend::new();
    b.capabilities["snapshotListing"] = json!(listing_capability);
    b.offers.list_snapshots = offers_listing;
    Arc::new(b)
}

/// A provider that lists nothing of this host's: every other road is the stub's refusal.
struct EmptyBoat {
    inner: StubBackend,
}

impl EmptyBoat {
    fn new() -> Self {
        EmptyBoat { inner: StubBackend::new() }
    }
}

#[async_trait]
impl MachineBackend for EmptyBoat {
    fn capabilities(&self) -> wsp_engine::machine::Capabilities {
        self.inner.capabilities()
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        self.inner.pricing()
    }
    fn offers(&self) -> wsp_engine::machine::BackendOffers {
        self.inner.offers()
    }
    async fn create(&self, a: wsp_engine::machine::MachineSpec, b: Option<wsp_engine::machine::Waiting>, c: Option<String>) -> wsp_engine::errors::Result<Arc<dyn wsp_engine::machine::Machine>> {
        self.inner.create(a, b, c).await
    }
    async fn get(&self, id: &str) -> wsp_engine::errors::Result<Arc<dyn wsp_engine::machine::Machine>> {
        self.inner.get(id).await
    }
    async fn list(&self, _: Option<&IndexMap<String, String>>) -> wsp_engine::errors::Result<Vec<wsp_engine::machine::MachineListRow>> {
        Ok(vec![])
    }
    async fn delete_snapshot(&self, id: &str) -> wsp_engine::errors::Result<()> {
        self.inner.delete_snapshot(id).await
    }
    async fn place_limits(&self) -> wsp_engine::errors::Result<Option<wsp_wire::views::place_more::CloudLimits>> {
        self.inner.place_limits().await
    }
    async fn capacity(&self) -> wsp_engine::errors::Result<wsp_engine::machine::PlaceCapacity> {
        self.inner.capacity().await
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> wsp_engine::errors::Result<Vec<wsp_wire::wire::machine_link::SnapshotRow>> {
        self.inner.list_snapshots(ids).await
    }
}

fn boat_table(b: &Arc<StubBackend>) -> Rc<Providers> {
    Rc::new(Providers { wired: "boat".into(), rows: IndexMap::from([("boat".to_string(), b.clone() as Arc<dyn MachineBackend>)]) })
}

#[test]
fn holds_provider_images_sized() {
    // TS: places-remove-provider.test.ts:315 "names this host's image snapshots there with their sizes"; spec: HL1, H6
    run(async {
        let key = new_key();
        let backend = stub(true, true);
        *backend.snapshots.lock().unwrap() = Some(Ok(vec![json!({ "id": "wsp-acme-default-v1", "sizeBytes": 345_000_000 }), json!({ "id": "zero", "sizeBytes": 0 })]));
        let table = boat_table(&backend);
        let d = build(&key, Wires::default(), |s| {
            s.records = vec![];
            s.providers = Some(table);
            *s.recording.images.borrow_mut() = Some(vec![
                HeldImage { id: "wsp-acme-default-v1".into(), template_id: None },
                HeldImage { id: "zero".into(), template_id: None },
                HeldImage { id: "unlisted".into(), template_id: None },
            ]);
            *s.recording.sub_holds.borrow_mut() = Some(vec!["thread t1 has 1 commit no remote has".into()]);
        });
        let held = d.manage.holds("boat").await.unwrap();
        assert_eq!(
            js(&held),
            json!({ "forks": [], "projects": [], "unsaved": ["thread t1 has 1 commit no remote has"], "images": [{ "id": "wsp-acme-default-v1", "sizeBytes": 345_000_000 }, { "id": "zero" }, { "id": "unlisted" }] })
        );
        assert_eq!(*backend.asked.lock().unwrap(), vec!["listSnapshots wsp-acme-default-v1,zero,unlisted".to_string()], "the ids are given, so the provider reads its named snapshots alone");
        assert_eq!(d.manage.images_sized("boat").await.unwrap().len(), 3);
    });
}

#[test]
fn holds_provider_images_without_listing() {
    // spec: H6 (no listing without the capability, the method, or an image; a failed listing lists nothing), HL1 (no
    // images key where none, `unsaved` empty where no sub holds)
    run(async {
        let key = new_key();
        let image = || vec![HeldImage { id: "snap".into(), template_id: None }];
        let rows = json!([{ "id": "snap", "sizeBytes": 9 }]);
        for (what, capable, offers, images, failing) in [
            ("capability false", false, true, true, false),
            ("method not offered", true, false, true, false),
            ("no images", true, true, false, false),
            ("listing fails", true, true, true, true),
        ] {
            let backend = stub(capable, offers);
            *backend.snapshots.lock().unwrap() = Some(if failing { Err("429".to_string()) } else { Ok(rows.as_array().unwrap().clone()) });
            let table = boat_table(&backend);
            let d = build(&key, Wires::default(), |s| {
                s.records = vec![];
                s.providers = Some(table);
                *s.recording.images.borrow_mut() = Some(if images { image() } else { vec![] });
            });
            let held = d.manage.holds("boat").await.unwrap();
            let want = if images { json!({ "forks": [], "projects": [], "unsaved": [], "images": [{ "id": "snap" }] }) } else { json!({ "forks": [], "projects": [], "unsaved": [] }) };
            assert_eq!(js(&held), want, "{what}");
            let asked = backend.asked.lock().unwrap().len();
            assert_eq!(asked, usize::from(capable && offers && images), "{what}");
        }
    });
}

#[test]
fn holds_provider_without_records_is_empty() {
    // TS: oracle remove.holdsProvider; spec: HL1
    run(async {
        let d = build(&new_key(), Wires::default(), |_| {});
        assert_eq!(outcome(&d.manage.holds("boat").await), at("/remove/holdsProvider"));
    });
}

// ---------------------------------------------------------------- remove: providers (RM1-RM4)

#[test]
fn remove_provider_forget_asks_nothing() {
    // TS: places-remove-provider.test.ts:242 "forgets a provider whose key it refuses"; spec: RM2
    run(async {
        let key = new_key();
        let backend = Arc::new(StubBackend::new());
        let table = boat_table(&backend);
        let forgot = Rc::new(RefCell::new(Vec::<String>::new()));
        let f = forgot.clone();
        let forget_key: Rc<dyn PlaceKeyForgetter> = Rc::new(ForgetKey(Box::new(move |id: &str| {
            f.borrow_mut().push(id.to_string());
            Ok(Some("the key is still in WSP_BOAT_KEY".to_string()))
        })));
        let d = build(&key, Wires { forget_key: Some(forget_key), ..Wires::default() }, |s| {
            s.records = vec![];
            s.providers = Some(table);
            s.recording.has_forget_records.set(true);
            s.recording.has_release_records.set(true);
        });
        let answer = d.manage.remove("boat", forget()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "swept": [], "note": "the key is still in WSP_BOAT_KEY" }));
        assert_eq!(d.recording.calls(), vec!["forgetRecordsOn boat".to_string()], "the records go; the release of a real remove is not asked");
        assert_eq!(*forgot.borrow(), vec!["boat".to_string()]);
        assert_eq!(removed_ids(&d), vec!["boat".to_string()]);
        assert!(backend.asked.lock().unwrap().is_empty(), "nothing is asked of the provider");
    });
}

#[test]
fn remove_provider_forget_without_key_road_has_no_note() {
    // spec: RM2 (forget_records_on and forget_key only where present, note only where the key says one)
    run(async {
        let backend = Arc::new(StubBackend::new());
        let table = boat_table(&backend);
        let d = build(&new_key(), Wires::default(), |s| {
            s.records = vec![];
            s.providers = Some(table);
        });
        let answer = d.manage.remove("boat", forget()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "swept": [] }));
        assert!(d.recording.calls().is_empty());
        assert_eq!(removed_ids(&d), vec!["boat".to_string()]);
    });
}

#[test]
fn remove_provider_listed_without_backend_is_not_removed() {
    // TS: oracle remove.providerForget; spec: RM1, RM4
    run(async {
        let forgot = Rc::new(RefCell::new(0));
        let f = forgot.clone();
        let forget_key: Rc<dyn PlaceKeyForgetter> = Rc::new(ForgetKey(Box::new(move |_: &str| {
            *f.borrow_mut() += 1;
            Ok(Some("the key is still in WSP_BOAT_KEY".to_string()))
        })));
        let d = build(&new_key(), Wires { forget_key: Some(forget_key), ..Wires::default() }, |s| {
            s.recording.has_forget_records.set(true);
            s.recording.has_release_records.set(true);
        });
        assert_eq!(outcome(&d.manage.remove("boat", forget()).await), at("/remove/providerForget"));
        assert_eq!(outcome(&d.manage.remove("boat", ask()).await), at("/remove/providerForget"));
        assert!(d.recording.calls().is_empty());
        assert_eq!(*forgot.borrow(), 0);
        assert!(d.events.borrow().is_empty());
        assert!(d.store.writes.borrow().is_empty());
    });
}

#[test]
fn remove_provider_that_fails_to_list_goes_nowhere() {
    // TS: places-remove-provider.test.ts:228 "refuses in two halves when Boat refuses the key, which stays"; spec: RM3
    // (the answer is not `removed`: the records are not released, the event is not said, the key is not forgotten, and
    // the failure passes through). The provider's own words are places-a's provider_remove tests.
    run(async {
        let backend = Arc::new(StubBackend::new());
        let table = boat_table(&backend);
        let forgot = Rc::new(RefCell::new(0));
        let f = forgot.clone();
        let forget_key: Rc<dyn PlaceKeyForgetter> = Rc::new(ForgetKey(Box::new(move |_: &str| {
            *f.borrow_mut() += 1;
            Ok(None)
        })));
        let d = build(&new_key(), Wires { forget_key: Some(forget_key), ..Wires::default() }, |s| {
            s.records = vec![];
            s.providers = Some(table);
            *s.recording.machines.borrow_mut() = Some(vec!["m_1".into()]);
            s.recording.has_release_records.set(true);
        });
        let e = d.manage.remove("boat", ask()).await.expect_err("the listing fails");
        assert_eq!(e.message, "list is not on this stub");
        assert_eq!(*forgot.borrow(), 0);
        assert_eq!(d.recording.calls(), vec!["machinesOn boat".to_string()], "the machines this host's records hold are read, nothing is released");
        assert!(d.events.borrow().is_empty());
    });
}

#[test]
fn remove_provider_with_nothing_held_forgets_the_key_and_releases() {
    // TS: places-remove-provider.test.ts:97 "deletes every machine this host made there"; spec: RM3 (answer as it
    // answered; release and place.removed where removed)
    run(async {
        let backend = Arc::new(EmptyBoat::new());
        let table = Rc::new(Providers { wired: "boat".into(), rows: IndexMap::from([("boat".to_string(), backend.clone() as Arc<dyn MachineBackend>)]) });
        let forgot = Rc::new(RefCell::new(Vec::<String>::new()));
        let f = forgot.clone();
        let forget_key: Rc<dyn PlaceKeyForgetter> = Rc::new(ForgetKey(Box::new(move |id: &str| {
            f.borrow_mut().push(id.to_string());
            Ok(Some("the key is still in WSP_BOAT_KEY".to_string()))
        })));
        let d = build(&new_key(), Wires { forget_key: Some(forget_key), ..Wires::default() }, |s| {
            s.records = vec![];
            s.providers = Some(table);
            s.recording.has_release_records.set(true);
            *s.recording.machines.borrow_mut() = Some(vec![]);
        });
        let answer = d.manage.remove("boat", ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "swept": [], "deleted": 0, "note": "the key is still in WSP_BOAT_KEY" }));
        assert_eq!(d.recording.calls(), vec!["machinesOn boat".to_string(), "releaseRecordsOn boat".to_string()]);
        assert_eq!(*forgot.borrow(), vec!["boat".to_string()]);
        assert_eq!(removed_ids(&d), vec!["boat".to_string()]);
    });
}

#[test]
fn remove_unknown_id_not_removed() {
    // TS: places-remove.test.ts:176 "answers that nothing was removed for an id this host holds no place by"; oracle remove.unknown; spec: RM4
    run(async {
        let d = build(&new_key(), Wires::default(), |_| {});
        assert_eq!(outcome(&d.manage.remove("p_deadbeefdeadbeef", ask()).await), at("/remove/unknown"));
        assert!(d.recording.calls().is_empty());
        assert!(d.events.borrow().is_empty());
        assert!(d.store.writes.borrow().is_empty());
        assert!(d.stored(PLACE).is_some());
    });
}

// ---------------------------------------------------------------- remove: refusals (RM5, RM6)

#[test]
fn remove_refuses_unsaved_without_force() {
    // TS: oracle remove.unsaved; spec: RM6
    run(async {
        let d = build(&new_key(), Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [], "projects": [], "unsaved": ["site has 2 commits no remote has"] }));
        });
        let e = d.manage.remove(PLACE, ask()).await.expect_err("unsaved work");
        assert_eq!(err_json(&e), at("/remove/unsaved"));
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false")], "nothing was taken");
        assert_kept(&d);
    });
}

#[test]
fn remove_force_takes_a_computer_holding_unsaved_work() {
    // spec: RM6 (`force` goes past the unsaved refusal), RM10
    run(async {
        let d = build(&new_key(), Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [], "projects": [], "unsaved": ["site has 2 commits no remote has"] }));
        });
        let answer = d.manage.remove(PLACE, force()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("stillInstalled") }));
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false"), format!("projectsOn {PLACE}"), format!("forgetOn {PLACE}")]);
        assert_gone(&d);
    });
}

#[test]
fn remove_away_with_forks_needs_forget() {
    // TS: oracle remove.awayWithForks, awayForget; spec: RM6, RM11, RM13
    run(async {
        let d = build(&new_key(), Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [f1()], "projects": [], "unsaved": [] }));
            *s.recording.forgotten.borrow_mut() = took_of_json(took(json!([f1()]), json!([])));
        });
        let e = d.manage.remove(PLACE, ask()).await.expect_err("away with forks");
        let (said, fix) = halves("awayRefusal");
        usage(&e, &said, &fix);
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false")], "nothing was taken");
        assert_kept(&d);
        let answer = d.manage.remove(PLACE, forget()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([f1()]), json!([])), "swept": [], "note": w("forgottenForks") }));
        assert_gone(&d);
        // a project folder alone is as much as a fork
        let d = build(&new_key(), Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [], "projects": [{ "name": "app", "threads": 0 }], "unsaved": [] }));
        });
        let e = d.manage.remove(PLACE, ask()).await.expect_err("away with a project");
        usage(&e, &halves("awayRefusal").0, &halves("awayRefusal").1);
    });
}

#[test]
fn remove_forget_on_answering_link_refused() {
    // spec: RM6
    run(async {
        let o = order();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let d = build(&new_key(), Wires { leave: Some(leaver(&asked, |_| Ok(vec![]))), ..Wires::default() }, |_| {});
        let reach = reach_plain(&o, vec!["x"]);
        link(&d, PLACE, reach.clone());
        let e = d.manage.remove(PLACE, forget()).await.expect_err("forget on a link that answers");
        let (said, fix) = halves("forgetAnswersRefusal");
        usage(&e, &said, &fix);
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=true")]);
        assert!(reach.ops().is_empty() && asked.borrow().is_empty());
        assert!(d.ctx.live.borrow().contains_key(PLACE), "the link is not cut");
        assert!(d.stored(PLACE).is_some() && removed_ids(&d).is_empty());
    });
}

#[test]
fn remove_refusal_order_forget_then_away_then_unsaved() {
    // spec: RM6 (in this order)
    run(async {
        let o = order();
        let d = build(&new_key(), Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [f1()], "projects": [], "unsaved": ["site has 2 commits no remote has"] }));
        });
        let e = d.manage.remove(PLACE, ask()).await.expect_err("away comes before unsaved");
        usage(&e, &halves("awayRefusal").0, &halves("awayRefusal").1);
        let d = build(&new_key(), Wires::default(), |s| {
            *s.recording.holds.borrow_mut() = holds_of_json(json!({ "forks": [f1()], "projects": [], "unsaved": ["site has 2 commits no remote has"] }));
        });
        link(&d, PLACE, reach_plain(&o, vec![]));
        let e = d.manage.remove(PLACE, forget()).await.expect_err("forget comes before unsaved");
        usage(&e, &halves("forgetAnswersRefusal").0, &halves("forgetAnswersRefusal").1);
        let e = d.manage.remove(PLACE, ask()).await.expect_err("unsaved over an answering link");
        assert_eq!(err_json(&e), at("/remove/unsaved"));
    });
}

// ---------------------------------------------------------------- remove: nothing to reach (RM13 unlinked)

#[test]
fn remove_unlinked_without_login_still_installed() {
    // TS: places-remove.test.ts:165 "says the agent is still installed when the place was not connected to sweep";
    // :181 "never logs in to a computer that joined by typing a code"; spec: RM12, RM13
    run(async {
        let key = new_key();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { leave: Some(leaver(&asked, |_| Ok(vec![]))), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |s| {
            s.records = vec![record_json(json!({ "road": null }))];
        });
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("stillInstalled") }));
        assert!(asked.borrow().is_empty() && ran.borrow().is_empty(), "a computer joined by a code has no login to run anything over");
        assert_gone(&d);
    });
}

#[test]
fn remove_wired_with_no_leave_road_still_installed() {
    // TS: places-remove.test.ts:357 "says the agent is still installed on a host wired with no road to log in to one"; spec: RM7, RM12
    run(async {
        let key = new_key();
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |_| {});
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(answer.swept, Vec::<String>::new());
        assert_eq!(answer.note.as_deref(), Some(w("stillInstalled").as_str()));
        assert!(ran.borrow().is_empty(), "no leave road: the login is not read either");
        assert_gone(&d);
    });
}

#[test]
fn remove_unlinked_forget_forgotten_line() {
    // TS: oracle remove.awayForget; spec: RM13 (forget, forks or none)
    run(async {
        let d = build(&new_key(), Wires::default(), |_| {});
        let answer = d.manage.remove(PLACE, forget()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("forgotten") }));
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false"), format!("projectsOn {PLACE}"), format!("forgetOn {PLACE}")]);
        assert_gone(&d);
    });
}

#[test]
fn remove_forget_over_unreadable_login_runs_nothing() {
    // TS: places-remove.test.ts:622 (a forget does not stop on a login it could not read); spec: RM9, RM12, RM13
    run(async {
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let r = ran.clone();
        let road: Rc<dyn PlaceRunOver> = Rc::new(RunOverRoad(
            Box::new(move |l, s, _, _| {
                r.borrow_mut().push((l.ssh.clone(), s.to_string(), None));
                Ok(RunOver { exit_code: 1, stdout: String::new(), stderr: "sudo: a password is required\n".into() })
            }),
            RefCell::new(vec![]),
        ));
        let d = build(&new_key(), Wires { leave: Some(leaver(&asked, |_| Ok(vec![]))), run_over: Some(road), ..Wires::default() }, |_| {});
        let answer = d.manage.remove(PLACE, forget()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("forgotten") }));
        assert_eq!(held_only(&ran).len(), 1);
        assert!(asked.borrow().is_empty(), "nothing ran on a computer whose login could not be read");
        assert_gone(&d);
    });
}

#[test]
fn remove_drops_pending_and_record() {
    // TS: oracle remove.stillInstalled, log, events, after; spec: RM15, V18
    run(async {
        let d = build(&new_key(), Wires::default(), |s| {
            s.mark = Some(PLACE.into());
            s.caps = vec![(PLACE.into(), json!({ "threads": 3 })), (PLACE2.into(), json!({ "threads": 1 }))];
            s.pending = vec![pending_json("a_000000000001", "10.0.0.5", json!({ "placeId": PLACE })), pending_json("a_000000000002", "10.0.0.9", json!({}))];
            *s.recording.forgotten.borrow_mut() = took_of_json(took(json!([f1()]), json!([])));
        });
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer)["took"], took(json!([f1()]), json!([])));
        assert_eq!(js(&answer)["swept"], json!([]));
        assert_eq!(js(&answer)["note"], json!(w("stillInstalled")));
        assert_gone(&d);
        assert!(d.mark().is_none(), "the default mark that named it goes");
        assert_eq!(pending_ids(&d), vec!["a_000000000002".to_string()], "only the add naming this place goes");
        assert!(d.caps(PLACE2).is_some());
        let writes = d.store.writes.borrow().clone();
        let pos = |what: &str| writes.iter().position(|w| w == what).unwrap_or_else(|| panic!("no write {what} in {writes:?}"));
        assert!(pos("delete pending-computers/a_000000000001") < pos(&format!("delete places/{PLACE}")), "the pending add goes before the record: {writes:?}");
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false"), format!("projectsOn {PLACE}"), format!("forgetOn {PLACE}")]);
    });
}

// ---------------------------------------------------------------- remove: over the login (RM7-RM12)

fn leave_answer(took: &'static [&'static str]) -> impl Fn(&PlaceLeaveRequest) -> PlaceResult<Vec<String>> + 'static {
    move |_| Ok(took.iter().map(|s| s.to_string()).collect())
}

#[test]
fn remove_leave_over_login_keeps_record_until_answered() {
    // TS: places-remove.test.ts:245 "runs the leave over that login, keeps the record until it answered, and says which road it took"; spec: RM7, RM9, RM12
    run(async {
        let key = new_key();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let holder: Rc<RefCell<Option<Rc<MemStore>>>> = Rc::new(RefCell::new(None));
        let during = Rc::new(RefCell::new(Vec::<bool>::new()));
        let leave = {
            let (holder, during) = (holder.clone(), during.clone());
            leaver(&asked, move |_| {
                during.borrow_mut().push(holder.borrow().as_ref().unwrap().doc("places", PLACE).is_some());
                Ok(vec!["systemd system unit wsp-place-1234abcd.service (stopped)".into(), "/home/maya/.wsp/place.json".into()])
            })
        };
        let d = build(&key, Wires { leave: Some(leave), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |_| {});
        *holder.borrow_mut() = Some(d.store.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(*during.borrow(), vec![true], "still this host's while the leave runs");
        assert_eq!(
            asked.borrow().clone(),
            vec![PlaceLeaveRequest { place_id: PLACE.into(), name: NAME.into(), report: record(json!({})).report, ssh: login(), sudo_password: None, force: None, projects: Some(vec![]) }]
        );
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": ["systemd system unit wsp-place-1234abcd.service (stopped)", "/home/maya/.wsp/place.json"], "note": w("sweptOverSsh") }));
        assert_eq!(held_only(&ran).len(), 1);
        assert_eq!(ran.borrow().len(), 1, "the place file read is the only other thing run over the login");
        assert_gone(&d);
    });
}

#[test]
fn remove_leave_over_login_past_rc_output() {
    // TS: places-remove.test.ts:270 "runs the leave over a login whose rc file prints before the place file is read"; spec: RM9
    run(async {
        let key = new_key();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let k = key.public_key.clone();
        let road = box_road(move || Some(place_file(PLACE, &k)), Some("Welcome to box"), &ran);
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&["/home/maya/.wsp/place.json"]))), run_over: Some(road), ..Wires::default() }, |_| {});
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(asked.borrow().len(), 1);
        assert_eq!(answer.note.as_deref(), Some(w("sweptOverSsh").as_str()));
        assert_gone(&d);
    });
}

#[test]
fn remove_sudo_refusal_fails_before_anything() {
    // TS: places-remove.test.ts:278 "asks for the sudo password before anything touches that computer, and rides it on the leave once sudo took it"; spec: RM7, RM8
    run(async {
        let key = new_key();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let tried = Rc::new(RefCell::new(Vec::<(Option<String>, SudoVerb, String, PlaceLogin)>::new()));
        let sudo = {
            let tried = tried.clone();
            sudo_road(move |l, pw, verb, name| {
                tried.borrow_mut().push((pw.map(String::from), verb, name.to_string(), l.clone()));
                if pw != Some("right") {
                    return Err(sudo_kind_error("maya@10.0.0.5 runs sudo only with maya's password. Type it at wsp remove old-macbook in a terminal."));
                }
                Ok("taken".into())
            })
        };
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&["/home/maya/.wsp"]))), run_over: Some(same_box(&key, &ran)), sudo: Some(sudo), ..Wires::default() }, |_| {});
        let e = d.manage.remove(PLACE, ask()).await.expect_err("sudo wants a password");
        assert_eq!(e.kind, Some(Kind::parse("sudo")));
        assert_eq!(e.message, "maya@10.0.0.5 runs sudo only with maya's password. Type it at wsp remove old-macbook in a terminal.");
        assert!(asked.borrow().is_empty() && ran.borrow().is_empty(), "nothing ran before the password: the read of the place file waits for it");
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=false")], "nothing was taken");
        assert_kept(&d);
        assert_eq!(tried.borrow()[0].3, login(), "the key the add kept is handed to the road");
        let answer = d.manage.remove(PLACE, sudo_pw("right")).await.unwrap();
        let t = tried.borrow();
        assert_eq!(t.iter().map(|x| x.0.clone()).collect::<Vec<_>>(), vec![None, Some("right".to_string())]);
        assert!(t.iter().all(|x| x.1 == SudoVerb::Remove && x.2 == NAME));
        assert_eq!(asked.borrow().iter().map(|r| r.sudo_password.clone()).collect::<Vec<_>>(), vec![Some("right".to_string())]);
        assert_eq!(ran.borrow()[0].2.as_deref(), Some("right"), "TS :603 the place file is read with the password sudo took");
        assert_eq!(answer.note.as_deref(), Some(w("sweptOverSsh").as_str()));
        assert_gone(&d);
    });
}

#[test]
fn remove_free_sudo_rides_no_password() {
    // TS: places-remove.test.ts:308 "rides no password on a login that reaches root without one, whatever came with the remove"; spec: RM8, R20
    run(async {
        let key = new_key();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(same_box(&key, &ran)), sudo: Some(sudo_road(|_, _, _, _| Ok("free".into()))), ..Wires::default() }, |_| {});
        d.manage.remove(PLACE, sudo_pw("stray")).await.unwrap();
        assert_eq!(asked.borrow().len(), 1);
        assert_eq!(asked.borrow()[0].sudo_password, None);
        assert_eq!(ran.borrow()[0].2, None);
    });
}

#[test]
fn remove_host_key_changed_runs_nothing_and_goes() {
    // TS: places-remove.test.ts:544 "lets the record go where the login's sudo asks for a password and the box answers with another key"; spec: RM8
    run(async {
        let key = new_key();
        for typed in [None, Some("typed")] {
            let asked: Asked = Rc::new(RefCell::new(vec![]));
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let tried = Rc::new(RefCell::new(Vec::<Option<String>>::new()));
            let sudo = {
                let tried = tried.clone();
                sudo_road(move |_, pw, _, _| {
                    tried.borrow_mut().push(pw.map(String::from));
                    Err(place_host_key_changed("maya@10.0.0.5 answered with ssh-ed25519 AAAAother, not the ssh-ed25519 SHA256:kept you pinned"))
                })
            };
            let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(same_box(&key, &ran)), sudo: Some(sudo), ..Wires::default() }, |_| {});
            let ask = typed.map(sudo_pw).unwrap_or_else(self::ask);
            let answer = d.manage.remove(PLACE, ask).await.unwrap();
            assert!(ran.borrow().is_empty() && asked.borrow().is_empty(), "no read and no leave over a login whose key changed");
            assert_eq!(*tried.borrow(), vec![typed.map(String::from)]);
            assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("elsewhereRemoved") }));
            assert_gone(&d);
        }
    });
}

#[test]
fn remove_login_elsewhere_runs_nothing() {
    // TS: places-remove.test.ts:493 "runs nothing over a login that reaches another computer's place file, or none, and lets the record go saying so"; spec: RM9, RM13
    run(async {
        let key = new_key();
        let other_host = new_key();
        let cases: Vec<(&str, Option<String>)> = vec![
            ("another computer's file", Some(place_file("p_0123456789abcdef", &key.public_key))),
            ("no place file", None),
            ("this computer's id under another host's key", Some(place_file(PLACE, &other_host.public_key))),
        ];
        for (what, file) in cases {
            let asked: Asked = Rc::new(RefCell::new(vec![]));
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let road = box_road(move || file.clone(), None, &ran);
            let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(road), ..Wires::default() }, |_| {});
            let answer = d.manage.remove(PLACE, ask()).await.unwrap();
            assert_eq!(ran.borrow().iter().map(|r| r.1.clone()).collect::<Vec<_>>(), vec![word("/scripts/held")], "{what}: the read of the place file is the one thing that ran there");
            assert!(asked.borrow().is_empty(), "{what}");
            assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("elsewhereRemoved") }), "{what}");
            assert_eq!(answer.note.as_deref(), Some("maya@10.0.0.5 no longer reaches the computer added here as old-macbook, so nothing was changed on the machine it reaches; old-macbook is off this host, and whatever wsp left on old-macbook itself stays until wsp leave runs there"));
            assert_gone(&d);
        }
    });
}

#[test]
fn remove_login_elsewhere_names_the_other_computer() {
    // TS: places-remove.test.ts:509 "names the other computer added here where a failed add tried again left a stale record on the same login"; spec: RM9, RM13, RM15
    run(async {
        let key = new_key();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let k = key.public_key.clone();
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(box_road(move || Some(place_file(PLACE2, &k)), None, &ran)), ..Wires::default() }, |s| {
            s.records = vec![record_json(json!({})), record_json(json!({ "id": PLACE2, "name": "vps", "joinedAt": "2026-10-03T00:00:00.000Z" }))];
        });
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert!(asked.borrow().is_empty());
        assert_eq!(answer.note.as_deref(), Some(w("elsewhereRemovedOther").as_str()));
        assert_eq!(answer.note.as_deref(), Some("maya@10.0.0.5 reaches the other computer added here as vps, which stays added, so nothing was changed there; this record of old-macbook is off this host"));
        assert!(d.stored(PLACE).is_none());
        assert!(d.stored(PLACE2).is_some(), "the live computer is kept");
    });
}

#[test]
fn remove_releases_back_unless_shared() {
    // TS: places-remove.test.ts:509 (released == [] while the live record holds the forward); spec: RM15
    run(async {
        let key = new_key();
        let road_back = json!({ "ssh": SSH, "keyPath": "/home/dev/.ssh/id", "hostKey": "ssh-ed25519 AAAA", "from": "10.0.0.5", "back": { "boxPort": 4640 } });
        let road_plain = json!({ "ssh": SSH, "keyPath": "/home/dev/.ssh/id", "hostKey": "ssh-ed25519 AAAA", "from": "10.0.0.5" });
        // sole record on that login: released, as the login this host held
        let back = Rc::new(Back::default());
        let d = build(&key, Wires { back: Some(back.clone()), ..Wires::default() }, |s| s.records = vec![record_json(json!({ "road": road_back }))]);
        d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(*back.log.borrow(), vec![format!("release {SSH}")]);
        // a second record on the same login keeps it held
        let back = Rc::new(Back::default());
        let d = build(&key, Wires { back: Some(back.clone()), ..Wires::default() }, |s| {
            s.records = vec![record_json(json!({ "road": road_back })), record_json(json!({ "id": PLACE2, "name": "vps", "road": road_back, "joinedAt": "2026-10-03T00:00:00.000Z" }))];
        });
        d.manage.remove(PLACE, ask()).await.unwrap();
        assert!(back.log.borrow().is_empty(), "the other record holds the forward");
        assert!(d.stored(PLACE2).is_some());
        // a record whose road holds no back releases nothing
        let back = Rc::new(Back::default());
        let d = build(&key, Wires { back: Some(back.clone()), ..Wires::default() }, |s| s.records = vec![record_json(json!({ "road": road_plain }))]);
        d.manage.remove(PLACE, ask()).await.unwrap();
        assert!(back.log.borrow().is_empty());
        // another record on another login does not keep it
        let back = Rc::new(Back::default());
        let d = build(&key, Wires { back: Some(back.clone()), ..Wires::default() }, |s| {
            let mut elsewhere = road_back.clone();
            elsewhere["ssh"] = json!("maya@10.0.0.9");
            s.records = vec![record_json(json!({ "road": road_back })), record_json(json!({ "id": PLACE2, "name": "vps", "road": elsewhere, "joinedAt": "2026-10-03T00:00:00.000Z" }))];
        });
        d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(*back.log.borrow(), vec![format!("release {SSH}")]);
    });
}

#[test]
fn remove_refused_leave_over_login_keeps_still_installed() {
    // TS: places-remove.test.ts:331 "keeps the sentence a person has always read where the box will not answer the login, and still lets it go";
    // :641 "lets go of a computer whose login stops answering at the read, running nothing there"; spec: RM9, RM12, RM13
    run(async {
        let key = new_key();
        // the leave's login refusal
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let leave = leaver(&asked, |_| Err(place_login_refused("ssh: connect to host 10.0.0.5 port 22: Connection refused")));
        let d = build(&key, Wires { leave: Some(leave), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |_| {});
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("stillInstalled") }));
        assert_eq!(asked.borrow().len(), 1);
        assert_gone(&d);
        // the read's login refusal: nothing runs, the leave is never asked
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let refusing: Rc<dyn PlaceRunOver> = Rc::new(RunOverRoad(Box::new(|_, _, _, _| Err(place_login_refused("ssh: connect to host 10.0.0.5 port 22: Connection refused"))), RefCell::new(vec![])));
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(refusing), ..Wires::default() }, |_| {});
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert!(asked.borrow().is_empty());
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": w("stillInstalled") }));
        assert_gone(&d);
    });
}

#[test]
fn remove_leave_ran_and_stopped_says_so() {
    // TS: places-remove.test.ts:344 "says the leave ran there and stopped in that computer's own words"; spec: RM12, RM13
    run(async {
        let key = new_key();
        let said = "vps ran the leave and did not finish it: Failed to stop: Unit is masked.";
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { leave: Some(leaver(&asked, move |_| Err(HostError::new(said)))), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |_| {});
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(answer.note.as_deref(), Some(format!("{}; {}", w("loginRoadSaid"), w("stillInstalled")).as_str()));
        assert!(!answer.note.unwrap().contains("did not answer the login"));
        assert_eq!(answer.swept, Vec::<String>::new());
        assert_gone(&d);
    });
}

#[test]
fn remove_unreadable_read_refuses_remove_and_update() {
    // TS: places-remove.test.ts:622 "refuses a remove and an update whose place file read times out or fails, and runs nothing more there"; spec: H3, RM9, U4
    run(async {
        let key = new_key();
        for (exit_code, stderr, said) in [(124, "", "uncheckedTimeout"), (1, "sudo: a password is required\n", "uncheckedSudo")] {
            let asked: Asked = Rc::new(RefCell::new(vec![]));
            let upds: Upds = Rc::new(RefCell::new(vec![]));
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let r = ran.clone();
            let road: Rc<dyn PlaceRunOver> = Rc::new(RunOverRoad(
                Box::new(move |l, s, _, _| {
                    r.borrow_mut().push((l.ssh.clone(), s.to_string(), None));
                    Ok(RunOver { exit_code, stdout: String::new(), stderr: stderr.into() })
                }),
                RefCell::new(vec![]),
            ));
            let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(road), update: Some(updater(&upds, |_| Ok(None))), ..Wires::default() }, |_| {});
            let (said, fix) = halves(said);
            usage(&d.manage.remove(PLACE, ask()).await.expect_err("remove"), &said, &fix);
            usage(&d.manage.update(PLACE, ask()).await.expect_err("update"), &said, &fix);
            assert_eq!(held_only(&ran).len(), 2);
            assert_eq!(ran.borrow().len(), 2);
            assert!(asked.borrow().is_empty() && upds.borrow().is_empty());
            assert!(d.stored(PLACE).is_some());
            assert!(removed_ids(&d).is_empty());
        }
    });
}

// ---------------------------------------------------------------- remove: with a link (RM10-RM13)

const OVER_LINK: [&str; 2] = ["/home/maya/.wsp/place.json", "/home/maya/.wsp/daemon-token"];

fn assert_cut(socket: &FakeSocket, cancelled: &std::cell::Cell<bool>, reach: &FakeReach) {
    assert_eq!(*socket.closed.borrow(), Some((1000, "removed from this host".to_string())));
    assert!(cancelled.get(), "the seen poll is cancelled");
    assert!(reach.closed.get());
}

#[test]
fn remove_linked_place_sweeps_itself() {
    // TS: places-remove.test.ts:25 "asks the linked place to sweep itself, drops the workspace standing on it, and answers what came off"; spec: RM13, RM15
    run(async {
        let o = order();
        let reach = reach_plain(&o, vec!["the systemd user unit", "/home/maya/.wsp/place.json"]);
        let d = build(&new_key(), Wires::default(), |s| s.setup = setup_over(guest(&order(), |_| None)));
        let (socket, cancelled) = link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": ["the systemd user unit", "/home/maya/.wsp/place.json"] }));
        assert_eq!(d.recording.calls(), vec![format!("holdsOn {PLACE} read=true"), format!("projectsOn {PLACE}"), format!("dropOn {PLACE}")]);
        let asked = reach.asked.borrow();
        assert_eq!(asked.last().unwrap(), &("place.leave".to_string(), json!({ "projects": [] })));
        drop(asked);
        assert_cut(&socket, &cancelled, &reach);
        assert_gone(&d);
    });
}

#[test]
fn remove_over_login_with_link_leaves_over_login_not_link() {
    // TS: places-remove.test.ts:403 "takes that road on a computer that is holding a link too"; spec: RM12, RM15
    run(async {
        let key = new_key();
        let o = order();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        const TOOK_LINES: [&str; 2] = ["systemd system unit wsp-place-1234abcd.service (stopped)", "/home/maya/.wsp/place.json"];
        let took_lines = TOOK_LINES;
        let reach = reach_plain(&o, OVER_LINK.to_vec());
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&TOOK_LINES))), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |s| s.setup = setup_over(guest(&o, |_| None)));
        let (socket, cancelled) = link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(asked.borrow().len(), 1);
        assert_eq!(asked.borrow()[0].ssh, login());
        assert_eq!(reach.ops(), Vec::<String>::new(), "the agent on the link is never asked: a second sweep after the box's own leave would be a second copy");
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": took_lines, "note": w("sweptOverSsh") }));
        assert_cut(&socket, &cancelled, &reach);
        assert_gone(&d);
    });
}

#[test]
fn remove_over_login_with_link_takes_runtime_forced() {
    // spec: RM10, RM11, RM12; TS: places-setup.test.ts:598 (projects under the runtime's folder go with the leave)
    run(async {
        let key = new_key();
        let cases: [(Option<bool>, bool, bool, Option<bool>); 5] = [
            // (report takesRuntime, linked at the read, ask.force, the leave's force)
            (Some(true), true, false, Some(true)),
            (Some(false), true, false, Some(true)),
            (None, true, false, None),
            (Some(true), false, false, None),
            (None, false, true, Some(true)),
        ];
        for (takes, linked, ask_force, want) in cases {
            let o = order();
            let asked: Asked = Rc::new(RefCell::new(vec![]));
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let reach = reach_plain(&o, vec![]);
            let rep = match takes {
                Some(t) => report_json(json!({ "takesRuntime": t })),
                None => report_json(json!({})),
            };
            let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |s| {
                s.records = vec![record_json(json!({ "report": rep }))];
                s.setup = setup_over(guest(&o, |_| None));
                *s.recording.projects.borrow_mut() = vec![project_on("pr_1", "app", Some("/wsp/projects/app/checkout")), project_on("pr_2", "mine", Some("/home/maya/mine")), project_on("pr_3", "bare", None), project_on("pr_4", "deep", Some("/wsp/projects/a/b/checkout"))];
            });
            if linked {
                link(&d, PLACE, reach.clone());
            }
            let ask = if ask_force { force() } else { ask() };
            d.manage.remove(PLACE, ask).await.unwrap();
            let req = asked.borrow()[0].clone();
            assert_eq!(req.force, want, "takes {takes:?} linked {linked} ask.force {ask_force}");
            assert_eq!(req.projects, Some(vec!["app".to_string()]), "only the checkouts under /wsp/projects, by their one name");
            let listings = o.lock().unwrap().iter().filter(|s| *s == "reach:exec").count();
            assert_eq!(listings, usize::from(linked && takes == Some(true)), "the listing of an older wsp's checkouts is read only where the report says the leave takes the runtime whole");
        }
    });
}

#[test]
fn remove_leave_over_login_failure_then_link_sweeps() {
    // TS: places-remove.test.ts:421 "falls back to the sweep on the link where that login will not answer";
    // :455 "takes the link road at once where the login does not answer the probe"; :482 "carries that computer's own words into the note";
    // spec: RM7, RM12, RM13
    run(async {
        let key = new_key();
        let said = "vps ran the leave and did not finish it: Failed to stop: Unit is masked.";
        // (what, leave answers, dial answers, the note, the leave road asked, the probe asked)
        let cases: Vec<(&str, Option<HostError>, Option<&str>, String, usize, usize)> = vec![
            ("login refusal", Some(place_login_refused("ssh: connect to host 10.0.0.5 port 22: Connection refused")), None, w("sweptOverLink"), 1, 0),
            ("leave said something", Some(HostError::new(said)), None, w("sweptOverLinkSaid"), 1, 0),
            ("probe fails", None, Some("ssh: connect to host 10.0.0.5 port 22: Operation timed out"), w("sweptOverLink"), 0, 1),
        ];
        for (what, leave_err, dial_err, note, leave_asked, dialled) in cases {
            let o = order();
            let asked: Asked = Rc::new(RefCell::new(vec![]));
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let probed = Rc::new(RefCell::new(Vec::<PlaceLogin>::new()));
            let dial: Option<Rc<dyn PlaceDialler>> = dial_err.map(|e| {
                let probed = probed.clone();
                Rc::new(Dial(Box::new(move |l: &PlaceLogin| {
                    probed.borrow_mut().push(l.clone());
                    Err(HostError::new(e))
                }))) as Rc<dyn PlaceDialler>
            });
            let leave = leaver(&asked, move |_| leave_err.clone().map_or(Ok(vec![]), Err));
            let reach = reach_plain(&o, OVER_LINK.to_vec());
            let d = build(&key, Wires { leave: Some(leave), run_over: Some(same_box(&key, &ran)), dial, ..Wires::default() }, |s| s.setup = setup_over(guest(&o, |_| None)));
            let (socket, cancelled) = link(&d, PLACE, reach.clone());
            let answer = d.manage.remove(PLACE, ask()).await.unwrap();
            assert_eq!(asked.borrow().len(), leave_asked, "{what}");
            assert_eq!(*probed.borrow(), if dialled == 1 { vec![login()] } else { vec![] }, "{what}");
            assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": OVER_LINK, "note": note }), "{what}");
            let asked_over_link = reach.asked.borrow();
            assert_eq!(asked_over_link.iter().filter(|(op, _)| op == "place.leave").count(), 1, "{what}");
            drop(asked_over_link);
            assert_cut(&socket, &cancelled, &reach);
            assert_gone(&d);
        }
    });
}

#[test]
fn remove_leave_drops_link_mid_leave() {
    // TS: places-remove.test.ts:433 "lets go of a computer whose own leave dropped the link under it"; spec: RM12, RM15
    run(async {
        let key = new_key();
        let o = order();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let holder: Rc<RefCell<Option<Rc<PlaceDoorContext>>>> = Rc::new(RefCell::new(None));
        let leave = {
            let holder = holder.clone();
            leaver(&asked, move |_| {
                // the stop on that unit: the daemon dies and the link goes while the answer is on its way
                holder.borrow().as_ref().unwrap().live.borrow_mut().shift_remove(PLACE);
                Ok(vec!["systemd system unit wsp-place-1234abcd.service (stopped)".into()])
            })
        };
        let reach = reach_plain(&o, OVER_LINK.to_vec());
        let d = build(&key, Wires { leave: Some(leave), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |s| s.setup = setup_over(guest(&o, |_| None)));
        *holder.borrow_mut() = Some(d.ctx.clone());
        link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": ["systemd system unit wsp-place-1234abcd.service (stopped)"], "note": w("sweptOverSsh") }));
        assert!(reach.ops().is_empty());
        assert_gone(&d);
        assert!(d.store.writes_to(&format!("put places/{PLACE}")).is_empty(), "the record goes and stays gone: nothing wrote it back");
    });
}

#[test]
fn remove_linked_login_elsewhere_sweeps_over_link() {
    // TS: places-remove.test.ts:580 "sweeps a linked computer over its link where its login reaches another machine, and runs nothing over the login"; spec: RM9, RM13
    run(async {
        let key = new_key();
        let o = order();
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let reach = reach_plain(&o, OVER_LINK.to_vec());
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(box_road(|| None, None, &ran)), ..Wires::default() }, |s| s.setup = setup_over(guest(&o, |_| None)));
        link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert!(asked.borrow().is_empty());
        assert_eq!(ran.borrow().iter().map(|r| r.1.clone()).collect::<Vec<_>>(), vec![word("/scripts/held")]);
        assert!(reach.ops().contains(&"place.leave".to_string()));
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": OVER_LINK, "note": w("elsewhereSweptOverLink") }));
        assert_gone(&d);
    });
}

fn codex_reads() -> Vec<String> {
    vec![format!("wsp-server\t{}\tagents/mcp/codex/context7", word("/unmerge/codexDigest")), format!("wsp-mcp 0 0 {}", b64(&word("/unmerge/codexText")))]
}

fn wrote(o: &Order, file: &str) -> Option<usize> {
    at_order(o, |c| c.starts_with("link:") && c.contains(&format!("f='{file}'")) && c.contains("mv -f \"$n\" \"$r\""))
}

#[test]
fn remove_servers_taken_out_of_agent_files_before_leave() {
    // TS: places-remove.test.ts:37 "takes the servers wsp merged into the agents' own files there back out before it asks that computer to leave"; spec: RM13 (V7)
    run(async {
        let o = order();
        let config = word("/unmerge/codexConfig");
        let reach = reach_plain(&o, vec!["/home/maya/.wsp"]);
        let d = build(&new_key(), Wires::default(), |s| s.setup = setup_over(unmerging(&o, codex_reads())));
        link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(answer.swept, vec![format!("context7 (out of {config})"), "/home/maya/.wsp".to_string()]);
        assert_eq!(answer.note, None);
        let wrote_at = wrote(&o, &config).unwrap_or_else(|| panic!("the file is written back: {:?}", o.lock().unwrap()));
        let leave_at = at_order(&o, |c| c == "reach:place.leave").unwrap();
        assert!(wrote_at < leave_at, "the file is written back over the link before the leave takes the folder holding the list");
        assert_gone(&d);
    });
}

#[test]
fn remove_unread_agent_file_named_in_note() {
    // TS: places-remove.test.ts:76 "names an agent's file there it left unread in the note, not among what came off, and writes nothing over it"; spec: RM13
    run(async {
        let o = order();
        let reads = vec![format!("wsp-server\t{}\tagents/mcp/codex/context7", "0".repeat(64)), "wsp-mcp 0 0 -plain".to_string()];
        let reach = reach_plain(&o, vec!["/home/maya/.wsp"]);
        let d = build(&new_key(), Wires::default(), |s| s.setup = setup_over(unmerging(&o, reads)));
        link(&d, PLACE, reach);
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(answer.swept, vec!["/home/maya/.wsp".to_string()]);
        assert_eq!(answer.note.as_deref(), Some(w("serversUnread").as_str()));
        assert!(at_order(&o, |c| c.contains("mv -f \"$n\" \"$r\"")).is_none(), "nothing is written over it");
    });
}

#[test]
fn remove_link_sweep_lines_in_order() {
    // spec: RM13 (swept = the servers' lines, the cgroup script's non-empty lines, then the answer's), RM14
    run(async {
        let o = order();
        let config = word("/unmerge/codexConfig");
        let reach = reach_with(&o, vec!["/home/maya/.wsp", "/home/maya/.wsp/daemon-token"], |op, cmd| {
            (op == "exec" && cmd.starts_with("wsp_end()")).then(|| Ok(exec_out(0, "/sys/fs/cgroup/wsp/threads/t1\n\n/sys/fs/cgroup/wsp/threads/t2\n", "")))
        });
        let d = build(&new_key(), Wires::default(), |s| s.setup = setup_over(unmerging(&o, codex_reads())));
        link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, force()).await.unwrap();
        assert_eq!(
            answer.swept,
            vec![format!("context7 (out of {config})"), "/sys/fs/cgroup/wsp/threads/t1".into(), "/sys/fs/cgroup/wsp/threads/t2".into(), "/home/maya/.wsp".into(), "/home/maya/.wsp/daemon-token".into()]
        );
        let (w_at, c_at, l_at) = (wrote(&o, &config).unwrap(), at_order(&o, |c| c == "reach:cgroups").unwrap(), at_order(&o, |c| c == "reach:place.leave").unwrap());
        assert!(w_at < c_at && c_at < l_at, "{:?}", o.lock().unwrap());
        let asked = reach.asked.borrow();
        let cgroups = asked.iter().find(|(_, p)| p["cmd"] == json!(word("/scripts/cgroupsEnd"))).expect("the threads' cgroups are ended");
        assert_eq!(cgroups.1["timeoutMs"], json!(60_000));
        assert_eq!(asked.last().unwrap(), &("place.leave".to_string(), json!({ "force": true, "projects": [] })));
    });
}

#[test]
fn remove_link_sweep_survives_a_cgroup_script_failure() {
    // spec: RM13 (the cgroup script's failure is ignored)
    run(async {
        let o = order();
        let reach = reach_with(&o, vec!["/home/maya/.wsp"], |op, cmd| (op == "exec" && cmd.starts_with("wsp_end()")).then(|| Err(HostError::new("exec timed out"))));
        let d = build(&new_key(), Wires::default(), |s| s.setup = setup_over(guest(&o, |_| None)));
        link(&d, PLACE, reach);
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": ["/home/maya/.wsp"] }));
        assert_gone(&d);
    });
}

#[test]
fn remove_link_sweep_failure_note() {
    // spec: RM13 (a failure of the sweep over the link; `before` is the away sentence or the login road line)
    run(async {
        let key = new_key();
        let failed = format!("{NAME} was connected but did not finish the sweep: socket reset; run wsp leave on that computer");
        // no login at all
        let o = order();
        let reach = reach_with(&o, vec![], |op, _| (op == "place.leave").then(|| Err(HostError::new("socket reset"))));
        let d = build(&key, Wires::default(), |s| s.setup = setup_over(guest(&o, |_| None)));
        link(&d, PLACE, reach);
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": [], "note": failed }));
        assert_gone(&d);
        // the leave over the login ran and stopped, then the link failed too
        let o = order();
        let said = "vps ran the leave and did not finish it: Failed to stop: Unit is masked.";
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let reach = reach_with(&o, vec![], |op, _| (op == "place.leave").then(|| Err(HostError::new("socket reset"))));
        let d = build(&key, Wires { leave: Some(leaver(&asked, move |_| Err(HostError::new(said)))), run_over: Some(same_box(&key, &ran)), ..Wires::default() }, |s| s.setup = setup_over(guest(&o, |_| None)));
        link(&d, PLACE, reach);
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(answer.note.as_deref(), Some(format!("{}, and {failed}", w("loginRoadSaid")).as_str()));
        assert_eq!(answer.swept, Vec::<String>::new());
        assert_gone(&d);
        // the login reaches another machine, then the link failed
        let o = order();
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let reach = reach_with(&o, vec![], |op, _| (op == "place.leave").then(|| Err(HostError::new("socket reset"))));
        let d = build(&key, Wires { leave: Some(leaver(&asked, leave_answer(&[]))), run_over: Some(box_road(|| None, None, &ran)), ..Wires::default() }, |s| s.setup = setup_over(guest(&o, |_| None)));
        link(&d, PLACE, reach);
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(answer.note.as_deref(), Some(format!("{}, so nothing was changed on the machine it reaches, and {failed}", w("away")).as_str()));
        assert!(asked.borrow().is_empty());
        assert_gone(&d);
    });
}

#[test]
fn remove_blocked_computer_sweeps_over_link() {
    // TS: places-unable.test.ts:32 "takes the computer out over the link, asking it to sweep itself"; spec: RM13
    run(async {
        let o = order();
        let blocked = report_json(json!({ "name": "srv", "runsWorkspaces": false, "workspacesBlocked": "this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1" }));
        let reach = reach_plain(&o, vec!["/root/.wsp/place.json"]);
        let d = build(&new_key(), Wires::default(), |s| {
            s.records = vec![record_json(json!({ "name": "srv", "report": blocked }))];
            s.setup = setup_over(guest(&o, |_| None));
        });
        link(&d, PLACE, reach.clone());
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert!(answer.removed);
        assert_eq!(reach.ops().iter().filter(|op| *op == "place.leave").count(), 1);
        assert_eq!(answer.swept, vec!["/root/.wsp/place.json".to_string()]);
        assert_gone(&d);
    });
}

// ---------------------------------------------------------------- remove: servers and files out of a box's stores

struct FolderLinks {
    link: Arc<dyn MachineLink>,
    home: String,
}

#[async_trait(?Send)]
impl LinkReads for FolderLinks {
    fn backend_of(&self, _: &str) -> Option<Arc<dyn MachineBackend>> {
        None
    }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> {
        Err(HostError::new(format!("{place_id} has not said what it forks with")))
    }
    fn folder_computer(&self, _: &str) -> Option<FolderComputer> {
        Some(FolderComputer { machine: PlaceFolderMachine::new(self.link.clone(), NAME, &self.home, None), home: self.home.clone(), cpu: 2, mem_mb: 4096, tools: true })
    }
}

/// The door's manage area again, over a views area that reaches the computer as a folder machine on `link`.
fn manage_over_folders(d: &Door, link: Arc<FakeMachineLink>, home: &str) -> Rc<ManageDoor> {
    let links: Rc<dyn LinkReads> = Rc::new(FolderLinks { link: link as Arc<dyn MachineLink>, home: home.to_string() });
    let views = PlaceViewsArea::new(d.ctx.clone(), d.records.clone(), d.setup.clone(), links.clone());
    ManageDoor::new(d.ctx.clone(), d.records.clone(), d.setup.clone(), views, links)
}

fn login_report(home: &str) -> Value {
    report_json(json!({ "login": { "HOME": home, "USER": "root", "PATH": "/usr/bin" } }))
}

fn stores(home: &str) -> IndexMap<String, String> {
    IndexMap::from([("claude".to_string(), format!("{home}/.claude-cfg")), ("codex".to_string(), "/var/lib/wsp/logins/codex".to_string())])
}

#[test]
fn remove_servers_out_of_box_logins_as_root_and_maya() {
    // TS: places-remove.test.ts:103 (two cases: root, maya) "takes wsp's servers out of the files a box's threads read on a computer joined as <user>, its logins folder only where root reaches it"; spec: RM13 (V7, V8)
    run(async {
        for login in [("root", "/root", "/var/lib/wsp/logins/codex/config.toml"), ("maya", "/home/maya", "/home/maya/.codex/config.toml")] {
            let (user, home, codex_file) = login;
            let o = order();
            let claude = word("/unmerge/claudeText");
            let sha_claude = word("/unmerge/claudeDigest");
            let sha_codex = word("/unmerge/codexOnlyDigest");
            let keys = [format!("wsp-server\t{sha_claude}\tagents/mcp/claude/gsc"), format!("wsp-server\t{sha_codex}\tagents/mcp/codex/context7")].join("\n");
            let reads = Mutex::new(VecDeque::from(vec![keys, format!("wsp-mcp 0 0 {}", b64(&claude)), format!("wsp-mcp 0 0 {}", b64(&word("/unmerge/codexOnlyText")))]));
            let who = format!("Linux\n0\nroot\n{user}\n1\n{home}\n/usr/bin\n");
            let g = guest(&o, move |cmd| {
                if cmd.contains("command -v runuser") {
                    Some((0, who.clone()))
                } else if cmd.contains("WSP_LAUNCHED") {
                    Some((0, "WSP_LAUNCHED\n".to_string()))
                } else if cmd.contains("WSP_POLL") {
                    Some((0, polled(&reads.lock().unwrap().pop_front().unwrap_or_default())))
                } else {
                    None
                }
            });
            let d = build(&new_key(), Wires::default(), |s| {
                s.records = vec![record_json(json!({ "report": login_report(home) }))];
                s.setup = setup_over(g.clone());
                *s.recording.stores.borrow_mut() = Some(stores(home));
            });
            let manage = manage_over_folders(&d, g, home);
            link(&d, PLACE, reach_plain(&o, vec![]));
            let answer = manage.remove(PLACE, ask()).await.unwrap();
            assert_eq!(answer.swept, vec![format!("gsc (out of {home}/.claude-cfg/.claude.json)"), format!("context7 (out of {codex_file})")], "{user}");
            for file in [format!("{home}/.claude-cfg/.claude.json"), codex_file.to_string()] {
                assert!(wrote(&o, &file).is_some(), "{user}: {file} is written back: {:?}", o.lock().unwrap());
            }
        }
    });
}

#[test]
fn remove_store_files_off_before_leave() {
    // TS: places-remove.test.ts:136 "takes the files wsp landed in a box's Codex store off over the link before it asks that computer to leave"; spec: RM11 (V9)
    run(async {
        let o = order();
        let g = guest(&o, |cmd| {
            if cmd.contains("command -v runuser") {
                Some((0, "Linux\n0\nroot\nroot\n1\n/root\n/usr/bin\n".to_string()))
            } else if cmd.contains("wsp-unland") {
                Some((0, "wsp-unland\tgone\t.codex/AGENTS.md\n".to_string()))
            } else {
                None
            }
        });
        let d = build(&new_key(), Wires::default(), |s| {
            s.records = vec![record_json(json!({ "report": login_report("/root") }))];
            s.setup = setup_over(g.clone());
            *s.recording.stores.borrow_mut() = Some(stores("/root"));
        });
        let manage = manage_over_folders(&d, g, "/root");
        link(&d, PLACE, reach_plain(&o, vec!["/root/.wsp"]));
        let answer = manage.remove(PLACE, ask()).await.unwrap();
        let off = at_order(&o, |c| c.starts_with("link:") && c.contains("wsp-unland")).unwrap_or_else(|| panic!("{:?}", o.lock().unwrap()));
        assert!(o.lock().unwrap()[off].contains("('.codex'/*) root='/var/lib/wsp/logins/codex'"));
        assert!(off < at_order(&o, |c| c == "reach:place.leave").unwrap());
        assert_eq!(answer.swept, vec!["/var/lib/wsp/logins/codex/AGENTS.md".to_string(), "/root/.wsp".to_string()], "the store files come before the leave's own lines");
    });
}

// ---------------------------------------------------------------- remove: plugins (RM11, RM14)

fn plugin_plan(keys: &[(&str, Option<&str>)]) -> Vec<PlaceUndo> {
    keys.iter().map(|(name, cmd)| PlaceUndo { key: format!("plugins/{name}"), label: name.to_string(), ids: vec![format!("plugins/{name}")], owner: Some(format!("plugins/{name}")), cmd: cmd.map(String::from), ..PlaceUndo::default() }).collect()
}

fn provisioner(plan: Vec<PlaceUndo>) -> Rc<Provision> {
    let p = Provision::default();
    *p.undos.borrow_mut() = Some(plan);
    Rc::new(p)
}

#[test]
fn remove_plugins_lines_first_and_kept_note() {
    // TS: places-setup.test.ts:499 "takes the plugins wsp put on off before the sweep"; :568 "names a plugin the link could not take off rather than saying nothing of it"; spec: RM11, RM14
    run(async {
        let o = order();
        let provision = provisioner(plugin_plan(&[("lint", Some("take-off-lint")), ("stuck", Some("take-off-stuck"))]));
        let g = guest(&o, |cmd| cmd.contains("take-off-stuck").then(|| (1, String::new())));
        let reach = reach_plain(&o, vec!["/home/maya/.wsp"]);
        let d = build(&new_key(), Wires { provision: Some(provision.clone()), ..Wires::default() }, |s| {
            s.records = vec![record_json(record_with_plugins(json!({})))];
            s.setup = setup_over(g);
        });
        link(&d, PLACE, reach);
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        // only the plugins wsp put on, before the sweep takes the folder its agent may run from; never the one it had
        assert_eq!(*provision.log.borrow(), vec![format!("undo {HOME} plugins/lint,plugins/stuck")]);
        assert_eq!(js(&answer), json!({ "removed": true, "took": took(json!([]), json!([])), "swept": ["plugin lint", "/home/maya/.wsp"], "note": w("pluginsKeptStuck") }));
        let (lint, stuck, leave) = (at_order(&o, |c| c.contains("take-off-lint")).unwrap(), at_order(&o, |c| c.contains("take-off-stuck")).unwrap(), at_order(&o, |c| c == "reach:place.leave").unwrap());
        assert!(lint < stuck && stuck < leave, "{:?}", o.lock().unwrap());
        assert_gone(&d);
    });
}

#[test]
fn remove_plugins_off_over_login_when_link_down() {
    // TS: places-setup.test.ts:525 "takes the plugins off over the login where the link is down, and names each one it could not take off"; spec: RM11, RM14
    run(async {
        let key = new_key();
        let provision = provisioner(plugin_plan(&[("lint", Some("take-off-lint")), ("stuck", Some("take-off-stuck"))]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let held = word("/scripts/held");
        let k = key.public_key.clone();
        let r = ran.clone();
        let road: Rc<dyn PlaceRunOver> = Rc::new(RunOverRoad(
            Box::new(move |l, s, _, pw| {
                r.borrow_mut().push((l.ssh.clone(), s.to_string(), pw.map(String::from)));
                if s == held {
                    return Ok(RunOver { exit_code: 0, stdout: held_stdout(Some(place_file(PLACE, &k)), None), stderr: String::new() });
                }
                Ok(if s.contains("stuck") { RunOver { exit_code: 1, stdout: String::new(), stderr: "Plugin is in use".into() } } else { RunOver::default() })
            }),
            RefCell::new(vec![]),
        ));
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let leave = {
            let r = ran.clone();
            leaver(&asked, move |_| {
                r.borrow_mut().push(("leave".into(), "leave".into(), None));
                Ok(vec!["/opt/wsp".into()])
            })
        };
        let sudo = sudo_road(|_, _, _, _| Ok("taken".into()));
        let d = build(&key, Wires { provision: Some(provision.clone()), run_over: Some(road), leave: Some(leave), sudo: Some(sudo), ..Wires::default() }, |s| {
            s.records = vec![record_json(record_with_plugins(json!({})))];
        });
        let answer = d.manage.remove(PLACE, sudo_pw("right")).await.unwrap();
        let scripts: Vec<String> = ran.borrow().iter().map(|r| r.1.clone()).collect();
        assert_eq!(scripts, vec![word("/scripts/held"), "take-off-lint".to_string(), "take-off-stuck".to_string(), "leave".to_string()]);
        assert!(ran.borrow()[1..3].iter().all(|r| r.0 == SSH && r.2.as_deref() == Some("right")), "over the login the install used, with the password sudo took");
        assert_eq!(answer.swept, vec!["plugin lint".to_string(), "/opt/wsp".to_string()]);
        assert_eq!(answer.note.as_deref(), Some(format!("{}; {}", w("sweptOverSsh"), w("pluginsKeptStuck")).as_str()));
        assert_gone(&d);
    });
}

#[test]
fn remove_plugins_without_a_plan_are_kept_and_nothing_runs() {
    // spec: RM11, RM14 (a plugin the plan holds no command for is named, none runs); V10: a login that does not stand runs nothing
    run(async {
        let key = new_key();
        let provision = provisioner(plugin_plan(&[]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let asked: Asked = Rc::new(RefCell::new(vec![]));
        let d = build(&key, Wires { provision: Some(provision), run_over: Some(box_road(|| None, None, &ran)), leave: Some(leaver(&asked, leave_answer(&[]))), ..Wires::default() }, |s| {
            s.records = vec![record_json(record_with_plugins(json!({})))];
        });
        let answer = d.manage.remove(PLACE, ask()).await.unwrap();
        assert_eq!(ran.borrow().len(), 1, "only the place file was read: the login reaches another machine, so no plugin command runs there");
        assert_eq!(answer.swept, Vec::<String>::new());
        assert_eq!(answer.note.as_deref(), Some(format!("{}; {}", w("elsewhereRemoved"), w("pluginsKeptTwo")).as_str()));
        assert!(asked.borrow().is_empty());
    });
}

// ---------------------------------------------------------------- update (U1-U6)

struct Upd {
    place_id: String,
    name: String,
    daemon: bool,
    link: bool,
    ssh: Option<PlaceLogin>,
    sudo_password: Option<String>,
    report: wsp_wire::wire::place_link::PlaceReport,
}

type Upds = Rc<RefCell<Vec<Upd>>>;

fn updater(upds: &Upds, answer: impl Fn(&PlaceUpdateRequest) -> PlaceResult<Option<PlaceUpdateLanded>> + 'static) -> Rc<dyn PlaceUpdater> {
    let upds = upds.clone();
    Rc::new(Update(Box::new(move |req| {
        upds.borrow_mut().push(Upd { place_id: req.place_id.clone(), name: req.name.clone(), daemon: req.daemon, link: req.link.is_some(), ssh: req.ssh.clone(), sudo_password: req.sudo_password.clone(), report: req.report.clone() });
        answer(&req)
    })))
}

fn landed() -> PlaceUpdateLanded {
    PlaceUpdateLanded { road: UpdateRoad::Link, at: "/home/maya/.wsp/daemon/wsp-daemon".into(), kept: Some("/home/maya/.wsp/daemon/wsp-daemon.old".into()) }
}

fn behind_record(over: Value) -> Value {
    let mut r = record_json(json!({ "report": report_json(json!({ "daemonVersion": daemon_version() - 1 })) }));
    merge(&mut r, over);
    r
}

#[test]
fn update_unknown_refuses() {
    // TS: oracle update.unknown; places-update.test.ts:172 "refuses one this host does not hold"; spec: U1
    run(async {
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(None))), ..Wires::default() }, |_| {});
        let e = d.manage.update("p_nope", ask()).await.expect_err("no such place");
        assert_eq!(err_json(&e), at("/update/unknown"));
        assert_eq!(e.message, w("noSuchPlace"));
        assert_eq!(e.kind, None);
        assert!(upds.borrow().is_empty() && d.setup.calls().is_empty());
    });
}

#[test]
fn update_refused_while_setup_or_sync() {
    // spec: U2
    run(async {
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(None))), ..Wires::default() }, |s| {
            s.setup.setting.borrow_mut().insert(PLACE.into());
            *s.setup.busy.borrow_mut() = Some("old-macbook is being set up; wait for it to finish".into());
        });
        let e = d.manage.update(PLACE, ask()).await.expect_err("busy with a setup");
        assert_eq!((e.message.as_str(), e.kind.clone()), ("old-macbook is being set up; wait for it to finish", Some(Kind::Conflict)));
        d.setup.setting.borrow_mut().clear();
        // a busy sentence for a computer not setting is not read
        d.setup.syncing.borrow_mut().insert(PLACE.into());
        let e = d.manage.update(PLACE, ask()).await.expect_err("syncing");
        assert_eq!((e.message.as_str(), e.kind.clone()), (w("syncing").as_str(), Some(Kind::Conflict)));
        assert!(upds.borrow().is_empty(), "nothing is sent");
        assert!(!d.setup.calls().iter().any(|c| c.starts_with("syncSoon")));
        // setting with no sentence is no refusal
        d.setup.syncing.borrow_mut().clear();
        d.setup.setting.borrow_mut().insert(PLACE.into());
        *d.setup.busy.borrow_mut() = None;
        d.manage.update(PLACE, ask()).await.unwrap();
        assert_eq!(upds.borrow().len(), 1);
    });
}

#[test]
fn update_behind_without_updater_refuses() {
    // TS: oracle update.behind; places-update.test.ts:216 "says so plainly on a host wired with no road to put a daemon on a computer"; spec: U3
    run(async {
        let d = build(&new_key(), Wires::default(), |s| s.records = vec![behind_record(json!({}))]);
        let e = d.manage.update(PLACE, ask()).await.expect_err("behind");
        assert_eq!(err_json(&e), at("/update/behind"));
        assert_eq!(e.message, NO_PLACE_UPDATER);
        assert_eq!(e.message, w("noPlaceUpdater"));
        assert!(d.setup.calls().is_empty(), "the recipe is not started behind a refusal");
    });
}

#[test]
fn update_current_without_updater_answers_name() {
    // TS: oracle update.current; spec: U3, U6
    run(async {
        let d = build(&new_key(), Wires::default(), |_| {});
        assert_eq!(outcome(&d.manage.update(PLACE, ask()).await), at("/update/current"));
        assert_eq!(d.setup.calls(), vec![format!("syncSoon {PLACE} 0")], "the record follows the recipe web");
    });
}

#[test]
fn update_follower_syncs_soon() {
    // TS: places-setup.test.ts:614 "asks the updater every time ... and never runs the setup again"; spec: U6
    run(async {
        for (recipe, syncs) in [(Some("web"), true), (Some("none"), false), (None, false)] {
            let upds: Upds = Rc::new(RefCell::new(vec![]));
            let rec = match recipe {
                Some(r) => record_json(json!({ "recipe": r })),
                None => record_json(json!({ "recipe": null })),
            };
            let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(None))), ..Wires::default() }, |s| s.records = vec![rec]);
            let answer = d.manage.update(PLACE, ask()).await.unwrap();
            assert_eq!(js(&answer), json!({ "name": NAME }), "{recipe:?}");
            assert_eq!(d.setup.calls(), if syncs { vec![format!("syncSoon {PLACE} 0")] } else { vec![] }, "{recipe:?}");
            assert!(!d.setup.calls().iter().any(|c| c.starts_with("startSetup")), "an update never runs the setup again");
        }
    });
}

#[test]
fn update_asks_updater_every_time_current_computer() {
    // TS: places-update.test.ts:172 "picks up no byte for a place already running this daemon"; places-setup.test.ts:614; spec: U3, U5
    run(async {
        let o = order();
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(None))), ..Wires::default() }, |_| {});
        link(&d, PLACE, reach_plain(&o, vec![]));
        let answer = d.manage.update(PLACE, ask()).await.unwrap();
        assert_eq!(js(&answer), json!({ "name": NAME }));
        assert_eq!(upds.borrow().iter().map(|u| u.daemon).collect::<Vec<_>>(), vec![false], "asked all the same, with no binary");
        assert_eq!(d.links.asked.borrow().len(), 0);
    });
}

#[test]
fn update_updater_refusal_for_away_computer_passes_through() {
    // TS: places-update.test.ts:192 "hands back the updater's own refusal for a computer that is away, on an update that carries no binary"; spec: U5
    run(async {
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let away = "srv is not connected and this wsp has no login for it";
        let d = build(&new_key(), Wires { update: Some(updater(&upds, move |req| if req.link.is_none() && req.ssh.is_none() { Err(HostError::new(away)) } else { Ok(None) })), ..Wires::default() }, |s| {
            s.records = vec![record_json(json!({ "name": "srv", "road": null }))];
        });
        let e = d.manage.update(PLACE, ask()).await.expect_err("away");
        assert_eq!(e.message, away);
        assert_eq!(upds.borrow().iter().map(|u| u.daemon).collect::<Vec<_>>(), vec![false]);
        assert!(!d.setup.calls().iter().any(|c| c.starts_with("syncSoon")), "the recipe is not started behind it");
    });
}

#[test]
fn update_over_login_reads_place_file_first() {
    // TS: places-remove.test.ts:316 "asks for the sudo password before an update over the login, and rides it on the update once sudo took it"; :603; spec: U4, U5
    run(async {
        let key = new_key();
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let marks = ran.clone();
        let sudo = sudo_road(move |l, pw, verb, name| {
            marks.borrow_mut().push((l.ssh.clone(), format!("sudo {verb:?} {name}"), pw.map(String::from)));
            if pw != Some("right") {
                return Err(sudo_kind_error("maya@10.0.0.5 runs sudo only with maya's password. Type it at wsp add old-macbook --update in a terminal."));
            }
            Ok("taken".into())
        });
        let mark = ran.clone();
        let up = updater(&upds, move |_| {
            mark.borrow_mut().push((String::new(), "update".into(), None));
            Ok(None)
        });
        let d = build(&key, Wires { update: Some(up), run_over: Some(same_box(&key, &ran)), sudo: Some(sudo), ..Wires::default() }, |_| {});
        let e = d.manage.update(PLACE, ask()).await.expect_err("sudo wants a password");
        assert!(e.message.contains("wsp add old-macbook --update"));
        assert_eq!(e.kind, Some(Kind::parse("sudo")));
        assert!(upds.borrow().is_empty());
        let answer = d.manage.update(PLACE, sudo_pw("right")).await.unwrap();
        assert_eq!(js(&answer), json!({ "name": NAME }));
        let steps: Vec<String> = ran.borrow().iter().map(|r| if r.1 == word("/scripts/held") { "read".to_string() } else { r.1.clone() }).collect();
        assert_eq!(steps, vec!["sudo Update old-macbook", "sudo Update old-macbook", "read", "update"], "the password is asked for first, then the place file is read, then the binary goes");
        assert_eq!(ran.borrow()[2].2.as_deref(), Some("right"), "the place file is read with the password sudo took");
        let u = &upds.borrow()[0];
        assert_eq!((u.place_id.as_str(), u.name.as_str(), u.daemon, u.link), (PLACE, NAME, false, false));
        assert_eq!(u.ssh, Some(login()));
        assert_eq!(u.sudo_password.as_deref(), Some("right"));
        assert_eq!(u.report, record(json!({})).report);
    });
}

#[test]
fn update_link_road_passes_link_and_login() {
    // TS: places-update.test.ts:86 "puts the binary over the link as bytes"; spec: U4 (with a link nothing is read over the login), U5
    run(async {
        let key = new_key();
        let o = order();
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let ran: Ran = Rc::new(RefCell::new(vec![]));
        let sudo = sudo_road(|_, _, _, _| panic!("a link needs no password"));
        let d = build(&key, Wires { update: Some(updater(&upds, |_| Ok(None))), run_over: Some(same_box(&key, &ran)), sudo: Some(sudo), ..Wires::default() }, |_| {});
        link(&d, PLACE, reach_plain(&o, vec![]));
        d.manage.update(PLACE, ask()).await.unwrap();
        assert!(ran.borrow().is_empty());
        let u = &upds.borrow()[0];
        assert!(u.link);
        assert_eq!(u.ssh, Some(login()), "the login rides with the link, as the record carries it");
        assert_eq!(u.sudo_password, None);
        assert_eq!(u.report, record(json!({})).report);
    });
}

#[test]
fn update_login_elsewhere_refused() {
    // TS: places-remove.test.ts:593 "refuses an update over a login that reaches another machine before a byte goes there"; :509 (the named record); spec: U4
    run(async {
        let key = new_key();
        for (what, other, file, said) in [("another machine", false, Some(place_file("p_0123456789abcdef", &key.public_key)), "otherRefusal"), ("no place file", false, None, "otherRefusal"), ("a stale record beside the live one", true, Some(place_file(PLACE2, &key.public_key)), "otherRefusalNamed")] {
            let upds: Upds = Rc::new(RefCell::new(vec![]));
            let ran: Ran = Rc::new(RefCell::new(vec![]));
            let d = build(&key, Wires { update: Some(updater(&upds, |_| Ok(None))), run_over: Some(box_road(move || file.clone(), None, &ran)), ..Wires::default() }, |s| {
                if other {
                    s.records = vec![record_json(json!({})), record_json(json!({ "id": PLACE2, "name": "vps", "joinedAt": "2026-10-03T00:00:00.000Z" }))];
                }
            });
            let e = d.manage.update(PLACE, ask()).await.expect_err(what);
            let (said, fix) = halves(said);
            usage(&e, &said, &fix);
            assert!(upds.borrow().is_empty(), "{what}: not a byte goes there");
            assert_kept(&d);
        }
    });
}

#[test]
fn update_host_key_changed_refuses() {
    // TS: places-remove.test.ts:567 "refuses an update where the login's sudo asks and the box answers with another key"; spec: U4 (R20)
    run(async {
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let sudo = sudo_road(|_, _, _, _| Err(place_host_key_changed("maya@10.0.0.5 answered with ssh-ed25519 AAAAother, not the ssh-ed25519 SHA256:kept you pinned")));
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(None))), sudo: Some(sudo), ..Wires::default() }, |_| {});
        let e = d.manage.update(PLACE, ask()).await.expect_err("host key changed");
        assert!(is_place_host_key_changed(&e));
        assert!(upds.borrow().is_empty());
        assert_kept(&d);
    });
}

#[test]
fn update_refused_login_fails_as_login_refusal() {
    // spec: U4 (`refused` fails as a login refusal with its sentence)
    run(async {
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let refusing: Rc<dyn PlaceRunOver> = Rc::new(RunOverRoad(Box::new(|_, _, _, _| Err(place_login_refused("ssh: connect to host 10.0.0.5 port 22: Connection refused"))), RefCell::new(vec![])));
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(None))), run_over: Some(refusing), ..Wires::default() }, |_| {});
        let e = d.manage.update(PLACE, ask()).await.expect_err("login refused");
        assert!(is_place_login_refused(&e));
        assert_eq!(e.kind, Some(Kind::parse("login")));
        assert_eq!(e.message, "ssh: connect to host 10.0.0.5 port 22: Connection refused");
        assert!(upds.borrow().is_empty());
    });
}

fn move_record(d: &Door, version: u32) -> LocalFut<'_, ()> {
    Box::pin(async move {
        d.records.change(PLACE, move |r| Some(PlaceRecord { report: report(json!({ "daemonVersion": version })), ..r })).await.unwrap();
    })
}

#[test]
fn update_waits_for_new_version() {
    // TS: places-update.test.ts:86 "puts the binary over the link as bytes, and the row reads the new version once that computer dials back on it";
    // :124 "is asked again by an update, so the row it answers with carries the new daemon's facts"; places-setup.test.ts:630; spec: U5
    run(async {
        let o = order();
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let dv = daemon_version();
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(Some(landed())))), ..Wires::default() }, |s| {
            s.records = vec![behind_record(json!({}))];
            s.opts = Box::new(|o| o.update_wait_ms = Some(5_000));
        });
        d.links.forking.borrow_mut().insert(PLACE.into(), Ok(StubBackend::new().arc()));
        link(&d, PLACE, reach_plain(&o, vec![]));
        let m = d.manage.clone();
        let handle = tokio::task::spawn_local(async move { m.update(PLACE, ask()).await });
        pass(&d.clock, 700).await;
        assert!(!handle.is_finished(), "the update waits for the computer to dial back on the new daemon");
        assert!(d.links.asked.borrow().is_empty());
        move_record(&d, dv).await;
        pass(&d.clock, 700).await;
        let reply = handle.await.unwrap().unwrap();
        assert_eq!(js(&reply), json!({ "name": NAME, "daemon": { "road": "link", "at": "/home/maya/.wsp/daemon/wsp-daemon", "kept": "/home/maya/.wsp/daemon/wsp-daemon.old", "from": dv - 1, "to": dv } }));
        assert_eq!(*d.links.asked.borrow(), vec![PLACE.to_string()], "the facts the new daemon says are read before the answer");
        let u = &upds.borrow()[0];
        assert!(u.daemon && u.link);
        assert_eq!(u.report.daemon_version.0.0 as u32, dv - 1, "the updater is told what the place said about itself");
    });
}

#[test]
fn update_slow_note_after_wait() {
    // TS: places-update.test.ts:159 "answers what the row still reads, with the reason, when the computer has not come back on it inside the wait"; spec: U5
    run(async {
        let o = order();
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let dv = daemon_version();
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(Some(landed())))), ..Wires::default() }, |s| {
            s.records = vec![behind_record(json!({}))];
            s.opts = Box::new(|o| o.update_wait_ms = Some(1_000));
        });
        link(&d, PLACE, reach_plain(&o, vec![]));
        let m = d.manage.clone();
        let handle = tokio::task::spawn_local(async move { m.update(PLACE, ask()).await });
        pass(&d.clock, 1_600).await;
        let reply = handle.await.unwrap().unwrap();
        assert_eq!(
            js(&reply),
            json!({ "name": NAME, "daemon": { "road": "link", "at": "/home/maya/.wsp/daemon/wsp-daemon", "kept": "/home/maya/.wsp/daemon/wsp-daemon.old", "from": dv - 1, "to": dv - 1, "note": w("updateSlow1") } })
        );
        assert!(d.links.asked.borrow().is_empty(), "no facts are read where the version did not move");
        assert_eq!(w("updateSlow1"), "old-macbook took the daemon and had not dialled back on it within 1s; its row reads the new version once it does");
    });
}

#[test]
fn update_current_computer_landing_has_no_note() {
    // spec: U5 (note only where `to < DAEMON_VERSION`; no facts where `to == from`)
    run(async {
        let o = order();
        let upds: Upds = Rc::new(RefCell::new(vec![]));
        let dv = daemon_version();
        let d = build(&new_key(), Wires { update: Some(updater(&upds, |_| Ok(Some(landed())))), ..Wires::default() }, |_| {});
        link(&d, PLACE, reach_plain(&o, vec![]));
        let reply = d.manage.update(PLACE, ask()).await.unwrap();
        assert_eq!(js(&reply)["daemon"], json!({ "road": "link", "at": "/home/maya/.wsp/daemon/wsp-daemon", "kept": "/home/maya/.wsp/daemon/wsp-daemon.old", "from": dv, "to": dv }));
        assert!(d.links.asked.borrow().is_empty());
        assert!(!upds.borrow()[0].daemon);
        assert_eq!(d.setup.calls(), vec![format!("syncSoon {PLACE} 0")]);
    });
}

// ---------------------------------------------------------------- the rows update and unable read

#[test]
fn behind_row_says_what_brings_it_level() {
    // TS: places-update.test.ts:19 "says so on its row with what brings it level"; spec: U3 (the row's side is views V16, V19)
    run(async {
        let dv = daemon_version();
        let behind_word = w("behindWord");
        let d = build(&new_key(), Wires::default(), |s| {
            s.records = vec![record_json(json!({ "name": "spoo", "report": report_json(json!({ "name": "spoo", "daemonVersion": dv - 1 })) })), record_json(json!({ "id": PLACE2, "name": "box", "report": report_json(json!({ "name": "box" })) }))];
        });
        let old = js(&d.views.view_of(&record(json!({ "name": "spoo", "report": report_json(json!({ "name": "spoo", "daemonVersion": dv - 1 })) })), None));
        assert_eq!(old["daemonVersion"], json!(dv - 1));
        assert_eq!(old["behind"], json!({ "word": behind_word, "fix": "wsp add spoo --update", "act": "update" }));
        let level = js(&d.views.view_of(&record(json!({ "id": PLACE2, "name": "box" })), None));
        assert!(level.get("behind").is_none());
        // this computer's own row: the app's road
        let here_behind = door(|s| s.opts = Box::new(move |o| o.here_daemon = Some(Rc::new(Here(Some(dv - 1))))));
        let row = js(&here_behind.views.here_row("here"));
        assert_eq!(row["daemonVersion"], json!(dv - 1));
        assert_eq!(row["behind"], json!({ "word": behind_word, "fix": "run wsp update here", "act": "install" }));
        let here_level = door(|s| s.opts = Box::new(move |o| o.here_daemon = Some(Rc::new(Here(Some(dv))))));
        let row = js(&here_level.views.here_row("here"));
        assert_eq!(row["daemonVersion"], json!(dv));
        assert!(row.get("behind").is_none());
        // no daemon started here yet is a version this host does not know
        let here_none = door(|s| s.opts = Box::new(move |o| o.here_daemon = Some(Rc::new(HereNone))));
        assert!(js(&here_none.views.here_row("here")).get("daemonVersion").is_none());
    });
}

struct HereNone;
#[async_trait(?Send)]
impl wsp_core::types::wiring::HereDaemon for HereNone {
    async fn version(&self) -> Result<u32, HostError> {
        Ok(daemon_version())
    }
    fn held(&self) -> Option<Option<u32>> {
        Some(None)
    }
    fn fix(&self) -> String {
        "updating the wsp app".into()
    }
}

#[test]
fn blocked_report_row_carries_the_doctors_sentence() {
    // TS: places-unable.test.ts:13 "holds the link, moves the last seen stamp, and carries the doctor's sentence on the row rather than as a refused dial"
    // (the link, the stamp and the absences are places-a's attach; the row's side is views V16)
    run(async {
        let d = build(&new_key(), Wires::default(), |_| {});
        let blocked = report_json(json!({ "name": "srv", "runsWorkspaces": false, "workspacesBlocked": "this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1" }));
        let row = js(&d.views.view_of(&record(json!({ "name": "srv", "report": blocked })), None));
        assert_eq!(row["blocked"], json!(w("blocked")));
        assert!(row.get("dialled").is_none());
    });
}
