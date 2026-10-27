// SPDX-License-Identifier: AGPL-3.0-only
//! The host's git writes one at a time, removing a worktree wsp made, a worktree gone, and the one sweep.
//! folders.ts:201-358, 448-472 at 65f6b89c7; spec/folders_tree.md.
use std::path::Path;
use std::rc::Rc;

use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use serde_json::{json, Value};
use wsp_core::console::warn;
use wsp_core::types::wiring::{last_line_of, NO_COPIER_HERE};
use wsp_engine::exec_detached::INLINE_EXEC_MS;
use wsp_frames::True;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::projects::{KEPT_ABANDONED_LINE, KEPT_RUNNING_LINE, WORKTREE_BUSY_LINE};
use wsp_wire::types::pull_request::PullRequestState;
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::views::workspace_more::{WorktreeFolder, WorktreeSettled, WorktreeSettledWhy};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::{HostError, Kind};
use wsp_words::js::js_trim;
use wsp_words::projects::{kept_changed_line, not_made_worktree_line, worktree_changed_line};
use wsp_words::words::computer::thread_word;

use super::place::under;
use super::{Folders, WorktreeRemove};
use crate::projects::workspaces::seam::{record_of, DaemonAsk, DaemonWork, Entry, GitAsk, Queued};

/// `GitAsk` for a git run in `cwd` with no timeout and no input.
fn git_ask(cwd: &str, args: &[&str]) -> GitAsk {
    GitAsk { cwd: cwd.to_owned(), args: args.iter().map(|arg| arg.to_string()).collect(), timeout_ms: None, input: None }
}

/// How often the sweep runs. folders.ts:274.
pub const SWEEP_MS: i64 = 10 * 60_000;
/// How long a settled worktree stands before it goes. folders.ts:275.
pub const SETTLED_STANDS_MS: i64 = 6 * 3_600_000;
/// How long one that could not go is tried before it is the person's. folders.ts:276.
pub const KEPT_FOR_MS: i64 = 7 * 24 * 3_600_000;

/// `removeWorktree`'s options. folders.ts:216.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RemoveAsk {
    /// The workspace is being deleted: no busy check, and the record is the delete's to drop.
    pub ending: bool,
    /// Refuse as a removal would, remove nothing.
    pub check: bool,
}

/// A worktree as git lists it. folders.ts:458.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedWorktree {
    pub path: String,
    pub branch: Option<String>,
}

// ts: folders.ts:450-453
//|   /** The last line git said, stderr first, for a sentence that quotes it. */
//|   const lastLine = (res: { stdout: string; stderr: string }): string => (res.stderr.trim() || res.stdout.trim()).split("\n").at(-1) ?? "";
//|   /** What git said went wrong: its first error or fatal line, not a hint printed under it, else its last line. */
//|   const gitSaid = (res: { stdout: string; stderr: string }): string => res.stderr.split("\n").find(l => /^(error|fatal): /.test(l))?.trim() ?? lastLine(res);
/// What git said went wrong: its first error or fatal line, else its last line.
pub fn git_said(res: &ExecResult) -> String {
    match res.stderr.split('\n').find(|line| line.starts_with("error: ") || line.starts_with("fatal: ")) {
        Some(line) => js_trim(line).to_owned(),
        None => last_line_of_result(res),
    }
}

/// The last line of `stderr.trim() || stdout.trim()`, untrimmed as the TS splits it.
fn last_line_of_result(res: &ExecResult) -> String {
    let whole = if js_trim(&res.stderr).is_empty() { js_trim(&res.stdout) } else { js_trim(&res.stderr) };
    whole.rsplit('\n').next().unwrap_or("").to_owned()
}

