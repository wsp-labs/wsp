// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-mcp-on.ts at 65f6b89c7. Spec: spec/slate_box.md (rules 1-8).
//!
//! A slate's command server on the computer its thread runs on, started there through that computer's daemon
//! (`stdio.open`) as the login its lines run as; its variables written first on its own stdin as NUL-separated pairs,
//! never on a command line. An http server asked from there too (`http.post`), the sign-in read there; or from here
//! with the sign-in the agent keeps here, read at each message.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

pub use wsp_engine::target_line::TargetLogin;
pub use wsp_wire::wire::daemon::{HttpPostReply, EXEC_TIMEOUT_MAX_MS};

use wsp_core::ids::Ids;
use wsp_engine::target_line::as_login;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::number_string;

use crate::slate_mcp::{
    McpHttpAnswer, McpHttpRequest, McpPipe, McpPipeEvents, McpPipeOpen, McpPost, McpPostRoad, McpServerSpec, McpSignIn, McpTransport,
};

/// One computer's daemon for one server: its frames, its events, and its end (slate-mcp-on.ts:278).
pub trait StdioRoad {
    /// Fails with the daemon's refusal.
    fn send(&self, frame: Value) -> LocalBoxFuture<'static, Result<Value, String>>;
    /// Every event the road carries; the answer stops them.
    fn listen(&self, f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()>;
    /// Settles when the road closes.
    fn closed(&self) -> Shared<LocalBoxFuture<'static, ()>>;
    fn close(&self);
}

/// Dials a road to the thread's computer's daemon.
pub type StdioRoadDial = Rc<dyn Fn() -> LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>>>;

/// Where a thread's slate keeps its files on its machine (slate-mcp-on.ts:290).
pub const BOX_SLATES: &str = "/tmp/wsp-slates";

pub fn box_slate_dir(thread_id: &str) -> String {
    format!("{BOX_SLATES}/{thread_id}")
}

/// The loop that reads the server's variables off its stdin before it becomes the server (slate-mcp-on.ts:294).
pub const ENV_THEN_SERVER: &str = "while IFS= read -r -d '' k && [ -n \"$k\" ] && IFS= read -r -d '' v; do export \"$k=$v\"; done; ";

/// The command `stdio.open` runs; `t` is the stdio transport (spec rule 2).
pub fn server_line(login: &TargetLogin, t: &McpTransport, cwd: &str) -> String {
    let (command, args, own_cwd) = match t {
        McpTransport::Stdio { command, args, cwd, .. } => (command.as_str(), args.as_slice(), cwd.as_deref()),
        McpTransport::Http { .. } => ("", &[][..], None),
    };
    let folder = own_cwd.unwrap_or(cwd);
    let program: Vec<String> = std::iter::once(command).chain(args.iter().map(String::as_str)).map(shell_quote).collect();
    as_login(login, &format!("{ENV_THEN_SERVER}cd -- {} 2>/dev/null; exec {}", shell_quote(folder), program.join(" ")))
}

pub fn env_pairs(vars: &IndexMap<String, String>) -> String {
    let mut out = String::new();
    for (k, v) in vars {
        out.push_str(k);
        out.push('\0');
        out.push_str(v);
        out.push('\0');
    }
    out.push('\0');
    out
}

/// Node's `Buffer.from(text, "base64")`: characters outside the alphabet are skipped and decoding stops at a `=`.
fn decode_lenient(text: &str) -> Vec<u8> {
    let mut clean: String =
        text.chars().take_while(|&c| c != '=').filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/').collect();
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    STANDARD_NO_PAD.decode(clean.as_bytes()).unwrap_or_default()
}

/// A frame field as JavaScript's `String(value)` reads it.
fn js_text(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Null) => "null".into(),
        Some(_) => "[object Object]".into(),
    }
}

fn send_quiet(fut: LocalBoxFuture<'static, Result<Value, String>>) {
    tokio::task::spawn_local(async move {
        let _ = fut.await;
    });
}

const CLOSED_CONNECTION: &str = "the server's connection is closed";

