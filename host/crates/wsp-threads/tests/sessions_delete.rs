// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_delete.md. TS cases: forget.test.ts (sessions.forget), folder-threads.test.ts:672-1135 (delete),
//! host/test/activity.test.ts:264-330 (delete with processes), box-panels.test.ts (the group and the shells),
//! owed-box.test.ts and box-stop.test.ts (a held send). Every effect is a hook that writes one line to `Rig::log`, so
//! each case asserts the whole order of what a delete did.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::channel::oneshot;
use support::*;
use wsp_wire::types::projects::{THREAD_WORKING_LINE, WORKTREE_BUSY_LINE};
use wsp_wire::views::session::SessionStatus;
use wsp_words::exit::{not_found_refusal, HostError};
use wsp_words::projects::thread_on_machine_line;
use wsp_words::words::computer::{send_given_up_line, thread_forget_refusal, thread_word};

const W: &str = "w1";
const T: &str = "thread-aa";
const B: &str = "thread-bb";
const WORKTREE: &str = "/wt/feat-x";

fn conflict(line: &str) -> (String, Option<String>, Option<Kind>) {
    (line.to_string(), None, Some(Kind::Conflict))
}

struct Meets {
    log: Rc<RefCell<Vec<String>>>,
    ended: RefCell<Vec<ActivityEnded>>,
    refuse: RefCell<Option<HostError>>,
    seen: RefCell<Vec<(Vec<String>, ProcessesAsk, MeetVerb, Option<Caller>)>>,
}

#[async_trait(?Send)]
impl ActivityDoor for Meets {
    async fn read(&self, _ask: ActivityAsk, _caller: Option<Caller>) -> Result<ActivityView, HostError> {
        panic!("a delete never reads the activity table")
    }
    async fn end(&self, _ask: EndAsk, _caller: Option<Caller>) -> Result<ActivityEnded, HostError> {
        panic!("a delete never ends a process by pid")
    }
    async fn meet(
        &self,
        thread_ids: &[String],
        processes: ProcessesAsk,
        verb: MeetVerb,
        caller: Option<Caller>,
    ) -> Result<Vec<ActivityEnded>, HostError> {
        self.log.borrow_mut().push(format!("meet {} {processes:?} {verb:?}", thread_ids.join(",")));
        self.seen.borrow_mut().push((thread_ids.to_vec(), processes, verb, caller));
        if let Some(e) = self.refuse.borrow().clone() {
            return Err(e);
        }
        Ok(self.ended.borrow().clone())
    }
}

struct Rig {
    host: Rc<FakeHost>,
    area: SessionsArea,
    log: Rc<RefCell<Vec<String>>>,
    meets: Rc<Meets>,
    /// Whether the kind ends a thread's group (`end_thread` answers a future).
    group: Rc<Cell<bool>>,
    guard: Rc<RefCell<Option<HostError>>>,
    leads_sub: Rc<Cell<bool>>,
    turn_runs: Rc<Cell<bool>>,
    open_error: Rc<RefCell<Option<HostError>>>,
}

fn ended_one() -> ActivityEnded {
    serde_json::from_value(json!({ "pid": 4200, "ended": [], "forced": [], "left": [] })).expect("an ended process")
}

fn deleted(worktree: Option<&str>, ended: Option<Vec<ActivityEnded>>) -> ThreadDeleted {
    ThreadDeleted {
        workspace_id: W.to_string(),
        worktree: worktree.map(String::from),
        threads: 1,
        ended,
        machine_deleted: false,
        on: None,
        question: None,
    }
}

