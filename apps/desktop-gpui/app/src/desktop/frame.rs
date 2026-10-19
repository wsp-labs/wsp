// SPDX-License-Identifier: AGPL-3.0-only
//! The app window's frame as `desktop:window.ts` picks it: on a Mac the hidden title bar over the header; on the Linux
//! desktops that draw title bars, none, with the window's controls drawn over the header's right end on its ground
//! and ink; on every other Linux desktop the desktop's own frame. Where the header row is the frame it drags the
//! window. Also the raise every way back to the window shares (`desktop:main.ts` `raiseWindow`).

use std::cell::Cell;
use std::sync::OnceLock;

use gpui_kit::assets::IconName;
use gpui_kit::base::{Root, RootPlugin};
use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, App, Context, Decorations, Div, MouseButton, Pixels, Stateful, Window, WindowControlArea, WindowControls,
    WindowDecorations,
};

use crate::screens::app_shell::frame::TOPBAR;
use crate::ui::ActivePalette;

/// The Linux desktops that draw a title bar over a window, as XDG_CURRENT_DESKTOP names them (`TITLE_BAR_DESKTOPS`).
pub const TITLE_BAR_DESKTOPS: [&str; 13] =
    ["gnome", "kde", "xfce", "x-cinnamon", "cinnamon", "mate", "unity", "budgie", "pantheon", "lxqt", "lxde", "deepin", "cosmic"];

/// `SESSION_DESKTOPS`: the session files of the listed desktops whose names are not the desktop's own.
const SESSION_DESKTOPS: [(&str, &str); 10] = [
    ("plasma", "KDE"),
    ("plasmax11", "KDE"),
    ("plasmawayland", "KDE"),
    ("ubuntu", "GNOME"),
    ("ubuntu-xorg", "GNOME"),
    ("ubuntu-wayland", "GNOME"),
    ("gnome-xorg", "GNOME"),
    ("gnome-wayland", "GNOME"),
    ("gnome-classic", "GNOME"),
    ("budgie-desktop", "Budgie"),
];

/// One window control's width; the row's height is the header's.
pub const CONTROL_WIDTH: Pixels = px(40.);

/// `desktopOf`: XDG_CURRENT_DESKTOP's names, else the desktop the started session file belongs to. An empty value is
/// no value, as JavaScript's falsy string is.
pub fn desktop_of(current: Option<&str>, session: Option<&str>) -> Option<String> {
    if let Some(current) = current.filter(|c| !c.is_empty()) {
        return Some(current.to_owned());
    }
    let session = session.filter(|s| !s.is_empty())?;
    let lower = session.to_lowercase();
    Some(SESSION_DESKTOPS.iter().find(|(file, _)| *file == lower).map_or(session, |(_, desktop)| desktop).to_owned())
}

/// `drawsTitleBar`: whether any of the colon separated names is a listed desktop.
pub fn draws_title_bar(desktop: Option<&str>) -> bool {
    desktop.unwrap_or("").split(':').any(|name| TITLE_BAR_DESKTOPS.contains(&name.trim().to_lowercase().as_str()))
}

/// Who draws the app window's frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    /// The system's: a Mac's hidden inset title bar, or a Linux desktop's own frame.
    System,
    /// Linux on a listed desktop: no title bar; the app draws the controls over its header.
    Drawn,
}

/// `frameOf` for this build's platform and the desktop the session names.
pub fn frame_for(linux: bool, desktop: Option<&str>) -> Frame {
    if linux && draws_title_bar(desktop) {
        Frame::Drawn
    } else {
        Frame::System
    }
}

static FRAME: OnceLock<Frame> = OnceLock::new();

/// Reads the desktop off this launch's environment, once, before the first window.
pub fn choose() -> Frame {
    *FRAME.get_or_init(|| {
        let var = |name| std::env::var(name).ok();
        frame_for(
            cfg!(target_os = "linux"),
            desktop_of(var("XDG_CURRENT_DESKTOP").as_deref(), var("XDG_SESSION_DESKTOP").as_deref()).as_deref(),
        )
    })
}

