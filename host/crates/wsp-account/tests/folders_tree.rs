// SPDX-License-Identifier: AGPL-3.0-only
//! The host's own git writes one at a time, the removal of a worktree wsp made, the record that stays while threads
//! name it, and the one sweep every ten minutes. Ported at the area's seam from worktree-sweep.test.ts and the removal
//! and worktree-gone cases of folder-threads.test.ts at 65f6b89c7, plus the cases spec/folders_tree.md pins beyond
//! them. Every sentence is the TS source's own text (protocol/src/projects.ts, types/wiring.ts). Time is the test
//! clock's: nothing sleeps.
//!
//! A worktree's JSON is compared as a value: `WorktreeFolder` writes its keys in the struct's order, not the TS
//! literals' order, so key order is not asserted here.
#[path = "support/base.rs"]
mod base;
#[path = "support/folders.rs"]
mod folders;

use std::cell::RefCell;
use std::rc::Rc;

use base::{err_answer, kinded, plain, project, rec, rec_json, record, run, FakeBase};
use folders::{preferences, FakeFolders};
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_account::projects::folders::tree::{
    git_said, parse_worktrees, ListedWorktree, RemoveAsk, KEPT_FOR_MS, SETTLED_STANDS_MS, SWEEP_MS,
};
use wsp_account::projects::folders::{folders_area, Folders};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_wire::views::workspace_more::WorktreeSettledWhy;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::{HostError, Kind};

const STATE: &str = "/home/me/.wsp";
const GONE_PATH: &str = "/nonexistent/wsp-worktrees/feat-x";
const NO_COPIER_HERE: &str =
    "this host has no daemon binary beside it to make a worktree with; stage this wsp's daemon binary and try again";
const BUSY: &str = "a thread is working in that worktree; let its turn end or stop it first";
const RUNNING: &str = "a thread is running";
const ABANDONED: &str = "kept 7 days; remove it by hand";
const START: i64 = 1_000_000;

struct Rig {
    base: Rc<FakeBase>,
    fake: Rc<FakeFolders>,
    folders: Rc<Folders>,
}

impl Rig {
    fn new() -> Rig {
        let base = FakeBase::new();
        let fake = FakeFolders::new(base.clone());
        let folders = folders_area(fake.clone());
        base.hold_project(project("p_web", "web", "here", json!({ "git": { "top": "/home/me/web" } })));
        Rig { base, fake, folders }
    }
    /// A made, standing worktree's record at `path`, `over` written on top of its worktree.
    fn tree(&self, id: &str, path: &str, over: Value) -> Entry {
        let mut worktree = json!({ "path": path, "branch": "feat/x", "made": true });
        base::merge(&mut worktree, over);
        self.base.hold(record(id, "local", "p_web", json!({ "worktree": worktree })))
    }
    /// A thread names the record.
    fn thread_on(&self, id: &str) {
        self.base.threads.borrow_mut().push((format!("t_{id}"), base::thread_on(id)));
    }
    /// Replaces the scripted answers of a call name with one.
    fn reply(&self, name: &str, answer: Value) {
        self.base.script.borrow_mut().shift_remove(name);
        self.base.answer_with(name, vec![answer]);
    }
    fn git_says(&self, args: &str, stdout: &str) {
        self.reply(&format!("git:{args}"), json!({ "exitCode": 0, "stdout": stdout, "stderr": "" }));
    }
    fn after_worktree(&self, command: &str) {
        *self.base.prefs.borrow_mut() = Some(preferences(json!({ "projectDefaults": { "p_web": { "afterWorktree": command } } })));
    }
    fn persisted_trees(&self) -> Vec<Value> {
        self.base.persisted.borrow().iter().map(|r| serde_json::to_value(r).unwrap()["worktree"].clone()).collect()
    }
    fn persisted_ids(&self) -> Vec<String> {
        self.base.persisted.borrow().iter().map(|r| r.id.clone()).collect()
    }
    fn calls_made(&self) -> usize {
        self.base.calls.borrow().len()
    }
    /// The calls that write or reach out, in order.
    fn effects(&self) -> Vec<String> {
        self.base.names_of(&["git", "copier_worktree_remove", "persist", "status_now", "drop", "read_pull_request", "local_exec", "emit"])
    }
}

fn refusal<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected a refusal"),
    }
}

fn worktree_of(entry: &Entry) -> Value {
    rec_json(entry).get("worktree").cloned().unwrap_or(Value::Null)
}

fn exec(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: code.into(), stdout: stdout.to_owned(), stderr: stderr.to_owned() }
}

fn status_ask(path: &str) -> Value {
    json!({ "cwd": path, "args": ["status", "--porcelain"], "timeoutMs": null, "input": null })
}

fn remove_ask(path: &str, force: bool) -> Value {
    json!({ "from": "/home/me/web", "home": STATE, "path": path, "force": force })
}

fn on_disk() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap().to_owned();
    (dir, path)
}

async fn settle() {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
}

type Log = Rc<RefCell<Vec<String>>>;

/// Work that logs its start and end around an optional gate, and answers `out`.
fn work(
    log: &Log,
    name: &'static str,
    gate: Option<oneshot::Receiver<()>>,
    out: Result<Value, HostError>,
) -> LocalBoxFuture<'static, Result<Value, HostError>> {
    let log = log.clone();
    async move {
        log.borrow_mut().push(format!("{name}:start"));
        if let Some(g) = gate {
            let _ = g.await;
        }
        log.borrow_mut().push(format!("{name}:end"));
        out
    }
    .boxed_local()
}

