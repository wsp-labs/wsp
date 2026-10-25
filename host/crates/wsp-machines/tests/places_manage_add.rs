// SPDX-License-Identifier: AGPL-3.0-only
//! Red tests for `places/manage`, unit add (`spec/places_manage_add.md`): add, dial, road, exec, adds, report_of,
//! home_of. Ports of `places-install.test.ts`, the dial cases of `places-link.test.ts`, the road cases of
//! `places-door.test.ts` and the `adds` case of `places-setup.test.ts:461`. Every sentence a person reads is written
//! out whole here, read off the TS; none is taken from the Rust lines, which are todo bodies.
mod places_b_common;
use places_b_common::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt;
use indexmap::IndexSet;
use serde_json::{json, Map, Value};
use tokio::sync::{oneshot, Notify};
use tokio::task::spawn_local;
use wsp_core::place_forward::PlaceForward;
use wsp_core::devices::{HeldDevice, MintMore};
use wsp_core::reach::{ReachError, ReachStats};
use wsp_core::types::pending::{DaemonReach, DeviceDoor, IssuedCode, PairedDevice};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::devices::DeviceView;
use wsp_machines::places::context::PlaceDoorContext;
use wsp_machines::places::helpers::{HeldForward, Live, ADD_RUNNING_FIX};
use wsp_machines::places::manage::SetupStarted;
use wsp_machines::places::records::{AwaitingInstall, LocalFut, PlaceRecordsArea};
use wsp_machines::places::types::*;
use wsp_store::keys::key_fingerprint;
use wsp_wire::views::place::{PlaceAddStep, PlaceBack};
use wsp_wire::wire::devices::{join_token, read_join_token, DeviceVia};
use wsp_words::exit::{HostError, Kind};

const ISO: &str = "2023-11-14T22:13:20.000Z";
const DOOR_URL: &str = "http://192.168.1.20:4400";
const PENDING: &str = "pending-computers";
const NO_INSTALLER: &str = "this host cannot install the agent on a computer over ssh; run wsp add with no argument for the line to type on that computer";
const ADD_RUNNING: &str = "an add under that id is still running. Leave the id out, or name the add afresh.";
const NOT_ANSWERING: &str = "is not answering; it connects on its own when it is on";
const NO_DIAL: &str = "wsp has no road to dial old-macbook: it joined by typing a code, so it connects on its own when it is on.";
const NO_PORT: &str = "old-macbook is connected but has not said which port its daemon is on, so nothing can carry a pane to it yet; it says so on its next link";

fn no_link(name: &str) -> String {
    format!("{name} took the daemon and has not dialled this host yet; check that it can reach this computer on the address it was given, and wsp computers shows it the moment it does")
}

fn cut_line(line: &str) -> String {
    format!("{} (cut {} characters)", &line[..400], line.len() - 400)
}

// ---------------------------------------------------------------- errors

fn plain(message: &str) -> HostError {
    HostError::new(message)
}

/// `refusal(happened, fix)`: the message is `happened. fix`, the fix beside it.
fn refused(said: &str, fix: &str) -> HostError {
    HostError { message: format!("{said} {fix}"), fix: Some(fix.into()), ..plain("") }
}

fn taken_back(message: &str) -> HostError {
    HostError { marks: vec![PLACE_ADD_TAKEN_BACK_MARK.to_string()], ..plain(message) }
}

fn absent_kind() -> Option<Kind> {
    Some(Kind::Other("absent".into()))
}

// ---------------------------------------------------------------- codes

/// The devices door with every issue and spend written down, and an issue that can be made to fail.
#[derive(Default)]
struct Mint {
    issued: RefCell<Vec<(i64, u64)>>,
    spent: RefCell<Vec<String>>,
    good: RefCell<IndexSet<String>>,
    fail: RefCell<Option<String>>,
}

#[async_trait(?Send)]
impl DeviceDoor for Mint {
    async fn issue(&self, now: i64, ttl_ms: u64, _here: bool) -> Result<IssuedCode, HostError> {
        if let Some(said) = self.fail.borrow().clone() {
            return Err(plain(&said));
        }
        self.issued.borrow_mut().push((now, ttl_ms));
        let code = format!("CODE{}", self.issued.borrow().len());
        self.good.borrow_mut().insert(code.clone());
        Ok(IssuedCode { code, expires_at: now + ttl_ms as i64 })
    }
    async fn redeem(&self, _: &str, _: &str, _: i64) -> Result<Option<PairedDevice>, HostError> {
        Ok(None)
    }
    async fn spend(&self, code: &str, _now: i64) -> Result<bool, HostError> {
        self.spent.borrow_mut().push(code.to_string());
        Ok(self.good.borrow_mut().shift_remove(code))
    }
    async fn admit(&self, name: &str, _now: i64) -> Result<PairedDevice, HostError> {
        Ok(PairedDevice { device_id: format!("d_{name}"), device_token: format!("token-{name}"), device: device_view(name) })
    }
    async fn admit_account(&self, _name: &str, _via: DeviceVia, _now: i64) -> Result<PairedDevice, HostError> {
        Err(plain("no account here"))
    }
    async fn refuses(&self, _fingerprint: &str) -> Result<bool, HostError> {
        Ok(false)
    }
    async fn mint(&self, _name: &str, _scope: ThreadScope, _now: i64, _more: MintMore) -> Result<PairedDevice, HostError> {
        Err(plain("no scoped tokens here"))
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        Ok(())
    }
    async fn r#match(&self, _token: &str) -> Result<Option<HeldDevice>, HostError> {
        Ok(None)
    }
    async fn seen(&self, _id: &str, _now: i64) -> Result<Option<HeldDevice>, HostError> {
        Ok(None)
    }
    async fn list(&self) -> Result<Vec<DeviceView>, HostError> {
        Ok(Vec::new())
    }
    async fn revoke(&self, _id: &str) -> Result<bool, HostError> {
        Ok(false)
    }
}

// ---------------------------------------------------------------- the rig

#[derive(Clone)]
struct Inside {
    ctx: Rc<PlaceDoorContext>,
    records: Rc<PlaceRecordsArea>,
    clock: Rc<FakeClock>,
    store: Rc<MemStore>,
}

/// What the installer reaches of the door it runs inside: filled once the door stands.
#[derive(Clone, Default)]
struct Hand(Rc<RefCell<Option<Inside>>>);

impl Hand {
    fn get(&self) -> Inside {
        self.0.borrow().clone().expect("the door stands")
    }
}

/// One install in flight, with what the computer's own join and attach would do.
struct Run {
    req: PlaceInstallRequest,
    stage: PlaceStaging,
    hand: Hand,
}

impl Run {
    fn code(&self) -> String {
        read_join_token(&self.req.code).code
    }
    fn st(&self, step: PlaceAddStep, state: StageState, note: Option<&str>) {
        (self.stage)(step, state, note.map(String::from), None, None);
    }
    fn entry(&self) -> Rc<RefCell<AwaitingInstall>> {
        self.hand.get().records.awaiting.borrow().get(&self.code()).cloned().expect("an awaiting entry for the code it was handed")
    }
    /// The join: notes the place the code became, writes the record, marks it the default, and holds a link where
    /// `reach` is given. The attach's wake is the caller's.
    async fn land(&self, rec: PlaceRecord, reach: Option<Rc<FakeReach>>) -> Option<Rc<FakeSocket>> {
        let inside = self.hand.get();
        let id = rec.id.clone();
        self.entry().borrow_mut().place_id = Some(id.clone());
        inside.records.keep(rec).await.unwrap();
        inside.records.mark_default(&id).await.unwrap();
        reach.map(|r| link_on(&inside.ctx, &id, r))
    }
    /// The attach: once the add waits on the entry, brings the link up `after_ms` later and wakes it.
    fn attach_later(&self, place_id: &str, after_ms: u64) {
        let inside = self.hand.get();
        let entry = self.entry();
        let id = place_id.to_string();
        spawn_local(async move {
            for _ in 0..1000 {
                let woken = entry.borrow().woken.clone();
                if let Some(woken) = woken {
                    inside.clock.advance(after_ms);
                    link_on(&inside.ctx, &id, FakeReach::answering(vec![]));
                    woken(&id);
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("the add never waited on the entry");
        });
    }
}

fn link_on(ctx: &PlaceDoorContext, place_id: &str, reach: Rc<dyn DaemonReach>) -> Rc<FakeSocket> {
    let socket = Rc::new(FakeSocket::default());
    ctx.live.borrow_mut().insert(place_id.to_string(), Live { socket: socket.clone(), reach, seen: Box::new(|| {}), forward: None });
    socket
}

type Installing = Rc<dyn Fn(Run) -> LocalFut<'static, PlaceResult<PlaceInstalled>>>;

macro_rules! inst {
    ([$($cap:ident),*] |$rn:ident| $body:expr) => {{
        $(let $cap = $cap.clone();)*
        let f: Installing = Rc::new(move |$rn: Run| -> LocalFut<'static, PlaceResult<PlaceInstalled>> {
            $(let $cap = $cap.clone();)*
            Box::pin(async move { $body })
        });
        f
    }};
}

struct Rig {
    d: Door,
    mint: Rc<Mint>,
    seen: Rc<RefCell<Vec<PlaceInstallRequest>>>,
}

/// A door with no record, this installer wired, and the codes written down.
fn rig(install: Installing, tweak: impl FnOnce(&mut Seed)) -> Rig {
    let hand = Hand::default();
    let mint = Rc::new(Mint::default());
    let seen: Rc<RefCell<Vec<PlaceInstallRequest>>> = Rc::default();
    let (h, m, s) = (hand.clone(), mint.clone(), seen.clone());
    let d = door(|seed| {
        seed.records = vec![];
        tweak(seed);
        let wiring = std::mem::replace(&mut seed.wiring, Box::new(|_| {}));
        seed.wiring = Box::new(move |w| {
            wiring(w);
            w.install = Some(Rc::new(Install(Box::new(move |req, stage| {
                s.borrow_mut().push(req.clone());
                install(Run { req, stage, hand: h.clone() })
            }))));
        });
        let opts = std::mem::replace(&mut seed.opts, Box::new(|_| {}));
        seed.opts = Box::new(move |o| {
            opts(o);
            o.devices = m;
        });
    });
    *hand.0.borrow_mut() = Some(Inside { ctx: d.ctx.clone(), records: d.records.clone(), clock: d.clock.clone(), store: d.store.clone() });
    Rig { d, mint, seen }
}

fn ask(over: impl FnOnce(&mut AddAsk)) -> AddAsk {
    let mut a = AddAsk { address: "root@10.0.0.9".into(), host_urls: vec![DOOR_URL.into()], ..Default::default() };
    over(&mut a);
    a
}

/// A computer as its own join wrote it: no road, nothing set up.
fn joined(name: &str, dialed: &str) -> PlaceRecord {
    record(json!({
        "id": PLACE, "name": name, "road": null, "applied": null, "picks": null, "recipe": null,
        "report": js(&report(json!({ "name": name, "dialed": dialed }))),
    }))
}

fn back_at(port: u32) -> PlaceBack {
    serde_json::from_value(json!({ "boxPort": port })).unwrap()
}

fn installed(name: &str) -> PlaceInstalled {
    PlaceInstalled { name: name.into(), ..Default::default() }
}

fn stage_json(d: &Door) -> Vec<Value> {
    d.stages.borrow().iter().map(js).collect()
}

fn stage_words(d: &Door) -> Vec<String> {
    stage_json(d).iter().map(|v| format!("{} {}", v["step"].as_str().unwrap(), v["state"].as_str().unwrap())).collect()
}

fn pendings(d: &Door) -> Vec<Value> {
    d.setups.borrow().iter().filter_map(|e| if let SetupEvent::Pending(p) = e { Some(js(p)) } else { None }).collect()
}

fn adds_json(d: &Door) -> Vec<Value> {
    d.manage.adds().iter().map(js).collect()
}

fn job_of(d: &Door, id: &str) -> Value {
    adds_json(d).into_iter().find(|j| j["addId"] == id).unwrap_or_else(|| panic!("no job {id}"))
}

fn writes_of_places(d: &Door) -> usize {
    d.store.writes_to("places/").len()
}

fn awaiting_keys(d: &Door) -> Vec<String> {
    d.records.awaiting.borrow().keys().cloned().collect()
}

async fn until(what: &str, f: impl Fn() -> bool) {
    for _ in 0..1000 {
        if f() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("never came: {what}");
}

async fn fail_add(r: &Rig, a: AddAsk) -> HostError {
    r.d.manage.add(a, NOW).await.expect_err("the add fails")
}

// ================================================================ add: refusals before anything is written

// TS: places-install.test.ts:17 "refuses on a host that wired no road onto a computer it has never met"
// spec: A1
#[test]
fn add_without_installer_refuses() {
    run(async {
        let d = door(|s| s.records = vec![]);
        let e = d.manage.add(ask(|_| {}), NOW).await.unwrap_err();
        assert_eq!(e.message, NO_INSTALLER);
        assert_eq!(e.kind, None);
        assert_eq!(e.fix, None);
        assert!(d.store.writes.borrow().is_empty());
        assert!(d.manage.adds().is_empty());
        assert!(awaiting_keys(&d).is_empty());
        assert!(d.devices.issued.borrow().is_empty());
        assert!(d.setups.borrow().is_empty());
    });
}

// TS: places-install.test.ts:498 "refuses an add under the id of one still running, and leaves that one's job as it was"
// spec: A3
#[test]
fn add_running_id_refused_as_usage() {
    run(async {
        let gate = Rc::new(Notify::new());
        let r = rig(
            inst!([gate] |rn| {
                rn.st(PlaceAddStep::Connect, StageState::Running, None);
                gate.notified().await;
                Err(plain("root@10.0.0.9 did not answer on port 22"))
            }),
            |_| {},
        );
        let m = r.d.manage.clone();
        let first = spawn_local(async move { m.add(ask(|a| a.add_id = Some("a_one".into())), NOW).await });
        until("the first add runs", || r.d.manage.adds().len() == 1).await;
        let e = fail_add(&r, ask(|a| {
            a.add_id = Some("a_one".into());
            a.address = "maya@elsewhere".into();
        }))
        .await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(e.message, ADD_RUNNING);
        assert_eq!(e.fix.as_deref(), Some(ADD_RUNNING_FIX));
        assert_eq!(
            adds_json(&r.d),
            vec![json!({ "addId": "a_one", "address": "root@10.0.0.9", "startedAt": ISO, "state": "running", "steps": [{ "step": "connect", "state": "running" }] })]
        );
        let docs = r.d.pending_docs();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0]["address"], "root@10.0.0.9");
        assert_eq!(r.seen.borrow().len(), 1);
        gate.notify_one();
        first.await.unwrap().unwrap_err();
    });
}

