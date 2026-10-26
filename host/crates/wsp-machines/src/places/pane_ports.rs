// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/pane-ports.ts` at 65f6b89c7: the ports a Browser pane opens on this computer for a
//! folder on a computer the person joined. Spec: `spec/places_pane_ports.md`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::clock::Cancel;
use wsp_core::types::pending::Clock;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::wire::limits::{PortForward, PortForwardKind, RelayPort, FORWARD_MAX_PER_TARGET};
use wsp_wire::wire::place_link::{
    pane_forward_cap_line, pane_forward_floor_line, pane_forward_gone_line,
    pane_forward_quiet_line, pane_forward_stopped_line,
};
use wsp_words::exit::{HostError, Kind};
use wsp_words::js_date::iso_string;

use super::helpers::{listen_near, Forward, ForwardConn, LoopbackListener, OnConn};

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

type Listener = Rc<dyn Fn(&ForwardEvent)>;

/// The line a pane's next ask is told when the quiet hour or the person ended its forward (pane-ports.ts:33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ended {
    Quiet,
    Stopped,
}

/// One forward a pane holds (pane-ports.ts:24).
struct Held {
    view: PortForward,
    local_port: u16,
    conns: Rc<RefCell<IndexMap<String, Rc<dyn ForwardConn>>>>,
    stop: Rc<dyn Fn()>,
    held_at: Cell<i64>,
    used_at: Rc<Cell<i64>>,
    due: RefCell<Cancel>,
}

#[derive(Default)]
struct PaneState {
    held: IndexMap<String, Rc<Held>>,
    ended: IndexMap<String, Ended>,
    listeners: Rc<RefCell<Vec<Option<Listener>>>>,
}

const HOLD_MS: i64 = PANE_HOLD_MS as i64;
const QUIET_MS: i64 = PANE_QUIET_MS as i64;
const BUSY_RECHECK_MS: i64 = PANE_BUSY_RECHECK_MS as i64;

fn usage(message: String) -> HostError {
    HostError::with_kind(message, Kind::Usage)
}

fn no_cancel() -> Cancel {
    Rc::new(|| {})
}

fn cancel_due(h: &Held) {
    let cancel = h.due.replace(no_cancel());
    cancel();
}

impl PanePorts {
    pub fn new(deps: PanePortsDeps) -> Rc<PanePorts> {
        Rc::new(PanePorts {
            deps,
            state: RefCell::new(PaneState::default()),
        })
    }

    /// The port on this computer a pane reaches that computer's `port` by, opened or held another hold
    /// (pane-ports.ts:78). `port` is a u32 so 65,536 and up reach the floor's refusal.
    pub async fn reach(
        self: &Rc<Self>,
        key: &str,
        port: u32,
        asker: PaneAsker,
        on_conn: PaneOnConn,
    ) -> Result<u16, HostError> {
        let now = self.deps.clock.now();
        let standing = self.state.borrow().held.get(key).cloned();
        if let Some(h) = standing {
            h.held_at.set(now);
            self.arm(key, &h);
            return Ok(h.local_port);
        }
        let told = self.state.borrow_mut().ended.shift_remove(key);
        if let Some(told) = told {
            let line = match told {
                Ended::Quiet => pane_forward_quiet_line(i64::from(port)),
                Ended::Stopped => pane_forward_stopped_line(i64::from(port)),
            };
            return Err(usage(line));
        }
        let asked = match u16::try_from(port) {
            Ok(asked) if asked >= 1024 => asked,
            _ => return Err(usage(pane_forward_floor_line(i64::from(port)))),
        };
        let open = self
            .state
            .borrow()
            .held
            .values()
            .filter(|h| h.view.workspace_id == asker.workspace_id)
            .count();
        if open as i64 >= FORWARD_MAX_PER_TARGET {
            return Err(usage(pane_forward_cap_line(
                &asker.name,
                FORWARD_MAX_PER_TARGET,
            )));
        }

        let used_at = Rc::new(Cell::new(now));
        let used: Rc<dyn Fn()> = {
            let (used_at, clock) = (Rc::clone(&used_at), Rc::clone(&self.deps.clock));
            Rc::new(move || used_at.set(clock.now()))
        };
        let accept: OnConn = Rc::new(move |stream| on_conn(stream, Rc::clone(&used)));
        let (servers, local_port) = listen_near(asked, accept).await?;

        let conns: Rc<RefCell<IndexMap<String, Rc<dyn ForwardConn>>>> =
            Rc::new(RefCell::new(IndexMap::new()));
        let stop: Rc<dyn Fn()> = {
            let (conns, servers) = (Rc::clone(&conns), servers.clone());
            Rc::new(move || {
                let open: Vec<Rc<dyn ForwardConn>> = conns.borrow().values().cloned().collect();
                for conn in open {
                    conn.destroy();
                }
                for server in &servers {
                    server.close();
                }
            })
        };
        let server: Rc<LoopbackListener> = Rc::clone(
            servers
                .first()
                .expect("listen_near answers at least one listener"),
        );
        let view = PortForward {
            workspace_id: asker.workspace_id.clone(),
            port: RelayPort(JsInt::from(i64::from(local_port))),
            started_at: iso_string(now as f64).unwrap_or_default(),
            name: asker.name.clone(),
            kind: PortForwardKind::Url,
        };
        let h = Rc::new(Held {
            view: view.clone(),
            local_port,
            conns: Rc::clone(&conns),
            stop: Rc::clone(&stop),
            held_at: Cell::new(now),
            used_at,
            due: RefCell::new(no_cancel()),
        });
        let replaced = self
            .state
            .borrow_mut()
            .held
            .insert(key.to_string(), Rc::clone(&h));
        drop(replaced);
        self.arm(key, &h);
        let replaced = self.deps.forwards.borrow_mut().insert(
            key.to_string(),
            Forward {
                server,
                local_port,
                conns,
                stop: Some(stop),
            },
        );
        drop(replaced);
        self.emit(&ForwardEvent::Open { forward: view });
        Ok(local_port)
    }

