// SPDX-License-Identifier: AGPL-3.0-only
//! Spec `spec/places_manage_settings.md`: list, rows, set, find, pending, choose, setUp, the recipe roads, skip,
//! estimate, setupLog, on and close of the place door's manage half. Expected values come from
//! `tests/fixtures/oracle/places_b.json` and `places_b_settings.json` (the TS door on fakes), the TS tests named in
//! each comment, and TS sentences written out literally (the wsp-words functions are todo bodies).
mod places_b_common;
use places_b_common::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_machines::places::helpers::listen_near;
use wsp_machines::places::manage::{SetupStarted, SyncRead};
use wsp_machines::places::records::{AwaitingInstall, LocalFut};
use wsp_machines::places::types::*;
use wsp_store::keys::new_place_key_pair;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PlaceSetAlso, PlaceSetup, PlaceSetupStep};
use wsp_wire::views::place_more::{CloudLimits, PlaceSettingWord, PlaceSettingsAsk, PlaceView};
use wsp_words::exit::HostError;

const NAMED_FIX: &str = "Run wsp computers to read the ones this host holds.";
const SKIPPED_FOR_NOW: &str = "skipped for now; finish it from the computer's page in Settings";
const NOW_ISO: &str = "2023-11-14T22:13:20.000Z";

// ---------------------------------------------------------------- the second oracle

fn oracle2() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/places_b_settings.json")).expect("the settings oracle fixture");
    serde_json::from_str(&text).expect("the settings oracle parses")
}

fn at2(path: &str) -> Value {
    oracle2().pointer(path).cloned().unwrap_or_else(|| panic!("nothing in the settings oracle at {path}"))
}

// ---------------------------------------------------------------- helpers

fn ask(v: Value) -> PlaceSettingsAsk {
    serde_json::from_value(v).expect("a settings ask")
}

fn word(w: &str) -> PlaceSettingWord {
    serde_json::from_value(json!(w)).expect("a setting word")
}

fn also(v: Value) -> PlaceSetAlso {
    serde_json::from_value(v).expect("a set-also")
}

async fn set(d: &Door, id: &str, s: Value, reset: &[&str], a: Value) -> PlaceResult<PlaceView> {
    d.manage.set(id, ask(s), reset.iter().map(|w| word(w)).collect(), also(a)).await
}

/// The refusal an answer is, as the oracle writes one.
fn fails<T>(r: PlaceResult<T>) -> Value {
    err_json(&r.err().expect("a refusal"))
}

fn view(v: &PlaceView) -> Value {
    js(v)
}

/// The record the settings oracle's TS door held: the places b record with no applied rows and one plugin pick.
fn mine(over: Value) -> Value {
    let mut base = record_json(json!({ "applied": null }));
    merge(&mut base, json!({ "picks": js(&picks("web", &["lint"])) }));
    merge(&mut base, over);
    base
}

fn two(s: &mut Seed) {
    s.records = vec![mine(json!({})), mine(json!({ "id": PLACE2, "name": "box2" }))];
}

fn pending_of(id: &str, place_id: &str, name: &str) -> Value {
    pending_json(id, "10.0.0.5", json!({ "placeId": place_id, "name": name, "startedAt": NOW_ISO, "choices": js(&picks("web", &["lint"])) }))
}

fn setup_json(state: &str, add_id: &str, waiting: Value) -> Value {
    json!({ "state": state, "addId": add_id, "startedAt": NOW_ISO, "steps": [], "waiting": waiting })
}

fn wait_json(row: &str, label: &str) -> Value {
    json!({ "row": row, "label": label, "url": "https://auth.example/x", "code": "C1", "expiresAt": NOW_ISO, "state": "waiting" })
}

fn applied_row(id: &str, outcome: &str, extra: Value) -> Value {
    let mut r = json!({ "id": id, "label": id.to_uppercase(), "outcome": outcome });
    merge(&mut r, extra);
    r
}

fn setup_of(v: Value) -> PlaceSetup {
    serde_json::from_value(v).expect("a setup")
}

fn log_lines(d: &Door) -> Vec<String> {
    d.setup.calls()
}

fn no_writes(d: &Door) {
    assert_eq!(d.store.writes.borrow().clone(), Vec::<String>::new(), "nothing is written");
}

/// A place file as the held read prints it, naming `id` and `key`.
fn held_stdout(id: &str, key: &str) -> String {
    format!("\nWSP_PLACE_HELD\n{}\n", serde_json::to_string_pretty(&json!({ "placeId": id, "name": "x", "hostName": "zingzys-mac", "hostUrls": [], "hostPublicKey": key, "keyPath": "/k" })).unwrap())
}

type Seen = Rc<RefCell<Vec<(PlaceLogin, String, u64, Option<String>)>>>;

fn reading(seen: &Seen, answer: impl Fn() -> PlaceResult<RunOver> + 'static) -> Rc<dyn PlaceRunOver> {
    let seen = seen.clone();
    Rc::new(RunOverRoad(
        Box::new(move |l, s, ms, pw| {
            seen.borrow_mut().push((l.clone(), s.to_string(), ms, pw.map(String::from)));
            answer()
        }),
        RefCell::new(vec![]),
    ))
}

fn read_ok(stdout: String) -> impl Fn() -> PlaceResult<RunOver> {
    move || Ok(RunOver { exit_code: 0, stdout: stdout.clone(), stderr: String::new() })
}

struct Resolver {
    asked: Cell<u32>,
    fail: Cell<bool>,
}

#[async_trait(?Send)]
impl RecipeResolver for Resolver {
    async fn resolve(&self, slug: &str) -> PlaceResult<Resolved> {
        self.asked.set(self.asked.get() + 1);
        if self.fail.get() {
            return Err(HostError::new("no such recipe".to_string()));
        }
        Ok(Resolved { file: picks(slug, &[]), items: IndexMap::new(), hash: "h2".into() })
    }
    async fn resolve_file(&self, file: &RecipeFile) -> PlaceResult<Resolved> {
        Ok(Resolved { file: file.clone(), items: IndexMap::new(), hash: "h2".into() })
    }
}

fn resolver() -> Rc<Resolver> {
    Rc::new(Resolver { asked: Cell::new(0), fail: Cell::new(false) })
}

fn with_resolver(s: &mut Seed, r: &Rc<Resolver>) {
    let r = r.clone();
    s.opts = Box::new(move |o| o.recipes = Some(Rc::new(move || Some(r.clone() as Rc<dyn RecipeResolver>))));
}

fn changes(keys: &[&str]) -> SyncRead {
    SyncRead { change_keys: keys.iter().map(|k| k.to_string()).collect() }
}

/// A stub backend offering `capacity`, answering a computer with room for two forks and two machines running.
fn capacity_backend() -> StubBackend {
    let mut b = StubBackend::new();
    b.offers.capacity = true;
    *b.capacity.lock().unwrap() = Some(Ok(json!({ "cores": 4, "memMb": 8192, "memRoomMb": 8192, "machineMemMb": 8192, "diskFreeBytes": 100_000_000_000u64, "images": [], "machines": { "running": 2, "paused": 1 } })));
    b
}

fn limits_json() -> CloudLimits {
    serde_json::from_value(json!({ "plan": { "trial": false, "machinesAtOnce": 3, "sizes": ["small"] }, "creditHours": 12.5 })).expect("limits")
}

fn rows_door() -> Door {
    door(|s| {
        s.recording.running.borrow_mut().insert(PLACE.into(), 2);
    })
}

// ================================================================ list, rows, find

// TS: places-settings.test.ts:25 "gives every row its cap: a joined computer the rule's off its shape, this computer the rule's off its own, a cloud 3 machines and $10 a day"
// spec: L1
#[test]
fn list_plain_rows_match_the_oracle() {
    run(async {
        let d = rows_door();
        let rows = d.manage.list(NOW).await.expect("list");
        assert_eq!(js(&rows), at("/rows/list"));
        let rows = d.manage.rows().await.expect("rows");
        assert_eq!(js(&rows), at("/rows/plain"));
        assert_eq!(rows.iter().map(|r| view(r)["id"].as_str().unwrap().to_string()).collect::<Vec<_>>(), vec!["here", PLACE, "boat"]);
    });
}

// TS: places-settings.test.ts:25, :35 (rows carry what a person set)
// spec: L2
#[test]
fn rows_carry_set_caps_and_the_mark() {
    run(async {
        let d = door(|s| {
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
            s.caps = vec![(PLACE.into(), json!({ "threads": 3, "napMs": null })), ("boat".into(), json!({ "machines": 2, "size": { "cpu": 4, "memMb": 8192 } }))];
            s.mark = Some(PLACE.into());
        });
        assert_eq!(js(&d.manage.rows().await.unwrap()), at("/rows/set"));
        let gone = door(|s| {
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
            s.mark = Some("p_00000000000000ff".into());
        });
        assert_eq!(js(&gone.manage.rows().await.unwrap()), at("/rows/goneMark"));
    });
}

// spec: L2 "rows_have_no_fork_room"
#[test]
fn rows_have_no_fork_room() {
    run(async {
        let d = rows_door();
        link(&d, PLACE, FakeReach::answering(vec![]));
        let backend = std::sync::Arc::new(capacity_backend());
        d.links.backends.borrow_mut().insert(PLACE.into(), backend.clone() as std::sync::Arc<dyn wsp_engine::machine::MachineBackend>);
        *d.recording.spent.borrow_mut() = Some(Ok(vec![SpentToday { place: "boat".into(), today_usd: 1.5 }]));
        let rows = d.manage.rows().await.unwrap();
        for r in &rows {
            assert!(view(r).get("forks").is_none(), "{} has no forks key", view(r)["id"]);
            assert!(view(r).get("spentTodayUsd").is_none(), "rows reads no spend");
        }
        assert!(backend.asked.lock().unwrap().is_empty(), "rows asks no computer for its room: {:?}", backend.asked.lock().unwrap());
    });
}

// spec: L1 "list_forks_spend_and_limits_on_rows"
#[test]
fn list_forks_spend_and_limits_on_rows() {
    run(async {
        let mut boat = StubBackend::new();
        boat.offers.place_limits = true;
        *boat.limits.lock().unwrap() = Some(Ok(Some(limits_json())));
        let providers = Rc::new(Providers { wired: "boat".into(), rows: IndexMap::from([("boat".to_string(), boat.arc())]) });
        let d = door(|s| {
            s.providers = Some(providers.clone());
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
            *s.recording.spent.borrow_mut() = Some(Ok(vec![SpentToday { place: "boat".into(), today_usd: 1.5 }]));
        });
        link(&d, PLACE, FakeReach::answering(vec![]));
        d.links.backends.borrow_mut().insert(PLACE.into(), capacity_backend().arc());

        let first = d.manage.list(NOW).await.expect("list");
        let rows: Vec<Value> = first.iter().map(view).collect();
        assert_eq!(rows[0]["id"], "here");
        assert!(rows[0].get("forks").is_none() && rows[0].get("spentTodayUsd").is_none());
        // The joined computer answered its room: two running, and a 4096 MB fork in 8192 MB of room is two more.
        assert_eq!(rows[1]["forks"], json!({ "running": 2, "room": 2 }));
        assert!(rows[1].get("spentTodayUsd").is_none());
        assert_eq!(rows[2]["id"], "boat");
        assert_eq!(rows[2]["spentTodayUsd"], json!(1.5));
        assert!(rows[2].get("plan").is_none(), "the first listing of a cloud never waits on its limits read");
        assert!(rows[2].get("forks").is_none());

        settle().await;
        let again: Vec<Value> = d.manage.list(NOW).await.unwrap().iter().map(view).collect();
        assert_eq!(again[2]["plan"], json!({ "trial": false, "machinesAtOnce": 3, "sizes": ["small"] }));
        assert_eq!(again[2]["creditHours"], json!(12.5));
        assert_eq!(again[2]["spentTodayUsd"], json!(1.5));
        assert_eq!(again[1]["forks"], json!({ "running": 2, "room": 2 }));
    });
}

// spec: L1 (a failed spend read and a failed capacity read are none, never a failed listing)
#[test]
fn list_survives_a_failed_spend_and_a_failed_capacity_read() {
    run(async {
        let d = door(|s| {
            *s.recording.spent.borrow_mut() = Some(Err("the cloud would not say".into()));
        });
        link(&d, PLACE, FakeReach::answering(vec![]));
        let b = capacity_backend();
        *b.capacity.lock().unwrap() = Some(Err("capacity read failed".into()));
        d.links.backends.borrow_mut().insert(PLACE.into(), b.arc());
        let rows: Vec<Value> = d.manage.list(NOW).await.expect("a failed read is none").iter().map(view).collect();
        assert_eq!(rows.len(), 3);
        assert!(rows[1].get("forks").is_none());
        assert!(rows[2].get("spentTodayUsd").is_none());
    });
}

// spec: L3 "find_by_id_or_shared_name"
#[test]
fn find_by_id_or_shared_name() {
    run(async {
        let d = door(|s| {
            s.records = vec![record_json(json!({})), record_json(json!({ "id": PLACE2 })), record_json(json!({ "id": "p_0000000000000003", "name": "box3" }))];
        });
        let ids = |v: Vec<PlaceRecord>| v.into_iter().map(|r| r.id).collect::<Vec<_>>();
        // TS: places-b oracle /pending/find
        assert_eq!(json!(ids(d.manage.find("old-macbook").await.unwrap())), at("/pending/find"));
        assert_eq!(ids(d.manage.find(PLACE2).await.unwrap()), vec![PLACE2]);
        assert_eq!(ids(d.manage.find("box3").await.unwrap()), vec!["p_0000000000000003"]);
        assert!(d.manage.find("nobody").await.unwrap().is_empty());
        assert!(d.manage.find("here").await.unwrap().is_empty(), "this computer is no record");
    });
}

// ================================================================ set: the row and the refusals

