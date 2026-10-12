// SPDX-License-Identifier: AGPL-3.0-only
// The ops that read or end what a thread runs, answered off the runtime: Activity's own, and settle and delete,
// which meet the processes a thread still runs before they take it. What the reply carries beside id and ok.
import type { Caller, RuntimeRequest } from "@wsp/protocol";
import type { Runtime } from "./types/api.js";

export type ProcessesRequest = Extract<RuntimeRequest, { op: "activity.read" | "activity.end" | "sessions.settle" | "sessions.delete" }>;

export const isProcessesRequest = (msg: RuntimeRequest): msg is ProcessesRequest => msg.op === "activity.read" || msg.op === "activity.end" || msg.op === "sessions.settle" || msg.op === "sessions.delete";

export async function answerProcesses(rt: Pick<Runtime, "activity" | "sessions">, msg: ProcessesRequest, origin: Caller | undefined): Promise<Record<string, unknown>> {
  const computer = "computer" in msg && msg.computer !== undefined ? { computer: msg.computer } : {};
  const processes = "processes" in msg && msg.processes !== undefined ? { processes: msg.processes } : {};
  switch (msg.op) {
    case "activity.read":
      return { activity: await rt.activity.read({ ...computer, ...(msg.thread !== undefined ? { thread: msg.thread } : {}) }, origin) };
    case "activity.end":
      return { ...(await rt.activity.end({ ...computer, pid: msg.pid, startedAt: msg.startedAt }, origin)) };
    case "sessions.settle":
      return { ...(await rt.sessions.settle(msg.threadIds, origin, { ...(msg.finished === true ? { finished: true } : {}), ...processes })) };
    case "sessions.delete":
      return { ...(await rt.sessions.delete(msg.threadId, origin, { ...processes, ...(msg.check === true ? { check: true } : {}) })) };
  }
}
