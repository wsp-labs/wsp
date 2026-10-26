// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/place-forward.ts at 65f6b89c7: the road from this computer to the daemon on a
//! computer joined as a place, a loopback port whose every connection is a tunnel on the place's link. Spec:
//! spec/tunnels.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::FutureExt;
use indexmap::IndexMap;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use wsp_wire::types::app_ports::LOOPBACK;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

use crate::reach::{DaemonReach, Frame};
use crate::tunnel::{plumb_tunnel, tunnel_frame, TunnelConn, TunnelConns, TunnelLink, TunnelOptions};

/// What an open is refused with when the listener came up on no port. place-forward.ts:39.
pub const NO_FREE_PORT: &str = "this computer offered no free port for the road to a place";

/// One port on this computer's loopback that carries to the daemon on a joined computer (place-forward.ts:14).
#[derive(Debug)]
pub struct PlaceForward {
    /// The port on this computer's loopback that carries to the daemon on that one.
    pub port: u16,
    held: ForwardHeld,
}

/// The listener's stop and the connections riding it.
#[derive(Debug, Default)]
struct ForwardHeld {
    conns: TunnelConns,
    stop: RefCell<Option<oneshot::Sender<()>>>,
    closed: Cell<bool>,
}

impl PlaceForward {
    /// A frame the place pushed, handed to the connection it belongs to; anything else is nobody's here.
    pub fn event(&self, e: &DaemonEvent) {
        tunnel_frame(&self.held.conns, e, None);
    }

    /// Closes the listener and destroys every connection riding it; a second call does nothing.
    pub fn close(&self) {
        if self.held.closed.replace(true) {
            return;
        }
        if let Some(stop) = self.held.stop.borrow_mut().take() {
            let _ = stop.send(());
        }
        let riding: Vec<TunnelConn> = self.held.conns.borrow().values().cloned().collect();
        self.held.conns.borrow_mut().clear();
        for conn in riding {
            conn.destroy();
        }
    }
}

/// Opens one such port on LOOPBACK, port 0, for one place: each accepted connection is plumbed as tunnel `pl<n>` (n
/// from 1, per forward) to `daemon_port` over `reach.request`. A listener that cannot bind fails with the OS's words.
/// place-forward.ts:25.
pub async fn open_place_forward(reach: Rc<dyn DaemonReach>, daemon_port: u16) -> Result<PlaceForward, HostError> {
    let listener = TcpListener::bind((LOOPBACK, 0)).await.map_err(|e| HostError::new(e.to_string()))?;
    let port = listener.local_addr().map_err(|e| HostError::new(e.to_string()))?.port();
    if port == 0 {
        return Err(HostError::new(NO_FREE_PORT));
    }
    let conns: TunnelConns = Rc::new(RefCell::new(IndexMap::new()));
    let (stop, stopped) = oneshot::channel();
    tokio::task::spawn_local(accept_all(listener, reach, daemon_port, conns.clone(), stopped));
    Ok(PlaceForward { port, held: ForwardHeld { conns, stop: RefCell::new(Some(stop)), closed: Cell::new(false) } })
}

async fn accept_all(listener: TcpListener, reach: Rc<dyn DaemonReach>, daemon_port: u16, conns: TunnelConns, mut stopped: oneshot::Receiver<()>) {
    let mut seq = 0u64;
    loop {
        tokio::select! {
            _ = &mut stopped => return,
            accepted = listener.accept() => {
                let Ok((socket, _)) = accepted else { continue };
                seq += 1;
                let opts = TunnelOptions { port: daemon_port, conns: conns.clone(), tunnel_id: format!("pl{seq}"), touched: None, refused: None, opened: None };
                plumb_tunnel(reach_link(reach.clone()), socket, opts);
            }
        }
    }
}

fn reach_link(reach: Rc<dyn DaemonReach>) -> TunnelLink {
    Rc::new(move |op: &str, params: Frame| {
        let reach = reach.clone();
        let op = op.to_string();
        async move { reach.request(&op, params).await.map_err(|e| HostError::new(e.message)) }.boxed_local()
    })
}
