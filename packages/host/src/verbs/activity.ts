// SPDX-License-Identifier: AGPL-3.0-only
// Activity on the command line and the tools: this computer's processes and a
// box's by the thread that started them, the End of one by the pid and start a
// read listed, and the --processes choice settle and delete hand the host.
import { z } from "zod";
import {
  ActivityEnded,
  ActivityView,
  PROCESSES_CHOICES,
  endGoneSaid,
  endGraceWord,
  fmtBytes,
  threadWord,
  usageRefusal,
  type ActivityProc,
  type ProcessesChoice,
} from "@wsp/protocol";
import { usageIs, tool, type CliVerb, type HostClient, flag, oneOf } from "./client.js";
import { CLOUD_ON } from "../cloud.js";
import { cloudText } from "../cloud-text.js";
import { threadOf } from "./workspaces-help.js";
import { asText } from "./io.js";
import { activityTable } from "./activity-table.js";

/** What each verb says the start is, which `wsp activity --json` lists as startedAt. */
const STARTED_WORDS = "the start activity listed it with, its startedAt in epoch milliseconds";

export async function readActivity(client: HostClient, o: { computer?: string; thread?: string }): Promise<ActivityView> {
  return ActivityView.parse((await client.request("activity.read", { ...o })).activity);
}

export async function endProcess(client: HostClient, o: { computer?: string; pid: number; startedAt: number }): Promise<ActivityEnded> {
  return ActivityEnded.parse(await client.request("activity.end", { ...o }));
}

const cpuWord = (cpu: number): string => `${cpu.toFixed(1)}%`;

const procLine = (p: ActivityProc): string =>
  `${"  ".repeat(p.depth + 1)}${p.pid}  ${p.kind}  ${p.command}${p.ports.length > 0 ? `  :${p.ports.join(" :")}` : ""}  ${fmtBytes(p.memory)}  ${cpuWord(p.cpu)}`;

/** A read as the tool answers it: each thread with its totals and its tree, then wsp's own and what has no thread; the
 * Rust tool server lays its rows out the same way. */
export function activityLines(view: ActivityView): string {
  const lines: string[] = [];
  for (const t of view.threads) lines.push(`thread ${threadWord(t.threadId)} ${t.title}  ${fmtBytes(t.memory)}  ${cpuWord(t.cpu)}`, ...t.processes.map(procLine));
  if (view.wsp.length > 0) lines.push("wsp", ...view.wsp.map(procLine));
  if (view.unowned.length > 0) lines.push("no thread", ...view.unowned.map(procLine));
  return lines.length === 0 ? `no thread runs a process on ${view.computer}` : lines.join("\n");
}

export function endedLines(e: ActivityEnded): string {
  if (e.freed !== undefined) return `freed the agent thread ${threadWord(e.freed)} kept`;
  const [first, ...under] = e.ended;
  if (first === undefined) return endGoneSaid(e.pid);
  const lines = [`ended ${first.pid} ${first.name}${under.length > 0 ? ` and ${under.length} under it` : ""}`];
  if (e.forced.length > 0) lines.push(`KILL after TERM's ${endGraceWord}: ${e.forced.join(" ")}`);
  if (e.left.length > 0) lines.push(`still running after KILL: ${e.left.join(" ")}`);
  return lines.join("\n");
}

async function readFor(client: HostClient, computer: string | undefined, ref: string | undefined): Promise<ActivityView> {
  const thread = ref === undefined ? undefined : (await threadOf(client, ref)).id;
  return readActivity(client, { ...(computer !== undefined ? { computer } : {}), ...(thread !== undefined ? { thread } : {}) });
}

export const processesAsked = (value: string | undefined, on?: string): ProcessesChoice | undefined => oneOf("processes", PROCESSES_CHOICES, value, on);

export const ProcessesIn = z.enum(PROCESSES_CHOICES).optional().describe(cloudText("what to do with the processes a thread still runs beside its agent: stop ends each by identity, and on a box ends everything in the thread's cgroup there; keep leaves them running; refused without one while any run<!-- cloud -->. A thread on a cloud machine is not read and goes as before<!-- /cloud -->", CLOUD_ON));

