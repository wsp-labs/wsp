// SPDX-License-Identifier: AGPL-3.0-only
//! Port of `packages/runtime/src/threads/forks.ts` at 65f6b89c7: a fork read off the thread it forks, every refusal
//! said before anything is written. Free functions over the context, as the TS's are. Spec: `spec/forks.md`.
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use serde_json::{json, Value};
use wsp_adapters::port::{CountedTurn, ForkTurn, ForkedSession};
use wsp_core::console;
use wsp_core::context::Joined;
use wsp_core::types::api::{ForkOptions, StartOptions};
use wsp_core::types::harness::HarnessAdapter;
use wsp_core::types::internal::{KeptImage, KeptImages, ATTACHMENTS, ATTACHMENT_KEYS};
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::sub_thread::SUB_TURN_GONE_LINE;
use wsp_wire::views::session::{ForkSource, ForkedFrom, SessionView};
use wsp_wire::views::session_events::{copied_from_of, TurnResult};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::sub_row::sub_turn_gone;
use wsp_wire::views::workspace::{scope_of, Caller, ProjectView};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::attachments::{attachment_key, is_image};
use wsp_words::exit::{not_found_refusal, refusal, usage_refusal, HostError, Kind};
use wsp_words::projects::{copies_folder, no_branches_line, place_branch_line};
use wsp_words::views::session::fold_threads;
use wsp_words::words::computer::thread_word;
use wsp_words::words::thread::{
    fork_agent_fix, fork_agent_line, fork_no_anchor_line, fork_no_turn_line, fork_turns_fix, fork_turns_line, FORK_BESIDE_FIX, FORK_BESIDE_LINE, FORK_NO_ANCHOR_FIX,
    FORK_NO_CHECKPOINT_FIX, FORK_NO_CHECKPOINT_LINE, FORK_NO_TURN_FIX, FORK_RESUME_FIX, FORK_RESUME_LINE, FORK_RUNNING_FIX, FORK_RUNNING_LINE,
};
use wsp_machines::machines::boot::entries::RowAt;

use super::threads::{Entry, Sessions, ThreadRecords};

/// Every `ctx.<x>` the fork functions call (README R2).
pub trait ForksCtx {
    fn ready(&self) -> LocalBoxFuture<'static, Result<(), HostError>>;
    fn store(&self) -> Rc<dyn Store>;
    fn sessions(&self) -> Sessions;
    fn thread_records(&self) -> ThreadRecords;
    fn latest_on(&self, thread_id: &str) -> Option<SessionView>;
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView>;
    /// The running row's turn (`ctx.runningOn(thread)?.turnId`).
    fn running_turn(&self, thread_id: &str) -> Option<String>;
    /// The launching row's turn (`ctx.launchingOn(thread)?.turnId`).
    fn launching_turn(&self, thread_id: &str) -> Option<String>;
    fn entry_of_row(&self, at: &RowAt, origin: Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>>;
    fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String>;
    fn folder_of(&self, workspace_id: &str, session: &str) -> Option<String>;
    fn agent_label(&self, agent: &str) -> String;
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined>;
    fn keep_sent_images(&self, workspace_id: &str, thread_id: &str, request_id: Option<String>, attachments: Vec<Attachment>) -> LocalBoxFuture<'static, ()>;
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()>;
    fn project_held(&self, project: &str) -> ProjectView;
    fn place_name(&self, computer: &str) -> String;
    fn git_here(&self, top: &str, args: Vec<String>) -> LocalBoxFuture<'static, Result<ExecResult, HostError>>;
    /// `new Set(ctx.live.keys())`.
    fn live_ids(&self) -> Vec<String>;
    /// `ctx.worktreeFolder(project, top, branch, from, root, ref)`.
    fn worktree_folder(&self, project: &ProjectView, top: &str, branch: &str, from: &str, root: Option<String>, at_ref: &str) -> LocalBoxFuture<'static, Result<Entry, HostError>>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    fn take_setup_line(&self, workspace_id: &str) -> Option<String>;
    fn remove_worktree(&self, entry: &Entry, force: bool) -> LocalBoxFuture<'static, Result<(), HostError>>;
}

/// forks.ts:27 `ForkPlan.source`.
#[derive(Clone)]
pub struct ForkPlanSource {
    pub thread_id: String,
    pub workspace_id: String,
    pub entry: Entry,
    pub harness: String,
    pub title: String,
    pub session: Option<String>,
    pub cwd: Option<String>,
}

/// forks.ts:30 `ForkPlan.turn`.
#[derive(Clone, Debug, PartialEq)]
pub struct ForkPlanTurn {
    pub turn_id: String,
    pub anchor: Option<String>,
    pub at_ref: Option<String>,
    pub after: Vec<CountedTurn>,
}

/// forks.ts:34 `ForkPlan.picks`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForkPicks {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub context_window: Option<String>,
    pub fast: Option<bool>,
    pub permission_mode: Option<String>,
}