impl Rig {
    /// A workspace holding `thread-aa` with two finished turns and `thread-bb` with one, each thread with a record.
    fn new(entry_extra: Value) -> Rig {
        let host = FakeHost::new();
        host.add_entry(entry(W, entry_extra));
        for (key, thread, at) in [("a-1", T, 1000), ("a-2", T, 2000), ("b-1", B, 3000)] {
            let v = view(
                key,
                W,
                json!({ "threadId": thread, "claudeSessionId": format!("S-{thread}"), "status": "completed", "startedAt": T0 + at, "endedAt": T0 + at + 500 }),
            );
            host.add_row(key, row(v, &format!("turn-{key}")));
        }
        host.add_record(T, W, thread_record(json!({ "harness": "claude" })));
        host.add_record(B, W, thread_record(json!({ "harness": "claude" })));
        for (thread, turn) in [(T, "turn-a-1"), (T, "turn-a-2"), (B, "turn-b-1")] {
            host.recorded.borrow_mut().push(event(json!({ "type": "session.start", "workspaceId": W, "sessionId": format!("S-{thread}"), "turnId": turn, "threadId": thread })));
        }
        let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let meets = Rc::new(Meets {
            log: log.clone(),
            ended: RefCell::new(Vec::new()),
            refuse: RefCell::new(None),
            seen: RefCell::new(Vec::new()),
        });
        let group = Rc::new(Cell::new(true));
        let guard: Rc<RefCell<Option<HostError>>> = Rc::new(RefCell::new(None));
        let leads_sub = Rc::new(Cell::new(false));
        let turn_runs = Rc::new(Cell::new(false));
        let open_error: Rc<RefCell<Option<HostError>>> = Rc::new(RefCell::new(None));

        let m = meets.clone();
        host.hooks.set::<dyn Fn() -> Rc<dyn ActivityDoor>>("SessionsCtx::activity", Rc::new(move || m.clone() as Rc<dyn ActivityDoor>));
        let (l, g) = (log.clone(), guard.clone());
        host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> Result<(), HostError>>(
            "SessionsCtx::spawn_guard",
            Rc::new(move |act: &str, _o: Option<&Caller>| {
                l.borrow_mut().push(format!("spawn_guard {act}"));
                match g.borrow().clone() {
                    Some(e) => Err(e),
                    None => Ok(()),
                }
            }),
        );
        let (l, s) = (log.clone(), leads_sub.clone());
        host.hooks.set::<dyn Fn(Option<&str>, Option<&Caller>) -> bool>(
            "SessionsCtx::lead_of_sub",
            Rc::new(move |w: Option<&str>, _o: Option<&Caller>| {
                l.borrow_mut().push(format!("lead_of_sub {w:?}"));
                s.get()
            }),
        );
        let (l, t) = (log.clone(), turn_runs.clone());
        host.hooks.set::<dyn Fn(&str) -> bool>(
            "SessionsCtx::turn_runs",
            Rc::new(move |w: &str| {
                l.borrow_mut().push(format!("turn_runs {w}"));
                t.get()
            }),
        );
        let (l, g) = (log.clone(), group.clone());
        host.hooks.set::<dyn Fn(&Entry, &str, EndThread) -> Option<LocalBoxFuture<'static, Result<Option<String>, HostError>>>>(
            "SessionsCtx::end_thread",
            Rc::new(move |_e: &Entry, thread: &str, o: EndThread| {
                if !g.get() {
                    return None;
                }
                l.borrow_mut().push(format!("end_thread {thread} remove={} away={}", o.remove, o.away));
                Some(async { Ok::<Option<String>, HostError>(None) }.boxed_local())
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str)>("SessionsCtx::reap_kept", Rc::new(move |t: &str| l.borrow_mut().push(format!("reap_kept {t}"))));
        let l = log.clone();
        host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::end_thread_shells",
            Rc::new(move |_e: &Entry, t: &str| {
                l.borrow_mut().push(format!("end_thread_shells {t}"));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::delete_workspace",
            Rc::new(move |w: &str, _o: Option<&Caller>| {
                l.borrow_mut().push(format!("delete_workspace {w}"));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&Entry, &str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_checkpoints",
            Rc::new(move |_e: &Entry, t: &str| {
                l.borrow_mut().push(format!("drop_checkpoints {t}"));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&Entry, &[String]) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_thread_files",
            Rc::new(move |_e: &Entry, ids: &[String]| {
                l.borrow_mut().push(format!("drop_thread_files {}", ids.join(",")));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str, &[String]) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_sent_images",
            Rc::new(move |w: &str, ids: &[String]| {
                l.borrow_mut().push(format!("drop_sent_images {w} {}", ids.join(",")));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str, &str, &str)>(
            "SessionsCtx::row_gone",
            Rc::new(move |row: &str, w: &str, t: &str| l.borrow_mut().push(format!("row_gone {row} {w} {t}"))),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str, bool) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::slates_forget",
            Rc::new(move |t: &str, keep: bool| {
                l.borrow_mut().push(format!("slates_forget {t} keep={keep}"));
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let l = log.clone();
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, ()>>(
            "SessionsCtx::persist_sessions",
            Rc::new(move |w: &str| {
                l.borrow_mut().push(format!("persist_sessions {w}"));
                async {}.boxed_local()
            }),
        );
        let (l, h) = (log.clone(), host.clone());
        host.hooks.set::<dyn Fn(&str, &str, Option<&[String]>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::drop_from_transcript",
            Rc::new(move |w: &str, thread: &str, turns: Option<&[String]>| {
                l.borrow_mut().push(format!("drop_from_transcript {w} {thread} {:?}", turns.map(|t| t.to_vec())));
                h.recorded.borrow_mut().retain(|e| serde_json::to_value(e).expect("an event serializes")["threadId"] != thread);
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let (l, h, o) = (log.clone(), host.clone(), open_error.clone());
        host.hooks.set::<dyn Fn(&str, &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>>(
            "SessionsCtx::open_thread",
            Rc::new(move |w: &str, thread: &str| {
                l.borrow_mut().push(format!("open_thread {w} {thread}"));
                let answer = match o.borrow().clone() {
                    Some(e) => Err(e),
                    None => Ok(h.thread_events(thread)),
                };
                async move { answer }.boxed_local()
            }),
        );
        let area = SessionsArea::new(host.clone());
        Rig { host, area, log, meets, group, guard, leads_sub, turn_runs, open_error }
    }

    fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }

    fn rows_of(&self, thread: &str) -> Vec<String> {
        self.host
            .rows
            .borrow()
            .iter()
            .filter(|(_, r)| r.borrow().view.thread_id.as_deref() == Some(thread))
            .map(|(k, _)| k.clone())
            .collect()
    }

    fn has_record(&self, thread: &str) -> bool {
        self.host.records.borrow().contains_key(thread)
    }

    fn events_of(&self, thread: &str) -> usize {
        self.host.thread_events(thread).len()
    }

    /// Nothing written, nothing dropped, nothing ended.
    fn untouched(&self) {
        assert_eq!(self.rows_of(T), vec!["a-1", "a-2"]);
        assert_eq!(self.rows_of(B), vec!["b-1"]);
        assert!(self.has_record(T) && self.has_record(B));
        assert_eq!((self.events_of(T), self.events_of(B)), (2, 1));
    }

    async fn delete(&self, thread: &str, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError> {
        self.area.delete_thread(thread, None, ask).await
    }

    async fn forget(&self, thread: &str, check: bool) -> Result<(), HostError> {
        self.area.forget_thread(thread, None, check).await
    }
}

fn worktree() -> Value {
    json!({ "worktree": { "path": WORKTREE, "branch": "feat/x", "made": true } })
}

/// The reads of a delete that is decided and goes on to drop one thread of a folder.
fn dropped(thread: &str, rows: &[&str], keep: bool) -> Vec<String> {
    let mut log = vec![
        format!("reap_kept {thread}"),
        format!("end_thread {thread} remove=true away=false"),
        format!("end_thread_shells {thread}"),
        format!("open_thread {W} {thread}"),
        format!("drop_checkpoints {thread}"),
        format!("drop_thread_files {thread}"),
        format!("drop_sent_images {W} {thread}"),
    ];
    if keep {
        log.retain(|l| !l.starts_with("end_thread") && !l.starts_with("end_thread_shells"));
    }
    for r in rows {
        log.push(format!("row_gone {r} {W} {thread}"));
    }
    log.push(format!("drop_from_transcript {W} {thread} None"));
    log.push(format!("persist_sessions {W}"));
    log.push(format!("slates_forget {thread} keep={keep}"));
    log
}

// D1: sessions.ts:1402 (no TS test pins the order; spawn-guard.test.ts pins the guard's refusals)
#[test]
fn a_thread_nobody_holds_is_not_found_after_the_guard_ran() {
    run_local(async {
        let r = Rig::new(json!({}));
        let e = r.delete("thr_nobody", DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word("thr_nobody")))));
        assert_eq!(
            r.log(),
            vec!["lead_of_sub None".to_string(), "spawn_guard delete".to_string()],
            "the guard runs before the absence is read"
        );
        assert_eq!(r.host.hooks.called("ready").len(), 1);
        // A guard that refuses is the refusal, whatever the thread is.
        let refused = HostError::with_kind("a thread may not delete", Kind::Usage);
        *r.guard.borrow_mut() = Some(refused.clone());
        let e = r.delete("thr_nobody", DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&refused));
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&refused));
        r.untouched();
        // A caller the workspace does not reach reads as absent too.
        *r.guard.borrow_mut() = None;
        r.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                    async { Ok::<Option<Entry>, HostError>(None) }.boxed_local()
                }),
            );
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word(T)))));
        r.untouched();
    });
}

// D1
#[test]
fn the_guard_names_the_act_by_who_leads_the_sub() {
    run_local(async {
        let r = Rig::new(json!({}));
        r.leads_sub.set(true);
        r.delete(T, DeleteThreadAsk { processes: None, check: Some(true) }).await.unwrap();
        assert_eq!(r.log(), vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete_sub".to_string()]);
        // The workspace is a row's, else the record's.
        let r = Rig::new(json!({}));
        r.host.rows.borrow_mut().clear();
        r.host.records.borrow_mut().get_mut(T).unwrap().workspace_id = "w2".to_string();
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(e.kind, Some(Kind::NotFound), "w2 holds no live workspace here");
        assert_eq!(r.log()[0], "lead_of_sub Some(\"w2\")".to_string());
    });
}

// D2: sub-lifecycle.test.ts:364
#[test]
fn a_sub_threads_delete_is_the_subs() {
    run_local(async {
        let r = Rig::new(
            json!({ "sub": { "on": "boat", "repo": "r", "base": { "branch": "main", "sha": "abc" }, "branch": "b", "folder": "/f" } }),
        );
        let asked: Rc<RefCell<Vec<(String, Option<Caller>, DeleteThreadAsk)>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = asked.clone();
        let answer = ThreadDeleted {
            workspace_id: W.to_string(),
            worktree: None,
            threads: 1,
            ended: None,
            machine_deleted: true,
            on: Some("Boat".to_string()),
            question: None,
        };
        let given = answer.clone();
        r.host
            .hooks
            .set::<dyn Fn(&Entry, &str, Option<&Caller>, DeleteThreadAsk) -> LocalBoxFuture<'static, Result<ThreadDeleted, HostError>>>(
                "SessionsCtx::sub_delete",
                Rc::new(move |_e: &Entry, t: &str, o: Option<&Caller>, ask: DeleteThreadAsk| {
                    seen.borrow_mut().push((t.to_string(), o.cloned(), ask));
                    let a = given.clone();
                    async move { Ok::<ThreadDeleted, HostError>(a) }.boxed_local()
                }),
            );
        let ask = DeleteThreadAsk { processes: Some(ProcessesAsk::Keep), check: Some(true) };
        let got = r.area.delete_thread(T, Some(Caller::Road(WorkspaceOrigin::Here)), ask.clone()).await.unwrap();
        assert_eq!(got, answer);
        assert_eq!(asked.borrow().clone(), vec![(T.to_string(), Some(Caller::Road(WorkspaceOrigin::Here)), ask)]);
        assert_eq!(
            r.log(),
            vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string()],
            "the sub's own road does the rest"
        );
        r.untouched();
        // The sub's refusal is the delete's.
        r.host
            .hooks
            .set::<dyn Fn(&Entry, &str, Option<&Caller>, DeleteThreadAsk) -> LocalBoxFuture<'static, Result<ThreadDeleted, HostError>>>(
                "SessionsCtx::sub_delete",
                Rc::new(|_e: &Entry, _t: &str, _o: Option<&Caller>, _a: DeleteThreadAsk| {
                    async { Err::<ThreadDeleted, HostError>(HostError::with_kind("work is not pushed", Kind::Conflict)) }.boxed_local()
                }),
            );
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), conflict("work is not pushed"));
    });
}

