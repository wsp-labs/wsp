// SPDX-License-Identifier: AGPL-3.0-only
//! The rules (spec/rules.md). Every expected value is the TS's: oracle/rules.oracle.ts ran `rulesArea` on a fake
//! context over the world in tests/fixtures/oracle/rules.json, and `World` below builds the same context from that
//! world. The TS has no test file of its own for rules.ts; `test/spawn-guard.test.ts` (55 cases over a whole host)
//! reaches it, and each of its rules is pinned here at the area's seam.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use common::{error_json, live, oracle, plain, project, record, run};
use wsp_account::account::core_types::Live;
use wsp_account::account::rules::{like_of, CapsHeldAt, RulesArea, RulesCtx, TreeFacts};
use wsp_core::types::internal::WorkspaceLike;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents, WorkspaceOrigin};
use wsp_words::exit::HostError;
use wsp_words::words::computer::{AcrossAct, SpawnAct};

/// The oracle's world as a context.
struct World {
    fixture: Value,
    live: Vec<Live>,
    projects: Rc<RefCell<IndexMap<String, ProjectView>>>,
    landing: Rc<RefCell<IndexMap<String, i64>>>,
    puts: RefCell<Vec<ProjectView>>,
    pref_sets: RefCell<Vec<Value>>,
    target: Option<String>,
    no_machines_line: Option<String>,
}

fn world() -> Rc<World> {
    let fixture = oracle("rules");
    let w = &fixture["world"];
    let live = w["live"].as_array().unwrap().iter().map(|e| {
        let entry = live(record(&e["record"]));
        entry.creating.set(e["creating"] == json!(true));
        entry
    });
    let projects = w["projects"].as_array().unwrap().iter().map(|p| (p["id"].as_str().unwrap().to_string(), project(p))).collect();
    let landing = w["landing"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap())).collect();
    Rc::new(World {
        live: live.collect(),
        projects: Rc::new(RefCell::new(projects)),
        landing: Rc::new(RefCell::new(landing)),
        puts: RefCell::new(vec![]),
        pref_sets: RefCell::new(vec![]),
        target: Some("wC1".into()),
        no_machines_line: None,
        fixture,
    })
}

impl World {
    fn w(&self) -> &Value {
        &self.fixture["world"]
    }
    fn repo_of(&self, id: &str) -> Option<String> {
        self.w()["projects"].as_array().unwrap().iter().find(|p| p["id"] == json!(id)).map(|p| p["repo"].as_str().unwrap().to_string())
    }
    fn facts(v: &Value) -> TreeFacts {
        TreeFacts {
            workspace_id: v["workspaceId"].as_str().map(String::from),
            parent_thread_id: v["parentThreadId"].as_str().map(String::from),
            root_thread_id: v["rootThreadId"].as_str().map(String::from),
        }
    }
    fn agents(v: &Value) -> WorkspaceAgents {
        // wsp-wire's WorkspaceAgents still carries `spawn` (a3b870a6e); the cut dropped it. Drift named in the report.
        let mut v = v.clone();
        v["spawn"] = json!(true);
        serde_json::from_value(v).unwrap()
    }
}

