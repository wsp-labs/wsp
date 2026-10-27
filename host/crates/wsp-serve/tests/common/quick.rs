// SPDX-License-Identifier: AGPL-3.0-only
//! The TS tests' harness that answers each turn at once and records what it was started with (`harness(starts)` of
//! folder-threads.test.ts, worktree-sweep.test.ts and the like): a resume keeps its session, every new session is numbered on one
//! count the harnesses share, and a test may hold its turns until it lets them go.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::json;
use wsp_adapters::port::{
    AdapterError, AttachmentRoad, ExecFailure, ForkTurn, ForkedSession, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture,
    SteerOutcome, TurnImage, TurnResult,
};
use wsp_core::types::harness::HarnessAdapterFactory;

use super::adapter::{event, result};

pub use super::live::LiveStart;

/// What differs between the TS tests' copies of this harness.
#[derive(Clone)]
pub struct QuickOptions {
    /// The first four groups of every session id it mints; the last group is the count.
    pub session_prefix: &'static str,
    /// Says `session.start` as a start returns even where no turn is held.
    pub announces: bool,
    /// Names each turn's end `a<n>`.
    pub anchors: bool,
    /// `forkSession` readies a copy that resumes the session it was given.
    pub copies: bool,
    /// Takes images inline.
    pub inline: bool,
}

impl Default for QuickOptions {
    fn default() -> QuickOptions {
        QuickOptions { session_prefix: "55555555-5555-4555-8555", announces: false, anchors: true, copies: false, inline: false }
    }
}

pub struct Quick {
    o: QuickOptions,
    pub starts: RefCell<Vec<LiveStart>>,
    n: Cell<u32>,
    /// Turns are held until `release`, where this is set.
    pub holding: Cell<bool>,
    waiting: RefCell<Vec<oneshot::Sender<()>>>,
}

impl Quick {
    pub fn new(o: QuickOptions) -> Rc<Quick> {
        Rc::new(Quick { o, starts: RefCell::default(), n: Cell::new(0), holding: Cell::new(false), waiting: RefCell::default() })
    }

    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let quick = self.clone();
        Arc::new(move |_ctx| Ok(Arc::new(QuickAdapter { quick: quick.clone() }) as Arc<dyn HarnessAdapter>))
    }

    /// `holding.waiting.splice(0).forEach(go => go())`.
    pub fn release(&self) {
        for go in self.waiting.borrow_mut().drain(..) {
            let _ = go.send(());
        }
    }

    pub fn cwds(&self) -> Vec<Option<String>> {
        self.starts.borrow().iter().map(|s| s.cwd.clone()).collect()
    }
}

struct QuickAdapter {
    quick: Rc<Quick>,
}

impl HarnessAdapter for QuickAdapter {
    fn steers(&self) -> bool {
        false
    }
    fn forks(&self) -> bool {
        self.quick.o.copies
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.quick.o.inline.then_some(AttachmentRoad::Inline)
    }
    fn fork_session(&self, session: String, _turn: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        if !self.quick.o.copies {
            return None;
        }
        Some(async move { Ok(ForkedSession::Resume { resume: session, drop: Box::new(|| async { Ok(()) }.boxed_local()) }) }.boxed_local())
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let q = &self.quick;
        q.n.set(q.n.get() + 1);
        let n = q.n.get();
        q.starts.borrow_mut().push(LiveStart {
            prompt: o.prompt.clone(),
            resume: o.resume.clone(),
            cwd: o.cwd.clone(),
            images: o.images.clone(),
        });
        let session_id = if q.o.announces {
            format!("{}-{n:012}", q.o.session_prefix)
        } else {
            o.resume.clone().unwrap_or_else(|| format!("{}-{n:012}", q.o.session_prefix))
        };
        let on_event = o.on_event.clone().expect("an event sink");
        let held = q.holding.get();
        let (go, wait) = oneshot::channel::<()>();
        if held {
            q.waiting.borrow_mut().push(go);
        }
        if held || q.o.announces {
            on_event(event(json!({ "type": "session.start", "sessionId": session_id })));
        }
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let anchors = q.o.anchors;
        let id = session_id.clone();
        tokio::task::spawn_local(async move {
            if held {
                let _ = wait.await;
            } else {
                tokio::task::yield_now().await;
            }
            let done = result(json!({ "status": "completed", "text": "ok" }));
            on_event(event(json!({ "type": "session.start", "sessionId": id })));
            if anchors {
                on_event(event(json!({ "type": "turn.anchor", "sessionId": id, "anchor": format!("a{n}") })));
            }
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&done).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(done);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(QuickSession { local_id: session_id, finished: finished.shared() }))
    }
}

struct QuickSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for QuickSession {
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
        _prompt: String,
        _id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        None
    }
}
