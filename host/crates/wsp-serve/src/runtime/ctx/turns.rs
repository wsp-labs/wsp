// SPDX-License-Identifier: AGPL-3.0-only
//! The context's bodies for the turns, forks and conversations areas (turns.ts:17-866, forks.ts:1-240,
//! conversations.ts:68-273; wired at runtime.ts:467 and :482). Each method reads a core field, forwards to the area
//! that owns it, or calls a platform function. A method name several traits share is called through the area or the
//! struct's own field, never through another trait of the context.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_account::account::core_types::{AccountError, Work};
use wsp_account::account::usage::{LimitReading, UsageCtx};
use wsp_adapters::port::{ExecFailure, HarnessExec};
use wsp_core::console;
use wsp_core::harness_catalog::harness_catalog;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_machines::machines::boot::entries::{EntryAsk, RowAt};
use wsp_machines::machines::machines::Row;
use wsp_machines::usage::accounts::{account_of, BurnCall, ComputerName, NamedAccount, ReplayedStamp};
use wsp_machines::usage::Vaulted;
use wsp_machines::usage::{EntryTokens, UsageEntry, UsageSource};
use wsp_threads::threads::agents::lists;
use wsp_threads::threads::threads::tree::TellAs;
use wsp_threads::threads::threads::{Entry, ThreadsCtx};
use wsp_threads::threads::turns::{BurnAsk, LedgerUse, LimitAsk, Sweep, UsageAccount};
use wsp_wire::views::session::{SessionView, TitleSource};
use wsp_words::exit::{not_found_refusal, HostError};

use super::Ctx;
use crate::runtime::join::spawn_detached;

/// `ctx.ready()` as the one error the area traits carry.
fn started(ctx: &Ctx) -> LocalBoxFuture<'static, Result<(), HostError>> {
    let boot = ctx.boot.get();
    async move { boot.ready().await.map_err(|e| e.0) }.boxed_local()
}

fn failed(e: AccountError) -> HostError {
    HostError::new(e.message())
}

/// An exec failure as the host's own error, which is what a trait method that can fail answers.
fn exec_failed(e: ExecFailure) -> HostError {
    HostError::new(e.message)
}

/// A row's key in `sessions`, put there for one call where the map does not hold it: the TS acts on the object it is
/// handed, the area looks the row up by its key.
fn turns_with_row_keyed<R>(ctx: &Ctx, row: &Row, f: impl FnOnce(&str) -> R) -> R {
    let held = ctx.sessions.borrow().iter().find(|(_, held)| Rc::ptr_eq(held, row)).map(|(key, _)| key.clone());
    if let Some(key) = held {
        return f(&key);
    }
    let key = row.borrow().view.id.clone();
    let displaced = ctx.sessions.borrow_mut().insert(key.clone(), row.clone());
    let out = f(&key);
    let mut sessions = ctx.sessions.borrow_mut();
    match displaced {
        Some(held) => {
            sessions.insert(key, held);
        }
        None => {
            sessions.shift_remove(&key);
        }
    }
    out
}

fn view_of(row: &Row) -> SessionView {
    row.borrow().view.clone()
}

