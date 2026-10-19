// SPDX-License-Identifier: AGPL-3.0-only
//! The shell's feed and what it drives, through the roads the app takes: the feed dialled at a host started for
//! these tests alone (host-client's tests/common), the Quit question's stop against that host with a stand-in wsp
//! command, and the system notifications and the keep-awake hold on GPUI's test platform.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::desktop::feed::{self, Ask, Fed, FeedState, OnQuit};
use desktop_gpui::desktop::notify::{self, OutsideLine, NEEDS_YOU, OUTSIDE_HELD};
use desktop_gpui::desktop::{awake, quit, tray};
use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{div, IntoElement, Render, SystemNotificationResponse, TestAppContext};
use serde_json::{json, Value};
use wsp_host_client::protocol::PreferencesSetRequest;

const WAIT: Duration = Duration::from_secs(30);

/// The next state the feed hands over that `wanted` holds of, within WAIT; every event it hands on goes to `events`.
fn state_where(
    runtime: &tokio::runtime::Runtime,
    rx: &mut mpsc::UnboundedReceiver<Fed>,
    events: &mut Vec<Value>,
    wanted: impl Fn(&FeedState) -> bool,
) -> FeedState {
    let until = Instant::now() + WAIT;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        let next = runtime.block_on(async { tokio::time::timeout(left, rx.next()).await });
        match next {
            Ok(Some(Fed::State(state))) if wanted(&state) => return *state,
            Ok(Some(Fed::State(_))) => {}
            Ok(Some(Fed::Event(event))) => events.push(event),
            Ok(None) => panic!("the feed ended"),
            Err(_) => panic!("no state the test waits for came within {WAIT:?}"),
        }
    }
}

/// The feed on a real host: the switches and the rows read at once, the rows read again after a turn, a pick the
/// host refuses kept in its words, a switch pushed, and a host that goes read as lost and dialled again.
#[test]
fn the_feed_follows_a_real_host_and_reads_lost_when_it_goes() {
    let host = Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let (tx, mut rx) = mpsc::unbounded();
    let feed = feed::follow(None, host.state(), host.env(), &runtime, feed::SETTLE, Duration::from_millis(300), tx);
    let mut events = Vec::new();

    let first = state_where(&runtime, &mut rx, &mut events, |s| !s.lost);
    assert!(first.sessions.is_empty(), "{:?}", first.sessions);
    assert_eq!(first.prefs, feed::Prefs::default(), "a new host's switches are the defaults");
    assert!(!first.remote, "the host serving the state file here");
    assert!(!first.places.is_empty(), "the computers are read");

    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    // The rows are read again after the turn ends, and the end was handed on before that read.
    let ran =
        state_where(&runtime, &mut rx, &mut events, |s| !s.sessions.is_empty() && s.sessions.iter().all(|r| r["status"] != "running"));
    let model = ran.tray();
    assert!(!model.rows.is_empty());
    let done =
        events.iter().find(|e| e["type"] == "session.done").unwrap_or_else(|| panic!("no session.done was handed on: {events:?}")).clone();
    let reread = ran;
    let line =
        tray::tray_notice(&done, &reread.sessions, &reread.workspaces, &reread.places, &reread.prefs.choices).expect("a finish is said");
    assert_eq!(line.title, "first finished");
    assert!(line.show && !line.sound, "{line:?}");

    feed.ask(Ask::Answer { session_id: "s_none".to_owned(), ask_id: "a9".to_owned(), option_id: "o".to_owned() });
    let refused = state_where(&runtime, &mut rx, &mut events, |s| s.refused.contains_key("a9"));
    assert_eq!(refused.refused["a9"], "this host holds no turn of that thread");

    let client = runtime.block_on(host.client());
    let patch: PreferencesSetRequest = common::ask(json!({ "patch": { "onQuit": "keep", "keepAwake": false, "notifyDone": "off" } }));
    common::answered("preferences.set", runtime.block_on(client.call(patch)));
    let pushed = state_where(&runtime, &mut rx, &mut events, |s| s.prefs.on_quit == OnQuit::Keep);
    assert!(!pushed.prefs.keep_awake);
    assert_eq!(pushed.prefs.choices.notify_done, notify::NotifyChoice::Off);
    drop(client);

    drop(host);
    let lost = state_where(&runtime, &mut rx, &mut events, |s| s.lost);
    assert!(lost.asks.is_empty());
    let model = lost.tray();
    assert_eq!(model.rows[0].label(), tray::NOT_RUNNING);
    assert_eq!((model.badge, model.needs_you, model.title.as_str()), (0, false, ""));
    drop(feed);
}

