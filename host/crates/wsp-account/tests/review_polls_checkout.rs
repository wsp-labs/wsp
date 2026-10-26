// SPDX-License-Identifier: AGPL-3.0-only
//! Review probe: a tile's checkout answers while the pull request read it starts is still out, as the TS's
//! `void ctx.readPullRequest(entry, false)` and `void ctx.readTree(entry)` (workspaces.ts:802-804) do.
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

use futures::future::LocalBoxFuture;
use std::time::Duration;

use async_trait::async_trait;
use wsp_account::projects::folders::tree::{ListedWorktree, RemoveAsk};
use wsp_account::projects::workspaces::git::{PullRequestOn, SentTo};
use wsp_account::projects::workspaces::seam::{Base, DaemonWork, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld, Queued};
use wsp_core::clock::Clock;
use wsp_core::ids::Ids;
use wsp_core::types::api::FolderFor;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{RunningExec, WorkspaceRecord};
use wsp_wire::types::changes::ViewedMarks;
use wsp_wire::types::pull_request::{GitUpdateReply, PullRequestPage};
use wsp_wire::types::tree::TreeRecord;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;

/// The git fake, with a pull request read that the git host never answers.
struct Slow(Rc<git::FakeGit>);

#[async_trait(?Send)]
impl Base for Slow {
    async fn ready(&self) -> Result<(), HostError> {
        self.0.ready().await
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.0.clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.0.ids()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.0.store()
    }
    fn live(&self) -> Live {
        self.0.live()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.0.projects_held()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.0.project_held(project_id)
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.0.session_views()
    }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        self.0.thread_records()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.0.persist(record).await
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        self.0.view(record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.0.emit(event)
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.0.entry_of(id, origin, ask).await
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.0.spawn_guard(act, origin)
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        self.0.kind_of(computer)
    }
    fn place_name(&self, place_id: &str) -> String {
        self.0.place_name(place_id)
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.copy_blocked(entry).await
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<serde_json::Value, HostError> {
        self.0.with_daemon(entry, work).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<serde_json::Value, HostError> {
        self.0.on_this_computer(work).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        self.0.git_here(ask).await
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        self.0.checkout_of(record)
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.0.preferences().await
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        self.0.write_daemon_roots(entry, strict).await
    }
    fn home_dir(&self) -> String {
        self.0.home_dir()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        self.0.machine_id(entry)
    }
}

#[async_trait(?Send)]
impl GitCtx for Slow {
    async fn read_pull_request(&self, entry: &Entry, force: bool, whole: bool) -> Result<Option<Value>, HostError> {
        let _ = (entry, force, whole);
        futures::future::pending::<()>().await;
        unreachable!()
    }
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.worktree_mounted(entry).await
    }
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult, HostError> {
        self.0.machine_exec(entry, cmd, timeout_ms).await
    }
    async fn exec_stream_of(&self, entry: &Entry, argv: Vec<String>, cwd: Option<String>) -> Result<RunningExec, HostError> {
        self.0.exec_stream_of(entry, argv, cwd).await
    }
    async fn queued(&self, id: &str, work: Queued<serde_json::Value>) -> Result<serde_json::Value, HostError> {
        self.0.queued(id, work).await
    }
    async fn keep_tree(&self, entry: &Entry, tree: TreeRecord) -> Result<(), HostError> {
        self.0.keep_tree(entry, tree).await
    }
    async fn take_pull_request(&self, entry: &Entry, pr: serde_json::Value) -> Result<(), HostError> {
        self.0.take_pull_request(entry, pr).await
    }
    fn read_lead_of(&self, entry: &Entry) {
        self.0.read_lead_of(entry)
    }
    async fn folder_for(&self, ask: FolderForAsk, origin: Option<Caller>) -> Result<(Entry, Option<String>, Option<String>), HostError> {
        self.0.folder_for(ask, origin).await
    }
    fn take_setup_line(&self, id: &str) -> Option<String> {
        self.0.take_setup_line(id)
    }
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        self.0.resolve_project(word, origin).await
    }
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.0.refuse_recording(word, origin)
    }
    fn refuse_relayed(&self, record: &WorkspaceRecord, origin: Option<&Caller>) -> Result<(), HostError> {
        self.0.refuse_relayed(record, origin)
    }
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        self.0.project_folder(project).await
    }
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str, made_for: Option<String>) -> Result<Entry, HostError> {
        self.0.worktree_folder(project, top, branch, made_for).await
    }
    async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError> {
        self.0.worktrees_of(top).await
    }
    fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        self.0.folders_of(project_id)
    }
    async fn remove_worktree(&self, entry: &Entry, force: bool, ask: RemoveAsk) -> Result<(), HostError> {
        self.0.remove_worktree(entry, force, ask).await
    }
    async fn read_checkout(&self, entry: &Entry, fresh: bool) -> Result<Option<wsp_wire::types::changes::Checkout>, HostError> {
        self.0.read_checkout(entry, fresh).await
    }
    async fn read_tree(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.read_tree(entry).await
    }
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String {
        self.0.label_of(entry, thread_id)
    }
    async fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<serde_json::Value>, HostError> {
        self.0.open_thread(workspace_id, thread_id).await
    }
    fn default_agent_of(&self, entry: &Entry) -> String {
        self.0.default_agent_of(entry)
    }
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool {
        self.0.agent_off(entry, agent)
    }
    fn has_adapter(&self, agent: &str) -> bool {
        self.0.has_adapter(agent)
    }
    async fn drafter(&self, entry: &Entry, agent: &str) -> Result<Option<Option<String>>, HostError> {
        self.0.drafter(entry, agent).await
    }
    async fn put_files(&self, entry: &Entry, path: &str, text: &str, before: &[&str]) -> Result<ExecResult, HostError> {
        self.0.put_files(entry, path, text, before).await
    }
    async fn draft_for(&self, entry: &Entry, agent: &str, prompt_file: &str, model: Option<&str>) -> Result<Option<String>, HostError> {
        self.0.draft_for(entry, agent, prompt_file, model).await
    }
    fn viewed_marks(&self) -> Rc<std::cell::RefCell<indexmap::IndexMap<String, ViewedMarks>>> {
        self.0.viewed_marks()
    }
    async fn persist_sessions(&self, workspace_id: &str) -> Result<(), HostError> {
        self.0.persist_sessions(workspace_id).await
    }
    async fn pull_request_on(&self, workspace_id: &str, origin: Option<Caller>) -> Result<PullRequestOn, HostError> {
        self.0.pull_request_on(workspace_id, origin).await
    }
    async fn read_page(&self, entry: &Entry, remote: &str, number: i64, fresh: bool) -> Result<(PullRequestPage, bool), HostError> {
        self.0.read_page(entry, remote, number, fresh).await
    }
    async fn merge_settings(&self, remote: &str) -> Result<serde_json::Value, HostError> {
        self.0.merge_settings(remote).await
    }
    async fn read_host(&self, entry: &Entry, frame: wsp_account::account::pull_requests::FrameFor) -> Result<serde_json::Value, HostError> {
        self.0.read_host(entry, frame).await
    }
    async fn post_as_person(&self, remote: &str, number: i64, ask: serde_json::Value) -> Result<serde_json::Value, HostError> {
        self.0.post_as_person(remote, number, ask).await
    }
    async fn to_first_thread(&self, workspace_id: &str, prompt: &str, origin: Option<Caller>) -> Result<SentTo, HostError> {
        self.0.to_first_thread(workspace_id, prompt, origin).await
    }
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str, origin: Option<Caller>) -> Result<SentTo, HostError> {
        self.0.send_detached(workspace_id, prompt, thread, origin).await
    }
    async fn child_of(
        &self,
        lead: &Entry,
        child: &str,
        origin: Option<Caller>,
        lead_thread: Option<&str>,
        child_thread: Option<&str>,
    ) -> Result<Entry, HostError> {
        self.0.child_of(lead, child, origin, lead_thread, child_thread).await
    }
    async fn update_copy(&self, entry: &Entry) -> Result<GitUpdateReply, HostError> {
        self.0.update_copy(entry).await
    }
    fn drop_page_cache(&self, remote: &str, number: i64) {
        self.0.drop_page_cache(remote, number)
    }
    fn is_local_workspace(&self, record: &WorkspaceRecord) -> bool {
        self.0.is_local_workspace(record)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        self.0.spawn(f)
    }
}

#[test]
fn review_checkout_answers_while_the_pull_request_read_is_out() {
    let w = world(json!({}));
    let slow: Rc<dyn GitCtx> = Rc::new(Slow(w.fake.clone()));
    let answered = run(async { tokio::time::timeout(Duration::from_secs(2), area::checkout(&slow, "ws1", origin(), false)).await });
    assert!(answered.is_ok(), "the checkout waited on the git host's pull request read; the TS answers without it");
}
