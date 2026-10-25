// SPDX-License-Identifier: AGPL-3.0-only
//! Tests of spec/slate_box.md rules 1-8: slate-mcp-on.ts at 65f6b89c7. The daemon road is a fake in the test's
//! hands; TS slate-box.test.ts:635-657 is ported whole, and what slate-box-http, slate-here-http and
//! slate-box-server assert of these roads is ported against fakes.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use base64::Engine as _;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::task::LocalSet;

use wsp_core::ids::OsIds;
use wsp_engine::target_line::{Platform, TargetLogin};
use wsp_slates::slate_mcp::{McpHttpAnswer, McpHttpRequest, McpPipeEvents, McpPost, McpServerSpec, McpSignIn, McpTransport};
use wsp_slates::slate_mcp_on::*;
use wsp_wire::wire::daemon::HttpBearer;

fn b64(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(text.as_bytes())
}

fn unb64(text: &str) -> String {
    String::from_utf8(base64::engine::general_purpose::STANDARD.decode(text).unwrap()).unwrap()
}

fn root_login() -> TargetLogin {
    TargetLogin { platform: Platform::Linux, home: "/root".into(), path: None, user: "root".into(), run_as: None }
}

fn stdio(command: &str, args: &[&str], env: &[(&str, &str)], cwd: Option<&str>) -> McpTransport {
    McpTransport::Stdio { command: command.into(), args: args.iter().map(|s| s.to_string()).collect(), env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), cwd: cwd.map(String::from), tool_timeout_sec: None }
}

fn spec(t: &McpTransport, env: &[(&str, &str)]) -> McpServerSpec {
    McpServerSpec { transport: t.clone(), cwd: "/srv/app".into(), env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), secrets: vec![], open: None, post: None, sign_in: None }
}

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn server_line_quotes_as_the_ts() {
    // spec rule 2: the lines the TS printed for both logins.
    assert_eq!(
        server_line(&root_login(), &stdio("npx", &["-y", "it's"], &[], None), "/root/my proj"),
        "export HOME='/root'; cd \"$HOME\" 2>/dev/null; while IFS= read -r -d '' k && [ -n \"$k\" ] && IFS= read -r -d '' v; do export \"$k=$v\"; done; cd -- '/root/my proj' 2>/dev/null; exec 'npx' '-y' 'it'\\''s'"
    );
    let ubuntu = TargetLogin { platform: Platform::Linux, home: "/home/ubuntu".into(), path: Some("/usr/bin:/bin".into()), user: "ubuntu".into(), run_as: Some("ubuntu".into()) };
    assert_eq!(
        server_line(&ubuntu, &stdio("node", &[], &[], Some("/srv")), "/x"),
        "runuser -u 'ubuntu' -- bash -c 'export HOME='\\''/home/ubuntu'\\'' PATH='\\''/usr/bin:/bin'\\''; cd \"$HOME\" 2>/dev/null; while IFS= read -r -d '\\'''\\'' k && [ -n \"$k\" ] && IFS= read -r -d '\\'''\\'' v; do export \"$k=$v\"; done; cd -- '\\''/srv'\\'' 2>/dev/null; exec '\\''node'\\'''"
    );
}

#[test]
fn env_pairs_ends_with_an_empty_name() {
    // spec rule 3, and rule 1.
    assert_eq!(env_pairs(&map(&[("A", "1"), ("B", "two words")])), "A\u{0}1\u{0}B\u{0}two words\u{0}\u{0}");
    assert_eq!(env_pairs(&IndexMap::new()), "\u{0}");
    assert_eq!(BOX_SLATES, "/tmp/wsp-slates");
    assert_eq!(box_slate_dir("t1"), "/tmp/wsp-slates/t1");
    assert_eq!(sign_in_gone_line("spoo", "claude mcp login x"), "its address wants a sign-in that spoo does not hold now; run claude mcp login x there, then start the run again");
}

// ---- a daemon road in the test's hands ----