#[async_trait::async_trait(?Send)]
impl wsp_threads::threads::turns::TurnsCtx for Ctx {
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        self.sessions.clone()
    }
    fn records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        self.thread_records.clone()
    }
    fn live_entry(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn closing(&self) -> bool {
        self.state.with(|s| s.closing)
    }
    fn record(&self, event: wsp_wire::views::session_events_more::SessionEvent) {
        self.transcripts.get().record(event)
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) {
        if let Err(e) = self.bus.emit(event) {
            console::warn(&e.message);
        }
    }
    fn pass(&self, event: wsp_wire::wire::events_union::EventUnion) {
        if let Err(e) = self.bus.pass(event) {
            console::warn(&e.message);
        }
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().persist_sessions(workspace_id)
    }
    async fn persist_record(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.records.get().persist(&record).await
    }
    fn held_at_close(&self) -> wsp_threads::threads::turns::HeldAtClose {
        self.held_at_close.clone()
    }
    async fn slates_turn_ended(&self, thread_id: &str, turn_id: &str) -> Result<(), HostError> {
        self.slates.get().turn_ended(thread_id, turn_id).await
    }
    fn keep_checkpoint(&self, entry: &Entry, ask: wsp_threads::threads::threads::CheckpointAsk) -> wsp_core::context::Joined {
        let (threads, entry) = (self.threads.get(), entry.clone());
        async move { threads.keep_checkpoint(&entry, ask).await }.boxed_local().shared()
    }
    fn checkpoints_landing(&self) -> Rc<RefCell<IndexMap<String, wsp_core::context::Joined>>> {
        self.threads.get().checkpoints_landing.clone()
    }
    async fn read_turn_changes(&self, entry: &Entry, ask: wsp_threads::threads::threads::TurnChangesAsk) -> Result<bool, HostError> {
        Ok(self.threads.get().read_turn_changes(entry, ask).await)
    }
    fn lead_asks(&self) -> Rc<RefCell<IndexMap<String, wsp_adapters::port::PermissionAsk>>> {
        self.threads.get().lead_asks.clone()
    }
    fn mode_label(&self, harness: &str, mode: &str) -> Option<String> {
        harness_catalog(harness)?.permission_modes.iter().find(|m| m.value == mode).map(|m| m.label.clone())
    }
    fn settle_cut(&self, row: &Row, reason: &str, by_stop: bool, unreached: bool) {
        let threads = self.threads.get();
        turns_with_row_keyed(self, row, |key| threads.settle_cut(key, reason, &|_| reason.to_string(), by_stop, unreached));
    }
    fn line_try(&self, row: &Row) -> bool {
        let threads = self.threads.get();
        turns_with_row_keyed(self, row, |key| threads.line_try(key))
    }
    fn prompt_held(&self, request_id: &str) {
        self.threads.get().prompt_held(request_id)
    }
    async fn name_in_harness(&self, view: SessionView, title: &str) -> Result<(), HostError> {
        self.agents.get().name_in_harness(&view, title).await;
        Ok(())
    }
    fn source_of(&self, view: &SessionView) -> Option<String> {
        let word = match wsp_threads::threads::agents::lists::source_of(view) {
            TitleSource::Seed => "seed",
            TitleSource::Auto => "auto",
            TitleSource::Person => "person",
        };
        Some(word.to_string())
    }
    fn hold_ask(&self, ask: LocalBoxFuture<'static, ()>) {
        drop(self.agents.get().hold(ask));
    }
    async fn refresh_title(&self, row: &Row, ended: bool) -> Result<(), HostError> {
        self.agents.get().refresh_title(&view_of(row), ended).await;
        Ok(())
    }
    async fn make_title(&self, row: &Row) -> Result<(), HostError> {
        self.agents.get().make_title(&view_of(row)).await
    }
    fn usage_account_of(&self, entry: &Entry, harness: &str, account: Option<(String, Option<String>)>) -> UsageAccount {
        let record = entry.borrow().record.borrow().clone();
        let login_stands = ThreadsCtx::login_stands(self, entry, harness);
        let vault = ThreadsCtx::vault_on(self, entry);
        let vaulted = match self.threads.get().vaulted_for(harness, Some(login_stands), Some(&vault)) {
            Some("token") => Some(Vaulted::Token),
            Some(_) => Some(Vaulted::Key),
            None => None,
        };
        let agent_name = harness_catalog(harness).map(|catalog| catalog.label).unwrap_or_else(|| harness.to_string());
        let computer = ComputerName {
            id: wsp_threads::threads::threads::ThreadsArea::usage_computer_of(&record),
            name: self.channels.get().computer_of(entry),
        };
        let named = account.map(|(id, label)| NamedAccount { id, label });
        let found = account_of(harness, &agent_name, named.as_ref(), vaulted, &computer);
        let road = serde_json::to_value(found.road).ok().and_then(|word| word.as_str().map(str::to_string));
        UsageAccount { key: found.key, label: Some(found.label), road }
    }
    fn usage_computer_of(&self, record: &WorkspaceRecord) -> String {
        wsp_threads::threads::threads::ThreadsArea::usage_computer_of(record)
    }
    async fn ledger_add(&self, row: LedgerUse) -> Result<(), HostError> {
        let tokens = row.tokens.map(|t| EntryTokens {
            input: Some(t.input.0),
            output: Some(t.output.0),
            cached: t.cached.map(|n| n.0),
            cache_write: t.cache_write.map(|n| n.0),
            reasoning: t.reasoning.map(|n| n.0),
        });
        let entry = UsageEntry {
            at: row.at,
            agent: row.agent,
            account: row.account,
            account_label: row.account_label.unwrap_or_default(),
            computer: row.computer,
            project: row.project,
            model: row.model,
            turns: Some(f64::from(row.turns)),
            tokens,
            cost_usd: row.cost_usd,
            session: row.session,
            source: if row.source == "log" { UsageSource::Log } else { UsageSource::Wsp },
        };
        self.usage_parts().ledger.add(entry).await
    }
    async fn ledger_limit(&self, ask: LimitAsk) -> Result<(Value, Value), HostError> {
        let reading = LimitReading {
            key: ask.key,
            agent: ask.agent,
            label: ask.label.unwrap_or_default(),
            road: ask.road.unwrap_or_default(),
            computer: ask.computer,
            limit: ask.limit,
            at: None,
        };
        UsageCtx::file_limit(self, reading).await.map_err(failed)
    }
    async fn alerts_read(&self, before: Value, after: Value) -> Result<(), HostError> {
        UsageCtx::alerts_read(self, before, after).await.map_err(failed)
    }
    async fn usage_accounts(&self) -> Result<wsp_wire::types::usage::AccountsAnswer, HostError> {
        self.usage.get().usage_accounts().await.map_err(failed)
    }
    fn burn_add(&self, ask: BurnAsk) {
        self.usage_parts().burn.add(BurnCall {
            account: ask.account,
            thread_id: ask.thread_id,
            tokens: ask.tokens,
            replayed: ask.replayed.map(|(run, at)| ReplayedStamp { run, at: at as i64 }),
        });
    }
    fn notify_end(&self, row: &Row, notify: &[String], result: wsp_wire::views::session_events::TurnResult, open: bool) {
        let named = {
            let held = row.borrow();
            TellAs { by: held.notify_by.clone(), road: held.notify_road }
        };
        self.threads.get().notify_end(row, notify, &named, &result, open)
    }
    fn agent_retried(&self, turn_id: &str, why: Option<&str>) {
        self.waits.get().agent_retried(turn_id, why)
    }
    fn end_snooze_for(&self, view: &SessionView) {
        self.boot.get().end_snooze_for(view.thread_id.as_deref(), view.root_thread_id.as_deref())
    }
    fn idle_hold(&self, workspace_id: &str) {
        self.idle.get().hold(workspace_id)
    }
    fn idle_release(&self, workspace_id: &str) {
        self.idle.get().release(workspace_id)
    }
    fn arm_disk_watch(&self) {
        self.reach.get().arm_disk_watch()
    }
    async fn read_disk(&self, entry: &Entry) -> Result<(), HostError> {
        self.reach.get().read_disk(entry).await;
        Ok(())
    }
    fn harness_exec(&self, entry: &Entry, timeout_ms: u64) -> Arc<dyn HarnessExec> {
        lists::harness_exec_on(entry.borrow().machine.clone(), timeout_ms)
    }
    fn shared_daemon(&self, entry: &Entry) -> bool {
        let kind = entry.borrow().record.borrow().kind;
        self.kinds.get().module_of(kind).map(|module| module.shared_daemon()).unwrap_or(false)
    }
    fn turn_groups(&self) -> Rc<RefCell<IndexMap<String, IndexMap<i64, String>>>> {
        self.channels.get().turn_groups.clone()
    }
    fn arm_roots_recheck(&self) {
        self.channels.get().arm_roots_recheck()
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        self.channels.get().port_roots_moved(workspace_id)
    }
    fn hold_kept(&self, thread_id: &str, hold: wsp_threads::threads::threads::KeptHand) -> Option<wsp_threads::threads::threads::KeptHand> {
        self.threads.get().hold_kept(thread_id, hold)
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError> {
        self.device_door.revoke(device_id).await.map(|_| ())
    }
    async fn read_checkout(&self, entry: &Entry, fresh: bool) -> Result<(), HostError> {
        self.reach.get().read_checkout(entry, fresh).await;
        Ok(())
    }
    async fn ran_on_branch(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError> {
        self.pull_requests.get().ran_on_branch(entry, thread_id).await.map_err(failed)
    }
    async fn read_pull_request(&self, entry: &Entry, fresh: bool) -> Result<(), HostError> {
        self.pull_requests.get().read_pull_request(entry, fresh, false).await;
        Ok(())
    }
    async fn read_lead_of(&self, entry: &Entry) -> Result<(), HostError> {
        self.reach.get().read_lead_of(entry);
        Ok(())
    }
    fn pull_request_state(&self, entry: &Entry) -> Option<String> {
        let state = entry.borrow().record.borrow().pr.as_ref().map(|pr| pr.state)?;
        serde_json::to_value(state).ok().and_then(|word| word.as_str().map(str::to_string))
    }
    async fn settle_tree(&self, entry: &Entry) -> Result<(), HostError> {
        self.pull_requests.get().settle_tree(entry).await;
        Ok(())
    }
    fn drop_images(&self, entry: &Entry, dir: &str) {
        self.agents.get().drop_images(entry, dir)
    }
    fn send_back(&self, row: &Row, unread: Vec<String>, stopped: bool) {
        self.threads.get().send_back(row, &unread, stopped)
    }
    async fn take_review(&self, entry: &Entry, thread_id: &str, text: &str) -> Result<(), HostError> {
        self.start_from.get().take_review(entry, thread_id, text).await
    }
    fn is_held_away(&self, workspace_id: &str) -> bool {
        self.boot.get().is_held_away(workspace_id)
    }
    fn exec_sweep(&self, entry: &Entry) -> Option<Sweep> {
        let factory = self.kinds.get().exec_factory_for(&entry.borrow(), None, None).ok()?;
        let _probe = factory.sweep(&[])?;
        Some(Box::new(move |keep: Vec<String>| match factory.sweep(&keep) {
            Some(swept) => async move { swept.await.map_err(exec_failed) }.boxed_local(),
            None => async { Ok(Vec::new()) }.boxed_local(),
        }))
    }
    fn adapter_for(
        &self,
        entry: &Entry,
        ask: wsp_threads::threads::agents::launch::AdapterAsk,
    ) -> Result<wsp_threads::threads::agents::launch::BuiltAdapter, HostError> {
        self.agents.get().adapter_for(entry, ask)
    }
    fn open_thread(
        &self,
        workspace_id: &str,
        thread_id: &str,
    ) -> LocalBoxFuture<'static, Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>> {
        let (transcripts, workspace_id, thread_id) = (self.transcripts.get(), workspace_id.to_string(), thread_id.to_string());
        async move { transcripts.open_thread(&workspace_id, &thread_id) }.boxed_local()
    }
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        spawn_detached(work);
    }
}

