// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_settle.md. TS cases: settle-tree.test.ts, thread-marks.test.ts, replaces.test.ts, and
//! host/test/activity.test.ts:225 (settle with processes).
#[path = "support_a/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use support::*;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{ThreadMarks, ThreadMarksPinned};
use wsp_words::exit::{not_found_refusal, refusal, usage_refusal};
use wsp_words::words::computer::{
    not_under_line, replaced_already_fix, replaced_already_line, replaces_working_fix, replaces_working_line, subagent_settle_line, thread_word, NOT_UNDER_FIX,
    SETTLE_ALREADY, SETTLE_WORKING, SUBAGENT_SETTLE_FIX,
};

const HOUR: f64 = 3_600_000.0;

/// The preferences the settle reads: only `settleAfter` matters, the rest are the keys the record requires.
fn prefs_doc(settle_after: &str) -> Value {
    json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {}, "projectLook": {}, "projectOrder": [], "labs": false, "settleAfter": settle_after })
}

const LEAD: &str = "lead-0001";
const PARSER: &str = "parser-01";
const TESTS: &str = "tests-001";
const PRINTER: &str = "printer1";
const LINKER: &str = "linker-1";

/// Every number as an f64, so a stamp the port writes as an integer or a float compares the same.
fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().expect("a number")),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
        other => other.clone(),
    }
}

/// One call of the context's `mark`: the ids, what its closure gave each of them, and who asked.
#[derive(Clone, Debug, PartialEq)]
struct MarkCall {
    ids: Vec<String>,
    stamps: Vec<(String, Value)>,
    origin: Option<Caller>,
}

struct Meets {
    host: Rc<FakeHost>,
    ended: RefCell<Vec<ActivityEnded>>,
    refuse: RefCell<Option<HostError>>,
    seen: RefCell<Vec<(Vec<String>, ProcessesAsk, MeetVerb, Option<Caller>)>>,
}

#[async_trait(?Send)]
impl ActivityDoor for Meets {
    async fn read(&self, _ask: ActivityAsk, _caller: Option<Caller>) -> Result<ActivityView, HostError> {
        panic!("a settle never reads the activity table")
    }
    async fn end(&self, _ask: EndAsk, _caller: Option<Caller>) -> Result<ActivityEnded, HostError> {
        panic!("a settle never ends a process by pid")
    }
    async fn meet(&self, thread_ids: &[String], processes: ProcessesAsk, verb: MeetVerb, caller: Option<Caller>) -> Result<Vec<ActivityEnded>, HostError> {
        self.host.hooks.log("meet", vec![thread_ids.join(",")]);
        self.seen.borrow_mut().push((thread_ids.to_vec(), processes, verb, caller));
        if let Some(e) = self.refuse.borrow().clone() {
            return Err(e);
        }
        Ok(self.ended.borrow().clone())
    }
}

struct World {
    host: Rc<FakeHost>,
    area: SessionsArea,
    facts: Rc<RefCell<IndexMap<String, Value>>>,
    under: Rc<RefCell<IndexMap<String, Vec<String>>>>,
    marks: Rc<RefCell<Vec<MarkCall>>>,
    settled_asked: Rc<RefCell<Vec<Option<f64>>>>,
    meets: Rc<Meets>,
    prefs: Rc<RefCell<Value>>,
}

fn facts_doc(id: &str, status: &str, title: &str) -> Value {
    json!({
        "id": id, "threadId": id, "workspaceId": "w1", "harness": "claude", "startedBy": "person", "status": status,
        "title": title, "sessionId": format!("s-{id}"), "turns": 1, "ran": true,
    })
}

fn scoped(thread: &str, root: &str) -> Caller {
    Caller::Scoped {
        origin: WorkspaceOrigin::Relayed,
        by: serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w1", "rootThreadId": root })).expect("a thread scope"),
    }
}

fn marks_of(doc: Value) -> ThreadMarks {
    serde_json::from_value(doc).expect("thread marks the fixture builds")
}

