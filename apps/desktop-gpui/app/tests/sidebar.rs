// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar against a host started for these tests alone (host-client's own harness): what it reads, what it
//! writes, and what the host's pushes change, each checked on the sidebar and, for a settle, a restore and a delete, on
//! the host itself; a rename is checked by the host's answer the sidebar says.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::shared;
use desktop_gpui::sidebar::model::{Item, Section, Shape};
use desktop_gpui::sidebar::{Link, Processes, Sidebar};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, TestAppContext, VisualTestContext};
use serde_json::Value;
use wsp_host_client::protocol::SessionsListRequest;

/// The sidebar in a window as the app draws it, attached to the shared host.
fn attached(cx: &mut TestAppContext) -> (Entity<Sidebar>, &mut VisualTestContext) {
    // The host's replies wake the app from tokio's threads, which a deterministic test executor refuses.
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let host = shared();
    let client = runtime.block_on(host.host.client());
    let sidebar = cx.new(Sidebar::new);
    sidebar.update(cx, |s, cx| s.attach(Link { host: Arc::new(client), runtime }, cx));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(sidebar.clone(), window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    (sidebar, cx)
}

/// Runs the app's tasks until `done` holds of the sidebar, as pushes arrive from the host's own threads.
fn wait_for(cx: &mut VisualTestContext, sidebar: &Entity<Sidebar>, what: &str, done: impl Fn(&Sidebar) -> bool) {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if sidebar.read_with(cx, |s, _| done(s)) {
            return;
        }
        let note = sidebar.read_with(cx, |s, _| s.note().map(str::to_owned));
        assert!(Instant::now() < until, "the sidebar never showed {what}; its note: {note:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn key_of(s: &Sidebar, title: &str) -> Option<String> {
    s.threads().iter().find(|t| t.title == title).map(|t| t.key.clone())
}

/// The tile a thread is drawn as, by title: its shape, or none where the list does not draw it.
fn shape_of(s: &Sidebar, title: &str) -> Option<Shape> {
    s.lines().iter().find_map(|l| match &l.item {
        Item::Tile(t) if s.threads()[t.thread].title == title => Some(t.shape),
        _ => None,
    })
}

/// The host's own row for a thread, read on a client of its own.
fn host_row(thread: &str) -> Option<Value> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let client = shared().host.client().await;
        let request: SessionsListRequest = serde_json::from_value(serde_json::json!({})).unwrap();
        let rows = client.call(request).await.unwrap().sessions;
        rows.iter().map(|r| serde_json::to_value(r).unwrap()).find(|r| r["threadId"] == thread)
    })
}

#[gpui_kit::test]
fn reads_the_threads_and_draws_each_as_a_tile(cx: &mut TestAppContext) {
    let (sidebar, cx) = attached(cx);
    wait_for(cx, &sidebar, "the first thread", |s| key_of(s, "first").is_some());
    let key = sidebar.read_with(cx, |s, _| key_of(s, "first").unwrap());
    let thread = sidebar.read_with(cx, |s, _| s.threads().iter().find(|t| t.key == key).cloned().unwrap());
    assert_eq!(thread.project_name.as_deref(), Some("repo"));
    assert!(thread.unread, "a finished turn nobody opened reads as done");
    assert_eq!(sidebar.read_with(cx, |s, _| shape_of(s, "first")), Some(Shape::Full));
    assert!(sidebar.read_with(cx, |s, _| s.cloud_card_shown()), "a host with this computer alone offers a cloud");
    cx.run_until_parked();
    assert!(cx.debug_bounds(Box::leak(format!("tile-{key}").into_boxed_str())).is_some(), "the tile is drawn in the window");
}

#[gpui_kit::test]
fn a_thread_started_elsewhere_arrives_as_the_host_pushes_it(cx: &mut TestAppContext) {
    let (sidebar, cx) = attached(cx);
    wait_for(cx, &sidebar, "the first read", |s| !s.threads().is_empty());
    assert_eq!(sidebar.read_with(cx, |s, _| key_of(s, "pushed")), None);
    shared().host.wsp(&["run", "repo", "pushed", "--agent", "claude"]);
    wait_for(cx, &sidebar, "the thread wsp run opened", |s| shape_of(s, "pushed") == Some(Shape::Full));
}

#[gpui_kit::test]
fn settle_restore_rename_and_delete_reach_the_host(cx: &mut TestAppContext) {
    let (sidebar, cx) = attached(cx);
    shared().host.wsp(&["run", "repo", "to write on", "--agent", "claude"]);
    wait_for(cx, &sidebar, "the thread to write on", |s| key_of(s, "to write on").is_some());
    let key = sidebar.read_with(cx, |s, _| key_of(s, "to write on").unwrap());

    // This host runs no wsp-daemon, so a settle that asks about the thread's processes is left standing; keeping
    // them is the answer the web's question offers.
    sidebar.update(cx, |s, cx| s.settle(&key, cx));
    wait_for(cx, &sidebar, "why the settle was left", |s| s.note().is_some_and(|n| n.starts_with("Not settled")));
    sidebar.update(cx, |s, cx| s.settle_with(&key, Processes::Keep, cx));
    wait_for(cx, &sidebar, "the thread under Settled", |s| {
        s.lines().iter().any(|l| matches!(l.item, Item::Head { section: Section::Settled, .. })) && shape_of(s, "to write on").is_none()
    });
    assert!(host_row(&key).is_some_and(|r| r["settledAt"].is_number()), "the host keeps the settle");

    sidebar.update(cx, |s, cx| s.restore(&key, cx));
    wait_for(cx, &sidebar, "the thread back in the list", |s| shape_of(s, "to write on") == Some(Shape::Full));
    assert!(host_row(&key).is_some_and(|r| !r["settledAt"].is_number()), "the host drops the settle");

    // A rename goes through the agent's own session, which the stand-in agent keeps none of: the host's answer is
    // said over the list and the title stays.
    sidebar.update(cx, |s, cx| s.rename(&key, "written on", cx));
    wait_for(cx, &sidebar, "the host's answer to the rename", |s| s.note() == Some("Not renamed: no-session"));
    assert!(sidebar.read_with(cx, |s, _| key_of(s, "to write on")).is_some());

    sidebar.update(cx, |s, cx| s.delete_with(&key, Processes::Keep, cx));
    wait_for(cx, &sidebar, "the thread gone", |s| key_of(s, "to write on").is_none());
    assert!(host_row(&key).is_none(), "the host has no row of the thread left");
}
