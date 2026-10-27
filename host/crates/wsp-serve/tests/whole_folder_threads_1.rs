// SPDX-License-Identifier: AGPL-3.0-only
//! The first half of packages/runtime/test/folder-threads.test.ts at 65f6b89c7, case for case: a thread on this computer runs in the
//! project folder; another branch runs it in a worktree of the project's repo. What is proved here is the record a start lands on, the
//! folder its turn is handed, what the copier is asked for, what a rewind may move in a folder threads share. The copier and the daemon
//! are fakes; git is real. Each test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use common::folder_host::*;
use common::local::{write_stub, Around, AroundBackend};
use common::*;
use futures::stream::LocalBoxStream;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AttachmentRoad, Env, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, ForkTurn, ForkedSession, HarnessAdapter,
    HarnessSession, HarnessStartOptions, SharedFuture, WriteOutcome,
};
use wsp_core::types::api::{RewindAsk, StartOptions};
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_core::types::wiring::HereDaemon;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::projects::{
    cwd_outside_line, no_branches_line, worktree_command_failed_line, worktree_command_running_line, worktree_setup_line,
    worktree_step_words, WorktreeCommandEnd,
};
use wsp_words::words::thread::REWIND_SHARED_LINE;

// ===== a thread on a project on this computer =====

// "a thread on a project on this computer" > "runs in the project folder, two at once, on one record and with no copy made"
#[test]
fn runs_in_the_project_folder_two_at_once_on_one_record_and_with_no_copy_made() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let (a, b) = futures::future::join(
            h.fx.rt.workspaces().folder_for(ask(&id_of(&project), None, None), None),
            h.fx.rt.workspaces().folder_for(ask(project["name"].as_str().expect("a name"), None, None), None),
        )
        .await;
        let (a, b) = (a.expect("a folder"), b.expect("a folder"));
        assert_eq!(a.workspace.id, b.workspace.id);
        assert_eq!(ws_json(&a)["folder"], path_of(&folder).as_str());
        assert!(ws_json(&a)["worktree"].is_null());
        ran(&h, &a.workspace.id, "hello").await;
        ran(&h, &b.workspace.id, "hello").await;
        assert_eq!(cwd_strings(&h), [Some(path_of(&folder)), Some(path_of(&folder))]);
        assert_eq!(asks(&h), 0);
        assert!(worktrees_asked(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on a project on this computer" > "hands its turn no port of its own: threads here share the person's ports"
#[test]
fn hands_its_turn_no_port_of_its_own_threads_here_share_the_persons_ports() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), None, None)).await;
        ran(&h, &at.workspace.id, "hello").await;
        assert!(ws_json(&at).get("portBase").is_none());
        assert_eq!(h.agents.starts.borrow().len(), 1);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on a project on this computer" > "is what a create on a project here answers, whatever name it is given, twice over"
#[test]
fn is_what_a_create_on_a_project_here_answers_whatever_name_it_is_given_twice_over() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let made = |name: &str| {
            let rt = h.fx.rt.clone();
            let project = id_of(&project);
            let name = name.to_string();
            async move { create_on(&rt, CreateOn { project: Some(project), ..CreateOn::named(&name) }, None).await.expect("a workspace").view }
        };
        let one = made("hello").await;
        let two = made("hello").await;
        assert_eq!(two.id, one.id);
        assert_eq!(j(&one)["folder"], path_of(&folder).as_str());
        assert_eq!(asks(&h), 0);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on a project on this computer" > "has its folder's record made when asked before any thread, once, and the first thread then runs on it"
#[test]
fn has_its_folders_record_made_when_asked_before_any_thread_once_and_the_first_thread_then_runs_on_it() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        assert!(workspaces_of(&h).await.is_empty());
        let (a, b) = futures::future::join(
            h.fx.rt.workspaces().folder(&id_of(&project), None),
            h.fx.rt.workspaces().folder(project["name"].as_str().expect("a name"), None),
        )
        .await;
        let (a, b) = (a.expect("a folder"), b.expect("a folder"));
        assert_eq!(a.id, b.id);
        assert_eq!(j(&a)["folder"], path_of(&folder).as_str());
        assert!(j(&a)["worktree"].is_null());
        assert_eq!(workspaces_of(&h).await.iter().map(id_of).collect::<Vec<_>>(), std::slice::from_ref(&a.id));
        assert_eq!(folder_for(&h, ask(&id_of(&project), None, None)).await.workspace.id, a.id);
        assert_eq!(asks(&h), 0);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on a project on this computer" > "asks no git of a start that names nothing and the daemon nothing but the turn's snapshot"
