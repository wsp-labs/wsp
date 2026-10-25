// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/golden-image.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::init_job::LoginChoice;
use crate::types::init_job::LoginState;
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::wire::capabilities::WorkspaceSize;
use serde::{Deserialize, Serialize};
/// TS: packages/protocol/src/views/golden-image.ts:10 at a3b870a6e.
/// The daemon's own enum: wsp-frames declares it, with the same values in the same order.
pub use wsp_frames::MachineKind;
use wsp_frames::True;

mod agents_bin;
mod recipe;

pub use agents_bin::*;
pub use recipe::*;