// TS: places-settings.test.ts:91 "refuses a number the row's kind does not take, a place this host does not hold and a set with no number, as usage, and writes nothing"
// spec: S1
#[test]
fn set_unknown_place_names_every_row() {
    run(async {
        let d = door(|_| {});
        assert_eq!(fails(set(&d, "p_nope", json!({ "threads": 2 }), &[], json!({})).await), at("/set/unknown"));
        assert_eq!(at("/set/unknown/err"), "no place named p_nope; you have zingzys-mac, old-macbook, boat");
        assert_eq!(at("/set/unknown/kind"), "usage");
        assert_eq!(at("/set/unknown/fix"), Value::Null);
        no_writes(&d);
        assert!(d.event_types().is_empty());
    });
}

// TS: places-settings.test.ts:91, :296 ; spec: S3, S4 "set_refused_setting_writes_nothing"
#[test]
fn set_refused_setting_writes_nothing() {
    run(async {
        let d = door(|_| {});
        let here = "zingzys-mac takes threads at once, turn limit, machines under a root thread and levels deep, not machines at once";
        let r = fails(set(&d, "here", json!({ "machines": 2 }), &[], json!({})).await);
        assert_eq!(r, json!({ "err": here, "kind": "usage", "fix": null }));
        // TS: places-settings.test.ts:296
        let nap = "zingzys-mac takes threads at once, turn limit, machines under a root thread and levels deep, not nap after";
        assert_eq!(fails(set(&d, "here", json!({ "napMs": 300_000 }), &[], json!({})).await), json!({ "err": nap, "kind": "usage", "fix": null }));
        // TS: places-b oracle /set/providerThreads
        assert_eq!(fails(set(&d, "boat", json!({ "threads": 2 }), &[], json!({})).await), at("/set/providerThreads"));
        // TS: places-settings.test.ts:71 set and reset of one number
        assert_eq!(
            fails(set(&d, PLACE, json!({ "threads": 2 }), &["threads"], json!({})).await),
            json!({ "err": "old-macbook: threads at once is both set and reset; name it once", "kind": "usage", "fix": null })
        );
        assert_eq!(fails(set(&d, PLACE, json!({ "threads": 2 }), &["threads"], json!({})).await), at2("/cases/bothSetAndReset"));
        // a set that names nothing
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({})).await), at2("/cases/emptySet"));
        assert_eq!(
            at2("/cases/emptySet/err"),
            "nothing to set on old-macbook: it takes threads at once, nap after, turn limit, machines under a root thread, levels deep, a new name and a new ssh login"
        );
        no_writes(&d);
        assert!(d.event_types().is_empty());
        assert!(d.naps.borrow().is_empty());
        assert_eq!(d.store.all(CAPS), Vec::<Value>::new());
    });
}

// spec: S4 (every check runs before any write: a bad setting beside a rename leaves the name alone)
#[test]
fn set_bad_setting_beside_a_rename_writes_nothing() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({}))]);
        let r = fails(set(&d, PLACE, json!({ "machines": 2 }), &[], json!({ "name": "renamed" })).await);
        assert_eq!(r, at2("/cases/badSettingWithRename"));
        assert_eq!(d.stored(PLACE).unwrap()["name"], "old-macbook");
        no_writes(&d);
    });
}

// TS: places-b oracle /set/renameToHere ; spec: S4 "set_rename_taken_refused"
#[test]
fn set_rename_taken_refused() {
    run(async {
        let d = door(|s| two(s));
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({ "name": "zingzys-mac" })).await), at("/set/renameToHere"));
        assert_eq!(at("/set/renameToHere/err"), "zingzys-mac is already the name of another computer. Pick a name no other computer goes by.");
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({ "name": "BOAT" })).await), at2("/cases/renameToProvider"));
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({ "name": "box2" })).await), at2("/cases/renameToOtherRecord"));
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({ "name": "   " })).await), at2("/cases/blankRename") /* the trimmed name is empty but still given */);
        assert_eq!(at2("/cases/blankRename/err"), "a computer's name cannot be blank. Give it a name with a letter or a digit in it.");
        assert_eq!(fails(set(&d, "here", json!({}), &[], json!({ "name": "x" })).await), at2("/cases/hereRename"));
        assert_eq!(fails(set(&d, "boat", json!({}), &[], json!({ "name": "x" })).await), json!({ "err": "boat keeps its own name. Rename one of the computers you added instead.", "kind": "usage", "fix": "Rename one of the computers you added instead." }));
        no_writes(&d);
        // its own name is no clash
        let ok = set(&d, PLACE, json!({}), &[], json!({ "name": "old-macbook" })).await.expect("its own name");
        assert_eq!(view(&ok)["name"], "old-macbook");
    });
}

// TS: places-b oracle /set/hereRecipe ; spec: S4 "set_recipe_on_here_refused"
#[test]
fn set_recipe_on_here_refused() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({ "recipe": null }))]);
        assert_eq!(fails(set(&d, "here", json!({}), &[], json!({ "recipe": "web" })).await), at("/set/hereRecipe"));
        assert_eq!(at("/set/hereRecipe/err"), "no place named here; you have old-macbook");
        // a cloud has no record either, and the names are the records' alone
        assert_eq!(fails(set(&d, "boat", json!({}), &[], json!({ "recipe": "web" })).await), at2("/cases/providerRecipe"));
        assert!(log_lines(&d).is_empty(), "no follow ran: {:?}", log_lines(&d));
        no_writes(&d);
    });
}

// spec: S4 (a login is refused for this computer, a cloud, a blank one and one that is no user@host)
#[test]
fn set_login_shape_refused() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({}))]);
        assert_eq!(fails(set(&d, "here", json!({}), &[], json!({ "ssh": "root@x" })).await), at2("/cases/hereSsh"));
        assert_eq!(fails(set(&d, "boat", json!({}), &[], json!({ "ssh": "root@x" })).await), at2("/cases/boatSsh"));
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({ "ssh": "nope" })).await), at2("/cases/badSsh"));
        assert_eq!(fails(set(&d, PLACE, json!({}), &[], json!({ "ssh": "  " })).await), at2("/cases/blankSsh"));
        assert_eq!(at2("/cases/badSsh/err"), "nope is not an ssh login like root@host. Write it as the user and the host ssh logs in with, root@hetzner for one.");
        no_writes(&d);
    });
}

// ================================================================ set: the login read

/// A door of two computers whose ssh road answers as `answer`; the key the door holds is the one a place file names.
fn login_door(answer: Option<Rc<dyn Fn(&str) -> PlaceResult<RunOver>>>) -> (Door, Seen, String) {
    let kp = new_place_key_pair();
    let key = kp.public_key.clone();
    let seen: Seen = Rc::new(RefCell::new(Vec::new()));
    let d = door(|s| {
        two(s);
        s.host_key = kp.clone();
        if let Some(answer) = answer {
            let k = key.clone();
            let road = reading(&seen, move || answer(&k));
            s.wiring = Box::new(move |w| w.run_over = Some(road));
        }
    });
    (d, seen, key)
}

fn reply(stdout: impl Fn(&str) -> String + 'static) -> Option<Rc<dyn Fn(&str) -> PlaceResult<RunOver>>> {
    Some(Rc::new(move |key| Ok(RunOver { exit_code: 0, stdout: stdout(key), stderr: String::new() })))
}

fn assert_read_once_as_asked(seen: &Seen, ssh: &str) {
    let seen = seen.borrow();
    assert_eq!(seen.len(), 1, "the new login is read once");
    let (login, script, ms, password) = &seen[0];
    assert_eq!(login, &PlaceLogin { ssh: ssh.into(), key_path: Some("/home/dev/.ssh/id".into()), host_key: None });
    assert_eq!(script, at2("/heldScript").as_str().unwrap());
    assert_eq!(*ms, 20_000);
    assert_eq!(*password, None);
}

// TS: place's reachedOver as manage.ts:140-145 reads it ; spec: S5 "set_login_reaching_other_computer_refused"
#[test]
fn set_login_reaching_other_computer_refused() {
    run(async {
        for (name, stdout) in [("loginOther", 0), ("loginNamesOtherRecord", 1), ("loginNoFile", 2)] {
            let (d, seen, _) = login_door(reply(move |key| match stdout {
                0 => held_stdout("p_00000000000000ff", key),
                1 => held_stdout(PLACE2, key),
                _ => "\nWSP_PLACE_HELD\n".to_string(),
            }));
            let r = fails(set(&d, PLACE, json!({}), &[], json!({ "ssh": "root@10.0.0.9" })).await);
            assert_eq!(r, at2(&format!("/cases/{name}/answer")), "{name}");
            assert_eq!(r["err"], "root@10.0.0.9 reaches a computer that is not old-macbook, so nothing was saved. Give the login that reaches old-macbook itself.");
            assert_read_once_as_asked(&seen, "root@10.0.0.9");
            assert_eq!(d.stored(PLACE).unwrap(), mine(json!({})), "{name}: nothing is written");
            no_writes(&d);
            assert!(d.event_types().is_empty());
        }
    });
}

// spec: S5 "set_login_unread_refused"
#[test]
fn set_login_unread_refused() {
    run(async {
        let exit: Option<Rc<dyn Fn(&str) -> PlaceResult<RunOver>>> = Some(Rc::new(|_| Ok(RunOver { exit_code: 3, stdout: String::new(), stderr: "first\nlast line\n".into() })));
        let silent: Option<Rc<dyn Fn(&str) -> PlaceResult<RunOver>>> = Some(Rc::new(|_| Ok(RunOver { exit_code: 3, stdout: String::new(), stderr: String::new() })));
        let throws: Option<Rc<dyn Fn(&str) -> PlaceResult<RunOver>>> = Some(Rc::new(|_| Err(HostError::new("ssh: connect to host 10.0.0.9 port 22: Connection refused".to_string()))));
        for (name, road) in [("loginExit", exit), ("loginExitSilent", silent), ("loginThrows", throws), ("loginNoRoad", None)] {
            let (d, seen, _) = login_door(road);
            let r = fails(set(&d, PLACE, json!({}), &[], json!({ "ssh": "root@10.0.0.9" })).await);
            assert_eq!(r, at2(&format!("/cases/{name}/answer")), "{name}");
            if name == "loginNoRoad" {
                assert!(seen.borrow().is_empty());
            } else {
                assert_read_once_as_asked(&seen, "root@10.0.0.9");
            }
            assert_eq!(d.stored(PLACE).unwrap(), mine(json!({})), "{name}: nothing is written");
            no_writes(&d);
            assert!(d.event_types().is_empty());
        }
        assert_eq!(at2("/cases/loginExit/err"), "root@10.0.0.9 could not be checked as old-macbook, so nothing was saved: last line. Check that ssh root@10.0.0.9 logs in as root, or through a sudo that asks no password, and try again.");
        assert_eq!(at2("/cases/loginExitSilent/err"), "root@10.0.0.9 could not be checked as old-macbook, so nothing was saved: the read exited 3. Check that ssh root@10.0.0.9 logs in as root, or through a sudo that asks no password, and try again.");
        assert_eq!(at2("/cases/loginNoRoad/err"), "root@10.0.0.9 could not be checked as old-macbook, so nothing was saved: this host holds no ssh road to read it over. Check that ssh root@10.0.0.9 logs in as root, or through a sudo that asks no password, and try again.");
    });
}

// spec: S5, S8 (a login that reads as the same computer is kept on the road, trimmed, the key file and host key standing)
#[test]
fn set_login_same_computer_is_saved_trimmed() {
    run(async {
        for ssh in ["root@10.0.0.9", "  root@10.0.0.9  "] {
            let (d, seen, _) = login_door(reply(|key| held_stdout(PLACE, key)));
            let placed = set(&d, PLACE, json!({}), &[], json!({ "ssh": ssh })).await.expect("the same computer");
            assert_read_once_as_asked(&seen, "root@10.0.0.9");
            let road = json!({ "ssh": "root@10.0.0.9", "keyPath": "/home/dev/.ssh/id", "hostKey": "ssh-ed25519 AAAA", "from": "10.0.0.5" });
            assert_eq!(d.stored(PLACE).unwrap()["road"], road);
            assert_eq!(d.stored(PLACE).unwrap()["road"], at2("/cases/loginSame/record/road"));
            assert_eq!(view(&placed)["road"], json!({ "ssh": "root@10.0.0.9", "from": "10.0.0.5" }));
            assert_eq!(d.caps(PLACE), None, "a login alone sets no settings");
            assert_eq!(d.event_types(), vec!["place.changed"]);
            assert_eq!(at2("/cases/loginSame/events"), json!(1));
        }
    });
}

// spec: S4 (a login other than the record's own is the only one read)
#[test]
fn set_login_equal_to_the_records_own_is_not_read() {
    run(async {
        let (d, seen, _) = login_door(reply(|_| held_stdout("p_00000000000000ff", "x")));
        let placed = set(&d, PLACE, json!({}), &[], json!({ "ssh": "maya@10.0.0.5" })).await.expect("its own login");
        assert!(seen.borrow().is_empty());
        assert_eq!(at2("/cases/loginOwn/scripts"), json!([]));
        assert_eq!(view(&placed)["road"], json!({ "ssh": "maya@10.0.0.5", "from": "10.0.0.5" }));
        assert_eq!(d.stored(PLACE).unwrap()["road"]["ssh"], "maya@10.0.0.5");
        assert_eq!(d.event_types(), vec!["place.changed"]);
    });
}

// ================================================================ set: settings

