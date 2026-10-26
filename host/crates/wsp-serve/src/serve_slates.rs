// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-slates.ts at 65f6b89c7: the slate ops a socket asks, answered off the
//! runtime's slates, as what the reply carries beside id and ok. Spec: spec/serve_helpers.md, H22-H23.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use wsp_slates::pending::wire::{SlateEventAnswer, SlateReadAnswer};
use wsp_slates::slates::{
    ApprovalScope, ApproveParams, CatalogParams, EventParams, EventScope, Press, ReadParams, SlateTarget, Slates, StateParams, WriteParams,
};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::slate_wire::SlateWireValues;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::{SlatesApproveScope, SlatesEventEvent};
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

/// `msg.op.startsWith("slates.")`.
pub fn is_slate_request(msg: &RuntimeRequest) -> bool {
    msg.op.name().starts_with("slates.")
}

/// The holds one socket took, by thread and sources, oldest first under each key. serve-slates.ts:10-15. Every hold
/// also goes on the socket's detaches, which its close runs.
#[derive(Default)]
pub struct SlateHolds {
    pub held: IndexMap<String, Vec<Rc<dyn Fn()>>>,
}

/// The key a hold is kept under: `<threadId>\u0000<sources sorted, joined with ",">`. serve-slates.ts:57, 63.
pub fn hold_key(thread_id: &str, sources: &[String]) -> String {
    let mut sorted = sources.to_vec();
    sorted.sort();
    format!("{thread_id}\u{0}{}", sorted.join(","))
}

/// `answerSlate(slates, msg, origin, holds)`. serve-slates.ts:17-74. `detach` pushes a leave onto the socket's
/// detaches.
pub async fn answer_slate(
    slates: &Rc<dyn Slates>,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
    holds: &Rc<RefCell<SlateHolds>>,
    detach: &dyn Fn(Box<dyn FnOnce()>),
) -> Result<Payload, HostError> {
    match &msg.op {
        RuntimeOp::SlatesGet { thread_id, .. } => {
            let slate = slates.get(thread_id).await?.map_or(Value::Null, |view| view.to_json());
            one("slate", slate)
        }
        RuntimeOp::SlatesWrite { thread_id, turn_token, text, document, check, if_version, values, press, .. } => {
            let params = WriteParams {
                target: target(thread_id, turn_token),
                text: text.clone(),
                document: document
                    .as_ref()
                    .map(|document| Value::Object(document.iter().map(|(name, value)| (name.clone(), value.0.clone())).collect())),
                check: *check,
                if_version: if_version.map(JsInt::get),
                values: values.as_ref().map(values_of),
                press: press.as_ref().map(|press| Press {
                    piece: press.piece.clone(),
                    index: press.index.map(JsInt::get),
                    action: press.action.map(JsInt::get),
                }),
            };
            spread(&slates.write(params, origin.as_ref()).await?)
        }
        RuntimeOp::SlatesState { thread_id, turn_token, values, start, if_version, .. } => {
            let params = StateParams {
                target: target(thread_id, turn_token),
                values: values.as_ref().map(values_of),
                start: start.clone(),
                if_version: if_version.map(JsInt::get),
            };
            spread(&slates.state(params, origin.as_ref()).await?)
        }
        RuntimeOp::SlatesRead { thread_id, turn_token, values, text, sketch, document, .. } => {
            let params = ReadParams {
                target: target(thread_id, turn_token),
                values: values.clone(),
                text: *text,
                sketch: *sketch,
                document: *document,
            };
            read_payload(&slates.read(params, origin.as_ref()).await?)
        }
        RuntimeOp::SlatesCatalog { name, thread_id, turn_token, .. } => {
            let params = CatalogParams { target: target(thread_id, turn_token), name: name.clone() };
            one("text", Value::from(slates.catalog(params, origin.as_ref()).await?))
        }
        RuntimeOp::SlatesShown { thread_id, .. } => {
            slates.shown(thread_id).await?;
            Ok(Payload::new())
        }
        RuntimeOp::SlatesEvent { thread_id, version, piece, event, request_id, scope, row_action, .. } => {
            let params = EventParams {
                thread_id: thread_id.clone(),
                version: version.get(),
                piece: piece.clone(),
                event: event_word(*event).into(),
                request_id: request_id.clone(),
                scope: scope.as_ref().map(|scope| EventScope {
                    item: scope.item.as_ref().map(|item| item.as_ref().map_or(Value::Null, |kept| kept.0.clone())),
                    index: scope.index.get(),
                }),
                row_action: row_action.map(JsInt::get),
            };
            event_payload(&slates.event(params).await?)
        }
        RuntimeOp::SlatesApprove { thread_id, key, scope, .. } => {
            let params = ApproveParams { thread_id: thread_id.clone(), key: key.clone(), scope: approval_scope(*scope) };
            slates.approve(params).await?;
            Ok(Payload::new())
        }
        RuntimeOp::SlatesCancel { thread_id, run, .. } => {
            slates.cancel(thread_id, run).await?;
            Ok(Payload::new())
        }
        RuntimeOp::SlatesRevoke { thread_id, key, .. } => {
            slates.revoke(thread_id, key).await?;
            Ok(Payload::new())
        }
        RuntimeOp::SlatesSubscribe { thread_id, sources, .. } => {
            let release = slates.subscribe(thread_id, sources);
            let live = Rc::new(Cell::new(true));
            let once: Rc<dyn Fn()> = Rc::new(move || {
                if live.replace(false) {
                    release()
                }
            });
            holds.borrow_mut().held.entry(hold_key(thread_id, sources)).or_default().push(once.clone());
            detach(Box::new(move || once()));
            Ok(Payload::new())
        }
        RuntimeOp::SlatesUnsubscribe { thread_id, sources, .. } => {
            unhold(holds, &hold_key(thread_id, sources));
            Ok(Payload::new())
        }
        RuntimeOp::SlatesResolve { thread_id, paths, .. } => spread(&slates.resolve(thread_id, paths).await?),
        RuntimeOp::SlatesImage { thread_id, src, have, .. } => spread(&slates.image(thread_id, src, have.as_deref()).await?),
        _ => Err(HostError::new(format!("{} is not answered here", msg.op.name()))),
    }
}

