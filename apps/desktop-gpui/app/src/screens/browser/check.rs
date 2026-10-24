// SPDX-License-Identifier: AGPL-3.0-only
//! `desktop-gpui --screen browser`: the pane on a page this process serves, with no host, driven through the
//! platform's own input (XTEST on Linux, `NSApplication sendEvent:` on the Mac) and one line printed per check, so
//! what only a run can prove (the cut, the press over it, the keys either side) reads off the terminal.

use std::cell::Cell;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::component::Root;
use gpui_kit::{
    actions, div, prelude::*, px, size, App, AsyncWindowContext, Bounds, Context, Entity, FocusHandle, KeyBinding, Pixels, Point,
    Subscription, Window, WindowBounds, WindowOptions,
};

use super::page::{Page, PageEvent};
use super::BrowserScreen;
use crate::screens::Opened;
use crate::ui::ActivePalette;

/// The page the checks drive: it reports each key, press and its field's text in its title, which the app reads.
const PAGE: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>ready</title>
<style>
  body { font: 15px system-ui, sans-serif; margin: 0; background: #fff; }
  header { position: sticky; top: 0; background: #2563eb; color: white; padding: 12px 16px; }
  input { font: inherit; padding: 6px 8px; width: 220px; }
  .row { padding: 10px 16px; border-bottom: 1px solid #ddd; }
  .row:nth-child(odd) { background: #f4f4f5; }
</style></head>
<body>
<header><input id="field" placeholder="type here"> a page on <span id="origin"></span></header>
<div id="rows"></div>
<script>
  let presses = 0, keys = [];
  const say = () => { document.title = "presses=" + presses + ";keys=" + keys.join(",") + ";field=" + field.value; };
  document.getElementById("origin").textContent = location.host;
  const rows = document.getElementById("rows");
  for (let i = 0; i < 400; i++) { const d = document.createElement("div"); d.className = "row"; d.textContent = "Row " + i; rows.appendChild(d); }
  const field = document.getElementById("field");
  addEventListener("mousedown", () => { presses++; say(); }, true);
  addEventListener("keydown", (e) => {
    keys.push((e.ctrlKey ? "ctrl-" : "") + (e.metaKey ? "cmd-" : "") + (e.altKey ? "alt-" : "") + (e.shiftKey ? "shift-" : "") + e.key.toLowerCase());
    say();
  }, true);
  field.addEventListener("input", say);
  let last = scrollY, moved = 0, since = 0, running = false, rates = [];
  function tick(now) {
    if (scrollY !== last) { moved++; last = scrollY; }
    if (now - since >= 1000) {
      if (moved === 0) { running = false; return; }
      rates.push(Math.round(moved * 1000 / (now - since)));
      document.title = "scroll-fps=" + rates.join(",");
      moved = 0; since = now;
    }
    requestAnimationFrame(tick);
  }
  addEventListener("scroll", () => { if (running) return; running = true; moved = 0; since = performance.now(); last = scrollY; requestAnimationFrame(tick); });
  addEventListener("load", () => setTimeout(() => { field.focus(); say(); }, 0));
</script>
</body></html>"#;

/// A page that registers a service worker the way a page gets around the init script, then says what answers `/probe`.
const WORKER_PAGE: &str = r#"<!doctype html><title>worker</title><script>
  const sw = navigator.serviceWorker;
  const say = async (state) => { document.title = "sw=" + state + ";probe=" + await (await fetch("/probe")).text(); };
  const claimed = () => sw.controller ? null : new Promise((done) => sw.addEventListener("controllerchange", done, { once: true }));
  if (location.search === "?register") {
    ServiceWorkerContainer.prototype.register.call(sw, "/sw.js").then(() => sw.ready).then(claimed).then(() => say("registered"),
      (e) => { document.title = "sw=refused " + e.name; });
  } else {
    sw.getRegistrations().then((all) => say("registrations " + all.length));
  }
</script>"#;

/// The worker: it answers `/probe` itself, where the server would answer "server".
const WORKER: &str = r#"self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));
self.addEventListener("fetch", (e) => { if (new URL(e.request.url).pathname === "/probe") e.respondWith(new Response("worker")); });"#;

/// The folder a check run keeps its store and the pages' data in, apart from the person's, gone when the run ends.
pub fn scratch() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wsp-browser-check-{}", std::process::id()));
    _ = std::fs::create_dir_all(&dir);
    dir
}

fn finish(failed: u32) -> ! {
    _ = std::fs::remove_dir_all(scratch());
    std::process::exit(if failed == 0 { 0 } else { 1 });
}

/// Serves [`PAGE`] on an ephemeral port of 127.0.0.1 from a thread of its own.
pub fn serve() -> std::io::Result<String> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = format!("http://{}/", listener.local_addr()?);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut reader = BufReader::new(&stream);
                let mut first = String::new();
                _ = reader.read_line(&mut first);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let mut stream = &stream;
                // `/hold` answers its head and never its body, so the page stays loading until it is stopped.
                if first.starts_with("GET /hold") {
                    _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 100000\r\n\r\n<p>held"
                    );
                    std::thread::sleep(Duration::from_secs(60));
                    return;
                }
                let (kind, body) = match first.split_whitespace().nth(1).unwrap_or("/") {
                    path if path.starts_with("/sw-page") => ("text/html", WORKER_PAGE),
                    "/sw.js" => ("text/javascript", WORKER),
                    "/probe" => ("text/plain", "server"),
                    _ => ("text/html", PAGE),
                };
                _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    Ok(address)
}

actions!(browser_check, [AppChord, PageChord]);

thread_local! {
    static START: Instant = Instant::now();
}

/// One fact a line, after the milliseconds since launch.
pub fn log(fact: impl std::fmt::Display) {
    let ms = START.with(|start| start.elapsed().as_millis());
    println!("{ms:>7} {fact}");
}

/// How the checks went, counted as they print.
#[derive(Default)]
struct Tally {
    passed: Cell<u32>,
    failed: Cell<u32>,
}

impl Tally {
    fn check(&self, name: &str, ok: bool, detail: impl std::fmt::Display) {
        if ok {
            self.passed.set(self.passed.get() + 1);
        } else {
            self.failed.set(self.failed.get() + 1);
        }
        log(format!("check {name}: {} ({detail})", if ok { "yes" } else { "NO" }));
    }
}

/// The window the checks drive: a column standing for the thread, and the pane at the panel's default width.
pub struct CheckPane {
    screen: Entity<BrowserScreen>,
    focus: FocusHandle,
    app_chords: u32,
    page_chords: u32,
    /// Loads the page began, which a pick of Hard reload adds one to.
    loads: Rc<Cell<u32>>,
    _page: Option<Subscription>,
}

impl CheckPane {
    fn page(&self, cx: &App) -> Option<Entity<Page>> {
        self.screen.read(cx).page_entity(cx)
    }

    fn set_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.screen.update(cx, |s, cx| s.set_menu(open, cx));
    }

    fn menu_open(&self, cx: &App) -> bool {
        self.screen.read(cx).menu_open()
    }

    /// The menu as the page sees it: the first thing cut out of it.
    fn menu(&self, cx: &App) -> Option<Bounds<Pixels>> {
        self.page(cx)?.read(cx).placed()?.1.first().copied()
    }

    fn typed(&self, cx: &App) -> String {
        self.screen.read(cx).field().read(cx).text(cx).to_string()
    }
}

