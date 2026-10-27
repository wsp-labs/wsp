// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/worktree-sweep.test.ts at 65f6b89c7, case for case. The one sweep of the worktrees wsp made. A
//! worktree settles when its pull request merged or closed, when the branch it pushed is gone at the remote, or when the
//! person removed it; a settled one goes once six hours have passed with nothing uncommitted and no turn running, and says why it
//! stays otherwise. git is real; the copier removes as the daemon binary's verb does, and the daemon answers the branch and
//! the pull request. Each test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use async_trait::async_trait;
use common::daemon::git;
use common::local::{git_copier, local_wiring, CopierLog, GitCopier};
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::daemon_channel::Frame;
use wsp_core::types::api::{FolderForAsk, ProjectAddAsk, StartOptions};
use wsp_engine::copier::{Copier, CopyAsk, CopyReport, CopyRoad, WorktreeAsk, WorktreeAt, WorktreeProject, WorktreeRemoval, WorktreeRemove, WorktreeReport};
use wsp_engine::errors::{EngineError, Result as EngineResult};
use wsp_engine::local_backend::MemoryHere;
use wsp_wire::types::projects::{KEPT_ABANDONED_LINE, KEPT_RUNNING_LINE};
use wsp_words::projects::kept_changed_line;

const SWEEP: i64 = 10 * 60_000;
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

/// A folder of the test's own, removed when the test ends.
struct Scratch(Vec<tempfile::TempDir>);

impl Scratch {
    fn new() -> Scratch {
        Scratch(Vec::new())
    }
    fn dir(&mut self) -> PathBuf {
        let dir = tempfile::tempdir().expect("a folder");
        let path = std::fs::canonicalize(dir.path()).expect("a real path");
        self.0.push(dir);
        path
    }
}

/// A project folder cloned from a bare origin, or with no remote at all.
fn project(scratch: &mut Scratch, remote: bool) -> (PathBuf, Option<PathBuf>) {
    let root = scratch.dir();
    let folder = root.join("spoo");
    if !remote {
        std::fs::create_dir(&folder).expect("a folder");
        git(&folder, &["init", "-q", "-b", "main"]);
        git(&folder, &["commit", "-q", "--allow-empty", "-m", "first"]);
        return (folder, None);
    }
    let origin = root.join("origin.git");
    let run = |args: &[&str]| {
        let ok = std::process::Command::new("git").args(args).status().expect("git").success();
        assert!(ok, "git {args:?}");
    };
    run(&["init", "-q", "--bare", "-b", "main", &origin.display().to_string()]);
    run(&["clone", "-q", &origin.display().to_string(), &folder.display().to_string()]);
    git(&folder, &["commit", "-q", "--allow-empty", "-m", "first"]);
    git(&folder, &["push", "-q", "origin", "main"]);
    (folder, Some(origin))
}

fn pr(branch: &str, state: &str) -> Value {
    json!({
        "number": 7, "url": "https://github.com/dev/spoo/pull/7", "state": state, "host": "github.com", "draft": false, "base": "main",
        "branch": branch, "headOid": "a".repeat(40), "headSubject": "the work", "mergeable": "mergeable", "mergeState": "clean",
        "review": "none", "checks": [], "additions": 1, "deletions": 0, "changedFiles": 1, "commits": 1,
    })
}

/// This computer's daemon: the branch a folder has checked out, read with git, and the pull request of a branch as the test sets
/// it; every frame noted.
struct Daemon {
    rec: Rc<RecDaemon>,
    prs: Rc<RefCell<HashMap<String, Value>>>,
}

