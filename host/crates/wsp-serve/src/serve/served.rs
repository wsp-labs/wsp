// SPDX-License-Identifier: AGPL-3.0-only
//! The runtime as its socket serves it (spec/README.md, "The seams"): wsp-core's `Runtime`, with the doors wsp-core
//! still holds as empty handle traits (ruling 6400658264, 2) read typed, and the per-socket roads of wsp-core's
//! `sys-socket.ts`, `sign-in-socket.ts` and `forwards.ts`, whose ports have a writer of their own (ruling 6402177374,
//! 8). The join folds this trait into `Runtime` once those land; nothing a socket sees changes.
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_core::types::api::Runtime;
use wsp_machines::places::types::PlaceDoor;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::limits::PortForward;
use wsp_wire::wire::runtime_ops::RuntimeRequest;
use wsp_words::exit::HostError;

use crate::serve_broker::CloudSignIn;
use crate::serve_processes::ActivityOps;
use crate::serve_slates::SlateDoor;
use crate::serve_start::StartRoads;

/// A frame a per-socket road pushes or answers with: one JSON object, written as given.
pub type Payload = serde_json::Map<String, serde_json::Value>;
/// The socket's send, as a road is handed it: the refusal log rides it (spec R4).
pub type Send = Rc<dyn Fn(Payload)>;

/// One socket as a per-socket road needs it: whether it is still open, and where its leaves go, which its close
/// runs. sys-socket.ts:15, sign-in-socket.ts:28.
#[derive(Clone)]
pub struct SocketHooks {
    pub open: Rc<dyn Fn() -> bool>,
    pub detach: Rc<dyn Fn(Box<dyn FnOnce()>)>,
}

/// A per-socket road's answer to one op: it sends its own reply (and its pushes) through `send`.
pub trait SocketOps {
    fn answer(&self, msg: RuntimeRequest, send: Send, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>;
}

/// The forwards a host holds, as the app lists and stops them. forwards.ts:9-14.
pub trait ForwardsSource {
    fn list(&self) -> Vec<PortForward>;
    /// Whether one was there to stop.
    fn stop(&self, workspace_id: &str, port: u16) -> bool;
    /// The source's `forward.*` events, as JSON objects; answers the detach.
    fn on(&self, f: Rc<dyn Fn(&serde_json::Value)>) -> Box<dyn FnOnce()>;
}

/// The runtime a socket serves. Implemented by `AssembledRuntime` (spec/runtime_of.md) and by the tests' fake.
pub trait ServedRuntime: Runtime {
    /// `rt.places`, typed: none on a runtime wired without places.
    fn place_door(&self) -> Option<Rc<dyn PlaceDoor>>;
    /// `rt.slates`, as `answerSlate` calls it.
    fn slate_door(&self) -> Rc<dyn SlateDoor>;
    /// `rt.activity` and the settle and delete of `rt.sessions`, as `answerProcesses` calls them.
    fn activity_door(&self) -> Rc<dyn ActivityOps>;
    /// `sysSocket(rt, socket)`: the readings one socket subscribes to. sys-socket.ts:15.
    fn sys_ops(&self, socket: SocketHooks) -> Rc<dyn SocketOps>;
    /// `signInSocket(rt.agents, socket, cloudSignIn)`. sign-in-socket.ts:26.
    fn sign_in_ops(&self, socket: SocketHooks, cloud: Option<CloudSignIn>) -> Rc<dyn SocketOps>;
    /// `isSignInOp(msg)`: the op is one of the six sign-in ops. sign-in-socket.ts:14.
    fn is_sign_in_op(&self, msg: &RuntimeRequest) -> bool;
    /// `forwardsOf(opts.forwards, rt.places?.paneForwards)`: one source over the host's and the panes'. forwards.ts:17.
    fn forwards(&self, host: Option<Rc<dyn ForwardsSource>>) -> Rc<dyn ForwardsSource>;
    /// `startAt` and `answeredStart` of threads/answered-start.ts (serve_start.rs).
    fn start_roads(&self) -> Rc<dyn StartRoads>;
}
