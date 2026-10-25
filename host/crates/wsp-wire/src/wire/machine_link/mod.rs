// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/wire/machine-link.ts.

#![allow(unused_imports)]

use super::helpers::ReqId;
pub use super::machine_replies::{
    ContainerPlacedBy, ProcCost, SysHistoryReply, SysPoint, TranscriptsListReply, TranscriptsReadReply, UsageLogsReply,
};
use crate::types::check::parse;
use crate::types::check::{num_above, num_at_least, num_at_most, refine, regex_check, text_at_least, text_at_most};
use crate::types::js_number::JsInt;
use crate::types::{Check, JsNumber, SchemaError};
use crate::views::golden_image::MachineKind;
use crate::views::place::{MachineBind, MachineShare, WorkspaceCopy};
use crate::views::workspace::{MachineFacts, MachineState};
use crate::wire::capabilities::{Capabilities, WorkspaceSize};
use crate::wire::daemon::{CgroupMemory, ProcEntry};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_frames::{False, True};

mod local_runs_one_fix;
mod machine_handle;
mod said_fix;

pub use local_runs_one_fix::*;
pub use machine_handle::*;
pub use said_fix::*;
