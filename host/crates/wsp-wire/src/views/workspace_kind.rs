// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/workspace-kind.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use crate::types::check::Check;

/// TS: packages/protocol/src/views/workspace-kind.ts:8 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::WorkspaceKind;

impl Check for WorkspaceKind {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["Cloud", "Local", "Place"]))
    }
}
