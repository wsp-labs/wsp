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
  /** Why the agent's last push of the branch was refused, as the done read found it in the branch's push log: a token
   * that cannot write, a protected branch. Absent where no push was refused or none was tried. */
  pushRefused: z.string().optional(),
});
export type SubResult = z.infer<typeof SubResult>;

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
  size: z.object({ cpu: z.number().int(), memMb: z.number().int() }),
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
});
export type SubRow = z.infer<typeof SubRow>;

/** Whether a read leaves nothing the remote lacks: nothing committed past it, nothing changed, and a read that answered. */
export const subAllPushed = (r: SubResult): boolean => r.readFailed === undefined && r.unpushed === 0 && r.changed === 0;

/** Whether a turn ran on a sub-thread's machine since deleted: one that started before its last release, or one whose
 * start is not known on a sub-thread released at all. Its checkpoints went with that machine. */
export const subTurnGone = (sub: { releasedAt?: number } | undefined, startedAt: number | undefined): boolean =>
  sub?.releasedAt !== undefined && (startedAt === undefined || startedAt < sub.releasedAt);