// spec: A2
#[test]
fn add_id_shape() {
    run(async {
        let r = rig(inst!([] |rn| { let _ = rn; Err(plain("root@10.0.0.9 did not answer on port 22")) }), |_| {});
        fail_add(&r, ask(|_| {})).await;
        fail_add(&r, ask(|_| {})).await;
        fail_add(&r, ask(|a| a.add_id = Some("a_given".into()))).await;
        let jobs = adds_json(&r.d);
        let ids: Vec<String> = jobs.iter().map(|j| j["addId"].as_str().unwrap().to_string()).collect();
        for id in &ids[..2] {
            assert_eq!(id.len(), 14, "{id}");
            assert!(id.starts_with("a_"), "{id}");
            assert!(id[2..].chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "{id}");
        }
        assert_ne!(ids[0], ids[1]);
        assert_eq!(ids[2], "a_given");
        let said: Vec<String> = stage_json(&r.d).iter().map(|v| v["addId"].as_str().unwrap().to_string()).collect();
        assert_eq!(said, ids);
    });
}

// spec: A4
#[test]
fn add_second_on_same_address_refused_while_standing() {
    run(async {
        let seeds = vec![
            pending_json("a_late", "root@10.0.0.9", json!({ "step": "floor", "name": "late", "startedAt": "2026-10-03T00:00:00.000Z" })),
            pending_json("a_early", "root@10.0.0.9", json!({ "step": "connect", "startedAt": "2026-10-01T00:00:00.000Z" })),
            pending_json("a_failed", "root@10.0.0.9", json!({ "failed": { "said": "x" }, "startedAt": "2026-10-00T00:00:00.000Z" })),
        ];
        let r = rig(inst!([] |rn| { let _ = rn; Ok(PlaceInstalled::default()) }), |s| s.pending = seeds.clone());
        let e = fail_add(&r, ask(|_| {})).await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(e.message, "root@10.0.0.9 is already being added (connecting over ssh). Finish it with wsp add root@10.0.0.9 --resume --recipe <name>, or in the app.");
        assert_eq!(e.fix.as_deref(), Some("Finish it with wsp add root@10.0.0.9 --resume --recipe <name>, or in the app."));
        for s in &seeds {
            assert_eq!(r.d.store.doc(PENDING, s["id"].as_str().unwrap()).as_ref(), Some(s));
        }
        assert!(r.seen.borrow().is_empty());
        assert!(r.d.store.writes.borrow().is_empty());
        assert!(r.d.manage.adds().is_empty());
        assert!(r.mint.issued.borrow().is_empty());
        assert!(pendings(&r.d).is_empty());

        let only = vec![pending_json("a_named", "root@10.0.0.9", json!({ "name": "box" }))];
        let r = rig(inst!([] |rn| { let _ = rn; Ok(PlaceInstalled::default()) }), |s| s.pending = only.clone());
        let e = fail_add(&r, ask(|_| {})).await;
        assert_eq!(e.message, "root@10.0.0.9 is already being added (joined; waiting on what goes on it). Finish it with wsp add box --resume --recipe <name>, or in the app.");
        assert_eq!(e.fix.as_deref(), Some("Finish it with wsp add box --resume --recipe <name>, or in the app."));
    });
}

// spec: A4
#[test]
fn add_takes_over_failed_pending_on_same_address() {
    run(async {
        let seeds = vec![
            pending_json("a_f1", "root@10.0.0.9", json!({ "failed": { "said": "x" }, "startedAt": "2026-10-01T00:00:00.000Z" })),
            pending_json("a_f2", "root@10.0.0.9", json!({ "failed": { "said": "y", "fix": "z" }, "startedAt": "2026-10-03T00:00:00.000Z" })),
            pending_json("a_other", "root@10.0.0.7", json!({})),
        ];
        let r = rig(inst!([] |rn| { let _ = rn; Err(plain("root@10.0.0.9 did not answer on port 22")) }), |s| s.pending = seeds.clone());
        fail_add(&r, ask(|a| a.add_id = Some("a_new".into()))).await;
        let writes = r.d.store.writes_to(PENDING);
        assert_eq!(&writes[..3], ["delete pending-computers/a_f1", "delete pending-computers/a_f2", "put pending-computers/a_new"]);
        let frames = pendings(&r.d);
        assert_eq!(frames[0], json!({ "id": "a_f1" }));
        assert_eq!(frames[1], json!({ "id": "a_f2" }));
        assert_eq!(frames[2]["id"], "a_new");
        assert!(r.d.store.doc(PENDING, "a_f1").is_none());
        assert!(r.d.store.doc(PENDING, "a_f2").is_none());
        assert_eq!(r.d.store.doc(PENDING, "a_other").as_ref(), Some(&seeds[2]));
        assert!(r.d.store.doc(PENDING, "a_new").is_some());
        assert_eq!(r.seen.borrow().len(), 1);
    });
}

// TS: places-install.test.ts:443 "keeps no add whose join code was never issued, since nothing would ever end it"
// spec: A7
#[test]
fn add_keeps_no_job_when_code_never_issued() {
    run(async {
        let r = rig(inst!([] |rn| { let _ = rn; Ok(installed("box")) }), |_| {});
        *r.mint.fail.borrow_mut() = Some("the state file is read-only".into());
        let e = fail_add(&r, ask(|a| a.add_id = Some("a_ro".into()))).await;
        assert_eq!(e.message, "the state file is read-only");
        assert!(r.d.manage.adds().is_empty());
        assert!(awaiting_keys(&r.d).is_empty());
        assert!(r.seen.borrow().is_empty());
        assert!(r.d.setups.borrow().len() == 1);
        // TS: the pending add was written before the code was asked for, and the throw is outside the try that
        // marks it failed, so it stands as written (manage.ts:216,239).
        let doc = r.d.store.doc(PENDING, "a_ro").expect("the pending add stands");
        assert_eq!(doc["step"], "connect");
        assert!(doc.get("failed").is_none());
    });
}

// ================================================================ add: what is written and handed to the install

