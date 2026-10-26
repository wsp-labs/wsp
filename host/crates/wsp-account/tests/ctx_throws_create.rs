// SPDX-License-Identifier: AGPL-3.0-only
//! The context's lookups that throw in the create and lookup half: `actRefusal`, `placeOfThread`, `listedFor`,
//! `backendOfKind(...).capabilities`, and the rules a word to a workspace is judged by (`refusalFor`, `ofThreadsRepository`,
//! `elsewhereRefusal`, `awayFor`, `projectReached`). Each throw is the area's answer and nothing after it runs.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/create.rs"]
mod create_support;

use std::rc::Rc;

use base::{plain, project, record, run};
use create_support::{asker, FakeCreate};
use serde_json::{json, Value};
use wsp_account::projects::workspaces::create::{create, landing, list, resolve, CreateCtx};
use wsp_core::types::wiring::CreateWorkspaceOptions;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

const THREAD: &str = "t_lead1234567";

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
    fn hold(&self, id: &str, kind: &str, project: &str, over: Value) {
        self.f.base.hold(record(id, kind, project, over));
    }
    fn calls(&self, of: &[&str]) -> Vec<String> {
        self.f.base.names_of(of)
    }
    fn throws(&self, method: &str, message: &str) {
        self.f.throws.borrow_mut().insert(method.to_owned(), plain(message));
    }
}

fn lead() -> Caller {
    asker(THREAD, "ws_lead", "t_root")
}

fn opts(project: &str, name: &str) -> CreateWorkspaceOptions {
    CreateWorkspaceOptions { project: project.to_owned(), name: name.to_owned(), ..Default::default() }
}

/// A thread's scope over the project it works on, with no workspace of its own listed.
fn thread_setup(h: &H) {
    *h.f.scope_project.borrow_mut() = Some("pr_cloud".to_owned());
    *h.f.listed.borrow_mut() = Some(vec![]);
}

fn err_of<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Ok(_) => panic!("expected the throw"),
        Err(e) => e,
    }
}

// workspaces.ts:70 (landing): a thread's sized landing asks actRefusal, whose throw is the answer.
#[test]
fn ctx_throws_act_refusal_passes_through_landing() {
    run(async {
        let h = H::new();
        h.throws("act_refusal", "the thread's act could not be read");
        let got = err_of(landing(&h.ctx, "pr_cloud", true, Some(lead())).await);
        assert_eq!(got.message, "the thread's act could not be read");
        assert!(h.calls(&["resolve_project"]).is_empty(), "no project resolved after the throw");
    });
}

// workspaces.ts:90 (create): a thread's golden create reads placeOfThread for its refusal, and the throw ends the create.
#[test]
fn ctx_throws_place_of_thread_passes_through_create() {
    run(async {
        let h = H::new();
        h.throws("place_of_thread", "the thread's place is not readable");
        let mut o = opts("pr_cloud", "kid");
        o.golden = Some("snap_g".to_owned());
        let got = err_of(create(&h.ctx, o, Some(lead())).await);
        assert_eq!(got.message, "the thread's place is not readable");
        assert_eq!(h.calls(&["place_of_thread", "persist", "forking_add"]), vec!["place_of_thread".to_string()]);
    });
}

// workspaces.ts:231 (list): a thread's list is what listedFor answers, so its throw is the list's.
#[test]
fn ctx_throws_listed_for_passes_through_list() {
    run(async {
        let h = H::new();
        h.throws("listed_for", "the thread's tree is not readable");
        let got = err_of(list(&h.ctx, Some(lead())).await);
        assert_eq!(got.message, "the thread's tree is not readable");
        assert!(h.calls(&["drives"]).is_empty(), "no view judged after the throw");
    });
}

// workspaces.ts:77 (landing): `backendOfKind(kind, place).capabilities` on a folder kind throws where no backend answers.
#[test]
fn ctx_throws_backend_capabilities_passes_through_landing() {
    run(async {
        let h = H::new();
        h.throws("backend_capabilities", "no backend answers for this kind");
        let got = err_of(landing(&h.ctx, "pr_here", false, None).await);
        assert_eq!(got.message, "no backend answers for this kind");
        assert!(h.calls(&["landing_place", "landing_capabilities"]).is_empty(), "no landing read after the throw");
    });
}

