// SPDX-License-Identifier: AGPL-3.0-only
//! Where a thread on this computer, or on a box the person joined, runs: the project folder's record, a worktree's
//! record and `folderFor`'s reading of the start's words. Ported at the area's seam from folder-threads.test.ts,
//! box-folder.test.ts and the folder cases of local-workspace.test.ts at 65f6b89c7, plus the cases spec/folders_place.md
//! pins beyond them. Every sentence is the TS source's own text (protocol/src/projects.ts, words/units.ts,
//! words/computer.ts, words/thread.ts, types/wiring.ts).
//!
//! The record's JSON is compared as a value: serde_json keeps the struct's key order (`WorkspaceRecord` and
//! `WorktreeFolder` do not follow the TS literals' order), so key order is not asserted here.
#[path = "support/base.rs"]
mod base;
#[path = "support/folders.rs"]
mod folders;

use std::rc::Rc;

use base::{err_answer, kinded, plain, project, rec, rec_json, record, run, view_of, FakeBase};
use folders::{preferences, FakeFolders, FakeHere};
use serde_json::{json, Value};
use wsp_account::projects::folders::place::{carry_modules, under, FolderAsk};
use wsp_account::projects::folders::{folders_area, Folders};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{WorktreeFolder, WorktreeReport};
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::exit::{HostError, Kind};

const CREATED_AT: &str = "1970-01-01T00:16:40.000Z";
const STATE: &str = "/home/me/.wsp";
const NO_COPIER_HERE: &str = "this host has no daemon binary beside it to make a worktree with; stage this wsp's daemon binary and try again";
const SETUP_HEAD: &str = "Before this thread started, its new worktree ran:";

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
        Rig { base, fake, folders }
    }
    fn hold(&self, p: ProjectView) -> ProjectView {
        self.base.hold_project(p.clone());
        p
    }
    fn after_worktree(&self, command: &str) {
        *self.base.prefs.borrow_mut() = Some(preferences(json!({ "projectDefaults": { "p_web": { "afterWorktree": command } } })));
    }
    fn script_worktrees(&self, stdout: &str) {
        self.base.answer_with("git:worktree list --porcelain", vec![json!({ "exitCode": 0, "stdout": stdout, "stderr": "" })]);
    }
    /// Nothing was written, emitted or copied.
    fn untouched(&self) {
        assert_eq!(self.base.persisted.borrow().len(), 0);
        assert_eq!(self.base.events.borrow().clone(), Vec::<Value>::new());
        assert_eq!(self.base.args_of("copier_worktree"), Vec::<Value>::new());
        assert_eq!(self.base.names_of(&["attach", "persist", "emit", "write_daemon_roots", "local_exec"]), Vec::<String>::new());
    }
}

fn web() -> ProjectView {
    project("p_web", "web", "here", json!({ "git": { "top": "/home/me/web" } }))
}

fn boxed() -> ProjectView {
    project("p_box", "box", "pl_spoo", json!({ "path": "/root/box", "git": { "top": "/root/box" } }))
}

use base::thread;

fn tree(v: Value) -> WorktreeFolder {
    serde_json::from_value(v).expect("a test worktree reads")
}

fn report(v: Value) -> WorktreeReport {
    let mut base = json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true, "carried": [], "fresh": true, "modules": [], "ms": 1 });
    base::merge(&mut base, v);
    serde_json::from_value(base).expect("a test report reads")
}

fn refusal<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected a refusal"),
    }
}

/// The record `recordFolder` writes for a local folder, with `over` on top.
fn folder_record(id: &str, name: &str, over: Value) -> Value {
    let mut v = json!({
        "id": id, "name": name, "kind": "local", "machineId": "local", "phase": "running", "golden": "", "createdAt": CREATED_AT,
        "project": "p_web", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false, "idleWindowMs": null,
    });
    base::merge(&mut v, over);
    v
}

fn created(rig: &Rig, entry: &Entry) -> Value {
    json!({ "type": "workspace.created", "workspace": serde_json::to_value(view_of(&rig.base, &rec(entry))).unwrap() })
}

fn worktree_of(entry: &Entry) -> Value {
    rec_json(entry).get("worktree").cloned().unwrap_or(Value::Null)
}

fn persisted_json(rig: &Rig) -> Value {
    serde_json::to_value(&*rig.base.persisted.borrow()).unwrap()
}

fn module_json() -> Value {
    serde_json::to_value(carry_modules()).unwrap()
}

fn copier_ask(branch: &str, checkpoint: Value) -> Value {
    json!({ "from": "/home/me/web", "home": STATE, "project": "p_web", "branch": branch, "modules": module_json(), "checkpoint": checkpoint })
}

fn on_disk() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 1: record_folder

// Ports folder-threads.test.ts:164 (the record a start lands on) and spec rule 1.
#[test]
fn record_folder_writes_the_record_in_the_ts_key_order() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let entry = run(rig.folders.record_folder(&p, None, None)).unwrap();
    let want = folder_record("ws_11111111", "web", json!({}));
    assert_eq!(rec_json(&entry), want);
    assert_eq!(persisted_json(&rig), json!([want]));
    assert_eq!(rig.base.events.borrow().clone(), vec![created(&rig, &entry)]);
    assert!(Rc::ptr_eq(&rig.base.live.borrow()["ws_11111111"], &entry));
    assert_eq!(rig.base.args_of("admit_folder"), vec![json!({ "kind": "local", "name": "web", "place": null })]);
    assert_eq!(rig.base.args_of("folder_machine"), vec![json!({ "kind": "local", "place": null })]);
    assert_eq!(rig.base.clock.pending(), 0);
}

#[test]
fn record_folder_writes_the_daemon_roots_before_it_emits_created() {
    let rig = Rig::new();
    let p = rig.hold(web());
    run(rig.folders.record_folder(&p, None, None)).unwrap();
    assert_eq!(
        rig.base.names_of(&["admit_folder", "folder_machine", "attach", "persist", "write_daemon_roots", "emit"]),
        vec!["admit_folder", "folder_machine", "attach", "persist", "write_daemon_roots", "emit"]
    );
    assert_eq!(rig.base.args_of("write_daemon_roots"), vec![json!({ "id": "ws_11111111", "strict": false })]);
}