impl Render for CheckPane {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("check")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &AppChord, _, cx| {
                this.app_chords += 1;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &PageChord, _, cx| {
                this.page_chords += 1;
                cx.notify();
            }))
            .size_full()
            .flex()
            .bg(cx.palette().background)
            .text_color(cx.palette().foreground)
            .child(div().flex_1().p_4().child("The thread"))
            .child(div().w(px(540.)).h_full().border_l_1().border_color(cx.palette().border).child(self.screen.clone()))
    }
}

/// Opens the check window on `url` and runs the checks, printing a line for each; exits with their count failed.
pub fn open(cx: &mut App) {
    let url = match serve() {
        Ok(url) => url,
        Err(e) => {
            eprintln!("desktop-gpui: browser check: the page could not be served: {e}");
            std::process::exit(1);
        }
    };
    log(format!("page: serving {url}"));
    let mod_key = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
    cx.bind_keys([
        KeyBinding::new(&format!("{mod_key}-w"), AppChord, Some(crate::screens::app_shell::keys::PREVIEW)),
        KeyBinding::new(&format!("{mod_key}-k"), PageChord, None),
    ]);
    let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..WindowOptions::default() };
    let opened = cx.open_window(options, |window, cx| {
        let opened = Opened { tab: Some("browser:check".into()), ..Opened::default() };
        let screen = cx.new(|cx| BrowserScreen::standalone(opened, window, cx));
        let pane =
            cx.new(|cx| CheckPane { screen, focus: cx.focus_handle(), app_chords: 0, page_chords: 0, loads: Rc::default(), _page: None });
        pane.update(cx, |p, cx| window.focus(&p.focus, cx));
        let root_pane = pane.clone();
        window
            .spawn(cx, async move |cx| {
                let failed = run(root_pane, url, cx).await;
                finish(failed);
            })
            .detach();
        cx.new(|cx| Root::new(pane, window, cx))
    });
    if let Err(e) = opened {
        eprintln!("desktop-gpui: browser check: the window did not open: {e}");
        finish(1);
    }
    cx.activate(true);
}

async fn wait(cx: &mut AsyncWindowContext, ms: u64) {
    cx.background_executor().timer(Duration::from_millis(ms)).await;
}

/// Waits up to `ms` for `ready`, polling every 50 ms.
async fn until(cx: &mut AsyncWindowContext, ms: u64, mut ready: impl FnMut(&mut Window, &mut App) -> bool) -> bool {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        if cx.update(|window, cx| ready(window, cx)).unwrap_or(false) {
            return true;
        }
        wait(cx, 50).await;
    }
    false
}

fn title(pane: &Entity<CheckPane>, cx: &App) -> String {
    pane.read(cx).page(cx).map(|p| p.read(cx).title.clone()).unwrap_or_default()
}

fn placed(pane: &Entity<CheckPane>, cx: &App) -> Option<(Bounds<Pixels>, Vec<Bounds<Pixels>>)> {
    pane.read(cx).page(cx)?.read(cx).placed().cloned()
}

fn field_of(title: &str, name: &str) -> String {
    title.split(';').find_map(|part| part.strip_prefix(&format!("{name}="))).unwrap_or_default().to_owned()
}