impl World {
    fn new() -> World {
        let host = FakeHost::new();
        let facts: Rc<RefCell<IndexMap<String, Value>>> = Rc::new(RefCell::new(IndexMap::new()));
        let under: Rc<RefCell<IndexMap<String, Vec<String>>>> = Rc::new(RefCell::new(IndexMap::new()));
        let marks: Rc<RefCell<Vec<MarkCall>>> = Rc::new(RefCell::new(Vec::new()));
        let settled_asked: Rc<RefCell<Vec<Option<f64>>>> = Rc::new(RefCell::new(Vec::new()));
        let prefs = Rc::new(RefCell::new(prefs_doc("1h")));
        let meets = Rc::new(Meets { host: host.clone(), ended: RefCell::new(Vec::new()), refuse: RefCell::new(None), seen: RefCell::new(Vec::new()) });

        let f = facts.clone();
        host.hooks.set::<dyn Fn(&str) -> Option<ThreadFacts>>(
            "SessionsCtx::thread_facts",
            Rc::new(move |id: &str| f.borrow().get(id).map(|d| serde_json::from_value(d.clone()).expect("thread facts the fixture builds"))),
        );
        let u = under.clone();
        host.hooks.set::<dyn Fn(&str) -> Vec<String>>("SessionsCtx::tree_under", Rc::new(move |id: &str| u.borrow().get(id).cloned().unwrap_or_default()));
        let recs = host.records.clone();
        let asked = settled_asked.clone();
        host.hooks.set::<dyn Fn(&str, Option<f64>) -> bool>(
            "SessionsCtx::settled_now",
            Rc::new(move |id: &str, ms: Option<f64>| {
                asked.borrow_mut().push(ms);
                recs.borrow().get(id).map(|h| h.settled_at.is_some()).unwrap_or(false)
            }),
        );
        let p = prefs.clone();
        host.hooks.set::<dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>>(
            "LaunchCtx::preferences",
            Rc::new(move || {
                let prefs: Preferences = serde_json::from_value(p.borrow().clone()).expect("preferences the fixture builds");
                async move { Ok::<Preferences, HostError>(prefs) }.boxed_local()
            }),
        );
        let m = meets.clone();
        host.hooks.set::<dyn Fn() -> Rc<dyn ActivityDoor>>("SessionsCtx::activity", Rc::new(move || m.clone() as Rc<dyn ActivityDoor>));
        let recs = host.records.clone();
        let calls = marks.clone();
        host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(move |ids: &[String], stamps: StampsOf, origin: Option<&Caller>| {
                let given: Vec<(String, Value)> = ids.iter().map(|id| (id.clone(), norm(&Value::Object(stamps(id))))).collect();
                let mut records = recs.borrow_mut();
                for (id, stamp) in &given {
                    let Some(held) = records.get_mut(id) else { continue };
                    let mut doc = serde_json::to_value(&*held).expect("a record serializes");
                    for (k, v) in stamp.as_object().expect("stamps are an object") {
                        if v.is_null() {
                            doc.as_object_mut().expect("a record is an object").remove(k);
                        } else {
                            doc[k] = v.clone();
                        }
                    }
                    *held = serde_json::from_value(doc).expect("a record the stamps leave");
                }
                calls.borrow_mut().push(MarkCall { ids: ids.to_vec(), stamps: given, origin: origin.cloned() });
                async { Ok::<(), HostError>(()) }.boxed_local()
            }),
        );
        let area = SessionsArea::new(host.clone());
        World { host, area, facts, under, marks, settled_asked, meets, prefs }
    }

    /// A thread with facts and a record.
    fn thread(&self, id: &str, status: &str, title: &str) {
        self.facts.borrow_mut().insert(id.to_string(), facts_doc(id, status, title));
        self.host.add_record(id, "w1", thread_record(json!({ "harness": "claude" })));
    }

    fn status(&self, id: &str, status: &str) {
        self.facts.borrow_mut()[id]["status"] = json!(status);
    }

    fn under(&self, lead: &str, ids: &[&str]) {
        self.under.borrow_mut().insert(lead.to_string(), ids.iter().map(|s| s.to_string()).collect());
    }

    /// A lead with three children: one done with a grandchild done under it, one still working, one failed.
    fn tree(&self) {
        self.thread(LEAD, "completed", "lead the work");
        self.thread(PARSER, "completed", "build the parser");
        self.thread(TESTS, "completed", "write its tests");
        self.thread(PRINTER, "running", "build the printer");
        self.thread(LINKER, "failed", "build the linker");
        self.under(LEAD, &[PARSER, TESTS, PRINTER, LINKER]);
        self.under(PARSER, &[TESTS]);
    }

    fn marks(&self) -> Vec<MarkCall> {
        self.marks.borrow().clone()
    }

    fn record(&self, id: &str) -> ThreadRecord {
        self.host.records.borrow()[id].clone()
    }

    fn is_settled(&self, id: &str) -> bool {
        self.record(id).settled_at.is_some()
    }

    fn stamp_of(&self, call: usize, id: &str) -> Value {
        self.marks()[call].stamps.iter().find(|(i, _)| i == id).map(|(_, s)| s.clone()).expect("the thread was marked")
    }
}

fn ids(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn answer(r: &Result<wsp_wire::wire::runtime::SessionSettleResult, HostError>) -> Value {
    norm(&serde_json::to_value(r.as_ref().expect("the settle answered")).expect("an answer serializes"))
}

fn now() -> f64 {
    T0 as f64
}

// E1: settle-tree.test.ts:164
#[test]
fn a_subagents_id_refuses_the_whole_list() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.host.hooks.set::<dyn Fn(&str) -> bool>("SessionsCtx::subagent_known", Rc::new(|id: &str| id == "task_count"));
        let e = w.area.settle_threads(ids(&["task_count"]), None, SettleAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&subagent_settle_line("task_count"), SUBAGENT_SETTLE_FIX, Some(Kind::Usage))));
        assert!(e.message.contains("a subagent settles with its lead's turn"));
        let e = w.area.settle_threads(ids(&[PARSER, "task_count"]), None, SettleAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&subagent_settle_line("task_count"), SUBAGENT_SETTLE_FIX, Some(Kind::Usage))));
        let e = w.area.restore_threads(ids(&["task_count"]), None).await.unwrap_err();
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(w.marks(), Vec::<MarkCall>::new(), "a refused list moves nothing");
        assert_eq!(w.host.hooks.called("meet"), Vec::<String>::new());
    });
}

