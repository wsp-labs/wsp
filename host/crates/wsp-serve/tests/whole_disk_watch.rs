// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/disk-watch.test.ts at 65f6b89c7, case for case. The disk of a machine whose stop snapshots it
//! (Boat), read on a beat while it runs: past the mark what wsp made there is cleared once per crossing and every thread
//! on it is told how full it is and the next size up with its disk; a move to that size is a nap and a wake that
//! resumes at it.
mod common;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::adapter::{event, result};
use common::stub_backend::Fut;
use common::*;
use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnResult};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{MachineShape, RootDisk, Size};
use wsp_engine::sweep_leftovers::SWEEP_STAGED_CMD;
use wsp_machines::machines::reach::{DISK_CLOSE_WAIT_MS, DISK_READ_MS};
use wsp_wire::types::JsNumber;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::capabilities::{MachineSizeOffer, PauseMode};
use wsp_words::words::thread::{disk_mark_line, DiskMark, DiskMove};

use wsp_account::projects::workspaces::phases::RESIZE_NAP_MS;
use wsp_core::types::wiring::SizeAsk;
use wsp_engine::disk_sync::DISK_USE_CMD;

/// Boat's root disk per size as measured inside a box (2026-10-12): small and default share one, large grows it, and
/// xlarge was never measured, so it has none here.
fn root_disk(size: Size) -> Option<RootDisk> {
    match (size.cpu as u32, size.mem_mb as u32) {
        (2, 4096) => Some(RootDisk { name: "small".into(), disk_gb: 75.0 }),
        (4, 8192) => Some(RootDisk { name: "default".into(), disk_gb: 75.0 }),
        (8, 16384) => Some(RootDisk { name: "large".into(), disk_gb: 160.0 }),
        _ => None,
    }
}

struct Answers;

struct Done(String, SharedFuture<TurnResult>);

impl HarnessSession for Done {
    fn local_id(&self) -> String {
        self.0.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.1.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

impl HarnessAdapter for Answers {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let session_id = "33333333-3333-4333-8333-333333333333".to_string();
        let done = result(json!({ "status": "completed", "text": "built" }));
        let on_event = o.on_event.clone().expect("an event sink");
        on_event(event(json!({ "type": "session.start", "sessionId": session_id })));
        on_event(event(json!({ "type": "turn.done", "sessionId": session_id, "result": serde_json::to_value(&done).expect("a result") })));
        on_event(event(json!({ "type": "session.end", "sessionId": session_id, "exitCode": 0, "sawResult": true })));
        let finished: LocalBoxFuture<'static, TurnResult> = futures::future::ready(done).boxed_local();
        Ok(Arc::new(Done(session_id, finished.shared())))
    }
    fn steers(&self) -> bool {
        false
    }
}

/// A turn that answers at once, so a thread stands on the machine with no turn running.
fn answers() -> HarnessAdapterFactory {
    Arc::new(|_ctx| Ok(Arc::new(Answers) as Arc<dyn HarnessAdapter>))
}

/// A turn that runs until the test releases it: `session.end` without a result, then the turn completes empty.
struct Slow {
    release: Rc<RefCell<Option<Box<dyn FnOnce()>>>>,
}

impl HarnessAdapter for Slow {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let session_id = "44444444-4444-4444-8444-444444444444".to_string();
        let on_event = o.on_event.clone().expect("an event sink");
        on_event(event(json!({ "type": "session.start", "sessionId": session_id })));
        let (tx, rx) = tokio::sync::oneshot::channel::<TurnResult>();
        let ending = session_id.clone();
        *self.release.borrow_mut() = Some(Box::new(move || {
            on_event(event(json!({ "type": "session.end", "sessionId": ending, "exitCode": 0, "sawResult": false })));
            let _ = tx.send(result(json!({ "status": "completed", "text": "" })));
        }));
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn was released") }.boxed_local();
        Ok(Arc::new(Done(session_id, finished.shared())))
    }
    fn steers(&self) -> bool {
        false
    }
}

