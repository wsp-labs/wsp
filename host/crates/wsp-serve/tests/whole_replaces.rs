// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/replaces.test.ts at 65f6b89c7, case for case. A restart: a thread started with replaces names
//! the thread it replaces, the host settles that one once the new one's first turn starts, and both rows list the link,
//! through a host restart too. Each test names the TS test above it.
mod common;

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;
use wsp_core::types::wiring::SessionHandle;
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::{HostError, Kind};
use wsp_words::views::session::fold_threads;

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

struct Lead {
    h: Rc<Held>,
    fx: Fx,
    ws: String,
    l: String,
    a: String,
    b: String,
    stuck: Rc<dyn SessionHandle>,
}

fn as_thread(ws: &str, thread: &str, root: &str) -> Caller {
    let by =
        serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": ws, "rootThreadId": root })).expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Relayed, by }
}

/// A lead with two children, the first stuck working and the second done; turns are held, so each ends only when the
/// case ends it. The lead's turn is 0, the stuck child's 1, the done child's 2.
async fn lead(store: &Rc<dyn Store>, backend: &Arc<StubBackend>) -> Lead {
    let h = Held::new(false);
    let fx = runtime(backend).store(store).adapter("claude", h.factory()).build();
    let rt = &*fx.rt;
    let ws = create_on(rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let l = rt.sessions().start(&ws, start("lead the work"), None).await.expect("a start").view().thread_id.expect("a thread");
    let stuck = rt
        .sessions()
        .start(
            &ws,
            StartOptions { title: Some("Landing fix: security fixes".into()), ..start("build the parser") },
            Some(as_thread(&ws, &l, &l)),
        )
        .await
        .expect("a start");
    let a = stuck.view().thread_id.expect("a thread");
    let done = rt.sessions().start(&ws, start("build the printer"), Some(as_thread(&ws, &l, &l))).await.expect("a start");
    let b = done.view().thread_id.expect("a thread");
    h.end(2, "printer built", json!({}));
    h.end(0, "waiting on the children", json!({}));
    settle().await;
    Lead { h, fx, ws, l, a, b, stuck }
}

async fn fresh() -> (Rc<dyn Store>, Arc<StubBackend>, State) {
    let state = sqlite_state();
    (state.store.clone(), StubBackend::new(None), state)
}

async fn threads(rt: &AssembledRuntime, ws: &str) -> BTreeMap<String, Value> {
    let rows = rt.sessions().list(Some(ws.to_string()), None).await.expect("a list");
    fold_threads(&rows).into_iter().map(|t| (t.id.clone(), j(&t))).collect()
}

async fn refused<T>(r: impl std::future::Future<Output = Result<T, HostError>>) -> HostError {
    match r.await {
        Ok(_) => panic!("the call was not refused"),
        Err(e) => e,
    }
}

fn short(id: &str) -> &str {
    &id[..8]
}

fn is_number(v: &Value) -> bool {
    v.is_number()
}

// "a restart that names the thread it replaces" > "records the link both ways, settles the replaced thread once the restart starts, and keeps both across a host restart"
#[test]
fn records_the_link_both_ways_settles_the_replaced_thread_once_the_restart_starts_and_keeps_both_across_a_host_restart() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        c.h.end(1, "", json!({ "status": "interrupted" }));
        c.stuck.finished().await;
        settle().await;
        assert!(threads(rt, &c.ws).await[&c.a].get("settledAt").is_none());
        let again = rt
            .sessions()
            .start(
                &c.ws,
                StartOptions {
                    title: Some("Landing fix: security fixes".into()),
                    replaces: Some(c.a.clone()),
                    ..start("build the parser")
                },
                Some(as_thread(&c.ws, &c.l, &c.l)),
            )
            .await
            .expect("a start");
        let a2 = again.view().thread_id.expect("a thread");
        settle().await;
        let now = threads(rt, &c.ws).await;
        assert_matches(&now[&a2], json!({ "replaces": c.a, "status": "running", "parentThreadId": c.l }));
        assert!(now[&a2].get("settledAt").is_none());
        assert_matches(&now[&c.a], json!({ "replacedBy": a2, "status": "interrupted" }));
        assert!(is_number(&now[&c.a]["settledAt"]));
        let rows = rt.sessions().list(Some(c.ws.clone()), None).await.expect("a list");
        let row_of = |id: &str| j(rows.iter().find(|r| r.thread_id.as_deref() == Some(id)).expect("a row"));
        assert_matches(&row_of(&a2), json!({ "replaces": c.a }));
        assert_matches(&row_of(&c.a), json!({ "replacedBy": a2 }));
        c.h.end(3, "parser built", json!({}));
        again.finished().await;
        rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&store).adapter("claude", c.h.factory()).build();
        let kept = threads(&rt2.rt, &c.ws).await;
        assert_matches(&kept[&a2], json!({ "replaces": c.a }));
        assert_matches(&kept[&c.a], json!({ "replacedBy": a2 }));
        assert!(is_number(&kept[&c.a]["settledAt"]));
        rt2.rt.close().await.expect("a close");
    });
}

