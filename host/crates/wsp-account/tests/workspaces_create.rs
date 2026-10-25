// SPDX-License-Identifier: AGPL-3.0-only
//! The workspaces door's create and lookup half: where workspaces land, making one, and reading one or all back with the
//! caller's rules. spec/workspaces_create.md; workspaces.ts:66-288 and :1211-1215 at 65f6b89c7. One test per row of the
//! spec's "Rules against tests" table, then the strong cases of the TS tests the test map lists (runtime-create,
//! spawn-guard, projects, places, reconcile, box-lead) that those names do not cover, each naming the file:line it ports.
//! Every sentence is the TS protocol function's own text, written out, since wsp-words' bodies are not written yet.
#[path = "support/base.rs"]
mod base;
#[path = "support/create.rs"]
mod create_support;

use std::rc::Rc;

use base::{kinded, plain, project, rec, rec_json, record, run, view_of};
use create_support::{asker, caps, refusal, the_person, FakeCreate};
use futures::channel::oneshot;
use serde_json::{json, Value};
use wsp_account::projects::workspaces::create::{create, creating, get, held_or_creating, landing, list, resolve, CreateCtx};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_core::types::api::Landing;
use wsp_core::types::wiring::{CreateWorkspaceOptions, SubAsk};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_words::exit::Kind;

const THREAD: &str = "t_lead1234567";
const WORD: &str = "t_lead12";
const ISO_BEGAN: &str = "1970-01-01T00:16:40.000Z";

struct H {
    f: Rc<FakeCreate>,
    ctx: Rc<dyn CreateCtx>,
}

impl H {
    fn new() -> H {
        let f = FakeCreate::new();
        f.base.hold_project(project("pr_cloud", "spoo", "solari", json!({})));
        f.base.hold_project(project("pr_here", "wsp", "here", json!({})));
        f.base.hold_project(project("pr_box", "lab", "pl_spoo", json!({})));
        let ctx = f.ctx();
        H { f, ctx }
    }
    fn hold(&self, id: &str, kind: &str, project: &str, over: Value) -> Entry {
        self.f.base.hold(record(id, kind, project, over))
    }
    fn calls(&self, of: &[&str]) -> Vec<String> {
        self.f.base.names_of(of)
    }
    fn events(&self) -> Vec<Value> {
        self.f.base.events.borrow().clone()
    }
    fn writes(&self) -> Vec<String> {
        self.f.base.store.writes.borrow().clone()
    }
    fn nothing_stored_or_said(&self) {
        assert!(self.f.base.persisted.borrow().is_empty(), "a record was persisted");
        assert!(self.events().is_empty(), "an event was emitted");
        assert!(self.writes().is_empty(), "the store was written");
    }
    fn entry(&self, id: &str) -> Entry {
        self.f.base.live.borrow().get(id).cloned().unwrap_or_else(|| panic!("{id} is not live"))
    }
    fn is_live(&self, id: &str) -> bool {
        self.f.base.live.borrow().contains_key(id)
    }
}

fn opts(project: &str, name: &str) -> CreateWorkspaceOptions {
    CreateWorkspaceOptions { project: project.to_owned(), name: name.to_owned(), ..Default::default() }
}

fn with_parent(mut o: CreateWorkspaceOptions, parent: &str) -> CreateWorkspaceOptions {
    o.parent = Some(parent.to_owned());
    o
}

fn lead() -> Caller {
    asker(THREAD, "ws_lead", "t_root")
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn agents_ask() -> serde_json::Map<String, Value> {
    json!({ "spawn": true, "maxMachines": 7 }).as_object().unwrap().clone()
}

fn view_json(h: &H, id: &str) -> Value {
    json_of(&view_of(&h.f.base, &rec(&h.entry(id))))
}

fn project_ref(id: &str, name: &str, computer: &str) -> Value {
    json!({ "id": id, "name": name, "path": format!("/home/me/{name}"), "computer": computer })
}

/// A shown or failed fork's view as the fake builds it, over `extra`.
fn fork_view(id: &str, name: &str, phase: &str, extra: Value) -> WorkspaceView {
    let mut v = json!({ "id": id, "name": name, "machineId": "", "phase": phase, "kind": "cloud", "golden": "", "createdAt": "2026-10-25T00:00:00.000Z", "project": project_ref("pr_cloud", "spoo", "solari") });
    base::merge(&mut v, extra);
    serde_json::from_value(v).unwrap()
}

// --------------------------------------------------------------------------------------------------- rule 1: landing

#[test]
fn landing_on_a_folder_kind_names_the_place_and_its_backend() {
    run(async {
        let h = H::new();
        let here = landing(&h.ctx, "pr_here", false, Some(the_person())).await.unwrap();
        assert_eq!(here, Landing { place: None, name: "Mac".to_owned(), capabilities: caps(1), kind: Some(WorkspaceKind::Local) });
        let boxed = landing(&h.ctx, "pr_box", false, Some(the_person())).await.unwrap();
        assert_eq!(boxed, Landing { place: Some("pl_spoo".to_owned()), name: "spoo".to_owned(), capabilities: caps(2), kind: Some(WorkspaceKind::Place) });
        assert_eq!(h.f.base.args_of("backend_capabilities"), vec![json!({ "kind": "local", "place": null }), json!({ "kind": "place", "place": "pl_spoo" })]);
        assert_eq!(h.calls(&["ready", "resolve_project", "landing_place", "landing_capabilities", "backend_capabilities"]), [
            "ready", "resolve_project", "backend_capabilities", "ready", "resolve_project", "backend_capabilities"
        ]);
    });
}

#[test]
fn landing_of_a_fork_names_the_landing_place() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("landing_place", vec![json!("solari"), json!(null)]);
        h.f.base.answer_with("landing_capabilities", vec![json!(caps_json(9)), json!(caps_json(8))]);
        let at_place = landing(&h.ctx, "pr_cloud", false, None).await.unwrap();
        assert_eq!(at_place, Landing { place: Some("solari".to_owned()), name: "solari".to_owned(), capabilities: caps(9), kind: Some(WorkspaceKind::Cloud) });
        let wired = landing(&h.ctx, "pr_cloud", false, None).await.unwrap();
        assert_eq!(wired, Landing { place: None, name: "boat".to_owned(), capabilities: caps(8), kind: Some(WorkspaceKind::Cloud) });
        assert_eq!(h.f.base.args_of("landing_capabilities"), vec![json!({ "place": "solari" }), json!({ "place": null })]);
        assert_eq!(h.calls(&["ready", "resolve_project", "landing_place", "landing_capabilities", "backend_capabilities"]), [
            "ready", "resolve_project", "landing_place", "landing_capabilities", "ready", "resolve_project", "landing_place", "landing_capabilities"
        ]);
    });
}

fn caps_json(tag: u32) -> Value {
    json_of(&caps(tag))
}

#[test]
fn a_thread_cannot_ask_a_sized_landing() {
    run(async {
        let h = H::new();
        let err = landing(&h.ctx, "pr_cloud", true, Some(lead())).await.unwrap_err();
        assert_eq!(err, refusal_of_act("size"));
        assert_eq!(h.f.base.args_of("act_refusal"), vec![json!({ "threadId": THREAD, "act": "size" })]);
        assert!(h.f.base.args_of("resolve_project").is_empty());
    });
}

