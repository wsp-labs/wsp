// Review probe: the machine exec road and the pipe pool, driven past what the ported tests cover.
// Only the tests that fail on 11db4a4e6 are kept. Each is preceded by its finding number, severity and the TS line.
mod support;

use std::future::Future;
use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};

use futures::future::LocalBoxFuture;
use futures::{FutureExt, StreamExt};
use regex::Regex;
use wsp_adapters::port::{ExecFailure, ExecOptions as Launch, ExecStream, ExecStreamFactory};
use wsp_core::machine_exec::{machine_exec_stream, MachineExecOptions};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::machine::ExecResult;

use support::guest::{errors, Hook, Inner, ScriptGuest, Step};

fn opts(poll_ms: u64) -> MachineExecOptions {
    MachineExecOptions { poll_ms: Some(poll_ms), ..Default::default() }
}

fn hook<F, Fut>(f: F) -> Hook
where
    F: Fn(String, Inner) -> Fut + 'static,
    Fut: Future<Output = EngineResult<ExecResult>> + 'static,
{
    Arc::new(move |cmd: String, _stdin: Option<Vec<u8>>, inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
        Box::pin(f(cmd, inner))
    })
}

fn said(stdout: &str) -> EngineResult<ExecResult> {
    Ok(ExecResult { exit_code: support::code(0), stdout: stdout.to_string(), stderr: String::new() })
}

fn capture(re: &str, text: &str) -> String {
    Regex::new(re).unwrap().captures(text).map(|c| c[1].to_string()).unwrap_or_default()
}

fn with_input(lines: &[&str]) -> Launch {
    Launch { input: Some(lines.iter().map(|l| l.to_string()).collect()), ..Default::default() }
}

// kill() twice: the first KILL exec fails, the second call is the retry.
// F4 MAJOR: TS packages/runtime/src/machine-exec.ts:654
#[test]
fn a_second_kill_sends_its_own_kill_exec_when_the_first_one_failed() {
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![]);
        let sent = Arc::new(Mutex::new(Vec::<String>::new()));
        let fails = Arc::new(AtomicI32::new(1));
        let (s, f) = (sent.clone(), fails.clone());
        guest.set_hook(hook(move |cmd, inner| {
            let res = if cmd.contains("kill -KILL") {
                s.lock().unwrap().push(cmd.clone());
                if f.fetch_sub(1, SeqCst) > 0 {
                    Err(errors::plain("the link dropped"))
                } else {
                    inner(&cmd)
                }
            } else {
                inner(&cmd)
            };
            async move { res }
        }));
        let stream = machine_exec_stream(machine, opts(5), None).launch("sleep 300", Launch::default());
        support::settle().await;
        stream.kill();
        support::settle().await;
        stream.kill();
        support::settle().await;
        let log = sent.lock().unwrap().clone();
        println!("KILL execs sent: {}", log.len());
        for c in &log {
            println!("  {c}");
        }
        assert_eq!(log.len(), 2, "the TS sends one KILL exec per kill() call");
        assert!(!guest.child_alive(), "the second KILL reaches the run's group");
    });
}

// teardown() then kill(), and kill() then teardown(): one exec per call, in call order.
// F4 MAJOR: TS packages/runtime/src/machine-exec.ts:654
#[test]
fn teardown_then_kill_and_kill_then_teardown_send_each_signal_in_call_order() {
    support::run_local(async {
        for order in ["tk", "kt", "tt", "kk"] {
            let (guest, machine) = ScriptGuest::new(vec![]);
            let sent = Arc::new(Mutex::new(Vec::<String>::new()));
            let s = sent.clone();
            guest.set_hook(hook(move |cmd, inner| {
                if cmd.contains("kill -TERM") || cmd.contains("kill -KILL") {
                    s.lock().unwrap().push(if cmd.contains("kill -KILL") { "KILL".into() } else { "TERM".into() });
                }
                let res = inner(&cmd);
                async move { res }
            }));
            let stream = machine_exec_stream(machine, opts(5), None).launch("sleep 300", Launch::default());
            support::settle().await;
            for c in order.chars() {
                if c == 't' {
                    stream.teardown();
                } else {
                    stream.kill();
                }
            }
            support::settle().await;
            let want: Vec<&str> = order.chars().map(|c| if c == 't' { "TERM" } else { "KILL" }).collect();
            println!("{order}: {:?}", sent.lock().unwrap());
            assert_eq!(*sent.lock().unwrap(), want, "{order}");
        }
    });
}

