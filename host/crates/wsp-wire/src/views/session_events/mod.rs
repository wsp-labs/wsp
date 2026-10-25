// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/session-events.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

pub use super::session_events_more::*;
use crate::literal;
use crate::types::attachments::AttachmentRecord;
use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::usage::TurnLimit;
use crate::types::usage::UsageTokens;
use crate::views::session::SessionOrigin;
use crate::views::session::SessionView;
use crate::views::session::TurnRefusal;
use serde::{Deserialize, Serialize};
use wsp_frames::True;
// ---- Hand-written below: the functions of the unit.

use crate::types::check::js_trim;
use crate::types::env::{HOST_KEY_ENV, HOST_TOKEN_ENV, HOST_URL_ENV, TURN_TOKEN_ENV};
use std::collections::HashMap;

mod agent_starting_ms;
mod turn_changed_file;

pub use agent_starting_ms::*;
pub use turn_changed_file::*;