#[test]
fn asks_no_git_of_a_start_that_names_nothing_and_the_daemon_nothing_but_the_turns_snapshot() {
    drive(|| async {
        let s = Scratch::new();
        let seen: Rc<RefCell<Vec<String>>> = Rc::default();
        let kept = seen.clone();
        let around: Around = Rc::new(move |cmd: String| {
            kept.borrow_mut().push(cmd.clone());
            async move { Ok(cmd) }.boxed_local()
        });
        let backend = AroundBackend::new(&s.dir(), None, around);
        let h = here(&s, HereOpts { backend: Some(backend), ..Default::default() });
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), None, None)).await;
        ran(&h, &at.workspace.id, "warm").await;
        // A turn's checkpoint and the read of its checkout go off its road after it ends.
        until_for(10_000, &mut || async { h.ops().iter().any(|o| o == "git.checkpoint") && h.ops().iter().any(|o| o == "git.status") })
            .await;
        let before = (seen.borrow().len(), h.ops().len());
        h.agents.holding.set(true);
        let found = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let handle = start(&h, &found.workspace.id, "hello").await;
        // The look at the agent's launcher goes off the start's road, at the launch or once the start is slow.
        assert_eq!(seen.borrow()[before.0..].iter().filter(|c| !c.contains("command -v")).count(), 0);
        assert_eq!(h.ops()[before.1..], ["git.snapshot"]);
        assert_eq!(asks(&h), 0);
        assert!(worktrees_asked(&h).is_empty());
        h.agents.release();
        handle.finished().await;
        assert_eq!(h.agents.starts.borrow().len(), 2);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a new worktree's dependencies =====

/// Stand-ins for each ecosystem's tool on a PATH of their own, each writing its name, its words and the folder it ran in to one log; a
/// tool named in failing says why on stderr and exits 1.
struct Tools {
    backend: Arc<dyn wsp_engine::machine::MachineBackend>,
    log: PathBuf,
}

fn tools(s: &Scratch, failing: &[&str]) -> Tools {
    let bin = s.dir();
    let log = bin.join("ran.log");
    for tool in ["npm", "pnpm", "yarn", "bun", "composer", "uv", "poetry", "cargo", "go"] {
        let body = if failing.contains(&tool) {
            format!("echo \"{tool} could not install: lockfile out of date\" >&2\nexit 1\n")
        } else {
            format!("echo \"{tool} $* @ $(pwd)\" >> '{}'\n", log.display())
        };
        write_stub(&bin.join(tool), &format!("#!/bin/sh\n{body}"));
    }
    // On the command line itself: bash with a socket for stdin sources the system's bashrc, which may put a real tool's folder first on the PATH.
    let front = bin.display().to_string();
    let around: Around = Rc::new(move |cmd: String| {
        let prefixed = format!("PATH='{front}':\"$PATH\"; {cmd}");
        async move { Ok(prefixed) }.boxed_local()
    });
    let env = IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))]);
    Tools { backend: AroundBackend::new(&bin, Some(env), around), log }
}

impl Tools {
    fn ran(&self) -> Vec<String> {
        match std::fs::read_to_string(&self.log) {
            Ok(text) => text.trim().split('\n').map(str::to_string).collect(),
            Err(_) => vec![],
        }
    }
}

async fn dependencies_of(lockfile: &str, carry: &[&str], never: &[&str], line: &str) {
    let s = Scratch::new();
    let t = tools(&s, &[]);
    let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
    let project = add(&h, &s.repo_with(&[lockfile])).await;
    let at = folder_for(&h, ask(&id_of(&project), Some("feat/deps"), None)).await;
    let path = path_of(&h.worktrees.join(id_of(&project)).join("feat-deps"));
    let asked: Vec<Value> = worktrees_asked(&h)[0]["modules"]
        .as_array()
        .expect("the modules")
        .iter()
        .filter(|m| m["lockfiles"].as_array().expect("lockfiles").iter().any(|l| l == lockfile))
        .map(|m| json!({ "carry": m["carry"], "never": m["never"] }))
        .collect();
    assert_eq!(asked, [json!({ "carry": carry, "never": never })]);
    assert_eq!(t.ran(), [format!("{line} @ {path}")]);
    ran(&h, &at.workspace.id, "go").await;
    assert_eq!(h.agents.cwds()[0], Some(path.clone()));
    folder_for(&h, ask(&id_of(&project), Some("feat/deps"), None)).await;
    assert_eq!(t.ran(), [format!("{line} @ {path}")]);
    h.fx.rt.close().await.expect("a close");
}

// "a new worktree's dependencies" > "of a project with uv.lock carries nothing and runs uv sync --locked once at its top before its thread starts"
#[test]
fn of_a_project_with_uv_lock_carries_nothing_and_runs_uv_sync_locked_once_at_its_top_before_its_thread_starts() {
    drive(|| dependencies_of("uv.lock", &[], &[".venv"], "uv sync --locked"));
}

// "a new worktree's dependencies" > "of a project with Cargo.lock carries nothing and runs cargo fetch --locked once at its top before its thread starts"
#[test]
fn of_a_project_with_cargo_lock_carries_nothing_and_runs_cargo_fetch_locked_once_at_its_top_before_its_thread_starts() {
    drive(|| dependencies_of("Cargo.lock", &[], &["target"], "cargo fetch --locked"));
}