// workspaces.ts:262 (resolve, absent branch): refusalFor is asked first; its throw is the answer.
#[test]
fn ctx_throws_refusal_for_passes_through_resolve() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        h.hold("ws_theirs", "cloud", "pr_box", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        h.throws("refusal_for", "the workspace's record is not readable");
        let got = err_of(resolve(&h.ctx, "ws_theirs", Some(lead())).await);
        assert_eq!(got.message, "the workspace's record is not readable");
        assert_eq!(
            h.calls(&["refusal_for", "of_threads_repository", "elsewhere_refusal", "away_for", "project_reached"]),
            vec!["refusal_for".to_string()]
        );
    });
}

// workspaces.ts:264 (resolve): after a refusal, ofThreadsRepository is asked of the workspace's project.
#[test]
fn ctx_throws_of_threads_repository_passes_through_resolve() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        h.hold("ws_theirs", "cloud", "pr_box", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        h.throws("of_threads_repository", "the repository is not readable");
        let got = err_of(resolve(&h.ctx, "ws_theirs", Some(lead())).await);
        assert_eq!(got.message, "the repository is not readable");
        assert_eq!(
            h.calls(&["refusal_for", "of_threads_repository", "elsewhere_refusal", "away_for", "project_reached"]),
            vec!["refusal_for".to_string(), "of_threads_repository".to_string()]
        );
    });
}

// workspaces.ts:265 (resolve): a folder workspace asks elsewhereRefusal, whose throw is the answer.
#[test]
fn ctx_throws_elsewhere_refusal_passes_through_resolve() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        h.hold("ws_theirs", "local", "pr_here", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        h.throws("elsewhere_refusal", "the folder's computer is not reachable");
        let got = err_of(resolve(&h.ctx, "ws_theirs", Some(lead())).await);
        assert_eq!(got.message, "the folder's computer is not reachable");
        assert_eq!(
            h.calls(&["refusal_for", "of_threads_repository", "elsewhere_refusal", "away_for", "project_reached"]),
            vec!["refusal_for".to_string(), "of_threads_repository".to_string(), "elsewhere_refusal".to_string()]
        );
    });
}

// workspaces.ts:265 (resolve): a non-folder workspace asks awayFor, whose throw is the answer.
#[test]
fn ctx_throws_away_for_passes_through_resolve() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        h.hold("ws_theirs", "cloud", "pr_box", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        h.throws("away_for", "the workspace is away");
        let got = err_of(resolve(&h.ctx, "ws_theirs", Some(lead())).await);
        assert_eq!(got.message, "the workspace is away");
        assert_eq!(
            h.calls(&["refusal_for", "of_threads_repository", "elsewhere_refusal", "away_for", "project_reached"]),
            vec!["refusal_for".to_string(), "of_threads_repository".to_string(), "away_for".to_string()]
        );
    });
}

// workspaces.ts:268 (resolve): when no other refusal stands, projectReached is asked for the folder rule.
#[test]
fn ctx_throws_project_reached_passes_through_resolve() {
    run(async {
        let h = H::new();
        thread_setup(&h);
        h.hold("ws_theirs", "cloud", "pr_box", json!({ "name": "theirs" }));
        h.f.hidden.borrow_mut().push("ws_theirs".to_owned());
        h.throws("project_reached", "the thread's reach is not readable");
        let got = err_of(resolve(&h.ctx, "ws_theirs", Some(lead())).await);
        assert_eq!(got.message, "the thread's reach is not readable");
        assert_eq!(
            h.calls(&["refusal_for", "of_threads_repository", "elsewhere_refusal", "away_for", "project_reached"]),
            vec!["refusal_for".to_string(), "of_threads_repository".to_string(), "away_for".to_string(), "project_reached".to_string()]
        );
    });
}
