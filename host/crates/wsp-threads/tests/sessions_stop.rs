// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_stop.md. TS cases: owed-box.test.ts, box-stop.test.ts, stop-past-grace.test.ts, thread-cap.test.ts,
//! sub-queue.test.ts, subagents.test.ts (task stops), permission-relay.test.ts (a caller the stop cannot reach).
//!
//! Not tested here: K5 (a send waiting on an owed end is given up) and the wake of sends behind a running turn
//! (K1 with a box). Both read state only the launch road writes (`held_sends`, `behind_turn`, `waits_for`), and
//! `SessionsArea::stops` is `pub(crate)`, so a test outside the crate cannot set or read it.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::Shared;
use support::*;
use wsp_core::console;
use wsp_core::types::wiring::{AccessMove, SteerTaken};
use wsp_machines::machines::boot::entries::RowAt;
use wsp_threads::threads::sessions::stop::Ended;
use wsp_threads::threads::threads::cap::CapWaiting;
use wsp_wire::views::session::SessionStatus;
use wsp_wire::wire::limits::STOP_REACH_MS;
use wsp_wire::wire::runtime::{
    task_stop_refused_line, task_stop_unsupported_line, SessionAnswerResult, SessionInterruptOutcome, SessionInterruptResult,
    SessionStartOutcome,
};
use wsp_words::place_state::cap_stopped_line;
use wsp_words::words::thread::TURN_STOPPED_LINE;

const KEY: &str = "sess-1";
const TURN: &str = "turn-1";
/// Its first eight characters, `thr-ab12`, are what the host log names it by.
const THREAD: &str = "thr-ab12cd34";

type Log = Rc<RefCell<Vec<String>>>;

fn say(log: &Log, line: impl Into<String>) {
    log.borrow_mut().push(line.into());
}

fn fail(message: &str) -> HostError {
    HostError::new(message)
}

/// A promise the test settles by hand: the future and what settles it.
fn gate() -> (Joined, Box<dyn FnOnce()>) {
    let (tx, rx) = oneshot::channel::<()>();
    let joined: Joined = rx.map(|_| ()).boxed_local().shared();
    (
        joined,
        Box::new(move || {
            let _ = tx.send(());
        }),
    )
}

/// Runs `f` detached; the cell holds its answer once it has one.
fn spawn<T: 'static>(f: impl Future<Output = T> + 'static) -> Rc<RefCell<Option<T>>> {
    let cell = Rc::new(RefCell::new(None));
    let put = cell.clone();
    tokio::task::spawn_local(async move {
        let v = f.await;
        *put.borrow_mut() = Some(v);
    });
    cell
}

/// A harness session the test drives: it records what it is asked, and `finished` settles when it ends.
struct FakeHandle {
    view: SessionView,
    turn: String,
    log: Log,
    done: Shared<LocalBoxFuture<'static, TurnResult>>,
    finish: RefCell<Option<oneshot::Sender<()>>>,
    ends_on_interrupt: bool,
    hangs: bool,
    holds: RefCell<Option<Rc<dyn Fn() -> bool>>>,
    calls: RefCell<Vec<String>>,
    tasks: Option<Rc<dyn Fn(&str) -> Result<TaskStop, HostError>>>,
}

impl FakeHandle {
    fn finish(&self) {
        if let Some(tx) = self.finish.borrow_mut().take() {
            let _ = tx.send(());
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl SessionHandle for FakeHandle {
    fn id(&self) -> String {
        self.view.id.clone()
    }
    fn workspace_id(&self) -> String {
        self.view.workspace_id.clone()
    }
    fn finished(&self) -> Shared<LocalBoxFuture<'static, TurnResult>> {
        self.done.clone()
    }
    fn turn_id(&self) -> String {
        self.turn.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        SessionStartOutcome::Started
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        self.view.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.calls.borrow_mut().push("interrupt".into());
        let probe = self.holds.borrow().clone();
        match probe {
            Some(p) => say(&self.log, format!("interrupt(holds={})", p())),
            None => say(&self.log, "interrupt"),
        }
        if self.ends_on_interrupt {
            self.finish();
        }
        if self.hangs {
            futures::future::pending().boxed_local()
        } else {
            futures::future::ready(()).boxed_local()
        }
    }
    fn steer(&self, _prompt: String, _id: Option<String>, _images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, _ask_id: String, _option_id: String, _reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _mode: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, task: String) -> Option<Local<'static, TaskStop>> {
        let answer = self.tasks.clone()?;
        self.calls.borrow_mut().push(format!("stop_task({task})"));
        let r = answer(&task);
        Some(async move { r }.boxed_local())
    }
}

struct Scene {
    host: Rc<FakeHost>,
    area: SessionsArea,
    log: Log,
}

/// One workspace `w1`; every call a stop makes of the runtime is logged in order, and answers its neutral default.
fn scene() -> Scene {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    let area = SessionsArea::new(host.clone());
    let s = Scene { host, area, log: Rc::new(RefCell::new(Vec::new())) };
    set_under(&s, vec![]);
    set_queued(&s, false);
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&str) -> Rc<dyn Fn()>>(
        "SessionsCtx::cap_stopping",
        Rc::new(move |row: &str| {
            say(&l, format!("cap_stopping({row})"));
            let l = l.clone();
            let release: Rc<dyn Fn()> = Rc::new(move || say(&l, "release"));
            release
        }),
    );
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&str) -> ()>("SessionsCtx::try_stopped", Rc::new(move |turn: &str| say(&l, format!("try_stopped({turn})"))));
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&str, &str) -> ()>(
        "SessionsCtx::cap_stop",
        Rc::new(move |turn: &str, line: &str| say(&l, format!("cap_stop({turn}, {line})"))),
    );
    s
}

fn set_under(s: &Scene, under: Vec<String>) {
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> LocalBoxFuture<'static, Result<Vec<String>, HostError>>>(
        "SessionsCtx::stop_under",
        Rc::new(move |thread: &str, _o: Option<&Caller>| {
            say(&l, format!("stop_under({thread})"));
            let under = under.clone();
            async move { Ok(under) }.boxed_local()
        }),
    );
}

fn set_queued(s: &Scene, queued: bool) {
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<bool, HostError>>>(
        "SessionsCtx::drop_queued",
        Rc::new(move |turn: &str| {
            say(&l, format!("drop_queued({turn})"));
            async move { Ok(queued) }.boxed_local()
        }),
    );
}

