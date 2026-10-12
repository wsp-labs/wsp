// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread on a cloud: a thread a lead starts with `run --on`, on a machine
// of its own forked from the person's image. Nothing of the project is copied:
// the lead's branch is pushed, the sub-thread fetches it, works on a branch of
// its own and pushes that. What the start answers, the preamble above the
// lead's task, and every line the start refuses or passes with.
import { z } from "zod";
import { refusal, refusalParts } from "./exit.js";
import { seedNotHereRefusal } from "./project-seed.js";
import { ownerRepoOf } from "./start.js";
import { cloudNamed, PROVIDER_KEY_WORDS, providerKeyName } from "./place-word.js";
import { fmtMemGb, sizeFromWord, sizeWord } from "./words/units.js";

/** The most a sub-thread's picked files may come to, packed: the files worth picking are a few KB of `.env` style
 * configs, so anything past this is a data folder picked by mistake. */
export const SUB_SEED_MAX_BYTES = 20 * 1024 * 1024;

const count = z.number().int().nonnegative();

/** What a sub-thread's start answers beside the thread: the machine, the branches and the files sent. */
export const SubOut = z.object({
  on: z.string(),
  machine: z.string(),
  size: z.object({ cpu: z.number().int(), memMb: z.number().int() }),
  rateUsdPerHour: z.number(),
  repo: z.string(),
  base: z.object({ branch: z.string(), sha: z.string() }),
  branch: z.string(),
  sent: z.object({ paths: z.array(z.string()), bytes: count, missing: z.array(z.string()) }),
  notSent: z.object({ uncommitted: count, why: z.string().optional() }),
  deleteAfterMinutes: count,
});
export type SubOut = z.infer<typeof SubOut>;

/** What the host answers a sub-thread's machine with, for the start that opens the thread on it: the answer's `sub`,
 * the folder the thread opens in, the preamble above its task and the agent the checks read. */
export const SubStarted = z.object({ out: SubOut, folder: z.string(), preamble: z.string(), harness: z.string() });
export type SubStarted = z.infer<typeof SubStarted>;

/** Why a word beside --on is refused: the sub-thread runs in a folder of its own on the cloud, under the asking thread. */
export const subBesideRefusal = (word: string): Error =>
  refusal(`--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it takes no ${word}`, `Drop ${word}, or run the work without --on.`, "usage");

/** One size a cloud sells a sub-thread, by its class's own name. */
export interface SubSize {
  name: string;
  cpu: number;
  memMb: number;
  rateUsdPerHour: number;
}

/** A size as a sub-thread names it. */
export const subSizeWord = (size: { cpu: number; memMb: number }): string => sizeWord(size);

/** The size a word names, or nothing where it is not `<cpu>x<memGb>`. */
export const subSizeOf = (word: string): { cpu: number; memMb: number } | undefined => sizeFromWord(word);

/** A size in the words the preamble and the card say it in: `small, 2 vCPU, 4 GB`. */
export const subSizeWords = (size: Pick<SubSize, "name" | "cpu" | "memMb">): string => `${size.name}, ${size.cpu} vCPU, ${fmtMemGb(size.memMb)}`;

const rateWord = (usdPerHour: number): string => `$${usdPerHour.toFixed(3)}/h`;

/** The folder a sub-thread works in on its machine: the repository's own name off its remote, under root's home. */
export function subFolderOf(remote: string): string {
  const name = (ownerRepoOf(remote) ?? remote).split("/").at(-1)!.replace(/\.git$/, "");
  return `/root/${name}`;
}

/** The branch a sub-thread pushes when its start names none: `wsp/` and the machine's name as a slug. */
export function subBranchOf(machine: string): string {
  const slug = machine.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 48).replace(/-+$/, "");
  return `wsp/${slug === "" ? "sub-thread" : slug}`;
}

/** Whether git takes a name for a branch, by `git check-ref-format --branch`'s rules: never HEAD, no leading `-` or
 * `/`, no trailing `/` or `.`, no `//`, `..` or `@{`, no control character, space or any of `~^:?*[\`, and no part
 * that starts with `.` or ends with `.lock`. */
