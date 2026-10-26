// SPDX-License-Identifier: AGPL-3.0-only
//! A Ctrl+Tab onto a thread whose newest page was read ahead, in the shell main.rs builds against a host started for
//! this test alone: the page draws in the first frame after the key, with no loading line, and the host's own page
//! that follows folds nothing again.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, TestAppContext, VisualTestContext};

fn until(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
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
fn a_ctrl_tab_onto_a_neighbour_read_ahead_draws_its_rows_at_once_with_no_loading_line(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    for prompt in ["first", "second", "third"] {
        host.wsp(&["run", "repo", prompt, "--agent", "claude"]);
    }
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(wsp_host_client::HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let first = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread");
    let (workspace_id, thread_id) = (first.workspace_id.clone(), first.thread_id.clone().expect("a thread id"));

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    shell.update_in(cx, |s, window, cx| {
        s.go(Route::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() }, window, cx)
    });
    until(cx, "the thread screen", |cx| shell.read_with(cx, |s, _| s.thread_screen().is_some()));
    let view = shell.read_with(cx, |s, cx| s.thread_screen().unwrap().read(cx).view().clone());
    until(cx, "the first thread's page", |cx| view.read_with(cx, |v, _| v.thread() == Some(thread_id.as_str()) && v.loaded()));
    let others: Vec<String> = threads.iter().filter_map(|t| t.thread_id.clone()).filter(|t| *t != thread_id).collect();
    until(cx, "both neighbours read ahead", |cx| {
        let ahead = view.read_with(cx, |v, _| v.read_ahead_of());
        others.iter().all(|t| ahead.iter().any(|(_, held)| held == t))
    });
    let loads = view.read_with(cx, |v, _| v.loads());
    // What the view holds the first time it tells of another thread: the key's own update, before any host answer.
    let first: Rc<RefCell<Option<(String, bool, usize)>>> = Rc::default();
    let (seen, left) = (first.clone(), thread_id.clone());
    cx.update(|_, cx| {
        cx.observe(&view, move |view, cx| {
            let v = view.read(cx);
            if seen.borrow().is_none() && v.thread().is_some_and(|t| t != left) {
                *seen.borrow_mut() = Some((v.thread().unwrap().to_owned(), v.loaded(), v.rows().len()));
            }
        })
        .detach()
    });

    cx.simulate_keystrokes(&keys::keystroke("ctrl+tab", cfg!(target_os = "macos")).unwrap());
    let landed = match shell.read_with(cx, |s, _| s.route().clone()) {
        Route::Thread { thread_id, .. } => thread_id,
        other => panic!("Ctrl+Tab opened no thread: {other:?}"),
    };
    assert!(others.contains(&landed), "Ctrl+Tab stayed on the open thread");
    let (shown, loaded, rows) = first.borrow().clone().expect("the view never moved to the thread");
    assert_eq!(shown, landed);
    assert!(loaded && rows > 0, "the page read ahead was not drawn in the frame after the key: loaded {loaded}, {rows} rows");
    assert_eq!(view.read_with(cx, |v, _| v.loads()), loads + 1);
    assert!(cx.debug_bounds("transcript-loading").is_none(), "the loading line showed");
    assert!(cx.debug_bounds("transcript-held").is_none(), "the last thread's rows were held though the page was read");
    assert!(cx.debug_bounds("row-0").is_some(), "the neighbour's rows are not drawn");
    assert!(!view.read_with(cx, |v, _| v.read_ahead_of()).iter().any(|(_, t)| *t == landed), "the page drawn is still held ahead");

    settle(cx);
    assert!(cx.debug_bounds("transcript-loading").is_none(), "the loading line showed once the host's page landed");
    assert_eq!(view.read_with(cx, |v, _| v.loads()), loads + 1, "the host's page, the same as the one read ahead, was folded again");
    until(cx, "the thread left read ahead", |cx| view.read_with(cx, |v, _| v.read_ahead_of()).iter().any(|(_, t)| *t == thread_id));
}

/// A screenshot-sized PNG with noise in it, so it weighs like one.
fn shot(path: &std::path::Path, seed: u32) {
    let mut at = seed.wrapping_mul(2_654_435_761) | 1;
    let pixels = image::RgbImage::from_fn(320, 200, |x, y| {
        at ^= at << 13;
        at ^= at >> 17;
        at ^= at << 5;
        image::Rgb([(x % 251) as u8 ^ (at & 0xff) as u8, (y % 241) as u8, ((x + y) % 239) as u8])
    });
    pixels.save(path).unwrap();
}

/// Ctrl+Tab held through 30 threads, each carrying an image: after every step the warm set stays within its one bound,
/// the pages read ahead are the warm threads' alone, and the images of every thread left behind are gone from the view.
#[gpui_kit::test]
fn a_30_thread_ctrl_tab_walk_keeps_the_warm_set_in_its_bound_and_drops_what_left_threads_held(cx: &mut TestAppContext) {
    use desktop_gpui::screens::warm;
    const THREADS: usize = 30;
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    let named = regex::Regex::new(r"\bthread ([0-9a-f-]{36})\b").unwrap();
    for i in 0..THREADS {
        let said = host.wsp(&["run", "repo", &format!("thread {i}"), "--agent", "claude"]);
        let thread = named.captures(&said).unwrap_or_else(|| panic!("wsp run named no thread: {said}"))[1].to_owned();
        let path = host.dir.path().join(format!("shot-{i}.png"));
        shot(&path, i as u32);
        host.wsp(&["send", &thread, "--file", &path.to_string_lossy(), "what is in it"]);
    }
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(wsp_host_client::HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let top = threads.iter().max_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.))).expect("a thread");
    let (workspace_id, thread_id) = (top.workspace_id.clone(), top.thread_id.clone().expect("a thread id"));

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), Launch::default(), window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    until(cx, "the host", |cx| shell.read_with(cx, |s, _| s.connected()));
    until(cx, "every thread in the sidebar", |cx| shell.read_with(cx, |s, cx| s.sidebar().read(cx).items().len() >= THREADS));
    shell.update_in(cx, |s, window, cx| s.go(Route::Thread { workspace_id, thread_id }, window, cx));
    until(cx, "the thread screen", |cx| shell.read_with(cx, |s, _| s.thread_screen().is_some()));
    let view = shell.read_with(cx, |s, cx| s.thread_screen().unwrap().read(cx).view().clone());

    let mut visited: Vec<String> = Vec::new();
    for step in 0..THREADS {
        let open = match shell.read_with(cx, |s, _| s.route().clone()) {
            Route::Thread { thread_id, .. } => thread_id,
            other => panic!("step {step} opened no thread: {other:?}"),
        };
        until(cx, "the open thread's rows and its image", |cx| {
            view.read_with(cx, |v, _| v.thread() == Some(open.as_str()) && v.loaded() && !v.images_held().0.is_empty())
        });
        settle(cx);
        let set = cx.update(|_, cx| warm::set(cx));
        let held = set.held();
        assert!(held.len() <= warm::HELD, "step {step}: {} threads warm off screen", held.len());
        assert_eq!(set.open().map(|(_, t)| t.as_str()), Some(open.as_str()), "step {step}: the open thread leads the warm set");
        let ahead = view.read_with(cx, |v, _| v.read_ahead_of());
        assert!(ahead.len() <= warm::HELD, "step {step}: {} pages read ahead", ahead.len());
        assert!(ahead.iter().all(|k| held.contains(k)), "step {step}: a page is held for a thread outside the warm set: {ahead:?}");
        for left in visited.iter().filter(|t| !held.iter().any(|(_, w)| w == *t) && **t != open) {
            assert!(!ahead.iter().any(|(_, t)| t == left), "step {step}: {left} left the warm set and kept its page");
        }
        let (rows, (images, released)) = view.read_with(cx, |v, _| {
            let keys: Vec<String> = v.rows().iter().map(|r| r.key.clone()).collect();
            let (images, released) = v.images_held();
            (keys, (images.into_iter().map(str::to_owned).collect::<Vec<_>>(), released))
        });
        assert!(images.iter().all(|k| rows.contains(k)), "step {step}: images of a thread left behind are still held: {images:?}");
        assert_eq!(released, 0, "step {step}: images a left thread let go of never left the atlas");
        visited.push(open);
        cx.simulate_keystrokes(&keys::keystroke("ctrl+tab", cfg!(target_os = "macos")).unwrap());
    }
    assert_eq!(visited.iter().collect::<std::collections::HashSet<_>>().len(), THREADS, "the walk did not reach every thread");
}