fn set_held(s: &Scene, turn: &'static str, held: CapHeld) {
    s.host.hooks.set::<dyn Fn(&str) -> Option<CapHeld>>(
        "SessionsCtx::cap_held",
        Rc::new(move |id: &str| if id == turn { Some(held.clone()) } else { None }),
    );
}

/// The kind ends a thread's group; `words` answers what each end says, by the ask it was made with. Logged when the
/// end runs, not when it is made.
fn group_end(s: &Scene, words: impl Fn(EndThread) -> Result<Option<String>, HostError> + 'static) {
    let l = s.log.clone();
    let words = Rc::new(words);
    s.host.hooks.set::<dyn Fn(&Entry, &str, EndThread) -> Option<LocalBoxFuture<'static, Result<Option<String>, HostError>>>>(
        "SessionsCtx::end_thread",
        Rc::new(move |_e: &Entry, thread: &str, o: EndThread| {
            let (l, words, thread) = (l.clone(), words.clone(), thread.to_string());
            Some(
                async move {
                    say(&l, format!("end_thread({thread}, remove={}, away={})", o.remove, o.away));
                    words(o)
                }
                .boxed_local(),
            )
        }),
    );
}

/// The workspace runs at a computer the person joined, named `hetzner`.
fn on_box(s: &Scene) {
    s.host.hooks.set::<dyn Fn(&WorkspaceRecord) -> Option<String>>(
        "SessionsCtx::workspace_place",
        Rc::new(|_r: &WorkspaceRecord| Some("place-1".to_string())),
    );
    s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::place_name_of", Rc::new(|_p: &str| "hetzner".to_string()));
}

fn set_away(s: &Scene, away: Rc<Cell<bool>>) {
    s.host.hooks.set::<dyn Fn(&str) -> bool>("SessionsCtx::place_away", Rc::new(move |_p: &str| away.get()));
}

fn set_link(s: &Scene, on: bool) {
    s.host.hooks.set::<dyn Fn(&str) -> bool>("SessionsCtx::has_link", Rc::new(move |_p: &str| on));
}

/// The kind owes an end to the computer: `owed` is its `paid`, `Some(None)` where nothing is owed, `None` where the
/// kind has no such end.
fn owes(s: &Scene, owed: Option<Option<Joined>>) {
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<Option<Option<Joined>>, HostError>>>(
        "SessionsCtx::end_owed",
        Rc::new(move |_e: &Entry, thread: &str| {
            say(&l, format!("end_owed({thread})"));
            let owed = owed.clone();
            async move { Ok(owed) }.boxed_local()
        }),
    );
}

/// The ping a stop sends: answers when the returned closure is called with its answer.
fn ping_by_hand(s: &Scene) -> Box<dyn FnOnce(Result<(), HostError>)> {
    let (tx, rx) = oneshot::channel::<Result<(), HostError>>();
    let rx = RefCell::new(Some(rx));
    s.host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
        "SessionsCtx::ping",
        Rc::new(move |_p: &str| {
            let rx = rx.borrow_mut().take().expect("one ping");
            async move { rx.await.unwrap_or(Ok(())) }.boxed_local()
        }),
    );
    Box::new(move |r| {
        let _ = tx.send(r);
    })
}

/// The door's absent listener for the place, and how many times it was taken off.
fn absent_listener(s: &Scene) -> (Rc<RefCell<Option<Rc<dyn Fn()>>>>, Rc<Cell<u32>>) {
    let held: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let offs = Rc::new(Cell::new(0));
    let (h, o) = (held.clone(), offs.clone());
    s.host.hooks.set::<dyn Fn(&str, Rc<dyn Fn()>) -> Rc<dyn Fn()>>(
        "SessionsCtx::on_place_absent",
        Rc::new(move |_p: &str, f: Rc<dyn Fn()>| {
            *h.borrow_mut() = Some(f);
            let o = o.clone();
            let off: Rc<dyn Fn()> = Rc::new(move || o.set(o.get() + 1));
            off
        }),
    );
    (held, offs)
}

fn handle(s: &Scene, ends_on_interrupt: bool, hangs: bool) -> Rc<FakeHandle> {
    let (tx, rx) = oneshot::channel::<()>();
    let done = rx.map(|_| result(json!({ "status": "interrupted" }))).boxed_local().shared();
    Rc::new(FakeHandle {
        view: view(KEY, "w1", json!({ "threadId": THREAD })),
        turn: TURN.to_string(),
        log: s.log.clone(),
        done,
        finish: RefCell::new(Some(tx)),
        ends_on_interrupt,
        hangs,
        holds: RefCell::new(None),
        calls: RefCell::new(Vec::new()),
        tasks: None,
    })
}

/// A handle that answers a subagent stop with `answer`.
fn handle_with_tasks(s: &Scene, answer: Result<TaskStop, HostError>) -> Rc<FakeHandle> {
    let h = handle(s, true, false);
    let h = Rc::try_unwrap(h).ok().expect("a fresh handle");
    let answer = Rc::new(answer);
    Rc::new(FakeHandle { tasks: Some(Rc::new(move |_t: &str| (*answer).clone())), ..h })
}

/// Lets `h` say whether the thread's next turn is held at the moment the agent is asked to stop.
fn probe_holds(h: &FakeHandle, area: &SessionsArea) {
    let area = area.clone();
    *h.holds.borrow_mut() = Some(Rc::new(move || area.stop_holds(THREAD)));
}

struct Spec {
    key: &'static str,
    turn: &'static str,
    thread: &'static str,
    status: &'static str,
    handle: Option<Rc<FakeHandle>>,
    end: bool,
    launch: Option<Joined>,
    unanswered: bool,
}

fn spec() -> Spec {
    Spec { key: KEY, turn: TURN, thread: THREAD, status: "running", handle: None, end: true, launch: None, unanswered: false }
}

