// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/machines/thread-ends.ts` at 65f6b89c7: the end of a thread's cgroup on a computer the person
//! joined, and the debt a computer that did not answer owes. Spec: spec/thread_ends.md.

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use wsp_engine::machine::ExecResult;
use wsp_words::exit::HostError;

/// The store collection of the threads each computer is owed an end of, by place id (thread-ends.ts:14).
pub const OWED: &str = "thread-ends";

/// One computer's debt (thread-ends.ts:16).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owed {
    pub threads: Vec<String>,
}

/// What the area reads off the rest of the runtime (thread-ends.ts:33-51).
#[async_trait(?Send)]
pub trait ThreadEndsRuntime {
    async fn store_get(&self, collection: &str, id: &str) -> Result<Option<serde_json::Value>, HostError>;
    async fn store_put(&self, collection: &str, id: &str, value: serde_json::Value) -> Result<(), HostError>;
    async fn store_delete(&self, collection: &str, id: &str) -> Result<(), HostError>;
    async fn ready(&self) -> Result<(), HostError>;
    /// `ctx.runningOn(thread) !== undefined`.
    fn running_on(&self, thread_id: &str) -> bool;
    /// `ctx.launchingOn(thread) !== undefined`.
    fn launching_on(&self, thread_id: &str) -> bool;
    fn stop_holds(&self, thread_id: &str) -> bool;
    fn warn(&self, line: &str);
}

/// The place door as this area reads it (thread-ends.ts:46, 101, 111).
#[async_trait(?Send)]
pub trait ThreadEndsDoor {
    async fn exec(&self, place_id: &str, script: &str, timeout_ms: u64) -> Result<ExecResult, HostError>;
    /// `door.link(place) !== undefined`.
    fn link(&self, place_id: &str) -> bool;
    fn name_of(&self, place_id: &str) -> String;
}

/// The door's events this area hears (thread-ends.ts:87).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceEvent {
    Present { place_id: String },
    Removed { place_id: String },
    Other,
}

/// What a stop or a delete asks of `end` (thread-ends.ts:24): wsp-core's `EndThread`, the same two flags.
pub use wsp_core::types::internal::EndThread as EndAsk;

/// A debt still owed: `paid` resolves once that computer ran the end or was removed (thread-ends.ts:27).
pub struct OwedEnd {
    pub paid: futures::future::LocalBoxFuture<'static, ()>,
}

/// `threadEnds(ctx, door)` (thread-ends.ts:33).
pub struct ThreadEnds {
    pub(crate) rt: Rc<dyn ThreadEndsRuntime>,
    pub(crate) door: Rc<dyn ThreadEndsDoor>,
    /// `waiting`: what waits on a thread's owed end, by `<place>\n<thread>`.
    pub(crate) waiting: RefCell<IndexMap<String, Vec<futures::channel::oneshot::Sender<()>>>>,
}

/// `survivorsOf` (thread-ends.ts:31): the pids the end script names as still standing.
pub fn survivors_of(stderr: &str) -> Vec<i64> {
    todo!()
}

impl ThreadEnds {
    pub fn new(rt: Rc<dyn ThreadEndsRuntime>, door: Rc<dyn ThreadEndsDoor>) -> Rc<Self> {
        todo!()
    }

    /// The door's listener (thread-ends.ts:87): the wiring calls it for every door event. A payment it starts runs
    /// detached.
    pub fn on_place_event(self: &Rc<Self>, event: &PlaceEvent) {
        todo!()
    }

    /// `end` (thread-ends.ts:97), spec rule 8: what a stop could not end there, in the person's words.
    pub async fn end(self: &Rc<Self>, place_id: &str, thread_id: &str, ask: EndAsk) -> Result<Option<String>, HostError> {
        todo!()
    }

    /// `owed` (thread-ends.ts:125), spec rule 9.
    pub async fn owed(self: &Rc<Self>, place_id: &str, thread_id: &str) -> Result<Option<OwedEnd>, HostError> {
        todo!()
    }
}