#[test]
fn record_folder_names_a_worktree_project_at_branch_and_takes_a_free_name() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.base.hold(record("ws_old", "local", "p_web", json!({ "name": "web@feat/x" })));
    let worktree = tree(json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true }));
    let first = run(rig.folders.record_folder(&p, Some(worktree.clone()), Some("ws_parent".to_owned()))).unwrap();
    assert_eq!(
        rec_json(&first),
        folder_record(
            "ws_11111111",
            "web@feat/x 2",
            json!({ "worktree": { "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true }, "parentWorkspaceId": "ws_parent" })
        )
    );
    let headless = tree(json!({ "path": "/tmp/detached", "made": false }));
    let second = run(rig.folders.record_folder(&p, Some(headless), None)).unwrap();
    assert_eq!(rec_json(&second), folder_record("ws_22222222", "web", json!({ "worktree": { "path": "/tmp/detached", "made": false } })));
    let third = run(rig.folders.record_folder(&p, None, None)).unwrap();
    assert_eq!(rec_json(&third)["name"], json!("web 2"));
    assert_eq!(rig.base.persisted.borrow().iter().map(|r| r.name.clone()).collect::<Vec<_>>(), vec!["web@feat/x 2", "web", "web 2"]);
}

#[test]
fn record_folder_arms_the_sweep_only_for_a_made_worktree() {
    let rig = Rig::new();
    let p = rig.hold(web());
    run(rig.folders.record_folder(&p, None, None)).unwrap();
    run(rig.folders.record_folder(&p, Some(tree(json!({ "path": "/tmp/theirs", "branch": "theirs", "made": false }))), None)).unwrap();
    base::assert_due(&rig.base.clock, &[]);
    run(rig.folders.record_folder(&p, Some(tree(json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true }))), None)).unwrap();
    base::assert_due(&rig.base.clock, &[1_600_000]);
    assert!(rig.fake.sweep.borrow().timer.is_some());
}

// Ports box-folder.test.ts:42: a folder on a computer the person joined is a place's record.
#[test]
fn record_folder_on_a_joined_box_names_the_place_and_its_machine() {
    let rig = Rig::new();
    let p = rig.hold(boxed());
    let entry = run(rig.folders.record_folder(&p, None, None)).unwrap();
    let want = folder_record("ws_11111111", "box", json!({ "kind": "place", "place": "pl_spoo", "machineId": "pl_spoo", "project": "p_box" }));
    assert_eq!(rec_json(&entry), want);
    assert_eq!(persisted_json(&rig), json!([want]));
    assert_eq!(rig.base.args_of("admit_folder"), vec![json!({ "kind": "place", "name": "box", "place": "pl_spoo" })]);
    assert_eq!(rig.base.args_of("folder_machine"), vec![json!({ "kind": "place", "place": "pl_spoo" })]);
    assert_eq!(rig.base.events.borrow().clone(), vec![created(&rig, &entry)]);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 2: project_folder

#[test]
fn project_folder_answers_the_held_record() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.base.hold(record("ws_tree", "local", "p_web", json!({ "worktree": { "path": "/tmp/t", "made": false } })));
    let folder = rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    rig.base.hold(record("ws_other", "local", "p_other", json!({})));
    let got = run(rig.folders.project_folder(&p)).unwrap();
    assert!(Rc::ptr_eq(&got, &folder));
    assert_eq!(rig.base.args_of("one_folder"), Vec::<Value>::new());
    rig.untouched();
}

