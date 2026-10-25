// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-slates.ts at 65f6b89c7: the slate ops a socket asks, answered off the
//! runtime's slates, as what the reply carries beside id and ok. Spec: spec/serve_helpers.md, H22-H23.
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_slates::slates::Slates;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

/// `msg.op.startsWith("slates.")`.
pub fn is_slate_request(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// The holds one socket took, by thread and sources, oldest first under each key. serve-slates.ts:10-15. Every hold
/// also goes on the socket's detaches, which its close runs.
#[derive(Default)]
pub struct SlateHolds {
    pub held: IndexMap<String, Vec<Rc<dyn Fn()>>>,
}

/// The key a hold is kept under: `<threadId>\u0000<sources sorted, joined with ",">`. serve-slates.ts:57, 63.
pub fn hold_key(thread_id: &str, sources: &[String]) -> String {
    todo!()
}

/// `answerSlate(slates, msg, origin, holds)`. serve-slates.ts:17-74. `detach` pushes a leave onto the socket's
/// detaches.
pub async fn answer_slate(
    slates: &Rc<dyn Slates>,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
    holds: &Rc<std::cell::RefCell<SlateHolds>>,
    detach: &dyn Fn(Box<dyn FnOnce()>),
) -> Result<Payload, HostError> {
    todo!()
}
