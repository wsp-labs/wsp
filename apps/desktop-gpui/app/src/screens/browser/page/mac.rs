// SPDX-License-Identifier: AGPL-3.0-only
//! The page on the Mac: WKWebView, which wry adds as a subview of GPUI's view, so it draws over GPUI's Metal layer.
//! Its layer is masked to its rectangle less the holes, its `hitTest:` passes a point in a hole to GPUI's view under
//! it, a press on it is replayed into GPUI, and while it is first responder GPUI's `performKeyEquivalent:` keeps only
//! the app's chords, so every other key reaches the page.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::path::PathBuf;

use cocoa::base::{id, nil, BOOL, NO, YES};
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use futures::channel::mpsc::UnboundedSender;
use gpui_kit::{point, px, App, Bounds, Keystroke, Modifiers, Pixels, Window};
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{WebViewBuilder, WebViewBuilderExtDarwin as _, WebViewExtMacOS as _};

use super::{keys, Raw, INIT_SCRIPT};
use crate::screens::app_shell::menu::Os;

type Method = c_void;
type Imp = *const c_void;

#[link(name = "objc")]
extern "C" {
    fn class_getInstanceMethod(cls: *const Class, sel: Sel) -> *mut Method;
    fn method_getImplementation(method: *mut Method) -> Imp;
    fn method_setImplementation(method: *mut Method, imp: Imp) -> Imp;
    fn method_getTypeEncoding(method: *mut Method) -> *const c_char;
    fn class_addMethod(cls: *const Class, name: Sel, imp: Imp, types: *const c_char) -> BOOL;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPathCreateMutable() -> *mut c_void;
    fn CGPathAddRect(path: *mut c_void, transform: *const c_void, rect: NSRect);
    fn CGPathRelease(path: *const c_void);
    fn CGPathContainsPoint(path: *const c_void, transform: *const c_void, point: NSPoint, eo_fill: bool) -> bool;
}

/// What each page's view needs inside AppKit's calls: its holes in its own top-left points, and where its presses go.
struct Hooks {
    holes: Vec<Bounds<Pixels>>,
    tx: UnboundedSender<Raw>,
}

thread_local! {
    static HOOKS: RefCell<HashMap<usize, Hooks>> = RefCell::new(HashMap::new());
    /// The implementations the swizzles stand in front of, by selector name.
    static ORIGINAL: RefCell<HashMap<&'static str, Imp>> = RefCell::new(HashMap::new());
    static CHORDS: Vec<Keystroke> = keys::app_chords(Os::Mac);
}

pub struct Native {
    webview: wry::WebView,
    /// The WKWebView, which wry retains for the web view's life.
    view: id,
    /// GPUI's view, the web view's superview.
    gpui: id,
    shown: Cell<bool>,
}

impl Native {
    pub fn new(url: &str, dir: Option<PathBuf>, tx: UnboundedSender<Raw>, window: &mut Window, _cx: &mut App) -> Result<Native, String> {
        let (load, title, ipc, open, nav, gone) = (tx.clone(), tx.clone(), tx.clone(), tx.clone(), tx.clone(), tx.clone());
        let webview = WebViewBuilder::new()
            .with_incognito(dir.is_none())
            .with_url(url)
            .with_visible(false)
            .with_accept_first_mouse(true)
            .with_initialization_script(INIT_SCRIPT)
            .with_on_page_load_handler(move |event, _| {
                _ = load.unbounded_send(Raw::Load(matches!(event, wry::PageLoadEvent::Started)));
            })
            .with_document_title_changed_handler(move |t| _ = title.unbounded_send(Raw::Titled(t)))
            .with_ipc_handler(move |request| {
                if request.body() == "wsp:moved" {
                    _ = ipc.unbounded_send(Raw::MovedInPlace);
                }
            })
            .with_new_window_req_handler(move |url, _| {
                _ = open.unbounded_send(Raw::NewWindow(url));
                wry::NewWindowResponse::Deny
            })
            .with_on_web_content_process_terminate_handler(move || _ = gone.unbounded_send(Raw::Gone(String::new())))
            .with_navigation_handler(move |url| {
                if super::stays(&url) {
                    return true;
                }
                _ = nav.unbounded_send(Raw::Leave(url));
                false
            })
            .build_as_child(&*window)
            .map_err(|e| format!("the page could not be made: {e}"))?;
        let view = objc2::rc::Retained::as_ptr(&webview.webview()) as id;
        // SAFETY: wry made `view` a subview of GPUI's view on this thread, and both live past this call.
        let gpui: id = unsafe { msg_send![view, superview] };
        unsafe {
            install(view, gpui);
            // Today's guest opens its own windows, as `allowpopups` lets it; each becomes a tab.
            let configuration: id = msg_send![view, configuration];
            let preferences: id = msg_send![configuration, preferences];
            let _: () = msg_send![preferences, setJavaScriptCanOpenWindowsAutomatically: YES];
            // A web view made in a window does not take its keys until a press says so.
            let ns_window: id = msg_send![view, window];
            let first: id = msg_send![ns_window, firstResponder];
            if first == view {
                let _: BOOL = msg_send![ns_window, makeFirstResponder: gpui];
            }
        }
        HOOKS.with(|hooks| hooks.borrow_mut().insert(view as usize, Hooks { holes: Vec::new(), tx }));
        Ok(Native { webview, view, gpui, shown: Cell::new(false) })
    }