// Ports folder-threads.test.ts:164 and :199: two first threads at once, one record.
#[test]
fn two_first_threads_make_one_project_folder() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let (a, b) = run(async { futures::join!(rig.folders.project_folder(&p), rig.folders.project_folder(&p)) });
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(Rc::ptr_eq(&a, &b));
    assert_eq!(rec(&a).id, "ws_11111111");
    assert_eq!(rig.base.live.borrow().keys().cloned().collect::<Vec<_>>(), vec!["ws_11111111"]);
    assert_eq!(persisted_json(&rig), json!([folder_record("ws_11111111", "web", json!({}))]));
    assert_eq!(rig.base.events.borrow().clone(), vec![created(&rig, &a)]);
    assert_eq!(rig.base.args_of("one_folder"), vec![json!({ "key": "p_web\u{0}" }), json!({ "key": "p_web\u{0}" })]);
    assert_eq!(rig.base.args_of("admit_folder").len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------
// Rules 3, 4, 5: worktree_folder, CARRY_MODULES, the setup line

#[test]
fn worktree_folder_refuses_with_no_copier() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.fake.copier.set(false);
    let e = refusal(run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/x", None, None, None)));
    assert_eq!(e, kinded(NO_COPIER_HERE, Kind::Invalid));
    assert_eq!(rig.base.args_of("one_folder"), vec![json!({ "key": "p_web\u{0}feat/x" })]);
    assert_eq!(rig.base.args_of("here_daemon"), Vec::<Value>::new());
    rig.untouched();
}

// Ports folder-threads.test.ts:461: no daemon binary, and the project folder still takes threads.
#[test]
fn place_for_a_branch_with_no_daemon_binary_is_refused_and_the_project_folder_still_takes_threads() {
    let rig = Rig::new();
    rig.hold(web());
    rig.fake.copier.set(false);
    let ask = FolderAsk { project: Some("web".into()), branch: Some("feat/x".into()), ..Default::default() };
    assert_eq!(refusal(run(rig.folders.place_for(&ask, None))), kinded(NO_COPIER_HERE, Kind::Invalid));
    rig.untouched();
    let placed = run(rig.folders.place_for(&FolderAsk { project: Some("web".into()), ..Default::default() }, None)).unwrap();
    assert_eq!(rec_json(&placed.entry), folder_record("ws_11111111", "web", json!({})));
    assert!(placed.entry.borrow().record.borrow().worktree.is_none());
}

// Ports folder-threads.test.ts:452: a daemon behind is refused in one sentence and the binary is never run.
#[test]
fn worktree_folder_refuses_a_daemon_behind_before_the_copier_runs() {
    let rig = Rig::new();
    let p = rig.hold(web());
    *rig.fake.here.borrow_mut() = Some(Rc::new(FakeHere { version: (DAEMON_VERSION - 1) as u32, fix: "npm i -g @wsp-labs/wsp".to_owned() }));
    let e = refusal(run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/x", None, None, None)));
    let said = format!("this computer's wsp daemon is version {} and this wsp needs {}; npm i -g @wsp-labs/wsp stages the right one", DAEMON_VERSION - 1, DAEMON_VERSION);
    assert_eq!(e, plain(&said));
    assert_eq!(rig.base.names_of(&["here_daemon", "copier_worktree"]), vec!["here_daemon"]);
    rig.untouched();
}

// Ports folder-threads.test.ts:468: the branch the folder holds is the project folder, no copy made.
#[test]
fn worktree_folder_at_the_top_is_the_project_folder() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.base.answer_with("copier_worktree", vec![json!({ "path": "/home/me/web", "branch": "main", "made": false, "carried": [], "fresh": false, "modules": [], "ms": 1 })]);
    let got = run(rig.folders.worktree_folder(&p, "/home/me/web", "main", None, None, None)).unwrap();
    assert_eq!(rec_json(&got), folder_record("ws_11111111", "web", json!({})));
    assert_eq!(persisted_json(&rig), json!([folder_record("ws_11111111", "web", json!({}))]));
    assert_eq!(rig.base.args_of("one_folder"), vec![json!({ "key": "p_web\u{0}main" }), json!({ "key": "p_web\u{0}" })]);
    assert_eq!(rig.base.args_of("local_exec"), Vec::<Value>::new());
}

// Ports folder-threads.test.ts:427: the copier is asked from the repo's top under the host's folder, every ecosystem.
#[test]
fn worktree_folder_asks_the_copier_for_the_branch_under_the_hosts_folder() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let got = run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/login", None, None, Some("cp1".to_owned()))).unwrap();
    assert_eq!(rig.base.args_of("copier_worktree"), vec![copier_ask("feat/login", json!("cp1"))]);
    assert_eq!(worktree_of(&got), json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-login", "branch": "feat/login", "made": true }));
    assert_eq!(rec(&got).name, "web@feat/login");
}

#[test]
fn carry_modules_lists_the_nine_ecosystems_as_the_verb_is_handed_them() {
    // Ports folder-threads.test.ts:427: the ask's modules are the catalogue's, less their rebuild commands.
    let node = [".next", ".turbo", ".cache"];
    assert_eq!(
        serde_json::to_value(carry_modules()).unwrap(),
        json!([
            { "id": "npm", "lockfiles": ["package-lock.json", "npm-shrinkwrap.json"], "carry": ["node_modules"], "never": node },
            { "id": "pnpm", "lockfiles": ["pnpm-lock.yaml"], "carry": ["node_modules"], "never": node, "installed": "node_modules/.pnpm/lock.yaml" },
            { "id": "yarn", "lockfiles": ["yarn.lock"], "carry": ["node_modules", ".yarn/cache", ".yarn/unplugged"], "never": node },
            { "id": "bun", "lockfiles": ["bun.lock", "bun.lockb"], "carry": ["node_modules"], "never": node },
            { "id": "composer", "lockfiles": ["composer.lock"], "carry": ["vendor"], "never": [] },
            { "id": "uv", "lockfiles": ["uv.lock"], "carry": [], "never": [".venv"] },
            { "id": "poetry", "lockfiles": ["poetry.lock"], "carry": [], "never": [".venv"] },
            { "id": "cargo", "lockfiles": ["Cargo.lock"], "carry": [], "never": ["target"] },
            { "id": "go", "lockfiles": ["go.sum"], "carry": [], "never": [] },
        ])
    );
}

// Ports folder-threads.test.ts:374 and :389: the line of a fresh worktree opens the first start that names it.
#[test]
fn worktree_folder_keeps_the_setup_line_of_a_fresh_worktree_for_one_start() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.after_worktree("make setup");
    let got = run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/x", Some("ws_parent".to_owned()), Some("t_root".to_owned()), None)).unwrap();
    assert_eq!(
        rec_json(&got),
        folder_record(
            "ws_11111111",
            "web@feat/x",
            json!({ "worktree": { "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true, "madeFor": "t_root" }, "parentWorkspaceId": "ws_parent" })
        )
    );
    assert_eq!(rig.base.args_of("copier_worktree"), vec![copier_ask("feat/x", Value::Null)]);
    assert_eq!(rig.fake_exec_commands(), vec!["cd /home/me/.wsp/worktrees/p_web/feat-x && make setup"]);
    assert_eq!(rig.folders.take_setup_line("ws_11111111"), Some(format!("{SETUP_HEAD}\n- make setup")));
    assert_eq!(rig.folders.take_setup_line("ws_11111111"), None);
}

// Ports folder-threads.test.ts:863 and :1024: a worktree made again comes back to its record and the tree it was made for.
#[test]
fn a_worktree_made_again_comes_back_to_the_record_and_its_tree() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let held = rig.base.hold(record(
        "ws_again",
        "local",
        "p_web",
        json!({ "worktree": { "path": "/home/me/.wsp/worktrees/p_web/feat-again", "branch": "feat/again", "made": true, "madeFor": "t_root", "gone": true, "settled": { "at": 5, "why": "removed" } } }),
    ));
    let got = run(rig.folders.worktree_folder(&p, "/home/me/web", "feat/again", None, Some("t_other".to_owned()), None)).unwrap();
    assert!(Rc::ptr_eq(&got, &held));
    assert_eq!(worktree_of(&got), json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-again", "branch": "feat/again", "made": true, "madeFor": "t_root" }));
    assert_eq!(rig.base.persisted.borrow().iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["ws_again"]);
    assert_eq!(rig.base.events.borrow().clone(), Vec::<Value>::new());
    assert_eq!(rig.base.args_of("admit_folder"), Vec::<Value>::new());
}

#[test]
fn two_starts_of_one_branch_make_one_worktree() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let (a, b) = run(async {
        futures::join!(
            rig.folders.worktree_folder(&p, "/home/me/web", "feat/login", None, None, None),
            rig.folders.worktree_folder(&p, "/home/me/web", "feat/login", None, None, None)
        )
    });
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(Rc::ptr_eq(&a, &b));
    assert_eq!(rig.base.args_of("copier_worktree"), vec![copier_ask("feat/login", Value::Null)]);
    assert_eq!(rig.base.args_of("one_folder"), vec![json!({ "key": "p_web\u{0}feat/login" }), json!({ "key": "p_web\u{0}feat/login" })]);
    assert_eq!(rig.base.persisted.borrow().iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["ws_11111111"]);
    assert_eq!(rig.base.events.borrow().clone(), vec![created(&rig, &a)]);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 6: settle_worktree

impl Rig {
    /// The commands `local_exec` was handed, in order.
    fn fake_exec_commands(&self) -> Vec<String> {
        self.base.args_of("local_exec").iter().map(|a| a["cmd"].as_str().unwrap().to_owned()).collect()
    }
    fn notices(&self) -> Vec<Value> {
        self.base.events.borrow().clone()
    }
}

fn notice(message: &str) -> Value {
    json!({ "type": "host.notice", "message": message })
}

// Ports folder-threads.test.ts:298, :326 and :339: installs in the folder holding the lockfile, then the project's own.
#[test]
fn settle_worktree_runs_installs_then_the_after_worktree_command_and_says_each() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.after_worktree("echo after");
    let at = "/home/me/.wsp/worktrees/p_web/feat-x";
    let made = report(json!({ "modules": [
        { "id": "uv", "folder": ".", "rebuild": true },
        { "id": "npm", "folder": "web", "rebuild": true },
        { "id": "pnpm", "folder": ".", "rebuild": false },
    ] }));
    let line = run(rig.folders.settle_worktree(&p, &made)).unwrap();
    let npm = "npm install --no-save --prefer-offline --no-audit --no-fund";
    assert_eq!(
        rig.fake_exec_commands(),
        vec![format!("cd {at} && uv sync --locked"), format!("cd {at}/web && {npm}"), format!("cd {at} && echo after")]
    );
    assert_eq!(
        rig.base.args_of("local_exec").iter().map(|a| (a["timeoutMs"].clone(), a["stdin"].clone())).collect::<Vec<_>>(),
        vec![(json!(180000), json!(0)), (json!(180000), json!(0)), (json!(180000), json!(0))]
    );
    assert_eq!(
        rig.notices(),
        vec![
            notice("uv sync --locked runs in the new worktree at ~/.wsp/worktrees/p_web/feat-x before its threads start"),
            notice(&format!("{npm} runs in the new worktree at ~/.wsp/worktrees/p_web/feat-x/web before its threads start")),
            notice("echo after runs in the new worktree at ~/.wsp/worktrees/p_web/feat-x before its threads start"),
        ]
    );
    assert_eq!(rig.base.names_of(&["emit", "local_exec"]), vec!["emit", "local_exec", "emit", "local_exec", "emit", "local_exec"]);
    assert_eq!(line, Some(format!("{SETUP_HEAD}\n- uv sync --locked\n- {npm} in web\n- echo after")));
}

// Ports folder-threads.test.ts:349: every command runs whatever the one before did, and each failure is said.
#[test]
fn settle_worktree_says_a_timeout_in_minutes_and_a_failure_with_its_last_line() {
    let rig = Rig::new();
    let p = rig.hold(web());
    rig.after_worktree("make seed");
    let warned = wsp_core::console::capture();
    rig.base.answer_with(
        "local_exec",
        vec![
            json!({ "exitCode": 124, "stdout": "", "stderr": "" }),
            json!({ "exitCode": 1, "stdout": "ignored", "stderr": "warning: x\ngo: updates to go.sum needed\n" }),
            json!({ "exitCode": 3, "stdout": "", "stderr": "" }),
            json!({ "exitCode": 2, "stdout": " built nothing\nfailed at step 3 \n", "stderr": "  \n" }),
        ],
    );
    let made = report(json!({ "modules": [
        { "id": "pnpm", "folder": ".", "rebuild": true },
        { "id": "go", "folder": "svc", "rebuild": true },
        { "id": "uv", "folder": ".", "rebuild": true },
    ] }));
    let line = run(rig.folders.settle_worktree(&p, &made)).unwrap();
    let at = "/home/me/.wsp/worktrees/p_web/feat-x";
    let pnpm = "pnpm install --frozen-lockfile --prefer-offline";
    let go = "go list -mod=readonly -deps -test ./... > /dev/null";
    let shown = "~/.wsp/worktrees/p_web/feat-x";
    assert_eq!(
        rig.fake_exec_commands(),
        vec![format!("cd {at} && {pnpm}"), format!("cd {at}/svc && {go}"), format!("cd {at} && uv sync --locked"), format!("cd {at} && make seed")]
    );
    assert_eq!(
        rig.notices(),
        vec![
            notice(&format!("{pnpm} runs in the new worktree at {shown} before its threads start")),
            notice(&format!("{pnpm} in the new worktree at {shown} was stopped after 3 minutes; threads there start without it")),
            notice(&format!("{go} runs in the new worktree at {shown}/svc before its threads start")),
            notice(&format!("{go} in the new worktree at {shown}/svc exited 1: go: updates to go.sum needed; threads there start without it")),
            notice(&format!("uv sync --locked runs in the new worktree at {shown} before its threads start")),
            notice(&format!("uv sync --locked in the new worktree at {shown} exited 3; threads there start without it")),
            notice(&format!("make seed runs in the new worktree at {shown} before its threads start")),
            notice(&format!("make seed in the new worktree at {shown} exited 2: failed at step 3; threads there start without it")),
        ]
    );
    assert_eq!(
        warned.lines(),
        vec![
            format!("{pnpm} in {at} exited 124: "),
            format!("{go} in {at}/svc exited 1: warning: x\ngo: updates to go.sum needed"),
            format!("uv sync --locked in {at} exited 3: "),
            "make seed in /home/me/.wsp/worktrees/p_web/feat-x exited 2: built nothing\nfailed at step 3".to_owned(),
        ]
    );
    assert_eq!(
        line,
        Some(format!(
            "{SETUP_HEAD}\n- {pnpm} (was stopped after 3 minutes)\n- {go} in svc (exited 1: go: updates to go.sum needed)\n- uv sync --locked (exited 3)\n- make seed (exited 2: failed at step 3)"
        ))
    );
}

// Ports folder-threads.test.ts:307 and :317: a module already installed, or none the catalogue knows, runs nothing.
#[test]
fn settle_worktree_with_nothing_to_run_answers_nothing() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let made = report(json!({ "modules": [
        { "id": "pnpm", "folder": ".", "rebuild": false },
        { "id": "gem", "folder": ".", "rebuild": true },
    ] }));
    assert_eq!(run(rig.folders.settle_worktree(&p, &made)).unwrap(), None);
    assert_eq!(run(rig.folders.settle_worktree(&p, &report(json!({})))).unwrap(), None);
    assert_eq!(rig.base.names_of(&["emit", "local_exec"]), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 7: worktree_mounted

// Ports folder-threads.test.ts:406: a worktree wsp made is mounted again, never the project folder or a person's own.
#[test]
fn worktree_mounted_mounts_only_a_made_standing_worktree() {
    let rig = Rig::new();
    rig.hold(web());
    rig.hold(project("p_flat", "flat", "here", json!({})));
    let dir = on_disk();
    let path = dir.path().to_str().unwrap().to_owned();
    let hold = |id: &str, project: &str, worktree: Value| rig.base.hold(record(id, "local", project, if worktree.is_null() { json!({}) } else { json!({ "worktree": worktree }) }));
    let skipped = vec![
        hold("ws_folder", "p_web", Value::Null),
        hold("ws_theirs", "p_web", json!({ "path": path, "branch": "b", "made": false })),
        hold("ws_gone", "p_web", json!({ "path": path, "branch": "b", "made": true, "gone": true })),
        hold("ws_missing", "p_web", json!({ "path": "/nonexistent/tree", "branch": "b", "made": true })),
        hold("ws_norepo", "p_flat", json!({ "path": path, "branch": "b", "made": true })),
    ];
    for entry in &skipped {
        run(rig.folders.worktree_mounted(entry)).unwrap();
    }
    assert_eq!(rig.base.args_of("copier_worktree_mount"), Vec::<Value>::new());
    let standing = hold("ws_standing", "p_web", json!({ "path": path, "branch": "b", "made": true }));
    run(rig.folders.worktree_mounted(&standing)).unwrap();
    assert_eq!(rig.base.args_of("copier_worktree_mount"), vec![json!({ "from": "/home/me/web", "home": STATE, "path": path })]);
    rig.fake.copier.set(false);
    run(rig.folders.worktree_mounted(&standing)).unwrap();
    assert_eq!(rig.base.args_of("copier_worktree_mount").len(), 1);
}

#[test]
fn a_failed_mount_warns_and_answers() {
    let rig = Rig::new();
    rig.hold(web());
    let dir = on_disk();
    let path = dir.path().to_str().unwrap().to_owned();
    let entry = rig.base.hold(record("ws_tree", "local", "p_web", json!({ "worktree": { "path": path, "branch": "b", "made": true } })));
    rig.base.answer_with("copier_worktree_mount", vec![err_answer("overlay busy", None)]);
    let warned = wsp_core::console::capture();
    run(rig.folders.worktree_mounted(&entry)).unwrap();
    assert_eq!(warned.lines(), vec![format!("the worktree at {path} was not mounted again: overlay busy")]);
    assert_eq!(rig.base.args_of("copier_worktree_mount"), vec![json!({ "from": "/home/me/web", "home": STATE, "path": path })]);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 8: worktree_record_at

#[test]
fn worktree_record_at_keeps_made_for_of_the_record_it_writes_again() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let held = rig.base.hold(record(
        "ws_tree",
        "local",
        "p_web",
        json!({ "worktree": {
            "path": "/tmp/wt", "branch": "old", "made": false, "madeFor": "t_root",
            "settled": { "at": 5, "why": "merged" }, "kept": "a thread is running", "removeFailed": "locked", "gone": true,
        } }),
    ));
    let again = tree(json!({ "path": "/tmp/wt", "branch": "feat/x", "made": true, "madeFor": "t_other" }));
    let got = run(rig.folders.worktree_record_at(&p, again, None)).unwrap();
    assert!(Rc::ptr_eq(&got, &held));
    assert_eq!(worktree_of(&got), json!({ "path": "/tmp/wt", "branch": "feat/x", "made": true, "madeFor": "t_root" }));
    assert_eq!(rig.base.persisted.borrow().iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["ws_tree"]);
    base::assert_due(&rig.base.clock, &[1_600_000]);
    let no_branch = tree(json!({ "path": "/tmp/wt", "made": false }));
    run(rig.folders.worktree_record_at(&p, no_branch, None)).unwrap();
    assert_eq!(worktree_of(&got), json!({ "path": "/tmp/wt", "made": true, "madeFor": "t_root" }));
    assert_eq!(rig.base.events.borrow().clone(), Vec::<Value>::new());
    assert_eq!(rig.base.live.borrow().len(), 1);
}

#[test]
fn worktree_record_at_makes_a_child_of_the_asking_folder() {
    let rig = Rig::new();
    let p = rig.hold(web());
    let plain_tree = tree(json!({ "path": "/tmp/by-hand", "branch": "by-hand", "made": false, "madeFor": "t_root" }));
    let got = run(rig.folders.worktree_record_at(&p, plain_tree, Some("ws_ask".to_owned()))).unwrap();
    assert_eq!(
        rec_json(&got),
        folder_record(
            "ws_11111111",
            "web@by-hand",
            json!({ "worktree": { "path": "/tmp/by-hand", "branch": "by-hand", "made": false, "madeFor": "t_root" }, "parentWorkspaceId": "ws_ask" })
        )
    );
    assert_eq!(rig.base.events.borrow().clone(), vec![created(&rig, &got)]);
    base::assert_due(&rig.base.clock, &[]);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 10: place_for and folder_for

const GUEST_NAMES: &str = "a line from a machine names the project it means, since the folder it was typed in is not one this host can read. Name the project on the line.";
const ASK_PLACE_BRANCH: &str = "wsp makes no worktrees on spoo yet; start the thread without a branch, or make one inside the project folder with git worktree add and name it as the folder";

fn ask_in(project: &str) -> FolderAsk {
    FolderAsk { project: Some(project.to_owned()), ..Default::default() }
}

#[test]
fn place_for_refuses_a_guest_naming_no_project() {
    let rig = Rig::new();
    rig.hold(web());
    rig.base.hold(record("ws_guest", "cloud", "p_web", json!({})));
    let origin = Some(thread("t1", "ws_guest", "t1"));
    let e = refusal(run(rig.folders.place_for(&FolderAsk::default(), origin)));
    assert_eq!(e, kinded(GUEST_NAMES, Kind::Usage));
    assert_eq!(rig.base.args_of("resolve_project"), Vec::<Value>::new());
    rig.untouched();
}

#[test]
fn place_for_refuses_a_box_thread_naming_another_computers_project() {
    let rig = Rig::new();
    rig.hold(web());
    rig.hold(boxed());
    let asking = rig.base.hold(record("ws_box_ask", "place", "p_box", json!({ "place": "pl_spoo" })));
    let said = "a thread on spoo cannot start one on Mac yet; the person can start it there, or name a project on spoo";
    rig.base.answer_with("elsewhere_refusal", vec![err_answer(said, Some("usage"))]);
    let origin = || Some(thread("t1", "ws_box_ask", "t1"));
    let e = refusal(run(rig.folders.place_for(&ask_in("web"), origin())));
    assert_eq!(e, kinded(said, Kind::Usage));
    assert_eq!(rig.base.args_of("elsewhere_refusal"), vec![json!({ "project": "p_web", "word": "web", "scoped": true })]);
    assert_eq!(rig.base.args_of("resolve_project"), Vec::<Value>::new());
    rig.untouched();
    // A word its own project answers is its own, whatever another computer's is called.
    let got = run(rig.folders.place_for(&ask_in("box"), origin())).unwrap();
    assert!(Rc::ptr_eq(&got.entry, &asking));
    assert_eq!(rig.base.args_of("elsewhere_refusal").len(), 1);
    assert_eq!(rig.base.args_of("resolve_project"), vec![json!({ "word": "box", "scoped": true })]);
}

#[test]
fn place_for_refuses_branch_and_cwd_together() {
    let rig = Rig::new();
    rig.hold(web());
    let ask = FolderAsk { project: Some("web".into()), branch: Some("x".into()), cwd: Some("/home/me/web/src".into()), ..Default::default() };
    let e = refusal(run(rig.folders.place_for(&ask, None)));
    assert_eq!(e, kinded("name a branch or a folder, not both", Kind::Usage));
    rig.untouched();
    assert_eq!(rig.base.args_of("git"), Vec::<Value>::new());
}

#[test]
fn place_for_on_a_box_refuses_a_branch_and_a_cwd_outside() {
    let rig = Rig::new();
    rig.hold(boxed());
    let ask = |branch: Option<&str>, cwd: Option<&str>| FolderAsk { project: Some("box".into()), branch: branch.map(Into::into), cwd: cwd.map(Into::into), ..Default::default() };
    let outside = |path: &str| kinded(&format!("{path} is not in box or a worktree of it; wsp add it first"), Kind::Usage);
    assert_eq!(refusal(run(rig.folders.place_for(&ask(Some("x"), None), None))), kinded(ASK_PLACE_BRANCH, Kind::Usage));
    assert_eq!(refusal(run(rig.folders.place_for(&ask(None, Some("/etc/passwd")), None))), outside("/etc/passwd"));
    assert_eq!(refusal(run(rig.folders.place_for(&ask(None, Some("src")), None))), outside("src"));
    assert_eq!(refusal(run(rig.folders.place_for(&ask(None, Some("/root/box/../other")), None))), outside("/root/other"));
    assert_eq!(refusal(run(rig.folders.place_for(&ask(None, Some("/root/boxer")), None))), outside("/root/boxer"));
    rig.untouched();
    assert_eq!(rig.base.args_of("git"), Vec::<Value>::new());
    let inside = run(rig.folders.place_for(&ask(None, Some("/root/box/./src//x")), None)).unwrap();
    assert_eq!(inside.cwd, Some("/root/box/src/x".to_owned()));
    assert_eq!(rec(&inside.entry).place, Some("pl_spoo".to_owned()));
    assert_eq!(inside.setup, None);
}

#[test]
fn place_for_runs_beside_the_asking_thread() {
    let rig = Rig::new();
    rig.hold(web());
    rig.hold(project("p_other", "other", "here", json!({})));
    let folder = rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    let asking = rig.base.hold(record("ws_ask", "local", "p_web", json!({})));
    let on_tree = rig.base.hold(record("ws_ask_tree", "local", "p_web", json!({ "worktree": { "path": "/tmp/t", "branch": "b", "made": true } })));
    let other = rig.base.hold(record("ws_ask_other", "local", "p_other", json!({})));
    let origin = |ws: &str| Some(thread("t1", ws, "t1"));
    let placed = run(rig.folders.place_for(&FolderAsk::default(), origin("ws_ask"))).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &asking));
    assert_eq!(placed.cwd, None);
    assert_eq!(placed.setup, None);
    assert_eq!(rig.base.args_of("resolve_project"), Vec::<Value>::new());
    let cwd = FolderAsk { cwd: Some("/home/me/web/src".into()), ..Default::default() };
    let placed = run(rig.folders.place_for(&cwd, origin("ws_ask"))).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &asking));
    assert_eq!(placed.cwd, Some("/home/me/web/src".to_owned()));
    // Beside a worktree's thread a start naming nothing runs in that worktree; a cwd runs in the project folder.
    let placed = run(rig.folders.place_for(&ask_in("p_web"), origin("ws_ask_tree"))).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &on_tree));
    let placed = run(rig.folders.place_for(&cwd, origin("ws_ask_tree"))).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &folder));
    assert_eq!(placed.cwd, Some("/home/me/web/src".to_owned()));
    // A thread of another project is beside nothing here: the project folder answers.
    let placed = run(rig.folders.place_for(&ask_in("web"), origin("ws_ask_other"))).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &folder));
    assert!(!Rc::ptr_eq(&placed.entry, &other));
    rig.untouched();
}