// TS: places-install.test.ts:24 "mints a code the computer spends ..." (the request the install is handed)
// spec: A5, A7, A8
#[test]
fn add_writes_pending_before_install_and_hands_token() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                let inside = rn.hand.get();
                let empty = { let e = rn.entry(); let e = e.borrow(); e.place_id.is_none() && e.login.is_none() && e.back.is_none() && e.woken.is_none() };
                cap.borrow_mut().push(json!({
                    "pending": inside.store.doc(PENDING, "a_mine"),
                    "awaiting": inside.records.awaiting.borrow().keys().cloned().collect::<Vec<_>>(),
                    "entryEmpty": empty,
                    "job": inside.records.adds.borrow().get("a_mine").map(js),
                }));
                Err(plain("root@10.0.0.9 did not answer on port 22"))
            }),
            |s| s.records = vec![record_json(json!({}))],
        );
        fail_add(
            &r,
            ask(|a| {
                a.add_id = Some("a_mine".into());
                a.name = Some("box".into());
                a.ssh_port = Some(2222);
                a.key_path = Some("/Users/lena/.ssh/hetzner".into());
                a.host_key = Some("ssh-ed25519 SHA256:abc".into());
                a.sudo_password = Some("hunter2".into());
                a.door_port = Some(4640);
                a.relay = Some("https://h1.example".into());
                a.recipe = Some("web".into());
            }),
        )
        .await;
        let cap = cap.borrow();
        assert_eq!(
            cap[0]["pending"],
            json!({
                "id": "a_mine", "address": "root@10.0.0.9", "sshPort": 2222, "name": "box", "keyPath": "/Users/lena/.ssh/hetzner",
                "hostKey": "ssh-ed25519 SHA256:abc", "step": "connect", "choices": js(&picks("box", &[])), "recipe": "web", "startedAt": ISO,
            })
        );
        assert_eq!(cap[0]["awaiting"], json!(["CODE1"]));
        assert_eq!(cap[0]["entryEmpty"], true);
        assert_eq!(cap[0]["job"], json!({ "addId": "a_mine", "address": "root@10.0.0.9", "sshPort": 2222, "startedAt": ISO, "state": "running", "steps": [] }));

        assert_eq!(*r.mint.issued.borrow(), vec![(NOW as i64, 600_000)]);
        let seen = r.seen.borrow();
        let req = &seen[0];
        assert_eq!(req.code, join_token("CODE1", &key_fingerprint(&r.d.host_key.public_key)));
        assert_eq!(read_join_token(&req.code).code, "CODE1");
        assert_eq!(req.held, Some(vec![PLACE.to_string()]));
        assert_eq!(req.host_urls, vec![DOOR_URL.to_string()]);
        assert_eq!(req.address, "root@10.0.0.9");
        assert_eq!(req.name.as_deref(), Some("box"));
        assert_eq!(req.ssh_port, Some(2222));
        assert_eq!(req.key_path.as_deref(), Some("/Users/lena/.ssh/hetzner"));
        assert_eq!(req.host_key.as_deref(), Some("ssh-ed25519 SHA256:abc"));
        assert_eq!(req.sudo_password.as_deref(), Some("hunter2"));
        assert_eq!(req.door_port, Some(4640));
        assert_eq!(req.relay.as_deref(), Some("https://h1.example"));
        assert!(req.before_deploy.is_some());
        assert!(awaiting_keys(&r.d).is_empty());
    });
}

// spec: A5
#[test]
fn add_pending_choices_default_to_the_address_with_nothing_picked() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                let id = rn.hand.get().records.adds.borrow().keys().next().cloned().unwrap();
                cap.borrow_mut().push(rn.hand.get().store.doc(PENDING, &id).unwrap());
                Err(plain("root@10.0.0.9 did not answer on port 22"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_bare".into()))).await;
        assert_eq!(cap.borrow()[0], json!({ "id": "a_bare", "address": "root@10.0.0.9", "step": "connect", "choices": js(&picks("root@10.0.0.9", &[])), "startedAt": ISO }));
    });
}

// spec: A7
#[test]
fn add_job_put_afresh_at_end() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                let adds = rn.hand.get().records.adds.clone();
                let keys: Vec<String> = adds.borrow().keys().cloned().collect();
                cap.borrow_mut().push(json!({ "keys": keys, "last": adds.borrow().values().last().map(js) }));
                Err(plain(&format!("{} did not answer on port 22", rn.req.address)))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_1".into()))).await;
        fail_add(&r, ask(|a| a.add_id = Some("a_2".into()))).await;
        fail_add(&r, ask(|a| {
            a.add_id = Some("a_1".into());
            a.address = "root@10.0.0.7".into();
            a.ssh_port = Some(22);
        }))
        .await;
        let cap = cap.borrow();
        assert_eq!(cap[2]["keys"], json!(["a_2", "a_1"]));
        assert_eq!(cap[2]["last"], json!({ "addId": "a_1", "address": "root@10.0.0.7", "sshPort": 22, "startedAt": ISO, "state": "running", "steps": [] }));
        let ids: Vec<String> = r.d.manage.adds().iter().map(|j| j.add_id.clone()).collect();
        assert_eq!(ids, ["a_2", "a_1"]);
        assert_eq!(job_of(&r.d, "a_1")["said"], "root@10.0.0.7 did not answer on port 22");
    });
}

// spec: A8, A5
#[test]
fn add_before_deploy_writes_undo_and_login() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                let before = rn.req.before_deploy.clone().expect("beforeDeploy");
                let store = rn.hand.get().store;
                before.before_deploy("rm -rf /root/.wsp".into(), "root@10.0.0.9".into(), None).await.unwrap();
                cap.borrow_mut().push(store.doc(PENDING, "a_b").unwrap());
                before.before_deploy("rm -rf /root/.wsp".into(), "root@10.0.0.9".into(), Some("ssh-ed25519 NEW".into())).await.unwrap();
                cap.borrow_mut().push(store.doc(PENDING, "a_b").unwrap());
                Err(plain("systemctl exited 1"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| {
            a.add_id = Some("a_b".into());
            a.host_key = Some("ssh-ed25519 OLD".into());
        }))
        .await;
        let cap = cap.borrow();
        let base = json!({ "id": "a_b", "address": "root@10.0.0.9", "hostKey": "ssh-ed25519 OLD", "step": "wsp", "choices": js(&picks("root@10.0.0.9", &[])), "startedAt": ISO, "undo": "rm -rf /root/.wsp", "login": "root@10.0.0.9" });
        assert_eq!(cap[0], base);
        let mut newer = base.clone();
        newer["hostKey"] = json!("ssh-ed25519 NEW");
        assert_eq!(cap[1], newer);
        let after = r.d.store.doc(PENDING, "a_b").unwrap();
        assert_eq!(after["step"], "wsp");
        assert_eq!(after["undo"], "rm -rf /root/.wsp");
        assert_eq!(after["login"], "root@10.0.0.9");
        assert_eq!(after["failed"], json!({ "said": "systemctl exited 1" }));
    });
}

// spec: A5 (a failed write of a later move is swallowed and the chain goes on)
#[test]
fn add_before_deploy_failed_write_is_swallowed() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                let store = rn.hand.get().store;
                store.fail_writes.borrow_mut().insert(PENDING.to_string(), "disk full".into());
                let moved = rn.req.before_deploy.clone().unwrap().before_deploy("undo".into(), "root@10.0.0.9".into(), None).await;
                cap.borrow_mut().push(json!({ "ok": moved.is_ok(), "stored": store.doc(PENDING, "a_s").unwrap()["step"] }));
                Err(plain("systemctl exited 1"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_s".into()))).await;
        assert_eq!(cap.borrow()[0], json!({ "ok": true, "stored": "connect" }));
        let after = r.d.store.doc(PENDING, "a_s").unwrap();
        assert_eq!(after["step"], "wsp");
        assert_eq!(after["undo"], "undo");
        assert_eq!(after["failed"], json!({ "said": "systemctl exited 1" }));
    });
}

// ================================================================ add: stages

// spec: A6
#[test]
fn add_stage_check_moves_pending() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                let store = rn.hand.get().store;
                rn.st(PlaceAddStep::Connect, StageState::Running, None);
                settle().await;
                cap.borrow_mut().push(store.doc(PENDING, "a_c").unwrap()["step"].clone());
                rn.st(PlaceAddStep::Check, StageState::Running, None);
                settle().await;
                cap.borrow_mut().push(store.doc(PENDING, "a_c").unwrap()["step"].clone());
                rn.st(PlaceAddStep::Root, StageState::Running, None);
                settle().await;
                cap.borrow_mut().push(store.doc(PENDING, "a_c").unwrap()["step"].clone());
                Err(plain("sudo asks for a password"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_c".into()))).await;
        assert_eq!(*cap.borrow(), vec![json!("connect"), json!("check"), json!("check")]);
        let frames = pendings(&r.d);
        assert_eq!(frames.iter().filter_map(|f| f["pending"]["step"].as_str()).collect::<Vec<_>>()[..2], ["connect", "check"]);
    });
}

// spec: A6
#[test]
fn add_stages_reach_on_stage_and_job() {
    run(async {
        let cap: Rc<RefCell<Vec<Value>>> = Rc::default();
        let r = rig(
            inst!([cap] |rn| {
                (rn.stage)(PlaceAddStep::Connect, StageState::Running, None, None, None);
                (rn.stage)(PlaceAddStep::Connect, StageState::Done, Some("Ubuntu 24.04".into()), None, Some(1200));
                (rn.stage)(PlaceAddStep::Wsp, StageState::Running, None, None, None);
                cap.borrow_mut().push(js(&rn.hand.get().records.adds.borrow().get("a_s").unwrap().clone()));
                Err(plain("systemctl exited 1"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_s".into()))).await;
        assert_eq!(
            cap.borrow()[0],
            json!({ "addId": "a_s", "address": "root@10.0.0.9", "startedAt": ISO, "state": "running", "steps": [
                { "step": "connect", "state": "done", "note": "Ubuntu 24.04", "ms": 1200 }, { "step": "wsp", "state": "running" } ] })
        );
        assert_eq!(
            stage_json(&r.d),
            vec![
                json!({ "addId": "a_s", "step": "connect", "state": "running" }),
                json!({ "addId": "a_s", "step": "connect", "state": "done", "note": "Ubuntu 24.04", "ms": 1200 }),
                json!({ "addId": "a_s", "step": "wsp", "state": "running" }),
                json!({ "addId": "a_s", "step": "wsp", "state": "failed", "note": "systemctl exited 1" }),
            ]
        );
    });
}

// TS: places-install.test.ts:114 "names the step an install stopped on, and the code it minted opens nothing afterwards"
// spec: A15, A6
#[test]
fn add_failure_names_the_step_it_stopped_on_and_spends_the_code() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.st(PlaceAddStep::Wsp, StageState::Running, None);
                Err(plain("ssh refused the login (publickey)"))
            }),
            |_| {},
        );
        let e = fail_add(&r, ask(|_| {})).await;
        assert_eq!(e, plain("ssh refused the login (publickey)"));
        assert_eq!(stage_words(&r.d), ["wsp running", "wsp failed"]);
        assert_eq!(stage_json(&r.d)[1]["note"], "ssh refused the login (publickey)");
        assert_eq!(*r.mint.spent.borrow(), vec!["CODE1".to_string()]);
        assert!(r.mint.good.borrow().is_empty());
        assert!(!r.mint.spend("CODE1", NOW as i64).await.unwrap());
    });
}

// TS: places-install.test.ts:140 "says a step the installer already marked failed once, not a second time as the add ends"
// spec: A15, A6
#[test]
fn add_failed_step_the_installer_marked_is_not_said_twice() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.st(PlaceAddStep::Check, StageState::Running, None);
                rn.st(PlaceAddStep::Root, StageState::Running, None);
                rn.st(PlaceAddStep::Root, StageState::Failed, Some("sudo on dev@spoo asks for dev's password"));
                Err(plain("sudo on dev@spoo asks for dev's password"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.address = "dev@spoo".into())).await;
        assert_eq!(stage_words(&r.d), ["check running", "root running", "root failed"]);
        assert_eq!(job_of(&r.d, r.d.manage.adds()[0].add_id.as_str())["state"], "failed");
    });
}

// TS: places-install.test.ts:161 "draws a failure on the step that was under way, not on a step that only said what it had done"
// spec: A6, A15
#[test]
fn add_failure_drawn_on_the_step_under_way() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.st(PlaceAddStep::Connect, StageState::Running, None);
                rn.st(PlaceAddStep::HostKey, StageState::Done, Some("ssh-ed25519 SHA256:abc"));
                Err(plain("root@spoo runs zsh as root's shell"))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.address = "root@spoo".into())).await;
        assert_eq!(stage_words(&r.d), ["connect running", "host-key done", "connect failed"]);
    });
}