// ts: folders.ts:462-466
//|     for (const line of read.stdout.split("\n")) {
//|       if (line.startsWith("worktree ")) trees.push({ path: line.slice("worktree ".length) });
//|       else if (line.startsWith("branch refs/heads/") && trees.length > 0) trees[trees.length - 1]!.branch = line.slice("branch refs/heads/".length);
//|     }
//|     return trees;
/// Parses `git worktree list --porcelain`.
pub fn parse_worktrees(stdout: &str) -> Vec<ListedWorktree> {
    let mut trees: Vec<ListedWorktree> = Vec::new();
    for line in stdout.split('\n') {
        if let Some(path) = line.strip_prefix("worktree ") {
            trees.push(ListedWorktree { path: path.to_owned(), branch: None });
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            if let Some(last) = trees.last_mut() {
                last.branch = Some(branch.to_owned());
            }
        }
    }
    trees
}

impl Folders {
    // ts: folders.ts:201-212
    //|   /** The host's own git writes on one folder, one at a time: two threads' commits, a discard and a removal never
    //|    * interleave. The agents' own git is theirs, and git's index lock is the answer when theirs meets this. */
    //|   const gitWrites = new Map<string, Promise<unknown>>();
    //|   const queued = <T>(id: string, work: () => Promise<T>): Promise<T> => {
    //|     const run = (gitWrites.get(id) ?? Promise.resolve()).catch(() => {}).then(work);
    //|     const tail = run.catch(() => {});
    //|     gitWrites.set(id, tail);
    //|     void tail.then(() => {
    //|       if (gitWrites.get(id) === tail) gitWrites.delete(id);
    //|     });
    //|     return run;
    //|   };
    /// The host's own git writes on one folder, one at a time; the caller gets its own result (chain, first form).
    pub fn queued(&self, id: &str, work: Queued<serde_json::Value>) -> Queued<serde_json::Value> {
        let previous = self.git_writes.borrow().get(id).cloned();
        let tail_slot: Rc<std::cell::RefCell<Option<Shared<LocalBoxFuture<'static, ()>>>>> = Rc::new(std::cell::RefCell::new(None));
        let writes = self.git_writes.clone();
        let key = id.to_owned();
        let own_slot = tail_slot.clone();
        let run = async move {
            if let Some(previous) = previous {
                previous.await;
            }
            let answer = work.await;
            let tail = own_slot.borrow().clone();
            if let Some(tail) = tail {
                let mut held = writes.borrow_mut();
                if held.get(&key).is_some_and(|current| current.ptr_eq(&tail)) {
                    held.shift_remove(&key);
                }
            }
            answer
        }
        .boxed_local()
        .shared();
        let tail = run.clone().map(|_| ()).boxed_local().shared();
        *tail_slot.borrow_mut() = Some(tail.clone());
        self.git_writes.borrow_mut().insert(id.to_owned(), tail);
        run.boxed_local()
    }