async fn run(pane: Entity<CheckPane>, url: String, cx: &mut AsyncWindowContext) -> u32 {
    let tally = Tally::default();
    let input = match cx.update(|window, _| synth::Input::new(window)) {
        Ok(Ok(input)) => input,
        Ok(Err(e)) => {
            log(format!("check input: NO (no way to drive the window: {e})"));
            return 1;
        }
        Err(e) => {
            log(format!("check input: NO (the window is gone: {e})"));
            return 1;
        }
    };
    let frames = Rc::new(Cell::new(0u32));
    let counted = frames.clone();
    let _counting = cx.update(|window, _| {
        window.on_drawn(move |_, _| {
            counted.set(counted.get() + 1);
            true
        })
    });
    // The page opens once the window stands, as a person opens one: typed into the pane the window already shows.
    wait(cx, 500).await;
    _ = cx.update(|window, cx| pane.update(cx, |p, cx| p.screen.update(cx, |s, cx| s.open_text(&url, window, cx))));
    let loaded = until(cx, 20_000, |_, cx| title(&pane, cx).starts_with("presses=")).await;
    tally.check("page loads", loaded, format!("title {:?}", cx.update(|_, cx| title(&pane, cx)).unwrap_or_default()));
    let Some(page) = cx.update(|_, cx| pane.read(cx).page(cx)).ok().flatten() else { return tally.failed.get() + 1 };
    _ = cx.update(|_, cx| {
        pane.update(cx, |p, cx| {
            let loads = p.loads.clone();
            p._page = Some(cx.subscribe(&page, move |_, _, event: &PageEvent, _| {
                if *event == PageEvent::Loading(true) {
                    loads.set(loads.get() + 1);
                }
            }));
        })
    });
    frames.set(0);
    until(cx, 3_000, |_, cx| placed(&pane, cx).is_some()).await;

    let at = cx.update(|window, cx| placed(&pane, cx).map(|(b, _)| (b, window.scale_factor()))).ok().flatten();
    let Some((slot, scale)) = at else {
        let (zoom, active) = cx.update(|window, _| (window.zoom(), window.is_window_active())).unwrap_or_default();
        let drawn = frames.get();
        tally.check(
            "bounds",
            false,
            format!("the page was never placed; {drawn} frame(s) drawn in 3 s since it loaded, zoom {zoom}, active {active}"),
        );
        return tally.failed.get();
    };
    let native = cx.update(|_, cx| synth::page_geometry(&page, cx)).ok().flatten();
    let want = super::page::device(slot, scale);
    // GTK places a child in its own whole-scale units, so at scale 2 an odd device edge lands a pixel off.
    let near =
        |n: (i32, i32, i32, i32)| [(n.0, want.0), (n.1, want.1), (n.2, want.2), (n.3, want.3)].iter().all(|(a, b)| (a - b).abs() <= 1);
    tally.check("bounds", native.is_some_and(near), format!("slot {want:?} in device px at scale {scale}, the page's own {native:?}"));
    let drawn = cx.update(|_, cx| page.read(cx).native().and_then(|n| n.drawn_size())).ok().flatten();
    tally.check(
        "the page fills its slot",
        drawn.is_some_and(|(w, h)| (w - want.2).abs() <= 2 && (h - want.3).abs() <= 2),
        format!("the web view draws {drawn:?} device px in a {}x{} slot", want.2, want.3),
    );
    let center = slot.center();

    // Zoomed as Cmd+Plus twice leaves the window, the page still stands in its slot, laid out at the slot's size.
    _ = cx.update(|_, cx| cx.set_window_zoom(1.2));
    let moved = until(cx, 3_000, |window, cx| window.zoom() == 1.2 && placed(&pane, cx).is_some_and(|(b, _)| b != slot)).await;
    let zoomed = cx.update(|window, cx| placed(&pane, cx).map(|(b, _)| (b, window.scale_factor()))).ok().flatten();
    _ = cx.update(|_, cx| page.read(cx).run_script("document.title = 'inner=' + innerWidth + 'x' + innerHeight"));
    until(cx, 2_000, |_, cx| title(&pane, cx).starts_with("inner=")).await;
    let t = cx.update(|_, cx| title(&pane, cx)).unwrap_or_default();
    let inner: Option<(f32, f32)> = field_of(&t, "inner").split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
    let native = cx.update(|_, cx| synth::page_geometry(&page, cx)).ok().flatten();
    let fills = zoomed.is_some_and(|(at, scale)| {
        let fits = inner.is_some_and(|(w, h)| (w - f32::from(at.size.width)).abs() <= 1. && (h - f32::from(at.size.height)).abs() <= 1.);
        let want = super::page::device(at, scale);
        let near =
            |n: (i32, i32, i32, i32)| [(n.0, want.0), (n.1, want.1), (n.2, want.2), (n.3, want.3)].iter().all(|(a, b)| (a - b).abs() <= 1);
        fits && native.is_some_and(near)
    });
    tally.check(
        "the page fills its slot at 1.2x",
        moved && fills,
        format!("slot {zoomed:?} in window px at that scale, the page's own {native:?} device px, its viewport {inner:?} css px"),
    );
    _ = cx.update(|_, cx| {
        cx.set_window_zoom(1.);
        page.read(cx).run_script("say()");
    });
    until(cx, 3_000, |window, cx| window.zoom() == 1. && placed(&pane, cx).is_some_and(|(b, _)| b == slot)).await;

    // At rest, with the page loaded, GPUI draws nothing: a page that is still costs the window no frames.
    wait(cx, 300).await;
    frames.set(0);
    wait(cx, 1000).await;
    let at_rest = frames.replace(0);
    tally.check("the window rests", at_rest <= 10, format!("{at_rest} frame(s) drawn in a second with the page still"));

    // Before any press the page holds no keys, wherever the pointer is.
    input.move_to(center, cx).await;
    input.chord(&["mod", "k"], cx).await;
    wait(cx, 300).await;
    let (to_app, t) = cx.update(|_, cx| (pane.read(cx).page_chords, title(&pane, cx))).unwrap_or_default();
    let state = cx.update(|_, cx| synth::window_state(&page, cx)).unwrap_or_default();
    tally.check(
        "keys before a press go to the app",
        to_app == 1 && !field_of(&t, "keys").contains('k'),
        format!("the app saw mod-k {to_app} time(s), the page saw {:?}; {state}", field_of(&t, "keys")),
    );

    // The More menu over the page: cut out of it, and a press on its first row reaches GPUI.
    frames.set(0);
    _ = cx.update(|_, cx| pane.update(cx, |p, cx| p.set_menu(true, cx)));
    let cut = until(cx, 2_000, |_, cx| pane.read(cx).menu(cx).is_some()).await;
    let menu = cx.update(|_, cx| pane.read(cx).menu(cx)).ok().flatten();
    let state = cx.update(|_, cx| synth::window_state(&page, cx)).unwrap_or_default();
    tally.check("menu cut", cut, format!("cut from the page: {menu:?}; {} frame(s) drawn since it opened; {state}", frames.get()));
    if let Some((ok, detail)) = cx.update(|window, cx| synth::cut_holds(&page, menu, window, cx)).ok().flatten() {
        tally.check("mask", ok, detail);
    }
    if let Some(menu) = menu {
        let row = gpui_kit::point(menu.center().x, menu.top() + px(20.));
        if let Some((ok, detail)) = cx.update(|window, cx| synth::hit_reaches_app(&page, row, window, cx)).ok().flatten() {
            tally.check("hit test", ok, detail);
        }
        let loads = cx.update(|_, cx| pane.read(cx).loads.get()).unwrap_or_default();
        input.click(row, cx).await;
        let picked = until(cx, 3_000, |_, cx| pane.read(cx).loads.get() > loads && !pane.read(cx).menu_open(cx)).await;
        until(cx, 10_000, |_, cx| title(&pane, cx).starts_with("presses=")).await;
        let presses_after = cx.update(|_, cx| field_of(&title(&pane, cx), "presses")).unwrap_or_default();
        tally.check(
            "press on the menu reaches the app",
            picked && presses_after == "0",
            format!("Hard reload ran {}, the page counted {presses_after} press(es) after its reload", picked as u8),
        );
    }

    // A press on the page closes the menu and gives the page the keys.
    _ = cx.update(|_, cx| pane.update(cx, |p, cx| p.set_menu(true, cx)));
    until(cx, 2_000, |_, cx| pane.read(cx).menu(cx).is_some()).await;
    wait(cx, 200).await;
    let below = center + gpui_kit::point(px(0.), px(200.));
    input.click(below, cx).await;
    let closed = until(cx, 2_000, |_, cx| !pane.read(cx).menu_open(cx)).await;
    let pressed = until(cx, 2_000, |_, cx| field_of(&title(&pane, cx), "presses") == "1").await;
    tally.check(
        "press on the page closes the menu",
        closed && pressed,
        format!("menu closed {}, the page saw the press {}", closed as u8, pressed as u8),
    );

    // The page holds the keys now: a letter and Escape are its, the close chord is the app's, the palette's the page's.
    let before = cx.update(|_, cx| (pane.read(cx).app_chords, pane.read(cx).page_chords)).unwrap_or_default();
    input.key("a", cx).await;
    input.key("escape", cx).await;
    input.chord(&["mod", "k"], cx).await;
    input.chord(&["mod", "w"], cx).await;
    wait(cx, 500).await;
    let (after, t) = cx.update(|_, cx| ((pane.read(cx).app_chords, pane.read(cx).page_chords), title(&pane, cx))).unwrap_or_default();
    let keys = field_of(&t, "keys");
    tally.check("page keys", keys.contains('a') && keys.contains("escape"), format!("the page saw {keys:?}"));
    tally.check(
        "mod-k goes to the page",
        after.1 == before.1 && keys.contains("-k"),
        format!("the app's palette chord count {} then {}", before.1, after.1),
    );
    tally.check(
        "mod-w goes to the app",
        after.0 == before.0 + 1 && !keys.contains("-w"),
        format!("the app's close chord count {} then {}", before.0, after.0),
    );

    // A press on the address field returns the keys to the app.
    input.click(slot.origin + gpui_kit::point(px(150.), px(-20.)), cx).await;
    input.key("x", cx).await;
    wait(cx, 400).await;
    let (typed, t) = cx.update(|_, cx| (pane.read(cx).typed(cx), title(&pane, cx))).unwrap_or_default();
    tally.check("keys return to the app", typed.contains('x') && !field_of(&t, "keys").contains('x'), format!("the field holds {typed:?}"));
    input.key("escape", cx).await;

    // Scroll: wheel notches at 60 a second over the page; the page says in how many frames a second it moved.
    if input.scroll(center, 180, cx).await {
        wait(cx, 1200).await;
        let t = cx.update(|_, cx| title(&pane, cx)).unwrap_or_default();
        let rates = t.strip_prefix("scroll-fps=").unwrap_or_default().to_owned();
        log(format!("scroll: frames a second in which the page moved, each second as the page said it: {rates}"));
    }

    // Stop stops a page that is loading, where today's reloads it.
    let base = cx.update(|_, cx| pane.read(cx).page(cx).map(|p| p.read(cx).url.clone())).ok().flatten().unwrap_or_default();
    let held = format!("{}hold", base.trim_end_matches('/').to_owned() + "/");
    _ = cx.update(|window, cx| pane.update(cx, |p, cx| p.screen.update(cx, |s, cx| s.open_text(&held, window, cx))));
    let loading = until(cx, 5_000, |_, cx| pane.read(cx).screen.read(cx).loading(cx)).await;
    let loads = cx.update(|_, cx| pane.read(cx).loads.get()).unwrap_or_default();
    input.click(slot.origin + gpui_kit::point(px(72.), px(-20.)), cx).await;
    let stopped = until(cx, 3_000, |_, cx| !pane.read(cx).screen.read(cx).loading(cx)).await;
    wait(cx, 500).await;
    let reloaded = cx.update(|_, cx| pane.read(cx).loads.get()).unwrap_or_default() > loads;
    let now = cx.update(|_, cx| pane.read(cx).page(cx).map(|p| p.read(cx).url.clone())).ok().flatten().unwrap_or_default();
    tally.check(
        "Stop stops",
        loading && stopped && !reloaded,
        format!("{held} loading {}, stopped {}, loaded again {}, the page on {now}", loading as u8, stopped as u8, reloaded as u8),
    );

    // Back walks the page's history, then reaches the servers list.
    let mut presses = 0;
    frames.set(0);
    while presses < 4 && cx.update(|_, cx| pane.read(cx).screen.read(cx).on_page(cx)).unwrap_or(false) {
        input.click(slot.origin + gpui_kit::point(px(20.), px(-20.)), cx).await;
        wait(cx, 400).await;
        presses += 1;
    }
    let listed = until(cx, 2_000, |_, cx| !pane.read(cx).screen.read(cx).on_page(cx) && placed(&pane, cx).is_none()).await;
    let state = cx.update(|_, cx| synth::window_state(&page, cx)).unwrap_or_default();
    tally.check(
        "Back reaches the servers list",
        listed,
        format!("after {presses} press(es) of Back the page is hidden and the list shows; {} frame(s) drawn; {state}", frames.get()),
    );
    input.click(slot.origin + gpui_kit::point(px(46.), px(-20.)), cx).await;
    let again = until(cx, 3_000, |_, cx| placed(&pane, cx).is_some()).await;
    tally.check("Forward from the list shows the page again", again, "the page stands in its slot");

    // The page holds the keys and the person moves to another app; the pane then shows another tab, which hides the
    // page and leaves that app its keys.
    let (url, base_page) = (base.clone(), page.clone());
    let main_screen = cx.update(|_, cx| pane.read(cx).screen.clone()).ok();
    input.click(center, cx).await;
    match input.other_app() {
        Ok(None) => {}
        Err(why) => tally.check("hiding the page leaves another app its keys", false, why),
        Ok(Some(other)) => {
            wait(cx, 300).await;
            let held = cx.update(|_, cx| base_page.read(cx).native().is_some_and(|n| n.focused())).unwrap_or(false);
            _ = cx.update(|window, cx| {
                let opened = Opened { tab: Some("browser:list".into()), ..Opened::default() };
                let list = cx.new(|cx| BrowserScreen::standalone(opened, window, cx));
                pane.update(cx, |p, cx| {
                    p.screen = list;
                    cx.notify();
                });
            });
            let hidden = until(cx, 2_000, |_, cx| base_page.read(cx).placed().is_none()).await;
            wait(cx, 300).await;
            let holder = input.focus_name(other);
            tally.check(
                "hiding the page leaves another app its keys",
                held && hidden && holder == "the other app",
                format!("the page held the keys {}, hid {}, X gives the keys to {holder}", held as u8, hidden as u8),
            );
            input.close_other_app(other);
            close_tab(&pane, "browser:list", main_screen.clone(), cx).await;
            until(cx, 2_000, |_, cx| placed(&pane, cx).is_some()).await;
        }
    }

    // A page that closes itself stays, as today's guest does: it still says what it does, and Refresh loads it.
    match fresh_tab(&pane, "browser:closes", &url, cx).await {
        None => tally.check("a page that closes itself stays", false, "its page never loaded"),
        Some(screen) => {
            let tab = cx.update(|_, cx| screen.read(cx).page_entity(cx)).ok().flatten();
            if let Some(tab) = &tab {
                _ = cx.update(|_, cx| {
                    tab.read(cx).run_script("window.close(); setTimeout(() => { document.title = 'closed itself'; }, 300);")
                });
            }
            let spoke = until(cx, 3_000, |_, cx| tab.as_ref().is_some_and(|t| t.read(cx).title == "closed itself")).await;
            input.click(slot.origin + gpui_kit::point(px(72.), px(-20.)), cx).await;
            let reloaded = spoke && until(cx, 5_000, |_, cx| tab.as_ref().is_some_and(|t| t.read(cx).title.starts_with("presses="))).await;
            tally.check(
                "a page that closes itself stays",
                spoke && reloaded,
                format!("it set its title after the close {}, Refresh loaded it {}", spoke as u8, reloaded as u8),
            );
            close_tab(&pane, "browser:closes", main_screen.clone(), cx).await;
        }
    }

    // Closing a second window that holds a page leaves the app and this window's page standing.
    let second = cx.update(|_, cx| {
        let bounds = Bounds::centered(None, size(px(700.), px(600.)), cx);
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..WindowOptions::default() };
        cx.open_window(options, |window, cx| {
            let opened = Opened { tab: Some("browser:second".into()), ..Opened::default() };
            let screen = cx.new(|cx| BrowserScreen::standalone(opened, window, cx));
            screen.update(cx, |s, cx| s.open_text(&url, window, cx));
            cx.new(|cx| Root::new(screen, window, cx))
        })
    });
    match second {
        Ok(Ok(second)) => {
            let key = super::store::Key { thread: "".into(), tab: "browser:second".into() };
            let held = |cx: &App| super::store::Store::get_existing(cx).read(cx).entry(&key).and_then(|e| e.page.clone());
            let loaded = until(cx, 20_000, |_, cx| held(cx).is_some_and(|p| p.read(cx).title.starts_with("presses="))).await;
            _ = second.update(cx, |_, window, _| window.remove_window());
            wait(cx, 1500).await;
            let gone = cx.update(|_, cx| held(cx).is_none()).unwrap_or(false);
            let standing = cx.update(|_, cx| base_page.read(cx).placed().is_some()).unwrap_or(false);
            tally.check(
                "closing a window that holds a page",
                loaded && gone && standing,
                format!(
                    "its page loaded {}, went with it {}, the app lives and this window's page stands {}",
                    loaded as u8, gone as u8, standing as u8
                ),
            );
        }
        _ => tally.check("closing a window that holds a page", false, "the second window did not open"),
    }

    // Pages opened and closed in turn leave nothing behind but the store's one network process.
    let before = synth::held();
    let mut made = 0;
    for n in 0..CHURN {
        let name = format!("browser:churn{n}");
        made += usize::from(fresh_tab(&pane, &name, &url, cx).await.is_some());
        close_tab(&pane, &name, main_screen.clone(), cx).await;
    }
    wait(cx, 2_000).await;
    let after = synth::held();
    tally.check(
        &format!("{CHURN} pages opened and closed"),
        made == CHURN && after.descriptors <= before.descriptors && after.network <= 1,
        format!("{made} loaded; before {before}; after {after}"),
    );

    // Leave the menu up over the page for a look.
    _ = cx.update(|_, cx| pane.update(cx, |p, cx| p.set_menu(true, cx)));
    wait(cx, 1500).await;
    _ = cx.update(|_, cx| pane.update(cx, |p, cx| p.set_menu(false, cx)));

    // A worker a page registered answers no page once the store is made again, as at the app's next start.
    if synth::REMAKES_STORE {
        let worker = format!("{url}sw-page?register");
        let registered = fresh_tab(&pane, "browser:worker", &worker, cx).await.is_some();
        let said = cx.update(|_, cx| title(&pane, cx)).unwrap_or_default();
        close_tab(&pane, "browser:worker", None, cx).await;
        _ = cx.update(|_, cx| super::store::Store::get(cx).update(cx, |s, cx| s.closed(&check_key(), cx)));
        wait(cx, 1_000).await;
        synth::remake_store();
        let after = fresh_tab(&pane, "browser:worker2", &format!("{url}sw-page"), cx).await.is_some();
        let now = cx.update(|_, cx| title(&pane, cx)).unwrap_or_default();
        tally.check(
            "a service worker does not outlive the store",
            registered && said.ends_with("probe=worker") && after && now == "sw=registrations 0;probe=server",
            format!("before the store was made again the page said {said:?}, after {now:?}"),
        );
    }
    log(format!("checks: {} passed, {} failed", tally.passed.get(), tally.failed.get()));
    tally.failed.get()
}

