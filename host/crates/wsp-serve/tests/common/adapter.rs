// SPDX-License-Identifier: AGPL-3.0-only
//! The fake harness adapters the TS whole-runtime tests hand `createRuntime({ adapters })`: `held` from
//! runtime-fixture.ts:109 (a turn that runs until the test ends it), the never-settling turn of runtime-gone.test.ts:12,
//! and `agent` of fork.test.ts:63 (a turn that answers at once, forks a session by copy or by start). Each adapter is
//! in-process, as the TS ones are: the runtime's own reading is under test, not a binary.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AdapterEvent, AttachmentRoad, ExecFailure, ForkTurn, ForkedSession, HarnessAdapter, HarnessSession, HarnessStartOptions,
    OnEvent, SharedFuture, SteerOutcome, TurnImage, TurnResult,
};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};

/// An adapter event from its JSON as the TS writes it.
pub fn event(doc: Value) -> AdapterEvent {
    serde_json::from_value(doc).unwrap_or_else(|e| panic!("not an adapter event: {e}"))
}

/// A turn result from its JSON as the TS writes it.
pub fn result(doc: Value) -> TurnResult {
    serde_json::from_value(doc).unwrap_or_else(|e| panic!("not a turn result: {e}"))
}

/// What a start was asked, the fields a test reads of `HarnessStartOptions` (which holds closures and cannot be kept).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Started {
    pub prompt: String,
    pub resume: Option<String>,
    pub copy: bool,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub permission_mode: Option<String>,
    /// `fork`: the session and the turn the start forks at.
    pub fork: Option<(String, String)>,
    pub images: usize,
}

impl Started {
    pub fn of(o: &HarnessStartOptions) -> Started {
        Started {
            prompt: o.prompt.clone(),
            resume: o.resume.clone(),
            copy: o.copy,
            cwd: o.cwd.clone(),
            model: o.model.clone(),
            effort: o.effort.clone(),
            permission_mode: o.permission_mode.clone(),
            fork: o.fork.clone(),
            images: o.images.len(),
        }
    }
}

fn shared<T: Clone + 'static>(rx: oneshot::Receiver<T>) -> SharedFuture<T> {
    let fut: LocalBoxFuture<'static, T> = async move { rx.await.expect("the turn's end was never sent") }.boxed_local();
    fut.shared()
}

struct Turn {
    session_id: String,
    on_event: OnEvent,
    finish: Rc<RefCell<Option<oneshot::Sender<TurnResult>>>>,
}

/// runtime-fixture.ts:109 `held(steers)`: a harness whose every turn runs until the test ends it; a resumed start
/// keeps the session id, as the real one does, and steers when told to.
pub struct Held {
    steers: bool,
    pub starts: RefCell<Vec<Started>>,
    /// The launch environment of each turn, in the order they were launched.
    pub envs: RefCell<Vec<wsp_adapters::port::Env>>,
    pub steered: RefCell<Vec<String>>,
    turns: RefCell<Vec<Turn>>,
    results: RefCell<Vec<Option<TurnResult>>>,
    seq: Cell<u32>,
}

impl Held {
    pub fn new(steers: bool) -> Rc<Held> {
        Rc::new(Held {
            steers,
            starts: RefCell::default(),
            envs: RefCell::default(),
            steered: RefCell::default(),
            turns: RefCell::default(),
            results: RefCell::default(),
            seq: Cell::new(0),
        })
    }

    /// The factory a runtime's `adapters` takes.
    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let held = self.clone();
        Arc::new(move |ctx: HarnessAdapterContext| {
            Ok(Arc::new(HeldAdapter { held: held.clone(), env: ctx.env.clone() }) as Arc<dyn HarnessAdapter>)
        })
    }

    pub fn turns(&self) -> usize {
        self.turns.borrow().len()
    }

    /// The harness's result lands while its process keeps running.
    pub fn reply(&self, turn: usize, text: &str, more: Value) {
        let mut doc = json!({ "status": "completed", "text": text });
        if let (Some(doc), Some(more)) = (doc.as_object_mut(), more.as_object()) {
            doc.extend(more.clone());
        }
        let r = result(doc);
        let turns = self.turns.borrow();
        let t = &turns[turn];
        let mut results = self.results.borrow_mut();
        if results.len() <= turn {
            results.resize(turn + 1, None);
        }
        results[turn] = Some(r.clone());
        (t.on_event)(event(
            json!({ "type": "turn.done", "sessionId": t.session_id, "result": serde_json::to_value(&r).expect("a result") }),
        ));
    }

    /// The process exits, after its reply.
    pub fn exit(&self, turn: usize) {
        let (on_event, session_id, finish) = {
            let turns = self.turns.borrow();
            let t = &turns[turn];
            (t.on_event.clone(), t.session_id.clone(), t.finish.clone())
        };
        on_event(event(json!({ "type": "session.end", "sessionId": session_id, "exitCode": 0, "sawResult": true })));
        let r = self.results.borrow()[turn].clone().expect("a reply came first");
        let tx = finish.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(r);
        }
    }

    pub fn end(&self, turn: usize, text: &str, more: Value) {
        self.reply(turn, text, more);
        self.exit(turn);
    }
}

