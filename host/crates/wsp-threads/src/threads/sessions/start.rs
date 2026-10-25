// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:255-475, 1486-1496: the checks a start makes before it holds the thread's row.
//! Spec: spec/sessions_start.md.
use std::rc::Rc;

use wsp_core::types::api::{ForkOptions, StartOptions};
use wsp_core::types::wiring::SessionHandle;
use wsp_wire::types::attachments::AttachmentRecord;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_words::exit::HostError;

use super::{ForkPlan, SessionsArea, Tree};
use crate::pending::{Entry, Waiting};
use crate::threads::launch::{BuiltAdapter, ThreadLaunch};

/// A start that passed every check, carried to its launch (sessions_launch).
pub struct Prepared {
    pub workspace_id: String,
    pub origin: Option<Caller>,
    /// The options with the harness settled (TS `o`), the title trimmed in.
    pub opts: StartOptions,
    pub prefs: Preferences,
    pub plan: Option<ForkPlan>,
    pub thread_id: String,
    pub opens: bool,
    pub entry: Entry,
    pub replaces: Option<String>,
    pub resume: Option<String>,
    /// The worktree path found removed by hand, where one was.
    pub gone_now: Option<String>,
    pub title: Option<String>,
    pub tree: Tree,
    /// The notify targets this start named (TS `asked`).
    pub asked: Option<Vec<String>>,
    pub notify: Option<Vec<String>>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub notice: Option<String>,
    pub clouds: Option<Vec<String>>,
    /// Whether the thread ran no turn when the checks ran (TS `cleared`).
    pub cleared: bool,
    pub turn_token: String,
    pub launch: ThreadLaunch,
    pub built: BuiltAdapter,
    pub fork_turn: Option<serde_json::Value>,
    pub records: Vec<AttachmentRecord>,
    pub mcp_servers: Option<indexmap::IndexMap<String, McpServerSpec>>,
    pub waiting: Waiting,
}

/// What the checks came to: an answer already, or a start to launch.
pub enum Checked {
    Answered(Rc<dyn SessionHandle>),
    Launch(Box<Prepared>),
}

impl SessionsArea {
    /// sessions.ts:1489, the wrapper over the whole start. Rule S26; the rest is `prepare` then `launch_prepared`.
    pub async fn start_send(&self, workspace_id: &str, opts: StartOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        todo!()
    }

    /// sessions.ts:255-475. `claimed` takes the warm agent's thread where the start claimed one. Rules S1-S25.
    pub async fn prepare(&self, workspace_id: &str, opts: StartOptions, origin: Option<Caller>, claimed: &std::cell::RefCell<Option<String>>) -> Result<Checked, HostError> {
        todo!()
    }

    /// sessions.ts:897.
    pub async fn fork_send(&self, opts: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
        todo!()
    }
}