// The exit settles after the lines the reap read are handed out, not before the first of them.
// F12 MINOR: TS packages/runtime/src/machine-exec.ts:507
#[test]
fn exited_settles_only_after_the_lines_of_the_final_reap_were_taken() {
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![Step::only_exit(0)]);
        let g = guest.clone();
        guest.set_hook(hook(move |cmd, inner| {
            let res = inner(&cmd);
            if cmd.contains("__WSP_EOF_") {
                g.append("x\ny\n");
            }
            async move { res }
        }));
        let stream = machine_exec_stream(machine, opts(5), None).launch("echo hi", Launch::default());
        let mut lines = stream.lines();
        let first = lines.next().await.unwrap().unwrap();
        assert_eq!(first, "x");
        let settled = stream.exited().now_or_never();
        println!("exited after the first of two reaped lines: {settled:?}");
        assert!(settled.is_none(), "the TS finishes the stream only once the generator is resumed past the reaped lines");
    });
}

// A reap whose size is negative: a safe integer to the TS.
// F10 MINOR: TS packages/runtime/src/machine-exec.ts:466
#[test]
fn a_reap_that_answers_a_negative_size_still_yields_its_chunk_and_removes_nothing_more() {
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![Step::only_exit(0)]);
        let removals = Arc::new(AtomicUsize::new(0));
        let r = removals.clone();
        guest.set_hook(hook(move |cmd, inner| {
            if cmd.contains("__WSP_REAPED_") {
                r.fetch_add(1, SeqCst);
                let mark = capture(r"(__WSP_REAPED_[a-z0-9]+__)", &cmd);
                return std::future::ready(said(&format!("aGVsbG8K\n{mark} -5\n")));
            }
            if cmd.contains("rm -rf") {
                r.fetch_add(100, SeqCst);
            }
            std::future::ready(inner(&cmd))
        }));
        let stream = machine_exec_stream(machine, opts(5), None).launch("echo hi", Launch::default());
        let (lines, failure) = support::collect(&*stream).await;
        println!("lines {lines:?} failure {failure:?} reaps+removes {}", removals.load(SeqCst));
        assert_eq!(lines, vec!["hello"]);
        assert_eq!(removals.load(SeqCst), 1, "one reap exec, and no second exec to remove the files");
    });
}

// An exit file holding a number past 32 bits.
// F9 MINOR: TS packages/runtime/src/machine-exec.ts:589
#[test]
fn an_exit_code_past_32_bits_settles_as_that_number_not_as_no_code() {
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![]);
        guest.set_hook(hook(move |cmd, inner| {
            if cmd.contains("__WSP_EOF_") {
                let sentinel = capture(r"(__WSP_EOF_[a-z0-9]+__)", &cmd);
                return std::future::ready(said(&format!("\n{sentinel} 4294967296 up \n")));
            }
            std::future::ready(inner(&cmd))
        }));
        let stream = machine_exec_stream(machine, opts(5), None).launch("echo hi", Launch::default());
        let _ = support::collect(&*stream).await;
        let code = stream.exited().await;
        println!("exit settled as {code:?}");
        assert!(code.is_some(), "the TS settles 4294967296");
    });
}

// base64 answered by the guest that is not clean: the bytes node's Buffer.from(text, "base64") reads.
fn lines_of_bytes(bytes: &[u8]) -> Vec<String> {
    let mut parts: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
    if parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts.iter().map(|p| String::from_utf8_lossy(p).into_owned()).collect()
}