// E1: settle-tree.test.ts:165, thread-marks.test.ts:339
#[test]
fn a_name_nothing_holds_is_not_found_and_moves_nothing() {
    run_local(async {
        let w = World::new();
        w.tree();
        let e = w.area.settle_threads(ids(&[PARSER, "thr_nobody"]), None, SettleAsk::default()).await.unwrap_err();
        assert_eq!(e.message, "no thread thr_nobo");
        assert_eq!(e.kind, Some(Kind::NotFound));
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word("thr_nobody")))));
        let e = w.area.restore_threads(ids(&["thr_nobody"]), None).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal("no thread thr_nobo")));
        assert_eq!(w.marks(), Vec::<MarkCall>::new());
        assert!(!w.is_settled(PARSER));
    });
}

// E1: settle-tree.test.ts:136
#[test]
fn a_thread_not_under_the_caller_refuses_and_a_stranger_is_not_found() {
    run_local(async {
        let w = World::new();
        w.tree();
        // The caller's own token settles itself and what is under it, never its lead or a thread beside it.
        let asked: Rc<RefCell<Vec<(String, Option<Caller>)>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = asked.clone();
        w.host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> bool>(
            "SessionsCtx::settles_thread",
            Rc::new(move |id: &str, origin: Option<&Caller>| {
                seen.borrow_mut().push((id.to_string(), origin.cloned()));
                id == PARSER || id == TESTS
            }),
        );
        let drives: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let seen = drives.clone();
        w.host.hooks.set::<dyn Fn(Option<&str>, Option<&Caller>) -> bool>(
            "SessionsCtx::drives_thread",
            Rc::new(move |id: Option<&str>, _origin: Option<&Caller>| {
                seen.borrow_mut().push(id.unwrap_or("").to_string());
                id != Some("thr_elsewhere")
            }),
        );
        w.thread("thr_elsewhere", "completed", "another tree");
        let as_parser = scoped(PARSER, LEAD);

        let e = w.area.settle_threads(ids(&[LEAD]), Some(as_parser.clone()), SettleAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&not_under_line(LEAD), NOT_UNDER_FIX, Some(Kind::Usage))));
        assert!(e.message.contains(&format!("thread {}", &LEAD[..8])));
        let e = w.area.settle_threads(ids(&[PRINTER]), Some(as_parser.clone()), SettleAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&not_under_line(PRINTER), NOT_UNDER_FIX, Some(Kind::Usage))));
        let e = w.area.restore_threads(ids(&[LEAD]), Some(as_parser.clone())).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&not_under_line(LEAD), NOT_UNDER_FIX, Some(Kind::Usage))));
        // A thread the caller does not drive at all reads as absent.
        let e = w.area.settle_threads(ids(&["thr_elsewhere"]), Some(as_parser.clone()), SettleAsk::default()).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal(&format!("no thread {}", thread_word("thr_elsewhere")))));
        // A refused list moves nothing, the thread the token reaches included.
        let e = w.area.settle_threads(ids(&[PARSER, PRINTER]), Some(as_parser.clone()), SettleAsk::default()).await.unwrap_err();
        assert_eq!(e.kind, Some(Kind::Usage));
        assert_eq!(w.marks(), Vec::<MarkCall>::new());
        assert!(!w.is_settled(PARSER));
        assert!(asked.borrow().iter().all(|(_, o)| o.as_ref() == Some(&as_parser)), "the caller reaches the check");
        assert_eq!(drives.borrow().clone(), vec![LEAD.to_string(), PRINTER.to_string(), LEAD.to_string(), "thr_elsewhere".to_string(), PRINTER.to_string()]);

        // Its own tree settles and restores.
        let done = w.area.settle_threads(ids(&[TESTS]), Some(as_parser.clone()), SettleAsk::default()).await;
        assert_eq!(answer(&done), json!({ "settled": [{ "threadId": TESTS, "title": "write its tests" }], "left": [] }));
        let back = w.area.restore_threads(ids(&[TESTS]), Some(as_parser.clone())).await.unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json!({ "restored": [{ "threadId": TESTS, "title": "write its tests" }] }));
        assert_eq!(w.marks().iter().map(|m| m.origin.clone()).collect::<Vec<_>>(), vec![Some(as_parser.clone()), Some(as_parser)]);
    });
}

// E2: settle-tree.test.ts:49
#[test]
fn settle_reads_answers_busy_settled_finished_and_titled() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.thread("asking-01", "completed", "asks");
        w.facts.borrow_mut()["asking-01"]["asking"] = json!("ask_1");
        w.thread("waiting-1", "completed", "waits");
        w.facts.borrow_mut()["waiting-1"]["waitingOn"] = json!({
            "threadId": LINKER, "workspaceId": "w1", "sessionId": "s-1", "title": "build the linker",
            "prompt": { "askId": "ask_2", "toolName": "Bash", "input": "{}", "options": [] },
        });
        w.thread("stopped-1", "interrupted", "stopped");
        w.host.records.borrow_mut()[PARSER].settled_at = Some(JsNumber(5.0));
        let reads = w.area.settle_reads(&ids(&[LEAD, PARSER]), None).await.unwrap();
        assert_eq!(
            [LEAD, PARSER, PRINTER, LINKER, "asking-01", "waiting-1", "stopped-1"].map(|id| (reads.busy)(id)),
            [false, false, true, false, true, true, false],
            "running, asking or waiting on something is busy",
        );
        assert_eq!(
            [LEAD, PRINTER, LINKER, "stopped-1", "thr_nofacts"].map(|id| (reads.finished)(id)),
            [true, false, false, true, false],
            "completed or interrupted is finished, failed is not",
        );
        assert_eq!([PARSER, LEAD].map(|id| (reads.settled)(id)), [true, false]);
        assert_eq!(w.settled_asked.borrow().clone(), vec![Some(HOUR), Some(HOUR)], "the quiet time of the settleAfter preference");
        assert_eq!((reads.titled)(&ids(&[PARSER, LEAD])), vec![(PARSER.to_string(), "build the parser".to_string()), (LEAD.to_string(), "lead the work".to_string())]);

        *w.prefs.borrow_mut() = prefs_doc("never");
        let reads = w.area.settle_reads(&ids(&[LEAD]), None).await.unwrap();
        (reads.settled)(LEAD);
        assert_eq!(w.settled_asked.borrow().last().cloned(), Some(None), "never has no quiet time");
    });
}

