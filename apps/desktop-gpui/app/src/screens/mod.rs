// SPDX-License-Identifier: AGPL-3.0-only
//! The screens the shell routes to. A screen lives in its own folder here and plugs in with two lines in this file:
//! its `pub mod`, and one arm in [`screen`], `settings::Settings::NAME => Some(entry::<settings::Settings>()),`, which
//! both the shell's routing and `desktop-gpui --screen <name>` call. Its [`Screen::PLACE`] says where it opens.

pub mod app_shell;
pub mod composer;
pub mod usage;

use std::cell::RefCell;

pub use app_shell::{entry, navigate, Entry, Host, Mounted, Opened, Pane, Parts, Place, Route, Screen, THREAD};

/// The screen named `name`, where it goes and how it is built, or None for a name no screen has.
pub fn screen(name: &str) -> Option<Entry> {
    if let Some(added) = ADDED.with(|added| added.borrow().iter().find(|(n, _)| *n == name).map(|(_, e)| *e)) {
        return Some(added);
    }
    match name {
        composer::screen::ComposerScreen::NAME => Some(entry::<composer::screen::ComposerScreen>()),
        usage::UsageScreen::NAME => Some(entry::<usage::UsageScreen>()),
        _ => None,
    }
}

thread_local! {
    static ADDED: RefCell<Vec<(&'static str, Entry)>> = const { RefCell::new(Vec::new()) };
}

/// Adds a screen to [`screen`]'s answers on this thread, ahead of its arms, as a test mounts one through the shell's
/// own road under a name a real screen may also take, such as [`THREAD`].
#[doc(hidden)]
pub fn add(name: &'static str, entry: Entry) {
    ADDED.with(|added| added.borrow_mut().push((name, entry)));
}
