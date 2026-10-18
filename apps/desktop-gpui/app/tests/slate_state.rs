// SPDX-License-Identifier: AGPL-3.0-only
//! What the slate keeps and asks around its pieces, against hosts started for this binary alone: one ask and one read in
//! flight with one more after, at most ten asks a second, hidden pieces asking nothing, long formats, the pane step,
//! the sources outside the slate asked again as they move, what a thread's slate keeps past a thread switch, the tab
//! opened once for a slate never shown, and the holds taken again when the socket comes back.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{root, shared, Throwaway};
use desktop_gpui::screens::slate::{self, words, Outcome, Raise, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Pane, Route, Screen};
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, point, prelude::*, px, size, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};
use wsp_host_client::{protocol, Env, HostClient};

/// The binary's shared host, and the folder of the project `bench` the fixture stages on.
fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn host() -> &'static Throwaway {
    bench().0
}

fn thread_with(slate: &str) -> String {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "test", file: None });
    fixture::write(host, &thread, slate);
    thread
}

fn globals(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
}

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap())
}

fn client(state: &Path, env: &Env) -> Host {
    let runtime = runtime();
    let client = Arc::new(runtime.block_on(HostClient::connect(state, env)).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

fn opened(thread: &str) -> Opened {
    Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None }
}

/// The panel on `thread` in a window of its own, inside gpui-kit's Root, drawn while `Holder` holds it.
fn mount<'a>(cx: &'a mut TestAppContext, handle: &Host, thread: &str) -> (Entity<Holder>, &'a mut VisualTestContext) {
    let (handle, opened) = (handle.clone(), opened(thread));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle, opened, window, cx));
        let holder = cx.new(|_| Holder { screen: Some(screen) });
        made = Some(holder.clone());
        Root::new(holder, window, cx)
    });
    (made.unwrap(), cx)
}

/// What draws the panel: a thread switch drops the screen as `RightPanel` drops its tab's.
struct Holder {
    screen: Option<Entity<SlateScreen>>,
}

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.screen.clone())
    }
}

fn screen_of(holder: &Entity<Holder>, cx: &mut VisualTestContext) -> Entity<SlateScreen> {
    cx.read(|cx| holder.read(cx).screen.clone().expect("a panel"))
}

