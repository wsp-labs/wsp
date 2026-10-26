// SPDX-License-Identifier: AGPL-3.0-only
//! The slate's controls pressed and typed into against a host started for this binary alone: a press that moves the
//! host's value, a submit, a toggle's write, and a press refused, its refusal drawn under it. `SLATE_PNG=<folder>`
//! with `shots` (ignored) draws the controls, the image pieces and the image dialog in both themes.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use common::{shared, Throwaway};
use desktop_gpui::screens::slate::{Load, Outcome, SlateScreen};
use desktop_gpui::screens::{Host, Opened, Screen};
use desktop_gpui::ui::{self, ActivePalette, Side, Still, UiAssets};
use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    div, prelude::*, px, size, AnyWindowHandle, Context, Entity, HeadlessAppContext, Modifiers, TestAppContext, VisualTestContext, Window,
};
use serde_json::{json, Value};
use wsp_host_client::HostClient;

const CONTROLS: &str = r#"<slate title="Controls">
  <value name="presses" start={0} />
  <value name="name" start="" />
  <value name="on" start={false} />
  <value name="target" start="ftp://example.com/file" />
  <column>
    <button id="add" label="Add one" onPress={set($presses, $presses + 1)} />
    <input id="who" label="Your name" value={$name} placeholder="Ada" onSubmit={set($presses, $presses + 10)} />
    <toggle id="flag" label="Show done" value={$on} note="Ticked steps stay in the list" />
    <button id="bad" label="Open the ftp link" onPress={open($target)} />
  </column>
</slate>"#;

fn bench() -> (&'static Throwaway, &'static PathBuf) {
    static FOLDER: OnceLock<PathBuf> = OnceLock::new();
    let host = &shared().host;
    (host, FOLDER.get_or_init(|| fixture::project(host)))
}

fn thread_with(text: &str) -> String {
    let (host, folder) = bench();
    let thread = fixture::staged(host, folder, &fixture::State { name: "controls", file: None });
    fixture::write(host, &thread, text);
    thread
}

fn client() -> Host {
    let host = bench().0;
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message)));
    Host::new(client, runtime)
}

fn opened(thread: &str) -> Opened {
    Opened { thread_id: Some(thread.to_owned()), workspace_id: None, tab: Some("slate".into()), arg: None }
}

fn open<'a>(cx: &'a mut TestAppContext, thread: &str) -> (Entity<SlateScreen>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let handle = client();
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened(thread), window, cx));
        made = Some(screen.clone());
        Root::new(screen, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let screen = made.unwrap();
    until(cx, &screen, "the slate drawn and answered", |s| {
        s.slate().is_some_and(|slate| slate.formulas().iter().all(|f| slate.resolved.contains_key(f)))
    });
    (screen, cx)
}

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

/// The host's own value at `$name`, as its last push or answer gave it.
fn host_value(s: &SlateScreen, name: &str) -> Option<Value> {
    s.slate()?.record.values.get(name).cloned()
}