    // ts: folders.ts:213-232
    //|   /** Takes a worktree wsp made away with git, under the folder's queue: never while a turn runs there and never over
    //|    * files no commit holds unless forced; the verb keeps a detached HEAD under refs/rescue first. The record stays,
    //|    * gone, while threads name it, and goes with the last of them. */
    //|   const removeWorktree = (entry: LiveWorkspace, force: boolean, o: { ending?: boolean; check?: boolean } = {}): Promise<void> =>
    //|     queued(entry.record.id, async () => {
    //|       const tree = entry.record.worktree;
    //|       const project = ctx.projectHeld(entry.record.project);
    //|       const top = project.git?.top;
    //|       if (tree === undefined || tree.made !== true || top === undefined) throw new Error(notMadeWorktreeLine(tree?.branch ?? ""));
    //|       const copier = local?.copier;
    //|       if (copier === undefined) throw Object.assign(new Error(NO_COPIER_HERE), { kind: "invalid" });
    //|       if (o.ending !== true && ctx.turnRuns(entry.record.id)) throw Object.assign(new Error(WORKTREE_BUSY_LINE), { kind: "conflict" });
    //|       if (existsSync(tree.path) && !force) await refuseChanged(tree.path);
    //|       if (o.check === true) return;
    //|       if (existsSync(tree.path)) {
    //|         await copier.worktreeRemove({ from: top, home: ctx.stateFolder(), path: tree.path, force });
    //|       }
    //|       setupLines.delete(entry.record.id);
    //|       if (o.ending !== true) await worktreeGone(entry, "removed");
    //|     });
    /// Takes a worktree wsp made away, under the folder's queue (spec rule 2).
    pub async fn remove_worktree(self: &Rc<Self>, entry: &Entry, force: bool, ask: RemoveAsk) -> Result<(), HostError> {
        let id = record_of(entry).id;
        let me = self.clone();
        let target = entry.clone();
        let work: Queued<Value> = async move {
            let record = record_of(&target);
            let top = me.ctx.project_held(&record.project)?.git.map(|git| git.top);
            let (Some(tree), Some(top)) = (record.worktree.clone().filter(|tree| tree.made), top) else {
                let branch = record.worktree.as_ref().and_then(|tree| tree.branch.clone()).unwrap_or_default();
                return Err(HostError::new(not_made_worktree_line(&branch)));
            };
            if !me.ctx.has_copier() {
                return Err(HostError::with_kind(NO_COPIER_HERE, Kind::Invalid));
            }
            if !ask.ending && me.ctx.turn_runs(&record.id) {
                return Err(HostError::with_kind(WORKTREE_BUSY_LINE, Kind::Conflict));
            }
            if Path::new(&tree.path).exists() && !force {
                me.refuse_changed(&tree.path).await?;
            }
            if ask.check {
                return Ok(Value::Null);
            }
            if Path::new(&tree.path).exists() {
                let remove = WorktreeRemove { from: top, home: me.ctx.state_folder()?, path: tree.path.clone(), force };
                me.ctx.copier_worktree_remove(remove).await?;
            }
            me.setup_lines.borrow_mut().shift_remove(&record.id);
            if !ask.ending {
                me.worktree_gone(&target, WorktreeSettledWhy::Removed, false).await?;
            }
            Ok(Value::Null)
        }
        .boxed_local();
        self.queued(&id, work).await.map(|_| ())
    }

    // ts: folders.ts:233-239
    //|   /** Refuses over files no commit holds in a folder, naming how many: they would go with a removal. */
    //|   const refuseChanged = async (path: string): Promise<void> => {
    //|     const read = await ctx.gitHere(path, ["status", "--porcelain"]);
    //|     if (read.exitCode !== 0) throw new Error(read.stderr.trim() || read.stdout.trim());
    //|     const changed = read.stdout.split("\n").filter(l => l.trim() !== "").length;
    //|     if (changed > 0) throw Object.assign(new Error(worktreeChangedLine(changed)), { kind: "conflict" });
    //|   };
    /// Refuses over files no commit holds in a folder, naming how many.
    pub async fn refuse_changed(&self, path: &str) -> Result<(), HostError> {
        let read = self.ctx.git_here(git_ask(path, &["status", "--porcelain"])).await?;
        if read.exit_code.get() != 0 {
            let said = if js_trim(&read.stderr).is_empty() { js_trim(&read.stdout) } else { js_trim(&read.stderr) };
            return Err(HostError::new(said));
        }
        let changed = read.stdout.split('\n').filter(|line| !js_trim(line).is_empty()).count();
        if changed > 0 {
            return Err(HostError::with_kind(worktree_changed_line(changed as f64), Kind::Conflict));
        }
        Ok(())
    }