// D2: sessions.ts:1411
#[test]
fn a_thread_on_a_machine_is_refused() {
    run_local(async {
        let r = Rig::new(json!({ "kind": "cloud", "name": "box1" }));
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), (thread_on_machine_line("box1"), None, Some(Kind::Usage)));
        r.untouched();
        assert_eq!(r.log(), vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string()]);
    });
}

// D3, D7: folder-threads.test.ts:1076
#[test]
fn the_last_thread_takes_its_worktree() {
    run_local(async {
        let r = Rig::new(worktree());
        // A check names what would go, and changes nothing: the other thread keeps the worktree.
        let check = r.delete(B, DeleteThreadAsk { processes: None, check: Some(true) }).await.unwrap();
        assert_eq!(check, deleted(None, None));
        r.untouched();
        // The first of two takes itself alone, and the worktree stays.
        let first = r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(first, deleted(None, None));
        assert_eq!(r.rows_of(T), Vec::<String>::new());
        assert!(!r.has_record(T) && r.has_record(B));
        assert!(!r.log().iter().any(|l| l.starts_with("delete_workspace")));
        r.log.borrow_mut().clear();
        // The last one takes the worktree: the group ended, the shells ended, the workspace deleted, and no row
        // is dropped one by one.
        let last = r.delete(B, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(last, deleted(Some(WORKTREE), None));
        assert_eq!(
            r.log(),
            vec![
                format!("lead_of_sub Some(\"{W}\")"),
                "spawn_guard delete".to_string(),
                format!("turn_runs {W}"),
                format!("end_thread {B} remove=true away=false"),
                format!("end_thread_shells {B}"),
                format!("delete_workspace {W}"),
            ],
        );
    });
}

// D3
#[test]
fn a_thread_with_only_a_record_still_holds_the_worktree() {
    run_local(async {
        let r = Rig::new(worktree());
        r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() != Some(B));
        // B has a record and no row, as one whose rows fell off the index cap does.
        let got = r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(got, deleted(None, None));
        assert!(!r.log().iter().any(|l| l.starts_with("delete_workspace")));
        assert!(r.has_record(B));
        // And a row with no record holds it as well.
        let r = Rig::new(worktree());
        r.host.records.borrow_mut().shift_remove(B);
        let got = r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(got, deleted(None, None));
        assert_eq!(r.rows_of(B), vec!["b-1"]);
    });
}

