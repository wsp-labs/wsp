// SPDX-License-Identifier: AGPL-3.0-only
//! channels.ts:580-934: the served and copy channels, shell tokens, where a child starts, one repository. Spec:
//! spec/channels_copy.md.
use std::collections::BTreeSet;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::types::internal::ChildStart;
use wsp_wire::views::workspace::{Caller, ProjectView};

use super::ChannelsArea;
use crate::account::core_types::{AccountResult, Frame, Entry, Served};

/// `"fork" | "run"`: the road a child starts by (channels.ts:845).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildRoad {
    Fork,
    Run,
}

/// What `terminalToken` answers (channels.ts:672).
#[derive(Clone, Debug, PartialEq)]
pub struct ShellToken {
    pub device_id: String,
    pub env: IndexMap<String, String>,
}

impl ChannelsArea {
    /// C1-C5, channels.ts:587. `carries` is the client's frame list.
    pub async fn served_channel(self: &Rc<Self>, entry: &Entry, served: Served, on_event: Rc<dyn Fn(Value)>, carries: &[&str]) -> AccountResult<Rc<dyn DaemonChannel>> {
        todo!()
    }

    /// C6, channels.ts:672. `named` is the frame's `thread` value, whatever its type.
    pub async fn terminal_token(self: &Rc<Self>, entry: &Entry, named: Option<&Value>) -> AccountResult<Option<ShellToken>> {
        todo!()
    }

    /// C7, channels.ts:685.
    pub async fn shell_sign_ins(self: &Rc<Self>, entry: &Entry, thread_id: &str, token: &str) -> IndexMap<String, String> {
        todo!()
    }

    /// C8, channels.ts:699.
    pub async fn write_terminal(self: &Rc<Self>, entry: &Entry, own: &str, line: &str) {
        todo!()
    }

    /// C9, channels.ts:719: the workspace's shell tokens, read once from the store.
    pub async fn shell_tokens_of(self: &Rc<Self>, workspace_id: &str) -> Rc<std::cell::RefCell<IndexMap<String, String>>> {
        todo!()
    }

    /// C9, channels.ts:747.
    pub async fn shell_opened(self: &Rc<Self>, workspace_id: &str, pty_id: &str, device_id: &str) {
        todo!()
    }

    /// C9, channels.ts:751.
    pub async fn shells_ended(self: &Rc<Self>, workspace_id: &str, pty_ids: &[String]) {
        todo!()
    }

    /// C9, channels.ts:764.
    pub async fn shells_answered(self: &Rc<Self>, workspace_id: &str, frame: &Frame, reply: &Frame) {
        todo!()
    }

    /// C9, channels.ts:774.
    pub async fn shell_held_devices(self: &Rc<Self>) -> AccountResult<BTreeSet<String>> {
        todo!()
    }

    /// C10-C12, channels.ts:791.
    pub async fn copy_channel(self: &Rc<Self>, entry: &Entry, on_event: Rc<dyn Fn(Value)>, carries: &[&str]) -> AccountResult<Rc<dyn DaemonChannel>> {
        todo!()
    }

    /// C13, channels.ts:845.
    pub async fn lead_start(self: &Rc<Self>, lead: &Entry, child: &str, road: ChildRoad) -> AccountResult<ChildStart> {
        todo!()
    }

    /// C14, channels.ts:884.
    pub async fn start_child_on(self: &Rc<Self>, child: &Entry, branch: &str, lead: &str) -> AccountResult<String> {
        todo!()
    }

    /// C15, channels.ts:899.
    pub fn repository_of(remote: &str) -> Option<String> {
        todo!()
    }

    /// C15, channels.ts:910. The TS holds the read per caller object for one request; this reads every call (open
    /// point 1 in the report: a borrowed `Caller` has no identity that outlives the request safely).
    pub fn origin_now(&self, project: &ProjectView, caller: Option<&Caller>) -> String {
        todo!()
    }

    /// C15, channels.ts:923.
    pub fn same_repository(&self, a: &ProjectView, b: &ProjectView, caller: Option<&Caller>) -> bool {
        todo!()
    }
}
