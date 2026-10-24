// SPDX-License-Identifier: AGPL-3.0-only
//! What a thread switch asks the host, counted on the wire: the thread screen as the frame mounts it, against a host
//! started for this test alone and reached through a proxy that reads every frame the app sends. Once each thread of
//! the workspace was opened, the composer's side of a switch asks nothing: no `agents.read`, which runs the agents' own
//! command lines on the host, no `preferences.get`, and no workspace `sessions.list`.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
mod wire;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::composer::model::Hold;
use desktop_gpui::screens::composer::{self};
use desktop_gpui::screens::thread_view::{header, ThreadScreen};
use desktop_gpui::screens::{self, Opened, THREAD};
use desktop_gpui::sidebar::ThreadItem;
use desktop_gpui::thread_view::{ThreadView, ThreadViewEvent};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, TestAppContext, VisualTestContext};
use wsp_host_client::HostClient;

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + Duration::from_secs(40);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn settle(cx: &mut VisualTestContext) {
    let end = Instant::now() + Duration::from_millis(800);
    while Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn a_switch_asks_the_host_nothing_for_the_composer_once_each_thread_was_open(cx: &mut TestAppContext) {
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    for prompt in ["short-1", "short-2", "short-3"] {
        host.wsp(&["run", "repo", prompt, "--agent", "claude"]);
    }
    let (sent, _dir, state) = wire::proxied(&host);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = runtime.block_on(HostClient::connect(&state, &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let id = |prompt: &str| threads.iter().find(|t| t.prompt.as_deref() == Some(prompt)).and_then(|t| t.thread_id.clone()).unwrap();
    let workspace = threads[0].workspace_id.clone();
    let order = ["short-1", "short-2", "short-3"].map(id);
    let shell = screens::Host::new(Arc::new(client), runtime.clone());

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        composer::init(cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let tally = |ops: &[(String, bool)]| {
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for (op, named) in ops {
            *by.entry(if *named { format!("{op} (workspace)") } else { op.clone() }).or_default() += 1;
        }
        by
    };
    let switches = 6;

    // The baseline: the thread view alone, with the header's read on each load, as the screen has them less its composer.
    let baseline = {
        let view = cx.new(ThreadView::new);
        let (_, base) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        base.simulate_resize(size(px(1100.), px(800.)));
        base.update(|_, cx| {
            cx.subscribe(&view, |view, event: &ThreadViewEvent, cx| {
                if matches!(event, ThreadViewEvent::Loaded) {
                    view.update(cx, header::refresh);
                }
            })
            .detach()
        });
        let follow = |thread: &str, base: &mut VisualTestContext| {
            let item =
                ThreadItem { workspace_id: workspace.clone(), thread_id: thread.to_owned(), project: String::new(), title: String::new() };
            view.update(base, |v, cx| v.follow(&shell, &item, false, cx));
            until(base, "the thread loaded", |cx| view.read_with(cx, |v, _| v.thread() == Some(thread) && v.loaded()));
            settle(base);
        };
        for thread in &order {
            follow(thread, base);
        }
        sent.lock().unwrap().clear();
        for at in 0..switches {
            follow(&order[at % 3], base);
        }
        let ops: Vec<(String, bool)> = sent.lock().unwrap().drain(..).collect();
        view.update(base, |v, cx| v.stop(cx));
        settle(base);
        sent.lock().unwrap().clear();
        tally(&ops)
    };
    println!("{switches} switches of the view and header alone: {baseline:?}");

    let entry = screens::screen(THREAD).unwrap();
    let opened = |thread: &str| Opened { workspace_id: Some(workspace.clone()), thread_id: Some(thread.to_owned()), ..Opened::default() };
    let mut mounted = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = (entry.mount)(&shell, opened(&order[0]), window, cx);
        let view = screen.view.clone();
        mounted = Some(screen);
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1100.), px(800.)));
    let mounted = mounted.unwrap();
    let screen = mounted.view.clone().downcast::<ThreadScreen>().ok().unwrap();
    let (view, composer) = screen.read_with(cx, |s, _| (s.view().clone(), s.composer().clone()));
    let open = |thread: &str, cx: &mut VisualTestContext| {
        cx.update(|window, cx| (mounted.show)(opened(thread), window, cx));
        until(cx, "the thread loaded and its composer linked", |cx| {
            view.read_with(cx, |v, _| v.thread() == Some(thread) && v.loaded())
                && composer
                    .read_with(cx, |c, _| c.thread_key() == Some(thread) && !matches!(c.hold(), Some(Hold::Connecting | Hold::HostDown)))
        });
        settle(cx);
    };
    for thread in &order {
        open(thread, cx);
    }
    let first: Vec<(String, bool)> = sent.lock().unwrap().drain(..).collect();
    println!("opening the three threads the first time: {:?}", tally(&first));
    assert_eq!(first.iter().filter(|(op, _)| op == "agents.read").count(), 1, "the skills are read once for the workspace's agent");
    assert_eq!(first.iter().filter(|(op, _)| op == "preferences.get").count(), 1, "midTurn is read once");

    for at in 0..switches {
        open(&order[at % 3], cx);
    }
    let after: Vec<(String, bool)> = sent.lock().unwrap().drain(..).collect();
    println!("{switches} switches among them: {:?}", tally(&after));
    let count = |op: &str, named: bool| after.iter().filter(|(o, n)| o == op && *n == named).count();
    let any = |op: &str| after.iter().filter(|(o, _)| o == op).count();
    assert_eq!(any("agents.read"), 0, "no agent probe on a switch");
    assert_eq!(any("preferences.get"), 0, "no preference read on a switch");
    let was = |op: &str| baseline.get(op).copied().unwrap_or(0);
    assert_eq!(count("sessions.list", true), was("sessions.list (workspace)"), "the composer read the workspace's rows on a switch");
    assert_eq!(count("harnesses.list", true), was("harnesses.list (workspace)"), "the composer read the agents on a switch");
    assert_eq!(count("sessions.list", false), was("sessions.list"), "the composer read the threads on a switch");
}
