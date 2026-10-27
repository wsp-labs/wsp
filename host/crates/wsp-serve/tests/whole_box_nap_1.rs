// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/box-nap.test.ts at 65f6b89c7, first part: "the nap of a workspace on a computer that keeps no
//! image" and "the nap and rebuild of a workspace whose pause keeps the disk", case for case (the second part,
//! `whole_box_nap_2.rs`, holds the wake that finds a Boat box already running). The TS mocks `linkBackoffMs` to 0 so a
//! daemon link redials at once; the Rust link has no such seam, so a redial waits its real backoff here.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::box_daemons::{late_daemon, slow_daemon, BoxDaemon};
use common::*;
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_engine::box_backend::plan::box_budgets;
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_serve::AssembledRuntime;
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::views::session_events::DAEMON_UNIT;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
use wsp_words::words::computer::WAKE_STOPPED;

/// A stub whose guest answers what the vault road asks of it: the listing of the home it would archive and the size of the
/// archive it wrote. Every tar and untar is recorded by machine, so a nap that took the vault road is read back here by
/// name rather than guessed at from a blob.
struct Guest {
    backend: Arc<StubBackend>,
    tars: Rc<RefCell<Vec<String>>>,
    untars: Rc<RefCell<Vec<String>>>,
}

fn guest_backend() -> Guest {
    let backend = StubBackend::new(None);
    let (tars, untars): (Rc<RefCell<Vec<String>>>, Rc<RefCell<Vec<String>>>) = (Rc::default(), Rc::default());
    let (tarred, untarred) = (tars.clone(), untars.clone());
    backend.set_exec_impl(exec_fn(move |m, cmd| {
        if cmd.contains(DAEMON_TOKEN_PATH) {
            return Ok(token_guest_answer(cmd));
        }
        if cmd.contains("ls -A /root") {
            return Ok(exec_ok("notes.md\n"));
        }
        if cmd.starts_with("wc -c <") {
            return Ok(exec_ok("1000\n"));
        }
        if cmd.contains("tar czf") {
            tarred.borrow_mut().push(m.id().to_string());
        }
        if cmd.contains("tar xzf") {
            untarred.borrow_mut().push(m.id().to_string());
        }
        Ok(exec_ok(""))
    }));
    backend.set_downloads(Rc::new(|_| b"tarbytes".to_vec()));
    Guest { backend, tars, untars }
}

/// Every command every machine of this backend was asked, oldest first.
fn commands(backend: &Arc<StubBackend>) -> Vec<String> {
    backend.machines().iter().flat_map(|m| m.exec_log()).collect()
}

async fn get(rt: &Rc<AssembledRuntime>, id: &str) -> Value {
    j(&rt.workspaces().get(id, None, None).await.expect("the workspace"))
}

fn names(backend: &Arc<StubBackend>) -> Vec<String> {
    backend.machines().iter().map(|m| m.id().to_string()).collect()
}

/// What such a computer says about itself: a workspace on it is a copy of the computer, and the pause of one is the stop of
/// its machine on that computer's own disk.
fn imageless() -> Guest {
    let made = guest_backend();
    made.backend.set_capabilities(|c| {
        c.images = false;
        c.pause_mode = Some(PauseMode::Disk);
    });
    made
}