fn refusal_of_act(act: &str) -> wsp_words::exit::HostError {
    base::refused(&format!("act refusal: {act} for {THREAD}"), "ask the person", Kind::Usage)
}

// spawn-guard.test.ts:388 "a thread's landing resolves its own repository's projects alone ... "
#[test]
fn a_threads_landing_resolves_through_the_project_door_as_the_person_does() {
    run(async {
        let h = H::new();
        let thread = landing(&h.ctx, "pr_cloud", false, Some(lead())).await.unwrap();
        let person = landing(&h.ctx, "pr_cloud", false, Some(the_person())).await.unwrap();
        assert_eq!(thread, person);
        assert_eq!(h.f.base.args_of("resolve_project"), vec![json!({ "word": "pr_cloud", "scoped": true }), json!({ "word": "pr_cloud", "scoped": false })]);
        h.f.base.answer_with("resolve_project", vec![base::err_answer("no such project nothing-here", Some("not-found"))]);
        let err = landing(&h.ctx, "nothing-here", false, Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("no such project nothing-here", Kind::NotFound));
    });
}

// --------------------------------------------------------------------------------------------- rules 2.1 to 2.5

#[test]
fn create_with_sub_goes_to_spawn_sub() {
    run(async {
        let h = H::new();
        let mut o = opts("pr_cloud", "kid");
        o.sub = Some(SubAsk { on: "boat".to_owned(), ..Default::default() });
        let made = create(&h.ctx, o, Some(lead())).await.unwrap();
        assert_eq!(made.notice.as_deref(), Some("sub started"));
        assert_eq!(h.f.base.args_of("spawn_sub"), vec![json!({ "name": "kid", "project": "pr_cloud", "on": "boat", "scoped": true })]);
        assert_eq!(h.calls(&["ready", "spawn_sub", "name_given", "resolve_project", "spawn_guard", "forking_add", "create_staged"]), ["ready", "spawn_sub"]);
        h.nothing_stored_or_said();
    });
}

#[test]
fn a_thread_cannot_name_an_image_or_a_size() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("place_of_thread", vec![json!("Boat")]);
        let mut named = opts("pr_cloud", "kid");
        named.golden = Some("snap_g".to_owned());
        named.spec.cpu = Some(JsNumber(4.0));
        let err = create(&h.ctx, named.clone(), Some(lead())).await.unwrap_err();
        let said = format!(
            "this request came out of thread {WORD} on Boat, and a thread forks the image its own workspace runs; naming an image to fork from is not a thread's to ask for"
        );
        assert_eq!(err, kinded(&said, Kind::Usage));
        let mut here = named.clone();
        h.f.base.script.borrow_mut().shift_remove("place_of_thread");
        let err = create(&h.ctx, here.clone(), Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            kinded(&format!("this request came out of thread {WORD}, and a thread forks the image its own workspace runs; naming an image to fork from is not a thread's to ask for"), Kind::Usage)
        );
        here.golden = None;
        let err = create(&h.ctx, here, Some(lead())).await.unwrap_err();
        assert_eq!(err, refusal_of_act("size"));
        let mut mem = opts("pr_cloud", "kid");
        mem.spec.mem_mb = Some(JsNumber(2048.0));
        let err = create(&h.ctx, mem, Some(lead())).await.unwrap_err();
        assert_eq!(err, refusal_of_act("size"));
        assert_eq!(h.calls(&["resolve_project", "spawn_guard", "entry_of", "forking_add"]), Vec::<String>::new());
        h.nothing_stored_or_said();
    });
}

// workspaces.ts:99 a thread naming a parent has its own tree check first
#[test]
fn a_thread_naming_a_parent_it_may_not_drive_is_refused_before_anything_else() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("entry_of", vec![base::err_answer("no workspace ws_far", Some("not-found"))]);
        let err = create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_far"), Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_far", Kind::NotFound));
        assert_eq!(h.f.base.args_of("entry_of"), vec![json!({ "id": "ws_far", "thread": null, "act": null, "scoped": true })]);
        assert_eq!(h.calls(&["name_given", "resolve_project", "spawn_guard", "forking_add"]), Vec::<String>::new());
        h.nothing_stored_or_said();
    });
}

#[test]
fn invariant_3_a_threads_create_takes_its_parent_from_its_scope_not_the_ask() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.hold("ws_kid", "cloud", "pr_cloud", json!({ "name": "kid" }));
        create(&h.ctx, opts("pr_cloud", "child"), Some(lead())).await.unwrap();
        let staged = h.f.base.args_of("create_staged");
        assert_eq!(staged[0]["parent"], json!("ws_lead"));
        assert_eq!(staged[0]["thread"], json!(THREAD));
        assert_eq!(h.f.base.args_of("lead_start"), vec![json!({ "parent": "ws_lead", "name": "child", "how": "run" })]);
        let shown = h.f.base.args_of("show_create");
        assert_eq!(shown[0]["view"]["parentWorkspaceId"], json!("ws_lead"));
        assert!(h.f.base.args_of("entry_of").is_empty());
        // A parent the ask names is checked as the thread's to drive, then used.
        create(&h.ctx, with_parent(opts("pr_cloud", "grandchild"), "ws_kid"), Some(lead())).await.unwrap();
        assert_eq!(h.f.base.args_of("entry_of"), vec![json!({ "id": "ws_kid", "thread": null, "act": null, "scoped": true })]);
        assert_eq!(h.f.base.args_of("create_staged")[1]["parent"], json!("ws_kid"));
        assert_eq!(h.f.base.args_of("lead_start")[1], json!({ "parent": "ws_kid", "name": "grandchild", "how": "fork" }));
        // The person's create with no parent has none.
        create(&h.ctx, opts("pr_cloud", "root"), Some(the_person())).await.unwrap();
        assert_eq!(h.f.base.args_of("create_staged")[2]["parent"], json!(null));
        assert_eq!(h.f.base.args_of("lead_start").len(), 2);
    });
}

#[test]
fn a_parent_on_a_folder_kind_is_refused() {
    run(async {
        let h = H::new();
        let err = create(&h.ctx, with_parent(opts("pr_here", "kid"), "ws_x"), Some(the_person())).await.unwrap_err();
        assert_eq!(err, refusal("wsp is a project folder, not a machine to fork", "Open a thread in it with wsp run wsp \"<task>\".", Kind::Usage));
        assert_eq!(h.calls(&["refuse_recording", "project_folder", "spawn_guard", "forking_add"]), Vec::<String>::new());
        h.nothing_stored_or_said();
    });
}

