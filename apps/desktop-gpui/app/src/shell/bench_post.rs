// SPDX-License-Identifier: AGPL-3.0-only
//! A replay's wheel events on a Mac as AppKit hands a trackpad's to the window's view: made from a thread of their own,
//! each at its due time, and handed to the view's `scrollWheel:` on the main queue, so one waits behind a frame the main
//! thread is drawing as a trackpad's does, and traced as the window hears it. Nothing goes through the window server,
//! which asks a permission of a process that posts events, even to itself.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gpui_kit::{canvas, prelude::*, AnyElement, DispatchPhase, Entity, ListState, Pixels, Point, ScrollWheelEvent, Task, Window};

use super::Shell;
use crate::scroll_trace::{self, Wheel};
use crate::thread_view::ThreadView;

/// What the window hears of the posted events: when each still unheard one was due and when it was handed to the main
/// queue, in order, and the list it scrolls.
struct Heard {
    due: Arc<Mutex<VecDeque<(Instant, Instant)>>>,
    list: ListState,
    view: Option<Entity<ThreadView>>,
    count: Cell<usize>,
    /// Where the list stood when the event being handled came.
    handling: Cell<Option<f32>>,
    /// Whether a frame has painted the listener, before which no event is posted.
    listening: Cell<bool>,
}

thread_local! {
    static HEARD: RefCell<Option<Rc<Heard>>> = const { RefCell::new(None) };
}

/// The shell's listener for the posted events while a replay runs: the capture phase traces each as it comes, with
/// where the list stood, and the bubble phase where the list stands once the window has handled it.
pub(super) fn listener() -> Option<AnyElement> {
    let heard = HEARD.with(|h| h.borrow().clone())?;
    let paint = move |_, _, window: &mut Window, _: &mut gpui_kit::App| {
        heard.listening.set(true);
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| match phase {
            DispatchPhase::Capture => {
                let Some(due) = heard.due.lock().ok().and_then(|mut due| due.pop_front()) else { return };
                let spot = match &heard.view {
                    Some(view) => view.read(cx).spot(event.position.y),
                    None => scroll_trace::Spot {
                        top: -f32::from(heard.list.scroll_px_offset_for_scrollbar().y),
                        max: f32::from(heard.list.max_offset_for_scrollbar().y),
                        under: Some(
                            if heard.list.viewport_bounds().contains(&event.position) { "the list" } else { "off the list" }.to_owned(),
                        ),
                    },
                };
                if heard.count.get() == 0 {
                    let number = mac::Screen::of(window).map_or(-1, |s| s.number());
                    let under = spot.under.as_deref().unwrap_or("the thread's rows");
                    eprintln!(
                        "bench: the first posted wheel event reached window {number} at {:?} in it, over {under}, as {:?} in phase {:?}",
                        event.position, event.delta, event.touch_phase
                    );
                }
                heard.handling.set(Some(spot.top));
                scroll_trace::input_posted(due.0, Some(due.1), event, Some(spot));
                heard.count.set(heard.count.get() + 1);
            }
            // The bubble phase reaches this listener, painted over the list, before the list: where the list stands
            // after an event is read once the view has handed it on, in `delivered`.
            DispatchPhase::Bubble => {}
        });
    };
    Some(canvas(|_, _, _| (), paint).absolute().size_0().into_any_element())
}

/// The view handed an event on in `ms`: the window has handled it, so where the list stands now is where it left it.
fn delivered(ms: f64) {
    let Some(heard) = HEARD.with(|h| h.borrow().clone()) else { return };
    let Some(top) = heard.handling.take() else { return };
    let after = -f32::from(heard.list.scroll_px_offset_for_scrollbar().y);
    if heard.count.get() == 1 {
        eprintln!("bench: the first event moved the list's top from {top} to {after}");
    }
    scroll_trace::note(|w| w.input_ms += ms);
    scroll_trace::handled(after);
}

