// SPDX-License-Identifier: AGPL-3.0-only
// What a sub-thread's row carries beside every thread's: its machine's state on
// the cloud, the branch it pushes, what the host read off the machine at its
// last done, and when the machine goes or went.
import { z } from "zod";

const count = z.number().int().nonnegative();

/** What the host read off a sub-thread's machine when it was last done, in its folder and off local refs alone.
 * `readFailed` says the read did not answer, which counts as work not pushed; `norepo` that the agent never fetched
 * the code, so nothing git holds is left. */
export const SubResult = z.object({
  branch: z.string(),
  pushedSha: z.string().nullable(),
  commits: count,
  unpushed: count,
  changed: count,
  onBranch: z.string().optional(),
  norepo: z.literal(true).optional(),
  readFailed: z.string().optional(),
  readAt: z.string(),
  /** Why the agent's last push of this turn was refused, read off that push's output in the transcript where it
   * failed: the remote's words for a token that cannot write or a protected branch. Absent where it went through,
   * none was tried, or the transcript no longer holds it. */
  pushRefused: z.string().optional(),
});
export type SubResult = z.infer<typeof SubResult>;

/** The steps a start of a sub-thread's machine took, as the host wrote them while it went: the lead's branch it read
 * (absent on a resume, which reads none), the machine the cloud made with the cloud's own states for it as they
 * changed, the picked files it landed, when the machine stood for the agent, and the step it failed on with the two
 * halves of why. Each step's `ms` is set once it is done; `since` is when it began. The lines a start passes with
 * ride beside them, and `log` keeps its stage lines as the command line prints them, the newest SUB_SPAWN_LOG_LINES. */
export const SubSpawn = z.object({
  startedAt: z.number(),
  branch: z.object({ branch: z.string(), sha: z.string(), ms: count }).optional(),
  machine: z.object({ since: z.number(), id: z.string().optional(), states: z.array(z.string()), ms: count.optional() }).optional(),
  files: z.object({ since: z.number(), paths: z.array(z.string()), bytes: count, ms: count.optional() }).optional(),
  readyAt: z.number().optional(),
  failed: z.object({ step: z.enum(["machine", "files"]), said: z.string(), fix: z.string().optional() }).optional(),
  /** Changes in the lead's folder nobody committed, which did not travel. */
  uncommitted: count.optional(),
  /** The message a send into the released sub-thread asked, kept until its machine stands: a start that fails writes
   * no turn, so this is what Try again sends again. */
  asked: z.string().optional(),
  /** Why no git-ignored file went. */
  notSent: z.string().optional(),
  /** A machine that failed before the agent started and was replaced, in its line. */
  replaced: z.array(z.string()).optional(),
  log: z.array(z.string()),
});
export type SubSpawn = z.infer<typeof SubSpawn>;

/** How many stage lines a start keeps for its log. */
export const SUB_SPAWN_LOG_LINES = 40;

/** Whether a start is still under way: begun, neither standing nor failed. */
export const subSpawning = (spawn: SubSpawn | undefined): boolean => spawn !== undefined && spawn.readyAt === undefined && spawn.failed === undefined;

/** A sub-thread's machine as its row reads it: starting before its first turn has run, running, napping (asleep, held
 * or waiting on its delete), and released once the box is deleted at the cloud with the thread kept. */
export const SubState = z.enum(["waiting", "starting", "running", "napping", "released"]);
export type SubState = z.infer<typeof SubState>;

/** The work a done read found not pushed, which holds the machine from its delete. */
export const SubHeld = z.object({ files: count, commits: count });
export type SubHeld = z.infer<typeof SubHeld>;

export const SubRow = z.object({
  state: SubState,
  on: z.string(),
  machine: z.string(),
  /** `name` is the class the cloud sold it as; absent on a record from before the name was kept. */
  size: z.object({ cpu: z.number().int(), memMb: z.number().int(), name: z.string().optional() }),
  /** Absent on a record from before the rate was kept. */
  rateUsdPerHour: z.number().optional(),
  branch: z.string(),
  result: SubResult.optional(),
  held: SubHeld.optional(),
  /** When the machine is deleted, ms epoch. */
  deleteAt: z.number().optional(),
  releasedAt: z.number().optional(),
  /** The cloud left this host with the machine: a send waits on the cloud being added again. */
  cloudGone: z.literal(true).optional(),
  /** Why the agent's session is not kept on the host, so a send after the delete goes on from the transcript. */
  sessionNotKept: z.string().optional(),
  /** The latest start of its machine. Absent on a record from before starts were written down. */
  spawn: SubSpawn.optional(),
});
export type SubRow = z.infer<typeof SubRow>;

/** Whether a read leaves nothing the remote lacks: nothing committed past it, nothing changed, and a read that answered. */
export const subAllPushed = (r: SubResult): boolean => r.readFailed === undefined && r.unpushed === 0 && r.changed === 0;

/** Whether a turn ran on a sub-thread's machine since deleted: one that started before its last release, or one whose
 * start is not known on a sub-thread released at all. Its checkpoints went with that machine. */
export const subTurnGone = (sub: { releasedAt?: number } | undefined, startedAt: number | undefined): boolean =>
  sub?.releasedAt !== undefined && (startedAt === undefined || startedAt < sub.releasedAt);