struct HeldAdapter {
    held: Rc<Held>,
    env: wsp_adapters::port::Env,
}

impl HarnessAdapter for HeldAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let held = &self.held;
        held.starts.borrow_mut().push(Started::of(&o));
        held.envs.borrow_mut().push(self.env.clone());
        held.seq.set(held.seq.get() + 1);
        let session_id = o.resume.clone().unwrap_or_else(|| format!("00000000-0000-4000-8000-{:012}", held.seq.get()));
        let (tx, rx) = oneshot::channel();
        let on_event = o.on_event.clone().expect("an event sink");
        held.turns.borrow_mut().push(Turn {
            session_id: session_id.clone(),
            on_event: on_event.clone(),
            finish: Rc::new(RefCell::new(Some(tx))),
        });
        on_event(event(json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5" })));
        Ok(Arc::new(HeldSession { held: held.clone(), local_id: session_id, finished: shared(rx) }))
    }
    fn steers(&self) -> bool {
        self.held.steers
    }
}

struct HeldSession {
    held: Rc<Held>,
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for HeldSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        prompt: String,
        _id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        if !self.held.steers {
            return None;
        }
        self.held.steered.borrow_mut().push(prompt);
        Some(async { Ok(SteerOutcome::Accepted) }.boxed_local())
    }
}

/// runtime-gone.test.ts:12 `held`: a turn that announces itself and never settles on its own, so only the runtime
/// ending it ends it.
pub fn never_ends() -> HarnessAdapterFactory {
    Arc::new(|_ctx| Ok(Arc::new(NeverEnds(Cell::new(0))) as Arc<dyn HarnessAdapter>))
}

struct NeverEnds(Cell<u32>);

