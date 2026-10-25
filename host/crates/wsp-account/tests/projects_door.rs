// SPDX-License-Identifier: AGPL-3.0-only
//! The projects door past the add: listing, resolving a word to a project with a thread's own rules, what a project's
//! computer holds, removing a project with everything it made, the start defaults and the branch. Ported from
//! packages/runtime/test/projects.test.ts, places-fork.test.ts, folder-threads.test.ts and the spawn-guard rules at
//! 65f6b89c7, at the area's seam over `FakeProjects`; every sentence is the TS's own text (spec/projects_door.md).
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::rc::Rc;

use base::{kinded, person, project, record, refused, run, thread};
use projects::{catalog, fails, pj, FakeProjects};
use serde_json::{json, Value};
use wsp_account::projects::projects::door::RemoveWords;
use wsp_account::projects::projects::Projects;
use wsp_core::types::api::ProjectsApi;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_words::exit::Kind;

fn rig() -> (Rc<FakeProjects>, Rc<Projects>) {
    let fake = FakeProjects::new();
    let area = fake.area();
    (fake, area)
}

fn p(id: &str, name: &str, computer: &str, created: &str) -> ProjectView {
    project(id, name, computer, json!({ "createdAt": created }))
}

fn hold(fake: &FakeProjects, pr: ProjectView) -> ProjectView {
    fake.base.hold_project(pr.clone());
    fake.base.store.seed("projects", &pr.id, pj(&pr));
    pr
}

fn lead() -> Caller {
    thread("t_lead", "ws_lead", "t_lead")
}

fn ids(list: &[ProjectView]) -> Vec<String> {
    list.iter().map(|x| x.id.clone()).collect()
}

fn unsaved_line() -> &'static str {
    "lab at /home/me/lab holds 2 commits not pushed"
}

// ---------------------------------------------------------------------------------------------------------------
// list, computers

#[test]
fn invariant_3_projects_list_serves_a_thread_the_projects_it_may_start_on() {
    // spec rule 1; the scope is read off the origin, never off the ask (packages/runtime/AGENTS.md invariant 3).
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_a", "alpha", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_b", "beta", "pl_box", "2026-10-02T00:00:00.000Z"));
        hold(&fake, p("pr_c", "gamma", "solari", "2026-10-03T00:00:00.000Z"));
        *fake.reached.borrow_mut() = vec!["pr_b".to_owned()];

        let served = area.list(Some(lead())).await.unwrap();
        assert_eq!(ids(&served), vec!["pr_b"]);
        assert_eq!(
            fake.base.args_of("project_reached"),
            vec![
                json!({ "scoped": true, "project": "pr_a" }),
                json!({ "scoped": true, "project": "pr_b" }),
                json!({ "scoped": true, "project": "pr_c" }),
            ]
        );

        let before = fake.base.args_of("project_reached").len();
        let all = area.list(Some(person())).await.unwrap();
        assert_eq!(ids(&all), vec!["pr_a", "pr_b", "pr_c"]);
        let none = area.list(None).await.unwrap();
        assert_eq!(ids(&none), vec!["pr_a", "pr_b", "pr_c"]);
        assert_eq!(fake.base.args_of("project_reached").len(), before, "a caller with no scope is asked no reach");
        assert_eq!(fake.base.names_of(&["ready"]), vec!["ready"; 3]);
    });
}

#[test]
fn list_sorts_by_created_at_and_keeps_ties_in_order() {
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_late", "late", "here", "2026-10-02T00:00:00.000Z"));
        hold(&fake, p("pr_tie1", "tie-one", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_early", "early", "here", "2026-09-30T00:00:00.000Z"));
        hold(&fake, p("pr_tie2", "tie-two", "pl_box", "2026-10-01T00:00:00.000Z"));
        let listed = area.list(None).await.unwrap();
        assert_eq!(ids(&listed), vec!["pr_early", "pr_tie1", "pr_tie2", "pr_late"]);
    });
}