fn at(log: &Log, entry: &str) -> usize {
    log.borrow().iter().position(|l| l == entry).unwrap_or_else(|| panic!("{entry} is not in {:?}", log.borrow()))
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 1: queued

#[test]
fn queued_runs_one_folders_writes_in_call_order_and_others_alongside() {
    let rig = Rig::new();
    run(async {
        let log: Log = Rc::new(RefCell::new(vec![]));
        let (open_a, gate_a) = oneshot::channel();
        let fa = rig.folders.queued("ws_a", work(&log, "a", Some(gate_a), Ok(json!("a"))));
        let fb = rig.folders.queued("ws_a", work(&log, "b", None, Ok(json!("b"))));
        let fc = rig.folders.queued("ws_b", work(&log, "c", None, Ok(json!("c"))));
        let (ha, hb, hc) = (tokio::task::spawn_local(fa), tokio::task::spawn_local(fb), tokio::task::spawn_local(fc));
        settle().await;
        let mut held = log.borrow().clone();
        held.sort();
        assert_eq!(held, vec!["a:start", "c:end", "c:start"]);
        open_a.send(()).unwrap();
        assert_eq!(ha.await.unwrap().unwrap(), json!("a"));
        assert_eq!(hb.await.unwrap().unwrap(), json!("b"));
        assert_eq!(hc.await.unwrap().unwrap(), json!("c"));
        assert!(at(&log, "c:end") < at(&log, "a:end"));
        assert!(at(&log, "a:start") < at(&log, "a:end"));
        assert!(at(&log, "a:end") < at(&log, "b:start"));
        assert!(at(&log, "b:start") < at(&log, "b:end"));
    });
}

#[test]
fn a_failed_write_does_not_block_the_next_and_its_caller_sees_it() {
    let rig = Rig::new();
    run(async {
        let log: Log = Rc::new(RefCell::new(vec![]));
        let (open_a, gate_a) = oneshot::channel();
        let fa = rig.folders.queued("ws_a", work(&log, "a", Some(gate_a), Err(plain("boom"))));
        let fb = rig.folders.queued("ws_a", work(&log, "b", None, Ok(json!(2))));
        let (ha, hb) = (tokio::task::spawn_local(fa), tokio::task::spawn_local(fb));
        settle().await;
        assert_eq!(*log.borrow(), vec!["a:start"]);
        open_a.send(()).unwrap();
        assert_eq!(ha.await.unwrap().err(), Some(plain("boom")));
        assert_eq!(hb.await.unwrap().ok(), Some(json!(2)));
        assert_eq!(*log.borrow(), vec!["a:start", "a:end", "b:start", "b:end"]);
    });
}

// The key map is not reachable from here: what shows it is that a key a later write holds stays when an earlier
// tail settles, and that a key whose chain is over holds the next write behind nothing.
#[test]
fn the_queue_forgets_a_key_once_its_tail_settles() {
    let rig = Rig::new();
    run(async {
        let log: Log = Rc::new(RefCell::new(vec![]));
        let (open_a, gate_a) = oneshot::channel();
        let (open_b, gate_b) = oneshot::channel();
        let fa = rig.folders.queued("ws_a", work(&log, "a", Some(gate_a), Ok(json!("a"))));
        let fb = rig.folders.queued("ws_a", work(&log, "b", Some(gate_b), Ok(json!("b"))));
        let (ha, hb) = (tokio::task::spawn_local(fa), tokio::task::spawn_local(fb));
        open_a.send(()).unwrap();
        settle().await;
        assert_eq!(*log.borrow(), vec!["a:start", "a:end", "b:start"]);
        // a's tail settled while b's is the key's: c waits for b.
        let fc = rig.folders.queued("ws_a", work(&log, "c", None, Ok(json!("c"))));
        let hc = tokio::task::spawn_local(fc);
        settle().await;
        assert_eq!(*log.borrow(), vec!["a:start", "a:end", "b:start"]);
        open_b.send(()).unwrap();
        ha.await.unwrap().unwrap();
        hb.await.unwrap().unwrap();
        hc.await.unwrap().unwrap();
        assert_eq!(*log.borrow(), vec!["a:start", "a:end", "b:start", "b:end", "c:start", "c:end"]);
        // The chain is over: the next write starts at once.
        let fd = rig.folders.queued("ws_a", work(&log, "d", None, Ok(json!("d"))));
        assert_eq!(tokio::task::spawn_local(fd).await.unwrap().unwrap(), json!("d"));
        assert_eq!(log.borrow().len(), 8);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 2 and 3: remove_worktree, refuse_changed

#[test]
fn remove_refuses_a_worktree_wsp_did_not_make() {
    // Ports folder-threads.test.ts:817.
    let rig = Rig::new();
    rig.base.hold_project(project("p_flat", "flat", "here", json!({})));
    let (_dir, path) = on_disk();
    let theirs = rig.tree("ws_theirs", &path, json!({ "branch": "theirs", "made": false }));
    let none = rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    let flat =
        rig.base.hold(record("ws_flat", "local", "p_flat", json!({ "worktree": { "path": path, "branch": "feat/x", "made": true } })));
    let said = |branch: &str| plain(&format!("wsp made no worktree for {branch}; git worktree remove takes one wsp did not make"));
    assert_eq!(refusal(run(rig.folders.remove_worktree(&theirs, true, RemoveAsk::default()))), said("theirs"));
    assert_eq!(refusal(run(rig.folders.remove_worktree(&none, false, RemoveAsk::default()))), said(""));
    assert_eq!(refusal(run(rig.folders.remove_worktree(&flat, false, RemoveAsk::default()))), said("feat/x"));
    assert_eq!(rig.effects(), Vec::<String>::new());
}

#[test]
fn remove_with_no_copier_refuses_before_it_reads_anything() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let entry = rig.tree("ws_tree", &path, json!({}));
    rig.fake.copier.set(false);
    rig.fake.running.borrow_mut().insert("ws_tree".to_owned());
    assert_eq!(refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk::default()))), kinded(NO_COPIER_HERE, Kind::Invalid));
    assert_eq!(rig.effects(), Vec::<String>::new());
}