    pub fn show(&self, at: Bounds<Pixels>, holes: &[Bounds<Pixels>], _scale: f32) {
        _ = self.webview.set_bounds(wry::Rect {
            position: LogicalPosition::new(f64::from(at.origin.x), f64::from(at.origin.y)).into(),
            size: LogicalSize::new(f64::from(at.size.width), f64::from(at.size.height)).into(),
        });
        let local: Vec<Bounds<Pixels>> = holes.iter().map(|h| Bounds::new(h.origin - at.origin, h.size)).collect();
        // SAFETY: the view is wry's live WKWebView, on the main thread.
        unsafe { mask(self.view, at.size, &local) };
        HOOKS.with(|hooks| {
            if let Some(h) = hooks.borrow_mut().get_mut(&(self.view as usize)) {
                h.holes = local;
            }
        });
        if !self.shown.replace(true) {
            _ = self.webview.set_visible(true);
        }
    }

    pub fn hide(&self) {
        if self.shown.replace(false) {
            _ = self.webview.set_visible(false);
        }
    }

    pub fn focused(&self) -> bool {
        // SAFETY: as above.
        unsafe { holds_keys(self.view) }
    }

    pub fn take_focus(&self) {
        _ = self.webview.focus();
    }

    pub fn release_focus(&self) {
        if self.focused() {
            // SAFETY: as above; GPUI's view takes first responder back as it does after a style change.
            unsafe {
                let window: id = msg_send![self.view, window];
                let _: BOOL = msg_send![window, makeFirstResponder: self.gpui];
            }
        }
    }

    pub fn url(&self) -> Option<String> {
        self.webview.url().ok().filter(|u| !u.is_empty())
    }

    pub fn load(&self, url: &str) {
        _ = self.webview.load_url(url);
    }

    pub fn reload(&self) {
        _ = self.webview.reload();
    }

    pub fn stop(&self) {
        // SAFETY: as above.
        unsafe {
            let _: () = msg_send![self.view, stopLoading];
        }
    }

    pub fn back(&self) {
        // SAFETY: as above.
        unsafe {
            let _: id = msg_send![self.view, goBack];
        }
    }

    pub fn forward(&self) {
        // SAFETY: as above.
        unsafe {
            let _: id = msg_send![self.view, goForward];
        }
    }

    pub fn can_go_back(&self) -> bool {
        // SAFETY: as above.
        unsafe {
            let can: BOOL = msg_send![self.view, canGoBack];
            can == YES
        }
    }

    pub fn can_go_forward(&self) -> bool {
        // SAFETY: as above.
        unsafe {
            let can: BOOL = msg_send![self.view, canGoForward];
            can == YES
        }
    }

    /// WebKit follows the window's backing scale itself; the zoom is the tab's alone.
    pub fn set_zoom(&self, zoom: f32, _scale: f32) {
        _ = self.webview.zoom(f64::from(zoom));
    }

    pub fn run_script(&self, script: &str) {
        _ = self.webview.evaluate_script(script);
    }

    /// The WKWebView and GPUI's view, for the checks.
    pub fn views(&self) -> (id, id) {
        (self.view, self.gpui)
    }

    /// The size the web view draws at, in device pixels; for the checks.
    pub fn drawn_size(&self) -> Option<(i32, i32)> {
        self.geometry().map(|(_, _, w, h)| (w, h))
    }

