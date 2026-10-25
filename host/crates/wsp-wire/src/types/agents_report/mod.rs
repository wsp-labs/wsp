// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/agents-report.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use crate::literal;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::thread_defaults::AgentSetupView;
use crate::wire::capabilities::PauseMode;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_frames::True;

mod server_add;
mod skill_preview_bytes;

pub use server_add::*;
pub use skill_preview_bytes::*;