/// Presses a piece's own box a little in from its top left corner, where its control stands.
fn press(cx: &mut VisualTestContext, selector: &str) {
    cx.run_until_parked();
    let at = cx.debug_bounds(Box::leak(selector.to_owned().into_boxed_str())).unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(at.origin + gpui_kit::point(px(12.), px(12.)), Modifiers::none());
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_press_moves_the_hosts_value(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    press(cx, "slate-piece-add-button");
    until(cx, &screen, "the host counting the press", |s| host_value(s, "presses") == Some(json!(1)));
    until(cx, &screen, "the press answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
}

/// A table whose one action opens each row, so a press anywhere on a row raises it; five rows put one at its middle.
const ROWS: &str = r#"<slate title="Rows">
  <value name="n" start={0} />
  <column>
    <table id="list" items={[{ name: "a" }, { name: "b" }, { name: "c" }, { name: "d" }, { name: "e" }]} key={item.name}>
      <col title="Name" value={item.name} />
      <action label="Count" onPress={set($n, $n + 1)} />
    </table>
  </column>
</slate>"#;

#[gpui_kit::test]
fn a_second_press_on_a_row_within_half_a_second_is_folded_into_the_first(cx: &mut TestAppContext) {
    let thread = thread_with(ROWS);
    let (screen, cx) = open(cx, &thread);
    let table = cx.debug_bounds("slate-piece-list-table").expect("the table is drawn");
    let row = table.center();
    let first = Instant::now();
    cx.simulate_click(row, Modifiers::none());
    until(cx, &screen, "the first press answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    cx.simulate_click(row, Modifiers::none());
    assert!(first.elapsed() < Duration::from_millis(500), "the second press came inside the fold");
    until(cx, &screen, "the first press counted", |s| host_value(s, "n").and_then(|n| n.as_i64()).is_some_and(|n| n >= 1));
    std::thread::sleep(Duration::from_millis(300));
    until(cx, &screen, "nothing out", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    assert_eq!(cx.read(|cx| host_value(screen.read(cx), "n")), Some(json!(1)), "the second press folded into the first");
}

/// A row's own fill, carrying a path it reads in the row.
const ROW_FILL: &str = r#"<slate title="Row fill">
  <column>
    <table id="checks" items={[{ name: "a" }, { name: "b" }, { name: "c" }, { name: "d" }, { name: "e" }]} key={item.name}>
      <col title="Check" value={item.name} />
      <action label="Ask" onPress={fill("About this check:", item.name)} />
    </table>
  </column>
</slate>"#;

#[gpui_kit::test]
fn a_rows_fill_carries_the_rows_own_value(cx: &mut TestAppContext) {
    let kept = tempfile::tempdir().unwrap();
    cx.update(|cx| desktop_gpui::local_store::init(kept.path().join("local-store.json"), cx));
    let thread = thread_with(ROW_FILL);
    let (screen, cx) = open(cx, &thread);
    let table = cx.debug_bounds("slate-piece-checks-table").expect("the table is drawn");
    cx.simulate_click(table.center(), Modifiers::none());
    until(cx, &screen, "the press answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let draft =
        cx.update(|_, cx| desktop_gpui::screens::composer::store::draft(cx.global::<desktop_gpui::local_store::LocalStore>(), &thread));
    let draft = draft.map(|(text, _)| text).unwrap_or_default();
    let name = draft.strip_prefix("About this check:\n\n> item.name: ").map(|rest| rest.trim_end_matches('\n'));
    assert!(
        matches!(name, Some("\"a\"" | "\"b\"" | "\"c\"" | "\"d\"" | "\"e\"")),
        "the pressed row's own name, as the web reads it in the row: {draft:?}"
    );
}

#[gpui_kit::test]
fn enter_writes_the_field_and_submits(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    cx.run_until_parked();
    let field = cx.debug_bounds("slate-field").expect("the field is drawn");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("Grace");
    cx.simulate_keystrokes("enter");
    until(cx, &screen, "the host holding the typed name", |s| host_value(s, "name") == Some(json!("Grace")));
    until(cx, &screen, "the submit raised", |s| host_value(s, "presses") == Some(json!(10)));
}

#[gpui_kit::test]
fn a_second_enter_while_the_first_write_is_out_submits_nothing(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    cx.run_until_parked();
    let field = cx.debug_bounds("slate-field").expect("the field is drawn");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("Grace");
    let enter = || gpui_kit::Keystroke::parse("enter").unwrap();
    cx.update(|window, cx| window.dispatch_keystroke(enter(), cx));
    // The window's thread runs nothing until it is parked, so the first Enter's write stays unanswered past the fold.
    std::thread::sleep(Duration::from_millis(600));
    cx.update(|window, cx| window.dispatch_keystroke(enter(), cx));
    until(cx, &screen, "the submit raised", |s| host_value(s, "presses") != Some(json!(0)));
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(50));
        cx.run_until_parked();
    }
    until(cx, &screen, "nothing out", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    assert_eq!(host_value_of(cx, &screen), Some(json!(10)), "the second Enter came while the first was out, so it submits nothing");
}

#[gpui_kit::test]
fn a_toggle_writes_its_value(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    cx.run_until_parked();
    let switch = cx.debug_bounds("switch").expect("the switch is drawn");
    cx.simulate_click(switch.center(), Modifiers::none());
    until(cx, &screen, "the host holding the toggle on", |s| host_value(s, "on") == Some(json!(true)));
}

/// A press that fills the composer.
const FILL: &str = r#"<slate title="Fill">
  <value name="topic" start="the flaky test" />
  <column>
    <button id="ask" label="Ask about it" onPress={fill("Look into this:", $topic)} />
  </column>
</slate>"#;

type Composer = Entity<desktop_gpui::screens::composer::Composer>;

/// The slate panel over the thread's composer.
struct Beside {
    screen: Entity<SlateScreen>,
    composer: Composer,
}

impl Render for Beside {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(div().h(px(560.)).child(self.screen.clone()))
            .child(div().flex_1().child(self.composer.clone()))
    }
}

fn beside<'a>(cx: &'a mut TestAppContext, thread: &str) -> (Entity<SlateScreen>, Composer, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let handle = client();
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened(thread), window, cx));
        let composer = cx.new(|cx| desktop_gpui::screens::composer::Composer::new(window, cx));
        made = Some((screen.clone(), composer.clone()));
        Root::new(cx.new(|_| Beside { screen, composer }), window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let (screen, composer) = made.unwrap();
    until(cx, &screen, "the slate drawn and answered", |s| {
        s.slate().is_some_and(|slate| slate.formulas().iter().all(|f| slate.resolved.contains_key(f)))
    });
    let thread = thread.to_owned();
    composer.update_in(cx, |c, window, cx| c.set_thread_key(thread, window, cx));
    (screen, composer, cx)
}

/// What the composer holds once the press is answered and the panel drew again.
fn composed(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>, composer: &Composer) -> String {
    until(cx, screen, "the press answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    composer.read_with(cx, |c, cx| c.text(cx))
}

#[gpui_kit::test]
fn fill_puts_its_words_at_the_caret_and_keeps_the_draft(cx: &mut TestAppContext) {
    let thread = thread_with(FILL);
    let (screen, composer, cx) = beside(cx, &thread);
    composer.update_in(cx, |c, window, cx| c.set_text("my words", window, cx));
    press(cx, "slate-piece-ask-button");
    let text = composed(cx, &screen, &composer);
    assert_eq!(text, "my words\nLook into this:\n\n> $topic: \"the flaky test\"\n", "the words go in at the caret on lines of their own");
}

#[gpui_kit::test]
fn a_refused_press_says_why_under_it(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    press(cx, "slate-piece-bad-button");
    until(cx, &screen, "the press refused", |s| s.slate().is_some_and(|sl| matches!(sl.outcomes.get("bad"), Some(Outcome::Refused(_)))));
    cx.run_until_parked();
    let line = cx.debug_bounds("slate-outcome").expect("the refusal is drawn");
    let button = cx.debug_bounds("slate-piece-bad-button").unwrap();
    assert!(line.origin.y > button.origin.y, "the refusal stands under its button");
    assert_eq!(host_value_of(cx, &screen), Some(json!(0)), "a refused press moves nothing");
}

fn host_value_of(cx: &mut VisualTestContext, screen: &Entity<SlateScreen>) -> Option<Value> {
    cx.read(|cx| host_value(screen.read(cx), "presses"))
}

/// A link a browser opens as https://example.com/a: the extra slash before the host is dropped.
const SLASHES: &str = r#"<slate title="Links">
  <value name="target" start="https:///example.com/a" />
  <column>
    <button id="slashes" label="Open it" onPress={open($target)} />
  </column>
</slate>"#;

#[gpui_kit::test]
fn a_link_with_an_extra_slash_waits_on_the_person(cx: &mut TestAppContext) {
    let thread = thread_with(SLASHES);
    let (screen, cx) = open(cx, &thread);
    press(cx, "slate-piece-slashes-button");
    until(cx, &screen, "the press answered", |s| s.slate().is_some_and(|sl| sl.busy.is_empty()));
    assert_eq!(cx.opened_url(), None, "example.com is not allowed for the thread, so nothing opens before the person says so");
    let asked = cx.read(|cx| screen.read(cx).consent.linking.clone());
    assert_eq!(asked.as_deref(), Some("https:///example.com/a"), "the link sheet asks first");
}

/// MEDIA with its pictures named whole: the stand-in agent says its folder is the root.
fn media() -> String {
    let shots = bench().1.join("shots");
    MEDIA.replace("\"shots/", &format!("\"{}/", shots.display()))
}

#[gpui_kit::test]
fn a_thumbnail_opens_the_dialog_and_escape_closes_it(cx: &mut TestAppContext) {
    let thread = thread_with(&media());
    let (_, cx) = open(cx, &thread);
    let limit = Instant::now() + Duration::from_secs(20);
    while cx.debug_bounds("slate-image-loading").is_some() {
        assert!(Instant::now() < limit, "the pictures never came");
        std::thread::sleep(Duration::from_millis(20));
        cx.run_until_parked();
    }
    let thumb = cx.debug_bounds("slate-gal-thumb-0").expect("the gallery's first thumbnail is drawn");
    cx.simulate_click(thumb.origin + gpui_kit::point(px(20.), px(20.)), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.update(gpui_kit::component::WindowExt::has_active_dialog), "the thumbnail opens the dialog");
    assert!(cx.debug_bounds("slate-image-at").is_some(), "the dialog says where it is among three");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.update(gpui_kit::component::WindowExt::has_active_dialog), "Escape closes it");
}

const KEPT: &str = r#"<slate title="Kept">
  <column>
    <section id="files" title="Files" collapsible>
      <text id="inside">Inside the fold</text>
    </section>
    <bars id="cpu" label="CPU" items={[{ n: "node", v: 42 }]} name={item.n} value={item.v} />
    <bars id="mem" label="Memory" items={[{ n: "git", v: 120 }]} name={item.n} value={item.v} />
  </column>
</slate>"#;

/// The panel in a frame that can leave it out, as a tab behind another or a shut panel does.
struct Hider {
    screen: Entity<SlateScreen>,
    show: Rc<Cell<bool>>,
}

impl Render for Hider {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(self.show.get().then(|| self.screen.clone()))
    }
}

