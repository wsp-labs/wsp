// SPDX-License-Identifier: AGPL-3.0-only
//! The slate panel against a host started for this binary alone (host-client's tests/common): what it reads and draws,
//! that every formula a drawn piece reads is the host's answer (ruling D2), what it writes and presses, and the pushes
//! that keep it live. `d2_round_trips` (ignored) prints the `slates.resolve` round trips the panel makes.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::model::Record;
use desktop_gpui::screens::slate::pieces::KNOWN;
use desktop_gpui::screens::slate::{words, Load, Outcome, Raise, Slate, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, Context, Entity, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};
use wsp_host_client::{protocol, HostClient};

/// The binary's shared host, stopped as the binary exits, and the folder of the project `bench` the fixture stages on.
fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn host() -> &'static Throwaway {
    bench().0
}

/// A fresh thread on bench, with `slate` written on it where one is given.
fn thread_with(slate: Option<&str>) -> String {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "test", file: None });
    if let Some(text) = slate {
        fixture::write(host, &thread, text);
    }
    thread
}

/// The app's globals and a client of the shared host on a runtime of its own.
fn init(cx: &mut TestAppContext) -> Host {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        Arc::new(runtime.block_on(HostClient::connect(&host().state(), &host().env())).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

/// The panel on `thread` in a window of its own, inside gpui-kit's Root as the app wraps every window.
fn mount<'a>(cx: &'a mut TestAppContext, handle: &Host, thread: &str) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    let opened = Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    (made.unwrap(), cx)
}

fn open<'a>(cx: &'a mut TestAppContext, thread: &str) -> (Entity<SlateScreen>, Host, &'a mut VisualTestContext) {
    let handle = init(cx);
    let (screen, cx) = mount(cx, &handle, thread);
    (screen, handle, cx)
}

