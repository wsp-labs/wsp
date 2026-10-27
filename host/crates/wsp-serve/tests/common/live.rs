// SPDX-License-Identifier: AGPL-3.0-only
//! The TS tests' hand-driven harness (`steerable` of runtime-steer.test.ts, `harness` of steer-images.test.ts and the
//! like): an adapter whose turns run until the test ends them, that keeps what each start and each steer was handed,
//! answers a steer as told, and lets the test say any adapter event of a turn, in process as the TS ones are.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AttachmentRoad, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, SteerOutcome,
    TurnImage, TurnResult,
};
use wsp_core::types::harness::HarnessAdapterFactory;

use super::adapter::{event, result};

/// What a start was handed, the fields these tests read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveStart {
    pub prompt: String,
    pub resume: Option<String>,
    pub cwd: Option<String>,
    pub images: Vec<TurnImage>,
}

/// What a steer was handed: its prompt and the images it carried (none where it carried none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LiveSteer {
    pub prompt: String,
    pub images: Vec<TurnImage>,
}

struct Turn {
    session_id: String,
    on_event: OnEvent,
    finish: Rc<RefCell<Option<oneshot::Sender<TurnResult>>>>,
}

/// The options of a live harness: which of the adapter's abilities it has, and whether a start announces itself.
#[derive(Clone)]
pub struct LiveOptions {
    pub steers: bool,
    pub steers_images: bool,
    pub inline: bool,
    /// Says `session.start` itself as a start returns, as `held` does; `steerable` leaves it to the test.
    pub announces: bool,
    /// One session id for every turn, where the TS fixes it.
    pub session: Option<String>,
}

impl Default for LiveOptions {
    fn default() -> LiveOptions {
        LiveOptions { steers: true, steers_images: false, inline: false, announces: true, session: None }
    }
}

pub struct Live {
    o: LiveOptions,
    pub starts: RefCell<Vec<LiveStart>>,
    pub steered: RefCell<Vec<LiveSteer>>,
    /// What a steer answers: accepted, or not running.
    pub answer: Cell<bool>,
    turns: RefCell<Vec<Turn>>,
    seq: Cell<u32>,
}

impl Live {
    pub fn new(o: LiveOptions) -> Rc<Live> {
        Rc::new(Live {
            o,
            starts: RefCell::default(),
            steered: RefCell::default(),
            answer: Cell::new(true),
            turns: RefCell::default(),
            seq: Cell::new(0),
        })
    }

    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let live = self.clone();
        Arc::new(move |_ctx| Ok(Arc::new(LiveAdapter { live: live.clone() }) as Arc<dyn HarnessAdapter>))
    }

    pub fn turns(&self) -> usize {
        self.turns.borrow().len()
    }

    pub fn session_id(&self, turn: usize) -> String {
        self.turns.borrow()[turn].session_id.clone()
    }

    /// Says an adapter event of the turn, as its harness does.
    pub fn emit(&self, turn: usize, doc: Value) {
        let on_event = self.turns.borrow()[turn].on_event.clone();
        on_event(event(doc));
    }

    /// `session.start` with the model the fixtures name.
    pub fn init(&self, turn: usize) {
        let session_id = self.session_id(turn);
        self.emit(turn, json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5" }));
    }

    /// One tool call of the turn that writes `text`, as a large file write does.
    pub fn write(&self, turn: usize, text: &str) {
        let session_id = self.session_id(turn);
        self.emit(
            turn,
            json!({ "type": "turn.delta", "sessionId": session_id, "kind": "tool_use", "text": text, "toolName": "Write", "toolUseId": format!("tu_{turn}") }),
        );
    }

    /// The turn's result lands, the process ends, and `finished` settles.
    pub fn end(&self, turn: usize, text: &str) {
        let session_id = self.session_id(turn);
        let done = result(json!({ "status": "completed", "text": text }));
        self.emit(turn, json!({ "type": "turn.done", "sessionId": session_id, "result": serde_json::to_value(&done).expect("a result") }));
        self.emit(turn, json!({ "type": "session.end", "sessionId": session_id, "exitCode": 0, "sawResult": true }));
        self.finish(turn, done);
    }

    /// `finish(result)` alone: settles `finished` with no event said.
    pub fn finish(&self, turn: usize, done: TurnResult) {
        let finish = self.turns.borrow()[turn].finish.clone();
        let tx = finish.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(done);
        }
    }
}

struct LiveAdapter {
    live: Rc<Live>,
}

impl HarnessAdapter for LiveAdapter {
    fn steers(&self) -> bool {
        self.live.o.steers
    }
    fn steers_images(&self) -> bool {
        self.live.o.steers_images
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.live.o.inline.then_some(AttachmentRoad::Inline)
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let live = &self.live;
        live.starts.borrow_mut().push(LiveStart {
            prompt: o.prompt.clone(),
            resume: o.resume.clone(),
            cwd: o.cwd.clone(),
            images: o.images.clone(),
        });
        live.seq.set(live.seq.get() + 1);
        let session_id = live
            .o
            .session
            .clone()
            .or_else(|| o.resume.clone())
            .unwrap_or_else(|| format!("00000000-0000-4000-8000-{:012}", live.seq.get()));
        let (tx, rx) = oneshot::channel();
        let on_event = o.on_event.clone().expect("an event sink");
        live.turns.borrow_mut().push(Turn {
            session_id: session_id.clone(),
            on_event: on_event.clone(),
            finish: Rc::new(RefCell::new(Some(tx))),
        });
        if live.o.announces {
            on_event(event(json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5" })));
        }
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end was never sent") }.boxed_local();
        Ok(Arc::new(LiveSession { live: live.clone(), local_id: session_id, finished: finished.shared() }))
    }
}

struct LiveSession {
    live: Rc<Live>,
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for LiveSession {
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
        images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        if !self.live.o.steers {
            return None;
        }
        self.live.steered.borrow_mut().push(LiveSteer { prompt, images });
        let accepted = self.live.answer.get();
        Some(async move { Ok(if accepted { SteerOutcome::Accepted } else { SteerOutcome::NotRunning }) }.boxed_local())
    }
}