export function subBranchNameOk(name: string): boolean {
  if (name === "" || name === "HEAD" || name.startsWith("-") || name.startsWith("/") || name.endsWith("/") || name.endsWith(".")) return false;
  if (/[\x00-\x20\x7f~^:?*[\\]/.test(name) || name.includes("..") || name.includes("//") || name.includes("@{")) return false;
  return name.split("/").every(part => part !== "" && !part.startsWith(".") && !part.endsWith(".lock"));
}

/** The first message's head on a sub-thread: where it runs, the git lines that fetch the lead's pushed commit onto a
 * branch of its own, and the rule to push that branch before the turn ends. Names the files sent, never their
 * contents. The task follows it after a blank line. */
export function subPreamble(o: {
  on: string;
  size: SubSize;
  lead: string;
  folder: string;
  sent: readonly string[];
  setupGit: string;
  repo: string;
  base: { branch: string; sha: string };
  branch: string;
  deleteAfterMinutes: number;
}): string {
  const lead = o.lead.slice(0, 8);
  const folder = o.sent.length === 0 ? `The folder you are in, ${o.folder}, is empty.` : `The folder you are in, ${o.folder}, holds only the git-ignored files the person picked: ${o.sent.join(", ")}.`;
  const deleted = o.deleteAfterMinutes === 0 ? "as soon as your last turn ends" : `${o.deleteAfterMinutes} minutes after your last turn ends`;
  return [
    `This thread runs on a machine of its own on ${o.on} (${subSizeWords(o.size)}), started by thread ${lead}. The code is not here yet. ${folder} Get the code first, in this folder:`,
    "",
    o.setupGit,
    "git init -q",
    `git remote add origin ${o.repo}`,
    `git fetch -q --filter=blob:none origin ${o.base.branch}`,
    `git checkout -q -b ${o.branch} ${o.base.sha}`,
    "",
    `Work on ${o.branch}, then commit and run git push -u origin ${o.branch} before your turn ends. Thread ${lead} merges that branch. This machine is deleted ${deleted}, and anything not pushed is lost with it.`,
    "",
    "The task:",
  ].join("\n");
}

/** The first message whole: the preamble, a blank line, and the lead's task as given. */
export const subOpening = (preamble: string, task: string): string => `${preamble}\n\n${task}`;

const usage = (happened: string, fix: string): Error => refusal(happened, fix, "usage");
const provider = (happened: string, fix: string): Error => refusal(happened, fix);

/** S1: a sub-thread is a child, so a line out of no thread has no lead to notify or merge into. */
export const subNoThreadRefusal = (on: string): Error =>
  usage(`run --on ${on} starts a sub-thread of the thread that asks, and this line came from no thread`, "Start it from a thread, or run the work on this computer with wsp run <project>.");
/** `size` from a terminal with no cloud named. */
export const subSizeNoCloudRefusal = (): Error => usage("--size sizes a sub-thread's machine, and this line starts none", "Add --on boat.");
/** A lead runs on this computer or on a computer the person added, never on a machine wsp forked. */
export const subLeadOnMachineRefusal = (on: string, thread: string, machine: string): Error =>
  usage(`run --on ${on} starts a sub-thread of a thread on this computer or on a computer you added, and thread ${thread.slice(0, 8)} runs on ${machine}, a machine wsp forked`, "Start it from a thread on this computer.");
/** A sub-thread asking to start a thread: it starts none of its own, and its lead can. */
export const subStartsNothingRefusal = (thread: string, lead: string): Error =>
  usage(`sub-thread ${thread.slice(0, 8)} starts no threads of its own`, `Send the work to thread ${lead.slice(0, 8)}, which started it, with wsp send ${lead.slice(0, 8)} "<message>".`);
/** S3: the word names no cloud this host holds a key for. */
export const subNoCloudRefusal = (word: string, held: readonly string[]): Error =>
  usage(`${word} is not a cloud this host holds`, `wsp add boat adds it. It holds: ${held.length === 0 ? "none" : held.join(", ")}.`);
/** S4: no remote, so nothing for the machine to fetch. */
export const subNoRemoteRefusal = (folder: string, on: string): Error =>
  usage(`${folder} has no remote, so a machine on ${on} has nothing to clone`, "Push it to GitHub or GitLab, add the remote with git remote add origin <url>, then start the sub-thread again.");
/** S4: a remote on a host the image signs in to no command for. */
export const subOtherHostRefusal = (folder: string, host: string, on: string): Error =>
  usage(`${folder}'s remote is on ${host}, and a machine on ${on} signs in to GitHub and GitLab only`, "Run this work on this computer.");
/** S5: the lead is on no branch. */
export const subDetachedRefusal = (folder: string): Error => usage(`${folder} is on no branch`, "Check one out and push it.");
/** S5: the lead's branch is not on the remote. */
export const subNoUpstreamRefusal = (branch: string): Error => usage(`${branch} is not on the remote yet`, `Run git push -u origin ${branch}, then start the sub-thread again.`);
/** S5: the lead holds commits the remote lacks, which the sub-thread could not fetch. */
export const subAheadRefusal = (branch: string, ahead: number): Error =>
  usage(`${branch} has ${ahead} ${ahead === 1 ? "commit" : "commits"} the remote does not`, "Run git push, then start the sub-thread again.");
/** S5: git could not count the lead's commits against the remote, so some may not be on it. */
export const subAheadUnknownRefusal = (branch: string): Error =>
  usage(`git could not count ${branch}'s commits against the remote, so some of them may not be on it`, "Run git fetch, then git push, then start the sub-thread again.");
/** S6: no image on the cloud. */
export const subNoImageRefusal = (on: string): Error => usage(`${on} holds no image of yours yet`, `Build one in Settings > Computers > ${on}, or run wsp init --on ${on.toLowerCase()}.`);
/** S6: the image holds no sign-in for the remote's git host, so the sub-thread could fetch nothing private and push nothing. */
export const subNoSignInRefusal = (on: string, host: { label: string; cli: string; login: string }, repo: string): Error =>
  usage(
    `the image on ${on} holds no ${host.label} sign-in (${host.cli}), so a sub-thread could neither clone ${repo} nor push its branch`,
    `Run wsp init --on ${on.toLowerCase()} again with the ${host.login} set to copy.`,
  );
/** S7: a size the plan does not sell, or no class's figures at all. */
export const subSizeRefusal = (on: string, word: string, sold: readonly SubSize[]): Error => {
  const named = sold.map(s => `${s.name} ${subSizeWord(s)} (${rateWord(s.rateUsdPerHour)})`);
  const list = named.length <= 1 ? (named[0] ?? "nothing") : `${named.slice(0, -1).join(", ")} and ${named.at(-1)!}`;
  return usage(`${on} sells ${list} on this plan; ${word} is not one of them`, `Name one of them, or leave the size out for ${on}'s own.`);
};
/** S8: a name git refuses for a branch. */
export const subBranchNameRefusal = (name: string): Error => usage(`${name} is not a name git takes for a branch`, "Name another, or leave --branch out for wsp/<slug>.");
/** S8: the result branch is the lead's own. */
export const subLeadBranchRefusal = (name: string): Error => usage(`the sub-thread pushes ${name}, and that is the lead's own branch`, "Name another, or leave --branch out for wsp/<slug>.");
/** S9: a sub-machine runs its sub-thread alone. */
export const subMachineTakenRefusal = (machine: string, thread: string): Error =>
  usage(`${machine} is the machine of sub-thread ${thread.slice(0, 8)} and runs that thread alone`, `Send to ${thread.slice(0, 8)} to continue it.`);
/** S11: the agent named is not on the image. */
export const subNoAgentRefusal = (on: string, agent: string, carried: readonly string[]): Error =>
  usage(`the image on ${on} carries no ${agent}`, `Add it to the recipe and build the image again, or name one it carries: ${carried.join(", ")}.`);
/** S12: the picked files come to more than a sub-thread carries. */
export const subSeedTooBigRefusal = (project: string, bytes: string, max: string, largest: readonly string[]): Error =>
  usage(`the files picked for ${project} come to ${bytes}, over the ${max} a sub-thread carries; the largest are ${largest.join(", ")}`, `Pick again with wsp projects seed ${project}.`);
/** Why a machine failed before its sub-thread's first turn, the three a fresh machine cures. */
export type SubFault = { kind: "unusable"; box: string; why: string; requestId?: string } | { kind: "env"; box: string } | { kind: "cancelled"; box: string; why: string };

/** The line a first failure says once its machine is deleted and another started. */
export function subReplacedLine(f: SubFault, machine: string, on: string): string {
  if (f.kind === "unusable") return `${on} gave ${f.box} with nothing able to run on it (${f.why}); wsp deleted it and started another.`;
  if (f.kind === "env") return `${on}'s machine ${f.box} did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another.`;
  return `${on} could not get a machine for ${machine} and cancelled the create (${f.why}); wsp started another.`;
}

/** A sub-thread's turn the kernel ended for memory, as its finished line says it: the machine's size, that the
 * cloud's own processes hold part of it, and the next size up to start the work again at, where the cloud sells one. */
export function subOomLine(machine: string, on: string, size: Pick<SubSize, "name" | "cpu" | "memMb">, bigger: string | undefined): string {
  const again = bigger !== undefined ? `Start the work again with size ${bigger}.` : `${on} sells no bigger size, so the work has to use less memory.`;
  return `the agent on ${machine} was killed for memory: its machine is ${subSizeWords(size)}, and ${on}'s own processes hold part of that. ${again}`;
}

/** The refusal a second failure in a row fails the spawn with, naming both machines. */
export function subTwiceRefusal(first: SubFault, second: SubFault, machine: string, on: string): Error {
  const fix = "Start it again in a few minutes.";
  if (first.kind === "unusable" && second.kind === "unusable") {
    const request = second.requestId ?? first.requestId;
    return provider(`${on} gave two machines in a row that could not run commands (${first.box}, ${second.box}${request === undefined ? "" : `; request ${request}`}); ${on} is restoring badly right now`, fix);
  }
  if (first.kind === "env" && second.kind === "env") return provider(`${on}'s machines are slow to take a setup right now (${first.box}, ${second.box}, exit 124 on both)`, fix);
  if (first.kind === "cancelled" && second.kind === "cancelled") return provider(`${on} cancelled two creates in a row for ${machine} (${first.why}; ${second.why})`, fix);
  const said = (f: SubFault): string => (f.kind === "env" ? `${f.box}: setup not taken, exit 124` : `${f.box}: ${f.why}`);
  return provider(`${on} gave two machines in a row that failed before the sub-thread started (${said(first)}; ${said(second)})`, fix);
}

/** The line a start passes with when the lead's folder holds changes nobody committed. */
export const subUncommittedLine = (n: number, folder: string, branch: string, sha: string): string =>
  `${n} uncommitted ${n === 1 ? "change" : "changes"} in ${folder} did not travel; the sub-thread starts at ${branch} as pushed (${sha.slice(0, 7)}).`;
/** Why nothing ignored went: nothing is picked for the project. */
export const subNothingPickedWhy = (project: string): string => `nothing is picked for ${project}`;
/** Why nothing ignored went: the project's folder is on another computer. */
export const subPickedElsewhereWhy = (project: string, computer: string): string => refusalParts(seedNotHereRefusal(project, computer)).said.replace(/\.$/, "");
/** The line a start passes with when no git-ignored file was sent. */
export const subNotSentLine = (why: string, project: string): string =>
  `no git-ignored files were sent: ${why}.${why.startsWith("nothing is picked") ? ` wsp projects seed ${project} shows what could go.` : ""}`;
/** The stage line for the picked files landing. */
export const subCopyingLine = (files: number): string => `copying ${files} ${files === 1 ? "file" : "files"}`;
/** The first line a terminal prints: where the sub-thread runs. */
export const subOpenedLine = (threadId: string, on: string, folder: string): string => `thread ${threadId.slice(0, 8)} on ${on.toLowerCase()} in ${folder}`;

/** The work a done read found not pushed, in the words the delete's question and the lead's refusal give it. */
function heldWords(held: { files: number; commits: number }): string {
  const parts = [held.commits > 0 ? `${held.commits} ${held.commits === 1 ? "commit" : "commits"}` : "", held.files > 0 ? `${held.files} changed ${held.files === 1 ? "file" : "files"}` : ""].filter(Boolean);
  return parts.length === 0 ? "work" : parts.join(" and ");
}

/** What a sub-thread holds that no remote has: the counts its last read found, or that the read did not answer. */
export interface SubUnpushed {
  files: number;
  commits: number;
  readFailed?: string;
}

/** The second line of the delete's question, and the half of the lead's refusal that says what is not pushed. */
export function subUnpushedLine(held: SubUnpushed, branch: string): string {
  if (held.readFailed !== undefined) return `its machine could not be read (${held.readFailed}), so work on ${branch} may not be pushed and would go with it.`;
  return `${heldWords(held)} on ${branch} ${held.files + held.commits === 1 ? "is" : "are"} not pushed and go with it.`;
}

/** The question a person answers before a sub-thread and its machine go. */
export function subDeleteQuestion(o: { threadId: string; title: string; on: string; branch: string; held?: SubUnpushed }): string {
  const first = `Delete sub-thread ${o.threadId.slice(0, 8)} (${o.title}) and its machine on ${o.on}?`;
  const second = o.held === undefined ? `Its turns leave this computer and its machine is deleted; what it pushed to ${o.branch} stays on the remote.` : subUnpushedLine(o.held, o.branch);
  return `${first}\n${second.charAt(0).toUpperCase()}${second.slice(1)}`;
}

/** A lead's own token asking the delete of a sub-thread whose work is not pushed: losing it is the person's call. */
export const subHeldDeleteRefusal = (threadId: string, held: SubUnpushed, branch: string): Error =>
  usage(
    held.readFailed !== undefined
      ? `sub-thread ${threadId.slice(0, 8)}'s machine could not be read (${held.readFailed}), so work on ${branch} may not be pushed`
      : `sub-thread ${threadId.slice(0, 8)} holds ${heldWords(held)} not pushed on ${branch}`,
    "Send it a push, or ask the person to delete it.",
  );

/** A send into a sub-thread whose cloud this host no longer holds. */
export const subCloudGoneRefusal = (on: string): Error => usage(`${on} is no longer on this host`, `Add it with wsp add ${on.toLowerCase()} and send again.`);

/** A send into a sub-thread whose machine was deleted after it was done, before a new machine can be made for it. */
export const subReleasedRefusal = (threadId: string, on: string, branch: string): Error =>
  usage(`sub-thread ${threadId.slice(0, 8)}'s machine on ${on} was deleted after it was done; its work is on ${branch}`, `Merge ${branch} into its lead, or start a new sub-thread with wsp run --on ${on.toLowerCase()}.`);

/** What a delete Boat refused says, and when it is asked again. */
export const subDeleteRefusedLine = (on: string, machine: string, said: string, retryMinutes: number): string =>
  `${on} would not delete ${machine} (${said}); it is tried again in ${retryMinutes} minutes`;

/** The line a cloud's remove adds to its question for sub-threads whose machines hold work no remote has: it goes
 * with them. Names five and counts the rest. */
export function subHeldOnRemoveLine(held: readonly { threadId: string; branch: string; files: number; commits: number }[]): string | undefined {
  if (held.length === 0) return undefined;
  const named = held.slice(0, 5).map(h => `${h.threadId.slice(0, 8)} (${heldWords(h)} on ${h.branch})`);
  const rest = held.length > 5 ? ` and ${held.length - 5} more` : "";
  return `${held.length === 1 ? "A sub-thread holds" : `${held.length} sub-threads hold`} work not pushed, which goes with ${held.length === 1 ? "its machine" : "their machines"}: ${named.join(", ")}${rest}.`;
}

/** A sub-thread's cloud as a line names it, off the word its record keeps: `boat` is Boat. */
export function cloudSaid(on: string): string {
  const id = cloudNamed(Object.keys(PROVIDER_KEY_WORDS), on);
  return id === undefined ? on : providerKeyName(id);
}

/** What a sub-thread's delete did. */
export const subDeletedLine = (threadId: string, on: string): string => `deleted sub-thread ${threadId.slice(0, 8)} and its machine on ${on}`;