/// The state of one server a pipe holds on its road (slate-mcp-on.ts:308).
struct PipeState {
    killed: bool,
    dialled: Option<Rc<dyn StdioRoad>>,
    opened: Option<Rc<dyn StdioRoad>>,
    stop: Option<Box<dyn FnOnce()>>,
    held: Vec<String>,
}

struct Pipe {
    computer: String,
    stdio_id: String,
    events: McpPipeEvents,
    state: RefCell<PipeState>,
}

impl Pipe {
    fn write_to(&self, r: &Rc<dyn StdioRoad>, text: &str) {
        send_quiet(r.send(json!({ "op": "stdio.write", "stdioId": self.stdio_id, "data": STANDARD.encode(text.as_bytes()) })));
    }

    fn stop_listening(&self) {
        let stop = self.state.borrow_mut().stop.take();
        if let Some(stop) = stop {
            stop();
        }
    }

    fn end(&self, r: &Rc<dyn StdioRoad>) {
        self.stop_listening();
        let fut = r.send(json!({ "op": "stdio.close", "stdioId": self.stdio_id }));
        let r = r.clone();
        tokio::task::spawn_local(async move {
            let _ = fut.await;
            r.close();
        });
    }

    fn refuse(&self, message: &str, dialled: Option<Rc<dyn StdioRoad>>) {
        (self.events.exit)(&format!("it could not be started on {}: {message}", self.computer));
        self.stop_listening();
        if let Some(r) = dialled {
            r.close();
        }
    }

    fn listener(&self) -> Rc<dyn Fn(&Value)> {
        let id = self.stdio_id.clone();
        let events = self.events.clone();
        Rc::new(move |e: &Value| {
            if e.get("stdioId").and_then(Value::as_str) != Some(id.as_str()) {
                return;
            }
            match e.get("type").and_then(Value::as_str) {
                Some("stdio.out") => (events.out)(&decode_lenient(&js_text(e.get("data")))),
                Some("stdio.err") => (events.err)(String::from_utf8_lossy(&decode_lenient(&js_text(e.get("data")))).as_ref()),
                Some("stdio.exit") => match e.get("code").filter(|c| c.is_number()) {
                    Some(code) => (events.exit)(&format!("the server exited with {}", js_text(Some(code)))),
                    None => (events.exit)("the server was stopped"),
                },
                _ => {}
            }
        })
    }

    async fn open(self: Rc<Self>, dial: StdioRoadDial, frame: Value) {
        let r = match dial().await {
            Ok(r) => r,
            Err(message) => return self.refuse(&message, None),
        };
        if self.state.borrow().killed {
            r.close();
            return;
        }
        let stop = r.listen(self.listener());
        {
            let mut st = self.state.borrow_mut();
            st.dialled = Some(r.clone());
            st.stop = Some(stop);
        }
        let closed = r.closed();
        let me = self.clone();
        tokio::task::spawn_local(async move {
            closed.await;
            (me.events.exit)(&format!("the link to {} closed", me.computer));
        });
        match r.send(frame).await {
            Err(message) => {
                let dialled = self.state.borrow().dialled.clone();
                self.refuse(&message, dialled);
            }
            Ok(_) => {
                if self.state.borrow().killed {
                    self.end(&r);
                    return;
                }
                let held = {
                    let mut st = self.state.borrow_mut();
                    st.opened = Some(r.clone());
                    std::mem::take(&mut st.held)
                };
                for text in held {
                    self.write_to(&r, &text);
                }
            }
        }
    }
}

impl McpPipe for Pipe {
    fn write(&self, text: &str) {
        let opened = {
            let mut st = self.state.borrow_mut();
            match &st.opened {
                Some(r) => Some(r.clone()),
                None => {
                    st.held.push(text.to_string());
                    None
                }
            }
        };
        if let Some(r) = opened {
            self.write_to(&r, text);
        }
    }

    fn kill(&self) {
        let opened = {
            let mut st = self.state.borrow_mut();
            if st.killed {
                return;
            }
            st.killed = true;
            st.opened.clone()
        };
        if let Some(r) = opened {
            self.end(&r);
        }
    }

    fn pid(&self) -> Option<u32> {
        None
    }
}

