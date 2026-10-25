// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/workspace.ts at a3b870a6e, apart only for the repo's file-size law; workspace.rs re-exports them.

#![allow(unused_imports)]

use super::workspace::*;
use crate::literal;
use crate::types::changes::Checkout;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::pull_request::PullRequestSeen;
use crate::types::start::ReviewDraft;
use crate::types::start::WorkspaceFrom;
use crate::types::tree::TreeFact;
use crate::types::workspace_look::WorkspaceGlyph;
use crate::types::workspace_look::WorkspaceTheme;
use crate::views::workspace_kind::WorkspaceKind;
use crate::wire::capabilities::PauseMode;
use crate::wire::capabilities::WorkspaceSize;
use serde::{Deserialize, Serialize};
/// TS: packages/protocol/src/views/workspace.ts:321 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::Carried;
use wsp_frames::True;

mod project_branch;
mod workspace_status;

pub use project_branch::*;
pub use workspace_status::*;