// "replaces a failed thread the same way"
#[test]
fn replaces_a_failed_thread_the_same_way() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        c.h.end(1, "", json!({ "status": "failed", "error": "no parser" }));
        c.stuck.finished().await;
        settle().await;
        let again = rt
            .sessions()
            .start(&c.ws, StartOptions { replaces: Some(c.a.clone()), ..start("build the parser") }, Some(as_thread(&c.ws, &c.l, &c.l)))
            .await
            .expect("a start");
        settle().await;
        let now = threads(rt, &c.ws).await;
        assert_matches(&now[&c.a], json!({ "status": "failed", "replacedBy": again.view().thread_id }));
        assert!(is_number(&now[&c.a]["settledAt"]));
        c.h.end(3, "parser built", json!({}));
        rt.close().await.expect("a close");
    });
}

// "is refused for a working thread, saying stop it first, and starts nothing"
#[test]
fn is_refused_for_a_working_thread_saying_stop_it_first_and_starts_nothing() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        let before = threads(rt, &c.ws).await.len();
        let e = refused(rt.sessions().start(
            &c.ws,
            StartOptions { replaces: Some(c.a.clone()), ..start("build the parser") },
            Some(as_thread(&c.ws, &c.l, &c.l)),
        ))
        .await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains(&format!("thread {} is still working", short(&c.a))), "{}", e.message);
        assert_eq!(threads(rt, &c.ws).await.len(), before);
        assert!(threads(rt, &c.ws).await[&c.a].get("replacedBy").is_none());
        c.h.end(1, "parser built", json!({}));
        rt.close().await.expect("a close");
    });
}

// "is refused outside a thread's own tree, naming the thread, and for a name nothing holds"
#[test]
fn is_refused_outside_a_threads_own_tree_naming_the_thread_and_for_a_name_nothing_holds() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        c.h.end(1, "", json!({ "status": "interrupted" }));
        c.stuck.finished().await;
        settle().await;
        let e = refused(rt.sessions().start(
            &c.ws,
            StartOptions { replaces: Some(c.a.clone()), ..start("build the parser") },
            Some(as_thread(&c.ws, &c.b, &c.l)),
        ))
        .await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains(&format!("thread {} is not under this thread", short(&c.a))), "{}", e.message);
        let e = refused(rt.sessions().start(
            &c.ws,
            StartOptions { replaces: Some(c.l.clone()), ..start("lead again") },
            Some(as_thread(&c.ws, &c.a, &c.l)),
        ))
        .await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains(&format!("thread {}", short(&c.l))), "{}", e.message);
        let e = refused(rt.sessions().start(
            &c.ws,
            StartOptions { replaces: Some("thr_nobody".into()), ..start("build it") },
            Some(as_thread(&c.ws, &c.l, &c.l)),
        ))
        .await;
        assert_eq!(e.kind, Some(Kind::NotFound));
        assert_eq!(e.message, "no thread thr_nobo");
        assert!(threads(rt, &c.ws).await[&c.a].get("settledAt").is_none());
        c.h.end(1, "parser built", json!({}));
        rt.close().await.expect("a close");
    });
}

// "is refused on a send into a thread that has run, which replaces nothing"
#[test]
fn is_refused_on_a_send_into_a_thread_that_has_run_which_replaces_nothing() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        c.h.end(1, "", json!({ "status": "interrupted" }));
        c.stuck.finished().await;
        settle().await;
        let e = refused(rt.sessions().start(
            &c.ws,
            StartOptions { thread: Some(c.b.clone()), replaces: Some(c.a.clone()), ..start("and the parser") },
            None,
        ))
        .await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains("a restart opens a thread"), "{}", e.message);
        rt.close().await.expect("a close");
    });
}