// E2
#[test]
fn a_thread_with_a_record_and_no_facts_is_titled_by_its_short_id() {
    run_local(async {
        let w = World::new();
        w.host.add_record("thr_recordonly", "w1", thread_record(json!({ "harness": "claude" })));
        let done = w.area.settle_threads(ids(&["thr_recordonly"]), None, SettleAsk::default()).await;
        assert_eq!(answer(&done), json!({ "settled": [{ "threadId": "thr_recordonly", "title": thread_word("thr_recordonly") }], "left": [] }));
    });
}

// E3: replaces.test.ts:75
#[test]
fn a_restart_of_a_working_tree_is_refused() {
    run_local(async {
        let w = World::new();
        w.tree();
        // The thread itself works.
        w.status(PARSER, "running");
        let e = w.area.replaceable_check(PARSER, Some(&scoped(LEAD, LEAD))).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&replaces_working_line(PARSER), &replaces_working_fix(PARSER), Some(Kind::Usage))));
        assert!(e.message.contains(&format!("thread {} is still working", &PARSER[..8])));
        // A thread under it works.
        w.status(PARSER, "completed");
        w.status(TESTS, "running");
        let e = w.area.replaceable_check(PARSER, None).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&replaces_working_line(PARSER), &replaces_working_fix(PARSER), Some(Kind::Usage))));
        // A thread under it asking the person works too.
        w.status(TESTS, "completed");
        w.facts.borrow_mut()[TESTS]["asking"] = json!("ask_1");
        let e = w.area.replaceable_check(PARSER, None).await.unwrap_err();
        assert_eq!(e.kind, Some(Kind::Usage));
        // Working comes before a restart already standing.
        w.host.hooks.set::<dyn Fn() -> IndexMap<String, String>>("SessionsCtx::restarts", Rc::new(|| IndexMap::from([(PARSER.to_string(), "again-001".to_string())])));
        let e = w.area.replaceable_check(PARSER, None).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&replaces_working_line(PARSER), &replaces_working_fix(PARSER), Some(Kind::Usage))));
    });
}

// E3: replaces.test.ts:104
#[test]
fn a_thread_restarted_once_is_refused() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.host.hooks.set::<dyn Fn() -> IndexMap<String, String>>("SessionsCtx::restarts", Rc::new(|| IndexMap::from([(PARSER.to_string(), "again-001".to_string())])));
        let e = w.area.replaceable_check(PARSER, Some(&scoped(LEAD, LEAD))).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&replaced_already_line(PARSER, "again-001"), &replaced_already_fix("again-001"), Some(Kind::Usage))));
        assert!(e.message.contains(&format!("thread {} was restarted as {}", &PARSER[..8], "again-00")));
        // A thread nobody restarted passes.
        assert!(w.area.replaceable_check(TESTS, None).await.is_ok());
    });
}

// E1, E3: replaces.test.ts:118
#[test]
fn a_restart_meets_the_same_refusals_as_a_settle_and_passes_a_stopped_thread() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.host.hooks.set::<dyn Fn(&str, Option<&Caller>) -> bool>("SessionsCtx::settles_thread", Rc::new(|id: &str, _o: Option<&Caller>| id != "thr_beside"));
        w.thread("thr_beside", "completed", "beside");
        let e = w.area.replaceable_check("thr_beside", Some(&scoped(PRINTER, LEAD))).await.unwrap_err();
        assert_eq!(said(&e), said(&refusal(&not_under_line("thr_beside"), NOT_UNDER_FIX, Some(Kind::Usage))));
        let e = w.area.replaceable_check("thr_nobody", Some(&scoped(LEAD, LEAD))).await.unwrap_err();
        assert_eq!(said(&e), said(&not_found_refusal("no thread thr_nobo")));
        assert!(w.area.replaceable_check(PARSER, Some(&scoped(LEAD, LEAD))).await.is_ok());
        assert!(w.area.replaceable_check(PARSER, None).await.is_ok());
        assert_eq!(w.marks(), Vec::<MarkCall>::new(), "the check writes nothing");
    });
}