    // ts: folders.ts:240-248
    //|   /** Drops the checkpoint refs one thread holds in its folder's repo, best effort: a ref left behind pins files and
    //|    * nothing else, and a delete is not refused for it. */
    //|   const dropCheckpoints = async (entry: LiveWorkspace, threadId: string): Promise<void> => {
    //|     await ctx.checkpointsLanding.get(threadId)?.catch(() => {});
    //|     if (ctx.notARepo(entry.record)) return;
    //|     await ctx.withDaemon(entry, ask => ask({ op: "git.checkpointDrop", cwd: ctx.checkoutOf(entry.record), scope: entry.record.id, thread: threadId })).catch((e: unknown) =>
    //|       console.warn(`the checkpoints of thread ${threadWord(threadId)} were not dropped: ${e instanceof Error ? e.message : String(e)}`),
    //|     );
    //|   };
    /// Drops the checkpoint refs one thread holds in its folder's repo, best effort.
    pub async fn drop_checkpoints(&self, entry: &Entry, thread_id: &str) -> Result<(), HostError> {
        self.ctx.checkpoints_landed(thread_id).await;
        let record = record_of(entry);
        if self.ctx.not_a_repo(&record)? {
            return Ok(());
        }
        let ctx = self.ctx.clone();
        let thread = thread_id.to_owned();
        let work: DaemonWork = Box::new(move |ask: DaemonAsk| {
            async move {
                let cwd = ctx.checkout_of(&record)?;
                ask(json!({ "op": "git.checkpointDrop", "cwd": cwd, "scope": record.id, "thread": thread })).await
            }
            .boxed_local()
        });
        if let Err(e) = self.ctx.with_daemon(entry, work).await {
            warn(&format!("the checkpoints of thread {} were not dropped: {}", thread_word(thread_id), e.message));
        }
        Ok(())
    }

    // ts: folders.ts:249-251
    //|   /** Whether any thread names a record, by a row of its turns or by its own record. */
    //|   const holdsThread = (workspaceId: string): boolean =>
    //|     [...sessions.values()].some(s => s.view.workspaceId === workspaceId) || [...threadRecords.values()].some(t => t.workspaceId === workspaceId);
    /// Whether any thread names a record, by a row of its turns or by its own record.
    pub fn holds_thread(&self, workspace_id: &str) -> bool {
        self.ctx.session_views().iter().any(|view| view.workspace_id == workspace_id)
            || self.ctx.thread_records().iter().any(|(_, thread)| thread.workspace_id == workspace_id)
    }

    // ts: folders.ts:252-264
    //|   /** A worktree no longer on disk: its threads go on in the project folder, and its record goes once no thread is
    //|    * left on it. */
    //|   const worktreeGone = async (entry: LiveWorkspace, why: WorktreeSettled["why"], o: { starting?: boolean } = {}): Promise<void> => {
    //|     const tree = entry.record.worktree;
    //|     if (tree === undefined) return;
    //|     // A start found it gone, and its thread is about to name the record.
    //|     if (o.starting !== true && !holdsThread(entry.record.id)) {
    //|       await ctx.drop(entry.record.id);
    //|       return;
    //|     }
    //|     entry.record.worktree = { path: tree.path, made: tree.made, ...(tree.branch !== undefined ? { branch: tree.branch } : {}), ...(tree.madeFor !== undefined ? { madeFor: tree.madeFor } : {}), settled: tree.settled ?? { at: clock.now(), why }, gone: true };
    //|     await ctx.persist(entry.record);
    //|   };
    /// A worktree no longer on disk: its record goes once no thread is left on it, else it is marked gone.
    pub async fn worktree_gone(&self, entry: &Entry, why: WorktreeSettledWhy, starting: bool) -> Result<(), HostError> {
        let record = record_of(entry);
        let Some(tree) = record.worktree.clone() else {
            return Ok(());
        };
        if !starting && !self.holds_thread(&record.id) {
            return self.ctx.drop_workspace(&record.id).await;
        }
        let settled = tree.settled.clone().unwrap_or_else(|| WorktreeSettled { at: JsNumber::from(self.ctx.clock().now()), why });
        let next = WorktreeFolder {
            path: tree.path,
            made: tree.made,
            branch: tree.branch,
            made_for: tree.made_for,
            settled: Some(settled),
            kept: None,
            remove_failed: None,
            gone: Some(True),
        };
        entry.borrow().record.borrow_mut().worktree = Some(next);
        self.ctx.persist(record_of(entry)).await
    }

