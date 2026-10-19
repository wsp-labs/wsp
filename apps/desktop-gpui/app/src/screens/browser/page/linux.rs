// SPDX-License-Identifier: AGPL-3.0-only
//! The page on Linux: WebKitGTK in an X11 window wry makes a child of GPUI's. X11 hands a key to the window under the
//! pointer when that window is inside the focused one, so the page's window passes on every key it gets while the
//! page does not hold the keys, and the app's chords while it does, as the X events GPUI would have had.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::channel::mpsc::UnboundedSender;
use gpui_kit::{point, px, App, Bounds, Global, Modifiers, Pixels, Window};
use gtk::glib::translate::IntoGlib as _;
use gtk::glib::{self, Propagation};
use gtk::prelude::*;
use webkit2gtk::{
    PermissionRequestExt as _, PolicyDecisionExt as _, ResponsePolicyDecisionExt as _, SettingsExt as _, WebViewExt as _,
    WebsiteDataManagerExtManual as _,
};
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};
use wry::{WebViewBuilder, WebViewExtUnix as _};
use x11rb::connection::Connection as _;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self, ClipOrdering, ConnectionExt as _, EventMask, InputFocus, Rectangle};
use x11rb::rust_connection::RustConnection;

use super::{device, device_holes, glib_loop, keys, visible, DataDir, Raw, Rect, INIT_SCRIPT};
use crate::screens::app_shell::menu::Os;

/// GTK, started once for the process on X11.
struct Gtk {
    /// The scale GTK draws at: the window's whole part, the rest the page's zoom.
    scale: i32,
}

impl Global for Gtk {}

thread_local! {
    /// The pages' own connection to the X server, for what GDK does not reach: the shape, the focus, a key passed on.
    static X11: Option<RustConnection> = x11rb::connect(None).ok().map(|(conn, _)| conn);
    /// One store for every page's cookies and storage, which each web view holds a reference to.
    static CONTEXT: RefCell<Option<wry::WebContext>> = const { RefCell::new(None) };
    /// The pages that load once the store's service workers are cleared, or None once they are.
    static WAITING: RefCell<Option<Vec<Waiting>>> = const { RefCell::new(None) };
}

/// A page and the address it loads once the store's service workers are cleared.
type Waiting = (glib::WeakRef<webkit2gtk::WebView>, String);

/// Makes the store again on the next page, clearing its service workers as the app's start does.
pub fn remake_context() {
    CONTEXT.with(|context| context.borrow_mut().take());
    WAITING.with(|waiting| waiting.borrow_mut().take());
}

/// The store's service workers are gone: the pages waiting on them load, unless one was sent elsewhere meanwhile.
fn cleared() {
    for (view, url) in WAITING.with(|waiting| waiting.borrow_mut().take()).unwrap_or_default() {
        if let Some(view) = view.upgrade().filter(|v| v.uri().is_none()) {
            view.load_uri(&url);
        }
    }
    glib_loop::nudge();
}

fn ensure_gtk(window: &Window, cx: &mut App) -> Result<i32, String> {
    if let Some(gtk) = cx.try_global::<Gtk>() {
        return Ok(gtk.scale);
    }
    gtk::gdk::set_allowed_backends("x11");
    gtk::init().map_err(|e| format!("GTK did not start on X11: {e}"))?;
    let scale = (window.scale_factor().floor() as i32).max(1);
    if let Some(display) = gtk::gdk::Display::default().and_then(|d| d.downcast::<gdkx11::X11Display>().ok()) {
        display.set_window_scale(scale);
    }
    glib_loop::start(cx);
    cx.set_global(Gtk { scale });
    Ok(scale)
}

/// GPUI names its X11 window through XCB; wry takes it only as Xlib.
struct Parent(XlibWindowHandle);

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, wry::raw_window_handle::HandleError> {
        // SAFETY: wry only copies the id; the store drops a window's pages before GPUI destroys the X window.
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(self.0)) })
    }
}

