// SPDX-License-Identifier: AGPL-3.0-only
//! Road 2: wry as a child of our own GPUI window (gpui-pre with our patches), placed from the pane's bounds each
//! frame. GPUI draws into the window under the page, so a popover shows only where the page is cut away: an X11
//! shape on Linux, a layer mask on the Mac.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::*;
use wry::dpi::{LogicalPosition, LogicalSize};

use crate::{log, rect};

pub const NAME: &str = "wry child of the gpui-pre window";

pub struct Page {
    webview: Rc<wry::WebView>,
    origin: Rc<Cell<Point<Pixels>>>,
    last: Option<Bounds<Pixels>>,
    overlays: Vec<Bounds<Pixels>>,
}

impl Page {
    pub fn new(url: &str, window: &mut Window, cx: &mut App) -> Entity<Self> {
        #[cfg(target_os = "linux")]
        linux::ensure_gtk(cx);
        let builder = wry::WebViewBuilder::new()
            .with_url(url)
            .with_ipc_handler(|request| log(format!("page: {}", request.body())))
            .with_on_page_load_handler(|event, url| {
                if let wry::PageLoadEvent::Finished = event {
                    log(format!("page: load finished {url}"));
                }
            });
        #[cfg(target_os = "linux")]
        let webview = builder.build_as_child(&linux::X11Parent::new(window)).expect("wry builds the webview");
        #[cfg(not(target_os = "linux"))]
        let webview = builder.build_as_child(&*window).expect("wry builds the webview");
        log(format!("page: created for {url}"));
        let origin = Rc::new(Cell::new(Point::default()));
        #[cfg(target_os = "linux")]
        linux::forward_input(&webview, origin.clone(), window, cx);
        cx.new(|_| Self { webview: Rc::new(webview), origin, last: None, overlays: Vec::new() })
    }

    pub fn load(&mut self, url: &str, _: &mut Context<Self>) {
        let _ = self.webview.load_url(url);
    }

    /// Cuts each overlay, in window coordinates, out of the page so GPUI's drawing under it shows.
    pub fn set_overlays(&mut self, overlays: Vec<Bounds<Pixels>>) {
        self.overlays = overlays;
        self.cut();
    }

    fn cut(&self) {
        let Some(pane) = self.last else { return };
        let holes: Vec<Bounds<Pixels>> = self
            .overlays
            .iter()
            .map(|o| o.intersect(&pane))
            .filter(|o| !o.is_empty())
            .map(|o| Bounds::new(o.origin - pane.origin, o.size))
            .collect();
        log(format!("page cut: {} hole(s) {}", holes.len(), holes.iter().map(|h| rect(*h)).collect::<Vec<_>>().join(" ")));
        #[cfg(target_os = "linux")]
        linux::cut(&self.webview, pane.size, &holes);
        #[cfg(target_os = "macos")]
        mac::cut(&self.webview, pane.size, &holes);
    }