#[test]
fn remove_refuses_while_a_turn_runs_unless_ending() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let entry = rig.tree("ws_tree", &path, json!({}));
    rig.fake.running.borrow_mut().insert("ws_tree".to_owned());
    assert_eq!(refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk::default()))), kinded(BUSY, Kind::Conflict));
    assert_eq!(rig.base.args_of("turn_runs"), vec![json!({ "id": "ws_tree" })]);
    assert_eq!(rig.effects(), Vec::<String>::new());
    run(rig.folders.remove_worktree(&entry, false, RemoveAsk { ending: true, check: false })).unwrap();
    assert_eq!(rig.base.args_of("copier_worktree_remove"), vec![remove_ask(&path, false)]);
    assert_eq!(rig.effects(), vec!["git", "copier_worktree_remove"]);
}

// Ports folder-threads.test.ts:800 and :974: files no commit holds are never lost to an unforced removal.
#[test]
fn remove_refuses_over_uncommitted_files_naming_how_many() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let entry = rig.tree("ws_tree", &path, json!({}));
    let many = "that worktree has 2 files not committed; commit them, or remove it with --force to lose them";
    let one = "that worktree has 1 file not committed; commit it, or remove it with --force to lose it";
    rig.git_says("status --porcelain", " M a.txt\n?? b.txt\n\n   \n");
    assert_eq!(refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk::default()))), kinded(many, Kind::Conflict));
    rig.git_says("status --porcelain", "?? notes.md\n");
    assert_eq!(refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk::default()))), kinded(one, Kind::Conflict));
    rig.reply("git:status --porcelain", json!({ "exitCode": 128, "stdout": "", "stderr": "fatal: not a git repository\n" }));
    assert_eq!(refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk::default()))), plain("fatal: not a git repository"));
    rig.reply("git:status --porcelain", json!({ "exitCode": 128, "stdout": "oops\n", "stderr": " " }));
    assert_eq!(refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk::default()))), plain("oops"));
    assert_eq!(rig.base.args_of("git"), vec![status_ask(&path); 4]);
    assert_eq!(rig.base.args_of("copier_worktree_remove"), Vec::<Value>::new());
    assert_eq!(rig.persisted_ids(), Vec::<String>::new());
    // Blank lines only are a clean tree; a forced removal asks git nothing.
    rig.git_says("status --porcelain", " \n\n");
    run(rig.folders.remove_worktree(&entry, false, RemoveAsk { ending: true, check: false })).unwrap();
    run(rig.folders.remove_worktree(&entry, true, RemoveAsk { ending: true, check: false })).unwrap();
    assert_eq!(rig.base.args_of("git").len(), 5);
    assert_eq!(rig.base.args_of("copier_worktree_remove"), vec![remove_ask(&path, false), remove_ask(&path, true)]);
}

#[test]
fn remove_with_check_removes_nothing() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let entry = rig.tree("ws_tree", &path, json!({}));
    rig.thread_on("ws_tree");
    run(rig.folders.remove_worktree(&entry, false, RemoveAsk { ending: false, check: true })).unwrap();
    assert_eq!(rig.base.args_of("git"), vec![status_ask(&path)]);
    assert_eq!(rig.effects(), vec!["git"]);
    // It refuses as the removal would.
    rig.git_says("status --porcelain", "?? a\n");
    let e = refusal(run(rig.folders.remove_worktree(&entry, false, RemoveAsk { ending: false, check: true })));
    assert_eq!(e, kinded("that worktree has 1 file not committed; commit it, or remove it with --force to lose it", Kind::Conflict));
    assert_eq!(rig.effects(), vec!["git", "git"]);
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true }));
}

// Ports folder-threads.test.ts:766, :800 and :1056: the tree goes, the record stays gone while a thread names it.
#[test]
fn remove_takes_the_tree_and_marks_the_record_gone() {
    let rig = Rig::new();
    rig.after_worktree("make setup");
    let (_dir, path) = on_disk();
    rig.base.answer_with(
        "copier_worktree",
        vec![
            json!({ "path": path, "branch": "feat/x", "made": true, "carried": [], "fresh": true, "modules": [], "ms": 1 }),
            json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-y", "branch": "feat/y", "made": true, "carried": [], "fresh": true, "modules": [], "ms": 1 }),
        ],
    );
    let p = rig.base.projects.borrow()["p_web"].clone();
    let first = run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/x", None, None, None)).unwrap();
    let second = run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/y", None, None, None)).unwrap();
    let (first_id, second_id) = (rec(&first).id, rec(&second).id);
    rig.thread_on(&first_id);
    rig.base.calls.borrow_mut().clear();
    rig.base.persisted.borrow_mut().clear();
    run(rig.folders.remove_worktree(&first, false, RemoveAsk::default())).unwrap();
    assert_eq!(rig.base.args_of("copier_worktree_remove"), vec![remove_ask(&path, false)]);
    assert_eq!(rig.base.names_of(&["copier_worktree_remove", "persist", "drop"]), vec!["copier_worktree_remove", "persist"]);
    assert_eq!(
        worktree_of(&first),
        json!({ "path": path, "branch": "feat/x", "made": true, "settled": { "at": START, "why": "removed" }, "gone": true })
    );
    assert_eq!(rig.persisted_trees(), vec![worktree_of(&first)]);
    assert_eq!(rig.folders.take_setup_line(&first_id), None);
    assert_eq!(rig.folders.take_setup_line(&second_id), Some("Before this thread started, its new worktree ran:\n- make setup".to_owned()));
}

