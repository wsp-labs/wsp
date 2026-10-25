// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/context.ts` at 65f6b89c7: what every area of the place door reads. Spec:
//! `spec/places_readings.md`.

use std::cell::{OnceCell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_core::devices::DeviceDoor;
use wsp_store::store::Store;
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_words::exit::HostError;

use super::helpers::{Forward, Live};
use super::types::{PlaceDoor, PlaceDoorOptions, PlaceEvent, PlaceRecord, PlaceRecording, PlaceWiring};

/// One read of a place's backend facts in flight, shared by every road asking at once.
pub type AskingBackend = Shared<LocalBoxFuture<'static, Result<Arc<dyn MachineBackend>, HostError>>>;

/// A watcher of the door's events.
pub type PlaceWatcher = Rc<dyn Fn(&PlaceEvent)>;

/// What every area of the place door reads (context.ts:13). Every map is memory only and empty at birth.
pub struct PlaceDoorContext {
    pub opts: PlaceDoorOptions,
    pub store: Rc<dyn Store>,
    pub devices: Rc<DeviceDoor>,
    pub wiring: Rc<PlaceWiring>,
    pub recording: Rc<dyn PlaceRecording>,
    /// The one clock: `opts.clock`, else the real one. The TS `now` and `schedule` options are this.
    pub clock: Rc<dyn Clock>,
    pub seen_every_ms: u64,
    pub dial_wait_ms: u64,
    pub frame_wait_ms: u64,
    pub relink_wait_ms: u64,
    pub live: RefCell<IndexMap<String, Live>>,
    /// The records as they stand, by id: `load` fills it and every write keeps it.
    pub kept: RefCell<IndexMap<String, PlaceRecord>>,
    /// The backend each place offers, built once from what it said about it.
    pub backends: RefCell<IndexMap<String, Arc<dyn MachineBackend>>>,
    pub forwards: Rc<RefCell<IndexMap<String, Forward>>>,
    /// The read of one place's backend facts in flight.
    pub asking: RefCell<IndexMap<String, AskingBackend>>,
    /// The door's watchers in registration order; a removed one is a tombstone so an emit in flight skips it.
    pub watchers: RefCell<Vec<Option<PlaceWatcher>>>,
    door: OnceCell<Weak<dyn PlaceDoor>>,
}

/// The context every area is built over (context.ts:37), with the TS defaults for the four waits.
pub fn place_door_context(opts: PlaceDoorOptions) -> Rc<PlaceDoorContext> {
    let _ = opts;
    todo!()
}

impl PlaceDoorContext {
    /// What stands for each agent on that computer as its last report and this host's vault say, read at the call;
    /// `None` for a place not kept (context.ts:57).
    pub fn sign_ins_here(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, AgentSignInState>> {
        let _ = (place_id, agents);
        todo!()
    }

    /// Every watcher, in registration order, iterated live (context.ts:68, spec 03 rule 3.7).
    pub fn emit(&self, e: &PlaceEvent) {
        let _ = e;
        todo!()
    }

    /// Adds a watcher; the answer removes it.
    pub fn watch(self: &Rc<Self>, f: PlaceWatcher) -> Box<dyn FnOnce()> {
        let _ = f;
        todo!()
    }

    /// The door, once every area is built. Panics before [`PlaceDoorContext::set_door`].
    pub fn door(&self) -> Rc<dyn PlaceDoor> {
        todo!()
    }

    /// Set once, by the door's assembly; a second call panics.
    pub fn set_door(&self, door: Weak<dyn PlaceDoor>) {
        let _ = (door, &self.door);
        todo!()
    }
}