impl wsp_threads::threads::forks::ForksCtx for Ctx {
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>> {
        started(self)
    }
    fn store(&self) -> Rc<dyn wsp_store::store::Store> {
        self.store.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> wsp_threads::threads::threads::ThreadRecords {
        self.thread_records.clone()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.agents.get().rows_on(thread_id)
    }
    fn running_turn(&self, thread_id: &str) -> Option<String> {
        self.threads.get().running_on(thread_id).map(|(_, turn)| turn)
    }
    fn launching_turn(&self, thread_id: &str) -> Option<String> {
        self.threads.get().launching_on(thread_id).map(|(turn, _)| turn)
    }
    fn entry_of_row(
        &self,
        at: &RowAt,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        let (boot, at) = (self.boot.get(), at.clone());
        async move {
            match boot.entry_of_row(&at, origin.as_ref(), None, false).await? {
                Some(entry) => Ok(entry),
                None => Err(not_found_refusal(&wsp_words::words::computer::no_workspace_refusal(None))),
            }
        }
        .boxed_local()
    }
    fn open_thread(
        &self,
        workspace_id: &str,
        thread_id: &str,
    ) -> LocalBoxFuture<'static, Result<Vec<wsp_wire::views::session_events_more::SessionEvent>, HostError>> {
        let (transcripts, workspace_id, thread_id) = (self.transcripts.get(), workspace_id.to_string(), thread_id.to_string());
        async move { transcripts.open_thread(&workspace_id, &thread_id) }.boxed_local()
    }
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.channels.get().started_as(workspace_id, thread_id)
    }
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String> {
        self.channels.get().folder_of(workspace_id, session)
    }
    fn agent_label(&self, agent: &str) -> String {
        wsp_threads::threads::agents::AgentsArea::agent_label(agent)
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<wsp_core::context::Joined> {
        self.transcripts.get().kept_writes.borrow().get(workspace_id).cloned()
    }
    fn keep_sent_images(
        &self,
        workspace_id: &str,
        thread_id: &str,
        request_id: Option<String>,
        attachments: Vec<wsp_wire::types::attachments::Attachment>,
    ) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().keep_sent_images(workspace_id, thread_id, request_id.as_deref(), &attachments)
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.transcripts.get().persist_sessions(workspace_id)
    }
    fn project_held(&self, project: &str) -> Result<wsp_wire::views::workspace::ProjectView, HostError> {
        self.channels.get().project_held(project)
    }
    fn place_name(&self, computer: &str) -> String {
        self.builders_area.get().place_name(computer)
    }
    fn git_here(
        &self,
        top: &str,
        args: Vec<String>,
    ) -> LocalBoxFuture<'static, Result<wsp_wire::wire::machine_link::ExecResult, HostError>> {
        let (kinds, top) = (self.kinds.get(), top.to_string());
        async move {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            kinds.git_here(&top, &args, None, None).await.map_err(|e| e.into_host())
        }
        .boxed_local()
    }
    fn live_ids(&self) -> Vec<String> {
        self.live.borrow().keys().cloned().collect()
    }
    fn worktree_folder(
        &self,
        project: &wsp_wire::views::workspace::ProjectView,
        top: &str,
        branch: &str,
        from: &str,
        root: Option<String>,
        at_ref: &str,
    ) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        let (folders, project) = (self.folders.get(), project.clone());
        let (top, branch, from, at_ref) = (top.to_string(), branch.to_string(), from.to_string(), at_ref.to_string());
        async move { folders.worktree_folder(&project, &top, &branch, Some(from), root, Some(at_ref)).await }.boxed_local()
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        self.channels.get().checkout_of(record)
    }
    fn take_setup_line(&self, workspace_id: &str) -> Option<String> {
        self.folders.get().take_setup_line(workspace_id)
    }
    fn remove_worktree(&self, entry: &Entry, force: bool) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let (folders, entry) = (self.folders.get(), entry.clone());
        async move {
            folders.remove_worktree(&entry, force, wsp_account::projects::folders::tree::RemoveAsk { ending: false, check: false }).await
        }
        .boxed_local()
    }
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        spawn_detached(work);
    }
}