// Ports folder-threads.test.ts:766 and worktree-sweep.test.ts:233: a tree gone from disk skips the copier.
#[test]
fn remove_of_a_tree_gone_from_disk_skips_the_copier() {
    let rig = Rig::new();
    let named = rig.tree("ws_named", GONE_PATH, json!({}));
    let free = rig.tree("ws_free", GONE_PATH, json!({}));
    rig.thread_on("ws_named");
    run(rig.folders.remove_worktree(&named, false, RemoveAsk::default())).unwrap();
    run(rig.folders.remove_worktree(&free, true, RemoveAsk::default())).unwrap();
    assert_eq!(rig.base.args_of("git"), Vec::<Value>::new());
    assert_eq!(rig.base.args_of("copier_worktree_remove"), Vec::<Value>::new());
    assert_eq!(
        worktree_of(&named),
        json!({ "path": GONE_PATH, "branch": "feat/x", "made": true, "settled": { "at": START, "why": "removed" }, "gone": true })
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_named"]);
    assert_eq!(rig.base.args_of("drop"), vec![json!({ "id": "ws_free" })]);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 4: drop_checkpoints

// Ports folder-threads.test.ts:787 and the landing case that follows it.
#[test]
fn drop_checkpoints_waits_on_the_landing_and_warns_a_refusal() {
    let rig = Rig::new();
    let entry = rig.base.hold(record("ws_f", "local", "p_web", json!({})));
    let thread = "01234567-89ab-4cde-8f01-23456789abcd";
    let land = rig.fake.hold_landing(thread);
    let warned = wsp_core::console::capture();
    run(async {
        let mut dropping = Box::pin(rig.folders.drop_checkpoints(&entry, thread));
        assert!(futures::poll!(&mut dropping).is_pending());
        assert_eq!(rig.base.asks(), Vec::<Value>::new());
        land.send(()).unwrap();
        dropping.await.unwrap();
    });
    assert_eq!(rig.base.asks(), vec![json!({ "op": "git.checkpointDrop", "cwd": "/checkouts/ws_f", "scope": "ws_f", "thread": thread })]);
    assert_eq!(warned.lines(), Vec::<String>::new());
    // A refusal is warned and the call answers.
    rig.base.answer_with("ask:git.checkpointDrop", vec![err_answer("no git.checkpointDrop here", None)]);
    run(rig.folders.drop_checkpoints(&entry, thread)).unwrap();
    assert_eq!(warned.lines(), vec!["the checkpoints of thread 01234567 were not dropped: no git.checkpointDrop here"]);
    // A record that is not a repo asks the daemon nothing.
    rig.fake.not_repo.borrow_mut().insert("ws_f".to_owned());
    run(rig.folders.drop_checkpoints(&entry, thread)).unwrap();
    assert_eq!(rig.base.asks().len(), 2);
    assert_eq!(rig.base.args_of("with_daemon").len(), 2);
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 5 and 6: holds_thread, worktree_gone

#[test]
fn holds_thread_reads_session_rows_and_thread_records() {
    let rig = Rig::new();
    assert!(!rig.folders.holds_thread("ws_a"));
    rig.base
        .sessions
        .borrow_mut()
        .push(serde_json::from_value(json!({ "id": "s1", "workspaceId": "ws_a", "harness": "claude", "status": "completed" })).unwrap());
    assert!(rig.folders.holds_thread("ws_a"));
    assert!(!rig.folders.holds_thread("ws_b"));
    rig.base.threads.borrow_mut().push(("t2".to_owned(), base::thread_on("ws_b")));
    assert!(rig.folders.holds_thread("ws_b"));
    assert!(!rig.folders.holds_thread("ws_c"));
}

#[test]
fn worktree_gone_drops_a_record_no_thread_names() {
    let rig = Rig::new();
    let entry = rig.tree("ws_tree", GONE_PATH, json!({ "settled": { "at": 5, "why": "merged" } }));
    let before = worktree_of(&entry);
    run(rig.folders.worktree_gone(&entry, WorktreeSettledWhy::Removed, false)).unwrap();
    assert_eq!(rig.base.args_of("drop"), vec![json!({ "id": "ws_tree" })]);
    assert_eq!(rig.persisted_ids(), Vec::<String>::new());
    assert_eq!(worktree_of(&entry), before);
    // A record with no worktree is nothing to mark.
    let folder = rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    run(rig.folders.worktree_gone(&folder, WorktreeSettledWhy::Removed, false)).unwrap();
    assert_eq!(rig.base.args_of("drop").len(), 1);
}

// Ports folder-threads.test.ts:863 and worktree-sweep.test.ts:233: gone keeps the first settled and drops the marks.
#[test]
fn worktree_gone_keeps_a_record_a_thread_names_as_gone_with_its_first_settled() {
    let rig = Rig::new();
    let settled = rig.tree(
        "ws_settled",
        GONE_PATH,
        json!({ "madeFor": "t_root", "settled": { "at": 5, "why": "merged" }, "kept": "a thread is running", "removeFailed": "locked" }),
    );
    let fresh = rig.tree("ws_fresh", "/nonexistent/other", json!({ "kept": ABANDONED }));
    rig.base.sessions.borrow_mut().push(
        serde_json::from_value(json!({ "id": "s1", "workspaceId": "ws_settled", "harness": "claude", "status": "completed" })).unwrap(),
    );
    rig.thread_on("ws_fresh");
    run(rig.folders.worktree_gone(&settled, WorktreeSettledWhy::Removed, false)).unwrap();
    run(rig.folders.worktree_gone(&fresh, WorktreeSettledWhy::Deleted, false)).unwrap();
    assert_eq!(
        worktree_of(&settled),
        json!({ "path": GONE_PATH, "branch": "feat/x", "made": true, "madeFor": "t_root", "settled": { "at": 5, "why": "merged" }, "gone": true })
    );
    assert_eq!(
        worktree_of(&fresh),
        json!({ "path": "/nonexistent/other", "branch": "feat/x", "made": true, "settled": { "at": START, "why": "deleted" }, "gone": true })
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_settled", "ws_fresh"]);
    assert_eq!(rig.base.args_of("drop"), Vec::<Value>::new());
}

#[test]
fn worktree_gone_while_starting_keeps_the_record() {
    let rig = Rig::new();
    let entry = rig.tree("ws_tree", GONE_PATH, json!({}));
    run(rig.folders.worktree_gone(&entry, WorktreeSettledWhy::Removed, true)).unwrap();
    assert_eq!(
        worktree_of(&entry),
        json!({ "path": GONE_PATH, "branch": "feat/x", "made": true, "settled": { "at": START, "why": "removed" }, "gone": true })
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_tree"]);
    assert_eq!(rig.base.args_of("drop"), Vec::<Value>::new());
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 7: runs_in

// Ports folder-threads.test.ts:637 and :1038: a gone tree's threads go on in the folder the record answers.
#[test]
fn runs_in_moves_a_gone_trees_threads_to_the_folder() {
    let rig = Rig::new();
    let folder = "/home/me/web";
    let gone = rig.tree("ws_gone", "/wt/a", json!({ "gone": true }));
    let standing = rig.tree("ws_standing", "/wt/a", json!({}));
    let bare = rig.base.hold(record("ws_bare", "local", "p_web", json!({})));
    assert_eq!(rig.folders.runs_in(&gone, None, folder), folder);
    assert_eq!(rig.folders.runs_in(&gone, Some("/wt/a"), folder), folder);
    assert_eq!(rig.folders.runs_in(&gone, Some("/wt/a/sub"), folder), folder);
    assert_eq!(rig.folders.runs_in(&gone, Some("/wt/ab"), folder), "/wt/ab");
    assert_eq!(rig.folders.runs_in(&gone, Some("/elsewhere"), folder), "/elsewhere");
    assert_eq!(rig.folders.runs_in(&standing, Some("/wt/a"), folder), "/wt/a");
    assert_eq!(rig.folders.runs_in(&standing, None, folder), folder);
    assert_eq!(rig.folders.runs_in(&bare, Some("/x"), folder), "/x");
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 8 and 9: arm_sweep

#[test]
fn sweep_has_work_counts_only_made_standing_worktrees() {
    let rig = Rig::new();
    assert!(!rig.folders.sweep_has_work());
    rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    rig.tree("ws_theirs", GONE_PATH, json!({ "made": false }));
    rig.tree("ws_gone", GONE_PATH, json!({ "gone": true }));
    assert!(!rig.folders.sweep_has_work());
    rig.tree("ws_made", GONE_PATH, json!({}));
    assert!(rig.folders.sweep_has_work());
}

// Ports worktree-sweep.test.ts:182: no timer while no worktree wsp made stands, one when the first is.
#[test]
fn arm_sweep_holds_no_timer_without_a_made_worktree() {
    let rig = Rig::new();
    rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    rig.tree("ws_theirs", GONE_PATH, json!({ "made": false }));
    rig.tree("ws_gone", GONE_PATH, json!({ "gone": true }));
    rig.folders.arm_sweep();
    base::assert_due(&rig.base.clock, &[]);
    assert!(rig.fake.sweep.borrow().timer.is_none());
    rig.tree("ws_made", GONE_PATH, json!({}));
    rig.folders.arm_sweep();
    rig.folders.arm_sweep();
    base::assert_due(&rig.base.clock, &[START + SWEEP_MS]);
    assert!(rig.fake.sweep.borrow().timer.is_some());
}

#[test]
fn arm_sweep_arms_one_timer_and_re_arms_after_the_sweep() {
    let rig = Rig::new();
    rig.base.hold_project(project("p_local", "local", "here", json!({ "remote": "", "git": { "top": "/home/me/local" } })));
    let (_dir, path) = on_disk();
    rig.base.hold(record("ws_tree", "local", "p_local", json!({ "worktree": { "path": path, "branch": "feat/x", "made": true } })));
    rig.folders.arm_sweep();
    run(async {
        rig.base.clock.advance(SWEEP_MS - 1);
        assert!(rig.fake.sweep.borrow().sweeping.is_none());
        base::assert_due(&rig.base.clock, &[START + SWEEP_MS]);
        rig.base.clock.advance(1);
        // The timer's slot is cleared and the sweep is in flight, with no second timer yet.
        assert!(rig.fake.sweep.borrow().timer.is_none());
        let sweeping = rig.fake.sweep.borrow().sweeping.clone().expect("the sweep is in flight");
        base::assert_due(&rig.base.clock, &[]);
        sweeping.await;
        settle().await;
    });
    assert!(rig.fake.sweep.borrow().sweeping.is_none());
    assert!(rig.fake.sweep.borrow().timer.is_some());
    base::assert_due(&rig.base.clock, &[START + 2 * SWEEP_MS]);
    // The tree has no remote: the sweep asked nothing of git or the host.
    assert_eq!(rig.effects(), Vec::<String>::new());
}

#[test]
fn a_stopped_sweep_arms_nothing() {
    let rig = Rig::new();
    rig.tree("ws_made", GONE_PATH, json!({}));
    rig.fake.sweep.borrow_mut().stopped = true;
    rig.folders.arm_sweep();
    base::assert_due(&rig.base.clock, &[]);
    rig.fake.sweep.borrow_mut().stopped = false;
    rig.fake.sweep.borrow_mut().sweeping = Some(async {}.boxed_local().shared());
    rig.folders.arm_sweep();
    base::assert_due(&rig.base.clock, &[]);
    rig.fake.sweep.borrow_mut().sweeping = None;
    let cancel: wsp_core::clock::Cancel = Rc::new(|| {});
    rig.fake.sweep.borrow_mut().timer = Some(cancel);
    rig.folders.arm_sweep();
    base::assert_due(&rig.base.clock, &[]);
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 10 and 11: sweep_worktrees, sweep_one

#[test]
fn one_failed_tree_does_not_stop_the_sweep() {
    let rig = Rig::new();
    rig.tree("ws_a", "/gone/a", json!({}));
    rig.tree("ws_b", "/gone/b", json!({}));
    rig.base.answer_with("drop", vec![err_answer("disk full", None), json!(null)]);
    let warned = wsp_core::console::capture();
    run(rig.folders.sweep_worktrees()).unwrap();
    assert_eq!(warned.lines(), vec!["the worktree at /gone/a was not swept: disk full"]);
    assert_eq!(rig.base.args_of("drop"), vec![json!({ "id": "ws_a" }), json!({ "id": "ws_b" })]);
}

// Ports worktree-sweep.test.ts:313: a worktree the person made is never touched, nor one already gone.
#[test]
fn the_sweep_never_touches_a_worktree_the_person_made_or_one_already_gone() {
    let rig = Rig::new();
    rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    let (_dir, path) = on_disk();
    rig.tree("ws_theirs", &path, json!({ "made": false, "settled": { "at": 1, "why": "merged" } }));
    rig.tree("ws_gone", &path, json!({ "gone": true, "settled": { "at": 1, "why": "merged" } }));
    rig.base.clock.advance(8 * 24 * 3_600_000);
    run(rig.folders.sweep_worktrees()).unwrap();
    assert_eq!(rig.base.names(), Vec::<String>::new());
}

#[test]
fn sweep_one_marks_a_missing_path_removed() {
    let rig = Rig::new();
    let entry = rig.tree("ws_tree", GONE_PATH, json!({}));
    rig.thread_on("ws_tree");
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(
        worktree_of(&entry),
        json!({ "path": GONE_PATH, "branch": "feat/x", "made": true, "settled": { "at": START, "why": "removed" }, "gone": true })
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_tree"]);
    assert_eq!(rig.effects(), vec!["persist"]);
}

#[test]
fn sweep_one_leaves_a_tree_settled_under_six_hours() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - (SETTLED_STANDS_MS - 1), "why": "merged" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled }));
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(rig.effects(), Vec::<String>::new());
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled }));
}

// Ports worktree-sweep.test.ts:221: six hours to the millisecond is old enough; a running turn keeps the tree.
#[test]
fn sweep_one_keeps_a_tree_a_turn_runs_in() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - SETTLED_STANDS_MS, "why": "merged" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled }));
    rig.fake.running.borrow_mut().insert("ws_tree".to_owned());
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled, "kept": RUNNING }));
    assert_eq!(rig.persisted_trees(), vec![worktree_of(&entry)]);
    assert_eq!(rig.base.args_of("status_now"), vec![json!({ "id": "ws_tree" })]);
    assert_eq!(rig.effects(), vec!["persist", "status_now"]);
}