/// A row the host holds. Its `end` logs the call and settles the row as interrupted, as the turn's own end does.
fn put(s: &Scene, sp: Spec) -> Row {
    let v = view(sp.key, "w1", json!({ "threadId": sp.thread, "status": sp.status, "startedAt": T0 }));
    let r = row(v, sp.turn);
    r.borrow_mut().handle = sp.handle.map(|h| h as Rc<dyn SessionHandle>);
    if sp.end {
        let log = s.log.clone();
        let shown = Rc::downgrade(&r);
        r.borrow_mut().end = Some(Rc::new(move |reason: &str, by_stop: Option<bool>, unreached: Option<bool>| {
            say(&log, format!("end({reason}, {by_stop:?}, {unreached:?})"));
            if let Some(shown) = shown.upgrade() {
                shown.borrow_mut().view.status = SessionStatus::Interrupted;
            }
        }));
    }
    r.borrow_mut().launch = sp.launch;
    r.borrow_mut().unanswered = sp.unanswered;
    s.host.add_row(sp.key, r.clone());
    r
}

/// A cap hold nothing has looked at yet.
fn held() -> CapHeld {
    CapHeld { place_id: None, lenders: Vec::new(), order: 0, stopped: None, stops: Vec::new(), waiting: None, through: None }
}

/// A cap hold whose turn waits with `wait`.
fn held_waiting(wait: ThreadCapWait) -> CapHeld {
    let waiting = CapWaiting {
        wait,
        workspace_id: "w1".to_string(),
        thread_id: THREAD.to_string(),
        session_id: KEY.to_string(),
        wake: Rc::new(|| {}),
        stop: Rc::new(|_line: &str| {}),
    };
    CapHeld { waiting: Some(waiting), ..held() }
}

fn answered(r: Result<SessionInterruptResult, HostError>) -> Value {
    serde_json::to_value(r.expect("a stop answers")).expect("an answer serializes")
}

fn stop_lines(cap: &console::Captured) -> Vec<String> {
    cap.lines().into_iter().filter(|l| l.starts_with("stop on thread")).collect()
}

fn strs(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

fn thread_wait() -> ThreadCapWait {
    serde_json::from_value(json!({ "placeId": "p1", "place": "mac", "running": 1, "atOnce": 1 })).expect("a wait")
}

// ---- holds -----------------------------------------------------------------------------------------------------

// K1: box-stop.test.ts:304 (a second Stop keeps the first Stop's hold)
#[test]
fn a_second_hold_joins_the_first() {
    run_local(async {
        let s = scene();
        let (g1, open1) = gate();
        let (g2, open2) = gate();
        assert!(!s.area.stop_holds(THREAD));
        s.area.hold_next(THREAD, g1, None);
        assert!(s.area.stop_holds(THREAD));
        s.area.hold_next(THREAD, g2, Some("hetzner"));
        open2();
        settle().await;
        assert!(s.area.stop_holds(THREAD), "the second gate settling alone does not release the first's hold");
        open1();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K1
#[test]
fn a_first_hold_settling_alone_keeps_the_second() {
    run_local(async {
        let s = scene();
        let (g1, open1) = gate();
        let (g2, open2) = gate();
        s.area.hold_next(THREAD, g1, None);
        s.area.hold_next(THREAD, g2, None);
        open1();
        settle().await;
        assert!(s.area.stop_holds(THREAD), "the thread's gate is the second, joined with the first");
        open2();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K1
#[test]
fn the_hold_is_forgotten_when_its_gate_settles() {
    run_local(async {
        let s = scene();
        let (g, open) = gate();
        s.area.hold_next(THREAD, g, Some("hetzner"));
        settle().await;
        assert!(s.area.stop_holds(THREAD));
        assert!(!s.area.stop_holds("thr-other"), "a hold is by thread");
        open();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// ---- does the computer hear the stop -----------------------------------------------------------------------------

// K2: box-stop.test.ts:112
#[test]
fn a_computer_away_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        assert!(s.area.stop_unheard("place-1", None).await);
        assert_eq!(s.host.hooks.called("ping"), Vec::<String>::new(), "nothing is asked of a computer known away");
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2
#[test]
fn a_computer_with_no_link_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, false);
        set_away(&s, Rc::new(Cell::new(false)));
        assert!(s.area.stop_unheard("place-1", None).await);
        assert_eq!(s.host.hooks.called("ping"), Vec::<String>::new());
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2: box-stop.test.ts:185
#[test]
fn a_ping_answered_with_no_agent_stop_hears() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, offs) = absent_listener(&s);
        let answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        assert_eq!(*said.borrow(), None, "waiting on the ping");
        answer(Ok(()));
        settle().await;
        assert_eq!(*said.borrow(), Some(false));
        assert_eq!(offs.get(), 1, "the listener is taken off");
        assert_eq!(s.host.clock.pending(), 0, "the quiet timer is cleared");
        assert_eq!(s.host.hooks.called("ping"), vec!["ping(place-1)".to_string()]);
    });
}

// K2: box-stop.test.ts:197
#[test]
fn no_answer_in_stop_reach_ms_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, offs) = absent_listener(&s);
        let _answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        s.host.clock.advance(STOP_REACH_MS - 1);
        settle().await;
        assert_eq!(*said.borrow(), None, "not before the time is up");
        s.host.clock.advance(1);
        settle().await;
        assert_eq!(*said.borrow(), Some(true));
        assert_eq!(offs.get(), 1);
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2
#[test]
fn an_absent_event_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (absent, offs) = absent_listener(&s);
        let answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        assert_eq!(*said.borrow(), None);
        let f = absent.borrow().clone().expect("the host listens for the place going absent");
        f();
        settle().await;
        assert_eq!(*said.borrow(), Some(true));
        assert_eq!(offs.get(), 1);
        assert_eq!(s.host.clock.pending(), 0);
        answer(Ok(()));
        settle().await;
        assert_eq!(*said.borrow(), Some(true), "it settles once: a later ping answer changes nothing");
    });
}

// K2
#[test]
fn a_failed_ping_does_not_hear() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, offs) = absent_listener(&s);
        let answer = ping_by_hand(&s);
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", None).await }
        });
        settle().await;
        answer(Err(fail("the link closed")));
        settle().await;
        assert_eq!(*said.borrow(), Some(true));
        assert_eq!(offs.get(), 1);
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// K2: box-stop.test.ts:185 (the agent's stop lands first, so the computer was heard)
#[test]
fn an_agent_stop_that_settles_first_decides_by_whether_the_place_is_away() {
    run_local(async {
        for goes_away in [false, true] {
            let s = scene();
            set_link(&s, true);
            let away = Rc::new(Cell::new(false));
            set_away(&s, away.clone());
            let (_absent, offs) = absent_listener(&s);
            let answer = ping_by_hand(&s);
            let (stopping, stopped) = gate();
            let said = spawn({
                let area = s.area.clone();
                async move { area.stop_unheard("place-1", Some(stopping)).await }
            });
            settle().await;
            answer(Ok(()));
            settle().await;
            assert_eq!(*said.borrow(), None, "an answered ping with an agent stop out settles nothing");
            s.host.clock.advance(STOP_REACH_MS * 10);
            settle().await;
            assert_eq!(*said.borrow(), None, "an answered ping clears the quiet timer");
            away.set(goes_away);
            stopped();
            settle().await;
            assert_eq!(*said.borrow(), Some(goes_away));
            assert_eq!(offs.get(), 1);
        }
    });
}