#[test]
fn computers_answers_the_rows_after_ready() {
    // spec rule 2.
    run(async {
        let (fake, area) = rig();
        let rows = area.computers().await.unwrap();
        assert_eq!(
            rows.iter().map(|r| (r.id.as_str(), r.name.as_str())).collect::<Vec<_>>(),
            vec![("here", "Mac"), ("pl_box", "box"), ("solari", "solari")]
        );
        assert_eq!(fake.base.names_of(&["ready", "computer_rows"]), vec!["ready", "computer_rows"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// resolve

#[test]
fn resolve_takes_an_id_before_a_name() {
    run(async {
        let (fake, area) = rig();
        // The first project is named with the id of the second: the id wins, wherever the project sits.
        let namesake = hold(&fake, p("pr_y", "pr_x", "here", "2026-10-01T00:00:00.000Z"));
        let by_id = hold(&fake, p("pr_x", "lab", "here", "2026-10-02T00:00:00.000Z"));
        let twin = hold(&fake, p("pr_z", "lab", "pl_box", "2026-10-03T00:00:00.000Z"));
        assert_eq!(area.resolve("pr_x", None).await.unwrap(), by_id);
        assert_ne!(by_id, namesake);
        // Two projects of one name: the first held answers.
        assert_eq!(area.resolve("lab", None).await.unwrap(), by_id);
        assert_ne!(by_id, twin);
        assert_eq!(area.resolve("pr_y", None).await.unwrap(), namesake);
    });
}

#[test]
fn a_project_is_named_by_id_or_by_name_and_a_word_that_names_none_is_refused_with_the_ones_there_are() {
    // projects.test.ts:263; the words are the TS test's own.
    run(async {
        let (fake, area) = rig();
        let landing = hold(&fake, project("pr_1", "spoo-landing", "solari", json!({})));
        assert_eq!(area.resolve("pr_1", None).await.unwrap(), landing);
        assert_eq!(area.resolve("spoo-landing", None).await.unwrap(), landing);
        let refusal = area.resolve("nothing", None).await.unwrap_err();
        assert_eq!(refusal, kinded("no project \"nothing\"; this host holds spoo-landing", Kind::NotFound));
    });
}

#[test]
fn resolve_for_the_person_names_every_project_when_none_matches() {
    // spec rule 3, the person's half: every name in held order, a refusal of kind not-found.
    run(async {
        let (fake, area) = rig();
        hold(&fake, project("pr_1", "spoo-landing", "here", json!({ "createdAt": "2026-10-02T00:00:00.000Z" })));
        hold(&fake, project("pr_2", "lab", "here", json!({ "createdAt": "2026-10-01T00:00:00.000Z" })));
        let refusal = area.resolve("nothing", Some(person())).await.unwrap_err();
        assert_eq!(refusal, kinded("no project \"nothing\"; this host holds spoo-landing, lab", Kind::NotFound));
        assert_eq!(fake.base.names_of(&["project_of_scope", "project_reached"]), Vec::<String>::new(), "the person is asked no scope");
    });
}

#[test]
fn resolve_for_a_thread_answers_its_own_project_first() {
    // spec rule 3: a thread's own project by id or by name answers before any other of that name, and is never asked
    // whether it may be reached.
    run(async {
        let (fake, area) = rig();
        let other = hold(&fake, p("pr_other", "lab", "pl_box", "2026-10-01T00:00:00.000Z"));
        let own = hold(&fake, p("pr_own", "lab", "here", "2026-10-02T00:00:00.000Z"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        assert_eq!(area.resolve("lab", Some(lead())).await.unwrap(), own);
        assert_eq!(area.resolve("pr_own", Some(lead())).await.unwrap(), own);
        assert_eq!(fake.base.args_of("project_reached"), Vec::<Value>::new());
        assert_eq!(fake.base.args_of("project_of_scope"), vec![json!({ "scoped": true }); 2]);
        // Another project it may reach is answered by its id.
        *fake.reached.borrow_mut() = vec!["pr_other".to_owned()];
        assert_eq!(area.resolve("pr_other", Some(lead())).await.unwrap(), other);
        assert_eq!(fake.base.args_of("project_reached"), vec![json!({ "scoped": true, "project": "pr_other" })]);
    });
}

#[test]
fn resolve_for_a_thread_refuses_in_the_ts_order() {
    // spec rule 3: no scope project, no match, an elsewhere refusal, the folder rule, then the repository rule.
    run(async {
        let bare = |word: &str| kinded(&format!("no project \"{word}\""), Kind::NotFound);
        let folder = refused(
            "thread t_lead may not start children in lab: it is a folder the person keeps of this repository, and a thread started there would stand outside your tree",
            "Name a project wsp projects lists, or ask the person to start this one.",
            Kind::Usage,
        );
        let repository = refused(
            "thread t_lead works on frontend, and other is a project of another repository; a thread starts children on its own repository's projects alone",
            "Name a project of the same repository, or ask the person to start this one.",
            Kind::Usage,
        );
        let elsewhere = refused("lab lives on box; a thread there starts nothing here", "Ask the person.", Kind::Usage);

        // (a) the scope names no project: every word is absent, even one the host holds.
        let (fake, area) = rig();
        hold(&fake, p("pr_lab", "lab", "here", "2026-10-01T00:00:00.000Z"));
        assert_eq!(area.resolve("lab", Some(lead())).await.unwrap_err(), bare("lab"));
        assert_eq!(fake.base.names_of(&["project_reached", "elsewhere_refusal", "of_threads_repository"]), Vec::<String>::new());

        // (b) its own project is no longer held and the word names none.
        let (fake, area) = rig();
        fake.base.answer_with("project_of_scope", vec![json!("pr_gone")]);
        assert_eq!(area.resolve("nothing", Some(lead())).await.unwrap_err(), bare("nothing"));

        // (c) its own project is held and the word names none.
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here", "2026-10-01T00:00:00.000Z"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        assert_eq!(area.resolve("nothing", Some(lead())).await.unwrap_err(), bare("nothing"));

        // (d) a project it may not reach, where the context says why: that refusal, whole.
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_lab", "lab", "pl_box", "2026-10-02T00:00:00.000Z"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        fake.base.answer_with("elsewhere_refusal", vec![fails(&elsewhere.message, Some("usage"))]);
        let got = area.resolve("lab", Some(lead())).await.unwrap_err();
        assert_eq!((got.message.as_str(), got.kind.clone()), (elsewhere.message.as_str(), Some(Kind::Usage)));
        assert_eq!(fake.base.args_of("elsewhere_refusal"), vec![json!({ "scoped": true, "project": "pr_lab", "word": "lab" })]);
        assert_eq!(fake.base.args_of("of_threads_repository"), Vec::<Value>::new());

        // (e) no elsewhere refusal, and the project is a folder of the thread's repository.
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_lab", "lab", "here", "2026-10-02T00:00:00.000Z"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        fake.base.answer_with("of_threads_repository", vec![json!(true)]);
        assert_eq!(area.resolve("lab", Some(lead())).await.unwrap_err(), folder);
        assert_eq!(fake.base.names_of(&["project_reached", "elsewhere_refusal", "of_threads_repository"]), vec!["project_reached", "elsewhere_refusal", "of_threads_repository"]);

        // (f) every other project: the repository rule, naming the thread's own project.
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_x", "other", "here", "2026-10-02T00:00:00.000Z"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        assert_eq!(area.resolve("other", Some(lead())).await.unwrap_err(), repository);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// folder_stands, seed_into, unsaved

#[test]
fn folder_stands_is_false_on_a_road_with_none() {
    // spec rule 4: the road's answer where it has one, false where it has none.
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_here", "lab", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_box", "web", "pl_box", "2026-10-02T00:00:00.000Z"));
        assert!(!area.folder_stands("pr_here").await.unwrap());
        fake.base.answer_with("road_folder_stands:box", vec![json!(true)]);
        assert!(area.folder_stands("web").await.unwrap());
        fake.base.answer_with("road_folder_stands:mac", vec![json!(false)]);
        assert!(!area.folder_stands("lab").await.unwrap());
        let asked = fake.base.args_of("road_folder_stands");
        assert_eq!(asked.iter().map(|a| a["kind"].clone()).collect::<Vec<_>>(), vec![json!("mac"), json!("box"), json!("mac")]);
        assert_eq!(asked[1]["project"], "pr_box");
        assert_eq!(asked[1]["deps"]["computerName"], "box");
        fake.base.answer_with("road_folder_stands:box", vec![fails("pl_box said no", Some("unreachable"))]);
        assert_eq!(area.folder_stands("pr_box").await.unwrap_err().message, "pl_box said no");
        assert_eq!(area.folder_stands("nothing").await.unwrap_err(), kinded("no project \"nothing\"; this host holds lab, web", Kind::NotFound));
    });
}

#[test]
fn seed_into_refuses_a_road_with_no_folder() {
    // spec rule 5: a road with no seed_into is a plain error naming the project and the computer.
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_box", "web", "pl_box", "2026-10-01T00:00:00.000Z"));
        let plan = projects::menu("/home/me/web");
        let refusal = area.seed_into("pr_box", plan.clone(), vec![".env".to_owned()]).await.unwrap_err();
        assert_eq!(refusal, base::plain("web keeps no folder of its own on box to copy files into"));
        // A road with one gets the archive of the files named, with no memory and no commits.
        fake.base.answer_with("road_seed_into:box", vec![json!(null)]);
        area.seed_into("pr_box", plan.clone(), vec![".env".to_owned()]).await.unwrap();
        let packed = fake.base.args_of("seed_wiring_pack");
        let last = packed.last().unwrap();
        assert_eq!(last["choice"], json!({ "files": [".env"], "memory": false, "commits": false }));
        assert_eq!(last["plan"], serde_json::to_value(&plan).unwrap());
        assert_eq!(last["homes"], json!({ "claude": "/home/me/.claude" }));
        let landed = fake.base.args_of("road_seed_into");
        assert_eq!(landed.last().unwrap()["tar"], "seed");
        assert_eq!(landed.last().unwrap()["project"], "pr_box");
    });
}

#[test]
fn unsaved_asks_the_road() {
    // spec rule 6: the road's line where it has one, none where it has none.
    run(async {
        let (fake, area) = rig();
        let lab = hold(&fake, p("pr_lab", "lab", "here", "2026-10-01T00:00:00.000Z"));
        assert_eq!(area.unsaved(&lab).await.unwrap(), None);
        fake.base.answer_with("road_unsaved:mac", vec![json!(unsaved_line())]);
        assert_eq!(area.unsaved(&lab).await.unwrap(), Some(unsaved_line().to_owned()));
        fake.base.answer_with("road_unsaved:mac", vec![fails("git said no", None)]);
        assert_eq!(area.unsaved(&lab).await.unwrap_err().message, "git said no");
        let asked = fake.base.args_of("road_unsaved");
        assert_eq!(asked.len(), 3);
        assert_eq!(asked[0]["kind"], "mac");
        assert_eq!(asked[0]["deps"]["computerName"], "Mac");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// remove

/// A project here named lab, with a git top, a state path and a copier wired.
fn removable() -> (Rc<FakeProjects>, Rc<Projects>, ProjectView) {
    let (fake, area) = rig();
    fake.copier.set(true);
    let lab = hold(&fake, project("pr_lab", "lab", "here", json!({ "git": { "top": "/home/me/lab" } })));
    (fake, area, lab)
}

fn nothing_removed(fake: &FakeProjects) {
    assert_eq!(fake.base.store.writes.borrow().clone(), Vec::<String>::new());
    assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
    assert_eq!(fake.base.names_of(&["delete_workspace", "copier_worktree_forget", "road_remove", "forget_icon"]), Vec::<String>::new());
    assert!(fake.base.projects.borrow().contains_key("pr_lab"));
}

#[test]
fn remove_counts_only_folders_a_thread_names_as_standing() {
    // spec rule 7: a bare folder (a folder record no thread names) does not stand; a record of a machine always does.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.hold(record("ws_a", "local", "pr_lab", json!({ "name": "work" })));
        fake.base.hold(record("ws_bare", "local", "pr_lab", json!({ "name": "folder" })));
        fake.base.hold(record("ws_cloud", "cloud", "pr_lab", json!({ "name": "forked" })));
        fake.base.hold(record("ws_else", "local", "pr_else", json!({ "name": "elsewhere" })));
        *fake.holding.borrow_mut() = vec!["ws_a".to_owned()];
        let refusal = area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap_err();
        assert_eq!(refusal, kinded("lab has workspaces standing on it: work, forked; delete their threads first", Kind::Conflict));
        nothing_removed(&fake);
        assert_eq!(
            fake.base.args_of("holds_thread"),
            vec![json!({ "id": "ws_a" }), json!({ "id": "ws_bare" }), json!({ "id": "ws_cloud" })]
        );

        // With only the bare folder left, the remove goes through and takes that record with it.
        fake.base.live.borrow_mut().shift_remove("ws_a");
        fake.base.live.borrow_mut().shift_remove("ws_cloud");
        let said = area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap();
        assert_eq!(said.said, "lab removed");
        assert_eq!(fake.base.args_of("delete_workspace").iter().map(|a| a["id"].clone()).collect::<Vec<_>>(), vec![json!("ws_bare")]);
        assert!(fake.base.live.borrow().contains_key("ws_else"));
    });
}

#[test]
fn takes_the_records_no_thread_names_with_it_a_worktree_wsp_made_and_the_frozen_cop() {
    // folder-threads.test.ts:766 at the seam: a thread stands and the remove is refused with the frozen copies left;
    // once none stands, the bare record goes and the frozen copies of the project's worktrees with it.
    run(async {
        let (fake, area, lab) = removable();
        fake.base.hold(record("ws_home", "local", "pr_lab", json!({ "name": "home" })));
        fake.base.hold(record("ws_tree", "local", "pr_lab", json!({ "name": "feat-w" })));
        *fake.holding.borrow_mut() = vec!["ws_home".to_owned()];
        let refusal = area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap_err();
        assert_eq!(refusal, kinded("lab has workspaces standing on it: home; delete their threads first", Kind::Conflict));
        assert_eq!(fake.base.args_of("copier_worktree_forget"), Vec::<Value>::new());
        fake.holding.borrow_mut().clear();
        area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap();
        assert_eq!(
            fake.base.args_of("copier_worktree_forget"),
            vec![json!({ "from": "/home/me/lab", "home": "/home/me/.wsp", "project": lab.id })]
        );
        assert_eq!(fake.base.args_of("delete_workspace").iter().map(|a| a["id"].clone()).collect::<Vec<_>>(), vec![json!("ws_home"), json!("ws_tree")]);
        assert!(fake.base.live.borrow().is_empty());
        assert_eq!(area.list(None).await.unwrap(), vec![]);
    });
}

#[test]
fn a_project_is_dropped_once_no_workspace_stands_on_it_and_the_drop_goes_out_as_an_event() {
    // projects.test.ts:250.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.hold(record("ws_work", "cloud", "pr_lab", json!({ "name": "work" })));
        let refusal = area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap_err();
        assert_eq!(refusal, kinded("lab has workspaces standing on it: work; delete their threads first", Kind::Conflict));
        fake.base.live.borrow_mut().shift_remove("ws_work");
        area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap();
        assert_eq!(area.list(None).await.unwrap(), vec![]);
        assert_eq!(fake.base.events.borrow().clone(), vec![json!({ "type": "project.removed", "projectId": "pr_lab" })]);
    });
}

#[test]
fn remove_refuses_unsaved_work_unless_forced() {
    // spec rule 7: the unsaved refusal is usage, names the work and the way past it; force skips the question.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("road_unsaved", vec![json!("2 commits no remote has")]);
        let refusal = area.remove_with("pr_lab", Some(person()), RemoveWords::default()).await.unwrap_err();
        assert_eq!(
            refusal,
            refused(
                "2 commits no remote has, which removing lab would lose",
                "Copy that work off its computer from the path named or push its branches, then remove lab again; wsp projects remove lab --force removes it anyway.",
                Kind::Usage,
            )
        );
        nothing_removed(&fake);
        assert_eq!(fake.base.args_of("road_unsaved").len(), 1);

        let forced = area.remove_with("pr_lab", Some(person()), RemoveWords { force: true, ..Default::default() }).await.unwrap();
        assert_eq!((forced.said.as_str(), forced.unsaved), ("lab removed", None));
        assert_eq!(fake.base.args_of("road_unsaved").len(), 1, "a forced remove asks nothing of the road's unsaved read");
        assert!(!fake.base.projects.borrow().contains_key("pr_lab"));
    });
}

#[test]
fn stops_a_project_s_own_remove_on_its_folder_holding_work_no_remote_has_naming_its() {
    // places-fork.test.ts:564: refused, the check reads the same and takes nothing, then forced.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("road_unsaved", vec![json!(unsaved_line())]);
        let refusal = area.remove_with("lab", None, RemoveWords::default()).await.unwrap_err();
        assert_eq!(refusal.message, "lab at /home/me/lab holds 2 commits not pushed, which removing lab would lose. Copy that work off its computer from the path named or push its branches, then remove lab again; wsp projects remove lab --force removes it anyway.");
        let checked = area.remove_with("lab", None, RemoveWords { check: true, force: true, ..Default::default() }).await.unwrap();
        assert_eq!((checked.said.as_str(), checked.unsaved.as_deref()), ("", Some(unsaved_line())));
        assert!(fake.base.projects.borrow().contains_key("pr_lab"));
        area.remove_with("lab", None, RemoveWords { force: true, ..Default::default() }).await.unwrap();
        assert!(!fake.base.projects.borrow().contains_key("pr_lab"));
    });
}

