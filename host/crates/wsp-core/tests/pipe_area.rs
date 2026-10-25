// SPDX-License-Identifier: AGPL-3.0-only
//! pipe_area: the pool as the runtime runs it, from "the pipe pool in the runtime" in
//! packages/runtime/test/pipe.test.ts. Spec: spec/pipe_area.md.
mod pipe_daemon;

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_core::clock::{Clock, FakeClock};
use wsp_core::pipe::pool::{PipeDoor, PipeOpen};
use wsp_core::pipe::{pipe_area, PipeArea, PipeAreaContext, PipeBroker, PipeDuplex, PipeHandler, PipeLive, PIPE_BEHIND_AGAIN_MS};
use wsp_wire::wire::daemon_version::PIPE_DAEMON_VERSION;
use wsp_wire::wire::events_union::EventUnion;

use pipe_daemon::{sleep, until, Daemon, Mode, TOKEN};

struct Entry {
    id: String,
    phase: String,
    place: &'static str,
    machine_id: String,
}

struct Broker {
    handler: Option<PipeHandler>,
}

impl PipeBroker for Broker {
    fn ready(&self) -> bool {
        true
    }
    fn pipe(&self) -> Option<PipeHandler> {
        self.handler.clone()
    }
    fn revoke(&self, _workspace_id: &str) -> Option<LocalBoxFuture<'static, Result<(), String>>> {
        None
    }
}

/// The parts of the runtime the pool reads: its live records, its bus, a daemon token and the broker the host wired.
/// A record on `boat` is brokered and one on `hetzner` is not.
struct Rt {
    daemon: Daemon,
    broker: Option<Rc<dyn PipeBroker>>,
    live: RefCell<Vec<Entry>>,
    listeners: Rc<RefCell<Vec<(u64, Rc<dyn Fn(&EventUnion)>)>>>,
    next_listener: Cell<u64>,
    // The machines whose token entry is held: dropping one makes the next token read write the host's token again.
    daemon_tokens: RefCell<HashSet<String>>,
    writes: RefCell<Vec<i64>>,
    clock: Rc<dyn Clock>,
}

impl PipeAreaContext for Rt {
    fn broker(&self) -> Option<Rc<dyn PipeBroker>> {
        self.broker.clone()
    }

    fn live(&self, workspace_id: &str) -> Option<PipeLive> {
        self.live.borrow().iter().find(|e| e.id == workspace_id).map(|e| PipeLive {
            id: e.id.clone(),
            phase: e.phase.clone(),
            machine_id: e.machine_id.clone(),
        })
    }

    fn live_ids(&self) -> Vec<String> {
        self.live.borrow().iter().map(|e| e.id.clone()).collect()
    }

    fn daemon_reach_url(&self, _workspace_id: &str) -> LocalBoxFuture<'static, Result<String, String>> {
        let url = self.daemon.url();
        async move { Ok(url) }.boxed_local()
    }

    // The host's own token, written on the machine the first time it is asked for after the entry is dropped, as
    // daemonTokenOf does.
    fn daemon_token_of(&self, workspace_id: &str) -> LocalBoxFuture<'static, Result<Option<String>, String>> {
        let machine = self.live.borrow().iter().find(|e| e.id == workspace_id).map(|e| e.machine_id.clone());
        if let Some(machine) = machine {
            if self.daemon_tokens.borrow_mut().insert(machine) {
                self.daemon.write(TOKEN);
                self.writes.borrow_mut().push(self.clock.now());
            }
        }
        async { Ok(Some(TOKEN.to_string())) }.boxed_local()
    }

    fn daemon_version(&self, _workspace_id: &str) -> LocalBoxFuture<'static, Result<Option<i64>, String>> {
        async { Ok(Some(PIPE_DAEMON_VERSION)) }.boxed_local()
    }

    fn forget_daemon_token(&self, machine_id: &str) {
        self.daemon_tokens.borrow_mut().remove(machine_id);
    }

    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }

    fn on_any(&self, listener: Rc<dyn Fn(&EventUnion)>) -> Box<dyn FnOnce()> {
        let id = self.next_listener.get();
        self.next_listener.set(id + 1);
        self.listeners.borrow_mut().push((id, listener));
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut().retain(|(i, _)| *i != id))
    }

    fn ready(&self) -> LocalBoxFuture<'static, Result<(), String>> {
        async { Ok(()) }.boxed_local()
    }
}

impl Rt {
    fn put(&self, id: &str, place: &'static str, phase: &str) {
        let entry = Entry { id: id.to_string(), phase: phase.to_string(), place, machine_id: format!("m-{id}") };
        let mut live = self.live.borrow_mut();
        match live.iter_mut().find(|e| e.id == id) {
            Some(slot) => *slot = entry,
            None => live.push(entry),
        }
    }