// K2
#[test]
fn an_unanswered_ping_and_an_agent_stop_that_lands_first_was_heard() {
    run_local(async {
        let s = scene();
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        let (_absent, _offs) = absent_listener(&s);
        let _answer = ping_by_hand(&s);
        let (stopping, stopped) = gate();
        let said = spawn({
            let area = s.area.clone();
            async move { area.stop_unheard("place-1", Some(stopping)).await }
        });
        settle().await;
        stopped();
        settle().await;
        assert_eq!(*said.borrow(), Some(false));
        assert_eq!(s.host.clock.pending(), 0);
    });
}

// ---- finding the row ----------------------------------------------------------------------------------------------

// K3: box-stop.test.ts:102
#[test]
fn a_row_nobody_holds_is_not_found() {
    run_local(async {
        let s = scene();
        let r = s.area.interrupt_row("sess_nobody", None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-found" }));
        assert_eq!(s.host.hooks.called("entry_of_row"), Vec::<String>::new());
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(sess_nobody)", "release"]));
    });
}

// K3: thread-cap.test.ts:467 (a held turn is named by its turn id, its row under its launch's id)
#[test]
fn a_row_is_found_by_its_turn_id() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(TURN, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(h.calls(), strs(&["interrupt"]));
        assert!(s.log.borrow().contains(&"try_stopped(turn-1)".to_string()));
    });
}

// K4: permission-relay.test.ts:434
#[test]
fn a_row_the_caller_cannot_reach_reads_not_found() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let outside = Caller::Scoped {
            origin: WorkspaceOrigin::Here,
            by: serde_json::from_value(json!({ "kind": "thread", "threadId": "beside", "workspaceId": "w1", "rootThreadId": "beside" }))
                .expect("a scope"),
        };
        let seen: Rc<RefCell<Vec<Option<Caller>>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = seen.clone();
        let ats: Rc<RefCell<Vec<RowAt>>> = Rc::new(RefCell::new(Vec::new()));
        let at_sink = ats.clone();
        s.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(move |at: &RowAt, o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| {
                    at_sink.borrow_mut().push(at.clone());
                    sink.borrow_mut().push(o.cloned());
                    async move { Ok(None) }.boxed_local()
                }),
            );
        let r = s.area.interrupt_row(KEY, Some(outside.clone()), None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-found" }), "the same answer as a row nobody holds");
        assert_eq!(*seen.borrow(), vec![Some(outside)]);
        assert_eq!(*ats.borrow(), vec![RowAt { thread_id: Some(THREAD.to_string()), workspace_id: "w1".to_string() }]);
        assert_eq!(s.host.hooks.called("entry_of_row"), vec![format!("entry_of_row(Some(\"{THREAD}\"), w1, false)")]);
        assert_eq!(h.calls(), Vec::<String>::new(), "the agent hears nothing");
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(sess-1)", "release"]));
    });
}

// K3, K9: session-row.test.ts (a thread listed off its record), thread-cap.test.ts:467
#[test]
fn a_trimmed_thread_is_stopped_by_its_record() {
    run_local(async {
        let s = scene();
        s.host.add_record(THREAD, "w1", thread_record(json!({ "harness": "claude", "ended": "completed" })));
        set_under(&s, vec!["child-thr".to_string()]);
        let ats: Rc<RefCell<Vec<RowAt>>> = Rc::new(RefCell::new(Vec::new()));
        let at_sink = ats.clone();
        let host = s.host.clone();
        s.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(move |at: &RowAt, _o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| {
                    at_sink.borrow_mut().push(at.clone());
                    let live = host.live(&at.workspace_id);
                    async move { Ok(live) }.boxed_local()
                }),
            );
        let r = s.area.interrupt_row(THREAD, None, None).await;
        assert_eq!(
            answered(r),
            json!({ "outcome": "not-running", "under": ["child-thr"] }),
            "a thread nothing runs, its tree stopped under it"
        );
        let asked = RowAt { thread_id: Some(THREAD.to_string()), workspace_id: "w1".to_string() };
        let calls = ats.borrow();
        assert!(!calls.is_empty() && calls.iter().all(|c| *c == asked), "the trimmed row names the thread and its workspace: {calls:?}");
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(thr-ab12cd34)",
                "release",
                "drop_queued(thr-ab12cd34)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(thr-ab12cd34)",
                "release",
            ]),
            "the thread id is the turn"
        );
    });
}

// K4: a trimmed thread whose workspace the caller does not reach reads the same absence
#[test]
fn a_trimmed_thread_the_caller_cannot_reach_reads_not_found() {
    run_local(async {
        let s = scene();
        s.host.add_record(THREAD, "w1", thread_record(json!({ "harness": "claude" })));
        s.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(|_a: &RowAt, _o: Option<&Caller>, _talk: Option<TreeTalk>, _b: bool| async move { Ok(None) }.boxed_local()),
            );
        let r = s.area.interrupt_row(THREAD, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-found" }));
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(thr-ab12cd34)", "release"]));
    });
}

// ---- one subagent -------------------------------------------------------------------------------------------------

// K6: subagents.test.ts:428
#[test]
fn a_task_stop_stops_only_the_subagent() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(h.calls(), strs(&["stop_task(a1)"]), "the turn itself is not stopped");
        assert_eq!(*s.log.borrow(), Vec::<String>::new(), "no cap stop, no tree, no end of the row");
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Running);
    });
}

// K6: subagents.test.ts:428 (the outcome as it came)
#[test]
fn a_task_stop_the_harness_answers_not_running_comes_back_as_it_came() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::NotRunning));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), strs(&["stop_task(a1)"]));
    });
}

