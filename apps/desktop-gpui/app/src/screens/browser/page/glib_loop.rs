// SPDX-License-Identifier: AGPL-3.0-only
//! GLib's default main context, run from GPUI's own loop. GTK and WebKit live on the main thread, which GPUI owns, so
//! a task on GPUI's foreground executor prepares the context, a thread of its own waits on the context's descriptors
//! for GLib's timeout, and the task checks and dispatches once the wait ends: GLib runs when it has work, not on a timer.

use std::sync::mpsc;

use futures::channel::oneshot;
use gpui_kit::{App, Global};
use gtk::glib::ffi as g;

/// The loop, started once.
struct Running;

impl Global for Running {}

type Wait = (Vec<g::GPollFD>, i32, oneshot::Sender<Vec<g::GPollFD>>);

const EMPTY: g::GPollFD = g::GPollFD { fd: -1, events: 0, revents: 0 };

pub fn start(cx: &mut App) {
    if cx.has_global::<Running>() {
        return;
    }
    cx.set_global(Running);
    let (waits, waiting) = mpsc::channel::<Wait>();
    let spawned = std::thread::Builder::new().name("glib-wait".into()).spawn(move || {
        while let Ok((mut fds, timeout, done)) = waiting.recv() {
            poll(&mut fds, timeout);
            _ = done.send(fds);
        }
    });
    if let Err(e) = spawned {
        eprintln!("desktop-gpui: browser: GLib's wait thread did not start: {e}");
        return;
    }
    cx.spawn(async move |_| {
        // SAFETY: the default context is GLib's for the life of the process, and only this thread acquires it.
        let context = unsafe { g::g_main_context_default() };
        unsafe { g::g_main_context_acquire(context) };
        let mut fds = vec![EMPTY; 16];
        loop {
            let mut priority = 0;
            let mut timeout = 0;
            // SAFETY: the context is held by this thread, and `fds` is as long as the count `query` is given.
            let count = unsafe {
                g::g_main_context_prepare(context, &mut priority);
                loop {
                    let count = g::g_main_context_query(context, priority, &mut timeout, fds.as_mut_ptr(), fds.len() as i32);
                    if count as usize <= fds.len() {
                        break count as usize;
                    }
                    fds.resize(count as usize, EMPTY);
                }
            };
            let mut asked = fds[..count].to_vec();
            if timeout == 0 {
                poll(&mut asked, 0);
            } else {
                let (done, answered) = oneshot::channel();
                if waits.send((asked.clone(), timeout, done)).is_err() {
                    return;
                }
                match answered.await {
                    Ok(polled) => asked = polled,
                    Err(_) => return,
                }
            }
            fds[..count].copy_from_slice(&asked);
            // GLib may say a source is ready on every pass; GPUI's own work runs between passes.
            Yield(false).await;
            // SAFETY: as above; `check` reads the `revents` the wait wrote.
            unsafe {
                if g::g_main_context_check(context, priority, fds.as_mut_ptr(), count as i32) != 0 {
                    g::g_main_context_dispatch(context);
                }
            }
        }
    })
    .detach();
}

/// Pending once, then ready: hands the thread back to GPUI's executor for one turn.
struct Yield(bool);

impl std::future::Future for Yield {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        if std::mem::replace(&mut self.0, true) {
            return std::task::Poll::Ready(());
        }
        cx.waker().wake_by_ref();
        std::task::Poll::Pending
    }
}

/// Ends the wait now: GLib signals its own wait only for a source added from another thread, and GPUI's calls into
/// GTK are on this one.
pub fn nudge() {
    // SAFETY: waking the default context is safe from any thread at any time.
    unsafe { g::g_main_context_wakeup(g::g_main_context_default()) };
}

fn poll(fds: &mut [g::GPollFD], timeout: i32) {
    // GPollFD is `struct pollfd` on Linux: an int and two shorts.
    let fds_ptr = fds.as_mut_ptr().cast::<libc::pollfd>();
    // SAFETY: `fds_ptr` points at `fds.len()` initialised entries for the call's length.
    unsafe { libc::poll(fds_ptr, fds.len() as libc::nfds_t, timeout) };
}