/// Draws the window each round, as one on screen does, whose watch the test's clock never fires.
fn until(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, what: &str, done: impl Fn(&SlateScreen) -> bool) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        if cx.read(|cx| done(screen.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the panel never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn settled(s: &SlateScreen) -> bool {
    s.slate().is_some_and(|slate| slate.formulas().iter().all(|f| slate.resolved.contains_key(f)))
}

fn prop(s: &SlateScreen, piece: &str, name: &str) -> Option<Value> {
    let slate = s.slate()?;
    slate.props(slate.doc()?.piece(piece)?).get(name).cloned()
}

/// Waits out the tenth of a second the panel leaves between two asks.
fn spaced(cx: &mut VisualTestContext) {
    std::thread::sleep(Duration::from_millis(150));
    cx.run_until_parked();
}

/// What the host holds at `$path`, read with `slates.read`.
fn on_host(handle: &Host, thread: &str, path: &str) -> Option<Value> {
    let (thread, path) = (thread.to_owned(), path.to_owned());
    let read = futures::executor::block_on(handle.run(move |host| async move {
        let ask: protocol::SlatesReadRequest =
            serde_json::from_value(json!({ "threadId": thread, "sketch": false, "text": false })).unwrap();
        host.call(ask).await
    }))
    .unwrap_or_else(|f| panic!("slates.read: {}", f.message));
    read.state.get(&path).cloned()
}

fn slates_get(handle: &Host, thread: &str) -> Value {
    let thread = thread.to_owned();
    let got = futures::executor::block_on(handle.run(move |host| async move {
        let ask: protocol::SlatesGetRequest = serde_json::from_value(json!({ "threadId": thread })).unwrap();
        host.call(ask).await
    }))
    .unwrap_or_else(|f| panic!("slates.get: {}", f.message));
    serde_json::to_value(&got.slate).unwrap()
}

const COUNTER: &str = r#"<slate><value name="n" start={1} /><column><text id="n" value={$n + 1} /></column></slate>"#;

#[gpui_kit::test]
fn an_ask_while_one_is_out_waits_for_it_and_goes_once_after(cx: &mut TestAppContext) {
    let own = Own::new();
    let thread = own.thread_with(COUNTER);
    globals(cx);
    let handle = client(&own.state(), &own.env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    spaced(cx);
    screen.update(cx, |s, _| s.timings.clear());
    // A host slower than the tenth of a second between asks: stopped while the asks go.
    own.pause(true);
    for _ in 0..3 {
        screen.update(cx, |s, cx| s.resolve(cx));
        spaced(cx);
    }
    own.pause(false);
    until(cx, &screen, "the ask out and the one after it", |s| s.timings.len() >= 2);
    spaced(cx);
    spaced(cx);
    assert_eq!(cx.read(|cx| screen.read(cx).timings.len()), 2, "three asks while one is out are that one and one after it");
}

#[gpui_kit::test]
fn a_read_while_one_is_out_waits_for_it_and_goes_once_after(cx: &mut TestAppContext) {
    let thread = thread_with(COUNTER);
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    let before = cx.read(|cx| screen.read(cx).gets);
    screen.update(cx, |s, cx| {
        for _ in 0..3 {
            s.reload(cx);
        }
    });
    until(cx, &screen, "the read and the one after it", |s| s.gets >= before + 2);
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| screen.read(cx).gets), before + 2, "three reads at once are one out and one after it");
}

#[gpui_kit::test]
fn asks_go_at_most_ten_times_a_second(cx: &mut TestAppContext) {
    let thread = thread_with(COUNTER);
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    spaced(cx);
    screen.update(cx, |s, _| s.timings.clear());
    let end = Instant::now() + Duration::from_secs(1);
    while Instant::now() < end {
        screen.update(cx, |s, cx| s.resolve(cx));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(2));
    }
    let asked = cx.read(|cx| screen.read(cx).timings.len());
    assert!((5..=11).contains(&asked), "a second of pushes asks about ten times, not on every push: {asked}");
}

#[gpui_kit::test]
fn a_hidden_piece_reading_the_clock_does_not_tick(cx: &mut TestAppContext) {
    let thread = thread_with(
        r#"<slate><value name="show" start={false} /><value name="n" start={1} /><column>
  <text id="n" value={$n + 1} />
  <text id="now" when={$show} value={time.now} />
</column></slate>"#,
    );
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    assert!(!cx.read(|cx| screen.read(cx).slate().unwrap().formulas().contains(&"time.now".to_owned())));
    spaced(cx);
    screen.update(cx, |s, _| s.timings.clear());
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(300));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    assert_eq!(cx.read(|cx| screen.read(cx).timings.len()), 0, "a hidden clock read keeps nothing asking");
    host().wsp(&["slate", "state", &thread, "$show=true"]);
    until(cx, &screen, "the shown clock answered", |s| prop(s, "now", "value").is_some_and(|v| v.is_number()));
}

#[gpui_kit::test]
fn a_format_past_the_hosts_cap_draws_its_words_and_holes(cx: &mut TestAppContext) {
    let words = "word ".repeat(110);
    let thread = thread_with(&format!(
        r#"<slate><value name="n" start={{5}} /><value name="m" start="x" /><column><text id="long">{words}{{$n}} and {{$m}}.</text></column></slate>"#
    ));
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the long text answered", settled);
    assert_eq!(cx.read(|cx| prop(screen.read(cx), "long", "value")), Some(json!(format!("{words}5 and x."))));
}