// Ports worktree-sweep.test.ts:208: a settled tree with files no commit holds stays, and says how many.
#[test]
fn sweep_one_keeps_a_tree_with_changes() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - SETTLED_STANDS_MS, "why": "closed" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled }));
    rig.git_says("status --porcelain", "?? a\n M b\n?? c\n");
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(
        worktree_of(&entry),
        json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled, "kept": "3 files not committed" })
    );
    assert_eq!(rig.base.args_of("git"), vec![status_ask(&path)]);
    assert_eq!(rig.base.args_of("copier_worktree_remove"), Vec::<Value>::new());
    assert_eq!(rig.effects(), vec!["git", "persist", "status_now"]);
    rig.git_says("status --porcelain", "?? a\n");
    let one = rig.tree("ws_one", &path, json!({ "settled": settled }));
    run(rig.folders.sweep_one(&one)).unwrap();
    assert_eq!(worktree_of(&one)["kept"], json!("1 file not committed"));
}

// Ports worktree-sweep.test.ts:193: a settled clean tree goes, and its record stays gone while a thread names it.
#[test]
fn sweep_one_removes_a_settled_clean_tree() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - SETTLED_STANDS_MS, "why": "merged" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled }));
    rig.thread_on("ws_tree");
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(rig.base.args_of("git"), vec![status_ask(&path), status_ask(&path)]);
    assert_eq!(rig.base.args_of("copier_worktree_remove"), vec![remove_ask(&path, false)]);
    assert_eq!(rig.base.names_of(&["git", "copier_worktree_remove", "persist"]), vec!["git", "git", "copier_worktree_remove", "persist"]);
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled, "gone": true }));
}