// K6: subagents.test.ts:441
#[test]
fn a_refused_task_stop_comes_back_in_the_agents_words() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::agent_label", Rc::new(|_h: &str| "Claude Code".to_string()));
        let h = handle_with_tasks(&s, Ok(TaskStop::Refused { error: "No task found with ID: zz9".to_string() }));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("zz9".to_string())).await;
        assert_eq!(
            answered(r),
            json!({ "outcome": "refused", "error": task_stop_refused_line("Claude Code", "No task found with ID: zz9") })
        );
        assert_eq!(h.calls(), strs(&["stop_task(zz9)"]));
        assert_eq!(s.host.hooks.called("agent_label"), vec!["agent_label(claude)".to_string()]);
    });
}

// K6: subagents.test.ts:446
#[test]
fn a_harness_with_no_task_stop_is_unsupported() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::agent_label", Rc::new(|_h: &str| "Claude Code".to_string()));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "unsupported", "error": task_stop_unsupported_line("Claude Code") }));
        assert_eq!(h.calls(), Vec::<String>::new());
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Running, "the turn runs on");
    });
}

// K6: a harness that has the road and says it is unsupported
#[test]
fn a_task_stop_the_harness_calls_unsupported_is_unsupported() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::agent_label", Rc::new(|_h: &str| "Claude Code".to_string()));
        let h = handle_with_tasks(&s, Ok(TaskStop::Unsupported));
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "unsupported", "error": task_stop_unsupported_line("Claude Code") }));
        assert_eq!(h.calls(), strs(&["stop_task(a1)"]));
    });
}

// K6: subagents.test.ts:456
#[test]
fn a_task_on_a_turn_not_running_is_not_running() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h.clone()), status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), Vec::<String>::new(), "a turn that is over has no child to stop");
    });
}

// K6: a running row with no handle has no road to its subagents
#[test]
fn a_task_on_a_row_with_no_handle_is_not_running() {
    run_local(async {
        let s = scene();
        put(&s, spec());
        let r = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(*s.log.borrow(), Vec::<String>::new());
    });
}

// ---- held and queued turns, and the tree under -------------------------------------------------------------------

// K7: thread-cap.test.ts:445 (a stop at a held turn: marked, never launches)
#[test]
fn a_held_turn_is_marked_and_never_launches() {
    run_local(async {
        let s = scene();
        let wait = thread_wait();
        set_held(&s, "turn-held", held_waiting(wait.clone()));
        let (launch, open) = gate();
        put(&s, Spec { key: "turn-held", turn: "turn-held", launch: Some(launch), end: false, ..spec() });
        let said = spawn({
            let area = s.area.clone();
            let l = s.log.clone();
            async move { area.stop_turn(Rc::new(move || say(&l, "marked")), "turn-held", None, None).await }
        });
        settle().await;
        let line = cap_stopped_line(Some(&wait));
        assert_eq!(
            *s.log.borrow(),
            vec![
                format!("cap_stop(turn-held, {line})"),
                "marked".to_string(),
                "drop_queued(turn-held)".to_string(),
                format!("stop_under({THREAD})")
            ],
            "marked before anything waits"
        );
        assert!(said.borrow().is_none(), "the stop waits for the held turn's launch to settle");
        open();
        settle().await;
        let r = said.borrow_mut().take().expect("the stop answers once the launch settled");
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(s.log.borrow().len(), 4, "no try, no agent stop, no end of the group");
    });
}

// K7: thread-cap.test.ts:949 (a held thread stopped before its first look has no wait to say)
#[test]
fn a_held_turn_that_has_not_looked_is_marked_with_no_wait() {
    run_local(async {
        let s = scene();
        set_held(&s, "turn-held", held());
        let (launch, open) = gate();
        put(&s, Spec { key: "turn-held", turn: "turn-held", launch: Some(launch), end: false, ..spec() });
        open();
        let r = s.area.stop_turn(Rc::new(|| {}), "turn-held", None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(s.log.borrow()[0], format!("cap_stop(turn-held, {})", cap_stopped_line(None)));
    });
}

// K8: thread-cap.test.ts:922 (a lead stopped while its detached child is held)
#[test]
fn a_stop_on_a_held_turn_takes_the_tree_under_it() {
    run_local(async {
        let s = scene();
        set_held(&s, "turn-held", held());
        set_under(&s, vec!["child-thr".to_string()]);
        let (launch, open) = gate();
        put(&s, Spec { key: "turn-held", turn: "turn-held", launch: Some(launch), end: false, ..spec() });
        open();
        let r = s.area.stop_turn(Rc::new(|| {}), "turn-held", None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "under": ["child-thr"] }));
    });
}

// K7: sub-queue.test.ts:285 (a turn queued on its machine is dropped)
#[test]
fn a_queued_turn_is_dropped() {
    run_local(async {
        let s = scene();
        set_queued(&s, true);
        put(&s, Spec { end: false, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(
            *s.log.borrow(),
            strs(&["cap_stopping(sess-1)", "release", "drop_queued(turn-1)", "release"]),
            "nothing under it, no try, no stop of an agent"
        );
    });
}

// K8: sub-queue.test.ts:285
#[test]
fn a_stop_takes_the_tree_under_first() {
    run_local(async {
        let s = scene();
        set_under(&s, vec!["sub-thr".to_string()]);
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "under": ["sub-thr"] }));
        let log = s.log.borrow().clone();
        let at = |what: &str| log.iter().position(|l| l == what).unwrap_or_else(|| panic!("{what} is in {log:?}"));
        assert!(at("stop_under(thr-ab12cd34)") < at("try_stopped(turn-1)"));
        assert!(at("stop_under(thr-ab12cd34)") < at("interrupt"), "the children go first");
    });
}