/// forks.ts:26 `ForkPlan`: F5.
#[derive(Clone)]
pub struct ForkPlan {
    pub source: ForkPlanSource,
    pub turn: Option<ForkPlanTurn>,
    pub events: Vec<SessionEvent>,
    pub picks: ForkPicks,
    pub forked_from: ForkedFrom,
}

/// The start a fork runs (`(workspaceId, o) => Promise<SessionHandle>`, forks.ts:199).
pub type StartFn = Rc<dyn Fn(String, StartOptions) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>>;

/// An event as its JSON, for the keys a fork reads by name (`e.turnId`, `e.at`, `e.type`).
fn value_of(event: &SessionEvent) -> Value {
    serde_json::to_value(event).unwrap_or(Value::Null)
}

/// A thread's event with its JSON beside it, read in transcript order.
struct Read {
    event: SessionEvent,
    value: Value,
}

impl Read {
    fn kind(&self) -> &str {
        self.value["type"].as_str().unwrap_or("")
    }
    fn turn(&self) -> Option<&str> {
        self.value["turnId"].as_str()
    }
    fn at(&self) -> Option<f64> {
        self.value["at"].as_f64()
    }
}

/// The time the sub machine of the workspace's record was released at, if it was.
fn released_at(entry: &Entry) -> Option<f64> {
    let record = entry.borrow().record.borrow().clone();
    record.sub.and_then(|sub| sub.released_at.map(|at| at.0))
}

/// forks.ts:42: F1.
pub async fn fork_opened(ctx: &Rc<dyn ForksCtx>, workspace_id: &str, opened: StartOptions, origin: Option<Caller>) -> Result<(StartOptions, Option<ForkPlan>), HostError> {
    let Some(fork) = opened.fork.clone() else { return Ok((opened, None)) };
    if opened.thread.is_some() || opened.replaces.is_some() {
        return Err(usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX));
    }
    if opened.outside.is_some() {
        return Err(usage_refusal(FORK_RESUME_LINE, FORK_RESUME_FIX));
    }
    let plan = read_fork(ctx, &fork, origin).await?;
    let source = plan.source.harness.clone();
    if let Some(asked) = &opened.harness {
        if asked != &source {
            return Err(refusal(&fork_agent_line(&ctx.agent_label(&source), &ctx.agent_label(asked)), &fork_agent_fix(&ctx.agent_label(asked)), Some(Kind::Usage)));
        }
    }
    let picks = plan.picks.clone();
    let mut out = opened;
    out.harness = Some(source);
    if out.model.is_none() {
        out.model = picks.model.clone();
    }
    if out.effort.is_none() {
        out.effort = picks.effort.clone();
    }
    if out.context_window.is_none() {
        out.context_window = picks.context_window.clone();
    }
    if out.fast.is_none() && picks.fast == Some(true) {
        out.fast = Some(true);
    }
    if out.permission_mode.is_none() && out.access.is_none() {
        out.permission_mode = picks.permission_mode.clone();
    }
    if out.cwd.is_none() && plan.source.workspace_id == workspace_id {
        out.cwd = plan.source.cwd.clone();
    }
    Ok((out, Some(plan)))
}