/// Stand-in `launchctl` and `systemctl` that hold the unit while `held` stands and then refuse to stop it, and hold
/// nothing otherwise.
fn managers(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    let script = r#"#!/bin/sh
case "$1 $2" in
  "--user show"|print\ *) [ -f "HERE/held" ] || exit 113; echo ActiveState=active; echo UnitFileState=enabled; echo MainPID=1 ;;
  "--user stop"|bootout\ *) echo "Failed to stop: Access denied" >&2; exit 1 ;;
esac
"#
    .replace("HERE", &dir.display().to_string());
    for name in ["launchctl", "systemctl"] {
        let (draft, path) = (dir.join(format!("{name}.draft")), dir.join(name));
        std::fs::write(&draft, &script).unwrap();
        // Copied by a child: a write handle of this process's, inherited by another test's fork, makes the exec ETXTBSY.
        assert!(std::process::Command::new("/bin/cp").arg(&draft).arg(&path).env_clear().status().unwrap().success());
        std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
}

/// Quit and stop wsp asks the manager to stop the unit it holds and says its refusal while the host still serves, and
/// ends a host `wsp up` started by the pid its lock records.
#[test]
fn quit_and_stop_says_the_managers_refusal_and_ends_a_host_wsp_up_started() {
    let host = Throwaway::start();
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let stubs = tempfile::tempdir().unwrap();
    managers(stubs.path());
    let mut env = host.env();
    let path = format!("{}:{}", stubs.path().display(), env.get("PATH").cloned().unwrap_or_default());
    env.insert("PATH".to_owned(), path);
    assert_eq!(runtime.block_on(quit::working_here(&host.state(), &env)), 0);
    std::fs::write(stubs.path().join("held"), "").unwrap();
    let refused = runtime.block_on(quit::stop_wsp(&host.state(), &env)).expect_err("the manager refused");
    assert!(refused.ends_with(" exited 1 and said: Failed to stop: Access denied"), "{refused}");
    std::fs::remove_file(stubs.path().join("held")).unwrap();
    runtime.block_on(quit::stop_wsp(&host.state(), &env)).expect("the host wsp up started is ended by its pid");
    assert!(!host.state().with_file_name("host.lock").exists(), "the host let the lock go");
}

struct Blank;

impl Render for Blank {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        div()
    }
}

fn need(body: &str) -> OutsideLine {
    OutsideLine { title: NEEDS_YOU.to_owned(), body: body.to_owned(), show: true, sound: false }
}

/// Opens nothing but counts the opens, and a road whose reopen counts too, taken as a bundled app's on every
/// platform, since a Mac test binary runs from no bundle.
fn road(cx: &mut TestAppContext) -> Rc<Cell<usize>> {
    let reopened = Rc::new(Cell::new(0));
    let counted = reopened.clone();
    cx.update(|cx| {
        notify::init(Rc::new(move |_| counted.set(counted.get() + 1)), cx);
        notify::take_as_bundled(true, cx);
    });
    reopened
}

/// `main.ts:229`: an app the system takes no line from (a Mac app outside a bundle) hands it nothing, says why in the
/// log, and still counts the line as said, so its sound and the Dock's bounce stand in.
#[gpui_kit::test]
fn a_line_from_an_app_outside_a_bundle_is_refused_and_logged(cx: &mut TestAppContext) {
    road(cx);
    cx.update(|cx| notify::take_as_bundled(false, cx));
    assert!(cx.update(|cx| notify::say(&need("outside"), None, cx)));
    assert!(cx.shown_system_notifications().is_empty(), "the system was handed nothing");
    assert_eq!(cx.update(|cx| notify::refused_count(cx)), 1);
    cx.update(|cx| notify::take_as_bundled(true, cx));
    assert!(cx.update(|cx| notify::say(&need("bundled"), None, cx)));
    assert_eq!(cx.shown_system_notifications().len(), 1);
    assert_eq!(cx.update(|cx| notify::refused_count(cx)), 1);
}