/// The frame chosen at launch; the system's where none was.
pub fn chosen() -> Frame {
    FRAME.get().copied().unwrap_or(Frame::System)
}

/// What the app window asks the platform for: no system frame where the app draws it.
pub fn decorations() -> Option<WindowDecorations> {
    (chosen() == Frame::Drawn).then_some(WindowDecorations::Client)
}

/// Whether a window is drawn without the system's frame now; a compositor may still give it one.
fn drawn_here(window: &Window) -> bool {
    cfg!(target_os = "linux") && matches!(window.window_decorations(), Decorations::Client { .. })
}

/// The room the header's right end leaves for the controls, none where the system draws them.
pub fn controls_room(window: &Window) -> Pixels {
    if drawn_here(window) {
        CONTROL_WIDTH * controls(window.window_controls()).len() as f32
    } else {
        px(0.)
    }
}

/// The room at the right end of the row the controls stand over, so what the row holds ends before them.
#[derive(IntoElement)]
pub struct ControlsRoom;

impl RenderOnce for ControlsRoom {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div().flex_none().w(controls_room(window))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

/// Minimize and maximize where the window manager honours them; close always.
fn controls(held: WindowControls) -> Vec<Control> {
    [(held.minimize, Control::Minimize), (held.maximize, Control::Maximize), (true, Control::Close)]
        .into_iter()
        .filter_map(|(on, control)| on.then_some(control))
        .collect()
}

thread_local! {
    /// A press on a frame row, which a move then turns into the window's move.
    static PRESSED: Cell<bool> = const { Cell::new(false) };
}

/// Whether a header row is the window's frame here: a Mac's, and Linux's where the app draws the frame.
pub fn drags() -> bool {
    cfg!(target_os = "macos") || chosen() == Frame::Drawn
}

/// A row that starts a column, made the window's drag region where it is the frame (`drag-region`).
pub fn drag(row: Stateful<Div>) -> Stateful<Div> {
    if !drags() {
        return row;
    }
    drag_region(row)
}

/// The row as the window's drag region on any platform: the move starts once a press on the row's own ground moves.
/// A press a button or a focusable child took (GPUI's default prevented, as both do) starts none, so its click stays.
pub fn drag_region(row: Stateful<Div>) -> Stateful<Div> {
    row.window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |_, window, _| PRESSED.set(!window.default_prevented()))
        .on_mouse_up(MouseButton::Left, |_, _, _| PRESSED.set(false))
        .on_mouse_move(|e, window, _| {
            if PRESSED.replace(false) && e.dragging() {
                window.start_window_move();
            }
        })
}

/// The window's controls over the header's right end, and its rem at the run's zoom.
pub struct WindowFrame;

impl WindowFrame {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        WindowFrame
    }
}

/// Puts the frame on every window opened after it; call once, after the app's other root plugins, so the controls
/// stand over their overlays as the system's would.
pub fn init(cx: &mut App) {
    Root::register_plugin::<WindowFrame>(cx, WindowFrame::new);
}

impl Render for WindowFrame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !drawn_here(window) {
            return div().into_any_element();
        }
        window_controls(window.window_controls(), window.is_maximized(), cx).into_any_element()
    }
}

