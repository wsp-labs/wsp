// SPDX-License-Identifier: AGPL-3.0-only
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { FsHashReply, FsImageReply, threadWord, isLocalWorkspace, folderOnJoined, HERE_PLACE_ID, HTTP_POST_DAEMON_VERSION, SETTLE_MS, slateMachineOnBox, type AgentsTarget } from "@wsp/protocol";
import { isServerNotSetUp } from "../agents-read.js";
import { pipeOn, postOn } from "../slate-mcp-on.js";
import { lazySlates } from "../lazy-slates.js";
import type { Slates } from "../slates.js";
import type { RuntimeContext, SlatesArea } from "../context.js";
import type { LiveWorkspace } from "../types/wiring.js";

export function slatesArea(ctx: RuntimeContext): SlatesArea {
  const { opts, store, local, bus, clock, live, threadRecords, sessions, transcripts } = ctx;
  const harnessOf = (threadId: string): string | undefined => ctx.latestOn(threadId)?.harness ?? threadRecords.get(threadId)?.harness;
  /** Where a box thread's agent reads its servers: that computer's login for a folder there, else the machine itself. */
  const serversTarget = (box: LiveWorkspace): AgentsTarget =>
    box.record.place !== undefined && folderOnJoined(box.record.kind) ? { placeId: box.record.place, project: box.record.project } : { workspaceId: box.record.id };
  /** A machine wsp made on a joined computer, from before a thread there ran in a folder: the daemon that answers it is
   * the computer's own, so a server it holds open would run on the computer, outside the machine. */
  const machineOnBox = (box: LiveWorkspace): boolean => box.record.place !== undefined && !folderOnJoined(box.record.kind);
  const slates: Slates = lazySlates({
    store,
    now: () => clock.now(),
    record: e => ctx.record(e),
    emit: e => bus.emit(e),
    thread: threadId => {
      const latest = ctx.latestOn(threadId);
      const workspaceId = latest?.workspaceId ?? threadRecords.get(threadId)?.workspaceId;
      if (workspaceId === undefined) return undefined;
      const running = [...sessions.values()].find(s => s.view.threadId === threadId && s.view.status === "running");
      const entry = live.get(workspaceId);
      // A run starts where the thread's next turn would: the folder its session ran in, a --cwd or a worktree, else
      // the folder the workspace's kind holds its project in, on that kind's computer.
      const ranIn = latest?.cwd ?? (latest?.claudeSessionId === undefined ? undefined : ctx.folderOf(workspaceId, latest.claudeSessionId));
      if (entry === undefined) return { workspaceId, rootThreadId: ctx.rootOf(threadId), sessionId: latest?.claudeSessionId ?? latest?.id ?? threadId, ...(running !== undefined ? { turnId: running.turnId } : {}) };
      const folder = ctx.runsIn(entry, ranIn, ctx.moduleOf(entry.record.kind).folder(entry.record) ?? ctx.checkoutOf(entry.record));
      // A run on the host from a thread on a box starts in the project's folder here, where the project is also here.
      const here = isLocalWorkspace(entry.record) ? folder : existsSync(ctx.checkoutOf(entry.record)) ? ctx.checkoutOf(entry.record) : homedir();
      return {
        workspaceId,
        rootThreadId: ctx.rootOf(threadId),
        sessionId: latest?.claudeSessionId ?? latest?.id ?? threadId,
        ...(running !== undefined ? { turnId: running.turnId } : {}),
        folder,
        hostFolder: here,
        computer: ctx.computerOf(entry),
      };
    },
    machineOf: threadId => {
      const entry = ctx.boxOf(threadId);
      return entry?.machine;
    },
    imageOn: threadId => {
      const entry = ctx.boxOf(threadId);
      if (entry === undefined) return undefined;
      return { name: ctx.computerOf(entry), read: path => ctx.withDaemon(entry, async ask => FsImageReply.parse(await ask({ op: "fs.image", path }))) };
    },
    hashOn: threadId => {
      const entry = ctx.boxOf(threadId);
      if (entry === undefined) return undefined;
      return (root, paths) => ctx.withDaemon(entry, async ask => FsHashReply.parse(await ask({ op: "fs.hash", root, paths })));
    },
    asleep: threadId => {
      const entry = ctx.boxOf(threadId);
      return entry !== undefined && entry.record.phase !== "running";
    },
    wake: async threadId => {
      const entry = ctx.boxOf(threadId);
      if (entry === undefined) return;
      await ctx.bootWork.get(entry.record.id);
      if (entry.record.phase !== "running") await ctx.workspaces.wake(entry.record.id);
    },
    loaded: () => ctx.ready(),
    settled: async threadId => ctx.settledNow(threadId, SETTLE_MS[(await ctx.preferences.get()).settleAfter]),
    under: lead => ctx.treeUnder(lead),
    threadOfToken: token => ctx.threadOfToken(token),
    ownProcess: (threadId, pid, kind) => ctx.ownProcess(threadId, pid, kind),
    mcpServer: async (threadId, name) => {
      const workspaceId = ctx.latestOn(threadId)?.workspaceId ?? threadRecords.get(threadId)?.workspaceId;
      const harness = harnessOf(threadId);
      const entry = workspaceId === undefined ? undefined : live.get(workspaceId);
      if (entry === undefined || harness === undefined) throw new Error(`this host holds no folder for thread ${threadId}`);
      // A thread on another computer starts its server there, off that computer's config, with its sign-ins.
      const box = ctx.boxOf(threadId);
      if (box !== undefined) {
        if (machineOnBox(box)) throw new Error(slateMachineOnBox([name], box.record.name, ctx.computerOf(box)));
        const { login, ...spec } = await ctx.agentsRead.server(serversTarget(box), { agent: harness, name });
        if (login === undefined) throw new Error(`${ctx.computerOf(box)} did not say who its lines run as`);
        return { ...spec, open: pipeOn(ctx.computerOf(box), login, () => ctx.stdioRoad(box)), post: postOn(ctx.computerOf(box), () => ctx.stdioRoad(box, HTTP_POST_DAEMON_VERSION), spec.signIn) };
      }
      const reader = opts.agentsReader;
      if (reader?.server === undefined) throw new Error("this host reads no agent's MCP config");
      return reader.server({ kind: "here", projects: [{ id: workspaceId!, name: "thread", path: ctx.checkoutOf(entry.record) }] }, { agent: harness, name });
    },
    away: threadId => {
      const box = ctx.boxOf(threadId);
      return box === undefined ? undefined : { computer: ctx.computerOf(box), host: ctx.placeName(HERE_PLACE_ID), ...(machineOnBox(box) ? { machine: box.record.name } : {}) };
    },
    missingOn: async (threadId, servers) => {
      const box = ctx.boxOf(threadId);
      const agent = harnessOf(threadId);
      if (box === undefined || agent === undefined) return { agent: { id: agent ?? "", name: agent ?? "" }, thread: threadId, missing: [] };
      // Only a config that names no such server refuses the write; a computer that could not be read now leaves the
      // run to say why when it starts.
      const named = await Promise.all(servers.map(name => ctx.agentsRead.server(serversTarget(box), { agent, name }).then(() => true, (e: unknown) => !isServerNotSetUp(e))));
      return { agent: { id: agent, name: ctx.agentLabel(agent) }, thread: threadId, missing: servers.filter((_, i) => !named[i]) };
    },
    sources: (threadId, workspaceId) => ({
      threadId,
      workspaceId,
      now: clock.now(),
      rows: () => ctx.rowsOn(threadId).sort((a, b) => (a.startedAt ?? 0) - (b.startedAt ?? 0)),
      results: () => (transcripts.get(workspaceId) ?? []).flatMap(e => (e.type === "session.done" && e.threadId === threadId ? [e.result] : [])),
      account: async () => {
        const entry = live.get(workspaceId);
        const harness = ctx.latestOn(threadId)?.harness ?? threadRecords.get(threadId)?.harness;
        if (entry === undefined || harness === undefined) return undefined;
        const key = ctx.usageAccountOf(entry, harness).key;
        return (await ctx.usageAccounts()).accounts.find(a => a.key === key);
      },
      workspace: () => {
        const entry = live.get(workspaceId);
        if (entry === undefined) return undefined;
        const project = entry.record.project;
        return {
          computer: ctx.computerOf(entry),
          ...(project !== undefined ? { project: ctx.projectHeld(project).name } : {}),
          folder: ctx.checkoutOf(entry.record),
          ...(entry.checkout !== undefined ? { checkout: entry.checkout } : {}),
          ...(entry.pr !== undefined ? { pr: entry.pr } : {}),
          rateUsdPerHour: entry.record.size === undefined ? 0 : ctx.backendFor(entry.record).pricing.rateUsdPerHour(entry.record.size),
          accruedUsd: ctx.accrued.get(workspaceId) ?? 0,
        };
      },
    }),
    deliver: async ({ threadId, workspaceId, prompt, requestId }) => {
      let queuedNow: (() => void) | undefined;
      const queued = new Promise<{ outcome: "queued" }>(resolve => {
        queuedNow = () => resolve({ outcome: "queued" });
      });
      const off = bus.on("session.queued", e => {
        if (e.type === "session.queued" && e.requestId === requestId) queuedNow?.();
      });
      try {
        const started = ctx.sessionsApi.start(workspaceId, { prompt, thread: threadId, requestId, startedBy: "person", via: "slate" });
        const first = await Promise.race([started, queued]);
        if ("outcome" in first && !("turnId" in first)) {
          started.catch((e: unknown) => console.warn(`a press waiting in thread ${threadWord(threadId)} was not sent: ${e instanceof Error ? e.message : String(e)}`));
          return { outcome: "queued" };
        }
        return { outcome: first.outcome, turnId: first.turnId };
      } finally {
        off();
      }
    },
    watchPr: workspaceId => {
      const entry = live.get(workspaceId);
      if (entry !== undefined) ctx.pollPullRequest(entry);
    },
    runEnv: () => local?.env() ?? (process.env as Record<string, string>),
    ...(opts.statePath !== undefined ? { secretsFile: join(ctx.stateFolder(), "slates.secrets.json"), slatesDir: join(ctx.stateFolder(), "slates") } : {}),
  });
  bus.on("workspace.napped", e => {
    if (e.type === "workspace.napped") slates.napped(e.workspaceId);
  });
  return { slates };
}