struct Boat {
    backend: Arc<StubBackend>,
    swept: Rc<RefCell<Vec<String>>>,
    reads: Rc<Cell<usize>>,
}

/// Boat as the stub plays it: a stop that snapshots the disk, two sizes with their disks, and df answering in turn off
/// `readings` (used and free in kB), the last one standing once they run out.
fn boat(readings: Vec<String>) -> Boat {
    let backend = StubBackend::new(None);
    backend.set_capabilities(|c| {
        c.pause_mode = Some(PauseMode::Disk);
        c.sizes = [(2.0, 4096.0, 0.018), (4.0, 8192.0, 0.036), (8.0, 16384.0, 0.072), (16.0, 32768.0, 0.2)]
            .into_iter()
            .map(|(cpu, mem, rate)| MachineSizeOffer { cpu: JsNumber(cpu), mem_mb: JsNumber(mem), rate_usd_per_hour: JsNumber(rate), name: None })
            .collect();
    });
    backend.set_root_disk_of(Some(Rc::new(root_disk)));
    let answer = backend.exec_impl_fn();
    let swept: Rc<RefCell<Vec<String>>> = Rc::default();
    let reads = Rc::new(Cell::new(0usize));
    let (note, count) = (swept.clone(), reads.clone());
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if cmd == DISK_USE_CMD {
            let at = count.get().min(readings.len() - 1);
            count.set(count.get() + 1);
            let said = format!("{}\n", readings[at]);
            return Box::pin(async move { Ok(exec_ok(&said)) });
        }
        if cmd == SWEEP_STAGED_CMD || cmd.contains("rm -rf /root/.npm") {
            note.borrow_mut().push(if cmd == SWEEP_STAGED_CMD { "staged" } else { "caches" }.to_string());
        }
        answer(m, cmd, stdin)
    }));
    Boat { backend, swept, reads }
}

/// df's used and free on a box's root, in kB: 68928192 on a 75 GB disk (read on a small Boat box, 2026-10-12), the
/// filesystem's own blocks and its reserve taken, and the 12 and 160 GB disks at the same share.
const DISK_75_GB_KB: i64 = 68928192;
const DISK_12_GB_KB: i64 = 11004751;
const DISK_160_GB_KB: i64 = 146730014;

/// One df answer for a disk of that many kB at that share used.
fn full(disk_kb: i64, pct: i64) -> String {
    let used = ((disk_kb * pct) as f64 / 100.0).round() as i64;
    format!("{used} {}", disk_kb - used)
}

async fn settle() {
    for _ in 0..20 {
        sleep_ms(2).await;
    }
}

fn notices(events: &Events) -> Vec<Value> {
    events.of("session.notice")
}

async fn first_status(fx: &Fx) -> Value {
    status_rows(&fx.rt).await[0].clone()
}

async fn build(fx: &Fx, workspace: &str) -> Rc<dyn wsp_core::types::wiring::SessionHandle> {
    let turn = fx
        .rt
        .sessions()
        .start(workspace, StartOptions { prompt: "build".into(), ..Default::default() }, None)
        .await
        .expect("a start");
    turn.finished().await;
    turn
}