#[async_trait(?Send)]
impl RulesCtx for World {
    fn no_machines_line(&self) -> Option<String> {
        self.no_machines_line.clone()
    }
    async fn store_put_project(&self, project: &ProjectView) -> Result<(), HostError> {
        self.puts.borrow_mut().push(project.clone());
        Ok(())
    }
    fn door_name_of(&self, place_id: &str) -> Option<String> {
        Some(self.place_name(place_id))
    }
    fn landing(&self) -> Rc<RefCell<IndexMap<String, i64>>> {
        self.landing.clone()
    }
    fn live(&self) -> Vec<Live> {
        self.live.clone()
    }
    fn live_get(&self, id: &str) -> Option<Live> {
        self.live.iter().find(|e| e.record.borrow().id == id).cloned()
    }
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>> {
        self.projects.clone()
    }
    fn sessions_size(&self) -> usize {
        self.w()["sessionsSize"].as_u64().unwrap() as usize
    }
    fn thread_records_size(&self) -> usize {
        self.w()["threadRecords"].as_array().unwrap().len()
    }
    fn thread_record(&self, thread_id: &str) -> Option<TreeFacts> {
        self.w()["threadRecords"].as_array().unwrap().iter().find(|t| t["threadId"] == json!(thread_id)).map(World::facts)
    }
    fn rows_on(&self, thread_id: &str) -> Vec<TreeFacts> {
        self.w()["rows"][thread_id].as_array().map(|rows| rows.iter().map(World::facts).collect()).unwrap_or_default()
    }
    async fn target_workspace(&self) -> Result<Option<String>, HostError> {
        Ok(self.target.clone())
    }
    async fn set_preferences(&self, patch: Value) -> Result<(), HostError> {
        self.pref_sets.borrow_mut().push(patch);
        Ok(())
    }
    fn capability(&self, record: &WorkspaceRecord, can: &str) -> bool {
        record.kind == WorkspaceKind::Cloud && record.place.as_deref() != Some("bare") && can == "snapshot"
    }
    fn pause_mode_declared(&self, record: &WorkspaceRecord) -> bool {
        record.kind == WorkspaceKind::Cloud && record.place.as_deref() != Some("bare")
    }
    fn forks_no_machines(&self, record: &WorkspaceRecord) -> bool {
        !(record.kind == WorkspaceKind::Cloud && record.place.as_deref() != Some("bare"))
    }
    fn relayed(&self, kind: WorkspaceKind, machine_id: Option<&str>) -> bool {
        kind == WorkspaceKind::Cloud && machine_id.is_some_and(|m| !m.is_empty())
    }
    fn turn_reach(&self, entry: &Live) -> Option<Map<String, Value>> {
        let r = entry.record.borrow();
        match r.kind {
            WorkspaceKind::Cloud => Some(json!({ "url": format!("wss://host/{}", r.id) }).as_object().unwrap().clone()),
            WorkspaceKind::Place => Some(Map::new()),
            _ => None,
        }
    }
    fn wsp_mcp(&self, entry: &Live) -> Option<Value> {
        let r = entry.record.borrow();
        (r.kind == WorkspaceKind::Cloud).then(|| json!({ "command": "wsp", "args": ["mcp", r.id] }))
    }
    fn same_repository(&self, a: &ProjectView, b: &ProjectView, _caller: Option<&Caller>) -> bool {
        self.repo_of(&a.id) == self.repo_of(&b.id)
    }
    fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        self.projects.borrow().get(id).cloned().ok_or_else(|| plain(&format!("no project {id}")))
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        if computer == "here" {
            WorkspaceKind::Local
        } else if self.w()["joined"].as_array().unwrap().contains(&json!(computer)) {
            WorkspaceKind::Place
        } else {
            WorkspaceKind::Cloud
        }
    }
    fn place_name(&self, id: &str) -> String {
        self.w()["placeNames"][id].as_str().unwrap_or(id).to_string()
    }
    fn agents_held(&self, record: &WorkspaceRecord) -> WorkspaceAgents {
        if let Some(a) = &record.agents {
            return a.clone();
        }
        let at = self.place_id_of(record).unwrap_or_default();
        let placed = &self.w()["placeAgents"][at.as_str()];
        let fallback = json!({ "maxMachines": 3, "maxDepth": 1 });
        World::agents(if placed.is_null() { &fallback } else { placed })
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        match record.kind {
            WorkspaceKind::Local => Some("here".into()),
            WorkspaceKind::Cloud => Some(record.place.clone().unwrap_or("solari".into())),
            _ => record.place.clone(),
        }
    }
}

fn area(w: &Rc<World>) -> Rc<RulesArea> {
    RulesArea::new(w.clone())
}

fn caller(fixture: &Value, name: &str) -> Option<Caller> {
    let c = &fixture["callers"][name];
    match c {
        Value::Null => None,
        Value::String(_) => Some(Caller::Road(serde_json::from_value::<WorkspaceOrigin>(c.clone()).unwrap())),
        _ => Some(Caller::Scoped { origin: serde_json::from_value(c["origin"].clone()).unwrap(), by: serde_json::from_value::<ThreadScope>(c["by"].clone()).unwrap() }),
    }
}

fn caller_names(fixture: &Value) -> Vec<String> {
    fixture["callers"].as_object().unwrap().keys().cloned().collect()
}

fn record_of(w: &World, id: &str) -> WorkspaceRecord {
    w.live_get(id).unwrap().record.borrow().clone()
}

