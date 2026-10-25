// SPDX-License-Identifier: AGPL-3.0-only
//! The door every socket passes before it is let in, and the refusal log. serve.ts:349-367, 444-463, 493-848.
//! Spec: spec/serve_door.md.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use wsp_wire::wire::devices::TicketPurpose;
use wsp_words::exit::HostError;

use wsp_core::forwards::{ForwardEvent, ForwardsSource};
use wsp_core::types::wiring::Detach;
use wsp_machines::places::pane_ports::PanePorts;
use wsp_wire::wire::limits::PortForward;

use super::served::{DynRuntime, Payload};
use super::socket::{Incoming, Socket};
use super::{Authed, NowFn, Released, ServeOptions};
use crate::serve_app_procs::AppProcsHub;

/// A ticket minted over an authed socket, redeemed once as `?ticket=` on the next connect. serve.ts:319-325.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub purpose: TicketPurpose,
    pub expires_at: i64,
    /// Whether the socket that minted it was a computer the person paired.
    pub paired: bool,
}

/// What every socket of one server shares (serve.ts:376-418, 453). Nothing here outlives the process (spec R7).
pub struct Served {
    pub rt: Rc<DynRuntime>,
    pub opts: ServeOptions,
    pub(crate) now: NowFn,
    pub(crate) released: Released,
    pub(crate) ticket_ttl_ms: u64,
    pub(crate) pair_ttl_ms: u64,
    pub(crate) auth_deadline_ms: u64,
    pub(crate) tickets: RefCell<IndexMap<String, Ticket>>,
    /// The computers a doctor's road runs on now; one set per server (serve.ts:391-393).
    pub(crate) doctoring: RefCell<HashSet<String>>,
    /// The restart this host is going through, so asks that overlap start one (serve.ts:394-395).
    pub(crate) restarting: RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>,
    /// Every live socket a device holds, by hold id: the device id and the socket (serve.ts:451-453).
    pub(crate) held: RefCell<IndexMap<u64, (String, Weak<Socket>)>>,
    pub(crate) next_hold: Cell<u64>,
    /// `forwardsOf(opts.forwards, rt.places?.paneForwards)`, built at the first forwards op: the panes' source is read
    /// from the place door then, not at start.
    pub(crate) forwards: std::cell::OnceCell<Rc<dyn ForwardsSource>>,
    pub(crate) app_procs: Rc<AppProcsHub>,
}

/// The panes' forwards as the one source `forwardsOf` takes. wsp-machines' `PanePorts` lists, stops and tells of its
/// forwards through its own `PaneForwards` and `ForwardEvent`, the same TS type as wsp-core's `ForwardsSource`; this
/// maps the one onto the other until it implements the core's.
pub(crate) struct PaneSource(pub Rc<PanePorts>);

impl ForwardsSource for PaneSource {
    fn list(&self) -> Vec<PortForward> {
        todo!()
    }
    fn stop(&self, workspace_id: &str, port: i64) -> bool {
        todo!()
    }
    fn on(&self, listener: Rc<dyn Fn(&ForwardEvent)>) -> Detach {
        todo!()
    }
}

impl Served {
    /// The shared state of one server over `rt` (serve.ts:376-395): refuses an empty host token (spec D1) and reads
    /// every default (spec W1). The app-process hub is the runtime's own (`app_procs_hub_of`).
    pub fn new(rt: Rc<DynRuntime>, opts: ServeOptions) -> Result<Rc<Served>, HostError> {
        todo!()
    }
    /// `now()`.
    pub fn now(&self) -> i64 {
        todo!()
    }
    /// The tickets held now, for the tests' reading of D2 and P17.
    pub fn tickets(&self) -> IndexMap<String, Ticket> {
        todo!()
    }
    /// Who a token names (serve.ts:444-449, spec D19): the host's own by `safe_equal`, else the device door's match;
    /// nobody with no door; an error is nobody to the door's callers.
    pub async fn who_is(&self, token: &str) -> Result<Option<Authed>, HostError> {
        todo!()
    }
    /// The one road a device is taken away by: the record, then `answer`, then every socket it held closed 4401
    /// (serve.ts:458-463, spec W8, P27).
    pub async fn revoke_device(&self, id: &str, answer: Option<Box<dyn FnOnce(bool)>>) -> Result<bool, HostError> {
        todo!()
    }
    /// The one mint a join code comes from (serve.ts:401).
    pub async fn issue_code(&self, here: bool) -> Result<wsp_core::devices::IssuedCode, HostError> {
        todo!()
    }
}

impl Socket {
    /// One frame off the socket (serve.ts:628-848): the checks every frame passes (spec D6-D11), then the door while
    /// the socket is not let in (D12-D19), else the switch (`answer_frame`).
    pub fn on_message(self: &Rc<Self>, raw: Incoming) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// Takes both limits off: the auth frame passed, a code paid, a device admitted, a place answered, the deadline,
    /// the cap, the close (serve.ts:594-599, spec D5).
    pub(crate) fn through_door(&self) {
        todo!()
    }
    /// Remembers the socket under the device that authed it and takes the road off that device (serve.ts:608-614,
    /// spec D25).
    pub(crate) fn bind(self: &Rc<Self>, device: wsp_core::devices::HeldDevice) {
        todo!()
    }
}

/// The log line of one refused frame (serve.ts:357-367, spec D20-D24): `refused <op> kind=<kind>: <sentence>`.
pub fn refused_line(frame: Option<&serde_json::Value>, payload: &Payload, said: Option<&str>) -> String {
    todo!()
}