/// `needs-you.test.ts:70`, `:79`, `:108`, and the click road: a line said with no window focused reaches the
/// system with its title and body; a click opens a window where none stands and opens that line's thing; a focused
/// window says nothing; a sound-only line shows nothing; past fifty lines the oldest click only raises.
#[gpui_kit::test]
fn a_line_reaches_the_system_only_while_no_window_is_focused(cx: &mut TestAppContext) {
    let reopened = road(cx);
    let opened = Rc::new(Cell::new(0));
    let open = {
        let opened = opened.clone();
        Rc::new(move |_: &mut gpui_kit::App| opened.set(opened.get() + 1)) as notify::Opener
    };
    assert!(cx.update(|cx| notify::say(&need("sign in to GitHub CLI login"), Some(open.clone()), cx)));
    let shown = cx.shown_system_notifications();
    assert_eq!(shown.len(), 1);
    assert_eq!((shown[0].title.as_ref(), shown[0].body.as_ref()), (NEEDS_YOU, "sign in to GitHub CLI login"));
    cx.simulate_system_notification_response(SystemNotificationResponse { tag: shown[0].tag.clone(), action_id: None });
    assert_eq!((reopened.get(), opened.get()), (1, 1), "the click opened a window, then the line's thing");

    let sound_only = OutsideLine { show: false, sound: true, ..need("x") };
    assert!(cx.update(|cx| notify::say(&sound_only, None, cx)), "a sound-only line is said");
    assert_eq!(cx.shown_system_notifications().len(), 1, "and shows nothing");

    let (_, window) = cx.add_window_view(|_, _| Blank);
    window.deactivate_window();
    window.update(|window, _| window.activate_window());
    window.run_until_parked();
    assert!(!window.cx.update(|cx| notify::say(&need("focused"), Some(open.clone()), cx)), "a focused window says nothing");
    assert_eq!(window.shown_system_notifications().len(), 1);
    window.deactivate_window();
    assert!(window.cx.update(|cx| notify::say(&need("blurred"), None, cx)));
    assert_eq!(window.shown_system_notifications().len(), 2);

    for n in 0..OUTSIDE_HELD {
        window.cx.update(|cx| notify::say(&need(&n.to_string()), Some(open.clone()), cx));
    }
    let oldest = window.shown_system_notifications()[1].tag.clone();
    let before = opened.get();
    window.simulate_system_notification_response(SystemNotificationResponse { tag: oldest, action_id: None });
    assert_eq!(opened.get(), before, "a line no longer held raises the window and opens nothing");
    let newest = window.shown_system_notifications().last().unwrap().tag.clone();
    window.simulate_system_notification_response(SystemNotificationResponse { tag: newest, action_id: None });
    assert_eq!(opened.get(), before + 1);
    assert_eq!(reopened.get(), 1, "a window stands, so a click raises it and opens no other");
}

/// The keep-awake hold through GPUI's own: taken while wanted, let go once not.
#[gpui_kit::test]
fn the_keep_awake_hold_follows_what_is_wanted(cx: &mut TestAppContext) {
    cx.update(|cx| awake::hold(true, cx));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1);
    assert!(cx.update(|cx| awake::held(cx)));
    cx.update(|cx| awake::hold(true, cx));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 1, "a second ask holds once");
    cx.update(|cx| awake::hold(false, cx));
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0);
    cx.update(|cx| {
        awake::hold(true, cx);
        awake::hold(false, cx);
    });
    cx.run_until_parked();
    assert_eq!(cx.active_idle_sleep_preventions(), 0, "a hold granted after it was let go is dropped");
    assert!(!cx.update(|cx| awake::held(cx)));
}