fn parent(window: &Window) -> Result<(Parent, u32), String> {
    match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xcb(xcb)) => {
            let mut xlib = XlibWindowHandle::new(xcb.window.get().into());
            xlib.visual_id = xcb.visual_id.map_or(0, |id| id.get().into());
            Ok((Parent(xlib), xcb.window.get()))
        }
        Ok(RawWindowHandle::Xlib(xlib)) => Ok((Parent(xlib), xlib.window as u32)),
        _ => Err(super::WAYLAND_LINE.to_owned()),
    }
}

pub struct Native {
    webview: wry::WebView,
    /// The X11 window wry made for the page, a child of GPUI's.
    container: u32,
    /// GPUI's own X11 window.
    toplevel: u32,
    gtk_scale: i32,
    /// The window's scale, which a press on the page is read back at.
    scale: Rc<Cell<f32>>,
    /// The page holds the keys: a press on it gave them, GPUI's focus leaving the pane takes them back.
    holds_keys: Rc<Cell<bool>>,
    shown: Cell<bool>,
}

impl Native {
    pub fn new(url: &str, dir: DataDir, tx: UnboundedSender<Raw>, window: &mut Window, cx: &mut App) -> Result<Native, String> {
        let gtk_scale = ensure_gtk(window, cx)?;
        let (parent, toplevel) = parent(window)?;
        let (title, open, nav) = (tx.clone(), tx.clone(), tx.clone());
        let mut made = false;
        let webview = CONTEXT.with(|context| {
            let mut context = context.borrow_mut();
            let context = context.get_or_insert_with(|| {
                made = true;
                WAITING.with(|waiting| *waiting.borrow_mut() = Some(Vec::new()));
                wry::WebContext::new(Some(dir.path.clone()))
            });
            WebViewBuilder::new_with_web_context(context)
                .with_focused(false)
                .with_visible(false)
                .with_initialization_script(INIT_SCRIPT)
                // wry's default saves every download unasked; no page here saves a file.
                .with_download_started_handler(|_, _| false)
                .with_document_title_changed_handler(move |t| _ = title.unbounded_send(Raw::Titled(t)))
                .with_new_window_req_handler(move |url, _| {
                    _ = open.unbounded_send(Raw::NewWindow(url));
                    wry::NewWindowResponse::Deny
                })
                .with_navigation_handler(move |url| {
                    if super::stays(&url) {
                        return true;
                    }
                    _ = nav.unbounded_send(Raw::Leave(url));
                    false
                })
                .build_as_child(&parent)
                .map_err(|e| format!("the page could not be made: {e}"))
        });
        let built = webview.and_then(|webview| {
            let container = webview.webview().toplevel().and_then(|t| t.window()).and_then(|w| w.downcast::<gdkx11::X11Window>().ok());
            container.map(|c| (webview, c)).ok_or_else(|| "the page's X11 window was not made".to_owned())
        });
        // A store whose first page failed never cleared its service workers, and would hold every later page waiting.
        let (webview, container) = built.inspect_err(|_| {
            if made {
                remake_context();
            }
        })?;
        let view = webview.webview();
        if made {
            clear_workers(&view);
        }
        let waiting = WAITING.with(|waiting| waiting.borrow_mut().as_mut().map(|w| w.push((view.downgrade(), url.to_owned()))).is_some());
        if !waiting {
            view.load_uri(url);
        }
        let native = Native {
            webview,
            container: container.xid() as u32,
            toplevel,
            gtk_scale,
            scale: Rc::new(Cell::new(window.scale_factor())),
            holds_keys: Rc::default(),
            shown: Cell::new(false),
        };
        native.wire(tx);
        glib_loop::nudge();
        Ok(native)
    }

