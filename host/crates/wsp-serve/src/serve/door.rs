// SPDX-License-Identifier: AGPL-3.0-only
//! The door every socket passes before it is let in, and the refusal log. serve.ts:349-367, 444-463, 493-848.
//! Spec: spec/serve_door.md.
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_wire::wire::devices::TicketPurpose;
use wsp_words::exit::HostError;

use wsp_core::forwards::{forwards_of, ForwardEvent, ForwardsSource};
use wsp_core::types::wiring::Detach;
use wsp_machines::places::pane_ports::{ForwardEvent as PaneEvent, PaneForwards, PanePorts};
use wsp_machines::places::types::{AuthAnswer, Proved, NO_PLACE_DOOR};
use wsp_store::keys::{key_fingerprint, open_frame, verify_place_bytes, Frame};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::place::{ForwardCloseEvent, ForwardOpenEvent};
use wsp_wire::wire::limits::{PortForward, RelayPort};
use wsp_wire::wire::place_link::{
    DeviceAuthRequest, PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, SealOpenRequest, PLACE_CODE_REFUSAL, SEAL_CLIENT,
    SEAL_UNSERVED,
};
use wsp_wire::wire::runtime_ops::{parse_runtime_request, RuntimeOp, RuntimeRequest};

use wsp_core::devices::{safe_equal, thread_of, DeviceDoor, HeldDevice, NO_DEVICE_DOOR};
use wsp_wire::types::daemon_contract::AUTH_DEADLINE_MS;
use wsp_wire::wire::devices::{
    device_admission_transcript, AccountDevice, DeviceVia, DEVICE_ACCOUNT_UNSERVED, DEVICE_AUTH_REFUSAL, DEVICE_REVOKED_REFUSAL,
    PAIR_CODE_REFUSAL, PAIR_CODE_TTL_MS,
};
use wsp_wire::wire::runtime::{request_secrets, DEVICE_OPS, RUNTIME_OPS, THREAD_OPS};
use wsp_words::exit::marked_cut;
use wsp_words::v8_json::{parse_json, parse_json_lax};
use wsp_words::wire::runtime::thread_op_refusal;
use wsp_words::words::computer::{device_held_refusal, is_object_frame, REQUEST_NOT_AN_OBJECT, SCOPED_TOKEN_ROAD_REFUSAL, UNAUTHORIZED};
use wsp_words::words::thread::{issues_line, redacted, Issue, PathPart};

use super::served::{DynRuntime, Payload};
use super::socket::{Incoming, Socket, INFINITE_ID};
use super::{Authed, NowFn, Released, ServeOptions, Signer, INVALID_JSON, NO_AUTH_TOKEN, SCANNED_SECRETS, TICKET_TTL_MS};
use crate::serve::answer::answer_frame;
use crate::serve_app_procs::{app_procs_hub_for, AppProcsHub};

/// The close code a socket refused at the door is closed with (socket.rs keeps the same one).
const UNAUTHORIZED_CLOSE: u16 = 4401;

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
        self.0.list()
    }
    fn stop(&self, workspace_id: &str, port: i64) -> bool {
        u16::try_from(port).is_ok_and(|port| self.0.stop(workspace_id, port))
    }
    fn on(&self, listener: Rc<dyn Fn(&ForwardEvent)>) -> Detach {
        self.0.on(Rc::new(move |event: &PaneEvent| listener(&pane_event(event))))
    }
}