/// The controls at the window's top right, on the header's ground in the ink its toggles take (the page's
/// `--titlebar-ground` and `--titlebar-ink`): those the window manager honours, maximize showing restore once maximized.
pub fn window_controls(held: WindowControls, maximized: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let (ground, ink, hover) = (p.background, p.foreground.opacity(0.8), p.accent);
    let (danger, on_danger) = (p.destructive, p.on_destructive);
    h_flex().id("window-controls").debug_selector(|| "window-controls".into()).absolute().top_0().right_0().h(TOPBAR).bg(ground).children(
        controls(held).into_iter().map(move |control| {
            let (id, icon) = match control {
                Control::Minimize => ("window-minimize", IconName::WindowMinimize),
                Control::Maximize if maximized => ("window-maximize", IconName::WindowRestore),
                Control::Maximize => ("window-maximize", IconName::WindowMaximize),
                Control::Close => ("window-close", IconName::WindowClose),
            };
            let close = control == Control::Close;
            div()
                .id(id)
                .debug_selector(move || id.into())
                .occlude()
                .flex()
                .flex_none()
                .w(CONTROL_WIDTH)
                .h_full()
                .items_center()
                .justify_center()
                .text_color(ink)
                .hover(move |s| if close { s.bg(danger).text_color(on_danger) } else { s.bg(hover) })
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match control {
                        Control::Minimize => window.minimize_window(),
                        Control::Maximize => window.zoom_window(),
                        Control::Close => window.remove_window(),
                    }
                })
                .child(Icon::new(icon).small())
        }),
    )
}

/// gpui-kit's own root plugin, before this one, sets the theme's rem and puts a window the app frames in its resize
/// border; this one scales that rem by the run's zoom.
impl RootPlugin for WindowFrame {
    fn prepare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.set_rem_size(cx.theme().font_size * super::zoom::level(cx).factor());
    }
}

/// `raiseWindow`: a minimised window restored, the app brought forward over whatever the person is in, then the
/// window shown and focused.
pub fn raise(window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "macos")]
    restore(window);
    cx.activate(true);
    window.activate_window();
}

/// AppKit's deminiaturize; X11's `_NET_ACTIVE_WINDOW`, which `activate_window` sends, restores an iconic window itself.
#[cfg(target_os = "macos")]
fn restore(window: &Window) {
    use cocoa::base::{id, nil, BOOL, NO};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
    // SAFETY: the handle's view is GPUI's own, live in its window, and this runs on the main thread.
    unsafe {
        let view = handle.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![view, window];
        if ns_window == nil {
            return;
        }
        let minimised: BOOL = msg_send![ns_window, isMiniaturized];
        if minimised != NO {
            let _: () = msg_send![ns_window, deminiaturize: nil];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_is_read_as_window_ts_reads_it() {
        assert_eq!(desktop_of(Some("ubuntu:GNOME"), Some("plasma")).as_deref(), Some("ubuntu:GNOME"));
        assert_eq!(desktop_of(Some(""), Some("plasmawayland")).as_deref(), Some("KDE"));
        assert_eq!(desktop_of(None, Some("Ubuntu-Xorg")).as_deref(), Some("GNOME"));
        assert_eq!(desktop_of(None, Some("budgie-desktop")).as_deref(), Some("Budgie"));
        assert_eq!(desktop_of(None, Some("sway")).as_deref(), Some("sway"));
        assert_eq!(desktop_of(None, Some("")), None);
        assert_eq!(desktop_of(None, None), None);
    }

    #[test]
    fn each_listed_desktop_gets_the_drawn_frame_and_the_rest_their_own() {
        for desktop in [
            "GNOME",
            "ubuntu:GNOME",
            "KDE",
            "XFCE",
            "X-Cinnamon",
            "MATE",
            "Unity",
            "Budgie:GNOME",
            "Pantheon",
            "LXQt",
            "LXDE",
            "Deepin",
            "COSMIC",
        ] {
            assert_eq!(frame_for(true, Some(desktop)), Frame::Drawn, "{desktop}");
            assert_eq!(frame_for(false, Some(desktop)), Frame::System, "{desktop} off Linux");
        }
        for desktop in [Some("sway"), Some("Hyprland"), Some("i3"), Some("openbox"), Some(""), None] {
            assert_eq!(frame_for(true, desktop), Frame::System, "{desktop:?}");
        }
        let session = |file| frame_for(true, desktop_of(None, Some(file)).as_deref());
        assert_eq!(session("plasmax11"), Frame::Drawn);
        assert_eq!(session("gnome-classic"), Frame::Drawn);
        assert_eq!(session("openbox"), Frame::System);
    }
}