fn daemon() -> Daemon {
    let prs: Rc<RefCell<HashMap<String, Value>>> = Rc::default();
    let known = prs.clone();
    let rec = RecDaemon::new(move |f: &Frame| {
        let text = |key: &str| f.get(key).and_then(Value::as_str).unwrap_or("undefined").to_string();
        match f["op"].as_str().unwrap_or_default() {
            "git.status" => {
                let cwd = PathBuf::from(text("cwd"));
                let head = git(&cwd, &["symbolic-ref", "--short", "HEAD"]);
                json!({ "id": 1, "ok": true, "branch": { "oid": git(&cwd, &["rev-parse", "HEAD"]), "head": head, "ahead": 0, "behind": 0 }, "entries": [], "root": text("cwd") })
            }
            "git.prRead" => match known.borrow().get(&text("branch")) {
                Some(pr) => json!({ "id": 1, "ok": true, "pr": pr }),
                None => json!({ "id": 1, "ok": true }),
            },
            "git.checkpoint" => json!({ "id": 1, "ok": true, "ref": "refs/wsp/checkpoints/x", "commit": "c", "changed": false }),
            other => json!({ "id": 1, "ok": false, "error": format!("no {other} here") }),
        }
    });
    Daemon { rec, prs }
}

impl Daemon {
    fn prs_read(&self) -> Vec<Value> {
        self.rec.frames.borrow().iter().filter(|f| f["op"] == "git.prRead").map(|f| Value::Object(f.clone())).collect()
    }
}

/// A copier whose removal git refuses: it notes the ask and throws the line a locked worktree answers.
struct Locked {
    inner: Rc<GitCopier>,
    log: CopierLog,
}

fn other(message: String) -> EngineError {
    EngineError::Other { message, code: None }
}