#[test]
fn removes_with_no_force_a_computer_whose_project_folder_holds_nothing_beyond_its_r() {
    // places-fork.test.ts:409 at the seam: a road that reads nothing unsaved lets the remove go with no force.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("road_unsaved", vec![Value::Null]);
        let done = area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap();
        assert_eq!((done.said.as_str(), done.unsaved), ("lab removed", None));
        assert_eq!(fake.base.args_of("road_unsaved").len(), 1);
        assert!(!fake.base.projects.borrow().contains_key("pr_lab"));
    });
}

#[test]
fn remove_with_check_answers_unsaved_and_removes_nothing() {
    // spec rule 7 and its edge case: force with check asks unsaved and answers it; check with none unsaved answers
    // an empty said and no unsaved.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("road_unsaved", vec![json!(unsaved_line())]);
        let forced = area.remove_with("pr_lab", None, RemoveWords { force: true, check: true, keep_look: false }).await.unwrap();
        assert_eq!((forced.said.as_str(), forced.unsaved.as_deref()), ("", Some(unsaved_line())));
        nothing_removed(&fake);
        assert_eq!(fake.base.args_of("road_unsaved").len(), 1);

        fake.base.answer_with("road_unsaved", vec![Value::Null]);
        let clean = area.remove_with("pr_lab", None, RemoveWords { check: true, ..Default::default() }).await.unwrap();
        assert_eq!((clean.said.as_str(), clean.unsaved), ("", None));
        nothing_removed(&fake);
        // The same words through the api door: force and check only.
        let through: wsp_core::types::api::ProjectRemoved = area.remove("pr_lab", None, Some(true), Some(true)).await.unwrap();
        assert_eq!((through.said.as_str(), through.unsaved), ("", None));
    });
}