/// forks.ts:67: F2 to F5.
pub async fn read_fork(ctx: &Rc<dyn ForksCtx>, fork: &ForkSource, origin: Option<Caller>) -> Result<ForkPlan, HostError> {
    ctx.ready().await?;
    let thread_id = fork.thread_id.clone();
    let latest = ctx.latest_on(&thread_id);
    let record = ctx.thread_records().borrow().get(&thread_id).cloned();
    let not_found = || not_found_refusal(&format!("no thread {}", thread_word(&thread_id)));
    let workspace_id = latest.as_ref().map(|l| l.workspace_id.clone()).or_else(|| record.as_ref().map(|r| r.workspace_id.clone()).filter(|w| !w.is_empty()));
    let Some(workspace_id) = workspace_id else { return Err(not_found()) };
    let at = RowAt { thread_id: Some(thread_id.clone()), workspace_id: workspace_id.clone() };
    let entry = ctx.entry_of_row(&at, origin).await.map_err(|_| not_found())?;
    let Some(harness) = record.as_ref().map(|r| r.harness.clone()).or_else(|| latest.as_ref().map(|l| l.harness.clone())) else {
        return Err(not_found());
    };
    // A slate's events are its own thread's, and the fork opens with no slate.
    // An outside conversation the thread opened on is written ahead of its first turn under no turn, and is carried
    // with any turn the fork carries through, since the agent's session holds it.
    let read: Vec<Read> = ctx
        .open_thread(&workspace_id, &thread_id)
        .await?
        .into_iter()
        .map(|event| {
            let value = value_of(&event);
            Read { event, value }
        })
        .filter(|r| r.kind() != "session.slate")
        .collect();
    let earlier: Vec<SessionEvent> = read.iter().filter(|r| r.kind() == "session.earlier").map(|r| r.event.clone()).collect();
    let events: Vec<&Read> = read.iter().filter(|r| r.turn().is_some()).collect();
    let mut order: Vec<String> = Vec::new();
    for r in &events {
        if let Some(turn) = r.turn() {
            if !order.iter().any(|o| o == turn) {
                order.push(turn.to_string());
            }
        }
    }
    let running: Vec<String> = [ctx.running_turn(&thread_id), ctx.launching_turn(&thread_id)].into_iter().flatten().collect();
    let ended: Vec<&str> = events.iter().filter(|r| r.kind() == "session.end" || r.kind() == "session.done").filter_map(|r| r.turn()).collect();
    let finished: Vec<String> = order.iter().filter(|t| ended.contains(&t.as_str()) && !running.contains(t)).cloned().collect();
    let turn_id: Option<String> = match &fork.turn_id {
        Some(turn) => {
            if !order.contains(turn) {
                return Err(refusal(&fork_no_turn_line(turn), FORK_NO_TURN_FIX, Some(Kind::NotFound)));
            }
            if !finished.contains(turn) {
                return Err(refusal(FORK_RUNNING_LINE, FORK_RUNNING_FIX, Some(Kind::Conflict)));
            }
            Some(turn.clone())
        }
        None => {
            let picked = fork.at.map(JsInt::as_f64);
            if picked == Some(0.0) {
                None
            } else {
                let count = finished.len() as f64;
                let at = picked.unwrap_or(count);
                if at < 1.0 || at > count {
                    return Err(refusal(&fork_turns_line(count), &fork_turns_fix(count), Some(Kind::Usage)));
                }
                Some(finished[at as usize - 1].clone())
            }
        }
    };
    let entry_released = released_at(&entry);
    // A turn's checkpoint and the session it cut at went with a sub-thread's machine once that machine was deleted.
    if let Some(turn) = &turn_id {
        let first_at = events.iter().find(|r| r.turn() == Some(turn.as_str()) && r.at().is_some()).and_then(|r| r.at());
        if sub_turn_gone(entry_released, first_at) {
            return Err(HostError::with_kind(SUB_TURN_GONE_LINE, Kind::Conflict));
        }
    }
    let last_of = |kind: &str, turn: &str| -> Option<&Read> { events.iter().rev().find(|r| r.kind() == kind && r.turn() == Some(turn)).copied() };
    let counted = |turn: &str| CountedTurn {
        anchor: last_of("session.checkpoint", turn).and_then(|r| r.value["anchor"].as_str().map(String::from)),
        result: last_of("session.done", turn).and_then(|r| serde_json::from_value::<TurnResult>(r.value["result"].clone()).ok()),
    };
    let picked_index = turn_id.as_ref().and_then(|t| order.iter().position(|o| o == t));
    let kept: Vec<String> = match picked_index {
        Some(i) => order[..=i].to_vec(),
        None => Vec::new(),
    };
    let checkpoint = turn_id.as_deref().and_then(|t| last_of("session.checkpoint", t));
    let title = fold_threads(&ctx.rows_on(&thread_id))
        .first()
        .map(|t| t.title.clone())
        .or_else(|| record.as_ref().and_then(|r| r.forked_from.as_ref().map(|f| f.title.clone())))
        .unwrap_or_else(|| thread_word(&thread_id));
    let session = latest.as_ref().and_then(|l| l.claude_session_id.clone()).or_else(|| ctx.started_as(&workspace_id, &thread_id));
    let cwd = latest.as_ref().and_then(|l| l.cwd.clone()).or_else(|| session.as_deref().and_then(|s| ctx.folder_of(&workspace_id, s)));
    let permission_mode = record.as_ref().and_then(|r| r.permission_mode.clone()).or_else(|| latest.as_ref().and_then(|l| l.permission_mode.clone()));
    let turn = match (&turn_id, picked_index) {
        (Some(turn), Some(i)) => Some(ForkPlanTurn {
            turn_id: turn.clone(),
            anchor: checkpoint.and_then(|r| r.value["anchor"].as_str().map(String::from)),
            at_ref: checkpoint.and_then(|r| r.value["ref"].as_str().map(String::from)),
            after: order.iter().skip(i + 1).map(|t| counted(t)).collect(),
        }),
        _ => None,
    };
    let mut carried = if turn_id.is_some() { earlier } else { Vec::new() };
    carried.extend(events.iter().filter(|r| r.turn().is_some_and(|t| kept.iter().any(|k| k == t))).map(|r| r.event.clone()));
    let picks = ForkPicks {
        model: latest.as_ref().and_then(|l| l.model.clone()),
        effort: latest.as_ref().and_then(|l| l.effort.clone()),
        context_window: latest.as_ref().and_then(|l| l.context_window.clone()),
        fast: latest.as_ref().and_then(|l| l.fast).filter(|fast| *fast),
        permission_mode,
    };
    Ok(ForkPlan {
        source: ForkPlanSource {
            thread_id: thread_id.clone(),
            workspace_id,
            entry,
            harness,
            title: title.clone(),
            session,
            cwd,
        },
        turn,
        events: carried,
        picks,
        forked_from: ForkedFrom { thread_id, turn_id, title },
    })
}

