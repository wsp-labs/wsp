// SPDX-License-Identifier: AGPL-3.0-only
//! channels.ts:280-580: whose shell is whose, the kill guard and the port roots. Spec: spec/channels_ptys.md.
use std::rc::Rc;

use indexmap::IndexMap;

use wsp_core::daemon_channel::DaemonChannel;

use super::ChannelsArea;
use crate::account::core_types::{AccountResult, Live};
use crate::account::proc_scopes::ThreadTerminal;

impl ChannelsArea {
    /// Y1, channels.ts:299: `WORKSPACE_FRAMES` then the guest road's own.
    pub fn guest_road_frames() -> Vec<&'static str> {
        todo!()
    }

    /// Y2, channels.ts:312.
    pub fn held_to_owner(self: &Rc<Self>, owner: &str, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        todo!()
    }

    /// Y4, channels.ts:340.
    pub fn app_main(&self) -> Vec<i64> {
        todo!()
    }

    /// Y3, channels.ts:358.
    pub fn kill_guarded(self: &Rc<Self>, channel: Rc<dyn DaemonChannel>, here: bool) -> Rc<dyn DaemonChannel> {
        todo!()
    }

    /// Y5, channels.ts:382: the workspace's tags, read once from the store.
    pub async fn tags_of(self: &Rc<Self>, workspace_id: &str) -> Rc<std::cell::RefCell<IndexMap<String, String>>> {
        todo!()
    }

    /// Y8, channels.ts:398.
    pub fn thread_terminals(&self) -> Vec<ThreadTerminal> {
        todo!()
    }

    /// Y6, channels.ts:410: queues one write of the tags as they stand when it runs.
    pub fn keep_tags(self: &Rc<Self>, workspace_id: &str) {
        todo!()
    }

    /// Y6: the workspace's last queued tag write, settled (None when none is out), for a caller that must see it land.
    pub fn tag_write(&self, workspace_id: &str) -> Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>> {
        todo!()
    }

    /// Y5, channels.ts:440.
    pub fn thread_tagged(self: &Rc<Self>, workspace_id: &str, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        todo!()
    }

    /// Y9, channels.ts:472.
    pub async fn end_thread_shells(self: &Rc<Self>, entry: &Live, thread_id: &str) -> AccountResult<()> {
        todo!()
    }

    /// Y2, channels.ts:483.
    pub fn shared_daemon_replaced(&self) {
        todo!()
    }

    /// Y10, channels.ts:495.
    pub fn arm_roots_recheck(self: &Rc<Self>) {
        todo!()
    }

    /// Y10, channels.ts:507.
    pub fn port_roots_of(&self, workspace_id: &str) -> Vec<i64> {
        todo!()
    }

    /// Y10, channels.ts:521.
    pub fn port_roots_moved(&self, workspace_id: &str) {
        todo!()
    }

    /// Y11, channels.ts:529. `ptys` is the workspace's entry of `port_pids`.
    pub fn rooted_ports(self: &Rc<Self>, workspace_id: &str, folder: &str, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        todo!()
    }
}
