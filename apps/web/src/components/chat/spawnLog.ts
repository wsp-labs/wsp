// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's latest machine start as the line under its message reads it:
// the steps off the row's spawn, each with its mark, words, facts and time, in
// the order the host takes them, the lines it passed with among them, and the
// agent's own start last, done once the agent answered.
import { SUB_SPAWN_WORDS as W, cloudSaid, fmtBytes, subUncommittedLine, type SessionView, type SubRow, type SubSpawn } from "@wsp/protocol";
import type { StepLine } from "../../settings/add/setup.js";

/** What the line needs besides the row: the agent by its name and picks, the project, and when the agent first
 * answered after the start, ms epoch, null while it has not. */
export interface SpawnFacts {
  readonly agent: string;
  readonly model?: string | undefined;
  readonly effort?: string | undefined;
  readonly project: string;
  readonly answeredAt: number | null;
}

export interface SpawnLog {
  /** The cloud by the name a person reads. */
  readonly on: string;
  /** When the start began, ms epoch, which the live line's time counts from. */
  readonly since: number;
  /** How long it took to the agent's first answer; absent while it runs or where it failed. */
  readonly tookMs?: number;
  readonly failed: boolean;
  readonly steps: ReadonlyArray<StepLine>;
  /** The stage lines as the command line prints them, oldest first. */
  readonly log: ReadonlyArray<string>;
}

type Step = "working" | "done" | "failed" | "waiting";

/** Whether a refused send's words are the failed start the thread's row carries, which its start line says. The host
 * puts that row on the bus before it answers the refusal, so the row is read once, as it stands. */
export function startFailureSays(rows: ReadonlyArray<SessionView> | undefined, threadId: string | undefined, message: string): boolean {
  return (rows ?? []).some(row => row.threadId === threadId && row.sub?.spawn?.failed !== undefined && message.startsWith(row.sub.spawn.failed.said));
}

export function spawnLogOf(sub: Pick<SubRow, "on" | "spawn">, facts: SpawnFacts): SpawnLog | undefined {
  const spawn: SubSpawn | undefined = sub.spawn;
  if (spawn === undefined) return undefined;
  const on = cloudSaid(sub.on);
  const failed = spawn.failed;
  const steps: StepLine[] = [];
  // Every step after the one that failed reads not started.
  let stopped = false;
  const add = (line: StepLine): void => {
    if (!stopped || line.state === "warned") return void steps.push(line);
    const { ms: _ms, since: _since, said: _said, fix: _fix, ...rest } = line;
    steps.push({ ...rest, state: "waiting" });
  };
  const stateOf = (step: { since: number; ms?: number | undefined }, id: "machine" | "files"): Step => (failed?.step === id ? "failed" : step.ms !== undefined ? "done" : "working");
  const timed = (state: Step, step: { since: number; ms?: number | undefined }): Pick<StepLine, "ms" | "since"> => (state === "done" ? { ms: step.ms! } : state === "working" ? { since: step.since } : {});
  const failure = (): Pick<StepLine, "said" | "fix"> => ({ said: failed!.said, ...(failed!.fix !== undefined ? { fix: failed!.fix } : {}) });

  if (spawn.branch !== undefined) add({ id: "branch", name: W.branch(true), note: W.branchFact(spawn.branch.branch, spawn.branch.sha), state: "done", ms: spawn.branch.ms });
  if (spawn.machine !== undefined || failed?.step === "machine") {
    const machine = spawn.machine ?? { since: spawn.startedAt, states: [] };
    const state = stateOf(machine, "machine");
    const note = W.machineFact(machine.id, machine.states);
    add({ id: "machine", name: W.machine(on, state === "done"), state, ...(note !== undefined ? { note } : {}), ...timed(state, machine), ...(state === "failed" ? failure() : {}) });
    if (state === "failed") stopped = true;
  }
  for (const line of spawn.replaced ?? []) add({ id: `replaced:${line}`, name: line, state: "warned" });
  if (spawn.uncommitted !== undefined && spawn.branch !== undefined) add({ id: "uncommitted", name: subUncommittedLine(spawn.uncommitted, facts.project, spawn.branch.branch, spawn.branch.sha), state: "warned" });
  if (spawn.notSent !== undefined) add({ id: "not-sent", name: W.notSent(spawn.notSent), state: "warned" });
  if (spawn.files !== undefined) {
    const state = stateOf(spawn.files, "files");
    add({ id: "files", name: W.files(spawn.files.paths.length, state === "done"), note: W.filesFact(spawn.files.paths, fmtBytes(spawn.files.bytes)), state, ...timed(state, spawn.files), ...(state === "failed" ? failure() : {}) });
    if (state === "failed") stopped = true;
  }
  const ready = spawn.readyAt;
  const answered = ready !== undefined && facts.answeredAt !== null;
  const agentNote = ready === undefined ? undefined : W.agentFact(facts.model, facts.effort);
  add({
    id: "agent",
    name: W.agent(facts.agent),
    state: ready === undefined ? "waiting" : answered ? "done" : "working",
    ...(agentNote !== undefined ? { note: agentNote } : {}),
    ...(ready === undefined ? {} : answered ? { ms: Math.max(0, facts.answeredAt! - ready) } : { since: ready }),
  });
  return {
    on,
    since: spawn.startedAt,
    ...(answered ? { tookMs: Math.max(0, facts.answeredAt! - spawn.startedAt) } : {}),
    failed: failed !== undefined,
    steps,
    log: spawn.log,
  };
}
