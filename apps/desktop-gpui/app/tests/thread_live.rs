// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view against a host started for this test alone (host-client's tests/common): it reads a thread from its
//! newest page, marks it read while its window is active, takes a turn started after it opened as live events, and
//! when the host goes away and comes back it reads the thread again and follows it; a long thread pages back as the
//! reader reaches its top, and a refused read is said once. It runs without libtest's harness for the reason pixels.rs
//! gives.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::thread_view::find;
use desktop_gpui::screens::Host;
use desktop_gpui::sidebar::ThreadItem;
use desktop_gpui::thread_view::{Link, ThreadView};
use desktop_gpui::transcript::RowKind;
use desktop_gpui::ui::UiAssets;
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{px, size, AnyWindowHandle, AppContext, Entity, HeadlessAppContext};
use wsp_host_client::protocol::SessionEvent;
use wsp_host_client::HostClient;

fn app() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    cx.allow_parking();
    cx
}

/// The view in a window of its own, as the thread screen draws it, active as the app's window is when used.
fn window(cx: &mut HeadlessAppContext, active: bool) -> (Entity<ThreadView>, AnyWindowHandle) {
    let mut view = None;
    let handle = cx
        .open_window(size(px(900.), px(600.)), |window, cx| {
            let made = cx.new(ThreadView::new);
            view = Some(made.clone());
            cx.new(|cx| Root::new(made, window, cx))
        })
        .unwrap();
    let handle: AnyWindowHandle = handle.into();
    if active {
        cx.update_window(handle, |_, window, _| window.activate_window()).unwrap();
    }
    (view.unwrap(), handle)
}

/// Draws a frame of `handle`, as the platform would once something changed.
fn draw(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
}

/// Runs the app and draws `handle` until `done` holds of the view, or fails after `secs` with what the view shows.
fn until(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    view: &Entity<ThreadView>,
    secs: u64,
    what: &str,
    done: impl Fn(&ThreadView) -> bool,
) {
    let limit = Instant::now() + Duration::from_secs(secs);
    loop {
        cx.run_until_parked();
        draw(cx, handle);
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
    a_hidden_window_stamps_the_thread_read_only_once_it_is_active();
    println!("test a_hidden_window_stamps_the_thread_read_only_once_it_is_active ... ok");
    a_long_thread_opens_on_its_newest_page_and_pages_back_at_the_top();
    println!("test a_long_thread_opens_on_its_newest_page_and_pages_back_at_the_top ... ok");
    a_refused_history_is_said_and_not_followed_as_a_lost_socket();
    println!("test a_refused_history_is_said_and_not_followed_as_a_lost_socket ... ok");
    a_child_started_after_the_thread_opened_joins_its_listing();
    println!("test a_child_started_after_the_thread_opened_joins_its_listing ... ok");
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
    let (view, handle) = window(&mut cx, true);
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));

    // Reads: the first turn, the person's message and the stand-in's reply.
    until(&mut cx, handle, &view, 30, "the first turn", |v| person(v, "first") && replies(v) == 1);

    // Writes: opening the thread marks it read.
    let limit = Instant::now() + Duration::from_secs(20);
    while runtime.block_on(read_at(&client, &item.thread_id)).unwrap_or(0.) <= before {
        cx.run_until_parked();
        draw(&mut cx, handle);
        assert!(Instant::now() < limit, "opening the thread did not mark it read");
        std::thread::sleep(Duration::from_millis(100));
    }

    // Live: a turn started after the view opened arrives as events, without the view reading the thread again.
    assert_eq!(cx.update(|cx| view.read(cx).loads()), 1, "the transcript is read whole once");
    host.wsp(&["send", &item.thread_id, "--detach", "second"]);
    until(&mut cx, handle, &view, 60, "the second turn and its reply", |v| person(v, "second") && replies(v) == 2);
    assert_eq!(cx.update(|cx| view.read(cx).loads()), 1, "a live turn arrives as events, not as the transcript read again");

    // The host goes away: the view says so and keeps what it shows.
    host.stop();
    until(&mut cx, handle, &view, 30, "the lost connection", |v| matches!(v.link(), Link::Lost(_)) && person(v, "second"));

    // It comes back on the same state file: the view reads the thread again, which is what sets the link live.
    host.restart();
    until(&mut cx, handle, &view, 60, "following again", |v| *v.link() == Link::Live && person(v, "second") && replies(v) == 2);
    assert_eq!(cx.update(|cx| view.read(cx).loads()), 2, "coming back reads the transcript whole once more");
}