#[test]
fn a_thread_that_names_no_parent_is_never_refused_as_a_folder_fork() {
    run(async {
        let h = H::new();
        let made = create(&h.ctx, opts("pr_here", "kid"), Some(lead())).await.unwrap();
        assert_eq!(made.view.id, "ws_folder_pr_here");
        assert_eq!(h.calls(&["entry_of", "refuse_recording", "project_folder"]), ["refuse_recording", "project_folder"]);
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_on_a_folder_kind_answers_the_folder_record_and_refuses_fork_words() {
    run(async {
        let h = H::new();
        let made = create(&h.ctx, opts("pr_here", " scratch "), Some(the_person())).await.unwrap();
        assert_eq!(json_of(&made), view_json(&h, "ws_folder_pr_here"));
        assert_eq!(h.f.base.args_of("refuse_recording"), vec![json!({ "word": "scratch", "scoped": false })]);
        assert_eq!(h.calls(&["ready", "name_given", "resolve_project", "refuse_recording", "project_folder", "persist", "emit"]), [
            "ready", "name_given", "resolve_project", "refuse_recording", "project_folder"
        ]);
        h.nothing_stored_or_said();

        let mut from = opts("pr_here", "scratch");
        from.golden = Some("snap_g".to_owned());
        let err = create(&h.ctx, from.clone(), Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("wsp is on this computer, where threads run in its folder and nothing forks, so it takes no --from", Kind::Invalid));
        from.golden = None;
        from.spec.cpu = Some(JsNumber(4.0));
        let err = create(&h.ctx, from.clone(), Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("wsp is on this computer, where threads run in its folder and nothing forks, so it takes no --size", Kind::Invalid));
        from.golden = Some("snap_g".to_owned());
        from.spec.cpu = None;
        from.spec.mem_mb = Some(JsNumber(4096.0));
        let err = create(&h.ctx, from, Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("wsp is on this computer, where threads run in its folder and nothing forks, so it takes no --from, --size", Kind::Invalid));
        assert_eq!(h.f.base.args_of("project_folder").len(), 1);
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_on_a_folder_kind_names_the_box_in_the_copy_takes_none_line() {
    run(async {
        let h = H::new();
        let mut o = opts("pr_box", "scratch");
        o.golden = Some("snap_g".to_owned());
        let err = create(&h.ctx, o, Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("lab is on spoo, where threads run in its folder and nothing forks, so it takes no --from", Kind::Invalid));
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_on_a_folder_kind_sets_agents() {
    run(async {
        let h = H::new();
        let mut o = opts("pr_here", "scratch");
        o.agents = Some(agents_ask());
        let made = create(&h.ctx, o, Some(the_person())).await.unwrap();
        let agents = json!({ "spawn": true, "maxMachines": 7, "maxDepth": 4 });
        let expected = serde_json::to_value(record("ws_folder_pr_here", "local", "pr_here", json!({ "name": "wsp", "machineId": "", "agents": agents }))).unwrap();
        assert_eq!(h.f.base.persisted.borrow().iter().map(json_of).collect::<Vec<_>>(), vec![expected]);
        assert_eq!(h.f.base.args_of("agents_from"), vec![json!({ "place": "here", "agents": { "spawn": true, "maxMachines": 7 } })]);
        assert_eq!(made.view.agents.as_ref().map(json_of), Some(agents));
        assert!(h.events().is_empty());
        // A folder record that holds a place names it.
        h.hold("ws_folder_pr_box", "place", "pr_box", json!({ "place": "pl_spoo" }));
        let mut o = opts("pr_box", "scratch");
        o.agents = Some(agents_ask());
        create(&h.ctx, o, Some(the_person())).await.unwrap();
        assert_eq!(h.f.base.args_of("agents_from")[1]["place"], json!("pl_spoo"));
        assert_eq!(h.f.base.persisted.borrow().len(), 2);
    });
}

#[test]
fn create_on_a_folder_kind_refuses_the_agents_of_a_thread() {
    run(async {
        let h = H::new();
        let mut o = opts("pr_here", "scratch");
        o.agents = Some(agents_ask());
        let err = create(&h.ctx, o, Some(lead())).await.unwrap_err();
        assert_eq!(err, refusal_of_act("agents"));
        assert!(h.f.base.args_of("project_folder").is_empty());
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_on_a_folder_kind_stops_at_a_relayed_recording() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("refuse_recording", vec![base::err_answer("recording scratch is this computer's own act", Some("usage"))]);
        let err = create(&h.ctx, opts("pr_here", "scratch"), Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("recording scratch is this computer's own act", Kind::Usage));
        assert!(h.f.base.args_of("project_folder").is_empty());
        h.nothing_stored_or_said();
    });
}

// --------------------------------------------------------------------------------------------- rules 2.6 to 2.8

#[test]
fn create_refuses_a_parent_not_held_or_still_creating() {
    run(async {
        let h = H::new();
        let said = |id: &str| kinded(&format!("no workspace {id} to fork from; a child workspace is made out of one this computer holds"), Kind::Invalid);
        let err = create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_gone"), Some(the_person())).await.unwrap_err();
        assert_eq!(err, said("ws_gone"));
        let held = h.hold("ws_busy", "cloud", "pr_cloud", json!({ "name": "busy" }));
        held.borrow_mut().creating = true;
        let err = create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_busy"), Some(the_person())).await.unwrap_err();
        assert_eq!(err, said("ws_busy"));
        assert_eq!(h.calls(&["refuse_relayed", "place_guard", "spawn_guard", "forking_add", "create_staged"]), ["refuse_relayed", "refuse_relayed"]);
        assert_eq!(h.f.base.args_of("refuse_relayed")[0], json!({ "kind": "cloud", "name": "kid", "project": "pr_cloud", "scoped": false }));
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_refuses_a_parent_of_another_repository() {
    run(async {
        let h = H::new();
        h.hold("ws_p", "cloud", "pr_box", json!({ "name": "lead" }));
        let err = create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_p"), Some(the_person())).await.unwrap_err();
        assert_eq!(
            err,
            kinded("lead is a workspace of lab and this one is made for spoo; a child starts on its parent's branch, so both hold one project", Kind::Invalid)
        );
        assert_eq!(h.f.base.args_of("same_repository"), vec![json!({ "a": "pr_box", "b": "pr_cloud", "scoped": false })]);
        assert_eq!(h.calls(&["place_guard", "spawn_guard", "forking_add"]), Vec::<String>::new());
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_takes_a_parent_of_another_project_of_the_same_repository() {
    run(async {
        let h = H::new();
        h.hold("ws_p", "cloud", "pr_box", json!({ "name": "lead" }));
        h.f.base.answer_with("same_repository", vec![json!(true)]);
        create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_p"), Some(the_person())).await.unwrap();
        assert_eq!(h.f.base.args_of("create_staged").len(), 1);
        // A parent of the project itself is never compared.
        h.hold("ws_q", "cloud", "pr_cloud", json!({ "name": "same" }));
        create(&h.ctx, with_parent(opts("pr_cloud", "kid2"), "ws_q"), Some(the_person())).await.unwrap();
        assert_eq!(h.f.base.args_of("same_repository").len(), 1);
    });
}

#[test]
fn create_takes_the_spawn_slot_and_frees_it_on_a_name_taken() {
    run(async {
        let h = H::new();
        let taken = "dup is already a workspace; pick another name, or delete it first";
        h.f.base.answer_with("name_refusal", vec![json!(taken)]);
        let err = create(&h.ctx, opts("pr_cloud", "dup"), Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded(taken, Kind::Conflict));
        assert_eq!(h.calls(&["place_guard", "spawn_guard", "name_refusal", "free_slot", "forking_add", "forking_delete", "create_staged"]), [
            "place_guard", "spawn_guard", "name_refusal", "free_slot"
        ]);
        assert_eq!(h.f.base.args_of("spawn_guard"), vec![json!({ "act": "fork", "scoped": false })]);
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_frees_the_slot_on_a_threads_agents() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        let mut o = opts("pr_cloud", "kid");
        o.agents = Some(agents_ask());
        let err = create(&h.ctx, o, Some(lead())).await.unwrap_err();
        assert_eq!(err, refusal_of_act("agents"));
        assert_eq!(h.calls(&["spawn_guard", "free_slot", "name_refusal", "forking_add", "create_staged"]), ["spawn_guard", "free_slot"]);
        h.nothing_stored_or_said();
    });
}

// Rule 3.4: workspaces.ts:139-157 counts and takes the place, and the name, with no await between.
#[test]
fn a_second_create_of_a_name_in_flight_is_refused_not_raced() {
    run(async {
        let h = H::new();
        h.f.yield_in_guard.set(true);
        let (a, b) = futures::future::join(create(&h.ctx, opts("pr_cloud", "race"), None), create(&h.ctx, opts("pr_cloud", "race"), None)).await;
        assert!(a.is_ok(), "the first create is the one that holds the name");
        assert_eq!(b.unwrap_err(), kinded("race is already a workspace; pick another name, or delete it first", Kind::Conflict));
        assert_eq!(h.calls(&["spawn_guard", "free_slot"]), ["spawn_guard", "spawn_guard", "free_slot", "free_slot"]);
        assert!(h.f.forking.borrow().is_empty());
    });
}

// ----------------------------------------------------------------------------------------------------- rule 2.9

// runtime-create.test.ts:30 "a plain create reports every awaited step in order"
#[test]
fn create_answers_the_staged_workspace() {
    run(async {
        let h = H::new();
        let made = create(&h.ctx, opts("pr_cloud", " pricing "), Some(the_person())).await.unwrap();
        assert_eq!(json_of(&made), view_json(&h, "ws_11111111"));
        assert_eq!(made.view.id, "ws_11111111");
        assert_eq!(made.view.name, "pricing");
        assert_eq!(h.calls(&["ready", "name_given", "resolve_project", "refuse_relayed", "place_guard", "spawn_guard", "name_refusal", "forking_add", "supersede_failed", "stage_reporter", "lead_start", "show_create", "create_staged", "unfork", "forking_delete", "free_slot"]), [
            "ready", "name_given", "resolve_project", "refuse_relayed", "place_guard", "spawn_guard", "name_refusal", "forking_add", "supersede_failed", "stage_reporter", "create_staged", "forking_delete", "free_slot"
        ]);
        assert_eq!(h.f.base.args_of("stage_reporter"), vec![json!({ "id": "ws_11111111", "name": "pricing", "began": 1_000_000, "thread": null })]);
        assert_eq!(h.f.base.args_of("forking_add"), vec![json!({ "name": "pricing" })]);
        assert_eq!(h.f.base.args_of("forking_delete"), vec![json!({ "name": "pricing" })]);
        assert_eq!(h.f.base.args_of("create_staged")[0]["base"], json!(null));
        assert!(h.f.forking.borrow().is_empty());
        h.nothing_stored_or_said();
    });
}

#[test]
fn create_guards_the_landing_place_or_the_wired_one() {
    run(async {
        let h = H::new();
        create(&h.ctx, opts("pr_cloud", "a"), None).await.unwrap();
        h.f.base.answer_with("landing_place", vec![json!("solari")]);
        create(&h.ctx, opts("pr_cloud", "b"), None).await.unwrap();
        assert_eq!(h.f.base.args_of("place_guard"), vec![json!({ "place": "boat" }), json!({ "place": "solari" })]);
        assert_eq!(h.f.base.args_of("landing_place"), vec![json!({ "computer": "solari" }), json!({ "computer": "solari" })]);
    });
}

// fork-start-events.test.ts:89 "names its lead in every event and read from the first, which is the record"
#[test]
fn create_of_a_threads_fork_shows_it_queued_first() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        let mut o = with_parent(opts("pr_cloud", "forked"), "ws_lead");
        o.golden = None;
        create(&h.ctx, o, Some(lead())).await.unwrap();
        assert_eq!(h.calls(&["lead_start", "show_create", "create_staged"]), ["lead_start", "show_create", "create_staged"]);
        assert_eq!(h.f.base.args_of("show_create"), vec![json!({ "view": {
            "id": "ws_11111111", "name": "forked", "machineId": "", "phase": "queued", "kind": "cloud", "golden": "", "createdAt": ISO_BEGAN,
            "project": project_ref("pr_cloud", "spoo", "solari"), "parentThreadId": THREAD, "rootThreadId": "t_root", "parentWorkspaceId": "ws_lead",
        } })]);
        assert_eq!(h.f.base.args_of("create_staged")[0]["base"], json!("feedface"));
        assert_eq!(h.f.base.args_of("stage_reporter")[0]["thread"], json!(THREAD));
        // The person's fork shows nothing ahead of its stages.
        create(&h.ctx, with_parent(opts("pr_cloud", "by-hand"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(h.f.base.args_of("show_create").len(), 1);
    });
}

#[test]
fn create_of_a_child_says_the_lead_lines_in_its_notice() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.f.base.answer_with("lead_start", vec![json!({ "base": "b1", "onLeads": true, "lines": ["lead line one"] })]);
        h.f.base.answer_with("create_staged", vec![json!({ "notice": "kept builder stopped" })]);
        h.f.base.answer_with("start_child_on", vec![json!("child on lead")]);
        let made = create(&h.ctx, with_parent(opts("pr_cloud", "child"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(made.notice.as_deref(), Some("kept builder stopped\nlead line one\nchild on lead"));
        assert_eq!(made.view.id, "ws_11111111");
        assert_eq!(h.f.base.args_of("lead_start"), vec![json!({ "parent": "ws_lead", "name": "child", "how": "fork" })]);
        assert_eq!(h.f.base.args_of("start_child_on"), vec![json!({ "child": "ws_11111111", "base": "b1", "parent": "lead" })]);
        assert_eq!(h.f.base.args_of("create_staged")[0]["base"], json!("b1"));
        // With no notice of its own, the lines alone.
        h.f.base.script.borrow_mut().shift_remove("create_staged");
        let made = create(&h.ctx, with_parent(opts("pr_cloud", "child2"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(made.notice.as_deref(), Some("lead line one\nchild on lead"));
    });
}

// branch-of-record.test.ts:121 "names its lead and its branch of record in the first event"
#[test]
fn a_brokered_child_says_its_branch_of_record_and_starts_nothing_on_the_lead() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.f.base.answer_with("lead_start", vec![json!({ "base": "b1", "onLeads": true, "lines": [] })]);
        *h.f.staged_over.borrow_mut() = json!({ "branchOfRecord": { "repoId": 1, "owner": "acme", "name": "spoo", "branch": "wsp/child" } });
        let made = create(&h.ctx, with_parent(opts("pr_cloud", "child"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(made.notice.as_deref(), Some("child started on wsp/child"));
        assert!(h.f.base.args_of("start_child_on").is_empty());
    });
}

#[test]
fn a_lead_start_with_no_lines_leaves_the_notice_alone() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.f.base.answer_with("create_staged", vec![json!({ "notice": "kept builder stopped" })]);
        let made = create(&h.ctx, with_parent(opts("pr_cloud", "child"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(made.notice.as_deref(), Some("kept builder stopped"));
        let mut expected = json_of(&view_of(&h.f.base, &rec(&h.entry("ws_11111111"))));
        expected["notice"] = json!("kept builder stopped");
        assert_eq!(json_of(&made), expected);
    });
}

#[test]
fn a_child_starts_on_its_lead_only_where_the_lead_start_says_so() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.f.base.answer_with("lead_start", vec![json!({ "base": "b1", "onLeads": false, "lines": [] }), json!({ "base": null, "onLeads": true, "lines": ["only this"] })]);
        let made = create(&h.ctx, with_parent(opts("pr_cloud", "one"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(made.notice, None);
        let made = create(&h.ctx, with_parent(opts("pr_cloud", "two"), "ws_lead"), Some(the_person())).await.unwrap();
        assert_eq!(made.notice.as_deref(), Some("only this"));
        assert!(h.f.base.args_of("start_child_on").is_empty());
    });
}

// ------------------------------------------------------------------------------------------ rules 2.10 and 2.11

// runtime-create.test.ts:189 "a create that fails after the fork kills its machine, reports failed with the reason"
#[test]
fn a_failed_create_takes_the_machine_back_and_keeps_a_failed_view() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("create_staged", vec![base::err_answer("the provider refused the fork", None)]);
        h.f.stage_holds.set(true);
        let err = create(&h.ctx, opts("pr_cloud", "doomed"), Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain("the provider refused the fork"));
        assert!(!h.is_live("ws_11111111"));
        assert_eq!(h.f.base.args_of("unfork"), vec![json!({ "id": "ws_11111111" })]);
        assert_eq!(h.f.base.args_of("failed_view"), vec![json!({ "id": "ws_11111111", "name": "doomed", "kind": "cloud", "golden": "", "began": 1_000_000, "project": "pr_cloud", "said": "the provider refused the fork" })]);
        let failed = h.f.failed.borrow().clone();
        assert_eq!(failed.keys().collect::<Vec<_>>(), ["ws_11111111"]);
        assert_eq!(
            json_of(&failed["ws_11111111"]),
            json!({ "id": "ws_11111111", "name": "doomed", "machineId": "", "phase": "gone", "kind": "cloud", "golden": "", "createdAt": "began-1000000", "project": project_ref("pr_cloud", "spoo", "solari"), "gone": "the provider refused the fork" })
        );
        assert_eq!(h.f.base.args_of("report"), vec![json!({ "id": "ws_11111111", "stage": "failed", "line": "the provider refused the fork", "notice": null })]);
        assert_eq!(h.writes(), ["delete creates/workspace/ws_11111111"]);
        assert!(h.f.base.persisted.borrow().is_empty());
        assert!(h.events().is_empty());
        assert_eq!(h.calls(&["unfork", "failed_view", "report", "forking_delete", "free_slot"]), ["unfork", "failed_view", "report", "forking_delete", "free_slot"]);
    });
}

#[test]
fn a_failed_create_whose_machine_is_missing_counts_it_gone() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("create_staged", vec![base::err_answer("boom", None)]);
        h.f.base.answer_with("unfork", vec![json!("missing")]);
        h.f.stage_holds.set(true);
        let err = create(&h.ctx, opts("pr_cloud", "doomed"), None).await.unwrap_err();
        assert_eq!(err, plain("boom"));
        assert!(!h.is_live("ws_11111111"));
        assert_eq!(h.f.failed.borrow().keys().collect::<Vec<_>>(), ["ws_11111111"]);
        assert!(h.events().is_empty());
    });
}

#[test]
fn a_failed_create_whose_machine_will_not_stop_keeps_the_record() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("create_staged", vec![base::err_answer("the daemon never answered", None)]);
        h.f.base.answer_with("unfork", vec![json!("provider would not delete it")]);
        h.f.stage_holds.set(true);
        let warned = wsp_core::console::capture();
        let err = create(&h.ctx, opts("pr_cloud", "stuck"), None).await.unwrap_err();
        assert_eq!(err, plain("the daemon never answered"));
        assert_eq!(warned.lines(), ["workspace ws_11111111 failed to create and its machine m_ws_11111111 would not stop; the record stays for wsp delete"]);
        let entry = h.entry("ws_11111111");
        assert!(!entry.borrow().creating, "creating is cleared");
        let kept = serde_json::to_value(record("ws_11111111", "cloud", "pr_cloud", json!({ "name": "stuck", "machineId": "m_ws_11111111" }))).unwrap();
        assert_eq!(rec_json(&entry), kept);
        assert_eq!(h.f.base.persisted.borrow().iter().map(json_of).collect::<Vec<_>>(), vec![kept.clone()]);
        assert!(h.f.failed.borrow().is_empty());
        let view = json_of(&view_of(&h.f.base, &rec(&entry)));
        assert_eq!(h.events(), vec![json!({ "type": "workspace.created", "workspace": view })]);
        assert_eq!(h.f.base.args_of("report").len(), 1);
        assert_eq!(h.calls(&["unfork", "persist", "report", "emit", "forking_delete", "free_slot"]), ["unfork", "persist", "report", "emit", "forking_delete", "free_slot"]);
    });
}

#[test]
fn a_kept_record_that_cannot_be_stored_warns_and_the_create_still_fails_with_its_own_error() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("create_staged", vec![base::err_answer("fork failed", None)]);
        h.f.base.answer_with("unfork", vec![json!("stuck")]);
        h.f.base.answer_with("persist", vec![base::err_answer("disk full", None)]);
        h.f.stage_holds.set(true);
        let warned = wsp_core::console::capture();
        let err = create(&h.ctx, opts("pr_cloud", "stuck"), None).await.unwrap_err();
        assert_eq!(err, plain("fork failed"));
        assert_eq!(
            warned.lines(),
            [
                "workspace ws_11111111 not stored: disk full".to_owned(),
                "workspace ws_11111111 failed to create and its machine m_ws_11111111 would not stop; the record stays for wsp delete".to_owned()
            ]
        );
        assert_eq!(h.events().len(), 1);
    });
}

#[test]
fn a_failed_create_keeps_a_pending_row_holding_a_request() {
    run(async {
        let h = H::new();
        let pending = json!({ "request": { "body": "{}" } });
        h.f.base.store.seed("creates", "workspace/ws_11111111", pending.clone());
        h.f.base.answer_with("create_staged", vec![base::err_answer("fork failed", None)]);
        let err = create(&h.ctx, opts("pr_cloud", "asked"), None).await.unwrap_err();
        assert_eq!(err, plain("fork failed"));
        assert!(h.writes().is_empty(), "the pending row stays for the sweep");
        assert_eq!(h.f.base.store.row("creates", "workspace/ws_11111111"), Some(pending));
        // A row with no request goes with the id.
        h.f.base.store.seed("creates", "workspace/ws_22222222", json!({ "key": "k" }));
        let err = create(&h.ctx, opts("pr_cloud", "bare"), None).await.unwrap_err();
        assert_eq!(err, plain("fork failed"));
        assert_eq!(h.writes(), ["delete creates/workspace/ws_22222222"]);
    });
}

#[test]
fn a_failed_create_frees_the_name_and_the_slot() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("create_staged", vec![base::err_answer("fork failed", None), json!({})]);
        let err = create(&h.ctx, opts("pr_cloud", "again"), None).await.unwrap_err();
        assert_eq!(err, plain("fork failed"));
        assert!(h.f.forking.borrow().is_empty());
        assert_eq!(h.calls(&["forking_add", "forking_delete", "free_slot"]), ["forking_add", "forking_delete", "free_slot"]);
        assert!(h.f.shown.borrow().is_empty());
        let made = create(&h.ctx, opts("pr_cloud", "again"), None).await.unwrap();
        assert_eq!(made.view.id, "ws_22222222");
        assert_eq!(made.view.name, "again");
    });
}

#[test]
fn a_failed_create_a_thread_asked_for_stays_under_it_failed_with_why() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.f.base.answer_with("create_staged", vec![base::err_answer("fork failed", None)]);
        let err = create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_lead"), Some(lead())).await.unwrap_err();
        assert_eq!(err, plain("fork failed"));
        let failed = json!({
            "id": "ws_11111111", "name": "kid", "machineId": "", "phase": "gone", "kind": "cloud", "golden": "", "createdAt": ISO_BEGAN,
            "project": project_ref("pr_cloud", "spoo", "solari"), "parentThreadId": THREAD, "rootThreadId": "t_root", "parentWorkspaceId": "ws_lead", "gone": "fork failed",
        });
        assert_eq!(h.f.failed.borrow().values().map(json_of).collect::<Vec<_>>(), vec![failed.clone()]);
        assert!(h.f.base.args_of("failed_view").is_empty(), "the shown view is the one kept");
        assert_eq!(h.events(), vec![json!({ "type": "workspace.created", "workspace": failed })]);
        assert!(h.f.shown.borrow().is_empty(), "the shown row is cleared");
        assert_eq!(h.f.base.args_of("report").len(), 1);
        assert_eq!(h.calls(&["show_create", "report", "emit"]), ["show_create", "report", "emit"]);
    });
}