#[async_trait(?Send)]
impl Copier for Locked {
    async fn make(&self, ask: &CopyAsk) -> EngineResult<CopyReport> {
        self.inner.make(ask).await
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> EngineResult<()> {
        self.inner.remove(from, to, road).await
    }
    async fn worktree(&self, ask: &WorktreeAsk) -> EngineResult<WorktreeReport> {
        self.inner.worktree(ask).await
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> EngineResult<()> {
        self.inner.worktree_mount(o).await
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> EngineResult<()> {
        self.inner.worktree_forget(o).await
    }
    async fn worktree_remove(&self, o: &WorktreeRemove) -> EngineResult<WorktreeRemoval> {
        self.log.lock().unwrap().worktrees_removed.push(o.clone());
        Err(other(format!("fatal: '{}' is locked; use 'unlock' to override", o.path)))
    }
    fn has_room(&self) -> bool {
        self.inner.has_room()
    }
    async fn room(&self) -> Option<MemoryHere> {
        self.inner.room().await
    }
}

struct Host {
    fx: Fx,
    log: CopierLog,
    daemon: Daemon,
    time: FakeClock,
    agent: Rc<Quick>,
    _scratch: Scratch,
    _state: State,
}

fn host(mut scratch: Scratch, hold: bool, locked: bool) -> Host {
    let root = scratch.dir();
    let state_dir = root.join("state");
    std::fs::create_dir(&state_dir).expect("a folder");
    let (copier, log) = git_copier();
    let copier: Rc<dyn Copier> = if locked { Rc::new(Locked { inner: copier, log: log.clone() }) } else { copier };
    let d = daemon();
    let time = FakeClock::new(1_790_856_000_000);
    let agent = Quick::new(QuickOptions { session_prefix: "66666666-6666-4666-8666", announces: true, anchors: false, ..Default::default() });
    agent.holding.set(hold);
    let local = local_wiring(&root, copier);
    let state_path = state_dir.join("state.json").display().to_string();
    let open = d.rec.open();
    let state = sqlite_state();
    let backend = StubBackend::new(None);
    let fx = runtime(&backend)
        .store(&state.store)
        .clock(&time)
        .adapter("claude", agent.factory())
        .with(move |o| {
            o.local = Some(local);
            o.state_path = Some(state_path);
            o.daemon_channel = Some(open);
        })
        .build();
    Host { fx, log, daemon: d, time, agent, _scratch: scratch, _state: state }
}

/// The worktree record a branch start lands on, with one thread run in it so the record outlives its folder.
async fn worktree_with_thread(h: &Host, folder: &Path, branch: &str, wait: bool) -> Value {
    let added = h.fx.rt.projects().add(ProjectAddAsk { source: folder.display().to_string(), ..Default::default() }, None).await.expect("a project");
    let at = h
        .fx
        .rt
        .workspaces()
        .folder_for(FolderForAsk { project: Some(added.project.id.clone()), branch: Some(branch.into()), ..Default::default() }, None)
        .await
        .expect("a folder");
    let turn = h.fx.rt.sessions().start(&at.workspace.id, StartOptions { prompt: "work".into(), ..Default::default() }, None).await.expect("a start");
    if wait {
        turn.finished().await;
    }
    j(&at.workspace)
}

async fn record_of(h: &Host, id: &str) -> Option<Value> {
    h.fx.rt.workspaces().list(None).await.expect("a list").iter().map(j).find(|w| w["id"] == id)
}

async fn worktree_of(h: &Host, id: &str) -> Value {
    record_of(h, id).await.expect("the record")["worktree"].clone()
}

/// The clock moved on by `ms`, then one sweep run there and waited out, and `done` holding after it. A sweep arms the next one
/// only once it is over, so a long move runs one sweep on the way and the one that counts runs where it stands.
async fn sweep<F: std::future::Future<Output = bool>>(h: &Host, ms: i64, mut done: impl FnMut() -> F) {
    later(h, ms).await;
    later(h, SWEEP).await;
    until_for(3_000, &mut done).await;
}

/// The clock moved on by `ms`, and whatever sweep that ran waited out: the next one is armed, or no worktree wsp made is left
/// for one to look at, and the host holds no timer for it.
async fn later(h: &Host, ms: i64) {
    h.time.advance(ms);
    until_for(3_000, &mut || {
        let (rt, time) = (h.fx.rt.clone(), h.time.clone());
        async move {
            time.due_for(time.now() + SWEEP)
                || !rt.workspaces().list(None).await.expect("a list").iter().map(j).any(|w| {
                    let tree = &w["worktree"];
                    tree["made"] == true && tree["gone"] != true
                })
        }
    })
    .await;
}

fn removed_paths(h: &Host) -> Vec<String> {
    h.log.lock().unwrap().worktrees_removed.iter().map(|r| r.path.clone()).collect()
}

fn path_of(tree: &Value) -> String {
    tree["path"].as_str().expect("a path").to_string()
}

fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

async fn settled(h: &Host, id: &str) -> bool {
    !worktree_of(h, id).await["settled"].is_null()
}

// "the sweep of worktrees wsp made" > "holds no timer while no worktree wsp made stands, and arms one when the first is made"
#[test]
fn holds_no_timer_while_no_worktree_wsp_made_stands_and_arms_one_when_the_first_is_made() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let added = h.fx.rt.projects().add(ProjectAddAsk { source: folder.display().to_string(), ..Default::default() }, None).await.expect("a project");
        h.fx.rt.workspaces().list(None).await.expect("a list");
        let idle = h.time.pending();
        h.fx.rt.workspaces().worktree(&added.project.id, "feat/one", None).await.expect("a worktree");
        assert_eq!(h.time.pending(), idle + 1);
        assert!(h.time.due_for(h.time.now() + SWEEP));
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "removes one whose pull request merged once six hours have passed, clean and with no turn running"
#[test]
fn removes_one_whose_pull_request_merged_once_six_hours_have_passed_clean_and_with_no_turn_running() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let (id, path) = (made["id"].as_str().expect("an id").to_string(), path_of(&made["worktree"]));
        h.daemon.prs.borrow_mut().insert("feat/x".into(), pr("feat/x", "merged"));
        sweep(&h, 0, || settled(&h, &id)).await;
        assert_eq!(worktree_of(&h, &id).await["settled"]["why"], "merged");
        assert!(exists(&path));
        sweep(&h, 6 * HOUR, || async { worktree_of(&h, &id).await["gone"] == true }).await;
        assert!(!exists(&path));
        assert_eq!(removed_paths(&h), [path]);
        assert!(!git(&folder, &["branch", "--list", "feat/x"]).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "keeps a settled one with a file no commit holds, and says how many"
#[test]
fn keeps_a_settled_one_with_a_file_no_commit_holds_and_says_how_many() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let (id, path) = (made["id"].as_str().expect("an id").to_string(), path_of(&made["worktree"]));
        h.daemon.prs.borrow_mut().insert("feat/x".into(), pr("feat/x", "closed"));
        std::fs::write(Path::new(&path).join("draft.md"), "mine\n").expect("a file");
        sweep(&h, 0, || settled(&h, &id)).await;
        sweep(&h, 6 * HOUR, || async { !worktree_of(&h, &id).await["kept"].is_null() }).await;
        assert_eq!(worktree_of(&h, &id).await["kept"], kept_changed_line(1.0));
        assert!(Path::new(&path).join("draft.md").exists());
        assert!(removed_paths(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "keeps a settled one while a turn runs in it"
#[test]
fn keeps_a_settled_one_while_a_turn_runs_in_it() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, true, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", false).await;
        let (id, path) = (made["id"].as_str().expect("an id").to_string(), path_of(&made["worktree"]));
        h.daemon.prs.borrow_mut().insert("feat/x".into(), pr("feat/x", "merged"));
        sweep(&h, 0, || settled(&h, &id)).await;
        sweep(&h, 6 * HOUR, || async { !worktree_of(&h, &id).await["kept"].is_null() }).await;
        assert_eq!(worktree_of(&h, &id).await["kept"], KEPT_RUNNING_LINE);
        assert!(exists(&path));
        h.agent.release();
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "marks git's refusal, and clears it once the person removed the worktree by hand"
#[test]
fn marks_gits_refusal_and_clears_it_once_the_person_removed_the_worktree_by_hand() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, true);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let (id, path) = (made["id"].as_str().expect("an id").to_string(), path_of(&made["worktree"]));
        h.daemon.prs.borrow_mut().insert("feat/x".into(), pr("feat/x", "merged"));
        sweep(&h, 0, || settled(&h, &id)).await;
        sweep(&h, 6 * HOUR, || async { !worktree_of(&h, &id).await["removeFailed"].is_null() }).await;
        let refused = worktree_of(&h, &id).await;
        assert_eq!(refused["removeFailed"], format!("fatal: '{path}' is locked; use 'unlock' to override"));
        assert!(refused.get("kept").is_none());
        std::fs::remove_dir_all(&path).expect("a removal");
        git(&folder, &["worktree", "prune"]);
        sweep(&h, 0, || async { worktree_of(&h, &id).await["gone"] == true }).await;
        let after = worktree_of(&h, &id).await;
        assert!(after.get("removeFailed").is_none());
        assert!(after.get("kept").is_none());
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "leaves one it could not remove for seven days to the person, and stops trying"
#[test]
fn leaves_one_it_could_not_remove_for_seven_days_to_the_person_and_stops_trying() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let (id, path) = (made["id"].as_str().expect("an id").to_string(), path_of(&made["worktree"]));
        h.daemon.prs.borrow_mut().insert("feat/x".into(), pr("feat/x", "merged"));
        std::fs::write(Path::new(&path).join("draft.md"), "mine\n").expect("a file");
        sweep(&h, 0, || settled(&h, &id)).await;
        sweep(&h, 7 * DAY, || async { worktree_of(&h, &id).await["kept"] == KEPT_ABANDONED_LINE }).await;
        std::fs::remove_file(Path::new(&path).join("draft.md")).expect("a removal");
        later(&h, SWEEP).await;
        later(&h, SWEEP).await;
        assert!(exists(&path));
        assert_eq!(worktree_of(&h, &id).await["kept"], KEPT_ABANDONED_LINE);
        assert!(removed_paths(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "settles a pushed branch the remote no longer has, and removes it six hours on"
#[test]
fn settles_a_pushed_branch_the_remote_no_longer_has_and_removes_it_six_hours_on() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, origin) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let (id, path) = (made["id"].as_str().expect("an id").to_string(), path_of(&made["worktree"]));
        git(Path::new(&path), &["push", "-q", "-u", "origin", "feat/x"]);
        git(&origin.expect("an origin"), &["branch", "-D", "feat/x"]);
        sweep(&h, 0, || settled(&h, &id)).await;
        assert_eq!(worktree_of(&h, &id).await["settled"]["why"], "deleted");
        sweep(&h, 6 * HOUR, || async { worktree_of(&h, &id).await["gone"] == true }).await;
        assert!(!exists(&path));
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "never settles a branch that was never pushed"
#[test]
fn never_settles_a_branch_that_was_never_pushed() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let id = made["id"].as_str().expect("an id").to_string();
        later(&h, 8 * DAY).await;
        later(&h, SWEEP).await;
        let held = worktree_of(&h, &id).await;
        assert!(held.get("settled").is_none());
        assert!(held.get("kept").is_none());
        assert!(exists(&path_of(&held)));
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "never settles a worktree of a repo with no remote, and asks no host about it"
#[test]
fn never_settles_a_worktree_of_a_repo_with_no_remote_and_asks_no_host_about_it() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, false);
        let h = host(scratch, false, false);
        let made = worktree_with_thread(&h, &folder, "feat/x", true).await;
        let id = made["id"].as_str().expect("an id").to_string();
        later(&h, 8 * DAY).await;
        later(&h, SWEEP).await;
        let held = worktree_of(&h, &id).await;
        assert!(held.get("settled").is_none());
        assert!(exists(&path_of(&held)));
        assert!(h.daemon.prs_read().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "never removes a worktree the person made, merged pull request and all, and gives it no kept line"
#[test]
fn never_removes_a_worktree_the_person_made_merged_pull_request_and_all_and_gives_it_no_kept_line() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let mine = scratch.dir().join("mine");
        let h = host(scratch, false, false);
        git(&folder, &["worktree", "add", "-q", "-b", "feat/y", &mine.display().to_string()]);
        h.daemon.prs.borrow_mut().insert("feat/y".into(), pr("feat/y", "merged"));
        let made = worktree_with_thread(&h, &folder, "feat/y", true).await;
        assert_matches(&made["worktree"], json!({ "path": mine.display().to_string(), "made": false }));
        let id = made["id"].as_str().expect("an id").to_string();
        later(&h, 8 * DAY).await;
        later(&h, SWEEP).await;
        let held = worktree_of(&h, &id).await;
        assert!(mine.exists());
        assert_eq!(held, json!({ "path": mine.display().to_string(), "branch": "feat/y", "made": false }));
        assert!(removed_paths(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "the sweep of worktrees wsp made" > "reads the pull request of a worktree no thread ever ran in"
#[test]
fn reads_the_pull_request_of_a_worktree_no_thread_ever_ran_in() {
    drive(|| async {
        let mut scratch = Scratch::new();
        let (folder, _) = project(&mut scratch, true);
        let h = host(scratch, false, false);
        let added = h.fx.rt.projects().add(ProjectAddAsk { source: folder.display().to_string(), ..Default::default() }, None).await.expect("a project");
        h.fx.rt.workspaces().worktree(&added.project.id, "feat/z", None).await.expect("a worktree");
        sweep(&h, 0, || async { h.daemon.prs_read().iter().any(|f| f["branch"] == "feat/z") }).await;
        h.fx.rt.close().await.expect("a close");
    });
}
