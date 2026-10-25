// SPDX-License-Identifier: AGPL-3.0-only
//! pipe_pool: the pool of pipes to a machine's daemon, from the first three groups of
//! packages/runtime/test/pipe.test.ts ("the pipe pool", "... a machine that takes every pipe", "... and the link").
//! Spec: spec/pipe_pool.md.
mod pipe_daemon;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use futures::FutureExt;
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wsp_core::clock::{Clock, FakeClock};
use wsp_core::pipe::pool::{PipeDoor, PipeOpen};
use wsp_core::pipe::{
    pipe_pool, PipeDuplex, PipeHandler, PipePool, PipePoolOptions, PipeReach, PIPE_BEHIND_AGAIN_MS, PIPE_DIAL_RATE, PIPE_IDLE,
    PIPE_IDLE_MAX_MS,
};
use wsp_wire::types::daemon_contract::PIPE_POOL_MAX;
use wsp_wire::wire::daemon_version::PIPE_DAEMON_VERSION;

use pipe_daemon::{random_bytes, sha_hex, sleep, until, until_for, within, Bytes, Daemon, Far, Mode, TOKEN};

struct Handed {
    workspace_id: String,
    open: PipeOpen,
    duplex: Option<PipeDuplex>,
}

type HandedList = Rc<RefCell<Vec<Handed>>>;

/// The duplex the pool handed over as the `i`th, moved out so a test can read and write it across awaits.
fn take_duplex(handed: &HandedList, i: usize) -> PipeDuplex {
    handed.borrow_mut()[i].duplex.take().expect("that duplex was already taken")
}

fn door_word(door: PipeDoor) -> &'static str {
    match door {
        PipeDoor::Http => "http",
        PipeDoor::Gh => "gh",
    }
}

#[derive(Default)]
struct PoolOpts {
    ready: Option<Rc<dyn Fn() -> bool>>,
    version: Option<Rc<dyn Fn() -> Option<i64>>>,
    idle_max_ms: Option<u64>,
    behind_again_ms: Option<u64>,
    dial_rate: Option<usize>,
    backoff_ms: Option<Rc<dyn Fn(u32) -> u64>>,
    no_handler: bool,
    clock: Option<Rc<dyn Clock>>,
}

struct On {
    pool: Rc<PipePool>,
    handed: HandedList,
    logs: Rc<RefCell<Vec<String>>>,
    rewrites: Rc<RefCell<Vec<String>>>,
}

fn pool_on(d: &Daemon, o: PoolOpts) -> On {
    let handed: HandedList = Rc::new(RefCell::new(Vec::new()));
    let logs: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let rewrites: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let handler: PipeHandler = {
        let handed = handed.clone();
        Rc::new(move |workspace_id: &str, open: PipeOpen, duplex: PipeDuplex| {
            handed.borrow_mut().push(Handed { workspace_id: workspace_id.to_string(), open, duplex: Some(duplex) });
            Ok(())
        })
    };
    let url = d.url();
    let version = o.version.clone();
    let ready = o.ready.clone();
    let no_handler = o.no_handler;
    let backoff = o.backoff_ms.clone().unwrap_or_else(|| Rc::new(|_attempt: u32| 20u64));
    let pool = pipe_pool(PipePoolOptions {
        reach: Rc::new(move |_id: &str| {
            let url = url.clone();
            async move { Ok::<_, String>(Some(PipeReach { url, token: TOKEN.to_string() })) }.boxed_local()
        }),
        version: Rc::new(move |_id: &str| {
            let v = match &version {
                None => Some(PIPE_DAEMON_VERSION),
                Some(f) => f(),
            };
            async move { Ok::<_, String>(v) }.boxed_local()
        }),
        handler: Rc::new(move || if no_handler { None } else { Some(handler.clone()) }),
        ready: Rc::new(move || match &ready {
            None => true,
            Some(f) => f(),
        }),
        rewrite: {
            let rewrites = rewrites.clone();
            Rc::new(move |id: &str| rewrites.borrow_mut().push(id.to_string()))
        },
        clock: o.clock.clone(),
        backoff_ms: Some(backoff),
        idle_max_ms: o.idle_max_ms,
        behind_again_ms: o.behind_again_ms,
        dial_rate: o.dial_rate,
        log: {
            let logs = logs.clone();
            Some(Rc::new(move |line: &str| logs.borrow_mut().push(line.to_string())))
        },
    });
    On { pool, handed, logs, rewrites }
}