/// The record a matrix row names: a live one, `none`, or the create-shaped `noProject`.
fn like_named(w: &World, id: &str) -> Option<WorkspaceLike> {
    match id {
        "none" => None,
        "noProject" => Some(WorkspaceLike { id: None, kind: WorkspaceKind::Cloud, name: "nameless".into(), machine_id: Some(String::new()), root_thread_id: None, project: None, worktree: None }),
        _ => Some(like_of(&record_of(w, id))),
    }
}

/// The oracle's answer encoding: `null` for nothing, a value, or `{ thrown }`.
fn answer<T: serde::Serialize>(got: Result<T, HostError>) -> Value {
    match got {
        Ok(v) => serde_json::to_value(v).unwrap(),
        Err(e) => json!({ "thrown": error_json(&e) }),
    }
}

fn unit(got: Result<(), HostError>) -> Value {
    match got {
        Ok(()) => Value::Null,
        Err(e) => json!({ "thrown": error_json(&e) }),
    }
}

fn opt_err(got: Result<Option<HostError>, HostError>) -> Value {
    match got {
        Ok(None) => Value::Null,
        Ok(Some(e)) => error_json(&e),
        Err(e) => json!({ "thrown": error_json(&e) }),
    }
}

fn across(word: &str) -> AcrossAct {
    serde_json::from_value(json!(word)).unwrap()
}

fn act(word: &str) -> SpawnAct {
    serde_json::from_value(json!(word)).unwrap()
}

fn ids(list: &[Live]) -> Vec<String> {
    list.iter().map(|e| e.record.borrow().id.clone()).collect()
}

#[test]
fn r14_refusal_for_matrix() {
    let w = world();
    let rules = area(&w);
    let mut wrong = vec![];
    for row in w.fixture["refusalFor"].as_array().unwrap() {
        let c = caller(&w.fixture, row["caller"].as_str().unwrap());
        let rec = like_named(&w, row["record"].as_str().unwrap());
        let got = answer(rules.refusal_for(rec.as_ref(), c.as_ref()));
        if got != row["answer"] {
            wrong.push(format!("{} / {}: {got} != {}", row["caller"], row["record"], row["answer"]));
        }
    }
    assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), w.fixture["refusalFor"].as_array().unwrap().len(), wrong.join("\n"));
}

#[test]
fn r14_project_rule_before_computer_rule_before_tree_rule() {
    let w = world();
    let rules = area(&w);
    // t4 runs on studio under t1: beta-studio is on its own computer but another repository; alpha-here is its
    // repository on another computer; alpha-other is on Solari, another tree.
    let c = caller(&w.fixture, "t4");
    assert_eq!(
        rules.refusal_for(Some(&like_of(&record_of(&w, "wBb"))), c.as_ref()).unwrap().as_deref(),
        Some("thread t4 works on alpha-studio, and beta-studio is a project of another repository; a thread starts children on its own repository's projects alone")
    );
    assert_eq!(
        rules.refusal_for(Some(&like_of(&record_of(&w, "wL"))), c.as_ref()).unwrap().as_deref(),
        Some("alpha-here is on this computer, and a thread on studio cannot work there yet; ask thread t1, your lead, with wsp send t1 \"<message>\"")
    );
    let t2 = caller(&w.fixture, "t2");
    assert_eq!(
        rules.refusal_for(Some(&like_of(&record_of(&w, "wC2"))), t2.as_ref()).unwrap().as_deref(),
        Some("thread t2 may drive the workspace it runs on and the ones it forked, and alpha-other is neither")
    );
}

#[test]
fn r14_no_record_is_nobodys_to_refuse() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        assert_eq!(rules.refusal_for(None, caller(&w.fixture, &name).as_ref()).unwrap(), None, "{name}");
    }
}

#[test]
fn invariant_3_the_rules_read_the_stamped_scope_alone() {
    let w = world();
    let rules = area(&w);
    // The same request on the same road reads differently only by the scope stamped on it: a road with no scope
    // reaches alpha-other, the scope of t2 (on alpha-fork under t1) does not, and nothing else of the request counts.
    let other = like_of(&record_of(&w, "wC2"));
    assert_eq!(rules.refusal_for(Some(&other), Some(&Caller::Road(WorkspaceOrigin::Here))).unwrap(), None);
    let scoped = caller(&w.fixture, "t2");
    assert!(rules.refusal_for(Some(&other), scoped.as_ref()).unwrap().is_some());
    let relayed_t2 = caller(&w.fixture, "tRelayed");
    assert_eq!(answer(rules.refusal_for(Some(&other), relayed_t2.as_ref())), json!("thread t2 may drive the workspace it runs on and the ones it forked, and alpha-other is neither"));
}