// Ports folder-threads.test.ts:164: the project named by id and by name is one folder's record, no copy made.
#[test]
fn two_starts_naming_the_project_by_id_and_by_name_run_in_one_folder_record() {
    let rig = Rig::new();
    rig.hold(web());
    let (a, b) = run(async { futures::join!(rig.folders.folder_for(ask_in("p_web"), None), rig.folders.folder_for(ask_in("web"), None)) });
    let (a, b) = (a.unwrap(), b.unwrap());
    assert!(Rc::ptr_eq(&a.entry, &b.entry));
    assert_eq!(rec_json(&a.entry), folder_record("ws_11111111", "web", json!({})));
    assert_eq!(rig.base.persisted.borrow().len(), 1);
    assert_eq!(rig.base.args_of("copier_worktree"), Vec::<Value>::new());
    assert_eq!(rig.base.args_of("git"), Vec::<Value>::new());
}

// Ports folder-threads.test.ts:489: a worktree the person made is recorded as not wsp's, for the asking tree.
#[test]
fn place_for_with_a_cwd_in_a_plain_git_worktree_records_it_for_the_tree() {
    let rig = Rig::new();
    rig.hold(web());
    rig.base.hold(record("ws_ask", "local", "p_web", json!({})));
    rig.script_worktrees("worktree /home/me/web\nHEAD abc\nbranch refs/heads/main\n\nworktree /tmp/by-hand\nHEAD def\nbranch refs/heads/by-hand\n\n");
    let ask = FolderAsk { project: Some("web".into()), cwd: Some("/tmp/by-hand/sub".into()), ..Default::default() };
    let placed = run(rig.folders.place_for(&ask, Some(thread("t_child", "ws_ask", "t_root")))).unwrap();
    assert_eq!(placed.cwd, Some("/tmp/by-hand/sub".to_owned()));
    assert_eq!(
        rec_json(&placed.entry),
        folder_record("ws_11111111", "web@by-hand", json!({ "worktree": { "path": "/tmp/by-hand", "branch": "by-hand", "made": false, "madeFor": "t_root" } }))
    );
    assert_eq!(persisted_json(&rig), json!([rec_json(&placed.entry)]));
    assert_eq!(rig.base.args_of("git"), vec![json!({ "cwd": "/home/me/web", "args": ["worktree", "list", "--porcelain"], "timeoutMs": null, "input": null })]);
    assert_eq!(rig.base.args_of("copier_worktree"), Vec::<Value>::new());
    assert_eq!(rig.base.events.borrow().len(), 1);
}

