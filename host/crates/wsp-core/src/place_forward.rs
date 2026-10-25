// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/place-forward.ts: the road from this computer to the daemon on a computer joined as a
//! place. Interface only; the implementer opens the loopback listener and the tunnels.

use wsp_wire::wire::daemon_events::DaemonEvent;

/// One port on this computer's loopback that carries to the daemon on a joined computer (place-forward.ts:14).
#[derive(Debug)]
pub struct PlaceForward {
    /// The port on this computer's loopback that carries to the daemon on that one.
    pub port: u16,
}

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