#[test]
fn remove_deletes_the_records_then_the_road_then_the_row_and_forgets_the_look() {
    // spec rule 7, in order: records, the frozen copies, the road's own remove (the record still held), the held map,
    // the store row, the event, the look.
    run(async {
        let (fake, area, lab) = removable();
        fake.base.hold(record("ws_a", "local", "pr_lab", json!({ "name": "work" })));
        fake.base.hold(record("ws_b", "local", "pr_lab", json!({ "name": "folder" })));
        fake.base.answer_with("road_remove", vec![json!("lab is no longer a project here; its code is where it was")]);
        let done = area.remove_with("pr_lab", Some(person()), RemoveWords::default()).await.unwrap();
        assert_eq!(done.said, "lab is no longer a project here; its code is where it was");
        assert_eq!(
            fake.base.names_of(&["spawn_guard", "delete_workspace", "copier_worktree_forget", "road_remove", "forget_icon"]),
            vec!["spawn_guard", "delete_workspace", "delete_workspace", "copier_worktree_forget", "road_remove", "forget_icon"]
        );
        assert_eq!(fake.base.args_of("spawn_guard"), vec![json!({ "act": "delete", "scoped": false })]);
        assert_eq!(fake.base.args_of("copier_worktree_forget"), vec![json!({ "from": "/home/me/lab", "home": "/home/me/.wsp", "project": "pr_lab" })]);
        // The road runs while the record is still held and nothing has been written.
        let road = &fake.base.args_of("road_remove")[0];
        assert_eq!((road["held"].clone(), road["writes"].clone()), (json!(true), json!([])));
        // The look goes last: the record, the row and the event are all gone by then.
        let look = &fake.base.args_of("forget_icon")[0];
        assert_eq!(look["project"], lab.id);
        assert_eq!((look["held"].clone(), look["writes"].clone(), look["events"].clone()), (json!(false), json!(["delete projects/pr_lab"]), json!(1)));
        assert_eq!(fake.base.store.row("projects", "pr_lab"), None);
        assert_eq!(fake.base.events.borrow().clone(), vec![json!({ "type": "project.removed", "projectId": "pr_lab" })]);
        assert!(fake.base.live.borrow().is_empty());
        assert!(fake.base.projects.borrow().is_empty());
    });
}

