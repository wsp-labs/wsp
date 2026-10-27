// SPDX-License-Identifier: AGPL-3.0-only
//! `machineRuns` of runtime-comeback.test.ts:30 at 65f6b89c7: a harness whose run lives on the machine, not in this
//! process. Every event it emits is a line of the run's log, and an attach replays that log from its first line before
//! the rest of it arrives, which is what reading the guest's own log from byte zero does. Forgetting a run is the
//! machine having swept it.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_adapters::port::{
    AdapterAttachOptions, AdapterError, AdapterEvent, Env, ExecFailure, ExecFailureKind, HarnessAdapter, HarnessExec, HarnessSession,
    HarnessStartOptions, OnEvent, SharedFuture, TitleTurn, TurnResult,
};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};

use super::adapter::{event, result};
use super::Gate;

struct Run {
    log: Vec<AdapterEvent>,
    session_id: String,
    local_id: String,
    live: Option<OnEvent>,
    settle: Option<oneshot::Sender<TurnResult>>,
    result: Option<TurnResult>,
}

/// What each attach was handed.
#[derive(Clone, Debug, PartialEq)]
pub struct Reopened {
    pub prompt: Option<String>,
    pub effort: Option<String>,
}

pub struct MachineRuns {
    named: bool,
    runs: RefCell<IndexMap<String, Rc<RefCell<Run>>>>,
    minted: Cell<u32>,
    unreachable: RefCell<Option<String>>,
    attempts: Cell<u32>,
    asked: RefCell<Vec<String>>,
    reopened: RefCell<Vec<Reopened>>,
    /// The launch environment of each turn this fixture started, in order; an attach after a restart adds none.
    pub envs: RefCell<Vec<Env>>,
    /// The message each turn this fixture started was handed, in order.
    pub prompts: RefCell<Vec<String>>,
}

impl MachineRuns {
    pub fn new() -> Rc<MachineRuns> {
        MachineRuns::build(false)
    }

    /// `namedAtLaunch`: the harness names its session at the launch and a resume keeps it, as Claude Code does.
    pub fn named_at_launch() -> Rc<MachineRuns> {
        MachineRuns::build(true)
    }

    fn build(named: bool) -> Rc<MachineRuns> {
        Rc::new(MachineRuns {
            named,
            runs: RefCell::default(),
            minted: Cell::new(0),
            unreachable: RefCell::default(),
            attempts: Cell::new(0),
            asked: RefCell::default(),
            reopened: RefCell::default(),
            envs: RefCell::default(),
            prompts: RefCell::default(),
        })
    }

    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        self.holding_for(None, false)
    }

    /// `{ ...inner, attach: async o => (await answered, inner.attach(o)) }` of runtime-comeback.test.ts:497: the harness
    /// whose attach waits for `gate`; with `title` too, its read of the agent's own store (`sessionTitle`) waits for it
    /// and then runs one command, as the TS case at :523 does.
    pub fn holding(self: &Rc<Self>, gate: &Rc<Gate>, title: bool) -> HarnessAdapterFactory {
        self.holding_for(Some(Holder::at(Some(gate.clone()))), title)
    }

    /// The same, with a gate the test sets later and a count of the attaches asked, as the TS case at :861 keeps
    /// (`asked++` before `await gate`).
    pub fn gated(self: &Rc<Self>, holder: &Rc<Holder>) -> HarnessAdapterFactory {
        self.holding_for(Some(holder.clone()), false)
    }

    fn holding_for(self: &Rc<Self>, hold: Option<Rc<Holder>>, title: bool) -> HarnessAdapterFactory {
        let runs = self.clone();
        Arc::new(move |ctx: HarnessAdapterContext| {
            Ok(Arc::new(RunsAdapter { runs: runs.clone(), env: ctx.env.clone(), hold: hold.clone(), hold_title: title })
                as Arc<dyn HarnessAdapter>)
        })
    }

    /// `emit(handle, event)`: one more line of the run's log, delivered to whoever reads it now.
    pub fn emit(&self, handle: &str, doc: Value) {
        let run = self.runs.borrow().get(handle).cloned().expect("a run under that handle");
        let ev = event(doc);
        run.borrow_mut().log.push(ev.clone());
        deliver(&run, ev);
    }

    pub fn handles(&self) -> Vec<String> {
        self.runs.borrow().keys().cloned().collect()
    }

    /// `sweep(handle)`: the machine forgot the run.
    pub fn sweep(&self, handle: &str) {
        self.runs.borrow_mut().shift_remove(handle);
    }

    /// `unreach(e)`: nothing on the machine answers the question the attach asks, until `None`.
    pub fn unreach(&self, message: Option<&str>) {
        *self.unreachable.borrow_mut() = message.map(str::to_string);
    }

    pub fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }

    pub fn reopened(&self) -> Vec<Reopened> {
        self.reopened.borrow().clone()
    }

    pub fn attempts(&self) -> u32 {
        self.attempts.get()
    }
}

fn deliver(run: &Rc<RefCell<Run>>, ev: AdapterEvent) {
    if let AdapterEvent::TurnDone { result, .. } = &ev {
        run.borrow_mut().result = Some(result.clone());
    }
    let live = run.borrow().live.clone();
    if let Some(live) = live {
        live(ev.clone());
    }
    if matches!(ev, AdapterEvent::SessionEnd { .. }) {
        let (tx, r) = {
            let mut r = run.borrow_mut();
            (r.settle.take(), r.result.clone())
        };
        if let Some(tx) = tx {
            let _ = tx.send(r.unwrap_or_else(|| result(serde_json::json!({ "status": "failed" }))));
        }
    }
}

