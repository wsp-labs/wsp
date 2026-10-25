// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/sys-socket.ts at 65f6b89c7: one socket's live readings, one subscription per
//! workspace or computer per socket. Spec: spec/sockets.md.
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use serde_json::{Map, Value};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::helpers::ReqId;
use wsp_words::exit::HostError;

use crate::types::api::Runtime;
use crate::types::wiring::{Detach, SysListener};

/// Where a frame for this socket goes: an answer or a pushed reading, as the door writes it.
pub type Send = Rc<dyn Fn(Map<String, Value>)>;

/// What the door knows of its socket: whether it is still open, and the leaves it runs when it closes.
/// sys-socket.ts:15 `socket`.
pub trait SocketSide {
    fn open(&self) -> bool;
    fn detach(&self, leave: Box<dyn FnOnce()>);
}

/// The two watches the handler asks of the runtime (sys-socket.ts:15 `Pick<Runtime, "workspaces" | "watchPlaceSys">`).
#[async_trait(?Send)]
pub trait SysWatch {
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<Detach, HostError>;
    async fn watch_workspace_sys(&self, workspace_id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError>;
}

#[async_trait(?Send)]
impl<R: Runtime + ?Sized> SysWatch for R {
    async fn watch_place_sys(&self, place_id: &str, listener: SysListener) -> Result<Detach, HostError> {
        todo!()
    }

    async fn watch_workspace_sys(&self, workspace_id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        todo!()
    }
}

/// `sys.subscribe` or `sys.unsubscribe`, as the door hands it on. sys-socket.ts:10.
#[derive(Clone, Debug, PartialEq)]
pub struct SysAsk {
    pub id: ReqId,
    pub unsubscribe: bool,
    pub workspace_id: Option<String>,
    pub place_id: Option<String>,
}

/// One socket's handler of every sys op. Registers the socket's leave at once, which detaches every watch the socket
/// holds. Refuses an ask naming both or neither of `workspace_id` and `place_id` with SYS_SUBSCRIBE_ONE_OF before
/// anything is sent. sys-socket.ts:15.
pub fn sys_socket(rt: Rc<dyn SysWatch>, socket: Rc<dyn SocketSide>) -> Rc<dyn Fn(SysAsk, Send, Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>> {
    todo!()
}
