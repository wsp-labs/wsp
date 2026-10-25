// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/preferences.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use crate::types::check::from_json;
use crate::types::check::{Check, SchemaError};
use crate::types::general_prefs::MidTurn;
use crate::types::general_prefs::NewThreadIn;
use crate::types::general_prefs::NotifyChoice;
use crate::types::general_prefs::OnQuit;
use crate::types::general_prefs::SendKey;
use crate::types::general_prefs::SettleAfter;
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use crate::types::project_look::ProjectHue;
use crate::types::project_look::ProjectIcon;
use crate::types::project_look::ProjectIconHash;
use crate::types::thread_defaults::AgentDefaults;
use crate::types::thread_defaults::AgentDefaultsPatch;
use crate::types::thread_defaults::ProjectOverrides;
use crate::types::thread_defaults::ProjectOverridesPatch;
use crate::types::usage::TurnLimit;
use crate::views::session::ForkedFrom;
use crate::views::session::SessionOrigin;
use crate::views::session::SessionStatus;
use crate::views::session::ThreadCapWait;
use crate::views::session::ThreadPlacement;
use crate::views::session::ThreadWaitingOn;
use crate::views::session_events::SessionEvent;
use crate::views::sub_row::SubRow;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_frames::True;
// ---- Hand-written below: the functions of the unit.

use crate::types::env::{CLOUD_ENV, LABS_ENV};
use crate::types::general_prefs::{general_defaults, patched_general, GeneralPatch, GeneralPreferences};
use crate::types::thread_defaults::Fields;
use std::collections::HashMap;

mod preferences_patch;
mod ssh_behind_kind;

pub use preferences_patch::*;
pub use ssh_behind_kind::*;