// Ports worktree-sweep.test.ts:233: git's refusal is kept on the record, in its last line.
#[test]
fn sweep_one_marks_a_failed_removal() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - SETTLED_STANDS_MS, "why": "merged" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled }));
    let locked = format!("fatal: '{path}' is locked; use 'unlock' to override");
    rig.base.answer_with("copier_worktree_remove", vec![err_answer(&locked, None)]);
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled, "removeFailed": locked }));
    assert_eq!(rig.persisted_trees(), vec![worktree_of(&entry)]);
    assert_eq!(
        rig.base.names_of(&["copier_worktree_remove", "persist", "status_now", "drop"]),
        vec!["copier_worktree_remove", "persist", "status_now"]
    );
}

// Ports worktree-sweep.test.ts:261: a week of failing leaves the tree to the person, and the sweep stops trying.
#[test]
fn sweep_one_abandons_after_a_week_and_then_leaves_it() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - KEPT_FOR_MS, "why": "merged" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled }));
    rig.base.answer_with("copier_worktree_remove", vec![err_answer("hint: use unlock\nfatal: locked\n", None)]);
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(
        worktree_of(&entry),
        json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled, "removeFailed": "fatal: locked", "kept": ABANDONED })
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_tree"]);
    let calls = rig.calls_made();
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(rig.calls_made(), calls);
    assert_eq!(rig.base.args_of("copier_worktree_remove").len(), 1);
}

