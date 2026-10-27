// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/types/events.ts: the host's event bus and the timing constants the areas share.
//! Spec: spec/events.md. One thread owns the bus (chapter 03, rule 3.1), so it is `Rc` and `RefCell`, never `Send`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use indexmap::IndexMap;
use wsp_wire::types::js_number::JsInt;
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

/// The bus's whole state and no more (chapter 03, rule 3.20). A listener removed while a delivery is walking the
/// lists leaves `None` in its slot, so the walk by index skips it and reaches one added mid-delivery (rule 3.7); the
/// slots are swept out when the outermost delivery ends, and a listener removed between deliveries leaves none.
pub struct BusState {
    pub listeners: IndexMap<String, Vec<Option<EventListener>>>,
    pub ring: VecDeque<EventUnion>,
    pub stream: String,
    pub head: u64,
    /// How many deliveries are walking the lists now.
    pub delivering: usize,
}

/// The bus `eventBus()` makes: the read side plus `emit` and `pass`. events.ts:59-98.
pub struct Bus {
    pub state: RefCell<BusState>,
    /// The bus's own handle, so `on`, which takes `&self`, can hand an unlisten a way back to the state.
    me: Weak<Bus>,
}

/// A fresh bus with `head` 0 and an empty ring. `stream` is the id minted once for it, through the ids seam
/// (chapter 03, rule 3.16), where the TS calls `randomUUID()` (events.ts:62).
pub fn event_bus(stream: String) -> Rc<Bus> {
    Rc::new_cyclic(|me| Bus {
        state: RefCell::new(BusState { listeners: IndexMap::new(), ring: VecDeque::new(), stream, head: 0, delivering: 0 }),
        me: me.clone(),
    })
}

impl Bus {
    /// Drops every listener: the close's last act, since a listener that holds the runtime keeps it alive after the
    /// close, the bus being a part of the runtime it holds. The ring stays readable.
    pub fn clear(&self) {
        let listeners = std::mem::take(&mut self.state.borrow_mut().listeners);
        drop(listeners);
    }

    /// Stamps `seq = ++head`, pushes the stamped event on the ring, trims the ring to `EVENT_RING_CAP` from its oldest
    /// end, then calls the event type's listeners in registration order and then the `ANY_EVENT` ones, inline, with no
    /// borrow of the state held across a call (rule 3.3). The first listener `Err` stops delivery and is returned; the
    /// event stays in the ring. Answers the stamped `seq`. events.ts:81-88.
    pub fn emit(&self, event: EventUnion) -> Result<u64, HostError> {
        let (stamped, seq) = {
            let mut st = self.state.borrow_mut();
            st.head += 1;
            let stamped = with_seq(event, st.head);
            st.ring.push_back(stamped.clone());
            while st.ring.len() > EVENT_RING_CAP {
                st.ring.pop_front();
            }
            (stamped, st.head)
        };
        self.deliver(event_type(&stamped), &stamped)?;
        Ok(seq)
    }

    /// Calls the `ty` listeners then the `ANY_EVENT` ones, by index, with no borrow held (rule 3.7).
    fn deliver(&self, ty: &str, event: &EventUnion) -> Result<(), HostError> {
        let _walk = Walk::enter(&self.state);
        for key in [ty, ANY_EVENT] {
            let mut i = 0;
            loop {
                let next = {
                    let st = self.state.borrow();
                    match st.listeners.get(key) {
                        Some(list) if i < list.len() => list[i].clone(),
                        _ => break,
                    }
                };
                i += 1;
                if let Some(listener) = next {
                    listener(event)?;
                }
            }
        }
        Ok(())
    }

    /// Delivers to every listener as `emit` does and puts nothing in the ring: `seq` is the current head where
    /// `head > 0`, and the event goes as given, its own `seq` untouched, before any emit. events.ts:91-96.
    pub fn pass(&self, event: EventUnion) -> Result<(), HostError> {
        let head = self.state.borrow().head;
        let stamped = if head > 0 { with_seq(event, head) } else { event };
        self.deliver(event_type(&stamped), &stamped)
    }
}

/// One delivery's hold on the lists: the slots of listeners removed meanwhile are swept when the last one lets go.
struct Walk<'a>(&'a RefCell<BusState>);