// "the disk of a machine whose stop snapshots it" > "is read on the beat with no turn ending, and past 90% what wsp made is cleared and the thread told, again only once it fell 5 points under"
#[test]
fn is_read_on_the_beat_with_no_turn_ending_and_past_90_percent_what_wsp_made_is_cleared_and_the_thread_told_again_only_once_it_fell_5_points_under() {
    drive(|| async {
        // 50%, then 93% (the crossing), 92% after the sweep, 88% and 91% (never under 85, so nothing new), 84% (under), and
        // 91% a second crossing.
        let b = boat([50, 93, 92, 88, 91, 84, 91].map(|pct| full(DISK_75_GB_KB, pct)).to_vec());
        let fc = FakeClock::at_now();
        let fx = runtime(&b.backend).clock(&fc).adapter("claude", answers()).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        let turn = build(&fx, &ws.view.id).await;
        settle().await;
        assert!(notices(&events).is_empty());

        fc.advance(DISK_READ_MS as i64);
        settle().await;
        assert_eq!(*b.swept.borrow(), ["staged", "caches"]);
        let thread = turn.view().thread_id.expect("a thread");
        let text = disk_mark_line(&DiskMark {
            machine: "web".into(),
            on: "default".into(),
            pct: 92.0,
            used_bytes: 63413937.0 * 1024.0,
            size_bytes: DISK_75_GB_KB as f64 * 1024.0,
            freed_bytes: 689282.0 * 1024.0,
            machine_goes: false,
            r#move: Some(DiskMove { thread: thread.clone(), name: "large".into(), cpu: 8.0, mem_mb: 16384.0, disk_gb: 160.0 }),
        });
        let told = notices(&events);
        assert_eq!(told.len(), 1);
        assert_eq!(told[0]["threadId"], thread.as_str());
        assert!(told[0]["turnId"].is_string());
        assert_eq!(told[0]["text"], text.as_str());
        // default keeps small's root disk, so the move offered skips it for large, the first size whose disk is bigger.
        assert!(text.contains("wsp cleared 706 MB of its own staging and build caches there"), "{text}");
        assert!(
            text.contains(&format!("Free space on its disk. Or move it to large (8 vCPU, 16 GB, 160 GB disk): wsp resize {thread} --size 8x16 stops it and resumes it at that size, with its files, which ends a turn running there.")),
            "{text}"
        );
        // /default \(|for files/
        assert!(!text.contains("default (") && !text.contains("for files"), "{text}");
        assert!(first_status(&fx).await["reason"].as_str().is_some_and(|r| r.starts_with("its disk is 92% full: ")));

        // Under the line at 88%, past it at 91%: never 5 points under, so nothing is cleared or said again.
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        assert!(first_status(&fx).await.get("reason").is_none());
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        assert_eq!(notices(&events).len(), 1);
        assert_eq!(*b.swept.borrow(), ["staged", "caches"]);
        // 84%, then past the line again: a second crossing is said again.
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        assert_eq!(notices(&events).len(), 2);
        assert_eq!(*b.swept.borrow(), ["staged", "caches", "staged", "caches"]);
        fx.rt.close().await.expect("a close");
    });
}

// "a machine still clearing holds back no other machine's read on the beats after"
#[test]
fn a_machine_still_clearing_holds_back_no_other_machines_read_on_the_beats_after() {
    drive(|| async {
        let b = boat(vec!["500000 500000".into()]);
        let held = Gate::new();
        let answer = b.backend.exec_impl_fn();
        let reads: Rc<RefCell<HashMap<String, u32>>> = Rc::default();
        let counted = reads.clone();
        let waiting = held.clone();
        b.backend.set_exec_impl(Rc::new(move |m, cmd, stdin| -> Fut<_> {
            if cmd == DISK_USE_CMD {
                let n = {
                    let mut reads = counted.borrow_mut();
                    let n = reads.get(m.id()).copied().unwrap_or(0) + 1;
                    reads.insert(m.id().to_string(), n);
                    n
                };
                // Under the line at the turn's end, past it from the first beat on.
                let said = if n == 1 { "500000 500000\n" } else { "950000 50000\n" };
                return Box::pin(async move { Ok(exec_ok(said)) });
            }
            // The first machine's clearing sits on Boat for minutes; the other's answers at once.
            if cmd == SWEEP_STAGED_CMD && m.id() == "m1" {
                let (waiting, answer) = (waiting.clone(), answer.clone());
                return Box::pin(async move {
                    waiting.wait().await;
                    answer(m, cmd, stdin).await
                });
            }
            answer(m, cmd, stdin)
        }));
        let fc = FakeClock::at_now();
        let fx = runtime(&b.backend).clock(&fc).adapter("claude", answers()).build();
        for name in ["web", "api"] {
            let ws = create_on(&fx.rt, CreateOn::golden(name).cpu(2.0).mem(4096.0), None).await.expect("a workspace");
            build(&fx, &ws.view.id).await;
        }
        settle().await;
        let (stuck, other) = ("m1".to_string(), "m2".to_string());
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        let after = (reads.borrow()[&stuck], reads.borrow()[&other]);
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        assert_eq!(reads.borrow()[&other], after.1 + 2);
        assert_eq!(reads.borrow()[&stuck], after.0);
        held.open();
        settle().await;
        fx.rt.close().await.expect("a close");
    });
}