    // ts: folders.ts:265-271
    //|   /** The folder a thread's next turn runs in: where its session last ran, unless that was a worktree that is gone,
    //|    * whose threads go on in the folder the record answers now. */
    //|   const runsIn = (entry: LiveWorkspace, ranIn: string | undefined, folder: string): string => {
    //|     const tree = entry.record.worktree;
    //|     if (ranIn === undefined) return folder;
    //|     return tree?.gone === true && under(ranIn, tree.path) ? folder : ranIn;
    //|   };
    /// The folder a thread's next turn runs in.
    pub fn runs_in(&self, entry: &Entry, ran_in: Option<&str>, folder: &str) -> String {
        let Some(ran_in) = ran_in else {
            return folder.to_owned();
        };
        match record_of(entry).worktree {
            Some(tree) if tree.gone.is_some() && under(ran_in, &tree.path) => folder.to_owned(),
            _ => ran_in.to_owned(),
        }
    }

    // ts: folders.ts:277-278
    //|   /** Whether any worktree wsp made still stands, which is all the sweep has to look at. */
    //|   const sweepHasWork = (): boolean => [...live.values()].some(e => e.record.worktree?.made === true && e.record.worktree.gone !== true);
    /// Whether any worktree wsp made still stands.
    pub fn sweep_has_work(&self) -> bool {
        let live = self.ctx.live();
        let standing = live.borrow().values().any(|entry| record_of(entry).worktree.is_some_and(|tree| tree.made && tree.gone.is_none()));
        standing
    }

    // ts: folders.ts:279-296
    //|   /** Armed only while a worktree wsp made stands: a host with none holds no timer for it. */
    //|   const armSweep = (): void => {
    //|     if (ctx.state.sweepTimer !== undefined || ctx.state.sweeping !== undefined) return;
    //|     if (ctx.state.sweepStopped || !sweepHasWork()) return;
    //|     ctx.state.sweepTimer = clock.schedule(
    //|       () => {
    //|         ctx.state.sweepTimer = undefined;
    //|         ctx.state.sweeping = sweepWorktrees()
    //|           .catch((e: unknown) => console.warn(`the sweep of worktrees stopped: ${e instanceof Error ? e.message : String(e)}`))
    //|           .finally(() => {
    //|             ctx.state.sweeping = undefined;
    //|             armSweep();
    //|           });
    //|       },
    //|       SWEEP_MS,
    //|       { unref: true },
    //|     );
    //|   };
    /// Arms the sweep's timer, only while a worktree wsp made stands.
    pub fn arm_sweep(self: &Rc<Self>) {
        let state = self.ctx.state();
        if state.with(|s| s.sweep_timer.is_some() || s.sweeping.is_some() || s.sweep_stopped) {
            return;
        }
        if !self.sweep_has_work() {
            return;
        }
        let me = self.clone();
        let cancel = self.ctx.clock().schedule(Box::new(move || me.start_sweep()), SWEEP_MS);
        state.with(|s| s.sweep_timer = Some(cancel));
    }

    /// The timer's body: the sweep in flight, which re-arms the timer once it settles.
    fn start_sweep(self: &Rc<Self>) {
        let state = self.ctx.state();
        state.with(|s| s.sweep_timer = None);
        let me = self.clone();
        let sweeping = async move {
            if let Err(e) = me.sweep_worktrees().await {
                warn(&format!("the sweep of worktrees stopped: {}", e.message));
            }
            me.ctx.state().with(|s| s.sweeping = None);
            me.arm_sweep();
        }
        .boxed_local()
        .shared();
        state.with(|s| s.sweeping = Some(sweeping.clone()));
        self.ctx.spawn(sweeping.boxed_local());
    }