#[test]
fn r17_listed_for_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        let got = rules.listed_for(caller(&w.fixture, &name).as_ref()).map(|l| ids(&l));
        assert_eq!(answer(got), w.fixture["listedFor"][&name], "{name}");
    }
}

#[test]
fn r17_a_creating_record_is_never_listed() {
    let w = world();
    let rules = area(&w);
    assert!(!ids(&rules.held()).contains(&"wNew".to_string()));
    assert_eq!(ids(&rules.held()).len(), 14);
    for name in caller_names(&w.fixture) {
        assert!(!ids(&rules.listed_for(caller(&w.fixture, &name).as_ref()).unwrap()).contains(&"wNew".to_string()), "{name}");
    }
}

#[test]
fn r15_refuse_relayed_matrix() {
    let w = world();
    let rules = area(&w);
    let mut wrong = vec![];
    for row in w.fixture["refuseRelayed"].as_array().unwrap() {
        let c = caller(&w.fixture, row["caller"].as_str().unwrap());
        let rec = like_named(&w, row["record"].as_str().unwrap());
        let got = unit(rules.refuse_relayed(rec.as_ref(), c.as_ref(), across(row["act"].as_str().unwrap()), row["thread"].as_str()));
        if got != row["answer"] {
            wrong.push(format!("{} / {} / {}: {got} != {}", row["caller"], row["record"], row["thread"], row["answer"]));
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn r16_refuse_named_matrix() {
    let w = world();
    let rules = area(&w);
    for row in w.fixture["refuseNamed"].as_array().unwrap() {
        let c = caller(&w.fixture, row["caller"].as_str().unwrap());
        let got = unit(rules.refuse_named(row["workspaceId"].as_str().unwrap(), c.as_ref()));
        assert_eq!(got, row["answer"], "{} / {}", row["caller"], row["workspaceId"]);
    }
}

#[test]
fn r16_a_thread_reads_absence_for_an_unknown_id() {
    let w = world();
    let rules = area(&w);
    let e = rules.refuse_named("wNowhere", caller(&w.fixture, "t1").as_ref()).unwrap_err();
    assert_eq!(error_json(&e), json!({ "message": "no such workspace", "kind": "not-found" }));
    // The same words for an id it does hold but may not reach.
    let e = rules.refuse_named("wC2", caller(&w.fixture, "t1").as_ref()).unwrap_err();
    assert_eq!(error_json(&e), json!({ "message": "no such workspace", "kind": "not-found" }));
    // A person names nothing absent.
    assert!(rules.refuse_named("wNowhere", None).is_ok());
}

#[test]
fn r7_of_threads_repository_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        for (p, want) in w.fixture["ofThreadsRepository"][&name].as_object().unwrap() {
            assert_eq!(answer(rules.of_threads_repository(caller(&w.fixture, &name).as_ref(), p)), *want, "{name} / {p}");
        }
    }
}

#[test]
fn r8_project_reached_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        for (p, want) in w.fixture["projectReached"][&name].as_object().unwrap() {
            assert_eq!(answer(rules.project_reached(caller(&w.fixture, &name).as_ref(), p)), *want, "{name} / {p}");
        }
    }
}

#[test]
fn r9_opens_in_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        for (id, want) in w.fixture["opensIn"][&name].as_object().unwrap() {
            assert_eq!(answer(rules.opens_in(&record_of(&w, id), caller(&w.fixture, &name).as_ref())), *want, "{name} / {id}");
        }
    }
}

#[test]
fn r11_elsewhere_refusal_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        let want = &w.fixture["elsewhereRefusal"][&name];
        for p in w.fixture["world"]["projects"].as_array().unwrap() {
            let id = p["id"].as_str().unwrap();
            let word = format!("the {} word", p["name"].as_str().unwrap());
            let got = opt_err(rules.elsewhere_refusal(caller(&w.fixture, &name).as_ref(), &project(p), &word));
            // The oracle caught a throw around the whole map, so a thrown row reads as the caller's whole answer.
            if want.get("thrown").is_some() {
                assert_eq!(got.get("thrown"), want.get("thrown"), "{name} / {id}");
            } else {
                assert_eq!(got, want[id], "{name} / {id}");
            }
        }
    }
}