// "a host closing waits on a read still clearing only so long"
#[test]
fn a_host_closing_waits_on_a_read_still_clearing_only_so_long() {
    drive(|| async {
        let b = boat(vec!["950000 50000".into()]);
        let answer = b.backend.exec_impl_fn();
        // The clearing never answers, as a sweep can sit for its whole five minute guard.
        b.backend.set_exec_impl(Rc::new(move |m, cmd, stdin| -> Fut<_> {
            if cmd == SWEEP_STAGED_CMD {
                return Box::pin(futures::future::pending());
            }
            answer(m, cmd, stdin)
        }));
        let fx = runtime(&b.backend).adapter("claude", answers()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        build(&fx, &ws.view.id).await;
        settle().await;
        let began = Instant::now();
        fx.rt.close().await.expect("a close");
        assert!(began.elapsed() < Duration::from_millis(DISK_CLOSE_WAIT_MS as u64 + 2_000), "{:?}", began.elapsed());
    });
}

// "tells a sub-thread's machine it goes once its thread is done, and offers a move only where one is given"
#[test]
fn tells_a_sub_threads_machine_it_goes_once_its_thread_is_done_and_offers_a_move_only_where_one_is_given() {
    let words = DiskMark {
        machine: "fix-upload".into(),
        on: "Boat".into(),
        pct: 93.4,
        used_bytes: 65.6e9,
        size_bytes: 70.6e9,
        freed_bytes: 0.0,
        machine_goes: false,
        r#move: None,
    };
    assert_eq!(
        disk_mark_line(&DiskMark { machine_goes: true, ..words.clone() }),
        "The disk of fix-upload is 93.4% used (65.6 of 70.6 GB) and still filling. Boat cannot save a stop of a full disk, and deletes one whose stops keep failing with its disk. wsp found none of its own staging or build caches there to clear. Free space on its disk, or let the thread finish: its machine is deleted once it is done."
    );
    // /to clear\. Free space on its disk\.$/
    assert!(disk_mark_line(&DiskMark { machine_goes: false, ..words }).ends_with("to clear. Free space on its disk."));
}

// "names the disk and the size it moves to in one unit, decimal GB as Boat sells them"
#[test]
fn names_the_disk_and_the_size_it_moves_to_in_one_unit_decimal_gb_as_boat_sells_them() {
    // df on a small box's 75 GB disk (2026-10-12): 25745280 kB used, 43182912 free.
    let text = disk_mark_line(&DiskMark {
        machine: "web".into(),
        on: "Boat".into(),
        pct: 37.4,
        used_bytes: 25745280.0 * 1024.0,
        size_bytes: (25745280.0 + 43182912.0) * 1024.0,
        freed_bytes: 689282.0 * 1024.0,
        machine_goes: false,
        r#move: Some(DiskMove { thread: "t1".into(), name: "large".into(), cpu: 8.0, mem_mb: 16384.0, disk_gb: 160.0 }),
    });
    assert!(text.contains("(26.4 of 70.6 GB)"), "{text}");
    assert!(text.contains("wsp cleared 706 MB"), "{text}");
    assert!(text.contains("160 GB disk"), "{text}");
}

/// `/Or move it to (.*?): wsp resize \S+ --size (\S+) /` joined by a space.
fn offer(text: &str) -> Option<String> {
    let rest = &text[text.find("Or move it to ")? + "Or move it to ".len()..];
    let colon = rest.find(": wsp resize ")?;
    let after = &rest[colon + ": wsp resize ".len()..];
    let tail = after[after.find(' ')?..].strip_prefix(" --size ")?;
    let size = &tail[..tail.find(' ')?];
    Some(format!("{} {size}", &rest[..colon]))
}

// "offers the first size up that grows the disk the machine reads, and none on large"
#[test]
fn offers_the_first_size_up_that_grows_the_disk_the_machine_reads_and_none_on_large() {
    drive(|| async {
        // A small box on Boat's older 12 GB disk moves to default; one on the 75 GB disk default shares moves to large; on
        // large nothing is offered, since xlarge's disk was never measured.
        let mut offers: Vec<Option<String>> = vec![];
        for (cpu, mem_mb, disk_kb) in [(2.0, 4096.0, DISK_12_GB_KB), (2.0, 4096.0, DISK_75_GB_KB), (8.0, 16384.0, DISK_160_GB_KB)] {
            let b = boat(vec![full(disk_kb, 95)]);
            let fx = runtime(&b.backend).adapter("claude", answers()).build();
            let events = fx.events();
            let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(cpu).mem(mem_mb), None).await.expect("a workspace");
            build(&fx, &ws.view.id).await;
            settle().await;
            let told = notices(&events);
            assert_eq!(told.len(), 1);
            offers.push(offer(told[0]["text"].as_str().expect("a text")));
            fx.rt.close().await.expect("a close");
        }
        assert_eq!(
            offers,
            [Some("default (4 vCPU, 8 GB, 75 GB disk) 4x8".to_string()), Some("large (8 vCPU, 16 GB, 160 GB disk) 8x16".to_string()), None]
        );
    });
}