// Decision 1: the trees are compared by their JSON text, and a write that changes nothing persists nothing.
#[test]
fn sweep_one_writes_nothing_when_the_tree_did_not_change() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let settled = json!({ "at": START - SETTLED_STANDS_MS, "why": "merged" });
    let entry = rig.tree("ws_tree", &path, json!({ "settled": settled, "kept": RUNNING }));
    rig.fake.running.borrow_mut().insert("ws_tree".to_owned());
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(rig.persisted_ids(), Vec::<String>::new());
    assert_eq!(rig.base.args_of("status_now"), Vec::<Value>::new());
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true, "settled": settled, "kept": RUNNING }));
}

// Ports worktree-sweep.test.ts:329: a tree no thread ever ran in has its pull request read, forced.
#[test]
fn sweep_one_reads_the_pull_request_of_a_tree_no_thread_ran_in() {
    let rig = Rig::new();
    let (_dir, path) = on_disk();
    let entry = rig.tree("ws_tree", &path, json!({}));
    run(rig.folders.sweep_one(&entry)).unwrap();
    assert_eq!(rig.base.args_of("read_pull_request"), vec![json!({ "id": "ws_tree", "force": true })]);
    assert_eq!(rig.effects(), vec!["read_pull_request", "git", "git"]);
    assert_eq!(worktree_of(&entry), json!({ "path": path, "branch": "feat/x", "made": true }));
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 12 and 13: settled_now, branch_gone_at_remote

const TREE_AT: &str = "/home/me/.wsp/worktrees/p_web/feat-x";

fn pr(state: &str, over: Value) -> Value {
    let mut v = json!({ "number": 7, "url": "https://github.com/acme/web/pull/7", "state": state, "base": "main" });
    base::merge(&mut v, over);
    v
}

fn config_ask(path: &str, key: &str) -> Value {
    json!({ "cwd": path, "args": ["config", "--get", format!("branch.feat/x.{key}")], "timeoutMs": null, "input": null })
}

// Ports worktree-sweep.test.ts:193 and :208: a merged or closed pull request settles the tree, at its own time.
#[test]
fn settled_now_stamps_merged_and_closed_from_the_pr() {
    let rig = Rig::new();
    let merged = rig.tree("ws_merged", TREE_AT, json!({}));
    let closed = rig.tree("ws_closed", TREE_AT, json!({}));
    let open = rig.tree("ws_open", TREE_AT, json!({}));
    rig.base.answer_with(
        "read_pull_request",
        vec![pr("merged", json!({ "mergedAt": 123456 })), pr("closed", json!({})), pr("open", json!({}))],
    );
    let got = run(rig.folders.settled_now(&merged)).unwrap();
    assert_eq!(serde_json::to_value(got).unwrap(), json!({ "at": 123456, "why": "merged" }));
    assert_eq!(
        worktree_of(&merged),
        json!({ "path": TREE_AT, "branch": "feat/x", "made": true, "settled": { "at": 123456, "why": "merged" } })
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_merged"]);
    assert_eq!(rig.base.names_of(&["read_pull_request", "persist", "status_now"]), vec!["read_pull_request", "persist", "status_now"]);
    let got = run(rig.folders.settled_now(&closed)).unwrap();
    assert_eq!(serde_json::to_value(got).unwrap(), json!({ "at": START, "why": "closed" }));
    assert_eq!(worktree_of(&closed)["settled"], json!({ "at": START, "why": "closed" }));
    // An open pull request settles nothing, and no upstream asks the remote nothing.
    assert!(run(rig.folders.settled_now(&open)).unwrap().is_none());
    assert_eq!(worktree_of(&open), json!({ "path": TREE_AT, "branch": "feat/x", "made": true }));
    assert_eq!(
        rig.base.args_of("read_pull_request"),
        vec![
            json!({ "id": "ws_merged", "force": true }),
            json!({ "id": "ws_closed", "force": true }),
            json!({ "id": "ws_open", "force": true })
        ]
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_merged", "ws_closed"]);
    assert_eq!(rig.base.args_of("local_exec"), Vec::<Value>::new());
}

// Ports worktree-sweep.test.ts:277: a pushed branch the remote no longer has is deleted, stamped now.
#[test]
fn settled_now_stamps_deleted_when_the_remote_says_no_branch() {
    let rig = Rig::new();
    let entry = rig.tree("ws_tree", TREE_AT, json!({}));
    rig.git_says("config --get branch.feat/x.remote", "origin\n");
    rig.git_says("config --get branch.feat/x.merge", "refs/heads/feat/x\n");
    rig.reply("local_exec", json!({ "exitCode": 2, "stdout": "", "stderr": "" }));
    let got = run(rig.folders.settled_now(&entry)).unwrap();
    assert_eq!(serde_json::to_value(got).unwrap(), json!({ "at": START, "why": "deleted" }));
    assert_eq!(
        worktree_of(&entry),
        json!({ "path": TREE_AT, "branch": "feat/x", "made": true, "settled": { "at": START, "why": "deleted" } })
    );
    assert_eq!(rig.base.args_of("git"), vec![config_ask(TREE_AT, "remote"), config_ask(TREE_AT, "merge")]);
    assert_eq!(
        rig.base.args_of("local_exec"),
        vec![json!({
            "cmd": format!("GIT_TERMINAL_PROMPT=0 git -C {TREE_AT} ls-remote --exit-code --heads origin refs/heads/feat/x"),
            "timeoutMs": 20000, "stdin": null,
        })]
    );
    assert_eq!(rig.persisted_ids(), vec!["ws_tree"]);
    assert_eq!(rig.base.args_of("status_now"), vec![json!({ "id": "ws_tree" })]);
}

#[test]
fn branch_gone_at_remote_reads_exit_2_alone() {
    let rig = Rig::new();
    rig.git_says("config --get branch.feat/x.remote", "origin\n");
    rig.git_says("config --get branch.feat/x.merge", "refs/heads/feat/x\n");
    rig.base.answer_with(
        "local_exec",
        vec![
            json!({ "exitCode": 0, "stdout": "abc\trefs/heads/feat/x\n", "stderr": "" }),
            json!({ "exitCode": 1, "stdout": "", "stderr": "" }),
            json!({ "exitCode": 128, "stdout": "", "stderr": "fatal: unable to access" }),
            json!({ "exitCode": 2, "stdout": "", "stderr": "" }),
        ],
    );
    let mut said = vec![];
    for _ in 0..4 {
        said.push(run(rig.folders.branch_gone_at_remote(TREE_AT, "feat/x")).unwrap());
    }
    assert_eq!(said, vec![false, false, false, true]);
    let cmd = format!("GIT_TERMINAL_PROMPT=0 git -C {TREE_AT} ls-remote --exit-code --heads origin refs/heads/feat/x");
    assert_eq!(rig.base.args_of("local_exec"), vec![json!({ "cmd": cmd, "timeoutMs": 20000, "stdin": null }); 4]);
}

// Ports worktree-sweep.test.ts:289: a branch with no upstream is never gone, and the remote is never asked.
#[test]
fn a_branch_with_no_upstream_is_never_gone() {
    let rig = Rig::new();
    for (remote, merge) in [("", "refs/heads/feat/x\n"), ("origin\n", ""), ("origin\n", " \n"), (".\n", "refs/heads/feat/x\n")] {
        rig.git_says("config --get branch.feat/x.remote", remote);
        rig.git_says("config --get branch.feat/x.merge", merge);
        assert!(!run(rig.folders.branch_gone_at_remote(TREE_AT, "feat/x")).unwrap());
    }
    assert_eq!(rig.base.args_of("local_exec"), Vec::<Value>::new());
    assert_eq!(rig.base.args_of("git").len(), 8);
}

// Ports worktree-sweep.test.ts:301: a repo with no remote settles nothing and asks no host about it.
#[test]
fn settled_now_asks_nothing_with_no_remote() {
    let rig = Rig::new();
    rig.base.hold_project(project("p_local", "local", "here", json!({ "remote": "", "git": { "top": "/home/me/local" } })));
    let entry =
        rig.base.hold(record("ws_tree", "local", "p_local", json!({ "worktree": { "path": TREE_AT, "branch": "feat/x", "made": true } })));
    assert!(run(rig.folders.settled_now(&entry)).unwrap().is_none());
    assert_eq!(rig.base.names(), Vec::<String>::new());
    assert_eq!(rig.persisted_ids(), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 14 and 15: git_said, worktrees_of

#[test]
fn git_said_takes_the_first_error_line_else_the_last_line() {
    let said = |stdout: &str, stderr: &str| git_said(&exec(1, stdout, stderr));
    assert_eq!(said("", "hint: a\nerror: pathspec 'a' did not match\nfatal: other\n"), "error: pathspec 'a' did not match");
    assert_eq!(said("", "warning: x\nfatal: bad object   \nerror: later"), "fatal: bad object");
    assert_eq!(said("error: on stdout only", "hint: nothing\n"), "hint: nothing");
    assert_eq!(said("", "warning: a\nlast line here\n"), "last line here");
    assert_eq!(said("one\ntwo\n", ""), "two");
    assert_eq!(said("one\ntwo\n", "  \n"), "two");
    assert_eq!(said("", "a\n  b  \n"), "  b");
    assert_eq!(said("", "  fatal: indented\nerror:no space\nplain"), "plain");
    assert_eq!(said("", ""), "");
}

#[test]
fn worktrees_of_parses_porcelain_and_reads_none_on_failure() {
    let rig = Rig::new();
    let porcelain = "branch refs/heads/stray\nworktree /home/me/web\nHEAD abc\nbranch refs/heads/main\n\nworktree /tmp/detached\nHEAD def\ndetached\n\nworktree /tmp/by hand\nbranch refs/heads/feat/x\n";
    let want = vec![
        ListedWorktree { path: "/home/me/web".to_owned(), branch: Some("main".to_owned()) },
        ListedWorktree { path: "/tmp/detached".to_owned(), branch: None },
        ListedWorktree { path: "/tmp/by hand".to_owned(), branch: Some("feat/x".to_owned()) },
    ];
    assert_eq!(parse_worktrees(porcelain), want);
    assert_eq!(parse_worktrees(""), Vec::<ListedWorktree>::new());
    rig.git_says("worktree list --porcelain", porcelain);
    assert_eq!(run(rig.folders.worktrees_of("/home/me/web")).unwrap(), want);
    rig.reply("git:worktree list --porcelain", json!({ "exitCode": 128, "stdout": porcelain, "stderr": "fatal: not a git repository" }));
    assert_eq!(run(rig.folders.worktrees_of("/home/me/web")).unwrap(), Vec::<ListedWorktree>::new());
    let ask = json!({ "cwd": "/home/me/web", "args": ["worktree", "list", "--porcelain"], "timeoutMs": null, "input": null });
    assert_eq!(rig.base.args_of("git"), vec![ask.clone(), ask]);
}