/// Run by hand, on GPUI's own Linux platform under a session bus with a stand-in notification server and a display:
/// a line reaches org.freedesktop.Notifications whole, and the click the server sends back opens what it was about.
/// The stand-ins need python3 with dbus-python and PyGObject. From `apps/desktop-gpui`:
/// `dbus-run-session -- sh -c 'python3 -I app/tests/desktop_feed/notifications.py /tmp/notify.log & python3 -I
/// app/tests/desktop_feed/portal.py /tmp/portal.log & sleep 1; xvfb-run -a cargo test -p desktop-gpui --test desktop_feed
/// -- --ignored --skip a_hundred_lines'`; each server writes what it was sent to the file it is given.
#[test]
#[ignore]
fn a_line_reaches_the_session_bus_and_its_click_comes_back() {
    let opened = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = opened.clone();
    gpui_kit::application().run(move |cx| {
        notify::init(Rc::new(|_| {}), cx);
        let line = OutsideLine { title: format!("{} finished", "a".repeat(200)), body: "zmac".to_owned(), show: true, sound: false };
        let open = Rc::new(move |cx: &mut gpui_kit::App| {
            seen.store(true, std::sync::atomic::Ordering::SeqCst);
            cx.quit();
        });
        assert!(notify::say(&line, Some(open), cx));
        cx.spawn(async |cx: &mut gpui_kit::AsyncApp| {
            cx.background_executor().timer(Duration::from_secs(10)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    assert!(opened.load(std::sync::atomic::Ordering::SeqCst), "the click never came back");
}

/// This process's threads by name, and its connections to the session bus.
fn threads_and_connections() -> (std::collections::BTreeMap<String, usize>, usize) {
    let mut threads = std::collections::BTreeMap::new();
    for task in std::fs::read_dir("/proc/self/task").unwrap() {
        let comm = std::fs::read_to_string(task.unwrap().path().join("comm")).unwrap_or_default();
        *threads.entry(comm.trim().to_owned()).or_insert(0) += 1;
    }
    let mut busctl = std::process::Command::new("busctl");
    busctl.env_clear().args(["--user", "list", "--unique", "--no-legend"]);
    for name in ["DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR", "PATH"] {
        if let Some(value) = std::env::var_os(name) {
            busctl.env(name, value);
        }
    }
    let names = busctl.output().unwrap();
    let pid = std::process::id().to_string();
    let connections =
        String::from_utf8_lossy(&names.stdout).lines().filter(|line| line.split_whitespace().nth(1) == Some(pid.as_str())).count();
    (threads, connections)
}

/// Run by hand as the one above, alone since it counts the whole process's threads, with the stand-in server given
/// `--hold` so it never clicks or closes a line, and with `-- --ignored --exact
/// a_hundred_lines_left_in_the_tray_hold_no_thread_or_connection_each`: a hundred lines the server keeps leave the
/// app's threads and its session-bus connections where one line left them.
#[test]
#[ignore]
fn a_hundred_lines_left_in_the_tray_hold_no_thread_or_connection_each() {
    let counts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen = counts.clone();
    gpui_kit::application().run(move |cx| {
        notify::init(Rc::new(|_| {}), cx);
        let line = |n: usize| OutsideLine { title: format!("turn {n} finished"), body: format!("line {n}"), show: true, sound: false };
        assert!(notify::say(&line(0), None, cx));
        cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            seen.lock().unwrap().push(threads_and_connections());
            cx.update(|cx| (1..=100).for_each(|n| assert!(notify::say(&line(n), None, cx))));
            cx.background_executor().timer(Duration::from_secs(3)).await;
            seen.lock().unwrap().push(threads_and_connections());
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    let counts = counts.lock().unwrap();
    let [(one, one_bus), (hundred, hundred_bus)] = &counts[..] else { panic!("the counts were not taken: {counts:?}") };
    eprintln!("after one line: {} threads {one:?}, {one_bus} bus connections", one.values().sum::<usize>());
    eprintln!("after 101 lines: {} threads {hundred:?}, {hundred_bus} bus connections", hundred.values().sum::<usize>());
    assert_eq!(hundred, one, "a hundred more lines changed the app's threads");
    assert_eq!(hundred_bus, one_bus, "a hundred more lines changed the app's bus connections");
}

/// Run by hand as the one above: the keep-awake hold on GPUI's own Linux platform asks the desktop portal to
/// inhibit suspend, which the stand-in portal writes to its file as an Inhibit, then a Close once let go.
#[test]
#[ignore]
fn the_keep_awake_hold_asks_the_desktop_portal() {
    let held = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = held.clone();
    gpui_kit::application().run(move |cx| {
        awake::hold(true, cx);
        cx.spawn(async move |cx: &mut gpui_kit::AsyncApp| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            cx.update(|cx| {
                seen.store(awake::held(cx), std::sync::atomic::Ordering::SeqCst);
                awake::hold(false, cx);
            });
            cx.background_executor().timer(Duration::from_millis(500)).await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    assert!(held.load(std::sync::atomic::Ordering::SeqCst), "the portal never granted the hold");
}
