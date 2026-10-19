// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view against a host started for this test alone (host-client's tests/common): it reads a thread from its
//! newest page, marks it read while its window is active, takes a turn started after it opened as live events, and
//! when the host goes away and comes back it reads the thread again and follows it; a long thread pages back as the
//! reader reaches its top, and a refused read is said once. It runs without libtest's harness for the reason pixels.rs
//! gives.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
mod wire;

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
    a_subagents_page_reads_and_follows_nothing_the_thread_screen_follows();
    println!("test a_subagents_page_reads_and_follows_nothing_the_thread_screen_follows ... ok");
    a_thread_on_this_computer_holds_its_readings_until_it_leaves();
    println!("test a_thread_on_this_computer_holds_its_readings_until_it_leaves ... ok");
}

/// The machine line's readings of this computer's own workspace: asked of the host once the thread shows, kept across
/// a second thread on the same workspace, taken back when the view stops following.
fn a_thread_on_this_computer_holds_its_readings_until_it_leaves() {
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    host.wsp(&["run", "repo", "second", "--agent", "claude"]);
    let (sent, _dir, state) = wire::proxied(&host);
    let client = Arc::new(runtime.block_on(HostClient::connect(&state, &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let (first, second) = (runtime.block_on(item(&client, "first")), runtime.block_on(item(&client, "second")));

    let mut cx = app();
    let sidebar = cx.update(|cx| cx.new(desktop_gpui::sidebar::Sidebar::new));
    let link = desktop_gpui::sidebar::Link { host: client.clone(), runtime: runtime.clone(), page: None };
    cx.update(|cx| sidebar.update(cx, |s, cx| s.attach(link, cx)));
    cx.update(|cx| desktop_gpui::screens::terminal::read_workspaces_from(&sidebar, cx));
    let (view, handle) = window(&mut cx, true);
    let shell = Host::new(client.clone(), runtime.clone());
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&shell, &first, false, cx)));
    until(&mut cx, handle, &view, 30, "the listing", |v| v.listing().is_some_and(|l| l.workspace.is_some()));
    let count = |op: &str| wire::count(&sent, op);
    let limit = Instant::now() + Duration::from_secs(20);
    while count("sys.subscribe") == 0 && Instant::now() < limit {
        settle(&mut cx, handle, 100);
    }
    assert_eq!(count("sys.subscribe"), 1, "the thread on this computer asked for no readings");
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&shell, &second, false, cx)));
    settle(&mut cx, handle, 1500);
    assert_eq!((count("sys.subscribe"), count("sys.unsubscribe")), (1, 0), "a thread on the same workspace asked again");
    let live = desktop_gpui::screens::computer::live::live;
    // A reading near full while the road is down reaches the machine line with no move of the sidebar's.
    let machine = |cx: &mut HeadlessAppContext| cx.update(|cx| view.read(cx).listing().and_then(|l| l.machine.clone()));
    cx.update(|cx| {
        use desktop_gpui::screens::computer::model::{Sample, Usage};
        let gb = 1024. * 1024. * 1024.;
        let full =
            Sample { cpu: 1., load1: 2., mem: Usage { used: 3.8 * gb, total: 4. * gb }, disk: Usage { used: 1., total: 2. }, at: 1. };
        live(cx).update(cx, |l, cx| {
            l.keep(&second.workspace_id, |r| {
                r.ring.push(full);
                r.live = false;
            });
            cx.notify();
        })
    });
    settle(&mut cx, handle, 300);
    let after = machine(&mut cx);
    assert_eq!(after.as_deref(), Some("Out of memory, 3.8 of 4 GB"), "a reading near full did not reach the machine line");
    // A host with no wsp-daemon refuses the ask; one that took it is told to stop.
    let answered = cx.update(|cx| live(cx).read(cx).readings(&first.workspace_id).live);
    cx.update(|cx| view.update(cx, |v, cx| v.stop(cx)));
    let holders = |cx: &mut HeadlessAppContext| cx.update(|cx| live(cx).read(cx).holders(&first.workspace_id));
    let limit = Instant::now() + Duration::from_secs(20);
    while (holders(&mut cx) > 0 || answered && count("sys.unsubscribe") == 0) && Instant::now() < limit {
        settle(&mut cx, handle, 100);
    }
    assert_eq!(holders(&mut cx), 0, "the readings were held after the thread left");
    assert_eq!(count("sys.unsubscribe"), usize::from(answered), "the ask was not taken back once, answered {answered}");
}