// "the nap of a workspace on a computer that keeps no image" > "pauses the machine and reads nothing of the computer's home: no listing, no archive, no stored vault, and the record says nothing about a backup"
#[test]
fn pauses_the_machine_and_reads_nothing_of_the_computers_home_no_listing_no_archive_no_stored_vault_and_the_record_says_nothing_about_a_backup() {
    drive(|| async {
        let g = imageless();
        let state = sqlite_state();
        let fx = runtime(&g.backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::named("x"), None).await.expect("a workspace");
        let m = g.backend.machine(0);
        // What the create left on the machine, so what the nap itself sends is read on its own.
        let sent = (m.exec_log().len(), m.run_log().len());
        let napped = fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(j(&napped)["phase"], "napping");
        assert!(m.paused.get());
        // The pause costs the machine nothing: no command and no script, which is where the tens of seconds went.
        assert_eq!((m.exec_log().len(), m.run_log().len()), sent);
        // The two commands the vault road runs on the machine: the breadcrumb it appends to that computer's home and the
        // listing it enumerates from. Neither is sent.
        assert!(!commands(&g.backend).iter().any(|c| c.contains(".wsp-upgraded")));
        assert!(!commands(&g.backend).iter().any(|c| c.contains("ls -A /root")));
        assert!(g.tars.borrow().is_empty());
        assert!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read").is_none());
        let record = get(&fx.rt, &ws.view.id).await;
        assert!(record.get("vaultedAt").is_none() && record.get("vaultRefused").is_none(), "{record}");
        let woken = fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(g.backend.machine(0).resumes.get(), 1);
        assert!(g.untars.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "drops what a nap before this rule wrote on the record about a backup, so no row reads a refusal about a vault the workspace never had"
#[test]
fn drops_what_a_nap_before_this_rule_wrote_on_the_record_about_a_backup_so_no_row_reads_a_refusal_about_a_vault_the_workspace_never_had() {
    drive(|| async {
        let g = imageless();
        let state = sqlite_state();
        let fx = runtime(&g.backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::named("x"), None).await.expect("a workspace");
        // The record as spoo's own was after one nap over the cap: the stamp and the refusal a vault road wrote.
        let mut held = state.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record");
        held["vaultedAt"] = json!("2026-09-17T00:00:00.000Z");
        held["vaultRefused"] = json!("the export was 580 MB, over the 200 MB cap");
        state.store.put("workspaces", &ws.view.id, held).await.expect("a write");
        fx.rt.close().await.expect("a close");
        let back = runtime(&g.backend).store(&state.store).build();
        let read = get(&back.rt, &ws.view.id).await;
        assert!(read.get("vaultRefused").is_none(), "{read}");
        assert!(read.get("vaultedAt").is_none(), "{read}");
        back.rt.close().await.expect("a close");
    });
}

// "a wake that runs out its attempts fails on the same machine, and nothing is carried anywhere"
#[test]
fn a_wake_that_runs_out_its_attempts_fails_on_the_same_machine_and_nothing_is_carried_anywhere() {
    drive(|| async {
        let g = imageless();
        let port = dropping_port().await;
        let state = sqlite_state();
        let fx = runtime(&g.backend).store(&state.store).build();
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        g.backend.lifecycle().set_wake_attempts(1);
        let ws = create_on(&fx.rt, CreateOn::named("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", port.port));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("daemon on m1 did not answer"), "{}", refused.message);
        assert_eq!(get(&fx.rt, &ws.view.id).await["machineId"], "m1");
        assert_eq!(g.backend.machines().len(), 1);
        assert!(!m1.killed.get());
        assert!(g.untars.borrow().is_empty());
        assert!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read").is_none());
        fx.rt.close().await.expect("a close");
    });
}

/// A napped workspace whose machine is reached the way a Boat box is, by its preview route: the daemon behind it takes the
/// socket at once and answers its auth frame only once `daemon.up()` is called, so the wait is on the link, as it is on a
/// box whose disk is still streaming in. The clock is the runtime's, stepped by `pump`.
struct Boat {
    fx: Fx,
    guest: Guest,
    fc: FakeClock,
    ws_id: String,
    m1: Arc<StubMachine>,
    daemon: Rc<BoxDaemon>,
    state: State,
}

async fn boat_wake(slow: bool) -> Boat {
    let guest = imageless();
    let fc = FakeClock::at_now();
    let state = sqlite_state();
    let fx = runtime(&guest.backend).store(&state.store).clock(&fc).build();
    guest.backend.lifecycle().set_budgets(box_budgets());
    let daemon = Rc::new(if slow { slow_daemon().await } else { late_daemon().await });
    let ws = create_on(&fx.rt, CreateOn::named("x"), None).await.expect("a workspace");
    let m1 = guest.backend.machine(0);
    m1.set_preview_url(&daemon.url());
    fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
    Boat { fx, guest, fc, ws_id: ws.view.id, m1, daemon, state }
}

impl Boat {
    /// Steps the clock half a second at a time, turning the loop between steps so the socket's frames are read, until
    /// `done` holds or twenty minutes have passed.
    async fn pump(&self, done: impl Fn() -> bool, each: impl Fn()) {
        let mut spent = 0;
        while !done() && spent < 20 * 60_000 {
            for _ in 0..5 {
                sleep_ms(1).await;
            }
            self.fc.advance(500);
            each();
            spent += 500;
        }
    }
    fn waking(&self) -> Waking {
        let (rt, id) = (self.fx.rt.clone(), self.ws_id.clone());
        let done = Rc::new(Cell::new(false));
        let ended = done.clone();
        let handle = tokio::task::spawn_local(async move {
            let outcome = rt.workspaces().wake(&id, None).await.map(|v| j(&v)).map_err(|e| e.message);
            ended.set(true);
            outcome
        });
        Waking { handle, done }
    }
    async fn close(self) {
        self.fx.rt.close().await.expect("a close");
        drop(self.daemon);
        drop(self.state);
    }
}

struct Waking {
    handle: JoinHandle<Result<Value, String>>,
    done: Rc<Cell<bool>>,
}

impl Waking {
    fn ended(&self) -> bool {
        self.done.get()
    }
    async fn result(self) -> Result<Value, String> {
        self.handle.await.expect("the wake task")
    }
}

fn machine_and_phase(woke: &Value) -> (String, String) {
    (woke["machineId"].as_str().unwrap_or_default().to_string(), woke["phase"].as_str().unwrap_or_default().to_string())
}

// "a Boat wake whose daemon answers two and a half minutes after the box is up succeeds on the first send, on the same machine"
#[test]
fn a_boat_wake_whose_daemon_answers_two_and_a_half_minutes_after_the_box_is_up_succeeds_on_the_first_send_on_the_same_machine() {
    drive(|| async {
        // Five wakes on the dev host (2026-09-27) had no daemon answer 120 s after the box read ready; the one kept after its
        // failure, bx_eva5rxgz, served on the next send.
        let t = boat_wake(true).await;
        let began = t.fc.now();
        let waking = t.waking();
        t.pump(|| waking.ended(), || if t.fc.now() - began >= 150_000 { t.daemon.up() }).await;
        let woke = waking.result().await.expect("a wake");
        assert_eq!(machine_and_phase(&woke), ("m1".to_string(), "running".to_string()));
        assert_eq!(t.guest.backend.machines().len(), 1);
        t.close().await;
    });
}

// "a Boat wake whose daemon starts listening seven minutes after the box is up succeeds on the first send, on the same machine"
#[test]
fn a_boat_wake_whose_daemon_starts_listening_seven_minutes_after_the_box_is_up_succeeds_on_the_first_send_on_the_same_machine() {
    drive(|| async {
        // Two wakes on the dev host (2026-09-29) had their daemon listening 8 and 12 minutes after boot: Boat starts the
        // restored services only once its restore is done, and nothing Boat's API says tells that moment apart.
        let t = boat_wake(false).await;
        let began = t.fc.now();
        let waking = t.waking();
        // What the row reads a minute past the old cut, which the app words as waking with the time it has taken.
        let at_six: Rc<RefCell<Option<JoinHandle<String>>>> = Rc::default();
        t.pump(
            || waking.ended(),
            || {
                if at_six.borrow().is_none() && t.fc.now() - began >= 6 * 60_000 {
                    let (rt, id) = (t.fx.rt.clone(), t.ws_id.clone());
                    *at_six.borrow_mut() =
                        Some(tokio::task::spawn_local(async move { get(&rt, &id).await["phase"].as_str().unwrap_or_default().to_string() }));
                }
                if t.fc.now() - began >= 7 * 60_000 {
                    t.daemon.up();
                }
            },
        )
        .await;
        let woke = waking.result().await.expect("a wake");
        assert_eq!(machine_and_phase(&woke), ("m1".to_string(), "running".to_string()));
        let six = at_six.borrow_mut().take().expect("a read at six minutes");
        assert_eq!(six.await.expect("a task"), "waking");
        assert!(t.fc.now() - began >= 7 * 60_000);
        assert!(t.daemon.dials() > 1, "{}", t.daemon.dials());
        assert_eq!(t.guest.backend.machines().len(), 1);
        t.close().await;
    });
}

/// A Boat wake whose daemon's unit is enabled and never started, as Boat left bx_z4284vcx (2026-09-29): the edge refuses
/// every upgrade until the host runs the unit's start over the machine's exec road.
async fn unstarted_wake() -> Boat {
    let t = boat_wake(false).await;
    let start = format!("systemctl start {DAEMON_UNIT}");
    let m = t.m1.clone();
    let command = start.clone();
    t.m1.set_start_daemon(Some(Rc::new(move || {
        let (m, command) = (m.clone(), command.clone());
        Box::pin(async move { m.exec(&command, ExecOptions::default()).await })
    })));
    let base = t.guest.backend.exec_impl_fn();
    let daemon = t.daemon.clone();
    t.guest.backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if cmd == start {
            daemon.up();
        }
        base(m, cmd, stdin)
    }));
    t
}

