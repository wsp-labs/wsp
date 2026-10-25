// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/account/box-panels.ts at 65f6b89c7: the panes of a folder on a joined computer over
//! the one link that computer holds. Spec: spec/box_panels.md.
use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_wire::wire::daemon::ProcScope;

use crate::account::core_types::{AccountResult, Frame, Entry, Release, Served};

/// box-panels.ts:21: the most cgroups one watch names, the daemon's own cap.
pub const WATCH_CGROUPS_MAX: usize = 16;

/// box-panels.ts:37: the ops a pane's channel into a folder there answers itself, in the TS set's order.
pub const PANEL_OPS: [&str; 9] = ["ports.watch", "sys.watch", "sys.unwatch", "proc.watch", "proc.unwatch", "proc.inspect", "proc.kill", "container.stop", "pty.create"];

/// B1, box-panels.ts:26. `None` in an ask is "every process"; the answer `None` is the whole table.
pub fn proc_watch_scope(asks: &[Option<Vec<String>>]) -> Option<Vec<String>> {
    todo!()
}

/// B2, box-panels.ts:50: the refusal frame, or None where the daemon names watches.
pub fn named_watch_refusal(name: &str, daemon_version: Option<i64>) -> Option<Frame> {
    todo!()
}

/// One thread's session row as the cgroup order reads it (box-panels.ts:70).
#[derive(Clone, Debug, PartialEq)]
pub struct StartedRow {
    pub workspace_id: String,
    pub thread_id: Option<String>,
    pub started_at: Option<f64>,
}

/// What the opener reads off the runtime's context.
#[async_trait(?Send)]
pub trait BoxPanelsCtx {
    /// `bus.on("workspace.deleted", ...)`: `f` gets the deleted workspace's id.
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>);
    /// `ctx.sessions.values()`' views, in map order.
    fn session_rows(&self) -> Vec<StartedRow>;
    /// `ctx.threadRecords`: thread id and workspace id, in map order.
    fn thread_records(&self) -> Vec<(String, String)>;
    /// `placeDoor.link(placeId)`: the link handle as an identity, None where none stands.
    fn link(&self, place_id: &str) -> Option<Rc<()>>;
    /// `placeDoor.nameOf(placeId)`.
    fn name_of(&self, place_id: &str) -> String;
    /// `(await placeDoor.reportOf(placeId))?.daemonVersion`.
    async fn daemon_version(&self, place_id: &str) -> Option<i64>;
    /// `placeDoor.watchSys(placeId, () => {})`: the hold's release, or the reason it was refused.
    async fn watch_sys(&self, place_id: &str) -> Result<Release, String>;
    /// `ctx.scopesOn(placeId)` (proc_scopes P6).
    fn scopes_on(&self, place_id: &str) -> Vec<ProcScope>;
    /// `ctx.threadEnv(entry)`, keys in order.
    fn thread_env(&self, entry: &Entry) -> IndexMap<String, String>;
    /// A `void` promise: runs on the host's thread, nobody waits for it.
    fn spawn(&self, f: futures::future::LocalBoxFuture<'static, ()>);
}

/// box-panels.ts:39 `BoxPanels`: one pane's channel parts.
pub trait BoxPanels {
    fn answers(&self, op: &str) -> bool;
    fn send(&self, frame: Frame) -> futures::future::LocalBoxFuture<'static, AccountResult<Frame>>;
    /// Whether an event the link carried is this pane's.
    fn heard(&self, event: &Value) -> bool;
    fn close(&self);
}

/// The pushes each folder's panes register to name their ports again (`watchers`), by workspace.
pub type Watchers = Rc<RefCell<IndexMap<String, Vec<Rc<dyn Fn()>>>>>;

/// The watch each thread's cgroup is in, per folder (`placements`).
pub type Placements = Rc<RefCell<IndexMap<String, IndexMap<String, usize>>>>;

/// box-panels.ts:60 `boxPanelsOpener`: one per runtime.
pub struct BoxPanelsOpener {
    pub(crate) ctx: Rc<dyn BoxPanelsCtx>,
    pub(crate) watchers: Watchers,
    pub(crate) placements: Placements,
}

impl BoxPanelsOpener {
    /// Registers the `workspace.deleted` listener that drops a folder's placements (B11).
    pub fn new(ctx: Rc<dyn BoxPanelsCtx>, watchers: Watchers, placements: Placements) -> Rc<BoxPanelsOpener> {
        todo!()
    }

    /// box-panels.ts:78: one pane on `place_id`'s link, its frames served by `served`, its terminals in `folder`.
    pub fn open(self: &Rc<Self>, entry: &Entry, place_id: &str, served: Served, folder: &str) -> Rc<dyn BoxPanels> {
        todo!()
    }
}
