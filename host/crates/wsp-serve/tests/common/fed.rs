// SPDX-License-Identifier: AGPL-3.0-only
//! `fed-runs.ts`: runs of the real Claude adapter fed by hand, with the lines Claude Code 2.1.280 printed (recorded
//! 2026-10-08, trimmed to the fields the adapter reads) and the task line and result origin 2.1.295 printed (recorded
//! 2026-10-09). Every run the adapter launches is fed by hand: its opening line, the lines written into it, and its output.
//! An attach after a host restart reads the run's output again from its first line and the lines it took. A run's next write
//! can lose its answer, after the line landed or before, a write can wait on a gate before it lands, and a run can fail the way
//! a launch on a machine fails once its start has answered.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::FutureExt as _;
use serde_json::{json, Value};
use tokio::sync::Notify;
use wsp_adapters::port::{
    AttachRunOptions, Attached, Env, ExecFailure, ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, SharedFuture, WriteOutcome,
};

use super::Gate;

pub const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";

pub fn init() -> String {
    json!({ "type": "system", "subtype": "init", "cwd": "/work", "session_id": SID, "tools": ["Bash"], "model": "claude-haiku-4-5-20251001", "claude_code_version": "2.1.280", "capabilities": ["interrupt_receipt_v1", "interrupt_cancel_queued_v1", "msg_lifecycle_v1"] }).to_string()
}

pub fn said(id: &str, text: &str) -> String {
    json!({ "type": "assistant", "message": { "model": "claude-haiku-4-5-20251001", "id": id, "type": "message", "role": "assistant", "content": [{ "type": "text", "text": text }], "usage": { "input_tokens": 10, "output_tokens": 300 } }, "parent_tool_use_id": null, "session_id": SID }).to_string()
}

pub fn result(text: &str, turns: u32, origin: Option<&str>) -> String {
    let mut doc = json!({ "type": "result", "subtype": "success", "is_error": false, "duration_ms": 4000, "num_turns": turns, "result": text, "session_id": SID, "total_cost_usd": 0.0128, "usage": { "input_tokens": 10, "output_tokens": 300 } });
    if let Some(kind) = origin {
        doc["origin"] = json!({ "kind": kind });
    }
    doc.to_string()
}

pub fn lifecycle(uuid: &str, state: &str) -> String {
    json!({ "type": "command_lifecycle", "command_uuid": uuid, "state": state, "session_id": SID }).to_string()
}

pub fn tasks(ids: &[&str]) -> String {
    let rows: Vec<Value> = ids.iter().map(|id| json!({ "task_id": id, "task_type": "local_bash", "description": "pnpm exec vitest run" })).collect();
    json!({ "type": "system", "subtype": "background_tasks_changed", "tasks": rows, "session_id": SID }).to_string()
}

pub fn task_done(id: &str) -> String {
    json!({ "type": "system", "subtype": "task_notification", "task_id": id, "status": "completed", "session_id": SID }).to_string()
}

/// What a box's write says when nothing came back after the append, landed or not.
pub const WRITE_LOST: &str = "remote write failed on m1: nothing came back saying WSP_OK, the word the guest prints once the message landed; the machine did not answer";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Loses {
    Landed,
    Lost,
}

/// One stream over a run: the queue its lines are read from, and how it ends.
struct StreamCtl {
    queue: RefCell<VecDeque<String>>,
    wake: Notify,
    ended: Cell<bool>,
    failed: RefCell<Option<ExecFailure>>,
    exit: Rc<Gate>,
    code: Cell<Option<i32>>,
}

impl StreamCtl {
    fn new(replay: &[String]) -> Rc<StreamCtl> {
        Rc::new(StreamCtl {
            queue: RefCell::new(replay.iter().cloned().collect()),
            wake: Notify::new(),
            ended: Cell::new(false),
            failed: RefCell::new(None),
            exit: Gate::new(),
            code: Cell::new(None),
        })
    }
    fn feed(&self, line: &str) {
        self.queue.borrow_mut().push_back(line.to_string());
        self.wake.notify_waiters();
    }
    fn end(&self, code: Option<i32>) {
        if self.ended.get() {
            return;
        }
        self.ended.set(true);
        self.code.set(code);
        self.exit.open();
        self.wake.notify_waiters();
    }
}

pub struct Run {
    pub id: String,
    pub opening: String,
    pub env: Env,
    /// What was written into the run, in order.
    pub writes: RefCell<Vec<String>>,
    pub log: RefCell<Vec<String>>,
    pub loses: Cell<Option<Loses>>,
    pub gate: RefCell<Option<Rc<Gate>>>,
    current: RefCell<Option<Rc<StreamCtl>>>,
}

impl Run {
    /// The lines the run prints, kept in its log and fed to the stream open now.
    pub fn push(&self, lines: &[String]) {
        for line in lines {
            self.log.borrow_mut().push(line.clone());
            if let Some(ctl) = self.current.borrow().as_ref() {
                ctl.feed(line);
            }
        }
    }
    pub fn end(&self, code: Option<i32>) {
        if let Some(ctl) = self.current.borrow().as_ref() {
            ctl.end(code);
        }
    }
    pub fn fail(&self, message: &str) {
        self.fail_with(ExecFailure { kind: ExecFailureKind::Other, message: message.into() });
    }
    pub fn fail_with(&self, failure: ExecFailure) {
        if let Some(ctl) = self.current.borrow().as_ref() {
            *ctl.failed.borrow_mut() = Some(failure);
            ctl.end(None);
        }
    }
    pub fn writes(&self) -> Vec<String> {
        self.writes.borrow().clone()
    }
}