// workspaces.ts:128-131 a parent that did not answer gives the name and the slot back
#[test]
fn a_failed_lead_start_gives_the_name_and_slot_back() {
    run(async {
        let h = H::new();
        h.hold("ws_lead", "cloud", "pr_cloud", json!({ "name": "lead" }));
        h.f.base.answer_with("lead_start", vec![base::err_answer("lead did not answer", None)]);
        let err = create(&h.ctx, with_parent(opts("pr_cloud", "kid"), "ws_lead"), None).await.unwrap_err();
        assert_eq!(err, plain("lead did not answer"));
        assert!(h.f.base.args_of("create_staged").is_empty());
        assert!(h.f.forking.borrow().is_empty());
        assert_eq!(h.calls(&["lead_start", "failed_view", "report", "forking_delete", "free_slot"]), ["lead_start", "failed_view", "report", "forking_delete", "free_slot"]);
        assert_eq!(h.f.failed.borrow().keys().collect::<Vec<_>>(), ["ws_11111111"]);
    });
}

// ------------------------------------------------------------------------------------------------- rules 3 to 6

#[test]
fn get_reads_through_entry_of() {
    run(async {
        let h = H::new();
        h.hold("ws_a", "cloud", "pr_cloud", json!({ "name": "a" }));
        let view = get(&h.ctx, "ws_a", Some(the_person()), None).await.unwrap();
        assert_eq!(json_of(&view), view_json(&h, "ws_a"));
        let view = get(&h.ctx, "ws_a", Some(lead()), Some("t_9")).await.unwrap();
        assert_eq!(view.id, "ws_a");
        assert_eq!(
            h.f.base.args_of("entry_of"),
            vec![json!({ "id": "ws_a", "thread": null, "act": null, "scoped": false }), json!({ "id": "ws_a", "thread": "t_9", "act": null, "scoped": true })]
        );
        let err = get(&h.ctx, "ws_zz", None, None).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_zz", Kind::NotFound));
    });
}