// E4-E7: settle-tree.test.ts:48
#[test]
fn settle_takes_the_tree_and_names_the_lead() {
    run_local(async {
        let w = World::new();
        w.tree();
        let done = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk::default()).await;
        assert_eq!(
            answer(&done),
            json!({ "settled": [{ "threadId": PARSER, "title": "build the parser" }, { "threadId": TESTS, "title": "write its tests" }], "left": [] }),
        );
        let marks = w.marks();
        assert_eq!(marks.len(), 1, "one mark for all, written once");
        assert_eq!(marks[0].ids, ids(&[PARSER, TESTS]));
        assert_eq!(w.stamp_of(0, PARSER), norm(&json!({ "readAt": now(), "settledAt": now(), "settleNamedAt": now() })));
        assert_eq!(w.stamp_of(0, TESTS), norm(&json!({ "readAt": now(), "settledAt": now() })));
        assert_eq!([LEAD, PARSER, TESTS, PRINTER, LINKER].map(|id| w.is_settled(id)), [false, true, true, false, false]);

        // A second settle leaves it as already settled and writes nothing.
        let again = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk::default()).await;
        assert_eq!(answer(&again), json!({ "settled": [], "left": [{ "threadId": PARSER, "why": SETTLE_ALREADY }] }));
        assert_eq!(w.marks().len(), 1, "nothing was taken, so nothing is marked");
    });
}

// E4: settle-tree.test.ts:48
#[test]
fn settle_takes_a_name_given_twice_once() {
    run_local(async {
        let w = World::new();
        w.tree();
        let done = w.area.settle_threads(ids(&[TESTS, TESTS]), None, SettleAsk::default()).await;
        assert_eq!(answer(&done), json!({ "settled": [{ "threadId": TESTS, "title": "write its tests" }], "left": [] }));
        assert_eq!(w.marks()[0].ids, ids(&[TESTS]));
        assert_eq!(w.stamp_of(0, TESTS), norm(&json!({ "readAt": now(), "settledAt": now(), "settleNamedAt": now() })));
    });
}

// E4: settle-tree.test.ts:66
#[test]
fn settle_leaves_a_working_tree() {
    run_local(async {
        let w = World::new();
        w.tree();
        let left = w.area.settle_threads(ids(&[LEAD]), None, SettleAsk::default()).await;
        assert_eq!(answer(&left), json!({ "settled": [], "left": [{ "threadId": LEAD, "why": SETTLE_WORKING }] }));
        assert_eq!(w.marks(), Vec::<MarkCall>::new());
        // Once nothing in it works it settles whole, the lead first.
        w.status(PRINTER, "completed");
        let done = w.area.settle_threads(ids(&[LEAD]), None, SettleAsk::default()).await;
        assert_eq!(
            answer(&done),
            json!({ "settled": [
                { "threadId": LEAD, "title": "lead the work" }, { "threadId": PARSER, "title": "build the parser" },
                { "threadId": TESTS, "title": "write its tests" }, { "threadId": PRINTER, "title": "build the printer" },
                { "threadId": LINKER, "title": "build the linker" },
            ], "left": [] }),
        );
        assert_eq!(w.marks()[0].ids, ids(&[LEAD, PARSER, TESTS, PRINTER, LINKER]));
    });
}

// E4: settle-tree.test.ts:97
#[test]
fn settle_leaves_a_tree_with_a_thread_asking_the_person() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.status(PRINTER, "completed");
        w.facts.borrow_mut()[PRINTER]["asking"] = json!("ask_1");
        let left = w.area.settle_threads(ids(&[LEAD, PRINTER]), None, SettleAsk::default()).await;
        assert_eq!(
            answer(&left),
            json!({ "settled": [], "left": [{ "threadId": LEAD, "why": SETTLE_WORKING }, { "threadId": PRINTER, "why": SETTLE_WORKING }] }),
        );
        let done = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk::default()).await;
        assert_eq!(answer(&done)["left"], json!([]));
    });
}

// E4-E6: settle-tree.test.ts:77
#[test]
fn settle_finished_takes_only_finished_children() {
    run_local(async {
        let w = World::new();
        w.tree();
        let done = w.area.settle_threads(ids(&[LEAD]), None, SettleAsk { finished: Some(true), processes: None }).await;
        assert_eq!(
            answer(&done),
            json!({ "settled": [{ "threadId": PARSER, "title": "build the parser" }, { "threadId": TESTS, "title": "write its tests" }], "left": [] }),
            "the lead, the working child and the failed child stay",
        );
        // What it moved and the thread it named are written together, so the workspace is told once.
        let marks = w.marks();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].ids, ids(&[PARSER, TESTS, LEAD]));
        assert_eq!(w.stamp_of(0, PARSER), norm(&json!({ "readAt": now(), "settledAt": now() })));
        assert_eq!(w.stamp_of(0, TESTS), norm(&json!({ "readAt": now(), "settledAt": now() })));
        assert_eq!(w.stamp_of(0, LEAD), norm(&json!({ "settleNamedAt": now() })), "the named thread itself is not settled");
        assert_eq!([LEAD, PARSER, TESTS, PRINTER, LINKER].map(|id| w.is_settled(id)), [false, true, true, false, false]);
    });
}

