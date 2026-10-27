// SPDX-License-Identifier: AGPL-3.0-only
//! The context's phases and git halves of the workspaces area (`PhasesCtx`, workspaces.ts:22-65, 289-645, and `GitCtx`,
//! workspaces.ts:647-1053): each member is a forward to the area that holds it, a read of the entry's engine
//! workspace or machine, or a call of an engine function. `exec_stream_of` is the one body of its own.
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{select, Either, LocalBoxFuture};
use futures::stream::LocalBoxStream;
use futures::{FutureExt as _, StreamExt as _};
use serde_json::Value;
use wsp_account::account::core_types::AccountError;
use wsp_account::account::pull_requests::FrameFor;
use wsp_account::projects::folders::place::FolderAsk;
use wsp_account::projects::folders::tree::RemoveAsk;
use wsp_account::projects::workspaces::git::{GitCtx, PullRequestOn, SentTo};
use wsp_account::projects::workspaces::phases::{MachineRead, PhasesCtx, ResumeAsks};
use wsp_account::projects::workspaces::seam::Entry;
use wsp_adapters::port::{AdapterError, DraftAsk, ExecFailure, ExecFailureKind, ExecOptions, ExecStream, SharedFuture, WriteOutcome};
use wsp_core::types::internal::label_of;
use wsp_core::types::wiring::{settled, RunningExec, SizeAsk, WorkspaceRecord};
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::exec_detached::put::{put_files, GuestWrite, PutFilesOptions};
use wsp_engine::gone::{machine_state_word, read_gone};
use wsp_engine::lifecycle::WakeResult;
use wsp_engine::machine::{ExecOptions as MachineExecOptions, Machine, MachineState};
use wsp_machines::machines::area_error::AreaError;
use wsp_threads::threads::agents::launch::AdapterAsk;
use wsp_threads::threads::agents::lists;
use wsp_wire::views::workspace::{Caller, ProjectView, ReachState, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::Capabilities;
use wsp_wire::wire::machine_link::{ExecResult, MachineShape};
use wsp_words::exit::HostError;
use wsp_words::words::thread::{gone_words, GoneSeenBy, GoneSighting};
use wsp_words::workspace_state::{is_local_workspace, kind_words, machine_word};

use super::Ctx;
use crate::runtime::core::{reads_state, took_the_resume};
use crate::runtime::join::spawn_detached;

/// A refusal the areas raise, handed on as the engine error a lifecycle call raises, with the kind, status and code it
/// carries so `isMissing` reads it the same.
fn engine_said(e: HostError) -> EngineError {
    let kind = e.kind.as_ref().and_then(|kind| serde_json::from_value::<ErrorKind>(Value::String(kind.as_str().to_string())).ok());
    EngineError::Refused { message: e.message, kind, status: e.status.map(|s| s as u16), code: e.code }
}

/// A TS `throw` out of an area the account crate wraps.
fn said(e: AccountError) -> HostError {
    e.into()
}

fn read_of(state: MachineState) -> MachineRead {
    match state {
        MachineState::Running => MachineRead::Running,
        MachineState::Paused => MachineRead::Paused,
        MachineState::Gone => MachineRead::Gone,
        _ => MachineRead::Other,
    }
}

fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn machine_of(entry: &Entry) -> Arc<dyn Machine> {
    entry.borrow().machine.clone()
}

impl Ctx {
    /// The module of the entry's kind, where the host knows the kind.
    fn phases_module(&self, entry: &Entry) -> Option<Rc<dyn wsp_core::types::internal::KindModule>> {
        self.kinds.get().module_of(record_of(entry).kind).ok()
    }

    fn phases_backend(&self, entry: &Entry) -> Result<Arc<dyn wsp_engine::machine::MachineBackend>, HostError> {
        self.kinds.get().backend_for(&record_of(entry))
    }

    /// `launchAdapterFor(entry, named)`: the adapter that agent runs here with.
    async fn phases_adapter(&self, entry: &Entry, agent: &str) -> Result<wsp_threads::threads::agents::launch::BuiltAdapter, HostError> {
        self.agents.get().launch_adapter_for(entry, AdapterAsk { named: Some(agent.to_string()), ..AdapterAsk::default() }).await
    }
}

#[async_trait(?Send)]
impl PhasesCtx for Ctx {
    async fn unfork(&self, entry: &Entry) -> Result<(), EngineError> {
        self.machines.get().unfork(entry).await.map_err(engine_said)
    }
    async fn sub_lost(&self, entry: &Entry, line: &str) -> Result<(), HostError> {
        self.sub_life.get().sub_lost(entry, line).await.map(|_| ()).map_err(AreaError::into_host)
    }
    fn tell_threads(&self, workspace_id: &str, line: &str) {
        self.reach.get().tell_threads(workspace_id, &|_| line.to_string());
    }
    async fn emit_status(&self, entry: &Entry, reach: &str, words: Option<&str>) -> Result<(), HostError> {
        let state: ReachState = serde_json::from_value(Value::String(reach.to_string())).map_err(|e| HostError::new(e.to_string()))?;
        self.reach.get().emit_status(entry, state, words).await.map_err(AreaError::into_host)
    }
    fn follow_machine(&self, entry: &Entry) {
        self.machines.get().follow_machine(entry);
    }
    fn reach_of(&self, entry: &Entry) -> String {
        let state = self.reach.get().reach_of(entry).unwrap_or(ReachState::Unsupported);
        serde_json::to_value(state).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default()
    }
    async fn nap_with(&self, id: &str) -> Result<WorkspaceView, HostError> {
        self.machines.get().nap_with(id, None).await
    }
    fn refuse_pauseless(&self, entry: &Entry, act: &str) -> Result<(), HostError> {
        self.rules.get().refuse_pauseless(entry, act)
    }
    fn talks_to_tree_on(&self, id: &str, origin: Option<&Caller>) -> Result<bool, HostError> {
        self.boot.get().talks_to_tree_on(id, origin)
    }
    fn refuse_sub_wake(&self, entry: &Entry) -> Result<(), HostError> {
        self.sub_life.get().refuse_sub_wake(entry)
    }
    async fn recover_gone(&self, entry: &Entry) -> Result<Option<WorkspacePhase>, HostError> {
        self.machines.get().recover_gone(entry).await
    }
    async fn machine_state(&self, entry: &Entry) -> Result<MachineRead, EngineError> {
        Ok(read_of(machine_of(entry).state().await?))
    }
    fn provider_said(&self, e: &EngineError) -> String {
        wsp_core::status::provider_said(e)
    }
    async fn settle_gone(&self, entry: &Entry, by: &str, at: i64, answer: Option<String>) -> Result<bool, HostError> {
        let by: GoneSeenBy = serde_json::from_value(Value::String(by.to_string())).map_err(|e| HostError::new(e.to_string()))?;
        let machine_id = record_of(entry).machine_id;
        let words = gone_words(&machine_id, Some(&GoneSighting { by, at: at as f64, answer }));
        Ok(settled(self.machines.get().settle_gone(entry, &words).await?))
    }
    async fn adopt_pause(&self, entry: &Entry) -> Result<(), HostError> {
        self.machines.get().adopt_pause(entry).await
    }
    async fn runs_under_napping(&self, entry: &Entry) -> Result<bool, HostError> {
        Ok(self.machines.get().runs_under_napping(entry).await)
    }
    async fn adopt_running(&self, entry: &Entry) -> Result<(), HostError> {
        self.machines.get().adopt_running(entry).await
    }
    async fn proven(&self, entry: &Entry) -> Result<WorkspaceView, HostError> {
        self.machines.get().proven(entry).await
    }
    fn resume_asks(&self, entry: &Entry) -> Option<ResumeAsks> {
        let lifecycle = self.kinds.get().lifecycle_of(&entry.borrow()).ok()?;
        lifecycle.budgets().resume_asks.map(|asks| ResumeAsks { for_ms: asks.for_ms.0 as i64, every_ms: asks.every_ms.0 as i64 })
    }
    async fn sleeps(&self, ms: i64) {
        wsp_machines::machines::kinds::KindsCtx::sleep(self, ms as f64).await;
    }
    async fn took_the_resume(&self, entry: &Entry) -> bool {
        took_the_resume(reads_state(&*machine_of(entry), &self.clock, self.defaults.provider_read_ms).await)
    }
    async fn ws_wake(&self, entry: &Entry, landed: bool) -> Result<WakeResult, EngineError> {
        let ws = entry.borrow().ws.clone();
        ws.wake(landed).await
    }
    async fn ws_rebuild(&self, entry: &Entry) -> Result<(), EngineError> {
        let ws = entry.borrow().ws.clone();
        ws.rebuild().await
    }
    async fn ws_upgrade(&self, entry: &Entry) -> Result<(), EngineError> {
        let ws = entry.borrow().ws.clone();
        ws.upgrade(Default::default()).await
    }
    fn ws_current_phase(&self, entry: &Entry) -> WorkspacePhase {
        match entry.borrow().ws.current_phase() {
            wsp_engine::lifecycle::WorkspacePhase::Running => WorkspacePhase::Running,
            wsp_engine::lifecycle::WorkspacePhase::Napping => WorkspacePhase::Napping,
            wsp_engine::lifecycle::WorkspacePhase::Waking => WorkspacePhase::Waking,
        }
    }
    fn ws_note_paused(&self, entry: &Entry) {
        entry.borrow().ws.note_paused();
    }
    fn arm_late_read(&self, entry: &Entry) {
        self.machines.get().arm_late_read(entry);
    }
    fn can_resume_at(&self, entry: &Entry) -> bool {
        machine_of(entry).offers().resume_at
    }
    fn provider_of(&self, entry: &Entry) -> Option<String> {
        self.channels.get().provider_of(&record_of(entry))
    }
    async fn backend_capabilities(&self, entry: &Entry) -> Result<Capabilities, HostError> {
        let backend = self.phases_backend(entry)?;
        let _ = backend.limits().await;
        Ok(backend.capabilities())
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        self.threads.get().thread_runs(thread_id)
    }
    async fn shape_of(&self, entry: &Entry) -> Option<MachineShape> {
        self.records.get().shape_of(machine_of(entry)).await
    }
    fn size_built(&self, shape: Option<&MachineShape>, asked: SizeAsk) -> SizeAsk {
        let built =
            self.records.get().size_built(shape, &wsp_wire::wire::capabilities::WorkspaceSize { cpu: asked.cpu, mem_mb: asked.mem_mb });
        SizeAsk { cpu: built.cpu, mem_mb: built.mem_mb }
    }
    async fn status_now(&self, entry: &Entry) -> Result<(), HostError> {
        self.reach.get().status_now(entry).await.map_err(AreaError::into_host)
    }
    fn refuse_cannot(&self, entry: &Entry, cap: &str, act: &str) -> Result<(), HostError> {
        self.rules.get().refuse_cannot(entry, cap, act)
    }
    fn keeps_images(&self, entry: &Entry) -> bool {
        self.phases_backend(entry).is_ok_and(|backend| self.kinds.get().keeps_images(&*backend))
    }
    fn pause_keeps_disk(&self, entry: &Entry) -> bool {
        self.phases_backend(entry).is_ok_and(|backend| self.kinds.get().pause_keeps_disk(&*backend))
    }
    fn name_given(&self, name: &str) -> String {
        wsp_machines::machines::create::CreateArea::name_given(name)
    }
    fn name_refusal(&self, name: &str) -> Option<String> {
        self.create.get().name_refusal(name)
    }
    fn put_look(&self, entry: &Entry, key: &str, value: Option<Value>) {
        let key = match key {
            "theme" => wsp_machines::images::records::LookKey::Theme,
            "glyph" => wsp_machines::images::records::LookKey::Glyph,
            _ => return,
        };
        let value = value.map(|v| if v.is_null() { None } else { Some(v) });
        let live = entry.borrow();
        let mut record = live.record.borrow_mut();
        self.records.get().put_look(&mut record, key, value);
    }
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        let channels = self.channels.get();
        channels.served_by_its_computer(entry).map(|_| channels.computer_of(entry))
    }
    fn computer_of(&self, entry: &Entry) -> String {
        self.channels.get().computer_of(entry)
    }
    fn has_deploy_daemon(&self, entry: &Entry) -> bool {
        self.phases_module(entry).is_some_and(|module| module.deploy_daemon(&entry.borrow()).is_some())
    }
    async fn deploy_daemon_on(&self, entry: &Entry) -> Result<(), HostError> {
        self.daemon.get().deploy_daemon_on(entry).await.map(|_| ())
    }
    async fn delete_queued(&self, id: &str) -> Option<Result<(), HostError>> {
        let held = self.sub_queue.get().delete_queued(id)?;
        held.await;
        Some(Ok(()))
    }
    async fn refuse_changed(&self, path: &str) -> Result<(), HostError> {
        self.folders.get().refuse_changed(path).await
    }
    fn path_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    async fn cut_off(&self, entry: &Entry) -> Result<(), HostError> {
        let record = record_of(entry);
        wsp_core::pipe::area::cut_off(self, &record.id, &record.name).await;
        Ok(())
    }
    fn end_sessions(&self, id: &str, reason: &str) {
        self.machines.get().end_sessions(id, reason);
    }
    async fn drop_checkpoints(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError> {
        self.folders.get().drop_checkpoints(entry, thread_id).await
    }
    async fn remove_worktree_ending(&self, entry: &Entry) -> Result<(), HostError> {
        self.folders.get().remove_worktree(entry, false, RemoveAsk { ending: true, ..RemoveAsk::default() }).await
    }
    async fn drop_thread_files(&self, entry: &Entry, thread_ids: Vec<String>) -> Result<(), HostError> {
        self.agents.get().drop_thread_files(entry, &thread_ids).await;
        Ok(())
    }
    fn driven(&self, entry: &Entry) -> bool {
        kind_words(&record_of(entry).kind).driven
    }
    async fn machine_kill(&self, entry: &Entry) -> Result<(), EngineError> {
        machine_of(entry).kill().await
    }
    async fn drop_workspace(&self, id: &str) -> Result<(), HostError> {
        self.machines.get().drop(id).await
    }
    async fn read_gone(&self, entry: &Entry) -> Result<String, HostError> {
        let backend = self.phases_backend(entry)?;
        let state = read_gone(&*backend, machine_of(entry).id()).await?;
        Ok(machine_state_word(state).to_string())
    }
    fn idle_touch(&self, id: &str) {
        self.idle.get().touch(id);
    }
    fn has_restart_daemon(&self, entry: &Entry) -> bool {
        self.phases_module(entry).is_some_and(|module| module.restart_daemon(&entry.borrow()).is_some())
    }
    async fn restart_daemon_of(&self, entry: &Entry) -> Result<(), HostError> {
        let record = record_of(entry);
        let module = self.kinds.get().module_of(record.kind)?;
        let restart = module.restart_daemon(&entry.borrow());
        match restart {
            Some(restart) => restart.await,
            None => Err(HostError::new(format!(
                "{}'s daemon runs on {}, which this host does not hold the process of",
                record.name,
                machine_word(&record.kind)
            ))),
        }
    }
    fn shared_daemon(&self, entry: &Entry) -> bool {
        self.phases_module(entry).is_some_and(|module| module.shared_daemon())
    }
    fn shared_daemon_replaced(&self) {
        self.channels.get().shared_daemon_replaced();
    }
    fn forget_polled_reach(&self, id: &str) {
        self.reach.get().polled_reach.borrow_mut().shift_remove(id);
    }
    async fn push_status(&self, entry: &Entry) -> Result<(), HostError> {
        self.reach.get().push_status(entry).await.map_err(AreaError::into_host)
    }
    fn forget_failed_create(&self, id: &str) -> bool {
        self.create.get().failed_creates.borrow_mut().shift_remove(id).is_some()
    }
}

/// The stream `exec_stream_of` hands back: the inner run with the reason the machine going away ends it by, which
/// reaches the reader as soon as it is known and not one poll late (workspaces.ts:671-690).
struct HeldExec {
    inner: Arc<dyn ExecStream>,
    ended: futures::future::Shared<LocalBoxFuture<'static, String>>,
    held: Rc<RefCell<indexmap::IndexMap<u64, super::ExecHeld>>>,
    key: u64,
}

impl ExecStream for HeldExec {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        let (ended, held, key) = (self.ended.clone(), self.held.clone(), self.key);
        futures::stream::unfold(Some(self.inner.lines()), move |state| {
            let (ended, held) = (ended.clone(), held.clone());
            async move {
                let mut inner = state?;
                match select(inner.next(), ended).await {
                    Either::Left((Some(line), _)) => Some((line, Some(inner))),
                    Either::Left((None, _)) => {
                        held.borrow_mut().shift_remove(&key);
                        None
                    }
                    Either::Right((reason, _)) => {
                        held.borrow_mut().shift_remove(&key);
                        Some((Err(ExecFailure { kind: ExecFailureKind::Other, message: reason }), None))
                    }
                }
            }
        })
        .boxed_local()
    }
    fn run(&self) -> Option<String> {
        self.inner.run()
    }
    fn pid(&self) -> Option<u32> {
        self.inner.pid()
    }
    fn teardown(&self) {
        self.inner.teardown();
    }
    fn kill(&self) {
        self.inner.kill();
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        self.inner.write(line)
    }
    fn close_input(&self) {
        self.inner.close_input();
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        let (inner, ended) = (self.inner.exited(), self.ended.clone());
        async move {
            match select(inner, ended).await {
                Either::Left((code, _)) => code,
                Either::Right(_) => None,
            }
        }
        .boxed_local()
        .shared()
    }
}