mod the_pipe_pool {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn keeps_4_idle_pipes_each_authed_with_the_daemon_s_token_as_a_pipe_and_dials_no_mo() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == PIPE_IDLE).await;
                sleep(100).await;
                assert_eq!(PIPE_IDLE, 4);
                assert_eq!(d.parked_len(), 4);
                assert_eq!(d.first_frames(), vec![json!({ "id": 0, "op": "auth", "token": TOKEN, "pipe": true }); 4]);
                assert_eq!(on.pool.idle("ws-a"), 4);
                assert_eq!(*on.logs.borrow(), vec!["ws-a: dialling 4 pipes, 4 kept idle".to_string()]);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dials_a_replacement_for_each_pipe_taken_and_hands_it_over_under_the_workspace_it() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                d.take("http", 1200);
                until(|| on.handed.borrow().len() == 1 && d.parked_len() == 4).await;
                assert_eq!(on.handed.borrow()[0].workspace_id, "ws-a");
                assert_eq!(on.handed.borrow()[0].open, PipeOpen::new(PipeDoor::Http, 1200));
                assert_eq!(d.first_frame_count(), 5);
                assert_eq!(
                    on.logs.borrow()[1..].to_vec(),
                    vec!["ws-a: a pipe opened on the http door after 1200 ms held, dialling its replacement".to_string()]
                );
                for i in 0..4 {
                    d.take(if i % 2 == 0 { "gh" } else { "http" }, 0);
                }
                until(|| on.handed.borrow().len() == 5 && d.parked_len() == 4).await;
                assert_eq!(d.first_frame_count(), 9);
                // Four sockets, so the host may read their pipe.open frames in any order.
                let mut doors: Vec<&str> = on.handed.borrow()[1..].iter().map(|h| door_word(h.open.door)).collect();
                doors.sort();
                assert_eq!(doors, vec!["gh", "gh", "http", "http"]);
                sleep(100).await;
                assert_eq!(d.first_frame_count(), 9);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn carries_a_taken_pipe_s_bytes_both_ways_binary_and_unchanged_and_either_end_closi() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                let far = d.take("http", 0);
                until(|| on.handed.borrow().len() == 1).await;
                let mut duplex = take_duplex(&on.handed, 0);
                let request = b"POST /v1/messages HTTP/1.1\r\nhost: 127.0.0.1:7071\r\ncontent-length: 2\r\n\r\n{}".to_vec();
                far.send(request.clone());
                let mut read = vec![0u8; request.len()];
                within(2000, duplex.read_exact(&mut read))
                    .await
                    .expect("the request reached the handler in time")
                    .expect("read the request");
                assert_eq!(read, request);
                let reply = random_bytes(70_000);
                duplex.write_all(&reply).await.expect("write the reply");
                duplex.flush().await.expect("flush the reply");
                // Frames are binary and carry the reply's bytes unchanged; a write may be one frame or several.
                let mut got = Vec::new();
                within(2000, async {
                    while got.len() < reply.len() {
                        got.extend(far.next_frame().await);
                    }
                })
                .await
                .expect("the reply reached the daemon in time");
                assert_eq!(sha_hex(&got), sha_hex(&reply));
                assert_eq!(got.len(), reply.len());
                far.close(1000);
                let mut rest = Vec::new();
                within(5000, async {
                    let _ = duplex.read_to_end(&mut rest).await;
                })
                .await
                .expect("the handler's side ended when the daemon closed");
                // And the other way: the handler ending its side closes the pipe at the daemon.
                let far2 = d.take("gh", 0);
                until(|| on.handed.borrow().len() == 2).await;
                let closed = far2.closed();
                let mut duplex2 = take_duplex(&on.handed, 1);
                duplex2.shutdown().await.expect("end the handler's side");
                within(5000, closed).await.expect("the daemon's end closed");
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn holds_the_daemon_back_while_the_handler_reads_nothing_and_gives_every_byte_once() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                let far = d.take("http", 0);
                until(|| on.handed.borrow().len() == 1).await;
                let mut duplex = take_duplex(&on.handed, 0);
                let mut hash = Sha256::new();
                let chunk = Bytes::from(random_bytes(1 << 20));
                let mut sent = 0usize;
                let mut stalled = false;
                // Sent until a 1 MiB frame cannot be flushed within 200 ms: the host stopped reading, so TCP pushed back.
                while sent < 512 << 20 {
                    let ack = far.send_bytes(chunk.clone());
                    hash.update(&chunk);
                    sent += chunk.len();
                    if tokio::time::timeout(Duration::from_millis(200), ack).await.is_err() {
                        stalled = true;
                        break;
                    }
                }
                sleep(200).await;
                assert!(stalled, "the daemon sent {sent} bytes and never stalled");
                assert!(sent < 128 << 20, "the daemon's sends stalled only after {sent} bytes");
                let want = hex::encode(hash.finalize());
                let mut got = Sha256::new();
                let mut read = 0usize;
                let mut buf = vec![0u8; 64 << 10];
                let done = within(20_000, async {
                    while read < sent {
                        let n = duplex.read(&mut buf).await.expect("read the pipe");
                        assert!(n > 0, "the pipe ended after {read} of {sent} bytes");
                        got.update(&buf[..n]);
                        read += n;
                    }
                })
                .await;
                assert!(done.is_some(), "read only {read} of {sent} bytes");
                assert_eq!(read, sent);
                assert_eq!(hex::encode(got.finalize()), want);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn holds_the_handler_s_writes_back_while_the_daemon_reads_nothing_and_delivers_ever() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                let far = d.take("http", 0);
                until(|| on.handed.borrow().len() == 1).await;
                let mut duplex = take_duplex(&on.handed, 0);
                far.discard();
                far.pause();
                let mut hash = Sha256::new();
                let chunk = random_bytes(1 << 20);
                let mut written = 0usize;
                let mut stuck = false;
                let mut left_at = 0usize;
                // Written as a well-behaved writer writes, waiting on each write, until one does not complete: the daemon
                // reads nothing, so once the kernel's buffers are full the duplex takes no more. A write that is not
                // ready has taken no bytes, so the count of bytes it took is exact.
                while !stuck && written < 512 << 20 {
                    let mut off = 0;
                    while off < chunk.len() {
                        match tokio::time::timeout(Duration::from_millis(300), duplex.write(&chunk[off..])).await {
                            Ok(n) => {
                                let n = n.expect("write the pipe");
                                hash.update(&chunk[off..off + n]);
                                written += n;
                                off += n;
                            }
                            Err(_) => {
                                stuck = true;
                                left_at = off;
                                break;
                            }
                        }
                    }
                }
                assert!(stuck, "the handler wrote {written} bytes and was never held back");
                assert_eq!(far.received(), 0);
                assert!(written < 128 << 20, "the handler's writes held back only after {written} bytes");
                far.resume();
                let rest = &chunk[left_at..];
                within(20_000, duplex.write_all(rest))
                    .await
                    .expect("the held write completed once the daemon read")
                    .expect("write the rest of the chunk");
                hash.update(rest);
                written += rest.len();
                within(20_000, duplex.flush()).await.expect("flushed in time").expect("flush");
                until_for(20_000, || far.received() == written).await;
                assert_eq!(far.digest_hex(), hex::encode(hash.finalize()));
            })
            .await;
    }

    // A clone's pack crosses one pipe over a 150 ms road, where a window of its own (64 KiB a round trip) is 0.4 MB/s:
    // nothing here waits on the daemon's word before the next bytes go (#2352).
    #[tokio::test(flavor = "current_thread")]
    async fn puts_8_mib_through_to_a_daemon_that_answers_nothing_with_no_window_of_its_own_ho() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                let far = d.take("http", 0);
                until(|| on.handed.borrow().len() == 1).await;
                let mut duplex = take_duplex(&on.handed, 0);
                far.discard();
                let chunk = random_bytes(64 << 10);
                let total = 8usize << 20;
                let mut written = 0usize;
                while written < total {
                    let wrote = within(5_000, duplex.write_all(&chunk)).await;
                    assert!(matches!(wrote, Some(Ok(()))), "the writer waited after {written} bytes with {} received", far.received());
                    written += chunk.len();
                }
                within(10_000, duplex.flush()).await.expect("flushed in time").expect("flush");
                until_for(10_000, || far.received() == total).await;
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn replaces_a_pipe_left_idle_past_its_limit_with_a_fresh_one_and_logs_no_dial_for_i() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                assert_eq!(PIPE_IDLE_MAX_MS, 600_000);
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts { idle_max_ms: Some(300), ..Default::default() });
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                let first = d.parked();
                let closed: Vec<_> = first.iter().map(|ws| ws.closed()).collect();
                within(5000, futures::future::join_all(closed)).await.expect("every first pipe was closed");
                until(|| {
                    let parked = d.parked();
                    parked.len() == 4 && parked.iter().all(|ws| !first.iter().any(|f| Rc::ptr_eq(f, ws)))
                })
                .await;
                assert!(d.first_frame_count() >= 8);
                assert_eq!(*on.logs.borrow(), vec!["ws-a: dialling 4 pipes, 4 kept idle".to_string()]);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closes_the_connection_a_pipe_brings_when_nothing_serves_pipes_yet_and_still_dial() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts { no_handler: true, ..Default::default() });
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                let far = d.take("http", 0);
                within(5000, far.closed()).await.expect("the connection was closed");
                until(|| d.parked_len() == 4).await;
                assert!(on.logs.borrow().contains(&"ws-a: nothing here serves pipes yet, so that connection was closed".to_string()));
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dials_nothing_while_the_switch_reads_off_and_fills_the_pool_once_it_reads_on() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on_switch = Rc::new(Cell::new(false));
                let switch = on_switch.clone();
                let on = pool_on(&d, PoolOpts { ready: Some(Rc::new(move || switch.get())), ..Default::default() });
                on.pool.up("ws-a");
                sleep(150).await;
                assert_eq!(d.first_frame_count(), 0);
                on_switch.set(true);
                until(|| d.parked_len() == 4).await;
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dials_no_pipe_to_a_daemon_older_than_the_pipes_and_fills_the_pool_once_an_update() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                assert_eq!(PIPE_BEHIND_AGAIN_MS, 60_000);
                let d = Daemon::start(Mode::Pipes).await;
                let version = Rc::new(Cell::new(PIPE_DAEMON_VERSION - 1));
                let asked = Rc::new(Cell::new(0usize));
                let (v, a) = (version.clone(), asked.clone());
                let on = pool_on(
                    &d,
                    PoolOpts {
                        behind_again_ms: Some(150),
                        version: Some(Rc::new(move || {
                            a.set(a.get() + 1);
                            Some(v.get())
                        })),
                        ..Default::default()
                    },
                );
                on.pool.up("ws-a");
                until(|| asked.get() == 2).await;
                assert_eq!(d.first_frame_count(), 0);
                assert_eq!(
                    *on.logs.borrow(),
                    vec![format!("ws-a: its daemon ({}) serves no pipes yet, so it is asked again each minute", PIPE_DAEMON_VERSION - 1)]
                );
                version.set(PIPE_DAEMON_VERSION);
                until(|| d.parked_len() == 4).await;
                assert_eq!(on.pool.idle("ws-a"), 4);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closes_a_pipe_the_daemon_answers_as_a_link_and_asks_again_each_minute_saying_so() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Old).await;
                let t = FakeClock::new(0);
                let asked = Rc::new(Cell::new(0usize));
                let a = asked.clone();
                let on = pool_on(
                    &d,
                    PoolOpts {
                        clock: Some(t.clock()),
                        version: Some(Rc::new(move || {
                            a.set(a.get() + 1);
                            Some(PIPE_DAEMON_VERSION)
                        })),
                        ..Default::default()
                    },
                );
                on.pool.up("ws-a");
                let behind_ms = PIPE_BEHIND_AGAIN_MS as i64;
                for minute in 1..=3usize {
                    until(|| t.due_for(minute as i64 * behind_ms)).await;
                    until(|| d.open() == 0).await;
                    assert_eq!(on.pool.idle("ws-a"), 0);
                    if minute == 3 {
                        d.become_mode(Mode::Pipes);
                    }
                    t.advance(behind_ms - 1);
                    assert_eq!(asked.get(), minute);
                    t.advance(1);
                    assert_eq!(asked.get(), minute + 1);
                }
                let said = on
                    .logs
                    .borrow()
                    .iter()
                    .filter(|l| l.as_str() == "ws-a: its daemon answered a pipe as a link, so it is asked again each minute")
                    .count();
                assert_eq!(said, 1);
                until(|| d.parked_len() == 4).await;
                assert_eq!(on.pool.idle("ws-a"), 4);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn has_the_host_s_token_written_again_when_the_daemon_refuses_it_by_its_close_or_it() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                for mode in [Mode::Refuse, Mode::Deny] {
                    let d = Daemon::start(mode).await;
                    let t = FakeClock::at_now();
                    let on = pool_on(&d, PoolOpts { clock: Some(t.clock()), ..Default::default() });
                    on.pool.up("ws-a");
                    until(|| on.rewrites.borrow().len() == 1).await;
                    sleep(100).await;
                    assert!(d.first_frame_count() <= 4);
                    assert_eq!(on.pool.idle("ws-a"), 0);
                    assert_eq!(
                        on.logs.borrow().last().cloned(),
                        Some("ws-a: its daemon refused the token a pipe was dialled with, so the host writes its own there again and dials in 60 s".to_string())
                    );
                    d.become_mode(Mode::Pipes);
                    t.advance(PIPE_BEHIND_AGAIN_MS as i64 - 1);
                    sleep(100).await;
                    assert_eq!(d.parked_len(), 0);
                    t.advance(1);
                    until(|| d.parked_len() == 4).await;
                    assert_eq!(*on.rewrites.borrow(), vec!["ws-a".to_string()]);
                }
            })
            .await;
    }
}