#[gpui_kit::test]
fn a_fold_and_a_pick_outlive_a_frame_unseen(cx: &mut TestAppContext) {
    let thread = thread_with(KEPT);
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    cx.executor().allow_parking();
    let handle = client();
    let show = Rc::new(Cell::new(true));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let screen = cx.new(|cx| SlateScreen::new(handle.clone(), opened(&thread), window, cx));
        made = Some(screen.clone());
        let hider = cx.new(|_| Hider { screen, show: show.clone() });
        Root::new(hider, window, cx)
    });
    cx.simulate_resize(size(px(540.), px(748.)));
    let screen = made.unwrap();
    until(cx, &screen, "the slate drawn and answered", |s| {
        s.slate().is_some_and(|slate| slate.formulas().iter().all(|f| slate.resolved.contains_key(f)))
    });
    assert!(cx.debug_bounds("slate-piece-inside-text").is_some(), "the section stands open");
    press(cx, "slate-section");
    // The panel's 200 ms close draws the body until it ends.
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("slate-piece-inside-text").is_none(), "its head folds it");
    let memory = cx.debug_bounds("segment").expect("the switch is drawn");
    cx.simulate_click(memory.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("slate-piece-mem-bars").is_some(), "the second segment shows its list");
    for drawn in [false, true] {
        show.set(drawn);
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    assert!(cx.debug_bounds("slate-piece-inside-text").is_none(), "the fold is kept");
    assert!(cx.debug_bounds("slate-piece-mem-bars").is_some(), "the pick is kept");
}

