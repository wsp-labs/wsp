// SPDX-License-Identifier: AGPL-3.0-only
//! The switch every frame of a socket that is let in goes through (serve.ts:850-878, 1884-1895): the prefix ops,
//! then the op units, then the catch. Spec: spec/serve_ops.md, O1-O3.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::Value;
use wsp_core::sign_in_socket::{sign_in_op, sign_in_socket};
use wsp_core::sys_socket::Send;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::{refusal_fields, HostError};

use super::ops::answer_ops;
use super::places::{answer_places, refused_off_own_road};
use super::served::Payload;
use super::socket::{SignInRoad, Socket};
use super::workspaces::answer_workspaces;
use crate::serve_app_procs::{is_app_procs_request, AppProcsSocket, Push};
use crate::serve_broker::{answer_broker, is_broker_op};
use crate::serve_conversations::{answer_conversations, is_conversations_request};
use crate::serve_processes::{answer_processes, is_processes_request};
use crate::serve_projects::{answer_project, is_project_request};
use crate::serve_slates::{answer_slate, is_slate_request};

/// Whether an op unit answered the frame, or it was not that unit's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handled {
    Answered,
    NotMine,
}

/// Answers one parsed frame of a socket that is let in. `frame` is the frame as it parsed, for the refusal log.
/// Every reply goes through `Socket::answer`; an error any op raises becomes the refusal of spec O2 and tells
/// `opts.failed`.
pub fn answer_frame(socket: &Rc<Socket>, frame: serde_json::Value, msg: RuntimeRequest) -> LocalBoxFuture<'static, ()> {
    let socket = socket.clone();
    async move {
        let origin = socket.road().origin(msg.origin);
        if let Err(error) = switch(&socket, &frame, &msg, origin).await {
            let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
            socket.answer(Some(&frame), failure(&id, &error), None);
            if let Some(failed) = &socket.served.opts.failed {
                failed(msg.op.name(), &error);
            }
        }
    }
    .boxed_local()
}

/// `{id, ok: true, ...fields}`: the reply a frame is answered with under its id.
pub(super) fn ok_reply(id: &Value, fields: Payload) -> Payload {
    let mut payload = Payload::new();
    payload.insert("id".into(), id.clone());
    payload.insert("ok".into(), Value::Bool(true));
    payload.extend(fields);
    payload
}

/// `{id, ok: false, error, kind?}`: a refusal the switch or an op sends beside its sentence.
pub(super) fn refusal_reply(id: &Value, error: &str, kind: Option<&str>) -> Payload {
    let mut payload = Payload::new();
    payload.insert("id".into(), id.clone());
    payload.insert("ok".into(), Value::Bool(false));
    payload.insert("error".into(), Value::from(error));
    if let Some(kind) = kind {
        payload.insert("kind".into(), Value::from(kind));
    }
    payload
}

/// The prefix ops, the op units, in the TS order (serve.ts:869-878).
async fn switch(socket: &Rc<Socket>, frame: &Value, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<(), HostError> {
    let served = socket.served.clone();
    let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
    let prefixed = if is_slate_request(msg) {
        let holds = socket.slate_holds.clone();
        let detach = |leave: Box<dyn FnOnce()>| socket.detach(leave);
        Some(answer_slate(&(*served.rt).slates(), msg, origin.clone(), &holds, &detach).await?)
    } else if is_project_request(msg) {
        Some(answer_project((*served.rt).projects(), msg, origin.clone()).await?)
    } else if is_app_procs_request(msg) {
        let pushed = socket.clone();
        let push: Push = Rc::new(move |payload: Payload| pushed.send(payload));
        let watcher = AppProcsSocket { send: push, detaches: socket.detaches.clone(), own: socket.road().own_road() };
        Some(served.app_procs.answer(msg, &watcher)?)
    } else if is_processes_request(msg) {
        Some(answer_processes(&*served.rt, msg, origin.clone()).await?)
    } else if is_conversations_request(msg) {
        Some(answer_conversations((*served.rt).conversations(), msg, origin.clone()).await?)
    } else if is_broker_op(msg) {
        Some(answer_broker(served.opts.broker.as_ref(), msg, socket.road().own_road()).await?)
    } else {
        None
    };
    if let Some(fields) = prefixed {
        socket.answer(Some(frame), ok_reply(&id, fields), None);
        return Ok(());
    }
    if let Some(sign_in) = sign_in_op(&msg.op) {
        if refused_off_own_road(socket, frame, msg) {
            return Ok(());
        }
        let answering = socket.clone();
        let answering_frame = frame.clone();
        let send: Send = Rc::new(move |payload: Payload| answering.answer(Some(&answering_frame), payload, None));
        let road = sign_in_road(socket);
        road(sign_in, send, origin).await?;
        return Ok(());
    }
    if answer_places(socket, frame, msg, origin.clone()).await? == Handled::Answered {
        return Ok(());
    }
    if answer_workspaces(socket, frame, msg, origin.clone()).await? == Handled::Answered {
        return Ok(());
    }
    answer_ops(socket, frame, msg, origin).await?;
    Ok(())
}

/// The socket's sign-in road, built the first time a sign-in op asks (sign-in-socket.ts:26, serve.ts:562).
fn sign_in_road(socket: &Rc<Socket>) -> SignInRoad {
    if let Some(road) = socket.sign_in_road.borrow().clone() {
        return road;
    }
    let cloud = socket.served.opts.broker.as_ref().and_then(|broker| broker.cloud_sign_in());
    let road = sign_in_socket(socket.served.rt.clone(), socket.clone(), cloud);
    *socket.sign_in_road.borrow_mut() = Some(road.clone());
    road
}

/// What the catch sends: the refusal of spec O2, `kind` and `fix` where the error carries them, then the lists.
fn failure(id: &Value, error: &HostError) -> Payload {
    let mut payload = Payload::new();
    payload.insert("id".into(), id.clone());
    payload.insert("ok".into(), Value::Bool(false));
    payload.insert("error".into(), Value::from(error.message.as_str()));
    if let Some(kind) = &error.kind {
        payload.insert("kind".into(), Value::from(kind.as_str()));
    }
    if let Some(fix) = &error.fix {
        payload.insert("fix".into(), Value::from(fix.as_str()));
    }
    if let Ok(Value::Object(lists)) = serde_json::to_value(refusal_fields(error)) {
        payload.extend(lists);
    }
    payload
}