    fn place(&mut self, bounds: Bounds<Pixels>) {
        if self.last == Some(bounds) {
            return;
        }
        self.last = Some(bounds);
        self.origin.set(bounds.origin);
        let _ = self.webview.set_bounds(wry::Rect {
            position: LogicalPosition::new(f64::from(bounds.origin.x), f64::from(bounds.origin.y)).into(),
            size: LogicalSize::new(f64::from(bounds.size.width), f64::from(bounds.size.height)).into(),
        });
        log(format!("pane bounds: {}", rect(bounds)));
        self.cut();
    }
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        div().relative().size_full().child(
            canvas(move |bounds, _, cx| this.update(cx, |page, _| page.place(bounds)), |_, _, _, _| {})
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
        )
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let _ = self.webview.set_visible(false);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gtk::glib::Propagation;
    use gtk::prelude::*;
    use std::time::Duration;
    use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle, WindowHandle, XlibWindowHandle};
    use wry::WebViewExtUnix as _;

    struct Gtk;
    impl Global for Gtk {}

    /// GTK on X11, its loop run from GPUI's between frames, since GPUI owns the main thread.
    pub fn ensure_gtk(cx: &mut App) {
        if cx.has_global::<Gtk>() {
            return;
        }
        gtk::gdk::set_allowed_backends("x11");
        gtk::init().expect("GTK starts on X11");
        cx.set_global(Gtk);
        // gpui-kit's webview polls every 16 ms; SPIKE_GTK_POLL_MS sets ours, to measure what the interval costs a scroll.
        let poll = std::env::var("SPIKE_GTK_POLL_MS").ok().and_then(|ms| ms.parse().ok()).unwrap_or(4);
        log(format!("gtk poll: every {poll} ms"));
        cx.spawn(async move |cx| loop {
            while gtk::events_pending() {
                gtk::main_iteration_do(false);
            }
            cx.background_executor().timer(Duration::from_millis(poll)).await;
        })
        .detach();
    }

    /// GPUI names its X11 window through XCB; wry takes it only as Xlib.
    pub struct X11Parent(XlibWindowHandle);

    impl X11Parent {
        pub fn new(window: &Window) -> Self {
            match HasWindowHandle::window_handle(window).expect("the window has a handle").as_raw() {
                RawWindowHandle::Xcb(xcb) => {
                    let mut xlib = XlibWindowHandle::new(xcb.window.get().into());
                    xlib.visual_id = xcb.visual_id.map_or(0, |id| id.get().into());
                    Self(xlib)
                }
                RawWindowHandle::Xlib(xlib) => Self(xlib),
                other => panic!("wry embeds only in an X11 window, and GPUI opened {other:?}"),
            }
        }
    }

    impl HasWindowHandle for X11Parent {
        fn window_handle(&self) -> Result<WindowHandle<'_>, wry::raw_window_handle::HandleError> {
            // SAFETY: the GPUI window outlives the webview built in it.
            Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Xlib(self.0)) })
        }
    }

    thread_local! {
        static X11: x11rb::rust_connection::RustConnection =
            x11rb::connect(None).expect("a second X11 connection for the page's shape").0;
    }

    /// Shapes the page's X11 window to its rectangle less the holes; GDK's shape call never reaches wry's container.
    pub fn cut(webview: &wry::WebView, size: Size<Pixels>, holes: &[Bounds<Pixels>]) {
        use gtk::cairo::{RectangleInt, Region};
        use x11rb::connection::Connection as _;
        use x11rb::protocol::shape::{self, ConnectionExt as _};
        use x11rb::protocol::xproto::{ClipOrdering, Rectangle};
        let Some(gdk) = webview.webview().toplevel().and_then(|top| top.window()) else { return };
        let Ok(x11) = gdk.downcast::<gdkx11::X11Window>() else { return };
        let int = |v: Pixels| f32::from(v).round() as i32;
        let region = Region::create_rectangle(&RectangleInt::new(0, 0, int(size.width), int(size.height)));
        for hole in holes {
            let _ = region.subtract_rectangle(&RectangleInt::new(
                int(hole.origin.x),
                int(hole.origin.y),
                int(hole.size.width),
                int(hole.size.height),
            ));
        }
        let rectangles: Vec<Rectangle> = (0..region.num_rectangles())
            .map(|i| region.rectangle(i))
            .map(|r| Rectangle { x: r.x() as i16, y: r.y() as i16, width: r.width() as u16, height: r.height() as u16 })
            .collect();
        X11.with(|conn| {
            let sent =
                conn.shape_rectangles(shape::SO::SET, shape::SK::BOUNDING, ClipOrdering::UNSORTED, x11.xid() as u32, 0, 0, &rectangles);
            let flushed = conn.flush();
            log(format!(
                "page cut: shaped X11 window {:#x} with {} rectangle(s), {}",
                x11.xid(),
                rectangles.len(),
                if sent.is_ok() && flushed.is_ok() { "sent" } else { "failed" }
            ));
        });
    }

    /// Replays a press on the page into GPUI, and with SPIKE_FORWARD_KEYS=1 its chords and Escape, which X11 gives WebKit.
    pub fn forward_input(webview: &wry::WebView, origin: Rc<Cell<Point<Pixels>>>, window: &Window, cx: &App) {
        let handle = window.window_handle();
        let async_cx = cx.to_async();
        webview.webview().connect_button_press_event(move |_, event| {
            let (x, y) = event.position();
            let position = origin.get() + point(px(x as f32), px(y as f32));
            async_cx
                .spawn(async move |cx| {
                    handle.update(cx, |_, window, cx| {
                        log("page: press forwarded to GPUI");
                        let modifiers = Modifiers::default();
                        window.dispatch_event(
                            PlatformInput::MouseDown(MouseDownEvent {
                                button: MouseButton::Left,
                                position,
                                modifiers,
                                click_count: 1,
                                first_mouse: false,
                            }),
                            cx,
                        );
                        window.dispatch_event(
                            PlatformInput::MouseUp(MouseUpEvent { button: MouseButton::Left, position, modifiers, click_count: 1 }),
                            cx,
                        );
                    })
                })
                .detach();
            Propagation::Proceed
        });
        if std::env::var_os("SPIKE_FORWARD_KEYS").is_none() {
            return;
        }
        let async_cx = cx.to_async();
        webview.webview().connect_key_press_event(move |_, event| {
            let state = event.state();
            let ctrl = state.contains(gtk::gdk::ModifierType::CONTROL_MASK);
            let alt = state.contains(gtk::gdk::ModifierType::MOD1_MASK);
            let shift = state.contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let name = event.keyval().to_lower().name().map(|n| n.to_lowercase()).unwrap_or_default();
            if !(ctrl || alt || name == "escape") {
                return Propagation::Proceed;
            }
            let chord = format!(
                "{}{}{}{}",
                if ctrl { "ctrl-" } else { "" },
                if alt { "alt-" } else { "" },
                if shift { "shift-" } else { "" },
                name
            );
            let Ok(keystroke) = Keystroke::parse(&chord) else { return Propagation::Proceed };
            async_cx
                .spawn(async move |cx| {
                    handle.update(cx, |_, window, cx| {
                        log(format!("page: key {chord} forwarded to GPUI"));
                        window.dispatch_event(
                            PlatformInput::KeyDown(KeyDownEvent { keystroke, is_held: false, prefer_character_input: false }),
                            cx,
                        );
                    })
                })
                .detach();
            Propagation::Stop
        });
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};
    use wry::WebViewExtMacOS as _;

    #[link(name = "QuartzCore", kind = "framework")]
    extern "C" {
        static kCAFillRuleEvenOdd: id;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPathCreateMutable() -> *mut std::ffi::c_void;
        fn CGPathAddRect(path: *mut std::ffi::c_void, transform: *const std::ffi::c_void, rect: NSRect);
        fn CGPathRelease(path: *mut std::ffi::c_void);
    }

    /// Masks WKWebView's layer to its bounds less each hole (even-odd), so GPUI's Metal layer under it shows there.
    pub fn cut(webview: &wry::WebView, size: Size<Pixels>, holes: &[Bounds<Pixels>]) {
        let view = webview.webview();
        let view = objc2::rc::Retained::as_ptr(&view) as id;
        unsafe {
            let _: () = msg_send![view, setWantsLayer: YES];
            let layer: id = msg_send![view, layer];
            if holes.is_empty() {
                let _: () = msg_send![layer, setMask: nil];
                return;
            }
            let (w, h) = (f64::from(size.width), f64::from(size.height));
            let path = CGPathCreateMutable();
            CGPathAddRect(path, std::ptr::null(), NSRect::new(NSPoint::new(0., 0.), NSSize::new(w, h)));
            let flipped: bool = msg_send![view, isFlipped];
            for hole in holes {
                let (x, hw, hh) = (f64::from(hole.origin.x), f64::from(hole.size.width), f64::from(hole.size.height));
                let y = if flipped { f64::from(hole.origin.y) } else { h - f64::from(hole.origin.y) - hh };
                CGPathAddRect(path, std::ptr::null(), NSRect::new(NSPoint::new(x, y), NSSize::new(hw, hh)));
            }
            let mask: id = msg_send![class!(CAShapeLayer), layer];
            let _: () = msg_send![mask, setFrame: NSRect::new(NSPoint::new(0., 0.), NSSize::new(w, h))];
            let _: () = msg_send![mask, setPath: path];
            let _: () = msg_send![mask, setFillRule: kCAFillRuleEvenOdd];
            let _: () = msg_send![layer, setMask: mask];
            CGPathRelease(path);
        }
    }
}
