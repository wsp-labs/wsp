// SPDX-License-Identifier: AGPL-3.0-only
// The branch of record: the one branch a thread on a machine whose credentials
// this host keeps may push, and the repo it is on by GitHub's id, written on the
// workspace's first record. A sub-thread's is its result branch, a pull request
// thread's the pull request's head, any other thread's `wsp/<slug>`. Read with gh
// on this computer, whose login is the one the host adds to every push.
import { gitHostOf, remoteHost } from "@wsp/catalog";
import { envExec, INLINE_EXEC_MS, LOCAL_MACHINE_ID, withEnvFromInput } from "@wsp/engine";
import { branchOfRecordUnreadRefusal, brokeredGitHubOnlyRefusal, cloneNoThreadRefusal, forkHeadGoneRefusal, forkNotPushableRefusal, lastLine, ownerRepoOf, shellLine, subBranchOf, type BranchOfRecord, type ProjectView } from "@wsp/protocol";
import { brokeredGitEnv, onRecordLines } from "../project-landing.js";
import { CLONE_MS, type LiveWorkspace, type WorkspaceRecord } from "../types/wiring.js";
import type { RuntimeContext } from "../context.js";

type Repo = Omit<BranchOfRecord, "branch">;

/** Whether a start on a place takes the road where the host keeps the machine's credentials: a provider's place, and
 * only once the host's broker answers ready. Until then every start keeps today's road. */
export const brokeredStart = (ctx: Pick<RuntimeContext, "opts">, provider: boolean): boolean => provider && ctx.opts.broker?.ready() === true;

/** One gh line on this computer with no prompt and a bounded wait: its tab-separated answer, refused naming the repo. */
async function ghHere(ctx: RuntimeContext, repo: string, args: readonly string[]): Promise<string[]> {
  const machine = await ctx.moduleOf("local").backend({ kind: "local" } as WorkspaceRecord).get(LOCAL_MACHINE_ID);
  const read = await machine.exec(`GH_PROMPT_DISABLED=1 ${shellLine(["gh", ...args])}`, { timeoutMs: INLINE_EXEC_MS }).catch((e: unknown) => ({ exitCode: -1, stdout: "", stderr: e instanceof Error ? e.message : String(e) }));
  if (read.exitCode !== 0) throw branchOfRecordUnreadRefusal(repo, lastLine(read.stderr) ?? `gh exited ${read.exitCode}`);
  return read.stdout.replace(/\r?\n$/, "").split("\t");
}

const shaOf = (repo: string, word: string | undefined): string => {
  if (word === undefined || !/^[0-9a-f]{40}$/.test(word)) throw branchOfRecordUnreadRefusal(repo, `no commit in ${JSON.stringify(word ?? "")}`);
  return word;
};

const idOf = (repo: string, word: string | undefined): number => {
  const id = Number(word);
  if (!Number.isSafeInteger(id) || id <= 0) throw branchOfRecordUnreadRefusal(repo, `no repository id in ${JSON.stringify(word ?? "")}`);
  return id;
};

/** owner/name off a remote GitHub holds; any other host is refused, since the host adds a credential for GitHub alone. */
function githubRepoOf(remote: string, project: string, on: string): string {
  const repo = gitHostOf(remote)?.id === "github" ? ownerRepoOf(remote) : undefined;
  if (repo === undefined) throw brokeredGitHubOnlyRefusal(project, remoteHost(remote) ?? remote, on);
  return repo;
}

/** The repo a remote names, as GitHub names it now: its id, and the owner and name a renamed repo answers with. */
export async function repoOfRecord(ctx: RuntimeContext, remote: string, project: string, on: string): Promise<Repo> {
  const repo = githubRepoOf(remote, project, on);
  const [id, owner, name] = await ghHere(ctx, repo, ["api", `repos/${repo}`, "--jq", "[.id, .owner.login, .name] | @tsv"]);
  return { repoId: idOf(repo, id), owner: owner ?? "", name: name ?? "" };
}

/** A pull request thread's branch of record: the pull request's head branch on the repo that holds it, read now. A
 * fork's head takes a thread's pushes only where its author allowed maintainers to push, so a start that pushes is
 * refused there before anything is made; a review pushes nothing and reads the head all the same. */
export async function pullRequestOfRecord(ctx: RuntimeContext, remote: string, number: number, project: string, on: string, pushes: boolean): Promise<BranchOfRecord> {
  const repo = githubRepoOf(remote, project, on);
  const [id, owner, name, branch, editable, base, head] = await ghHere(ctx, repo, ["api", `repos/${repo}/pulls/${number}`, "--jq", "[.head.repo.id, .head.repo.owner.login, .head.repo.name, .head.ref, .maintainer_can_modify, .base.repo.id, .head.sha] | @tsv"]);
  if (id === undefined || id === "") throw forkHeadGoneRefusal(number);
  if (pushes && id !== base && editable !== "true") throw forkNotPushableRefusal(owner ?? "", number);
  return { repoId: idOf(repo, id), owner: owner ?? "", name: name ?? "", branch: branch ?? "", startSha: shaOf(repo, head) };
}