// Ports folder-threads.test.ts:513: a project in a subfolder of a repo, a cwd in the repo's top beside it.
#[test]
fn place_for_with_a_cwd_in_the_repo_top_outside_the_project_subfolder_runs_in_the_project_folder() {
    let rig = Rig::new();
    rig.hold(project("p_web", "web", "here", json!({ "path": "/home/me/web/apps/web", "git": { "top": "/home/me/web" } })));
    let folder = rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    rig.script_worktrees("worktree /home/me/web\nbranch refs/heads/main\n");
    let ask = FolderAsk { project: Some("web".into()), cwd: Some("/home/me/web/docs".into()), ..Default::default() };
    let placed = run(rig.folders.place_for(&ask, None)).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &folder));
    assert_eq!(placed.cwd, Some("/home/me/web/docs".to_owned()));
    rig.untouched();
}

// Ports folder-threads.test.ts:504: outside the project and its worktrees, in one sentence.
#[test]
fn place_for_refuses_a_cwd_outside_the_project_and_its_worktrees_in_one_sentence() {
    let rig = Rig::new();
    rig.hold(web());
    rig.hold(project("p_plain", "notes", "here", json!({})));
    rig.script_worktrees("worktree /home/me/web\nbranch refs/heads/main\n");
    let ask = |p: &str| FolderAsk { project: Some(p.into()), cwd: Some("/home/me/elsewhere".into()), ..Default::default() };
    let e = refusal(run(rig.folders.place_for(&ask("web"), None)));
    assert_eq!(e, kinded("~/elsewhere is not in web or a worktree of it; wsp add it first", Kind::Usage));
    assert_eq!(rig.base.args_of("git").len(), 1);
    let e = refusal(run(rig.folders.place_for(&ask("notes"), None)));
    assert_eq!(e, kinded("~/elsewhere is not in notes or a worktree of it; wsp add it first", Kind::Usage));
    assert_eq!(rig.base.args_of("git").len(), 1);
    rig.untouched();
}