// TS: places-link.test.ts:141 "marks the step an install stopped in with the installer's one sentence and hands back that sentence alone"
// spec: A15
#[test]
fn add_failure_note_is_the_installers_one_sentence() {
    run(async {
        let sentence = "spoo took wsp but could not connect back: the host at http://100.129.166.28:4640 did not answer in 20s";
        let r = rig(
            inst!([] |rn| {
                rn.st(PlaceAddStep::Connect, StageState::Done, Some("Ubuntu 24.04"));
                rn.st(PlaceAddStep::Wsp, StageState::Done, Some("x86_64"));
                rn.st(PlaceAddStep::Service, StageState::Running, None);
                Err(plain(sentence))
            }),
            |_| {},
        );
        let e = fail_add(&r, ask(|a| a.address = "root@178.156.161.168".into())).await;
        assert_eq!(e, plain(sentence));
        let failed: Vec<Value> = stage_json(&r.d).into_iter().filter(|s| s["state"] == "failed").collect();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0]["step"], "service");
        assert_eq!(failed[0]["note"], sentence);
    });
}

// ================================================================ add: the join

// TS: places-install.test.ts:24 "mints a code the computer spends, says what each step is doing, and answers once that
// computer's link is up"; places-install.test.ts:421 "marks an add done with the computer it made once that computer has joined"
// spec: A10, A11, A12, A13, A9
#[test]
fn add_join_done_note_and_job_done() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.st(PlaceAddStep::Connect, StageState::Done, Some("Ubuntu 24.04"));
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(PlaceInstalled { name: "box".into(), host_key: Some("ssh-ed25519 SHA256:abc".into()), ..Default::default() })
            }),
            |s| {
                s.links.forking.borrow_mut().insert(PLACE.to_string(), Ok(StubBackend::new().arc()));
            },
        );
        let added = r.d.manage.add(ask(|a| {
            a.add_id = Some("a_mine".into());
            a.name = Some("box".into());
        }), NOW)
        .await
        .unwrap();
        assert_eq!(added.add_id, "a_mine");
        assert_eq!(added.host_key.as_deref(), Some("ssh-ed25519 SHA256:abc"));
        assert_eq!(added.said, None);
        let place = js(&added.place);
        assert_eq!(place["id"], PLACE);
        assert_eq!(place["name"], "box");
        assert_eq!(place["present"], true);
        assert_eq!(place["default"], true);
        assert_eq!(stage_words(&r.d), ["connect done", "join running", "join done"]);
        assert_eq!(stage_json(&r.d)[0], json!({ "addId": "a_mine", "step": "connect", "state": "done", "note": "Ubuntu 24.04" }));
        assert_eq!(stage_json(&r.d)[1], json!({ "addId": "a_mine", "step": "join", "state": "running" }));
        // The road its link came in on; TS says the note whether or not it names a road (manage.ts:300), so it is a string here too.
        assert_eq!(stage_json(&r.d)[2], json!({ "addId": "a_mine", "step": "join", "state": "done", "note": "at http://192.168.1.20:4400", "placeId": PLACE, "ms": 0 }));
        assert_eq!(
            job_of(&r.d, "a_mine"),
            json!({ "addId": "a_mine", "address": "root@10.0.0.9", "startedAt": ISO, "state": "done", "placeId": PLACE, "steps": [
                { "step": "connect", "state": "done", "note": "Ubuntu 24.04" },
                { "step": "join", "state": "done", "note": "at http://192.168.1.20:4400", "ms": 0 } ] })
        );
        // A12: what that computer forks with is asked once, over the link the join opened.
        assert_eq!(*r.d.links.asked.borrow(), vec![PLACE.to_string()]);
        assert!(awaiting_keys(&r.d).is_empty());
        assert!(r.mint.spent.borrow().is_empty());
    });
}

// TS: places-install.test.ts:67 "never says an address the box claims it dialled that this add did not hand it"
// spec: A11
#[test]
fn add_join_done_note_empty_for_an_address_not_handed() {
    run(async {
        let dialed = format!("https://whatever-it-likes.example/{}", "a".repeat(4000));
        let r = rig(
            inst!([dialed] |rn| {
                rn.land(joined("box", &dialed), Some(FakeReach::answering(vec![]))).await;
                Ok(installed("box"))
            }),
            |_| {},
        );
        r.d.manage.add(ask(|a| a.add_id = Some("a_x".into())), NOW).await.unwrap();
        let done = stage_json(&r.d).into_iter().find(|s| s["step"] == "join" && s["state"] == "done").unwrap();
        assert_eq!(done["note"], "");
        assert_eq!(done["placeId"], PLACE);
    });
}

// TS: places-link.test.ts:307 "says the link came in over ssh where the box dialled the forward on its own loopback"
// spec: A11
#[test]
fn add_join_done_note_over_ssh_for_the_forward() {
    run(async {
        let back = Rc::new(Back::default());
        let b = back.clone();
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("spoo", "http://127.0.0.1:14621"), Some(FakeReach::answering(vec![]))).await;
                Ok(PlaceInstalled { name: "spoo".into(), ssh: Some("root@spoo".into()), back: Some(back_at(14621)), ..Default::default() })
            }),
            |s| s.wiring = Box::new(move |w| w.back = Some(b)),
        );
        r.d.manage.add(ask(|a| {
            a.address = "spoo".into();
            a.door_port = Some(4640);
        }), NOW)
        .await
        .unwrap();
        let done = stage_json(&r.d).into_iter().find(|s| s["step"] == "join" && s["state"] == "done").unwrap();
        assert_eq!(done["note"], "over ssh");
        // TS: places-link.test.ts:315 "keeps the forward an install stood held for the record it made": held once,
        // for the record the add made, and the record's road carries it.
        assert_eq!(*back.log.borrow(), vec![format!("hold root@spoo /home/maya {PLACE}")]);
        assert_eq!(r.d.stored(PLACE).unwrap()["road"], json!({ "ssh": "root@spoo", "back": { "boxPort": 14621 } }));
    });
}

// TS: places-install.test.ts:297 (the record's half) "keeps the login the install used on the record ..."; places-link.test.ts:43
// spec: A9
#[test]
fn add_join_landed_before_install_answer_carries_road() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(PlaceInstalled {
                    name: "box".into(),
                    ssh: Some("root@65.21.4.12".into()),
                    ssh_key_path: Some("/Users/lena/.ssh/hetzner".into()),
                    host_key: Some("ssh-ed25519 SHA256:abc".into()),
                    back: None,
                })
            }),
            |_| {},
        );
        let added = r.d.manage.add(ask(|_| {}), NOW).await.unwrap();
        assert_eq!(r.d.stored(PLACE).unwrap()["road"], json!({ "ssh": "root@65.21.4.12", "keyPath": "/Users/lena/.ssh/hetzner", "hostKey": "ssh-ed25519 SHA256:abc" }));
        assert_eq!(writes_of_places(&r.d), 2);
        // What a client reads of the road is the login and where it came from, never the key file.
        assert_eq!(js(&added.place)["road"], json!({ "ssh": "root@65.21.4.12" }));
    });
}

// spec: A9 (an install that names no login writes the record once)
#[test]
fn add_join_landed_without_a_login_writes_the_record_once() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(installed("box"))
            }),
            |_| {},
        );
        r.d.manage.add(ask(|_| {}), NOW).await.unwrap();
        assert_eq!(writes_of_places(&r.d), 1);
        assert!(r.d.stored(PLACE).unwrap().get("road").is_none());
    });
}

// TS: places-install.test.ts:88 "waits for the link the agent dials, not the socket the join itself opened and closed"
// spec: A10
#[test]
fn add_waits_for_the_link_the_agent_dials() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                // The join's own socket opens and closes; the unit it installed opens the link a moment later.
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                rn.hand.get().ctx.live.borrow_mut().shift_remove(PLACE);
                rn.attach_later(PLACE, 250);
                Ok(installed("box"))
            }),
            |_| {},
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|_| {}), NOW).await });
        settle().await;
        until("the add waits", || stage_words(&r.d).last().map(|w| w == "join running").unwrap_or(false)).await;
        assert!(!h.is_finished());
        let added = h.await.unwrap().unwrap();
        assert_eq!(js(&added.place)["present"], true);
        // The wait is measured on the clock the door reads: the attach came 250 ms after the wait began.
        assert_eq!(stage_json(&r.d).last().unwrap()["ms"], 250);
        assert!(awaiting_keys(&r.d).is_empty());
    });
}

// spec: A11
#[test]
fn add_woken_with_no_record_fails_with_the_no_link_line() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                let entry = rn.entry();
                spawn_local(async move {
                    for _ in 0..1000 {
                        let woken = entry.borrow().woken.clone();
                        if let Some(woken) = woken {
                            woken("p_ghost");
                            return;
                        }
                        tokio::task::yield_now().await;
                    }
                });
                Ok(installed("box"))
            }),
            |_| {},
        );
        let e = fail_add(&r, ask(|_| {})).await;
        assert_eq!(e.message, no_link("box"));
        assert_eq!(stage_words(&r.d), ["join running", "join failed"]);
    });
}

// TS: places-install.test.ts:197 "gives up on a computer that took the agent and never dialled, in the sentence that says what to check"
// spec: A10
#[test]
fn add_join_wait_runs_out() {
    run(async {
        let asked: Rc<RefCell<Vec<PlaceLogin>>> = Rc::default();
        let a = asked.clone();
        let r = rig(
            inst!([] |rn| { let _ = rn; Ok(installed("box")) }),
            |s| {
                s.opts = Box::new(|o| o.join_wait_ms = Some(50));
                s.wiring = Box::new(move |w| w.log = Some(Rc::new(Log(Box::new(move |l| { a.borrow_mut().push(l.clone()); Ok(vec!["never read".into()]) })))));
            },
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| a.add_id = Some("a_w".into())), NOW).await });
        pass(&r.d.clock, 60).await;
        let e = h.await.unwrap().unwrap_err();
        assert_eq!(e, plain(&no_link("box")));
        assert!(asked.borrow().is_empty(), "no ssh login named, so the box is not read");
        assert_eq!(stage_words(&r.d), ["join running", "join failed"]);
        assert_eq!(stage_json(&r.d)[1]["note"], no_link("box"));
        let job = job_of(&r.d, "a_w");
        assert_eq!(job["state"], "failed");
        assert_eq!(job["said"], no_link("box"));
        assert_eq!(r.d.store.doc(PENDING, "a_w").unwrap()["failed"], json!({ "said": no_link("box") }));
        assert_eq!(*r.mint.spent.borrow(), vec!["CODE1".to_string()]);
        assert!(awaiting_keys(&r.d).is_empty());
    });
}

// spec: A10 (the wait is JOIN_WAIT_MS, 90 s, where the options name none)
#[test]
fn add_join_wait_defaults_to_ninety_seconds() {
    run(async {
        let r = rig(inst!([] |rn| { let _ = rn; Ok(installed("box")) }), |_| {});
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|_| {}), NOW).await });
        pass(&r.d.clock, 89_999).await;
        assert!(!h.is_finished());
        pass(&r.d.clock, 2).await;
        let e = h.await.unwrap().unwrap_err();
        assert_eq!(e.message, no_link("box"));
    });
}

