// SPDX-License-Identifier: AGPL-3.0-only
//! The preferences record and the agents read (spec/preferences.md). Every expected value is the TS's:
//! oracle/preferences.oracle.ts ran `preferencesArea` on a fake context, once per script of sets (the script is kept
//! in the fixture beside what it answered, stored, emitted and read back after a restart), and asked the agents read
//! every question the Rust area answers. `World` below is that fake context over the same scripts.
//!
//! The TS cases that reach this unit, and the test each is ported to, are listed in the spec's `## Tests`.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::join_all;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use common::{error_json, live, oracle, plain, record, run};
use wsp_account::account::core_types::{AccountResult, Entry};
use wsp_account::account::preferences::{
    AccessTable, AgentsWorkspace, CatalogAgentKeys, ChannelTarget, PreferencesArea, PreferencesCtx, ServerIcons, SetAnswer,
};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, Frame};
use wsp_wire::views::preferences::{Preferences, PreferencesPatch};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::HostError;

type Doc = Rc<RefCell<Option<Value>>>;

struct FakeIcons {
    folder: RefCell<String>,
    message: Option<String>,
    forgets: Rc<Cell<usize>>,
}

impl ServerIcons for FakeIcons {
    fn folder(&self) -> String {
        self.folder.borrow().clone()
    }
    fn forget(&self) -> Result<(), String> {
        self.forgets.set(self.forgets.get() + 1);
        match &self.message {
            Some(m) => Err(m.clone()),
            None => Ok(()),
        }
    }
}

struct FakeChannel {
    answer: Map<String, Value>,
}

#[async_trait(?Send)]
impl DaemonChannel for FakeChannel {
    async fn send(&self, _frame: Frame) -> Result<Frame, ChannelError> {
        Ok(self.answer.clone())
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        ChannelEnd { code: 1000, reason: String::new() }
    }
}

fn channel(answer: Value) -> Rc<dyn DaemonChannel> {
    Rc::new(FakeChannel { answer: answer.as_object().unwrap().clone() })
}

/// The fixture's world and one script's settings as a context. What the script leaves out reads as the TS fake's
/// defaults: the fixture's adapters, projects p1 to p3, no session rows, no icons.
struct World {
    fx: Rc<Value>,
    script: Value,
    doc: Doc,
    emitted: RefCell<Vec<Value>>,
    puts: RefCell<Vec<Value>>,
    store_gets: Cell<usize>,
    env_reads: Cell<usize>,
    held: RefCell<Option<Preferences>>,
    trace: RefCell<Vec<String>>,
    log: RefCell<Vec<Value>>,
    hook: RefCell<Option<Rc<dyn Fn(&str)>>>,
    forgot: RefCell<Vec<String>>,
    refusals: RefCell<HashSet<String>>,
    set_fails: RefCell<Option<String>>,
    last_on_event: RefCell<Option<Rc<dyn Fn(Value)>>>,
    icons: Option<Rc<FakeIcons>>,
    forgets: Rc<Cell<usize>>,
    relayed: Cell<bool>,
}

fn fixture() -> Rc<Value> {
    Rc::new(oracle("preferences"))
}

