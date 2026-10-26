// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-app-procs.ts at 65f6b89c7: the desktop app's processes, named by the app
//! and held here for the pages that list this computer's processes. The app's socket offers them and is told when
//! the first page starts watching and when the last one stops, so Electron is read only while somebody looks; each
//! list it sends goes to every socket that watches, and goes with the socket that sent it. Spec:
//! spec/serve_helpers.md, H1-H8.
use std::cell::RefCell;
use std::rc::Rc;

use serde_json::Value;
use wsp_wire::views::app_procs::AppProc;
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

pub const APP_PROCS_OFF_ROAD: &str = "only the wsp app on this computer names its processes, over the host's own road";
pub const APP_PROCS_WATCH_OFF_ROAD: &str =
    "the app's processes name the sites in the person's browser tabs, so only the host's own road reads them";
pub const APP_PROCS_UNOFFERED: &str = "appProcs.set comes after appProcs.offer on the same socket";

/// `msg.op.startsWith("appProcs.")`.
pub fn is_app_procs_request(msg: &RuntimeRequest) -> bool {
    msg.op.name().starts_with("appProcs.")
}

/// What a push to a socket carries: `{"type":"app.procs","procs":[..]}` or `{"type":"app.procs.wanted","wanted":..}`.
pub type Push = Rc<dyn Fn(Payload)>;
/// The list a socket's close runs, which is also how the hub knows the socket again (`Rc::ptr_eq`).
pub type DetachList = Rc<Detaches>;
type Detaches = RefCell<Vec<Box<dyn FnOnce()>>>;

/// One socket as the hub needs it. serve-app-procs.ts:17-24.
#[derive(Clone)]
pub struct AppProcsSocket {
    pub send: Push,
    pub detaches: DetachList,
    /// Whether the socket came in on the host's own road rather than through the relay.
    pub own: bool,
}

/// One offering socket: its push and its newest list.
pub(crate) struct Offer {
    pub push: Push,
    pub procs: Vec<AppProc>,
}

/// The hub's state: the watchers and the offers in the order they came, each keyed by the socket's detach list, and
/// the sockets whose close already lets go of both. serve-app-procs.ts:33-57.
#[derive(Default)]
pub struct AppProcsHub {
    pub(crate) watchers: RefCell<Vec<(DetachList, Push)>>,
    pub(crate) offers: RefCell<Vec<(DetachList, Offer)>>,
    pub(crate) joined: RefCell<Vec<std::rc::Weak<Detaches>>>,
}

impl AppProcsHub {
    /// `appProcsHub()`: an empty hub.
    pub fn new() -> Rc<AppProcsHub> {
        Rc::new(AppProcsHub::default())
    }
    /// Answers one `appProcs.*` op: what the reply carries beside id and ok (spec H3-H8). Takes `self` by `Rc`
    /// because the close it pushes onto the socket holds the hub.
    pub fn answer(self: &Rc<Self>, msg: &RuntimeRequest, socket: &AppProcsSocket) -> Result<Payload, HostError> {
        let key = &socket.detaches;
        match &msg.op {
            RuntimeOp::AppProcsWatch { .. } => {
                if !socket.own {
                    return Err(HostError::new(APP_PROCS_WATCH_OFF_ROAD));
                }
                self.join(key);
                let first = self.watchers.borrow().is_empty();
                {
                    let mut watchers = self.watchers.borrow_mut();
                    match watchers.iter_mut().find(|(held, _)| Rc::ptr_eq(held, key)) {
                        Some((_, push)) => *push = socket.send.clone(),
                        None => watchers.push((key.clone(), socket.send.clone())),
                    }
                }
                if first {
                    self.turned(true);
                }
                Ok(only("procs", procs_value(&self.all())))
            }
            RuntimeOp::AppProcsUnwatch { .. } => {
                self.unwatch(key);
                Ok(Payload::new())
            }
            RuntimeOp::AppProcsOffer { .. } => {
                if !socket.own {
                    return Err(HostError::new(APP_PROCS_OFF_ROAD));
                }
                self.join(key);
                {
                    let mut offers = self.offers.borrow_mut();
                    match offers.iter_mut().find(|(held, _)| Rc::ptr_eq(held, key)) {
                        Some((_, offer)) => offer.push = socket.send.clone(),
                        None => offers.push((key.clone(), Offer { push: socket.send.clone(), procs: Vec::new() })),
                    }
                }
                Ok(only("wanted", Value::Bool(!self.watchers.borrow().is_empty())))
            }
            RuntimeOp::AppProcsSet { procs, .. } => {
                if !self.offers.borrow().iter().any(|(held, _)| Rc::ptr_eq(held, key)) {
                    return Err(HostError::new(APP_PROCS_UNOFFERED));
                }
                if self.watchers.borrow().is_empty() {
                    return Ok(Payload::new());
                }
                if let Some((_, offer)) = self.offers.borrow_mut().iter_mut().find(|(held, _)| Rc::ptr_eq(held, key)) {
                    offer.procs = procs.0.clone();
                }
                self.push_to_watchers(pushed("app.procs", "procs", procs_value(&self.all())));
                Ok(Payload::new())
            }
            _ => Err(HostError::new(format!("{} is not answered here", msg.op.name()))),
        }
    }