impl<'a> Walk<'a> {
    fn enter(state: &'a RefCell<BusState>) -> Self {
        state.borrow_mut().delivering += 1;
        Walk(state)
    }
}

impl Drop for Walk<'_> {
    fn drop(&mut self) {
        let mut st = self.0.borrow_mut();
        st.delivering -= 1;
        if st.delivering == 0 {
            for list in st.listeners.values_mut() {
                list.retain(Option::is_some);
            }
        }
    }
}

impl EventBus for Bus {
    fn on(&self, event_type: &str, listener: EventListener) -> Unlisten {
        {
            let mut st = self.state.borrow_mut();
            let list = st.listeners.entry(event_type.to_owned()).or_default();
            if !list.iter().flatten().any(|l| Rc::ptr_eq(l, &listener)) {
                list.push(Some(listener.clone()));
            }
        }
        let bus = self.me.clone();
        let key = event_type.to_owned();
        Box::new(move || {
            let Some(bus) = bus.upgrade() else { return };
            let removed = {
                let mut st = bus.state.borrow_mut();
                let walking = st.delivering > 0;
                let Some(list) = st.listeners.get_mut(&key) else { return };
                let Some(at) = list.iter().position(|slot| slot.as_ref().is_some_and(|l| Rc::ptr_eq(l, &listener))) else { return };
                if walking {
                    list[at].take()
                } else {
                    list.remove(at)
                }
            };
            drop(removed);
        })
    }

    fn since(&self, after: Option<u64>, stream: Option<&str>) -> Since {
        let st = self.state.borrow();
        let head = st.head;
        let own = st.stream.clone();
        let Some(after) = after else {
            return Since { stream: own, head, events: Vec::new(), gap: false };
        };
        let floor = head - st.ring.len() as u64;
        let foreign = stream.is_some_and(|s| s != st.stream.as_str());
        if foreign || after > head || after < floor {
            return Since { stream: own, head, events: Vec::new(), gap: true };
        }
        let events = st.ring.iter().skip((after - floor) as usize).cloned().collect();
        Since { stream: own, head, events, gap: false }
    }
}