#[test]
fn remove_with_keep_look_keeps_the_image() {
    run(async {
        let (fake, area, _lab) = removable();
        area.remove_with("pr_lab", None, RemoveWords { keep_look: true, ..Default::default() }).await.unwrap();
        assert_eq!(fake.base.names_of(&["forget_icon"]), Vec::<String>::new());
        assert_eq!(fake.base.events.borrow().clone(), vec![json!({ "type": "project.removed", "projectId": "pr_lab" })]);
        // The api door has no keep-look and forgets.
        fake.base.hold_project(project("pr_two", "two", "here", json!({})));
        area.remove("pr_two", None, None, None).await.unwrap();
        assert_eq!(fake.base.args_of("forget_icon").iter().map(|a| a["project"].clone()).collect::<Vec<_>>(), vec![json!("pr_two")]);
    });
}

#[test]
fn a_thread_cannot_remove_a_project() {
    // spec rule 7: spawn_guard("delete", origin) is asked after ready and before anything is resolved or read.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("spawn_guard", vec![fails("a thread may not delete a project", Some("usage"))]);
        let refusal = area.remove_with("pr_lab", Some(lead()), RemoveWords { force: true, ..Default::default() }).await.unwrap_err();
        assert_eq!(refusal, kinded("a thread may not delete a project", Kind::Usage));
        assert_eq!(fake.base.args_of("spawn_guard"), vec![json!({ "act": "delete", "scoped": true })]);
        assert_eq!(fake.base.names(), vec!["ready", "spawn_guard"]);
        nothing_removed(&fake);
    });
}

