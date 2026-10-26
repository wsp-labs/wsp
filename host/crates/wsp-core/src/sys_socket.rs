// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/sys-socket.ts at 65f6b89c7: one socket's live readings, one subscription per
//! workspace or computer per socket. Spec: spec/sockets.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{Map, Value};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::helpers::ReqId;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::SYS_SUBSCRIBE_ONE_OF;
use wsp_words::exit::HostError;

use crate::types::api::Runtime;
use crate::types::wiring::{Detach, SysListener};

enum Held {
    Pending(Rc<Cell<bool>>),
    Live(Detach),
}

impl Held {
    fn release(self) {
        match self {
            Held::Pending(stopped) => stopped.set(true),
            Held::Live(detach) => detach(),
        }
    }
}

fn leave_if_pending(watched: &RefCell<IndexMap<String, Held>>, key: &str, stopped: &Rc<Cell<bool>>) {
    let mine = matches!(watched.borrow().get(key), Some(Held::Pending(flag)) if Rc::ptr_eq(flag, stopped));
    if mine {
        watched.borrow_mut().shift_remove(key);
    }
}

fn ok_frame(id: &ReqId) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("id".into(), serde_json::to_value(id).unwrap_or(Value::Null));
    out.insert("ok".into(), Value::Bool(true));
    out
}

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
        Runtime::watch_place_sys(self, place_id, listener).await
    }

    async fn watch_workspace_sys(&self, workspace_id: &str, listener: SysListener, origin: Option<Caller>) -> Result<Detach, HostError> {
        self.workspaces().watch_sys(workspace_id, listener, origin).await
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
pub fn sys_socket<R: SysWatch + ?Sized + 'static>(
    rt: Rc<R>,
    socket: Rc<dyn SocketSide>,
) -> Rc<dyn Fn(SysAsk, Send, Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>> {
    let watched: Rc<RefCell<IndexMap<String, Held>>> = Rc::new(RefCell::new(IndexMap::new()));
    let held = watched.clone();
    socket.detach(Box::new(move || {
        let all: Vec<Held> = held.borrow_mut().drain(..).map(|(_, h)| h).collect();
        for h in all {
            h.release();
        }
    }));
    Rc::new(move |msg: SysAsk, send: Send, origin: Option<Caller>| -> LocalBoxFuture<'static, Result<(), HostError>> {
        let rt = rt.clone();
        let socket = socket.clone();
        let watched = watched.clone();
        Box::pin(async move {
            if msg.workspace_id.is_some() == msg.place_id.is_some() {
                return Err(HostError::new(SYS_SUBSCRIBE_ONE_OF));
            }
            let key = match &msg.place_id {
                Some(place) => format!("place:{place}"),
                None => format!("workspace:{}", msg.workspace_id.as_deref().unwrap_or_default()),
            };
            if msg.unsubscribe {
                let gone = watched.borrow_mut().shift_remove(&key);
                if let Some(gone) = gone {
                    gone.release();
                }
                send(ok_frame(&msg.id));
                return Ok(());
            }
            if watched.borrow().contains_key(&key) {
                send(ok_frame(&msg.id));
                return Ok(());
            }
            let stopped = Rc::new(Cell::new(false));
            watched.borrow_mut().insert(key.clone(), Held::Pending(stopped.clone()));
            let watching = match msg.place_id.clone() {
                Some(place) => {
                    let to = send.clone();
                    let name = place.clone();
                    let listener: SysListener = Rc::new(move |sample| to(reading("place.sys", "placeId", &name, sample)));
                    rt.watch_place_sys(&place, listener).await
                }
                None => {
                    let workspace = msg.workspace_id.clone().unwrap_or_default();
                    let to = send.clone();
                    let name = workspace.clone();
                    let listener: SysListener = Rc::new(move |sample| to(reading("workspace.sys", "workspaceId", &name, sample)));
                    rt.watch_workspace_sys(&workspace, listener, origin).await
                }
            };
            let detach = match watching {
                Ok(detach) => detach,
                Err(e) => {
                    leave_if_pending(&watched, &key, &stopped);
                    return Err(e);
                }
            };
            if stopped.get() || !socket.open() {
                detach();
                leave_if_pending(&watched, &key, &stopped);
                send(ok_frame(&msg.id));
                return Ok(());
            }
            watched.borrow_mut().insert(key, Held::Live(detach));
            send(ok_frame(&msg.id));
            Ok(())
        })
    })
}

fn reading(kind: &str, id_key: &str, id: &str, sample: SysSample) -> Map<String, Value> {
    let mut out = Map::new();
    out.insert("type".into(), Value::from(kind));
    out.insert(id_key.into(), Value::from(id));
    out.insert("sample".into(), serde_json::to_value(sample).unwrap_or(Value::Null));
    out
}