/// Refuses a fork at a turn its harness cannot cut at: one that named no anchor, on a harness that finds none by count.
/// forks.ts:143: F6.
pub fn fork_turn_of(plan: &ForkPlan, by_count: bool) -> Result<Option<ForkTurn>, HostError> {
    let Some(turn) = &plan.turn else { return Ok(None) };
    if let Some(anchor) = &turn.anchor {
        return Ok(Some(ForkTurn::Anchor(anchor.clone())));
    }
    if !by_count {
        let label = wsp_core::harness_catalog::harness_catalog(&plan.source.harness).map(|c| c.label).unwrap_or_else(|| plan.source.harness.clone());
        return Err(refusal(&fork_no_anchor_line(&label), FORK_NO_ANCHOR_FIX, Some(Kind::Conflict)));
    }
    Ok(Some(ForkTurn::After(turn.after.clone())))
}

/// The source's events as the fork's transcript opens on them: under the fork's workspace and thread, each marked with
/// the thread it was copied from. forks.ts:153: F7.
pub fn copied_events(plan: &ForkPlan, workspace_id: &str, thread_id: &str) -> Vec<SessionEvent> {
    plan.events
        .iter()
        .filter_map(|event| {
            let mut value = value_of(event);
            let Value::Object(map) = &mut value else { return None };
            map.remove("pos");
            map.insert("workspaceId".to_string(), json!(workspace_id));
            map.insert("threadId".to_string(), json!(thread_id));
            let copied = copied_from_of(event).unwrap_or_else(|| plan.source.thread_id.clone());
            map.insert("copiedFrom".to_string(), json!(copied));
            serde_json::from_value(value).ok()
        })
        .collect()
}