// TS: places-install.test.ts:210 "puts the agent's own last lines under that sentence, read over the login the install used"
// spec: A10
#[test]
fn add_join_wait_runs_out_with_box_lines() {
    run(async {
        let asked: Rc<RefCell<Vec<PlaceLogin>>> = Rc::default();
        let a = asked.clone();
        let said = vec![
            "https://h645d7f8a8d48cbd6.example could not be dialled: not an http address".to_string(),
            "http://100.129.175.77:4420 did not answer in 10s".to_string(),
        ];
        let lines = said.clone();
        let r = rig(
            inst!([] |rn| { let _ = rn; Ok(PlaceInstalled { name: "box".into(), ssh: Some("root@10.0.0.9".into()), ssh_key_path: Some("/Users/lena/.ssh/hetzner".into()), ..Default::default() }) }),
            |s| {
                s.opts = Box::new(|o| o.join_wait_ms = Some(50));
                s.wiring = Box::new(move |w| w.log = Some(Rc::new(Log(Box::new(move |l| { a.borrow_mut().push(l.clone()); Ok(lines.clone()) })))));
            },
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|_| {}), NOW).await });
        pass(&r.d.clock, 60).await;
        let e = h.await.unwrap().unwrap_err();
        assert_eq!(e.message, [no_link("box"), said[0].clone(), said[1].clone()].join("\n"));
        assert_eq!(*asked.borrow(), vec![PlaceLogin { ssh: "root@10.0.0.9".into(), key_path: Some("/Users/lena/.ssh/hetzner".into()), host_key: None }]);
        // The step's note is one line: the box's lines ride the throw.
        assert_eq!(stage_json(&r.d).last().unwrap()["note"], no_link("box"));
    });
}

// TS: places-install.test.ts:238 "says the wait's own sentence and nothing else when the box will not answer the read either"
// spec: A10
#[test]
fn add_join_wait_with_an_unreadable_box_says_only_the_sentence() {
    run(async {
        let r = rig(
            inst!([] |rn| { let _ = rn; Ok(PlaceInstalled { name: "box".into(), ssh: Some("root@10.0.0.9".into()), ..Default::default() }) }),
            |s| {
                s.opts = Box::new(|o| o.join_wait_ms = Some(50));
                s.wiring = Box::new(|w| w.log = Some(Rc::new(Log(Box::new(|_| Err(plain("ssh: connect to host 10.0.0.9 port 22: Connection refused")))))));
            },
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|_| {}), NOW).await });
        pass(&r.d.clock, 60).await;
        assert_eq!(h.await.unwrap().unwrap_err().message, no_link("box"));
    });
}

// TS: places-install.test.ts:297 "keeps the login the install used on the record when the computer never dials back, which is the box that needs it most"
// spec: A9, A15
#[test]
fn add_wait_out_keeps_the_install_login_on_the_record() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                // The shape a box that never comes back has: its own join opens a socket and closes it.
                rn.land(joined("vps", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                rn.hand.get().ctx.live.borrow_mut().shift_remove(PLACE);
                Ok(PlaceInstalled { name: "vps".into(), ssh: Some("root@65.21.4.12".into()), ssh_key_path: Some("/Users/lena/.ssh/hetzner".into()), ..Default::default() })
            }),
            |s| s.opts = Box::new(|o| o.join_wait_ms = Some(60)),
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| a.key_path = Some("/Users/lena/.ssh/hetzner".into())), NOW).await });
        pass(&r.d.clock, 70).await;
        let e = h.await.unwrap().unwrap_err();
        assert_eq!(e.message, no_link("vps"));
        assert_eq!(r.d.stored(PLACE).unwrap()["road"], json!({ "ssh": "root@65.21.4.12", "keyPath": "/Users/lena/.ssh/hetzner" }));
        assert!(awaiting_keys(&r.d).is_empty());
    });
}

// ================================================================ add: the answer

// spec: A13
#[test]
fn add_without_choices_pending_at_floor() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(PlaceInstalled { name: "box".into(), host_key: Some("ssh-ed25519 SHA256:abc".into()), ..Default::default() })
            }),
            |_| {},
        );
        let added = r.d.manage.add(ask(|a| {
            a.add_id = Some("a_p".into());
            a.name = Some("box".into());
            a.ssh_port = Some(2222);
            a.key_path = Some("/k".into());
            a.host_key = Some("ssh-ed25519 SHA256:abc".into());
            a.recipe = Some("web".into());
        }), NOW)
        .await
        .unwrap();
        assert_eq!(added.said, None);
        assert_eq!(added.host_key.as_deref(), Some("ssh-ed25519 SHA256:abc"));
        assert_eq!(js(&added.place)["default"], true);
        // The pending add as it answers: at the floor, with the computer it made, and none of the record-only fields.
        assert_eq!(
            js(added.pending.as_ref().expect("a pending add")),
            json!({ "id": "a_p", "address": "root@10.0.0.9", "sshPort": 2222, "name": "box", "placeId": PLACE, "step": "floor",
                "choices": js(&picks("box", &[])), "recipe": "web", "startedAt": ISO })
        );
        // Behind the answer the floor runs; with no provisioner wired it only moves the add to choosing.
        until("the floor settles", || r.d.views.flooring.borrow().is_empty() && r.d.store.doc(PENDING, "a_p").map(|d| d["step"] == "choosing").unwrap_or(false)).await;
        let doc = r.d.store.doc(PENDING, "a_p").unwrap();
        assert_eq!(doc["placeId"], PLACE);
        assert_eq!(doc["keyPath"], "/k");
        assert_eq!(job_of(&r.d, "a_p")["state"], "done");
    });
}

// spec: A13 (the floor runs on the computer's link behind the answer)
#[test]
fn add_without_choices_runs_the_floor_behind_the_answer() {
    run(async {
        let provision = Rc::new(Provision::default());
        let p = provision.clone();
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(installed("box"))
            }),
            |s| {
                s.setup.link = RefCell::new(Some(FakeMachineLink::exec_says("")));
                s.wiring = Box::new(move |w| w.provision = Some(p));
            },
        );
        let added = r.d.manage.add(ask(|a| a.add_id = Some("a_f".into())), NOW).await.unwrap();
        assert_eq!(js(added.pending.as_ref().unwrap())["step"], "floor");
        until("the floor ran", || r.d.store.doc(PENDING, "a_f").map(|d| d["step"] == "choosing").unwrap_or(false)).await;
        assert_eq!(*provision.log.borrow(), vec!["floor /home/maya".to_string()]);
        assert!(r.d.setup.calls().contains(&format!("linkTo {PLACE}")));
        assert!(r.d.views.flooring.borrow().is_empty());
    });
}

// spec: A14
#[test]
fn add_with_choices_drops_pending_starts_setup() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(PlaceInstalled { name: "box".into(), host_key: Some("ssh-ed25519 SHA256:abc".into()), ..Default::default() })
            }),
            |_| {},
        );
        let added = r.d.manage.add(ask(|a| {
            a.add_id = Some("a_x".into());
            a.choices = Some(picks("box", &["lint"]));
            a.recipe = Some("web".into());
        }), NOW)
        .await
        .unwrap();
        assert!(added.pending.is_none());
        assert_eq!(added.said, None);
        assert_eq!(added.host_key.as_deref(), Some("ssh-ed25519 SHA256:abc"));
        assert_eq!(js(&added.place)["id"], PLACE);
        assert!(r.d.pending_docs().is_empty());
        assert_eq!(pendings(&r.d).last().unwrap(), &json!({ "id": "a_x" }));
        let started: Vec<String> = r.d.setup.calls().into_iter().filter(|c| c.starts_with("startedOrSaid")).collect();
        assert_eq!(started, vec![format!("startedOrSaid {PLACE} a_x picks=box recipe=web")]);
        // No floor: nothing is written to the pending add after it was dropped.
        assert_eq!(r.d.store.writes_to(PENDING).last().unwrap(), "delete pending-computers/a_x");
    });
}

// spec: A14 (where the setup did not start, its sentence rides the answer)
#[test]
fn add_with_choices_says_why_no_setup_started() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("box", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Ok(installed("box"))
            }),
            |s| s.setup.started = RefCell::new(SetupStarted { setup: None, said: Some("box is busy with a setup".into()) }),
        );
        let added = r.d.manage.add(ask(|a| a.choices = Some(picks("box", &[]))), NOW).await.unwrap();
        assert_eq!(added.said.as_deref(), Some("box is busy with a setup"));
        assert!(added.pending.is_none());
        assert_eq!(added.host_key, None);
        let started: Vec<String> = r.d.setup.calls().into_iter().filter(|c| c.starts_with("startedOrSaid")).collect();
        assert_eq!(started.len(), 1);
        assert!(started[0].ends_with("picks=box recipe=-"), "{}", started[0]);
    });
}

// ================================================================ add: failures

// TS: places-install.test.ts:352 "keeps an add that failed mid-way, its steps, what it said and the fix, for a second client to read off places.list"
// spec: A15
#[test]
fn add_failure_spends_code_marks_pending_and_job() {
    run(async {
        let gate = Rc::new(Notify::new());
        let original = refused("spoo has no curl or wget on its PATH", "Install one of them there, then add again.");
        let o = original.clone();
        let r = rig(
            inst!([gate, o] |rn| {
                rn.st(PlaceAddStep::Connect, StageState::Running, None);
                rn.st(PlaceAddStep::Connect, StageState::Done, Some("Ubuntu 24.04"));
                rn.st(PlaceAddStep::Wsp, StageState::Running, None);
                gate.notified().await;
                Err(o)
            }),
            |_| {},
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| {
            a.add_id = Some("a_spoo".into());
            a.address = "root@spoo".into();
            a.ssh_port = Some(2222);
        }), NOW).await });
        until("the install runs", || stage_words(&r.d).len() == 3).await;
        assert_eq!(
            adds_json(&r.d),
            vec![json!({ "addId": "a_spoo", "address": "root@spoo", "sshPort": 2222, "startedAt": ISO, "state": "running",
                "steps": [{ "step": "connect", "state": "done", "note": "Ubuntu 24.04" }, { "step": "wsp", "state": "running" }] })]
        );
        gate.notify_one();
        let e = h.await.unwrap().unwrap_err();
        assert_eq!(e, original);
        assert_eq!(
            adds_json(&r.d),
            vec![json!({ "addId": "a_spoo", "address": "root@spoo", "sshPort": 2222, "startedAt": ISO, "state": "failed",
                "said": "spoo has no curl or wget on its PATH.", "fix": "Install one of them there, then add again.",
                "steps": [{ "step": "connect", "state": "done", "note": "Ubuntu 24.04" },
                    { "step": "wsp", "state": "failed", "note": "spoo has no curl or wget on its PATH. Install one of them there, then add again." }] })]
        );
        let doc = r.d.store.doc(PENDING, "a_spoo").unwrap();
        assert_eq!(doc["failed"], json!({ "said": "spoo has no curl or wget on its PATH.", "fix": "Install one of them there, then add again." }));
        assert!(doc.get("placeId").is_none());
        assert_eq!(*r.mint.spent.borrow(), vec!["CODE1".to_string()]);
        assert!(awaiting_keys(&r.d).is_empty());
    });
}

