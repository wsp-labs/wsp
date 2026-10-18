// SPDX-License-Identifier: AGPL-3.0-only
//! The command palette (`components/palette/`) and the thread switcher (`components/switcher/`), drawn as layers over
//! the window by [`overlays`], on the pieces [`frame`] holds.

pub mod frame;
pub mod items;
pub mod overlays;
#[allow(clippy::module_inception)]
pub mod palette;
pub mod switcher;

pub use overlays::{attach, init, Overlays};
