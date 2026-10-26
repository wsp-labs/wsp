// SPDX-License-Identifier: AGPL-3.0-only
//! A measured window kept in front: raised at launch, and every moment it is covered, not key or on a locked screen said
//! in the log as it changes and in the frames file's line for that second, as macOS draws nothing for a covered window
//! or on a locked screen and GPUI caps a window that is not key at 30 fps.

use std::cell::Cell;
use std::time::{Duration, Instant};

use gpui_kit::{Context, Window};

/// How long a launch has to come to the front and take key before a bench gives up on it.
const GRACE: Duration = Duration::from_secs(10);
/// A bench's exit for a window it cannot measure, the one it takes for lost focus.
const AWAY: i32 = 3;

/// Whether the window was covered, not key, or on a locked screen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Away {
    pub covered: bool,
    pub unkeyed: bool,
    pub locked: bool,
}

impl Away {
    fn read(window: &Window) -> Away {
        Away { covered: !window.is_visible(), unkeyed: !window.is_window_active(), locked: LOCKED.get() }
    }

    fn or(self, other: Away) -> Away {
        Away { covered: self.covered || other.covered, unkeyed: self.unkeyed || other.unkeyed, locked: self.locked || other.locked }
    }

    /// The frames file's words for it, each led by a space.
    pub fn words(self) -> String {
        let word = |on: bool, word: &str| if on { format!(" {word}=1") } else { String::new() };
        format!("{}{}{}", word(self.covered, "covered"), word(self.unkeyed, "unkeyed"), word(self.locked, "locked"))
    }
}

const HERE: Away = Away { covered: false, unkeyed: false, locked: false };

thread_local! {
    static NOW: Cell<Away> = const { Cell::new(HERE) };
    static SEEN: Cell<Away> = const { Cell::new(HERE) };
    /// Whether the screen is locked, as the session last told the app.
    static LOCKED: Cell<bool> = const { Cell::new(false) };
    /// Whether a bench watches the window, and since when.
    static WATCH: Cell<Option<(bool, Instant)>> = const { Cell::new(None) };
}

/// How the window stood at any moment since the last take, which starts again from how it stands now.
pub fn take() -> Away {
    let now = NOW.get();
    SEEN.replace(now).or(now)
}

fn note(away: Away, strict: bool, began: Instant) {
    let was = NOW.replace(away);
    SEEN.set(SEEN.get().or(away));
    if was != away {
        let covered = if away.covered { "covered" } else { "in view" };
        let key = if away.unkeyed { "not key" } else { "key" };
        let screen = if away.locked { ", the screen locked" } else { "" };
        eprintln!("desktop-gpui: the window is {covered} and {key}{screen}");
    }
    if strict && (away.locked || began.elapsed() >= GRACE && away != HERE) {
        eprintln!("desktop-gpui: the bench window is covered, not key or on a locked screen, so it draws nothing anyone sees");
        std::process::exit(AWAY);
    }
}

/// The session locked or unlocked the screen.
#[cfg(any(target_os = "macos", test))]
fn screen_locked(locked: bool) {
    LOCKED.set(locked);
    if let Some((strict, began)) = WATCH.get() {
        note(Away { locked, ..NOW.get() }, strict, began);
    }
}

/// Raises the window, and follows whether it is covered, key or on a locked screen; with `strict`, a bench, the app exits
/// 3 once its screen locks, or its window is covered or not key after the launch's grace.
pub fn watch<T: 'static>(strict: bool, window: &mut Window, cx: &mut Context<T>) {
    crate::desktop::frame::raise(window, cx);
    #[cfg(target_os = "macos")]
    order_front(window);
    let began = Instant::now();
    WATCH.set(Some((strict, began)));
    #[cfg(target_os = "macos")]
    observe_lock();
    let away = Away::read(window);
    NOW.set(away);
    SEEN.set(away);
    cx.observe_window_activation(window, move |_, window, _| note(Away::read(window), strict, began)).detach();
    cx.observe_window_visibility(window, move |_, _, window, _| note(Away::read(window), strict, began)).detach();
    // A window that opened behind another and stayed there tells neither observer.
    cx.spawn_in(window, async move |_, cx| {
        cx.background_executor().timer(GRACE).await;
        _ = cx.update(|window, _| note(Away::read(window), strict, began));
    })
    .detach();
}

/// Above every app's windows, even where macOS keeps this app from taking key from the one in front.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn order_front(window: &Window) {
    use cocoa::base::{id, nil};
    use objc::{msg_send, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return };
    // SAFETY: the handle's view is GPUI's own, live in its window, and this runs on the main thread.
    unsafe {
        let view = handle.ns_view.as_ptr() as id;
        let ns_window: id = msg_send![view, window];
        if ns_window != nil {
            let _: () = msg_send![ns_window, orderFrontRegardless];
        }
    }
}

/// The session's word on the lock: `com.apple.screenIsLocked` and `com.apple.screenIsUnlocked`, posted to every app.
#[cfg(target_os = "macos")]
#[allow(deprecated)]
fn observe_lock() {
    use std::sync::OnceLock;

    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{class, msg_send, sel, sel_impl};

    static OBSERVING: OnceLock<()> = OnceLock::new();
    if OBSERVING.set(()).is_err() {
        return;
    }
    extern "C" fn locked(_: &Object, _: Sel, _: id) {
        screen_locked(true);
    }
    extern "C" fn unlocked(_: &Object, _: Sel, _: id) {
        screen_locked(false);
    }
    let mut decl = ClassDecl::new("WspScreenLockObserver", class!(NSObject)).expect("the class is declared once");
    // SAFETY: each method's signature is a notification observer's, `- (void)locked:(NSNotification *)note`; the
    // observer object is never released, so the center never calls a freed one; this runs on the main thread, whose run
    // loop the center posts to.
    unsafe {
        decl.add_method(sel!(locked:), locked as extern "C" fn(&Object, Sel, id));
        decl.add_method(sel!(unlocked:), unlocked as extern "C" fn(&Object, Sel, id));
        let class: &Class = decl.register();
        let observer: id = msg_send![class, new];
        let center: id = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
        let on = NSString::alloc(nil).init_str("com.apple.screenIsLocked");
        let off = NSString::alloc(nil).init_str("com.apple.screenIsUnlocked");
        let _: () = msg_send![center, addObserver: observer selector: sel!(locked:) name: on object: nil];
        let _: () = msg_send![center, addObserver: observer selector: sel!(unlocked:) name: off object: nil];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_take_says_how_the_window_stood_at_any_moment_since_the_last() {
        let began = Instant::now();
        note(HERE, false, began);
        take();
        note(Away { covered: true, ..HERE }, false, began);
        note(HERE, false, began);
        assert_eq!(take(), Away { covered: true, ..HERE }, "covered for a moment in the second");
        assert_eq!(take(), HERE, "in view since");
        note(Away { unkeyed: true, ..HERE }, false, began);
        assert_eq!(take().words(), " unkeyed=1");
        assert_eq!(take().words(), " unkeyed=1", "still not key");
    }

    #[test]
    fn a_second_the_screen_was_locked_in_is_marked_locked() {
        WATCH.set(Some((false, Instant::now())));
        take();
        screen_locked(true);
        screen_locked(false);
        assert_eq!(take().words(), " locked=1", "locked for a moment in the second");
        assert_eq!(take(), HERE, "unlocked since");
    }
}