// TS: places-b oracle /set/patch ; spec: S6 "set_patch_spawn_and_reset"
#[test]
fn set_patch_spawn_and_reset() {
    run(async {
        let d = door(|s| s.caps = vec![(PLACE.into(), json!({ "threads": 3, "spawn": { "maxMachines": 5 } }))]);
        let placed = set(&d, PLACE, json!({ "threads": 2, "spawn": { "maxDepth": 1 } }), &["nap"], json!({})).await.expect("set");
        assert_eq!(view(&placed), at("/set/patch/ok/place"));
        assert_eq!(d.caps(PLACE).unwrap(), at("/set/patchCaps"));
        assert_eq!(d.caps(PLACE).unwrap(), json!({ "threads": 2, "spawn": { "maxMachines": 5, "maxDepth": 1 } }));
        assert_eq!(js(&d.records.settings_held.borrow()[PLACE]), at("/set/patchCaps"));
        assert_eq!(at("/set/patchEvents"), json!(1));
        assert_eq!(d.event_types(), vec!["place.changed"]);
        assert_eq!(d.naps.borrow().clone(), vec![PLACE.to_string()]);
        // TS: places-b settings oracle: a reset of one part of the agents' caps keeps the other
        let e = door(|_| {});
        set(&e, PLACE, json!({ "spawn": { "maxMachines": 5, "maxDepth": 1 } }), &[], json!({})).await.unwrap();
        assert_eq!(e.caps(PLACE).unwrap(), at2("/cases/spawnBoth"));
        set(&e, PLACE, json!({}), &["max-depth"], json!({})).await.unwrap();
        assert_eq!(e.caps(PLACE).unwrap(), at2("/cases/spawnAfterDepthReset"));
        assert_eq!(js(&e.records.settings_held.borrow()[PLACE]), at2("/cases/spawnAfterDepthReset"));
    });
}

// TS: places-settings.test.ts:71 ; places-b oracle /set/resetAll ; spec: S6 "set_reset_all_deletes_caps"
#[test]
fn set_reset_all_deletes_caps() {
    run(async {
        let d = door(|s| s.caps = vec![(PLACE.into(), json!({ "threads": 3 })), ("boat".into(), json!({ "machines": 5, "spendPerDayUsd": 2 }))]);
        let back = set(&d, PLACE, json!({}), &["threads"], json!({})).await.expect("reset");
        assert_eq!(view(&back)["cap"], json!({ "threads": 2 }));
        assert_eq!(view(&back)["capDefault"], json!({ "threads": 2 }));
        assert!(view(&back).get("settings").is_none(), "nothing set reads as no settings");
        assert_eq!(d.caps(PLACE), None);
        assert_eq!(d.store.writes.borrow().clone(), vec![format!("delete caps/{PLACE}")]);
        assert_eq!(js(&d.records.settings_held.borrow()[PLACE]), json!({}));
        assert_eq!(at("/set/resetAllCaps"), Value::Null);
        // a cloud's spend goes back to its default and the machines stay as set
        let boat = set(&d, "boat", json!({}), &["spend"], json!({})).await.expect("reset spend");
        assert_eq!(view(&boat)["cap"], json!({ "machines": 5, "spendPerDayUsd": 10 }));
        assert_eq!(view(&boat)["settings"], json!({ "machines": 5 }));
        assert_eq!(d.caps("boat").unwrap(), json!({ "machines": 5 }));
    });
}

// TS: places-b oracle /set/nullNap ; spec: S6 "set_null_nap_is_given"
#[test]
fn set_null_nap_is_given() {
    run(async {
        let d = door(|_| {});
        let placed = set(&d, PLACE, json!({ "napMs": null }), &[], json!({})).await.expect("a null nap");
        assert_eq!(view(&placed), at("/set/nullNap/ok/place"));
        assert_eq!(d.caps(PLACE).unwrap(), at("/set/nullNapCaps"));
        assert_eq!(d.caps(PLACE).unwrap(), json!({ "napMs": null }));
        assert_eq!(d.naps.borrow().clone(), vec![PLACE.to_string()]);
        // TS: places-b settings oracle: a null turn limit is given too
        let e = door(|_| {});
        set(&e, PLACE, json!({ "turnLimitMs": null }), &[], json!({})).await.unwrap();
        assert_eq!(e.caps(PLACE).unwrap(), at2("/cases/turnLimitNull"));
        assert!(e.naps.borrow().is_empty(), "a turn limit is no nap");
    });
}

// TS: places-b settings oracle /twoAtOnceCaps ; spec: S6 "set_two_at_once_both_stand"
#[test]
fn set_two_at_once_both_stand() {
    run(async {
        let d = door(|_| {});
        let mut tasks = Vec::new();
        for s in [json!({ "threads": 2 }), json!({ "napMs": 1_800_000 }), json!({ "spawn": { "maxDepth": 1 } })] {
            let m = d.manage.clone();
            tasks.push(tokio::task::spawn_local(async move { m.set(PLACE, ask(s), vec![], also(json!({}))).await }));
        }
        for t in tasks {
            t.await.unwrap().expect("each set stands");
        }
        assert_eq!(d.caps(PLACE).unwrap(), at2("/cases/twoAtOnceCaps"));
        assert_eq!(js(&d.records.settings_held.borrow()[PLACE]), at2("/cases/twoAtOnceCaps"));
        assert_eq!(d.naps.borrow().clone(), vec![PLACE.to_string()]);
    });
}

// TS: places-settings.test.ts:257 "arms a workspace ... off its place's nap" (the set side) ; spec: S7 "set_nap_calls_nap_changed"
#[test]
fn set_nap_calls_nap_changed() {
    run(async {
        let d = door(|_| {});
        set(&d, "boat", json!({ "napMs": 300_000 }), &[], json!({})).await.unwrap();
        assert_eq!(d.naps.borrow().clone(), vec!["boat".to_string()]);
        // another setting leaves every window where it stands
        set(&d, "boat", json!({ "machines": 4 }), &[], json!({})).await.unwrap();
        assert_eq!(d.naps.borrow().len(), 1);
        set(&d, "boat", json!({ "napMs": null }), &[], json!({})).await.unwrap();
        assert_eq!(d.naps.borrow().len(), 2);
        let back = set(&d, "boat", json!({}), &["nap"], json!({})).await.unwrap();
        assert_eq!(d.naps.borrow().clone(), vec!["boat".to_string(); 3]);
        assert_eq!(view(&back)["napMs"], json!(1_200_000));
        assert!(view(&back).get("settings").is_some(), "the machines the person set stay");
        set(&d, "boat", json!({}), &["machines"], json!({})).await.unwrap();
        assert_eq!(d.naps.borrow().len(), 3, "a reset of another word calls no nap change");
        // a name alone names no nap
        set(&d, PLACE, json!({}), &[], json!({ "name": "renamed" })).await.ok();
        assert_eq!(d.naps.borrow().len(), 3);
    });
}

// TS: places-settings.test.ts:35 "sets one number and keeps the rest, answers the row with it, and stores only the numbers a person set"
// spec: S6, S10
#[test]
fn set_one_number_keeps_the_rest_and_stores_only_what_was_set() {
    run(async {
        let d = door(|_| {});
        let machines = set(&d, "boat", json!({ "machines": 5 }), &[], json!({})).await.expect("machines");
        assert_eq!(view(&machines)["id"], "boat");
        assert_eq!(view(&machines)["cap"], json!({ "machines": 5, "spendPerDayUsd": 10 }));
        let spend = set(&d, "boat", json!({ "spendPerDayUsd": 0 }), &[], json!({})).await.expect("spend");
        assert_eq!(view(&spend)["cap"], json!({ "machines": 5, "spendPerDayUsd": 0 }));
        let one = set(&d, PLACE, json!({ "threads": 1 }), &[], json!({})).await.expect("threads");
        assert_eq!(view(&one)["cap"], json!({ "threads": 1 }));
        assert_eq!(view(&one)["capDefault"], json!({ "threads": 2 }));
        assert_eq!(view(&one)["settings"], json!({ "threads": 1 }));
        let rows: Vec<Value> = d.manage.rows().await.unwrap().iter().map(view).collect();
        assert_eq!(rows[0]["cap"], json!({ "threads": 8 }));
        assert_eq!(rows[1]["cap"], json!({ "threads": 1 }));
        assert_eq!(rows[2]["cap"], json!({ "machines": 5, "spendPerDayUsd": 0 }));
        assert_eq!(d.caps("boat").unwrap(), json!({ "machines": 5, "spendPerDayUsd": 0 }));
        assert_eq!(d.caps(PLACE).unwrap(), json!({ "threads": 1 }));
        assert_eq!(d.caps("here"), None);
    });
}

// TS: places-settings.test.ts:184, :218, :354, :402, :441 (the set side: the settings the runtime's gates read)
// spec: S6
#[test]
fn set_updates_the_settings_the_runtime_reads() {
    run(async {
        let d = door(|_| {});
        let held = |d: &Door, id: &str| js(&d.records.settings_held.borrow().get(id).cloned().expect("settings held"));
        set(&d, "here", json!({ "threads": 1 }), &[], json!({})).await.unwrap();
        assert_eq!(held(&d, "here"), json!({ "threads": 1 }));
        set(&d, "here", json!({ "threads": 2 }), &[], json!({})).await.unwrap();
        assert_eq!(held(&d, "here"), json!({ "threads": 2 }));
        set(&d, "here", json!({ "turnLimitMs": 7_200_000 }), &[], json!({})).await.unwrap();
        assert_eq!(held(&d, "here"), json!({ "threads": 2, "turnLimitMs": 7_200_000 }));
        set(&d, "boat", json!({ "turnLimitMs": null, "spendPerDayUsd": 0.2 }), &[], json!({})).await.unwrap();
        assert_eq!(held(&d, "boat"), json!({ "spendPerDayUsd": 0.2, "turnLimitMs": null }));
        set(&d, "here", json!({}), &["threads", "turn-limit"], json!({})).await.unwrap();
        assert_eq!(held(&d, "here"), json!({}));
        assert_eq!(d.caps("here"), None);
    });
}

// TS: places-settings.test.ts:331 "reads off on a computer the person owns and six hours on a cloud, and takes a set and a reset on either"
// spec: S6, S10
#[test]
fn set_turn_limit_reads_default_and_reset() {
    run(async {
        let d = door(|_| {});
        let hour = 3_600_000u64;
        let here = set(&d, "here", json!({ "turnLimitMs": 2 * hour }), &[], json!({})).await.unwrap();
        assert_eq!(view(&here)["turnLimitMs"], json!(2 * hour));
        assert_eq!(view(&here)["turnLimitDefault"], Value::Null);
        assert_eq!(view(&here)["settings"], json!({ "turnLimitMs": 2 * hour }));
        let boat = set(&d, "boat", json!({ "turnLimitMs": null }), &[], json!({})).await.unwrap();
        assert_eq!(view(&boat)["turnLimitMs"], Value::Null);
        assert_eq!(view(&boat)["turnLimitDefault"], json!(6 * hour));
        assert_eq!(view(&boat)["settings"], json!({ "turnLimitMs": null }));
        assert_eq!(view(&set(&d, "here", json!({}), &["turn-limit"], json!({})).await.unwrap())["turnLimitMs"], Value::Null);
        let boat = set(&d, "boat", json!({}), &["turn-limit"], json!({})).await.unwrap();
        assert_eq!(view(&boat)["turnLimitMs"], json!(6 * hour));
        assert!(view(&boat).get("settings").is_none());
        assert_eq!(d.caps("boat"), None);
    });
}

// TS: places-settings.test.ts:296 "... says nothing of it on that row"
// spec: S1
#[test]
fn rows_say_nothing_of_a_nap_on_this_computer() {
    run(async {
        let d = door(|_| {});
        let _ = set(&d, "here", json!({ "napMs": 300_000 }), &[], json!({})).await;
        let rows: Vec<Value> = d.manage.rows().await.unwrap().iter().map(view).collect();
        assert!(rows[0].get("napMs").is_none());
    });
}

// TS: places-settings.test.ts:54 "tells every socket the row a set made, so a page another window holds open reads it at once"
// spec: S10 "set_answers_and_emits_fresh_row"
#[test]
fn set_answers_and_emits_fresh_row() {
    run(async {
        let d = door(|_| {});
        let answered = set(&d, PLACE, json!({ "threads": 1 }), &[], json!({})).await.unwrap();
        set(&d, "here", json!({ "threads": 2 }), &[], json!({})).await.unwrap();
        let events = d.events.borrow().clone();
        assert_eq!(events.len(), 2);
        let rows: Vec<Value> = events.iter().map(|e| match e {
            PlaceEvent::Changed(c) => js(&c.place),
            other => panic!("not a place.changed: {other:?}"),
        }).collect();
        assert_eq!(rows[0], view(&answered));
        assert_eq!(rows[1]["id"], "here");
        assert_eq!(rows[1]["cap"], json!({ "threads": 2 }));
        assert_eq!(view(&answered)["settings"], json!({ "threads": 1 }), "the row is read after the write");
    });
}

// TS: places-b settings oracle /together ; spec: S3, S8, S9, S10
#[test]
fn set_name_setting_and_recipe_together() {
    run(async {
        let d = door(|s| {
            two(s);
            s.pending = vec![pending_of("a_000000000001", PLACE, "old-macbook"), pending_of("a_000000000003", PLACE2, "box2")];
        });
        let placed = set(&d, PLACE, json!({ "threads": 2 }), &[], json!({ "name": "  renamed  ", "recipe": "api" })).await.expect("set");
        assert_eq!(view(&placed), at2("/cases/together/ok/place"));
        assert_eq!(d.stored(PLACE).unwrap(), mine(json!({ "name": "renamed", "recipe": "api" })));
        assert_eq!(d.stored(PLACE2).unwrap(), mine(json!({ "id": PLACE2, "name": "box2" })), "the other computer is untouched");
        assert_eq!(d.caps(PLACE).unwrap(), json!({ "threads": 2 }));
        assert_eq!(d.event_types(), vec!["place.changed"]);
        assert_eq!(log_lines(&d), vec![format!("syncSoon {PLACE} 0"), "recipesMoved web".to_string(), "recipesMoved api".to_string()]);
        assert!(d.naps.borrow().is_empty());
    });
}