#[test]
fn r12_acts_on_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        for (id, want) in w.fixture["actsOn"][&name].as_object().unwrap() {
            assert_eq!(json!(rules.acts_on(&like_of(&record_of(&w, id)), caller(&w.fixture, &name).as_ref())), *want, "{name} / {id}");
        }
    }
}

#[test]
fn r12_away_for_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        for (id, want) in w.fixture["awayFor"][&name].as_object().unwrap() {
            let got = opt_err(rules.away_for(&like_of(&record_of(&w, id)), caller(&w.fixture, &name).as_ref(), AcrossAct::Wake, &format!("\"{id}\"")));
            assert_eq!(got, *want, "{name} / {id}");
        }
    }
}

#[test]
fn r12_lead_acts_on_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        for (id, want) in w.fixture["leadActsOn"][&name].as_object().unwrap() {
            assert_eq!(answer(rules.lead_acts_on(&like_of(&record_of(&w, id)), caller(&w.fixture, &name).as_ref())), *want, "{name} / {id}");
        }
    }
}

#[test]
fn r13_talks_to_its_tree_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        assert_eq!(answer(rules.talks_to_its_tree(caller(&w.fixture, &name).as_ref())), w.fixture["talksToItsTree"][&name], "{name}");
    }
}

#[test]
fn r5_drives_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        let c = caller(&w.fixture, &name);
        for (id, want) in w.fixture["drives"][&name].as_object().unwrap() {
            let got = match id.as_str() {
                "unheld" => rules.drives(WorkspaceKind::Cloud, Some("mx"), c.as_ref()),
                "noMachine" => rules.drives(WorkspaceKind::Cloud, None, c.as_ref()),
                _ => {
                    let r = record_of(&w, id);
                    rules.drives(r.kind, Some(r.machine_id.as_str()), c.as_ref())
                }
            };
            assert_eq!(json!(got), *want, "{name} / {id}");
        }
    }
}

#[test]
fn r18_refuse_recording_matrix() {
    let w = world();
    let rules = area(&w);
    for name in caller_names(&w.fixture) {
        assert_eq!(unit(rules.refuse_recording("box9", caller(&w.fixture, &name).as_ref())), w.fixture["refuseRecording"][&name], "{name}");
    }
}

#[test]
fn r19_parent_of_table() {
    let w = world();
    let rules = area(&w);
    for (t, want) in w.fixture["parentOf"].as_object().unwrap() {
        assert_eq!(json!(rules.parent_of(t)), *want, "{t}");
    }
}

#[test]
fn r19_root_of_table() {
    let w = world();
    let rules = area(&w);
    for (t, want) in w.fixture["rootOf"].as_object().unwrap() {
        assert_eq!(json!(rules.root_of(t)), *want, "{t}");
    }
}

#[test]
fn r20_agents_of_table() {
    let w = world();
    let rules = area(&w);
    for (id, want) in w.fixture["agentsOf"].as_object().unwrap() {
        let got = rules.agents_of(&record_of(&w, id)).map(|a| json!({ "maxMachines": a.max_machines, "maxDepth": a.max_depth }));
        assert_eq!(json!(got), *want, "{id}");
    }
}

#[test]
fn r21_caps_held_at_names_the_workspace_or_the_computer() {
    let w = world();
    let rules = area(&w);
    assert_eq!(rules.agents_held_at(&record_of(&w, "wCa")), CapsHeldAt::Workspace("alpha-capped".into()));
    assert_eq!(rules.agents_held_at(&record_of(&w, "wC1")), CapsHeldAt::Computer("this computer".into()));
    assert_eq!(rules.agents_held_at(&record_of(&w, "wB1")), CapsHeldAt::Computer("studio".into()));
}

#[test]
fn r22_a_parent_loop_is_bounded() {
    let w = world();
    let rules = area(&w);
    // The walk's bound: 4 sessions + 12 thread records + 1 steps, each a parent counted.
    assert_eq!(rules.depth_under_root("tLoopA"), 4 + 12 + 1);
    assert_eq!(rules.depth_under_root("t6"), 2);
    assert_eq!(rules.depth_under_root("t1"), 0);
    assert_eq!(rules.depth_under_root("tOld"), 1);
}