/// The `type` key an event carries, the key `on` files its listeners under.
pub fn event_type(event: &EventUnion) -> &'static str {
    match event {
        EventUnion::WorkspaceCreating(_) => "workspace.creating",
        EventUnion::WorkspaceCreated(_) => "workspace.created",
        EventUnion::WorkspaceNapped(_) => "workspace.napped",
        EventUnion::WorkspaceWoken(_) => "workspace.woken",
        EventUnion::WorkspaceUpgraded(_) => "workspace.upgraded",
        EventUnion::WorkspaceRenamed(_) => "workspace.renamed",
        EventUnion::WorkspaceLook(_) => "workspace.look",
        EventUnion::WorkspaceDeleted(_) => "workspace.deleted",
        EventUnion::WorkspaceGone(_) => "workspace.gone",
        EventUnion::WorkspaceReleased(_) => "workspace.released",
        EventUnion::WorkspaceStatus(_) => "workspace.status",
        EventUnion::WorkspaceReview(_) => "workspace.review",
        EventUnion::WorkspaceViewed(_) => "workspace.viewed",
        EventUnion::WorkspaceCost(_) => "workspace.cost",
        EventUnion::SessionStart(_) => "session.start",
        EventUnion::SessionDelta(_) => "session.delta",
        EventUnion::SessionDone(_) => "session.done",
        EventUnion::SessionEnd(_) => "session.end",
        EventUnion::SessionSteer(_) => "session.steer",
        EventUnion::SessionNotify(_) => "session.notify",
        EventUnion::SessionPermission(_) => "session.permission",
        EventUnion::SessionPermissionClosed(_) => "session.permission.closed",
        EventUnion::SessionCheckpoint(_) => "session.checkpoint",
        EventUnion::SessionChanges(_) => "session.changes",
        EventUnion::SessionPlan(_) => "session.plan",
        EventUnion::SessionCompacted(_) => "session.compacted",
        EventUnion::SessionRetry(_) => "session.retry",
        EventUnion::SessionContext(_) => "session.context",
        EventUnion::SessionRun(_) => "session.run",
        EventUnion::SessionMoved(_) => "session.moved",
        EventUnion::SessionBehind(_) => "session.behind",
        EventUnion::SessionNotice(_) => "session.notice",
        EventUnion::SessionEarlier(_) => "session.earlier",
        EventUnion::SessionCapped(_) => "session.capped",
        EventUnion::SessionUncapped(_) => "session.uncapped",
        EventUnion::SessionSubagent(_) => "session.subagent",
        EventUnion::SessionSlate(_) => "session.slate",
        EventUnion::ThreadWaited(_) => "thread.waited",
        EventUnion::SlateValues(_) => "slate.values",
        EventUnion::SlateRun(_) => "slate.run",
        EventUnion::UsageAccount(_) => "usage.account",
        EventUnion::SessionQueued(_) => "session.queued",
        EventUnion::SessionHeld(_) => "session.held",
        EventUnion::SessionRow(_) => "session.row",
        EventUnion::SessionStarting(_) => "session.starting",
        EventUnion::ThreadMarked(_) => "thread.marked",
        EventUnion::ThreadHead(_) => "thread.head",
        EventUnion::ThreadRewound(_) => "thread.rewound",
        EventUnion::PortOpen(_) => "port.open",
        EventUnion::PortClose(_) => "port.close",
        EventUnion::InboxFile(_) => "inbox.file",
        EventUnion::GoldenStage(_) => "golden.stage",
        EventUnion::ForwardOpen(_) => "forward.open",
        EventUnion::ForwardClose(_) => "forward.close",
        EventUnion::ProjectAdded(_) => "project.added",
        EventUnion::ProjectAdd(_) => "project.add",
        EventUnion::ProjectRemoved(_) => "project.removed",
        EventUnion::ProjectImport(_) => "project.import",
        EventUnion::ProjectExport(_) => "project.export",
        EventUnion::PreferencesChanged(_) => "preferences.changed",
        EventUnion::ReleaseChanged(_) => "release.changed",
        EventUnion::PlaceStage(_) => "place.stage",
        EventUnion::PlaceSetup(_) => "place.setup",
        EventUnion::PlaceSync(_) => "place.sync",
        EventUnion::RecipesChanged(_) => "recipes.changed",
        EventUnion::PlacePending(_) => "place.pending",
        EventUnion::PlaceJoined(_) => "place.joined",
        EventUnion::PlacePresent(_) => "place.present",
        EventUnion::PlaceAbsent(_) => "place.absent",
        EventUnion::PlaceRemoved(_) => "place.removed",
        EventUnion::PlaceChanged(_) => "place.changed",
        EventUnion::AgentsChanged(_) => "agents.changed",
        EventUnion::UsageAlert(_) => "usage.alert",
        EventUnion::HostNotice(_) => "host.notice",
        EventUnion::AsideText(_) => "aside.text",
    }
}