// "a new worktree's dependencies" > "of a project with pnpm-lock.yaml carries node_modules and runs pnpm install --frozen-lockfile --prefer-offline once at its top before its thread starts"
#[test]
fn of_a_project_with_pnpm_lock_yaml_carries_node_modules_and_runs_pnpm_install_frozen_lockfile_prefer_offline_once_at_its_top_before_its_thread_starts(
) {
    drive(|| {
        dependencies_of(
            "pnpm-lock.yaml",
            &["node_modules"],
            &[".next", ".turbo", ".cache"],
            "pnpm install --frozen-lockfile --prefer-offline",
        )
    });
}

// "a new worktree's dependencies" > "runs a module's install in the folder of the worktree that holds its lockfile"
#[test]
fn runs_a_modules_install_in_the_folder_of_the_worktree_that_holds_its_lockfile() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &[]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let project = add(&h, &s.repo_with(&["api/uv.lock", "web/package-lock.json"])).await;
        folder_for(&h, ask(&id_of(&project), Some("feat/apps"), None)).await;
        let path = path_of(&h.worktrees.join(id_of(&project)).join("feat-apps"));
        assert_eq!(
            t.ran(),
            [format!("uv sync --locked @ {path}/api"), format!("npm install --no-save --prefer-offline --no-audit --no-fund @ {path}/web")]
        );
        h.fx.rt.close().await.expect("a close");
    });
}

/// The copier of `gitCopier()` that reports every module its worktree made as already installed for the branch's lockfile.
struct Installed(Rc<common::local::GitCopier>);

#[async_trait(?Send)]
impl wsp_engine::copier::Copier for Installed {
    async fn make(&self, ask: &wsp_engine::copier::CopyAsk) -> wsp_engine::errors::Result<wsp_engine::copier::CopyReport> {
        self.0.make(ask).await
    }
    async fn remove(&self, from: &str, to: &str, road: wsp_engine::copier::CopyRoad) -> wsp_engine::errors::Result<()> {
        self.0.remove(from, to, road).await
    }
    async fn worktree(&self, ask: &wsp_engine::copier::WorktreeAsk) -> wsp_engine::errors::Result<wsp_engine::copier::WorktreeReport> {
        let mut made = self.0.worktree(ask).await?;
        for m in &mut made.modules {
            m.rebuild = false;
        }
        Ok(made)
    }
    async fn worktree_mount(&self, o: &wsp_engine::copier::WorktreeAt) -> wsp_engine::errors::Result<()> {
        self.0.worktree_mount(o).await
    }
    async fn worktree_forget(&self, o: &wsp_engine::copier::WorktreeProject) -> wsp_engine::errors::Result<()> {
        self.0.worktree_forget(o).await
    }
    async fn worktree_remove(
        &self,
        o: &wsp_engine::copier::WorktreeRemove,
    ) -> wsp_engine::errors::Result<wsp_engine::copier::WorktreeRemoval> {
        self.0.worktree_remove(o).await
    }
    fn has_room(&self) -> bool {
        self.0.has_room()
    }
    async fn room(&self) -> Option<wsp_engine::local_backend::MemoryHere> {
        self.0.room().await
    }
}