struct RunsAdapter {
    runs: Rc<MachineRuns>,
    env: Env,
    hold: Option<Rc<Holder>>,
    hold_title: bool,
}

/// What an attach waits for: a gate that may be set after the harness is built, and every attach asked.
#[derive(Default)]
pub struct Holder {
    pub asked: Cell<u32>,
    pub gate: RefCell<Option<Rc<Gate>>>,
}

impl Holder {
    pub fn at(gate: Option<Rc<Gate>>) -> Rc<Holder> {
        Rc::new(Holder { asked: Cell::new(0), gate: RefCell::new(gate) })
    }

    /// `gate = new Promise(...)`.
    pub fn hold(&self, gate: &Rc<Gate>) {
        *self.gate.borrow_mut() = Some(gate.clone());
    }
}

fn open(run: &Rc<RefCell<Run>>, handle: &str, on_event: OnEvent, local_id: String) -> Arc<dyn HarnessSession> {
    let (tx, rx) = oneshot::channel();
    {
        let mut r = run.borrow_mut();
        r.settle = Some(tx);
        r.live = Some(on_event);
    }
    // A reader the machine replaced (a later attach took the run) never settles, as a TS promise nobody resolves.
    let finished: LocalBoxFuture<'static, TurnResult> = async move {
        match rx.await {
            Ok(r) => r,
            Err(_) => futures::future::pending().await,
        }
    }
    .boxed_local();
    Arc::new(RunSession { local_id, run: handle.to_string(), finished: finished.shared() })
}

impl HarnessAdapter for RunsAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let m = &self.runs;
        m.envs.borrow_mut().push(self.env.clone());
        m.prompts.borrow_mut().push(o.prompt.clone());
        m.minted.set(m.minted.get() + 1);
        let n = m.minted.get();
        // The shape a handle the guest's run directory reports has to have to be one of this host's.
        let handle = format!("/tmp/wsp-run/{n:012x}");
        // The CLI keys the session by its own id, not by the one the launch minted, so the row and the harness session
        // are two different ids across the restart.
        let session_id = o.resume.clone().unwrap_or_else(|| format!("sess-{n}"));
        let local_id = if m.named { session_id.clone() } else { format!("local-{n}") };
        let run = Rc::new(RefCell::new(Run {
            log: vec![],
            session_id: session_id.clone(),
            local_id: local_id.clone(),
            live: None,
            settle: None,
            result: None,
        }));
        m.runs.borrow_mut().insert(handle.clone(), run.clone());
        let on_event = o.on_event.clone().expect("an event sink");
        let session = open(&run, &handle, on_event, local_id);
        let cwd = o.cwd.clone().unwrap_or_else(|| "/root/work".into());
        let runs = m.clone();
        tokio::task::spawn_local(async move {
            runs.emit(
                &handle,
                serde_json::json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5", "cwd": cwd }),
            );
        });
        Ok(session)
    }

    fn steers(&self) -> bool {
        false
    }

    fn attaches(&self) -> bool {
        true
    }

    fn attach(&self, o: AdapterAttachOptions) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        let m = self.runs.clone();
        let hold = self.hold.clone();
        Some(
            async move {
                if let Some(hold) = hold {
                    hold.asked.set(hold.asked.get() + 1);
                    let gate = hold.gate.borrow().clone();
                    if let Some(gate) = gate {
                        gate.wait().await;
                    }
                }
                m.attempts.set(m.attempts.get() + 1);
                if let Some(message) = m.unreachable.borrow().clone() {
                    return Err(ExecFailure { kind: ExecFailureKind::Other, message });
                }
                m.reopened.borrow_mut().push(Reopened { prompt: o.prompt.clone(), effort: o.effort.clone() });
                // A machine that answered and no longer holds the run: no reader, and nothing is ever emitted for it.
                let Some(run) = m.runs.borrow().get(&o.run).cloned() else { return Ok(None) };
                let session = open(&run, &o.run, o.on_event.clone(), o.session_id.clone());
                let replayed = run.borrow().log.clone();
                tokio::task::spawn_local(async move {
                    for ev in replayed {
                        deliver(&run, ev);
                    }
                });
                Ok(Some(session))
            }
            .boxed_local(),
        )
    }

    fn session_title(
        &self,
        _id: String,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let gate = self.hold.clone().filter(|_| self.hold_title)?.gate.borrow().clone()?;
        Some(
            async move {
                gate.wait().await;
                exec.exec("true", None).await?;
                Ok(None)
            }
            .boxed_local(),
        )
    }

    // Answering nothing leaves the row on its seed, so the only thing that can stop a second question after the restart
    // is the start row the turn already wrote.
    fn title_for(
        &self,
        turn: TitleTurn,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        self.runs.asked.borrow_mut().push(turn.opening);
        Some(async { Ok(None) }.boxed_local())
    }
}

struct RunSession {
    local_id: String,
    run: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for RunSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn run(&self) -> Option<String> {
        Some(self.run.clone())
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}
