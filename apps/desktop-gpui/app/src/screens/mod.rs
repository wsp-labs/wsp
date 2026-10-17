// SPDX-License-Identifier: AGPL-3.0-only
//! The screens the shell routes to. A screen lives in its own folder here and plugs in with two lines in this file:
//! its `pub mod`, and one arm in [`screen`], which both the shell's routing and `desktop-gpui --screen <name>` call.

pub mod app_shell;

use gpui_kit::{App, Window};

pub use app_shell::{mount, navigate, Host, Mounted, Pane, Parts, Place, Route, Screen};

/// How a screen is built for the frame: [`mount`] of the screen's type.
pub type Mount = fn(&Host, &mut Window, &mut App) -> Mounted;

/// The screen named `name`, or None for a name no screen has.
#[allow(clippy::match_single_binding)]
pub fn screen(name: &str) -> Option<Mount> {
    match name {
        _ => None,
    }
}