// "a new worktree's dependencies" > "runs no install for a module whose folders the verb carried in already installed for the branch's lockfile"
#[test]
fn runs_no_install_for_a_module_whose_folders_the_verb_carried_in_already_installed_for_the_branchs_lockfile() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &[]);
        let (inner, _log) = common::local::git_copier();
        let copier: Rc<dyn wsp_engine::copier::Copier> = Rc::new(Installed(inner));
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), copier: Some(Some(copier)), ..Default::default() });
        let project = add(&h, &s.repo_with(&["pnpm-lock.yaml"])).await;
        folder_for(&h, ask(&id_of(&project), Some("feat/same"), None)).await;
        assert!(t.ran().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "runs nothing for a project folder start, nor in a worktree of a branch whose lockfiles no module knows"
#[test]
fn runs_nothing_for_a_project_folder_start_nor_in_a_worktree_of_a_branch_whose_lockfiles_no_module_knows() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &[]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let project = add(&h, &s.repo_with(&["Gemfile.lock"])).await;
        folder_for(&h, ask(&id_of(&project), None, None)).await;
        folder_for(&h, ask(&id_of(&project), Some("feat/none"), None)).await;
        assert!(t.ran().is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "runs the project's after-worktree command once in each new worktree, after the ecosystems' own"
#[test]
fn runs_the_projects_after_worktree_command_once_in_each_new_worktree_after_the_ecosystems_own() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &[]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let project = add(&h, &s.repo_with(&["uv.lock"])).await;
        let id = id_of(&project);
        let after = format!("echo \"after @ $(pwd)\" >> '{}'", t.log.display());
        h.fx.rt.preferences().set(prefs(json!({ "projectDefaults": { id.clone(): { "afterWorktree": after } } }))).await.expect("a set");
        folder_for(&h, ask(&id, Some("a"), None)).await;
        folder_for(&h, ask(&id, Some("a"), None)).await;
        folder_for(&h, ask(&id, Some("b"), None)).await;
        folder_for(&h, ask(&id, None, None)).await;
        let (a, b) = (path_of(&h.worktrees.join(&id).join("a")), path_of(&h.worktrees.join(&id).join("b")));
        assert_eq!(
            t.ran(),
            [format!("uv sync --locked @ {a}"), format!("after @ {a}"), format!("uv sync --locked @ {b}"), format!("after @ {b}")]
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "runs a command that waits on an answer with nothing on its stdin, so it ends at once rather than at the bound"
#[test]
fn runs_a_command_that_waits_on_an_answer_with_nothing_on_its_stdin_so_it_ends_at_once_rather_than_at_the_bound() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &[]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let project = add(&h, &s.repo()).await;
        let id = id_of(&project);
        let after = format!("read answer; echo \"read ended: $?\" >> '{}'", t.log.display());
        h.fx.rt.preferences().set(prefs(json!({ "projectDefaults": { id.clone(): { "afterWorktree": after } } }))).await.expect("a set");
        folder_for(&h, ask(&id, Some("feat/asks"), None)).await;
        // With stdin left open the read waits for the bound to kill the shell, and the echo never runs.
        assert_eq!(t.ran(), ["read ended: 1"]);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "says each command as it starts and the one that failed, and its thread starts all the same, opening with what ran"
#[test]
fn says_each_command_as_it_starts_and_the_one_that_failed_and_its_thread_starts_all_the_same_opening_with_what_ran() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &["pnpm"]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let events = h.fx.events();
        let project = add(&h, &s.repo_with(&["pnpm-lock.yaml", "go.sum"])).await;
        let id = id_of(&project);
        let thread = serve_started(&h, json!({ "project": id, "branch": "feat/stale", "prompt": "go" })).await;
        let tree = workspaces_of(&h).await.into_iter().find(|w| w["worktree"]["branch"] == "feat/stale").expect("the worktree's record");
        let path = path_of(&h.worktrees.join(&id).join("feat-stale"));
        let (pnpm, go) = ("pnpm install --frozen-lockfile --prefer-offline", "go list -mod=readonly -deps -test ./... > /dev/null");
        let end = WorktreeCommandEnd::Exited { exit_code: 1, said: "pnpm could not install: lockfile out of date".into() };
        let heard: Vec<String> = events.of("host.notice").iter().map(|e| e["message"].as_str().expect("a message").to_string()).collect();
        assert_eq!(
            heard,
            [
                worktree_command_running_line(pnpm, &path),
                worktree_command_failed_line(pnpm, &path, &end),
                worktree_command_running_line(go, &path)
            ]
        );
        assert_eq!(t.ran(), [format!("go list -mod=readonly -deps -test ./... @ {path}")]);
        let ws = id_of(&tree);
        let want = worktree_setup_line(&[worktree_step_words(pnpm, ".", Some(&end)), worktree_step_words(go, ".", None)]);
        until(|| async { behind_lines(&h, &ws, None).await == [want.clone()] }).await;
        assert_eq!(h.agents.cwds()[0], Some(path));
        ran_in(&h, &ws, StartOptions { prompt: "again".into(), thread: Some(thread), ..Default::default() }).await;
        assert_eq!(behind_lines(&h, &ws, None).await.len(), 1);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "opens the thread whose start asked for the worktree with what ran there, and no other thread that runs there first"
#[test]
fn opens_the_thread_whose_start_asked_for_the_worktree_with_what_ran_there_and_no_other_thread_that_runs_there_first() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &["pnpm"]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let project = add(&h, &s.repo_with(&["pnpm-lock.yaml"])).await;
        let id = id_of(&project);
        h.fx.rt.workspaces().worktree(&id, "feat/two", None).await.expect("a worktree");
        let tree = workspaces_of(&h).await.into_iter().find(|w| w["worktree"]["branch"] == "feat/two").expect("the record");
        let ws = id_of(&tree);
        let other = ran(&h, &ws, "b").await;
        let asked = serve_started(&h, json!({ "project": id, "branch": "feat/two", "prompt": "a" })).await;
        until(|| async { behind_lines(&h, &ws, Some(&asked)).await.len() == 1 }).await;
        assert!(behind_lines(&h, &ws, Some(&thread_of(&other))).await.is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "opens a thread a start names the worktree's record for with what ran there, and no thread already there that sends again"
#[test]
fn opens_a_thread_a_start_names_the_worktrees_record_for_with_what_ran_there_and_no_thread_already_there_that_sends_again() {
    drive(|| async {
        let s = Scratch::new();
        let t = tools(&s, &["pnpm"]);
        let h = here(&s, HereOpts { backend: Some(t.backend.clone()), ..Default::default() });
        let project = add(&h, &s.repo_with(&["pnpm-lock.yaml"])).await;
        h.fx.rt.workspaces().worktree(&id_of(&project), "feat/byid", None).await.expect("a worktree");
        let tree = workspaces_of(&h).await.into_iter().find(|w| w["worktree"]["branch"] == "feat/byid").expect("the record");
        let ws = id_of(&tree);
        let there = ran(&h, &ws, "already here").await;
        let other = thread_of(&there);
        serve_started(&h, json!({ "workspaceId": ws, "thread": other, "prompt": "again" })).await;
        let opened = serve_started(&h, json!({ "workspaceId": ws, "prompt": "by id" })).await;
        until(|| async { behind_lines(&h, &ws, Some(&opened)).await.len() == 1 }).await;
        assert!(behind_lines(&h, &ws, Some(&other)).await.is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a new worktree's dependencies" > "mounts a worktree wsp made again before each turn there and at the host's start, never the project folder"
#[test]
fn mounts_a_worktree_wsp_made_again_before_each_turn_there_and_at_the_hosts_start_never_the_project_folder() {
    drive(|| async {
        let s = Scratch::new();
        let first = here(&s, HereOpts::default());
        let project = add(&first, &s.repo()).await;
        let id = id_of(&project);
        let tree = folder_for(&first, ask(&id, Some("feat/mounted"), None)).await;
        let folder = folder_for(&first, ask(&id, None, None)).await;
        let path = path_of(&first.worktrees.join(&id).join("feat-mounted"));
        let mounted = |h: &Here| h.copier.lock().unwrap().worktrees_mounted.clone();
        assert!(mounted(&first).is_empty());
        ran(&first, &tree.workspace.id, "go").await;
        ran(&first, &folder.workspace.id, "go").await;
        assert_eq!(mounted(&first), std::slice::from_ref(&path));
        first.fx.rt.workspaces().exec(&tree.workspace.id, "true", None, None).await.expect("an exec");
        assert_eq!(mounted(&first), [path.clone(), path.clone()]);
        // A restart of the computer took the mounts; the host's own start puts them back before any turn.
        first.fx.rt.close().await.expect("a close");
        let second =
            here(&s, HereOpts { again: Some(Again { root: first.root.clone(), state: first.state.clone() }), ..Default::default() });
        second.fx.rt.workspaces().list(None).await.expect("a list");
        until(|| async { mounted(&second) == [path.clone()] }).await;
        second.fx.rt.close().await.expect("a close");
    });
}

// ===== a thread on another branch =====

// "a thread on another branch" > "runs in a worktree the copier makes under the host's folder, keyed by the project, read for every ecosystem the catalogue knows"
#[test]
fn runs_in_a_worktree_the_copier_makes_under_the_hosts_folder_keyed_by_the_project_read_for_every_ecosystem_the_catalogue_knows() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let id = id_of(&project);
        let at = folder_for(&h, ask(&id, Some("feat/login"), None)).await;
        let modules: Vec<Value> = wsp_catalog::git_hosts::ECOSYSTEM_MODULES.iter().map(|m| j(&m.carry)).collect();
        assert_eq!(
            worktrees_asked(&h),
            [json!({ "from": path_of(&folder), "home": path_of(&h.home), "project": id, "branch": "feat/login", "modules": modules })]
        );
        let path = path_of(&h.worktrees.join(&id).join("feat-login"));
        assert_eq!(ws_json(&at)["worktree"], json!({ "path": path, "branch": "feat/login", "made": true }));
        assert_eq!(ws_json(&at)["folder"], path.as_str());
        ran(&h, &at.workspace.id, "fix login").await;
        assert_eq!(h.agents.cwds()[0], Some(path));
        // The same branch asked again, twice at once, is the same record.
        let (again, twice) = futures::future::join(
            h.fx.rt.workspaces().folder_for(ask(&id, Some("feat/login"), None), None),
            h.fx.rt.workspaces().folder_for(ask(&id, Some("feat/login"), None), None),
        )
        .await;
        assert_eq!(again.expect("a folder").workspace.id, at.workspace.id);
        assert_eq!(twice.expect("a folder").workspace.id, at.workspace.id);
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on another branch" > "makes no record of the project folder to read which branch it holds"
#[test]
fn makes_no_record_of_the_project_folder_to_read_which_branch_it_holds() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), Some("feat/only"), None)).await;
        assert_eq!(workspaces_of(&h).await.iter().map(id_of).collect::<Vec<_>>(), std::slice::from_ref(&at.workspace.id));
        h.fx.rt.close().await.expect("a close");
    });
}

struct Behind(i64);

#[async_trait(?Send)]
impl HereDaemon for Behind {
    async fn version(&self) -> Result<u32, wsp_words::exit::HostError> {
        Ok(self.0 as u32)
    }
    fn fix(&self) -> String {
        "npm i -g @wsp-labs/wsp".into()
    }
}

// "a thread on another branch" > "is refused in one sentence when the daemon binary beside this host is behind, and the binary is never run"
#[test]
fn is_refused_in_one_sentence_when_the_daemon_binary_beside_this_host_is_behind_and_the_binary_is_never_run() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts { here_daemon: Some(Rc::new(Behind(DAEMON_VERSION - 1))), ..Default::default() });
        let project = add(&h, &s.repo()).await;
        let refused = h.fx.rt.workspaces().folder_for(ask(&id_of(&project), Some("feat/x"), None), None).await.expect_err("a refusal");
        let want = format!(
            "this computer's wsp daemon is version {} and this wsp needs {DAEMON_VERSION}; npm i -g @wsp-labs/wsp stages the right one",
            DAEMON_VERSION - 1
        );
        assert!(refused.message.contains(&want), "{}", refused.message);
        assert!(worktrees_asked(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on another branch" > "is refused where the host found no daemon binary, and the project folder still takes threads"
#[test]
fn is_refused_where_the_host_found_no_daemon_binary_and_the_project_folder_still_takes_threads() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts { copier: Some(None), ..Default::default() });
        let project = add(&h, &s.repo()).await;
        let refused = h.fx.rt.workspaces().folder_for(ask(&id_of(&project), Some("feat/x"), None), None).await.expect_err("a refusal");
        assert!(refused.message.contains(wsp_core::types::wiring::NO_COPIER_HERE), "{}", refused.message);
        assert!(ws_json(&folder_for(&h, ask(&id_of(&project), None, None)).await)["worktree"].is_null());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on another branch" > "runs in the project folder when the branch is the one the folder has checked out"
#[test]
fn runs_in_the_project_folder_when_the_branch_is_the_one_the_folder_has_checked_out() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        let project = add(&h, &folder).await;
        let at = folder_for(&h, ask(&id_of(&project), Some("main"), None)).await;
        assert_eq!(ws_json(&at)["folder"], path_of(&folder).as_str());
        assert!(ws_json(&at)["worktree"].is_null());
        assert!(worktrees_asked(&h).is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// "a thread on another branch" > "is refused on a folder git holds no repo in, which is still a project"
#[test]
fn is_refused_on_a_folder_git_holds_no_repo_in_which_is_still_a_project() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.dir();
        let project = add(&h, &folder).await;
        assert!(project["git"].is_null());
        let refused = h.fx.rt.workspaces().folder_for(ask(&id_of(&project), Some("x"), None), None).await.expect_err("a refusal");
        let want = no_branches_line(project["name"].as_str().expect("a name"));
        assert!(refused.message.contains(&want), "{}", refused.message);
        assert_eq!(ws_json(&folder_for(&h, ask(&id_of(&project), None, None)).await)["folder"], path_of(&folder).as_str());
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a folder a start names =====

// "a folder a start names" > "runs inside the project folder, and in a worktree the person made, which is recorded as not wsp's"
#[test]
fn runs_inside_the_project_folder_and_in_a_worktree_the_person_made_which_is_recorded_as_not_wsps() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let folder = s.repo();
        std::fs::create_dir(folder.join("src")).expect("a folder");
        let project = add(&h, &folder).await;
        let id = id_of(&project);
        let inside = folder_for(&h, ask(&id, None, Some(&folder.join("src")))).await;
        assert_eq!(inside.cwd, Some(path_of(&folder.join("src"))));
        assert!(ws_json(&inside)["worktree"].is_null());
        let theirs = s.dir().join("by-hand");
        common::daemon::git(&folder, &["worktree", "add", "-q", "-b", "by-hand", &path_of(&theirs)]);
        let there = folder_for(&h, ask(&id, None, Some(&theirs))).await;
        assert_eq!(ws_json(&there)["worktree"], json!({ "path": path_of(&theirs), "branch": "by-hand", "made": false }));
        assert_eq!(there.cwd, Some(path_of(&theirs)));
        h.fx.rt.close().await.expect("a close");
    });
}