/// Starts a server through `road` (spec rule 4); `mark` rides the spawn's environment and goes last in the variables.
pub fn pipe_on(computer: &str, login: TargetLogin, road: StdioRoadDial, mark: IndexMap<String, String>, ids: Rc<dyn Ids>) -> McpPipeOpen {
    let computer = computer.to_string();
    Rc::new(move |t: &McpTransport, spec: &McpServerSpec, events: McpPipeEvents| -> Rc<dyn McpPipe> {
        let stdio_id = hex::encode(ids.bytes(8));
        let mut vars = spec.env.clone();
        if let McpTransport::Stdio { env, .. } = t {
            for (k, v) in env {
                vars.insert(k.clone(), v.clone());
            }
        }
        for (k, v) in &mark {
            vars.insert(k.clone(), v.clone());
        }
        let mut frame = json!({ "op": "stdio.open", "stdioId": stdio_id, "cmd": server_line(&login, t, &spec.cwd) });
        if !mark.is_empty() {
            frame["env"] = serde_json::to_value(&mark).unwrap();
        }
        let pipe = Rc::new(Pipe {
            computer: computer.clone(),
            stdio_id,
            events,
            state: RefCell::new(PipeState { killed: false, dialled: None, opened: None, stop: None, held: vec![env_pairs(&vars)] }),
        });
        tokio::task::spawn_local(pipe.clone().open(road.clone(), frame));
        pipe
    })
}

pub fn sign_in_gone_line(computer: &str, line: &str) -> String {
    format!("its address wants a sign-in that {computer} does not hold now; run {line} there, then start the run again")
}

fn refused(status: u16) -> bool {
    status == 401 || status == 403
}

/// The renewal of a sign-in shared by every message refused while it runs (slate-mcp-on.ts:360).
type Renewal = Shared<LocalBoxFuture<'static, bool>>;

#[derive(Default)]
struct Renewals {
    pending: Option<Renewal>,
    done: u64,
    last: bool,
}

struct RenewOnRefusal {
    computer: String,
    sign_in: Option<McpSignIn>,
    inner: Rc<dyn McpPost>,
    renewals: Rc<RefCell<Renewals>>,
}

impl McpPost for RenewOnRefusal {
    fn send(&self, t: &McpTransport, req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        let inner = self.inner.clone();
        let computer = self.computer.clone();
        let sign_in = self.sign_in.clone();
        let renewals = self.renewals.clone();
        let t = t.clone();
        let sent_at = renewals.borrow().done;
        let first_sent = inner.send(&t, req.clone());
        async move {
            let first = first_sent.await?;
            let Some(sign_in) = sign_in else {
                return Ok(first);
            };
            if !refused(first.status) {
                return Ok(first);
            }
            let pending = renewals.borrow().pending.clone();
            let renewed = match pending {
                Some(renewal) => renewal.await,
                None => {
                    let (done, last) = {
                        let st = renewals.borrow();
                        (st.done, st.last)
                    };
                    if done > sent_at {
                        last
                    } else {
                        let renew = (sign_in.renew)();
                        let state = renewals.clone();
                        let renewal: Renewal = async move {
                            let ok = renew.await;
                            let mut st = state.borrow_mut();
                            st.pending = None;
                            st.done += 1;
                            st.last = ok;
                            ok
                        }
                        .boxed_local()
                        .shared();
                        renewals.borrow_mut().pending = Some(renewal.clone());
                        renewal.await
                    }
                }
            };
            if !renewed {
                return Err(sign_in_gone_line(&computer, &sign_in.line));
            }
            let again = inner.send(&t, req).await?;
            if refused(again.status) {
                return Err(sign_in_gone_line(&computer, &sign_in.line));
            }
            Ok(again)
        }
        .boxed_local()
    }

    fn close(&self) {
        self.inner.close();
    }
}

/// `send` once; a refusal for want of a sign-in renews once at a time and sends once more (spec rule 6).
pub fn renew_on_refusal(computer: &str, sign_in: Option<McpSignIn>, send: Rc<dyn McpPost>) -> Rc<dyn McpPost> {
    Rc::new(RenewOnRefusal { computer: computer.to_string(), sign_in, inner: send, renewals: Rc::new(RefCell::new(Renewals::default())) })
}