/// Runs the app until `done` holds of the panel, the host answering on its own threads meanwhile.
fn until(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, what: &str, done: impl Fn(&SlateScreen) -> bool) {
    let limit = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if cx.read(|cx| done(screen.read(cx))) {
            return;
        }
        assert!(Instant::now() < limit, "the panel never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Drawn, with every formula it reads answered.
fn settled(s: &SlateScreen) -> bool {
    s.slate().is_some_and(|slate| slate.formulas().iter().all(|f| slate.resolved.contains_key(f)))
}

fn drawn(cx: &mut VisualTestContext, selector: String) -> bool {
    cx.run_until_parked();
    cx.debug_bounds(Box::leak(selector.into_boxed_str())).is_some()
}

/// A piece's props as its view gets them.
fn props(s: &SlateScreen, piece: &str) -> serde_json::Map<String, Value> {
    let slate = s.slate().expect("a drawn slate");
    slate.props(slate.doc().unwrap().piece(piece).unwrap_or_else(|| panic!("no piece {piece}")))
}

fn prop(s: &SlateScreen, piece: &str, name: &str) -> Option<Value> {
    props(s, piece).get(name).cloned()
}

fn busy(s: &SlateScreen, piece: &str) -> bool {
    s.slate().is_some_and(|slate| slate.busy.contains(piece))
}

fn outcome(s: &SlateScreen, piece: &str) -> Option<Outcome> {
    s.slate().and_then(|slate| slate.outcomes.get(piece).cloned())
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

#[gpui_kit::test]
fn reads_a_slate_with_every_piece_and_draws_it(cx: &mut TestAppContext) {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::STATES[0]);
    let (screen, handle, cx) = open(cx, &thread);
    until(cx, &screen, "every.slate drawn and answered", settled);
    let stored = Record::read(&slates_get(&handle, &thread)).expect("the host's record");
    let (title, pieces, kinds, version) = cx.read(|cx| {
        let slate = screen.read(cx).slate().unwrap();
        let doc = slate.doc().unwrap();
        let kinds: HashSet<String> = doc.pieces.values().map(|p| p.kind.clone()).collect();
        (doc.title.clone(), doc.pieces.len(), kinds, slate.record.version)
    });
    let doc = stored.doc.as_ref().unwrap();
    assert_eq!(title, doc.title, "the title is the document's");
    let missing: Vec<_> = KNOWN.iter().filter(|k| !kinds.contains(**k)).collect();
    assert!(missing.is_empty(), "every piece type is read; missing {missing:?}");
    assert_eq!(pieces, doc.pieces.len(), "the piece count is the host's");
    assert_eq!(version, stored.version, "the version is the record's");
    assert_eq!(version, 1);
    let root = format!("slate-piece-{}-column", doc.root);
    for selector in ["slate-surface".to_owned(), "slate-header".to_owned(), root] {
        assert!(drawn(cx, selector.clone()), "the panel draws {selector}");
    }
}

#[gpui_kit::test]
fn a_thread_with_no_slate_draws_the_empty_state(cx: &mut TestAppContext) {
    let thread = thread_with(None);
    let (screen, _, cx) = open(cx, &thread);
    until(cx, &screen, "the empty state", |s| matches!(s.load, Load::Empty(_)));
    assert!(drawn(cx, "slate-empty-none".into()), "the empty state is marked none");
    assert!(!drawn(cx, "slate-surface".into()));
}

#[gpui_kit::test]
fn a_newer_schema_draws_its_own_line(cx: &mut TestAppContext) {
    // The host refuses to store a schema 3 document (D200 schema-unknown), so its record is given one here.
    let thread = thread_with(Some(r#"<slate title="Newer"><column><text value="Old" /></column></slate>"#));
    let (screen, handle, cx) = open(cx, &thread);
    until(cx, &screen, "the slate drawn", settled);
    let mut newer = slates_get(&handle, &thread);
    newer["document"]["schema"] = json!(3);
    screen.update(cx, |s, cx| s.take_record(Record::read(&newer), cx));
    until(cx, &screen, "the newer slate's empty state", |s| matches!(&s.load, Load::Empty(r) if r.newer.is_some()));
    assert_eq!(cx.read(|cx| screen.read(cx).record().unwrap().newer), Some(3));
    assert_eq!(words::newer(3), "This slate needs a newer wsp (schema 3)");
    assert!(drawn(cx, "slate-empty-newer".into()), "the empty state is marked newer");
}

const FORMULAS: &str = r#"<slate title="Formulas">
  <value name="n" start={5} />
  <value name="show" start={false} />
  <value name="items" start={[{ name: "a", v: 1 }, { name: "b", v: 2 }, { name: "c", v: 3 }]} />
  <column>
    <text id="double" value={$n * 2} />
    <text id="shown" when={$show} value="Shown" />
    <text id="refused" value={$n / 0} />
    <table id="rows" items={$items} key={item.name}><col title="Name" value={item.name} /><col title="Twice" value={item.v * 2} /></table>
  </column>
</slate>"#;

#[gpui_kit::test]
fn every_formula_drawn_is_the_hosts_answer(cx: &mut TestAppContext) {
    let thread = thread_with(Some(FORMULAS));
    let (screen, handle, cx) = open(cx, &thread);
    until(cx, &screen, "the formulas answered", settled);

    assert_eq!(cx.read(|cx| prop(screen.read(cx), "double", "value")), Some(json!(10)), "$n * 2 draws the host's 10");
    assert!(drawn(cx, "slate-piece-double-text".into()));

    let rows = cx.read(|cx| {
        let slate = screen.read(cx).slate().unwrap();
        slate.rows(slate.doc().unwrap().piece("rows").unwrap()).expect("the table's rows answered")
    });
    let twice: Vec<Value> = rows.iter().map(|r| r.props["columns"][1]["value"].clone()).collect();
    assert_eq!(twice, [json!(2), json!(4), json!(6)], "each row's own formula is answered: {rows:?}");
    assert_eq!(rows.iter().map(|r| r.props["key"].clone()).collect::<Vec<_>>(), [json!("a"), json!("b"), json!("c")]);

    let refused = cx.read(|cx| {
        let s = screen.read(cx);
        let slate = s.slate().unwrap();
        let expr = slate.formulas().into_iter().find(|f| f == "($n / 0)").expect("the refused formula is asked");
        (slate.resolved.get(&expr).cloned(), prop(s, "refused", "value"))
    });
    assert_eq!(refused, (Some(Value::Null), Some(Value::Null)), "the host answers no value for $n / 0 and nothing stands in for it");

    // No evaluator in the window: the same slate with nothing answered draws no value for any formula.
    let bare = cx.read(|cx| {
        let record = screen.read(cx).record().unwrap().clone();
        let slate = Slate::new(record);
        let doc = slate.doc().unwrap();
        (slate.props(doc.piece("double").unwrap()), slate.rows(doc.piece("rows").unwrap()), slate.shown("shown"))
    });
    assert!(!bare.0.contains_key("value") && bare.1.is_none() && !bare.2, "unanswered, a formula draws nothing: {bare:?}");

    assert!(!cx.read(|cx| screen.read(cx).slate().unwrap().shown("shown")), "when={{$show}} hides while $show is false");
    assert!(!drawn(cx, "slate-piece-shown-text".into()));
    screen.update(cx, |s, cx| s.write("$show".into(), json!(true), cx));
    until(cx, &screen, "the shown piece shown", |s| s.slate().is_some_and(|slate| slate.shown("shown")));
    assert!(drawn(cx, "slate-piece-shown-text".into()), "when={{$show}} shows once $show is true");
    assert_eq!(on_host(&handle, &thread, "$show"), Some(json!(true)));
}

#[gpui_kit::test]
fn a_formula_reading_the_clock_asks_again_each_second(cx: &mut TestAppContext) {
    let thread = thread_with(Some(r#"<slate><column><text id="now" value={time.now} /></column></slate>"#));
    let (screen, _, cx) = open(cx, &thread);
    until(cx, &screen, "time.now answered", settled);
    let first = cx.read(|cx| prop(screen.read(cx), "now", "value")).expect("time.now answered");
    screen.update(cx, |s, _| s.timings.clear());
    for second in 1..=3 {
        cx.executor().advance_clock(Duration::from_secs(1));
        until(cx, &screen, "the tick's round trip", |s| s.timings.len() >= second);
    }
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| screen.read(cx).timings.len()), 3, "one round trip a second over three seconds");
    let last = cx.read(|cx| prop(screen.read(cx), "now", "value")).unwrap();
    assert!(last.as_f64() > first.as_f64(), "the clock moved: {first} then {last}");
}

/// The panel as a tab the frame keeps built while another tab or a shut panel hides it.
struct Hidden {
    screen: Entity<SlateScreen>,
    show: bool,
}

impl Render for Hidden {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().when(self.show, |d| d.child(self.screen.clone()))
    }
}

#[gpui_kit::test]
fn a_slate_off_screen_stops_its_clock_and_asks_again_on_its_return(cx: &mut TestAppContext) {
    let thread = thread_with(Some(r#"<slate><column><text id="now" value={time.now} /></column></slate>"#));
    let handle = init(cx);
    let opened = Opened { thread_id: Some(thread.clone()), workspace_id: None, tab: Some("slate".into()), arg: None };
    let (mut made, mut kept) = (None, None);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened, window, cx));
        made = Some(screen.clone());
        let hidden = cx.new(|_| Hidden { screen, show: true });
        kept = Some(hidden.clone());
        Root::new(hidden, window, cx)
    });
    let (screen, hidden) = (made.unwrap(), kept.unwrap());
    until(cx, &screen, "time.now answered", settled);
    hidden.update(cx, |h, cx| {
        h.show = false;
        cx.notify();
    });
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(2_200));
    screen.update(cx, |s, _| s.timings.clear());
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| screen.read(cx).timings.len()), 0, "a hidden slate's clock asks nothing");
    hidden.update(cx, |h, cx| {
        h.show = true;
        cx.notify();
    });
    until(cx, &screen, "a round trip on the slate's return", |s| !s.timings.is_empty());
}