#[test]
fn held_or_creating_waits_on_boot_work() {
    run(async {
        let h = H::new();
        h.hold("ws_a", "cloud", "pr_cloud", json!({ "name": "a" }));
        let (tx, rx) = oneshot::channel();
        *h.f.boot_gate.borrow_mut() = Some(rx);
        let ctx = h.ctx.clone();
        let waiting = tokio::task::spawn_local(async move { held_or_creating(&ctx, "ws_a").await });
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert!(!waiting.is_finished(), "the answer waits on the boot work");
        assert_eq!(h.calls(&["ready", "boot_work"]), ["ready", "boot_work"]);
        tx.send(()).unwrap();
        let view = waiting.await.unwrap().unwrap().expect("the held record answers");
        assert_eq!(json_of(&view), view_json(&h, "ws_a"));
        assert_eq!(held_or_creating(&h.ctx, "ws_nothing").await.unwrap(), None);
        assert_eq!(h.f.base.args_of("boot_work"), vec![json!({ "id": "ws_a" }), json!({ "id": "ws_nothing" })]);
    });
}

#[test]
fn list_gives_the_person_shown_and_failed_forks_once() {
    run(async {
        let h = H::new();
        h.hold("ws_a", "cloud", "pr_cloud", json!({ "name": "a" }));
        h.hold("ws_b", "cloud", "pr_cloud", json!({ "name": "b" }));
        let shown = |id: &str| fork_view(id, id, "queued", json!({ "parentThreadId": "t_1", "rootThreadId": "t_root" }));
        let failed = |id: &str, parent: bool| {
            let mut v = fork_view(id, id, "gone", json!({ "gone": "boom" }));
            if parent {
                v.parent_thread_id = Some("t_1".to_owned());
            }
            v
        };
        for v in [shown("ws_s1"), shown("ws_a"), shown("ws_s_hidden")] {
            h.f.shown.borrow_mut().insert(v.id.clone(), v);
        }
        for v in [failed("ws_f1", true), failed("ws_f_bare", false), failed("ws_f_hidden", true), failed("ws_b", true)] {
            h.f.failed.borrow_mut().insert(v.id.clone(), v);
        }
        h.f.undriven.borrow_mut().extend(["ws_s_hidden".to_owned(), "ws_f_hidden".to_owned()]);
        let listed = list(&h.ctx, Some(the_person())).await.unwrap();
        let expected = vec![
            view_json(&h, "ws_a"),
            view_json(&h, "ws_b"),
            json_of(&shown("ws_s1")),
            json_of(&failed("ws_f1", true)),
        ];
        assert_eq!(listed.iter().map(json_of).collect::<Vec<_>>(), expected);
        assert_eq!(h.calls(&["ready", "listed_for"]), ["ready", "listed_for"]);
    });
}