// D3
#[test]
fn a_worktree_somebody_else_made_or_gone_is_never_taken() {
    run_local(async {
        for tree in [json!({ "path": WORKTREE, "made": false }), json!({ "path": WORKTREE, "made": true, "gone": true })] {
            let r = Rig::new(json!({ "worktree": tree }));
            r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() == Some(T));
            r.host.records.borrow_mut().shift_remove(B);
            let got = r.delete(T, DeleteThreadAsk::default()).await.unwrap();
            assert_eq!(got, deleted(None, None));
            assert!(!r.log().iter().any(|l| l.starts_with("delete_workspace") || l.starts_with("turn_runs")));
            let mut expected = vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string()];
            expected.extend(dropped(T, &["a-1", "a-2"], false));
            assert_eq!(r.log(), expected, "a worktree that is not the thread's to take stays, whoever's last thread this is");
        }
    });
}

// D3: folder-threads.test.ts:1056
#[test]
fn a_worktree_with_a_turn_running_is_busy() {
    run_local(async {
        let r = Rig::new(worktree());
        r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() == Some(T));
        r.host.records.borrow_mut().shift_remove(B);
        r.turn_runs.set(true);
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), conflict(WORKTREE_BUSY_LINE));
        assert_eq!(r.log(), vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string(), format!("turn_runs {W}")]);
        assert_eq!(r.rows_of(T), vec!["a-1", "a-2"]);
        // Once it ended, the delete takes the worktree.
        r.turn_runs.set(false);
        assert_eq!(r.delete(T, DeleteThreadAsk::default()).await.unwrap(), deleted(Some(WORKTREE), None));
        // A turn running somewhere in a folder that stays is not asked about: only a worktree that goes is.
        let r = Rig::new(json!({}));
        r.turn_runs.set(true);
        r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert!(!r.log().iter().any(|l| l.starts_with("turn_runs")));
    });
}