fn script_named(fx: &Value, name: &str) -> Value {
    fx["scripts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["script"]["name"] == json!(name))
        .unwrap_or_else(|| panic!("no script {name}"))
        .clone()
}

fn world_of(fx: &Rc<Value>, script: &Value, doc: Doc) -> Rc<World> {
    let forgets = Rc::new(Cell::new(0));
    let icons = script.get("icons").map(|i| {
        Rc::new(FakeIcons {
            folder: RefCell::new(format!("{}/.wsp/icons", fx["home"].as_str().unwrap())),
            message: i["forget"].as_str().map(String::from),
            forgets: forgets.clone(),
        })
    });
    Rc::new(World {
        fx: fx.clone(),
        script: script.clone(),
        doc,
        emitted: RefCell::new(vec![]),
        puts: RefCell::new(vec![]),
        store_gets: Cell::new(0),
        env_reads: Cell::new(0),
        held: RefCell::new(None),
        trace: RefCell::new(vec![]),
        log: RefCell::new(vec![]),
        hook: RefCell::new(None),
        forgot: RefCell::new(vec![]),
        refusals: RefCell::new(HashSet::new()),
        set_fails: RefCell::new(None),
        last_on_event: RefCell::new(None),
        icons,
        forgets,
        relayed: Cell::new(fx["agents"]["relayed"] == json!(true)),
    })
}

/// The world of a script written in this file, over a fresh store (or the one given).
fn adhoc(fx: &Rc<Value>, script: Value) -> (Rc<World>, Rc<PreferencesArea>) {
    let doc: Doc = Rc::new(RefCell::new(script.get("stored").cloned()));
    let w = world_of(fx, &script, doc);
    let area = PreferencesArea::new(w.clone());
    (w, area)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

impl World {
    fn list(&self, key: &str, default: Vec<String>) -> Vec<String> {
        self.script.get(key).map(strings).unwrap_or(default)
    }
    fn agent_world(&self) -> &Value {
        &self.fx["agents"]
    }
    fn entry_record(&self, id: &str) -> Option<Value> {
        self.agent_world()["entries"].get(id).cloned()
    }
}

#[async_trait(?Send)]
impl PreferencesCtx for World {
    fn env(&self) -> IndexMap<String, String> {
        self.env_reads.set(self.env_reads.get() + 1);
        self.script
            .get("env")
            .and_then(|e| e.as_object())
            .map(|e| e.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect())
            .unwrap_or_default()
    }
    fn server_icons(&self) -> Option<Rc<dyn ServerIcons>> {
        self.icons.clone().map(|i| i as Rc<dyn ServerIcons>)
    }
    fn home_dir(&self) -> String {
        self.fx["home"].as_str().unwrap().to_string()
    }
    fn adapter_ids(&self) -> Vec<String> {
        self.list("adapters", strings(&self.fx["adapters"]))
    }
    fn access_table(&self, agent: &str) -> AccessTable {
        match self.fx["tables"].get(agent) {
            Some(table) => AccessTable { label: table["label"].as_str().unwrap().to_string(), table: table.clone() },
            None => {
                let label = format!("the {agent} agent");
                AccessTable { label: label.clone(), table: json!({ "label": label, "permissionModes": [] }) }
            }
        }
    }
    fn project_held(&self, id: &str) -> bool {
        self.list("projects", strs(&["p1", "p2", "p3"])).iter().any(|p| p == id)
    }
    fn project_of(&self, id: &str) -> Result<(String, String), HostError> {
        match self.agent_world()["projectRows"].as_array().unwrap().iter().find(|p| p["id"] == json!(id)) {
            Some(p) => Ok((id.to_string(), p["name"].as_str().unwrap().to_string())),
            None => Err(plain(&format!("no project {id}"))),
        }
    }
    fn default_agent_of(&self, prefs: &Preferences) -> String {
        prefs.default_agent.clone().unwrap_or_else(|| "claude".to_string())
    }
    fn session_thread_keys(&self) -> Vec<String> {
        self.list("sessions", vec![])
    }
    fn thread_record_held(&self, thread_id: &str) -> bool {
        self.list("threadRecords", vec![]).iter().any(|t| t == thread_id)
    }
    async fn ready(&self) {
        self.trace.borrow_mut().push("ready".into());
        self.log.borrow_mut().push(json!("ready"));
        tokio::task::yield_now().await;
    }
    async fn stored(&self) -> AccountResult<Option<Value>> {
        self.store_gets.set(self.store_gets.get() + 1);
        tokio::task::yield_now().await;
        Ok(self.doc.borrow().clone())
    }
    async fn store(&self, prefs: &Preferences) -> AccountResult<()> {
        tokio::task::yield_now().await;
        let v = serde_json::to_value(prefs).unwrap();
        self.puts.borrow_mut().push(v.clone());
        *self.doc.borrow_mut() = Some(v);
        Ok(())
    }
    fn held(&self) -> Option<Preferences> {
        self.held.borrow().clone()
    }
    fn set_held(&self, prefs: Preferences) {
        *self.held.borrow_mut() = Some(prefs);
    }
    fn emit(&self, event: Value) {
        self.emitted.borrow_mut().push(event);
    }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        *self.hook.borrow_mut() = Some(f);
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<&Caller>) -> Result<Entry, HostError> {
        self.log.borrow_mut().push(json!(["entryOf", workspace_id, origin.is_some()]));
        match self.entry_record(workspace_id) {
            Some(r) => Ok(live(record(&r))),
            None => Err(plain(&format!("no workspace {workspace_id}"))),
        }
    }
    fn catalog_agents(&self) -> Vec<CatalogAgentKeys> {
        self.fx["catalogAgents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| CatalogAgentKeys {
                id: a["id"].as_str().unwrap().to_string(),
                state_home_env: a["stateHomeEnv"].as_str().map(String::from),
                plugin_seed_env: a["pluginSeedEnv"].as_str().map(String::from),
            })
            .collect()
    }
    fn thread_env(&self, _entry: &Entry, agent: &str) -> IndexMap<String, String> {
        self.fx["threadEnv"]
            .get(agent)
            .and_then(|e| e.as_object())
            .map(|e| e.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect())
            .unwrap_or_default()
    }
    fn checkout_of(&self, entry: &Entry) -> Result<String, HostError> {
        let r = entry.borrow().record.borrow().clone();
        if r.id == "wBad" {
            return Err(plain("no checkout here"));
        }
        Ok(format!("/work/{}", r.project))
    }
    fn is_local(&self, entry: &Entry) -> bool {
        entry.borrow().record.borrow().kind == WorkspaceKind::Local
    }
    fn at_provider(&self, entry: &Entry) -> bool {
        entry.borrow().record.borrow().place.as_deref() == Some("box")
    }
    fn pause_mode(&self, entry: &Entry) -> Option<String> {
        (entry.borrow().record.borrow().id == "wNap").then(|| "disk".to_string())
    }
    fn projects_on(&self, place_id: &str) -> Vec<(String, String, String)> {
        self.trace.borrow_mut().push(format!("projects_on {place_id}"));
        self.agent_world()["projectRows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["computer"] == json!(place_id))
            .map(|p| {
                (
                    p["id"].as_str().unwrap().to_string(),
                    p["name"].as_str().unwrap().to_string(),
                    p.get("checkout").unwrap_or(&p["path"]).as_str().unwrap().to_string(),
                )
            })
            .collect()
    }
    async fn workspace_channel(
        &self,
        workspace_id: &str,
        _on_event: Rc<dyn Fn(Value)>,
        origin: Option<&Caller>,
    ) -> AccountResult<Rc<dyn DaemonChannel>> {
        Ok(channel(json!({ "chan": format!("workspace {workspace_id}"), "origin": origin.is_some() })))
    }
    async fn local_channel(&self, _on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        self.log.borrow_mut().push(json!("localRoad"));
        Ok(channel(json!({ "chan": "local" })))
    }
    fn place_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> {
        if place_id != "pl_box" {
            return None;
        }
        let same = self.last_on_event.borrow().as_ref().is_some_and(|last| std::ptr::addr_eq(Rc::as_ptr(last), Rc::as_ptr(&on_event)));
        Some(channel(json!({ "chan": format!("link {place_id}"), "sameOnEvent": same })))
    }
    fn place_name(&self, place_id: &str) -> Option<String> {
        (place_id == "pl_gone").then(|| "studio".to_string())
    }
    async fn setup_write(&self, place_id: &str, agent: &str, change: Value) -> AccountResult<()> {
        self.log.borrow_mut().push(json!(["set", place_id, agent, change]));
        match self.set_fails.borrow().clone() {
            Some(m) => Err(plain(&m).into()),
            None => Ok(()),
        }
    }
    fn setup_view(&self, place_id: &str, agent: &str) -> Value {
        let _ = place_id;
        self.agent_world()["setupOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["agent"] == json!(agent))
            .map(|s| s["answer"].clone())
            .unwrap_or(Value::Null)
    }
    fn drop_setup_refusal(&self, place_id: &str, agent: &str) -> bool {
        self.refusals.borrow_mut().remove(&format!("{place_id}:{agent}"))
    }
    fn setup_refusal_moved(&self, place_id: &str, agent: &str) {
        self.log.borrow_mut().push(json!(["moved", place_id, agent]));
    }
    fn relayed(&self) -> bool {
        self.relayed.get()
    }
    fn forget_read(&self, workspace_id: &str) {
        self.forgot.borrow_mut().push(workspace_id.to_string());
    }
}

fn patch(v: &Value) -> PreferencesPatch {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("patch {v}: {e}"))
}

fn answer_json(r: AccountResult<SetAnswer>) -> Value {
    match r {
        Ok(a) => {
            let mut out = json!({ "ok": true, "preferences": serde_json::to_value(&a.preferences).unwrap() });
            if let Some(n) = a.notice {
                out["notice"] = json!(n);
            }
            out
        }
        Err(e) => json!({ "ok": false, "error": error_json(e.host().expect("a host error")) }),
    }
}

fn prefs_json(p: AccountResult<Preferences>) -> Value {
    serde_json::to_value(p.unwrap_or_else(|e| panic!("get failed: {}", e.message()))).unwrap()
}

fn steps_of(script: &Value) -> Vec<Value> {
    script["steps"].as_array().unwrap().iter().map(|s| s["patch"].clone()).collect()
}

/// Runs a fixture script the way the oracle ran it and holds every observable to the TS's: each answer, each record
/// put, each event, the forgets, the store reads, the record read back, and the record a new area reads off the store.
async fn check_script(fx: &Rc<Value>, name: &str) -> (Rc<World>, Value) {
    let want = script_named(fx, name);
    let script = want["script"].clone();
    let doc: Doc = Rc::new(RefCell::new(script.get("stored").cloned()));
    let w = world_of(fx, &script, doc.clone());
    let area = PreferencesArea::new(w.clone());
    let probe = script.get("probe") == Some(&json!(true));
    let before = if probe { Some(area.agents_latest_on().await.unwrap()) } else { None };
    let mut probes = vec![];
    let mut results = vec![];
    if script.get("concurrent") == Some(&json!(true)) {
        let runs = steps_of(&script).iter().map(|p| area.set(patch(p))).collect::<Vec<_>>();
        results.extend(join_all(runs).await.into_iter().map(answer_json));
    } else {
        for p in steps_of(&script) {
            results.push(answer_json(area.set(patch(&p)).await));
            if probe {
                probes.push(json!(area.agents_latest_on().await.unwrap()));
            }
        }
    }
    let after = prefs_json(area.get().await);
    let _ = area.get().await;
    assert_eq!(Value::Array(results), want["results"], "{name}: answers");
    assert_eq!(Value::Array(w.puts.borrow().clone()), want["puts"], "{name}: records put");
    assert_eq!(Value::Array(w.emitted.borrow().clone()), want["emitted"], "{name}: events");
    assert_eq!(w.forgets.get() as u64, want["forgets"].as_u64().unwrap(), "{name}: icon forgets");
    assert_eq!(w.store_gets.get() as u64, want["storeGets"].as_u64().unwrap(), "{name}: store reads");
    assert_eq!(after, want["after"], "{name}: the record read back");
    if probe {
        assert_eq!(json!(before.unwrap()), want["latestBefore"], "{name}: latest before");
        assert_eq!(Value::Array(probes), want["latestAfter"], "{name}: latest after each step");
    }
    let mut second_script = script.clone();
    second_script["steps"] = json!([]);
    let again = PreferencesArea::new(world_of(fx, &second_script, doc));
    assert_eq!(prefs_json(again.get().await), want["reopened"], "{name}: a new area on the same store");
    (w, want)
}

fn result_of(want: &Value, i: usize) -> &Value {
    &want["results"][i]
}

// F1, F2, preferences.test.ts:125, :143
#[test]
fn f2_get_reads_once_and_stamps_labs() {
    let fx = fixture();
    run(async {
        for name in [
            "get defaults",
            "get stored older labs",
            "get env labs over stored",
            "get env labs not one",
            "get malformed stored",
            "get stored before computer icons",
        ] {
            let (w, want) = check_script(&fx, name).await;
            assert!(w.held.borrow().is_some(), "{name}: the record is held after a read");
            assert_eq!(w.env_reads.get(), 1, "{name}: the env is read once, at construction");
            assert_eq!(w.store_gets.get(), 1, "{name}: the store is read once across two gets");
            let _ = want;
        }
        // A stored labs cannot outvote the environment's, and the environment's wins either way.
        let (_, older) = check_script(&fx, "get stored older labs").await;
        assert_eq!(older["after"]["labs"], json!(false));
        assert_eq!(older["after"]["theme"], json!("light"));
        let (_, env) = check_script(&fx, "get env labs over stored").await;
        assert_eq!(env["after"]["labs"], json!(true));
        assert_eq!(env["after"]["theme"], json!("dark"));
        let (_, not_one) = check_script(&fx, "get env labs not one").await;
        assert_eq!(not_one["after"]["labs"], json!(false));
        let (_, malformed) = check_script(&fx, "get malformed stored").await;
        assert_eq!(malformed["after"]["theme"], json!("system"));
    });
}

// TS: test/preferences.test.ts:143
#[test]
fn ported_labs_follow_the_env_the_area_was_handed() {
    let fx = fixture();
    run(async {
        let (_w, area) = adhoc(&fx, json!({ "name": "labs on", "env": { "WSP_LABS": "1" } }));
        let on = prefs_json(area.get().await);
        let (_w, area) = adhoc(&fx, json!({ "name": "labs off", "env": {} }));
        let off = prefs_json(area.get().await);
        assert_eq!(on["labs"], json!(true));
        assert_eq!(off["labs"], json!(false));
        let mut on_without_labs = on.clone();
        on_without_labs["labs"] = json!(false);
        assert_eq!(on_without_labs, off);
    });
}

// TS: test/preferences.test.ts:125
#[test]
fn ported_a_record_kept_before_computers_had_icons_reads_with_none() {
    let fx = fixture();
    run(async {
        let (_w, want) = check_script(&fx, "get stored before computer icons").await;
        assert_eq!(want["after"]["computerLook"], json!({}));
        assert_eq!(want["after"]["projectLook"], json!({ "pr_1": { "icon": "rocket" } }));
        assert_eq!(want["after"]["theme"], json!("light"));
    });
}

// F3
#[test]
fn f3_two_sets_at_once_keep_both_fields() {
    let fx = fixture();
    run(async {
        let (w, want) = check_script(&fx, "two sets at once").await;
        assert_eq!(result_of(&want, 0)["preferences"]["theme"], json!("light"));
        assert_eq!(result_of(&want, 0)["preferences"]["sidebarMode"], json!("list"));
        assert_eq!(result_of(&want, 1)["preferences"]["theme"], json!("light"));
        assert_eq!(result_of(&want, 1)["preferences"]["sidebarMode"], json!("spaces"));
        assert_eq!(w.puts.borrow().len(), 2);
    });
}

// TS: test/preferences.test.ts:132
#[test]
fn ported_two_patches_landing_at_once_keep_both_fields() {
    let fx = fixture();
    run(async {
        let (_w, area) = adhoc(&fx, json!({ "name": "two", "steps": [] }));
        let (a, b) = futures::join!(area.set(patch(&json!({ "theme": "light" }))), area.set(patch(&json!({ "sidebarMode": "spaces" }))));
        let a = answer_json(a);
        let b = answer_json(b);
        assert_eq!(a["preferences"]["theme"], json!("light"));
        assert_eq!(b["preferences"]["theme"], json!("light"));
        assert_eq!(b["preferences"]["sidebarMode"], json!("spaces"));
        let got = prefs_json(area.get().await);
        assert_eq!(got["theme"], json!("light"));
        assert_eq!(got["sidebarMode"], json!("spaces"));
    });
}

#[test]
fn f3_a_refused_set_writes_nothing_and_the_next_runs() {
    let fx = fixture();
    run(async {
        let (w, want) = check_script(&fx, "a refused set among two at once").await;
        assert_eq!(result_of(&want, 0)["ok"], json!(false));
        assert_eq!(result_of(&want, 1)["preferences"]["theme"], json!("light"));
        assert_eq!(result_of(&want, 2)["preferences"]["sidebarMode"], json!("spaces"));
        assert_eq!(w.puts.borrow().len(), 2, "the refused set wrote nothing");
        assert_eq!(w.emitted.borrow().len(), 2, "the refused set told nobody");
        let (w, want) = check_script(&fx, "refused then next").await;
        assert_eq!(result_of(&want, 1)["preferences"]["theme"], json!("dark"));
        assert_eq!(w.puts.borrow().len(), 1);
        let held = w.held.borrow().clone().unwrap();
        assert_eq!(serde_json::to_value(&held).unwrap()["theme"], json!("dark"));
    });
}

// TS: test/preferences.test.ts:29
#[test]
fn ported_preferences_over_the_wire() {
    let fx = fixture();
    run(async {
        let (w, want) = check_script(&fx, "over the wire").await;
        let first = &result_of(&want, 0)["preferences"];
        assert_eq!(
            (first["theme"].clone(), first["sidebarWidth"].clone(), first["terminalZoom"].clone()),
            (json!("light"), json!(312), json!({ "ws_a": 2 }))
        );
        let emitted = w.emitted.borrow();
        assert_eq!(emitted[0], json!({ "type": "preferences.changed", "preferences": first }));
        let second = &result_of(&want, 1)["preferences"];
        assert_eq!(
            (second["sidebarMode"].clone(), second["terminalSize"].clone(), second["terminalZoom"].clone()),
            (json!("spaces"), json!("app"), json!({ "ws_a": 2, "ws_b": -1 }))
        );
        assert!(second.get("sidebarWidth").is_none(), "a null width clears the width alone");
        let third = &result_of(&want, 2)["preferences"];
        assert_eq!(third["terminalZoom"], json!({ "ws_a": 2 }));
        assert!(want["after"].get("sidebarWidth").is_none());
        let reopened = &want["reopened"];
        assert_eq!(
            (reopened["theme"].clone(), reopened["sidebarMode"].clone(), reopened["terminalZoom"].clone()),
            (json!("light"), json!("spaces"), json!({ "ws_a": 2 }))
        );
    });
}

// TS: test/preferences.test.ts:57
#[test]
fn ported_a_dismissed_release_lands_on_the_record_and_the_next_area() {
    let fx = fixture();
    run(async {
        let (w, want) = check_script(&fx, "dismissed release").await;
        assert_eq!(result_of(&want, 0)["preferences"]["updateDismissed"], json!("0.3.3"));
        assert_eq!(w.emitted.borrow()[0]["preferences"]["updateDismissed"], json!("0.3.3"));
        assert_eq!(want["reopened"]["updateDismissed"], json!("0.3.3"));
    });
}

// TS: test/preferences.test.ts:71
#[test]
fn ported_usage_counts_read_on_until_turned_off_and_the_record_keeps_the_switch() {
    let fx = fixture();
    run(async {
        let (_w, want) = check_script(&fx, "usage counts").await;
        assert_eq!(result_of(&want, 0)["preferences"]["productUsage"], json!(false));
        assert_eq!(want["reopened"]["productUsage"], json!(false));
        let (_w, area) = adhoc(&fx, json!({ "name": "fresh" }));
        assert_eq!(prefs_json(area.get().await)["productUsage"], json!(true));
    });
}

// F4
#[test]
fn f4_threads_shut_keeps_held_leads_only() {
    let fx = fixture();
    run(async {
        let (_w, want) = check_script(&fx, "threads shut leads").await;
        assert_eq!(result_of(&want, 0)["preferences"]["threadsShut"], json!({ "lead1": true, "lead2": true }));
        assert_eq!(result_of(&want, 1)["preferences"]["threadsShut"], json!({ "lead2": true }));
        assert_eq!(result_of(&want, 2)["preferences"]["threadsShut"], json!({}));
        let (_w, want) = check_script(&fx, "threads shut stale kept when the patch names none").await;
        assert_eq!(
            result_of(&want, 0)["preferences"]["threadsShut"],
            json!({ "thr_stale": true }),
            "a patch that names no threadsShut leaves the record's alone"
        );
        assert_eq!(
            result_of(&want, 1)["preferences"]["threadsShut"],
            json!({}),
            "a patch that names threadsShut prunes the merge, the stale one with it"
        );
        let (w, want) = check_script(&fx, "threads shut refused default still prunes nothing").await;
        assert_eq!(result_of(&want, 0)["ok"], json!(false));
        assert!(w.puts.borrow().is_empty());
        let (_w, area) = adhoc(&fx, json!({ "name": "held", "sessions": ["lead1"], "threadRecords": ["lead2"] }));
        let mut shut = IndexMap::new();
        shut.insert("thr_gone".to_string(), true);
        shut.insert("lead2".to_string(), true);
        shut.insert("lead1".to_string(), true);
        let held: Vec<(String, bool)> = area.held_leads(&shut).into_iter().collect();
        assert_eq!(held, vec![("lead2".to_string(), true), ("lead1".to_string(), true)]);
    });
}

// TS: test/thread-lines.test.ts:157
#[test]
fn ported_a_lead_shut_on_the_record_and_a_write_drops_a_lead_the_host_does_not_hold() {
    let fx = fixture();
    run(async {
        let (_w, area) = adhoc(&fx, json!({ "name": "thread-lines 157", "sessions": ["lead"] }));
        let first = answer_json(area.set(patch(&json!({ "threadsShut": { "lead": true, "thr_gone": true } }))).await);
        assert_eq!(first["preferences"]["threadsShut"], json!({ "lead": true }));
        let second = answer_json(area.set(patch(&json!({ "threadsShut": { "lead": null } }))).await);
        assert_eq!(second["preferences"]["threadsShut"], json!({}));
    });
}

// TS: test/thread-lines.test.ts:170
#[test]
fn ported_a_shut_section_stands_while_the_thread_record_does_after_its_rows_fell_off() {
    let fx = fixture();
    run(async {
        let (_w, want) = check_script(&fx, "threads shut record alone").await;
        assert_eq!(result_of(&want, 0)["preferences"]["threadsShut"], json!({ "lead1": true }));
        let (_w, area) = adhoc(&fx, json!({ "name": "thread-lines 170", "sessions": [], "threadRecords": ["lead"] }));
        let first = answer_json(area.set(patch(&json!({ "threadsShut": { "lead": true, "thr_gone": true } }))).await);
        assert_eq!(first["preferences"]["threadsShut"], json!({ "lead": true }));
    });
}

#[test]
fn f4_set_answers_the_record() {
    let fx = fixture();
    run(async {
        let (_w, want) = check_script(&fx, "icons forget fails").await;
        assert!(result_of(&want, 1).get("notice").is_none(), "a set that does not turn icons off carries no notice");
        assert!(result_of(&want, 2).get("notice").is_none(), "turning icons on carries no notice and forgets nothing");
        let (_w, want) = check_script(&fx, "over the wire").await;
        for i in 0..3 {
            assert_eq!(result_of(&want, i).as_object().unwrap().keys().cloned().collect::<Vec<_>>(), vec!["ok", "preferences"]);
        }
    });
}

// F5, TS: test/preferences.test.ts:81
#[test]
fn f5_defaults_refusal_table() {
    let fx = fixture();
    run(async {
        let (w, want) = check_script(&fx, "refusal table").await;
        let steps = steps_of(&want["script"]);
        let refused: Vec<bool> = want["results"].as_array().unwrap().iter().map(|r| r["ok"] == json!(false)).collect();
        assert_eq!(steps.len(), refused.len());
        assert_eq!(w.puts.borrow().len(), refused.iter().filter(|r| !**r).count(), "a refused change writes nothing");
        for r in want["results"].as_array().unwrap() {
            if r["ok"] == json!(false) {
                assert_eq!(r["error"]["kind"], json!("usage"));
                assert!(r["error"].get("fix").is_none());
            }
        }
        // The sentences are the TS's own; these are the ones the TS tests quote.
        let said: Vec<String> =
            want["results"].as_array().unwrap().iter().filter_map(|r| r["error"]["message"].as_str().map(String::from)).collect();
        assert!(said.contains(&"Claude Code takes no plan access; it takes ask, auto-edit, full".to_string()));
        assert!(said.contains(&"No model id has a space in it".to_string()));
        assert!(said.contains(&"no project \"pX\"".to_string()));
        let (w, want) = check_script(&fx, "failed hook").await;
        assert_eq!(
            result_of(&want, 0)["error"],
            json!({ "message": "no adapter registered for harness \"nope\"; agents on this host: none", "kind": "usage" })
        );
        assert!(w.puts.borrow().is_empty());
        let get = prefs_json(adhoc(&fx, json!({ "name": "after the refusal", "adapters": [] })).1.get().await);
        assert_eq!(get, want["after"]);
    });
}

// TS: test/thread-defaults.test.ts:168
#[test]
fn ported_a_model_id_no_agent_runs_is_refused_and_nothing_of_the_change_is_stored() {
    let fx = fixture();
    run(async {
        let (w, area) = adhoc(&fx, json!({ "name": "thread-defaults 168" }));
        let refused = [
            (json!({ "agentDefaults": { "claude": { "models": { "custom": ["claude opus"] } } } }), "No model id has a space in it"),
            (json!({ "agentDefaults": { "codex": { "model": "gpt;5" } } }), "No model id has \";\" in it"),
            (json!({ "projectDefaults": { "p1": { "model": "-m x" } } }), "No model id has a space in it"),
            (json!({ "projectDefaults": { "p1": { "model": "-x" } } }), "No model id starts with \"-\""),
        ];
        for (p, message) in refused {
            let got = answer_json(area.set(patch(&p)).await);
            assert_eq!(got, json!({ "ok": false, "error": { "message": message, "kind": "usage" } }));
        }
        let held = prefs_json(area.get().await);
        assert_eq!(held["agentDefaults"], json!({}));
        assert_eq!(held["projectDefaults"], json!({}));
        assert!(w.puts.borrow().is_empty());
        let landed = answer_json(
            area.set(patch(&json!({ "agentDefaults": { "claude": { "models": { "custom": ["provider/model:tag"] } } } }))).await,
        );
        assert_eq!(landed["preferences"]["agentDefaults"]["claude"]["models"]["custom"], json!(["provider/model:tag"]));
    });
}

// TS: test/permission-relay.test.ts:540
#[test]
fn ported_an_access_word_the_agent_maps_to_no_mode_is_refused_never_run_looser() {
    let fx = fixture();
    run(async {
        let (w, area) = adhoc(&fx, json!({ "name": "permission-relay 540" }));
        let line = "Claude Code takes no plan access; it takes ask, auto-edit, full";
        let got = answer_json(area.set(patch(&json!({ "agentDefaults": { "claude": { "access": "plan" } } }))).await);
        assert_eq!(got, json!({ "ok": false, "error": { "message": line, "kind": "usage" } }));
        let got = answer_json(area.set(patch(&json!({ "projectDefaults": { "p1": { "access": "plan" } } }))).await);
        assert_eq!(got, json!({ "ok": false, "error": { "message": line, "kind": "usage" } }));
        assert!(w.puts.borrow().is_empty());
        let ok = answer_json(area.set(patch(&json!({ "agentDefaults": { "claude": { "access": "auto-edit" } } }))).await);
        assert_eq!(ok["preferences"]["agentDefaults"]["claude"]["access"], json!("auto-edit"));
    });
}

// TS: host/test/verbs-threads.test.ts:392 (the refusal inside the serialized write, nothing stored)
#[test]
fn ported_a_refused_access_word_leaves_the_stored_default_where_it_was() {
    let fx = fixture();
    run(async {
        let (w, area) = adhoc(&fx, json!({ "name": "verbs-threads 392" }));
        for word in ["ask", "full", "auto-edit"] {
            let ok = answer_json(area.set(patch(&json!({ "agentDefaults": { "claude": { "access": word } } }))).await);
            assert_eq!(ok["preferences"]["agentDefaults"]["claude"]["access"], json!(word));
        }
        let puts = w.puts.borrow().len();
        let got = answer_json(area.set(patch(&json!({ "agentDefaults": { "claude": { "access": "plan" } } }))).await);
        assert_eq!(got["error"]["message"], json!("Claude Code takes no plan access; it takes ask, auto-edit, full"));
        assert_eq!(w.puts.borrow().len(), puts);
        assert_eq!(prefs_json(area.get().await)["agentDefaults"]["claude"]["access"], json!("auto-edit"));
    });
}

// F6, TS: test/preferences.test.ts:173
#[test]
fn f6_icons_left_notice() {
    let fx = fixture();
    run(async {
        for name in [
            "icons forget fails",
            "icons forget ok",
            "icons absent",
            "icons message without a code",
            "icons message with a call",
            "icons message with a lower case code",
            "icons message with two codes",
        ] {
            check_script(&fx, name).await;
        }
        let (w, want) = check_script(&fx, "icons forget fails").await;
        assert_eq!(
            result_of(&want, 0)["notice"],
            json!("Server icons are off, but ~/.wsp/icons could not be deleted: permission denied. Delete it by hand.")
        );
        assert_eq!(result_of(&want, 0)["preferences"]["serverIcons"], json!(false));
        assert_eq!(
            w.emitted.borrow()[0]["preferences"]["serverIcons"],
            json!(false),
            "the record is kept and told to every socket either way"
        );
        assert!(result_of(&want, 1).get("notice").is_none(), "a later set carries no notice");
        assert_eq!(w.forgets.get(), 2, "the folder is forgotten at each set that turns icons off, and not at one that does not");
        let (_w, want) = check_script(&fx, "icons absent").await;
        assert!(result_of(&want, 0).get("notice").is_none());
        // A folder outside the home reads whole, the home itself reads as ~.
        for row in fx["folders"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "folder", "icons": { "forget": "EACCES: no" } }));
            *w.icons.as_ref().unwrap().folder.borrow_mut() = row["folder"].as_str().unwrap().to_string();
            let got = answer_json(area.set(patch(&json!({ "serverIcons": false }))).await);
            assert_eq!(got["notice"], row["notice"], "folder {}", row["folder"]);
        }
    });
}