/// The `seq` an event carries, where it carries one.
pub fn event_seq(event: &EventUnion) -> Option<u64> {
    let seq = match event {
        EventUnion::WorkspaceCreating(v) => v.seq,
        EventUnion::WorkspaceCreated(v) => v.seq,
        EventUnion::WorkspaceNapped(v) => v.seq,
        EventUnion::WorkspaceWoken(v) => v.seq,
        EventUnion::WorkspaceUpgraded(v) => v.seq,
        EventUnion::WorkspaceRenamed(v) => v.seq,
        EventUnion::WorkspaceLook(v) => v.seq,
        EventUnion::WorkspaceDeleted(v) => v.seq,
        EventUnion::WorkspaceGone(v) => v.seq,
        EventUnion::WorkspaceReleased(v) => v.seq,
        EventUnion::WorkspaceStatus(v) => v.seq,
        EventUnion::WorkspaceReview(v) => v.seq,
        EventUnion::WorkspaceViewed(v) => v.seq,
        EventUnion::WorkspaceCost(v) => v.seq,
        EventUnion::SessionStart(v) => v.seq,
        EventUnion::SessionDelta(v) => v.seq,
        EventUnion::SessionDone(v) => v.seq,
        EventUnion::SessionEnd(v) => v.seq,
        EventUnion::SessionSteer(v) => v.seq,
        EventUnion::SessionNotify(v) => v.seq,
        EventUnion::SessionPermission(v) => v.seq,
        EventUnion::SessionPermissionClosed(v) => v.seq,
        EventUnion::SessionCheckpoint(v) => v.seq,
        EventUnion::SessionChanges(v) => v.seq,
        EventUnion::SessionPlan(v) => v.seq,
        EventUnion::SessionCompacted(v) => v.seq,
        EventUnion::SessionRetry(v) => v.seq,
        EventUnion::SessionContext(v) => v.seq,
        EventUnion::SessionRun(v) => v.seq,
        EventUnion::SessionMoved(v) => v.seq,
        EventUnion::SessionBehind(v) => v.seq,
        EventUnion::SessionNotice(v) => v.seq,
        EventUnion::SessionEarlier(v) => v.seq,
        EventUnion::SessionCapped(v) => v.seq,
        EventUnion::SessionUncapped(v) => v.seq,
        EventUnion::SessionSubagent(v) => v.seq,
        EventUnion::SessionSlate(v) => v.seq,
        EventUnion::ThreadWaited(v) => v.seq,
        EventUnion::SlateValues(v) => v.seq,
        EventUnion::SlateRun(v) => v.seq,
        EventUnion::UsageAccount(v) => v.seq,
        EventUnion::SessionQueued(v) => v.seq,
        EventUnion::SessionHeld(v) => v.seq,
        EventUnion::SessionRow(v) => v.seq,
        EventUnion::SessionStarting(v) => v.seq,
        EventUnion::ThreadMarked(v) => v.seq,
        EventUnion::ThreadHead(v) => v.seq,
        EventUnion::ThreadRewound(v) => v.seq,
        EventUnion::PortOpen(v) => v.seq,
        EventUnion::PortClose(v) => v.seq,
        EventUnion::InboxFile(v) => v.seq,
        EventUnion::GoldenStage(v) => v.seq,
        EventUnion::ForwardOpen(v) => v.seq,
        EventUnion::ForwardClose(v) => v.seq,
        EventUnion::ProjectAdded(v) => v.seq,
        EventUnion::ProjectAdd(v) => v.seq,
        EventUnion::ProjectRemoved(v) => v.seq,
        EventUnion::ProjectImport(v) => v.seq,
        EventUnion::ProjectExport(v) => v.seq,
        EventUnion::PreferencesChanged(v) => v.seq,
        EventUnion::ReleaseChanged(v) => v.seq,
        EventUnion::PlaceStage(v) => v.seq,
        EventUnion::PlaceSetup(v) => v.seq,
        EventUnion::PlaceSync(v) => v.seq,
        EventUnion::RecipesChanged(v) => v.seq,
        EventUnion::PlacePending(v) => v.seq,
        EventUnion::PlaceJoined(v) => v.seq,
        EventUnion::PlacePresent(v) => v.seq,
        EventUnion::PlaceAbsent(v) => v.seq,
        EventUnion::PlaceRemoved(v) => v.seq,
        EventUnion::PlaceChanged(v) => v.seq,
        EventUnion::AgentsChanged(v) => v.seq,
        EventUnion::UsageAlert(v) => v.seq,
        EventUnion::HostNotice(v) => v.seq,
        EventUnion::AsideText(v) => v.seq,
    };
    seq.map(|n| n.get() as u64)
}

