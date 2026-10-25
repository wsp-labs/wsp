// SPDX-License-Identifier: AGPL-3.0-only
//! The small facts of the create area (the `facts` block of create.json: the name rules, the failed and shown creates,
//! the stage reporter and its listener, the folder join) and the one step the oracle cannot see, that a fork's `bind`
//! attaches, marks and hands the slot back with no await between. Spec: spec/create.md rules 1k and 2-10.

mod common;
#[path = "support/create.rs"]
mod support;

use futures::FutureExt;
use serde_json::{json, Value};
use support::{project_of, same, scope_of, view_of, World, VIEW_MARK};
use wsp_core::types::wiring::StageSaid;
use wsp_machines::machines::create::{CreateArea, CreateRuntime};
use wsp_machines::machines::machines::Entry;
use wsp_wire::views::workspace_events::WorkspaceCreateStage;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

use common::T0;

fn facts() -> Value {
    common::fixture("create")["facts"].clone()
}

fn pairs(v: &Value) -> Vec<(String, Value)> {
    v.as_array().expect("pairs").iter().map(|p| (p[0].as_str().expect("a string").to_string(), p[1].clone())).collect()
}

/// The oracle's held stage `{type: "workspace.creating", ...}` as the Rust area holds it: the event without its tag.
fn held(stage: &Value) -> Value {
    let mut s = stage.clone();
    s.as_object_mut().expect("a stage").remove("type");
    s
}

fn stages_of(w: &World) -> Value {
    Value::Array(w.area.create_stages.borrow().iter().map(|(id, e)| json!([id, serde_json::to_value(e).expect("a stage serializes")])).collect())
}

fn assert_same(got: &Value, want: &Value, what: &str) {
    assert!(same(got, want), "{what}\n got {got}\nwant {want}");
}

fn event(v: Value) -> EventUnion {
    serde_json::from_value(v).expect("the event parses")
}

fn cloud() -> WorkspaceKind {
    serde_json::from_value(json!("cloud")).expect("a kind")
}

#[test]
fn name_given_trims_as_javascript_does() {
    common::run_local(async {
        for (given, want) in pairs(&facts()["nameGiven"]) {
            assert_eq!(CreateArea::name_given(&given), want.as_str().expect("a name"), "name_given({given:?})");
        }
    })
}

#[test]
fn name_refusal_names_blank_taken_deleting_and_forking_and_compares_exactly() {
    common::run_local(async {
        let w = World::small();
        w.area.forking.borrow_mut().insert("forking".to_string());
        for (name, want) in pairs(&facts()["nameRefusal"]) {
            assert_eq!(w.area.name_refusal(&name).as_deref(), want.as_str(), "name_refusal({name:?})");
        }
    })
}

#[test]
fn failed_view_has_the_failed_creates_keys_in_order() {
    common::run_local(async {
        let w = World::small();
        let project = project_of(&json!({"id": "p1", "name": "site", "path": "", "computer": ""}));
        let view = w.area.failed_view("ws_f", "w1", cloud(), "", T0, &project, "it failed");
        let got = serde_json::to_value(&view).expect("a view serializes");
        // The oracle's `refOf` answers `{id, name}`; the Rust ref also holds a path and a computer, empty here.
        let mut want = facts()["failedView"].clone();
        want["project"] = json!({"id": "p1", "name": "site", "path": "", "computer": ""});
        assert_same(&got, &want, "the failed view");
        let keys: Vec<&str> = got.as_object().expect("an object").keys().map(String::as_str).collect();
        assert_eq!(keys, ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "gone"], "the keys are in the order failed_view writes them");
    })
}

#[test]
fn supersede_failed_removes_every_failed_create_of_the_name_and_emits_each_in_map_order() {
    common::run_local(async {
        let w = World::small();
        // The views' own ids differ from their keys: the event names the create by the key it was held under.
        for (key, name) in [("f1", "w1"), ("f2", "w2"), ("f3", "w1")] {
            w.area.failed_creates.borrow_mut().insert(key.to_string(), view_of(&format!("v-{key}"), name));
        }
        w.area.supersede_failed("w1");
        let left: Vec<String> = w.area.failed_creates.borrow().keys().cloned().collect();
        assert_eq!(left, facts()["afterSupersede"].as_array().expect("keys").iter().map(|k| k.as_str().expect("a key").to_string()).collect::<Vec<_>>());
        let fixture = common::fixture("create");
        let emitted: Vec<Value> = fixture["log"].as_array().expect("the facts log")[..2].to_vec();
        let got = w.log.take();
        assert_eq!(got.len(), emitted.len(), "one deleted event per superseded create: {got:#?}");
        for (g, e) in got.iter().zip(&emitted) {
            assert_same(g, e, "a workspace.deleted event, in map order");
        }
    })
}

