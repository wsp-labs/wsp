// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/pane-ports.ts` at 65f6b89c7: the ports a Browser pane opens on this computer for a
//! folder on a computer the person joined. Spec: `spec/places_pane_ports.md`.

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Clock;
use wsp_wire::wire::limits::PortForward;
use wsp_words::exit::HostError;

use super::helpers::Forward;

/// How long a pane's forward stands after the pane last asked for it (pane-ports.ts:14).
pub const PANE_HOLD_MS: u64 = 120_000;
/// How long a pane's forward stands with nothing connecting through it (pane-ports.ts:16).
pub const PANE_QUIET_MS: u64 = 3_600_000;
/// How long a forward with a connection open waits before it is read again (pane-ports.ts:18).
pub const PANE_BUSY_RECHECK_MS: u64 = 15_000;

/// A forward opening or closing, on the stream the Ports list reads (`ForwardEvent`).
#[derive(Clone, Debug, PartialEq)]
pub enum ForwardEvent {
    Open { forward: PortForward },
    Close { workspace_id: String, port: u16 },
}

/// The pane forwards as the app lists and stops them, beside the relay's (pane-ports.ts:21).
pub trait PaneForwards {
    fn list(&self) -> Vec<PortForward>;
    /// Stops the forward a workspace holds at that local port; false where none.
    fn stop(&self, workspace_id: &str, port: u16) -> bool;
    /// Every open and close, in order, until the answer runs.
    fn on(&self, f: Rc<dyn Fn(&ForwardEvent)>) -> Box<dyn FnOnce()>;
}

/// Who asks for a pane's port: the workspace it counts against and the name the cap's refusal says.
#[derive(Clone, Debug, PartialEq)]
pub struct PaneAsker {
    pub workspace_id: String,
    pub name: String,
}

/// A connection accepted for a pane, with the `used` call that moves the quiet hour.
pub type PaneOnConn = Rc<dyn Fn(tokio::net::TcpStream, Rc<dyn Fn()>)>;

/// What the pane ports read: the context's forwards, and its clock.
pub struct PanePortsDeps {
    pub forwards: Rc<RefCell<IndexMap<String, Forward>>>,
    pub clock: Rc<dyn Clock>,
}

/// The pane ports (pane-ports.ts:43). State is memory only.
pub struct PanePorts {
    deps: PanePortsDeps,
    state: RefCell<PaneState>,
}

#[derive(Default)]
struct PaneState {
    _private: (),
}

impl PanePorts {
    pub fn new(deps: PanePortsDeps) -> Rc<PanePorts> {
        let _ = deps;
        todo!()
    }

    /// The port on this computer a pane reaches that computer's `port` by, opened or held another hold
    /// (pane-ports.ts:78). `port` is a u32 so 65,536 and up reach the floor's refusal.
    pub async fn reach(self: &Rc<Self>, key: &str, port: u32, asker: PaneAsker, on_conn: PaneOnConn) -> Result<u16, HostError> {
        let _ = (key, port, asker, on_conn, &self.deps, &self.state);
        todo!()
    }

    /// The local port of the forward standing for that key, opened by no one (pane-ports.ts:116).
    pub fn standing(&self, key: &str, port: u32) -> Result<u16, HostError> {
        let _ = (key, port);
        todo!()
    }
}

impl PaneForwards for PanePorts {
    fn list(&self) -> Vec<PortForward> {
        todo!()
    }

    fn stop(&self, workspace_id: &str, port: u16) -> bool {
        let _ = (workspace_id, port);
        todo!()
    }

    fn on(&self, f: Rc<dyn Fn(&ForwardEvent)>) -> Box<dyn FnOnce()> {
        let _ = f;
        todo!()
    }
}