/// Runs the app and draws `handle` for `ms`, so anything sent late is sent.
fn settle(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        cx.run_until_parked();
        draw(cx, handle);
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn a_subagents_page_reads_and_follows_nothing_the_thread_screen_follows() {
    use desktop_gpui::screens::thread_view::{subagent::SubagentScreen, ThreadScreen};
    use desktop_gpui::screens::{self, Opened, Screen as _, THREAD};
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let (sent, _dir, state) = wire::proxied(&host);
    let client = Arc::new(runtime.block_on(HostClient::connect(&state, &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    let item = runtime.block_on(item(&client, "first"));
    let shell = Host::new(client, runtime);

    let mut cx = app();
    cx.update(|cx| {
        desktop_gpui::screens::composer::init(cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let opened = Opened { workspace_id: Some(item.workspace_id.clone()), thread_id: Some(item.thread_id.clone()), ..Opened::default() };
    let mut mounted = None;
    let handle: AnyWindowHandle = cx
        .open_window(size(px(1100.), px(800.)), |window, cx| {
            let screen = (screens::screen(THREAD).unwrap().mount)(&shell, opened, window, cx);
            let view = screen.view.clone();
            mounted = Some(screen);
            cx.new(|cx| Root::new(view, window, cx))
        })
        .unwrap()
        .into();
    let thread = mounted.as_ref().unwrap().view.clone().downcast::<ThreadScreen>().ok().unwrap();
    let view = cx.update(|cx| thread.read(cx).view().clone());
    until(&mut cx, handle, &view, 30, "the lead read", |v| person(v, "first") && replies(v) == 1);
    settle(&mut cx, handle, 1500);
    let lead: Vec<String> = cx.update(|cx| view.read(cx).rows().iter().map(|r| r.key.clone()).collect());
    sent.lock().unwrap().clear();

    let arg = format!("{}/{}/toolu-survey", item.workspace_id, item.thread_id);
    let page = Opened { arg: Some(arg.into()), ..Opened::default() };
    let sub =
        cx.update_window(handle, |_, window, cx| (screens::screen(SubagentScreen::NAME).unwrap().mount)(&shell, page, window, cx)).unwrap();
    settle(&mut cx, handle, 1500);
    let page_view = cx.update(|cx| sub.view.clone().downcast::<SubagentScreen>().ok().unwrap().read(cx).view().clone());
    let (history, follows) = (wire::count(&sent, "sessions.history"), wire::count(&sent, "events.subscribe"));
    assert_eq!((history, follows), (0, 0), "the subagent's page read the lead {history} times and followed it {follows} times again");
    assert_eq!(page_view, view, "the page drew a view of its own");

    // A running subagent's bar counts its time once a second.
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
    let running = desktop_gpui::transcript::ListedSubagent {
        call: "toolu-survey".to_owned(),
        title: "Survey".to_owned(),
        state: "running".to_owned(),
        model: Some("haiku".to_owned()),
        asked: None,
        started: now - 5_000.,
        ended: None,
        failure: None,
    };
    let listing = desktop_gpui::transcript::Listing {
        thread: item.thread_id.clone(),
        title: Some("first".to_owned()),
        subagents: vec![running],
        ..Default::default()
    };
    cx.update(|cx| view.update(cx, |v, cx| v.set_listing(listing, cx)));
    let screen = sub.view.clone().downcast::<SubagentScreen>().ok().unwrap();
    let shown: AnyWindowHandle =
        cx.open_window(size(px(1100.), px(800.)), |window, cx| cx.new(|cx| Root::new(sub.view.clone(), window, cx))).unwrap().into();
    settle(&mut cx, shown, 300);
    let clock = cx.update(|cx| screen.read(cx).ticking()).expect("the running subagent's bar counts on no clock");
    let ticks = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = ticks.clone();
    cx.update(|cx| cx.observe(&clock, move |_, _| counted.set(counted.get() + 1)).detach());
    for _ in 0..3 {
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        draw(&mut cx, shown);
    }
    assert!(ticks.get() >= 3, "the bar's time moved {} times in 3 s", ticks.get());
    cx.update_window(shown, |_, window, _| window.remove_window()).unwrap();
    drop((sub, screen, clock));
    settle(&mut cx, handle, 300);
    let back: Vec<String> = cx.update(|cx| view.read(cx).rows().iter().map(|r| r.key.clone()).collect());
    assert_eq!(back, lead, "leaving the page left the lead's rows cut to it");
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
    // A host that checkpoints each turn adds its checkpoint to the turn's 20 events.
    let said = whole.iter().filter(|e| !matches!(e, SessionEvent::SessionCheckpoint { .. })).count();
    assert_eq!((said, starts(&whole)), (240, 12), "twelve turns of 20 events");
    let newest = starts(&whole[whole.len() - 200..]);
    assert!(newest < 12, "the newest page holds the last {newest} turns, not all twelve");

    let mut cx = app();
    let (view, handle) = window(&mut cx, true);
    cx.update(|cx| view.update(cx, |v, cx| v.follow(&Host::new(client.clone(), runtime.clone()), &item, false, cx)));
    until(&mut cx, handle, &view, 30, "the newest page", |v| v.loaded() && person(v, "turn 11"));
    for _ in 0..10 {
        draw(&mut cx, handle);
        cx.run_until_parked();
    }
    let (ticks, complete) = cx.update(|cx| (people(view.read(cx)), view.read(cx).complete()));
    assert_eq!((ticks, complete), (newest, false), "opened at its end, the view holds the newest page alone");

    cx.update(|cx| view.update(cx, |v, cx| v.scroll_to_top(cx)));
    let top = |v: &ThreadView| {
        let row = &v.rows()[v.list().logical_scroll_top().item_ix];
        (row.key.clone(), row.text.clone(), row.turn.clone())
    };
    draw(&mut cx, handle);
    let read = cx.update(|cx| top(view.read(cx)));
    until(&mut cx, handle, &view, 30, "the older page", |v| v.complete());
    assert_eq!(cx.update(|cx| people(view.read(cx))), 12, "the page before it brings the rail its two ticks");
    assert!(cx.update(|cx| person(view.read(cx), "turn 0")), "the thread's first message is drawn");
    // A reply the newest page held only the end of is whole once the page lands, under the key of its first event.
    let now = cx.update(|cx| top(view.read(cx)));
    let same = now.0 == read.0 || (now.2 == read.2 && now.1.ends_with(&read.1));
    assert!(same, "the reader stays on the row they read as the page lands above it: {read:?}, now {now:?}");

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
    // The view reads its listing off the window's sidebar, as the app builds one.
    let sidebar = cx.update(|cx| cx.new(desktop_gpui::sidebar::Sidebar::new));
    let link = desktop_gpui::sidebar::Link { host: client.clone(), runtime: runtime.clone(), page: None };
    cx.update(|cx| sidebar.update(cx, |s, cx| s.attach(link, cx)));
    cx.update(|cx| desktop_gpui::screens::terminal::read_workspaces_from(&sidebar, cx));
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