// "a folder a start names" > "is refused outside the project and its worktrees, in one sentence"
#[test]
fn is_refused_outside_the_project_and_its_worktrees_in_one_sentence() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let elsewhere = s.dir();
        let refused = h.fx.rt.workspaces().folder_for(ask(&id_of(&project), None, Some(&elsewhere)), None).await.expect_err("a refusal");
        let want = cwd_outside_line(&path_of(&elsewhere), project["name"].as_str().expect("a name"));
        assert!(refused.message.contains(&want), "{}", refused.message);
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a subfolder of a repo added as a project =====

// "a subfolder of a repo added as a project" > "records the repo's top, and its threads run in the subfolder, in a worktree too"
#[test]
fn records_the_repos_top_and_its_threads_run_in_the_subfolder_in_a_worktree_too() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let top = s.repo();
        std::fs::create_dir_all(top.join("apps").join("web")).expect("a folder");
        let project = add(&h, &top.join("apps").join("web")).await;
        assert_eq!(project["git"], json!({ "top": path_of(&top) }));
        let id = id_of(&project);
        let at = folder_for(&h, ask(&id, Some("feat/web"), None)).await;
        assert_eq!(ws_json(&at)["folder"], path_of(&h.worktrees.join(&id).join("feat-web").join("apps").join("web")).as_str());
        let home = folder_for(&h, ask(&id, None, None)).await;
        ran(&h, &home.workspace.id, "hi").await;
        assert_eq!(h.agents.cwds()[0], Some(path_of(&top.join("apps").join("web"))));
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== the branch a new thread of a project here starts on =====

