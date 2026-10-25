// SPDX-License-Identifier: AGPL-3.0-only
//! The host's git writes one at a time, removing a worktree wsp made, a worktree gone, and the one sweep.
//! folders.ts:201-358, 448-472 at 65f6b89c7; spec/folders_tree.md.
use std::rc::Rc;

use wsp_wire::views::workspace_more::{WorktreeSettled, WorktreeSettledWhy};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::Folders;
use crate::projects::workspaces::seam::{Entry, Queued};

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
    todo!()
}

// ts: folders.ts:462-466
//|     for (const line of read.stdout.split("\n")) {
//|       if (line.startsWith("worktree ")) trees.push({ path: line.slice("worktree ".length) });
//|       else if (line.startsWith("branch refs/heads/") && trees.length > 0) trees[trees.length - 1]!.branch = line.slice("branch refs/heads/".length);
//|     }
//|     return trees;
/// Parses `git worktree list --porcelain`.
pub fn parse_worktrees(stdout: &str) -> Vec<ListedWorktree> {
    todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
    }

    // ts: folders.ts:249-251
    //|   /** Whether any thread names a record, by a row of its turns or by its own record. */
    //|   const holdsThread = (workspaceId: string): boolean =>
    //|     [...sessions.values()].some(s => s.view.workspaceId === workspaceId) || [...threadRecords.values()].some(t => t.workspaceId === workspaceId);
    /// Whether any thread names a record, by a row of its turns or by its own record.
    pub fn holds_thread(&self, workspace_id: &str) -> bool {
        todo!()
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
        todo!()
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
        todo!()
    }

    // ts: folders.ts:277-278
    //|   /** Whether any worktree wsp made still stands, which is all the sweep has to look at. */
    //|   const sweepHasWork = (): boolean => [...live.values()].some(e => e.record.worktree?.made === true && e.record.worktree.gone !== true);
    /// Whether any worktree wsp made still stands.
    pub fn sweep_has_work(&self) -> bool {
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
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
        todo!()
    }
}