#[test]
fn r3_r4_refuse_cannot_matrix() {
    let w = world();
    let rules = area(&w);
    for (id, cans) in w.fixture["refuseCannot"].as_object().unwrap() {
        let entry = w.live_get(id).unwrap();
        for (can, want) in cans.as_object().unwrap() {
            assert_eq!(unit(rules.refuse_cannot(&entry, can, &format!("take a {can}"))), *want, "{id} / {can}");
        }
    }
}

#[test]
fn r3_no_machines_line_outranks_the_provider_line() {
    let fixture = oracle("rules");
    let mut w = Rc::try_unwrap(world()).ok().unwrap();
    w.no_machines_line = Some("No provider here: add one with wsp add solari.".into());
    let w = Rc::new(w);
    let rules = area(&w);
    let entry = w.live_get("wC4").unwrap();
    assert_eq!(unit(rules.refuse_cannot(&entry, "snapshot", "take a snapshot")), fixture["refuseCannotNoMachinesLine"]);
}

#[test]
fn r4_refuse_pauseless_matrix() {
    let w = world();
    let rules = area(&w);
    for (id, want) in w.fixture["refusePauseless"].as_object().unwrap() {
        assert_eq!(unit(rules.refuse_pauseless(&w.live_get(id).unwrap(), "nap")), *want, "{id}");
    }
}

#[test]
fn r4_pauses_and_holds_slot_matrix() {
    let w = world();
    let rules = area(&w);
    for (id, want) in w.fixture["pauses"].as_object().unwrap() {
        assert_eq!(json!(rules.pauses(&record_of(&w, id))), *want, "pauses {id}");
    }
    for (id, want) in w.fixture["holdsSlot"].as_object().unwrap() {
        assert_eq!(json!(rules.holds_slot(&record_of(&w, id))), *want, "holdsSlot {id}");
    }
}

#[test]
fn r23_spawn_guard_matrix() {
    let fixture = oracle("rules");
    let mut wrong = vec![];
    for row in fixture["spawnGuard"].as_array().unwrap() {
        let w = world();
        let rules = area(&w);
        let c = caller(&fixture, row["caller"].as_str().unwrap());
        let got = match rules.spawn_guard(act(row["act"].as_str().unwrap()), c.as_ref()) {
            Ok(_) => json!("free"),
            Err(e) => json!({ "thrown": error_json(&e) }),
        };
        let landing = json!(w.landing.borrow().clone());
        if got != row["answer"] || landing != row["landing"] {
            wrong.push(format!("{} / {}: {got} {landing} != {} {}", row["caller"], row["act"], row["answer"], row["landing"]));
        }
    }
    assert!(wrong.is_empty(), "{} differ:\n{}", wrong.len(), wrong.join("\n"));
}

#[test]
fn r23_landing_slot_taken_in_the_same_step_and_freed_once() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    let t3 = caller(&fixture, "t3");
    let steps = fixture["slots"].as_array().unwrap();
    let landing = || json!(w.landing.borrow().clone());
    let a = rules.spawn_guard(SpawnAct::Fork, t3.as_ref()).unwrap();
    assert_eq!(landing(), steps[0]["landing"], "first");
    let b = rules.spawn_guard(SpawnAct::Fork, t3.as_ref()).unwrap();
    assert_eq!(landing(), steps[1]["landing"], "second");
    let c = rules.spawn_guard(SpawnAct::Fork, t3.as_ref()).unwrap();
    assert_eq!(landing(), steps[2]["landing"], "third");
    a();
    assert_eq!(landing(), steps[3]["landing"], "free first");
    a();
    assert_eq!(landing(), steps[4]["landing"], "free first again");
    b();
    c();
    assert_eq!(landing(), steps[5]["landing"], "free rest");
    // Freed to nothing, the root's key goes: t3 started with one slot held by the world.
    assert_eq!(landing(), json!({ "t3": 1 }));
}

#[test]
fn r23_a_sub_thread_starts_nothing() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    let e = rules.spawn_guard(SpawnAct::ThreadNew, caller(&fixture, "t5").as_ref()).err().unwrap();
    assert_eq!(
        error_json(&e),
        json!({ "message": "sub-thread t5 starts no threads of its own. Send the work to thread t1, which started it, with wsp send t1 \"<message>\".", "fix": "Send the work to thread t1, which started it, with wsp send t1 \"<message>\".", "kind": "usage" })
    );
    assert!(rules.spawn_guard(SpawnAct::Send, caller(&fixture, "t5").as_ref()).is_ok());
}