    fn all(&self) -> Vec<AppProc> {
        self.offers.borrow().iter().flat_map(|(_, offer)| offer.procs.iter().cloned()).collect()
    }

    /// Every offer is told whether anyone watches; the lists are dropped when nobody does.
    fn turned(&self, wanted: bool) {
        let pushes: Vec<Push> = self
            .offers
            .borrow_mut()
            .iter_mut()
            .map(|(_, offer)| {
                if !wanted {
                    offer.procs.clear();
                }
                offer.push.clone()
            })
            .collect();
        for push in pushes {
            push(pushed("app.procs.wanted", "wanted", Value::Bool(wanted)));
        }
    }

    fn push_to_watchers(&self, frame: Payload) {
        let pushes: Vec<Push> = self.watchers.borrow().iter().map(|(_, push)| push.clone()).collect();
        for push in pushes {
            push(frame.clone());
        }
    }

    fn unwatch(&self, key: &DetachList) {
        let (removed, empty) = {
            let mut watchers = self.watchers.borrow_mut();
            let before = watchers.len();
            watchers.retain(|(held, _)| !Rc::ptr_eq(held, key));
            (watchers.len() != before, watchers.is_empty())
        };
        if removed && empty {
            self.turned(false);
        }
    }

    /// The first ask of either kind from a socket adds its one close, which unwatches it and drops its offer.
    fn join(self: &Rc<Self>, key: &DetachList) {
        {
            let mut joined = self.joined.borrow_mut();
            joined.retain(|known| known.strong_count() > 0);
            if joined.iter().any(|known| known.upgrade().is_some_and(|known| Rc::ptr_eq(&known, key))) {
                return;
            }
            joined.push(Rc::downgrade(key));
        }
        let hub = self.clone();
        let own = key.clone();
        key.borrow_mut().push(Box::new(move || {
            hub.unwatch(&own);
            let before = hub.offers.borrow().len();
            hub.offers.borrow_mut().retain(|(held, _)| !Rc::ptr_eq(held, &own));
            if hub.offers.borrow().len() != before {
                hub.push_to_watchers(pushed("app.procs", "procs", procs_value(&hub.all())));
            }
        }));
    }
}

fn only(key: &str, value: Value) -> Payload {
    let mut out = Payload::new();
    out.insert(key.into(), value);
    out
}

fn pushed(kind: &str, key: &str, value: Value) -> Payload {
    let mut frame = Payload::new();
    frame.insert("type".into(), Value::from(kind));
    frame.insert(key.into(), value);
    frame
}

fn procs_value(procs: &[AppProc]) -> Value {
    serde_json::to_value(procs).unwrap_or(Value::Null)
}

thread_local! {
    /// The weak map of `appProcsHubOf`: a hub by its owner's address, kept while the owner lives.
    static HUBS: RefCell<Vec<(usize, Rc<dyn Fn() -> bool>, Rc<AppProcsHub>)>> = const { RefCell::new(Vec::new()) };
}

/// The hub of the owner at `key`, made at the first ask and dropped with its owner (`alive` says whether it stands).
pub(crate) fn hub_at(key: usize, alive: Rc<dyn Fn() -> bool>) -> Rc<AppProcsHub> {
    HUBS.with(|hubs| {
        let mut hubs = hubs.borrow_mut();
        hubs.retain(|(_, alive, _)| alive());
        if let Some((_, _, hub)) = hubs.iter().find(|(held, _, _)| *held == key) {
            return hub.clone();
        }
        let hub = AppProcsHub::new();
        hubs.push((key, alive, hub.clone()));
        hub
    })
}

/// `appProcsHubOf(owner)`: the one hub of a host, by the runtime it serves (a weak map keyed by the owner). Two
/// calls with one owner answer one hub; another owner gets its own.
pub fn app_procs_hub_of(owner: &Rc<dyn std::any::Any>) -> Rc<AppProcsHub> {
    let weak = Rc::downgrade(owner);
    hub_at(Rc::as_ptr(owner) as *const () as usize, Rc::new(move || weak.strong_count() > 0))
}

/// The hub of a served runtime, keyed as `app_procs_hub_of` keys any owner.
pub(crate) fn app_procs_hub_for(rt: &Rc<crate::serve::served::DynRuntime>) -> Rc<AppProcsHub> {
    let weak = Rc::downgrade(rt);
    hub_at(Rc::as_ptr(rt) as *const () as usize, Rc::new(move || weak.strong_count() > 0))
}