#[gpui_kit::test]
fn a_slate_with_no_clock_does_not_tick(cx: &mut TestAppContext) {
    let thread = thread_with(Some(r#"<slate><value name="n" start={1} /><column><text id="n" value={$n + 1} /></column></slate>"#));
    let (screen, _, cx) = open(cx, &thread);
    until(cx, &screen, "$n + 1 answered", settled);
    screen.update(cx, |s, _| s.timings.clear());
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
    }
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| screen.read(cx).timings.len()), 0, "no round trip without a clock read");
}

const WRITES: &str = r#"<slate title="Writes">
  <value name="name" start="" />
  <value name="presses" start={0} />
  <column>
    <input id="name" label="Name" value={$name} />
    <number id="count" label="Presses" value={$presses} />
    <button id="add" label="Add" onPress={set($presses, $presses + 1)} />
    <button id="soon" label="Soon gone" onPress={set($presses, $presses + 1)} />
  </column>
</slate>"#;

#[gpui_kit::test]
fn writes_and_presses_reach_the_host(cx: &mut TestAppContext) {
    let thread = thread_with(Some(WRITES));
    let (screen, handle, cx) = open(cx, &thread);
    until(cx, &screen, "the writes slate", settled);

    screen.update(cx, |s, cx| s.write("$name".into(), json!("wsp-landing"), cx));
    assert_eq!(cx.read(|cx| screen.read(cx).slate().unwrap().value("$name").cloned()), Some(json!("wsp-landing")), "drawn at once");
    until(cx, &screen, "the write answered", |s| s.slate().is_some_and(|slate| slate.mine.is_empty()));
    assert_eq!(on_host(&handle, &thread, "$name"), Some(json!("wsp-landing")), "the write reached the host");

    screen.update(cx, |s, cx| s.raise("add".into(), "press", Raise::default(), cx));
    until(cx, &screen, "the press answered", |s| !busy(s, "add"));
    assert_eq!(on_host(&handle, &thread, "$presses"), Some(json!(1)), "the press moved $presses on the host");
    until(cx, &screen, "the press drawn", |s| prop(s, "count", "value") == Some(json!(1)));

    // The agent drops the button as the person presses it, before the window has taken the rewrite.
    let soon = r#"    <button id="soon" label="Soon gone" onPress={set($presses, $presses + 1)} />
"#;
    fixture::write(host(), &thread, &WRITES.replace(soon, ""));
    screen.update(cx, |s, cx| s.raise("soon".into(), "press", Raise::default(), cx));
    until(cx, &screen, "the refused press answered", |s| !busy(s, "soon"));
    let refused = cx.read(|cx| outcome(screen.read(cx), "soon"));
    assert!(matches!(&refused, Some(Outcome::Refused(r)) if r.contains("is gone")), "a refused press is drawn as a refusal: {refused:?}");
    assert_eq!(on_host(&handle, &thread, "$presses"), Some(json!(1)));

    // A refused write stays drawn as typed, as the web's does, and the host keeps what it had.
    screen.update(cx, |s, cx| s.write("$gone".into(), json!(1), cx));
    cx.run_until_parked();
    std::thread::sleep(Duration::from_millis(500));
    cx.run_until_parked();
    assert_eq!(on_host(&handle, &thread, "$gone"), None, "the host refused $gone");
    assert_eq!(cx.read(|cx| screen.read(cx).slate().unwrap().value("$gone").cloned()), Some(json!(1)));
}