#[gpui_kit::test]
fn a_pane_step_refuses_a_pane_this_build_does_not_have(cx: &mut TestAppContext) {
    let thread = thread_with(
        r#"<slate><column>
  <button id="term" label="Terminal" onPress={pane("terminal")} />
  <button id="slate" label="Slate" onPress={pane("slate")} />
</column></slate>"#,
    );
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the buttons", settled);
    for piece in ["term", "slate"] {
        screen.update(cx, |s, cx| s.raise(piece.into(), "press", Raise::default(), cx));
    }
    until(cx, &screen, "both answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    let outcome = |p: &str| cx.read(|cx| screen.read(cx).slate().unwrap().outcomes.get(p).cloned());
    assert_eq!(outcome("term"), Some(Outcome::Refused(words::NO_PANE.into())));
    assert_eq!(outcome("slate"), None, "the slate's own pane opens here");
}

#[gpui_kit::test]
fn folds_picks_and_typed_words_outlive_the_panel_a_thread_switch_drops(cx: &mut TestAppContext) {
    let thread = thread_with(
        r#"<slate><value name="note" start="" /><column>
  <section id="sec" title="S" collapsible={true}><text id="t" value={$note} /></section>
  <input id="note" label="Note" value={$note} />
</column></slate>"#,
    );
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    screen.update(cx, |s, cx| {
        s.fold("sec".into(), false, cx);
        s.pick("first".into(), "second".into(), cx);
        if let slate::Load::Drawn(slate) = &mut s.load {
            slate.queue("$note".into(), json!("typed, not sent"));
        }
    });
    holder.update(cx, |h, cx| {
        h.screen = None;
        cx.notify();
    });
    drop(screen);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let limit = Instant::now() + Duration::from_secs(20);
    while on_host(&handle, &thread, "$note") != Some(json!("typed, not sent")) {
        assert!(Instant::now() < limit, "the typed words reached the host as the panel went");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(cx.read(slate::kept_threads), std::slice::from_ref(&thread));
    let again = cx.update(|window, cx| cx.new(|cx| SlateScreen::new(handle.clone(), opened(&thread), window, cx)));
    holder.update(cx, |h, cx| {
        h.screen = Some(again.clone());
        cx.notify();
    });
    until(cx, &again, "the slate drawn again", settled);
    let (folds, picks) = cx.read(|cx| {
        let slate = again.read(cx).slate().unwrap();
        (slate.folds.clone(), slate.picks.clone())
    });
    assert_eq!(folds.get("sec"), Some(&false), "the fold is kept");
    assert_eq!(picks.get("first").map(String::as_str), Some("second"), "the pick is kept");
    assert_eq!(cx.read(|cx| again.read(cx).slate().unwrap().value("$note").cloned()), Some(json!("typed, not sent")));
    assert!(cx.read(slate::kept_threads).is_empty(), "taken back, not kept twice");
}

#[gpui_kit::test]
fn a_fold_opened_asks_its_body_with_nothing_else_moving(cx: &mut TestAppContext) {
    let thread = thread_with(
        r#"<slate><value name="n" start={1} /><column>
  <section id="sec" title="S" collapsible={true} open={false}><text id="t" value={$n + 1} /></section>
</column></slate>"#,
    );
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    assert_eq!(cx.read(|cx| prop(screen.read(cx), "t", "value")), None, "a shut body asks nothing");
    spaced(cx);
    screen.update(cx, |s, cx| s.fold("sec".into(), true, cx));
    until(cx, &screen, "the opened body's value", |s| prop(s, "t", "value") == Some(json!(2)));
}

#[gpui_kit::test]
fn a_push_asks_again_only_while_the_slate_is_drawn(cx: &mut TestAppContext) {
    let thread = thread_with(r#"<slate><column><text id="t" value={thread.lastTurn.endedAt} /></column></slate>"#);
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    let before = cx.read(|cx| prop(screen.read(cx), "t", "value"));
    holder.update(cx, |h, cx| {
        h.screen = None;
        cx.notify();
    });
    std::thread::sleep(Duration::from_millis(2200));
    cx.run_until_parked();
    let asked = cx.read(|cx| screen.read(cx).timings.len());
    host().wsp(&["send", &thread, "again"]);
    let limit = Instant::now() + Duration::from_secs(4);
    while Instant::now() < limit {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(cx.read(|cx| screen.read(cx).timings.len()), asked, "a tab behind another asks nothing on the thread's pushes");
    holder.update(cx, |h, cx| {
        h.screen = Some(screen.clone());
        cx.notify();
    });
    until(cx, &screen, "the turn's end drawn once the slate shows again", |s| prop(s, "t", "value") != before);
}

/// Drops the panel as a thread switch does, then lets its release run.
fn switch_away(holder: &Entity<Holder>, screen: Entity<SlateScreen>, cx: &mut VisualTestContext) {
    holder.update(cx, |h, cx| {
        h.screen = None;
        cx.notify();
    });
    drop(screen);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// A new panel on `thread` in the holder, as a switch back draws one, once it has answered.
fn switch_back(holder: &Entity<Holder>, handle: &Host, thread: &str, cx: &mut VisualTestContext) -> Entity<SlateScreen> {
    let again = cx.update(|window, cx| cx.new(|cx| SlateScreen::new(handle.clone(), opened(thread), window, cx)));
    holder.update(cx, |h, cx| {
        h.screen = Some(again.clone());
        cx.notify();
    });
    until(cx, &again, "the slate drawn again", settled);
    again
}

fn write_on_host(handle: &Host, thread: &str, path: &str, value: Value) {
    let params = json!({ "threadId": thread, "values": { path: value } });
    futures::executor::block_on(handle.run(move |host| async move {
        let ask: protocol::SlatesStateRequest = serde_json::from_value(params).unwrap();
        host.call(ask).await
    }))
    .unwrap_or_else(|f| panic!("slates.state: {}", f.message));
}

const NOTE: &str = r#"<slate><value name="note" start="" /><column><input id="note" label="Note" value={$note} /></column></slate>"#;

#[gpui_kit::test]
fn a_value_kept_drawn_while_the_person_chooses_does_not_go_as_the_panel_goes(cx: &mut TestAppContext) {
    let thread = thread_with(NOTE);
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    write_on_host(&handle, &thread, "$note", json!("Bob"));
    until(cx, &screen, "the agent's value drawn", |s| s.slate().and_then(|s| s.value("$note")) == Some(&json!("Bob")));
    screen.update(cx, |s, cx| s.keep_drawn("$note".into(), json!("Ada"), cx));
    switch_away(&holder, screen, cx);
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(on_host(&handle, &thread, "$note"), Some(json!("Bob")), "neither Keep mine nor Take theirs was pressed");
    let again = switch_back(&holder, &handle, &thread, cx);
    assert_eq!(cx.read(|cx| again.read(cx).slate().unwrap().value("$note").cloned()), Some(json!("Bob")));
}

#[gpui_kit::test]
fn a_kept_value_gives_way_to_a_host_value_written_since(cx: &mut TestAppContext) {
    let thread = thread_with(NOTE);
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    // Kept as the release finds it: typed inside the field's wait, its send not landed by the time the agent writes.
    screen.update(cx, |s, _| {
        if let slate::Load::Drawn(slate) = &mut s.load {
            slate.queue("$note".into(), json!("Ada"));
        }
    });
    switch_away(&holder, screen, cx);
    let limit = Instant::now() + Duration::from_secs(20);
    while on_host(&handle, &thread, "$note") != Some(json!("Ada")) {
        assert!(Instant::now() < limit, "the typed words reached the host as the panel went");
        std::thread::sleep(Duration::from_millis(50));
    }
    write_on_host(&handle, &thread, "$note", json!("Bob"));
    let again = switch_back(&holder, &handle, &thread, cx);
    assert_eq!(cx.read(|cx| again.read(cx).slate().unwrap().value("$note").cloned()), Some(json!("Bob")), "the host's newer value draws");
    switch_away(&holder, again, cx);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(on_host(&handle, &thread, "$note"), Some(json!("Bob")), "the next switch sends nothing over it");
}

/// A shell in a 1280 by 800 window on `host`.
fn shell<'a>(cx: &'a mut TestAppContext, host: &Throwaway) -> (Entity<Shell>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, env, runtime) = (host.state(), host.env(), runtime());
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
        made = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (made.unwrap(), cx)
}

fn until_shell(cx: &mut VisualTestContext, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let limit = Instant::now() + Duration::from_secs(30);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < limit, "the window never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn workspace_of(handle: &Host, thread: &str) -> String {
    slates_get(handle, thread)["workspaceId"].as_str().expect("the slate's workspace").to_owned()
}

fn go(shell: &Entity<Shell>, workspace: &str, thread: &str, cx: &mut VisualTestContext) {
    let route = Route::Thread { workspace_id: workspace.to_owned(), thread_id: thread.to_owned() };
    shell.update_in(cx, |s, window, cx| s.go(route, window, cx));
    cx.run_until_parked();
}

fn drawn_in_tab(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.debug_bounds("slate-surface").is_some()
}

fn active(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> Option<Pane> {
    cx.read(|cx| shell.read(cx).panel().read(cx).active().filter(|_| shell.read(cx).panel().read(cx).is_open()))
}

/// Pushes every thread out of the window's own record of first showings, so the host's `shownOnce` alone keeps a
/// second showing shut, as it does across windows and restarts.
fn forget_shown_here(cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        for i in 0..slate::KEPT_THREADS {
            assert!(slate::first_showing(&format!("elsewhere-{i}"), cx));
        }
    });
}

/// Watches the panel for three seconds, past the host's answer to the panel's read, for a Slate tab that must not open.
fn never_slate(shell: &Entity<Shell>, cx: &mut VisualTestContext, what: &str) {
    let limit = Instant::now() + Duration::from_secs(3);
    while Instant::now() < limit {
        cx.run_until_parked();
        assert_ne!(active(shell, cx), Some(Pane::Slate), "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[gpui_kit::test]
fn a_slate_never_shown_opens_its_tab_once_as_its_thread_is_picked(cx: &mut TestAppContext) {
    let shown = thread_with(COUNTER);
    let other = thread_with(COUNTER);
    let handle = client(&host().state(), &host().env());
    let (workspace, elsewhere) = (workspace_of(&handle, &shown), workspace_of(&handle, &other));
    assert_eq!(slates_get(&handle, &shown)["shownOnce"], json!(false));
    let (shell, cx) = shell(cx, host());
    until_shell(cx, "the shell has its host", |cx| cx.read(|cx| shell.read(cx).connected()));
    go(&shell, &workspace, &shown, cx);
    until_shell(cx, "the Slate tab opened on its own", |cx| active(&shell, cx) == Some(Pane::Slate));
    until_shell(cx, "the slate drawn in its tab", drawn_in_tab);
    let limit = Instant::now() + Duration::from_secs(20);
    while slates_get(&handle, &shown)["shownOnce"] != json!(true) {
        assert!(Instant::now() < limit, "the host was told the slate was shown");
        std::thread::sleep(Duration::from_millis(50));
    }
    let panel = cx.read(|cx| shell.read(cx).panel().clone());
    panel.update_in(cx, |p, window, cx| p.close(0, window, cx));
    forget_shown_here(cx);
    go(&shell, &elsewhere, &other, cx);
    go(&shell, &workspace, &shown, cx);
    never_slate(&shell, cx, "a slate shown once does not open its tab again");
    panel.update_in(cx, |p, window, cx| p.add(Pane::Agents, None, window, cx));
    cx.update(|_, cx| assert!(slate::open_pane(Pane::Slate, cx), "this build mounts the Slate pane"));
    until_shell(cx, "the pane step's road opened the Slate tab", |cx| active(&shell, cx) == Some(Pane::Slate));
    cx.update(|_, cx| assert!(!slate::open_pane(Pane::Terminal, cx), "and no Terminal"));
}

/// Draws a frame after moving the test's clock past a fold's 200 ms.
fn folded_out(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_thread_switch_in_the_shell_keeps_the_fold_and_sends_the_words_typed(cx: &mut TestAppContext) {
    let here = thread_with(
        r#"<slate><value name="note" start="" /><column>
  <section id="sec" title="S" collapsible={true}><text id="t" value="body" /></section>
  <input id="note" label="Note" value={$note} />
</column></slate>"#,
    );
    let other = thread_with(COUNTER);
    let handle = client(&host().state(), &host().env());
    let (workspace, elsewhere) = (workspace_of(&handle, &here), workspace_of(&handle, &other));
    let (shell, cx) = shell(cx, host());
    until_shell(cx, "the shell has its host", |cx| cx.read(|cx| shell.read(cx).connected()));
    go(&shell, &workspace, &here, cx);
    until_shell(cx, "the Slate tab opened", |cx| active(&shell, cx) == Some(Pane::Slate));
    until_shell(cx, "the section drawn open", |cx| drawn_in_tab(cx) && cx.debug_bounds("slate-section-panel").is_some());
    let section = cx.debug_bounds("slate-section").expect("the section");
    cx.simulate_click(section.origin + point(px(24.), px(14.)), Modifiers::none());
    folded_out(cx);
    assert!(cx.debug_bounds("slate-section-panel").is_none(), "its head folds it");
    let field = cx.debug_bounds("slate-field").expect("the field");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("typed");
    go(&shell, &elsewhere, &other, cx);
    assert_eq!(cx.read(slate::kept_threads), std::slice::from_ref(&here), "the thread's slate is kept as the shell drops its panel");
    let limit = Instant::now() + Duration::from_secs(20);
    while on_host(&handle, &here, "$note") != Some(json!("typed")) {
        assert!(Instant::now() < limit, "the words typed inside the field's wait reached the host as the shell switched");
        std::thread::sleep(Duration::from_millis(50));
    }
    go(&shell, &workspace, &here, cx);
    until_shell(cx, "the Slate tab back on its thread", |cx| active(&shell, cx) == Some(Pane::Slate) && drawn_in_tab(cx));
    until_shell(cx, "the section drawn", |cx| cx.debug_bounds("slate-section").is_some());
    folded_out(cx);
    assert!(cx.debug_bounds("slate-section-panel").is_none(), "the fold came back with its thread");
    assert!(cx.read(slate::kept_threads).is_empty(), "taken back, not kept twice");
}

/// The stand-in claude, writing the slate in `file` as the thread's agent on a prompt that asks for one.
fn writing_stand_in(file: &Path) -> String {
    let bin = root().join("packages/host/dist/bin.js");
    let line = "while IFS= read -r _; do\n";
    assert!(fixture::stand_in().contains(line));
    fixture::stand_in().replace(
        line,
        &format!(
            "while IFS= read -r line; do\n  case \"$line\" in *slate-please*) node '{}' slate write \"$WSP_THREAD\" '{}' >/dev/null 2>&1 ;; esac\n",
            bin.display(),
            file.display()
        ),
    )
}

#[gpui_kit::test]
fn an_agents_first_write_opens_the_tab_once(cx: &mut TestAppContext) {
    let scratch = tempfile::tempdir().unwrap();
    let file = scratch.path().join("agent.slate");
    std::fs::write(&file, COUNTER).unwrap();
    let host = Throwaway::start_with(&writing_stand_in(&file), false);
    let folder = fixture::project(&host);
    let thread = fixture::staged(&host, &folder, &fixture::State { name: "agent", file: None });
    let handle = client(&host.state(), &host.env());
    let threads = futures::executor::block_on(handle.run(|h| async move { h.threads().await })).unwrap();
    let listed = threads.iter().find(|t| t.thread_id.as_deref().unwrap_or(&t.id) == thread).expect("the thread listed");
    let workspace = listed.workspace_id.clone();
    let (shell, cx) = shell(cx, &host);
    until_shell(cx, "the shell has its host", |cx| cx.read(|cx| shell.read(cx).connected()));
    go(&shell, &workspace, &thread, cx);
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_ne!(active(&shell, cx), Some(Pane::Slate), "no slate yet, no tab");
    host.wsp(&["send", &thread, "slate-please"]);
    until_shell(cx, "the agent's write opened the Slate tab", |cx| active(&shell, cx) == Some(Pane::Slate));
    until_shell(cx, "the slate drawn in its tab", drawn_in_tab);
    let panel = cx.read(|cx| shell.read(cx).panel().clone());
    let limit = Instant::now() + Duration::from_secs(20);
    while slates_get(&handle, &thread)["shownOnce"] != json!(true) {
        assert!(Instant::now() < limit, "the host was told the slate was shown");
        std::thread::sleep(Duration::from_millis(50));
    }
    panel.update_in(cx, |p, window, cx| p.close(0, window, cx));
    forget_shown_here(cx);
    host.wsp(&["send", &thread, "slate-please"]);
    never_slate(&shell, cx, "the agent's second write does not open it again");
}

/// A host of this test's own, on a state it can be stopped and started again on.
struct Own {
    dir: tempfile::TempDir,
    child: Option<Child>,
}

impl Own {
    fn state(&self) -> PathBuf {
        self.dir.path().join("state/state.json")
    }

    fn env(&self) -> Env {
        common::env_in(self.dir.path())
    }

    fn node(&self) -> Command {
        let mut command = Command::new("node");
        command.arg(root().join("packages/host/dist/bin.js")).env_clear().envs(self.env()).current_dir(self.dir.path());
        command
    }

    fn new() -> Own {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::create_dir_all(dir.path().join("state")).unwrap();
        let claude = dir.path().join("bin/claude");
        std::fs::write(&claude, fixture::stand_in()).unwrap();
        std::fs::set_permissions(&claude, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let mut own = Own { dir, child: None };
        own.up();
        own
    }

    fn up(&mut self) {
        let lock = self.state().with_file_name("host.lock");
        let _ = std::fs::remove_file(&lock);
        let log = std::fs::File::create(self.dir.path().join("up.log")).unwrap();
        let state = self.state().to_string_lossy().into_owned();
        let child = self
            .node()
            .args(["up", "--port", "0", "--no-relay", "--state", &state])
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let pid = child.id() as u64;
        self.child = Some(child);
        let limit = Instant::now() + Duration::from_secs(30);
        let listening = || {
            let lock = std::fs::read_to_string(&lock).unwrap_or_default();
            serde_json::from_str::<Value>(&lock)
                .ok()
                .is_some_and(|l| l["port"].as_u64().is_some_and(|p| p != 0) && l["pid"].as_u64() == Some(pid))
        };
        while !listening() {
            assert!(Instant::now() < limit, "no host came up");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Stops the host this test started, by the pid of its own child.
    fn down(&mut self) {
        let Some(mut child) = self.child.take() else { return };
        // SAFETY: a signal to the pid of the child this test spawned and has not yet reaped.
        unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
        let limit = Instant::now() + Duration::from_secs(20);
        while matches!(child.try_wait(), Ok(None)) && Instant::now() < limit {
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Stops or continues the host this test started, by the pid of its own child.
    fn pause(&self, stop: bool) {
        let child = self.child.as_ref().expect("a host up");
        // SAFETY: a signal to the pid of the child this test spawned and has not yet reaped.
        unsafe { libc::kill(child.id() as i32, if stop { libc::SIGSTOP } else { libc::SIGCONT }) };
    }

    fn wsp(&self, args: &[&str]) -> String {
        let state = self.state().to_string_lossy().into_owned();
        let out = self.node().args(args).args(["--state", &state]).stdin(Stdio::null()).output().unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        assert!(out.status.success(), "wsp {args:?}: {said}");
        said
    }

    /// A thread on a fresh project with `slate` written on it.
    fn thread_with(&self, slate: &str) -> String {
        let folder = self.dir.path().join("proj");
        std::fs::create_dir_all(&folder).unwrap();
        let git = |args: &[&str]| {
            assert!(Command::new("git").args(args).current_dir(&folder).env_clear().envs(self.env()).status().unwrap().success())
        };
        git(&["init", "-q"]);
        git(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "init"]);
        self.wsp(&["add", &folder.to_string_lossy(), "--name", "proj"]);
        let said = self.wsp(&["run", "proj", "Hello.", "--agent", "claude"]);
        let thread = said
            .split_whitespace()
            .skip_while(|w| *w != "thread")
            .nth(1)
            .map(|w| w.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != '-').to_owned())
            .unwrap_or_else(|| panic!("wsp run named no thread: {said}"));
        let file = self.dir.path().join("own.slate");
        std::fs::write(&file, slate).unwrap();
        self.wsp(&["slate", "write", &thread, &file.to_string_lossy()]);
        thread
    }
}

impl Drop for Own {
    fn drop(&mut self) {
        self.down();
    }
}

#[gpui_kit::test]
fn a_socket_that_comes_back_takes_its_holds_again_and_offline_acts_say_so(cx: &mut TestAppContext) {
    let mut own = Own::new();
    let thread = own.thread_with(
        r#"<slate><value name="n" start={1} /><column><text id="n" value={$n + 1} /><button id="go" label="Go" onPress={set($n, $n + 1)} /></column></slate>"#,
    );
    globals(cx);
    let handle = client(&own.state(), &own.env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the slate answered", settled);
    until(cx, &screen, "the shown hold taken", |s| s.holding());
    own.down();
    until(cx, &screen, "the socket's loss seen", |s| !s.live() && s.holds().is_empty());
    screen.update(cx, |s, cx| s.raise("go".into(), "press", Raise::default(), cx));
    until(cx, &screen, "the press answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    let outcome = cx.read(|cx| screen.read(cx).slate().unwrap().outcomes.get("go").cloned());
    assert_eq!(outcome, Some(Outcome::Refused(words::OFFLINE.into())), "offline, never the host's socket or state path");
    own.up();
    until(cx, &screen, "the socket back and the hold taken again", |s| s.live() && s.holding());
    screen.update(cx, |s, cx| s.raise("go".into(), "press", Raise::default(), cx));
    until(cx, &screen, "the press drawn", |s| prop(s, "n", "value") == Some(json!(3)));
}

#[gpui_kit::test]
fn the_thread_and_its_checkout_are_asked_again_as_they_move(cx: &mut TestAppContext) {
    let thread = thread_with(
        r#"<slate><column><text id="branch" value={git.branch} /><text id="ended" value={thread.lastTurn.endedAt} /></column></slate>"#,
    );
    globals(cx);
    let handle = client(&host().state(), &host().env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "the sources answered", settled);
    until(cx, &screen, "the git hold taken with the shown hold", |s| s.holds() == ["git", "slate"]);
    let ended = cx.read(|cx| prop(screen.read(cx), "ended", "value")).filter(Value::is_number).expect("thread.lastTurn.endedAt answered");
    host().wsp(&["send", &thread, "again"]);
    until(cx, &screen, "the thread asked again after its turn", |s| prop(s, "ended", "value").is_some_and(|v| v.is_number() && v != ended));
}

#[gpui_kit::test]
fn a_checkout_that_switches_branch_draws_the_new_branch(cx: &mut TestAppContext) {
    let host = Throwaway::start_with(&fixture::stand_in(), true);
    if host.daemon.is_none() {
        println!("skipped the branch switch, whose checkout the host reads through a wsp-daemon: {}", common::wsp_daemon().unwrap_err());
        return;
    }
    let folder = fixture::project(&host);
    let thread = fixture::staged(&host, &folder, &fixture::State { name: "branch", file: None });
    fixture::write(&host, &thread, r#"<slate><column><text id="branch" value={git.branch} /></column></slate>"#);
    globals(cx);
    let handle = client(&host.state(), &host.env());
    let (holder, cx) = mount(cx, &handle, &thread);
    let screen = screen_of(&holder, cx);
    until(cx, &screen, "git.branch answered", |s| prop(s, "branch", "value").is_some_and(|v| v.is_string()));
    let at = {
        let thread = thread.clone();
        futures::executor::block_on(handle.run(move |host| async move {
            let ask: protocol::SlatesResolveRequest =
                serde_json::from_value(json!({ "threadId": thread, "paths": ["thread.folder"] })).unwrap();
            host.call(ask).await
        }))
        .unwrap_or_else(|f| panic!("slates.resolve: {}", f.message))
        .values
        .get("thread.folder")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .unwrap_or_else(|| folder.clone())
    };
    let switched =
        Command::new("git").args(["checkout", "-q", "-b", "moved"]).current_dir(&at).env_clear().envs(host.env()).status().unwrap();
    assert!(switched.success(), "git checkout in {}", at.display());
    // The host reads a checkout at a turn's end, on view and after a write, never on a timer of its own.
    host.wsp(&["send", &thread, "again"]);
    until(cx, &screen, "the new branch drawn", |s| prop(s, "branch", "value") == Some(json!("moved")));
}