// D4: box-panels.test.ts, sessions.ts:1419
#[test]
fn a_working_thread_is_refused() {
    run_local(async {
        let r = Rig::new(json!({}));
        r.host.rows.borrow()["a-2"].borrow_mut().view.status = SessionStatus::Running;
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), conflict(THREAD_WORKING_LINE));
        // A check is refused the same way, and processes are never asked about first.
        let e = r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Ask), check: Some(true) }).await.unwrap_err();
        assert_eq!(said(&e), conflict(THREAD_WORKING_LINE));
        assert_eq!(
            r.log(),
            vec![
                format!("lead_of_sub Some(\"{W}\")"),
                "spawn_guard delete".to_string(),
                format!("lead_of_sub Some(\"{W}\")"),
                "spawn_guard delete".to_string()
            ]
        );
        assert_eq!(r.meets.seen.borrow().len(), 0);
        // A thread beside it that works is none of this delete's business.
        r.host.rows.borrow()["a-2"].borrow_mut().view.status = SessionStatus::Completed;
        r.host.rows.borrow()["b-1"].borrow_mut().view.status = SessionStatus::Running;
        r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(r.rows_of(B), vec!["b-1"]);
    });
}

// D5: folder-threads.test.ts:1087
#[test]
fn check_reads_and_changes_nothing() {
    run_local(async {
        let r = Rig::new(json!({}));
        assert_eq!(r.delete(T, DeleteThreadAsk { processes: None, check: Some(true) }).await.unwrap(), deleted(None, None));
        assert_eq!(r.log(), vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string()]);
        r.untouched();
        // On a worktree that would go, the check names it.
        let r = Rig::new(worktree());
        r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() == Some(T));
        r.host.records.borrow_mut().shift_remove(B);
        assert_eq!(r.delete(T, DeleteThreadAsk { processes: None, check: Some(true) }).await.unwrap(), deleted(Some(WORKTREE), None));
        assert!(!r.log().iter().any(|l| l.starts_with("delete_workspace") || l.starts_with("end_thread")));
        assert_eq!(r.rows_of(T), vec!["a-1", "a-2"]);
    });
}

// D5: activity.test.ts:264
#[test]
fn a_check_with_processes_meets_them_as_keep_and_a_refusal_changes_nothing() {
    run_local(async {
        let r = Rig::new(json!({}));
        for (asked, met) in
            [(ProcessesAsk::Stop, ProcessesAsk::Keep), (ProcessesAsk::Ask, ProcessesAsk::Ask), (ProcessesAsk::Keep, ProcessesAsk::Keep)]
        {
            r.log.borrow_mut().clear();
            let got = r.delete(T, DeleteThreadAsk { processes: Some(asked), check: Some(true) }).await.unwrap();
            assert_eq!(got, deleted(None, None), "a check answers with no ended, whatever the table held");
            assert_eq!(
                r.log(),
                vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string(), format!("meet {T} {met:?} Delete")],
                "stop is met as keep: refusals only"
            );
        }
        // A refusal from the table is the delete's, and nothing moved.
        let refused = HostError::with_kind("the thread to delete still runs 2 processes: 4200 node (dev, :5173) of thread-a", Kind::Usage);
        *r.meets.refuse.borrow_mut() = Some(refused.clone());
        let e = r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Ask), check: Some(true) }).await.unwrap_err();
        assert_eq!(said(&e), said(&refused));
        let e = r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Ask), check: None }).await.unwrap_err();
        assert_eq!(said(&e), said(&refused));
        r.untouched();
        assert!(!r.log().iter().any(|l| l.starts_with("reap_kept") || l.starts_with("end_thread")));
    });
}

// D6, D8: folder-threads.test.ts:672, :736
#[test]
fn delete_drops_rows_record_transcript_and_slate() {
    run_local(async {
        let r = Rig::new(json!({}));
        let got = r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(got, deleted(None, None));
        let mut expected = vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string()];
        expected.extend(dropped(T, &["a-1", "a-2"], false));
        assert_eq!(r.log(), expected);
        // This thread's rows alone: the other thread keeps its row, its record and its events.
        assert_eq!(r.rows_of(T), Vec::<String>::new());
        assert_eq!(r.rows_of(B), vec!["b-1"]);
        assert!(!r.has_record(T) && r.has_record(B));
        assert_eq!((r.events_of(T), r.events_of(B)), (0, 1));
        assert!(r.meets.seen.borrow().is_empty(), "no processes asked, none met");
    });
}

// D6: activity.test.ts:264
#[test]
fn delete_meets_processes_unless_none_asked_and_names_what_ended() {
    run_local(async {
        let r = Rig::new(json!({}));
        *r.meets.ended.borrow_mut() = vec![ended_one()];
        let origin = Caller::Road(WorkspaceOrigin::Here);
        let got = r
            .area
            .delete_thread(T, Some(origin.clone()), DeleteThreadAsk { processes: Some(ProcessesAsk::Stop), check: None })
            .await
            .unwrap();
        assert_eq!(got, deleted(None, Some(vec![ended_one()])));
        assert_eq!(r.meets.seen.borrow().clone(), vec![(vec![T.to_string()], ProcessesAsk::Stop, MeetVerb::Delete, Some(origin))]);
        let log = r.log();
        let at = |prefix: &str| log.iter().position(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix} in {log:?}"));
        assert!(
            at("meet") < at("reap_kept") && at("reap_kept") < at("end_thread "),
            "the processes are met before anything is ended: {log:?}"
        );
        assert_eq!(log.last().unwrap(), &format!("slates_forget {T} keep=false"));
        // A table with nothing ended leaves `ended` off the answer.
        let r = Rig::new(json!({}));
        let got = r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Stop), check: None }).await.unwrap();
        assert_eq!(got, deleted(None, None));
    });
}

