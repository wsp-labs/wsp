// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/context.ts` at 65f6b89c7: what every area of the place door reads. Spec:
//! `spec/places_readings.md`.

use std::cell::{OnceCell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use wsp_core::clock::RealClock;
use wsp_core::types::pending::Clock;
use wsp_core::types::pending::DeviceDoor;
use wsp_core::types::pending::Store;
use wsp_engine::machine::MachineBackend;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::views::place::PlaceProvisionRow;
use wsp_words::exit::HostError;

use super::helpers::{
    sign_ins_of, Forward, Live, DIAL_MS, LINK_FRAME_MS, RELINK_WAIT_MS, SEEN_EVERY_MS,
};
use super::types::{
    PlaceDoor, PlaceDoorOptions, PlaceEvent, PlaceRecord, PlaceRecording, PlaceWiring,
};

/// One read of a place's backend facts in flight, shared by every road asking at once.
pub type AskingBackend =
    Shared<LocalBoxFuture<'static, Result<Arc<dyn MachineBackend>, HostError>>>;

/// A watcher of the door's events.
pub type PlaceWatcher = Rc<dyn Fn(&PlaceEvent)>;

/// What every area of the place door reads (context.ts:13). Every map is memory only and empty at birth.
pub struct PlaceDoorContext {
    pub opts: PlaceDoorOptions,
    pub store: Rc<dyn Store>,
    pub devices: Rc<dyn DeviceDoor>,
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
    let clock: Rc<dyn Clock> = opts.clock.clone().unwrap_or_else(|| Rc::new(RealClock));
    Rc::new(PlaceDoorContext {
        store: Rc::clone(&opts.store),
        devices: Rc::clone(&opts.devices),
        wiring: Rc::new(opts.wiring.clone()),
        recording: Rc::clone(&opts.recording),
        clock,
        seen_every_ms: opts.seen_every_ms.unwrap_or(SEEN_EVERY_MS),
        dial_wait_ms: opts.dial_wait_ms.unwrap_or(DIAL_MS),
        frame_wait_ms: opts.frame_wait_ms.unwrap_or(LINK_FRAME_MS),
        relink_wait_ms: opts.relink_wait_ms.unwrap_or(RELINK_WAIT_MS),
        live: RefCell::new(IndexMap::new()),
        kept: RefCell::new(IndexMap::new()),
        backends: RefCell::new(IndexMap::new()),
        forwards: Rc::new(RefCell::new(IndexMap::new())),
        asking: RefCell::new(IndexMap::new()),
        watchers: RefCell::new(Vec::new()),
        door: OnceCell::new(),
        opts,
    })
}

impl PlaceDoorContext {
    /// What stands for each agent on that computer as its last report and this host's vault say, read at the call;
    /// `None` for a place not kept (context.ts:57).
    pub fn sign_ins_here(
        &self,
        place_id: &str,
        agents: Option<&[String]>,
    ) -> Option<IndexMap<String, AgentSignInState>> {
        let record = self.kept.borrow().get(place_id).cloned()?;
        let vault = self
            .opts
            .vault
            .as_ref()
            .map(|vault| vault())
            .unwrap_or_default();
        let rows: Vec<PlaceProvisionRow> = record
            .applied
            .iter()
            .flat_map(|applied| applied.rows.iter().map(|r| r.row.clone()))
            .collect();
        sign_ins_of(
            &record.report.agents,
            record.report.logins.as_deref(),
            &vault,
            &rows,
            agents,
        )
    }

    /// Every watcher, in registration order, iterated live (context.ts:68, spec 03 rule 3.7).
    pub fn emit(&self, e: &PlaceEvent) {
        let mut at = 0;
        loop {
            let watcher = match self.watchers.borrow().get(at) {
                None => break,
                Some(slot) => slot.clone(),
            };
            if let Some(watcher) = watcher {
                watcher(e);
            }
            at += 1;
        }
    }

    /// Adds a watcher; the answer removes it.
    pub fn watch(self: &Rc<Self>, f: PlaceWatcher) -> Box<dyn FnOnce()> {
        let at = {
            let mut watchers = self.watchers.borrow_mut();
            watchers.push(Some(f));
            watchers.len() - 1
        };
        let ctx = Rc::downgrade(self);
        Box::new(move || {
            if let Some(ctx) = ctx.upgrade() {
                let removed = ctx.watchers.borrow_mut().get_mut(at).and_then(Option::take);
                drop(removed);
            }
        })
    }

    /// The door, once every area is built. Before [`PlaceDoorContext::set_door`] it panics with "the place door is read
    /// before it is built".
    pub fn door(&self) -> Rc<dyn PlaceDoor> {
        let door = self
            .door
            .get()
            .expect("the place door is read before it is built");
        door.upgrade()
            .expect("the place door is dropped while its areas are still read")
    }

    /// Set once, by the door's assembly; a second call panics.
    pub fn set_door(&self, door: Weak<dyn PlaceDoor>) {
        assert!(self.door.set(door).is_ok(), "the place door is set once");
    }
}