// TS: places-install.test.ts:407 "keeps the kind a refused login carries ..."; places-install.test.ts:181 (the error leaves with its kind)
// spec: A15
#[test]
fn add_failure_login_refused_kind_rides_job_and_error() {
    run(async {
        let refusal = HostError { kind: Some(Kind::Other("login".into())), ..plain("maya@box: Permission denied (publickey).") };
        let rf = refusal.clone();
        let r = rig(inst!([rf] |rn| { let _ = rn; Err(rf) }), |_| {});
        let e = fail_add(&r, ask(|a| {
            a.add_id = Some("a_maya".into());
            a.address = "maya@box".into();
        }))
        .await;
        assert_eq!(e, refusal);
        assert_eq!(
            adds_json(&r.d),
            vec![json!({ "addId": "a_maya", "address": "maya@box", "startedAt": ISO, "state": "failed", "said": "maya@box: Permission denied (publickey).", "kind": "login",
                "steps": [{ "step": "connect", "state": "failed", "note": "maya@box: Permission denied (publickey)." }] })]
        );
        // Any other refusal leaves without a kind.
        let r = rig(inst!([] |rn| { let _ = rn; Err(plain("root@spoo runs zsh as root's shell")) }), |_| {});
        let e = fail_add(&r, ask(|_| {})).await;
        assert_eq!(e.kind, None);
        assert!(job_of(&r.d, r.d.manage.adds()[0].add_id.as_str()).get("kind").is_none());
    });
}

// TS: places-install.test.ts:392 "keeps the key a computer never met answered with, under its kind, so the app's Trust reads it off the record"
// spec: A15
// GAP: `HostError` has no field for the `hostKey` the TS stamps on the error beside its kind, so the half that puts the
// key on the job cannot be written; this pins the kind and that no key is invented where the error carries none.
#[test]
fn add_failure_host_key_rides_job() {
    run(async {
        let refusal = HostError { kind: Some(Kind::HostKey), ..plain("maya@box has never been reached") };
        let rf = refusal.clone();
        let r = rig(inst!([rf] |rn| { let _ = rn; Err(rf) }), |_| {});
        let e = fail_add(&r, ask(|a| {
            a.add_id = Some("a_maya".into());
            a.address = "maya@box".into();
        }))
        .await;
        assert_eq!(e, refusal);
        let job = job_of(&r.d, "a_maya");
        assert_eq!(job["kind"], "host-key");
        assert_eq!(job["state"], "failed");
        assert_eq!(job["said"], "maya@box has never been reached");
        assert!(job.get("hostKey").is_none());
    });
}

// TS: places-install.test.ts:472 "cuts the failed step's note to the length a log line keeps, and says the cut"
// spec: A15
#[test]
fn add_failure_note_cut_to_four_hundred() {
    run(async {
        let long = format!("root@10.0.0.9: {}", "y".repeat(5000));
        let l = long.clone();
        let r = rig(
            inst!([l] |rn| {
                rn.st(PlaceAddStep::Connect, StageState::Running, None);
                Err(plain(&format!("{l}\nsecond line")))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_long".into()))).await;
        let cut = cut_line(&long);
        let job = job_of(&r.d, "a_long");
        assert_eq!(job["steps"].as_array().unwrap().last().unwrap(), &json!({ "step": "connect", "state": "failed", "note": cut }));
        assert_eq!(stage_json(&r.d).last().unwrap(), &json!({ "addId": "a_long", "step": "connect", "state": "failed", "note": cut }));
        assert_eq!(job["said"], format!("{cut}\nsecond line"));
        assert_eq!(r.d.store.doc(PENDING, "a_long").unwrap()["failed"], json!({ "said": format!("{cut}\nsecond line") }));
    });
}

// TS: places-install.test.ts:455 "keeps the box's own lines under a failed add cut to the length a log line keeps, and says each cut"
// spec: A15, A10
#[test]
fn add_failed_job_keeps_box_lines_cut() {
    run(async {
        let long = format!("agent: {}", "x".repeat(5000));
        let lines = vec![long.clone(), "agent: dial refused".to_string()];
        let r = rig(
            inst!([] |rn| { let _ = rn; Ok(PlaceInstalled { name: "box".into(), ssh: Some("root@10.0.0.9".into()), ..Default::default() }) }),
            |s| {
                s.opts = Box::new(|o| o.join_wait_ms = Some(50));
                s.wiring = Box::new(move |w| w.log = Some(Rc::new(Log(Box::new(move |_| Ok(lines.clone()))))));
            },
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| a.add_id = Some("a_box".into())), NOW).await });
        pass(&r.d.clock, 60).await;
        let e = h.await.unwrap().unwrap_err();
        assert_eq!(e.message, format!("{}\n{}\nagent: dial refused", no_link("box"), long));
        let said = job_of(&r.d, "a_box")["said"].as_str().unwrap().to_string();
        assert_eq!(said.split('\n').collect::<Vec<_>>(), [no_link("box"), cut_line(&long), "agent: dial refused".to_string()]);
    });
}

// spec: A15 (a kept refusal keeps eleven lines and says how many it cut)
#[test]
fn add_failed_job_said_keeps_eleven_lines() {
    run(async {
        let lines: Vec<String> = (1..=12).map(|n| format!("line {n}")).collect();
        let l = lines.clone();
        let r = rig(inst!([l] |rn| { let _ = rn; Err(plain(&l.join("\n"))) }), |_| {});
        fail_add(&r, ask(|a| a.add_id = Some("a_many".into()))).await;
        let said = job_of(&r.d, "a_many")["said"].as_str().unwrap().to_string();
        let mut want: Vec<String> = lines[..11].to_vec();
        want.push("(cut 1 lines)".into());
        assert_eq!(said.split('\n').map(String::from).collect::<Vec<_>>(), want);
        assert_eq!(stage_json(&r.d).last().unwrap()["note"], "line 1");
    });
}

// TS: places-install.test.ts:261 "drops the record a join made when the install it landed in failed, since that install took the join back off the box"
// spec: A15
#[test]
fn add_taken_back_forgets_record() {
    run(async {
        let said = "spoo connected back but its agent did not start: systemctl exited 1; nothing this add put on it is left there";
        let taken = Rc::new(Cell::new(true));
        let sock: Rc<RefCell<Option<Rc<FakeSocket>>>> = Rc::default();
        let (t, sk) = (taken.clone(), sock.clone());
        let r = rig(
            inst!([t, sk] |rn| {
                let socket = rn.land(joined("spoo", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                *sk.borrow_mut() = socket;
                Err(if t.get() { taken_back(said) } else { plain(said) })
            }),
            |_| {},
        );
        let e = fail_add(&r, ask(|a| a.address = "root@spoo".into())).await;
        assert_eq!(e.message, said);
        assert!(r.d.stored(PLACE).is_none());
        assert!(!r.d.ctx.kept.borrow().contains_key(PLACE));
        assert_eq!(r.d.event_types(), ["place.removed"]);
        assert_eq!(sock.borrow().as_ref().unwrap().closed.borrow().as_ref().map(|c| c.1.clone()), Some("removed from this host".to_string()));
        assert!(r.d.ctx.live.borrow().get(PLACE).is_none());
        // The pending add carries the refusal; the computer it made is gone.
        assert_eq!(r.d.pending_docs()[0]["failed"], json!({ "said": said }));

        // An install that could not take its join back keeps the record, which is the road a remove sweeps by.
        taken.set(false);
        let r = rig(
            inst!([taken] |rn| {
                rn.land(joined("spoo", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                Err(if taken.get() { taken_back(said) } else { plain(said) })
            }),
            |_| {},
        );
        let e = fail_add(&r, ask(|a| a.address = "root@spoo".into())).await;
        assert_eq!(e.message, said);
        assert!(r.d.stored(PLACE).is_some());
        assert!(r.d.event_types().is_empty());
        assert!(r.d.ctx.live.borrow().get(PLACE).is_some());
    });
}

// TS: places-link.test.ts:376 "lets the forward go when the add that stood it left no record"
// spec: A15
#[test]
fn add_failure_releases_back_without_record() {
    run(async {
        let back = Rc::new(Back::default());
        let b = back.clone();
        let r = rig(
            inst!([] |rn| { let _ = rn; Ok(PlaceInstalled { name: "spoo".into(), ssh: Some("root@spoo".into()), back: Some(back_at(14621)), ..Default::default() }) }),
            |s| {
                s.opts = Box::new(|o| o.join_wait_ms = Some(30));
                s.wiring = Box::new(move |w| w.back = Some(b));
            },
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| {
            a.address = "spoo".into();
            a.door_port = Some(4640);
        }), NOW).await });
        pass(&r.d.clock, 40).await;
        assert_eq!(h.await.unwrap().unwrap_err().message, no_link("spoo"));
        assert_eq!(*back.log.borrow(), vec!["release root@spoo".to_string()]);
    });
}

// spec: A15 (a record that dials back through the forward keeps it held)
#[test]
fn add_failure_holds_back_through_the_landed_record() {
    run(async {
        let back = Rc::new(Back::default());
        let b = back.clone();
        let r = rig(
            inst!([] |rn| {
                rn.land(joined("spoo", DOOR_URL), Some(FakeReach::answering(vec![]))).await;
                rn.hand.get().ctx.live.borrow_mut().shift_remove(PLACE);
                Ok(PlaceInstalled { name: "spoo".into(), ssh: Some("root@spoo".into()), back: Some(back_at(14621)), ..Default::default() })
            }),
            |s| {
                s.opts = Box::new(|o| o.join_wait_ms = Some(30));
                s.wiring = Box::new(move |w| w.back = Some(b));
            },
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| a.address = "spoo".into()), NOW).await });
        pass(&r.d.clock, 40).await;
        assert_eq!(h.await.unwrap().unwrap_err().message, no_link("spoo"));
        assert_eq!(*back.log.borrow(), vec![format!("hold root@spoo /home/maya {PLACE}")]);
        assert_eq!(r.d.stored(PLACE).unwrap()["road"], json!({ "ssh": "root@spoo", "back": { "boxPort": 14621 } }));
    });
}

// spec: restart
#[test]
fn restart_keeps_failed_pending_drops_jobs() {
    run(async {
        let r = rig(
            inst!([] |rn| {
                rn.req.before_deploy.clone().unwrap().before_deploy("rm -rf /root/.wsp".into(), "root@10.0.0.9".into(), Some("ssh-ed25519 AAAA".into())).await.unwrap();
                Err(refused("spoo has no curl or wget on its PATH", "Install one of them there, then add again."))
            }),
            |_| {},
        );
        fail_add(&r, ask(|a| a.add_id = Some("a_r".into()))).await;
        let docs = r.d.pending_docs();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0]["step"], "wsp");
        assert_eq!(docs[0]["undo"], "rm -rf /root/.wsp");
        assert_eq!(docs[0]["login"], "root@10.0.0.9");
        assert_eq!(docs[0]["hostKey"], "ssh-ed25519 AAAA");
        assert_eq!(docs[0]["failed"]["said"], "spoo has no curl or wget on its PATH.");

        let after = door(|s| {
            s.records = vec![];
            s.pending = docs.clone();
        });
        assert!(after.manage.adds().is_empty());
        assert!(awaiting_keys(&after).is_empty());
        let read: Vec<Value> = after.views.pending_records().await.unwrap().iter().map(js).collect();
        assert_eq!(read, docs);
    });
}