// E4: settle-tree.test.ts:87
#[test]
fn settle_finished_leaves_a_finished_thread_with_work_under_it() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.thread("probe-001", "running", "probe the parser");
        w.under(LEAD, &[PARSER, TESTS, PRINTER, LINKER, "probe-001"]);
        w.under(PARSER, &[TESTS, "probe-001"]);
        let done = w.area.settle_threads(ids(&[LEAD]), None, SettleAsk { finished: Some(true), processes: None }).await;
        assert_eq!(
            answer(&done),
            json!({ "settled": [{ "threadId": TESTS, "title": "write its tests" }], "left": [{ "threadId": PARSER, "why": SETTLE_WORKING }] }),
        );
        assert_eq!([LEAD, PARSER, TESTS, PRINTER, LINKER, "probe-001"].map(|id| w.is_settled(id)), [false, false, true, false, false, false]);
    });
}

// E5, E7: activity.test.ts:225
#[test]
fn settle_with_processes_meets_them_first() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.status(PRINTER, "completed");
        let ended: ActivityEnded = serde_json::from_value(json!({ "pid": 4200, "ended": [], "forced": [], "left": [] })).unwrap();
        *w.meets.ended.borrow_mut() = vec![ended];
        let origin = scoped(LEAD, LEAD);
        let done = w.area.settle_threads(ids(&[PARSER]), Some(origin.clone()), SettleAsk { finished: None, processes: Some(ProcessesAsk::Stop) }).await;
        assert_eq!(
            answer(&done),
            json!({
                "settled": [{ "threadId": PARSER, "title": "build the parser" }, { "threadId": TESTS, "title": "write its tests" }],
                "left": [], "ended": [{ "pid": 4200, "ended": [], "forced": [], "left": [] }],
            }),
        );
        assert_eq!(w.meets.seen.borrow().clone(), vec![(ids(&[PARSER, TESTS]), ProcessesAsk::Stop, MeetVerb::Settle, Some(origin))]);
        let order: Vec<String> = w.host.hooks.calls().into_iter().filter(|c| c.starts_with("meet") || c == "mark").collect();
        assert_eq!(order, vec![format!("meet({PARSER},{TESTS})"), "mark".to_string()], "the processes are met before anything is marked");
    });
}

// E5, E7: activity.test.ts:225
#[test]
fn settle_asks_nothing_of_processes_unless_told_and_names_no_ended_where_none_ended() {
    run_local(async {
        let w = World::new();
        w.tree();
        let done = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk::default()).await;
        assert!(answer(&done).get("ended").is_none());
        assert_eq!(w.host.hooks.called("meet"), Vec::<String>::new());
        let w = World::new();
        w.tree();
        let done = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk { finished: None, processes: Some(ProcessesAsk::Keep) }).await;
        assert!(answer(&done).get("ended").is_none(), "ended rides the answer only where any");
        assert_eq!(w.meets.seen.borrow().len(), 1);
    });
}

// E5: activity.test.ts:200
#[test]
fn a_refusal_from_meet_settles_nothing() {
    run_local(async {
        let w = World::new();
        w.tree();
        let refused = usage_refusal("the thread to settle still runs 2 processes: 4200 node (dev, :5173) of parser-0:", "settle it again with processes stop.");
        *w.meets.refuse.borrow_mut() = Some(refused.clone());
        let e = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk { finished: None, processes: Some(ProcessesAsk::Ask) }).await.unwrap_err();
        assert_eq!(said(&e), said(&refused));
        assert_eq!(w.marks(), Vec::<MarkCall>::new());
        assert!(!w.is_settled(PARSER));
    });
}

// E6
#[test]
fn two_settles_in_one_ms_get_two_stamps() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.thread("other-001", "completed", "another");
        w.area.settle_threads(ids(&[TESTS]), None, SettleAsk::default()).await.unwrap();
        w.area.settle_threads(ids(&["other-001"]), None, SettleAsk::default()).await.unwrap();
        assert_eq!(w.stamp_of(0, TESTS)["settledAt"], json!(now()));
        assert_eq!(w.stamp_of(1, "other-001")["settledAt"], json!(now() + 1.0), "max(now, last + 1) with no time passed");
        assert_eq!(w.stamp_of(1, "other-001")["readAt"], json!(now() + 1.0));
        assert_eq!(w.stamp_of(1, "other-001")["settleNamedAt"], json!(now() + 1.0));
        w.host.clock.advance(10);
        w.area.settle_threads(ids(&[LINKER]), None, SettleAsk::default()).await.unwrap();
        assert_eq!(w.stamp_of(2, LINKER)["settledAt"], json!(now() + 10.0), "the clock, once it is past the last stamp");
        // The stamp moves on every settle, one that takes nothing included (sessions.ts:1136 sits outside the `if`).
        w.area.settle_threads(ids(&[LINKER]), None, SettleAsk::default()).await.unwrap();
        w.thread("more-0001", "completed", "more");
        w.area.settle_threads(ids(&["more-0001"]), None, SettleAsk::default()).await.unwrap();
        let last = w.marks().len() - 1;
        assert_eq!(w.stamp_of(last, "more-0001")["settledAt"], json!(now() + 12.0));
    });
}