mod the_pipe_pool_and_a_machine_that_takes_every_pipe {
    use super::*;

    fn open_at(d: &Daemon) -> usize {
        d.taken().iter().filter(|ws| ws.is_open()).count()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dials_no_more_than_4_pipes_a_second_for_a_workspace_whose_machine_takes_each_one() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                assert_eq!(PIPE_DIAL_RATE, 4);
                let d = Daemon::start(Mode::Grab).await;
                let t = FakeClock::at_now();
                let on = pool_on(&d, PoolOpts { clock: Some(t.clock()), ..Default::default() });
                on.pool.up("ws-a");
                // 4 at once, then 4 more each second.
                until(|| d.first_frame_count() >= 4).await;
                sleep(100).await;
                assert_eq!(d.first_frame_count(), 4);
                t.advance(1000);
                until(|| d.first_frame_count() >= 8).await;
                sleep(100).await;
                assert_eq!(d.first_frame_count(), 8);
                assert_eq!(on.handed.borrow().len(), d.first_frame_count());
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn holds_no_more_than_64_pipes_a_workspace_s_machine_has_taken_and_dials_again_only() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Grab).await;
                let on = pool_on(&d, PoolOpts { dial_rate: Some(1000), ..Default::default() });
                on.pool.up("ws-a");
                until(|| open_at(&d) == PIPE_POOL_MAX as usize).await;
                sleep(200).await;
                assert_eq!(PIPE_POOL_MAX, 64);
                assert_eq!(open_at(&d), 64);
                assert_eq!(d.first_frame_count(), 64);
                assert!(on.logs.borrow().contains(&"ws-a: 64 pipes are in use, so the next connection waits for one to close".to_string()));
                // 4 idle at the start and one more per take reach 64 after 60 takes; the 4 taken after that dial nothing.
                let replaced = on.logs.borrow().iter().filter(|l| l.ends_with(", dialling its replacement")).count();
                let waiting = on.logs.borrow().iter().filter(|l| l.ends_with(", its replacement waits for a pipe to close")).count();
                assert_eq!(replaced, 60);
                assert_eq!(waiting, 4);
                for ws in d.taken().iter().take(3) {
                    ws.close(1000);
                }
                until(|| d.first_frame_count() == 67 && open_at(&d) == 64).await;
                sleep(200).await;
                assert_eq!(d.first_frame_count(), 67);
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn backs_off_further_each_time_the_daemon_refuses_a_pipe_as_past_its_parked_pipes() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Full).await;
                let attempts: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(Vec::new()));
                let seen = attempts.clone();
                let on = pool_on(
                    &d,
                    PoolOpts {
                        dial_rate: Some(1000),
                        backoff_ms: Some(Rc::new(move |a: u32| {
                            seen.borrow_mut().push(a);
                            40u64
                        })),
                        ..Default::default()
                    },
                );
                on.pool.up("ws-a");
                until(|| attempts.borrow().len() >= 5).await;
                on.pool.down("ws-a");
                assert_eq!(attempts.borrow()[..5].to_vec(), vec![1, 2, 3, 4, 5]);
            })
            .await;
    }
}

