// SPDX-License-Identifier: AGPL-3.0-only
//! Spike: a native web view in the right panel, under GPUI, with an address field beside it and a GPUI popover over
//! it. One road per build (see Cargo.toml). Everything a run is judged on is printed to stdout, one fact a line.

mod page;
#[cfg(feature = "kit")]
#[path = "kit.rs"]
mod road;
#[cfg(all(feature = "wry", not(feature = "kit")))]
#[path = "wry_child.rs"]
mod road;

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Root};
use gpui_kit::*;

actions!(spike, [ToggleMenu, CloseMenu, FocusAddress, Chord]);

const SIDEBAR: f32 = 240.;
const DIVIDER: f32 = 6.;

thread_local! {
    static START: Instant = Instant::now();
}

/// One fact a line, after the milliseconds since launch; drive.sh reads these.
pub fn log(fact: impl std::fmt::Display) {
    let ms = START.with(|start| start.elapsed().as_millis());
    println!("{ms:>7} {fact}");
}

struct Args {
    url: Option<String>,
    menu_open: bool,
    no_page: bool,
    wayland: bool,
    panel: f32,
}

fn parse() -> Args {
    let mut args = Args { url: None, menu_open: false, no_page: false, wayland: false, panel: 640. };
    let mut words = std::env::args().skip(1);
    while let Some(word) = words.next() {
        match word.as_str() {
            "--url" => args.url = words.next(),
            "--menu-open" => args.menu_open = true,
            "--no-page" => args.no_page = true,
            "--wayland" => args.wayland = true,
            "--panel" => args.panel = words.next().and_then(|w| w.parse().ok()).unwrap_or(args.panel),
            other => {
                eprintln!("unknown flag {other}; flags: --url <url>, --menu-open, --no-page, --wayland, --panel <px>");
                std::process::exit(2);
            }
        }
    }
    args
}

