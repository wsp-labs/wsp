// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/runtime.ts.

#![allow(unused_imports)]

use super::helpers::ReqId;
use super::runtime_ops::{dbl_rule, env_key, env_name_issue, opt_rule};
pub use super::runtime_ops::{parse_runtime_request, RuntimeOp, RuntimeRequest};
use crate::types::agents_report::{AgentsTarget, McpScope};
use crate::types::attachments::Attachment;
use crate::types::check::{
    double_option, env_name, items_at_least, items_at_most, leaf, num_above, num_at_least, num_at_most, text_at_least, text_at_most, Zod,
};
use crate::types::conversations::ResumeAsk;
use crate::types::js_number::JsInt;
use crate::types::project_look::ProjectIconHash;
use crate::types::pull_request::{MergeMethod, PullRequestItem, ReactionContent};
use crate::types::recipe_file::RecipeFile;
use crate::types::slate_wire::SlateWireValues;
use crate::types::thread_defaults::AccessChoice;
use crate::types::usage::{ReadingsRange, UsageRange, UsageSplit};
use crate::types::workspace_look::{WorkspaceGlyph, WorkspaceTheme};
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::activity::{ActivityEnded, ProcessesAsk};
use crate::views::app_procs::AppProcList;
use crate::views::golden_image::MachineKind;
use crate::views::place::{PlaceSettingWord, PlaceSetupStep, SudoPassword};
use crate::views::preferences::{EditorId, PreferencesPatch};
use crate::views::project_bundle::TerminalScheme;
use crate::views::session::{ForkSource, SessionOrigin, SessionView, ThreadMarks};
use crate::views::session_events::RunState;
use crate::views::workspace::EventAsker;
use crate::views::workspace::{SeedChoice, WorkspaceOrigin};
use crate::wire::devices::{DaemonFrame, TicketPurpose};
use crate::wire::limits::RelayPort;
use crate::wire::place_link::{DeviceAuthRequest, PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, SealOpenRequest};
use indexmap::IndexMap;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use wsp_frames::{False, True};

mod op_types;
mod runtime_op_check;
mod runtime_op_pass_a;
mod runtime_op_pass_b;
mod settled_thread;

pub use op_types::*;
pub use runtime_op_check::*;
pub use runtime_op_pass_a::*;
pub use runtime_op_pass_b::*;
pub use settled_thread::*;