// TS: places-b settings oracle /recipeAlone ; spec: S3 "set_name_alone_settles_nothing"
#[test]
fn set_name_alone_settles_nothing() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({}))]);
        let placed = set(&d, PLACE, json!({}), &[], json!({ "name": "  new-name  " })).await.expect("a rename");
        assert_eq!(view(&placed)["name"], "new-name");
        assert!(view(&placed).get("settings").is_none());
        assert_eq!(d.stored(PLACE).unwrap(), mine(json!({ "name": "new-name" })));
        assert_eq!(d.caps(PLACE), None);
        assert_eq!(d.store.writes_to("caps"), Vec::<String>::new(), "no settings were settled");
        assert!(d.naps.borrow().is_empty());
        assert_eq!(d.event_types(), vec!["place.changed"]);
        assert!(log_lines(&d).is_empty());
        // a recipe alone settles nothing either, and the name of a record that follows nothing before
        let e = door(|s| s.records = vec![mine(json!({ "recipe": null }))]);
        let followed = set(&e, PLACE, json!({}), &[], json!({ "recipe": "web" })).await.expect("a recipe");
        assert_eq!(view(&followed)["recipe"], "web");
        assert_eq!(e.caps(PLACE), None);
        assert_eq!(at2("/cases/recipeAloneCaps"), Value::Null);
        assert_eq!(e.stored(PLACE).unwrap()["recipe"], "web");
        assert_eq!(log_lines(&e), vec![format!("syncSoon {PLACE} 0"), "recipesMoved web".to_string()]);
    });
}

// TS: places-b settings oracle /renameToOwn, pending a_1 ; spec: S8 "set_rename_renames_pending"
#[test]
fn set_rename_renames_pending() {
    run(async {
        let d = door(|s| {
            two(s);
            let mut other = pending_of("a_000000000002", "p_00000000000000ee", "old-macbook");
            merge(&mut other, json!({ "address": "10.0.0.8" }));
            s.pending = vec![pending_of("a_000000000001", PLACE, "old-macbook"), pending_of("a_000000000003", PLACE2, "box2"), other.clone()];
        });
        set(&d, PLACE, json!({}), &[], json!({ "name": "renamed" })).await.expect("rename");
        let docs = d.pending_docs();
        let named: Vec<(String, String)> = docs.iter().map(|p| (p["id"].as_str().unwrap().to_string(), p["name"].as_str().unwrap().to_string())).collect();
        assert_eq!(named, vec![("a_000000000001".into(), "renamed".into()), ("a_000000000003".into(), "box2".into()), ("a_000000000002".into(), "old-macbook".into())]);
        assert_eq!(docs[0], pending_of("a_000000000001", PLACE, "renamed"), "the rest of the pending add stands");
        // the add's frame says the new name
        let frames = d.setups.borrow().clone();
        let pendings: Vec<_> = frames.iter().filter_map(|f| if let SetupEvent::Pending(p) = f { Some(p.clone()) } else { None }).collect();
        assert_eq!(pendings.len(), 1);
        assert_eq!(pendings[0].id, "a_000000000001");
        assert_eq!(js(&pendings[0].pending.clone().expect("the add's view"))["name"], "renamed");
        // a settings-only set renames nothing
        set(&d, PLACE2, json!({ "threads": 1 }), &[], json!({})).await.unwrap();
        assert_eq!(d.pending_docs()[1]["name"], "box2");
        assert_eq!(d.setups.borrow().iter().filter(|f| matches!(f, SetupEvent::Pending(_))).count(), 1);
    });
}

// TS: places-b settings oracle /backAlone, /backShared ; spec: S8 "set_login_moves_back_forward"
#[test]
fn set_login_moves_back_forward() {
    run(async {
        for (shared, expect) in [(false, vec!["release maya@10.0.0.5", "hold root@10.0.0.9 /home/maya p_0000000000000001"]), (true, vec!["hold root@10.0.0.9 /home/maya p_0000000000000001"])] {
            let kp = new_place_key_pair();
            let key = kp.public_key.clone();
            let back = Rc::new(Back::default());
            let seen: Seen = Rc::new(RefCell::new(Vec::new()));
            let d = door(|s| {
                s.host_key = kp.clone();
                let mut records = vec![mine(json!({ "road": { "ssh": "maya@10.0.0.5", "from": "10.0.0.5", "back": { "boxPort": 7000 } } }))];
                if shared {
                    records.push(mine(json!({ "id": PLACE2, "name": "box2", "road": { "ssh": "maya@10.0.0.5", "from": "10.0.0.5" } })));
                }
                s.records = records;
                let road = reading(&seen, read_ok(held_stdout(PLACE, &key)));
                let held = back.clone();
                s.wiring = Box::new(move |w| {
                    w.run_over = Some(road);
                    w.back = Some(held);
                });
            });
            let placed = set(&d, PLACE, json!({}), &[], json!({ "ssh": "root@10.0.0.9" })).await.expect("a new login");
            assert_eq!(back.log.borrow().clone(), expect, "shared={shared}");
            let case = if shared { "backShared" } else { "backAlone" };
            assert_eq!(d.stored(PLACE).unwrap()["road"], at2(&format!("/cases/{case}/record/road")));
            assert_eq!(view(&placed)["road"], at2(&format!("/cases/{case}/answer/ok/place/road")));
        }
        // a record that dials back through no forward holds and releases nothing
        let back = Rc::new(Back::default());
        let kp = new_place_key_pair();
        let key = kp.public_key.clone();
        let seen: Seen = Rc::new(RefCell::new(Vec::new()));
        let d = door(|s| {
            s.host_key = kp.clone();
            s.records = vec![mine(json!({}))];
            let road = reading(&seen, read_ok(held_stdout(PLACE, &key)));
            let held = back.clone();
            s.wiring = Box::new(move |w| {
                w.run_over = Some(road);
                w.back = Some(held);
            });
        });
        set(&d, PLACE, json!({}), &[], json!({ "ssh": "root@10.0.0.9" })).await.unwrap();
        assert!(back.log.borrow().is_empty());
    });
}

// ================================================================ follow, recipes

fn follow_door() -> Door {
    door(|s| {
        s.records = vec![record_json(json!({ "sync": { "state": "behind", "changes": ["clis/gh"], "since": "2026-10-02T00:00:00.000Z" } })), record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "api" }))];
    })
}

// TS: places-b oracle /follow/followNone ; places-setup.test.ts:2302 "does nothing on a computer that follows no recipe" ; spec: F1, F2 "follow_none_drops_sync_and_frames"
#[test]
fn follow_none_drops_sync_and_frames() {
    run(async {
        let d = follow_door();
        let placed = d.manage.follow(PLACE, "none").await.expect("follow none");
        assert_eq!(view(&placed), at("/follow/followNone/ok"));
        assert_eq!(d.stored(PLACE).unwrap(), at("/follow/followNoneRecord"));
        assert!(d.stored(PLACE).unwrap().get("sync").is_none());
        assert_eq!(d.stored(PLACE).unwrap()["recipe"], "none");
        // the sync frame first, no sync soon, then the old recipe moved and "none" never
        assert_eq!(log_lines(&d), vec![format!("syncFrame {PLACE}"), "recipesMoved web".to_string()]);
        assert_eq!(js(&d.setup.sync_frames.borrow()[0]), json!({ "placeId": PLACE }));
        assert_eq!(at("/follow/setups/0"), json!({ "type": "place.sync", "placeId": PLACE }));
        assert_eq!(d.setup.setup_frames.borrow().len(), 0);
        // a follow of none by a record with no sync sends no frame
        let e = door(|s| s.records = vec![record_json(json!({}))]);
        e.manage.follow(PLACE, "none").await.unwrap();
        assert_eq!(log_lines(&e), vec!["recipesMoved web".to_string()]);
    });
}

// TS: places-b oracle /follow/followApi ; spec: F1, F2 "follow_recipe_keeps_sync_syncs_soon_moves_both_slugs"
#[test]
fn follow_recipe_keeps_sync_syncs_soon_moves_both_slugs() {
    run(async {
        let d = follow_door();
        let placed = d.manage.follow(PLACE, "api").await.expect("follow api");
        assert_eq!(view(&placed)["recipe"], "api");
        assert_eq!(at("/follow/followApi/ok"), "api");
        assert_eq!(d.stored(PLACE).unwrap()["sync"], json!({ "state": "behind", "changes": ["clis/gh"], "since": "2026-10-02T00:00:00.000Z" }));
        assert_eq!(d.stored(PLACE).unwrap()["recipe"], "api");
        assert_eq!(log_lines(&d), vec![format!("syncSoon {PLACE} 0"), "recipesMoved web".to_string(), "recipesMoved api".to_string()]);
        assert!(d.setup.sync_frames.borrow().is_empty());
        // from none to a recipe only the new one moved (the oracle's order of events)
        let e = door(|s| s.records = vec![record_json(json!({ "recipe": "none" }))]);
        e.manage.follow(PLACE, "api").await.unwrap();
        assert_eq!(log_lines(&e), vec![format!("syncSoon {PLACE} 0"), "recipesMoved api".to_string()]);
        // from nothing at all likewise
        let f = door(|s| s.records = vec![record_json(json!({ "recipe": null }))]);
        f.manage.follow(PLACE, "api").await.unwrap();
        assert_eq!(log_lines(&f), vec![format!("syncSoon {PLACE} 0"), "recipesMoved api".to_string()]);
    });
}

// TS: places-setup.test.ts:2277 "runs nothing when a computer set up from picks follows a recipe that holds those same picks", :2292 "says a follow, an unfollow and a changed recipe on the stream" ; spec: F1
#[test]
fn follow_the_recipe_it_follows_moves_it_once_and_leaves_no_sync() {
    run(async {
        let d = door(|s| s.records = vec![record_json(json!({ "recipe": null }))]);
        d.manage.follow(PLACE, "laptop").await.expect("follow");
        assert!(d.stored(PLACE).unwrap().get("sync").is_none());
        assert_eq!(d.stored(PLACE).unwrap()["recipe"], "laptop");
        // the follow asks for a sync at once: what is to run is setup's to decide
        assert_eq!(log_lines(&d), vec![format!("syncSoon {PLACE} 0"), "recipesMoved laptop".to_string()]);
        d.manage.follow(PLACE, "laptop").await.expect("again");
        let heard: Vec<String> = log_lines(&d).into_iter().filter(|l| l.starts_with("recipesMoved")).collect();
        assert_eq!(heard, vec!["recipesMoved laptop", "recipesMoved laptop"], "each follow says its recipe once");
    });
}

// TS: places-b oracle /follow/followUnknown ; spec: F2 "follow_unknown_refused"
#[test]
fn follow_unknown_refused() {
    run(async {
        let d = follow_door();
        assert_eq!(fails(d.manage.follow("p_nope", "web").await), at("/follow/followUnknown"));
        assert_eq!(at("/follow/followUnknown/err"), "no place named p_nope; you have old-macbook, box2");
        no_writes(&d);
        assert!(log_lines(&d).is_empty());
    });
}

fn names_of(v: Vec<String>) -> Vec<String> {
    v
}

// TS: places-setup.test.ts:2292, :2302 ; spec: F3 "recipe_changed_marks_behind_and_names"
#[test]
fn recipe_changed_marks_behind_and_names() {
    run(async {
        let r = resolver();
        let d = door(|s| {
            s.records = vec![
                record_json(json!({ "recipe": "web" })),
                record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "web", "sync": { "state": "behind", "changes": ["x"], "since": "2026-10-03T00:00:00.000Z" } })),
                record_json(json!({ "id": "p_0000000000000003", "name": "box3", "recipe": "api" })),
            ];
            s.setup.syncs.borrow_mut().insert(PLACE.into(), changes(&["skills/why"]));
            s.setup.syncs.borrow_mut().insert(PLACE2.into(), changes(&["a", "b"]));
            s.setup.syncs.borrow_mut().insert("p_0000000000000003".into(), changes(&["never"]));
            with_resolver(s, &r);
        });
        let names = d.manage.recipe_changed("web", 250).await.expect("recipe changed");
        assert_eq!(names_of(names), vec!["old-macbook", "box2"]);
        assert_eq!(
            log_lines(&d),
            vec![
                "recipesMoved web".to_string(),
                format!("syncOf {PLACE} resolved=true"),
                format!("markSync {PLACE} {{\"state\":\"behind\",\"changes\":[\"skills/why\"],\"since\":\"{NOW_ISO}\"}}"),
                format!("syncSoon {PLACE} 250"),
                format!("syncOf {PLACE2} resolved=true"),
                format!("markSync {PLACE2} {{\"state\":\"behind\",\"changes\":[\"a\",\"b\"],\"since\":\"2026-10-03T00:00:00.000Z\"}}"),
                format!("syncSoon {PLACE2} 250"),
            ]
        );
        assert_eq!(r.asked.get(), 1, "the recipe is read once for every computer that follows it");
    });
}

// TS: places-setup.test.ts:2381 "keeps a sign-in that lands while a sync runs ...", :2406 "runs a change that landed mid-sync once more at its end" ; spec: F3 "recipe_changed_busy_not_marked"
#[test]
fn recipe_changed_busy_not_marked() {
    run(async {
        let d = door(|s| {
            s.records = vec![
                record_json(json!({ "recipe": "web" })),
                record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "web" })),
                record_json(json!({ "id": "p_0000000000000003", "name": "box3", "recipe": "web" })),
            ];
            for id in [PLACE, PLACE2, "p_0000000000000003"] {
                s.setup.syncs.borrow_mut().insert(id.into(), changes(&["skills/why"]));
            }
            s.setup.setting.borrow_mut().insert(PLACE.into());
            s.setup.syncing.borrow_mut().insert(PLACE2.into());
        });
        let names = d.manage.recipe_changed("web", 0).await.unwrap();
        assert_eq!(names, vec!["old-macbook", "box2", "box3"], "a busy computer is named all the same");
        let calls = log_lines(&d);
        let marks: Vec<&String> = calls.iter().filter(|c| c.starts_with("markSync")).collect();
        assert_eq!(marks.len(), 1);
        assert!(marks[0].starts_with("markSync p_0000000000000003 "), "only the free computer reads Behind: {marks:?}");
        for id in [PLACE, PLACE2, "p_0000000000000003"] {
            assert!(calls.contains(&format!("syncSoon {id} 0")), "{id} still syncs soon");
        }
        // a change that lands mid-sync is asked for again each time it is said
        d.manage.recipe_changed("web", 0).await.unwrap();
        assert_eq!(log_lines(&d).iter().filter(|c| **c == format!("syncSoon {PLACE} 0")).count(), 2);
    });
}