// F7, TS: test/agents-brokered.test.ts:28 and :42
#[test]
fn f7_workspace_read_shape() {
    let fx = fixture();
    run(async {
        for want in fx["agents"]["workspaces"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
            let id = want["id"].as_str().unwrap();
            let origin = Caller::Road(WorkspaceOrigin::Here);
            let got = area.agents_workspace(id, want["origin"].as_bool().unwrap().then_some(&origin)).await;
            match (got, want.get("answer")) {
                (Ok(r), Some(answer)) => assert_eq!(workspace_json(&r).to_string(), answer.to_string(), "{id}"),
                (Err(e), None) => assert_eq!(error_json(&e), want["error"], "{id}"),
                (got, _) => panic!("{id}: {got:?} against {want}"),
            }
            assert_eq!(Value::Array(w.log.borrow().clone()), want["log"], "{id}: what the area asked of the context");
        }
        // The plain objects the TS tests read: a cloud workspace at a provider is brokered, one at no provider is not.
        let by = |id: &str| fx["agents"]["workspaces"].as_array().unwrap().iter().find(|w| w["id"] == json!(id)).unwrap()["answer"].clone();
        assert_eq!(by("wCloud")["brokered"], json!(true));
        assert!(by("wAway").get("brokered").is_none());
        assert!(by("wLocal").get("brokered").is_none());
        assert_eq!(by("wCloud")["stores"]["claude"], json!("/root/.claude-cfg"));
        assert_eq!(by("wCloud")["seeds"]["claude"], json!("/root/.claude-plugin-seed"));
        assert_eq!(by("wLocal")["stores"], json!({}));
        assert_eq!(by("wNap")["pauseMode"], json!("disk"));
    });
}