/// The pages the descriptor check opens and closes.
const CHURN: usize = 60;

fn check_key() -> super::store::Key {
    super::store::Key { thread: "".into(), tab: "browser:check".into() }
}

/// Puts a screen on a new tab named `name` in the pane, opens `url` in it and waits for its page to say it loaded.
async fn fresh_tab(pane: &Entity<CheckPane>, name: &str, url: &str, cx: &mut AsyncWindowContext) -> Option<Entity<BrowserScreen>> {
    let screen = cx
        .update(|window, cx| {
            let opened = Opened { tab: Some(name.to_owned().into()), ..Opened::default() };
            let screen = cx.new(|cx| BrowserScreen::standalone(opened, window, cx));
            pane.update(cx, |p, cx| {
                p.screen = screen.clone();
                cx.notify();
            });
            screen.update(cx, |s, cx| s.open_text(url, window, cx));
            screen
        })
        .ok()?;
    let page = |cx: &App| screen.read(cx).page_entity(cx);
    let loaded = until(cx, 20_000, |_, cx| page(cx).is_some_and(|p| p.read(cx).title.contains('='))).await;
    loaded.then_some(screen)
}

/// Closes the tab named `name` for good and puts `back` in the pane again.
async fn close_tab(pane: &Entity<CheckPane>, name: &str, back: Option<Entity<BrowserScreen>>, cx: &mut AsyncWindowContext) {
    _ = cx.update(|_, cx| {
        if let Some(back) = back {
            pane.update(cx, |p, cx| {
                p.screen = back;
                cx.notify();
            });
        }
        let key = super::store::Key { thread: "".into(), tab: name.to_owned().into() };
        super::store::Store::get(cx).update(cx, |s, cx| s.closed(&key, cx));
    });
    wait(cx, 100).await;
}