// TS: places-setup.test.ts:2302 "nothing when the recipe did not move", :2419 "holds a sync whose undo failed" ; spec: F3 "recipe_changed_no_changes_syncs_soon_unnamed"
#[test]
fn recipe_changed_no_changes_syncs_soon_unnamed() {
    run(async {
        let r = resolver();
        let d = door(|s| {
            s.records = vec![record_json(json!({ "recipe": "web" })), record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "web" })), record_json(json!({ "id": "p_0000000000000003", "name": "box3", "recipe": "none" }))];
            s.setup.syncs.borrow_mut().insert(PLACE.into(), changes(&[]));
            with_resolver(s, &r);
        });
        let names = d.manage.recipe_changed("web", 40).await.unwrap();
        assert!(names.is_empty());
        assert_eq!(
            log_lines(&d),
            vec!["recipesMoved web".to_string(), format!("syncOf {PLACE} resolved=true"), format!("syncSoon {PLACE} 40"), format!("syncOf {PLACE2} resolved=true")],
            "no read at all (a sync that could not be read) is skipped with no sync soon"
        );
        assert_eq!(r.asked.get(), 1);
        no_writes(&d);
    });
}

// TS: places-setup.test.ts:2438, :2465 "holds a sync cut partway behind" ; spec: F3 (the sync's own start is kept)
#[test]
fn recipe_changed_keeps_the_since_of_a_sync_held_behind() {
    run(async {
        let d = door(|s| {
            s.records = vec![record_json(json!({ "recipe": "web", "sync": { "state": "running", "changes": ["skills/why"], "since": "2026-10-05T01:02:03.000Z" } }))];
            s.setup.syncs.borrow_mut().insert(PLACE.into(), changes(&["skills/why", "clis/jq"]));
        });
        d.manage.recipe_changed("web", 0).await.unwrap();
        assert!(log_lines(&d).contains(&format!("markSync {PLACE} {{\"state\":\"behind\",\"changes\":[\"skills/why\",\"clis/jq\"],\"since\":\"2026-10-05T01:02:03.000Z\"}}")));
    });
}

// TS: manage.ts:763-781 (the resolve is asked again for the next record while it is none) ; spec: F3
#[test]
fn recipe_changed_asks_again_while_the_recipe_cannot_be_read() {
    run(async {
        let r = resolver();
        r.fail.set(true);
        let d = door(|s| {
            s.records = vec![record_json(json!({ "recipe": "web" })), record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "web" }))];
            s.setup.syncs.borrow_mut().insert(PLACE.into(), changes(&["a"]));
            with_resolver(s, &r);
        });
        let names = d.manage.recipe_changed("web", 0).await.unwrap();
        assert_eq!(names, vec!["old-macbook"]);
        assert_eq!(r.asked.get(), 2, "a failed read is none and each record asks again");
        let calls = log_lines(&d);
        assert!(calls.contains(&format!("syncOf {PLACE} resolved=false")));
        assert!(calls.contains(&format!("syncOf {PLACE2} resolved=false")));
        // and with no resolver wired the sync reads against none
        let e = door(|s| {
            s.records = vec![record_json(json!({ "recipe": "web" }))];
        });
        e.manage.recipe_changed("web", 0).await.unwrap();
        assert!(log_lines(&e).contains(&format!("syncOf {PLACE} resolved=false")));
    });
}

// TS: places-setup.test.ts:2292 "says a follow, an unfollow and a changed recipe on the stream, whether or not any computer moves" ; spec: F3
#[test]
fn recipe_changed_says_the_recipe_moved_first_even_with_no_follower() {
    run(async {
        let d = door(|_| {});
        let names = d.manage.recipe_changed("laptop", 0).await.unwrap();
        assert!(names.is_empty());
        assert_eq!(log_lines(&d), vec!["recipesMoved laptop".to_string()]);
    });
}

// TS: places-b oracle /follow/followers ; spec: F4 "followers_by_slug"
#[test]
fn followers_by_slug() {
    run(async {
        let d = door(|s| {
            s.records = vec![
                record_json(json!({ "recipe": "web" })),
                record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "api" })),
                record_json(json!({ "id": "p_0000000000000003", "name": "box3", "recipe": "web" })),
                record_json(json!({ "id": "p_0000000000000004", "name": "box4", "recipe": "none" })),
                record_json(json!({ "id": "p_0000000000000005", "name": "box5", "recipe": null })),
            ];
        });
        let got: Vec<(String, Vec<String>)> = d.manage.followers().await.unwrap().into_iter().collect();
        assert_eq!(got, vec![("web".to_string(), vec!["old-macbook".to_string(), "box3".to_string()]), ("api".to_string(), vec!["box2".to_string()])]);
        let e = follow_door();
        let got: Vec<(String, Vec<String>)> = e.manage.followers().await.unwrap().into_iter().collect();
        let as_json: serde_json::Map<String, Value> = got.into_iter().map(|(k, v)| (k, json!(v))).collect();
        assert_eq!(Value::Object(as_json), at("/follow/followers"));
    });
}

// TS: places-b oracle /follow/unfollow, /afterUnfollow ; spec: F5 "unfollow_keeps_sync_names_written"
#[test]
fn unfollow_keeps_sync_names_written() {
    run(async {
        let d = door(|s| {
            s.records = vec![
                record_json(json!({ "recipe": "api", "sync": { "state": "behind", "changes": ["clis/gh"], "since": "2026-10-02T00:00:00.000Z" } })),
                record_json(json!({ "id": PLACE2, "name": "box2", "recipe": "api" })),
                record_json(json!({ "id": "p_0000000000000003", "name": "box3", "recipe": "web" })),
            ];
        });
        let names = d.manage.unfollow("api").await.unwrap();
        assert_eq!(names, vec!["old-macbook", "box2"]);
        assert_eq!(d.stored(PLACE).unwrap()["recipe"], "none");
        assert_eq!(d.stored(PLACE2).unwrap()["recipe"], "none");
        assert_eq!(d.stored("p_0000000000000003").unwrap()["recipe"], "web");
        assert_eq!(d.stored(PLACE).unwrap()["sync"], json!({ "state": "behind", "changes": ["clis/gh"], "since": "2026-10-02T00:00:00.000Z" }), "the sync stays");
        assert_eq!(log_lines(&d), vec!["recipesMoved api".to_string()], "no sync frame, no sync soon");
        assert_eq!(d.store.writes.borrow().clone(), vec![format!("put places/{PLACE}"), format!("put places/{PLACE2}")]);
        // nobody follows it now: nothing named, nothing written, the stream still hears it
        let none = d.manage.unfollow("api").await.unwrap();
        assert!(none.is_empty());
        assert_eq!(d.store.writes.borrow().len(), 2);
        assert_eq!(log_lines(&d).iter().filter(|l| *l == "recipesMoved api").count(), 2);
        assert_eq!(at("/follow/unfollow"), json!(["old-macbook", "box2"]));
        assert_eq!(at("/follow/afterUnfollow"), json!([[PLACE, "none"], [PLACE2, "none"]]));
    });
}

// spec: F6 "picks_of_record"
#[test]
fn picks_of_record() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({})), mine(json!({ "id": PLACE2, "name": "box2", "picks": null }))]);
        assert_eq!(js(&d.manage.picks_of(PLACE).await.unwrap().expect("picks")), js(&picks("web", &["lint"])));
        assert!(d.manage.picks_of(PLACE2).await.unwrap().is_none());
        assert!(d.manage.picks_of("p_nope").await.unwrap().is_none());
    });
}

// ================================================================ pending, choose

fn two_pending(s: &mut Seed) {
    s.records = vec![mine(json!({ "picks": null }))];
    s.pending = vec![
        pending_json("a_000000000002", "10.0.0.9", json!({ "choices": js(&picks("10.0.0.9", &[])), "startedAt": "2026-10-03T00:00:00.000Z", "step": "wsp", "keyPath": "/k", "undo": "rm -rf x", "login": "root@10.0.0.9", "hostKey": "ssh-ed25519 SHA256:box" })),
        pending_json("a_000000000001", "10.0.0.5", json!({ "name": "old-macbook", "placeId": PLACE, "step": "choosing", "choices": js(&picks("x", &[])), "recipe": "web", "startedAt": "2026-10-02T00:00:00.000Z" })),
    ];
}

// TS: places-setup.test.ts:1182 "joins as a pending add ... client view has no undo; store keeps undo, login, hostKey" ; places-b oracle /pending/pending
// spec: P1
#[test]
fn pending_oldest_first_and_without_the_door_s_own_fields() {
    run(async {
        let d = door(|s| two_pending(s));
        let got = js(&d.manage.pending().await.unwrap());
        let oracle = at("/pending/pending");
        assert_eq!(got[0], oracle[0]);
        let mut second = oracle[1].clone();
        merge(&mut second, json!({ "failed": null }));
        assert_eq!(got[1], second, "the failed mark the TS oracle shows is places-a's load");
        for p in got.as_array().unwrap() {
            for k in ["keyPath", "undo", "login", "hostKey"] {
                assert!(p.get(k).is_none(), "a client reads no {k}");
            }
        }
        let held = d.store.doc("pending-computers", "a_000000000002").unwrap();
        assert_eq!(held["undo"], "rm -rf x");
        assert_eq!(held["login"], "root@10.0.0.9");
        assert_eq!(held["hostKey"], "ssh-ed25519 SHA256:box");
        assert_eq!(held["keyPath"], "/k");
    });
}

// TS: places-setup.test.ts:1248 "keeps a refusal at the checks on the pending add, read as Setup failed" (the read side) ; spec: P1
#[test]
fn pending_reads_a_failed_add() {
    run(async {
        let said = "root@10.0.0.9 runs no systemd, which is what keeps wsp's daemon up there";
        let d = door(|s| {
            s.pending = vec![pending_json("a_one", "root@10.0.0.9", json!({ "step": "check", "failed": { "said": said } }))];
        });
        let got = js(&d.manage.pending().await.unwrap());
        assert_eq!(got[0]["id"], "a_one");
        assert_eq!(got[0]["step"], "check");
        assert_eq!(got[0]["failed"], json!({ "said": said }));
    });
}

// TS: places-setup.test.ts:1221 "keeps the picks the person made so far on the pending add, says so on its stream, and sets up from them"
// places-b oracle /pending/choose, /chooseStored, /chooseUnknown ; spec: P2 "choose_replaces_choices_and_recipe"
#[test]
fn choose_replaces_choices_and_recipe() {
    run(async {
        let d = door(|s| two_pending(s));
        let chosen = picks("web", &["lint", "fmt", "old"]);
        let kept = d.manage.choose("10.0.0.5", chosen.clone(), None).await.expect("choose");
        assert_eq!(js(&kept), at("/pending/choose/ok"));
        assert_eq!(d.store.doc("pending-computers", "a_000000000001").unwrap(), at("/pending/chooseStored"));
        assert!(js(&kept).get("recipe").is_none(), "picks kept with no recipe named drop the one they started from");
        // a recipe named is kept; the add is found by its id and by the computer's id as well
        let kept = d.manage.choose("a_000000000001", chosen.clone(), Some("laptop".into())).await.unwrap();
        assert_eq!(js(&kept)["recipe"], "laptop");
        assert_eq!(d.store.doc("pending-computers", "a_000000000001").unwrap()["recipe"], "laptop");
        let kept = d.manage.choose(PLACE, picks("api", &[]), None).await.unwrap();
        assert_eq!(js(&kept)["choices"]["name"], "api");
        assert!(d.store.doc("pending-computers", "a_000000000001").unwrap().get("recipe").is_none());
        // the door's own fields stay on the other add, and the stream says the picks without them
        let other = d.manage.choose("10.0.0.9", chosen.clone(), None).await.unwrap();
        assert!(js(&other).get("undo").is_none());
        let held = d.store.doc("pending-computers", "a_000000000002").unwrap();
        assert_eq!((held["undo"].clone(), held["login"].clone(), held["keyPath"].clone(), held["hostKey"].clone()), (json!("rm -rf x"), json!("root@10.0.0.9"), json!("/k"), json!("ssh-ed25519 SHA256:box")));
        let frames = d.setups.borrow().clone();
        let last = frames.iter().rev().find_map(|f| if let SetupEvent::Pending(p) = f { Some(p.clone()) } else { None }).expect("a place.pending frame");
        assert_eq!(last.id, "a_000000000002");
        assert_eq!(js(&last.pending.unwrap())["choices"], js(&chosen));
        assert!(js(&frames_pending_view(&frames)).get("undo").is_none());
    });
}

fn frames_pending_view(frames: &[SetupEvent]) -> Value {
    frames.iter().rev().find_map(|f| if let SetupEvent::Pending(p) = f { p.pending.clone() } else { None }).map(|p| js(&p)).expect("a pending view")
}

