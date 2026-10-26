// SPDX-License-Identifier: AGPL-3.0-only
//! A harness session and an adapter a test drives by hand: the session finishes when the test says, and records
//! every call made on it.
#![allow(dead_code)]
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture, Shared};
use wsp_adapters::port::*;
use wsp_wire::views::session_events::TurnResult;

/// What a `FakeSession` was asked, in order.
#[derive(Default)]
pub struct SessionLog {
    pub calls: Vec<String>,
}

pub struct FakeSession {
    pub id: String,
    pub run: Option<String>,
    pub pid: Option<u32>,
    pub from: Option<u64>,
    finished: Shared<LocalBoxFuture<'static, TurnResult>>,
    finish: Mutex<Option<oneshot::Sender<Result<TurnResult, ()>>>>,
    pub log: Mutex<SessionLog>,
    /// Which optional roads the session has.
    pub steers: bool,
    pub answers: bool,
    pub sets_access: bool,
    pub stops_tasks: bool,
    pub tells_unread: Option<bool>,
    /// What `steer`, `answer`, `set_access` and `stop_task` answer.
    pub steer_answer: Mutex<Result<SteerOutcome, String>>,
    pub answer_answer: Mutex<AnswerOutcome>,
    pub access_answer: Mutex<AccessOutcome>,
    pub task_answer: Mutex<TaskStop>,
    pub kept: Mutex<Option<Box<dyn KeptAgent>>>,
}

impl FakeSession {
    /// A session with no optional road; `finished` waits for `finish` or `fail`.
    pub fn new(id: &str) -> Arc<Self> {
        Self::with(id, |_| {})
    }

    pub fn with(id: &str, set: impl FnOnce(&mut FakeSession)) -> Arc<Self> {
        let (tx, rx) = oneshot::channel::<Result<TurnResult, ()>>();
        // A failed finish reads as the TS promise's rejection; `TurnResult` has no error road, so the session's
        // `finished` answers a failed result and `failed()` tells the two apart for the test.
        let finished = rx
            .map(|r| match r {
                Ok(Ok(result)) => result,
                _ => serde_json::from_value(serde_json::json!({ "status": "failed" })).expect("a failed result"),
            })
            .boxed_local()
            .shared();
        let mut s = FakeSession {
            id: id.to_string(),
            run: None,
            pid: None,
            from: None,
            finished,
            finish: Mutex::new(Some(tx)),
            log: Mutex::new(SessionLog::default()),
            steers: false,
            answers: false,
            sets_access: false,
            stops_tasks: false,
            tells_unread: None,
            steer_answer: Mutex::new(Ok(SteerOutcome::Accepted)),
            answer_answer: Mutex::new(AnswerOutcome::Answered),
            access_answer: Mutex::new(AccessOutcome::Set),
            task_answer: Mutex::new(TaskStop::Accepted),
            kept: Mutex::new(None),
        };
        set(&mut s);
        Arc::new(s)
    }

    pub fn finish(&self, result: TurnResult) {
        if let Some(tx) = self.finish.lock().unwrap().take() {
            let _ = tx.send(Ok(result));
        }
    }

    pub fn fail(&self) {
        if let Some(tx) = self.finish.lock().unwrap().take() {
            let _ = tx.send(Err(()));
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().calls.clone()
    }

    fn note(&self, call: String) {
        self.log.lock().unwrap().calls.push(call);
    }
}

impl HarnessSession for FakeSession {
    fn local_id(&self) -> String {
        self.id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn run(&self) -> Option<String> {
        self.run.clone()
    }
    fn pid(&self) -> Option<u32> {
        self.pid
    }
    fn from(&self) -> Option<u64> {
        self.from
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.note("interrupt".into());
        futures::future::ready(()).boxed_local()
    }
    fn kept(&self) -> Option<Option<Box<dyn KeptAgent>>> {
        Some(self.kept.lock().unwrap().take())
    }
    fn steer(
        &self,
        prompt: String,
        id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        if !self.steers {
            return None;
        }
        self.note(format!("steer({prompt}, {id:?})"));
        let answer = self.steer_answer.lock().unwrap().clone().map_err(|message| ExecFailure { kind: ExecFailureKind::Other, message });
        Some(futures::future::ready(answer).boxed_local())
    }
    fn tells_unread(&self) -> Option<bool> {
        self.tells_unread
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        if !self.answers {
            return None;
        }
        self.note(format!(
            "answer({ask_id}, {}, {:?}, {}, {:?})",
            answer.option_id,
            answer.outcome,
            answer.deny_message,
            answer.reason.unwrap_or_default()
        ));
        Some(futures::future::ready(Ok(*self.answer_answer.lock().unwrap())).boxed_local())
    }
    fn set_access(&self, mode: String) -> Option<LocalBoxFuture<'static, Result<AccessOutcome, ExecFailure>>> {
        if !self.sets_access {
            return None;
        }
        self.note(format!("set_access({mode})"));
        Some(futures::future::ready(Ok(*self.access_answer.lock().unwrap())).boxed_local())
    }
    fn stop_task(&self, task: String) -> Option<LocalBoxFuture<'static, Result<TaskStop, ExecFailure>>> {
        if !self.stops_tasks {
            return None;
        }
        self.note(format!("stop_task({task})"));
        Some(futures::future::ready(Ok(self.task_answer.lock().unwrap().clone())).boxed_local())
    }
}

/// A turn result from its JSON (`status` required).
pub fn result(doc: serde_json::Value) -> TurnResult {
    serde_json::from_value(doc).expect("a turn result the fixture builds")
}

/// An adapter event from its JSON, as the adapters emit them.
pub fn adapter_event(doc: serde_json::Value) -> AdapterEvent {
    serde_json::from_value(doc).expect("an adapter event the fixture builds")
}
