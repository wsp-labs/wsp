// SPDX-License-Identifier: AGPL-3.0-only
// What a sub-thread's spawn said, per workspace, for the line under its brief:
// each stage with the facts it read, the lines it passed with, the raw log its
// command line prints, and once the agent answered, how long it all took.
import { create } from "zustand";
import type { StepLine } from "../../settings/add/setup.js";

export interface SpawnLog {
  /** The cloud by the name a person reads. */
  readonly on: string;
  /** When the spawn began, ms epoch, which the live line's time counts from. */
  readonly since: number;
  /** How long it took to the agent's first answer; absent while it runs. */
  readonly tookMs?: number;
  readonly steps: ReadonlyArray<StepLine>;
  /** The stage lines and the cloud's answers, oldest first, as `wsp run --on` prints them on stderr. */
  readonly log: ReadonlyArray<string>;
}

export const useSpawnLogs = create<{ readonly byWorkspace: Readonly<Record<string, SpawnLog>> }>(() => ({ byWorkspace: {} }));

export const useSpawnLog = (workspaceId: string): SpawnLog | undefined => useSpawnLogs(s => s.byWorkspace[workspaceId]);
