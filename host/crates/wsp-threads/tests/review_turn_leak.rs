// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448): a finished turn whose row leaves `sessions` is freed. The TS frees it with the row
//! (transcripts.ts:589-607 `capSessions` drops the oldest finished rows past 200 a workspace; sessions.ts:1443 and
//! :1473 drop a deleted thread's rows); nothing but the row object holds the turn's closures once its process is
//! over. Two cycles keep it here: the row holds `handle` and `end`, which hold the `TurnState`, which holds the row
//! (turns/run.rs:736, :878-882, :895-896 and :57), and the state holds the harness session, which holds the forward it was
//! opened with, which holds the state (run.rs:828-829, :851; a real session keeps its forward, as claude's `Turn`
//! does at wsp-adapters claude/turn.rs:391).
#![allow(dead_code, unused_imports)]

#[path = "support_a/mod.rs"]
mod support;
include!("turns_run/world.rs");

use std::rc::Weak;

/// A session that keeps the forward it was opened with for its life, as every real adapter's session does.
struct KeepsForward {
    inner: Arc<FakeSession>,
    forward: RefCell<Option<OnEvent>>,
}

impl HarnessSession for KeepsForward {
    fn local_id(&self) -> String {
        self.inner.local_id()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.inner.finished()
    }
    fn run(&self) -> Option<String> {
        None
    }
    fn pid(&self) -> Option<u32> {
        None
    }
    fn from(&self) -> Option<u64> {
        None
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.inner.interrupt()
    }
    fn kept(&self) -> Option<Option<Box<dyn KeptAgent>>> {
        None
    }
    fn steer(
        &self,
        _p: String,
        _id: Option<String>,
        _i: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<SteerOutcome, ExecFailure>>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, _a: String, _p: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        None
    }
    fn set_access(&self, _m: String) -> Option<LocalBoxFuture<'static, Result<AccessOutcome, ExecFailure>>> {
        None
    }
    fn stop_task(&self, _t: String) -> Option<LocalBoxFuture<'static, Result<TaskStop, ExecFailure>>> {
        None
    }
}

/// One whole turn on `w1` through `TurnsArea::run_turn`: opened, its session's end event, its result, settled; then
/// its row dropped from `sessions` as `capSessions` drops it. `cut` takes the row's `handle` and `end` off first, which
/// is what breaking the first cycle would do. Answers a weak reference to the dropped row.
async fn one_turn(host: &Rc<FakeHost>, entry: &Entry, i: usize, keeps_forward: bool, cut: bool) -> Weak<RefCell<SessionEntry>> {
    let (turn_id, sess) = (format!("turn-{i}"), format!("sess-{i}"));
    let turn = row(view(&turn_id, W, json!({ "threadId": THREAD, "startedAt": T0, "prompt": "write it" })), &turn_id);
    host.add_row(&turn_id, turn.clone());
    let fake = FakeSession::new(&sess);
    let slot: Rc<RefCell<Option<OnEvent>>> = Rc::new(RefCell::new(None));
    let mut t = turn_start(entry, &turn, &fake, &slot);
    t.turn_id = turn_id.clone();
    if keeps_forward {
        let (inner, held) = (fake.clone(), slot.clone());
        t.open = Box::new(move |f: OnEvent| {
            *held.borrow_mut() = Some(f.clone());
            Ok(Arc::new(KeepsForward { inner, forward: RefCell::new(Some(f)) }) as Arc<dyn HarnessSession>)
        });
    }
    let handle = TurnsArea::new(host.clone()).run_turn(t).expect("the turn opens");
    let forward = slot.borrow_mut().take().expect("open handed the forward over");
    forward(adapter_event(json!({ "type": "session.end", "sessionId": sess, "exitCode": 0, "sawResult": true })));
    fake.finish(result(json!({ "status": "completed", "text": "done" })));
    settle().await;
    let dropped = host.rows.borrow_mut().shift_remove(&sess).expect("the finished turn's row");
    assert_ne!(dropped.borrow().view.status, SessionStatus::Running);
    if cut {
        let mut r = dropped.borrow_mut();
        r.handle = None;
        r.end = None;
    }
    let weak = Rc::downgrade(&dropped);
    drop((dropped, turn, handle, forward, fake, slot));
    weak
}

fn rss_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").expect("/proc/self/status");
    let line = status.lines().find(|l| l.starts_with("VmRSS:")).expect("a VmRSS line");
    line.split_whitespace().nth(1).and_then(|n| n.parse().ok()).expect("VmRSS in kB")
}

// The first cycle: row -> handle and end -> TurnState -> row.
#[test]
fn a_finished_turn_whose_row_is_dropped_is_freed_with_its_row() {
    run_local(async {
        let (host, entry) = world();
        let weak = one_turn(&host, &entry, 1, false, false).await;
        assert!(
            weak.upgrade().is_none(),
            "the row capSessions dropped is still held (strong count {}): its handle and end hold the turn's state, which holds the row",
            weak.strong_count()
        );
    });
}

// The second cycle: TurnState -> session -> the forward it was opened with -> TurnState. The row's handle and end are
// taken off here, so only this cycle can hold the row.
#[test]
fn a_finished_turn_is_freed_though_its_session_keeps_its_forward() {
    run_local(async {
        let (host, entry) = world();
        let weak = one_turn(&host, &entry, 1, true, true).await;
        assert!(
            weak.upgrade().is_none(),
            "the turn's state outlives its row: the session it holds keeps the forward, which holds the state (strong count {})",
            weak.strong_count()
        );
    });
}

// The reading: 20,000 turns, each row dropped as capSessions drops it. The control run takes each dropped row's
// handle and end off by hand; what it grows by is the fake host's own logs. The port's run may grow by no more.
#[test]
fn twenty_thousand_turns_grow_the_process_no_more_than_their_logs() {
    const TURNS: usize = 20_000;
    run_local(async {
        let (host, entry) = world();
        for i in 0..200 {
            one_turn(&host, &entry, i, false, true).await;
        }
        let at_start = rss_kb();
        let mut cut_alive = 0;
        for i in 0..TURNS {
            if one_turn(&host, &entry, 1_000_000 + i, false, true).await.upgrade().is_some() {
                cut_alive += 1;
            }
        }
        let control = rss_kb().saturating_sub(at_start);
        assert_eq!(cut_alive, 0, "the control run frees every row");
        let before = rss_kb();
        let mut alive = 0;
        for i in 0..TURNS {
            if one_turn(&host, &entry, 2_000_000 + i, false, false).await.upgrade().is_some() {
                alive += 1;
            }
        }
        let grown = rss_kb().saturating_sub(before);
        eprintln!(
            "{TURNS} turns: control (handle and end cut) grew {control} kB; the port's run grew {grown} kB, {alive} dropped rows still alive, about {} bytes a turn",
            grown.saturating_sub(control) * 1024 / TURNS as u64
        );
        assert_eq!(alive, 0, "{alive} of {TURNS} dropped rows are still held");
        assert!(grown <= control + 2_048, "the port's {TURNS} turns grew the process by {grown} kB against {control} kB for their logs");
    });
}
