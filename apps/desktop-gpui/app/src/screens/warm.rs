// SPDX-License-Identifier: AGPL-3.0-only
//! Which threads stay warm, decided in one place: the one on screen and the few shown before it, newest first. The
//! browser pane keeps its pages for these threads alone, and threads loaded ahead of the person join this set rather
//! than keeping one of their own, so the app holds one bound and not one per holder.

use gpui_kit::{App, Global, SharedString, Window};

/// `LOADED_PANELS`: the threads kept warm, the one on screen among them.
pub const WARM: usize = 3;

/// The glyph pages a window keeps across a thread switch: every character, size and weight a thread draws adds tiles
/// to them that nothing else lets go of.
pub const GLYPH_PAGES: u64 = 16 << 20;

#[derive(Default)]
struct Warm(Vec<SharedString>);

impl Global for Warm {}

/// Marks `thread` shown, the newest; answers the threads warm now, newest first.
pub fn shown(thread: &SharedString, cx: &mut App) -> Vec<SharedString> {
    let warm = &mut cx.default_global::<Warm>().0;
    if warm.first() != Some(thread) {
        warm.retain(|t| t != thread);
        warm.insert(0, thread.clone());
        warm.truncate(WARM);
    }
    warm.clone()
}

/// `window` switched to `thread`: it joins the warm threads, and the window's glyph tiles go past [`GLYPH_PAGES`].
pub fn switched(thread: &str, window: &mut Window, cx: &mut App) {
    shown(&SharedString::from(thread.to_owned()), cx);
    // A switch can come inside a draw (the bench opens its threads from one), when the atlas must stand.
    window.defer(cx, |window, _| {
        trim_atlas(window, GLYPH_PAGES);
    });
}

/// Lets go of the window's glyph tiles once their pages pass `bound` bytes, to be drawn again for what is on screen;
/// true where they went.
pub fn trim_atlas(window: &mut Window, bound: u64) -> bool {
    window.atlas_usage().glyphs > bound && window.sweep_glyph_atlas()
}