/** The branch a thread on a machine of this host's is given where its start names none: `wanted`, numbered past a
 * branch another record of this host holds, a sub-thread's or a branch of record, or one the remote's heads list, the
 * `git ls-remote --heads` the caller read, has. The one rule for both, so two threads are never given one branch. */
export function freeBranchName(ctx: Pick<RuntimeContext, "live">, wanted: string, heads: string | undefined): string {
  const taken = new Set([...ctx.live.values()].flatMap(e => [e.record.branchOfRecord?.branch, e.record.sub?.branch]).filter(b => b !== undefined));
  for (const line of heads?.split("\n") ?? []) {
    const ref = line.split("\t")[1];
    if (ref?.startsWith("refs/heads/") === true) taken.add(ref.slice("refs/heads/".length));
  }
  let branch = wanted;
  for (let n = 2; taken.has(branch); n++) branch = `${wanted}-${n}`;
  return branch;
}

/** Any other thread's branch of record: `wsp/` and its name as a slug, by the one numbering rule against the remote's
 * heads read through this computer's git with no prompt. It starts from the base's tip as GitHub holds it now, the
 * default branch's where none is named, which the clone then takes; where the image already holds the checkout,
 * where that checkout stands, which only the machine can say, so the start commit is left to it. */
export async function namedOfRecord(ctx: RuntimeContext, o: { remote: string; name: string; project: string; on: string; base: string | undefined; carried: boolean }): Promise<BranchOfRecord> {
  const repo = await repoOfRecord(ctx, o.remote, o.project, o.on);
  const ownerRepo = `${repo.owner}/${repo.name}`;
  const tip = o.carried ? undefined : (await ghHere(ctx, ownerRepo, ["api", `repos/${ownerRepo}/commits/${o.base ?? "HEAD"}`, "--jq", ".sha"]))[0];
  const machine = await ctx.moduleOf("local").backend({ kind: "local" } as WorkspaceRecord).get(LOCAL_MACHINE_ID);
  const heads = await machine.exec(`GIT_TERMINAL_PROMPT=0 ${shellLine(["git", "ls-remote", "--heads", o.remote])}`, { timeoutMs: INLINE_EXEC_MS }).catch(() => undefined);
  const branch = freeBranchName(ctx, subBranchOf(o.name), heads?.exitCode === 0 ? heads.stdout : undefined);
  return { ...repo, branch, ...(o.carried ? {} : { startSha: shaOf(ownerRepo, tip) }) };
}

/** An op token for one command on a brokered machine, minted for the thread that asked for the machine or the thread
 * the start that made it opens, taken back when the command ends, and the git environment that carries it. A
 * workspace with neither has no thread to scope a token to, and its command is refused. */
export async function withOpToken<T>(ctx: RuntimeContext, entry: LiveWorkspace, op: string, run: (env: Record<string, string>) => Promise<T>): Promise<T> {
  const threadId = entry.record.parentThreadId ?? entry.opensThread;
  if (threadId === undefined) throw cloneNoThreadRefusal(entry.record.name);
  const scope = { kind: "thread" as const, threadId, workspaceId: entry.record.id, rootThreadId: entry.record.rootThreadId ?? threadId };
  const minted = await ctx.deviceDoor.mint(`op ${op}`, scope, ctx.clock.now(), { road: ctx.moduleOf(entry.record.kind).turnRoad });
  try {
    return await run(brokeredGitEnv(minted.deviceToken));
  } finally {
    await ctx.deviceDoor.revoke(minted.deviceId).catch((e: unknown) => console.warn(`the ${op} token of ${entry.record.name} was not taken away: ${e instanceof Error ? e.message : String(e)}`));
  }
}

/** Puts a brokered workspace's checkout on its branch of record, cloning first where `clone` holds the clone's lines,
 * through the host with an op token, and records the commit it started from where the record left that to the
 * machine. Refused with git's own last line. */
export async function landOnRecord(ctx: RuntimeContext, entry: LiveWorkspace, project: ProjectView, clone: readonly string[] | undefined): Promise<void> {
  const ofRecord = entry.record.branchOfRecord!;
  const lines = clone ?? ["set -e", ...onRecordLines({ checkout: project.path, ofRecord, origin: ownerRepoOf(project.remote) ?? "", held: entry.ofRecordHeld === true })];
  const ran = await withOpToken(ctx, entry, "clone", env => envExec(entry.machine, withEnvFromInput(lines.join("\n")), env, { timeoutMs: CLONE_MS }));
  if (ran.exitCode !== 0) throw new Error(lastLine(ran.stderr) ?? lastLine(ran.stdout) ?? `git exited ${ran.exitCode}`);
  const at = lastLine(ran.stdout);
  if (ofRecord.startSha !== undefined || at === undefined || !/^[0-9a-f]{40}$/.test(at)) return;
  entry.record.branchOfRecord = { ...ofRecord, startSha: at };
  await ctx.persist(entry.record);
}