struct FakeRoad {
    frames: RefCell<Vec<Value>>,
    listeners: Rc<RefCell<Vec<Option<Rc<dyn Fn(&Value)>>>>>,
    closes: Cell<u32>,
    closed_tx: RefCell<Option<tokio::sync::oneshot::Sender<()>>>,
    closed: Shared<LocalBoxFuture<'static, ()>>,
    /// The answer to each op: none answers `{}` at once.
    answer: RefCell<Box<dyn FnMut(&Value) -> LocalBoxFuture<'static, Result<Value, String>>>>,
}

impl FakeRoad {
    fn new(answer: Box<dyn FnMut(&Value) -> LocalBoxFuture<'static, Result<Value, String>>>) -> Rc<FakeRoad> {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        Rc::new(FakeRoad { frames: RefCell::new(vec![]), listeners: Rc::new(RefCell::new(vec![])), closes: Cell::new(0), closed_tx: RefCell::new(Some(tx)), closed: rx.map(|_| ()).boxed_local().shared(), answer: RefCell::new(answer) })
    }
    fn plain() -> Rc<FakeRoad> {
        FakeRoad::new(Box::new(|_| async { Ok(json!({})) }.boxed_local()))
    }
    fn emit(&self, event: Value) {
        let listeners: Vec<Rc<dyn Fn(&Value)>> = self.listeners.borrow().iter().flatten().cloned().collect();
        for l in listeners {
            l(&event);
        }
    }
    fn listening(&self) -> usize {
        self.listeners.borrow().iter().filter(|l| l.is_some()).count()
    }
    fn ops(&self) -> Vec<String> {
        self.frames.borrow().iter().map(|f| f["op"].as_str().unwrap_or("").to_string()).collect()
    }
}

impl StdioRoad for FakeRoad {
    fn send(&self, frame: Value) -> LocalBoxFuture<'static, Result<Value, String>> {
        self.frames.borrow_mut().push(frame.clone());
        (self.answer.borrow_mut())(&frame)
    }
    fn listen(&self, f: Rc<dyn Fn(&Value)>) -> Box<dyn FnOnce()> {
        let at = self.listeners.borrow().len();
        self.listeners.borrow_mut().push(Some(f));
        let listeners = self.listeners.clone();
        Box::new(move || listeners.borrow_mut()[at] = None)
    }
    fn closed(&self) -> Shared<LocalBoxFuture<'static, ()>> {
        self.closed.clone()
    }
    fn close(&self) {
        self.closes.set(self.closes.get() + 1);
        if let Some(tx) = self.closed_tx.borrow_mut().take() {
            let _ = tx.send(());
        }
    }
}

fn dial(road: &Rc<FakeRoad>) -> StdioRoadDial {
    let r = road.clone();
    Rc::new(move || {
        let r = r.clone();
        async move { Ok(r as Rc<dyn StdioRoad>) }.boxed_local()
    })
}

#[derive(Default, Clone)]
struct Heard {
    out: Rc<RefCell<Vec<u8>>>,
    err: Rc<RefCell<String>>,
    exits: Rc<RefCell<Vec<String>>>,
}

impl Heard {
    fn events(&self) -> McpPipeEvents {
        let (o, e, x) = (self.out.clone(), self.err.clone(), self.exits.clone());
        McpPipeEvents { out: Rc::new(move |b: &[u8]| o.borrow_mut().extend_from_slice(b)), err: Rc::new(move |t: &str| e.borrow_mut().push_str(t)), exit: Rc::new(move |w: &str| x.borrow_mut().push(w.to_string())) }
    }
}

async fn settle() {
    for _ in 0..5 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
}