/// A claude stand-in whose every turn is 20 events: the start, 16 short replies and a long last one, the result and
/// the end; the long reply makes each turn about a screen and a half tall.
const PAGED: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
long="Said."; i=0
while [ $i -lt 30 ]; do long="$long\\n\\nParagraph $i of the long reply."; i=$((i + 1)); done
while IFS= read -r _; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  i=0
  while [ $i -lt 16 ]; do
    printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"ok"}]}}\n' "$session"
    i=$((i + 1))
  done
  printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"%s"}]}}\n' "$session" "$long"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}\n' "$session"
done
"#;

fn starts(events: &[SessionEvent]) -> usize {
    events.iter().filter(|e| matches!(e, SessionEvent::SessionStart { .. })).count()
}

fn people(view: &ThreadView) -> usize {
    view.rows().iter().filter(|r| matches!(r.kind, RowKind::Person { .. })).count()
}

fn a_long_thread_opens_on_its_newest_page_and_pages_back_at_the_top() {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let mut host = Throwaway::start_with(PAGED, false);
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "turn 0", "--agent", "claude"]);
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let item = runtime.block_on(item(&client, "turn 0"));
    for n in 1..12 {
        host.wsp(&["send", &item.thread_id, &format!("turn {n}")]);
    }
    let whole = runtime.block_on(client.open(&item.workspace_id, &item.thread_id)).unwrap_or_else(|f| panic!("{}", f.message)).history;
    assert_eq!((whole.len(), starts(&whole)), (240, 12), "twelve turns of 20 events");
    let newest = starts(&whole[whole.len() - 200..]);
    assert_eq!(newest, 10, "the newest page holds the last ten turns");

    let mut cx = app();
    let (view, handle) = window(&mut cx, true);
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));
    until(&mut cx, handle, &view, 30, "the newest page", |v| v.loaded() && person(v, "turn 11"));
    for _ in 0..10 {
        draw(&mut cx, handle);
        cx.run_until_parked();
    }
    let (ticks, complete) = cx.update(|cx| (people(view.read(cx)), view.read(cx).complete()));
    assert_eq!((ticks, complete), (newest, false), "opened at its end, the view holds the newest page alone: two ticks fewer");

    cx.update(|cx| view.update(cx, |v, cx| v.scroll_to_top(cx)));
    let top = |v: &ThreadView| v.rows()[v.list().logical_scroll_top().item_ix].key.clone();
    draw(&mut cx, handle);
    let read = cx.update(|cx| top(view.read(cx)));
    until(&mut cx, handle, &view, 30, "the older page", |v| v.complete());
    assert_eq!(cx.update(|cx| people(view.read(cx))), 12, "the page before it brings the rail its two ticks");
    assert!(cx.update(|cx| person(view.read(cx), "turn 0")), "the thread's first message is drawn");
    assert_eq!(cx.update(|cx| top(view.read(cx))), read, "the reader stays on the row they read as the page lands above it");

    // The host goes away and comes back with the reader at the end, where no page is asked for: the newest page is
    // read again over the pages already held.
    cx.update(|cx| view.update(cx, |v, cx| v.scroll_to_end(cx)));
    for _ in 0..5 {
        draw(&mut cx, handle);
        cx.run_until_parked();
    }
    host.stop();
    until(&mut cx, handle, &view, 30, "the lost connection", |v| matches!(v.link(), Link::Lost(_)));
    host.restart();
    until(&mut cx, handle, &view, 60, "following again", |v| *v.link() == Link::Live && v.loads() == 2);
    let (first, ticks, complete) = cx.update(|cx| (person(view.read(cx), "turn 0"), people(view.read(cx)), view.read(cx).complete()));
    assert_eq!((first, ticks, complete), (true, 12, true), "a reconnect keeps the older pages already read");

    // Find, opened at the end, reads in every older page wherever the reader stands.
    let (other, other_handle) = window(&mut cx, true);
    cx.update(|cx| other.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));
    until(&mut cx, other_handle, &other, 30, "the newest page again", |v| v.loaded() && person(v, "turn 11"));
    assert!(!cx.update(|cx| other.read(cx).complete()), "the second view opens on the newest page");
    cx.update_window(other_handle, |_, window, cx| other.update(cx, |v, cx| find::open(v, window, cx))).unwrap();
    until(&mut cx, other_handle, &other, 30, "find to read in the older page", |v| v.complete() && person(v, "turn 0"));
}