#[test]
fn stage_reporter_emits_each_stage_and_holds_the_last_of_each_create_without_a_seq() {
    common::run_local(async {
        let w = World::small();
        let fixture = common::fixture("create");
        let scope = scope_of(&json!({"threadId": "t1", "rootThreadId": "r1"}));
        let reporter = w.area.stage_reporter("ws_9", "w9", T0 - 1500, Some(&scope));
        reporter(WorkspaceCreateStage::ForkRequested, "starting", Some(StageSaid { notice: Some("n".into()), detail: Some("d".into()), waiting: true, r#box: None }));
        w.area.stage_reporter("ws_8", "w8", T0, None)(WorkspaceCreateStage::Ready, "ready", Some(StageSaid { waiting: false, ..StageSaid::default() }));

        // The events are compared as written, key order included.
        let got = w.log.take();
        let want = &fixture["log"].as_array().expect("the facts log")[2..4];
        assert_eq!(got.len(), want.len(), "one workspace.creating per report: {got:#?}");
        for (g, e) in got.iter().zip(want) {
            assert_eq!(g.to_string(), e.to_string(), "an emitted stage, keys in order");
        }
        let held_now = stages_of(&w);
        let want_held: Vec<Value> = facts()["stagesHeld"].as_array().expect("stages").iter().map(|p| json!([p[0], held(&p[1])])).collect();
        assert_same(&held_now, &Value::Array(want_held), "the stages held after two creates reported");
    })
}

#[test]
fn the_stage_listener_holds_a_stage_without_its_seq_and_drops_it_when_the_create_lands_or_is_deleted() {
    common::run_local(async {
        let w = World::small();
        let scope = scope_of(&json!({"threadId": "t1", "rootThreadId": "r1"}));
        w.area.stage_reporter("ws_9", "w9", T0 - 1500, Some(&scope))(WorkspaceCreateStage::ForkRequested, "starting", Some(StageSaid { notice: Some("n".into()), detail: Some("d".into()), waiting: true, r#box: None }));
        w.area.stage_reporter("ws_8", "w8", T0, None)(WorkspaceCreateStage::Ready, "ready", Some(StageSaid { waiting: false, ..StageSaid::default() }));

        // The oracle's stage `x` is no stage the Rust wire reads; its `fork-requested` stands in, and the held stage is
        // expected as the oracle's with that one word changed.
        CreateRuntime::emit(&*w.rt, event(json!({"type": "workspace.creating", "workspaceId": "ws_7", "name": "w7", "stage": "fork-requested", "message": "m", "elapsedMs": 0, "seq": 44}))).expect("emit");
        let with_seq_stripped: Vec<Value> = facts()["stagesWithSeqStripped"]
            .as_array()
            .expect("stages")
            .iter()
            .map(|p| {
                let mut s = held(&p[1]);
                if p[0] == "ws_7" {
                    s["stage"] = json!("fork-requested");
                }
                json!([p[0], s])
            })
            .collect();
        assert_same(&stages_of(&w), &Value::Array(with_seq_stripped), "a stage the bus stamped with a seq is held without it");

        CreateRuntime::emit(&*w.rt, event(json!({"type": "workspace.created", "workspace": serde_json::to_value(view_of("ws_9", "w9")).expect("a view")}))).expect("emit");
        CreateRuntime::emit(&*w.rt, event(json!({"type": "workspace.deleted", "workspaceId": "ws_8"}))).expect("emit");
        let after: Vec<Value> = facts()["stagesAfter"]
            .as_array()
            .expect("stages")
            .iter()
            .map(|p| {
                let mut s = held(&p[1]);
                s["stage"] = json!("fork-requested");
                json!([p[0], s])
            })
            .collect();
        assert_same(&stages_of(&w), &Value::Array(after), "workspace.created drops its workspace's stage and workspace.deleted its own");
    })
}

#[test]
fn show_create_holds_the_view_by_its_id_and_emits_workspace_created_with_it() {
    common::run_local(async {
        let w = World::small();
        w.area.show_create(view_of("s1", "shown"));
        let shown: Vec<Value> = w.area.shown_creates.borrow().iter().map(|(k, v)| json!([k, {"id": v.id, "name": v.name}])).collect();
        assert_same(&Value::Array(shown), &facts()["shown"], "the shown creates");
        let fixture = common::fixture("create");
        let want = fixture["log"].as_array().expect("the facts log").last().expect("an entry").clone();
        assert_eq!(w.log.take(), vec![want], "the one event a show emits");
    })
}

#[test]
fn folders_of_lists_the_live_folder_entries_of_the_project_in_live_order() {
    common::run_local(async {
        let w = World::small();
        let got: Vec<String> = w.area.folders_of("p1").iter().map(|e| e.borrow().record.borrow().id.clone()).collect();
        let want: Vec<String> = facts()["foldersOf"].as_array().expect("ids").iter().map(|i| i.as_str().expect("an id").to_string()).collect();
        assert_eq!(got, want);
    })
}

#[test]
fn one_folder_joins_per_key_and_frees_the_key_when_the_make_settles_either_way() {
    common::run_local(async {
        let w = World::small();
        let made = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let (release, wait) = futures::channel::oneshot::channel::<Entry>();
        let m = made.clone();
        let one = w.area.one_folder(
            "k",
            Box::new(move || {
                m.set(m.get() + 1);
                async move { wait.await.map_err(|_| HostError::new("released")) }.boxed_local()
            }),
        );
        let m = made.clone();
        let two = w.area.one_folder("k", Box::new(move || {
            m.set(m.get() + 1);
            async { unreachable!("a second ask for the key joins the first") }.boxed_local()
        }));
        let (h1, h2) = (support_spawn(one), support_spawn(two));
        common::flush().await;
        let made_entry = w.entry("c").expect("a live folder entry to stand for the made one");
        release.send(made_entry).ok();
        let (first, second) = (h1.await.expect("task").expect("made"), h2.await.expect("task").expect("made"));
        assert!(std::rc::Rc::ptr_eq(&first, &second), "both callers get the one entry the one make made");
        assert_eq!(made.get(), 1, "the joined ask did not make");

        let m = made.clone();
        let failing = w.area.one_folder("k", Box::new(move || {
            m.set(m.get() + 1);
            async { Err(HostError::new("no")) }.boxed_local()
        }));
        assert_eq!(failing.await.err().map(|e| e.message), Some("no".to_string()), "a failed make rejects its callers");
        let m = made.clone();
        let again_entry = w.entry("d").expect("a second entry");
        let again = w.area.one_folder("k", Box::new(move || {
            m.set(m.get() + 1);
            async move { Ok(again_entry) }.boxed_local()
        }));
        let got = again.await.expect("made again");
        assert_eq!(got.borrow().record.borrow().id, "d");
        assert_eq!(made.get() as i64, facts()["oneFolderMade"].as_i64().expect("a count"), "a settled make, success or failure, frees the key");
    })
}

fn support_spawn<T: 'static>(f: futures::future::LocalBoxFuture<'static, T>) -> tokio::task::JoinHandle<T> {
    tokio::task::spawn_local(f)
}

#[test]
fn bind_attaches_marks_and_hands_the_slot_back_in_one_step() {
    common::run_local(async {
        let fixture = common::fixture("create");
        let sc = common::scene(&fixture, "a brokered fork with the branch named keeps it held and opens the named thread");
        let w = World::new(&sc["scene"]);
        let created = w.create().await.expect("the create");

        let bound = w.rt.bound.borrow().clone();
        assert_eq!(bound.len(), 1, "one fork, one bind");
        let seen = &bound[0];
        assert_eq!(seen["attached"], json!(true), "the live entry exists the moment bind returns: {seen}");
        assert_eq!(seen["machineId"], json!("m-new"), "bind put the machine on the record");
        assert_eq!(seen["creating"], json!(true), "the entry is marked creating in that step: {seen}");
        assert_eq!(seen["opensThread"], json!("t-open"), "the named thread is set in that step: {seen}");
        assert_eq!(seen["ofRecordHeld"], json!(true), "a branch the caller named is held in that step: {seen}");
        assert_eq!(seen["landed"], json!(true), "`landed` has run by the time bind returns, with no await between: {seen}");

        // Attach and landed are adjacent in the log: nothing was called between them.
        let log = w.log.take();
        let at = log.iter().position(|e| e[0] == "attach").expect("attach was called");
        assert_eq!(log[at + 1], json!(["landed"]), "landed follows attach with no call between: {log:#?}");
        assert_eq!(log.iter().filter(|e| e[0] == "landed").count(), 1, "the slot is handed back once");
        assert_eq!(created.view.created_at, VIEW_MARK);
        let entry = w.entry("ws_1").expect("the entry stays live");
        assert!(!entry.borrow().creating, "creating is cleared when the create is ready");
    })
}