const LIVE: &str = r#"<slate title="Live">
  <value name="n" start={1} />
  <run name="count" cmd="for i in 1 2 3; do echo $i; sleep 0.2; done" stream timeout={20} />
  <column>
    <text id="double" value={$n * 2} />
    <button id="go" label="Go" onPress={start($count)} />
    <output run={$count} />
  </column>
</slate>"#;

#[gpui_kit::test]
fn pushes_from_outside_are_drawn_without_a_reload(cx: &mut TestAppContext) {
    let thread = thread_with(Some(LIVE));
    let (screen, _, cx) = open(cx, &thread);
    until(cx, &screen, "the live slate", settled);

    host().wsp(&["slate", "state", &thread, "$n=5"]);
    until(cx, &screen, "slate.values drawn", |s| prop(s, "double", "value") == Some(json!(10)));

    let rewrite = LIVE.replace(r#"title="Live""#, r#"title="Rewritten""#);
    fixture::write(host(), &thread, &rewrite);
    until(cx, &screen, "session.slate drawn", |s| {
        s.slate().is_some_and(|slate| slate.doc().unwrap().title.as_deref() == Some("Rewritten"))
    });
    assert_eq!(cx.read(|cx| screen.read(cx).slate().unwrap().record.version), 2);

    screen.update(cx, |s, cx| s.raise("go".into(), "press", Raise::default(), cx));
    until(cx, &screen, "the run's consent", |s| s.consent.asking.is_some());
    let key = cx.read(|cx| screen.read(cx).consent.asking.as_ref().unwrap().key.clone());
    let approved = screen.update(cx, |s, cx| s.approve(key, "thread", cx));
    cx.run_until_parked();
    let lines = |s: &SlateScreen| s.slate().and_then(|slate| slate.lines.get("count").cloned()).unwrap_or_default();
    until(cx, &screen, "the run's lines streamed", |s| lines(s).len() >= 3);
    drop(approved);
    assert_eq!(cx.read(|cx| lines(screen.read(cx))), ["1", "2", "3"]);

    until(cx, &screen, "the run done", |s| s.slate().is_some_and(|slate| slate.record.run_state("count") == Some("done")));
    host().wsp(&["slate", "state", &thread, "--start", "count"]);
    until(cx, &screen, "the run restarted", |s| s.slate().is_some_and(|slate| slate.record.run_state("count") == Some("running")));
    until(cx, &screen, "the second run done", |s| s.slate().is_some_and(|slate| slate.record.run_state("count") == Some("done")));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| lines(screen.read(cx))), ["1", "2", "3"], "a run started again streams afresh");
}