// D6, D7: activity.test.ts:264
#[test]
fn a_worktree_delete_names_what_ended_too() {
    run_local(async {
        let r = Rig::new(worktree());
        r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() == Some(T));
        r.host.records.borrow_mut().shift_remove(B);
        *r.meets.ended.borrow_mut() = vec![ended_one()];
        let got = r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Ask), check: None }).await.unwrap();
        assert_eq!(got, deleted(Some(WORKTREE), Some(vec![ended_one()])));
    });
}

// D6, D8: activity.test.ts:264
#[test]
fn keep_leaves_the_processes_and_the_shells() {
    run_local(async {
        let r = Rig::new(json!({}));
        let got = r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Keep), check: None }).await.unwrap();
        assert_eq!(got, deleted(None, None));
        let mut expected = vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string(), format!("meet {T} Keep Delete")];
        expected.extend(dropped(T, &["a-1", "a-2"], true));
        assert_eq!(r.log(), expected, "no group end and no shell end, and the slate is told to keep");
        // The same with the worktree: the workspace still goes, the group and the shells stay.
        let r = Rig::new(worktree());
        r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() == Some(T));
        r.host.records.borrow_mut().shift_remove(B);
        r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Keep), check: None }).await.unwrap();
        assert_eq!(
            r.log(),
            vec![
                format!("lead_of_sub Some(\"{W}\")"),
                "spawn_guard delete".to_string(),
                format!("turn_runs {W}"),
                format!("meet {T} Keep Delete"),
                format!("delete_workspace {W}"),
            ],
        );
    });
}

// D8: the kind ends no group
#[test]
fn a_kind_with_no_group_end_ends_no_group() {
    run_local(async {
        let r = Rig::new(json!({}));
        r.group.set(false);
        r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        let mut expected = vec![format!("lead_of_sub Some(\"{W}\")"), "spawn_guard delete".to_string()];
        expected.extend(dropped(T, &["a-1", "a-2"], false).into_iter().filter(|l| !l.starts_with("end_thread ")));
        assert_eq!(r.log(), expected, "the shells are still ended");
    });
}

/// A thread's held send: a row still running with no handle, waiting for its launch, which ends once it is given up.
fn held_send(r: &Rig) -> (Rc<Cell<u32>>, oneshot::Sender<()>) {
    let v = view("held-1", W, json!({ "threadId": T, "claudeSessionId": "S-held", "status": "running", "startedAt": T0 + 5000 }));
    let (tx, rx) = oneshot::channel::<()>();
    let held = row(v, "turn-held");
    held.borrow_mut().launch = Some(rx.map(|_| ()).boxed_local().shared());
    r.host.add_row("held-1", held.clone());
    let given = Rc::new(Cell::new(0u32));
    let (g, l, v) = (given.clone(), r.log.clone(), held.clone());
    let line = send_given_up_line("aws-box");
    r.area.stops.borrow_mut().held_sends.insert(
        "held-1".to_string(),
        HeldSend {
            thread_id: T.to_string(),
            give_up: Rc::new(move || {
                g.set(g.get() + 1);
                l.borrow_mut().push("give_up held-1".to_string());
                v.borrow_mut().view.status = SessionStatus::Failed;
                line.clone()
            }),
        },
    );
    (given, tx)
}

// D8: owed-box.test.ts, box-stop.test.ts (a send waiting on an end owed to a computer that is away)
#[test]
fn a_held_send_goes_with_the_thread() {
    run_local(async {
        let r = Rig::new(json!({}));
        let (given, launch) = held_send(&r);
        let area = r.area.clone();
        let done: Rc<RefCell<Option<Result<ThreadDeleted, HostError>>>> = Rc::new(RefCell::new(None));
        let out = done.clone();
        tokio::task::spawn_local(async move {
            *out.borrow_mut() = Some(area.delete_thread(T, None, DeleteThreadAsk::default()).await);
        });
        settle().await;
        // The send is given up, and the delete waits for its launch to end before it drops anything.
        assert_eq!(given.get(), 1);
        assert!(done.borrow().is_none(), "the delete waits for the held launch");
        assert!(!r.log().iter().any(|l| l.starts_with("reap_kept")), "{:?}", r.log());
        launch.send(()).unwrap();
        settle().await;
        let got = done.borrow_mut().take().expect("the delete finished").expect("the delete answered");
        assert_eq!(got, deleted(None, None));
        let log = r.log();
        let at = |prefix: &str| log.iter().position(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("{prefix} in {log:?}"));
        assert!(at("give_up") < at("reap_kept"));
        assert!(log.contains(&format!("row_gone held-1 {W} {T}")), "the held row goes with the thread: {log:?}");
        assert_eq!(r.rows_of(T), Vec::<String>::new());
        assert_eq!(r.rows_of(B), vec!["b-1"]);
    });
}

