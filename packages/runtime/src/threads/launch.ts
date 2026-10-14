// SPDX-License-Identifier: AGPL-3.0-only
// What a thread's launch runs at, and the agent's process a new thread's composer starts ahead of its first send.
import { randomBytes, randomUUID } from "node:crypto";
import { existsSync } from "node:fs";
import { serverValuesOf } from "@wsp/catalog";
import { MCP_SERVER_NAME, NO_SLATE_MCP_ARG, SCOPED_MCP_ARG, THREAD_ENV, TURN_TOKEN_ENV, type McpServerSpec, cloudAddedLine, cloudRemovedLine, placeSaid, isLocalWorkspace, keptPicks, listedPick, scopeOf, startPicks, type AccessChoice, type Preferences, type SessionWarmResult, type StartPicks } from "@wsp/protocol";
import { harnessCatalog, notRunRefused } from "../harness-catalog.js";
import type { HarnessAdapter } from "../types/harness.js";
import type { Runtime } from "../types/api.js";
import { launchesAs, type KeptLaunch, type ThreadRecord } from "../types/internal.js";
import type { PlaceDoor } from "../places.js";
import type { LiveWorkspace } from "../types/wiring.js";
import type { RuntimeContext } from "../context.js";

/** The wsp server a thread's launch is handed. A thread another thread started has no slate: its launch says nothing of
 * one, and on this computer, where the server is the host's own wsp and knows the word, its server is told too. A box's
 * server is served by this host as a guest, which reads the same off the thread's token; the box's own wsp may be
 * older and never sees a word. */
export function servedTo(wsp: McpServerSpec | undefined, rootThreadId: string, threadId: string): McpServerSpec | undefined {
  if (wsp === undefined || rootThreadId === threadId) return wsp;
  return { ...wsp, noSlate: true as const, ...(wsp.args.includes(SCOPED_MCP_ARG) ? { args: [...wsp.args, NO_SLATE_MCP_ARG] } : {}) };
}

/** Puts a line ahead of a send's message for each cloud added or taken away since the thread last knew them, and answers
 * the clouds held, which the thread's record takes once its turn is launched, so a launch that fails leaves the line for
 * the next send. The caller asks it only for a thread that may start sub-threads: one in a folder, never a sub-thread.
 * A thread with no record yet is told nothing, and a record from before the clouds were kept starts keeping them. */
export async function cloudsTold(door: PlaceDoor | undefined, records: Map<string, ThreadRecord & { workspaceId: string }>, threadId: string, o: { prompt: string }): Promise<string[]> {
  const held = ((await door?.rows()) ?? []).filter(p => p.kind === "provider").map(placeSaid).sort();
  const seen = records.get(threadId)?.clouds;
  const lines = seen === undefined ? [] : [...held.filter(c => !seen.includes(c)).map(cloudAddedLine), ...seen.filter(c => !held.includes(c)).map(cloudRemovedLine)];
  if (lines.length > 0) o.prompt = `${lines.join("\n")}\n\n${o.prompt}`;
  return held;
}

/** The picks a launch names, each left to the thread's own or the defaults where absent. */
export type LaunchAsked = { model?: string; effort?: string; permissionMode?: string; access?: AccessChoice; contextWindow?: string; fast?: boolean };