// "the branch a new thread of a project here starts on" > "is the one its folder has checked out now, read again on each ask, for a repo, a worktree and a subfolder"
#[test]
fn is_the_one_its_folder_has_checked_out_now_read_again_on_each_ask_for_a_repo_a_worktree_and_a_subfolder() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let top = s.repo();
        let g = |args: &[&str]| common::daemon::git(&top, args);
        g(&["switch", "-q", "-c", "feature-x"]);
        std::fs::create_dir_all(top.join("apps").join("web")).expect("a folder");
        let tree = s.dir().join("tree");
        g(&["worktree", "add", "-q", "-b", "wt-branch", &path_of(&tree)]);
        let whole = add(&h, &top).await;
        let sub = add(&h, &top.join("apps").join("web")).await;
        let other = add(&h, &tree).await;
        let branch = |reference: String| {
            let rt = h.fx.rt.clone();
            async move { j(&rt.projects().branch(&reference, None).await.expect("a branch")) }
        };
        // The record keeps the remote's default; the folder is elsewhere.
        assert_ne!(whole["defaultBranch"], "feature-x");
        assert_eq!(branch(id_of(&whole)).await, json!({ "branch": "feature-x", "folder": true }));
        assert_eq!(branch(id_of(&sub)).await, json!({ "branch": "feature-x", "folder": true }));
        assert_eq!(branch(id_of(&other)).await, json!({ "branch": "wt-branch", "folder": true }));
        g(&["switch", "-q", "main"]);
        assert_eq!(branch(whole["name"].as_str().expect("a name").to_string()).await, json!({ "branch": "main", "folder": true }));
        g(&["switch", "-q", "--detach"]);
        assert_eq!(branch(id_of(&whole)).await, json!({ "branch": null, "folder": true }));
        h.fx.rt.close().await.expect("a close");
    });
}