/// `count median p95 max` of round trips, in milliseconds.
fn spread(mut t: Vec<Duration>) -> String {
    t.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let at = |q: f64| ms(t[((t.len() as f64 * q).ceil() as usize).clamp(1, t.len()) - 1]);
    format!("count {:>3}  median {:>7.2} ms  p95 {:>7.2} ms  max {:>7.2} ms", t.len(), at(0.5), at(0.95), ms(*t.last().unwrap()))
}

#[gpui_kit::test]
#[ignore = "a measurement: cargo test --test slate d2_round_trips -- --ignored --nocapture"]
fn d2_round_trips(cx: &mut TestAppContext) {
    let (host, folder) = bench();
    let states: Vec<_> = ["every", "many", "timers"].iter().map(|n| fixture::STATES.iter().find(|s| s.name == *n).unwrap()).collect();
    let threads: Vec<_> = states.iter().map(|s| (s.name, fixture::staged(host, folder, s))).collect();
    let handle = init(cx);
    for (name, thread) in threads {
        let (screen, vcx) = mount(cx, &handle, &thread);
        until(vcx, &screen, "the slate answered", settled);
        let formulas = vcx.read(|cx| screen.read(cx).slate().unwrap().formulas().len());
        screen.update(vcx, |s, _| s.timings.clear());
        for n in 1..=50 {
            screen.update(vcx, |s, cx| s.resolve(cx));
            // Polled without a sleep, so the window's own wait adds little to what the panel times.
            let limit = Instant::now() + Duration::from_secs(20);
            while vcx.read(|cx| screen.read(cx).timings.len()) < n {
                vcx.run_until_parked();
                assert!(Instant::now() < limit, "round trip {n} never landed");
            }
        }
        let timings = vcx.read(|cx| screen.read(cx).timings.clone());
        println!("D2 {name:<7} {formulas:>3} formulas  {}", spread(timings));
    }
}