fn workspace_json(w: &AgentsWorkspace) -> Value {
    let mut out = Map::new();
    out.insert("name".into(), json!(w.name));
    out.insert("phase".into(), json!(w.phase));
    out.insert("local".into(), json!(w.local));
    out.insert("project".into(), json!({ "id": w.project_id, "name": w.project_name, "path": w.project_path }));
    out.insert("stores".into(), Value::Object(w.stores.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
    out.insert("seeds".into(), Value::Object(w.seeds.iter().map(|(k, v)| (k.clone(), json!(v))).collect()));
    if w.brokered {
        out.insert("brokered".into(), json!(true));
    }
    if let Some(m) = &w.pause_mode {
        out.insert("pauseMode".into(), json!(m));
    }
    Value::Object(out)
}

// TS: test/places-agents.test.ts:39
#[test]
fn f7_projects_per_place() {
    let fx = fixture();
    run(async {
        for want in fx["agents"]["projects"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
            let place = want["place"].as_str().unwrap();
            let got = area.agents_projects(place).await;
            let rows: Vec<Value> = got.iter().map(|(id, name, path)| json!({ "id": id, "name": name, "path": path })).collect();
            assert_eq!(Value::Array(rows), want["answer"], "{place}");
            assert_eq!(
                *w.trace.borrow(),
                vec!["ready".to_string(), format!("projects_on {place}")],
                "{place}: the area waits for ready before it reads"
            );
            assert_eq!(Value::Array(w.log.borrow().clone()), want["log"]);
        }
        let box_rows = &fx["agents"]["projects"][0]["answer"];
        assert_eq!(
            box_rows[0],
            json!({ "id": "p1", "name": "landing", "path": "/srv/landing" }),
            "a checkout stands for its project's folder"
        );
        assert_eq!(box_rows[1]["path"], json!("/home/maya/api"));
    });
}

#[test]
fn f7_channel_targets() {
    let fx = fixture();
    run(async {
        for want in fx["agents"]["channels"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
            let target = match (want["target"].get("workspaceId"), want["target"].get("placeId")) {
                (Some(id), _) => ChannelTarget::Workspace(id.as_str().unwrap().to_string()),
                (_, Some(id)) => ChannelTarget::Place(id.as_str().unwrap().to_string()),
                _ => unreachable!(),
            };
            let on_event: Rc<dyn Fn(Value)> = Rc::new(|_| {});
            *w.last_on_event.borrow_mut() = Some(on_event.clone());
            let origin = Caller::Road(WorkspaceOrigin::Here);
            let got = area.agents_channel(target, on_event, want["origin"].as_bool().unwrap().then_some(&origin)).await;
            match (got, want.get("answer")) {
                (Ok(c), Some(answer)) => assert_eq!(Value::Object(c.send(Frame::new()).await.unwrap()), *answer, "{}", want["target"]),
                (Err(e), None) => assert_eq!(error_json(e.host().expect("a plain error")), want["error"], "{}", want["target"]),
                (Ok(_), None) => panic!("{} answered a channel", want["target"]),
                (Err(e), Some(_)) => panic!("{} refused: {}", want["target"], e.message()),
            }
            assert_eq!(Value::Array(w.log.borrow().clone()), want["log"], "{}: what the area asked of the context", want["target"]);
        }
    });
}

#[test]
fn f7_agents_changed_names_its_target() {
    let fx = fixture();
    run(async {
        for want in fx["agents"]["changed"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
            area.agents_changed(want["target"].as_object().map(|_| want["target"].clone()));
            let got: Vec<String> = w.emitted.borrow().iter().map(|e| e.to_string()).collect();
            let wanted: Vec<String> = want["emitted"].as_array().unwrap().iter().map(|e| e.to_string()).collect();
            assert_eq!(got, wanted, "{}", want["target"]);
        }
    });
}

#[test]
fn f7_setup_write_refusal_and_moved() {
    let fx = fixture();
    run(async {
        for want in fx["agents"]["writes"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
            for r in strings(&want["refusals"]) {
                w.refusals.borrow_mut().insert(r);
            }
            *w.set_fails.borrow_mut() = want["fails"].as_str().map(String::from);
            let (place, agent) = (want["place"].as_str().unwrap(), want["agent"].as_str().unwrap());
            let got = area.agents_setup_write(place, agent, want["change"].clone()).await;
            let label = format!("{place}:{agent}");
            match (got, want["ok"].as_bool().unwrap()) {
                (Ok(()), true) => {}
                (Err(e), false) => assert_eq!(error_json(e.host().expect("a host error")), want["error"], "{label}"),
                (got, ok) => panic!("{label}: {} against ok {ok}", if got.is_ok() { "ok".to_string() } else { "refused".to_string() }),
            }
            assert_eq!(Value::Array(w.log.borrow().clone()), want["calls"], "{label}: the calls, in order");
            let mut left: Vec<String> = w.refusals.borrow().iter().cloned().collect();
            left.sort();
            let mut wanted = strings(&want["refusalsLeft"]);
            wanted.sort();
            assert_eq!(left, wanted, "{label}: the refusals still held");
        }
    });
}

// TS: test/thread-defaults.test.ts:238 (the row names the variables only) and the agent with no adapter
#[test]
fn f7_setup_of_is_nothing_for_an_agent_with_no_adapter() {
    let fx = fixture();
    run(async {
        for want in fx["agents"]["setupOf"].as_array().unwrap() {
            let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
            let agent = want["agent"].as_str().unwrap();
            let got = area.agents_setup_of("here", agent);
            assert_eq!(got.clone().unwrap_or(Value::Null), want["answer"], "{agent}");
            assert_eq!(got.is_none(), agent == "ghost");
            let _ = w;
        }
        let claude = fx["agents"]["setupOf"][0]["answer"].clone();
        assert_eq!(claude["envNames"], json!(["A", "B"]));
    });
}

// TS: test/preferences.test.ts:159
#[test]
fn f7_latest_on_follows_the_agent_versions_switch() {
    let fx = fixture();
    run(async {
        let (_w, want) = check_script(&fx, "agent versions switch").await;
        assert_eq!(want["latestBefore"], json!(true));
        assert_eq!(want["latestAfter"], json!([false, false, true]));
    });
}

// TS: test/places-agents.test.ts:363
#[test]
fn f7_icons_are_asked_only_while_the_switch_reads_on() {
    let fx = fixture();
    run(async {
        let (w, area) = adhoc(&fx, json!({ "name": "places-agents 363", "icons": { "forget": null } }));
        assert!(area.icons_on().await.unwrap());
        answer_json(area.set(patch(&json!({ "theme": "dark" }))).await);
        assert_eq!(w.forgets.get(), 0);
        assert!(area.icons_on().await.unwrap());
        let off = answer_json(area.set(patch(&json!({ "serverIcons": false }))).await);
        assert_eq!(off["preferences"]["serverIcons"], json!(false));
        assert_eq!(w.forgets.get(), 1);
        assert!(!area.icons_on().await.unwrap(), "an ask started before the switch went off reads it again");
    });
}

#[test]
fn f7_relayed_reads_the_backends_answer() {
    let fx = fixture();
    run(async {
        let (w, area) = adhoc(&fx, json!({ "name": "agents" }));
        assert_eq!(area.agents_relayed(), fx["agents"]["relayed"].as_bool().unwrap());
        w.relayed.set(!w.relayed.get());
        assert_eq!(area.agents_relayed(), !fx["agents"]["relayed"].as_bool().unwrap());
    });
}

#[test]
fn f7_a_deleted_workspace_is_forgotten_by_the_read() {
    let fx = fixture();
    run(async {
        let (w, _area) = adhoc(&fx, json!({ "name": "agents" }));
        assert!(w.forgot.borrow().is_empty(), "nothing is forgotten at construction");
        let hook = w.hook.borrow().clone().expect("the area registers for workspace.deleted");
        hook("w1");
        hook("w2");
        assert_eq!(*w.forgot.borrow(), vec!["w1".to_string(), "w2".to_string()]);
    });
}