// "is refused for a thread that already has a restart, naming that restart, so the chain stays one line"
#[test]
fn is_refused_for_a_thread_that_already_has_a_restart_naming_that_restart_so_the_chain_stays_one_line() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        c.h.end(1, "", json!({ "status": "interrupted" }));
        c.stuck.finished().await;
        settle().await;
        let again = rt
            .sessions()
            .start(&c.ws, StartOptions { replaces: Some(c.a.clone()), ..start("build the parser") }, Some(as_thread(&c.ws, &c.l, &c.l)))
            .await
            .expect("a start");
        let a2 = again.view().thread_id.expect("a thread");
        c.h.end(3, "", json!({ "status": "interrupted" }));
        again.finished().await;
        settle().await;
        let e = refused(rt.sessions().start(
            &c.ws,
            StartOptions { replaces: Some(c.a.clone()), ..start("build the parser") },
            Some(as_thread(&c.ws, &c.l, &c.l)),
        ))
        .await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains(&format!("thread {} was restarted as {}", short(&c.a), short(&a2))), "{}", e.message);
        let e = refused(rt.sessions().replaceable(&c.a, Some(as_thread(&c.ws, &c.l, &c.l)))).await;
        assert_eq!(e.kind, Some(Kind::Usage));
        let now = threads(rt, &c.ws).await;
        assert_matches(&now[&c.a], json!({ "replacedBy": a2 }));
        let restarts: Vec<String> =
            now.values().filter(|t| t["replaces"] == c.a.as_str()).map(|t| t["id"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(restarts, std::slice::from_ref(&a2));
        rt.close().await.expect("a close");
    });
}

// "answers its refusals as a read of its own, which a verb makes before it forks or wakes a machine"
#[test]
fn answers_its_refusals_as_a_read_of_its_own_which_a_verb_makes_before_it_forks_or_wakes_a_machine() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        let e = refused(rt.sessions().replaceable(&c.a, Some(as_thread(&c.ws, &c.l, &c.l)))).await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains(&format!("thread {} is still working", short(&c.a))), "{}", e.message);
        c.h.end(1, "", json!({ "status": "interrupted" }));
        c.stuck.finished().await;
        settle().await;
        let e = refused(rt.sessions().replaceable(&c.a, Some(as_thread(&c.ws, &c.b, &c.l)))).await;
        assert_eq!(e.kind, Some(Kind::Usage));
        assert!(e.message.contains(&format!("thread {} is not under this thread", short(&c.a))), "{}", e.message);
        let e = refused(rt.sessions().replaceable("thr_nobody", Some(as_thread(&c.ws, &c.l, &c.l)))).await;
        assert_eq!(e.kind, Some(Kind::NotFound));
        rt.sessions().replaceable(&c.a, Some(as_thread(&c.ws, &c.l, &c.l))).await.expect("resolves");
        rt.sessions().replaceable(&c.a, None).await.expect("resolves");
        rt.close().await.expect("a close");
    });
}

// "started by the person for a child, stands under that child's lead, where the child stood"
#[test]
fn started_by_the_person_for_a_child_stands_under_that_childs_lead_where_the_child_stood() {
    drive(|| async {
        let (store, backend, _state) = fresh().await;
        let c = lead(&store, &backend).await;
        let rt = &*c.fx.rt;
        c.h.end(1, "", json!({ "status": "interrupted" }));
        c.stuck.finished().await;
        settle().await;
        let again = rt
            .sessions()
            .start(&c.ws, StartOptions { replaces: Some(c.a.clone()), ..start("build the parser") }, None)
            .await
            .expect("a start");
        settle().await;
        let now = threads(rt, &c.ws).await;
        assert_matches(
            &now[&again.view().thread_id.expect("a thread")],
            json!({ "replaces": c.a, "parentThreadId": c.l, "rootThreadId": c.l }),
        );
        c.h.end(3, "parser built", json!({}));
        again.finished().await;
        rt.close().await.expect("a close");
    });
}