    fn wire(&self, tx: UnboundedSender<Raw>) {
        let view = self.webview.webview();
        let gtk_scale = self.gtk_scale as f32;
        // The address as WebKit has it, a move in place included; wry's IPC handler would abort the app on a page whose
        // address is longer than the `http` crate takes, so no page message is heard on Linux.
        let moved = tx.clone();
        view.connect_uri_notify(move |_| _ = moved.unbounded_send(Raw::MovedInPlace));
        let pressed = tx.clone();
        let (holds, scale) = (self.holds_keys.clone(), self.scale.clone());
        view.connect_button_press_event(move |_, event| {
            let (x, y) = event.position();
            let at = point(px(x as f32 * gtk_scale / scale.get()), px(y as f32 * gtk_scale / scale.get()));
            holds.set(true);
            _ = pressed.unbounded_send(Raw::Pressed(at));
            Propagation::Proceed
        });
        // Loading from the moment a load is asked, as Electron's did-start-loading; a failed or stopped load finishes.
        let load = tx.clone();
        view.connect_load_changed(move |_, event| match event {
            webkit2gtk::LoadEvent::Started => _ = load.unbounded_send(Raw::Load(true)),
            webkit2gtk::LoadEvent::Finished => _ = load.unbounded_send(Raw::Load(false)),
            _ => {}
        });
        // Today's guest opens its own windows, as `allowpopups` lets it; each becomes a tab.
        if let Some(settings) = webkit2gtk::WebViewExt::settings(&view) {
            settings.set_javascript_can_open_windows_automatically(true);
        }
        // A load that fails leaves the page blank under the down bar, as today's guest does, not WebKit's error page.
        view.connect_load_failed(|_, _, _, _| true);
        // Today's guest is granted pointer lock and nothing that reaches the camera, microphone, location or notifications.
        view.connect_permission_request(|_, request| {
            if request.is::<webkit2gtk::PointerLockPermissionRequest>() {
                request.allow();
            } else {
                request.deny();
            }
            true
        });
        // A response this web view cannot show is a download, which no page here saves on its own.
        view.connect_decide_policy(|_, decision, kind| {
            let Some(response) =
                decision.downcast_ref::<webkit2gtk::ResponsePolicyDecision>().filter(|_| kind == webkit2gtk::PolicyDecisionType::Response)
            else {
                return false;
            };
            if response.is_mime_type_supported() {
                return false;
            }
            decision.ignore();
            true
        });
        // Today's guest has no menu on a right press.
        view.connect_context_menu(|_, _, _, _| true);
        // wry destroys the web view when its page calls `window.close()`, which would leave a dead tab; today's guest
        // ignores the call.
        disconnect(&view, "close");
        // wry's message handler holds the web view, which holds the handler: no page would ever be freed, and each
        // would keep a descriptor and its WebKit side. No page message is heard on Linux.
        if let Some(manager) = view.user_content_manager() {
            disconnect(&manager, "script-message-received");
        }
        let gone = tx.clone();
        view.connect_web_process_terminated(move |view, reason| {
            let url = view.uri().map(|u| u.to_string()).unwrap_or_default();
            _ = gone.unbounded_send(Raw::Gone(format!("renderer gone: {reason:?} at {url}")));
        });
        let chords = keys::app_chords(Os::Linux);
        let (holds, toplevel) = (self.holds_keys.clone(), self.toplevel);
        let pass = move |event: &gtk::gdk::EventKey, press: bool| -> Propagation {
            if holds.get() && !keys::is_app_chord(&chords, &key_name(event), &modifiers(event.state())) {
                return Propagation::Proceed;
            }
            pass_key(event, press, toplevel);
            Propagation::Stop
        };
        let pass = Rc::new(pass);
        if let Some(top) = view.toplevel() {
            let on_press = pass.clone();
            top.connect_key_press_event(move |_, event| on_press(event, true));
            top.connect_key_release_event(move |_, event| pass(event, false));
        }
    }

    pub fn show(&self, at: Bounds<Pixels>, holes: &[Bounds<Pixels>], scale: f32) {
        // Shown before it is sized: GTK allocates no size to a hidden child, and the web view would stay 2x2.
        if !self.shown.replace(true) {
            _ = self.webview.set_visible(true);
        }
        let (x, y, w, h) = device(at, scale);
        _ = self.webview.set_bounds(wry::Rect {
            position: PhysicalPosition::new(x, y).into(),
            size: PhysicalSize::new(w.max(1) as u32, h.max(1) as u32).into(),
        });
        self.cut(w, h, &device_holes(at, holes, scale));
        glib_loop::nudge();
    }