// "the branch a new thread of a project here starts on" > "is none on a folder git holds no repo in"
#[test]
fn is_none_on_a_folder_git_holds_no_repo_in() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.dir()).await;
        assert_eq!(
            j(&h.fx.rt.projects().branch(&id_of(&project), None).await.expect("a branch")),
            json!({ "branch": null, "folder": true })
        );
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== the folder Claude Code keys a thread's memory to =====

/// Every launch the real adapter makes, its command and its environment, answered at once.
struct Launches {
    seen: Rc<RefCell<Vec<(String, Env)>>>,
}

struct Told(Vec<String>);

impl ExecStream for Told {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        Box::pin(futures::stream::iter(self.0.clone().into_iter().map(Ok)))
    }
    fn run(&self) -> Option<String> {
        Some("/tmp/run".into())
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, _line: String) -> futures::future::LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Written) }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> SharedFuture<Option<i32>> {
        async { Some(0) }.boxed_local().shared()
    }
}

impl ExecStreamFactory for Launches {
    fn launch(&self, command: &str, o: ExecOptions) -> Arc<dyn ExecStream> {
        if !command.contains("--session-id") {
            return Arc::new(Told(vec![]));
        }
        self.seen.borrow_mut().push((command.to_string(), o.env.clone()));
        let session = command.split("--session-id ").nth(1).and_then(|rest| rest.split_whitespace().next()).unwrap_or_default().to_string();
        Arc::new(Told(vec![
            json!({ "type": "system", "subtype": "init", "session_id": session, "tools": [] }).to_string(),
            json!({ "type": "result", "subtype": "success", "is_error": false, "result": "ok", "session_id": session, "usage": { "input_tokens": 1, "output_tokens": 1 } })
                .to_string(),
        ]))
    }
}

/// The adapter minus the roads the runtime hands the machine's own exec, which would run the real binary.
struct Stripped(Arc<dyn HarnessAdapter>);

impl HarnessAdapter for Stripped {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.0.start(options)
    }
    fn steers(&self) -> bool {
        self.0.steers()
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.0.attachments()
    }
    fn forks(&self) -> bool {
        self.0.forks()
    }
    fn forks_by_count(&self) -> bool {
        self.0.forks_by_count()
    }
    fn fork_session(
        &self,
        session: String,
        turn: ForkTurn,
    ) -> Option<futures::future::LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        self.0.fork_session(session, turn)
    }
}

/// The Claude adapter as the runtime wires it, over an exec that records each turn's launch and answers it at once.
fn recording_claude(seen: Rc<RefCell<Vec<(String, Env)>>>) -> HarnessAdapterFactory {
    let real = wsp_core::adapters::harness_adapters()["claude"].clone();
    Arc::new(move |ctx: HarnessAdapterContext| {
        let exec: Arc<dyn ExecStreamFactory> = Arc::new(Launches { seen: seen.clone() });
        let inner = real(HarnessAdapterContext { exec_stream: exec, ..ctx })?;
        Ok(Arc::new(Stripped(inner)) as Arc<dyn HarnessAdapter>)
    })
}