/// The pane's event as the core's stream carries it: the same open and close, in the core's view types.
fn pane_event(event: &PaneEvent) -> ForwardEvent {
    match event {
        PaneEvent::Open { forward } => ForwardEvent::Open(ForwardOpenEvent { forward: forward.clone() }),
        PaneEvent::Close { workspace_id, port } => {
            ForwardEvent::Close(ForwardCloseEvent { workspace_id: workspace_id.clone(), port: RelayPort(JsInt::from(i64::from(*port))) })
        }
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
    /// `forwardsOf(opts.forwards, rt.places?.paneForwards)` (serve.ts:377). `serve_runtime` builds it at start; a
    /// `Served` driven without a listener builds it at the first forwards op.
    pub fn forwards_source(&self) -> Rc<dyn ForwardsSource> {
        self.forwards
            .get_or_init(|| {
                let panes = self.rt.places().map(|places| Rc::new(PaneSource(places.pane_forwards())) as Rc<dyn ForwardsSource>);
                forwards_of(vec![self.opts.forwards.clone(), panes])
            })
            .clone()
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
        let socket = self.clone();
        async move { socket.decide(raw).await }.boxed_local()
    }

    async fn decide(self: &Rc<Self>, raw: Incoming) {
        if self.handed_over.get() {
            return;
        }
        let Some((frame, infinite_id)) = self.read(&raw) else {
            self.answer(None, failure(Value::Null, INVALID_JSON), None);
            self.close_unauthed();
            return;
        };
        if infinite_id {
            INFINITE_ID.scope((), self.decided(frame)).await
        } else {
            self.decided(frame).await
        }
    }

    async fn decided(self: &Rc<Self>, frame: Value) {
        if !is_object_frame(&frame) {
            self.answer(Some(&frame), failure(Value::Null, REQUEST_NOT_AN_OBJECT), None);
            self.close_unauthed();
            return;
        }
        let asked = frame.get("op").and_then(Value::as_str);
        let asked_id = frame.get("id").cloned().unwrap_or(Value::Null);
        let by = self.road.borrow().by.clone();
        if let Some(by) = by {
            if !asked.is_some_and(|op| THREAD_OPS.contains(&op)) {
                let place = self.served.rt.workspaces().thread_place(&by);
                let line = thread_op_refusal(asked.unwrap_or("that frame"), &by.thread_id, place.as_deref());
                self.answer(Some(&frame), failure(asked_id, &line), None);
                return;
            }
        }
        let paired = self.road.borrow().as_paired();
        if paired && !asked.is_some_and(|op| DEVICE_OPS.contains(&op)) {
            let line = device_held_refusal(asked.unwrap_or("that frame"));
            self.answer(Some(&frame), failure(asked_id, &line), None);
            return;
        }
        let msg = match parse_runtime_request(&frame) {
            Ok(msg) => msg,
            Err(error) => {
                let issue = Issue { path: error.path.iter().map(path_part).collect(), message: error.message.clone() };
                self.answer(Some(&frame), failure(asked_id, &error.issue_text()), Some(&issues_line(&[issue])));
                self.close_unauthed();
                return;
            }
        };
        if self.authed() {
            return answer_frame(self, frame, msg).await;
        }
        self.at_door(&frame, msg).await;
    }

    /// The frames a socket not let in may send, one at a time (serve.ts:677-848, spec D12-D19).
    async fn at_door(self: &Rc<Self>, frame: &Value, msg: RuntimeRequest) {
        let id = serde_json::to_value(msg.op.id()).unwrap_or(Value::Null);
        let pending = self.proving.borrow_mut().take();
        if let Some((place_id, expect)) = pending {
            let RuntimeOp::PlaceProve(prove) = msg.op else {
                return self.refuse(frame, &id, UNAUTHORIZED);
            };
            return self.prove_place(frame, &id, &place_id, prove, &expect).await;
        }
        if self.deciding.get() {
            return self.refuse(frame, &id, UNAUTHORIZED);
        }
        self.deciding.set(true);
        match msg.op {
            RuntimeOp::SealOpen(open) => self.seal_open(frame, &id, open),
            RuntimeOp::PlaceJoin(join) => self.place_join(frame, &id, join).await,
            RuntimeOp::PlaceAuth(auth) => self.place_auth(frame, &id, auth).await,
            RuntimeOp::PairRedeem { code, name, .. } => self.pair_redeem(frame, &id, &code, &name).await,
            RuntimeOp::DeviceAuth(auth) => self.device_auth(frame, &id, auth).await,
            RuntimeOp::Auth { token, .. } => self.auth(frame, &id, &token).await,
            _ => self.refuse(frame, &id, UNAUTHORIZED),
        }
    }

    /// `seal.open`: agrees the key, answers the challenge in the clear, then seals every frame after it.
    fn seal_open(&self, frame: &Value, id: &Value, open: SealOpenRequest) {
        if self.seal.borrow().is_some() {
            return self.refuse(frame, id, UNAUTHORIZED);
        }
        let Some(places) = self.served.rt.places() else {
            return self.refuse(frame, id, SEAL_UNSERVED);
        };
        let Some(opened) = places.answer_challenge(SEAL_CLIENT, open.nonce.as_str(), Some(open.ephemeral.as_str())) else {
            return self.refuse(frame, id, UNAUTHORIZED);
        };
        self.deciding.set(false);
        self.send(object(json!({
            "id": id,
            "ok": true,
            "nonce": opened.nonce,
            "hostPublicKey": opened.host_public_key,
            "signature": opened.signature,
            "ephemeral": opened.ephemeral,
        })));
        *self.seal.borrow_mut() = Some(opened.seal);
        *self.seal_expect.borrow_mut() = Some(opened.expect);
    }

    /// `place.join`: a computer's first frame; the next one it sends is its prove.
    async fn place_join(self: &Rc<Self>, frame: &Value, id: &Value, join: PlaceJoinRequest) {
        if self.seal.borrow().is_some() {
            return self.refuse(frame, id, UNAUTHORIZED);
        }
        let Some(places) = self.served.rt.places() else {
            return self.refuse(frame, id, NO_PLACE_DOOR);
        };
        let opened = match places.join(join, &self.from, self.served.now() as u64).await {
            Ok(Some(opened)) => opened,
            Ok(None) => return self.refuse(frame, id, PLACE_CODE_REFUSAL),
            Err(error) => return self.refuse(frame, id, &error.message),
        };
        *self.proving.borrow_mut() = Some((opened.reply.place_id.clone(), opened.expect));
        self.deciding.set(false);
        self.through_door();
        let mut payload = object(json!({ "id": id, "ok": true }));
        payload.extend(object(&opened.reply));
        if let Some(notice) = opened.notice {
            payload.insert("notice".into(), Value::from(notice));
        }
        self.send(payload);
        *self.seal.borrow_mut() = Some(opened.seal);
    }

    /// `place.auth`: a place dialling back in; a refusal carries the door's own signed fields.
    async fn place_auth(self: &Rc<Self>, frame: &Value, id: &Value, auth: PlaceAuthRequest) {
        if self.seal.borrow().is_some() {
            return self.refuse(frame, id, UNAUTHORIZED);
        }
        let Some(places) = self.served.rt.places() else {
            return self.refuse(frame, id, NO_PLACE_DOOR);
        };
        let place_id = auth.place_id.clone();
        match places.auth(auth, self.served.now() as u64).await {
            Err(error) => self.refuse(frame, id, &error.message),
            Ok(AuthAnswer::Refusal { refusal, signed }) => self.refuse_signed(frame, id, &refusal, object(&signed)),
            Ok(AuthAnswer::Reply { reply, expect, seal }) => {
                *self.proving.borrow_mut() = Some((place_id, expect));
                self.deciding.set(false);
                self.through_door();
                let mut payload = object(json!({ "id": id, "ok": true }));
                payload.extend(object(&reply));
                self.send(payload);
                *self.seal.borrow_mut() = Some(seal);
            }
        }
    }

    /// `place.prove`: the place's signature and report. A refusal closes; otherwise the socket becomes the place's
    /// link and reads nothing more (D12).
    async fn prove_place(self: &Rc<Self>, frame: &Value, id: &Value, place_id: &str, prove: PlaceProveRequest, expect: &[u8]) {
        let Some(places) = self.served.rt.places() else {
            return self.refuse(frame, id, NO_PLACE_DOOR);
        };
        let now = self.served.now() as u64;
        let proved = match places.prove(place_id, prove, expect, &self.from, now).await {
            Ok(proved) => proved,
            Err(error) => Proved::Refusal { refusal: error.message },
        };
        let (report, device) = match proved {
            Proved::Refusal { refusal } => return self.refuse(frame, id, &refusal),
            Proved::Report { report, device } => (report, device),
        };
        self.handed_over.set(true);
        self.sink.pause();
        let mut payload = object(json!({ "id": id, "ok": true }));
        if let Some(device) = device {
            payload.insert("device".into(), json!({ "deviceId": device.device_id, "deviceToken": device.device_token }));
        }
        self.send(payload);
        let seal = self.seal.borrow_mut().take();
        if places.attach(place_id, self.sink.clone().link(), report, &self.from, now, seal).await.is_err() {
            self.sink.terminate();
        }
    }

    /// `pair.redeem`: a code this host holds lets the computer in and binds its device token.
    async fn pair_redeem(self: &Rc<Self>, frame: &Value, id: &Value, code: &str, name: &str) {
        let paired = match self.served.devices() {
            Ok(devices) => devices.redeem(code, name, self.served.now()).await.ok().flatten(),
            Err(_) => None,
        };
        let Some(paired) = paired else {
            return self.refuse(frame, id, PAIR_CODE_REFUSAL);
        };
        self.authed.set(true);
        self.through_door();
        let reply = object(json!({ "id": id, "ok": true, "deviceId": paired.device_id, "deviceToken": paired.device_token }));
        self.bind(HeldDevice { view: paired.device, road: None });
        self.send(reply);
    }

    /// `device.auth`: a computer on the account signs in with no code, over the seal this socket agreed.
    async fn device_auth(self: &Rc<Self>, frame: &Value, id: &Value, auth: DeviceAuthRequest) {
        let account = match (self.served.opts.admitted.clone(), self.served.opts.devices.is_some()) {
            (Some(account), true) => account,
            _ => return self.refuse(frame, id, DEVICE_ACCOUNT_UNSERVED),
        };
        let Some(signer) = account.signer() else {
            return self.refuse(frame, id, DEVICE_ACCOUNT_UNSERVED);
        };
        let expect = self.seal_expect.borrow().clone();
        let Some(expect) = expect else {
            return self.refuse(frame, id, UNAUTHORIZED);
        };
        let fingerprint = key_fingerprint(auth.public_key.as_str());
        let listed = || account.list().and_then(|rows| rows.into_iter().find(|row| row.fingerprint == fingerprint));
        let mut row = listed();
        if row.is_none() {
            let _ = account.refresh().await;
            row = listed();
        }
        let Some(row) = row else {
            return self.refuse(frame, id, DEVICE_AUTH_REFUSAL);
        };
        let Some(admitted) = self.admitted_by(&signer, &row, &fingerprint).await else {
            return self.refuse(frame, id, DEVICE_AUTH_REFUSAL);
        };
        if !verify_place_bytes(auth.public_key.as_str(), &expect, auth.signature.as_str()) {
            return self.refuse(frame, id, DEVICE_AUTH_REFUSAL);
        }
        let refused = match self.served.devices() {
            Ok(devices) => devices.refuses(&fingerprint).await.unwrap_or(true),
            Err(_) => true,
        };
        if refused {
            return self.refuse(frame, id, DEVICE_REVOKED_REFUSAL);
        }
        let via = DeviceVia {
            kind: "account".into(),
            relay_device_id: row.id.clone(),
            fingerprint: fingerprint.clone(),
            public_key: auth.public_key.as_str().to_string(),
            admitted_by: admitted,
        };
        let admitted_device = match self.served.devices() {
            Ok(devices) => devices.admit_account(&auth.name, via, self.served.now()).await.ok(),
            Err(_) => None,
        };
        let Some(paired) = admitted_device else {
            return self.refuse(frame, id, DEVICE_AUTH_REFUSAL);
        };
        self.authed.set(true);
        self.through_door();
        let mut reply = object(json!({ "id": id, "ok": true, "deviceId": paired.device_id, "deviceToken": paired.device_token }));
        reply.extend(released_fields(&self.served.released));
        self.bind(HeldDevice { view: paired.device, road: None });
        self.send(reply);
    }

    /// The key of the one admission of `row` that a key this host trusts signed, by the admitting key's fingerprint.
    async fn admitted_by(&self, signer: &Signer, row: &AccountDevice, fingerprint: &str) -> Option<String> {
        let here = self.served.devices().ok()?.list().await.ok()?;
        for admission in &row.admissions {
            let key = if admission.by == signer.fingerprint {
                Some(signer.public_key.clone())
            } else {
                here.iter()
                    .filter_map(|device| device.via.as_ref())
                    .find(|via| via.kind == "account" && via.fingerprint == admission.by)
                    .map(|via| via.public_key.clone())
            };
            let Some(key) = key else { continue };
            let transcript = device_admission_transcript(fingerprint, &admission.by, &admission.issued_at);
            if verify_place_bytes(&key, &transcript, &admission.signature) {
                return Some(admission.by.clone());
            }
        }
        None
    }

    /// `auth`: the host's own token, or a device's. A thread's token takes its road and its thread.
    async fn auth(self: &Rc<Self>, frame: &Value, id: &Value, token: &str) {
        // The TS awaits `whoIs` even for the host's own token, so a frame that comes in the same read finds the door deciding.
        tokio::task::yield_now().await;
        let Ok(Some(who)) = self.served.who_is(token).await else {
            return self.refuse(frame, id, UNAUTHORIZED);
        };
        if let Authed::Device(device) = &who {
            if device.view.scope.is_some() && !self.dialled_here() {
                return self.refuse(frame, id, SCOPED_TOKEN_ROAD_REFUSAL);
            }
        }
        self.authed.set(true);
        self.through_door();
        match who {
            Authed::Device(device) => {
                let thread = thread_of(&device);
                let device_id = device.view.id.clone();
                self.bind(device);
                if let Some(thread) = thread {
                    let mut road = self.road.borrow_mut();
                    road.stamped = Some(thread.road);
                    road.by = Some(thread.by);
                }
                let devices = self.served.opts.devices.clone();
                let now = self.served.now();
                tokio::task::spawn_local(async move {
                    if let Some(devices) = devices {
                        let _ = devices.seen(&device_id, now).await;
                    }
                });
            }
            host => self.road.borrow_mut().me = Some(host),
        }
        let mut reply = object(json!({ "id": id, "ok": true }));
        reply.extend(released_fields(&self.served.released));
        self.send(reply);
    }

    /// Reads one frame off the socket under its seal, then as JSON (D7). Nothing when either fails.
    fn read(&self, raw: &Incoming) -> Option<(Value, bool)> {
        let frame = match raw {
            Incoming::Text(text) => Frame::Text(text.as_str()),
            Incoming::Binary(bytes) => Frame::Binary(bytes.as_slice()),
        };
        let text = open_frame(self.seal.borrow_mut().as_mut(), frame).ok()?;
        let value = parse_json(&text).ok()?;
        let half = |id: Option<&Value>| id.and_then(Value::as_f64).is_some_and(|number| number.abs() == 0.5);
        let infinite_id = half(value.get("id")) && parse_json_lax(&text).is_some_and(|lax| lax.get("id") == Some(&Value::Null));
        Some((value, infinite_id))
    }

    fn close_unauthed(&self) {
        if !self.authed() {
            self.sink.close(UNAUTHORIZED_CLOSE, UNAUTHORIZED);
        }
    }

    /// `refuse` (serve.ts:678-681): one `auth` refusal, then the close.
    fn refuse(&self, frame: &Value, id: &Value, error: &str) {
        self.refuse_signed(frame, id, error, Payload::new());
    }

    fn refuse_signed(&self, frame: &Value, id: &Value, error: &str, signed: Payload) {
        let mut payload = failure(id.clone(), error);
        payload.insert("kind".into(), Value::from("auth"));
        payload.extend(signed);
        self.answer(Some(frame), payload, None);
        self.sink.close(UNAUTHORIZED_CLOSE, UNAUTHORIZED);
    }

    /// `own_road(upgrade)`: the host's reading of the road the upgrade came by (`ownRoad`, serve.ts:153-163).
    fn dialled_here(&self) -> bool {
        self.served.opts.own_road.as_ref().is_none_or(|rule| rule(&self.upgrade))
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
    let mut seen: HashSet<String> = HashSet::new();
    let secrets: Vec<String> =
        frame.map(request_secrets).unwrap_or_default().into_iter().filter(|secret| seen.insert(secret.clone())).collect();
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

/// `{id, ok: false, error}`: the one reply a refusal of the door is, under the id the frame asked with.
fn failure(id: Value, error: &str) -> Payload {
    let mut payload = Payload::new();
    payload.insert("id".into(), id);
    payload.insert("ok".into(), Value::Bool(false));
    payload.insert("error".into(), Value::from(error));
    payload
}

/// The keys of a value that serializes as a JSON object, in the order its fields are written.
fn object(value: impl Serialize) -> Payload {
    match serde_json::to_value(value) {
        Ok(Value::Object(map)) => map,
        _ => Payload::new(),
    }
}

/// `...released`: `version`, then `road`, each only where given (serve.ts:825-847).
fn released_fields(released: &Released) -> Payload {
    let mut fields = Payload::new();
    if let Some(version) = &released.version {
        fields.insert("version".into(), Value::from(version.as_str()));
    }
    if let Some(road) = &released.road {
        fields.insert("road".into(), Value::from(road.as_str()));
    }
    fields
}

/// One step of a schema error's path, as the issues line writes it.
fn path_part(part: &Value) -> PathPart {
    match part {
        Value::String(key) => PathPart::Key(key.clone()),
        other => PathPart::Index(other.as_f64().unwrap_or_default()),
    }
}