// D8: sessions.ts:1417 (a check, or a refusal on its processes, leaves a held send waiting)
#[test]
fn a_held_send_is_left_waiting_by_a_check_and_by_a_refusal() {
    run_local(async {
        let r = Rig::new(json!({}));
        let (given, _launch) = held_send(&r);
        r.delete(T, DeleteThreadAsk { processes: None, check: Some(true) }).await.unwrap();
        *r.meets.refuse.borrow_mut() = Some(HostError::with_kind("still runs a process", Kind::Usage));
        r.delete(T, DeleteThreadAsk { processes: Some(ProcessesAsk::Ask), check: None }).await.unwrap_err();
        assert_eq!(given.get(), 0);
        assert!(r.host.rows.borrow().contains_key("held-1"));
        // A turn that is not held is the thread working, though a held send is not.
        r.host.rows.borrow()["a-2"].borrow_mut().view.status = SessionStatus::Running;
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), conflict(THREAD_WORKING_LINE));
        assert_eq!(given.get(), 0, "refused before the sends are given up");
    });
}

// D8: sessions.ts:1431
#[test]
fn a_turn_that_started_while_the_held_sends_were_given_up_refuses() {
    run_local(async {
        let r = Rig::new(json!({}));
        r.host.hooks.set::<dyn Fn(&str) -> bool>("SessionsCtx::thread_runs", Rc::new(|t: &str| t == T));
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), conflict(THREAD_WORKING_LINE));
        assert!(!r.log().iter().any(|l| l.starts_with("reap_kept") || l.starts_with("end_thread")), "{:?}", r.log());
        r.untouched();
    });
}

// D8: sessions.ts:1436
#[test]
fn a_transcript_that_does_not_read_refuses_a_delete_before_anything_is_dropped() {
    run_local(async {
        let r = Rig::new(json!({}));
        *r.open_error.borrow_mut() = Some(HostError::new(format!("the transcript of {W} does not read")));
        let e = r.delete(T, DeleteThreadAsk::default()).await.unwrap_err();
        assert_eq!(e.message, format!("the transcript of {W} does not read"));
        assert_eq!(
            r.log(),
            vec![
                format!("lead_of_sub Some(\"{W}\")"),
                "spawn_guard delete".to_string(),
                format!("reap_kept {T}"),
                format!("end_thread {T} remove=true away=false"),
                format!("end_thread_shells {T}"),
                format!("open_thread {W} {T}"),
            ],
        );
        r.untouched();
        *r.open_error.borrow_mut() = None;
        r.delete(T, DeleteThreadAsk::default()).await.unwrap();
        assert_eq!(r.rows_of(T), Vec::<String>::new());
    });
}

fn never_ran(r: &Rig) {
    // A thread whose one turn the agent refused before it began: no agent session, so no turn that ran.
    r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() != Some(T));
    let v = view("junk-1", W, json!({ "threadId": T, "status": "failed", "startedAt": T0 + 500, "endedAt": T0 + 600 }));
    r.host.add_row("junk-1", row(v, "turn-junk"));
    // The record a launch that never began leaves says no turn worked.
    r.host.records.borrow_mut().get_mut(T).unwrap().worked = Some(false);
}

fn forgotten(thread: &str, rows: &[&str]) -> Vec<String> {
    let mut log =
        vec![format!("open_thread {W} {thread}"), format!("drop_thread_files {thread}"), format!("drop_sent_images {W} {thread}")];
    for r in rows {
        log.push(format!("row_gone {r} {W} {thread}"));
    }
    log.push(format!("slates_forget {thread} keep=false"));
    log.push(format!("drop_from_transcript {W} {thread} None"));
    log.push(format!("persist_sessions {W}"));
    log
}

// D9, D11: forget.test.ts:174
#[test]
fn forget_takes_a_thread_that_never_worked() {
    run_local(async {
        let r = Rig::new(json!({}));
        never_ran(&r);
        r.forget(T, false).await.unwrap();
        assert_eq!(r.log(), forgotten(T, &["junk-1"]), "no checkpoint, no group, no shell: nothing is asked of the machine but its files");
        assert_eq!(r.rows_of(T), Vec::<String>::new());
        assert_eq!(r.rows_of(B), vec!["b-1"], "the thread beside it is untouched");
        assert!(!r.has_record(T) && r.has_record(B));
        assert_eq!((r.events_of(T), r.events_of(B)), (0, 1));
        assert_eq!(r.host.hooks.called("ready").len(), 1);
    });
}

// D10, D11: forget.test.ts:419
#[test]
fn forget_takes_a_record_alone_that_says_it_never_worked() {
    run_local(async {
        let r = Rig::new(json!({}));
        r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() != Some(T));
        r.host.records.borrow_mut().insert(
            T.to_string(),
            ThreadRecord { workspace_id: W.to_string(), ..thread_record(json!({ "harness": "claude", "worked": false })) },
        );
        r.forget(T, false).await.unwrap();
        assert_eq!(r.log(), forgotten(T, &[]));
        assert!(!r.has_record(T));
    });
}

// D10: forget.test.ts:226
#[test]
fn forget_refuses_a_thread_that_worked() {
    run_local(async {
        let r = Rig::new(json!({}));
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
        assert_eq!(r.log(), Vec::<String>::new());
        r.untouched();
        // A row still running is a turn that ran.
        let r = Rig::new(json!({}));
        never_ran(&r);
        r.host.rows.borrow()["junk-1"].borrow_mut().view.status = SessionStatus::Running;
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
        // One worked row among rows that did not is enough, and the record's word does not reverse it.
        let r = Rig::new(json!({}));
        never_ran(&r);
        r.host.add_row(
            "worked-1",
            row(view("worked-1", W, json!({ "threadId": T, "claudeSessionId": "S-w", "status": "completed" })), "turn-worked"),
        );
        r.host.records.borrow_mut().get_mut(T).unwrap().worked = Some(false);
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
        assert_eq!(r.log(), Vec::<String>::new());
    });
}