impl Shell {
    /// Posts `events` from a thread of their own, each at its time from now, `pointer` over the list where given, and
    /// sets `done` once the last has had `settle`; a run whose events never reach the window stops and says so.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn replay_posted(
        &self,
        events: Vec<Wheel>,
        pointer: Option<(f32, f32)>,
        list: ListState,
        view: Option<Entity<ThreadView>>,
        settle: Duration,
        done: Rc<Cell<bool>>,
        window: &mut Window,
        cx: &mut gpui_kit::Context<Self>,
    ) -> Task<()> {
        let Some(screen) = mac::Screen::of(window) else {
            eprintln!("desktop-gpui: the bench window has no AppKit window to post its wheel events to");
            std::process::exit(2);
        };
        let origin = list.viewport_bounds().origin;
        let due = Arc::new(Mutex::new(VecDeque::new()));
        HEARD.with(|h| {
            *h.borrow_mut() = Some(Rc::new(Heard {
                due: due.clone(),
                list,
                view,
                count: Cell::new(0),
                handling: Cell::new(None),
                listening: Cell::new(false),
            }))
        });
        // The listener stands in the shell's own output, drawn whole once to hold it.
        cx.notify();
        let posted = Arc::new(AtomicBool::new(false));
        cx.spawn_in(window, async move |this, cx| {
            while !HEARD.with(|h| h.borrow().as_ref().is_some_and(|h| h.listening.get())) {
                cx.background_executor().timer(Duration::from_millis(10)).await;
            }
            let finished = posted.clone();
            let start = Instant::now();
            let thread = std::thread::Builder::new().name("bench-replay".into()).spawn(move || {
                for wheel in events {
                    let wheel = match pointer {
                        Some((x, y)) => Wheel { x, y, ..wheel },
                        None => wheel,
                    };
                    let at = start + Duration::from_secs_f64(wheel.t / 1000.);
                    if let Some(wait) = at.checked_duration_since(Instant::now()) {
                        std::thread::sleep(wait);
                    }
                    let position: Point<Pixels> = wheel.event(origin).position;
                    if let Ok(mut due) = due.lock() {
                        due.push_back((at, Instant::now()));
                    }
                    if let Err(e) = screen.post(&wheel, position) {
                        eprintln!("desktop-gpui: {e}");
                        std::process::exit(2);
                    }
                }
                finished.store(true, Ordering::SeqCst);
            });
            if let Err(e) = thread {
                eprintln!("desktop-gpui: the replay's thread did not start: {e}");
                std::process::exit(1);
            }
            while !posted.load(Ordering::SeqCst) {
                cx.background_executor().timer(Duration::from_millis(50)).await;
            }
            cx.background_executor().timer(settle).await;
            let heard = HEARD.with(|h| h.borrow_mut().take()).map_or(0, |h| h.count.get());
            if heard == 0 {
                eprintln!("desktop-gpui: none of the posted wheel events reached the window");
                std::process::exit(2);
            }
            done.set(true);
            _ = this.update(cx, |_, cx| cx.notify());
        })
    }
}

// cocoa 0.26 marks its AppKit names deprecated in favour of objc2, which the rest of the app still reaches them by.
#[allow(deprecated)]
mod mac {
    use std::ffi::c_void;

    use cocoa::base::{id, nil};
    use cocoa::foundation::NSRect;
    use gpui_kit::{Pixels, Point, TouchPhase, Window};
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    use crate::scroll_trace::Wheel;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    // CGEventField and CGScrollEventUnit values, from CGEventTypes.h.
    const UNIT_PIXEL: u32 = 0;
    const UNIT_LINE: u32 = 1;
    const IS_CONTINUOUS: u32 = 88;
    const FIXED_PT_DELTA_1: u32 = 93;
    const FIXED_PT_DELTA_2: u32 = 94;
    const POINT_DELTA_1: u32 = 96;
    const POINT_DELTA_2: u32 = 97;
    const SCROLL_PHASE: u32 = 99;

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateScrollWheelEvent2(
            source: *const c_void,
            units: u32,
            count: u32,
            wheel1: i32,
            wheel2: i32,
            wheel3: i32,
        ) -> *mut c_void;
        fn CGEventSetIntegerValueField(event: *mut c_void, field: u32, value: i64);
        fn CGEventSetDoubleValueField(event: *mut c_void, field: u32, value: f64);
        fn CGEventSetLocation(event: *mut c_void, location: CGPoint);
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    /// libdispatch's main queue, which `dispatch_get_main_queue()` names.
    #[repr(C)]
    struct DispatchQueue {
        _private: [u8; 0],
    }

    unsafe extern "C" {
        static _dispatch_main_q: DispatchQueue;
        fn dispatch_async_f(queue: *const DispatchQueue, context: *mut c_void, work: extern "C" fn(*mut c_void));
    }

    /// Whether the next event is the first, which the log names.
    static FIRST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

    /// The window's view, read on the main thread, for events made on another and handed to it on the main thread.
    #[derive(Clone, Copy)]
    pub(super) struct Screen {
        number: i64,
        /// GPUI's NSView, which takes the events as AppKit hands a trackpad's over, by `scrollWheel:`.
        view: usize,
        /// The window's content height, which turns the window's top-down points into AppKit's bottom-up ones.
        height: f64,
        /// The first screen's height, which turns AppKit's bottom-up points into Quartz's top-down ones.
        main_height: f64,
    }