#[test]
fn r23_send_bring_back_delete_sub_pass_at_the_depth_cap() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    let t6 = caller(&fixture, "t6");
    assert!(rules.spawn_guard(SpawnAct::ThreadNew, t6.as_ref()).is_err());
    for a in [SpawnAct::Send, SpawnAct::BringBack, SpawnAct::DeleteSub] {
        assert!(rules.spawn_guard(a, t6.as_ref()).is_ok(), "{a:?}");
    }
    assert_eq!(json!(w.landing.borrow().clone()), json!({ "t3": 1 }));
}

#[test]
fn r23_a_creating_record_counts_against_the_cap() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    // t1's tree holds alpha-fork, beta-fork, alpha-sub and the creating alpha-new; the gone and released ones free
    // their places.
    let e = rules.spawn_guard(SpawnAct::Fork, caller(&fixture, "t1").as_ref()).err().unwrap();
    assert_eq!(e.message, "thread t1 already holds 4 of its 3 machines; delete one before forking another");
    assert_eq!(e.kind, None);
}

#[test]
fn r24_a_root_with_no_caps_fails_as_the_ts_does() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    let e = rules.spawn_guard(SpawnAct::ThreadNew, caller(&fixture, "tO2").as_ref()).err().unwrap();
    assert_eq!(error_json(&e), json!({ "message": "Cannot read properties of undefined (reading 'maxDepth')" }));
}

#[test]
fn r25_agents_reach_table() {
    let w = world();
    let rules = area(&w);
    for (id, want) in w.fixture["agentsReach"].as_object().unwrap() {
        let got = rules.agents_reach(&w.live_get(id).unwrap());
        assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(want).unwrap(), "{id}");
    }
}

#[test]
fn r26_tree_of() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    assert_eq!(rules.tree_of(None), Default::default());
    let t2: ThreadScope = serde_json::from_value(fixture["callers"]["t2"]["by"].clone()).unwrap();
    let got = rules.tree_of(Some(&t2));
    assert_eq!(json!({ "parentThreadId": got.parent_thread_id, "rootThreadId": got.root_thread_id }), fixture["treeOf"]["t2"]);
}

#[test]
fn r26_place_of_thread_table() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    for (name, want) in fixture["placeOfThread"].as_object().unwrap() {
        let scope: ThreadScope = serde_json::from_value(fixture["callers"][name]["by"].clone()).unwrap();
        assert_eq!(answer(rules.place_of_thread(&scope)), *want, "{name}");
    }
}

#[test]
fn r26_act_refusal_table() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    for (name, want) in fixture["actRefusal"].as_object().unwrap() {
        let scope: ThreadScope = serde_json::from_value(fixture["callers"][name]["by"].clone()).unwrap();
        let got = rules.act_refusal(&scope, SpawnAct::Merge);
        assert_eq!(got.map(|e| error_json(&e)).unwrap_or_else(|e| json!({ "thrown": error_json(&e) })), *want, "{name}");
    }
}

#[test]
fn r1_remember_project_moves_the_map_then_the_store() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    let mut renamed = fixture["world"]["projects"][0].clone();
    renamed["name"] = json!("alpha renamed");
    run(async {
        rules.remember_project(project(&renamed)).await.unwrap();
    });
    let order: Vec<String> = w.projects.borrow().keys().cloned().collect();
    assert_eq!(json!(order), fixture["remember"]["mapOrder"]);
    assert_eq!(w.projects.borrow()["pA"].name, "alpha renamed");
    let puts = w.puts.borrow();
    assert_eq!(puts.len(), 1);
    assert_eq!(puts[0].id, "pA");
    assert_eq!(puts[0].name, "alpha renamed");
}

#[test]
fn r2_remember_target_writes_nothing_when_it_already_names_it() {
    let w = world();
    let rules = area(&w);
    run(async {
        rules.remember_target(&record_of(&w, "wC1")).await.unwrap();
    });
    assert!(w.pref_sets.borrow().is_empty());
}

#[test]
fn r2_remember_target_sets_workspace_alone() {
    let fixture = oracle("rules");
    let w = world();
    let rules = area(&w);
    run(async {
        rules.remember_target(&record_of(&w, "wB1")).await.unwrap();
    });
    assert_eq!(json!(*w.pref_sets.borrow()), fixture["remember"]["newTargetSets"]);
}