export function launchArea(ctx: RuntimeContext) {
  const { opts, deviceDoor, threadLaunch, setups } = ctx;
  /** What a launch on a thread runs at, against the agent's lists for the workspace with the marks the person's
   * defaults put on them: the access named, else the thread's own; the model, effort and window named, else the
   * thread's own latest, else the defaults where the thread has none. Read against the session the launch resumes. */
  const launchPicks = async (entry: LiveWorkspace, harness: string, adapter: HarnessAdapter, prefs: Preferences, threadId: string, o: LaunchAsked) => {
    const workspaceId = entry.record.id;
    const table = harnessCatalog(harness);
    // Checked against the binary's own lists, the ones the composer shows for this workspace, with the marks on
    // what the person's defaults resolve to here, which startPicks fills in for anything this start leaves out.
    const resolved = table === undefined ? undefined : ctx.defaultsOn(await ctx.catalogOn(table, entry, adapter), prefs, prefs.projectDefaults[entry.record.project]);
    const catalog = resolved?.catalog;
    const named = o.permissionMode ?? (o.access === undefined ? undefined : ctx.namedMode(catalog, harness, o.access));
    // The thread's own access, read against the list in front of us: a mode this harness does not take is a pick
    // that does not apply here, not a send to refuse. An access this send NAMED is still refused, by startPicks. The
    // model, effort and window it leaves out are the thread's own the same way; only a thread with none opens on
    // the defaults.
    const picksFor = (session: string | undefined): StartPicks & { contextWindow?: string } => {
      const access = named ?? (catalog === undefined ? undefined : listedPick(catalog.permissionModes, ctx.accessOf(workspaceId, threadId, session)));
      const ran = ctx.ranOn(workspaceId, threadId, session);
      const kept = keptPicks(catalog, ran, { ...(o.model !== undefined ? { model: o.model } : {}), ...(o.effort !== undefined ? { effort: o.effort } : {}), ...(o.contextWindow !== undefined ? { contextWindow: o.contextWindow } : {}) });
      const open = session === undefined && ran.model === undefined && ran.effort === undefined ? (resolved?.open ?? {}) : {};
      const model = kept.model ?? open.model;
      const effort = kept.effort ?? open.effort;
      let picks: StartPicks;
      try {
        picks = startPicks(catalog, { ...o, ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), permissionMode: access }, session === undefined, session === undefined ? undefined : ctx.resumedFact(workspaceId, session, "model"));
      } catch (e) {
        throw notRunRefused(catalog, model ?? o.model) ?? e;
      }
      return { ...picks, ...(kept.contextWindow !== undefined ? { contextWindow: kept.contextWindow } : {}) };
    };
    return { catalog, picksFor };
  };
  /** What a launch fixes for the life of the agent's process: the agent, the folder, the servers a caller named, the
   * binary's version and the person's setup for it. */
  const launchFixed = (entry: LiveWorkspace, harness: string, cwd: string, mcpServers: unknown, version: string | null | undefined): string => {
    const place = ctx.setupPlace(entry);
    return JSON.stringify({ harness, cwd, mcpServers, version, setup: place === undefined ? undefined : setups.launchOf(place, harness) });
  };
  /** The warm launches in flight, by workspace and agent: a second ask waits for the first, so one stands. */
  const warming = new Map<string, Promise<SessionWarmResult>>();
  const warmNow = async (entry: LiveWorkspace, harness: string, prefs: Preferences, asked: LaunchAsked & { cwd?: string }): Promise<SessionWarmResult> => {
    const workspaceId = entry.record.id;
    const threadId = randomUUID();
    const probe = ctx.adapterFor(entry, harness, {}, () => false, {}, threadId).adapter;
    if (probe.warm === undefined) return { warm: "none" };
    await ctx.confineSetup(entry, harness);
    const { catalog, picksFor } = await launchPicks(entry, harness, probe, prefs, threadId, asked);
    const picks = picksFor(undefined);
    const folder = await ctx.threadFolder(entry, asked);
    if (isLocalWorkspace(entry.record) && !existsSync(folder)) return { warm: "none" };
    const cwd = ctx.runsIn(entry, undefined, folder);
    const launch: KeptLaunch = { fixed: launchFixed(entry, harness, cwd, undefined, catalog?.version), picks: { ...picks } };
    const standing = ctx.warmOn(workspaceId, harness);
    if (standing !== undefined && launchesAs(standing[1].launch, launch) && launchesAs(launch, standing[1].launch)) {
      ctx.rewarm(standing[0]);
      return { warm: "standing" };
    }
    if (standing !== undefined) ctx.reapKept(standing[0]);
    const turnToken = randomBytes(16).toString("hex");
    const { scoped, env: launchEnv, wsp } = await threadLaunch(entry, threadId, threadId);
    try {
      const waiting = { on: false };
      const { adapter } = ctx.adapterFor(entry, harness, { [TURN_TOKEN_ENV]: turnToken, [THREAD_ENV]: threadId, ...launchEnv }, () => waiting.on, serverValuesOf(opts.vault?.() ?? {}), threadId);
      const served = adapter.mcpServers === true ? wsp : undefined;
      const launchServers = adapter.mcpServers === true || adapter.deniesServers === true ? await ctx.launchServersFor(entry, harness, adapter, cwd) : {};
      const agent = adapter.warm!({
        ...picks,
        cwd,
        ...(served !== undefined ? { mcpServers: { [MCP_SERVER_NAME]: served } } : {}),
        ...launchServers,
        ...(catalog?.source === "harness" && catalog.version !== null ? { version: catalog.version } : {}),
      });
      ctx.holdWarm(threadId, { workspaceId, agent, launch, turnToken, ...(scoped !== undefined ? { scopeDeviceId: scoped.deviceId } : {}), waiting, warm: { harness, claimed: false } });
      return { warm: "started" };
    } catch (e) {
      if (scoped !== undefined) void deviceDoor.revoke(scoped.deviceId).catch(() => {});
      throw e;
    }
  };

  const warm: Runtime["sessions"]["warm"] = async (workspaceId, asked, origin) => {
    await ctx.ready();
    // A thread's token starts nothing ahead: the process is the person's next thread's, and its token names a tree
    // of its own.
    if (scopeOf(origin) !== undefined) return { warm: "none" };
    const entry = await ctx.entryOf(workspaceId, origin);
    // A joined computer too small to keep one would end it the moment it stood.
    if (!ctx.moduleOf(entry.record.kind).keepsAgents || ctx.keptBox(workspaceId)?.cap === 0 || ctx.state.closing) return { warm: "none" };
    const prefs = ctx.state.preferencesHeld ?? (await ctx.preferences.get());
    const harness = asked.harness ?? ctx.defaultAgentOf(prefs, entry);
    if (ctx.agentOff(entry, harness)) return { warm: "none" };
    const key = `${workspaceId}\u0000${harness}`;
    const run = (warming.get(key) ?? Promise.resolve()).catch(() => {}).then(() => warmNow(entry, harness, prefs, asked));
    warming.set(key, run);
    void run.catch(() => {}).then(() => {
      if (warming.get(key) === run) warming.delete(key);
    });
    return run;
  };

  return { launchPicks, launchFixed, warm };
}
