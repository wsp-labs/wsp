// SPDX-License-Identifier: AGPL-3.0-only
//! The browser pane: a dev server's page or any web page in a right panel tab, drawn by the system's web view as a
//! child of the window, with the chrome row, the servers list and the bars around it drawn by GPUI.

pub mod bars;
pub mod check;
pub mod chrome;
pub mod field;
pub mod list;
pub mod more;
pub mod page;
pub mod ports;
pub mod reach;
pub mod recents;
pub mod refusal;
pub mod screen;
pub mod store;
pub mod tabs;
pub mod url;
pub mod words;

/// The name the pane's screen registers under, the panel's `browser` pane.
pub const NAME: &str = "browser";

pub use screen::BrowserScreen;
