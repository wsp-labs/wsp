// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/readings.ts` at 65f6b89c7: a computer's live readings over the one link it holds to
//! this host, one watch per link however many readers. Spec: `spec/places_readings.md`.

use std::cell::RefCell;
use std::rc::Rc;

use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::SysSample;
use wsp_words::exit::HostError;

use super::context::PlaceDoorContext;

/// One reader of a computer's samples.
pub type Reader = Rc<dyn Fn(&SysSample)>;

/// Lets go of one reader's hold; a second call does nothing.
pub type Release = Rc<dyn Fn()>;

/// The readings of every linked computer (readings.ts:97). State is memory only.
pub struct PlaceReadings {
    ctx: Rc<PlaceDoorContext>,
    state: RefCell<ReadingsState>,
}

#[derive(Default)]
struct ReadingsState {
    _private: (),
}

impl PlaceReadings {
    pub fn new(ctx: Rc<PlaceDoorContext>) -> Rc<PlaceReadings> {
        let _ = ctx;
        todo!()
    }

    /// Hands `f` every sample the computer pushes until the release runs; held while the computer is away. Fails with
    /// the daemon's refusal of the watch, or for a place this host holds no computer for (readings.ts:133).
    pub async fn watch(self: &Rc<Self>, place_id: &str, f: Reader) -> Result<Release, HostError> {
        let _ = (place_id, f, &self.ctx, &self.state);
        todo!()
    }

    /// A link that just attached: asked for the readings where somebody is still reading them (readings.ts:157).
    pub fn linked(self: &Rc<Self>, place_id: &str) {
        let _ = place_id;
        todo!()
    }

    /// A sample to each reader of a snapshot taken before the first call; any other event is dropped
    /// (readings.ts:164).
    pub fn heard(&self, place_id: &str, event: &DaemonEvent) {
        let _ = (place_id, event);
        todo!()
    }
}