struct Fed {
    run: Rc<Run>,
    ctl: Rc<StreamCtl>,
    taken: Option<Vec<String>>,
}

impl ExecStream for Fed {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let ctl = self.ctl.clone();
        Box::pin(futures::stream::unfold((ctl, false), |(ctl, done)| async move {
            if done {
                return None;
            }
            let waiting = ctl.clone();
            loop {
                let woken = waiting.wake.notified();
                tokio::pin!(woken);
                woken.as_mut().enable();
                let next = ctl.queue.borrow_mut().pop_front();
                if let Some(line) = next {
                    return Some((Ok(line), (ctl, false)));
                }
                let failed = ctl.failed.borrow_mut().take();
                if let Some(failed) = failed {
                    return Some((Err(failed), (ctl, true)));
                }
                if ctl.ended.get() {
                    return None;
                }
                woken.await;
            }
        }))
    }
    fn run(&self) -> Option<String> {
        Some(self.run.id.clone())
    }
    fn teardown(&self) {
        self.ctl.end(Some(143));
    }
    fn kill(&self) {
        self.ctl.end(None);
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        let run = self.run.clone();
        async move {
            let gate = run.gate.borrow().clone();
            if let Some(gate) = gate {
                gate.wait().await;
            }
            let loses = run.loses.take();
            if loses != Some(Loses::Lost) {
                run.writes.borrow_mut().push(line);
            }
            if loses.is_some() {
                return Err(ExecFailure { kind: ExecFailureKind::Other, message: WRITE_LOST.into() });
            }
            Ok(WriteOutcome::Written)
        }
        .boxed_local()
    }
    fn close_input(&self) {}
    fn taken(&self) -> Option<Vec<String>> {
        self.taken.clone()
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        let ctl = self.ctl.clone();
        async move {
            ctl.exit.wait().await;
            ctl.code.get()
        }
        .boxed_local()
        .shared()
    }
}

/// `fedRuns()`: the factory the adapter launches through, and the runs it launched.
pub struct FedRuns {
    pub runs: Rc<RefCell<Vec<Rc<Run>>>>,
}

pub fn fed_runs() -> (Arc<dyn ExecStreamFactory>, FedRuns) {
    let runs: Rc<RefCell<Vec<Rc<Run>>>> = Rc::default();
    (Arc::new(Factory { runs: runs.clone() }), FedRuns { runs })
}

struct Factory {
    runs: Rc<RefCell<Vec<Rc<Run>>>>,
}

impl ExecStreamFactory for Factory {
    fn launch(&self, _command: &str, o: ExecOptions) -> Arc<dyn ExecStream> {
        let id = format!("/tmp/wsp-run/{:012x}", rand::random::<u64>() & 0xffff_ffff_ffff);
        let opening = o.input.as_ref().and_then(|i| i.first().cloned()).unwrap_or_default();
        let run = Rc::new(Run {
            id,
            opening,
            env: o.env,
            writes: RefCell::default(),
            log: RefCell::default(),
            loses: Cell::new(None),
            gate: RefCell::new(None),
            current: RefCell::new(None),
        });
        let ctl = StreamCtl::new(&[]);
        *run.current.borrow_mut() = Some(ctl.clone());
        self.runs.borrow_mut().push(run.clone());
        Arc::new(Fed { run, ctl, taken: None })
    }
    fn attaches(&self) -> bool {
        true
    }
    fn attach(&self, run: &str, _options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        let found = self.runs.borrow().iter().find(|r| r.id == run).cloned();
        Some(
            async move {
                let Some(run) = found else { return Ok(Attached::Gone) };
                let replay = run.log.borrow().clone();
                let ctl = StreamCtl::new(&replay);
                *run.current.borrow_mut() = Some(ctl.clone());
                let mut taken = vec![run.opening.clone()];
                taken.extend(run.writes.borrow().iter().cloned());
                Ok(Attached::Stream(Arc::new(Fed { run, ctl, taken: Some(taken) })))
            }
            .boxed_local(),
        )
    }
}

impl FedRuns {
    pub fn run_of(&self, words: &str) -> Option<Rc<Run>> {
        self.runs.borrow().iter().find(|r| r.opening.contains(words)).cloned()
    }
    /// Every run whose opening carries these words, in launch order.
    pub fn runs_of(&self, words: &str) -> Vec<Rc<Run>> {
        self.runs.borrow().iter().filter(|r| r.opening.contains(words)).cloned().collect()
    }
    /// The id the nth message steered into a run went under.
    pub fn steered(&self, words: &str, n: usize) -> String {
        let run = self.run_of(words).expect("the run");
        let write = run.writes.borrow()[n].clone();
        serde_json::from_str::<Value>(&write).expect("a json line")["uuid"].as_str().expect("a uuid").to_string()
    }
}
