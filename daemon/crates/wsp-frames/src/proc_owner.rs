// SPDX-License-Identifier: AGPL-3.0-only
//! Whose a process is: the threads the host names to a computer's daemon, and the owner and kind the daemon stamps on
//! each row of proc.watch off them.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::validate::{bounded, bounded_opt};

/// How many threads one proc.scopes names, and how many pids of each list one thread names.
pub const PROC_SCOPES_MAX: usize = 256;
pub const PROC_SCOPE_PIDS_MAX: usize = 64;

/// What a process is to the person, the way Chrome's task manager names its rows: one list the daemon and the page
/// share. The daemon stamps only what it knows by fact, a thread's agent, a slate's
/// run and a tool server wsp started; the page names the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ProcKind {
    Agent,
    Tool,
    Dev,
    Test,
    Build,
    Slate,
    Terminal,
    Tab,
    App,
    Wsp,
    Other,
}

/// Which rung of the ladder named the owner: the process or a parent of it is one of the thread's own (`root`), a
/// Mac's fork tracker followed it from one of those (`fork`), its group was seen under the thread (`group`), its
/// environment carries the thread's mark (`env`), it stands in the thread's cgroup (`cgroup`), it runs in a
/// workspace's folder (`folder`), or it is in wsp's own coalition on a Mac (`coalition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ProcOwnedBy {
    Root,
    Fork,
    Group,
    Env,
    Cgroup,
    Folder,
    Coalition,
}

/// The owner a row carries: a thread for the first five rungs, a workspace alone for a folder, neither for a process
/// that is wsp's with no thread known.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcOwner {
    pub by: ProcOwnedBy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub thread: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub workspace: Option<String>,
    /// The thread is a warm agent's, started ahead of a send: no thread by this id exists until a send claims it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub warm: bool,
}

/// A process the host itself started for a thread, and what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcScopePid {
    pub pid: u32,
    pub kind: ProcKind,
}

/// One thread as the host names it to the computer its processes run on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ProcScope {
    #[serde(deserialize_with = "bounded::<_, 1, 64>")]
    pub thread: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded_opt::<_, 64>")]
    #[ts(optional)]
    pub workspace: Option<String>,
    /// The thread's agent processes: its running turn's, and the one kept up for its next turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "pids")]
    #[ts(as = "Option<Vec<u32>>", optional)]
    pub agents: Vec<u32>,
    /// The process groups its turns led, kept by the host for as long as each has members.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "pids")]
    #[ts(as = "Option<Vec<u32>>", optional)]
    pub groups: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded_opt::<_, 4096>")]
    #[ts(optional)]
    pub folder: Option<String>,
    /// A warm agent's: the id is the one its thread takes when a send claims it, and no thread has it yet.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub warm: bool,
    /// The cgroup a box puts the thread's turns in.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "bounded_opt::<_, 512>")]
    #[ts(optional)]
    pub cgroup: Option<String>,
    /// What the host started for the thread itself: slate runs and tool servers.
    #[serde(default, skip_serializing_if = "Vec::is_empty", deserialize_with = "scope_pids")]
    #[ts(as = "Option<Vec<ProcScopePid>>", optional)]
    pub pids: Vec<ProcScopePid>,
}

fn pids<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u32>, D::Error> {
    let list = Vec::<u32>::deserialize(d)?;
    if list.len() > PROC_SCOPE_PIDS_MAX {
        return Err(de::Error::custom(format!("at most {PROC_SCOPE_PIDS_MAX} pids, got {}", list.len())));
    }
    Ok(list)
}

fn scope_pids<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ProcScopePid>, D::Error> {
    let list = Vec::<ProcScopePid>::deserialize(d)?;
    if list.len() > PROC_SCOPE_PIDS_MAX {
        return Err(de::Error::custom(format!("at most {PROC_SCOPE_PIDS_MAX} pids, got {}", list.len())));
    }
    Ok(list)
}

pub(crate) fn scopes<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ProcScope>, D::Error> {
    let list = Vec::<ProcScope>::deserialize(d)?;
    if list.len() > PROC_SCOPES_MAX {
        return Err(de::Error::custom(format!("at most {PROC_SCOPES_MAX} threads, got {}", list.len())));
    }
    Ok(list)
}