#[test]
fn place_for_on_the_folders_own_branch_runs_in_the_folder() {
    let rig = Rig::new();
    rig.hold(web());
    let folder = rig.base.hold(record("ws_folder", "local", "p_web", json!({})));
    let asking = rig.base.hold(record("ws_ask", "local", "p_web", json!({})));
    rig.fake.branches.borrow_mut().insert("/home/me/web".to_owned(), Some("main".to_owned()));
    rig.fake.branches.borrow_mut().insert("/checkouts/ws_ask".to_owned(), Some("feature".to_owned()));
    let ask = |branch: &str| FolderAsk { project: Some("web".into()), branch: Some(branch.into()), ..Default::default() };
    let placed = run(rig.folders.place_for(&ask("main"), None)).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &folder));
    assert_eq!(placed.cwd, None);
    let placed = run(rig.folders.place_for(&ask("feature"), Some(thread("t1", "ws_ask", "t1")))).unwrap();
    assert!(Rc::ptr_eq(&placed.entry, &asking));
    assert_eq!(rig.base.args_of("branch_at"), vec![json!({ "path": "/home/me/web" }), json!({ "path": "/checkouts/ws_ask" })]);
    assert_eq!(rig.base.args_of("copier_worktree"), Vec::<Value>::new());
    rig.untouched();
}