const MEDIA: &str = r#"<slate title="Pictures">
  <value name="three" start={["shots/before.png", "shots/after.png", "shots/plot.png"]} />
  <column>
    <image src="shots/plot.png" caption="A saved plot" />
    <images id="gal" items={[{ p: "shots/before.png", c: "Before" }, { p: "shots/after.png", c: "After" }, { p: "shots/plot.png", c: "Plot" }]} src={item.p} caption={item.c} />
    <images layout="strip" items={[{ p: "shots/before.png", c: "Before" }, { p: "shots/after.png", c: "After" }, { p: "shots/plot.png", c: "Plot" }, { p: "shots/before.png", c: "Again" }]} src={item.p} caption={item.c} />
    <images layout="compare" items={[{ p: "shots/before.png", c: "Before" }, { p: "shots/after.png", c: "After" }]} src={item.p} caption={item.c} />
    <images items={[]} src={item.p} />
    <images layout="compare" items={$three} src={item} />
    <image src="https://example.com/chart.png" caption="An address not yet allowed" />
    <diagram label="Deploy">flowchart LR
  build[Build] --> check{Tests pass?}
  check -->|yes| ship([Ship])</diagram>
  </column>
</slate>"#;

const NUMBER: &str = r#"<slate title="Number">
  <value name="count" start={5} />
  <column>
    <input id="num" label="Count" kind="number" value={$count} />
  </column>