/// What the process holds open: its descriptors, WebKit's shared memory among them, and its WebKit network processes.
#[derive(Clone, Copy)]
pub struct Held {
    descriptors: usize,
    shared: usize,
    network: usize,
    web: usize,
}

impl std::fmt::Display for Held {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} descriptors, {} of WebKit's shared memory, {} network and {} web process(es)",
            self.descriptors, self.shared, self.network, self.web
        )
    }
}

#[cfg(target_os = "linux")]
mod synth {
    //! XTEST: the server makes each press and key as if a person had, so it reaches whichever window X picks.

    use super::*;
    use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection as _;
    use x11rb::protocol::xproto::{self, ConnectionExt as _};
    use x11rb::protocol::xtest::ConnectionExt as _;
    use x11rb::rust_connection::RustConnection;

    pub struct Input {
        conn: RustConnection,
        root: u32,
        window: u32,
        scale: f32,
        min_keycode: u8,
        keysyms: Vec<u32>,
        per: u8,
    }

    impl Input {
        pub fn new(window: &Window) -> Result<Input, String> {
            let xid = match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Xcb(xcb)) => xcb.window.get(),
                Ok(RawWindowHandle::Xlib(xlib)) => xlib.window as u32,
                _ => return Err("not an X11 window".to_owned()),
            };
            let (conn, screen) = x11rb::connect(None).map_err(|e| e.to_string())?;
            let root = conn.setup().roots[screen].root;
            let (min, max) = (conn.setup().min_keycode, conn.setup().max_keycode);
            let map = conn.get_keyboard_mapping(min, max - min + 1).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
            Ok(Input {
                conn,
                root,
                window: xid,
                scale: window.scale_factor(),
                min_keycode: min,
                per: map.keysyms_per_keycode,
                keysyms: map.keysyms,
            })
        }

        fn keycode(&self, name: &str) -> Option<u8> {
            let sym = match name {
                "mod" | "ctrl" => 0xffe3,
                "shift" => 0xffe1,
                "escape" => 0xff1b,
                one if one.len() == 1 => u32::from(one.as_bytes()[0]),
                _ => return None,
            };
            let index = self.keysyms.iter().position(|&s| s == sym)?;
            Some(self.min_keycode + (index / self.per as usize) as u8)
        }

        fn to_root(&self, at: Point<Pixels>) -> (i16, i16) {
            let (x, y) = ((f32::from(at.x) * self.scale) as i16, (f32::from(at.y) * self.scale) as i16);
            let moved = self.conn.translate_coordinates(self.window, self.root, x, y).ok().and_then(|c| c.reply().ok());
            moved.map_or((x, y), |m| (m.dst_x, m.dst_y))
        }

        fn fake(&self, kind: u8, detail: u8, at: (i16, i16)) {
            _ = self.conn.xtest_fake_input(kind, detail, x11rb::CURRENT_TIME, self.root, at.0, at.1, 0);
            _ = self.conn.flush();
        }

        pub async fn move_to(&self, at: Point<Pixels>, cx: &mut AsyncWindowContext) {
            self.fake(xproto::MOTION_NOTIFY_EVENT, 0, self.to_root(at));
            wait(cx, 120).await;
        }

        pub async fn click(&self, at: Point<Pixels>, cx: &mut AsyncWindowContext) {
            let at = self.to_root(at);
            self.fake(xproto::MOTION_NOTIFY_EVENT, 0, at);
            wait(cx, 120).await;
            self.fake(xproto::BUTTON_PRESS_EVENT, 1, (0, 0));
            wait(cx, 60).await;
            self.fake(xproto::BUTTON_RELEASE_EVENT, 1, (0, 0));
            wait(cx, 200).await;
        }

        pub async fn chord(&self, keys: &[&str], cx: &mut AsyncWindowContext) {
            let codes: Vec<u8> = keys.iter().filter_map(|k| self.keycode(k)).collect();
            for &code in &codes {
                self.fake(xproto::KEY_PRESS_EVENT, code, (0, 0));
            }
            for &code in codes.iter().rev() {
                self.fake(xproto::KEY_RELEASE_EVENT, code, (0, 0));
            }
            wait(cx, 150).await;
        }

        pub async fn key(&self, key: &str, cx: &mut AsyncWindowContext) {
            self.chord(&[key], cx).await;
        }

        /// `notches` wheel steps down at 60 a second over `at`.
        pub async fn scroll(&self, at: Point<Pixels>, notches: u32, cx: &mut AsyncWindowContext) -> bool {
            self.fake(xproto::MOTION_NOTIFY_EVENT, 0, self.to_root(at));
            let start = Instant::now();
            for n in 0..notches {
                self.fake(xproto::BUTTON_PRESS_EVENT, 5, (0, 0));
                self.fake(xproto::BUTTON_RELEASE_EVENT, 5, (0, 0));
                let next = start + Duration::from_micros(16_667 * u64::from(n + 1));
                cx.background_executor().timer(next.saturating_duration_since(Instant::now())).await;
            }
            true
        }
    }

    /// Where the page's X11 window stands in device pixels.
    pub fn page_geometry(page: &Entity<Page>, cx: &App) -> Option<(i32, i32, i32, i32)> {
        page.read(cx).native()?.geometry()
    }

    /// Whether the page's shape leaves the menu's middle out and keeps a point under it in.
    pub fn cut_holds(page: &Entity<Page>, menu: Option<Bounds<Pixels>>, window: &Window, cx: &App) -> Option<(bool, String)> {
        let page = page.read(cx);
        let (at, _) = page.placed()?;
        let menu = menu?;
        let rects = page.native()?.shape()?;
        let scale = window.scale_factor();
        let local = |p: Point<Pixels>| (((f32::from(p.x - at.origin.x)) * scale) as i32, ((f32::from(p.y - at.origin.y)) * scale) as i32);
        let inside = |(x, y): (i32, i32)| rects.iter().any(|&(rx, ry, rw, rh)| x >= rx && x < rx + rw && y >= ry && y < ry + rh);
        let over = local(menu.center());
        let under = local(gpui_kit::point(menu.center().x, menu.bottom() + px(40.)));
        let ok = !inside(over) && inside(under);
        Some((
            ok,
            format!(
                "page window shaped by {} rectangle(s); menu middle {over:?} cut {}, below it {under:?} kept {}",
                rects.len(),
                !inside(over),
                inside(under)
            ),
        ))
    }

    /// Which window X gives the keys to.
    pub fn window_state(page: &Entity<Page>, cx: &App) -> String {
        page.read(cx).native().map(|n| format!("keys held by {}", n.keys_held_by())).unwrap_or_default()
    }

    /// The store is made once a process on Linux, so it can be made again in one run.
    pub const REMAKES_STORE: bool = true;

    pub fn remake_store() {
        crate::screens::browser::page::remake_context();
    }

    pub fn held() -> Held {
        let links: Vec<String> = std::fs::read_dir("/proc/self/fd")
            .map(|dir| dir.flatten().filter_map(|e| std::fs::read_link(e.path()).ok()).map(|l| l.to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        let me = std::process::id().to_string();
        let children: Vec<String> = std::fs::read_dir("/proc")
            .map(|dir| {
                dir.flatten()
                    .filter_map(|e| std::fs::read_to_string(e.path().join("stat")).ok())
                    .filter(|stat| stat.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(1)) == Some(me.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        let named = |name: &str| children.iter().filter(|stat| stat.contains(name)).count();
        Held {
            descriptors: links.len(),
            shared: links.iter().filter(|l| l.contains("WebKitSharedMemory")).count(),
            network: named("(WebKitNetworkPr)"),
            web: named("(WebKitWebProces)"),
        }
    }

    impl Input {
        /// A window of another app, mapped and handed the keys.
        pub fn other_app(&self) -> Result<Option<u32>, String> {
            let made = (|| {
                let other = self.conn.generate_id().ok()?;
                let aux = xproto::CreateWindowAux::new();
                self.conn.create_window(0, other, self.root, 0, 0, 100, 100, 0, xproto::WindowClass::INPUT_OUTPUT, 0, &aux).ok()?;
                self.conn.map_window(other).ok()?;
                self.conn.flush().ok()?;
                std::thread::sleep(Duration::from_millis(100));
                self.conn.set_input_focus(xproto::InputFocus::PARENT, other, x11rb::CURRENT_TIME).ok()?;
                self.conn.flush().ok()?;
                Some(other)
            })();
            made.map(Some).ok_or_else(|| "another app's window could not be made".to_owned())
        }

        pub fn focus_name(&self, other: u32) -> &'static str {
            match self.conn.get_input_focus().ok().and_then(|c| c.reply().ok()).map(|r| r.focus) {
                Some(focus) if focus == other => "the other app",
                Some(focus) if focus == self.window => "GPUI's window",
                _ => "another window",
            }
        }

        pub fn close_other_app(&self, other: u32) {
            _ = self.conn.destroy_window(other);
            _ = self.conn.set_input_focus(xproto::InputFocus::PARENT, self.window, x11rb::CURRENT_TIME);
            _ = self.conn.flush();
        }
    }

    /// X routes the press by the shape: no read of its own to add here.
    pub fn hit_reaches_app(_: &Entity<Page>, _: Point<Pixels>, _: &Window, _: &App) -> Option<(bool, String)> {
        None
    }
}

#[cfg(target_os = "macos")]
mod synth {
    //! `NSApplication postEvent:`: each press and key goes through AppKit's own dispatch, `hitTest:` and
    //! `performKeyEquivalent:` included, as a person's would.

    use super::*;
    use crate::screens::browser::page::mac;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSPoint, NSRect};
    use objc::{class, msg_send, sel, sel_impl};
    use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    pub struct Input {
        window: id,
        view: id,
    }

    const LEFT_DOWN: u64 = 1;
    const LEFT_UP: u64 = 2;
    const MOVED: u64 = 5;
    const KEY_DOWN: u64 = 10;
    const KEY_UP: u64 = 11;
    const COMMAND: u64 = 1 << 20;
    const SHIFT: u64 = 1 << 17;

    impl Input {
        pub fn new(window: &Window) -> Result<Input, String> {
            let view = match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
                Ok(RawWindowHandle::AppKit(handle)) => handle.ns_view.as_ptr() as id,
                _ => return Err("not an AppKit window".to_owned()),
            };
            // SAFETY: GPUI's view lives as long as the window the checks run in.
            let window: id = unsafe { msg_send![view, window] };
            unsafe {
                let _: () = msg_send![window, makeKeyAndOrderFront: nil];
            }
            Ok(Input { window, view })
        }

        /// A top-left point of GPUI's view in the window's bottom-left coordinates.
        fn in_window(&self, at: Point<Pixels>) -> NSPoint {
            // SAFETY: as above.
            unsafe {
                let bounds: NSRect = msg_send![self.view, bounds];
                let flipped: cocoa::base::BOOL = msg_send![self.view, isFlipped];
                let y = f64::from(at.y);
                let local = NSPoint::new(f64::from(at.x), if flipped == YES { y } else { bounds.size.height - y });
                msg_send![self.view, convertPoint: local toView: nil]
            }
        }

        fn post(&self, event: id) {
            // SAFETY: posting an event the app made to its own queue, on the main thread.
            unsafe {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, postEvent: event atStart: cocoa::base::NO];
            }
        }

        fn mouse(&self, kind: u64, at: NSPoint) {
            // SAFETY: as above.
            unsafe {
                let number: i64 = msg_send![self.window, windowNumber];
                let event: id = msg_send![class!(NSEvent), mouseEventWithType: kind location: at modifierFlags: 0u64 timestamp: 0f64
                    windowNumber: number context: nil eventNumber: 0i64 clickCount: 1i64 pressure: 1f32];
                self.post(event);
            }
        }

        fn keyboard(&self, kind: u64, code: u16, chars: &str, flags: u64) {
            // SAFETY: as above.
            unsafe {
                let number: i64 = msg_send![self.window, windowNumber];
                let text = std::ffi::CString::new(chars).unwrap_or_default();
                let string: id = msg_send![class!(NSString), stringWithUTF8String: text.as_ptr()];
                let event: id = msg_send![class!(NSEvent), keyEventWithType: kind location: NSPoint::new(0., 0.) modifierFlags: flags
                    timestamp: 0f64 windowNumber: number context: nil characters: string charactersIgnoringModifiers: string
                    isARepeat: cocoa::base::NO keyCode: code];
                self.post(event);
            }
        }

        pub async fn move_to(&self, at: Point<Pixels>, cx: &mut AsyncWindowContext) {
            self.mouse(MOVED, self.in_window(at));
            wait(cx, 120).await;
        }

        pub async fn click(&self, at: Point<Pixels>, cx: &mut AsyncWindowContext) {
            let at = self.in_window(at);
            self.mouse(MOVED, at);
            wait(cx, 120).await;
            self.mouse(LEFT_DOWN, at);
            wait(cx, 60).await;
            self.mouse(LEFT_UP, at);
            wait(cx, 200).await;
        }

        pub async fn chord(&self, keys: &[&str], cx: &mut AsyncWindowContext) {
            let flags = keys.iter().fold(0, |f, k| match *k {
                "mod" => f | COMMAND,
                "shift" => f | SHIFT,
                _ => f,
            });
            for key in keys.iter().filter(|k| !matches!(**k, "mod" | "shift")) {
                let (code, chars) = match *key {
                    "a" => (0, "a"),
                    "k" => (40, "k"),
                    "w" => (13, "w"),
                    "x" => (7, "x"),
                    "escape" => (53, "\u{1b}"),
                    _ => continue,
                };
                self.keyboard(KEY_DOWN, code, chars, flags);
                self.keyboard(KEY_UP, code, chars, flags);
            }
            wait(cx, 150).await;
        }

        pub async fn key(&self, key: &str, cx: &mut AsyncWindowContext) {
            self.chord(&[key], cx).await;
        }

        /// AppKit makes no wheel event an app can post; scroll the page by hand to read its rate.
        pub async fn scroll(&self, _: Point<Pixels>, _: u32, _: &mut AsyncWindowContext) -> bool {
            log("scroll: scroll the page by hand; it prints scroll-fps in its title");
            false
        }
    }

    /// Where the page's view stands in GPUI's, in points.
    pub fn page_geometry(page: &Entity<Page>, cx: &App) -> Option<(i32, i32, i32, i32)> {
        page.read(cx).native()?.geometry()
    }

    /// Whether the page's mask hides the menu's middle and shows a point under it, read off the mask's own path.
    pub fn cut_holds(page: &Entity<Page>, menu: Option<Bounds<Pixels>>, _: &Window, cx: &App) -> Option<(bool, String)> {
        let page = page.read(cx);
        let (at, _) = page.placed()?;
        let menu = menu?;
        let (view, _) = page.native()?.views();
        let over = menu.center() - at.origin;
        let under = gpui_kit::point(menu.center().x, menu.bottom() + px(40.)) - at.origin;
        // SAFETY: the page's view is live on this thread.
        let (shows_over, shows_under, how) =
            unsafe { (mac::mask_shows(view, over)?, mac::mask_shows(view, under)?, mac::orientation(view)) };
        let ok = !shows_over && shows_under;
        Some((
            ok,
            format!(
                "menu middle cut {}, below it kept {shows_under}; {how}; look: the menu stands whole over the page, Pick me at its top",
                !shows_over
            ),
        ))
    }

    /// The window as AppKit has it: whether any of it shows, which GPUI's frames wait on, whether it is key, and what
    /// holds its keys.
    pub fn window_state(page: &Entity<Page>, cx: &App) -> String {
        let Some((view, gpui)) = page.read(cx).native().map(|n| n.views()) else { return String::new() };
        // SAFETY: both views are live on this thread.
        unsafe {
            let window: id = msg_send![gpui, window];
            let occlusion: u64 = msg_send![window, occlusionState];
            let key: cocoa::base::BOOL = msg_send![window, isKeyWindow];
            let first: id = msg_send![window, firstResponder];
            let holder = if first == gpui {
                "GPUI's view"
            } else if first == view {
                "the page"
            } else if first == window {
                "the window"
            } else {
                "another responder"
            };
            format!("window visible {}, key {}, keys held by {holder}", occlusion & 2 != 0, key == YES)
        }
    }

    /// WKWebView's store is the app's own, made once: the check's worker line is read by hand on the Mac.
    pub const REMAKES_STORE: bool = false;

    pub fn remake_store() {}

    pub fn held() -> Held {
        let descriptors = std::fs::read_dir("/dev/fd").map(|dir| dir.count()).unwrap_or_default();
        Held { descriptors, shared: 0, network: 0, web: 0 }
    }

    impl Input {
        /// Another app's window is not something the check can make on the Mac.
        pub fn other_app(&self) -> Result<Option<u32>, String> {
            Ok(None)
        }

        pub fn focus_name(&self, _: u32) -> &'static str {
            ""
        }

        pub fn close_other_app(&self, _: u32) {}
    }

    /// Whether AppKit's hit test at the menu item lands on GPUI's view rather than the page's.
    pub fn hit_reaches_app(page: &Entity<Page>, at: Point<Pixels>, _: &Window, cx: &App) -> Option<(bool, String)> {
        let (view, gpui) = page.read(cx).native()?.views();
        // SAFETY: both views are live on this thread.
        unsafe {
            let window: id = msg_send![gpui, window];
            let content: id = msg_send![window, contentView];
            let bounds: NSRect = msg_send![gpui, bounds];
            let flipped: cocoa::base::BOOL = msg_send![gpui, isFlipped];
            let y = f64::from(at.y);
            let local = NSPoint::new(f64::from(at.x), if flipped == YES { y } else { bounds.size.height - y });
            let in_window: NSPoint = msg_send![gpui, convertPoint: local toView: nil];
            let in_content: NSPoint = msg_send![content, convertPoint: in_window fromView: nil];
            let superview: id = msg_send![content, superview];
            let probe: NSPoint = if superview == nil { in_content } else { msg_send![content, convertPoint: in_content toView: superview] };
            let hit: id = msg_send![content, hitTest: probe];
            let ok = hit == gpui;
            Some((
                ok,
                format!(
                    "AppKit's hit test at the item lands on {}",
                    if hit == gpui {
                        "GPUI's view"
                    } else if hit == view {
                        "the page"
                    } else {
                        "another view"
                    }
                ),
            ))
        }
    }
}