    pub fn hide(&self) {
        if self.shown.replace(false) {
            _ = self.webview.set_visible(false);
            glib_loop::nudge();
        }
    }

    /// Shapes the page's window to its rectangle less the holes; GDK's own shape call never reaches wry's container.
    fn cut(&self, w: i32, h: i32, holes: &[Rect]) {
        let rectangles: Vec<Rectangle> = visible((0, 0, w, h), holes)
            .into_iter()
            .map(|(x, y, w, h)| Rectangle { x: x as i16, y: y as i16, width: w as u16, height: h as u16 })
            .collect();
        X11.with(|conn| {
            let Some(conn) = conn else { return };
            _ = conn.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::UNSORTED, self.container, 0, 0, &rectangles);
            _ = conn.flush();
        });
    }

    pub fn focused(&self) -> bool {
        self.holds_keys.get()
    }

    /// The page holds the keys: X's focus goes to its window, so a key reaches it wherever the pointer is.
    pub fn take_focus(&self) {
        self.holds_keys.set(true);
        self.webview.webview().grab_focus();
        set_focus(self.container);
        glib_loop::nudge();
    }

    /// The window holds the keys again; X's focus moves back only from the page's own window, never from another app's.
    pub fn release_focus(&self) {
        if self.holds_keys.replace(false) && input_focus() == Some(self.container) {
            set_focus(self.toplevel);
        }
    }

    /// Goes now, its X requests sent before the window it is a child of can be destroyed.
    pub fn close(self) {
        drop(self);
        if let Some(display) = gtk::gdk::Display::default() {
            display.sync();
        }
    }

    /// Which window X gives the keys to: the page's, GPUI's, or another; for the checks.
    pub fn keys_held_by(&self) -> &'static str {
        match input_focus() {
            Some(w) if w == self.container => "the page's window",
            Some(w) if w == self.toplevel => "GPUI's window",
            _ => "another window",
        }
    }

    pub fn url(&self) -> Option<String> {
        self.webview.url().ok().filter(|u| !u.is_empty())
    }

    pub fn load(&self, url: &str) {
        _ = self.webview.load_url(url);
        glib_loop::nudge();
    }

    pub fn reload(&self) {
        _ = self.webview.reload();
        glib_loop::nudge();
    }

    pub fn stop(&self) {
        self.webview.webview().stop_loading();
        glib_loop::nudge();
    }

    pub fn back(&self) {
        self.webview.webview().go_back();
        glib_loop::nudge();
    }

    pub fn forward(&self) {
        self.webview.webview().go_forward();
        glib_loop::nudge();
    }

    pub fn can_go_back(&self) -> bool {
        self.webview.webview().can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.webview.webview().can_go_forward()
    }

    /// GTK draws at the window's whole scale; the page's zoom carries the rest, so a 1.5 window shows CSS pixels at 1.5.
    pub fn set_zoom(&self, zoom: f32, scale: f32) {
        let rest = f64::from(scale) / f64::from(self.gtk_scale);
        self.scale.set(scale);
        _ = self.webview.zoom(f64::from(zoom) * rest);
        glib_loop::nudge();
    }

    pub fn run_script(&self, script: &str) {
        _ = self.webview.evaluate_script(script);
        glib_loop::nudge();
    }

    /// The size GTK gave the web view itself, in device pixels; for the checks.
    pub fn drawn_size(&self) -> Option<(i32, i32)> {
        let view = self.webview.webview();
        Some((view.allocated_width() * self.gtk_scale, view.allocated_height() * self.gtk_scale))
    }

    /// Where the page's window stands inside GPUI's, in device pixels; for the checks.
    pub fn geometry(&self) -> Option<(i32, i32, i32, i32)> {
        X11.with(|conn| {
            let g = conn.as_ref()?.get_geometry(self.container).ok()?.reply().ok()?;
            Some((i32::from(g.x), i32::from(g.y), i32::from(g.width), i32::from(g.height)))
        })
    }

    /// The page's shape as the X server holds it, in device pixels relative to the page; for the checks.
    pub fn shape(&self) -> Option<Vec<(i32, i32, i32, i32)>> {
        X11.with(|conn| {
            let reply = conn.as_ref()?.shape_get_rectangles(self.container, shape::SK::BOUNDING).ok()?.reply().ok()?;
            Some(reply.rectangles.iter().map(|r| (i32::from(r.x), i32::from(r.y), i32::from(r.width), i32::from(r.height))).collect())
        })
    }
}