</slate>"#;

#[gpui_kit::test]
fn a_whole_number_typed_echoes_back_as_the_fields_own_write(cx: &mut TestAppContext) {
    let thread = thread_with(NUMBER);
    let (screen, cx) = open(cx, &thread);
    // Focus and blur reach a field only in the active window.
    cx.update(|window, _| window.activate_window());
    let field = cx.debug_bounds("slate-field").expect("the field is drawn");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_keystrokes("backspace");
    cx.simulate_input("12");
    cx.executor().advance_clock(Duration::from_millis(300));
    until(cx, &screen, "the host holding 12", |s| host_value(s, "count").and_then(|v| v.as_f64()) == Some(12.0));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|window, cx| window.blur(cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("slate-conflict").is_none(), "the host's 12 is this field's own write, not someone else's");
}

const INPUTS: &str = r#"<slate title="Inputs">
  <value name="name" start="" />
  <value name="notes" start="" />
  <secret name="token" />
  <value name="pick" start={null} />
  <value name="answer" start="Link" />
  <value name="on" start={true} />
  <value name="failed" start={false} />
  <column>
    <row>
      <select label="Hide" value={$pick}><option value="Done" /><option value="Canceled" /></select>
      <toggle label="Failed only" value={$failed} />
    </row>
    <input label="Your name" value={$name} placeholder="Ada" onSubmit={set($on, true)} />
    <input label="Where it runs" value={$name} placeholder="a folder on this computer" />
    <input label="Notes" value={$notes} lines={3} onSubmit={set($on, true)} />
    <input label="Token" value={$token} />
    <select label="Hide" value={$pick}><option value="Done" /><option value="Canceled" /></select>
    <choices label="Which layer routes packets?" value={$answer} options={["Link", "Network", "Transport"]} answer="Network" />
    <toggle label="Show done" value={$on} note="Ticked steps stay in the list" />
    <button id="deploy" label="Deploy" held="Waiting on the build" onPress={set($on, true)} />
    <button label="Run hello" variant="primary" onPress={set($on, true)} />
  </column>
</slate>"#;

#[gpui_kit::test]
fn a_write_from_elsewhere_while_typing_leaves_the_typed_words_drawn_until_the_choice(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    // Focus and blur reach a field only in the active window.
    cx.update(|window, _| window.activate_window());
    let field = cx.debug_bounds("slate-field").expect("the field is drawn");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_input("Grace");
    cx.executor().advance_clock(Duration::from_millis(300));
    until(cx, &screen, "Grace sent", |s| host_value(s, "name") == Some(json!("Grace")));
    let elsewhere: wsp_host_client::protocol::SlatesStateRequest =
        serde_json::from_value(json!({ "threadId": thread, "values": { "$name": "Ada" } })).unwrap();
    futures::executor::block_on(client().run(move |host| async move { host.call(elsewhere).await })).unwrap();
    until(cx, &screen, "Ada pushed", |s| host_value(s, "name") == Some(json!("Ada")));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|window, cx| window.blur(cx));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(cx.debug_bounds("slate-conflict").is_some(), "Keep mine and Take theirs are offered");
    let drawn = cx.read(|cx| screen.read(cx).slate().and_then(|s| s.value("$name").cloned()));
    assert_eq!(drawn, Some(json!("Grace")), "the field draws what the person typed until they choose");
}

struct Panel {
    screen: Entity<SlateScreen>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background).child(self.screen.clone())
    }
}

/// One state drawn by the real offscreen renderer at 540 by `h` css px at 2x, `act` run on the window once it settled.
/// What a shot does to the panel before it is taken.
type Act<'a> = Option<&'a dyn Fn(&mut Window, &mut gpui_kit::App)>;

