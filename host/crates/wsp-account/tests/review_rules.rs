// SPDX-License-Identifier: AGPL-3.0-only
//! Review probe: the rules over the generated worlds of oracle/zz-review-rules-fuzz.oracle.ts, the same fake context
//! as tests/rules.rs, every answer compared with the TS's.
mod common;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use common::{error_json, live, plain, project, record};
use wsp_account::account::core_types::Entry;
use wsp_account::account::rules::{like_of, RulesArea, RulesCtx, TreeFacts};
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents, WorkspaceOrigin};
use wsp_words::exit::HostError;
use wsp_words::words::computer::{AcrossAct, SpawnAct};

struct World {
    w: Value,
    live: Vec<Entry>,
    projects: Rc<RefCell<IndexMap<String, ProjectView>>>,
    landing: Rc<RefCell<IndexMap<String, i64>>>,
}

fn world(w: &Value) -> Rc<World> {
    let live = w["live"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let entry = live(record(&e["record"]));
            entry.borrow_mut().creating = e["creating"] == json!(true);
            entry
        })
        .collect();
    let projects = w["projects"].as_array().unwrap().iter().map(|p| (p["id"].as_str().unwrap().to_string(), project(p))).collect();
    let landing = w["landing"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap())).collect();
    Rc::new(World { w: w.clone(), live, projects: Rc::new(RefCell::new(projects)), landing: Rc::new(RefCell::new(landing)) })
}

impl World {
    fn repo_of(&self, id: &str) -> Option<String> {
        self.w["projects"].as_array().unwrap().iter().find(|p| p["id"] == json!(id)).map(|p| p["repo"].as_str().unwrap().to_string())
    }
    fn facts(v: &Value) -> TreeFacts {
        TreeFacts {
            workspace_id: v["workspaceId"].as_str().map(String::from),
            parent_thread_id: v["parentThreadId"].as_str().map(String::from),
            root_thread_id: v["rootThreadId"].as_str().map(String::from),
        }
    }
}

#[async_trait(?Send)]
impl RulesCtx for World {
    fn no_machines_line(&self) -> Option<String> {
        None
    }
    async fn store_put_project(&self, _project: &ProjectView) -> Result<(), HostError> {
        Ok(())
    }
    fn door_name_of(&self, place_id: &str) -> Option<String> {
        Some(self.place_name(place_id))
    }
    fn landing(&self) -> Rc<RefCell<IndexMap<String, i64>>> {
        self.landing.clone()
    }
    fn live(&self) -> Vec<Entry> {
        self.live.clone()
    }
    fn live_get(&self, id: &str) -> Option<Entry> {
        self.live.iter().find(|e| e.borrow().record.borrow().id == id).cloned()
    }
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>> {
        self.projects.clone()
    }
    fn sessions_size(&self) -> usize {
        self.w["sessionsSize"].as_u64().unwrap() as usize
    }
    fn thread_records_size(&self) -> usize {
        self.w["threadRecords"].as_array().unwrap().len()
    }
    fn thread_record(&self, thread_id: &str) -> Option<ThreadRecord> {
        self.w["threadRecords"].as_array().unwrap().iter().find(|t| t["threadId"] == json!(thread_id)).map(|t| {
            let facts = World::facts(t);
            ThreadRecord {
                workspace_id: facts.workspace_id.unwrap_or_default(),
                harness: "claude".to_owned(),
                parent_thread_id: facts.parent_thread_id,
                root_thread_id: facts.root_thread_id,
                ..ThreadRecord::default()
            }
        })
    }
    fn rows_on(&self, thread_id: &str) -> Vec<TreeFacts> {
        self.w["rows"][thread_id].as_array().map(|rows| rows.iter().map(World::facts).collect()).unwrap_or_default()
    }
    async fn target_workspace(&self) -> Result<Option<String>, HostError> {
        Ok(None)
    }
    async fn set_preferences(&self, _patch: Value) -> Result<(), HostError> {
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
    fn turn_reach(&self, _entry: &Entry) -> Option<Map<String, Value>> {
        None
    }
    fn wsp_mcp(&self, _entry: &Entry) -> Option<Value> {
        None
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
        } else if self.w["joined"].as_array().unwrap().contains(&json!(computer)) {
            WorkspaceKind::Place
        } else {
            WorkspaceKind::Cloud
        }
    }
    fn place_name(&self, id: &str) -> String {
        self.w["placeNames"][id].as_str().unwrap_or(id).to_string()
    }
    fn agents_held(&self, record: &WorkspaceRecord) -> WorkspaceAgents {
        if let Some(a) = &record.agents {
            return a.clone();
        }
        let at = self.place_id_of(record).unwrap_or_default();
        let placed = &self.w["placeAgents"][at.as_str()];
        let fallback = json!({ "maxMachines": 3, "maxDepth": 1 });
        serde_json::from_value(if placed.is_null() { fallback } else { placed.clone() }).unwrap()
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        match record.kind {
            WorkspaceKind::Local => Some("here".into()),
            WorkspaceKind::Cloud => Some(record.place.clone().unwrap_or("solari".into())),
            _ => record.place.clone(),
        }
    }
}

