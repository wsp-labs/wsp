// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/place-link.ts.

#![allow(unused_imports)]

use super::helpers::{js_key_order, ReqId};
use crate::types::check::{num_at_least, num_at_most, refine, regex_check, text_at_least, text_at_most};
use crate::types::js_number::JsInt;
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::place::CopyWord;
use crate::wire::capabilities::WorkspaceSize;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// The bytes both ends of a link sign, and the bytes a host signs to refuse a place: wsp-frames builds them for the
/// daemon's side, so the host reads the one function.
pub use wsp_frames::{place_link_transcript, place_refusal_transcript, LinkEphemerals, LinkRole};
use wsp_frames::{False, True};
/// base64 of 32, 44, 64 and 32 bytes, read as zod's base64(n) reads them.
pub use wsp_frames::{PlaceEphemeral, PlaceNonce, PlacePublicKey, PlaceSignature};

mod join_mint;
mod place_file;

pub use join_mint::*;
pub use place_file::*;
