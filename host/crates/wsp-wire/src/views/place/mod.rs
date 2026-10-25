// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/place.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

pub use super::place_more::*;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::recipe_file::RecipeFile;
use crate::wire::limits::PortForward;
use crate::wire::limits::RelayPort;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
/// TS: packages/protocol/src/views/place.ts:187 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::CopyWord;

mod place_add_words;
mod place_setup;

pub use place_add_words::*;
pub use place_setup::*;