export const ACTIVITY_VERBS: readonly CliVerb[] = [
  {
    name: "activity",
    usage: "wsp activity [<thread>] [--computer <computer>] [--wide]",
    about: "this computer's processes, or a box's, as Settings > Activity lists them: wsp's own, then each thread heaviest first with what it started, then what has no thread, each with its kind, ports, memory and CPU; a thread named shows its own alone, on this computer or the box it runs on",
    page: "agent",
    options: { computer: { type: "string" }, wide: { type: "boolean" } },
    run: async ctx => {
      if (ctx.args.length > 1) throw usageRefusal("wsp activity takes one thread at most.", usageIs(ctx));
      const view = await readFor(await ctx.client(), flag(ctx.flags, "computer"), ctx.args[0]);
      ctx.out.emit(view, activityTable(view, { width: ctx.io.redraw?.columns() ?? Infinity, wide: ctx.flags["wide"] === true }));
      return 0;
    },
    tool: tool({
      description:
        "This computer's processes, or a box's with computer, grouped under the thread that started them: each thread's tree from its agent down to what the agent ran (a dev server, a test run, a tool server), with each process's pid, ppid, depth in the tree, kind (agent, tool, dev, test, build, slate, terminal, tab, app, wsp, other), command, memory in bytes, CPU as a share of one core, the TCP ports it listens on, and startedAt, which activity_end takes back with the pid. A thread's memory and cpu are its processes' together. wsp holds wsp's own processes, and unowned what the daemon names as wsp's with no thread; what the person started outside wsp is not listed. With thread, that thread's alone, on the computer it runs on. On a box a thread's processes are what stands in its cgroup there. A thread reads itself and the threads under it alone.",
      input: {
        thread: z.string().optional().describe("the thread, by its id or a prefix of it that names one, whose processes alone to list"),
        computer: z.string().optional().describe("the computer: this one, which is the default, or a box wsp computers lists"),
      },
      output: ActivityView.shape,
      call: async ({ thread, computer }, deps) => {
        const view = await readFor(await deps.client(), computer, thread);
        return asText(activityLines(view), view);
      },
    }),
  },
  {
    name: "activity end",
    usage: "wsp activity end <pid> --started <ms> [--computer <computer>]",
    about: `ends one process on this computer and everything under it, by the pid and the start wsp activity --json listed: TERM, then KILL after ${endGraceWord}; refuses a process wsp did not start, an agent, which wsp stop ends, and a pid that names another process now`,
    page: "agent",
    options: { started: { type: "string" }, computer: { type: "string" } },
    run: async ctx => {
      const [word] = ctx.args;
      const pid = Number(word);
      if (word === undefined || ctx.args.length !== 1 || !Number.isInteger(pid) || pid <= 0) throw usageRefusal("wsp activity end takes one pid.", usageIs(ctx));
      const started = flag(ctx.flags, "started");
      const startedAt = Number(started);
      if (started === undefined || started === "" || !Number.isFinite(startedAt)) throw usageRefusal(`wsp activity end takes --started, ${STARTED_WORDS}.`, usageIs(ctx));
      const computer = flag(ctx.flags, "computer");
      const ended = await endProcess(await ctx.client(), { pid, startedAt, ...(computer !== undefined ? { computer } : {}) });
      ctx.out.emit(ended, endedLines(ended));
      return 0;
    },
    tool: tool({
      description: `Ends one process on this computer and every process under it, by its pid and the startedAt activity listed it with: TERM to each, then KILL to whatever of them is still there after ${endGraceWord}, each only while it is still the process listed. Never a process group. Refused, with nothing signalled: a pid no longer running, a process wsp did not start (one activity does not list), a pid that names another process now (read activity again), an agent from its turn's process down to the agent's binary, or a process with one under it, which only the thread's stop ends, and wsp's own processes. A thread's kept agent, the one it keeps warm for its next turn, is freed through its thread instead. A thread ends only what it and the threads under it run. The answer lists each process ended, those KILL went to and any still running after it.`,
      input: {
        pid: z.number().int().positive().describe("the process's pid, as activity lists it"),
        started: z.number().describe(STARTED_WORDS),
        computer: z.string().optional().describe("the computer, which must be this one; a box ends a thread's processes when its thread settles with processes stop"),
      },
      output: ActivityEnded.shape,
      call: async ({ pid, started, computer }, deps) => {
        const ended = await endProcess(await deps.client(), { pid, startedAt: started, ...(computer !== undefined ? { computer } : {}) });
        return asText(endedLines(ended), ended);
      },
    }),
  },
];