#[test]
fn place_for_a_branch_makes_a_worktree_child_of_the_asking_folder() {
    let rig = Rig::new();
    rig.hold(web());
    rig.base.hold(record("ws_ask", "local", "p_web", json!({})));
    rig.fake.branches.borrow_mut().insert("/checkouts/ws_ask".to_owned(), Some("main".to_owned()));
    let ask = FolderAsk { project: Some("web".into()), branch: Some("feat/x".into()), ..Default::default() };
    let placed = run(rig.folders.place_for(&ask, Some(thread("t1", "ws_ask", "t_root")))).unwrap();
    assert_eq!(
        rec_json(&placed.entry),
        folder_record(
            "ws_11111111",
            "web@feat/x",
            json!({ "worktree": { "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true, "madeFor": "t_root" }, "parentWorkspaceId": "ws_ask" })
        )
    );
    assert_eq!(placed.cwd, None);
    assert_eq!(placed.setup, None);
    assert_eq!(rig.base.args_of("copier_worktree"), vec![copier_ask("feat/x", Value::Null)]);
    assert_eq!(rig.base.args_of("branch_at"), vec![json!({ "path": "/checkouts/ws_ask" })]);
    assert_eq!(rig.base.args_of("one_folder"), vec![json!({ "key": "p_web\u{0}feat/x" })]);
}

// Ports folder-threads.test.ts:445: asking a branch makes no record of the project folder to read the branch it holds.
#[test]
fn a_branch_start_makes_no_record_of_the_project_folder_to_read_its_branch() {
    let rig = Rig::new();
    rig.hold(web());
    let ask = FolderAsk { project: Some("web".into()), branch: Some("feat/only".into()), ..Default::default() };
    let placed = run(rig.folders.place_for(&ask, None)).unwrap();
    assert_eq!(rig.base.live.borrow().keys().cloned().collect::<Vec<_>>(), vec![rec(&placed.entry).id]);
    assert_eq!(rec(&placed.entry).worktree.map(|w| w.path), Some("/home/me/.wsp/worktrees/p_web/feat-only".to_owned()));
    assert_eq!(rig.base.args_of("branch_at"), vec![json!({ "path": "/home/me/web" })]);
}

// Ports folder-threads.test.ts:478: a folder git holds no repo in is still a project, and takes no branch.
#[test]
fn place_for_a_branch_on_a_project_with_no_repo_is_refused() {
    let rig = Rig::new();
    rig.hold(project("p_plain", "notes", "here", json!({})));
    let ask = FolderAsk { project: Some("notes".into()), branch: Some("x".into()), ..Default::default() };
    let e = refusal(run(rig.folders.place_for(&ask, None)));
    assert_eq!(e, kinded("notes is not a git repo, so it has no branches", Kind::Usage));
    assert_eq!(rig.base.args_of("branch_at"), Vec::<Value>::new());
    rig.untouched();
}

// Ports folder-threads.test.ts:513: the copier is asked from the repo's top for a project in a subfolder.
#[test]
fn place_for_a_branch_of_a_project_in_a_subfolder_asks_the_copier_from_the_repo_top() {
    let rig = Rig::new();
    rig.hold(project("p_web", "web", "here", json!({ "path": "/home/me/web/apps/web", "git": { "top": "/home/me/web" } })));
    let ask = FolderAsk { project: Some("web".into()), branch: Some("feat/web".into()), ..Default::default() };
    run(rig.folders.place_for(&ask, None)).unwrap();
    assert_eq!(rig.base.args_of("branch_at"), vec![json!({ "path": "/home/me/web/apps/web" })]);
    assert_eq!(rig.base.args_of("copier_worktree"), vec![copier_ask("feat/web", Value::Null)]);
}

// Ports folder-threads.test.ts:1327: a pick the agent does not take is refused before any worktree is made.
#[test]
fn place_for_reads_the_picks_before_it_makes_a_worktree() {
    let rig = Rig::new();
    rig.hold(web());
    rig.base.answer_with("picks_hold", vec![err_answer("codex takes no such model", Some("usage"))]);
    let picks = wsp_core::types::harness::StartPicksAsked { harness: Some("codex".into()), ..Default::default() };
    let ask = FolderAsk { project: Some("web".into()), branch: Some("feat/x".into()), picks: Some(picks), ..Default::default() };
    let e = refusal(run(rig.folders.place_for(&ask, None)));
    assert_eq!(e, kinded("codex takes no such model", Kind::Usage));
    assert_eq!(rig.base.args_of("picks_hold"), vec![json!({ "project": "p_web", "harness": "codex" })]);
    rig.untouched();
}

// Ports folder-threads.test.ts:374 and :389: the line a fresh worktree left opens only a start that opens a thread.
#[test]
fn folder_for_takes_the_setup_line_only_when_it_opens() {
    let rig = Rig::new();
    rig.hold(web());
    rig.after_worktree("make setup");
    rig.base.answer_with(
        "copier_worktree",
        vec![
            json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true, "carried": [], "fresh": true, "modules": [], "ms": 1 }),
            json!({ "path": "/home/me/.wsp/worktrees/p_web/feat-x", "branch": "feat/x", "made": true, "carried": [], "fresh": false, "modules": [], "ms": 1 }),
        ],
    );
    let ask = |opens: bool| FolderAsk { project: Some("web".into()), branch: Some("feat/x".into()), opens, ..Default::default() };
    let line = format!("{SETUP_HEAD}\n- make setup");
    let first = run(rig.folders.folder_for(ask(false), None)).unwrap();
    assert_eq!(first.setup, None);
    let named = run(rig.folders.place_for(&ask(true), None)).unwrap();
    assert_eq!(named.setup, None);
    assert!(Rc::ptr_eq(&named.entry, &first.entry));
    let opened = run(rig.folders.folder_for(ask(true), None)).unwrap();
    assert!(Rc::ptr_eq(&opened.entry, &first.entry));
    assert_eq!(opened.setup, Some(line));
    let again = run(rig.folders.folder_for(ask(true), None)).unwrap();
    assert_eq!(again.setup, None);
}

// ---------------------------------------------------------------------------------------------------------------
// Rule 11: under

#[test]
fn under_matches_the_folder_and_below_and_not_a_sibling_prefix() {
    assert!(under("/a/b", "/a/b"));
    assert!(under("/a/b/c", "/a/b"));
    assert!(under("/a/b/c/d", "/a/b"));
    assert!(!under("/a/bc", "/a/b"));
    assert!(!under("/a", "/a/b"));
    assert!(under("/a/b/c", "/a/b/"));
    assert!(under("/a/b/", "/a/b/"));
    assert!(!under("/a/b", "/a/b/"));
    assert!(under("/x", "/"));
}