#[tokio::test(flavor = "current_thread")]
async fn pipe_on_holds_writes_until_open_and_sends_variables_first() {
    // spec rule 4: the open with the mark as its env, then the variables, then the held writes, in order.
    LocalSet::new()
        .run_until(async {
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            let gate = Rc::new(RefCell::new(Some(rx)));
            let road = FakeRoad::new(Box::new(move |f: &Value| {
                let wait = if f["op"] == "stdio.open" { gate.borrow_mut().take() } else { None };
                async move {
                    if let Some(w) = wait {
                        let _ = w.await;
                    }
                    Ok(json!({}))
                }
                .boxed_local()
            }));
            let t = stdio("node", &["server.js"], &[("TOOL", "t")], None);
            let s = spec(&t, &[("PATH", "/usr/bin"), ("TOOL", "spec")]);
            let mark = map(&[("WSP_THREAD", "t1"), ("SLATE_DIR", "/tmp/wsp-slates/t1")]);
            let heard = Heard::default();
            let pipe = (pipe_on("hetzner", root_login(), dial(&road), mark.clone(), Rc::new(OsIds)))(&t, &s, heard.events());
            assert_eq!(pipe.pid(), None);
            pipe.write("first\n");
            settle().await;
            assert_eq!(road.ops(), vec!["stdio.open"]);
            let open = road.frames.borrow()[0].clone();
            let id = open["stdioId"].as_str().unwrap().to_string();
            assert!(id.len() == 16 && id.chars().all(|c| c.is_ascii_hexdigit()), "{id}");
            assert_eq!(open, json!({ "op": "stdio.open", "stdioId": id, "cmd": server_line(&root_login(), &t, "/srv/app"), "env": { "WSP_THREAD": "t1", "SLATE_DIR": "/tmp/wsp-slates/t1" } }));
            pipe.write("second\n");
            tx.send(()).unwrap();
            settle().await;
            pipe.write("third\n");
            settle().await;
            let writes: Vec<(String, String)> = road.frames.borrow()[1..].iter().map(|f| (f["op"].as_str().unwrap().to_string(), unb64(f["data"].as_str().unwrap()))).collect();
            assert_eq!(
                writes,
                vec![
                    ("stdio.write".to_string(), "PATH\u{0}/usr/bin\u{0}TOOL\u{0}t\u{0}WSP_THREAD\u{0}t1\u{0}SLATE_DIR\u{0}/tmp/wsp-slates/t1\u{0}\u{0}".to_string()),
                    ("stdio.write".to_string(), "first\n".to_string()),
                    ("stdio.write".to_string(), "second\n".to_string()),
                    ("stdio.write".to_string(), "third\n".to_string()),
                ]
            );
            assert!(road.frames.borrow()[1..].iter().all(|f| f["stdioId"] == json!(id)));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pipe_on_sends_variables_on_stdin_never_in_the_command() {
    // Invariant: a secret written as a server's variable rides its stdin, never stdio.open's command; an empty mark
    // sends no env key.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::plain();
            let t = stdio("server", &[], &[], None);
            let s = spec(&t, &[("API_KEY", "sk_TESTONLY_9f8e")]);
            let heard = Heard::default();
            let _pipe = (pipe_on("hetzner", root_login(), dial(&road), IndexMap::new(), Rc::new(OsIds)))(&t, &s, heard.events());
            settle().await;
            let open = road.frames.borrow()[0].clone();
            assert!(!open.to_string().contains("sk_TESTONLY_9f8e"), "{open}");
            assert!(open.get("env").is_none());
            assert_eq!(unb64(road.frames.borrow()[1]["data"].as_str().unwrap()), "API_KEY\u{0}sk_TESTONLY_9f8e\u{0}\u{0}");
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pipe_on_routes_only_its_own_stdio_events() {
    // spec rule 4.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::plain();
            let t = stdio("server", &[], &[], None);
            let heard = Heard::default();
            let _pipe = (pipe_on("hetzner", root_login(), dial(&road), IndexMap::new(), Rc::new(OsIds)))(&t, &spec(&t, &[]), heard.events());
            settle().await;
            let id = road.frames.borrow()[0]["stdioId"].clone();
            road.emit(json!({ "type": "stdio.out", "stdioId": "someone-else", "data": b64("not ours") }));
            road.emit(json!({ "type": "stdio.out", "stdioId": id, "data": b64("{\"id\":1}\n") }));
            road.emit(json!({ "type": "stdio.err", "stdioId": id, "data": b64("warn: x") }));
            road.emit(json!({ "type": "stdio.exit", "stdioId": id, "code": 3 }));
            road.emit(json!({ "type": "stdio.exit", "stdioId": id }));
            assert_eq!(String::from_utf8(heard.out.borrow().clone()).unwrap(), "{\"id\":1}\n");
            assert_eq!(*heard.err.borrow(), "warn: x");
            road.close();
            settle().await;
            assert_eq!(*heard.exits.borrow(), vec!["the server exited with 3".to_string(), "the server was stopped".into(), "the link to hetzner closed".into()]);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pipe_on_closes_the_road_when_the_daemon_refuses_and_says_the_refusal_first() {
    // TS slate-box.test.ts:635-657.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::new(Box::new(|f: &Value| {
                let refused = f["op"] == "stdio.open";
                async move { if refused { Err("unknown op: stdio.open".to_string()) } else { Ok(json!({})) } }.boxed_local()
            }));
            let t = stdio("stub", &[], &[], None);
            let s = spec(&t, &[]);
            let heard = Heard::default();
            let kill_after: Rc<RefCell<Option<Rc<dyn wsp_slates::slate_mcp::McpPipe>>>> = Rc::default();
            let k = kill_after.clone();
            let exits = heard.exits.clone();
            let events = McpPipeEvents {
                out: Rc::new(|_: &[u8]| {}),
                err: Rc::new(|_: &str| {}),
                exit: Rc::new(move |w: &str| {
                    exits.borrow_mut().push(w.to_string());
                    if let Some(p) = k.borrow().clone() {
                        p.kill();
                    }
                }),
            };
            let pipe = (pipe_on("hetzner", root_login(), dial(&road), IndexMap::new(), Rc::new(OsIds)))(&t, &s, events);
            *kill_after.borrow_mut() = Some(pipe);
            for _ in 0..100 {
                if road.closes.get() == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            settle().await;
            assert_eq!(road.closes.get(), 1);
            assert_eq!((heard.exits.borrow()[0].clone(), road.listening()), ("it could not be started on hetzner: unknown op: stdio.open".to_string(), 0));
            assert!(!road.ops().contains(&"stdio.write".to_string()));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pipe_on_a_kill_before_the_road_resolves_closes_it_unsent() {
    // spec rule 4.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::plain();
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            let rx = Rc::new(RefCell::new(Some(rx)));
            let r = road.clone();
            let slow: StdioRoadDial = Rc::new(move || {
                let r = r.clone();
                let wait = rx.borrow_mut().take();
                async move {
                    if let Some(w) = wait {
                        let _ = w.await;
                    }
                    Ok(r as Rc<dyn StdioRoad>)
                }
                .boxed_local()
            });
            let t = stdio("server", &[], &[], None);
            let heard = Heard::default();
            let pipe = (pipe_on("hetzner", root_login(), slow, IndexMap::new(), Rc::new(OsIds)))(&t, &spec(&t, &[]), heard.events());
            pipe.write("lost\n");
            pipe.kill();
            tx.send(()).unwrap();
            settle().await;
            assert_eq!((road.closes.get(), road.frames.borrow().len(), road.listening()), (1, 0, 0));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn pipe_on_kill_sends_stdio_close_once() {
    // spec rule 4: stop listening, stdio.close, then the road closes.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::plain();
            let t = stdio("server", &[], &[], None);
            let heard = Heard::default();
            let pipe = (pipe_on("hetzner", root_login(), dial(&road), IndexMap::new(), Rc::new(OsIds)))(&t, &spec(&t, &[]), heard.events());
            settle().await;
            pipe.kill();
            pipe.kill();
            settle().await;
            assert_eq!(road.ops(), vec!["stdio.open", "stdio.write", "stdio.close"]);
            assert_eq!(road.frames.borrow()[2], json!({ "op": "stdio.close", "stdioId": road.frames.borrow()[0]["stdioId"] }));
            assert_eq!((road.closes.get(), road.listening()), (1, 0));
        })
        .await;
}

// ---- the http roads ----

/// A post answering each message with the next status in `statuses`, recording what it was sent.
struct ScriptedPost {
    statuses: RefCell<Vec<u16>>,
    sent: RefCell<Vec<McpHttpRequest>>,
    closes: Cell<u32>,
}

impl ScriptedPost {
    fn new(statuses: &[u16]) -> Rc<ScriptedPost> {
        Rc::new(ScriptedPost { statuses: RefCell::new(statuses.to_vec()), sent: RefCell::new(vec![]), closes: Cell::new(0) })
    }
}

impl McpPost for ScriptedPost {
    fn send(&self, _t: &McpTransport, req: McpHttpRequest) -> LocalBoxFuture<'static, Result<McpHttpAnswer, String>> {
        self.sent.borrow_mut().push(req);
        let status = { let mut s = self.statuses.borrow_mut(); if s.len() > 1 { s.remove(0) } else { s[0] } };
        async move { Ok(McpHttpAnswer { status, content_type: None, session_id: None, body: String::new() }) }.boxed_local()
    }
    fn close(&self) {
        self.closes.set(self.closes.get() + 1);
    }
}

fn bearer() -> HttpBearer {
    serde_json::from_value(json!({ "file": "~/.claude/.credentials.json", "at": ["mcpOAuth", "cloudflare|2e40c71145c8b601"], "field": "accessToken" })).unwrap()
}

fn sign_in(renews: Rc<RefCell<Vec<bool>>>, count: Rc<Cell<u32>>, read: Option<Rc<dyn Fn() -> LocalBoxFuture<'static, Option<String>>>>) -> McpSignIn {
    McpSignIn {
        bearer: bearer(),
        renew: Rc::new(move || {
            count.set(count.get() + 1);
            let answer = { let mut r = renews.borrow_mut(); if r.len() > 1 { r.remove(0) } else { r[0] } };
            async move { answer }.boxed_local()
        }),
        line: "claude mcp login cloudflare".into(),
        read,
    }
}

fn http(url: &str) -> McpTransport {
    McpTransport::Http { url: url.into(), headers: IndexMap::new() }
}

fn req(ms: u64) -> McpHttpRequest {
    McpHttpRequest { headers: map(&[("Content-Type", "application/json")]), body: "{\"id\":1}".into(), ms }
}

#[tokio::test(flavor = "current_thread")]
async fn renew_on_refusal_renews_once_and_sends_again() {
    // spec rule 6 (TS slate-box-http.test.ts:161-175, slate-here-http.test.ts:160-173 at the road).
    LocalSet::new()
        .run_until(async {
            let inner = ScriptedPost::new(&[401, 200]);
            let count = Rc::new(Cell::new(0));
            let post = renew_on_refusal("hetzner", Some(sign_in(Rc::new(RefCell::new(vec![true])), count.clone(), None)), inner.clone());
            assert_eq!(post.send(&http("http://x"), req(1000)).await.unwrap().status, 200);
            assert_eq!((count.get(), inner.sent.borrow().len()), (1, 2));
            // No sign-in: the refusal stands as the answer.
            let bare = renew_on_refusal("hetzner", None, ScriptedPost::new(&[401]));
            assert_eq!(bare.send(&http("http://x"), req(1000)).await.unwrap().status, 401);
            // A status that is not a refusal never renews.
            let fine = ScriptedPost::new(&[500]);
            let none = Rc::new(Cell::new(0));
            let post = renew_on_refusal("hetzner", Some(sign_in(Rc::new(RefCell::new(vec![true])), none.clone(), None)), fine);
            assert_eq!(post.send(&http("http://x"), req(1000)).await.unwrap().status, 500);
            assert_eq!(none.get(), 0);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn renew_on_refusal_says_sign_in_gone_where_it_cannot_renew() {
    // spec rule 6 (TS slate-box-http.test.ts:177-185).
    LocalSet::new()
        .run_until(async {
            let gone = sign_in_gone_line("hetzner", "claude mcp login cloudflare");
            let count = Rc::new(Cell::new(0));
            let post = renew_on_refusal("hetzner", Some(sign_in(Rc::new(RefCell::new(vec![false])), count.clone(), None)), ScriptedPost::new(&[403]));
            assert_eq!(post.send(&http("http://x"), req(1000)).await, Err(gone.clone()));
            let again = renew_on_refusal("hetzner", Some(sign_in(Rc::new(RefCell::new(vec![true])), Rc::new(Cell::new(0)), None)), ScriptedPost::new(&[401, 401]));
            assert_eq!(again.send(&http("http://x"), req(1000)).await, Err(gone));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn renew_on_refusal_shares_one_renewal() {
    // spec rule 6: two messages refused while one renewal runs wait on that one.
    LocalSet::new()
        .run_until(async {
            let inner = ScriptedPost::new(&[401, 401, 200]);
            let count = Rc::new(Cell::new(0));
            let (tx, rx) = tokio::sync::oneshot::channel::<()>();
            let rx = Rc::new(RefCell::new(Some(rx)));
            let c = count.clone();
            let si = McpSignIn {
                bearer: bearer(),
                renew: Rc::new(move || {
                    c.set(c.get() + 1);
                    let wait = rx.borrow_mut().take();
                    async move {
                        if let Some(w) = wait {
                            let _ = w.await;
                        }
                        true
                    }
                    .boxed_local()
                }),
                line: "claude mcp login cloudflare".into(),
                read: None,
            };
            let post = renew_on_refusal("hetzner", Some(si), inner.clone());
            let t = http("http://x");
            let a = post.send(&t, req(1000));
            let b = post.send(&t, req(1000));
            let both = tokio::task::spawn_local(async move { (a.await, b.await) });
            settle().await;
            assert_eq!(count.get(), 1);
            tx.send(()).unwrap();
            let (a, b) = both.await.unwrap();
            assert_eq!((a.unwrap().status, b.unwrap().status), (200, 200));
            assert_eq!(count.get(), 1);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn post_on_names_where_the_bearer_is_never_what() {
    // Invariant (TS slate-box-http.test.ts:134-159 at the road): each message is one http.post naming the bearer's
    // place, the reply parsed; no token or Authorization header leaves this host.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::new(Box::new(|_f: &Value| async { Ok(json!({ "status": 200, "contentType": "text/event-stream", "sessionId": "s-1", "body": "data: {}" })) }.boxed_local()));
            let make = post_on("hetzner", dial(&road), Some(sign_in(Rc::new(RefCell::new(vec![false])), Rc::new(Cell::new(0)), None)));
            let post = make();
            let got = post.send(&http("http://127.0.0.1:8123/mcp"), req(5_000)).await.unwrap();
            assert_eq!(got, McpHttpAnswer { status: 200, content_type: Some("text/event-stream".into()), session_id: Some("s-1".into()), body: "data: {}".into() });
            assert_eq!(
                road.frames.borrow()[0],
                json!({ "op": "http.post", "url": "http://127.0.0.1:8123/mcp", "headers": { "Content-Type": "application/json" }, "body": "{\"id\":1}", "timeoutMs": 5000, "bearer": { "file": "~/.claude/.credentials.json", "at": ["mcpOAuth", "cloudflare|2e40c71145c8b601"], "field": "accessToken" } })
            );
            assert!(!road.frames.borrow()[0].to_string().to_lowercase().contains("authorization"));
            // No sign-in: no bearer key.
            let road2 = FakeRoad::new(Box::new(|_f: &Value| async { Ok(json!({ "status": 204, "body": "" })) }.boxed_local()));
            let post2 = post_on("hetzner", dial(&road2), None)();
            assert_eq!(post2.send(&http("http://x"), req(5_000)).await.unwrap().status, 204);
            assert!(road2.frames.borrow()[0].get("bearer").is_none());
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn post_on_caps_timeout_at_600000_and_dials_once() {
    // spec rule 7.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::new(Box::new(|_f: &Value| async { Ok(json!({ "status": 200, "body": "{}" })) }.boxed_local()));
            let dials = Rc::new(Cell::new(0));
            let (r, d) = (road.clone(), dials.clone());
            let counted: StdioRoadDial = Rc::new(move || {
                d.set(d.get() + 1);
                let r = r.clone();
                async move { Ok(r as Rc<dyn StdioRoad>) }.boxed_local()
            });
            let post = post_on("hetzner", counted, None)();
            post.send(&http("http://x"), req(900_000)).await.unwrap();
            post.send(&http("http://x"), req(1_000)).await.unwrap();
            assert_eq!(road.frames.borrow().iter().map(|f| f["timeoutMs"].clone()).collect::<Vec<_>>(), vec![json!(600000), json!(1000)]);
            assert_eq!(dials.get(), 1);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn post_on_refuses_after_close() {
    // spec rule 7: a closed post fails and closes the road it dialled.
    LocalSet::new()
        .run_until(async {
            let road = FakeRoad::new(Box::new(|_f: &Value| async { Ok(json!({ "status": 200, "body": "{}" })) }.boxed_local()));
            let post = post_on("hetzner", dial(&road), None)();
            post.send(&http("http://x"), req(1_000)).await.unwrap();
            post.close();
            settle().await;
            assert_eq!(road.closes.get(), 1);
            assert_eq!(post.send(&http("http://x"), req(1_000)).await, Err("the server's connection is closed".to_string()));
            assert_eq!(road.frames.borrow().len(), 1);
            // Closed before any message: nothing was dialled, nothing to close.
            let unused = FakeRoad::plain();
            let never = post_on("hetzner", dial(&unused), None)();
            never.close();
            settle().await;
            assert_eq!(unused.closes.get(), 0);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn post_here_reads_the_token_at_each_message() {
    // spec rule 8 (TS slate-here-http.test.ts:140-158 at the road): a renewed token is the one sent.
    LocalSet::new()
        .run_until(async {
            let inner = ScriptedPost::new(&[200]);
            let tokens = Rc::new(RefCell::new(vec!["tok_one".to_string(), "tok_two".to_string()]));
            let reads = Rc::new(Cell::new(0));
            let (t, r) = (tokens.clone(), reads.clone());
            let read: Rc<dyn Fn() -> LocalBoxFuture<'static, Option<String>>> = Rc::new(move || {
                r.set(r.get() + 1);
                let next = t.borrow_mut().remove(0);
                async move { Some(next) }.boxed_local()
            });
            let wrap = post_here("this computer", sign_in(Rc::new(RefCell::new(vec![false])), Rc::new(Cell::new(0)), Some(read)));
            let post = wrap(inner.clone());
            post.send(&http("http://x"), req(1_000)).await.unwrap();
            post.send(&http("http://x"), req(1_000)).await.unwrap();
            let auth: Vec<Option<String>> = inner.sent.borrow().iter().map(|r| r.headers.get("Authorization").cloned()).collect();
            assert_eq!(auth, vec![Some("Bearer tok_one".to_string()), Some("Bearer tok_two".to_string())]);
            assert_eq!(reads.get(), 2);
            assert_eq!(inner.sent.borrow()[0].headers.get("Content-Type").map(String::as_str), Some("application/json"));
            post.close();
            assert_eq!(inner.closes.get(), 1);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn post_here_sends_no_bearer_that_would_break_the_request_line() {
    // Invariant (TS slate-here-http.test.ts:186-193 at the road).
    LocalSet::new()
        .run_until(async {
            for token in [Some("tok_TESTONLY_old_5b1a\r\nX-Injected: 1".to_string()), Some("a\nb".into()), Some("a\u{0}b".into()), None] {
                let inner = ScriptedPost::new(&[200]);
                let t = token.clone();
                let read: Rc<dyn Fn() -> LocalBoxFuture<'static, Option<String>>> = Rc::new(move || {
                    let t = t.clone();
                    async move { t }.boxed_local()
                });
                let post = post_here("this computer", sign_in(Rc::new(RefCell::new(vec![false])), Rc::new(Cell::new(0)), Some(read)))(inner.clone());
                post.send(&http("http://x"), req(1_000)).await.unwrap();
                assert_eq!(inner.sent.borrow()[0].headers.get("Authorization"), None, "{token:?}");
            }
        })
        .await;
}
