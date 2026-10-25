// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/activity.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsInt;
use crate::types::js_number::JsNumber;
use serde::{Deserialize, Serialize};
use wsp_frames::ProcKind;
// ---- Hand-written below: the functions of the unit.

use crate::types::thread_cgroup::{cgroup_memory, in_cgroup};
use crate::views::activity_fold::PidRow;
use crate::wire::daemon::{by_proc_column, ProcSort};
use crate::wire::daemon_version::place_behind_line;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use wsp_frames::{CgroupMemory, ProcEntry, ProcOwnedBy};

mod processes_choices;
mod rows;

pub use processes_choices::*;
pub use rows::*;