/// The event with `seq` set, every other key as it was.
pub fn with_seq(event: EventUnion, seq: u64) -> EventUnion {
    match event {
        EventUnion::WorkspaceCreating(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceCreating(v)
        }
        EventUnion::WorkspaceCreated(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceCreated(v)
        }
        EventUnion::WorkspaceNapped(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceNapped(v)
        }
        EventUnion::WorkspaceWoken(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceWoken(v)
        }
        EventUnion::WorkspaceUpgraded(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceUpgraded(v)
        }
        EventUnion::WorkspaceRenamed(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceRenamed(v)
        }
        EventUnion::WorkspaceLook(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceLook(v)
        }
        EventUnion::WorkspaceDeleted(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceDeleted(v)
        }
        EventUnion::WorkspaceGone(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceGone(v)
        }
        EventUnion::WorkspaceReleased(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceReleased(v)
        }
        EventUnion::WorkspaceStatus(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceStatus(v)
        }
        EventUnion::WorkspaceReview(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceReview(v)
        }
        EventUnion::WorkspaceViewed(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceViewed(v)
        }
        EventUnion::WorkspaceCost(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::WorkspaceCost(v)
        }
        EventUnion::SessionStart(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionStart(v)
        }
        EventUnion::SessionDelta(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionDelta(v)
        }
        EventUnion::SessionDone(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionDone(v)
        }
        EventUnion::SessionEnd(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionEnd(v)
        }
        EventUnion::SessionSteer(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionSteer(v)
        }
        EventUnion::SessionNotify(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionNotify(v)
        }
        EventUnion::SessionPermission(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionPermission(v)
        }
        EventUnion::SessionPermissionClosed(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionPermissionClosed(v)
        }
        EventUnion::SessionCheckpoint(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionCheckpoint(v)
        }
        EventUnion::SessionChanges(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionChanges(v)
        }
        EventUnion::SessionPlan(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionPlan(v)
        }
        EventUnion::SessionCompacted(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionCompacted(v)
        }
        EventUnion::SessionRetry(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionRetry(v)
        }
        EventUnion::SessionContext(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionContext(v)
        }
        EventUnion::SessionRun(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionRun(v)
        }
        EventUnion::SessionMoved(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionMoved(v)
        }
        EventUnion::SessionBehind(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionBehind(v)
        }
        EventUnion::SessionNotice(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionNotice(v)
        }
        EventUnion::SessionEarlier(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionEarlier(v)
        }
        EventUnion::SessionCapped(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionCapped(v)
        }
        EventUnion::SessionUncapped(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionUncapped(v)
        }
        EventUnion::SessionSubagent(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionSubagent(v)
        }
        EventUnion::SessionSlate(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionSlate(v)
        }
        EventUnion::ThreadWaited(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ThreadWaited(v)
        }
        EventUnion::SlateValues(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SlateValues(v)
        }
        EventUnion::SlateRun(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SlateRun(v)
        }
        EventUnion::UsageAccount(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::UsageAccount(v)
        }
        EventUnion::SessionQueued(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionQueued(v)
        }
        EventUnion::SessionHeld(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionHeld(v)
        }
        EventUnion::SessionRow(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionRow(v)
        }
        EventUnion::SessionStarting(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::SessionStarting(v)
        }
        EventUnion::ThreadMarked(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ThreadMarked(v)
        }
        EventUnion::ThreadHead(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ThreadHead(v)
        }
        EventUnion::ThreadRewound(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ThreadRewound(v)
        }
        EventUnion::PortOpen(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PortOpen(v)
        }
        EventUnion::PortClose(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PortClose(v)
        }
        EventUnion::InboxFile(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::InboxFile(v)
        }
        EventUnion::GoldenStage(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::GoldenStage(v)
        }
        EventUnion::ForwardOpen(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ForwardOpen(v)
        }
        EventUnion::ForwardClose(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ForwardClose(v)
        }
        EventUnion::ProjectAdded(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ProjectAdded(v)
        }
        EventUnion::ProjectAdd(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ProjectAdd(v)
        }
        EventUnion::ProjectRemoved(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ProjectRemoved(v)
        }
        EventUnion::ProjectImport(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ProjectImport(v)
        }
        EventUnion::ProjectExport(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ProjectExport(v)
        }
        EventUnion::PreferencesChanged(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PreferencesChanged(v)
        }
        EventUnion::ReleaseChanged(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::ReleaseChanged(v)
        }
        EventUnion::PlaceStage(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceStage(v)
        }
        EventUnion::PlaceSetup(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceSetup(v)
        }
        EventUnion::PlaceSync(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceSync(v)
        }
        EventUnion::RecipesChanged(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::RecipesChanged(v)
        }
        EventUnion::PlacePending(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlacePending(v)
        }
        EventUnion::PlaceJoined(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceJoined(v)
        }
        EventUnion::PlacePresent(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlacePresent(v)
        }
        EventUnion::PlaceAbsent(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceAbsent(v)
        }
        EventUnion::PlaceRemoved(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceRemoved(v)
        }
        EventUnion::PlaceChanged(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::PlaceChanged(v)
        }
        EventUnion::AgentsChanged(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::AgentsChanged(v)
        }
        EventUnion::UsageAlert(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::UsageAlert(v)
        }
        EventUnion::HostNotice(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::HostNotice(v)
        }
        EventUnion::AsideText(mut v) => {
            v.seq = Some(JsInt::from(seq as i64));
            EventUnion::AsideText(v)
        }
    }
}