fn caller(c: &Value) -> Option<Caller> {
    match c {
        Value::Null => None,
        Value::String(_) => Some(Caller::Road(serde_json::from_value::<WorkspaceOrigin>(c.clone()).unwrap())),
        _ => Some(Caller::Scoped {
            origin: serde_json::from_value(c["origin"].clone()).unwrap(),
            by: serde_json::from_value::<ThreadScope>(c["by"].clone()).unwrap(),
        }),
    }
}

fn answer<T: serde::Serialize>(got: Result<T, HostError>) -> Value {
    match got {
        Ok(v) => serde_json::to_value(v).unwrap(),
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

fn ids(list: &[Entry]) -> Vec<String> {
    list.iter().map(|e| e.borrow().record.borrow().id.clone()).collect()
}

fn rec(w: &World, id: &str) -> WorkspaceRecord {
    w.live_get(id).unwrap().borrow().record.borrow().clone()
}

/// Every world, every door: the rows that differ, by door.
#[test]
fn review_rules_fuzz_against_the_ts() {
    let path = format!("{}/tests/fixtures/oracle/zz_review_rules_fuzz.json", env!("CARGO_MANIFEST_DIR"));
    let worlds: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut wrong: IndexMap<String, Vec<String>> = IndexMap::new();
    let mut asked = 0usize;
    let mut note = |door: &str, seed: &Value, at: String, got: &Value, want: &Value| {
        asked += 1;
        if got != want {
            wrong.entry(door.to_string()).or_default().push(format!("seed {seed} {at}: rust {got} ts {want}"));
        }
    };
    for item in worlds.as_array().unwrap() {
        let seed = &item["seed"];
        let wv = &item["world"];
        let a = &item["answers"];
        let w = world(wv);
        let rules = RulesArea::new(w.clone());
        let callers: Vec<(String, Option<Caller>)> =
            wv["callers"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), caller(v))).collect();
        let rec_ids: Vec<String> = wv["live"].as_array().unwrap().iter().map(|e| e["record"]["id"].as_str().unwrap().to_string()).collect();
        for (cn, c) in &callers {
            let c = c.as_ref();
            for id in &rec_ids {
                let like = like_of(&rec(&w, id));
                let r = rec(&w, id);
                note("refusalFor", seed, format!("{cn}/{id}"), &answer(rules.refusal_for(Some(&like), c)), &a["refusalFor"][cn][id]);
                let unit = |got: Result<(), HostError>| match got {
                    Ok(()) => Value::Null,
                    Err(e) => json!({ "thrown": error_json(&e) }),
                };
                note(
                    "refuseRelayed work",
                    seed,
                    format!("{cn}/{id}"),
                    &unit(rules.refuse_relayed(Some(&like), c, AcrossAct::Work, None)),
                    &a["refuseRelayedWork"][cn][id],
                );
                note(
                    "refuseRelayed thread",
                    seed,
                    format!("{cn}/{id}"),
                    &unit(rules.refuse_relayed(Some(&like), c, AcrossAct::Exec, Some("t2"))),
                    &a["refuseRelayedThread"][cn][id],
                );
                let want_acts = &a["actsOn"][cn][id];
                // A TS throw here is the kept departure (false for a throw); compare only where the TS answers.
                if want_acts.get("thrown").is_none() {
                    note("actsOn", seed, format!("{cn}/{id}"), &json!(rules.acts_on(&like, c)), want_acts);
                }
                note("opensIn", seed, format!("{cn}/{id}"), &answer(rules.opens_in(&r, c)), &a["opensIn"][cn][id]);
                note("leadActsOn", seed, format!("{cn}/{id}"), &answer(rules.lead_acts_on(&like, c)), &a["leadActsOn"][cn][id]);
                let away = match rules.away_for(&like, c, AcrossAct::Wake, &format!("\"{id}\"")) {
                    Ok(v) => opt_err(Ok(v)),
                    Err(e) => json!({ "thrown": error_json(&e) }),
                };
                note("awayFor", seed, format!("{cn}/{id}"), &away, &a["awayFor"][cn][id]);
            }
            note("refusalFor none", seed, cn.clone(), &answer(rules.refusal_for(None, c)), &a["refusalForNone"][cn]);
            note("listedFor", seed, cn.clone(), &answer(rules.listed_for(c).map(|l| ids(&l))), &a["listedFor"][cn]);
            for id in rec_ids.iter().map(String::as_str).chain(["wNowhere"]) {
                let got = match rules.refuse_named(id, c) {
                    Ok(()) => Value::Null,
                    Err(e) => json!({ "thrown": error_json(&e) }),
                };
                note("refuseNamed", seed, format!("{cn}/{id}"), &got, &a["refuseNamed"][cn][id]);
            }
            for (p, want) in a["projectReached"][cn].as_object().unwrap() {
                note("projectReached", seed, format!("{cn}/{p}"), &answer(rules.project_reached(c, p)), want);
            }
            for (p, want) in a["ofThreadsRepository"][cn].as_object().unwrap() {
                note("ofThreadsRepository", seed, format!("{cn}/{p}"), &answer(rules.of_threads_repository(c, p)), want);
            }
            for p in wv["projects"].as_array().unwrap() {
                let id = p["id"].as_str().unwrap();
                let word = format!("the {} word", p["name"].as_str().unwrap());
                note(
                    "elsewhereRefusal",
                    seed,
                    format!("{cn}/{id}"),
                    &opt_err(rules.elsewhere_refusal(c, &project(p), &word)),
                    &a["elsewhereRefusal"][cn][id],
                );
            }
            note("talksToItsTree", seed, cn.clone(), &answer(rules.talks_to_its_tree(c)), &a["talksToItsTree"][cn]);
        }
        for t in wv["threads"].as_array().unwrap() {
            let t = t.as_str().unwrap();
            note("parentOf", seed, t.into(), &json!(rules.parent_of(t)), &a["parentOf"][t]);
            note("rootOf", seed, t.into(), &json!(rules.root_of(t)), &a["rootOf"][t]);
        }
        for id in &rec_ids {
            note("agentsOf", seed, id.clone(), &json!(rules.agents_of(&rec(&w, id))), &a["agentsOf"][id]);
        }
        for row in a["spawnGuard"].as_array().unwrap() {
            let cn = row["caller"].as_str().unwrap();
            let c = callers.iter().find(|(k, _)| k == cn).unwrap().1.clone();
            let act_word = row["act"].as_str().unwrap();
            let fresh = world(wv);
            let rules = RulesArea::new(fresh.clone());
            if act_word == "fork x3" {
                let mut steps = vec![];
                let mut frees = vec![];
                for _ in 0..3 {
                    match rules.spawn_guard(SpawnAct::Fork, c.as_ref()) {
                        Ok(free) => {
                            frees.push(Some(free));
                            steps.push(json!({ "took": json!(fresh.landing.borrow().clone()) }));
                        }
                        Err(e) => {
                            frees.push(None);
                            steps.push(json!({ "refused": error_json(&e), "landing": json!(fresh.landing.borrow().clone()) }));
                        }
                    }
                }
                for f in frees {
                    if let Some(f) = f {
                        f();
                        f();
                    }
                    steps.push(json!({ "freed": json!(fresh.landing.borrow().clone()) }));
                }
                note("spawnGuard fork x3", seed, cn.into(), &json!(steps), &row["steps"]);
                continue;
            }
            let act: SpawnAct = serde_json::from_value(json!(act_word)).unwrap();
            let got = match rules.spawn_guard(act, c.as_ref()) {
                Ok(_) => json!("free"),
                Err(e) => json!({ "thrown": error_json(&e) }),
            };
            let landing = json!(fresh.landing.borrow().clone());
            note("spawnGuard", seed, format!("{cn}/{act_word}"), &json!([got, landing]), &json!([row["answer"], row["landing"]]));
        }
    }
    let total: usize = wrong.values().map(Vec::len).sum();
    let mut report = format!("{total} of {asked} answers differ\n");
    for (door, rows) in &wrong {
        report.push_str(&format!("== {door}: {}\n", rows.len()));
        for r in rows.iter().take(6) {
            report.push_str(&format!("  {r}\n"));
        }
    }
    eprintln!("{report}");
    assert_eq!(total, 0, "{report}");
}