    impl Screen {
        pub(super) fn of(window: &Window) -> Option<Screen> {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            let handle = HasWindowHandle::window_handle(window).ok()?;
            let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return None };
            // SAFETY: the handle's view is GPUI's own, live in its window, and this runs on the main thread.
            unsafe {
                let view = handle.ns_view.as_ptr() as id;
                let ns_window: id = msg_send![view, window];
                if ns_window == nil {
                    return None;
                }
                let number: i64 = msg_send![ns_window, windowNumber];
                let frame: NSRect = msg_send![ns_window, frame];
                let content: NSRect = msg_send![ns_window, contentRectForFrameRect: frame];
                let screens: id = msg_send![class!(NSScreen), screens];
                let first: id = msg_send![screens, firstObject];
                if first == nil {
                    return None;
                }
                let main: NSRect = msg_send![first, frame];
                Some(Screen { number, view: view as usize, height: content.size.height, main_height: main.size.height })
            }
        }

        /// The window's number, as AppKit names it.
        pub(super) fn number(&self) -> i64 {
            self.number
        }

        /// Makes `wheel` at `position` in the window's own pixels a platform scroll event and hands it to the window's
        /// view on the main thread, in turn behind whatever the main thread is drawing, as a trackpad's event waits.
        pub(super) fn post(&self, wheel: &Wheel, position: Point<Pixels>) -> Result<(), String> {
            let (x, y) = (f64::from(f32::from(position.x)), f64::from(f32::from(position.y)));
            // An event no window holds reads its location in screen points, so the point is placed where the screen's
            // points are the window's own: the view reads it as a point in the window, as it does a trackpad's.
            let at = CGPoint { x, y: self.main_height - (self.height - y) };
            // SAFETY: each CoreGraphics call gets the event it created, which may be made on any thread; the main
            // queue's job owns it and releases it once the view has taken it.
            unsafe {
                let units = if wheel.lines { UNIT_LINE } else { UNIT_PIXEL };
                let event = CGEventCreateScrollWheelEvent2(std::ptr::null(), units, 2, wheel.dy.round() as i32, wheel.dx.round() as i32, 0);
                if event.is_null() {
                    return Err("CoreGraphics made no scroll event".to_owned());
                }
                if !wheel.lines {
                    CGEventSetIntegerValueField(event, IS_CONTINUOUS, 1);
                    CGEventSetDoubleValueField(event, FIXED_PT_DELTA_1, f64::from(wheel.dy));
                    CGEventSetDoubleValueField(event, FIXED_PT_DELTA_2, f64::from(wheel.dx));
                    CGEventSetIntegerValueField(event, POINT_DELTA_1, wheel.dy.round() as i64);
                    CGEventSetIntegerValueField(event, POINT_DELTA_2, wheel.dx.round() as i64);
                    // NSEventPhase's values: began, changed, ended, cancelled.
                    let phase = match wheel.phase {
                        TouchPhase::Started => 1,
                        TouchPhase::Moved => 2,
                        TouchPhase::Ended => 4,
                        TouchPhase::Cancelled => 8,
                    };
                    CGEventSetIntegerValueField(event, SCROLL_PHASE, phase);
                }
                CGEventSetLocation(event, at);
                let job = Box::new(Job { view: self.view, event: event as usize });
                dispatch_async_f(&_dispatch_main_q, Box::into_raw(job).cast(), deliver);
            }
            Ok(())
        }
    }

    /// A scroll event for the view, handed over on the main thread.
    struct Job {
        view: usize,
        event: usize,
    }

    extern "C" fn deliver(job: *mut c_void) {
        // SAFETY: the job is the one `post` boxed for this call alone; the view is GPUI's, live while its window is,
        // which outlives the replay; this runs on the main queue, where AppKit's views take events.
        unsafe {
            let job = Box::from_raw(job.cast::<Job>());
            let (view, event) = (job.view as id, job.event as *mut c_void);
            let pool: id = msg_send![class!(NSAutoreleasePool), new];
            let ns: id = msg_send![class!(NSEvent), eventWithCGEvent: event];
            if ns != nil {
                if FIRST.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    let class = (*(view as *const Object)).class().name();
                    eprintln!("bench: the first wheel event goes to the window's view, a {class}, by scrollWheel:");
                }
                let began = std::time::Instant::now();
                let _: () = msg_send![view, scrollWheel: ns];
                super::delivered(began.elapsed().as_secs_f64() * 1000.);
            }
            let _: () = msg_send![pool, drain];
            CFRelease(event);
        }
    }
}