async fn through_a_poll(payload: &str) -> Vec<String> {
    let (guest, machine) = ScriptGuest::new(vec![]);
    let payload = payload.to_string();
    let polls = Arc::new(AtomicUsize::new(0));
    guest.set_hook(hook(move |cmd, inner| {
        if cmd.contains("__WSP_EOF_") {
            let sentinel = capture(r"(__WSP_EOF_[a-z0-9]+__)", &cmd);
            let n = polls.fetch_add(1, SeqCst);
            let body = if n == 0 { payload.clone() } else { String::new() };
            return std::future::ready(said(&format!("{body}\n{sentinel} 0 up \n")));
        }
        std::future::ready(inner(&cmd))
    }));
    let stream = machine_exec_stream(machine, opts(1), None).launch("echo hi", Launch::default());
    let (lines, _) = support::collect(&*stream).await;
    lines
}

// F11 MINOR: TS packages/runtime/src/machine-exec.ts:575
#[test]
fn base64_with_the_url_safe_characters_decodes_as_node_decodes_it() {
    support::run_local(async {
        for (text, hex) in [("a-b_", "6be6ff"), ("YW-_", "616fbf")] {
            let want = lines_of_bytes(&hex::decode(hex).unwrap());
            let got = through_a_poll(text).await;
            println!("{text:?}: node {want:?} rust {got:?}");
            assert_eq!(got, want, "{text}");
        }
    });
}

// many writes: the order the channel took them in.
// F2 BLOCKER: TS packages/runtime/src/machine-exec.ts:352
#[test]
fn writes_made_while_others_wait_reach_the_channel_in_the_order_they_were_made() {
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![]);
        guest.set_hook(Arc::new(move |cmd: String, _stdin: Option<Vec<u8>>, inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
            Box::pin(async move {
                if cmd.contains("echo WSP_OK") {
                    support::sleep_ms(120).await;
                }
                inner(&cmd)
            })
        }));
        let stream = machine_exec_stream(machine, opts(5), None).launch("cat", with_input(&["seed"]));
        support::settle().await;
        let mut done = Vec::new();
        for w in ["w1", "w2", "w3"] {
            let s = stream.clone();
            done.push(tokio::task::spawn_local(async move { s.write(w.to_string()).await }));
        }
        support::sleep_ms(180).await;
        let s = stream.clone();
        done.push(tokio::task::spawn_local(async move { s.write("w4".to_string()).await }));
        for d in done {
            let _ = d.await;
        }
        let input = guest.get_input();
        println!("channel took: {input:?}");
        assert_eq!(input, "seed\nw1\nw2\nw3\nw4\n");
    });
}

// The same lock order, with close_input: a line written before it must not go in after the TERM to the input pump.
// F2 BLOCKER: TS packages/runtime/src/machine-exec.ts:677
#[test]
fn a_write_made_before_close_input_goes_in_before_the_close() {
    use base64::Engine as _;
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![]);
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let ev = events.clone();
        guest.set_hook(Arc::new(move |cmd: String, _stdin: Option<Vec<u8>>, inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
            let ev = ev.clone();
            Box::pin(async move {
                if cmd.contains("echo WSP_OK") {
                    let b64 = capture(r"printf %s '([A-Za-z0-9+/=]*)' \| base64 -d >> ", &cmd);
                    let text = String::from_utf8_lossy(&base64::engine::general_purpose::STANDARD.decode(b64).unwrap_or_default()).trim().to_string();
                    ev.lock().unwrap().push(format!("w:{text}"));
                    support::sleep_ms(120).await;
                } else if cmd.contains(".tail") && cmd.contains("kill -TERM \"$P\"") {
                    ev.lock().unwrap().push("close".to_string());
                }
                inner(&cmd)
            })
        }));
        let stream = machine_exec_stream(machine, opts(5), None).launch("cat", with_input(&["seed"]));
        support::settle().await;
        let mut done = Vec::new();
        for w in ["w1", "w2", "w3"] {
            let s = stream.clone();
            done.push(tokio::task::spawn_local(async move { s.write(w.to_string()).await }));
        }
        support::sleep_ms(180).await;
        stream.close_input();
        for d in done {
            let _ = d.await;
        }
        support::sleep_ms(200).await;
        println!("order the guest saw: {:?}", events.lock().unwrap());
        assert_eq!(*events.lock().unwrap(), vec!["w:w1", "w:w2", "w:w3", "close"]);
    });
}

fn _unused(_: &ExecFailure) {}