/// A worker one preview registered never answers the next server on the same port: the store starts with none, as
/// today's session cancels every worker's script. The store's pages wait for it.
fn clear_workers(view: &webkit2gtk::WebView) {
    let Some(manager) = view.website_data_manager() else {
        cleared();
        return;
    };
    manager.clear(webkit2gtk::WebsiteDataTypes::SERVICE_WORKER_REGISTRATIONS, glib::TimeSpan(0), None::<&gtk::gio::Cancellable>, |done| {
        if let Err(e) = done {
            eprintln!("desktop-gpui: browser: the store's service workers were not cleared: {e}");
        }
        cleared();
    });
}

/// Disconnects every handler of `object`'s `signal`, none of which is this pane's.
fn disconnect(object: &impl IsA<glib::Object>, signal: &str) {
    let Some(id) = glib::subclass::signal::SignalId::lookup(signal, object.type_()) else { return };
    // SAFETY: the object is live, and this only disconnects the handlers on its own signal.
    unsafe {
        glib::gobject_ffi::g_signal_handlers_disconnect_matched(
            object.as_ptr().cast(),
            glib::gobject_ffi::G_SIGNAL_MATCH_ID,
            id.into_glib(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
    }
}

fn input_focus() -> Option<u32> {
    X11.with(|conn| Some(conn.as_ref()?.get_input_focus().ok()?.reply().ok()?.focus))
}

fn set_focus(window: u32) {
    X11.with(|conn| {
        let Some(conn) = conn else { return };
        _ = conn.set_input_focus(InputFocus::PARENT, window, x11rb::CURRENT_TIME);
        _ = conn.flush();
    });
}

/// The key as the X server gave it to the page, sent on to GPUI's window, which reads it as its own.
fn pass_key(event: &gtk::gdk::EventKey, press: bool, toplevel: u32) {
    X11.with(|conn| {
        let Some(conn) = conn else { return };
        let Some(root) = conn.setup().roots.first().map(|s| s.root) else { return };
        let key = xproto::KeyPressEvent {
            response_type: if press { xproto::KEY_PRESS_EVENT } else { xproto::KEY_RELEASE_EVENT },
            detail: event.hardware_keycode() as u8,
            sequence: 0,
            time: event.time(),
            root,
            event: toplevel,
            child: x11rb::NONE,
            root_x: 0,
            root_y: 0,
            event_x: 0,
            event_y: 0,
            state: xproto::KeyButMask::from((event.state().bits() & 0xffff) as u16),
            same_screen: true,
        };
        let mask = if press { EventMask::KEY_PRESS } else { EventMask::KEY_RELEASE };
        _ = conn.send_event(false, toplevel, mask, key);
        _ = conn.flush();
    });
}

fn key_name(event: &gtk::gdk::EventKey) -> String {
    event.keyval().to_lower().name().map(|n| n.to_lowercase()).unwrap_or_default()
}

fn modifiers(state: gtk::gdk::ModifierType) -> Modifiers {
    use gtk::gdk::ModifierType as M;
    Modifiers {
        control: state.contains(M::CONTROL_MASK),
        alt: state.contains(M::MOD1_MASK),
        shift: state.contains(M::SHIFT_MASK),
        platform: state.contains(M::SUPER_MASK) || state.contains(M::MOD4_MASK),
        function: false,
    }
}