/// The images the copied history carries, kept again under the fork's own thread, so each still opens there once the
/// source is deleted. An image whose bytes the source no longer holds is left out. forks.ts:162: F8.
pub async fn copy_images(ctx: &Rc<dyn ForksCtx>, plan: &ForkPlan, workspace_id: &str, thread_id: &str) -> Result<(), HostError> {
    if let Some(settled) = ctx.kept_writes(&plan.source.workspace_id) {
        settled.await;
    }
    let store = ctx.store();
    let keys = store.get(ATTACHMENT_KEYS, &plan.source.workspace_id).await?;
    let held: Vec<KeptImage> = keys
        .and_then(|value| serde_json::from_value::<KeptImages>(value).ok())
        .and_then(|kept| kept.threads.get(&plan.source.thread_id).cloned())
        .unwrap_or_default();
    if held.is_empty() {
        return Ok(());
    }
    for event in &plan.events {
        let value = value_of(event);
        let kind = value["type"].as_str().unwrap_or("");
        if kind != "session.start" && kind != "session.steer" {
            continue;
        }
        let Some(attachments) = value["attachments"].as_array() else { continue };
        if !attachments.iter().any(|a| is_image(a["mediaType"].as_str().unwrap_or(""))) {
            continue;
        }
        let request_id = value["requestId"].as_str();
        let mut images = Vec::new();
        for (index, a) in attachments.iter().enumerate() {
            let key = attachment_key(&plan.source.thread_id, request_id, index as f64);
            let kept = key.as_ref().and_then(|k| held.iter().find(|h| &h.key == k));
            let bytes = match kept {
                Some(k) => store.get_blob(ATTACHMENTS, &k.key).await?,
                None => None,
            };
            let name = a["name"].as_str().unwrap_or("").to_string();
            let media_type = a["mediaType"].as_str().unwrap_or("").to_string();
            // An entry that is no image, or whose bytes are gone, keeps its place so each image keeps its own index.
            images.push(match bytes {
                Some(bytes) if is_image(&media_type) => Attachment { media_type, bytes: base64_of(&bytes), name: Some(name) },
                _ => Attachment { media_type: "text/plain".to_string(), bytes: String::new(), name: Some(name) },
            });
        }
        ctx.keep_sent_images(workspace_id, thread_id, request_id.map(String::from), images).await;
    }
    Ok(())
}