fn starts(t: &Boat) -> usize {
    let start = format!("systemctl start {DAEMON_UNIT}");
    t.m1.exec_log().iter().filter(|c| **c == start).count()
}

/// `^daemon on m1 \(workspace ws_\w+\) had not answered {seconds} s into the wait, so wsp started it{tail}$`
fn start_said(line: &str, seconds: u32, tail: &str) -> bool {
    line.strip_prefix("daemon on m1 (workspace ws_")
        .and_then(|rest| rest.split_once(&format!(") had not answered {seconds} s into the wait, so wsp started it")))
        .is_some_and(|(id, after)| !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && after == tail)
}

// "a Boat wake whose daemon was never started starts it through the machine's exec a minute in, and succeeds on the first send"
#[test]
fn a_boat_wake_whose_daemon_was_never_started_starts_it_through_the_machines_exec_a_minute_in_and_succeeds_on_the_first_send() {
    drive(|| async {
        let t = unstarted_wake().await;
        let warned = capture();
        let began = t.fc.now();
        let waking = t.waking();
        t.pump(|| waking.ended(), || {}).await;
        let woke = waking.result().await.expect("a wake");
        assert_eq!(machine_and_phase(&woke), ("m1".to_string(), "running".to_string()));
        assert_eq!(starts(&t), 1);
        assert!(t.fc.now() - began >= 60_000);
        // The link redials on real timers, so how much of the test's clock passes before it lands varies; the bound is what
        // matters, long before the fifteen minute cut.
        assert!(t.fc.now() - began < 5 * 60_000, "{}", t.fc.now() - began);
        assert!(warned.lines().iter().any(|l| start_said(l, 60, " (exit 0)")), "{:?}", warned.lines());
        assert_eq!(t.guest.backend.machines().len(), 1);
        t.close().await;
    });
}

