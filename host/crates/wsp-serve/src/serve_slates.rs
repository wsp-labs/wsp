// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-slates.ts at 65f6b89c7: the slate ops a socket asks, answered off the
//! runtime's slates, as what the reply carries beside id and ok. Spec: spec/serve_helpers.md, H22-H23.
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

/// `msg.op.startsWith("slates.")`.
pub fn is_slate_request(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// The calls `answerSlate` makes on the runtime's slates (slates.ts's `Slates`), until wsp-slates lands and the join
/// reads its own type. Params are the request less `id`, `origin` and `op`, as JSON objects in the frame's order;
/// answers are the objects the reply spreads.
#[async_trait(?Send)]
pub trait SlateDoor {
    async fn get(&self, thread_id: &str) -> Result<serde_json::Value, HostError>;
    async fn write(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError>;
    async fn state(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError>;
    async fn read(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError>;
    async fn catalog(&self, params: Payload, origin: Option<Caller>) -> Result<Payload, HostError>;
    async fn shown(&self, thread_id: &str) -> Result<(), HostError>;
    async fn event(&self, params: Payload) -> Result<Payload, HostError>;
    async fn approve(&self, thread_id: &str, key: &str, scope: &str) -> Result<(), HostError>;
    async fn cancel(&self, thread_id: &str, run: &str) -> Result<(), HostError>;
    async fn revoke(&self, thread_id: &str, key: &str) -> Result<(), HostError>;
    /// A hold on the thread's sources; the answer lets go of it.
    fn subscribe(&self, thread_id: &str, sources: Vec<String>) -> Box<dyn FnOnce()>;
    async fn resolve(&self, thread_id: &str, paths: Vec<String>) -> Result<Payload, HostError>;
    async fn image(&self, thread_id: &str, src: &str, have: Option<String>) -> Result<Payload, HostError>;
}

/// The holds one socket took, by thread and sources, oldest first under each key. serve-slates.ts:10-15. Every hold
/// also goes on the socket's detaches, which its close runs.
#[derive(Default)]
pub struct SlateHolds {
    pub held: IndexMap<String, Vec<Rc<std::cell::Cell<Option<Box<dyn FnOnce()>>>>>>,
}

/// The key a hold is kept under: `<threadId>\u0000<sources sorted, joined with ",">`. serve-slates.ts:57, 63.
pub fn hold_key(thread_id: &str, sources: &[String]) -> String {
    todo!()
}

/// `answerSlate(slates, msg, origin, holds)`. serve-slates.ts:17-74. `detach` pushes a leave onto the socket's
/// detaches.
pub async fn answer_slate(
    slates: &Rc<dyn SlateDoor>,
    msg: &RuntimeRequest,
    origin: Option<Caller>,
    holds: &Rc<std::cell::RefCell<SlateHolds>>,
    detach: &dyn Fn(Box<dyn FnOnce()>),
) -> Result<Payload, HostError> {
    todo!()
}
