// SPDX-License-Identifier: AGPL-3.0-only
//! The small scripted harnesses of the session tests (session-held, session-mcp, session-run, session-opening,
//! session-search, runtime-notify ...): each start prints the same few lines (`session.start`, the reply's text in pieces,
//! `turn.done`, `session.end`) at once, a tick later, or once a gate opens, and records what it was asked. Beside it,
//! the one-off adapters the TS whole-runtime tests write inline (`const dying: HarnessAdapterFactory = () => ({ steers,
//! start })`): a factory over a start function, and a session that is a local id and the result it finishes with.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, TurnResult};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};

use super::adapter::{event, result, Started};
use super::Gate;

/// When a start prints its lines.
#[derive(Clone)]
pub enum When {
    /// Inside `start`, before it returns (an async IIFE that does not await).
    Sync,
    /// A tick after (`Promise.resolve().then(..)`).
    Soon,
    /// Once the gate opens.
    Gate(Rc<Gate>),
}

#[derive(Clone)]
struct Cfg {
    session: Option<String>,
    cwd: Option<String>,
    reply: Rc<dyn Fn(&str) -> String>,
    pieces: usize,
    when: When,
    mcp: bool,
}

pub struct Scripted {
    cfg: RefCell<Cfg>,
    pub starts: RefCell<Vec<Started>>,
    /// The `mcpServers` of each start, as JSON.
    pub mcp: RefCell<Vec<Option<Value>>>,
    opened: Cell<u32>,
}

impl Scripted {
    /// A turn that replies "done" a tick after its start, on a session id of its own.
    pub fn new() -> Rc<Scripted> {
        Rc::new(Scripted {
            cfg: RefCell::new(Cfg {
                session: None,
                cwd: None,
                reply: Rc::new(|_| "done".to_string()),
                pieces: 0,
                when: When::Soon,
                mcp: false,
            }),
            starts: RefCell::default(),
            mcp: RefCell::default(),
            opened: Cell::new(0),
        })
    }

    fn with(self: &Rc<Self>, f: impl FnOnce(&mut Cfg)) -> Rc<Self> {
        f(&mut self.cfg.borrow_mut());
        self.clone()
    }

    pub fn session(self: &Rc<Self>, id: &str) -> Rc<Self> {
        self.with(|c| c.session = Some(id.into()))
    }
    pub fn cwd(self: &Rc<Self>, cwd: &str) -> Rc<Self> {
        self.with(|c| c.cwd = Some(cwd.into()))
    }
    /// The reply's text as a function of the prompt.
    pub fn reply(self: &Rc<Self>, f: impl Fn(&str) -> String + 'static) -> Rc<Self> {
        self.with(|c| c.reply = Rc::new(f))
    }
    /// The reply is streamed as `n` text deltas of message `m1`, split by characters at even cuts (`n` is 1 or 2).
    pub fn pieces(self: &Rc<Self>, n: usize) -> Rc<Self> {
        self.with(|c| c.pieces = n)
    }
    pub fn when(self: &Rc<Self>, when: When) -> Rc<Self> {
        self.with(|c| c.when = when)
    }
    /// The adapter declares the MCP road (`mcpServers: true`).
    pub fn takes_mcp(self: &Rc<Self>) -> Rc<Self> {
        self.with(|c| c.mcp = true)
    }

    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let me = self.clone();
        Arc::new(move |_ctx: HarnessAdapterContext| Ok(Arc::new(ScriptAdapter { me: me.clone() }) as Arc<dyn HarnessAdapter>))
    }
}

struct ScriptAdapter {
    me: Rc<Scripted>,
}

fn text_pieces(text: &str, n: usize) -> Vec<String> {
    if n < 2 {
        return vec![text.to_string()];
    }
    let chars: Vec<char> = text.chars().collect();
    let cut = chars.len() / 2;
    vec![chars[..cut].iter().collect(), chars[cut..].iter().collect()]
}