struct Shell {
    focus: FocusHandle,
    address: Entity<InputState>,
    page: Option<Entity<road::Page>>,
    panel: f32,
    dragging: bool,
    menu_open: bool,
    menu_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl Shell {
    fn new(args: &Args, local: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = args.url.clone().unwrap_or(local);
        let address = cx.new(|cx| InputState::new(window, cx).default_value(url.clone()));
        cx.subscribe_in(&address, window, |this: &mut Self, input, event: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let url = input.read(cx).value().to_string();
                log(format!("address: enter {url}"));
                if let Some(page) = &this.page {
                    page.update(cx, |page, cx| page.load(&url, cx));
                }
            }
        })
        .detach();
        let page = (!args.no_page).then(|| road::Page::new(&url, window, cx));
        Self {
            focus: cx.focus_handle(),
            address,
            page,
            panel: args.panel,
            dragging: false,
            menu_open: args.menu_open,
            menu_bounds: Rc::new(Cell::new(None)),
        }
    }

    fn set_menu(&mut self, open: bool, why: &str, cx: &mut Context<Self>) {
        if self.menu_open != open {
            self.menu_open = open;
            log(format!("menu: {} ({why})", if open { "open" } else { "closed" }));
            if !open {
                self.menu_bounds.set(None);
                if let Some(page) = &self.page {
                    page.update(cx, |page, _| page.set_overlays(Vec::new()));
                }
            }
            cx.notify();
        }
    }

    fn menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let page = self.page.clone();
        let seen = self.menu_bounds.clone();
        let theme = cx.theme().clone();
        let card = v_flex_card(&theme)
            .id("menu")
            .occlude()
            .w(px(280.))
            .p_2()
            .gap_1()
            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| this.set_menu(false, "press outside", cx)))
            .child(div().px_2().py_1().text_sm().text_color(theme.muted_foreground).child("GPUI popover over the page"))
            .children(["Reload", "Open in browser", "Copy address", "Zoom in", "Zoom out", "Developer tools"].map(|item| {
                div().id(item).px_2().py_1().rounded_md().hover(|s| s.bg(theme.accent)).child(item).on_click(cx.listener(
                    move |this, _, _, cx| {
                        log(format!("menu: picked {item}"));
                        this.set_menu(false, "picked", cx);
                    },
                ))
            }));
        // Measured on a wrapper the card fills, since a child of the card sits inside its border.
        div()
            .mt_8()
            .relative()
            .child(card)
            .child(
                canvas(
                    move |bounds, _, cx| {
                        if seen.replace(Some(bounds)) != Some(bounds) {
                            log(format!("menu bounds: {}", rect(bounds)));
                            if let Some(page) = &page {
                                page.update(cx, |page, _| page.set_overlays(vec![bounds]));
                            }
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element()
    }
}

fn v_flex_card(theme: &gpui_kit::component::Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .bg(theme.popover)
        .text_color(theme.popover_foreground)
        .border_1()
        .border_color(theme.border)
        .rounded_lg()
        .shadow_lg()
}

pub fn rect(bounds: Bounds<Pixels>) -> String {
    format!(
        "{:.0},{:.0} {:.0}x{:.0}",
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height)
    )
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let width = f32::from(window.viewport_size().width);
        let panel = self.panel.clamp(240., (width - SIDEBAR - DIVIDER - 160.).max(240.));
        let menu = self.menu_open.then(|| self.menu(cx));
        div()
            .id("shell")
            .key_context("Spike")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &ToggleMenu, _, cx| {
                log("chord: ctrl-k reached GPUI");
                let open = !this.menu_open;
                this.set_menu(open, "ctrl-k", cx);
            }))
            .on_action(cx.listener(|this, _: &CloseMenu, _, cx| {
                log("escape: reached GPUI");
                this.set_menu(false, "escape", cx);
            }))
            .on_action(cx.listener(|this, _: &FocusAddress, window, cx| {
                log("chord: ctrl-l reached GPUI");
                this.address.update(cx, |address, cx| address.focus(window, cx));
            }))
            .on_action(|_: &Chord, _, _| log("chord: ctrl-shift-p reached GPUI"))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.dragging {
                    let width = f32::from(window.viewport_size().width);
                    this.panel = width - f32::from(event.position.x) - DIVIDER / 2.;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, cx| {
                    if this.dragging {
                        this.dragging = false;
                        log(format!("panel drag: ended at {:.0}", this.panel));
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div()
                    .w(px(SIDEBAR))
                    .flex_none()
                    .h_full()
                    .p_3()
                    .bg(theme.sidebar)
                    .child("Threads")
                    .child(div().text_sm().text_color(theme.muted_foreground).child("ctrl-k menu, ctrl-l address")),
            )
            .child(div().flex_1().h_full().p_3().child("The thread"))
            .child(div().id("divider").w(px(DIVIDER)).flex_none().h_full().bg(theme.border).cursor_col_resize().on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.dragging = true;
                    log("panel drag: started");
                    cx.stop_propagation();
                }),
            ))
            .child(
                div()
                    .w(px(panel))
                    .flex_none()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .h(px(44.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .child(div().flex_1().child(Input::new(&self.address)))
                            .child(
                                div()
                                    .id("menu-button")
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(theme.border)
                                    .child("Menu")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let open = !this.menu_open;
                                        this.set_menu(open, "button", cx);
                                    }))
                                    .children(menu.map(|menu| deferred(anchored().anchor(Anchor::TopRight).child(menu)))),
                            ),
                    )
                    .child(div().flex_1().min_h_0().children(self.page.clone())),
            )
    }
}

fn main() {
    let args = parse();
    let local = page::serve().expect("the page server binds 127.0.0.1");
    log(format!("road: {}", road::NAME));
    log(format!("local page: {local}"));
    // Both roads embed WebKitGTK only in an X11 window; --wayland shows what each does on a Wayland one.
    #[cfg(target_os = "linux")]
    let app = gpui_kit::platform::linux(if args.wayland { WindowingModes::WAYLAND } else { WindowingModes::X11 });
    #[cfg(not(target_os = "linux"))]
    let app = gpui_kit::application();
    app.with_assets(gpui_kit::assets::Assets).run(move |cx| {
        gpui_kit::init(cx);
        cx.bind_keys([
            KeyBinding::new("ctrl-k", ToggleMenu, Some("Spike")),
            KeyBinding::new("cmd-k", ToggleMenu, Some("Spike")),
            KeyBinding::new("escape", CloseMenu, Some("Spike")),
            KeyBinding::new("ctrl-l", FocusAddress, Some("Spike")),
            KeyBinding::new("cmd-l", FocusAddress, Some("Spike")),
            KeyBinding::new("ctrl-shift-p", Chord, Some("Spike")),
        ]);
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() };
        cx.open_window(options, |window, cx| {
            let shell = cx.new(|cx| Shell::new(&args, local, window, cx));
            shell.update(cx, |shell, cx| window.focus(&shell.focus, cx));
            cx.new(|cx| Root::new(shell, window, cx))
        })
        .expect("the window opens");
        log("window: opened");
        cx.activate(true);
    });
}
