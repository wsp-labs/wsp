// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `stateFolder`, `projectReached`, `elsewhereRefusal`, `ofThreadsRepository` and `scratch` throw through
//! the projects area. Each test drives the public entry that reaches the call and asserts the throw is the answer.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/projects.rs"]
mod projects;

use std::cell::RefCell;
use std::rc::Rc;

use base::{plain, project, record, run, thread};
use projects::{FakeBundler, FakeProjects};
use serde_json::{json, Value};
use wsp_account::projects::projects::door::RemoveWords;
use wsp_account::projects::projects::Projects;
use wsp_core::types::internal::ImportReport;
use wsp_core::types::wiring::ProjectImportOptions;
use wsp_wire::views::workspace::ProjectView;

const SOURCE: &str = "/Users/dev/code/proj";

fn rig() -> (Rc<FakeProjects>, Rc<Projects>) {
    let fake = FakeProjects::new();
    let area = fake.area();
    (fake, area)
}

fn hold(fake: &FakeProjects, pr: ProjectView) -> ProjectView {
    fake.base.hold_project(pr.clone());
    fake.base.store.seed("projects", &pr.id, projects::pj(&pr));
    pr
}

fn p(id: &str, name: &str, computer: &str) -> ProjectView {
    project(id, name, computer, json!({ "createdAt": "2026-10-01T00:00:00.000Z" }))
}

fn lead() -> wsp_wire::views::workspace::Caller {
    thread("t_lead", "ws_lead", "t_lead")
}

fn reporter() -> ImportReport {
    Rc::new(move |_stage, _message, _progress| Ok(()))
}

// projects.ts:343-348 (list): ctx.projectReached is the filter of a thread's list, so its throw rejects the list.
#[test]
fn ctx_throws_project_reached_passes_through_list() {
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here"));
        *fake.reached_fail.borrow_mut() = Some(plain("the thread's project is not readable"));
        let got = area.list(Some(lead())).await.expect_err("projectReached throws");
        assert_eq!(got.message, "the thread's project is not readable");
        assert_eq!(fake.base.args_of("project_reached").len(), 1);
    });
}

// projects.ts:355-379 (resolve): a thread naming a project it does not reach asks elsewhereRefusal, which throws.
#[test]
fn ctx_throws_elsewhere_refusal_passes_through_resolve() {
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here"));
        hold(&fake, p("pr_lab", "lab", "pl_box"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        *fake.elsewhere_fail.borrow_mut() = Some(plain("lab's computer is not reachable"));
        let got = area.resolve("lab", Some(lead())).await.expect_err("elsewhereRefusal throws");
        assert_eq!(got.message, "lab's computer is not reachable");
        assert_eq!(fake.base.names_of(&["of_threads_repository", "resolve_project"]), Vec::<String>::new());
    });
}

// projects.ts:355-379 (resolve): after no elsewhere refusal, ofThreadsRepository is asked, and its throw is the answer.
#[test]
fn ctx_throws_of_threads_repository_passes_through_resolve() {
    run(async {
        let (fake, area) = rig();
        hold(&fake, p("pr_own", "frontend", "here"));
        hold(&fake, p("pr_lab", "lab", "here"));
        fake.base.answer_with("project_of_scope", vec![json!("pr_own")]);
        *fake.threads_fail.borrow_mut() = Some(plain("the repository is not readable"));
        let got = area.resolve("lab", Some(lead())).await.expect_err("ofThreadsRepository throws");
        assert_eq!(got.message, "the repository is not readable");
        assert_eq!(fake.base.args_of("of_threads_repository").len(), 1);
    });
}

// projects.ts:426 (remove): stateFolder is an argument of the frozen copies' forget, read before the road removes anything.
#[test]
fn ctx_throws_state_folder_passes_through_remove() {
    run(async {
        let (fake, area) = rig();
        fake.copier.set(true);
        hold(&fake, project("pr_lab", "lab", "here", json!({ "git": { "top": "/home/me/lab" } })));
        *fake.state_fail.borrow_mut() = Some(plain("the state folder is not readable"));
        let got = area.remove_with("pr_lab", None, RemoveWords::default()).await.expect_err("stateFolder throws");
        assert_eq!(got.message, "the state folder is not readable");
        assert_eq!(fake.base.names_of(&["copier_worktree_forget", "road_remove", "forget_icon"]), Vec::<String>::new());
        assert!(fake.base.projects.borrow().contains_key("pr_lab"));
    });
}

// moves.rs (copyImport): scratch is read after packing and before the road lands, so its throw lands nothing.
#[test]
fn ctx_throws_scratch_passes_through_copy_import() {
    run(async {
        let (fake, area) = rig();
        let entry = fake.base.hold(record("ws_1", "cloud", "pr_x", json!({ "name": "task-1" })));
        *fake.scratch_fail.borrow_mut() = Some(plain("the scratch folder is not writable"));
        let plan = json!({ "source": SOURCE, "repo": false, "files": 1, "bytes": 10, "secrets": [], "values": [], "excluded": [], "skipped": [], "agents": [] });
        let opts = ProjectImportOptions {
            workspace_id: "ws_1".to_owned(),
            source: SOURCE.to_owned(),
            dest: SOURCE.to_owned(),
            replace: None,
            carry: None,
            rewrite: None,
            agents: None,
            bundler: FakeBundler::new(&fake.base, plan),
        };
        let got = area.copy_import(&entry, opts, reporter()).await.expect_err("scratch throws");
        assert_eq!(got.message, "the scratch folder is not writable");
        assert_eq!(fake.base.names_of(&["land_bundle"]), Vec::<String>::new());
    });
}