// ---------------------------------------------------------------- the pipe pool
mod pool {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use futures::future::LocalBoxFuture;
    use futures::{FutureExt, SinkExt, StreamExt};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::task::spawn_local;
    use tokio_tungstenite::tungstenite::Message;
    use tokio_tungstenite::{accept_async, WebSocketStream};
    use wsp_core::clock::FakeClock;
    use wsp_core::pipe::{pipe_pool, PipeDuplex, PipeHandler, PipePool, PipePoolOptions, PipeReach, PIPE_BEHIND_AGAIN_MS};
    use wsp_core::pipe::pool::PipeOpen;
    use wsp_wire::wire::daemon_version::PIPE_DAEMON_VERSION;

    use crate::support;

    pub struct Lab {
        pub pool: Rc<PipePool>,
        pub logs: Rc<RefCell<Vec<String>>>,
        pub rewrites: Rc<Cell<usize>>,
        pub handed: Rc<RefCell<Vec<PipeDuplex>>>,
        pub clock: FakeClock,
    }

    pub fn lab(url: Rc<dyn Fn() -> String>, idle_max_ms: Option<u64>) -> Lab {
        let logs: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let rewrites = Rc::new(Cell::new(0usize));
        let handed: Rc<RefCell<Vec<PipeDuplex>>> = Rc::new(RefCell::new(Vec::new()));
        let clock = FakeClock::new(0);
        let h = handed.clone();
        let handler: PipeHandler = Rc::new(move |_id: &str, _open: PipeOpen, duplex: PipeDuplex| {
            h.borrow_mut().push(duplex);
            Ok(())
        });
        let (l, r) = (logs.clone(), rewrites.clone());
        let pool = pipe_pool(PipePoolOptions {
            reach: Rc::new(move |_id: &str| {
                let url = url();
                async move { Ok::<_, String>(Some(PipeReach { url, token: "pipe-token".to_string() })) }.boxed_local()
            }),
            version: Rc::new(|_id: &str| async { Ok::<_, String>(Some(PIPE_DAEMON_VERSION)) }.boxed_local()),
            handler: Rc::new(move || Some(handler.clone())),
            ready: Rc::new(|| true),
            rewrite: Rc::new(move |_id: &str| r.set(r.get() + 1)),
            clock: Some(clock.clock()),
            backoff_ms: Some(Rc::new(|_a: u32| 20u64)),
            idle_max_ms,
            behind_again_ms: None,
            dial_rate: None,
            log: Some(Rc::new(move |line: &str| l.borrow_mut().push(line.to_string()))),
        });
        Lab { pool, logs, rewrites, handed, clock }
    }

    pub fn summary(l: &Lab) -> String {
        serde_json::json!({ "logs": *l.logs.borrow(), "rewrites": l.rewrites.get(), "handed": l.handed.borrow().len() }).to_string()
    }

    pub struct Srv {
        pub url: String,
        pub conns: Rc<Cell<usize>>,
        pub closes: Rc<RefCell<Vec<u16>>>,
        stop: tokio::task::AbortHandle,
    }

    impl Srv {
        pub fn stop(&self) {
            self.stop.abort();
        }
    }

    type Script = Rc<dyn Fn(WebSocketStream<TcpStream>, usize, Rc<RefCell<Vec<u16>>>) -> LocalBoxFuture<'static, ()>>;

    pub async fn ws_server(script: Script) -> Srv {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let conns = Rc::new(Cell::new(0usize));
        let closes: Rc<RefCell<Vec<u16>>> = Rc::new(RefCell::new(Vec::new()));
        let (c, cl) = (conns.clone(), closes.clone());
        let accept = spawn_local(async move {
            loop {
                let Ok((tcp, _)) = listener.accept().await else { return };
                let (script, c, cl) = (script.clone(), c.clone(), cl.clone());
                spawn_local(async move {
                    let Ok(ws) = accept_async(tcp).await else { return };
                    let n = c.get();
                    c.set(n + 1);
                    script(ws, n, cl).await;
                });
            }
        });
        Srv { url: format!("http://127.0.0.1:{port}/"), conns, closes, stop: accept.abort_handle() }
    }

