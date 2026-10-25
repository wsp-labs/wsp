// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-app-procs.ts at 65f6b89c7: the desktop app's processes, named by the app
//! and held here for the pages that list this computer's processes. The app's socket offers them and is told when
//! the first page starts watching and when the last one stops, so Electron is read only while somebody looks; each
//! list it sends goes to every socket that watches, and goes with the socket that sent it. Spec:
//! spec/serve_helpers.md, H1-H8.
use std::cell::RefCell;
use std::rc::Rc;

use wsp_wire::views::app_procs::AppProc;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve::served::Payload;

pub const APP_PROCS_OFF_ROAD: &str = "only the wsp app on this computer names its processes, over the host's own road";
pub const APP_PROCS_WATCH_OFF_ROAD: &str = "the app's processes name the sites in the person's browser tabs, so only the host's own road reads them";
pub const APP_PROCS_UNOFFERED: &str = "appProcs.set comes after appProcs.offer on the same socket";

/// `msg.op.startsWith("appProcs.")`.
pub fn is_app_procs_request(msg: &RuntimeRequest) -> bool {
    todo!()
}

/// What a push to a socket carries: `{"type":"app.procs","procs":[..]}` or `{"type":"app.procs.wanted","wanted":..}`.
pub type Push = Rc<dyn Fn(Payload)>;
/// The list a socket's close runs, which is also how the hub knows the socket again (`Rc::ptr_eq`).
pub type DetachList = Rc<RefCell<Vec<Box<dyn FnOnce()>>>>;

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
    pub(crate) joined: RefCell<Vec<std::rc::Weak<RefCell<Vec<Box<dyn FnOnce()>>>>>>,
}

impl AppProcsHub {
    /// `appProcsHub()`: an empty hub.
    pub fn new() -> Rc<AppProcsHub> {
        todo!()
    }
    /// Answers one `appProcs.*` op: what the reply carries beside id and ok (spec H3-H8). Takes `self` by `Rc`
    /// because the close it pushes onto the socket holds the hub.
    pub fn answer(self: &Rc<Self>, msg: &RuntimeRequest, socket: &AppProcsSocket) -> Result<Payload, HostError> {
        todo!()
    }
}

/// `appProcsHubOf(owner)`: the one hub of a host, by the runtime it serves (a weak map keyed by the owner). Two
/// calls with one owner answer one hub; another owner gets its own.
pub fn app_procs_hub_of(owner: &Rc<dyn std::any::Any>) -> Rc<AppProcsHub> {
    todo!()
}