async fn asks_for_the_start_again(first_fails: bool) {
    let t = unstarted_wake().await;
    let asked = t.m1.start_daemon_fn().expect("a start road");
    let tries = Rc::new(Cell::new(0u32));
    let counted = tries.clone();
    t.m1.set_start_daemon(Some(Rc::new(move || {
        counted.set(counted.get() + 1);
        if counted.get() == 1 {
            return if first_fails {
                Box::pin(async { Err(wsp_engine::errors::EngineError::Other { message: "Request timeout after 69998ms".into(), code: None }) })
            } else {
                Box::pin(async { Ok(exec_exit(EXEC_DEADLINE_EXIT, "", "")) })
            };
        }
        asked()
    })));
    let warned = capture();
    let began = t.fc.now();
    let waking = t.waking();
    t.pump(|| waking.ended(), || {}).await;
    let woke = waking.result().await.expect("a wake");
    assert_eq!(machine_and_phase(&woke), ("m1".to_string(), "running".to_string()));
    assert_eq!(tries.get(), 2);
    assert_eq!(starts(&t), 1);
    assert!(t.fc.now() - began >= 2 * 60_000);
    assert!(t.fc.now() - began < 5 * 60_000, "{}", t.fc.now() - began);
    let failed = if first_fails { ", and the start failed (Request timeout after 69998ms)".to_string() } else { format!(" (exit {EXEC_DEADLINE_EXIT})") };
    let said = warned.lines();
    assert!(said.iter().any(|l| start_said(l, 60, &failed)), "{said:?}");
    assert!(said.iter().any(|l| start_said(l, 120, " (exit 0)")), "{said:?}");
    t.close().await;
}

// "asks for the start again a minute after one that throws, so a start lost on a box still streaming its disk is not the last"
#[test]
fn asks_for_the_start_again_a_minute_after_one_that_throws_so_a_start_lost_on_a_box_still_streaming_its_disk_is_not_the_last() {
    drive(|| asks_for_the_start_again(true));
}