fn a_hidden_window_stamps_the_thread_read_only_once_it_is_active() {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let item = runtime.block_on(item(&client, "first"));
    let before = runtime.block_on(read_at(&client, &item.thread_id)).unwrap_or(0.);

    let mut cx = app();
    let (view, handle) = window(&mut cx, false);
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));
    until(&mut cx, handle, &view, 30, "the first turn", |v| person(v, "first") && replies(v) == 1);
    let hidden = Instant::now() + Duration::from_secs(3);
    while Instant::now() < hidden {
        cx.run_until_parked();
        draw(&mut cx, handle);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        runtime.block_on(read_at(&client, &item.thread_id)).unwrap_or(0.),
        before,
        "a window nobody can see stamped the thread read"
    );

    cx.update_window(handle, |_, window, _| window.activate_window()).unwrap();
    let limit = Instant::now() + Duration::from_secs(20);
    while runtime.block_on(read_at(&client, &item.thread_id)).unwrap_or(0.) <= before {
        cx.run_until_parked();
        draw(&mut cx, handle);
        assert!(Instant::now() < limit, "the window turning active did not stamp the thread read");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn a_refused_history_is_said_and_not_followed_as_a_lost_socket() {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let mut item = runtime.block_on(item(&client, "first"));
    item.workspace_id = "no-such-workspace".to_owned();

    let mut cx = app();
    let (view, handle) = window(&mut cx, true);
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));
    until(&mut cx, handle, &view, 30, "the refusal", |v| v.refused().is_some() || matches!(v.link(), Link::Lost(_)));
    let (refused, link, placeholder) = cx.update(|cx| {
        let v = view.read(cx);
        (v.refused().map(str::to_owned), v.link().clone(), v.placeholder())
    });
    assert_eq!(link, Link::Live, "a refusal is not a lost socket: no Reconnecting to the host");
    let refused = refused.expect("the refusal is said");
    assert!(refused.starts_with("history unavailable: "), "{refused}");
    assert_eq!(placeholder, None, "the refusal stands in place of Loading transcript");
}

/// A claude stand-in that answers ok; on a prompt saying spawn it first starts a child thread through wsp from inside
/// its turn, as an agent does with `wsp run`, and the child, told slow, takes a few seconds over its answer.
fn spawns(bin: &std::path::Path) -> String {
    format!(
        r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{{"type":"control_response","response":{{"subtype":"success","request_id":"init","response":{{"models":[{{"value":"default","resolvedModel":"claude-haiku-5-5"}}]}}}}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
while IFS= read -r line; do
  printf '{{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}}\n' "$session"
  case "$line" in *spawn*) node "{bin}" run repo "slow kid" --agent claude --detach > /dev/null 2>&1 ;; *slow*) sleep 4 ;; esac
  printf '{{"type":"assistant","session_id":"%s","message":{{"role":"assistant","content":[{{"type":"text","text":"ok"}}]}}}}\n' "$session"
  printf '{{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"%s"}}\n' "$session"
done
"#,
        bin = bin.display()
    )
}

fn a_child_started_after_the_thread_opened_joins_its_listing() {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let host = Throwaway::start_with(&spawns(&common::root().join("packages/host/dist/bin.js")), false);
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let item = runtime.block_on(item(&client, "first"));

    let mut cx = app();
    let (view, handle) = window(&mut cx, true);
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));
    until(&mut cx, handle, &view, 30, "the listing", |v| v.listing().is_some());
    assert!(cx.update(|cx| view.read(cx).listing().is_some_and(|l| l.children.is_empty())), "no child yet");

    host.wsp(&["send", &item.thread_id, "spawn one"]);
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    assert!(
        threads.iter().any(|t| t.parent_thread_id.as_deref() == Some(item.thread_id.as_str())),
        "the stand-in started no child: {:?}",
        threads.iter().map(|t| (&t.prompt, &t.parent_thread_id)).collect::<Vec<_>>()
    );
    until(&mut cx, handle, &view, 30, "the child in the listing", |v| v.listing().is_some_and(|l| !l.children.is_empty()));
    // Only the child's own row moves as it ends, after the thread's turn has: the listing follows it there too.
    let kind = |v: &ThreadView| v.listing().and_then(|l| l.children.first()).map(|c| format!("{:?}", c.kind));
    let working = cx.update(|cx| kind(view.read(cx)));
    until(&mut cx, handle, &view, 30, "the child's end in the listing", |v| kind(v) != working);
}