mod the_pipe_pool_and_the_link {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn closes_every_pipe_of_a_workspace_when_its_link_drops_and_dials_4_again_when_it_r() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                until(|| d.parked_len() == 4).await;
                d.take("http", 0);
                until(|| on.handed.borrow().len() == 1 && d.parked_len() == 4).await;
                let mut in_flight = take_duplex(&on.handed, 0);
                d.drop_network().await;
                until(|| on.pool.idle("ws-a") == 0).await;
                within(5000, async {
                    let mut rest = Vec::new();
                    let _ = in_flight.read_to_end(&mut rest).await;
                })
                .await
                .expect("the pipe the machine had taken ended with the link");
                sleep(100).await;
                assert_eq!(on.pool.idle("ws-a"), 0);
                d.back().await;
                until_for(5000, || d.parked_len() == 4 && on.pool.idle("ws-a") == 4).await;
                sleep(100).await;
                assert_eq!(d.parked_len(), 4);
                assert_eq!(
                    on.logs.borrow()[2..].to_vec(),
                    vec![
                        "ws-a: a pipe could not be dialled, so its others were closed and the pool is dialled again as the link comes back"
                            .to_string(),
                        "ws-a: 4 pipes idle".to_string(),
                    ]
                );
            })
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn closes_every_pipe_taken_ones_too_when_the_link_stops_dials_nothing_after_and_4_a() {
        tokio::task::LocalSet::new()
            .run_until(async move {
                let d = Daemon::start(Mode::Pipes).await;
                let on = pool_on(&d, PoolOpts::default());
                on.pool.up("ws-a");
                on.pool.up("ws-b");
                until(|| d.parked_len() == 8).await;
                let far: Far = d.take("http", 0);
                until(|| on.handed.borrow().len() == 1 && d.parked_len() == 8).await;
                let a_only = on.handed.borrow()[0].workspace_id.clone();
                let far_closed = far.closed();
                on.pool.down(&a_only);
                within(5000, far_closed).await.expect("the taken pipe was closed");
                until(|| d.parked_len() == 4).await;
                sleep(150).await;
                assert_eq!(d.parked_len(), 4);
                assert_eq!(on.pool.idle(&a_only), 0);
                let other = if a_only == "ws-a" { "ws-b" } else { "ws-a" };
                assert_eq!(on.pool.idle(other), 4);
                let dialled = d.first_frame_count();
                on.pool.up(&a_only);
                until(|| d.parked_len() == 8).await;
                assert_eq!(d.first_frame_count() - dialled, 4);
            })
            .await;
    }
}