// E8: thread-marks.test.ts:131
#[test]
fn a_sidebar_place_that_is_no_number_is_refused() {
    run_local(async {
        let w = World::new();
        w.tree();
        for (bad, word) in [(f64::NAN, "NaN"), (f64::INFINITY, "Infinity"), (f64::NEG_INFINITY, "-Infinity")] {
            let mut order = marks_of(json!({}));
            order.order = Some(Some(JsNumber(bad)));
            let e = w.area.mark_threads(ids(&[PARSER]), order, None).await.unwrap_err();
            let expected = usage_refusal(&format!("a thread's place in the sidebar is a number, and {word} is not one."), "Send a finite number, or leave the key out.");
            assert_eq!(said(&e), said(&expected));
            assert_eq!(e.kind, Some(Kind::Usage));
            let mut pinned = marks_of(json!({}));
            pinned.pinned = Some(ThreadMarksPinned::V1(JsNumber(bad)));
            let e = w.area.mark_threads(ids(&[PARSER]), pinned, None).await.unwrap_err();
            assert_eq!(said(&e), said(&expected));
        }
        assert_eq!(w.marks(), Vec::<MarkCall>::new(), "nothing lands on a record");
        assert_eq!(w.host.hooks.called("arm_resume"), Vec::<String>::new());
        // A finite key, a fraction included, goes through whole.
        w.area.mark_threads(ids(&[PARSER]), marks_of(json!({ "pinned": 1234.5625, "order": 1_727_431_200_000.25 })), None).await.unwrap();
        assert_eq!(w.stamp_of(0, PARSER), norm(&json!({ "pinnedAt": 1234.5625, "order": 1_727_431_200_000.25 })));
    });
}

// E9: thread-marks.test.ts:94, :113, :131, :159
#[test]
fn pin_fold_snooze_and_section_stamps() {
    run_local(async {
        let w = World::new();
        w.tree();
        let at = now();
        let until = at + 60.0 * 60_000.0;
        let placed = json!({ "name": "working", "whileState": "completed:sess-1" });
        let cases: Vec<(Value, Value)> = vec![
            (json!({ "pinned": true }), json!({ "pinnedAt": at })),
            (json!({ "pinned": false }), json!({ "pinnedAt": null })),
            (json!({ "pinned": 1234.5625 }), json!({ "pinnedAt": 1234.5625 })),
            (json!({ "order": 1_727_431_200_000.25 }), json!({ "order": 1_727_431_200_000.25 })),
            (json!({ "order": null }), json!({ "order": null })),
            (json!({ "folded": true }), json!({ "foldedAt": at })),
            (json!({ "folded": false }), json!({ "foldedAt": null })),
            (json!({ "snoozedUntil": until }), json!({ "snoozedUntil": until, "readAt": at })),
            (json!({ "snoozedUntil": null }), json!({ "snoozedUntil": null })),
            (json!({ "section": placed.clone() }), json!({ "section": placed.clone() })),
            (json!({ "section": null }), json!({ "section": null })),
            (json!({ "pinned": true, "section": placed.clone() }), json!({ "pinnedAt": at, "section": placed.clone() })),
            (json!({ "pinned": false, "section": null }), json!({ "pinnedAt": null, "section": null })),
            (json!({}), json!({})),
        ];
        let origin = scoped(LEAD, LEAD);
        for (n, (given, stamps)) in cases.iter().enumerate() {
            w.area.mark_threads(ids(&[PARSER, TESTS]), marks_of(given.clone()), Some(origin.clone())).await.unwrap();
            let marks = w.marks();
            assert_eq!(marks.len(), n + 1, "one mark for {given}");
            assert_eq!(marks[n].ids, ids(&[PARSER, TESTS]));
            assert_eq!(marks[n].origin, Some(origin.clone()));
            assert_eq!(w.stamp_of(n, PARSER), norm(stamps), "the stamps of {given}");
            assert_eq!(w.stamp_of(n, TESTS), norm(stamps), "every thread gets the same stamps of {given}");
        }
        assert_eq!(w.host.hooks.called("arm_resume"), Vec::<String>::new());
    });
}

// E9: thread-marks.test.ts:131
#[test]
fn a_snooze_reads_the_time_off_the_clock_at_the_mark() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.host.clock.advance(5_000);
        w.area.mark_threads(ids(&[PARSER]), marks_of(json!({ "pinned": true, "folded": true, "snoozedUntil": 9_000_000_000_000.0 })), None).await.unwrap();
        let at = now() + 5_000.0;
        assert_eq!(w.stamp_of(0, PARSER), norm(&json!({ "pinnedAt": at, "foldedAt": at, "snoozedUntil": 9_000_000_000_000.0, "readAt": at })));
    });
}

// E9
#[test]
fn resume_at_reset_arms_first_and_a_mark_of_only_that_writes_no_stamps() {
    run_local(async {
        let w = World::new();
        w.tree();
        let origin = scoped(LEAD, LEAD);
        w.area.mark_threads(ids(&[PARSER]), marks_of(json!({ "resumeAtReset": true })), Some(origin.clone())).await.unwrap();
        assert_eq!(w.host.hooks.called("arm_resume"), vec!["arm_resume(true)".to_string()]);
        assert_eq!(w.host.hooks.called("mark"), Vec::<String>::new(), "a mark with only resumeAtReset writes no stamps");
        w.area.mark_threads(ids(&[PARSER]), marks_of(json!({ "resumeAtReset": false })), None).await.unwrap();
        assert_eq!(w.host.hooks.called("arm_resume"), vec!["arm_resume(true)".to_string(), "arm_resume(false)".to_string()]);
        w.area.mark_threads(ids(&[PARSER]), marks_of(json!({ "resumeAtReset": true, "pinned": true })), None).await.unwrap();
        let order: Vec<String> = w.host.hooks.calls().into_iter().filter(|c| c.starts_with("arm_resume") || c == "mark").collect();
        assert_eq!(order, vec!["arm_resume(true)", "arm_resume(false)", "arm_resume(true)", "mark"], "the resume is armed before the stamps are written");
        assert_eq!(w.stamp_of(0, PARSER), norm(&json!({ "pinnedAt": now() })));
    });
}

