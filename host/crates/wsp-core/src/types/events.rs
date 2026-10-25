// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/types/events.ts: the host's event bus and the timing constants the areas share.
//! Spec: spec/events.md. One thread owns the bus (chapter 03, rule 3.1), so it is `Rc` and `RefCell`, never `Send`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

/// A listener runs inline on the emitter's call. An `Err` stops the later listeners and reaches the caller of `emit`,
/// as a throw in a TS listener does (chapter 03, rule 3.6). events.ts:7.
pub type EventListener = Rc<dyn Fn(&EventUnion) -> Result<(), HostError>>;

/// What `on` answers: deletes that one listener. Calling it twice is harmless, as `Set.delete` is.
pub type Unlisten = Box<dyn Fn()>;

/// The listener key that hears every event. events.ts:10.
pub const ANY_EVENT: &str = "*";

/// `since`'s answer. events.ts:15.
#[derive(Clone, Debug, PartialEq)]
pub struct Since {
    /// This bus's own stream id, whatever the caller sent.
    pub stream: String,
    /// The newest `seq` issued, 0 before any.
    pub head: u64,
    /// The ring's events after `after`, oldest first, with their ring `seq`. Empty on a gap.
    pub events: Vec<EventUnion>,
    pub gap: bool,
}

/// events.ts:9. The read side every area holds.
pub trait EventBus {
    /// Adds a listener for one event type, or for every event with `ANY_EVENT`. events.ts:65-73.
    fn on(&self, event_type: &str, listener: EventListener) -> Unlisten;
    /// The retained events after `after`, with the head and the stream; the gap table is spec/events.md's. events.ts:74-80.
    fn since(&self, after: Option<u64>, stream: Option<&str>) -> Since;
}

/// Events kept for a socket that comes back: one ring shared by every workspace. events.ts:21.
pub const EVENT_RING_CAP: usize = 5000;
/// How often a pull request with a pending check is read while a slate watches its checks. events.ts:23.
pub const SLATE_PR_POLL_MS: u64 = 30_000;
/// How long a machine's catalog answer stands before the binary is asked again. events.ts:26.
pub const CATALOG_TTL_MS: u64 = 5 * 60_000;
/// events.ts:29.
pub const CATALOG_PROBE_TIMEOUT_MS: u64 = 25_000;
/// events.ts:31.
pub const FIRST_RUN_READ_MS: u64 = 3_000;
/// events.ts:34.
pub const AGENT_VERSION_READ_MS: u64 = 3_000;
/// events.ts:38.
pub const SESSION_TITLE_TTL_MS: u64 = 10_000;
/// events.ts:42.
pub const CHECKOUT_TTL_MS: u64 = 10_000;
/// events.ts:45.
pub const PR_PAGE_HOLD_MS: u64 = 60_000;
/// events.ts:48.
pub const RATE_LIMIT_HOLD_MS: u64 = 60_000;
/// events.ts:50.
pub const SESSION_TITLE_TIMEOUT_MS: u64 = 15_000;
/// events.ts:53.
pub const SESSION_TITLE_REFRESH_MAX: usize = 20;
/// events.ts:57.
pub const TITLE_MAKE_TIMEOUT_MS: u64 = 25_000;

/// The bus's whole state and no more (chapter 03, rule 3.20). A removed listener leaves `None` in its slot, so a
/// delivery that iterates by index skips it and reaches one added mid-delivery (rule 3.7); an empty list is never
/// removed from the map.
pub struct BusState {
    pub listeners: IndexMap<String, Vec<Option<EventListener>>>,
    pub ring: VecDeque<EventUnion>,
    pub stream: String,
    pub head: u64,
}

/// The bus `eventBus()` makes: the read side plus `emit` and `pass`. events.ts:59-98.
pub struct Bus {
    pub state: RefCell<BusState>,
}

/// A fresh bus with `head` 0 and an empty ring. `stream` is the id minted once for it, through the ids seam
/// (chapter 03, rule 3.16), where the TS calls `randomUUID()` (events.ts:62).
pub fn event_bus(stream: String) -> Rc<Bus> {
    todo!()
}

impl Bus {
    /// Stamps `seq = ++head`, pushes the stamped event on the ring, trims the ring to `EVENT_RING_CAP` from its oldest
    /// end, then calls the event type's listeners in registration order and then the `ANY_EVENT` ones, inline, with no
    /// borrow of the state held across a call (rule 3.3). The first listener `Err` stops delivery and is returned; the
    /// event stays in the ring. Answers the stamped `seq`. events.ts:81-88.
    pub fn emit(&self, event: EventUnion) -> Result<u64, HostError> {
        todo!()
    }

    /// Delivers to every listener as `emit` does and puts nothing in the ring: `seq` is the current head where
    /// `head > 0`, and the event goes as given, its own `seq` untouched, before any emit. events.ts:91-96.
    pub fn pass(&self, event: EventUnion) -> Result<(), HostError> {
        todo!()
    }
}

impl EventBus for Bus {
    fn on(&self, event_type: &str, listener: EventListener) -> Unlisten {
        todo!()
    }

    fn since(&self, after: Option<u64>, stream: Option<&str>) -> Since {
        todo!()
    }
}

/// The `type` key an event carries, the key `on` files its listeners under.
pub fn event_type(event: &EventUnion) -> &'static str {
    todo!()
}

/// The `seq` an event carries, where it carries one.
pub fn event_seq(event: &EventUnion) -> Option<u64> {
    todo!()
}

/// The event with `seq` set, every other key as it was.
pub fn with_seq(event: EventUnion, seq: u64) -> EventUnion {
    todo!()
}