impl HarnessAdapter for ScriptAdapter {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let me = &self.me;
        let cfg = me.cfg.borrow().clone();
        me.starts.borrow_mut().push(Started::of(&o));
        me.mcp.borrow_mut().push(o.mcp_servers.as_ref().map(|m| serde_json::to_value(m).expect("servers as JSON")));
        me.opened.set(me.opened.get() + 1);
        let session_id =
            o.resume.clone().or(cfg.session.clone()).unwrap_or_else(|| format!("33333333-0000-4000-8000-{:012}", me.opened.get()));
        let text = (cfg.reply)(&o.prompt);
        let done: TurnResult = result(json!({ "status": "completed", "text": text }));
        let on_event: OnEvent = o.on_event.clone().expect("an event sink");
        let lines = {
            // A start that names a folder is run in it (`o.cwd ?? "/root/app"` of the TS harnesses that echo it).
            let mut lines = vec![match &o.cwd.clone().or(cfg.cwd.clone()) {
                Some(cwd) => json!({ "type": "session.start", "sessionId": session_id, "cwd": cwd }),
                None => json!({ "type": "session.start", "sessionId": session_id }),
            }];
            if cfg.pieces > 0 {
                for piece in text_pieces(&text, cfg.pieces) {
                    lines.push(json!({ "type": "turn.delta", "sessionId": session_id, "kind": "text", "text": piece, "messageId": "m1" }));
                }
            }
            lines.push(json!({ "type": "turn.done", "sessionId": session_id, "result": serde_json::to_value(&done).expect("a result") }));
            lines.push(json!({ "type": "session.end", "sessionId": session_id, "exitCode": 0, "sawResult": true }));
            lines
        };
        let feed = move |on_event: OnEvent| {
            for line in lines {
                on_event(event(line));
            }
        };
        let (tx, rx) = oneshot::channel();
        match cfg.when {
            When::Sync => {
                feed(on_event);
                let _ = tx.send(done);
            }
            When::Soon => {
                tokio::task::spawn_local(async move {
                    feed(on_event);
                    let _ = tx.send(done);
                });
            }
            When::Gate(gate) => {
                tokio::task::spawn_local(async move {
                    gate.wait().await;
                    feed(on_event);
                    let _ = tx.send(done);
                });
            }
        }
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn was dropped") }.boxed_local();
        Ok(Plain::of(&session_id, finished))
    }

    fn steers(&self) -> bool {
        false
    }

    fn mcp_servers(&self) -> bool {
        self.me.cfg.borrow().mcp
    }
}

/// `start: o => ...` of a TS adapter literal.
pub type StartFn = Rc<dyn Fn(HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError>>;

/// `() => ({ steers: false, start })`.
pub fn adapter_of(start: StartFn) -> HarnessAdapterFactory {
    Arc::new(move |_ctx| Ok(Arc::new(StartAdapter { start: start.clone() }) as Arc<dyn HarnessAdapter>))
}

struct StartAdapter {
    start: StartFn,
}

impl HarnessAdapter for StartAdapter {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        (self.start)(o)
    }
}

/// `{ localId, finished, interrupt: async () => {} }`.
pub struct Plain {
    pub local_id: String,
    pub finished: SharedFuture<TurnResult>,
}

impl Plain {
    pub fn of(local_id: &str, finished: LocalBoxFuture<'static, TurnResult>) -> Arc<dyn HarnessSession> {
        Arc::new(Plain { local_id: local_id.into(), finished: finished.shared() })
    }
}

impl HarnessSession for Plain {
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

/// `Promise.resolve().then(() => { for (e of events) o.onEvent(e); return result })`: the events go out after the start
/// has returned and the caller's own code after it has run, then the future answers `done`. A promise's microtask
/// runs after the caller's synchronous code; one `yield_now` here runs before the Rust start's own bookkeeping (the
/// thread record it writes once the turn is under way), so the events wait three turns.
pub fn after_a_turn(on_event: OnEvent, events: Vec<Value>, done: Value) -> LocalBoxFuture<'static, TurnResult> {
    async move {
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        for doc in events {
            on_event(event(doc));
        }
        result(done)
    }
    .boxed_local()
}

/// A `turn.done`, then a `session.end`, of the session.
pub fn done_and_end(session: &str, done: &Value, exit_code: i32, saw_result: bool) -> Vec<Value> {
    vec![
        json!({ "type": "turn.done", "sessionId": session, "result": done }),
        json!({ "type": "session.end", "sessionId": session, "exitCode": exit_code, "sawResult": saw_result }),
    ]
}