// spec: P2 "choose_unknown_refused"
#[test]
fn choose_unknown_refused() {
    run(async {
        let d = door(|s| two_pending(s));
        assert_eq!(fails(d.manage.choose("nobody", picks("web", &[]), None).await), at("/pending/chooseUnknown"));
        assert_eq!(at("/pending/chooseUnknown/err"), "no add of nobody is pending. Run wsp computers to read the adds still pending.");
        assert_eq!(at("/pending/chooseUnknown/fix"), "Run wsp computers to read the adds still pending.");
        // an add is named by id, address or place id, never by its name
        assert!(d.manage.choose("old-macbook", picks("web", &[]), None).await.is_err());
        no_writes(&d);
        assert!(d.setups.borrow().is_empty());
    });
}

// ================================================================ setUp

fn started_with(setup: Option<Value>, said: Option<&str>) -> SetupStarted {
    SetupStarted { setup: setup.map(setup_of), said: said.map(String::from) }
}

fn pending_for_set_up(over: Value) -> Value {
    let mut p = pending_json("a_wait", "root@10.0.0.9", json!({ "placeId": PLACE, "name": "spoo-pending", "recipe": "laptop", "choices": js(&picks("laptop", &["lint"])) }));
    merge(&mut p, over);
    p
}

// TS: places-b oracle /pending/setUpNotJoined ; spec: P3 "set_up_not_joined_refused"
#[test]
fn set_up_not_joined_refused() {
    run(async {
        let d = door(|s| two_pending(s));
        assert_eq!(fails(d.manage.set_up("10.0.0.9", SetUpAsk::default()).await), at("/pending/setUpNotJoined"));
        assert_eq!(at("/pending/setUpNotJoined/err"), "10.0.0.9 never joined, so there is nothing to set up. Add it again with wsp add 10.0.0.9.");
        assert_eq!(at("/pending/setUpNotJoined/fix"), "Add it again with wsp add 10.0.0.9.");
        // an add that has a name says it by that name, found by that name
        let e = door(|s| s.pending = vec![pending_json("a_named", "10.0.0.9", json!({ "name": "newbox" }))]);
        let r = fails(e.manage.set_up("newbox", SetUpAsk::default()).await);
        assert_eq!(r["err"], "newbox never joined, so there is nothing to set up. Add it again with wsp add 10.0.0.9.");
        assert_eq!(r["kind"], "usage");
        assert!(!log_lines(&d).iter().any(|c| c.starts_with("startSetup")));
        assert!(!log_lines(&e).iter().any(|c| c.starts_with("startSetup")));
        assert_eq!(d.pending_docs().len(), 2);
        assert_eq!(e.pending_docs().len(), 1);
    });
}

// TS: places-b oracle /pending/setUpTwo, /setUpUnknown ; spec: P3 "set_up_two_names_refused"
#[test]
fn set_up_two_names_refused() {
    run(async {
        let d = door(|s| s.records = vec![record_json(json!({})), record_json(json!({ "id": PLACE2 }))]);
        assert_eq!(fails(d.manage.set_up("old-macbook", SetUpAsk::default()).await), at("/pending/setUpTwo"));
        assert_eq!(at("/pending/setUpTwo/err"), "old-macbook: this host holds 2 places by that name; name one by its id (p_0000000000000001, p_0000000000000002). Name it by its id.");
        assert_eq!(at("/pending/setUpTwo/fix"), "Name it by its id.");
        // by id it is one
        assert!(d.manage.set_up(PLACE2, SetUpAsk::default()).await.is_ok());
        let e = door(|s| two_pending(s));
        assert_eq!(fails(e.manage.set_up("nobody", SetUpAsk::default()).await), at("/pending/setUpUnknown"));
        assert_eq!(at("/pending/setUpUnknown/err"), format!("no place named nobody; you have old-macbook. {NAMED_FIX}"));
        assert!(!log_lines(&e).iter().any(|c| c.starts_with("startSetup")));
    });
}

// TS: places-b oracle /pending/setUpNoPicks ; places-setup.test.ts:1196 "refuses a resume with nothing chosen" ; spec: P3 "set_up_no_picks_refused"
#[test]
fn set_up_no_picks_refused() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({ "picks": null }))]);
        assert_eq!(fails(d.manage.set_up("old-macbook", SetUpAsk::default()).await), at("/pending/setUpNoPicks"));
        assert_eq!(
            at("/pending/setUpNoPicks/err"),
            "old-macbook has nothing picked to go on it: choose in the app's Add a computer, or run wsp add old-macbook --resume --recipe <name>. Choose what goes on it in the app's Add a computer, or name a saved recipe with --recipe <name>."
        );
        assert_eq!(at("/pending/setUpNoPicks/fix"), "Choose what goes on it in the app's Add a computer, or name a saved recipe with --recipe <name>.");
        // a pending add whose choices hold no row is nothing chosen, and is left standing
        let e = door(|s| {
            s.records = vec![mine(json!({ "picks": null }))];
            s.pending = vec![pending_for_set_up(json!({ "choices": js(&picks("laptop", &[])), "recipe": null }))];
        });
        let r = fails(e.manage.set_up("root@10.0.0.9", SetUpAsk::default()).await);
        assert_eq!(r["kind"], "usage");
        assert_eq!(r["fix"], "Choose what goes on it in the app's Add a computer, or name a saved recipe with --recipe <name>.");
        assert_eq!(e.pending_docs().len(), 1, "the add stands");
        assert!(!log_lines(&e).iter().any(|c| c.starts_with("startSetup")));
        assert!(!log_lines(&d).iter().any(|c| c.starts_with("startSetup")));
        no_writes(&e);
    });
}

// TS: places-setup.test.ts:1196 "sets up from them at the resume", :1221 ; spec: P3 "set_up_uses_pending_choices"
#[test]
fn set_up_uses_pending_choices() {
    run(async {
        for reference in ["a_wait", "root@10.0.0.9", "spoo-pending", PLACE, "old-macbook"] {
            let d = door(|s| {
                s.records = vec![mine(json!({ "picks": null }))];
                s.pending = vec![pending_for_set_up(json!({}))];
                *s.setup.started.borrow_mut() = started_with(None, None);
            });
            let r = d.manage.set_up(reference, SetUpAsk { add_id: Some("a_fixed".into()), ..Default::default() }).await.expect(reference);
            assert_eq!(r.add_id, "a_fixed", "{reference}");
            assert_eq!(log_lines(&d), vec![format!("startSetup {PLACE} a_fixed picks=laptop recipe=laptop row=-")], "{reference}");
            assert!(d.pending_docs().is_empty(), "{reference}: the pending add is dropped");
            assert_eq!(view(&r.place)["id"], PLACE);
        }
    });
}

// spec: P3 steps 6-9 (what is given beats what the pending add holds)
#[test]
fn set_up_given_choices_and_recipe_beat_the_pending_add() {
    run(async {
        let d = door(|s| {
            s.records = vec![mine(json!({ "picks": null }))];
            s.pending = vec![pending_for_set_up(json!({}))];
        });
        // picks given with no recipe: the add's recipe is not carried, since the person moved off it
        d.manage.set_up("a_wait", SetUpAsk { choices: Some(picks("mine", &["fmt"])), add_id: Some("a_1".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&d), vec![format!("startSetup {PLACE} a_1 picks=mine recipe=- row=-")]);
        assert!(d.pending_docs().is_empty());
        // picks and recipe given
        let e = door(|s| {
            s.records = vec![mine(json!({ "picks": null }))];
            s.pending = vec![pending_for_set_up(json!({}))];
        });
        e.manage.set_up("a_wait", SetUpAsk { choices: Some(picks("mine", &["fmt"])), recipe: Some("given".into()), add_id: Some("a_2".into()), row: Some("signins/claude".into()) }).await.unwrap();
        assert_eq!(log_lines(&e), vec![format!("startSetup {PLACE} a_2 picks=mine recipe=given row=signins/claude")]);
        // a recipe given with no picks: the add's choices and the recipe given
        let f = door(|s| {
            s.records = vec![mine(json!({ "picks": null }))];
            s.pending = vec![pending_for_set_up(json!({}))];
        });
        f.manage.set_up("a_wait", SetUpAsk { recipe: Some("other".into()), add_id: Some("a_3".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&f), vec![format!("startSetup {PLACE} a_3 picks=laptop recipe=other row=-")]);
    });
}

// TS: places-setup.test.ts:827 "starts no second sign-in when the setup runs again", :1338, :1357, :1376 (a retry is a new start of what the record holds)
// spec: P3
#[test]
fn set_up_again_starts_from_the_picks_the_record_holds() {
    run(async {
        let d = door(|s| {
            s.records = vec![mine(json!({ "setup": setup_json("done", "a_old", json!([wait_json("signins/codex", "Codex")])) }))];
            *s.setup.started.borrow_mut() = started_with(Some(setup_json("running", "a_new", json!([]))), Some("said this"));
        });
        let r = d.manage.set_up(PLACE, SetUpAsk::default()).await.expect("again");
        let calls = log_lines(&d);
        assert_eq!(calls.len(), 1);
        let hex = r.add_id.strip_prefix("a_").expect("an add id starts a_");
        assert_eq!(hex.len(), 12, "a_ and 12 hex digits");
        assert!(hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{hex}");
        assert_eq!(calls[0], format!("startSetup {PLACE} {} picks=- recipe=- row=-", r.add_id));
        assert_eq!(js(&r.setup.clone().expect("the started setup")), setup_json("running", "a_new", json!([])));
        assert_eq!(r.said.as_deref(), Some("said this"));
        // each retry is its own add
        let again = d.manage.set_up(PLACE, SetUpAsk::default()).await.unwrap();
        assert_ne!(again.add_id, r.add_id);
        assert_eq!(view(&r.place)["id"], PLACE);
        assert!(view(&r.place)["setup"].is_object());
    });
}

// TS: places-setup.test.ts:1536, :1556, :1571, :1594, :1608, :1705 (a retry of one row) ; spec: P3 (`row` rides to the start)
#[test]
fn set_up_row_rides_to_the_start() {
    run(async {
        for row in ["tools/brew/gh", "signins/claude", "agents", "github", "none"] {
            let d = door(|s| s.records = vec![mine(json!({}))]);
            d.manage.set_up(PLACE, SetUpAsk { row: Some(row.into()), add_id: Some("a_r".into()), ..Default::default() }).await.expect(row);
            assert_eq!(log_lines(&d), vec![format!("startSetup {PLACE} a_r picks=- recipe=- row={row}")]);
        }
        // TS: places-setup.test.ts:1571 picks and a row together
        let d = door(|s| s.records = vec![mine(json!({}))]);
        d.manage.set_up(PLACE, SetUpAsk { choices: Some(picks("laptop", &["lint"])), row: Some("signins/claude".into()), add_id: Some("a_r".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&d), vec![format!("startSetup {PLACE} a_r picks=laptop recipe=- row=signins/claude")]);
        // TS: places-setup.test.ts:1888 picks given on a record that holds picks replace them in the start
        let e = door(|s| s.records = vec![mine(json!({}))]);
        e.manage.set_up(PLACE, SetUpAsk { choices: Some(picks("key", &[])), add_id: Some("a_k".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&e), vec![format!("startSetup {PLACE} a_k picks=key recipe=- row=-")]);
    });
}

// TS: places-setup.test.ts:1127 "refuses a fork there while it runs ... asked again with nothing new, it answers the run under way to follow"
// spec: P3 "set_up_running_followed"
#[test]
fn set_up_running_followed() {
    run(async {
        let running = setup_json("running", "a_running", json!([]));
        let d = door(|s| {
            s.records = vec![mine(json!({ "setup": running.clone() }))];
            s.setup.setting.borrow_mut().insert(PLACE.into());
            s.pending = vec![pending_for_set_up(json!({}))];
        });
        let r = d.manage.set_up(PLACE, SetUpAsk::default()).await.expect("followed");
        assert_eq!(r.add_id, "a_running");
        assert_eq!(js(&r.setup.clone().expect("the run under way")), running);
        assert_eq!(view(&r.place)["setup"], running);
        assert!(r.said.is_none());
        assert!(log_lines(&d).is_empty(), "nothing is started: {:?}", log_lines(&d));
        no_writes(&d);
        assert_eq!(d.pending_docs().len(), 1, "the pending add is left standing");
        // new picks, or a recipe, are what a running setup refuses: the start is asked
        d.manage.set_up(PLACE, SetUpAsk { choices: Some(picks("laptop", &["lint"])), add_id: Some("a_n".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&d), vec![format!("startSetup {PLACE} a_n picks=laptop recipe=- row=-")]);
        d.manage.set_up(PLACE, SetUpAsk { recipe: Some("web".into()), add_id: Some("a_m".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&d).len(), 2);
        // a setup the door is not running now is no run to follow
        let e = door(|s| s.records = vec![mine(json!({ "setup": running.clone() }))]);
        e.manage.set_up(PLACE, SetUpAsk { add_id: Some("a_x".into()), ..Default::default() }).await.unwrap();
        assert_eq!(log_lines(&e), vec![format!("startSetup {PLACE} a_x picks=- recipe=- row=-")]);
    });
}

// spec: P3 "set_up_drops_pending_and_starts"
#[test]
fn set_up_drops_pending_and_starts() {
    run(async {
        let d = door(|s| {
            s.records = vec![mine(json!({}))];
            s.pending = vec![pending_for_set_up(json!({ "choices": js(&picks("laptop", &["lint"])) })), pending_json("a_other", "10.0.0.7", json!({}))];
            *s.setup.started.borrow_mut() = started_with(None, Some("could not start"));
        });
        let r = d.manage.set_up("root@10.0.0.9", SetUpAsk { add_id: Some("a_go".into()), ..Default::default() }).await.unwrap();
        assert_eq!(d.pending_docs().len(), 1);
        assert_eq!(d.pending_docs()[0]["id"], "a_other");
        assert_eq!(d.store.writes.borrow().clone(), vec!["delete pending-computers/a_wait".to_string()], "the add goes before the start");
        let frames = d.setups.borrow().clone();
        assert!(matches!(&frames[0], SetupEvent::Pending(p) if p.id == "a_wait" && p.pending.is_none()));
        // the start is told the pending add's picks and recipe, and the answer carries what the start said
        assert_eq!(log_lines(&d), vec![format!("startSetup {PLACE} a_go picks=laptop recipe=laptop row=-")]);
        assert_eq!(r.said.as_deref(), Some("could not start"));
        assert!(r.setup.is_none());
        assert_eq!(r.add_id, "a_go");
        assert_eq!(view(&r.place)["id"], PLACE);
    });
}

// ================================================================ skip

type Skipper = Rc<dyn Fn() -> LocalFut<'static, PlaceResult<()>>>;

fn counted_skipper(d: &Door, calls: Rc<Cell<u32>>, row: &'static str) -> Skipper {
    let records = d.records.clone();
    Rc::new(move || {
        let (records, calls) = (records.clone(), calls.clone());
        Box::pin(async move {
            calls.set(calls.get() + 1);
            records
                .change(PLACE, |mut r| {
                    r.setup = Some(setup_of(setup_json("done", "a_live", json!([]))));
                    r.applied = Some(serde_json::from_value(json!({ "hash": "h1", "at": NOW_ISO, "rows": [applied_row(row, "skipped", json!({ "note": SKIPPED_FOR_NOW, "step": "signins" }))] })).unwrap());
                    Some(r)
                })
                .await
                .map(|_| ())
        })
    })
}

// TS: places-setup.test.ts:1633 "takes a sign-in that waits off the person", :1959 "skips a sign-in whose page ran out", :1571
// spec: SK2 "skip_live_sign_in_runs_skipper"
#[test]
fn skip_live_sign_in_runs_skipper() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({ "setup": setup_json("running", "a_live", json!([wait_json("signins/codex", "Codex")])) }))]);
        let calls = Rc::new(Cell::new(0));
        let skipper = counted_skipper(&d, calls.clone(), "signins/codex");
        d.setup.skippers.borrow_mut().insert(format!("{PLACE}/signins/codex"), skipper);
        let answered = d.manage.skip(PLACE, "signins/codex").await.expect("skip");
        assert_eq!(calls.get(), 1);
        let v = view(&answered);
        assert_eq!(v["setup"]["waiting"], json!([]));
        assert_eq!(v["applied"]["rows"][0]["outcome"], "skipped");
        assert_eq!(v["applied"]["rows"][0]["note"], SKIPPED_FOR_NOW);
        // the live skip is all that ran: manage wrote nothing, said nothing
        assert_eq!(d.store.writes.borrow().clone(), vec![format!("put places/{PLACE}")], "only the skipper's own write");
        assert!(d.setup.setup_frames.borrow().is_empty());
        assert!(log_lines(&d).is_empty());
        // another row of that computer has no live sign-in and is judged off the record
        let nothing = fails(d.manage.skip(PLACE, "signins/claude").await);
        assert_eq!(nothing["kind"], "usage");
        assert_eq!(calls.get(), 1);
    });
}

// spec: SK1 "skip_unknown_place" (the place is judged before a live sign-in is asked)
#[test]
fn skip_unknown_place_refused_before_any_skipper() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({}))]);
        let calls = Rc::new(Cell::new(0));
        let skipper = counted_skipper(&d, calls.clone(), "x");
        d.setup.skippers.borrow_mut().insert("p_nope/x".into(), skipper);
        assert_eq!(fails(d.manage.skip("p_nope", "x").await), at("/skip/unknown"));
        assert_eq!(at("/skip/unknown/err"), format!("no place named p_nope; you have old-macbook. {NAMED_FIX}"));
        assert_eq!(at("/skip/unknown/fix"), NAMED_FIX);
        assert_eq!(calls.get(), 0);
        no_writes(&d);
    });
}

