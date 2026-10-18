// SPDX-License-Identifier: AGPL-3.0-only
//! The slate's controls pressed and typed into against a host started for this binary alone: a press that moves the
//! host's value, a submit, a toggle's write, and a press refused, its refusal drawn under it. `SLATE_PNG=<folder>`
//! with `shots` (ignored) draws the controls, the image pieces and the image dialog in both themes.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
#[path = "slate/fixture.rs"]
mod fixture;

use std::path::PathBuf;
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
fn a_toggle_writes_its_value(cx: &mut TestAppContext) {
    let thread = thread_with(CONTROLS);
    let (screen, cx) = open(cx, &thread);
    cx.run_until_parked();
    let switch = cx.debug_bounds("switch").expect("the switch is drawn");
    cx.simulate_click(switch.center(), Modifiers::none());
    until(cx, &screen, "the host holding the toggle on", |s| host_value(s, "on") == Some(json!(true)));
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