#[test]
fn list_for_a_thread_gives_its_listed_views_alone() {
    run(async {
        let h = H::new();
        h.hold("ws_a", "cloud", "pr_cloud", json!({ "name": "a" }));
        h.hold("ws_b", "cloud", "pr_cloud", json!({ "name": "b" }));
        *h.f.listed.borrow_mut() = Some(vec!["ws_b".to_owned()]);
        h.f.shown.borrow_mut().insert("ws_s1".to_owned(), fork_view("ws_s1", "s1", "queued", json!({})));
        let listed = list(&h.ctx, Some(lead())).await.unwrap();
        assert_eq!(listed.iter().map(json_of).collect::<Vec<_>>(), vec![view_json(&h, "ws_b")]);
        assert!(h.f.base.args_of("drives").is_empty());
        assert_eq!(h.f.base.args_of("listed_for"), vec![json!({ "scoped": true })]);
    });
}

// workspaces.ts:1211 creating() leaves out a sub-thread's and a thread's fork, which stand under their lead
#[test]
fn creating_lists_only_stages_of_a_root_create() {
    let h = H::new();
    let stage = |id: &str| -> wsp_wire::views::workspace_events::WorkspaceCreatingEvent {
        serde_json::from_value(json!({ "workspaceId": id, "name": id, "stage": "fork-requested", "message": "asking for the fork", "elapsedMs": 5 })).unwrap()
    };
    h.hold("ws_root", "cloud", "pr_cloud", json!({}));
    h.hold("ws_sub", "cloud", "pr_cloud", json!({ "lead": { "threadId": "t_1", "workspaceId": "ws_lead" } }));
    h.f.shown.borrow_mut().insert("ws_shown".to_owned(), fork_view("ws_shown", "shown", "queued", json!({})));
    h.f.stages.borrow_mut().extend([stage("ws_root"), stage("ws_sub"), stage("ws_shown"), stage("ws_unrecorded")]);
    let listed = creating(&h.ctx);
    assert_eq!(listed.iter().map(json_of).collect::<Vec<_>>(), vec![json_of(&stage("ws_root")), json_of(&stage("ws_unrecorded"))]);
}