// ================================================================ adds

// TS: places-install.test.ts:524 "keeps every add still running and the last twenty that finished"
// spec: AD1
#[test]
fn adds_in_order() {
    run(async {
        let r = rig(inst!([] |rn| { let _ = rn; Err(plain("root@10.0.0.9 did not answer on port 22")) }), |_| {});
        for n in 0..22 {
            fail_add(&r, ask(|a| a.add_id = Some(format!("a_{n}")))).await;
        }
        let ids: Vec<String> = r.d.manage.adds().iter().map(|j| j.add_id.clone()).collect();
        assert_eq!(ids, (2..22).map(|n| format!("a_{n}")).collect::<Vec<_>>());
    });
}

// spec: AD1, R16 (a running add is never trimmed)
#[test]
fn adds_keep_running_beside_the_last_twenty() {
    run(async {
        let gate = Rc::new(Notify::new());
        let g = gate.clone();
        let r = rig(
            inst!([g] |rn| {
                if rn.req.address == "root@10.0.0.8" {
                    g.notified().await;
                }
                Err(plain("did not answer on port 22"))
            }),
            |_| {},
        );
        let m = r.d.manage.clone();
        let h = spawn_local(async move { m.add(ask(|a| {
            a.add_id = Some("a_run".into());
            a.address = "root@10.0.0.8".into();
        }), NOW).await });
        until("the first add runs", || r.d.manage.adds().len() == 1).await;
        for n in 0..22 {
            fail_add(&r, ask(|a| a.add_id = Some(format!("a_{n}")))).await;
        }
        let ids: Vec<String> = r.d.manage.adds().iter().map(|j| j.add_id.clone()).collect();
        let mut want = vec!["a_run".to_string()];
        want.extend((2..22).map(|n| format!("a_{n}")));
        assert_eq!(ids, want);
        assert_eq!(job_of(&r.d, "a_run")["state"], "running");
        gate.notify_one();
        h.await.unwrap().unwrap_err();
    });
}

// spec: AD2
#[test]
fn report_and_home_of() {
    run(async {
        let d = door(|_| {});
        let report = d.manage.report_of(PLACE).await.unwrap().expect("a report");
        assert_eq!(js(&report), at("/record/report"));
        assert_eq!(d.manage.home_of(PLACE).await.unwrap().as_deref(), Some("/home/maya"));
        assert!(d.manage.report_of("p_nope").await.unwrap().is_none());
        assert_eq!(d.manage.home_of("p_nope").await.unwrap(), None);
    });
}

// ================================================================ dial

/// A dial that answers when the test lets it.
struct GatedDial {
    gate: Rc<Notify>,
    logins: Rc<RefCell<Vec<PlaceLogin>>>,
}

#[async_trait(?Send)]
impl PlaceDialler for GatedDial {
    async fn dial(&self, login: &PlaceLogin) -> PlaceResult<()> {
        self.logins.borrow_mut().push(login.clone());
        self.gate.notified().await;
        Ok(())
    }
}

/// A road that neither answers nor refuses.
struct HangDial;

#[async_trait(?Send)]
impl PlaceDialler for HangDial {
    async fn dial(&self, _: &PlaceLogin) -> PlaceResult<()> {
        std::future::pending().await
    }
}

/// A link whose every request hangs.
struct HangReach;

