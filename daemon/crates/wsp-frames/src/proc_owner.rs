// SPDX-License-Identifier: AGPL-3.0-only
//! Whose a process is and what it is, as the daemon stamps them on each row of proc.watch.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a process is to the person, by what they know it as: a thread's agent, a tool server, a dev server, a test
/// run, a build, a slate's run, a terminal's shell, a browser tab, the app, wsp's own parts, or something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
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
}