#[async_trait(?Send)]
impl GitCtx for Ctx {
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        self.folders.get().worktree_mounted(entry).await
    }
    async fn machine_exec(&self, entry: &Entry, cmd: &str, timeout_ms: Option<u64>) -> Result<ExecResult, HostError> {
        let options = MachineExecOptions { timeout_ms, ..MachineExecOptions::default() };
        Ok(machine_of(entry).exec(cmd, options).await?)
    }
    async fn exec_stream_of(&self, entry: &Entry, argv: Vec<String>, cwd: Option<String>) -> Result<RunningExec, HostError> {
        let built = self.agents.get().launch_adapter_for(entry, AdapterAsk::default()).await?;
        let ran_in =
            self.kinds.get().thread_folder(&entry.borrow(), cwd.as_deref()).now_or_never().expect("thread_folder does not await")?;
        let unbounded = wsp_core::types::pending::MachineExecOptions {
            idle_ms: Some(f64::INFINITY),
            deadline_ms: Some(f64::INFINITY),
            ..Default::default()
        };
        let factory = self.kinds.get().exec_factory_for(&entry.borrow(), Some(unbounded), None)?;
        let command = wsp_adapters::pending::in_folder(
            Some(&ran_in),
            &argv.iter().map(|word| wsp_adapters::pending::shell_quote(word)).collect::<Vec<_>>().join(" "),
        );
        let inner = factory.launch(&command, ExecOptions { env: built.adapter.env().unwrap_or_default(), ..ExecOptions::default() });
        let (say, reason) = futures::channel::oneshot::channel::<String>();
        let ended = async move {
            match reason.await {
                Ok(reason) => reason,
                Err(_) => futures::future::pending().await,
            }
        }
        .boxed_local()
        .shared();
        let key = self.exec_seq.get();
        self.exec_seq.set(key + 1);
        let end: Rc<dyn Fn(&str)> = {
            let (say, inner) = (Cell::new(Some(say)), inner.clone());
            Rc::new(move |reason: &str| {
                if let Some(say) = say.take() {
                    let _ = say.send(reason.to_string());
                }
                inner.kill();
            })
        };
        self.execs.borrow_mut().insert(key, super::ExecHeld { workspace_id: record_of(entry).id, end });
        let stream = Arc::new(HeldExec { inner, ended, held: self.execs.clone(), key });
        Ok(RunningExec { stream, ran_in: Some(ran_in) })
    }
    async fn queued(&self, id: &str, work: wsp_account::projects::workspaces::seam::Queued<Value>) -> Result<Value, HostError> {
        self.folders.get().queued(id, work).await
    }
    async fn keep_tree(&self, entry: &Entry, tree: wsp_wire::types::tree::TreeRecord) -> Result<(), HostError> {
        self.pull_requests.get().keep_tree(entry, tree).await.map_err(said)
    }
    async fn take_pull_request(&self, entry: &Entry, pr: Value) -> Result<(), HostError> {
        let seen = if pr.is_null() { None } else { Some(serde_json::from_value(pr).map_err(|e| HostError::new(e.to_string()))?) };
        self.pull_requests.get().take_pull_request(entry, seen).await.map_err(said)
    }
    fn read_lead_of(&self, entry: &Entry) {
        self.reach.get().read_lead_of(entry);
    }
    async fn folder_for(
        &self,
        ask: wsp_core::types::api::FolderForAsk,
        origin: Option<Caller>,
    ) -> Result<(Entry, Option<String>, Option<String>), HostError> {
        let ask = FolderAsk { project: ask.project, branch: ask.branch, cwd: ask.cwd, picks: ask.picks, opens: ask.opens };
        let placed = self.folders.get().folder_for(ask, origin).await?;
        Ok((placed.entry, placed.cwd, placed.setup))
    }
    fn take_setup_line(&self, id: &str) -> Option<String> {
        self.folders.get().take_setup_line(id)
    }
    async fn resolve_project(&self, word: &str, origin: Option<Caller>) -> Result<ProjectView, HostError> {
        self.projects.get().resolve(word, origin).await
    }
    fn refuse_recording(&self, word: &str, origin: Option<&Caller>) -> Result<(), HostError> {
        self.rules.get().refuse_recording(word, origin)
    }
    fn refuse_relayed(&self, record: &WorkspaceRecord, origin: Option<&Caller>) -> Result<(), HostError> {
        let like = wsp_account::account::rules::like_of(record);
        self.rules.get().refuse_relayed(Some(&like), origin, wsp_words::words::computer::AcrossAct::Work, None)
    }
    async fn project_folder(&self, project: &ProjectView) -> Result<Entry, HostError> {
        self.folders.get().project_folder(project).await
    }
    async fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str, made_for: Option<String>) -> Result<Entry, HostError> {
        self.folders.get().worktree_folder(project, top, branch, None, made_for, None).await
    }
    async fn worktrees_of(&self, top: &str) -> Result<Vec<wsp_account::projects::folders::tree::ListedWorktree>, HostError> {
        self.folders.get().worktrees_of(top).await
    }
    fn folders_of(&self, project_id: &str) -> Vec<Entry> {
        self.create.get().folders_of(project_id)
    }
    async fn remove_worktree(&self, entry: &Entry, force: bool, ask: RemoveAsk) -> Result<(), HostError> {
        self.folders.get().remove_worktree(entry, force, ask).await
    }
    async fn read_checkout(&self, entry: &Entry, fresh: bool) -> Result<Option<wsp_wire::types::changes::Checkout>, HostError> {
        Ok(self.reach.get().read_checkout(entry, fresh).await)
    }
    async fn read_pull_request(&self, entry: &Entry, force: bool, whole: bool) -> Result<Option<Value>, HostError> {
        let seen = self.pull_requests.get().read_pull_request(entry, force, whole).await;
        seen.map(|seen| serde_json::to_value(seen).map_err(|e| HostError::new(e.to_string()))).transpose()
    }
    async fn read_tree(&self, entry: &Entry) -> Result<(), HostError> {
        self.reach.get().read_tree(entry).await.map(|_| ())
    }
    fn label_of(&self, entry: &Entry, thread_id: Option<&str>) -> String {
        label_of(&record_of(entry).name, thread_id)
    }
    async fn open_thread(&self, workspace_id: &str, thread_id: &str) -> Result<Vec<Value>, HostError> {
        let events = self.transcripts.get().open_thread(workspace_id, thread_id)?;
        events.iter().map(|event| serde_json::to_value(event).map_err(|e| HostError::new(e.to_string()))).collect()
    }
    fn default_agent_of(&self, entry: &Entry) -> String {
        let prefs =
            self.state.with(|state| state.preferences_held.clone()).unwrap_or_else(wsp_wire::views::preferences::default_preferences);
        self.agents.get().default_agent_of(&prefs, Some(entry))
    }
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool {
        self.agents.get().agent_off(entry, agent)
    }
    fn has_adapter(&self, agent: &str) -> bool {
        self.opts.adapters.contains_key(agent)
    }
    async fn drafter(&self, entry: &Entry, agent: &str) -> Result<Option<Option<String>>, HostError> {
        let built = self.phases_adapter(entry, agent).await?;
        let drafts = built.adapter.draft_for(DraftAsk::default(), lists::harness_exec_on(machine_of(entry), 0)).is_some();
        if !drafts {
            return Ok(None);
        }
        let model = match wsp_core::harness_catalog::harness_catalog(&built.harness) {
            None => None,
            Some(table) => {
                let catalog = self.agents.get().catalog_on(&table, entry, built.adapter.clone()).await;
                wsp_core::harness_catalog::smallest_model(Some(&catalog))
            }
        };
        Ok(Some(model))
    }
    async fn put_files(&self, entry: &Entry, path: &str, text: &str, before: &[&str]) -> Result<ExecResult, HostError> {
        let files = [GuestWrite { path: path.to_string(), text: text.to_string(), append: false }];
        let options = PutFilesOptions { before: before.iter().map(|line| line.to_string()).collect(), ..PutFilesOptions::default() };
        let put = put_files(&*machine_of(entry), &files, options).await?;
        Ok(ExecResult { exit_code: put.exit_code.into(), stdout: put.stdout, stderr: put.stderr })
    }
    async fn draft_for(&self, entry: &Entry, agent: &str, prompt_file: &str, model: Option<&str>) -> Result<Option<String>, HostError> {
        let built = self.phases_adapter(entry, agent).await?;
        let ask = DraftAsk { prompt_file: prompt_file.to_string(), model: model.map(str::to_string) };
        let exec = lists::harness_exec_on(machine_of(entry), wsp_core::types::events::TITLE_MAKE_TIMEOUT_MS);
        match built.adapter.draft_for(ask, exec) {
            Some(answer) => answer.await.map_err(|e| HostError::new(e.message)),
            None => Err(HostError::new(AdapterError("draftFor is not a function".into()).to_string())),
        }
    }
    fn viewed_marks(&self) -> Rc<RefCell<indexmap::IndexMap<String, indexmap::IndexMap<String, String>>>> {
        self.transcripts.get().viewed_marks.clone()
    }
    async fn persist_sessions(&self, workspace_id: &str) -> Result<(), HostError> {
        self.transcripts.get().persist_sessions(workspace_id).await;
        Ok(())
    }
    async fn pull_request_on(&self, workspace_id: &str, origin: Option<Caller>) -> Result<PullRequestOn, HostError> {
        let (entry, remote, number) = self.pull_requests.get().pull_request_on(workspace_id, origin.as_ref()).await?;
        Ok(PullRequestOn { entry, remote, number })
    }
    async fn read_page(
        &self,
        entry: &Entry,
        remote: &str,
        number: i64,
        fresh: bool,
    ) -> Result<(wsp_wire::types::pull_request::PullRequestPage, bool), HostError> {
        let read = self.pull_requests.get().read_page(entry, remote, number, fresh).await.map_err(said)?;
        let page = serde_json::from_value(serde_json::to_value(read.read).map_err(|e| HostError::new(e.to_string()))?)
            .map_err(|e| HostError::new(e.to_string()))?;
        Ok((page, read.here))
    }
    async fn merge_settings(&self, remote: &str) -> Result<Value, HostError> {
        let settings = self.pull_requests.get().merge_settings(remote).await.map_err(said)?;
        serde_json::to_value(settings).map_err(|e| HostError::new(e.to_string()))
    }
    async fn read_host(&self, entry: &Entry, frame: FrameFor) -> Result<Value, HostError> {
        let parse: Rc<dyn Fn(wsp_core::reach::Frame) -> Result<Value, AccountError>> = Rc::new(|reply| Ok(Value::Object(reply)));
        self.pull_requests.get().read_host(entry, frame, parse).await.map_err(said)
    }
    async fn post_as_person(&self, remote: &str, number: i64, ask: Value) -> Result<Value, HostError> {
        let frame: FrameFor = Rc::new(move |cwd: &str| {
            let mut frame = ask.as_object().cloned().unwrap_or_default();
            frame.insert("cwd".to_string(), Value::String(cwd.to_string()));
            frame
        });
        let parse: Rc<dyn Fn(wsp_core::reach::Frame) -> Result<Value, AccountError>> = Rc::new(|reply| Ok(Value::Object(reply)));
        self.pull_requests.get().post_as_person(remote, number, frame, parse).await.map_err(said)
    }
    async fn to_first_thread(&self, workspace_id: &str, prompt: &str, origin: Option<Caller>) -> Result<SentTo, HostError> {
        let sent = self.daemon.get().to_first_thread(workspace_id, prompt, origin.as_ref()).await?;
        sent_to(sent)
    }
    async fn send_detached(&self, workspace_id: &str, prompt: &str, thread: &str, origin: Option<Caller>) -> Result<SentTo, HostError> {
        let sent = self.daemon.get().send_detached(workspace_id, prompt, Some(thread), origin.as_ref()).await?;
        sent_to(sent)
    }
    async fn child_of(
        &self,
        lead: &Entry,
        child: &str,
        origin: Option<Caller>,
        lead_thread: Option<&str>,
        child_thread: Option<&str>,
    ) -> Result<Entry, HostError> {
        self.boot.get().child_of(lead, child, origin.as_ref(), lead_thread, child_thread).await
    }
    async fn update_copy(&self, entry: &Entry) -> Result<wsp_wire::types::pull_request::GitUpdateReply, HostError> {
        self.pull_requests.get().update_copy(entry).await.map_err(said)
    }
    fn drop_page_cache(&self, remote: &str, number: i64) {
        self.github_cache.delete(&format!("page:{remote}#{number}"));
    }
    fn is_local_workspace(&self, record: &WorkspaceRecord) -> bool {
        is_local_workspace(Some(&record.kind))
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        spawn_detached(f);
    }
}

fn sent_to(sent: wsp_machines::machines::daemon::DetachedSend) -> Result<SentTo, HostError> {
    use wsp_core::context::DetachedOutcome;
    let outcome = match sent.outcome {
        DetachedOutcome::Steered => "steered",
        DetachedOutcome::Queued => "queued",
        DetachedOutcome::Started => "started",
        DetachedOutcome::Held => "held",
    }
    .to_string();
    let capped = sent.capped.map(|cap| serde_json::to_value(cap).map_err(|e| HostError::new(e.to_string()))).transpose()?;
    Ok(SentTo { outcome, thread_id: sent.thread_id, harness: sent.harness, capped })
}