// E9: thread-marks.test.ts:339
#[test]
fn a_mark_the_runtime_refuses_is_the_marks_refusal() {
    run_local(async {
        let w = World::new();
        w.tree();
        let refused = not_found_refusal("no thread thr_nobo");
        let e2 = refused.clone();
        w.host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(move |_ids: &[String], _s: StampsOf, _o: Option<&Caller>| {
                let e = e2.clone();
                async move { Err::<(), HostError>(e) }.boxed_local()
            }),
        );
        let e = w.area.mark_threads(ids(&[PARSER, "thr_nobody"]), marks_of(json!({ "pinned": true })), None).await.unwrap_err();
        assert_eq!(said(&e), said(&refused));
    });
}

// E10: settle-tree.test.ts:114, :164
#[test]
fn restore_takes_back_what_one_settle_took() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.area.settle_threads(ids(&[PARSER]), None, SettleAsk::default()).await.unwrap();
        w.host.clock.advance(3 * 60 * 60 * 1000);
        let back = w.area.restore_threads(ids(&[PARSER]), None).await.unwrap();
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            json!({ "restored": [{ "threadId": PARSER, "title": "build the parser" }, { "threadId": TESTS, "title": "write its tests" }] }),
        );
        let marks = w.marks();
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[1].ids, ids(&[PARSER, TESTS]));
        // Taken out of the fold as just read: the quiet counts from now, and the settled stamp is taken off.
        let stamp = norm(&json!({ "readAt": now() + 3.0 * 3_600_000.0, "settledAt": null }));
        assert_eq!(w.stamp_of(1, PARSER), stamp);
        assert_eq!(w.stamp_of(1, TESTS), stamp);
        assert_eq!([LEAD, PARSER, TESTS, PRINTER, LINKER].map(|id| w.is_settled(id)), [false; 5]);
        // A thread nothing settled has nothing to take back, and nothing is marked.
        let none = w.area.restore_threads(ids(&[LEAD]), None).await.unwrap();
        assert_eq!(serde_json::to_value(&none).unwrap(), json!({ "restored": [] }));
        assert_eq!(w.marks().len(), 2);
    });
}

// E10: settle-tree.test.ts:131
#[test]
fn restore_leaves_a_thread_settled_by_an_earlier_settle() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.area.settle_threads(ids(&[TESTS]), None, SettleAsk::default()).await.unwrap();
        let second = w.area.settle_threads(ids(&[PARSER]), None, SettleAsk::default()).await;
        assert_eq!(answer(&second)["settled"], json!([{ "threadId": PARSER, "title": "build the parser" }]));
        let back = w.area.restore_threads(ids(&[PARSER]), None).await.unwrap();
        assert_eq!(serde_json::to_value(&back).unwrap(), json!({ "restored": [{ "threadId": PARSER, "title": "build the parser" }] }));
        assert_eq!([PARSER, TESTS].map(|id| w.is_settled(id)), [false, true], "the earlier settle's thread stays where it is");
        // Undo's ids take back exactly what the answer named.
        w.status(PRINTER, "completed");
        let all = w.area.settle_threads(ids(&[LEAD]), None, SettleAsk::default()).await.unwrap();
        let named: Vec<String> = all.settled.iter().map(|s| s.thread_id.clone()).collect();
        assert_eq!(named, ids(&[LEAD, PARSER, PRINTER, LINKER]));
        let back = w.area.restore_threads(named.clone(), None).await.unwrap();
        assert_eq!(back.restored.iter().map(|s| s.thread_id.clone()).collect::<Vec<_>>(), named);
        assert_eq!([LEAD, PARSER, TESTS, PRINTER, LINKER].map(|id| w.is_settled(id)), [false, false, true, false, false]);
    });
}

// E10: settle-tree.test.ts:148
#[test]
fn restore_of_a_finished_settle_takes_back_what_it_moved_and_none_settled_before_it() {
    run_local(async {
        let w = World::new();
        w.tree();
        w.area.settle_threads(ids(&[TESTS]), None, SettleAsk::default()).await.unwrap();
        let done = w.area.settle_threads(ids(&[LEAD]), None, SettleAsk { finished: Some(true), processes: None }).await;
        assert_eq!(answer(&done)["settled"], json!([{ "threadId": PARSER, "title": "build the parser" }]));
        assert_eq!(w.marks()[1].ids, ids(&[PARSER, LEAD]));
        let back = w.area.restore_threads(ids(&[LEAD]), None).await.unwrap();
        assert_eq!(
            serde_json::to_value(&back).unwrap(),
            json!({ "restored": [{ "threadId": PARSER, "title": "build the parser" }] }),
            "the lead was named, not settled, and the grandchild was settled by the settle before",
        );
        assert_eq!([LEAD, PARSER, TESTS, PRINTER, LINKER].map(|id| w.is_settled(id)), [false, false, true, false, false]);
    });
}
