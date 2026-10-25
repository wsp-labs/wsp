// SPDX-License-Identifier: AGPL-3.0-only
//! The paths wsp-engine and wsp-collect still import by name. Each item is the real one from wsp-wire or wsp-words;
//! delete this file once those two crates import them from there.
pub use wsp_wire::types::agents_report::{PluginBrings, PluginScope};
pub use wsp_wire::views::golden_image::ToolPin;
pub use wsp_words::exit::HostError;
pub use wsp_words::words::computer::Platform;
