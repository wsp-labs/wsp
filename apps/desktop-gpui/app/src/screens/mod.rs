// SPDX-License-Identifier: AGPL-3.0-only
//! The screens the shell routes to. A screen lives in its own folder here and plugs in with two lines in this file:
//! its `pub mod`, and one arm in [`screen`], `settings::Settings::NAME => Some(entry::<settings::Settings>()),`, which
//! both the shell's routing and `desktop-gpui --screen <name>` call. Its [`Screen::PLACE`] says where it opens.

pub mod app_shell;
pub mod browser;
pub mod composer;
pub mod computer;
pub mod dialogs;
pub mod notices;
pub mod palette;
pub mod pr_pane;
pub mod sidebar;
pub mod slate;
pub mod terminal;
pub mod thread_view;
pub mod usage;

use std::cell::RefCell;

pub use app_shell::{entry, navigate, Entry, Host, Mounted, Opened, Pane, Parts, Place, Route, Screen, THREAD};

/// The screen named `name`, where it goes and how it is built, or None for a name no screen has.
pub fn screen(name: &str) -> Option<Entry> {
    if let Some(added) = ADDED.with(|added| added.borrow().iter().find(|(n, _)| *n == name).map(|(_, e)| *e)) {
        return Some(added);
    }
    match name {
        browser::BrowserScreen::NAME => Some(entry::<browser::BrowserScreen>()),
        composer::screen::ComposerScreen::NAME => Some(entry::<composer::screen::ComposerScreen>()),
        computer::Computer::NAME => Some(entry::<computer::Computer>()),
        slate::SlateScreen::NAME => Some(entry::<slate::SlateScreen>()),
        terminal::TerminalScreen::NAME => Some(entry::<terminal::TerminalScreen>()),
        thread_view::ThreadScreen::NAME => Some(entry::<thread_view::ThreadScreen>()),
        thread_view::subagent::SubagentScreen::NAME => Some(entry::<thread_view::subagent::SubagentScreen>()),
        usage::UsageScreen::NAME => Some(entry::<usage::UsageScreen>()),
        dialogs::AddComputerScreen::NAME => Some(entry::<dialogs::AddComputerScreen>()),
        dialogs::AddCloudScreen::NAME => Some(entry::<dialogs::AddCloudScreen>()),
        dialogs::AddProjectScreen::NAME => Some(entry::<dialogs::AddProjectScreen>()),
        dialogs::ExportProjectScreen::NAME => Some(entry::<dialogs::ExportProjectScreen>()),
        pr_pane::changes::ChangesScreen::NAME => Some(entry::<pr_pane::changes::ChangesScreen>()),
        pr_pane::pull_request::PullRequestScreen::NAME => Some(entry::<pr_pane::pull_request::PullRequestScreen>()),
        pr_pane::files::FilesScreen::NAME => Some(entry::<pr_pane::files::FilesScreen>()),
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