// D10: forget.test.ts:274, :395
#[test]
fn forget_refuses_where_the_record_says_it_worked() {
    run_local(async {
        // Rows fell off the index cap: the record stands alone, and an absent `worked` reads as worked.
        for record in [json!({ "harness": "claude" }), json!({ "harness": "claude", "worked": true })] {
            let r = Rig::new(json!({}));
            r.host.rows.borrow_mut().retain(|_, row| row.borrow().view.thread_id.as_deref() != Some(T));
            r.host.records.borrow_mut().insert(T.to_string(), ThreadRecord { workspace_id: W.to_string(), ..thread_record(record) });
            let e = r.forget(T, false).await.unwrap_err();
            assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
            assert_eq!(r.log(), Vec::<String>::new());
            assert!(r.has_record(T));
        }
        // Rows that never ran with a record that says they did: refused all the same.
        let r = Rig::new(json!({}));
        never_ran(&r);
        r.host.records.borrow_mut().get_mut(T).unwrap().worked = None;
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
        assert_eq!(r.rows_of(T), vec!["junk-1"]);
    });
}

// D10: forget.test.ts:340
#[test]
fn a_turn_refused_for_want_of_a_sign_in_ran_nothing() {
    run_local(async {
        let r = Rig::new(json!({}));
        never_ran(&r);
        // The session id is on the row, since the announce landed; the refusal is what says nothing ran.
        {
            let rows = r.host.rows.borrow();
            let mut v = rows["junk-1"].borrow_mut();
            v.view.claude_session_id = Some("S-refused".to_string());
            v.view.refusal = Some(serde_json::from_value(json!("sign-in")).expect("a refusal"));
        }
        r.host.records.borrow_mut().get_mut(T).unwrap().worked = Some(false);
        r.forget(T, false).await.unwrap();
        assert_eq!(r.log(), forgotten(T, &["junk-1"]));
        // Once a later turn did work, the thread ran.
        let r = Rig::new(json!({}));
        never_ran(&r);
        {
            let rows = r.host.rows.borrow();
            let mut v = rows["junk-1"].borrow_mut();
            v.view.claude_session_id = Some("S-refused".to_string());
            v.view.refusal = Some(serde_json::from_value(json!("sign-in")).expect("a refusal"));
        }
        r.host.add_row(
            "later-1",
            row(
                view("later-1", W, json!({ "threadId": T, "claudeSessionId": "S-refused", "status": "completed", "startedAt": T0 + 900 })),
                "turn-later",
            ),
        );
        r.host.records.borrow_mut().get_mut(T).unwrap().worked = Some(false);
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
    });
}

// D9: forget.test.ts:243
#[test]
fn forget_of_a_thread_nobody_holds_is_not_found() {
    run_local(async {
        let r = Rig::new(json!({}));
        let e = r.forget("thr_nobody", false).await.unwrap_err();
        assert_eq!(e.message, "no thread thr_nobo");
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word("thr_nobody")))));
        // A caller the workspace does not reach reads the same absence, not the refusal past this gate.
        r.host
            .hooks
            .set::<dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>>(
                "SessionsCtx::entry_of_row",
                Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                    async { Ok::<Option<Entry>, HostError>(None) }.boxed_local()
                }),
            );
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word(T)))));
        assert_eq!(r.log(), Vec::<String>::new());
        r.untouched();
    });
}

// D11
#[test]
fn forget_check_changes_nothing() {
    run_local(async {
        let r = Rig::new(json!({}));
        never_ran(&r);
        r.forget(T, true).await.unwrap();
        assert_eq!(r.log(), Vec::<String>::new());
        assert_eq!(r.rows_of(T), vec!["junk-1"]);
        assert!(r.has_record(T));
        assert_eq!((r.events_of(T), r.events_of(B)), (2, 1));
        // A check is refused for a thread that worked, as the forget is.
        let r = Rig::new(json!({}));
        let e = r.forget(T, true).await.unwrap_err();
        assert_eq!(said(&e), (thread_forget_refusal(T), None, Some(Kind::Conflict)));
    });
}

// D11: forget.test.ts:196
#[test]
fn forget_with_a_transcript_that_does_not_read_changes_nothing_until_it_reads() {
    run_local(async {
        let r = Rig::new(json!({}));
        never_ran(&r);
        *r.open_error.borrow_mut() = Some(HostError::new(format!("the transcript of {W} does not read")));
        let e = r.forget(T, false).await.unwrap_err();
        assert_eq!(e.message, format!("the transcript of {W} does not read"));
        assert_eq!(r.log(), vec![format!("open_thread {W} {T}")], "refused before anything is changed");
        assert_eq!(r.rows_of(T), vec!["junk-1"]);
        assert!(r.has_record(T));
        *r.open_error.borrow_mut() = None;
        r.log.borrow_mut().clear();
        r.forget(T, false).await.unwrap();
        assert_eq!(r.log(), forgotten(T, &["junk-1"]));
        assert_eq!(r.rows_of(T), Vec::<String>::new());
    });
}