// "reads nothing on a machine whose stop keeps memory"
#[test]
fn reads_nothing_on_a_machine_whose_stop_keeps_memory() {
    drive(|| async {
        let fc = FakeClock::at_now();
        let memory = boat(vec!["950000 50000".into()]);
        memory.backend.set_capabilities(|c| c.pause_mode = Some(PauseMode::Memory));
        let other = runtime(&memory.backend).clock(&fc).adapter("claude", answers()).build();
        let on = create_on(&other.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        build(&other, &on.view.id).await;
        fc.advance(DISK_READ_MS as i64);
        settle().await;
        assert_eq!(memory.reads.get(), 0);
        other.rt.close().await.expect("a close");
    });
}

/// What `movable(backend)` of the TS collects: the sizes each resume landed at.
struct Movable {
    resumed_at: Rc<RefCell<Vec<(f64, f64)>>>,
    backend: Arc<StubBackend>,
}

impl Movable {
    /// The `waitMs` of every pause on a machine made after `movable` ran, as `pausedWith` collects them.
    fn paused_with(&self) -> Vec<Option<u64>> {
        self.backend.machines().iter().flat_map(|m| m.pause_waits.borrow().clone()).collect()
    }
}

fn landing_at_a_size(m: &Arc<StubMachine>, resumed_at: Option<Rc<RefCell<Vec<(f64, f64)>>>>, then: Option<&'static str>) {
    let machine = m.clone();
    m.set_resume_at(Some(Rc::new(move |cpu, mem_mb, _| {
        if let Some(resumed_at) = &resumed_at {
            resumed_at.borrow_mut().push((cpu, mem_mb as f64));
        }
        machine.paused.set(false);
        machine.set_shape(MachineShape {
            cpu: Some(JsNumber(cpu)),
            mem_mb: Some(JsNumber(mem_mb as f64)),
            ..machine.shape_now()
        });
        let failure = then.map(|message| EngineError::Other { message: message.into(), code: None });
        Box::pin(async move { failure.map_or(Ok(()), Err) })
    })));
}

/// The stub's machines resume at a size as Boat does: the shape the provider reports follows it.
fn movable(backend: &Arc<StubBackend>) -> Movable {
    let resumed_at: Rc<RefCell<Vec<(f64, f64)>>> = Rc::default();
    let create = backend.create_fn();
    let landed = resumed_at.clone();
    backend.set_create(Rc::new(move |spec, waiting, sent| {
        let made = create(spec, waiting, sent);
        let landed = landed.clone();
        Box::pin(async move {
            let m = made.await?;
            landing_at_a_size(&m, Some(landed), None);
            Ok(m)
        })
    }));
    Movable { resumed_at, backend: backend.clone() }
}

fn size_ask(cpu: f64, mem_mb: f64) -> SizeAsk {
    SizeAsk { cpu: JsNumber(cpu), mem_mb: JsNumber(mem_mb) }
}

// "a move to another size" > "naps the machine and wakes it at the size asked, the record and its rate following"
#[test]
fn naps_the_machine_and_wakes_it_at_the_size_asked_the_record_and_its_rate_following() {
    drive(|| async {
        let b = boat(vec!["500000 500000".into()]);
        let moved_by = movable(&b.backend);
        let fx = runtime(&b.backend).adapter("claude", answers()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        build(&fx, &ws.view.id).await;
        let m = b.backend.machine(0);
        let moved = fx.rt.workspaces().resize(&ws.view.id, size_ask(4.0, 8192.0), None).await.expect("a move");
        assert_eq!(*moved_by.resumed_at.borrow(), [(4.0, 8192.0)]);
        // The stop before the move waits as long as a full disk's archive takes (14 minutes for 67 GB, measured 2026-10-01).
        assert_eq!(moved_by.paused_with(), [Some(RESIZE_NAP_MS)]);
        assert_eq!(RESIZE_NAP_MS, 30 * 60_000);
        assert_eq!(m.resumes.get(), 0);
        assert_eq!(j(&moved)["phase"], "running");
        let status = status_rows(&fx.rt).await.into_iter().find(|s| s["id"] == ws.view.id.as_str()).expect("a row");
        assert_matches(&status["size"], json!({ "cpu": 4, "memMb": 8192 }));
        // The next wake checks the machine against the shape it now has, so it is no fault.
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        // The same size again is no move at all.
        fx.rt.workspaces().resize(&ws.view.id, size_ask(4.0, 8192.0), None).await.expect("a resize");
        assert_eq!(moved_by.resumed_at.borrow().len(), 1);
        fx.rt.close().await.expect("a close");
    });
}

// "says so when the machine came back at another size than asked, and the record keeps the size it has"
#[test]
fn says_so_when_the_machine_came_back_at_another_size_than_asked_and_the_record_keeps_the_size_it_has() {
    drive(|| async {
        let b = boat(vec!["500000 500000".into()]);
        movable(&b.backend);
        let fx = runtime(&b.backend).adapter("claude", answers()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        // A plain resume someone else sent: the machine comes back as it was.
        let m = b.backend.machine(0);
        let up = m.clone();
        m.set_resume_at(Some(Rc::new(move |_, _, _| {
            up.paused.set(false);
            Box::pin(async { Ok(()) })
        })));
        let refused = fx.rt.workspaces().resize(&ws.view.id, size_ask(4.0, 8192.0), None).await.expect_err("a refusal");
        assert!(refused.message.contains("web came back at 2x4, not the 4x8 asked for. Run wsp resize again."), "{}", refused.message);
        let status = status_rows(&fx.rt).await.into_iter().find(|s| s["id"] == ws.view.id.as_str()).expect("a row");
        assert_matches(&status["size"], json!({ "cpu": 2, "memMb": 4096 }));
        fx.rt.close().await.expect("a close");
    });
}

// "keeps the size the provider moved it to when the wake fails after the resume landed, so a later wake checks the right shape"
#[test]
fn keeps_the_size_the_provider_moved_it_to_when_the_wake_fails_after_the_resume_landed_so_a_later_wake_checks_the_right_shape() {
    drive(|| async {
        let b = boat(vec!["500000 500000".into()]);
        movable(&b.backend);
        let fx = runtime(&b.backend).adapter("claude", answers()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        let m = b.backend.machine(0);
        // Boat took the resume at large, and the wake failed after it, as a restore still streaming in fails one.
        landing_at_a_size(&m, None, Some("still restoring"));
        let refused = fx.rt.workspaces().resize(&ws.view.id, size_ask(8.0, 16384.0), None).await.expect_err("a refusal");
        assert!(refused.message.contains("still restoring"), "{}", refused.message);
        let status = status_rows(&fx.rt).await.into_iter().find(|s| s["id"] == ws.view.id.as_str()).expect("a row");
        assert_matches(&status["size"], json!({ "cpu": 8, "memMb": 16384 }));
        fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(j(&fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake"))["phase"], "running");
        fx.rt.close().await.expect("a close");
    });
}

// "is refused for a size not sold, on a machine that cannot move, and while a turn runs there"
#[test]
fn is_refused_for_a_size_not_sold_on_a_machine_that_cannot_move_and_while_a_turn_runs_there() {
    drive(|| async {
        let b = boat(vec!["500000 500000".into()]);
        let fx = runtime(&b.backend).adapter("claude", answers()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        let refused = fx.rt.workspaces().resize(&ws.view.id, size_ask(4.0, 8192.0), None).await.expect_err("a refusal");
        assert!(
            refused.message.contains("default changes no machine's size once it is made. Start a new thread at the size you want instead."),
            "{}",
            refused.message
        );
        // A thread's own token is refused in the move's own words: the bigger size is the person's spend.
        let opened = build(&fx, &ws.view.id).await;
        let thread = opened.view().thread_id.expect("a thread");
        let by: ThreadScope = serde_json::from_value(
            json!({ "kind": "thread", "threadId": thread, "workspaceId": ws.view.id, "rootThreadId": thread }),
        )
        .expect("a thread scope");
        let as_thread = Caller::Scoped { origin: WorkspaceOrigin::Here, by };
        let refused = fx.rt.workspaces().resize(&ws.view.id, size_ask(4.0, 8192.0), Some(as_thread)).await.expect_err("a refusal");
        assert!(refused.message.ends_with("never move a machine to another size"), "{}", refused.message);
        fx.rt.close().await.expect("a close");

        let held = boat(vec!["500000 500000".into()]);
        movable(&held.backend);
        let release: Rc<RefCell<Option<Box<dyn FnOnce()>>>> = Rc::default();
        let slow: HarnessAdapterFactory = {
            let release = release.clone();
            Arc::new(move |_ctx| Ok(Arc::new(Slow { release: release.clone() }) as Arc<dyn HarnessAdapter>))
        };
        let busy = runtime(&held.backend).adapter("claude", slow).build();
        let on = create_on(&busy.rt, CreateOn::golden("web").cpu(2.0).mem(4096.0), None).await.expect("a workspace");
        let refused = busy.rt.workspaces().resize(&on.view.id, size_ask(32.0, 65536.0), None).await.expect_err("a refusal");
        // /^32x64 is not a size this provider offers; the sizes are 2x4 \(.*\), 4x8 \(.*\), 8x16 \(.*\), 16x32 \(.*\)\. Ask for one of those instead\.$/
        assert!(
            refused.message.starts_with("32x64 is not a size this provider offers; the sizes are 2x4 (")
                && refused.message.ends_with("). Ask for one of those instead.")
                && in_order(&refused.message, &["2x4 (", "), 4x8 (", "), 8x16 (", "), 16x32 ("]),
            "{}",
            refused.message
        );
        let turn = busy
            .rt
            .sessions()
            .start(&on.view.id, StartOptions { prompt: "build".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        let refused = busy.rt.workspaces().resize(&on.view.id, size_ask(4.0, 8192.0), None).await.expect_err("a refusal");
        assert!(
            refused.message.contains("web has a turn running, and the stop a move to another size takes would end it. Stop the turn or wait for it to end, then move it."),
            "{}",
            refused.message
        );
        assert!(!held.backend.machine(0).paused.get());
        let release_turn = release.borrow_mut().take().expect("a turn to release");
        release_turn();
        turn.finished().await;
        busy.rt.close().await.expect("a close");
    });
}
