// SPDX-License-Identifier: AGPL-3.0-only
//! Which threads stay warm, decided in one place: the one on screen, the threads shown before it, newest first, and the
//! threads two Ctrl+Tab steps either way from it at its level. The thread view reads ahead the newest pages of the whole
//! set; the browser pane, whose every page is a web view with a process of its own, keeps its pages for the one on
//! screen and the ones shown before it alone (`LOADED_PANELS`). Whatever a thread outside them held goes, so the app
//! holds one bound and not one per holder.

use std::collections::VecDeque;

use gpui_kit::{App, Global, SharedString, Window};

/// The one bound on the threads kept warm off screen: the threads shown last and the threads around the open one,
/// counted together, the open one aside.
pub const HELD: usize = 7;
/// Of `HELD`, the threads shown last, newest first.
pub const RECENT: usize = 3;
/// Of `HELD`, the threads two Ctrl+Tab steps either way from the open one.
pub const AROUND: usize = HELD - RECENT;

/// The glyph pages a window keeps across a thread switch: every character, size and weight a thread draws adds tiles
/// to them that nothing else lets go of.
pub const GLYPH_PAGES: u64 = 16 << 20;

/// A thread by its workspace and its id.
pub type Key = (String, String);

/// The warm threads: the open one, the ones shown before it and the ones around it.
#[derive(Default, Debug, Clone)]
pub struct Set {
    open: Option<Key>,
    recent: VecDeque<Key>,
    around: Vec<Key>,
}

impl Set {
    /// `open` is on screen: the thread on screen before it joins the recent ones, newest first.
    pub fn show(&mut self, open: Key) {
        if self.open.as_ref() == Some(&open) {
            return;
        }
        if let Some(left) = self.open.replace(open.clone()) {
            self.recent.retain(|k| *k != left);
            self.recent.push_front(left);
        }
        self.recent.retain(|k| *k != open);
        self.recent.truncate(RECENT);
        self.around.clear();
    }

    /// The threads around the open one, as its level in the sidebar has them now.
    pub fn surround(&mut self, around: Vec<Key>) {
        self.around = around.into_iter().filter(|k| Some(k) != self.open.as_ref()).take(AROUND).collect();
    }

    /// The warm threads off screen, at most `HELD`: the ones around the open one first, then the ones shown last.
    pub fn held(&self) -> Vec<Key> {
        let mut held: Vec<Key> = Vec::with_capacity(HELD);
        for key in self.around.iter().chain(self.recent.iter()) {
            if Some(key) != self.open.as_ref() && !held.contains(key) {
                held.push(key.clone());
            }
        }
        held.truncate(HELD);
        held
    }

    /// The open thread's id and the ids of the threads shown before it, newest first: at most `RECENT + 1`.
    pub fn shown_last(&self) -> Vec<SharedString> {
        self.open.iter().chain(self.recent.iter()).map(|(_, thread)| SharedString::from(thread.clone())).collect()
    }

    /// Every warm thread's id, the open one first.
    pub fn threads(&self) -> Vec<SharedString> {
        self.open.iter().cloned().chain(self.held()).map(|(_, thread)| SharedString::from(thread)).collect()
    }

    pub fn open(&self) -> Option<&Key> {
        self.open.as_ref()
    }

    pub fn recent(&self) -> &VecDeque<Key> {
        &self.recent
    }
}

#[derive(Default)]
struct Warm(Set);

impl Global for Warm {}

/// `thread` on `workspace` is shown; answers its id and the ids of the threads shown before it, newest first.
pub fn shown(workspace: &str, thread: &str, cx: &mut App) -> Vec<SharedString> {
    let set = &mut cx.default_global::<Warm>().0;
    set.show((workspace.to_owned(), thread.to_owned()));
    set.shown_last()
}

/// `open` is on screen with `around` it at its level: answers the warm threads off screen.
pub fn surround(open: Key, around: Vec<Key>, cx: &mut App) -> Vec<Key> {
    let set = &mut cx.default_global::<Warm>().0;
    set.show(open);
    set.surround(around);
    set.held()
}

/// The warm set as it stands.
pub fn set(cx: &App) -> Set {
    cx.try_global::<Warm>().map(|w| w.0.clone()).unwrap_or_default()
}

/// `window` switched to `thread`: it joins the warm threads, and the window's glyph tiles go past [`GLYPH_PAGES`].
pub fn switched(workspace: &str, thread: &str, window: &mut Window, cx: &mut App) {
    shown(workspace, thread, cx);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(i: usize) -> Key {
        ("w".to_owned(), format!("t{i:02}"))
    }

    /// 60 switches over 30 threads: down them one Ctrl+Tab at a time, two thirds of the way back, then jumps.
    #[test]
    fn the_warm_set_stays_within_its_bound_over_a_60_switch_walk() {
        let n = 30;
        let mut set = Set::default();
        let mut most = 0;
        let steps: Vec<usize> = (0..n).chain((10..n).rev()).chain([3, 17, 9, 28, 0, 22, 5, 14, 26, 11]).collect();
        assert_eq!(steps.len(), 60);
        let mut shown: Vec<usize> = Vec::new();
        for open in steps {
            let around: Vec<Key> =
                [1, -1, 2, -2].iter().map(|by: &isize| key((open as isize + by).rem_euclid(n as isize) as usize)).collect();
            set.show(key(open));
            set.surround(around.clone());
            let held = set.held();
            most = most.max(held.len());
            assert!(held.len() <= HELD, "{} threads warm at t{open:02}", held.len());
            assert!(!held.contains(&key(open)), "the open thread is held twice");
            assert!(around.iter().all(|k| held.contains(k)), "a thread around t{open:02} is not warm: {held:?}");
            let last: Vec<Key> = shown.iter().rev().filter(|&&s| s != open).take(RECENT).map(|&s| key(s)).collect();
            assert!(last.iter().all(|k| held.contains(k) || around.contains(k)), "a thread shown last is not warm at t{open:02}");
            assert!(held.iter().all(|k| around.contains(k) || last.contains(k)), "a thread warm at t{open:02} is neither: {held:?}");
            shown.retain(|&s| s != open);
            shown.push(open);
        }
        assert_eq!(most, HELD, "a walk that jumps fills the bound and no more");
    }

    #[test]
    fn the_threads_shown_last_are_newest_first_and_the_open_one_leads() {
        let mut set = Set::default();
        for open in [0, 10, 20, 30] {
            set.show(key(open));
        }
        assert_eq!(set.recent, [key(20), key(10), key(0)]);
        set.show(key(10));
        assert_eq!(set.recent, [key(30), key(20), key(0)]);
        assert_eq!(set.threads(), ["t10", "t30", "t20", "t00"].map(SharedString::from));
    }
}
