// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/place-forward.ts at 65f6b89c7: the road from this computer to the daemon on a
//! computer joined as a place, a loopback port whose every connection is a tunnel on the place's link. Spec:
//! spec/tunnels.md.
use std::rc::Rc;

use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

use crate::reach::DaemonReach;

/// What an open is refused with when the listener came up on no port. place-forward.ts:39.
pub const NO_FREE_PORT: &str = "this computer offered no free port for the road to a place";

/// One port on this computer's loopback that carries to the daemon on a joined computer (place-forward.ts:14).
#[derive(Debug)]
pub struct PlaceForward {
    /// The port on this computer's loopback that carries to the daemon on that one.
    pub port: u16,
    held: ForwardHeld,
}

/// The listener and the connections riding it; the implementer's to fill.
#[derive(Debug, Default)]
struct ForwardHeld {}

impl PlaceForward {
    /// A frame the place pushed, handed to the connection it belongs to; anything else is nobody's here.
    pub fn event(&self, e: &DaemonEvent) {
        todo!()
    }

    /// Closes the listener and destroys every connection riding it; a second call does nothing.
    pub fn close(&self) {
        todo!()
    }
}

/// Opens one such port on LOOPBACK, port 0, for one place: each accepted connection is plumbed as tunnel `pl<n>` (n
/// from 1, per forward) to `daemon_port` over `reach.request`. A listener that cannot bind fails with the OS's words.
/// place-forward.ts:25.
pub async fn open_place_forward(reach: Rc<dyn DaemonReach>, daemon_port: u16) -> Result<PlaceForward, HostError> {
    todo!()
}
