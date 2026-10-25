// SPDX-License-Identifier: AGPL-3.0-only
//! channels.ts:1-280: the facts of a record and the roads to a daemon. Spec: spec/channels_daemon.md.
use std::rc::Rc;

use serde_json::Value;

use wsp_core::daemon_channel::DaemonChannel;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::place_more::PlaceSettings;
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_more::{ProjectRef, WorkspaceAgents};
use wsp_words::exit::HostError;

use super::ChannelsArea;
use crate::account::core_types::{AccountResult, Ask, Frame, Entry, Served, Work};

/// `{ deadlineMs }` of `turnLimitOf` (channels.ts:89): `f64::INFINITY` for no limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnWall {
    pub deadline_ms: f64,
}

/// What `stdioRoad` answers (`StdioRoad`, slate-mcp-on.ts): frames out, events in, the road's end.
pub struct StdioRoad {
    pub send: Served,
    /// Adds a listener; the answer takes it off.
    pub listen: Rc<dyn Fn(Rc<dyn Fn(Value)>) -> Rc<dyn Fn()>>,
    pub channel: Rc<dyn DaemonChannel>,
}

/// A frame's work on this computer's daemon: the `ask` and the home it runs in.
pub type HomeWork<T> = Box<dyn FnOnce(Ask, String) -> futures::future::LocalBoxFuture<'static, AccountResult<T>>>;

impl ChannelsArea {
    /// D1, channels.ts:33.
    pub fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        todo!()
    }

    /// D1, channels.ts:37.
    pub fn cut_before(&self, workspace_id: &str, thread_id: &str) -> bool {
        todo!()
    }

    /// D2, channels.ts:44. `fact` is the TS key (`cwd`, `permissionMode`, ...).
    pub fn resumed_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String> {
        todo!()
    }

    /// D2, channels.ts:55.
    pub fn folder_of(&self, workspace_id: &str, resume: &str) -> Option<String> {
        todo!()
    }

    /// D2, channels.ts:61.
    pub fn access_of(&self, workspace_id: &str, thread_id: &str, resume: Option<&str>) -> Option<String> {
        todo!()
    }

    /// D3, channels.ts:70.
    pub fn home_of(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }

    /// D4, channels.ts:80.
    pub fn provider_of(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }

    /// D4, channels.ts:84.
    pub fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        todo!()
    }

    /// D4, channels.ts:86.
    pub fn settings_at(&self, place_id: Option<&str>) -> Option<PlaceSettings> {
        todo!()
    }

    /// D5, channels.ts:89.
    pub fn turn_limit_of(&self, record: &WorkspaceRecord) -> Option<TurnWall> {
        todo!()
    }

    /// D6, channels.ts:95.
    pub fn agents_held(&self, record: &WorkspaceRecord) -> WorkspaceAgents {
        todo!()
    }

    /// D6, channels.ts:97.
    pub fn spawn_at(&self, place_id: Option<&str>) -> WorkspaceAgents {
        todo!()
    }

    /// D7, channels.ts:101.
    pub fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        todo!()
    }

    /// D7, channels.ts:109.
    pub fn ref_of(&self, project: &ProjectView) -> ProjectRef {
        todo!()
    }

    /// D8, channels.ts:114.
    pub fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        todo!()
    }

    /// D9, channels.ts:124.
    pub fn computer_of(&self, entry: &Entry) -> String {
        todo!()
    }

    /// D10, channels.ts:130.
    pub fn served_by_its_computer(&self, entry: &Entry) -> Option<Served> {
        todo!()
    }

    /// D12, channels.ts:138.
    pub async fn own_daemon_channel(&self, entry: &Entry, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        todo!()
    }

    /// D11, channels.ts:146.
    pub async fn channel_over(&self, reach: &DaemonReachView, name: &str, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        todo!()
    }

    /// D14, channels.ts:158.
    pub async fn with_daemon<T: 'static>(self: &Rc<Self>, entry: &Entry, work: Work<T>) -> AccountResult<T> {
        todo!()
    }

    /// D15, channels.ts:172.
    pub fn brokered(&self, entry: &Entry) -> bool {
        todo!()
    }

    /// D16, channels.ts:180.
    pub fn thread_on(&self, workspace_id: &str, named: Option<&Value>) -> Option<String> {
        todo!()
    }

    /// D16, channels.ts:189: `ask` wrapped so each op of `OP_TOKEN_OPS` carries a token of its own.
    pub fn with_op_tokens(self: &Rc<Self>, entry: &Entry, ask: Ask, hello: futures::future::Shared<futures::future::LocalBoxFuture<'static, Option<i64>>>) -> Ask {
        todo!()
    }

    /// D13, channels.ts:208.
    pub fn reply_of(&self, frame: &Frame, reply: Frame) -> AccountResult<Frame> {
        todo!()
    }

    /// D13, channels.ts:212.
    pub async fn over_channel<T: 'static>(&self, channel: Rc<dyn DaemonChannel>, work: Work<T>) -> AccountResult<T> {
        todo!()
    }

    /// D17, channels.ts:223. `need` defaults to `STDIO_DAEMON_VERSION` at the call sites that name none.
    pub async fn stdio_road(&self, entry: &Entry, need: i64) -> AccountResult<StdioRoad> {
        todo!()
    }

    /// D18, channels.ts:247.
    pub async fn on_this_computer<T: 'static>(&self, work: HomeWork<T>) -> AccountResult<T> {
        todo!()
    }

    /// D19, channels.ts:255.
    pub async fn place_guard(&self, place_id: &str) -> Result<(), HostError> {
        todo!()
    }

    /// D20, channels.ts:268.
    pub async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError> {
        todo!()
    }

    /// D20, channels.ts:272.
    pub async fn blocked_line(&self, place_id: Option<&str>) -> Option<String> {
        todo!()
    }

    /// D20, channels.ts:278.
    pub async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        todo!()
    }
}