    fn set_phase(&self, id: &str, phase: &str) {
        let mut live = self.live.borrow_mut();
        live.iter_mut().find(|e| e.id == id).expect("a live entry").phase = phase.to_string();
    }

    fn emit(&self, e: Value) {
        let event: EventUnion = serde_json::from_value(e).expect("a bus event");
        let listeners: Vec<Rc<dyn Fn(&EventUnion)>> = self.listeners.borrow().iter().map(|(_, l)| l.clone()).collect();
        for listener in listeners {
            listener(&event);
        }
    }

    fn writes(&self) -> usize {
        self.writes.borrow().len()
    }
}

fn rt_on(d: &Daemon, broker: Option<PipeHandler>, wired: bool, clock: Rc<dyn Clock>) -> Rc<Rt> {
    Rc::new(Rt {
        daemon: d.clone(),
        broker: if wired { Some(Rc::new(Broker { handler: broker }) as Rc<dyn PipeBroker>) } else { None },
        live: RefCell::new(Vec::new()),
        listeners: Rc::new(RefCell::new(Vec::new())),
        next_listener: Cell::new(0),
        daemon_tokens: RefCell::new(HashSet::new()),
        writes: RefCell::new(Vec::new()),
        clock,
    })
}

/// The area over `rt`, as the host wires it: a record on `boat` is brokered.
fn area_of(rt: &Rc<Rt>) -> Option<Rc<PipeArea>> {
    let live = rt.clone();
    let brokered: Rc<dyn Fn(&str) -> bool> = Rc::new(move |id: &str| live.live.borrow().iter().any(|e| e.id == id && e.place == "boat"));
    pipe_area(rt.clone(), brokered)
}

/// A pipe handler that keeps every duplex it is given alive, and notes `<workspace> <door>` when asked to.
fn handler_noting(handed: Option<Rc<RefCell<Vec<String>>>>, keep: Rc<RefCell<Vec<PipeDuplex>>>) -> PipeHandler {
    Rc::new(move |id: &str, open: PipeOpen, duplex: PipeDuplex| {
        keep.borrow_mut().push(duplex);
        if let Some(handed) = &handed {
            let door = match open.door {
                PipeDoor::Http => "http",
                PipeDoor::Gh => "gh",
            };
            handed.borrow_mut().push(format!("{id} {door}"));
        }
        Ok(())
    })
}

fn creating(workspace_id: &str, stage: &str) -> Value {
    json!({ "type": "workspace.creating", "workspaceId": workspace_id, "name": "a", "stage": stage, "message": stage, "elapsedMs": 0 })
}

fn created(workspace_id: &str, phase: &str) -> Value {
    json!({
        "type": "workspace.created",
        "workspace": {
            "id": workspace_id,
            "name": "a",
            "machineId": format!("m-{workspace_id}"),
            "phase": phase,
            "golden": "golden",
            "createdAt": "2026-10-25T00:00:00.000Z",
            "project": { "id": "p", "name": "p", "path": "/p", "computer": "c" },
        },
    })
}

mod the_pipe_pool_in_the_runtime {
    use super::*;

    const REFUSED_FIRST: &str =
        "ws-a: its daemon refused the token a pipe was dialled with, so the host writes its own there again and dials in 60 s";
    const REFUSED_AGAIN: &str = "ws-a: its daemon refused the host's token again right after the host wrote it, so something else writes that file too; the host writes its own there again and dials in 2 min, waiting twice as long each time up to 30 min";