// K8: thread-cap.test.ts:922 (the lead itself may already be over)
#[test]
fn the_tree_under_rides_an_answer_for_a_turn_that_is_over() {
    run_local(async {
        let s = scene();
        set_under(&s, vec!["sub-thr".to_string()]);
        put(&s, Spec { status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running", "under": ["sub-thr"] }));
    });
}

// ---- ending the turn and its group --------------------------------------------------------------------------------

// K9, K13, K17: stop-past-grace.test.ts:96 (accepted, nothing left)
#[test]
fn a_stop_with_no_group_to_end_stops_the_turn_alone() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }), "no `left` where the kind ends no group");
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(sess-1)",
                "release",
                "drop_queued(turn-1)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(turn-1)",
                "interrupt(holds=false)",
                "release",
            ])
        );
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K11, K13, K16: stop-past-grace.test.ts:96, box-stop.test.ts:185
#[test]
fn a_stop_ends_the_turn_then_its_group() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("a process this thread started still runs".to_string())));
        let h = handle(&s, false, false);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let said = spawn({
            let area = s.area.clone();
            async move { area.interrupt_row(KEY, None, None).await }
        });
        settle().await;
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(sess-1)",
                "release",
                "drop_queued(turn-1)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(turn-1)",
                "interrupt(holds=true)",
            ]),
            "the next turn is held before the agent is stopped, and the group is not ended while the turn runs"
        );
        assert!(said.borrow().is_none(), "the stop waits for the harness to finish");
        assert!(s.area.stop_holds(THREAD));
        h.finish();
        settle().await;
        let r = said.borrow_mut().take().expect("the stop answers once the turn finished");
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "a process this thread started still runs" }));
        assert_eq!(s.log.borrow()[6..].to_vec(), strs(&["end_thread(thr-ab12cd34, remove=false, away=false)", "release"]),);
        assert!(!s.area.stop_holds(THREAD), "the hold is released once the end is over");
    });
}

// K16, K18: box-stop.test.ts:185 (the hold is released however the end went)
#[test]
fn a_failed_group_end_still_releases_the_hold() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Err(fail("the daemon would not answer")));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        let e = r.expect_err("the end failed, so the stop fails");
        assert_eq!(e.message, "the daemon would not answer");
        assert!(!s.area.stop_holds(THREAD));
        assert_eq!(s.log.borrow().last(), Some(&"release".to_string()));
    });
}

// K12: box-stop.test.ts:102 (a second Stop on the stopped turn)
#[test]
fn a_row_not_running_is_not_running() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), Vec::<String>::new());
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K12
#[test]
fn a_running_row_with_no_road_to_its_process_is_not_running() {
    run_local(async {
        let s = scene();
        let r = put(&s, Spec { end: false, ..spec() });
        assert!(r.borrow().handle.is_none());
        let said = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(said), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end(")), "nothing ended the row");
    });
}

// K12: owed-box.test.ts:41 (a line's try has its own hour)
#[test]
fn a_lines_try_is_not_an_unanswered_row_to_stop_here() {
    run_local(async {
        let s = scene();
        s.host.hooks.set::<dyn Fn(&Row) -> bool>("SessionsCtx::line_try", Rc::new(|_r: &Row| true));
        put(&s, Spec { unanswered: true, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end(")));
    });
}

// K12, K16: box-stop.test.ts:185 (a thread whose turn is over still has its group)
#[test]
fn a_finished_thread_has_its_group_ended() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("a process this thread started still runs".to_string())));
        put(&s, Spec { status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running", "left": "a process this thread started still runs" }));
        assert_eq!(s.log.borrow()[5..].to_vec(), strs(&["end_thread(thr-ab12cd34, remove=false, away=false)", "release"]));
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K12: a running row with no handle and a group end but no end of its own reads the same
#[test]
fn a_running_row_with_no_handle_and_no_end_has_its_group_ended() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("left words".to_string())));
        put(&s, Spec { end: false, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running", "left": "left words" }));
    });
}

// K12
#[test]
fn another_turn_running_is_not_what_was_stopped() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("must not be said".to_string())));
        put(&s, Spec { status: "completed", ..spec() });
        let other = handle(&s, true, false);
        put(&s, Spec { key: "sess-2", turn: "turn-2", handle: Some(other.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end_thread")), "the group is not ended under the turn that runs");
        assert_eq!(other.calls(), Vec::<String>::new());
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K12
#[test]
fn another_turn_launching_is_not_what_was_stopped_either() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("must not be said".to_string())));
        put(&s, Spec { status: "completed", ..spec() });
        let (launch, _open) = gate();
        put(&s, Spec { key: "sess-2", turn: "turn-2", launch: Some(launch), end: false, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "not-running" }));
        assert!(!s.log.borrow().iter().any(|l| l.starts_with("end_thread")));
    });
}

// K10: sessions.ts:155 (a turn let through and still reaching its machine)
#[test]
fn a_turn_still_launching_is_stopped_once_it_is_there() {
    run_local(async {
        let s = scene();
        let (launch, open) = gate();
        let launching = put(&s, Spec { launch: Some(launch), ..spec() });
        let h = handle(&s, true, false);
        let rows = s.host.rows.clone();
        let said = spawn({
            let area = s.area.clone();
            async move { area.end_turn(launching, None, Rc::new(move || rows.borrow().get(KEY).cloned())).await }
        });
        settle().await;
        assert!(said.borrow().is_none());
        assert_eq!(h.calls(), Vec::<String>::new(), "nothing to stop until the launch settles");
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        open();
        settle().await;
        let ended = said.borrow_mut().take().expect("the end answers once the launch settled").expect("an end");
        assert_eq!(ended, Ended { outcome: SessionInterruptOutcome::Accepted, left: None });
        assert_eq!(h.calls(), strs(&["interrupt"]), "the row is read again and its handle stopped");
    });
}

// ---- no agent stop ------------------------------------------------------------------------------------------------

// K15: sessions.ts:181 (a row a restart left running, its machine silent)
#[test]
fn no_agent_stop_and_no_group_ends_the_row_stopped_and_unreached() {
    run_local(async {
        let s = scene();
        put(&s, Spec { unanswered: true, ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert!(s.log.borrow().contains(&format!("end({TURN_STOPPED_LINE}, Some(true), Some(true))")), "{:?}", s.log.borrow());
    });
}

// K15
#[test]
fn no_agent_stop_with_a_group_ends_the_row_stopped_and_answers_the_groups_words() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Ok(Some("left words".to_string())));
        put(&s, spec());
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "left words" }));
        let log = s.log.borrow().clone();
        let end = log
            .iter()
            .position(|l| *l == format!("end({TURN_STOPPED_LINE}, Some(true), None)"))
            .unwrap_or_else(|| panic!("the row ends stopped, not unreached: {log:?}"));
        let group = log.iter().position(|l| l.starts_with("end_thread")).expect("the group is ended");
        assert!(end < group, "the row first, then the group");
        assert!(!s.area.stop_holds(THREAD));
    });
}