/// `slates.unsubscribe` lets go of the oldest hold under its key, and forgets the key when none is left.
fn unhold(holds: &Rc<RefCell<SlateHolds>>, key: &str) {
    let oldest = {
        let mut holds = holds.borrow_mut();
        let oldest = holds.held.get_mut(key).filter(|held| !held.is_empty()).map(|held| held.remove(0));
        if holds.held.get(key).is_some_and(Vec::is_empty) {
            holds.held.shift_remove(key);
        }
        oldest
    };
    if let Some(release) = oldest {
        release();
    }
}

fn target(thread_id: &Option<String>, turn_token: &Option<String>) -> SlateTarget {
    SlateTarget { thread_id: thread_id.clone(), turn_token: turn_token.clone() }
}

fn values_of(values: &SlateWireValues) -> IndexMap<String, Value> {
    values.0.iter().map(|(name, value)| (name.clone(), value.0.clone())).collect()
}

fn event_word(event: SlatesEventEvent) -> &'static str {
    match event {
        SlatesEventEvent::Press => "press",
        SlatesEventEvent::Submit => "submit",
        SlatesEventEvent::Change => "change",
    }
}

fn approval_scope(scope: SlatesApproveScope) -> ApprovalScope {
    match scope {
        SlatesApproveScope::Once => ApprovalScope::Once,
        SlatesApproveScope::Thread => ApprovalScope::Thread,
        SlatesApproveScope::Refuse => ApprovalScope::Refuse,
    }
}

/// `SlateReadAnswer` as the wire carries it (wire.ts:220-236): `document` only where the read asked for it.
fn read_payload(read: &SlateReadAnswer) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    out.insert("version".into(), Value::from(read.version));
    out.insert("text".into(), Value::from(read.text.clone()));
    if let Some(document) = &read.document {
        out.insert("document".into(), document.clone().unwrap_or(Value::Null));
    }
    out.insert("values".into(), to_value(&read.values)?);
    out.insert("state".into(), to_value(&read.state)?);
    out.insert("derived".into(), to_value(&read.derived)?);
    out.insert("runs".into(), to_value(&read.runs)?);
    out.insert("problems".into(), to_value(&read.problems)?);
    out.insert("waiting".into(), to_value(&read.waiting)?);
    out.insert("comments".into(), to_value(&read.comments)?);
    out.insert("approvals".into(), to_value(&read.approvals)?);
    Ok(out)
}

/// `SlateEventAnswer` as the wire carries it (wire.ts:246-253): `turnId` and `ask` only where given.
fn event_payload(event: &SlateEventAnswer) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    out.insert("outcome".into(), to_value(&event.outcome)?);
    out.insert("said".into(), Value::from(event.said.clone()));
    if let Some(turn_id) = &event.turn_id {
        out.insert("turnId".into(), Value::from(turn_id.clone()));
    }
    if let Some(ask) = &event.ask {
        out.insert("ask".into(), ask.to_json());
    }
    Ok(out)
}

fn one(key: &str, value: Value) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    out.insert(key.into(), value);
    Ok(out)
}

fn to_value<T: Serialize>(answer: &T) -> Result<Value, HostError> {
    serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))
}

/// The answer's fields beside id and ok, as the TS spreads them.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match to_value(answer)? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}