#[test]
fn remove_keeps_the_record_when_the_road_cannot_reach_its_computer() {
    // spec rule 7: the road's remove runs before the record goes, so a computer that cannot be reached keeps both; and
    // a computer that cannot be read at all is refused before anything is deleted.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("road_remove", vec![fails("pl_box did not answer", Some("unreachable"))]);
        let refusal = area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap_err();
        assert_eq!(refusal.message, "pl_box did not answer");
        assert!(fake.base.projects.borrow().contains_key("pr_lab"));
        assert!(fake.base.store.row("projects", "pr_lab").is_some());
        assert_eq!(fake.base.events.borrow().clone(), Vec::<Value>::new());
        assert_eq!(fake.base.names_of(&["forget_icon"]), Vec::<String>::new());

        // A joined computer that is not connected: its absent sentence, before a record or a copy is touched.
        let (fake, area) = rig();
        hold(&fake, p("pr_box", "web", "pl_box", "2026-10-01T00:00:00.000Z"));
        fake.base.hold(record("ws_bare", "place", "pr_box", json!({ "name": "folder" })));
        fake.base.answer_with("forking_at", vec![fails("no backend", None)]);
        fake.base.answer_with("landing_backend", vec![fails("box is not answering; it connects on its own when it is on", None)]);
        let refusal = area.remove_with("web", None, RemoveWords::default()).await.unwrap_err();
        assert_eq!(refusal.message, "box is not answering; it connects on its own when it is on");
        assert_eq!(fake.base.names_of(&["delete_workspace", "road_remove"]), Vec::<String>::new());
        assert!(fake.base.live.borrow().contains_key("ws_bare"));
    });
}

