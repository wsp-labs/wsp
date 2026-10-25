// SPDX-License-Identifier: AGPL-3.0-only
//! The later shapes of packages/protocol/src/views/place.ts at a3b870a6e, apart only for the repo's file-size law; place.rs re-exports them.

#![allow(unused_imports)]

use super::place::*;
use crate::types::agents_report::AgentSignInState;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::recipe_file::RecipeFile;
use crate::views::place_kind::PlaceKind;
use crate::views::workspace::ProjectAddStage;
use crate::views::workspace::ProjectView;
use crate::views::workspace::WorkspaceAgents;
use crate::wire::capabilities::MachineSizeOffer;
use crate::wire::capabilities::WorkspaceSize;
use crate::wire::limits::PortForward;
use crate::wire::limits::RelayPort;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_frames::True;

mod computer_cap;
mod place_view;

pub use computer_cap::*;
pub use place_view::*;