    // ts: folders.ts:297-304
    //|   /** Every worktree wsp made and still holds, one at a time, so a sweep is never more git than one folder's at once. */
    //|   const sweepWorktrees = async (): Promise<void> => {
    //|     for (const entry of [...live.values()]) {
    //|       const tree = entry.record.worktree;
    //|       if (tree?.made !== true || tree.gone === true || live.get(entry.record.id) !== entry) continue;
    //|       await sweepOne(entry).catch((e: unknown) => console.warn(`the worktree at ${tree.path} was not swept: ${e instanceof Error ? e.message : String(e)}`));
    //|     }
    //|   };
    /// Every worktree wsp made and still holds, one at a time.
    pub async fn sweep_worktrees(self: &Rc<Self>) -> Result<(), HostError> {
        let snapshot: Vec<Entry> = self.ctx.live().borrow().values().cloned().collect();
        for entry in snapshot {
            let record = record_of(&entry);
            let Some(tree) = record.worktree.clone() else {
                continue;
            };
            if !tree.made || tree.gone.is_some() {
                continue;
            }
            let held = self.ctx.live().borrow().get(&record.id).is_some_and(|live| Rc::ptr_eq(live, &entry));
            if !held {
                continue;
            }
            if let Err(e) = self.sweep_one(&entry).await {
                warn(&format!("the worktree at {} was not swept: {}", tree.path, e.message));
            }
        }
        Ok(())
    }

    // ts: folders.ts:305-332
    //|   /** One worktree wsp made: settled when its pull request merged or closed, when the branch it pushed is gone at the
    //|    * remote, or when the person removed it; once settled six hours with nothing uncommitted and no turn running it
    //|    * goes, and otherwise says why it stays, until a week has passed and it is left to the person. */
    //|   const sweepOne = async (entry: LiveWorkspace): Promise<void> => {
    //|     const tree = entry.record.worktree!;
    //|     if (!existsSync(tree.path)) return worktreeGone(entry, "removed");
    //|     const settled = tree.settled ?? (await settledNow(entry));
    //|     if (settled === undefined || tree.kept === KEPT_ABANDONED_LINE) return;
    //|     const age = clock.now() - settled.at;
    //|     if (age < SETTLED_STANDS_MS) return;
    //|     const keep = async (said: { kept?: string; removeFailed?: string }): Promise<void> => {
    //|       const now = entry.record.worktree!;
    //|       const next: WorktreeFolder = { ...now, ...said, ...(age >= KEPT_FOR_MS ? { kept: KEPT_ABANDONED_LINE } : {}) };
    //|       if (JSON.stringify(next) === JSON.stringify(now)) return;
    //|       entry.record.worktree = next;
    //|       await ctx.persist(entry.record);
    //|       await ctx.statusNow(entry);
    //|     };
    //|     if (ctx.turnRuns(entry.record.id)) return keep({ kept: KEPT_RUNNING_LINE });
    //|     const status = await ctx.gitHere(tree.path, ["status", "--porcelain"]);
    //|     const changed = status.stdout.split("\n").filter(l => l.trim() !== "").length;
    //|     if (status.exitCode === 0 && changed > 0) return keep({ kept: keptChangedLine(changed) });
    //|     try {
    //|       await removeWorktree(entry, false);
    //|     } catch (e) {
    //|       await keep({ removeFailed: lastLineOf(e instanceof Error ? e.message : String(e)) });
    //|     }
    //|   };
    /// One worktree wsp made: settled, then removed, or kept with why (spec rule 11).
    pub async fn sweep_one(self: &Rc<Self>, entry: &Entry) -> Result<(), HostError> {
        let record = record_of(entry);
        let Some(tree) = record.worktree.clone() else {
            return Ok(());
        };
        if !Path::new(&tree.path).exists() {
            return self.worktree_gone(entry, WorktreeSettledWhy::Removed, false).await;
        }
        let settled = match tree.settled.clone() {
            Some(settled) => Some(settled),
            None => self.settled_now(entry).await?,
        };
        let Some(settled) = settled else {
            return Ok(());
        };
        if tree.kept.as_deref() == Some(KEPT_ABANDONED_LINE) {
            return Ok(());
        }
        let age = self.ctx.clock().now() as f64 - settled.at.0;
        if age < SETTLED_STANDS_MS as f64 {
            return Ok(());
        }
        let abandon = age >= KEPT_FOR_MS as f64;
        if self.ctx.turn_runs(&record.id) {
            return self.keep_tree(entry, |next| next.kept = Some(KEPT_RUNNING_LINE.to_owned()), abandon).await;
        }
        let status = self.ctx.git_here(git_ask(&tree.path, &["status", "--porcelain"])).await?;
        let changed = status.stdout.split('\n').filter(|line| !js_trim(line).is_empty()).count();
        if status.exit_code.get() == 0 && changed > 0 {
            return self.keep_tree(entry, |next| next.kept = Some(kept_changed_line(changed as f64)), abandon).await;
        }
        match self.remove_worktree(entry, false, RemoveAsk::default()).await {
            Ok(()) => Ok(()),
            Err(e) => self.keep_tree(entry, |next| next.remove_failed = Some(last_line_of(&e.message)), abandon).await,
        }
    }