#[async_trait(?Send)]
impl DaemonReach for HangReach {
    async fn ready(&self) -> Result<(), ReachError> {
        std::future::pending().await
    }
    async fn request(&self, _: &str, _: Map<String, Value>) -> Result<Map<String, Value>, ReachError> {
        std::future::pending().await
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {}
}

fn running_two(s: &mut Seed) {
    s.recording.running = RefCell::new(indexmap::IndexMap::from([(PLACE.to_string(), 2u64)]));
}

// spec: D1
#[test]
fn dial_unknown_refuses_with_names() {
    run(async {
        let d = door(|_| {});
        let e = d.manage.dial("p_nope", NOW).await.unwrap_err();
        assert_eq!(err_json(&e), at("/dial/unknown"));
        assert_eq!(e.message, "no place named p_nope; you have old-macbook");
        let d = door(|s| s.records = vec![record_json(json!({})), record_json(json!({ "id": PLACE2, "name": "second" }))]);
        let e = d.manage.dial("p_nope", NOW).await.unwrap_err();
        assert_eq!(e.message, "no place named p_nope; you have old-macbook, second");
        assert_eq!(e.kind, None);
    });
}

// TS: places-link.test.ts:24 "sends one frame over the link a computer is holding and answers how long it took"
// spec: D2, D3, D4
#[test]
fn dial_link_ping_moves_last_seen() {
    run(async {
        let d = door(running_two);
        let clock = d.clock.clone();
        let reach = FakeReach::new(move |op, _| {
            assert_eq!(op, "ping");
            clock.advance(37);
            Ok(Map::new())
        });
        link(&d, PLACE, reach.clone());
        let got = d.manage.dial(PLACE, NOW).await.unwrap();
        assert_eq!(js(&got.dialled), json!({ "at": ISO, "answered": true, "roundTripMs": 37 }));
        assert_eq!(got.line, "old-macbook answered in 37 ms.");
        let place = js(&got.place);
        assert_eq!(place["dialled"], js(&got.dialled));
        assert_eq!(place["present"], true);
        assert_eq!(place["lastSeenAt"], ISO);
        assert_eq!(place["running"], 2);
        assert_eq!(*reach.asked.borrow(), vec![("ping".to_string(), json!({}))]);
        let stored = d.stored(PLACE).unwrap();
        assert_eq!(stored["dialled"], js(&got.dialled));
        assert_eq!(stored["lastSeenAt"], ISO);
    });
}

// spec: D2, D3 (a link that does not answer leaves the silence dated as it was)
#[test]
fn dial_link_failure_says_what_it_said_and_keeps_last_seen() {
    run(async {
        let d = door(|_| {});
        link(&d, PLACE, FakeReach::new(|_, _| Err(reach_err("socket hang up"))));
        let got = d.manage.dial(PLACE, NOW).await.unwrap();
        assert_eq!(js(&got.dialled), json!({ "at": ISO, "answered": false, "said": "socket hang up" }));
        assert_eq!(got.line, "socket hang up");
        assert_eq!(d.stored(PLACE).unwrap()["lastSeenAt"], "2026-10-02T00:00:00.000Z");
        assert_eq!(d.stored(PLACE).unwrap()["dialled"], js(&got.dialled));
    });
}

// TS: places-link.test.ts:43 "logs in over the road the computer was installed on when it is holding no link, and says the computer is on"
// spec: D2, D3
#[test]
fn dial_ssh_answer_keeps_last_seen() {
    run(async {
        let logins: Rc<RefCell<Vec<PlaceLogin>>> = Rc::default();
        let l = logins.clone();
        let d = door(|s| {
            running_two(s);
            s.wiring = Box::new(move |w| w.dial = Some(Rc::new(Dial(Box::new(move |login| { l.borrow_mut().push(login.clone()); Ok(()) })))));
        });
        let got = d.manage.dial(PLACE, NOW).await;
        assert_eq!(outcome(&got), at("/dial/sshOk"));
        assert_eq!(got.unwrap().line, "maya@10.0.0.5 answered over ssh in 0 ms, so the computer is on; the agent on it is not dialling this host.");
        assert_eq!(*logins.borrow(), vec![PlaceLogin { ssh: "maya@10.0.0.5".into(), key_path: Some("/home/dev/.ssh/id".into()), host_key: Some("ssh-ed25519 AAAA".into()) }]);
        assert_eq!(d.stored(PLACE).unwrap(), at("/dial/sshOkRecord"));
    });
}

// TS: places-link.test.ts:164 "hands back ssh's own sentence when the login is refused, and keeps it on the row"
// spec: D2, D4
#[test]
fn dial_ssh_refused_says_ssh_own_sentence() {
    run(async {
        let d = door(|s| {
            running_two(s);
            s.wiring = Box::new(|w| w.dial = Some(Rc::new(Dial(Box::new(|_| Err(plain("ssh: connect to host 10.0.0.5 port 22: Connection refused")))))));
        });
        let got = d.manage.dial(PLACE, NOW).await;
        assert_eq!(outcome(&got), at("/dial/sshFails"));
        assert_eq!(d.stored(PLACE).unwrap()["dialled"]["said"], "ssh: connect to host 10.0.0.5 port 22: Connection refused");
    });
}

// TS: places-link.test.ts:235 "says there is no road at all on a computer that joined by typing a code and is holding no link"
// spec: D2
#[test]
fn dial_no_road_line() {
    run(async {
        let d = door(|s| {
            running_two(s);
            s.records = vec![record_json(json!({ "road": null }))];
            s.wiring = Box::new(|w| w.dial = Some(Rc::new(Dial(Box::new(|_| panic!("nothing is dialled where there is no road"))))));
        });
        let got = d.manage.dial(PLACE, NOW).await;
        assert_eq!(outcome(&got), at("/dial/noRoad"));
        assert_eq!(got.unwrap().line, NO_DIAL);

        // An ssh road with no dial wired is no road either, and nothing is installed.
        let d = door(|_| {});
        let got = d.manage.dial(PLACE, NOW).await.unwrap();
        assert_eq!(js(&got.dialled), json!({ "at": ISO, "answered": false, "said": NO_DIAL }));
        assert_eq!(got.line, NO_DIAL);
    });
}

// TS: places-link.test.ts:206 "bounds the dial itself, so a road that hangs rather than refusing still answers the hand that pressed"
// spec: D2
#[test]
fn dial_link_bound_is_said() {
    run(async {
        // The ssh road.
        let d = door(|s| {
            s.opts = Box::new(|o| o.dial_wait_ms = Some(3000));
            s.wiring = Box::new(|w| w.dial = Some(Rc::new(HangDial)));
        });
        let m = d.manage.clone();
        let h = spawn_local(async move { m.dial(PLACE, NOW).await });
        pass(&d.clock, 3100).await;
        let got = h.await.unwrap().unwrap();
        assert_eq!(js(&got.dialled), json!({ "at": ISO, "answered": false, "said": "ssh maya@10.0.0.5 was not answered in 3s" }));
        assert_eq!(got.line, "ssh maya@10.0.0.5 was not answered in 3s");

        // The link road.
        let d = door(|s| s.opts = Box::new(|o| o.dial_wait_ms = Some(3000)));
        d.ctx.live.borrow_mut().insert(PLACE.to_string(), Live { socket: Rc::new(FakeSocket::default()), reach: Rc::new(HangReach), seen: Box::new(|| {}), forward: None });
        let m = d.manage.clone();
        let h = spawn_local(async move { m.dial(PLACE, NOW).await });
        pass(&d.clock, 3100).await;
        let got = h.await.unwrap().unwrap();
        assert_eq!(js(&got.dialled), json!({ "at": ISO, "answered": false, "said": "ping on old-macbook was not answered in 3s" }));
        assert_eq!(d.stored(PLACE).unwrap()["lastSeenAt"], "2026-10-02T00:00:00.000Z");
    });
}

// TS: places-link.test.ts:77 "keeps a login that landed on that computer while the dial was out"
// spec: D4
#[test]
fn dial_keeps_a_login_that_landed_during_the_dial() {
    run(async {
        let gate = Rc::new(Notify::new());
        let g = gate.clone();
        let logins: Rc<RefCell<Vec<PlaceLogin>>> = Rc::default();
        let l = logins.clone();
        let d = door(move |s| {
            s.records = vec![record_json(json!({ "report": js(&report(json!({ "agents": ["codex"], "logins": [] }))) }))];
            s.wiring = Box::new(move |w| w.dial = Some(Rc::new(GatedDial { gate: g, logins: l })));
        });
        let m = d.manage.clone();
        let h = spawn_local(async move { m.dial(PLACE, NOW).await });
        until("the dial is out", || !logins.borrow().is_empty()).await;
        d.records.login_listed(PLACE, "codex").await.unwrap();
        gate.notify_one();
        let got = h.await.unwrap().unwrap();
        assert_eq!(js(&got.place)["signIns"], json!({ "codex": "signed-in" }));
        assert_eq!(d.stored(PLACE).unwrap()["report"]["logins"], json!(["codex/auth.json"]));
        assert_eq!(d.stored(PLACE).unwrap()["dialled"], js(&got.dialled));
    });
}

// spec: D4 (the record went meanwhile: the answer is built from the record as read, and nothing is written back)
#[test]
fn dial_record_gone_meanwhile_answers_from_the_read_record() {
    run(async {
        let gate = Rc::new(Notify::new());
        let g = gate.clone();
        let logins: Rc<RefCell<Vec<PlaceLogin>>> = Rc::default();
        let l = logins.clone();
        let d = door(move |s| s.wiring = Box::new(move |w| w.dial = Some(Rc::new(GatedDial { gate: g, logins: l }))));
        let m = d.manage.clone();
        let h = spawn_local(async move { m.dial(PLACE, NOW).await });
        until("the dial is out", || !logins.borrow().is_empty()).await;
        d.views.forget(PLACE).await.unwrap();
        gate.notify_one();
        let got = h.await.unwrap().unwrap();
        assert_eq!(js(&got.dialled), json!({ "at": ISO, "answered": true, "roundTripMs": 0 }));
        let place = js(&got.place);
        assert_eq!(place["id"], PLACE);
        assert_eq!(place["name"], "old-macbook");
        assert_eq!(place["dialled"], js(&got.dialled));
        assert!(d.stored(PLACE).is_none());
        assert!(!d.ctx.kept.borrow().contains_key(PLACE));
    });
}

// ================================================================ road

// TS: places-door.test.ts:16 "is refused with the place's name while that computer is not connected"
// spec: RD1
#[test]
fn road_not_linked_absent() {
    run(async {
        let d = door(|_| {});
        let e = d.manage.road(PLACE).await.unwrap_err();
        assert_eq!(e.message, format!("old-macbook {NOT_ANSWERING}"));
        let e = d.manage.road("p_nope").await.unwrap_err();
        assert_eq!(e.message, format!("p_nope {NOT_ANSWERING}"));
    });
}

// TS: places-door.test.ts:24 "is refused with its own sentence while that computer has not said which port its daemon bound"
// spec: RD1
#[test]
fn road_no_daemon_port() {
    run(async {
        let d = door(|_| {});
        link(&d, PLACE, FakeReach::answering(vec![]));
        let e = d.manage.road(PLACE).await.unwrap_err();
        assert_eq!(e.message, NO_PORT);
        assert!(d.ctx.live.borrow().get(PLACE).unwrap().forward.is_none());
    });
}

// TS: places-door.test.ts:32 "goes with the link that carried it ..." (the port the first ask opened and every later ask joins)
// spec: RD2
#[test]
fn road_one_forward_per_link_reopens_after_failure() {
    run(async {
        let d = door(|s| s.records = vec![record_json(json!({ "report": js(&report(json!({ "daemonPort": 4321 }))) }))]);
        let reach = FakeReach::new(|_, _| Ok(Map::new()));
        link(&d, PLACE, reach.clone());
        let failing: HeldForward = async { Err::<Rc<PlaceForward>, HostError>(plain("this computer offered no free port for the road to a place")) }.boxed_local().shared();
        d.ctx.live.borrow_mut().get_mut(PLACE).unwrap().forward = Some(failing);
        // A failed open passes on, and is taken off the entry so the next ask opens again.
        let e = d.manage.road(PLACE).await.unwrap_err();
        assert_eq!(e.message, "this computer offered no free port for the road to a place");
        assert!(d.ctx.live.borrow().get(PLACE).unwrap().forward.is_none());
        let first = d.manage.road(PLACE).await.unwrap();
        assert!(first > 0);
        assert!(d.ctx.live.borrow().get(PLACE).unwrap().forward.is_some());
        let (a, b) = futures::join!(d.manage.road(PLACE), d.manage.road(PLACE));
        assert_eq!((a.unwrap(), b.unwrap()), (first, first));
        // The port carries to the daemon port the computer reported, over the link that opened it.
        let _conn = tokio::net::TcpStream::connect(("127.0.0.1", first)).await.expect("the forward listens");
        until("the tunnel opens", || reach.asked.borrow().iter().any(|(op, _)| op == "tunnel.open")).await;
        let opened = reach.asked.borrow().iter().find(|(op, _)| op == "tunnel.open").cloned().unwrap();
        assert_eq!(opened.1["port"], 4321);
    });
}

// spec: RD2 (a failed open is taken off only the entry that asked for it)
#[test]
fn road_failed_open_leaves_a_newer_link_alone() {
    run(async {
        let d = door(|s| s.records = vec![record_json(json!({ "report": js(&report(json!({ "daemonPort": 4321 }))) }))]);
        link(&d, PLACE, FakeReach::new(|_, _| Ok(Map::new())));
        let (tx, rx) = oneshot::channel::<Result<Rc<PlaceForward>, HostError>>();
        let held: HeldForward = async move { rx.await.unwrap() }.boxed_local().shared();
        d.ctx.live.borrow_mut().get_mut(PLACE).unwrap().forward = Some(held);
        let m = d.manage.clone();
        let h = spawn_local(async move { m.road(PLACE).await });
        settle().await;
        // The computer dials again and the new link opens its own forward.
        link(&d, PLACE, FakeReach::new(|_, _| Ok(Map::new())));
        let port = d.manage.road(PLACE).await.unwrap();
        assert!(port > 0);
        tx.send(Err(plain("boom"))).ok();
        assert_eq!(h.await.unwrap().unwrap_err().message, "boom");
        assert!(d.ctx.live.borrow().get(PLACE).unwrap().forward.is_some());
    });
}

// ================================================================ exec

// spec: E1
#[test]
fn exec_not_linked_place_absent() {
    run(async {
        let d = door(|_| {});
        let e = d.manage.exec(PLACE, "ls", PlaceExecOpts::default()).await.unwrap_err();
        assert_eq!(e.message, format!("old-macbook {NOT_ANSWERING}"));
        assert_eq!(e.kind, absent_kind());
        let e = d.manage.exec("p_nope", "ls", PlaceExecOpts::default()).await.unwrap_err();
        assert_eq!(e.message, format!("p_nope {NOT_ANSWERING}"));
        assert_eq!(e.kind, absent_kind());
    });
}

// spec: E2, E3
#[test]
fn exec_stdin_base64_and_defaults() {
    run(async {
        let d = door(|_| {});
        let said = FakeReach::new(|_, _| Ok(obj(json!({ "exitCode": 3, "stdout": "out", "stderr": "err" }))));
        link(&d, PLACE, said.clone());
        let got = d.manage.exec(PLACE, "cat", PlaceExecOpts { timeout_ms: Some(5000), stdin: Some(b"hi\n".to_vec()) }).await.unwrap();
        assert_eq!(js(&got), json!({ "exitCode": 3, "stdout": "out", "stderr": "err" }));
        assert_eq!(*said.asked.borrow(), vec![("exec".to_string(), json!({ "cmd": "cat", "timeoutMs": 5000, "stdin": "aGkK" }))]);

        let bare = FakeReach::new(|_, _| Ok(Map::new()));
        link(&d, PLACE, bare.clone());
        let got = d.manage.exec(PLACE, "ls", PlaceExecOpts::default()).await.unwrap();
        assert_eq!(js(&got), json!({ "exitCode": -1, "stdout": "", "stderr": "" }));
        assert_eq!(*bare.asked.borrow(), vec![("exec".to_string(), json!({ "cmd": "ls" }))]);
    });
}

// spec: E2
#[test]
fn exec_failure_on_open_socket_passes_on() {
    run(async {
        let d = door(|_| {});
        let own = ReachError { message: "the daemon refused that command".into(), kind: None, status: Some(400.0), code: Some("refused".into()) };
        let o = own.clone();
        link(&d, PLACE, FakeReach::new(move |_, _| Err(o.clone())));
        let e = d.manage.exec(PLACE, "ls", PlaceExecOpts::default()).await.unwrap_err();
        assert_eq!(e.message, own.message);
    });
}

// spec: E2
#[test]
fn exec_failure_on_a_closed_socket_is_absent() {
    run(async {
        let d = door(|_| {});
        let socket = link(&d, PLACE, FakeReach::new(|_, _| Err(reach_err("socket closed"))));
        socket.0.shut.set(true);
        let e = d.manage.exec(PLACE, "ls", PlaceExecOpts::default()).await.unwrap_err();
        assert_eq!(e.message, format!("old-macbook {NOT_ANSWERING}"));
        assert_eq!(e.kind, absent_kind());
    });
}

// spec: E2
#[test]
fn exec_failure_after_link_went_is_absent() {
    run(async {
        // The link goes while the frame is out.
        let d = door(|_| {});
        let ctx = d.ctx.clone();
        link(&d, PLACE, FakeReach::new(move |_, _| {
            ctx.live.borrow_mut().shift_remove(PLACE);
            Err(reach_err("socket closed"))
        }));
        let e = d.manage.exec(PLACE, "ls", PlaceExecOpts::default()).await.unwrap_err();
        assert_eq!(e.message, format!("old-macbook {NOT_ANSWERING}"));
        assert_eq!(e.kind, absent_kind());

        // The link is replaced by another while the frame is out: the answer of the old one is not the computer's.
        let d = door(|_| {});
        let ctx = d.ctx.clone();
        link(&d, PLACE, FakeReach::new(move |_, _| {
            link_on(&ctx, PLACE, FakeReach::answering(vec![]));
            Err(reach_err("socket closed"))
        }));
        let e = d.manage.exec(PLACE, "ls", PlaceExecOpts::default()).await.unwrap_err();
        assert_eq!(e.message, format!("old-macbook {NOT_ANSWERING}"));
        assert_eq!(e.kind, absent_kind());
    });
}