fn expect_frame(d: &Door, frames: &Value) {
    let want: Vec<Value> = frames.as_array().unwrap().iter().map(|f| {
        let mut f = f.clone();
        f.as_object_mut().unwrap().remove("type");
        f
    }).collect();
    let got: Vec<Value> = d.setup.setup_frames.borrow().iter().map(js).collect();
    assert_eq!(got, want);
}

async fn skip_case(name: &str, over: Value, row: &str) -> (Door, Value) {
    let d = door(|s| s.records = vec![mine(over)]);
    let answered = d.manage.skip(PLACE, row).await.unwrap_or_else(|e| panic!("{name}: {e:?}"));
    let c = at2(&format!("/cases/{name}"));
    assert_eq!(view(&answered), c["answer"]["ok"], "{name}: the answer");
    assert_eq!(d.stored(PLACE).unwrap()["applied"], c["record"]["applied"], "{name}: applied as stored");
    assert_eq!(d.stored(PLACE).unwrap()["setup"], c["record"]["setup"], "{name}: setup as stored");
    expect_frame(&d, &c["setups"]);
    assert!(d.event_types().is_empty(), "skip says nothing as place.changed");
    let n = c["events"].as_u64().unwrap() as usize;
    assert_eq!(d.event_types().len(), n);
    (d, c)
}

// TS: places-b oracle /skip/failed ; places-setup.test.ts:1973 "sets a row that failed aside for later", :1826 (the skip side) ; spec: SK3 "skip_failed_row_goes_last_skipped"
#[test]
fn skip_failed_row_goes_last_skipped() {
    run(async {
        let d = door(|s| s.records = vec![record_json(json!({ "setup": setup_json("done", "a_000000000009", json!([])) }))]);
        let answered = d.manage.skip(PLACE, "clis/gh").await.expect("skip");
        assert_eq!(view(&answered), at("/skip/failed/ok"));
        assert_eq!(d.stored(PLACE).unwrap(), at("/skip/record"));
        expect_frame(&d, &at("/skip/setups"));
        assert_eq!(d.setup.setup_frames.borrow().len(), 1);
        let rows = view(&answered)["applied"]["rows"].clone();
        assert_eq!(rows[3]["id"], "clis/gh");
        assert_eq!(rows[3]["outcome"], "skipped");
        assert_eq!(rows[3]["note"], SKIPPED_FOR_NOW);
        assert_eq!(rows[3]["step"], "clis");
        assert!(rows[3].get("fix").is_none() && rows[3].get("kind").is_none());
        // the failed row was not last: it is taken out and put back last, the hash, time and items standing
        skip_case("skipFailedNotLast", json!({ "applied": { "hash": "h1", "at": NOW_ISO, "items": { "clis/jq": "1.7" }, "rows": [applied_row("clis/gh", "failed", json!({ "step": "clis", "note": "no network", "fix": "retry" })), applied_row("plugins/lint", "installed", json!({ "step": "plugins" }))] } }), "clis/gh").await;
        // the failed row's label wins over a wait's
        skip_case("skipFailedLabelOverWait", json!({ "setup": setup_json("done", "a_000000000009", json!([wait_json("clis/gh", "Wait label")])), "applied": { "hash": "h1", "at": NOW_ISO, "rows": [applied_row("clis/gh", "failed", json!({ "step": "clis" }))] } }), "clis/gh").await;
    });
}

// TS: places-b oracle /skipWaiting* ; places-setup.test.ts:1633 (off the record), :1983 ; spec: SK3 "skip_waiting_row_leaves_wait_and_frames"
#[test]
fn skip_waiting_row_leaves_wait_and_frames() {
    run(async {
        // the wait goes, the label is the wait's, a setup still running sends no frame, the applied rows are made
        let (d, _) = skip_case("skipWaitingRunning", json!({ "setup": setup_json("running", "a_000000000009", json!([wait_json("signins/codex", "Codex"), wait_json("signins/claude", "Claude Code")])) }), "signins/codex").await;
        assert!(d.setup.setup_frames.borrow().is_empty());
        assert_eq!(d.stored(PLACE).unwrap()["applied"], json!({ "hash": "", "at": NOW_ISO, "rows": [{ "id": "signins/codex", "label": "Codex", "outcome": "skipped", "note": SKIPPED_FOR_NOW }] }));
        assert_eq!(d.stored(PLACE).unwrap()["setup"]["waiting"].as_array().unwrap().len(), 1);
        // a setup no longer running says how it ended
        let (d, _) = skip_case("skipWaitingDone", json!({ "setup": setup_json("done", "a_000000000009", json!([wait_json("signins/codex", "Codex")])) }), "signins/codex").await;
        assert_eq!(js(&d.setup.setup_frames.borrow()[0]), json!({ "addId": "a_000000000009", "placeId": PLACE, "end": "ready" }));
        // no applied rows at all: made fresh with an empty hash and now
        skip_case("skipWaitingNoApplied", json!({ "setup": setup_json("done", "a_000000000009", json!([wait_json("signins/codex", "Codex")])) }), "signins/codex").await;
        // the step comes off any row with that id
        skip_case("skipStepOffAnyRow", json!({ "setup": setup_json("done", "a_000000000009", json!([wait_json("signins/codex", "Codex")])), "applied": { "hash": "h1", "at": NOW_ISO, "items": { "clis/jq": "1.7" }, "rows": [applied_row("signins/codex", "present", json!({ "step": "signins" })), applied_row("plugins/lint", "installed", json!({ "step": "plugins" }))] } }), "signins/codex").await;
        // a failed row on a setup still running sends no frame either
        skip_case("skipSetupRunningFailedRow", json!({ "setup": setup_json("running", "a_000000000009", json!([])), "applied": { "hash": "h1", "at": NOW_ISO, "rows": [applied_row("clis/gh", "failed", json!({ "step": "clis" }))] } }), "clis/gh").await;
    });
}

// TS: places-b oracle /skip/nothing ; places-setup.test.ts:1973 "refuses a row with nothing to skip" ; spec: SK3 "skip_nothing_refused"
#[test]
fn skip_nothing_refused() {
    run(async {
        let d = door(|s| s.records = vec![record_json(json!({ "setup": setup_json("done", "a_000000000009", json!([])) }))]);
        assert_eq!(fails(d.manage.skip(PLACE, "plugins/lint").await), at("/skip/nothing"));
        assert_eq!(at("/skip/nothing/err"), "nothing waits or failed under plugins/lint on old-macbook. Skip a row the computer's page shows waiting or failed.");
        assert_eq!(at("/skip/nothing/fix"), "Skip a row the computer's page shows waiting or failed.");
        // a row that is not there at all
        let r = fails(d.manage.skip(PLACE, "agents/claude").await);
        assert_eq!(r["err"], "nothing waits or failed under agents/claude on old-macbook. Skip a row the computer's page shows waiting or failed.");
        no_writes(&d);
        assert!(d.setup.setup_frames.borrow().is_empty());
        let e = door(|s| s.records = vec![mine(json!({ "setup": setup_json("done", "a_000000000009", json!([])) }))]);
        assert_eq!(fails(e.manage.skip(PLACE, "plugins/lint").await), at2("/cases/skipPresentRowNothing/answer"));
    });
}

// ================================================================ estimate, setup log

// TS: places-b oracle /estimate/none ; spec: ES1 "estimate_without_reader_refused"
#[test]
fn estimate_without_reader_refused() {
    run(async {
        let d = door(|_| {});
        assert_eq!(fails(d.manage.estimate("old-macbook", picks("web", &[])).await), at("/estimate/none"));
        assert_eq!(at("/estimate/none/err"), "this runtime weighs no picks; the host that serves the app wires the reader");
        assert_eq!(at("/estimate/none/kind"), Value::Null);
    });
}

fn weighing(s: &mut Seed) {
    let p = Provision::default();
    *p.weight.borrow_mut() = Some(PicksWeight { bytes: 123, unmeasured: 2 });
    let p: Rc<dyn PlaceProvisioner> = Rc::new(p);
    s.wiring = Box::new(move |w| w.provision = Some(p));
}

fn est(r: PlaceResult<wsp_wire::views::place_setup_words::PlaceEstimate>) -> Value {
    js(&r.expect("an estimate"))
}

// TS: places-b oracle /estimate/weighed, /nobody ; places-setup.test.ts:1510, :1524 ; spec: ES1 "estimate_reads_floor_and_free"
#[test]
fn estimate_reads_floor_and_free() {
    run(async {
        let d = door(|s| {
            weighing(s);
            s.pending = vec![pending_json("a_wait", "root@10.0.0.9", json!({ "placeId": PLACE, "name": "spoo-pending" })), pending_json("a_none", "root@10.0.0.77", json!({}))];
        });
        let weighed = json!({ "neededBytes": 123, "keptBytes": 5_000_000_000u64, "unmeasured": 2, "freeBytes": 1000 });
        assert_eq!(json!({ "ok": est(d.manage.estimate("old-macbook", picks("web", &[])).await) }), at("/estimate/weighed"));
        assert_eq!(est(d.manage.estimate("old-macbook", picks("web", &[])).await), weighed);
        // by a pending add (its id, address, name and place id) and by the computer's id
        for reference in ["a_wait", "root@10.0.0.9", "spoo-pending", PLACE] {
            assert_eq!(est(d.manage.estimate(reference, picks("web", &[])).await), weighed, "{reference}");
        }
        // an add that never joined, and a word nobody holds, said nothing of its room: the 2 GB floor and no free
        let nobody = json!({ "neededBytes": 123, "keptBytes": 2_147_483_648u64, "unmeasured": 2 });
        assert_eq!(json!({ "ok": est(d.manage.estimate("nobody", picks("web", &[])).await) }), at("/estimate/nobody"));
        assert_eq!(est(d.manage.estimate("nobody", picks("web", &[])).await), nobody);
        assert_eq!(est(d.manage.estimate("root@10.0.0.77", picks("web", &[])).await), nobody);
        assert_eq!(est(d.manage.estimate("a_none", picks("web", &[])).await), nobody);
    });
}