fn shoot(theme: &str, host: &Host, thread: &str, h: f32, act: Act) -> image::RgbaImage {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.set_global(Still(true));
        ui::theme::set_theme(theme, cx);
    });
    let mut made = None;
    let handle: AnyWindowHandle = cx
        .open_window(size(px(540.), px(h)), |window, cx| {
            let screen = cx.new(|cx| SlateScreen::new(host.clone(), opened(thread), window, cx));
            made = Some(screen.clone());
            let panel = cx.new(|_| Panel { screen });
            cx.new(|cx| Root::new(panel, window, cx))
        })
        .unwrap()
        .into();
    let screen = made.unwrap();
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        let ready = cx.update(|cx| matches!(&screen.read(cx).load, Load::Drawn(s) if !s.resolved.is_empty()));
        if ready || Instant::now() > until {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let settle = |cx: &mut HeadlessAppContext| {
        for _ in 0..12 {
            cx.update_window(handle, |_, window, cx| window.render_frame(cx)).unwrap();
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(100));
        }
    };
    settle(&mut cx);
    if let Some(act) = act {
        cx.update_window(handle, |_, window, cx| act(window, cx)).unwrap();
        cx.advance_clock(Duration::from_millis(600));
        settle(&mut cx);
    }
    cx.capture_screenshot(handle).expect("offscreen rendering must be available")
}

#[test]
#[ignore = "draws pictures for a person to look at; run with SLATE_PNG=<folder>"]
fn shots() {
    let out = PathBuf::from(std::env::var_os("SLATE_PNG").expect("SLATE_PNG names the folder"));
    let host = client();
    let media = thread_with(&media());
    let inputs = thread_with(INPUTS);
    let thumb = |window: &mut Window, cx: &mut gpui_kit::App| window.click("slate-gal-thumb-0", cx);
    let next = |window: &mut Window, cx: &mut gpui_kit::App| {
        window.click("slate-gal-thumb-0", cx);
        window.render_frame(cx);
        window.press("right", cx);
    };
    let deploy = |window: &mut Window, cx: &mut gpui_kit::App| window.hover("slate-deploy-held", cx);
    let states: [(&str, &str, f32, Act); 5] = [
        ("media", &media, 2100., None),
        ("dialog", &media, 748., Some(&thumb)),
        ("dialog-next", &media, 748., Some(&next)),
        ("inputs", &inputs, 1300., None),
        ("held", &inputs, 1300., Some(&deploy)),
    ];
    for (name, thread, h, act) in states {
        for theme in ["paper", "graphite"] {
            let image = shoot(theme, &host, thread, h, act);
            std::fs::create_dir_all(out.join(theme)).unwrap();
            image.save(out.join(theme).join(format!("{name}.png"))).unwrap();
            println!("{name:<8} {theme:<9} drawn");
        }
    }
}

#[gpui_kit::test]
fn a_press_the_host_said_nothing_of_draws_no_line_under_it(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    let before = cx.debug_bounds("slate-piece-add-button").expect("the button is drawn").size;
    press(cx, "slate-piece-add-button");
    until(cx, &screen, "the press answered", |s| {
        host_value(s, "presses") == Some(json!(1)) && s.slate().is_some_and(|sl| sl.busy.is_empty())
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let after = cx.debug_bounds("slate-piece-add-button").expect("the button is drawn").size;
    assert_eq!(after, before, "an empty outcome takes no line, so nothing under the button moves");
}

#[gpui_kit::test]
fn a_tables_figure_column_lines_up_with_the_one_beside_it(cx: &mut TestAppContext) {
    let thread = thread_with(
        r#"<slate title="Widths">
  <column>
    <table id="a" items={[{ n: "lint", t: "1" }]}>
      <col title="Check" value={item.n} />
      <col title="N" value={item.t} mono align="end" />
    </table>
    <table id="b" items={[{ n: "unit", t: "120044" }]}>
      <col title="Check" value={item.n} />
      <col title="T" value={item.t} mono align="end" />
    </table>
  </column>
</slate>
"#,
    );
    let (_, cx) = open(cx, &thread);
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let width = |cx: &mut VisualTestContext, s: &'static str| cx.debug_bounds(s).unwrap_or_else(|| panic!("{s} is drawn")).size.width;
    let (a, b) = (width(cx, "slate-a-head-1"), width(cx, "slate-b-head-1"));
    assert_eq!(a, b, "the figure column takes the widest figure of both tables, as `sharedWidths` does");
}