/// Lowercase base64 with padding, the RFC 4648 alphabet, as `Buffer.toString("base64")` writes it.
fn base64_of(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Readies a fork of one of a harness's sessions through a turn, the agent's refusal said in the caller's words.
/// forks.ts:181: F9.
pub async fn ready_fork(adapter: Arc<dyn HarnessAdapter>, session: &str, turn: ForkTurn, said: &dyn Fn(&str) -> HostError) -> Result<ForkedSession, HostError> {
    let Some(ready) = adapter.fork_session(session.to_string(), turn) else {
        return Err(said("adapter.forkSession is not a function"));
    };
    ready.await.map_err(|why| said(&why))
}

/// A thread moved onto a copy of its session: every row of it that named the session names the copy, which is what its
/// next turn resumes and what its facts are read under. forks.ts:191: F10.
pub fn move_session(ctx: &Rc<dyn ForksCtx>, thread_id: &str, from: &str, to: &str) {
    let sessions = ctx.sessions();
    for row in sessions.borrow().values() {
        let mut row = row.borrow_mut();
        if row.view.thread_id.as_deref() == Some(thread_id) && row.view.claude_session_id.as_deref() == Some(from) {
            row.view.claude_session_id = Some(to.to_string());
        }
    }
}

/// A fork's start: in its source's folder, or with a branch in a new worktree of the source's repo at the turn's
/// checkpoint, made where this computer makes them and taken away again, branch and all, where the start refuses.
/// forks.ts:199: F11.
pub async fn fork_start(ctx: &Rc<dyn ForksCtx>, start: StartFn, o: ForkOptions, origin: Option<Caller>) -> Result<Rc<dyn SessionHandle>, HostError> {
    let plan = read_fork(ctx, &o.fork, origin.clone()).await?;
    let entry = plan.source.entry.clone();
    let harness = plan.source.harness.clone();
    if let Some(asked) = &o.start.harness {
        if asked != &harness {
            return Err(refusal(&fork_agent_line(&ctx.agent_label(&harness), &ctx.agent_label(asked)), &fork_agent_fix(&ctx.agent_label(asked)), Some(Kind::Usage)));
        }
    }
    let ForkOptions { start: options, fork, branch } = o;
    let options = StartOptions { fork: Some(fork), ..options };
    let Some(branch) = branch else { return start(plan.source.workspace_id.clone(), options).await };
    let record = entry.borrow().record.borrow().clone();
    let project = ctx.project_held(&record.project);
    if !copies_folder(&record.kind) {
        return Err(usage_refusal(&place_branch_line(&ctx.place_name(&project.computer)), FORK_NO_CHECKPOINT_FIX));
    }
    let Some(top) = project.git.as_ref().map(|git| git.top.clone()) else {
        return Err(usage_refusal(&no_branches_line(&project.name), FORK_NO_CHECKPOINT_FIX));
    };
    let no_checkpoint = || refusal(FORK_NO_CHECKPOINT_LINE, FORK_NO_CHECKPOINT_FIX, Some(Kind::Conflict));
    let Some(at_ref) = plan.turn.as_ref().and_then(|turn| turn.at_ref.clone()) else { return Err(no_checkpoint()) };
    let checked = ctx.git_here(&top, vec!["rev-parse".into(), "--verify".into(), "-q".into(), format!("{at_ref}^{{commit}}")]).await?;
    if checked.exit_code.get() != 0 {
        return Err(no_checkpoint());
    }
    let before = ctx.live_ids();
    let root = scope_of(origin.as_ref()).map(|scope| scope.root_thread_id);
    let made = ctx.worktree_folder(&project, &top, &branch, &record.id, root, &at_ref).await?;
    let made_record = made.borrow().record.borrow().clone();
    let tree = made_record.worktree.clone();
    // The fork runs where in its tree its source ran in the source's checkout.
    let from = ctx.checkout_of(&record);
    let cwd = tree.as_ref().map(|tree| match plan.source.cwd.as_deref() {
        Some(source) if source.starts_with(&format!("{from}/")) => Path::new(&tree.path).join(&source[from.len() + 1..]).to_string_lossy().into_owned(),
        _ => tree.path.clone(),
    });
    let setup = ctx.take_setup_line(&made_record.id);
    let mut options = options;
    if let Some(cwd) = cwd {
        options.cwd = Some(cwd);
    }
    if let Some(setup) = setup {
        options.setup = Some(setup);
    }
    let made_id = made_record.id.clone();
    match start(made_id.clone(), options).await {
        Ok(handle) => Ok(handle),
        Err(e) => {
            if let Some(tree) = tree.as_ref().filter(|tree| tree.made && !before.contains(&made_id)) {
                if let Err(gone) = ctx.remove_worktree(&made, true).await {
                    console::warn(&format!("the worktree a refused fork made at {} was not removed: {}", tree.path, gone.message));
                }
                let _ = ctx.git_here(&top, vec!["branch".into(), "-D".into(), branch.clone()]).await;
            }
            Err(e)
        }
    }
}

/// The session a thread resumes whose record still owes a rewind's cut, which a host before copied rewinds wrote: a
/// copy through that cut. A copy refused for any reason, a session file Claude Code cleaned away or a read that ran
/// out of time alike, owes nothing more, and the resume runs uncut. forks.ts:231: F12.
pub async fn pay_cut_owed(ctx: &Rc<dyn ForksCtx>, adapter: Arc<dyn HarnessAdapter>, workspace_id: &str, thread_id: &str, resume: &str) -> String {
    let owed = ctx.thread_records().borrow().get(thread_id).and_then(|r| r.cut_owed.clone());
    let Some(owed) = owed else { return resume.to_string() };
    let Some(ready) = adapter.fork_session(resume.to_string(), ForkTurn::Anchor(owed)) else {
        return resume.to_string();
    };
    let answer = match ready.await {
        Ok(ForkedSession::Resume { resume: copy, .. }) => {
            move_session(ctx, thread_id, resume, &copy);
            copy
        }
        Ok(ForkedSession::Fork { .. }) => resume.to_string(),
        Err(why) => {
            console::warn(&format!("thread {} resumes without its owed cut: {why}", thread_word(thread_id)));
            resume.to_string()
        }
    };
    if let Some(record) = ctx.thread_records().borrow_mut().get_mut(thread_id) {
        record.cut_owed = None;
    }
    let _ = tokio::task::spawn_local(ctx.persist_sessions(workspace_id));
    answer
}
