// SPDX-License-Identifier: AGPL-3.0-only
//! threads_tree: rows, trees, tokens and who is told. threads.ts:64-97, 303-362, 523-585, 642-661, 1144-1179.
//! Spec: spec/threads_tree.md.
use indexmap::{IndexMap, IndexSet};
use wsp_core::types::internal::TurnAsked;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::session::{SessionView, ThreadWaitingOn};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_words::exit::HostError;

use super::{Entry, ThreadsArea};

/// The marks an error born of a computer that did not answer carries (TR1), named after the TS classes.
pub const SILENT_MARKS: [&str; 6] = ["MachineUnreached", "MachineUnreachableError", "DeadlineError", "LaunchUnanswered", "PlaceAbsent", "DaemonUnanswered"];

/// threads.ts:67 `computerSilent`: TR1.
pub fn computer_silent(e: Option<&HostError>) -> bool {
    todo!()
}

/// `{ notify, by?, road? }` (threads.ts:561).
#[derive(Clone, Debug, PartialEq)]
pub struct NotifyOn {
    pub notify: Vec<String>,
    pub by: Option<ThreadScope>,
    pub road: Option<WorkspaceOrigin>,
}

/// `{ by?, road? }` (threads.ts:615).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TellAs {
    pub by: Option<ThreadScope>,
    pub road: Option<WorkspaceOrigin>,
}

/// What `stopped_behind` reads of a row: its view and the tool calls it holds open.
#[derive(Clone, Debug)]
pub struct CallsOf {
    pub view: SessionView,
    pub calls: Option<IndexMap<String, wsp_core::types::internal::CallSeen>>,
}

/// What `tell_as` and `notify_end` read of a row.
#[derive(Clone, Debug, Default)]
pub struct Registered {
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<WorkspaceOrigin>,
    pub asked: Option<TurnAsked>,
}

impl ThreadsArea {
    /// threads.ts:75: TR2.
    pub fn thread_runs(&self, thread_id: &str) -> bool {
        todo!()
    }
    /// threads.ts:77: TR3. The turn id and its launch in flight.
    pub fn launching_on(&self, thread_id: &str) -> Option<(String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>)> {
        todo!()
    }
    /// threads.ts:84: TR3. The row id and turn id of the thread's running row that holds a handle.
    pub fn running_on(&self, thread_id: &str) -> Option<(String, String)> {
        todo!()
    }
    /// threads.ts:91: TR4.
    pub fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        todo!()
    }
    /// threads.ts:307: TR5, invariant 3.
    pub fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        todo!()
    }
    /// threads.ts:318: TR6.
    pub fn tree_under(&self, thread_id: &str) -> Vec<String> {
        todo!()
    }
    /// threads.ts:333: TR7.
    pub fn restarts(&self) -> IndexMap<String, String> {
        todo!()
    }
    /// threads.ts:346: TR8.
    pub fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&Caller>) -> bool {
        todo!()
    }
    /// threads.ts:353: TR9.
    pub fn settles_thread(&self, thread_id: &str, caller: Option<&Caller>) -> bool {
        todo!()
    }
    /// threads.ts:359: TR10.
    pub fn thread_title(&self, thread_id: &str) -> String {
        todo!()
    }
    /// threads.ts:527: TR11.
    pub fn stopped_behind(&self, s: &CallsOf) -> Option<ThreadWaitingOn> {
        todo!()
    }
    /// threads.ts:546: TR12.
    pub async fn stop_under(&self, thread_id: &str, caller: Option<Caller>) -> Vec<String> {
        todo!()
    }
    /// threads.ts:561: TR13.
    pub fn notify_on(&self, thread_id: &str) -> Option<NotifyOn> {
        todo!()
    }
    /// threads.ts:572: TR14.
    pub fn tellable(row: Option<&SessionView>) -> bool {
        todo!()
    }
    /// threads.ts:576: TR15.
    pub fn notify_reach(&self, from: &[String]) -> IndexSet<String> {
        todo!()
    }
    /// threads.ts:615: TR16.
    pub fn tell_as(s: &Registered) -> TellAs {
        todo!()
    }
    /// threads.ts:658: TR17.
    pub fn failure_of(&self, row_id: &str, turn_id: &str) -> Option<HostError> {
        todo!()
    }
    /// threads.ts:1149: TR18.
    pub fn usage_computer_of(r: &WorkspaceRecord) -> String {
        todo!()
    }
    /// threads.ts:1152: TR19. `vault` `None` reads `ctx.vault()`. Answers `"token"`, `"key"` or `None`.
    pub fn vaulted_for(&self, agent: &str, login_stands: Option<bool>, vault: Option<&IndexMap<String, String>>) -> Option<&'static str> {
        todo!()
    }
    /// threads.ts:1158: TR20. usage.ts's `accountOf` over this workspace's computer: `(key, label)`.
    pub fn usage_account_of(&self, entry: &Entry, harness: &str, named: Option<(String, Option<String>)>) -> (String, String) {
        todo!()
    }
    /// threads.ts:1169: TR20.
    pub async fn limit_details_due(&self, entry: &Entry, harness: &str) -> bool {
        todo!()
    }
}
