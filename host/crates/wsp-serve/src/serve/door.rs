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

use wsp_core::devices::{safe_equal, DeviceDoor, NO_DEVICE_DOOR};
use wsp_wire::types::daemon_contract::AUTH_DEADLINE_MS;
use wsp_wire::wire::devices::PAIR_CODE_TTL_MS;
use wsp_wire::wire::runtime::{request_secrets, RUNTIME_OPS};
use wsp_words::exit::marked_cut;
use wsp_words::words::computer::{is_object_frame, UNAUTHORIZED};
use wsp_words::words::thread::redacted;

use super::served::{DynRuntime, Payload};
use super::socket::{Incoming, Socket};
use super::{Authed, NowFn, Released, ServeOptions, NO_AUTH_TOKEN, SCANNED_SECRETS, TICKET_TTL_MS};
use crate::serve_app_procs::{app_procs_hub_for, AppProcsHub};

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
        if opts.auth_token.is_empty() {
            return Err(HostError::new(NO_AUTH_TOKEN));
        }
        let now: NowFn = opts.now.clone().unwrap_or_else(|| Rc::new(wall_ms));
        let released = opts.released.clone().unwrap_or_default();
        let ticket_ttl_ms = opts.ticket_ttl_ms.unwrap_or(TICKET_TTL_MS);
        let pair_ttl_ms = opts.pair_ttl_ms.unwrap_or(PAIR_CODE_TTL_MS as u64);
        let auth_deadline_ms = opts.auth_deadline_ms.unwrap_or(AUTH_DEADLINE_MS);
        let app_procs = app_procs_hub_for(&rt);
        Ok(Rc::new(Served {
            rt,
            opts,
            now,
            released,
            ticket_ttl_ms,
            pair_ttl_ms,
            auth_deadline_ms,
            tickets: RefCell::new(IndexMap::new()),
            doctoring: RefCell::new(HashSet::new()),
            restarting: RefCell::new(None),
            held: RefCell::new(IndexMap::new()),
            next_hold: Cell::new(0),
            forwards: std::cell::OnceCell::new(),
            app_procs,
        }))
    }
    /// `now()`.
    pub fn now(&self) -> i64 {
        (self.now)()
    }
    /// The tickets held now, for the tests' reading of D2 and P17.
    pub fn tickets(&self) -> IndexMap<String, Ticket> {
        self.tickets.borrow().clone()
    }
    /// The device door, or the refusal naming that none is wired (`devices()`, serve.ts:396-399).
    pub(crate) fn devices(&self) -> Result<Rc<dyn DeviceDoor>, HostError> {
        self.opts.devices.clone().ok_or_else(|| HostError::new(NO_DEVICE_DOOR))
    }
    /// Who a token names (serve.ts:444-449, spec D19): the host's own by `safe_equal`, else the device door's match;
    /// nobody with no door; an error is nobody to the door's callers.
    pub async fn who_is(&self, token: &str) -> Result<Option<Authed>, HostError> {
        if safe_equal(token, &self.opts.auth_token) {
            return Ok(Some(Authed::Host));
        }
        let Some(devices) = &self.opts.devices else {
            return Ok(None);
        };
        Ok(devices.r#match(token).await?.map(Authed::Device))
    }
    /// The one road a device is taken away by: the record, then `answer`, then every socket it held closed 4401
    /// (serve.ts:458-463, spec W8, P27).
    pub async fn revoke_device(&self, id: &str, answer: Option<Box<dyn FnOnce(bool)>>) -> Result<bool, HostError> {
        let revoked = self.devices()?.revoke(id).await?;
        if let Some(answer) = answer {
            answer(revoked);
        }
        let cut: Vec<Rc<Socket>> =
            self.held.borrow().values().filter(|(device, _)| device == id).filter_map(|(_, socket)| socket.upgrade()).collect();
        for socket in cut {
            socket.sink.close(4401, UNAUTHORIZED);
        }
        Ok(revoked)
    }
    /// The one mint a join code comes from (serve.ts:401).
    pub async fn issue_code(&self, here: bool) -> Result<wsp_core::devices::IssuedCode, HostError> {
        self.devices()?.issue(self.now(), self.pair_ttl_ms, here).await
    }
}

/// The wall clock in milliseconds, as `Date.now`.
fn wall_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_millis() as i64).unwrap_or(0)
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
        self.through.set(true);
        if let Some(timer) = self.deadline.borrow_mut().take() {
            timer.abort();
        }
    }
    /// Remembers the socket under the device that authed it and takes the road off that device (serve.ts:608-614,
    /// spec D25).
    pub(crate) fn bind(self: &Rc<Self>, device: wsp_core::devices::HeldDevice) {
        let id = device.view.id.clone();
        {
            let mut road = self.road.borrow_mut();
            road.paired_road = device.view.scope.is_none() && device.view.here.is_none();
            road.me = Some(Authed::Device(device));
        }
        let hold = self.served.next_hold.get();
        self.served.next_hold.set(hold + 1);
        self.served.held.borrow_mut().insert(hold, (id, Rc::downgrade(self)));
        self.bound.set(Some(hold));
    }
}

/// The log line of one refused frame (serve.ts:357-367, spec D20-D24): `refused <op> kind=<kind>: <sentence>`.
pub fn refused_line(frame: Option<&serde_json::Value>, payload: &Payload, said: Option<&str>) -> String {
    let asked = frame.filter(|frame| is_object_frame(frame)).and_then(|frame| frame.get("op")).and_then(|op| op.as_str());
    let op = match asked {
        Some(op) if RUNTIME_OPS.contains(&op) => op,
        _ => "frame",
    };
    let kind = payload.get("kind").and_then(|kind| kind.as_str()).unwrap_or("none");
    let mut secrets: Vec<String> = Vec::new();
    for secret in frame.map(request_secrets).unwrap_or_default() {
        if !secrets.contains(&secret) {
            secrets.push(secret);
        }
    }
    if secrets.len() > SCANNED_SECRETS {
        return format!("refused {op} kind={kind}: (sentence withheld, {} secret values)", secrets.len());
    }
    let text = match said {
        Some(said) => said.to_string(),
        None => js_string(payload.get("error")),
    };
    let first = text.split('\n').next().unwrap_or("");
    format!("refused {op} kind={kind}: {}", marked_cut(&redacted(first, &secrets), 400))
}

/// `String(value)` for the one value a refusal line reads.
fn js_string(value: Option<&serde_json::Value>) -> String {
    match value {
        None => "undefined".to_string(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}
