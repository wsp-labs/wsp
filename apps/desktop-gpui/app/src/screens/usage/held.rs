// SPDX-License-Identifier: AGPL-3.0-only
//! Answers kept by what was asked for while their window lives, after the web's `useHeld`: a page opened again inside
//! its hold draws the kept answer and asks the host nothing, and once the hold is past it asks again.

use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{App, Global, WindowId};

struct Answer {
    value: Rc<dyn Any>,
    /// Wall-clock ms, for the read's age a person reads.
    at: f64,
    /// The executor's clock, which tests move, for the hold.
    kept: Instant,
}

/// Every window's kept answers; a window's go when it closes.
#[derive(Default)]
struct Held(HashMap<WindowId, HashMap<String, Answer>>);

impl Global for Held {}

fn held(cx: &mut App) -> &mut Held {
    if !cx.has_global::<Held>() {
        cx.set_global(Held::default());
        cx.on_window_closed(|cx, window| {
            cx.global_mut::<Held>().0.remove(&window);
        })
        .detach();
    }
    cx.global_mut::<Held>()
}

/// The answer kept under `key` in `window`, with when it was answered and whether it is still inside `hold`.
pub fn get<T: 'static>(window: WindowId, key: &str, hold: Duration, cx: &mut App) -> Option<(Rc<T>, f64, bool)> {
    let now = cx.background_executor().now();
    let answer = held(cx).0.get(&window)?.get(key)?;
    let value = answer.value.clone().downcast::<T>().ok()?;
    Some((value, answer.at, now.saturating_duration_since(answer.kept) < hold))
}

pub fn put<T: 'static>(window: WindowId, key: String, value: Rc<T>, at: f64, cx: &mut App) {
    let kept = cx.background_executor().now();
    held(cx).0.entry(window).or_default().insert(key, Answer { value, at, kept });
}

/// Replaces what is kept under `key` with what an act on it answered, keeping when it was read.
pub fn keep<T: 'static>(window: WindowId, key: &str, value: Rc<T>, cx: &mut App) {
    if let Some(answer) = held(cx).0.get_mut(&window).and_then(|kept| kept.get_mut(key)) {
        answer.value = value;
    }
}