// "asks for the start again a minute after one that runs out its timeout, so a start lost on a box still streaming its disk is not the last"
#[test]
fn asks_for_the_start_again_a_minute_after_one_that_runs_out_its_timeout_so_a_start_lost_on_a_box_still_streaming_its_disk_is_not_the_last() {
    drive(|| asks_for_the_start_again(false));
}

// "starts nothing on a machine whose daemon answered, even once the minute has passed"
#[test]
fn starts_nothing_on_a_machine_whose_daemon_answered_even_once_the_minute_has_passed() {
    drive(|| async {
        let t = unstarted_wake().await;
        t.daemon.up();
        let began = t.fc.now();
        let waking = t.waking();
        t.pump(|| waking.ended(), || {}).await;
        let woke = waking.result().await.expect("a wake");
        assert_eq!(machine_and_phase(&woke), ("m1".to_string(), "running".to_string()));
        assert!(t.fc.now() - began < 60_000);
        // A start armed by the wait and never cleared would fire here.
        t.pump(|| t.fc.now() - began >= 2 * 60_000, || {}).await;
        assert_eq!(starts(&t), 0);
        t.close().await;
    });
}

// "a Boat wake whose daemon never answers fails once its fifteen minutes are out, saying so, and keeps the machine"
#[test]
fn a_boat_wake_whose_daemon_never_answers_fails_once_its_fifteen_minutes_are_out_saying_so_and_keeps_the_machine() {
    drive(|| async {
        let t = boat_wake(true).await;
        let began = t.fc.now();
        let waking = t.waking();
        t.pump(|| waking.ended(), || {}).await;
        let refused = waking.result().await.expect_err("a refusal");
        // /the wake of m1 did not finish: attempt 1: daemon on m1 did not answer within 900000 ms \(daemon link timed out after \d+ ms\).*the workspace keeps this machine and its disk/
        assert!(
            in_order(
                &refused,
                &[
                    "the wake of m1 did not finish: attempt 1: daemon on m1 did not answer within 900000 ms (daemon link timed out after ",
                    " ms)",
                    "the workspace keeps this machine and its disk"
                ]
            ),
            "{refused}"
        );
        assert!(t.fc.now() - began >= 15 * 60_000);
        assert!(t.fc.now() - began < 16 * 60_000, "{}", t.fc.now() - began);
        assert_eq!(get(&t.fx.rt, &t.ws_id).await["machineId"], "m1");
        assert!(!t.m1.killed.get());
        t.close().await;
    });
}

// "a Boat wake whose daemon exits as it starts fails inside a minute with the daemon's own words, and keeps the machine"
#[test]
fn a_boat_wake_whose_daemon_exits_as_it_starts_fails_inside_a_minute_with_the_daemons_own_words_and_keeps_the_machine() {
    drive(|| async {
        // A box whose binary predates --doors, under today's unit, which Restart=always starts again every second.
        let t = boat_wake(false).await;
        let asked = Rc::new(Cell::new(0u32));
        let counted = asked.clone();
        t.m1.set_daemon_exits(Some(Rc::new(move || {
            counted.set(counted.get() + 1);
            let said = (counted.get() >= 3).then(|| "error: unexpected argument '--doors' found; Usage: wsp-daemon [OPTIONS]".to_string());
            Box::pin(async move { Ok(said) })
        })));
        let began = t.fc.now();
        let waking = t.waking();
        t.pump(|| waking.ended(), || {}).await;
        let refused = waking.result().await.expect_err("a refusal");
        // /the wake of m1 did not finish: attempt 1: the daemon on m1 exits as it starts \(error: unexpected argument '--doors' found; Usage: wsp-daemon \[OPTIONS\]\).*the workspace keeps this machine and its disk/
        assert!(
            in_order(
                &refused,
                &[
                    "the wake of m1 did not finish: attempt 1: the daemon on m1 exits as it starts (error: unexpected argument '--doors' found; Usage: wsp-daemon [OPTIONS])",
                    "the workspace keeps this machine and its disk"
                ]
            ),
            "{refused}"
        );
        assert!(t.fc.now() - began < 60_000, "{}", t.fc.now() - began);
        assert_eq!(asked.get(), 3);
        assert_eq!(get(&t.fx.rt, &t.ws_id).await["machineId"], "m1");
        assert!(!t.m1.killed.get());
        // A look armed by the wait and never cleared would ask again here.
        t.pump(|| t.fc.now() - began >= 2 * 60_000, || {}).await;
        assert_eq!(asked.get(), 3);
        t.close().await;
    });
}