    /// The local port of the forward standing for that key, opened by no one (pane-ports.ts:116).
    pub fn standing(&self, key: &str, port: u32) -> Result<u16, HostError> {
        let held = self.state.borrow().held.get(key).cloned();
        held.map(|h| h.local_port)
            .ok_or_else(|| usage(pane_forward_gone_line(i64::from(port))))
    }

    /// Reads the forward when its hold or quiet hour runs out (pane-ports.ts:64).
    fn arm(self: &Rc<Self>, key: &str, h: &Rc<Held>) {
        cancel_due(h);
        let now = self.deps.clock.now();
        let busy = !h.conns.borrow().is_empty();
        let at = if busy {
            now + BUSY_RECHECK_MS
        } else {
            (h.held_at.get() + HOLD_MS).min(h.used_at.get() + QUIET_MS)
        };
        let (weak, key, mine) = (Rc::downgrade(self), key.to_string(), Rc::clone(h));
        let cancel = self.deps.clock.schedule(
            Box::new(move || {
                let Some(this) = weak.upgrade() else { return };
                let current = this
                    .state
                    .borrow()
                    .held
                    .get(&key)
                    .is_some_and(|cur| Rc::ptr_eq(cur, &mine));
                if !current {
                    return;
                }
                let later = this.deps.clock.now();
                let busy = !mine.conns.borrow().is_empty();
                if busy
                    || (later - mine.held_at.get() < HOLD_MS
                        && later - mine.used_at.get() < QUIET_MS)
                {
                    this.arm(&key, &mine);
                    return;
                }
                this.close(&key);
                if later - mine.used_at.get() >= QUIET_MS && later - mine.held_at.get() < HOLD_MS {
                    this.state.borrow_mut().ended.insert(key, Ended::Quiet);
                }
            }),
            (at - now).max(0),
        );
        *h.due.borrow_mut() = cancel;
    }

    /// Takes the forward under `key` down and emits its close (pane-ports.ts:53).
    fn close(&self, key: &str) {
        let removed = self.state.borrow_mut().held.shift_remove(key);
        let Some(h) = removed else { return };
        cancel_due(&h);
        let replaced = {
            let mut forwards = self.deps.forwards.borrow_mut();
            if forwards
                .get(key)
                .is_some_and(|f| Rc::ptr_eq(&f.conns, &h.conns))
            {
                forwards.shift_remove(key)
            } else {
                None
            }
        };
        drop(replaced);
        (h.stop)();
        self.emit(&ForwardEvent::Close {
            workspace_id: h.view.workspace_id.clone(),
            port: h.local_port,
        });
    }

    fn emit(&self, e: &ForwardEvent) {
        let listeners = Rc::clone(&self.state.borrow().listeners);
        let mut at = 0;
        loop {
            let listener = match listeners.borrow().get(at) {
                None => break,
                Some(slot) => slot.clone(),
            };
            if let Some(listener) = listener {
                listener(e);
            }
            at += 1;
        }
    }
}

impl PaneForwards for PanePorts {
    fn list(&self) -> Vec<PortForward> {
        self.state
            .borrow()
            .held
            .values()
            .map(|h| h.view.clone())
            .collect()
    }

    fn stop(&self, workspace_id: &str, port: u16) -> bool {
        let found = self
            .state
            .borrow()
            .held
            .iter()
            .find(|(_, h)| {
                h.view.workspace_id == workspace_id && h.view.port.get() == i64::from(port)
            })
            .map(|(key, h)| (key.clone(), Rc::clone(h)));
        let Some((key, h)) = found else { return false };
        self.close(&key);
        if self.deps.clock.now() - h.held_at.get() < HOLD_MS {
            self.state.borrow_mut().ended.insert(key, Ended::Stopped);
        }
        true
    }

    fn on(&self, f: Rc<dyn Fn(&ForwardEvent)>) -> Box<dyn FnOnce()> {
        let listeners = Rc::clone(&self.state.borrow().listeners);
        let at = {
            let mut all = listeners.borrow_mut();
            all.push(Some(f));
            all.len() - 1
        };
        Box::new(move || {
            let removed = listeners.borrow_mut().get_mut(at).and_then(Option::take);
            drop(removed);
        })
    }
}