impl HarnessAdapter for NeverEnds {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.0.set(self.0.get() + 1);
        let session_id = format!("11111111-0000-4000-8000-{:012}", self.0.get());
        let on_event = o.on_event.clone().expect("an event sink");
        on_event(event(json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5", "cwd": "/root/work" })));
        let never: LocalBoxFuture<'static, TurnResult> = futures::future::pending().boxed_local();
        Ok(Arc::new(Quiet { local_id: session_id, finished: never.shared() }))
    }
    fn steers(&self) -> bool {
        false
    }
}

struct Quiet {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Quiet {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

/// `sessionOf(prefix, n)` of fork.test.ts:49.
pub fn session_of(prefix: &str, n: usize) -> String {
    format!("{prefix}-0000-4000-8000-{n:012}")
}

/// What a fake agent was asked: each start, each fork it readied, each copy it dropped.
#[derive(Default)]
pub struct Asked {
    pub starts: RefCell<Vec<Started>>,
    /// `{ session, turn }` of each `forkSession` call; `turn` is `{ anchor }` or `{ after }` with the count.
    pub forks: RefCell<Vec<(String, ForkAt)>>,
    pub drops: Rc<RefCell<Vec<String>>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ForkAt {
    Anchor(String),
    After(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Road {
    /// Forks onto a copy of the session that its start resumes, as Claude Code does.
    Copy,
    /// Hands the start the turn and the start forks, answering a new session, as Codex does.
    Start,
}

/// fork.test.ts:63 `agent`'s options.
#[derive(Clone, Default)]
pub struct AgentOptions {
    pub refuse: Option<String>,
    pub anchorless: Vec<u32>,
    pub hang_from: Option<u32>,
    /// One counter shared by every agent of a runtime, so each turn has its own number.
    pub counter: Option<Rc<Cell<u32>>>,
}

/// An agent whose every turn announces its session, says `reply <n>`, names its end `a<n>` and replies at once.
pub fn agent(road: Road, asked: &Rc<Asked>, o: AgentOptions) -> HarnessAdapterFactory {
    let counter = o.counter.clone().unwrap_or_default();
    let asked = asked.clone();
    Arc::new(move |_ctx| {
        Ok(Arc::new(Scripted { road, asked: asked.clone(), o: o.clone(), counter: counter.clone() }) as Arc<dyn HarnessAdapter>)
    })
}

struct Scripted {
    road: Road,
    asked: Rc<Asked>,
    o: AgentOptions,
    counter: Rc<Cell<u32>>,
}

impl HarnessAdapter for Scripted {
    fn steers(&self) -> bool {
        false
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        Some(AttachmentRoad::Inline)
    }
    fn forks(&self) -> bool {
        true
    }
    fn forks_by_count(&self) -> bool {
        self.road == Road::Start
    }
    fn fork_session(&self, session: String, turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        let at = match &turn {
            ForkTurn::Anchor(anchor) => ForkAt::Anchor(anchor.clone()),
            ForkTurn::After(after) => ForkAt::After(after.len()),
        };
        self.asked.forks.borrow_mut().push((session.clone(), at));
        let refuse = self.o.refuse.clone();
        let road = self.road;
        let nth = self.asked.forks.borrow().len();
        let drops = self.asked.drops.clone();
        Some(
            async move {
                if let Some(refused) = refuse {
                    return Err(refused);
                }
                if road == Road::Start {
                    let turn = match turn {
                        ForkTurn::Anchor(anchor) => anchor,
                        ForkTurn::After(after) => format!("counted-{}", after.len()),
                    };
                    return Ok(ForkedSession::Fork { session, turn });
                }
                let copy = session_of("c0c0c0c0", nth);
                let dropped = copy.clone();
                Ok(ForkedSession::Resume {
                    resume: copy,
                    drop: Box::new(move || {
                        async move {
                            drops.borrow_mut().push(dropped);
                            Ok(())
                        }
                        .boxed_local()
                    }),
                })
            }
            .boxed_local(),
        )
    }
    fn start(&self, s: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.counter.set(self.counter.get() + 1);
        let n = self.counter.get();
        self.asked.starts.borrow_mut().push(Started::of(&s));
        let session = if s.fork.is_some() {
            session_of("f0f0f0f0", n as usize)
        } else {
            s.resume.clone().unwrap_or_else(|| session_of("50505050", n as usize))
        };
        let on_event = s.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        if self.o.hang_from.is_some_and(|from| n >= from) {
            let announced = session.clone();
            let emit = on_event.clone();
            tokio::task::spawn_local(async move {
                tokio::task::yield_now().await;
                emit(event(json!({ "type": "session.start", "sessionId": announced })));
            });
            let stop = Rc::new(RefCell::new(Some(tx)));
            return Ok(Arc::new(Hanging { local_id: session, finished: shared(rx), stop, on_event }));
        }
        let anchorless = self.o.anchorless.contains(&n);
        let model = s.model.clone().unwrap_or_else(|| "default-model".into());
        let session_id = session.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            let done = result(json!({ "status": "completed", "text": format!("reply {n}") }));
            on_event(event(json!({ "type": "session.start", "sessionId": session_id, "model": model })));
            on_event(event(json!({ "type": "turn.delta", "sessionId": session_id, "kind": "text", "text": format!("reply {n}") })));
            if !anchorless {
                on_event(event(json!({ "type": "turn.anchor", "sessionId": session_id, "anchor": format!("a{n}") })));
            }
            on_event(event(
                json!({ "type": "turn.done", "sessionId": session_id, "result": serde_json::to_value(&done).expect("a result") }),
            ));
            on_event(event(json!({ "type": "session.end", "sessionId": session_id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(done);
        });
        Ok(Arc::new(Quick { local_id: session, finished: shared(rx) }))
    }
}

struct Quick {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for Quick {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

struct Hanging {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    stop: Rc<RefCell<Option<oneshot::Sender<TurnResult>>>>,
    on_event: OnEvent,
}

impl HarnessSession for Hanging {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let (session, on_event, stop) = (self.local_id.clone(), self.on_event.clone(), self.stop.clone());
        async move {
            let interrupted = result(json!({ "status": "interrupted" }));
            on_event(event(
                json!({ "type": "turn.done", "sessionId": session, "result": serde_json::to_value(&interrupted).expect("a result") }),
            ));
            on_event(event(json!({ "type": "session.end", "sessionId": session, "exitCode": null, "sawResult": false })));
            let tx = stop.borrow_mut().take();
            if let Some(tx) = tx {
                let _ = tx.send(interrupted);
            }
        }
        .boxed_local()
    }
}