#[test]
fn remove_forgets_the_frozen_copies_only_for_a_folder_here_with_a_top_and_a_state_path() {
    // spec rule 7: a project top, a state path, a copier and a copying computer; each missing one skips the call.
    run(async {
        for (name, computer, top, state_path) in [("no state path", "here", true, false), ("a box", "pl_box", true, true), ("no git top", "here", false, true)] {
            let (fake, area) = rig();
            fake.copier.set(true);
            fake.state_path.set(state_path);
            let over = if top { json!({ "git": { "top": "/home/me/lab" } }) } else { json!({}) };
            hold(&fake, project("pr_lab", "lab", computer, over));
            area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap();
            assert_eq!(fake.base.args_of("copier_worktree_forget"), Vec::<Value>::new(), "{name}");
            assert!(!fake.base.projects.borrow().contains_key("pr_lab"), "{name}");
        }
    });
}

#[test]
fn a_failed_forget_of_the_frozen_copies_is_warned_and_the_remove_goes_on() {
    // spec rule 7: the failure is warned with the TS words and the remove carries on.
    run(async {
        let (fake, area, _lab) = removable();
        fake.base.answer_with("copier_worktree_forget", vec![fails("disk full", None)]);
        let warned = wsp_core::console::capture();
        area.remove_with("pr_lab", None, RemoveWords::default()).await.unwrap();
        assert_eq!(warned.lines(), vec!["the frozen copies of lab's worktrees stayed: disk full"]);
        assert!(!fake.base.projects.borrow().contains_key("pr_lab"));
        assert_eq!(fake.base.events.borrow().clone(), vec![json!({ "type": "project.removed", "projectId": "pr_lab" })]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// defaults, branch, icons

fn three_projects(fake: &FakeProjects) {
    hold(fake, p("pr_3", "third", "here", "2026-10-03T00:00:00.000Z"));
    hold(fake, p("pr_1", "first", "here", "2026-10-01T00:00:00.000Z"));
    hold(fake, p("pr_2", "second", "pl_box", "2026-10-02T00:00:00.000Z"));
    fake.base.answer_with("agent_lists:here", vec![json!([catalog("claude")])]);
    fake.base.answer_with("agent_lists:pl_box", vec![json!([catalog("claude"), catalog("codex")])]);
    fake.base.answer_with("thread_defaults:pr_2", vec![json!({ "agent": { "value": "codex", "from": "project" } })]);
}

#[test]
fn defaults_read_each_computers_agent_lists_once() {
    // spec rule 8: lists read once per computer, shared by its projects; the answer in list order.
    run(async {
        let (fake, area) = rig();
        three_projects(&fake);
        let defaults = area.defaults(Some(person())).await.unwrap();
        assert_eq!(defaults.keys().cloned().collect::<Vec<_>>(), vec!["pr_1", "pr_2", "pr_3"]);
        assert_eq!(serde_json::to_value(&defaults["pr_1"]).unwrap(), json!({ "agent": { "value": "claude", "from": "catalog" } }));
        assert_eq!(serde_json::to_value(&defaults["pr_2"]).unwrap(), json!({ "agent": { "value": "codex", "from": "project" } }));
        assert_eq!(
            fake.base.args_of("agent_lists"),
            vec![json!({ "computer": "here", "scoped": false }), json!({ "computer": "pl_box", "scoped": false })]
        );
        assert_eq!(
            fake.base.args_of("thread_defaults"),
            vec![
                json!({ "project": "pr_1", "firstAgent": "claude", "lists": ["claude"] }),
                json!({ "project": "pr_2", "firstAgent": "claude", "lists": ["claude", "codex"] }),
                json!({ "project": "pr_3", "firstAgent": "claude", "lists": ["claude"] }),
            ]
        );
        assert_eq!(fake.base.names_of(&["preferences"]), vec!["preferences"]);
        // A thread is listed only what it may start on, and its scope reaches the agent read.
        *fake.reached.borrow_mut() = vec!["pr_2".to_owned()];
        let mine = area.defaults(Some(lead())).await.unwrap();
        assert_eq!(mine.keys().cloned().collect::<Vec<_>>(), vec!["pr_2"]);
        assert_eq!(fake.base.args_of("agent_lists").last().unwrap(), &json!({ "computer": "pl_box", "scoped": true }));
    });
}

#[test]
fn defaults_fall_back_to_the_default_agent_when_none_runs() {
    // spec rule 8: the first agent that runs, else the default, is the context's `first_agent`; the area hands whatever
    // it answers to every project (the fallback itself is the context's, so this pins the hand-over).
    run(async {
        let (fake, area) = rig();
        fake.runs.borrow_mut().clear();
        three_projects(&fake);
        area.defaults(None).await.unwrap();
        assert_eq!(fake.base.names_of(&["first_agent"]), vec!["first_agent"]);
        let first: Vec<Value> = fake.base.args_of("thread_defaults").iter().map(|a| a["firstAgent"].clone()).collect();
        assert_eq!(first, vec![json!("claude"); 3]);
        fake.base.answer_with("first_agent", vec![json!("codex")]);
        area.defaults(None).await.unwrap();
        let first: Vec<Value> = fake.base.args_of("thread_defaults").iter().skip(3).map(|a| a["firstAgent"].clone()).collect();
        assert_eq!(first, vec![json!("codex"); 3]);
    });
}

#[test]
fn branch_asks_the_landing_road() {
    // spec rule 9, projects.test.ts:273: the branch the project names for a new copy, off the record and asking no
    // machine: resolve, the deps, the road of the project's computer.
    run(async {
        let (fake, area) = rig();
        let plain = hold(&fake, project("pr_plain", "plain", "solari", json!({ "defaultBranch": "trunk" })));
        hold(&fake, project("pr_based", "based", "solari", json!({ "base": "release" })));
        fake.base.answer_with("road_branch", vec![json!({ "branch": "trunk", "folder": false })]);
        let got = area.branch(&plain.id, None).await.unwrap();
        assert_eq!((got.branch.as_deref(), got.folder), (Some("trunk"), false));
        fake.base.answer_with("road_branch:provider", vec![json!({ "branch": "release", "folder": false })]);
        let based = area.branch("based", None).await.unwrap();
        assert_eq!((based.branch.as_deref(), based.folder), (Some("release"), false));
        let asked = fake.base.args_of("road_branch");
        assert_eq!(asked.iter().map(|a| (a["kind"].clone(), a["project"].clone())).collect::<Vec<_>>(), vec![(json!("provider"), json!("pr_plain")), (json!("provider"), json!("pr_based"))]);
        assert_eq!(asked[0]["deps"]["computerName"], "solari");
        // A folder here answers with its own branch and says it is a folder.
        hold(&fake, project("pr_here", "lab", "here", json!({})));
        fake.base.answer_with("road_branch:mac", vec![json!({ "branch": "feat/x", "folder": true })]);
        let here = area.branch("lab", None).await.unwrap();
        assert_eq!((here.branch.as_deref(), here.folder), (Some("feat/x"), true));
        assert_eq!(fake.base.args_of("road_branch").last().unwrap()["kind"], "mac");
    });
}

#[test]
fn branch_resolves_a_thread_s_word_by_the_thread_s_rules() {
    // spec rule 9: the resolve takes the origin, so a project a thread may not reach is refused before the road is asked.
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here", "2026-10-01T00:00:00.000Z"));
        hold(&fake, p("pr_lab", "lab", "here", "2026-10-02T00:00:00.000Z"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        let refusal = area.branch("lab", Some(lead())).await.unwrap_err();
        assert_eq!(refusal.kind, Some(Kind::Usage));
        assert_eq!(fake.base.args_of("road_branch"), Vec::<Value>::new());
    });
}

#[test]
fn icon_and_icons_forward_to_the_project_icons_area() {
    // spec rule 10.
    run(async {
        let (fake, area) = rig();
        fake.base.answer_with("set_icon", vec![json!("abc123")]);
        assert_eq!(area.icon("pr_lab", Some("PNG".to_owned())).await.unwrap(), Some("abc123".to_owned()));
        assert_eq!(fake.base.args_of("set_icon"), vec![json!({ "project": "pr_lab", "png": "PNG" })]);
        fake.base.answer_with("read_icons", vec![json!({ "abc123": "PNG", "gone": null })]);
        let read = area.icons(vec!["abc123".to_owned(), "gone".to_owned()]).await.unwrap();
        assert_eq!(read.iter().map(|(k, v)| (k.as_str(), v.as_deref())).collect::<Vec<_>>(), vec![("abc123", Some("PNG")), ("gone", None)]);
        assert_eq!(fake.base.args_of("read_icons"), vec![json!({ "hashes": ["abc123", "gone"] })]);
    });
}