// ---- a computer that does not hear --------------------------------------------------------------------------------

// K14: box-stop.test.ts:120, owed-box.test.ts:41
#[test]
fn a_stop_on_a_box_away_owes_the_end_and_holds_the_next_turn() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |o| Ok(Some(if o.away { "the box is away".to_string() } else { "the box answered".to_string() })));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let h = handle(&s, false, true);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "the box is away" }));
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "cap_stopping(sess-1)",
                "release",
                "drop_queued(turn-1)",
                "stop_under(thr-ab12cd34)",
                "try_stopped(turn-1)",
                "interrupt(holds=true)",
                "end_thread(thr-ab12cd34, remove=false, away=true)",
                "end(the box is away, Some(true), Some(true))",
                "end_owed(thr-ab12cd34)",
                "release",
            ]),
            "the agent is still asked to stop, the group is ended away, the row settles, then the owed end is read"
        );
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Interrupted);
        assert_eq!(s.host.hooks.called("place_name_of"), vec!["place_name_of(place-1)".to_string()], "the hold names the computer");
        assert!(s.area.stop_holds(THREAD), "the next turn waits for the end the computer owes");
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD), "and goes once it is paid");
    });
}

// K14: box-stop.test.ts:225 (a second Stop keeps the first Stop's hold)
#[test]
fn a_second_stop_on_the_stopped_turn_keeps_the_owed_hold() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |o| Ok(if o.away { Some("the box is away".to_string()) } else { None }));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let first = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(first), json!({ "outcome": "accepted", "left": "the box is away" }));
        let second = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(second), json!({ "outcome": "not-running" }));
        assert_eq!(h.calls(), strs(&["interrupt"]), "the second stop asks the agent nothing");
        assert!(s.area.stop_holds(THREAD), "the first stop's hold stands through the second");
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K14: a kind that owes nothing, or has no owed end, holds nothing past the stop
#[test]
fn an_away_stop_where_nothing_is_owed_holds_nothing() {
    run_local(async {
        for owed in [None, Some(None)] {
            let s = scene();
            on_box(&s);
            set_link(&s, true);
            set_away(&s, Rc::new(Cell::new(true)));
            group_end(&s, |_o| Ok(Some("the box is away".to_string())));
            owes(&s, owed);
            let h = handle(&s, false, true);
            put(&s, Spec { handle: Some(h), ..spec() });
            let r = s.area.interrupt_row(KEY, None, None).await;
            assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "the box is away" }));
            assert!(!s.area.stop_holds(THREAD));
        }
    });
}

// K14: no words from the away end read as stopped and unreached
#[test]
fn an_away_end_with_no_words_ends_the_row_stopped_and_unreached() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |_o| Ok(None));
        owes(&s, Some(None));
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert!(s.log.borrow().contains(&format!("end({TURN_STOPPED_LINE}, Some(true), Some(true))")), "{:?}", s.log.borrow());
    });
}

// K14, K2: box-stop.test.ts:197 (the box stops answering with its socket still open)
#[test]
fn a_computer_that_stops_answering_takes_the_away_road() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        group_end(&s, |o| Ok(Some(if o.away { "the box is slow".to_string() } else { "the box answered".to_string() })));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let _answer = ping_by_hand(&s);
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h), ..spec() });
        let said = spawn({
            let area = s.area.clone();
            async move { area.interrupt_row(KEY, None, None).await }
        });
        settle().await;
        assert!(said.borrow().is_none(), "the stop waits for the computer for STOP_REACH_MS");
        s.host.clock.advance(STOP_REACH_MS);
        settle().await;
        let r = said.borrow_mut().take().expect("the stop answers once the time is up");
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "the box is slow" }));
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Interrupted);
        assert!(s.area.stop_holds(THREAD));
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K16: box-stop.test.ts:185 (a box that answers is stopped as it was: the agent, then the group, nothing owed)
#[test]
fn a_computer_that_answers_hears_the_stop() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(false)));
        group_end(&s, |o| Ok(Some(if o.away { "the box is away".to_string() } else { "a process still runs".to_string() })));
        owes(&s, Some(Some(done())));
        let answer = ping_by_hand(&s);
        answer(Ok(()));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted", "left": "a process still runs" }));
        let log = s.log.borrow().clone();
        assert!(log.contains(&"end_thread(thr-ab12cd34, remove=false, away=false)".to_string()), "{log:?}");
        assert!(
            !log.iter().any(|l| l.starts_with("end(") || l.starts_with("end_owed")),
            "the turn ends itself and nothing is owed: {log:?}"
        );
        assert_eq!(
            s.host.rows.borrow()[KEY].borrow().view.status,
            SessionStatus::Running,
            "the row is settled by the turn, not by the stop"
        );
        assert!(!s.area.stop_holds(THREAD));
    });
}

// ---- the log and the cap stop ------------------------------------------------------------------------------------

// K17: a cap stop is opened by a stop with no task and released however it ends
#[test]
fn a_stop_with_no_task_opens_a_cap_stop_and_releases_it_whichever_way_it_ends() {
    run_local(async {
        let s = scene();
        let _ = s.area.interrupt_row("sess_nobody", None, None).await;
        assert_eq!(*s.log.borrow(), strs(&["cap_stopping(sess_nobody)", "release"]));

        let s = scene();
        group_end(&s, |_o| Err(fail("the daemon would not answer")));
        put(&s, Spec { status: "completed", ..spec() });
        let r = s.area.interrupt_row(KEY, None, None).await;
        assert!(r.is_err());
        let log = s.log.borrow().clone();
        assert_eq!(log.first(), Some(&"cap_stopping(sess-1)".to_string()));
        assert_eq!(log.last(), Some(&"release".to_string()), "released when the stop failed too");

        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h), ..spec() });
        let _ = s.area.interrupt_row(KEY, None, Some("a1".to_string())).await;
        assert_eq!(s.host.hooks.called("cap_stopping"), Vec::<String>::new(), "a subagent stop opens none");
    });
}