    /// Writes the tree with `said` over it, and at a week or more `kept` as abandoned, when that changes its JSON text.
    async fn keep_tree(&self, entry: &Entry, said: impl FnOnce(&mut WorktreeFolder), abandon: bool) -> Result<(), HostError> {
        let Some(now) = record_of(entry).worktree else {
            return Ok(());
        };
        let mut next = now.clone();
        said(&mut next);
        if abandon {
            next.kept = Some(KEPT_ABANDONED_LINE.to_owned());
        }
        if serde_json::to_string(&next).ok() == serde_json::to_string(&now).ok() {
            return Ok(());
        }
        entry.borrow().record.borrow_mut().worktree = Some(next);
        self.ctx.persist(record_of(entry)).await?;
        self.ctx.status_now(entry).await
    }

    // ts: folders.ts:333-349
    //|   /** Whether a worktree wsp made settled since the last sweep, stamped on its record when it did. Only a branch with an
    //|    * upstream is asked of its remote, and only the remote saying it has no such branch counts; no remote, no upstream
    //|    * or a remote that could not be read is no answer, so a branch never pushed keeps its worktree. */
    //|   const settledNow = async (entry: LiveWorkspace): Promise<WorktreeSettled | undefined> => {
    //|     const tree = entry.record.worktree!;
    //|     if (ctx.projectHeld(entry.record.project).remote === "") return undefined;
    //|     await ctx.readPullRequest(entry, true);
    //|     const pr = entry.record.pr;
    //|     let settled: WorktreeSettled | undefined =
    //|       pr?.state === "merged" ? { at: pr.mergedAt ?? clock.now(), why: "merged" } : pr?.state === "closed" ? { at: pr.closedAt ?? clock.now(), why: "closed" } : undefined;
    //|     if (settled === undefined && tree.branch !== undefined && (await branchGoneAtRemote(tree.path, tree.branch))) settled = { at: clock.now(), why: "deleted" };
    //|     if (settled === undefined) return undefined;
    //|     entry.record.worktree = { ...entry.record.worktree!, settled };
    //|     await ctx.persist(entry.record);
    //|     await ctx.statusNow(entry);
    //|     return settled;
    //|   };
    /// Whether a worktree wsp made settled since the last sweep, stamped on its record when it did.
    pub async fn settled_now(&self, entry: &Entry) -> Result<Option<WorktreeSettled>, HostError> {
        let record = record_of(entry);
        let Some(tree) = record.worktree.clone() else {
            return Ok(None);
        };
        if self.ctx.project_held(&record.project)?.remote.is_empty() {
            return Ok(None);
        }
        self.ctx.read_pull_request(entry, true).await?;
        let now = self.ctx.clock().now();
        let pr = record_of(entry).pr;
        let mut settled = match &pr {
            Some(pr) if matches!(pr.state, PullRequestState::Merged) => {
                Some(WorktreeSettled { at: pr.merged_at.unwrap_or_else(|| JsNumber::from(now)), why: WorktreeSettledWhy::Merged })
            }
            Some(pr) if matches!(pr.state, PullRequestState::Closed) => {
                Some(WorktreeSettled { at: pr.closed_at.unwrap_or_else(|| JsNumber::from(now)), why: WorktreeSettledWhy::Closed })
            }
            _ => None,
        };
        if settled.is_none() {
            if let Some(branch) = tree.branch.as_deref() {
                if self.branch_gone_at_remote(&tree.path, branch).await? {
                    settled = Some(WorktreeSettled { at: JsNumber::from(self.ctx.clock().now()), why: WorktreeSettledWhy::Deleted });
                }
            }
        }
        let Some(settled) = settled else {
            return Ok(None);
        };
        if let Some(mut current) = record_of(entry).worktree {
            current.settled = Some(settled.clone());
            entry.borrow().record.borrow_mut().worktree = Some(current);
        }
        self.ctx.persist(record_of(entry)).await?;
        self.ctx.status_now(entry).await?;
        Ok(Some(settled))
    }

