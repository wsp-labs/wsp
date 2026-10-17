// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view against a host started for this test alone (host-client's tests/common): it reads a thread whole,
//! marks it read, takes a turn started after it opened as live events, and when the host goes away and comes back it
//! reads the thread again and follows it. It runs without libtest's harness for the reason pixels.rs gives.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::thread_view::cards;
use desktop_gpui::screens::Host;
use desktop_gpui::sidebar::ThreadItem;
use desktop_gpui::thread_view::{Link, ThreadView};
use desktop_gpui::transcript::RowKind;
use desktop_gpui::ui::UiAssets;
use gpui_kit::{AppContext, Entity, HeadlessAppContext};
use wsp_host_client::HostClient;

fn app() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx.allow_parking();
    cx
}

/// Runs the app until `done` holds of the view, or fails after `secs` with what the view shows.
fn until(cx: &mut HeadlessAppContext, view: &Entity<ThreadView>, secs: u64, what: &str, done: impl Fn(&ThreadView) -> bool) {
    let limit = Instant::now() + Duration::from_secs(secs);
    loop {
        cx.run_until_parked();
        if cx.update(|cx| done(view.read(cx))) {
            return;
        }
        let shown = cx.update(|cx| {
            let v = view.read(cx);
            format!(
                "link {:?}, rows {:?}",
                v.link(),
                v.rows().iter().map(|r| r.text.chars().take(30).collect::<String>()).collect::<Vec<_>>()
            )
        });
        assert!(Instant::now() < limit, "{what} within {secs} s; the view shows {shown}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn person(view: &ThreadView, said: &str) -> bool {
    view.rows().iter().any(|r| matches!(r.kind, RowKind::Person { .. }) && r.text == said)
}

fn replies(view: &ThreadView) -> usize {
    view.rows().iter().filter(|r| matches!(r.kind, RowKind::Agent { .. })).count()
}

async fn item(client: &HostClient, prompt: &str) -> ThreadItem {
    let threads = client.threads().await.unwrap_or_else(|f| panic!("{}", f.message));
    let view = threads.iter().find(|t| t.prompt.as_deref() == Some(prompt)).unwrap_or_else(|| panic!("no thread for {prompt}"));
    ThreadItem::of(view).expect("a lead thread")
}

async fn read_at(client: &HostClient, thread: &str) -> Option<f64> {
    let threads = client.threads().await.unwrap_or_else(|f| panic!("{}", f.message));
    threads.iter().find(|t| t.thread_id.as_deref() == Some(thread)).and_then(|t| t.read_at)
}

fn main() {
    the_view_reads_marks_read_follows_live_and_comes_back_after_the_host_does();
    println!("test the_view_reads_marks_read_follows_live_and_comes_back_after_the_host_does ... ok");
}

fn the_view_reads_marks_read_follows_live_and_comes_back_after_the_host_does() {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let mut host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let item = runtime.block_on(item(&client, "first"));
    // The host stamps the thread read as wsp run starts it; opening it in the view stamps it again, later.
    let before = runtime.block_on(read_at(&client, &item.thread_id)).unwrap_or(0.);

    let mut cx = app();
    let view = cx.update(|cx| cx.new(ThreadView::new));
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));

    // Reads: the first turn, the person's message and the stand-in's reply.
    until(&mut cx, &view, 30, "the first turn", |v| person(v, "first") && replies(v) == 1);

    // Answers: a pick goes to the host as sessions.answer, and what the host would not take is said under the prompt.
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let session = threads
        .iter()
        .find(|t| t.thread_id.as_deref() == Some(item.thread_id.as_str()))
        .map(|t| t.id.clone())
        .expect("the thread's session");
    cx.update(|cx| view.update(cx, |v, cx| cards::answer(v, &session, "no-such-ask", "y", cx)));
    until(&mut cx, &view, 20, "the host's answer", |v| cards::refusal_of(v, "no-such-ask").is_some());
    let said = cx.update(|cx| cards::refusal_of(view.read(cx), "no-such-ask")).unwrap().said;
    let words = [
        "The prompt closed before the answer reached it.",
        "This thread's agent raises no prompt this host can answer.",
        "This host holds no turn of that thread.",
        "The prompt carries no option by that id.",
    ];
    assert!(words.contains(&said.as_str()), "the host answered {said}");

    // Writes: opening the thread marks it read.
    let limit = Instant::now() + Duration::from_secs(20);
    while runtime.block_on(read_at(&client, &item.thread_id)).unwrap_or(0.) <= before {
        cx.run_until_parked();
        assert!(Instant::now() < limit, "opening the thread did not mark it read");
        std::thread::sleep(Duration::from_millis(100));
    }

    // Live: a turn started after the view opened arrives as events, without the view reading the thread again.
    assert_eq!(cx.update(|cx| view.read(cx).loads()), 1, "the transcript is read whole once");
    host.wsp(&["send", &item.thread_id, "--detach", "second"]);
    until(&mut cx, &view, 60, "the second turn and its reply", |v| person(v, "second") && replies(v) == 2);
    assert_eq!(cx.update(|cx| view.read(cx).loads()), 1, "a live turn arrives as events, not as the transcript read again");

    // The host goes away: the view says so and keeps what it shows.
    host.stop();
    until(&mut cx, &view, 30, "the lost connection", |v| matches!(v.link(), Link::Lost(_)) && person(v, "second"));

    // It comes back on the same state file: the view reads the thread again, which is what sets the link live.
    host.restart();
    until(&mut cx, &view, 60, "following again", |v| *v.link() == Link::Live && person(v, "second") && replies(v) == 2);
    assert_eq!(cx.update(|cx| view.read(cx).loads()), 2, "coming back reads the transcript whole once more");
}
