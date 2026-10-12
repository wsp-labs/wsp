// SPDX-License-Identifier: AGPL-3.0-only
// What a sub-thread's spawn has said so far, per workspace, for the view that
// stands in the thread while its machine is made: each stage with the facts it
// read, the lines it passed with, and the raw log its command line prints.
import { create } from "zustand";
import type { StepLine } from "../../settings/add/setup.js";

export interface SpawnLog {
  /** The machine's size and rate, as the view's head says them. */
  readonly size: string;
  readonly steps: ReadonlyArray<StepLine>;
  /** The stage lines and the cloud's answers, oldest first, as `wsp run --on` prints them on stderr. */
  readonly log: ReadonlyArray<string>;
}

export const useSpawnLogs = create<{ readonly byWorkspace: Readonly<Record<string, SpawnLog>> }>(() => ({ byWorkspace: {} }));

export const useSpawnLog = (workspaceId: string): SpawnLog | undefined => useSpawnLogs(s => s.byWorkspace[workspaceId]);
