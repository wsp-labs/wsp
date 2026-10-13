// SPDX-License-Identifier: AGPL-3.0-only
// The push rule a cloud thread's git road applies to each command of a receive-pack request: the thread's branch of
// record on its recorded repo, created, updated, forced or deleted, and nothing else.
import type { BranchOfRecord } from "@wsp/protocol";

/** One `<old> <new> <ref>` command of a push. */
export interface RefCommand {
  old: string;
  new: string;
  ref: string;
}

const BRANCH_PREFIX = "refs/heads/";

/** The refusal sentence for one command, or undefined when the thread may push it. Force on its own branch needs no check. */
export function refusalFor(command: RefCommand, branch: string): string | undefined {
  const { ref } = command;
  if (ref === BRANCH_PREFIX + branch) return undefined;
  if (ref.startsWith(BRANCH_PREFIX)) return `wsp: ${ref.slice(BRANCH_PREFIX.length)} is not this thread's branch; push your work to ${branch}`;
  if (ref.startsWith("refs/tags/")) return "wsp: tags are made from your own computer, not from a cloud thread";
  return `wsp: ${ref} is not a branch; a cloud thread pushes only ${branch}`;
}

export const ATOMIC_REFUSAL = "wsp: refused with the rest of this atomic push";

export const pushRepoRefusal = (owner: string, name: string, record: BranchOfRecord): string =>
  `wsp: github.com/${owner}/${name} is not this thread's project; pushes go only to github.com/${record.owner}/${record.name}`;

export const repoHiddenRefusal = (owner: string, name: string): string =>
  `wsp: GitHub does not show github.com/${owner}/${name} to the gh sign-in on this computer; check gh auth status there`;

export const repoForbiddenRefusal = (owner: string, name: string): string =>
  `wsp: GitHub refused github.com/${owner}/${name} to the gh sign-in on this computer (SSO or a rate limit); check gh auth status there`;

export const SPOOL_FAILED = "wsp: your computer could not hold this push on its disk; free some space there and push again";
export const PACK_TOO_LARGE = "wsp: this push is larger than GitHub takes in one push; push it in smaller parts";
export const NO_BRANCH_REFUSAL = "wsp: this thread has no branch to push from a cloud machine; copy its commits to your computer and push them there";
export const NO_SIGNIN_REFUSAL = "wsp: sign gh in on this computer with gh auth login";
export const SIGNIN_REFUSED = "wsp: GitHub refused the gh sign-in on this computer; run gh auth login there";
export const PUSH_CERT_REFUSAL = "wsp: signed pushes are not taken from a cloud thread";
export const UNREADABLE_PUSH = "wsp: this push could not be read; push again with git";
export const GUARD_FAILED = "wsp: your computer could not check this push; push again";
export const UNREACHABLE = "wsp: GitHub did not answer your computer; try again";
export const lfsRefusal = "wsp: Git LFS does not run from a cloud thread";
export const pathRefusal = (path: string): string => `wsp: ${path} is not a git fetch or push`;

/** Each command with its verdict; under `atomic` one refusal refuses them all. */
export function judge(commands: RefCommand[], branch: string, atomic: boolean): { command: RefCommand; said?: string }[] {
  const judged = commands.map((command) => ({ command, said: refusalFor(command, branch) }));
  if (!atomic || judged.every((j) => !j.said)) return judged;
  return judged.map((j) => ({ command: j.command, said: j.said ?? ATOMIC_REFUSAL }));
}