    /// Where the page's view stands in GPUI's, top-left, in device pixels; for the checks.
    pub fn geometry(&self) -> Option<(i32, i32, i32, i32)> {
        // SAFETY: as above.
        unsafe {
            let frame: NSRect = msg_send![self.view, frame];
            let parent: NSRect = msg_send![self.gpui, bounds];
            let flipped: BOOL = msg_send![self.gpui, isFlipped];
            let window: id = msg_send![self.view, window];
            let scale: f64 = msg_send![window, backingScaleFactor];
            let y = if flipped == YES { frame.origin.y } else { parent.size.height - frame.origin.y - frame.size.height };
            let d = |v: f64| (v * scale).round() as i32;
            Some((d(frame.origin.x), d(y), d(frame.size.width), d(frame.size.height)))
        }
    }
}

impl Drop for Native {
    fn drop(&mut self) {
        HOOKS.with(|hooks| hooks.borrow_mut().remove(&(self.view as usize)));
    }
}

/// Whether the window's first responder is `view` or inside it.
unsafe fn holds_keys(view: id) -> bool {
    let window: id = msg_send![view, window];
    if window == nil {
        return false;
    }
    let first: id = msg_send![window, firstResponder];
    if first == view {
        return true;
    }
    let is_view: BOOL = msg_send![first, isKindOfClass: class!(NSView)];
    if is_view != YES {
        return false;
    }
    let inside: BOOL = msg_send![first, isDescendantOf: view];
    inside == YES
}