// K18: box-stop.test.ts:102
#[test]
fn every_stop_logs_one_line() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        group_end(&s, |o| Ok(if o.away { Some("the box is away".to_string()) } else { None }));
        owes(&s, Some(Some(done())));
        let h = handle(&s, false, true);
        put(&s, Spec { handle: Some(h), ..spec() });
        let cap = console::capture();
        let first = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(first), json!({ "outcome": "accepted", "left": "the box is away" }));
        let second = s.area.interrupt_row(KEY, None, None).await;
        assert_eq!(answered(second), json!({ "outcome": "not-running" }));
        let third = s.area.interrupt_row("sess_nobody", None, None).await;
        assert_eq!(answered(third), json!({ "outcome": "not-found" }));
        assert_eq!(
            stop_lines(&cap),
            strs(&[
                "stop on thread thr-ab12: accepted (the box is away)",
                "stop on thread thr-ab12: not-running",
                "stop on thread sess_nob: not-found",
            ])
        );
    });
}

// K18: subagents.test.ts:428
#[test]
fn a_task_stop_logs_the_subagent_beside_the_thread() {
    run_local(async {
        let s = scene();
        let h = handle_with_tasks(&s, Ok(TaskStop::Accepted));
        put(&s, Spec { handle: Some(h), ..spec() });
        let cap = console::capture();
        let r = s.area.interrupt_row(KEY, None, Some("task_0123456789".to_string())).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(stop_lines(&cap), strs(&["stop on thread thr-ab12 subagent task_012: accepted"]));
    });
}

// K18: a failure is logged with the outcome `failed: <message>` and is the caller's
#[test]
fn a_failed_stop_logs_it_and_fails() {
    run_local(async {
        let s = scene();
        group_end(&s, |_o| Err(fail("the daemon would not answer")));
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h), ..spec() });
        let cap = console::capture();
        let r = s.area.interrupt_row(KEY, None, None).await;
        let e = r.expect_err("the stop fails");
        assert_eq!(said(&e), ("the daemon would not answer".to_string(), None, None));
        assert_eq!(stop_lines(&cap), strs(&["stop on thread thr-ab12: failed: the daemon would not answer"]), "no `left`, one line");
        assert_eq!(s.log.borrow().last(), Some(&"release".to_string()));
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K18: the thread is the row's, else the id asked; the api's interrupt is this stop
#[test]
fn the_api_interrupt_names_the_thread_of_the_turn_it_was_asked_by() {
    run_local(async {
        let s = scene();
        let h = handle(&s, true, false);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let cap = console::capture();
        let api: &dyn SessionsApi = &s.area;
        let r = api.interrupt(TURN, None, None).await;
        assert_eq!(answered(r), json!({ "outcome": "accepted" }));
        assert_eq!(h.calls(), strs(&["interrupt"]));
        assert_eq!(stop_lines(&cap), strs(&["stop on thread thr-ab12: accepted"]), "asked by the turn id, logged by the row's thread");
    });
}

// ---- a line's try ------------------------------------------------------------------------------------------------

// K19, K14: owed-box.test.ts:41
#[test]
fn a_lines_try_is_ended_with_no_tree_and_no_mark() {
    run_local(async {
        let s = scene();
        on_box(&s);
        set_link(&s, true);
        set_away(&s, Rc::new(Cell::new(true)));
        s.host.hooks.set::<dyn Fn(&Row) -> bool>("SessionsCtx::line_try", Rc::new(|_r: &Row| true));
        group_end(&s, |o| Ok(if o.away { Some("the box is away".to_string()) } else { None }));
        let (paid, pay) = gate();
        owes(&s, Some(Some(paid)));
        let h = handle(&s, false, true);
        probe_holds(&h, &s.area);
        put(&s, Spec { handle: Some(h.clone()), ..spec() });
        let cap = console::capture();
        s.area.stop_try(KEY).await.expect("the try is stopped");
        assert_eq!(
            *s.log.borrow(),
            strs(&[
                "interrupt(holds=true)",
                "end_thread(thr-ab12cd34, remove=false, away=true)",
                "end(the box is away, Some(true), Some(true))",
                "end_owed(thr-ab12cd34)",
            ]),
            "no tree, no cap mark, no try_stopped, no drop of a queued turn"
        );
        assert_eq!(s.host.rows.borrow()[KEY].borrow().view.status, SessionStatus::Interrupted, "the row settles");
        assert!(s.area.stop_holds(THREAD), "the end is owed to the computer's next link");
        assert_eq!(cap.lines(), Vec::<String>::new(), "a line's try is not logged as a stop");
        pay();
        settle().await;
        assert!(!s.area.stop_holds(THREAD));
    });
}

// K19: sessions.ts:1482
#[test]
fn a_lines_try_with_no_row_is_left_alone() {
    run_local(async {
        let s = scene();
        s.area.stop_try("sess_nobody").await.expect("nothing to stop");
        assert_eq!(s.host.hooks.calls(), Vec::<String>::new());
        assert_eq!(*s.log.borrow(), Vec::<String>::new());
    });
}

// K5: owed-box.test.ts (a send waiting on an end owed to a computer that is away is given up by a stop on its row)
#[test]
fn a_send_waiting_on_an_owed_end_is_given_up() {
    run_local(async {
        let s = scene();
        put(&s, Spec { key: "held-turn", turn: "held-turn", handle: None, end: false, ..spec() });
        let given_up = Rc::new(Cell::new(0u32));
        let g = given_up.clone();
        let line = wsp_words::words::computer::send_given_up_line("aws-box");
        let said = line.clone();
        s.area.stops.borrow_mut().held_sends.insert(
            "held-turn".to_string(),
            HeldSend {
                thread_id: THREAD.to_string(),
                give_up: Rc::new(move || {
                    g.set(g.get() + 1);
                    said.clone()
                }),
            },
        );
        let answer = answered(s.area.interrupt_row("held-turn", None, None).await);
        assert_eq!(answer, json!({ "outcome": "accepted", "left": line }));
        assert_eq!(given_up.get(), 1, "the send is given up once");
        assert!(
            !s.log.borrow().iter().any(|c| c.starts_with("end(") || c.starts_with("try_stopped")),
            "nothing running is stopped: {:?}",
            s.log.borrow()
        );
        assert_eq!(s.host.hooks.called("stop_under"), Vec::<String>::new(), "the tree is not walked for a send that ran nothing");

        // With a task named, the held send is not what is asked about.
        let task = answered(s.area.interrupt_row("held-turn", None, Some("a1".into())).await);
        assert_eq!(task, json!({ "outcome": "not-running" }));
        assert_eq!(given_up.get(), 1);
    });
}