    #[tokio::test(flavor = "current_thread")]
    async fn dials_nothing_on_a_host_that_wired_no_broker() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let rt = rt_on(&d, None, false, wsp_core::clock::real_clock());
                rt.put("ws-a", "boat", "running");
                assert!(area_of(&rt).is_none());
                sleep(150).await;
                assert_eq!(d.first_frame_count(), 0);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn keeps_4_pipes_to_each_running_brokered_workspace_none_to_a_joined_computer_s_and() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let handed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
                let keep = Rc::new(RefCell::new(Vec::new()));
                let rt = rt_on(&d, Some(handler_noting(Some(handed.clone()), keep)), true, wsp_core::clock::real_clock());
                rt.put("ws-a", "boat", "running");
                rt.put("ws-b", "hetzner", "running");
                rt.put("ws-c", "boat", "napping");
                let area = area_of(&rt).expect("an area where a broker is wired");
                until(|| d.parked_len() == 4).await;
                sleep(150).await;
                assert_eq!(d.parked_len(), 4);
                d.take("gh", 0);
                until(|| handed.borrow().len() == 1 && d.parked_len() == 4).await;
                assert_eq!(*handed.borrow(), vec!["ws-a gh".to_string()]);
                area.close();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closes_a_workspace_s_pipes_as_it_naps_dials_4_again_as_it_wakes_and_none_after_i() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let keep = Rc::new(RefCell::new(Vec::new()));
                let rt = rt_on(&d, Some(handler_noting(None, keep)), true, wsp_core::clock::real_clock());
                let area = area_of(&rt).expect("an area where a broker is wired");
                rt.put("ws-a", "boat", "running");
                rt.emit(created("ws-a", "running"));
                until(|| d.parked_len() == 4).await;
                rt.set_phase("ws-a", "napping");
                rt.emit(json!({ "type": "workspace.napped", "workspaceId": "ws-a" }));
                until(|| d.parked_len() == 0).await;
                rt.set_phase("ws-a", "running");
                rt.emit(json!({ "type": "workspace.woken", "workspaceId": "ws-a", "machineId": "m-ws-a" }));
                until(|| d.parked_len() == 4).await;
                let dialled = d.first_frame_count();
                rt.emit(json!({ "type": "workspace.deleted", "workspaceId": "ws-a" }));
                until(|| d.parked_len() == 0).await;
                sleep(150).await;
                assert_eq!(d.first_frame_count(), dialled);
                area.close();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn writes_the_host_s_token_again_over_another_writer_s_and_has_4_pipes_parked_again() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let t = FakeClock::at_now();
                let warn = wsp_core::console::capture();
                let keep = Rc::new(RefCell::new(Vec::new()));
                let rt = rt_on(&d, Some(handler_noting(None, keep)), true, t.clock());
                rt.put("ws-a", "boat", "running");
                let area = area_of(&rt).expect("an area where a broker is wired");
                until(|| d.parked_len() == 4).await;
                assert_eq!(rt.writes(), 1);
                d.write("another-hosts-token");
                until(|| d.parked_len() == 0).await;
                sleep(100).await;
                assert_eq!(d.file(), "another-hosts-token");
                t.advance(PIPE_BEHIND_AGAIN_MS as i64);
                until(|| d.parked_len() == 4).await;
                assert_eq!(d.file(), TOKEN);
                assert_eq!(rt.writes(), 2);
                assert!(warn.lines().contains(&REFUSED_FIRST.to_string()), "the lines were {:?}", warn.lines());
                area.close();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn says_once_that_another_writer_keeps_taking_the_file_back_and_waits_twice_as_long() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let t = FakeClock::at_now();
                let warn = wsp_core::console::capture();
                let keep = Rc::new(RefCell::new(Vec::new()));
                let rt = rt_on(&d, Some(handler_noting(None, keep)), true, t.clock());
                rt.put("ws-a", "boat", "running");
                let area = area_of(&rt).expect("an area where a broker is wired");
                until(|| d.parked_len() == 4).await;
                // The other writer takes the file back each time the host's pipes are parked on it again.
                let mut waits: Vec<i64> = Vec::new();
                for _round in 0..4 {
                    d.write("another-hosts-token");
                    until(|| d.parked_len() == 0).await;
                    sleep(50).await;
                    let before = rt.writes();
                    let mut waited = 0i64;
                    while rt.writes() == before {
                        assert!(waited <= 16 * 60_000, "the host's token was not written again after {waited} ms");
                        t.advance(15_000);
                        waited += 15_000;
                        sleep(20).await;
                    }
                    waits.push(waited);
                    until(|| d.parked_len() == 4).await;
                }
                let behind = PIPE_BEHIND_AGAIN_MS as i64;
                assert_eq!(waits, vec![behind, 2 * behind, 4 * behind, 8 * behind]);
                let said: Vec<String> = warn.lines().into_iter().filter(|l| l.contains("refused")).collect();
                assert_eq!(said, vec![REFUSED_FIRST.to_string(), REFUSED_AGAIN.to_string()]);
                area.close();
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dials_a_brokered_workspace_s_pipes_while_its_create_still_runs_so_its_clone_reac() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let keep = Rc::new(RefCell::new(Vec::new()));
                let rt = rt_on(&d, Some(handler_noting(None, keep)), true, wsp_core::clock::real_clock());
                let area = area_of(&rt).expect("an area where a broker is wired");
                sleep(20).await;
                // A stage before the machine is bound has no record to dial.
                rt.emit(creating("ws-a", "fork-requested"));
                sleep(150).await;
                assert_eq!(d.first_frame_count(), 0);
                rt.put("ws-a", "boat", "running");
                rt.emit(creating("ws-a", "project-cloned"));
                until(|| d.parked_len() == 4).await;
                rt.emit(creating("ws-a", "failed"));
                until(|| d.parked_len() == 0).await;
                let dialled = d.first_frame_count();
                sleep(150).await;
                assert_eq!(d.first_frame_count(), dialled);
                area.close();
            })
            .await;
    }
}