impl wsp_threads::threads::conversations::ConversationsCtx for Ctx {
    fn clock(&self) -> Rc<dyn wsp_core::clock::Clock> {
        self.clock.clone()
    }
    fn bus(&self) -> Rc<dyn wsp_core::types::events::EventBus> {
        self.bus.clone()
    }
    fn sessions(&self) -> wsp_threads::threads::threads::Sessions {
        self.sessions.clone()
    }
    fn transcript_state(&self) -> wsp_core::context::StateCell<wsp_threads::threads::transcripts::TranscriptState> {
        self.transcript_state.clone()
    }
    fn live_project(&self, workspace_id: &str) -> Option<String> {
        self.live.borrow().get(workspace_id).map(|entry| entry.borrow().record.borrow().project.clone())
    }
    fn with_daemon(&self, entry: &Entry, frame: Value) -> LocalBoxFuture<'static, Result<Value, AccountError>> {
        let (channels, entry) = (self.channels.get(), entry.clone());
        async move {
            let work: Work<Value> = Box::new(move |ask| {
                let frame = match frame {
                    Value::Object(frame) => frame,
                    _ => wsp_core::reach::Frame::new(),
                };
                async move { ask(frame).await.map(Value::Object) }.boxed_local()
            });
            channels.with_daemon(&entry, work).await
        }
        .boxed_local()
    }
    fn harness_exec(&self, entry: &Entry, timeout_ms: u64) -> Arc<dyn HarnessExec> {
        lists::harness_exec_on(entry.borrow().machine.clone(), timeout_ms)
    }
    fn project_held(&self, project: &str) -> Result<wsp_wire::views::workspace::ProjectView, HostError> {
        self.channels.get().project_held(project)
    }
    fn worktrees_of(
        &self,
        top: &str,
    ) -> LocalBoxFuture<'static, Result<Vec<wsp_account::projects::folders::tree::ListedWorktree>, HostError>> {
        let (folders, top) = (self.folders.get(), top.to_string());
        async move { folders.worktrees_of(&top).await }.boxed_local()
    }
    fn folders_of(&self, project: &str) -> Vec<WorkspaceRecord> {
        self.create.get().folders_of(project).iter().map(|entry| entry.borrow().record.borrow().clone()).collect()
    }
    fn has_adapter(&self, harness: &str) -> bool {
        self.opts.adapters.contains_key(harness)
    }
    fn agent_off(&self, entry: &Entry, agent: &str) -> bool {
        self.agents.get().agent_off(entry, agent)
    }
    fn conversation_store(&self, entry: &Entry, harness: &str) -> Option<Arc<dyn wsp_adapters::port::ConversationStore>> {
        let ask = wsp_threads::threads::agents::launch::AdapterAsk { named: Some(harness.to_string()), ..Default::default() };
        self.agents.get().adapter_for(entry, ask).ok()?.adapter.conversations()
    }
    fn agent_label(&self, agent: &str) -> String {
        wsp_threads::threads::agents::AgentsArea::agent_label(agent)
    }
    fn computer_of(&self, entry: &Entry) -> String {
        self.channels.get().computer_of(entry)
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.threads.get().latest_on(thread_id)
    }
    fn reaches_row(&self, row: &SessionView, origin: Option<&wsp_wire::views::workspace::Caller>) -> Result<bool, HostError> {
        let at = RowAt { thread_id: row.thread_id.clone(), workspace_id: row.workspace_id.clone() };
        self.boot.get().reaches_row(&at, origin, None)
    }
    fn folder_for(
        &self,
        project: Option<String>,
        cwd: Option<String>,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> LocalBoxFuture<'static, Result<wsp_account::projects::folders::place::FolderPlaced, HostError>> {
        let folders = self.folders.get();
        let ask = wsp_account::projects::folders::place::FolderAsk { project, cwd, ..Default::default() };
        async move { folders.folder_for(ask, origin).await }.boxed_local()
    }
    fn entry_of(
        &self,
        workspace_id: &str,
        origin: Option<wsp_wire::views::workspace::Caller>,
    ) -> LocalBoxFuture<'static, Result<Entry, HostError>> {
        let (boot, workspace_id) = (self.boot.get(), workspace_id.to_string());
        async move { boot.entry_of(&workspace_id, origin.as_ref(), EntryAsk::default()).await }.boxed_local()
    }
    fn preferences(&self) -> LocalBoxFuture<'static, Result<wsp_wire::views::preferences::Preferences, HostError>> {
        let held = self.state.with(|s| s.preferences_held.clone());
        let preferences = self.preferences.get();
        async move {
            match held {
                Some(held) => Ok(held),
                None => preferences.get().await.map_err(failed),
            }
        }
        .boxed_local()
    }
    fn default_agent_of(&self, prefs: &wsp_wire::views::preferences::Preferences, entry: &Entry) -> String {
        self.agents.get().default_agent_of(prefs, Some(entry))
    }
}