// "a stop pulled ten seconds into a Boat wake whose daemon has not answered lets go at once, not when the fifteen minutes are out"
#[test]
fn a_stop_pulled_ten_seconds_into_a_boat_wake_whose_daemon_has_not_answered_lets_go_at_once_not_when_the_fifteen_minutes_are_out() {
    drive(|| async {
        let t = boat_wake(true).await;
        let waking = t.waking();
        let began = t.fc.now();
        t.pump(|| t.fc.now() - began >= 10_000, || {}).await;
        let pulled = t.fc.now();
        let stopped = Rc::new(Cell::new(false));
        let (rt, id, flag) = (t.fx.rt.clone(), t.ws_id.clone(), stopped.clone());
        let stopping = tokio::task::spawn_local(async move {
            let out = rt.workspaces().stop_wake(&id, None).await;
            flag.set(true);
            out
        });
        t.pump(|| stopped.get(), || {}).await;
        stopping.await.expect("a task").expect("a stop");
        assert!(t.fc.now() - pulled <= 1_000, "{}", t.fc.now() - pulled);
        let done = waking.done.clone();
        let refused = waking.result().await.expect_err("a refusal");
        assert!(refused.contains(WAKE_STOPPED), "{refused}");
        assert!(done.get());
        assert_eq!(get(&t.fx.rt, &t.ws_id).await["machineId"], "m1");
        t.close().await;
    });
}

// "leaves a computer that keeps an image exactly as it was: every nap stashes the vault, a failed wake keeps the machine, and the rebuild reads the vault"
#[test]
fn leaves_a_computer_that_keeps_an_image_exactly_as_it_was_every_nap_stashes_the_vault_a_failed_wake_keeps_the_machine_and_the_rebuild_reads_the_vault() {
    drive(|| async {
        let g = guest_backend();
        let state = sqlite_state();
        let port = dropping_port().await;
        let fx = runtime(&g.backend).store(&state.store).build();
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        g.backend.lifecycle().set_wake_attempts(1);
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", port.port));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(*g.tars.borrow(), ["m1"]);
        assert_eq!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read"), Some(b"tarbytes".to_vec()));
        assert!(get(&fx.rt, &ws.view.id).await.get("vaultedAt").is_some());
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("daemon on m1 did not answer"), "{}", refused.message);
        assert!(g.untars.borrow().is_empty());
        let rebuilt = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_eq!(j(&rebuilt)["machineId"], "m2");
        assert_eq!(*g.untars.borrow(), ["m2"]);
        fx.rt.close().await.expect("a close");
    });
}

// "the nap and rebuild of a workspace whose pause keeps the disk" > "naps with no export: it syncs the disk, reads no home, writes no archive, and stores no vault"
// A box keeps its image and its pause is a stop that snapshots the disk; a wake resumes that disk, so the nap reads nothing
// of the home. The vault is taken off the running machine only where it is read: a rebuild.
fn boxlike() -> Guest {
    let made = guest_backend();
    made.backend.set_capabilities(|c| c.pause_mode = Some(PauseMode::Disk));
    made
}