    /// Reads frames until the socket closes, noting the close code the client sent.
    pub async fn drain(mut ws: WebSocketStream<TcpStream>, closes: Rc<RefCell<Vec<u16>>>) {
        while let Some(next) = ws.next().await {
            match next {
                Ok(Message::Close(Some(frame))) => {
                    closes.borrow_mut().push(u16::from(frame.code));
                    return;
                }
                Ok(Message::Close(None)) => {
                    closes.borrow_mut().push(1005);
                    return;
                }
                Ok(_) => {}
                Err(_) => return,
            }
        }
    }

    pub async fn first_message(ws: &mut WebSocketStream<TcpStream>) -> bool {
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(_))) => return true,
                Some(Ok(_)) => continue,
                _ => return false,
            }
        }
    }

    // F1 BLOCKER: TS packages/runtime/src/pipe.ts:319
    #[test]
    fn p1_a_daemon_behind_https_is_dialled_with_a_tls_handshake() {
        support::run_local(async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let seen: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
            let accepted = Rc::new(Cell::new(0usize));
            let (s, a) = (seen.clone(), accepted.clone());
            let server = spawn_local(async move {
                loop {
                    let Ok((mut tcp, _)) = listener.accept().await else { return };
                    a.set(a.get() + 1);
                    let s = s.clone();
                    spawn_local(async move {
                        let mut first = [0u8; 1];
                        if let Ok(1) = tcp.read(&mut first).await {
                            s.borrow_mut().push(first[0]);
                        }
                        let mut rest = Vec::new();
                        let _ = tcp.read_to_end(&mut rest).await;
                    });
                }
            });
            let l = lab(Rc::new(move || format!("https://127.0.0.1:{port}/")), None);
            l.pool.up("ws-a");
            support::sleep_ms(500).await;
            println!(
                "P1 accepted={} firstByte={} logs={:?}",
                accepted.get(),
                seen.borrow().first().map(|b| format!("{b:x}")).unwrap_or("none".into()),
                l.logs.borrow()
            );
            assert_eq!(seen.borrow().first().copied(), Some(0x16), "the TS opens TLS: its first byte is the handshake record, 0x16");
            l.pool.close();
            server.abort();
        });
    }

    // `new URL` parses these and the ws library then throws inside the reach's then, so the TS dial waits out its 15 s
    // timer; tungstenite refuses them at once and the Rust pool logs the failed dial and backs off at the first poll.
    // F8 MINOR: TS packages/runtime/src/pipe.ts:319
    #[test]
    fn p7_a_url_the_ws_library_refuses_leaves_the_dial_to_its_timer() {
        support::run_local(async {
            for url in ["not a url", "http://", "ftp://127.0.0.1:1/", "http://127.0.0.1:1/#frag"] {
                let l = lab(Rc::new(move || url.to_string()), None);
                l.pool.up("ws-a");
                support::sleep_ms(200).await;
                let at0 = l.logs.borrow().len();
                l.clock.advance(14_999);
                support::sleep_ms(50).await;
                let at14999 = l.logs.borrow().len();
                l.clock.advance(1);
                support::sleep_ms(100).await;
                println!("P7 {url:?} logsAt0={at0} logsAt14999={at14999} logsAt15000={}", l.logs.borrow().len());
                assert_eq!((at0, at14999, l.logs.borrow().len()), (1, 1, 2), "{url}: no failed-dial line before the 15 s timer, one at it");
                l.pool.close();
            }
        });
    }

    // F6 MINOR: TS packages/runtime/src/pipe.ts:319
    #[test]
    fn p9_a_twenty_mib_frame_on_a_taken_pipe() {
        support::run_local(async {
            let srv = ws_server(Rc::new(|mut ws, n, closes| {
                Box::pin(async move {
                    if first_message(&mut ws).await {
                        if n == 0 {
                            let _ = ws.feed(Message::text(serde_json::json!({ "op": "pipe.open", "door": "http", "heldMs": 0 }).to_string())).await;
                            let _ = ws.feed(Message::binary(vec![7u8; 20 * 1024 * 1024])).await;
                            let _ = ws.flush().await;
                        } else {
                            let _ = ws.send(Message::text(serde_json::json!({ "id": 0, "ok": true }).to_string())).await;
                        }
                    }
                    drain(ws, closes).await;
                })
            }))
            .await;
            let l = lab({ let u = srv.url.clone(); Rc::new(move || u.clone()) }, None);
            l.pool.up("ws-a");
            support::until("the pipe to be handed over", 3_000, || l.handed.borrow().len() == 1).await;
            let mut duplex = l.handed.borrow_mut().remove(0);
            let got = Rc::new(Cell::new(0usize));
            let g = got.clone();
            let reader = spawn_local(async move {
                let mut buf = vec![0u8; 65536];
                while let Ok(n) = duplex.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    g.set(g.get() + n);
                }
            });
            support::sleep_ms(1500).await;
            println!("P9 handed=1 bytesRead={}", got.get());
            assert_eq!(got.get(), 20 * 1024 * 1024, "the TS reads every byte of a 20 MiB frame (ws allows 100 MiB)");
            reader.abort();
            l.pool.close();
            srv.stop();
        });
    }

    // F5 MINOR: TS packages/runtime/src/pipe.ts:287
    #[test]
    fn p10_a_small_write_with_no_flush_reaches_the_daemon() {
        support::run_local(async {
            let got: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
            let g = got.clone();
            let srv = ws_server(Rc::new(move |mut ws, n, closes| {
                let g = g.clone();
                Box::pin(async move {
                    if first_message(&mut ws).await {
                        if n == 0 {
                            let _ = ws.send(Message::text(serde_json::json!({ "op": "pipe.open", "door": "http", "heldMs": 0 }).to_string())).await;
                            while let Some(Ok(m)) = ws.next().await {
                                match m {
                                    Message::Binary(b) => g.borrow_mut().push(b.len()),
                                    Message::Close(_) => return,
                                    _ => {}
                                }
                            }
                            return;
                        }
                        let _ = ws.send(Message::text(serde_json::json!({ "id": 0, "ok": true }).to_string())).await;
                    }
                    drain(ws, closes).await;
                })
            }))
            .await;
            let l = lab({ let u = srv.url.clone(); Rc::new(move || u.clone()) }, None);
            l.pool.up("ws-a");
            support::until("the pipe to be handed over", 3_000, || l.handed.borrow().len() == 1).await;
            let mut duplex = l.handed.borrow_mut().remove(0);
            duplex.write_all(b"hello").await.unwrap();
            support::sleep_ms(1000).await;
            println!("P10 daemonGot={:?}", got.borrow());
            assert_eq!(*got.borrow(), vec![5usize], "the TS duplex sends a write at once; a handler that writes and then waits for a reply is never answered if the frame sits in the sink");
            l.pool.close();
            srv.stop();
        });
    }

    // F7 MINOR: TS packages/runtime/src/pipe.ts:382
    #[test]
    fn p11_a_refused_token_is_dialled_again_after_behind_again_ms_not_60_s() {
        support::run_local(async {
            let srv = ws_server(Rc::new(|mut ws, _n, closes| {
                Box::pin(async move {
                    if first_message(&mut ws).await {
                        let _ = ws.send(Message::text(serde_json::json!({ "id": 0, "ok": false, "error": "unauthorized" }).to_string())).await;
                    }
                    drain(ws, closes).await;
                })
            }))
            .await;
            let clock = FakeClock::new(0);
            let (rewrites, logs) = (Rc::new(Cell::new(0usize)), Rc::new(RefCell::new(Vec::<String>::new())));
            let (r, l) = (rewrites.clone(), logs.clone());
            let url = srv.url.clone();
            let pool = pipe_pool(PipePoolOptions {
                reach: Rc::new(move |_id: &str| {
                    let url = url.clone();
                    async move { Ok::<_, String>(Some(PipeReach { url, token: "pipe-token".to_string() })) }.boxed_local()
                }),
                version: Rc::new(|_id: &str| async { Ok::<_, String>(Some(PIPE_DAEMON_VERSION)) }.boxed_local()),
                handler: Rc::new(|| None),
                ready: Rc::new(|| true),
                rewrite: Rc::new(move |_id: &str| r.set(r.get() + 1)),
                clock: Some(clock.clock()),
                backoff_ms: Some(Rc::new(|_a: u32| 20u64)),
                idle_max_ms: None,
                behind_again_ms: Some(100),
                dial_rate: None,
                log: Some(Rc::new(move |line: &str| l.borrow_mut().push(line.to_string()))),
            });
            pool.up("ws-a");
            support::until("a refusal", 3_000, || rewrites.get() == 1).await;
            support::sleep_ms(100).await;
            let before = srv.conns.get();
            clock.advance(1000);
            support::sleep_ms(300).await;
            let last: String = logs.borrow().last().cloned().unwrap_or_default();
            println!("P11 refusalLog={:?} connsBefore={before} connsAfter1000ms={} rewrites={}", &last[last.len().saturating_sub(40)..], srv.conns.get(), rewrites.get());
            assert!(srv.conns.get() > before, "the pool's own behindAgainMs (100 ms, then the 1 s dial pace) is the first wait of the token refusal count; Rust waits the constant 60 s");
            pool.close();
            srv.stop();
        });
    }

    // The default log is console::warn, an eprintln!. std panics when the write fails (a stderr pipe nobody reads), where
    // node's console.warn drops the error: the panic lands inside fill and leaves the workspace with no dial scheduled.
    // F3 BLOCKER: TS packages/runtime/src/pipe.ts:152
    #[test]
    fn p12_the_pool_still_dials_when_the_default_log_cannot_write_to_a_broken_stderr() {
        if std::env::var("REVIEW_STDERR_CHILD").is_ok() {
            std::panic::set_hook(Box::new(|info| {
                let _ = std::fs::write("/tmp/rev-p12-panic.txt", info.to_string());
            }));
            support::run_local(async {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let port = listener.local_addr().unwrap().port();
                let accepted = Rc::new(Cell::new(0usize));
                let a = accepted.clone();
                let _server = spawn_local(async move {
                    loop {
                        let Ok((tcp, _)) = listener.accept().await else { return };
                        a.set(a.get() + 1);
                        spawn_local(async move {
                            let _ws = accept_async(tcp).await;
                            futures::future::pending::<()>().await;
                        });
                    }
                });
                support::sleep_ms(300).await;
                let pool = pipe_pool(PipePoolOptions {
                    reach: Rc::new(move |_id: &str| {
                        async move { Ok::<_, String>(Some(PipeReach { url: format!("http://127.0.0.1:{port}/"), token: "t".to_string() })) }.boxed_local()
                    }),
                    version: Rc::new(|_id: &str| async { Ok::<_, String>(Some(PIPE_DAEMON_VERSION)) }.boxed_local()),
                    handler: Rc::new(|| None),
                    ready: Rc::new(|| true),
                    rewrite: Rc::new(|_id: &str| {}),
                    clock: None,
                    backoff_ms: None,
                    idle_max_ms: None,
                    behind_again_ms: None,
                    dial_rate: None,
                    log: None,
                });
                pool.up("ws-a");
                support::sleep_ms(700).await;
                println!("CHILD accepted={}", accepted.get());
                pool.close();
            });
            return;
        }
        let exe = std::env::current_exe().unwrap();
        let mut child = std::process::Command::new(exe)
            .args(["pool::p12_the_pool_still_dials_when_the_default_log_cannot_write_to_a_broken_stderr", "--exact", "--nocapture", "--test-threads", "1"])
            .env("REVIEW_STDERR_CHILD", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        drop(child.stderr.take());
        let mut out = String::new();
        std::io::Read::read_to_string(child.stdout.as_mut().unwrap(), &mut out).unwrap();
        let _ = child.wait();
        println!("P12 panic hook saw: {:?}", std::fs::read_to_string("/tmp/rev-p12-panic.txt").unwrap_or_default());
        println!("P12 {}", out.lines().find(|l| l.starts_with("CHILD")).unwrap_or("CHILD printed nothing"));
        assert!(out.contains("CHILD accepted=4"), "the TS pool dials its four pipes whatever console.warn can write; child said: {out}");
    }
}