// -------------------------------------------------------------------------------------------------------- rule 7

fn two_sharing(h: &H) {
    h.hold("ws_1a2b3c4d", "cloud", "pr_cloud", json!({ "name": "pricing page" }));
    h.hold("ws_1a2bffff", "cloud", "pr_cloud", json!({ "name": "landing copy" }));
}

// projects.test.ts:416 "takes the id, the name, or enough of the id to name one"
#[test]
fn resolve_takes_id_then_tree_then_name_then_a_prefix() {
    run(async {
        let h = H::new();
        two_sharing(&h);
        let by = |word: &'static str| {
            let ctx = h.ctx.clone();
            async move { resolve(&ctx, word, Some(the_person())).await.unwrap() }
        };
        assert_eq!(by("ws_1a2b3c4d").await.name, "pricing page");
        assert_eq!(by("pricing page").await.id, "ws_1a2b3c4d");
        assert_eq!(by("ws_1a2b3").await.name, "pricing page");
        assert_eq!(by("ws_1a2bf").await.name, "landing copy");
        assert_eq!(json_of(&by("ws_1a2bffff").await), view_json(&h, "ws_1a2bffff"));
        // A thread: a row's id wins, then the tree's, then a row's name, over what it lists.
        h.hold("ws_far", "place", "pr_box", json!({ "name": "far" }));
        h.hold("ws_x", "cloud", "pr_cloud", json!({ "name": "ws_far" }));
        *h.f.listed.borrow_mut() = Some(vec!["ws_x".to_owned()]);
        h.f.tree_ids.borrow_mut().push("ws_far".to_owned());
        assert_eq!(resolve(&h.ctx, "ws_x", Some(lead())).await.unwrap().id, "ws_x");
        assert_eq!(resolve(&h.ctx, "ws_far", Some(lead())).await.unwrap().id, "ws_far");
        h.f.tree_ids.borrow_mut().clear();
        assert_eq!(resolve(&h.ctx, "ws_far", Some(lead())).await.unwrap().id, "ws_x");
    });
}

// projects.test.ts:425 "refuses a word that starts two of them with both ids, and one too short to name any as absent"
#[test]
fn resolve_refuses_a_prefix_that_starts_two() {
    run(async {
        let h = H::new();
        two_sharing(&h);
        let err = resolve(&h.ctx, "ws_1a2b", Some(the_person())).await.unwrap_err();
        assert_eq!(err, plain("2 workspaces start with ws_1a2b (ws_1a2b3c4d, ws_1a2bffff); give more of the id"));
        let err = resolve(&h.ctx, "ws_", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_", Kind::NotFound));
        h.nothing_stored_or_said();
    });
}

// projects.test.ts:440 "a workspace whose name is the start of two ids is its name, since a whole word wins over a prefix"
#[test]
fn a_whole_name_wins_over_a_prefix_of_two_ids() {
    run(async {
        let h = H::new();
        two_sharing(&h);
        h.hold("ws_named", "cloud", "pr_cloud", json!({ "name": "ws_1a2b" }));
        let found = resolve(&h.ctx, "ws_1a2b", Some(the_person())).await.unwrap();
        assert_eq!(found.id, "ws_named");
        let err = resolve(&h.ctx, "ws_1a2b3c4d5", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_1a2b3c4d5", Kind::NotFound));
    });
}

// projects.test.ts:432 "a thread whose reach holds one of the two reads their shared start as that one"
#[test]
fn a_threads_reach_reads_a_shared_start_as_the_one_it_holds() {
    run(async {
        let h = H::new();
        two_sharing(&h);
        *h.f.listed.borrow_mut() = Some(vec!["ws_1a2b3c4d".to_owned()]);
        let found = resolve(&h.ctx, "ws_1a2b", Some(lead())).await.unwrap();
        assert_eq!(found.id, "ws_1a2b3c4d");
        let err = resolve(&h.ctx, "ws_1a2bf", Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_1a2bf", Kind::NotFound));
        assert_eq!(h.f.base.args_of("listed_for").len(), 2);
        assert!(h.f.base.args_of("held").len() >= 2, "the tree and the refusal read what the host holds");
    });
}

fn thread_setup(h: &H) {
    *h.f.scope_project.borrow_mut() = Some("pr_cloud".to_owned());
    *h.f.listed.borrow_mut() = Some(vec![]);
}

// spawn-guard.test.ts:523 and box-lead.test.ts:423 a workspace outside the thread's tree is refused by the rule that keeps it out
#[test]
fn resolve_for_a_thread_refuses_with_the_rule_that_keeps_it_out() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        // Another repository: the repository rule, in the thread's words and its own project's name.
        h.hold("ws_theirs", "cloud", "pr_box", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        h.f.other_repo.borrow_mut().push("pr_box".to_owned());
        let err = resolve(&h.ctx, "ws_theirs", Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            refusal(
                &format!("thread {WORD} works on spoo, and the workspace ws_theirs holds another repository; a thread reaches its own repository's workspaces alone"),
                "Name a workspace of your own tree or a project of your own repository, or ask the person.",
                Kind::Usage
            )
        );
        // By name too.
        let err = resolve(&h.ctx, "theirs", Some(lead())).await.unwrap_err();
        assert!(err.message.contains("the workspace theirs holds another repository"));
        // The same repository, on a computer whose folder the thread may not reach: the folder rule.
        h.f.other_repo.borrow_mut().clear();
        let err = resolve(&h.ctx, "ws_theirs", Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            refusal(
                &format!("thread {WORD} may not start children in ws_theirs: it is a folder the person keeps of this repository, and a thread started there would stand outside your tree"),
                "Name a project wsp projects lists, or ask the person to start this one.",
                Kind::Usage
            )
        );
        // The folder reached: the workspace is still outside the tree, said in the reach rule's words.
        h.f.reached.borrow_mut().push("pr_box".to_owned());
        let err = resolve(&h.ctx, "ws_theirs", Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            refusal(
                &format!("thread {WORD} may drive the workspace it runs on and the ones it forked, and ws_theirs is neither"),
                "Start a child in lab's folder instead with wsp run lab \"<message>\", which opens a thread there in your own tree.",
                Kind::Usage
            )
        );
        h.nothing_stored_or_said();
    });
}

// box-lead.test.ts:313 and :99: the computer rule, where the lead acts there itself or the person must
#[test]
fn resolve_for_a_thread_reads_the_computer_rule() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        h.hold("ws_fork", "cloud", "pr_cloud", json!({ "name": "fork" }));
        h.hold("ws_folder", "local", "pr_here", json!({ "name": "folder" }));
        h.f.hidden.borrow_mut().extend(["ws_fork".to_owned(), "ws_folder".to_owned()]);
        h.f.base.answer_with("away_for", vec![base::err_answer("fork is on Boat; ask the person", Some("usage"))]);
        let err = resolve(&h.ctx, "ws_fork", Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("fork is on Boat; ask the person", Kind::Usage));
        assert_eq!(h.f.base.args_of("away_for"), vec![json!({ "id": "ws_fork", "act": "work", "word": "ws_fork" })]);
        assert!(h.f.base.args_of("elsewhere_refusal").is_empty());
        // A record that runs in its folder reads the folder rule instead.
        h.f.base.answer_with("elsewhere_refusal", vec![base::err_answer("folder is on spoo", Some("usage"))]);
        let err = resolve(&h.ctx, "ws_folder", Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("folder is on spoo", Kind::Usage));
        assert_eq!(h.f.base.args_of("elsewhere_refusal"), vec![json!({ "project": "pr_here", "word": "ws_folder" })]);
        // No away rule answering, a folder its thread may not reach: the folder refusal.
        h.f.base.script.borrow_mut().shift_remove("elsewhere_refusal");
        let err = resolve(&h.ctx, "ws_folder", Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            refusal(
                &format!("thread {WORD} may not start children in ws_folder: it is a folder the person keeps of this repository, and a thread started there would stand outside your tree"),
                "Name a project wsp projects lists, or ask the person to start this one.",
                Kind::Usage
            )
        );
        // The same folder, reached: the reach rule, in the words of a thread in a folder.
        h.f.reached.borrow_mut().push("pr_here".to_owned());
        let err = resolve(&h.ctx, "ws_folder", Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            refusal(
                &format!("thread {WORD} may drive the workspace it runs on and the ones it forked, and ws_folder is neither"),
                "Start a child in wsp's folder instead with wsp run wsp \"<message>\", which opens a thread there in your own tree.",
                Kind::Usage
            )
        );
        // A fork's kind says it forks a machine.
        h.f.base.script.borrow_mut().shift_remove("away_for");
        let err = resolve(&h.ctx, "ws_fork", Some(lead())).await.unwrap_err();
        assert_eq!(
            err,
            refusal(
                &format!("thread {WORD} may drive the workspace it runs on and the ones it forked, and ws_fork is neither"),
                "Start a child on spoo instead with wsp run spoo \"<message>\", which forks a machine in your own tree.",
                Kind::Usage
            )
        );
    });
}