// TS: places-setup.test.ts:1510 (freeBytes only when the computer reported), :1524 "counts the room the install loop keeps free off the computer's disk size"
// spec: ES1
#[test]
fn estimate_keeps_a_tenth_of_the_disk_past_the_floor() {
    run(async {
        let gb = 1024u64 * 1024 * 1024;
        let sized = report(json!({ "diskFreeBytes": 9 * gb, "diskSizeBytes": 75 * gb }));
        let d = door(|s| {
            weighing(s);
            s.records = vec![record_json(json!({ "report": js(&sized) }))];
        });
        // a tenth of 75 GB is kept free, past the 2 GB floor: 8053063680 bytes
        assert_eq!(est(d.manage.estimate(PLACE, picks("web", &[])).await), json!({ "neededBytes": 123, "keptBytes": 8_053_063_680u64, "unmeasured": 2, "freeBytes": 9 * gb }));
        // a computer that reported no free room says none, and a small disk keeps the floor
        let small = report(json!({ "diskFreeBytes": null, "diskSizeBytes": gb }));
        let e = door(|s| {
            weighing(s);
            s.records = vec![record_json(json!({ "report": js(&small) }))];
        });
        assert_eq!(est(e.manage.estimate(PLACE, picks("web", &[])).await), json!({ "neededBytes": 123, "keptBytes": 2_147_483_648u64, "unmeasured": 2 }));
    });
}

const LOG_PATH: &str = "/home/maya/.wsp/provision/log";

fn says(stdout: String) -> std::sync::Arc<FakeMachineLink> {
    FakeMachineLink::new(move |_, _| Ok(obj(json!({ "exitCode": 0, "stdout": stdout.clone(), "stderr": "", "truncated": false }))))
}

/// A log read of exactly `bytes` bytes of UTF-8 whose first line is a cut one holding the step `clis`.
fn log_of_bytes(bytes: usize, filler: &str) -> String {
    let head = "ut line [clis] cut\n";
    let tail = "2026-10-04T10:00:02Z [clis] whole\n";
    let middle = bytes - head.len() - tail.len() - 1;
    assert_eq!(middle % filler.len(), 0);
    let text = format!("{head}{}\n{tail}", filler.repeat(middle / filler.len()));
    assert_eq!(text.len(), bytes);
    text
}

// TS: places-setup.test.ts:1148 "tags each line of its log on that computer with its step, and reads a step's lines back off the end of that log"
// spec: LG1 "setup_log_drops_cut_first_line_and_filters_step"
#[test]
fn setup_log_drops_cut_first_line_and_filters_step() {
    run(async {
        let tail = ["2026-10-04T10:00:00Z [floor] apt-get install curl", "2026-10-04T10:00:01Z [clis] brew install gh", "", "2026-10-04T10:00:02Z [clis] the CLIs: done"].join("\n");
        let d = door(|s| s.records = vec![mine(json!({}))]);
        let link = says(format!("{tail}\n"));
        *d.setup.link.borrow_mut() = Some(link.clone());
        let clis = d.manage.setup_log(PLACE, Some(PlaceSetupStep::Clis)).await.expect("log");
        assert_eq!(clis, vec!["2026-10-04T10:00:01Z [clis] brew install gh", "2026-10-04T10:00:02Z [clis] the CLIs: done"]);
        let whole = d.manage.setup_log(PLACE, None).await.unwrap();
        assert_eq!(whole.len(), 3, "the whole log, blank lines left out");
        assert_eq!(whole[0], "2026-10-04T10:00:00Z [floor] apt-get install curl");
        assert!(d.manage.setup_log(PLACE, Some(PlaceSetupStep::Mcp)).await.unwrap().is_empty());
        // the command, and the link asked for this computer
        assert_eq!(at2("/setupLogPath"), LOG_PATH);
        assert_eq!(at2("/tailBytes"), json!(65536));
        let tails: Vec<_> = link.ops().into_iter().filter(|(_, p)| p.to_string().contains("tail -c")).collect();
        assert_eq!(tails.len(), 3, "one read per call");
        let sent = tails[0].1.to_string();
        assert!(sent.contains(&format!("tail -c 65536 '{LOG_PATH}' 2>/dev/null")), "{sent}");
        assert!(sent.contains("10000"), "the read is bounded at 10 s: {sent}");
        assert_eq!(log_lines(&d), vec![format!("linkTo {PLACE}"); 3]);
    });
}

// TS: manage.ts:831-836 (a read that filled its bytes started in the middle of a line) ; spec: LG1
#[test]
fn setup_log_drops_the_cut_line_only_once_the_bytes_filled() {
    run(async {
        let max = 65536;
        let d = door(|s| s.records = vec![mine(json!({}))]);
        // exactly the bytes asked for: the first line was cut, whatever step it names
        *d.setup.link.borrow_mut() = Some(says(log_of_bytes(max, "z")));
        let got = d.manage.setup_log(PLACE, None).await.unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[1], "2026-10-04T10:00:02Z [clis] whole");
        assert_eq!(d.manage.setup_log(PLACE, Some(PlaceSetupStep::Clis)).await.unwrap(), vec!["2026-10-04T10:00:02Z [clis] whole"], "the cut line holds the step and still goes");
        // one byte short: the first line is whole
        *d.setup.link.borrow_mut() = Some(says(log_of_bytes(max - 1, "z")));
        let got = d.manage.setup_log(PLACE, None).await.unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], "ut line [clis] cut");
        // the length is bytes of UTF-8, not characters: 40000 two-byte characters fill 80000 bytes
        *d.setup.link.borrow_mut() = Some(says(log_of_bytes(max + 1000, "é")));
        assert_eq!(d.manage.setup_log(PLACE, None).await.unwrap().len(), 2);
        let multibyte_under = log_of_bytes(max - 2, "é");
        assert!(multibyte_under.chars().count() < max);
        *d.setup.link.borrow_mut() = Some(says(multibyte_under));
        assert_eq!(d.manage.setup_log(PLACE, None).await.unwrap().len(), 3);
    });
}

// spec: LG1 "setup_log_no_home_empty"
#[test]
fn setup_log_no_home_empty() {
    run(async {
        let no_home = report(json!({ "login": { "USER": "maya", "PATH": "/usr/bin" } }));
        let d = door(|s| s.records = vec![record_json(json!({ "report": js(&no_home) }))]);
        assert!(d.manage.setup_log(PLACE, None).await.unwrap().is_empty());
        assert!(d.manage.setup_log(PLACE, Some(PlaceSetupStep::Clis)).await.unwrap().is_empty());
        assert!(log_lines(&d).is_empty(), "no link is asked for a computer with no home");
    });
}

// TS: manage.ts:826-829 ; spec: LG1 (no record)
#[test]
fn setup_log_unknown_place_refused() {
    run(async {
        let d = door(|s| s.records = vec![mine(json!({}))]);
        let r = fails(d.manage.setup_log("p_nope", None).await);
        assert_eq!(r, json!({ "err": format!("no place named p_nope; you have old-macbook. {NAMED_FIX}"), "kind": "usage", "fix": NAMED_FIX }));
    });
}

// ================================================================ on, close, restart

// spec: C1 "on_off_watchers"
#[test]
fn on_off_watchers() {
    run(async {
        let d = door(|_| {});
        let heard: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
        let (a, b) = (heard.clone(), heard.clone());
        let first = d.manage.on(Rc::new(move |_| a.borrow_mut().push("first")));
        let second = d.manage.on(Rc::new(move |_| b.borrow_mut().push("second")));
        assert_ne!(first, second);
        set(&d, PLACE, json!({ "threads": 1 }), &[], json!({})).await.unwrap();
        assert_eq!(heard.borrow().clone(), vec!["first", "second"], "watchers hear in the order added");
        d.manage.off(first);
        set(&d, PLACE, json!({ "threads": 2 }), &[], json!({})).await.unwrap();
        assert_eq!(heard.borrow().clone(), vec!["first", "second", "second"]);
        d.manage.off(second);
        d.manage.off(second);
        set(&d, PLACE, json!({ "threads": 3 }), &[], json!({})).await.unwrap();
        assert_eq!(heard.borrow().len(), 3);
    });
}

// TS: manage.ts:856-870 ; spec: C2 "close_cuts_links_wakes_waiters_clears"
#[test]
fn close_cuts_links_wakes_waiters_clears() {
    run(async {
        let back = Rc::new(Back::default());
        let d = door(|s| {
            s.records = vec![mine(json!({}))];
            s.caps = vec![(PLACE.into(), json!({ "threads": 3 }))];
            s.pending = vec![pending_of("a_000000000001", PLACE, "old-macbook")];
            let held = back.clone();
            s.wiring = Box::new(move |w| w.back = Some(held));
            s.setup.waiting.replace(vec!["p_waiter_one".into(), "p_waiter_two".into()]);
        });
        let reach = FakeReach::answering(vec![]);
        let (socket, seen_cancelled) = link(&d, PLACE, reach.clone());
        let reach2 = FakeReach::answering(vec![]);
        let (socket2, _) = link(&d, PLACE2, reach2.clone());
        d.records.awaiting.borrow_mut().insert("CODE1".into(), Rc::new(RefCell::new(AwaitingInstall::default())));

        // a forward with a real listener and a connection riding it
        struct Conn(Rc<Cell<u32>>);
        impl wsp_machines::places::helpers::ForwardConn for Conn {
            fn write(&self, _: &[u8]) {}
            fn end(&self) {}
            fn destroy(&self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let (listeners, port) = listen_near(38_417, Rc::new(|_| {})).await.expect("a listener");
        let bound = listeners[0].addr;
        let destroyed = Rc::new(Cell::new(0));
        let stopped = Rc::new(Cell::new(0));
        let conns: Rc<RefCell<IndexMap<String, Rc<dyn wsp_machines::places::helpers::ForwardConn>>>> = Rc::new(RefCell::new(IndexMap::new()));
        conns.borrow_mut().insert("t1".into(), Rc::new(Conn(destroyed.clone())));
        conns.borrow_mut().insert("t2".into(), Rc::new(Conn(destroyed.clone())));
        let s = stopped.clone();
        d.ctx.forwards.borrow_mut().insert(
            format!("{PLACE}/4000"),
            wsp_machines::places::helpers::Forward { server: listeners[0].clone(), local_port: port, conns: conns.clone(), stop: Some(Rc::new(move || s.set(s.get() + 1))) },
        );
        assert!(std::net::TcpListener::bind(bound).is_err(), "the forward's port is held before the close");

        d.manage.close().await.expect("close");

        assert_eq!(log_lines(&d), vec!["clearSyncTimers".to_string(), "woken p_waiter_one false".to_string(), "woken p_waiter_two false".to_string()]);
        assert_eq!(back.log.borrow().clone(), vec!["close".to_string()]);
        assert_eq!(socket.closed.borrow().clone(), Some((1000, "this host is stopping".to_string())));
        assert_eq!(socket2.closed.borrow().clone(), Some((1000, "this host is stopping".to_string())));
        assert!(seen_cancelled.get(), "the last-seen timer is cancelled");
        assert!(reach.closed.get() && reach2.closed.get());
        assert!(d.ctx.live.borrow().is_empty());
        assert_eq!(destroyed.get(), 2);
        assert!(conns.borrow().is_empty());
        assert_eq!(stopped.get(), 1);
        assert!(d.ctx.forwards.borrow().is_empty());
        assert!(std::net::TcpListener::bind(bound).is_ok(), "the listener is closed and its port is free");
        assert!(d.records.awaiting.borrow().is_empty());
        // records, caps and pending adds stay in the store
        assert!(d.stored(PLACE).is_some());
        assert_eq!(d.caps(PLACE).unwrap(), json!({ "threads": 3 }));
        assert_eq!(d.pending_docs().len(), 1);
    });
}

// TS: places-setup.test.ts:1196 "keeps the choices through a host restart" ; spec: "restart_keeps_caps_and_pending_choices"
#[test]
fn restart_keeps_caps_and_pending_choices() {
    run(async {
        let d = door(|s| {
            s.records = vec![mine(json!({ "picks": null }))];
            s.pending = vec![pending_json("a_wait", "root@10.0.0.9", json!({ "placeId": PLACE, "name": "old-macbook" }))];
        });
        let chosen = picks("laptop", &["lint"]);
        d.manage.choose("a_wait", chosen.clone(), Some("laptop".into())).await.unwrap();
        set(&d, PLACE, json!({ "threads": 1, "napMs": null }), &[], json!({ "name": "kept-name" })).await.unwrap();
        d.manage.close().await.unwrap();
        let before = js(&d.manage.pending().await.unwrap());
        let rows_before = js(&d.manage.rows().await.unwrap());

        // a second host over what the store holds
        let (records, caps, pending) = (d.store.all(PLACES), d.store.all(CAPS), d.pending_docs());
        let e = door(|s| {
            s.records = records.clone();
            s.caps = vec![(PLACE.into(), caps[0].clone())];
            s.pending = pending.clone();
        });
        assert_eq!(js(&e.manage.pending().await.unwrap()), before);
        assert_eq!(before[0]["choices"], js(&chosen));
        assert_eq!(before[0]["recipe"], "laptop");
        assert_eq!(before[0]["name"], "kept-name", "the add holding the computer's id takes its new name");
        let rows = js(&e.manage.rows().await.unwrap());
        assert_eq!(rows[1]["name"], "kept-name");
        assert_eq!(rows[1]["settings"], json!({ "threads": 1, "napMs": null }));
        assert_eq!(rows[1]["cap"], json!({ "threads": 1 }));
        assert_eq!(rows, rows_before);
        // nothing chosen is nothing set up: a resume with the stored choices starts from them
        let r = e.manage.set_up("root@10.0.0.9", SetUpAsk { add_id: Some("a_r".into()), ..Default::default() }).await.unwrap();
        assert_eq!(r.add_id, "a_r");
        assert_eq!(log_lines(&e), vec![format!("startSetup {PLACE} a_r picks=laptop recipe=laptop row=-")]);
        assert!(e.pending_docs().is_empty());
        let _ = d;
    });
}
