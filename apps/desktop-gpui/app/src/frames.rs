// SPDX-License-Identifier: AGPL-3.0-only
//! Frames drawn and renders of the views a live piece must leave alone, counted per thread so parallel tests never
//! share them. `DESKTOP_GPUI_FRAMES=<file>` appends one line of the counts to that file each second, with `covered=1`
//! or `unkeyed=1` for a second in which the window was covered or not key.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Write as _;
use std::time::{Duration, Instant};

use gpui_kit::{App, Window};

thread_local! {
    static COUNTS: RefCell<BTreeMap<&'static str, u64>> = const { RefCell::new(BTreeMap::new()) };
}

/// One more of `what`: "frame" for a frame drawn, else the view that rendered.
pub fn count(what: &'static str) {
    COUNTS.with(|counts| *counts.borrow_mut().entry(what).or_default() += 1);
}

/// The counts since the last take, which start again from nothing.
pub fn take() -> BTreeMap<&'static str, u64> {
    COUNTS.with(|counts| std::mem::take(&mut *counts.borrow_mut()))
}

/// Counts the window's frames, and with `DESKTOP_GPUI_FRAMES` set writes the counts out once a second.
pub fn watch(window: &mut Window, cx: &mut App) {
    window
        .on_drawn(|_, _| {
            count("frame");
            true
        })
        .detach();
    let Some(path) = std::env::var_os("DESKTOP_GPUI_FRAMES").filter(|path| !path.is_empty()) else { return };
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    let began = Instant::now();
    take();
    cx.spawn(async move |cx| loop {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        let mut line: String = take().iter().map(|(what, n)| format!(" {what}={n}")).collect();
        line.push_str(&crate::in_front::take().words());
        if writeln!(file, "t={}{line}", began.elapsed().as_secs()).is_err() {
            break;
        }
    })
    .detach();
}
