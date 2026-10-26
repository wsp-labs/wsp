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
    s.host.hooks.set::<dyn Fn(&str)>("SessionsCtx::try_stopped", Rc::new(move |turn: &str| say(&l, format!("try_stopped({turn})"))));
    let l = s.log.clone();
    s.host.hooks.set::<dyn Fn(&str, &str)>(
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