type OpenedRoad = Shared<LocalBoxFuture<'static, Result<Rc<dyn StdioRoad>, String>>>;

/// The state every message of one connection shares: the road dialled once, and whether the connection is closed.
struct PostState {
    opened: RefCell<Option<OpenedRoad>>,
    closed: Cell<bool>,
}

struct Ask {
    road: StdioRoadDial,
    sign_in: Option<McpSignIn>,
    state: Rc<PostState>,
}

impl McpPost for Ask {
    fn send(&self, t: &McpTransport, req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        if self.state.closed.get() {
            return async { Err(CLOSED_CONNECTION.to_string()) }.boxed_local();
        }
        let opened = {
            let mut slot = self.state.opened.borrow_mut();
            slot.get_or_insert_with(|| (self.road)().shared()).clone()
        };
        let state = self.state.clone();
        let sign_in = self.sign_in.clone();
        let url = match t {
            McpTransport::Http { url, .. } => url.clone(),
            McpTransport::Stdio { .. } => String::new(),
        };
        async move {
            let r = opened.await?;
            if state.closed.get() {
                return Err(CLOSED_CONNECTION.to_string());
            }
            let mut frame = json!({
                "op": "http.post",
                "url": url,
                "headers": req.headers,
                "body": req.body,
                "timeoutMs": req.ms.min(EXEC_TIMEOUT_MAX_MS as u64),
            });
            if let Some(sign_in) = &sign_in {
                frame["bearer"] = serde_json::to_value(&sign_in.bearer).unwrap();
            }
            let reply: HttpPostReply = serde_json::from_value(r.send(frame).await?).map_err(|e| e.to_string())?;
            Ok(McpHttpAnswer {
                status: reply.status.get() as u16,
                content_type: reply.content_type,
                session_id: reply.session_id,
                body: reply.body,
            })
        }
        .boxed_local()
    }

    fn close(&self) {
        self.state.closed.set(true);
        let opened = self.state.opened.borrow().clone();
        if let Some(opened) = opened {
            tokio::task::spawn_local(async move {
                if let Ok(r) = opened.await {
                    r.close();
                }
            });
        }
    }
}

/// A maker of one post per connection through the daemon's `http.post` (spec rule 7).
pub fn post_on(computer: &str, road: StdioRoadDial, sign_in: Option<McpSignIn>) -> Rc<dyn Fn() -> Rc<dyn McpPost>> {
    let computer = computer.to_string();
    Rc::new(move || {
        let ask = Rc::new(Ask {
            road: road.clone(),
            sign_in: sign_in.clone(),
            state: Rc::new(PostState { opened: RefCell::new(None), closed: Cell::new(false) }),
        });
        renew_on_refusal(&computer, sign_in.clone(), ask)
    })
}

/// This computer's post with the bearer the sign-in reads at each message, sent only where it would not break the line.
struct BearerHere {
    here: Rc<dyn McpPost>,
    read: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, Option<String>>>>,
}

impl McpPost for BearerHere {
    fn send(&self, t: &McpTransport, req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        let here = self.here.clone();
        let t = t.clone();
        let token_read = self.read.as_ref().map(|read| read());
        async move {
            let token = match token_read {
                Some(read) => read.await,
                None => None,
            };
            let req = match token {
                Some(token) if !token.chars().any(|c| matches!(c, '\r' | '\n' | '\0')) => {
                    let mut req = req;
                    req.headers.insert("Authorization".to_string(), format!("Bearer {token}"));
                    req
                }
                _ => req,
            };
            here.send(&t, req).await
        }
        .boxed_local()
    }

    fn close(&self) {
        self.here.close();
    }
}

/// This computer's own post with the bearer read at each message (spec rule 8).
pub fn post_here(computer: &str, sign_in: McpSignIn) -> McpPostRoad {
    let computer = computer.to_string();
    Rc::new(move |here: Rc<dyn McpPost>| {
        let inner: Rc<dyn McpPost> = Rc::new(BearerHere { here, read: sign_in.read.clone() });
        renew_on_refusal(&computer, Some(sign_in.clone()), inner)
    })
}