#[test]
fn naps_with_no_export_it_syncs_the_disk_reads_no_home_writes_no_archive_and_stores_no_vault() {
    drive(|| async {
        let g = boxlike();
        let state = sqlite_state();
        let fx = runtime(&g.backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m = g.backend.machine(0);
        let napped = fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(j(&napped)["phase"], "napping");
        assert!(m.paused.get());
        // The disk is still flushed before the stop snapshots it, but nothing of the home is read or archived.
        assert!(commands(&g.backend).iter().any(|c| c.starts_with("sync &&")));
        assert!(!commands(&g.backend).iter().any(|c| c.contains(".wsp-upgraded")));
        assert!(!commands(&g.backend).iter().any(|c| c.contains("ls -A /root")));
        assert!(g.tars.borrow().is_empty());
        assert!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read").is_none());
        let record = get(&fx.rt, &ws.view.id).await;
        assert!(record.get("vaultedAt").is_none() && record.get("vaultRefused").is_none(), "{record}");
        let woken = fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_eq!(j(&woken)["phase"], "running");
        assert_eq!(g.backend.machine(0).resumes.get(), 1);
        assert!(g.untars.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "a rebuild takes the vault off the running machine and carries it onto the replacement"
#[test]
fn a_rebuild_takes_the_vault_off_the_running_machine_and_carries_it_onto_the_replacement() {
    drive(|| async {
        let g = boxlike();
        let fx = runtime(&g.backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert!(g.tars.borrow().is_empty());
        let rebuilt = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_eq!(j(&rebuilt)["machineId"], "m2");
        // The archive was read off the running machine at the rebuild, not at any nap, and landed on the fork.
        assert_eq!(*g.tars.borrow(), ["m1"]);
        assert_eq!(*g.untars.borrow(), ["m2"]);
        fx.rt.close().await.expect("a close");
    });
}

// "a rebuild whose home is over the cap says so before it touches the machine, and replaces it without a backup"
#[test]
fn a_rebuild_whose_home_is_over_the_cap_says_so_before_it_touches_the_machine_and_replaces_it_without_a_backup() {
    drive(|| async {
        let g = boxlike();
        let base = g.backend.exec_impl_fn();
        g.backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            if cmd.starts_with("wc -c <") {
                return Box::pin(async { Ok(exec_ok(&format!("{}\n", 300 * 1024 * 1024))) });
            }
            base(m, cmd, stdin)
        }));
        let fx = runtime(&g.backend).build();
        let warned = Rc::new(capture());
        // The over-cap line is said while only the old machine stands, before the replacement is forked: the lines said by
        // the time the second machine is asked for.
        let at_second: Rc<RefCell<Option<Vec<String>>>> = Rc::default();
        let (create, seen, said_so_far) = (g.backend.create_fn(), at_second.clone(), warned.clone());
        g.backend.set_create(Rc::new(move |spec, waiting, sent| {
            seen.borrow_mut().get_or_insert_with(|| said_so_far.lines());
            create(spec, waiting, sent)
        }));
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        at_second.borrow_mut().take();
        let rebuilt = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_eq!(j(&rebuilt)["machineId"], "m2");
        let before_fork = at_second.borrow().clone().expect("the replacement was forked");
        let over_cap = |l: &String| l.contains("over the") && l.contains("cap");
        assert!(before_fork.iter().any(over_cap), "{before_fork:?}");
        let said = warned.lines();
        assert!(said.iter().any(|l| in_order(l, &["300 MB", "over the 200 MB cap"])), "{said:?}");
        // The home was read off the running machine, and nothing was carried onto the fork.
        assert_eq!(*g.tars.borrow(), ["m1"]);
        assert!(g.untars.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// "a rebuild of a napped workspace wakes it and takes the vault off the woken machine, not the one a nap stored"
#[test]
fn a_rebuild_of_a_napped_workspace_wakes_it_and_takes_the_vault_off_the_woken_machine_not_the_one_a_nap_stored() {
    drive(|| async {
        let g = boxlike();
        let fx = runtime(&g.backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert!(g.tars.borrow().is_empty());
        let rebuilt = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_eq!(j(&rebuilt)["machineId"], "m2");
        // The napped machine was resumed so the vault came off it live, then the fork carried it.
        assert_eq!(g.backend.machine(0).resumes.get(), 1);
        assert_eq!(*g.tars.borrow(), ["m1"]);
        assert_eq!(*g.untars.borrow(), ["m2"]);
        fx.rt.close().await.expect("a close");
    });
}

// "a rebuild refuses in one line when a napped workspace cannot be woken, and keeps the machine"
#[test]
fn a_rebuild_refuses_in_one_line_when_a_napped_workspace_cannot_be_woken_and_keeps_the_machine() {
    drive(|| async {
        let g = boxlike();
        let port = dropping_port().await;
        let fx = runtime(&g.backend).build();
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        g.backend.lifecycle().set_wake_attempts(1);
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        g.backend.machine(0).set_preview_url(&format!("ws://127.0.0.1:{}", port.port));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let refused = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect_err("a refusal");
        assert!(refused.message.contains("could not be woken to back up before the rebuild"), "{}", refused.message);
        // Nothing was replaced or read off the machine: the workspace keeps m1 and its disk.
        assert_eq!(names(&g.backend), ["m1"]);
        assert!(!g.backend.machine(0).killed.get());
        assert!(g.tars.borrow().is_empty());
        assert!(g.untars.borrow().is_empty());
        fx.rt.close().await.expect("a close");
    });
}