    // ts: folders.ts:350-358
    //|   /** True only where the branch has an upstream and its remote answers that it holds no such branch (exit 2). */
    //|   const branchGoneAtRemote = async (path: string, branch: string): Promise<boolean> => {
    //|     const remote = (await ctx.gitHere(path, ["config", "--get", `branch.${branch}.remote`])).stdout.trim();
    //|     const merge = (await ctx.gitHere(path, ["config", "--get", `branch.${branch}.merge`])).stdout.trim();
    //|     if (remote === "" || merge === "" || remote === ".") return false;
    //|     const machine = await ctx.moduleOf("local").backend({ kind: "local" } as WorkspaceRecord).get(LOCAL_MACHINE_ID);
    //|     const asked = await machine.exec(`GIT_TERMINAL_PROMPT=0 ${shellLine(["git", "-C", path, "ls-remote", "--exit-code", "--heads", remote, merge])}`, { timeoutMs: INLINE_EXEC_MS });
    //|     return asked.exitCode === 2;
    //|   };
    /// True only where the branch has an upstream and its remote answers that it holds no such branch (exit 2).
    pub async fn branch_gone_at_remote(&self, path: &str, branch: &str) -> Result<bool, HostError> {
        let remote_key = format!("branch.{branch}.remote");
        let merge_key = format!("branch.{branch}.merge");
        let remote = self.ctx.git_here(git_ask(path, &["config", "--get", &remote_key])).await?;
        let merge = self.ctx.git_here(git_ask(path, &["config", "--get", &merge_key])).await?;
        let remote = js_trim(&remote.stdout).to_owned();
        let merge = js_trim(&merge.stdout).to_owned();
        if remote.is_empty() || merge.is_empty() || remote == "." {
            return Ok(false);
        }
        let cmd =
            format!("GIT_TERMINAL_PROMPT=0 {}", shell_line(&["git", "-C", path, "ls-remote", "--exit-code", "--heads", &remote, &merge]));
        let asked = self.ctx.local_exec(&cmd, INLINE_EXEC_MS, None).await?;
        Ok(asked.exit_code.get() == 2)
    }

    // ts: folders.ts:456-467
    //|   /** Every worktree of a repo as git lists it now, read each time: the person and their agents add and remove their
    //|    * own, so nothing here is kept. */
    //|   const worktreesOf = async (top: string): Promise<{ path: string; branch?: string }[]> => {
    //|     const read = await ctx.gitHere(top, ["worktree", "list", "--porcelain"]);
    //|     if (read.exitCode !== 0) return [];
    //|     const trees: { path: string; branch?: string }[] = [];
    //|     for (const line of read.stdout.split("\n")) {
    //|       if (line.startsWith("worktree ")) trees.push({ path: line.slice("worktree ".length) });
    //|       else if (line.startsWith("branch refs/heads/") && trees.length > 0) trees[trees.length - 1]!.branch = line.slice("branch refs/heads/".length);
    //|     }
    //|     return trees;
    //|   };
    /// Every worktree of a repo as git lists it now; none where git fails.
    pub async fn worktrees_of(&self, top: &str) -> Result<Vec<ListedWorktree>, HostError> {
        let read = self.ctx.git_here(git_ask(top, &["worktree", "list", "--porcelain"])).await?;
        if read.exit_code.get() != 0 {
            return Ok(Vec::new());
        }
        Ok(parse_worktrees(&read.stdout))
    }
}