// "the folder Claude Code keys a thread's memory to" > "is a worktree's own for a thread in one wsp made, and the person's own for any other folder of the project"
#[test]
fn is_a_worktrees_own_for_a_thread_in_one_wsp_made_and_the_persons_own_for_any_other_folder_of_the_project() {
    drive(|| async {
        let s = Scratch::new();
        let launches: Rc<RefCell<Vec<(String, Env)>>> = Rc::default();
        let h = here(&s, HereOpts { adapters: vec![("claude", recording_claude(launches.clone()))], ..Default::default() });
        let top = s.repo();
        std::fs::create_dir_all(top.join("apps").join("web")).expect("a folder");
        let folder = add(&h, &top).await;
        let sub = add(&h, &top.join("apps").join("web")).await;
        let claude_home = path_of(&h.claude_home);
        // The key a launch names, read off its line and checked against its environment.
        let launched = |project: String, branch: Option<&'static str>, cwd: Option<PathBuf>| {
            let (h, launches, claude_home) = (&h, launches.clone(), claude_home.clone());
            async move {
                let at = folder_for(h, ask(&project, branch, cwd.as_deref())).await;
                let start = StartOptions {
                    prompt: "hi".into(),
                    harness: Some("claude".into()),
                    cwd: cwd.as_deref().map(path_of),
                    ..Default::default()
                };
                ran_in(h, &at.workspace.id, start).await;
                let (command, env) = launches.borrow().last().cloned().expect("a launch");
                let key = env["CLAUDE_CODE_PROJECT_DIR_NAME"].clone();
                assert!(
                    command.contains(&format!("--settings '{{\"autoMemoryDirectory\":\"{claude_home}/projects/{key}/memory\"}}'")),
                    "{command}"
                );
                key
            }
        };
        let key = |p: &Path| wsp_words::project_path::claude_project_key(&path_of(p));
        // A builder's worktree: its own key, never the one the person's sessions in the project folder keep.
        assert_eq!(launched(id_of(&folder), Some("feat/builder"), None).await, key(&h.worktrees.join(id_of(&folder)).join("feat-builder")));
        assert_eq!(launched(id_of(&sub), Some("feat/web"), None).await, key(&h.worktrees.join(id_of(&sub)).join("feat-web")));
        // The project folder, a subfolder project's own folder and a worktree the person made: the key Claude Code gives those folders in
        // the person's own terminal, its repo's main checkout.
        assert_eq!(launched(id_of(&folder), None, None).await, key(&top));
        assert_eq!(launched(id_of(&sub), None, None).await, key(&top));
        let theirs = s.dir().join("by-hand");
        common::daemon::git(&top, &["worktree", "add", "-q", "-b", "by-hand", &path_of(&theirs)]);
        assert_eq!(launched(id_of(&folder), None, Some(theirs)).await, key(&top));
        h.fx.rt.close().await.expect("a close");
    });
}

// ===== a rewind in a folder threads share =====

// "a rewind in a folder threads share" > "puts files back when no other thread ran there after the checkpoint, and only the conversation once one has"
#[test]
fn puts_files_back_when_no_other_thread_ran_there_after_the_checkpoint_and_only_the_conversation_once_one_has() {
    drive(|| async {
        let s = Scratch::new();
        let h = here(&s, HereOpts::default());
        let project = add(&h, &s.repo()).await;
        let at = folder_for(&h, ask(&id_of(&project), None, None)).await;
        let ws = at.workspace.id.clone();
        let first = ran(&h, &ws, "one").await;
        let thread = thread_of(&first);
        let second = ran_in(&h, &ws, StartOptions { prompt: "two".into(), thread: Some(thread.clone()), ..Default::default() }).await;
        ran_in(&h, &ws, StartOptions { prompt: "three".into(), thread: Some(thread.clone()), ..Default::default() }).await;
        until(|| async { h.ops().iter().filter(|o| *o == "git.checkpoint").count() == 3 }).await;
        let checkpoint = h.frames().into_iter().find(|f| f["op"] == "git.checkpoint").expect("a checkpoint");
        assert_matches(&checkpoint, json!({ "scope": ws, "thread": thread }));
        h.daemon.frames.borrow_mut().clear();
        let done =
            h.fx.rt
                .sessions()
                .rewind(&thread, RewindAsk { turn_id: Some(second.turn_id()), files: Some(true), ..Default::default() }, None)
                .await;
        assert_eq!(j(&done.expect("a rewind")), json!({ "turns": 1, "files": 1 }));
        assert_eq!(h.ops().iter().filter(|o| *o == "git.restore").count(), 1);
        // Another thread works in the same folder after the first turn.
        ran(&h, &ws, "beside").await;
        h.daemon.frames.borrow_mut().clear();
        let kept =
            h.fx.rt
                .sessions()
                .rewind(&thread, RewindAsk { turn_id: Some(first.turn_id()), files: Some(true), ..Default::default() }, None)
                .await;
        assert_eq!(j(&kept.expect("a rewind")), json!({ "turns": 1, "kept": REWIND_SHARED_LINE }));
        assert!(!h.ops().iter().any(|o| o == "git.restore"));
        h.fx.rt.close().await.expect("a close");
    });
}
