// SPDX-License-Identifier: AGPL-3.0-only
//! The recording daemon channel the TS whole-runtime tests write for themselves (`fakeDaemon` of bring-back.test.ts,
//! channels.test.ts, rewind.test.ts and the like): every frame kept with the token of the dial it went down, each op
//! answered as the test says or as the default does, the dials and the closes counted, and an event pushed to every
//! channel still open.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel, DaemonChannelOptions, Frame};

pub use super::daemon::Open;

/// How a test answers one op: the whole reply, `id` and `ok` included (an `id` left out is filled in as 1).
pub type Answer = Rc<dyn Fn(&Frame) -> LocalBoxFuture<'static, Value>>;

/// What runs before a frame is answered, given the frame and the token of the dial it came down.
pub type BeforeSend = Rc<dyn Fn(&Frame, &str) -> LocalBoxFuture<'static, ()>>;

pub struct RecDaemon {
    pub frames: Rc<RefCell<Vec<Frame>>>,
    /// The token each frame's dial carried, which says which machine's daemon it went to.
    pub tokens: Rc<RefCell<Vec<String>>>,
    pub dials: Rc<RefCell<Vec<(String, String)>>>,
    pub closed: Rc<Cell<u32>>,
    /// The version each dial's `daemon.hello` says, where one is said (right after the dial, as the daemon does).
    pub hello: Cell<Option<i64>>,
    answers: Rc<RefCell<HashMap<String, Answer>>>,
    default: Rc<dyn Fn(&Frame) -> Value>,
    before: RefCell<Option<BeforeSend>>,
    listeners: Rc<RefCell<Vec<(u64, Rc<dyn Fn(Value)>)>>>,
    next: Cell<u64>,
}

impl RecDaemon {
    /// A daemon that answers every op with `default` until a test answers it otherwise.
    pub fn new(default: impl Fn(&Frame) -> Value + 'static) -> Rc<RecDaemon> {
        Rc::new(RecDaemon {
            frames: Rc::default(),
            tokens: Rc::default(),
            dials: Rc::default(),
            closed: Rc::default(),
            hello: Cell::new(None),
            answers: Rc::default(),
            default: Rc::new(default),
            before: RefCell::new(None),
            listeners: Rc::default(),
            next: Cell::new(0),
        })
    }

    /// `answers[op] = f`.
    pub fn answer(&self, op: &str, f: impl Fn(&Frame) -> Value + 'static) {
        self.answers.borrow_mut().insert(op.into(), Rc::new(move |frame| futures::future::ready(f(frame)).boxed_local()));
    }

    /// As `answer`, where the reply waits on something.
    pub fn answer_later(&self, op: &str, f: impl Fn(&Frame) -> LocalBoxFuture<'static, Value> + 'static) {
        self.answers.borrow_mut().insert(op.into(), Rc::new(f));
    }

    pub fn unanswer(&self, op: &str) {
        self.answers.borrow_mut().remove(op);
    }

    pub fn before_send(&self, f: impl Fn(&Frame, &str) -> LocalBoxFuture<'static, ()> + 'static) {
        *self.before.borrow_mut() = Some(Rc::new(f));
    }

    /// Pushes an event to every channel still open, as the daemon pushes a pty's exit.
    pub fn push(&self, event: Value) {
        let heard: Vec<Rc<dyn Fn(Value)>> = self.listeners.borrow().iter().map(|(_, f)| f.clone()).collect();
        for f in heard {
            f(event.clone());
        }
    }

    /// The ops of every frame so far, in order.
    pub fn ops(&self) -> Vec<String> {
        self.frames.borrow().iter().map(|f| f.get("op").and_then(Value::as_str).unwrap_or_default().to_string()).collect()
    }

    pub fn open(self: &Rc<Self>) -> Open {
        let daemon = self.clone();
        Rc::new(move |dial: DaemonChannelOptions| {
            let daemon = daemon.clone();
            async move {
                daemon.dials.borrow_mut().push((dial.url.clone(), dial.token.clone()));
                let id = daemon.next.get() + 1;
                daemon.next.set(id);
                daemon.listeners.borrow_mut().push((id, dial.on_event.clone()));
                if let Some(version) = daemon.hello.get() {
                    // The hello follows the auth reply, which is what resolves the dial.
                    let on_event = dial.on_event.clone();
                    tokio::task::spawn_local(async move {
                        tokio::task::yield_now().await;
                        on_event(json!({ "type": "daemon.hello", "version": version }));
                    });
                }
                Ok(Rc::new(RecChannel { daemon: daemon.clone(), token: dial.token.clone(), id }) as Rc<dyn DaemonChannel>)
            }
            .boxed_local()
        })
    }
}

struct RecChannel {
    daemon: Rc<RecDaemon>,
    token: String,
    id: u64,
}

#[async_trait(?Send)]
impl DaemonChannel for RecChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let d = &self.daemon;
        d.frames.borrow_mut().push(frame.clone());
        d.tokens.borrow_mut().push(self.token.clone());
        let before = d.before.borrow().clone();
        if let Some(before) = before {
            before(&frame, &self.token).await;
        }
        let op = frame.get("op").and_then(Value::as_str).unwrap_or_default().to_string();
        let own = d.answers.borrow().get(&op).cloned();
        let reply = match own {
            Some(answer) => answer(&frame).await,
            None => (d.default)(&frame),
        };
        let mut reply = match reply {
            Value::Object(map) => map,
            other => panic!("a daemon answer is an object, not {other}"),
        };
        if !reply.contains_key("id") {
            let mut with_id = Frame::new();
            with_id.insert("id".into(), json!(1));
            with_id.extend(reply);
            reply = with_id;
        }
        Ok(reply)
    }
    fn close(&self) {
        self.daemon.closed.set(self.daemon.closed.get() + 1);
        self.daemon.listeners.borrow_mut().retain(|(id, _)| *id != self.id);
    }
    async fn closed(&self) -> ChannelEnd {
        futures::future::pending().await
    }
}