/// A rectangle in the view's own top-left points, as the view's coordinates have it.
unsafe fn in_view(view: id, rect: Bounds<Pixels>) -> NSRect {
    let bounds: NSRect = msg_send![view, bounds];
    let flipped: BOOL = msg_send![view, isFlipped];
    let (x, w, h) = (f64::from(rect.origin.x), f64::from(rect.size.width), f64::from(rect.size.height));
    let y = if flipped == YES { f64::from(rect.origin.y) } else { bounds.size.height - f64::from(rect.origin.y) - h };
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

/// A point in the view's own coordinates as top-left points.
unsafe fn top_left(view: id, at: NSPoint) -> gpui_kit::Point<Pixels> {
    let bounds: NSRect = msg_send![view, bounds];
    let flipped: BOOL = msg_send![view, isFlipped];
    let y = if flipped == YES { at.y } else { bounds.size.height - at.y };
    point(px(at.x as f32), px(y as f32))
}

/// Masks the view's layer to its bounds less each hole, even-odd, each rectangle taken into the layer's own space.
unsafe fn mask(view: id, size: gpui_kit::Size<Pixels>, holes: &[Bounds<Pixels>]) {
    let _: () = msg_send![view, setWantsLayer: YES];
    let layer: id = msg_send![view, layer];
    if holes.is_empty() {
        let _: () = msg_send![layer, setMask: nil];
        return;
    }
    // In whole points: the page's frame is set in points, and a hole's edge lands on one.
    let points = |b: Bounds<Pixels>| super::device(b, 1.);
    let holes: Vec<super::Rect> = holes.iter().map(|h| points(*h)).collect();
    let path = CGPathCreateMutable();
    for (x, y, w, h) in super::visible(points(Bounds::new(gpui_kit::Point::default(), size)), &holes) {
        let kept = Bounds::new(point(px(x as f32), px(y as f32)), gpui_kit::size(px(w as f32), px(h as f32)));
        let rect: NSRect = msg_send![view, convertRectToLayer: in_view(view, kept)];
        CGPathAddRect(path, std::ptr::null(), rect);
    }
    let shape: id = msg_send![class!(CAShapeLayer), layer];
    let frame: NSRect = msg_send![layer, bounds];
    let _: () = msg_send![shape, setFrame: frame];
    let _: () = msg_send![shape, setPath: path];
    let _: () = msg_send![layer, setMask: shape];
    CGPathRelease(path);
}

/// Whether the view's mask leaves `at` (top-left points in the view) showing the page; for the checks.
pub unsafe fn mask_shows(view: id, at: gpui_kit::Point<Pixels>) -> Option<bool> {
    let layer: id = msg_send![view, layer];
    let shape: id = msg_send![layer, mask];
    if shape == nil {
        return None;
    }
    let path: *const c_void = msg_send![shape, path];
    let probe = in_view(view, Bounds::new(at, gpui_kit::size(px(0.), px(0.))));
    let probe: NSPoint = msg_send![view, convertPointToLayer: probe.origin];
    Some(CGPathContainsPoint(path, std::ptr::null(), probe, false))
}

/// What the checks print about the view's coordinates, for the person reading them.
pub unsafe fn orientation(view: id) -> String {
    let flipped: BOOL = msg_send![view, isFlipped];
    let layer: id = msg_send![view, layer];
    let geometry: BOOL = msg_send![layer, isGeometryFlipped];
    let contents: BOOL = msg_send![layer, contentsAreFlipped];
    format!("view flipped {}, layer geometryFlipped {}, contentsAreFlipped {}", flipped == YES, geometry == YES, contents == YES)
}

/// Puts `imp` in front of `selector` on `class`: added to the class itself, so a method it inherits keeps serving every
/// other class; the original is kept to call through.
unsafe fn swizzle(class: &Class, selector: Sel, name: &'static str, imp: Imp) {
    if ORIGINAL.with(|o| o.borrow().contains_key(name)) {
        return;
    }
    let method = class_getInstanceMethod(class, selector);
    if method.is_null() {
        return;
    }
    let original = method_getImplementation(method);
    let types = method_getTypeEncoding(method);
    if class_addMethod(class, selector, imp, types) == NO {
        method_setImplementation(method, imp);
    }
    ORIGINAL.with(|o| o.borrow_mut().insert(name, original));
}

fn original(name: &str) -> Option<Imp> {
    ORIGINAL.with(|o| o.borrow().get(name).copied())
}

unsafe fn install(view: id, gpui: id) {
    let web: *const Class = msg_send![view, class];
    let gpui_class: *const Class = msg_send![gpui, class];
    let (web, gpui_class) = (&*web, &*gpui_class);
    swizzle(web, sel!(hitTest:), "hitTest:", hit_test as Imp);
    swizzle(web, sel!(mouseDown:), "mouseDown:", mouse_down as Imp);
    swizzle(web, sel!(rightMouseDown:), "rightMouseDown:", right_mouse_down as Imp);
    swizzle(web, sel!(willOpenMenu:withEvent:), "willOpenMenu:withEvent:", will_open_menu as Imp);
    swizzle(gpui_class, sel!(performKeyEquivalent:), "performKeyEquivalent:", perform_key_equivalent as Imp);
    let ui: id = msg_send![view, UIDelegate];
    if ui != nil {
        let ui_class: *const Class = msg_send![ui, class];
        let media = sel!(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:);
        let method = class_getInstanceMethod(ui_class, media);
        // wry grants a page the camera and microphone; today's guest is granted neither.
        if !method.is_null() {
            method_setImplementation(method, deny_media as Imp);
        }
    }
    let delegate: id = msg_send![view, navigationDelegate];
    if delegate != nil {
        let delegate_class: *const Class = msg_send![delegate, class];
        // wry says a load started only once it commits and never that one failed; WebKit says both.
        let started = sel!(webView:didStartProvisionalNavigation:);
        if class_getInstanceMethod(delegate_class, started).is_null() {
            class_addMethod(delegate_class, started, load_started as Imp, c"v@:@@".as_ptr());
        }
        for failed in [sel!(webView:didFailProvisionalNavigation:withError:), sel!(webView:didFailNavigation:withError:)] {
            if class_getInstanceMethod(delegate_class, failed).is_null() {
                class_addMethod(delegate_class, failed, load_failed as Imp, c"v@:@@@".as_ptr());
            }
        }
    }
}

/// Answers WebKit's media question with `WKPermissionDecisionDeny`, calling the block's function as its layout lays out.
extern "C" fn deny_media(_: &Object, _: Sel, _: id, _: id, _: id, _: isize, handler: *mut c_void) {
    const DENY: isize = 2;
    // SAFETY: `handler` is the block WebKit passes, whose function pointer sits after its isa and two ints.
    unsafe {
        let invoke = *(handler as *const *const c_void).add(2);
        let invoke: extern "C" fn(*mut c_void, isize) = std::mem::transmute(invoke);
        invoke(handler, DENY);
    }
}

fn said(web_view: id, raw: Raw) {
    HOOKS.with(|hooks| {
        if let Some(h) = hooks.borrow().get(&(web_view as usize)) {
            _ = h.tx.unbounded_send(raw);
        }
    });
}

extern "C" fn load_started(_: &Object, _: Sel, web_view: id, _: id) {
    said(web_view, Raw::Load(true));
}

extern "C" fn load_failed(_: &Object, _: Sel, web_view: id, _: id, _: id) {
    said(web_view, Raw::Load(false));
}

/// A point in a hole is not the page's: the hit falls through to GPUI's view under it.
extern "C" fn hit_test(this: &Object, selector: Sel, at: NSPoint) -> id {
    let this = this as *const Object as id;
    // SAFETY: AppKit calls this on the main thread with a live view; `at` is in the superview's coordinates.
    unsafe {
        let superview: id = msg_send![this, superview];
        let local: NSPoint = msg_send![this, convertPoint: at fromView: superview];
        let at_page = top_left(this, local);
        let in_hole =
            HOOKS.with(|hooks| hooks.borrow().get(&(this as usize)).is_some_and(|h| h.holes.iter().any(|hole| hole.contains(&at_page))));
        if in_hole {
            return nil;
        }
        let Some(imp) = original("hitTest:") else { return nil };
        let imp: extern "C" fn(id, Sel, NSPoint) -> id = std::mem::transmute(imp);
        imp(this, selector, at)
    }
}

unsafe fn pressed(this: id, event: id) {
    let in_window: NSPoint = msg_send![event, locationInWindow];
    let local: NSPoint = msg_send![this, convertPoint: in_window fromView: nil];
    let at = top_left(this, local);
    HOOKS.with(|hooks| {
        if let Some(h) = hooks.borrow().get(&(this as usize)) {
            _ = h.tx.unbounded_send(Raw::Pressed(at));
        }
    });
}

extern "C" fn mouse_down(this: &Object, selector: Sel, event: id) {
    let this = this as *const Object as id;
    // SAFETY: as in `hit_test`.
    unsafe {
        pressed(this, event);
        if let Some(imp) = original("mouseDown:") {
            let imp: extern "C" fn(id, Sel, id) = std::mem::transmute(imp);
            imp(this, selector, event);
        }
    }
}

extern "C" fn right_mouse_down(this: &Object, selector: Sel, event: id) {
    let this = this as *const Object as id;
    // SAFETY: as in `hit_test`.
    unsafe {
        pressed(this, event);
        if let Some(imp) = original("rightMouseDown:") {
            let imp: extern "C" fn(id, Sel, id) = std::mem::transmute(imp);
            imp(this, selector, event);
        }
    }
}

/// Today's guest has no menu on a right press.
extern "C" fn will_open_menu(this: &Object, selector: Sel, menu: id, event: id) {
    let this = this as *const Object as id;
    // SAFETY: as in `hit_test`.
    unsafe {
        if let Some(imp) = original("willOpenMenu:withEvent:") {
            let imp: extern "C" fn(id, Sel, id, id) = std::mem::transmute(imp);
            imp(this, selector, menu, event);
        }
        let _: () = msg_send![menu, removeAllItems];
    }
}

/// While a page is first responder, GPUI takes only the app's chords; every other key goes on to the menu and the page.
extern "C" fn perform_key_equivalent(this: &Object, selector: Sel, event: id) -> BOOL {
    let this = this as *const Object as id;
    // SAFETY: as in `hit_test`; `this` is GPUI's view.
    unsafe {
        let window: id = msg_send![this, window];
        let page_holds = HOOKS.with(|hooks| {
            hooks.borrow().keys().any(|&view| {
                let theirs: id = msg_send![view as id, window];
                theirs == window && holds_keys(view as id)
            })
        });
        if page_holds && !CHORDS.with(|chords| keys::is_app_chord(chords, &key_of(event), &modifiers_of(event))) {
            return NO;
        }
        let Some(imp) = original("performKeyEquivalent:") else { return NO };
        let imp: extern "C" fn(id, Sel, id) -> BOOL = std::mem::transmute(imp);
        imp(this, selector, event)
    }
}

unsafe fn key_of(event: id) -> String {
    let code: u16 = msg_send![event, keyCode];
    if code == 53 {
        return "escape".to_owned();
    }
    let chars: id = msg_send![event, charactersIgnoringModifiers];
    if chars == nil {
        return String::new();
    }
    let utf8: *const c_char = msg_send![chars, UTF8String];
    if utf8.is_null() {
        return String::new();
    }
    std::ffi::CStr::from_ptr(utf8).to_string_lossy().to_lowercase()
}

unsafe fn modifiers_of(event: id) -> Modifiers {
    let flags: u64 = msg_send![event, modifierFlags];
    Modifiers {
        shift: flags & (1 << 17) != 0,
        control: flags & (1 << 18) != 0,
        alt: flags & (1 << 19) != 0,
        platform: flags & (1 << 20) != 0,
        function: false,
    }
}
