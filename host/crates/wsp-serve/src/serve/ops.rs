// SPDX-License-Identifier: AGPL-3.0-only
//! Sessions, images, usage, forwards, folders, agents, editors and the host's own doors. serve.ts:333-347, 419-440,
//! 857-862, 1474-1633, 1643-1655, 1679-1883. Spec: spec/serve_ops.md.
use std::rc::Rc;

use wsp_wire::views::project_bundle::HostFolderListing;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use super::answer::Handled;
use super::door::Served;
use super::socket::Socket;

/// What `sessions.history` refuses a window with no thread with: the sentence and the fix. serve.ts:1495.
pub const HISTORY_WINDOW_LINE: &str = "before and limit page one thread's events.";
pub const HISTORY_WINDOW_FIX: &str = "Name the thread with threadId.";
/// What `places.readings` naming neither target is refused with, kind usage. serve.ts:1597.
pub const READINGS_TARGET_REFUSAL: &str = "places.readings names a placeId or a workspaceId";
/// The default image name `snapshots.*` read. serve.ts:1569, 1610.
pub const DEFAULT_IMAGE: &str = "default";

/// Answers the ops of this unit and sends their replies; `NotMine` for any other op.
pub async fn answer_ops(socket: &Rc<Socket>, frame: &serde_json::Value, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Handled, HostError> {
    todo!()
}

/// One level of folders on a joined computer, read by its own daemon over its link (serve.ts:419-440, spec O15).
pub async fn place_folders(served: &Served, place_id: &str, dir: Option<String>, hidden: Option<bool>, repos: bool, origin: Option<Caller>) -> Result<HostFolderListing, HostError> {
    todo!()
}

/// `forwards.stop`'s refusal for a forward the source does not hold. serve.ts:1652.
pub fn nothing_forwarding_line(port: u16) -> String {
    todo!()
}