#[test]
fn resolve_for_a_thread_names_nothing_as_absent_and_rethrows_a_project_refusal() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        let err = resolve(&h.ctx, "nothing", Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace nothing", Kind::NotFound));
        assert_eq!(h.f.base.args_of("resolve_project"), vec![json!({ "word": "nothing", "scoped": true })]);
        // A refusal of the project door that is not an absence is the answer, not the absence.
        h.f.base.answer_with("resolve_project", vec![base::err_answer("a thread may not use lab", Some("usage"))]);
        let err = resolve(&h.ctx, "lab", Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("a thread may not use lab", Kind::Usage));
        // A held record the thread has no word to refuse with (no project of its own) reads as absent.
        h.hold("ws_theirs", "cloud", "pr_box", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        *h.f.scope_project.borrow_mut() = None;
        h.f.base.script.borrow_mut().shift_remove("resolve_project");
        let err = resolve(&h.ctx, "ws_theirs", Some(lead())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace ws_theirs", Kind::NotFound));
    });
}

// runtime.test.ts:63 "a create whose fork the provider refused is reached by its name and cleared with delete"
#[test]
fn resolve_for_the_person_answers_a_failed_create() {
    run(async {
        let h = H::new();
        let failed = fork_view("ws_f1", "refused", "gone", json!({ "gone": "boom" }));
        h.f.failed.borrow_mut().insert("ws_f1".to_owned(), failed.clone());
        assert_eq!(resolve(&h.ctx, "ws_f1", Some(the_person())).await.unwrap(), failed);
        assert_eq!(resolve(&h.ctx, "refused", Some(the_person())).await.unwrap(), failed);
        assert!(h.f.base.args_of("refuse_relayed").is_empty());
        assert!(h.f.base.args_of("find_places").is_empty());
    });
}

// places.test.ts:188 "names a word that is a place and not a workspace with the road to a workspace there"
#[test]
fn resolve_of_a_place_name_says_a_place_is_no_workspace() {
    run(async {
        let h = H::new();
        h.f.base.answer_with("find_places", vec![json!(["old-macbook", "other"]), json!([])]);
        let err = resolve(&h.ctx, "old-macbook", Some(the_person())).await.unwrap_err();
        assert_eq!(
            err,
            kinded("old-macbook is a computer you joined, not a workspace; its forks are the workspaces. Name a machine on it: wsp threads lists what runs on old-macbook.", Kind::NotFound)
        );
        let err = resolve(&h.ctx, "nothing", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("no workspace nothing", Kind::NotFound));
        assert_eq!(h.f.base.args_of("find_places"), vec![json!({ "word": "old-macbook" }), json!({ "word": "nothing" })]);
    });
}

// box-lead.test.ts:378 a thread on a computer the person joined names by its id the workspace its lead's message goes to
#[test]
fn resolve_of_a_tree_workspace_answers_the_talked_fields() {
    run(async {
        let h = H::new();
        h.hold("ws_far", "place", "pr_box", json!({ "name": "far", "folder": "/srv/far", "home": "/home/far", "gone": "went away" }));
        *h.f.listed.borrow_mut() = Some(vec![]);
        h.f.tree_ids.borrow_mut().push("ws_far".to_owned());
        let found = resolve(&h.ctx, "ws_far", Some(lead())).await.unwrap();
        let mut full = view_json(&h, "ws_far");
        let keep = ["id", "name", "machineId", "phase", "kind", "golden", "createdAt", "project", "gone"];
        full.as_object_mut().unwrap().retain(|k, _| keep.contains(&k.as_str()));
        assert_eq!(json_of(&found), full);
        assert!(h.f.base.args_of("refuse_relayed").is_empty());
        assert!(!json_of(&found).as_object().unwrap().contains_key("folder"));
    });
}

#[test]
fn resolve_stops_at_a_relayed_request() {
    run(async {
        let h = H::new();
        h.hold("ws_a", "local", "pr_here", json!({ "name": "a" }));
        h.f.base.answer_with("refuse_relayed", vec![base::err_answer("a relayed request cannot reach a local workspace", Some("usage"))]);
        let err = resolve(&h.ctx, "ws_a", Some(the_person())).await.unwrap_err();
        assert_eq!(err, kinded("a relayed request cannot reach a local workspace", Kind::Usage));
        assert_eq!(h.f.base.args_of("refuse_relayed"), vec![json!({ "kind": "local", "name": "a", "project": "pr_here", "scoped": false })]);
        assert_eq!(h.calls(&["ready", "held", "refuse_relayed"]), ["ready", "held", "refuse_relayed"]);
    });
}
