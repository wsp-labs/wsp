// SPDX-License-Identifier: AGPL-3.0-only
// Compute is shared only inside a lead's tree; each child keeps the workspace identity every existing operation reads.
import { createHash, randomUUID } from "node:crypto";
import { isMissing, killUntilGone, OWNER_LABEL, WORKSPACE_LABEL, type Machine, type MachineBackend, type MachineSpec } from "@wsp/engine";
import { CREATE_READY, SUB_SPAWN_LOG_LINES, startsIsolatedChildren, fmtBytes, forksNoMachines, HERE_PLACE_ID, scopeOf, shellQuote as q, subBranchNameOk, subNoThreadRefusal, subSizeOf, usageRefusal, type Caller, type SeedChoice } from "@wsp/protocol";
import type { RuntimeContext } from "../context.js";
import type { CreatedWorkspace, CreateWorkspaceOptions, IsolatedCheckout, LiveWorkspace, StageReport, WorkspaceRecord } from "../types/wiring.js";
import { SEED_CHOICES, WORKSPACES } from "../types/internal.js";
import { checkoutIgnoredHash, transferCheckout, transferReturnedBranch } from "../projects/checkout-transfer.js";

export const ISOLATED_GROUPS = "isolated-groups";
export const ISOLATED_IDLE_MS = 30 * 60_000;
const CLEANUP_RETRY_MS = 60_000;
type Group = { id: string; root: string; place: string; spec: MachineSpec; members: string[]; machineId?: string; idleSince?: number };
export interface IsolatedArea {
  initialize(): Promise<void>;
  close(): Promise<void>;
  slotKeys(root: string): string[];
  starting(workspaceId: string): Promise<() => void>;
  backend(record: WorkspaceRecord, actual: MachineBackend): MachineBackend;
  spawn(opts: CreateWorkspaceOptions, origin: Caller | undefined, place: string): Promise<CreatedWorkspace>;
  checkRelease(entry: LiveWorkspace): Promise<void>;
  release(entry: LiveWorkspace): Promise<void>;
  revive(entry: LiveWorkspace): Promise<LiveWorkspace>;
  returned(lead: LiveWorkspace, child: LiveWorkspace): Promise<{ branch: string; head: string; from: string; cleanup: () => Promise<void> }>;
  sweep(): Promise<void>;
}

export function isolatedArea(ctx: RuntimeContext): IsolatedArea {
  const { store, live, clock } = ctx;
  const locks = new Map<string, Promise<unknown>>();
  const allocating = new Set<string>();
  const detached = new Set<string>();
  const closing = new Set<string>();
  const starts = new Map<string, number>();
  const activity = new Set<string>();
  let stopped = false;
  let timer: (() => void) | undefined;
  let retryAt = Infinity;
  const starting = async (workspaceId: string): Promise<() => void> => {
    await ctx.ready();
    const copy = live.get(workspaceId)?.record.isolated;
    if (copy === undefined) return () => {};
    if (closing.has(copy.group)) throw usageRefusal("The isolated runtime is being released.", "Wait for cleanup to finish before sending another message.");
    starts.set(workspaceId, (starts.get(workspaceId) ?? 0) + 1);
    activity.add(copy.group);
    void sweep().catch(e => console.warn(`isolated cleanup: ${String(e)}`));
    return () => { const count = (starts.get(workspaceId) ?? 1) - 1; if (count === 0) starts.delete(workspaceId); else starts.set(workspaceId, count); void sweep().catch(e => console.warn(`isolated cleanup: ${String(e)}`)); };
  };
  const groups = new Map<string, Group>();
  const initialize = async (): Promise<void> => { for (const raw of await store.list(ISOLATED_GROUPS)) { const group = raw as Group; groups.set(group.id, group); } };
  const saveGroup = async (group: Group): Promise<void> => { await store.put(ISOLATED_GROUPS, group.id, group); groups.set(group.id, group); };
  const dropGroup = async (id: string): Promise<void> => { await store.delete(ISOLATED_GROUPS, id); groups.delete(id); activity.delete(id); };
  const slotKeys = (root: string): string[] => [...groups.values()].filter(group => group.root === root && !group.members.some(id => allocating.has(id))).map(group => group.id);
  const serial = async <T>(key: string, work: () => Promise<T>): Promise<T> => {
    const before = locks.get(key) ?? Promise.resolve();
    const next = before.catch(() => {}).then(work);
    locks.set(key, next);
    try { return await next; } finally { if (locks.get(key) === next) locks.delete(key); }
  };
  const actualAt = (place: string): MachineBackend => {
    const at = ctx.placeDoorOf().backendOf(place);
    if (at === undefined || forksNoMachines(at.capabilities)) throw usageRefusal(`${ctx.placeName(place)} cannot start an isolated runtime.`, "Check that computer's runtime support in Computers and try again.");
    return at;
  };
  const groupOf = async (id: string) => { const group = await store.get(ISOLATED_GROUPS, id) as Group | undefined; if (group !== undefined) groups.set(id, group); return group; };
  const explicitlySettled = (threadId: string): boolean => {
    const at = ctx.threadRecords.get(threadId)?.settledAt;
    return at !== undefined && ctx.rowsOn(threadId).every(row => Math.max(row.startedAt ?? 0, row.endedAt ?? 0) <= at);
  };
  const held = () => usageRefusal("This child shares its runtime with its lead's other children.", "Settle or delete this child after returning its work; safe idle checkouts are removed after 30 minutes.");
  const wrap = (machine: Machine): Machine => new Proxy(machine, {
    get(target, property) {
      if (["kill", "pause", "snapshot", "resumeAt"].includes(String(property))) return async () => { throw held(); };
      const member = Reflect.get(target, property, target) as unknown;
      return typeof member === "function" ? member.bind(target) : member;
    },
  });
  const backend = (record: WorkspaceRecord, actual: MachineBackend): MachineBackend => new Proxy(actual, {
    get(target, property) {
      if (property === "capabilities") { const { pauseMode: _pause, ...caps } = target.capabilities; return { ...caps, diskSnapshots: false, replacesMachine: false }; }
      if (["lifecycle", "createRequest", "replayCreate"].includes(String(property))) return undefined;
      if (property === "create") return async (spec: MachineSpec) => serial(record.isolated!.group, async () => {
        const id = record.isolated!.group;
        let group = await groupOf(id);
        if (group === undefined) group = { id, root: record.isolated!.rootThreadId, place: record.place!, spec: { ...spec, idempotencyKey: `isolated/${randomUUID()}`, labels: { ...spec.labels, [OWNER_LABEL]: `${ctx.state.owner}/isolated/${id}`, [WORKSPACE_LABEL]: id, "wsp.isolated": id } }, members: [] };
        group.members = [...new Set([...group.members, record.id])];
        await saveGroup(group);
        try {
          const machine = group.machineId === undefined ? await target.create(group.spec) : await target.get(group.machineId);
          group.machineId = machine.id;
          await saveGroup(group);
          return wrap(machine);
        } catch (e) {
          group.members = group.members.filter(member => member !== record.id);
          await saveGroup(group);
          throw e;
        }
      });
      if (property === "get") return async (id: string) => wrap(await target.get(id));
      const member = Reflect.get(target, property, target) as unknown;
      return typeof member === "function" ? member.bind(target) : member;
    },
  });
  const read = async (entry: LiveWorkspace, cmd: string): Promise<string> => {
    const result = await entry.machine.exec(cmd, { timeoutMs: 30_000 });
    if (result.exitCode !== 0) throw new Error(`The child checkout could not be read: ${(result.stderr || result.stdout).trim().slice(-500)}. Check the computer and retry.`);
    return result.stdout.trim();
  };
  const clean = async (entry: LiveWorkspace): Promise<void> => {
    const copy = entry.record.isolated!;
    if ([...live.values()].some(e => e.record.isolated?.sourceWorkspaceId === entry.record.id && (e.record.phase !== "released" || starts.has(e.record.id) || allocating.has(e.record.id)))) throw usageRefusal("Another child still needs this checkout to return its work.", "Return and release that child before releasing its source checkout.");
    if (copy.state === "releasing") {
      const exists = await entry.machine.exec(`test -d ${q(copy.checkout)}`);
      if (exists.exitCode === 1) return;
      if (exists.exitCode !== 0) throw new Error("The releasing checkout could not be read. Reconnect the computer and retry cleanup.");
    } else if (copy.state !== "ready") return;
    if (ctx.turnRuns(entry.record.id) || starts.has(entry.record.id) || allocating.has(entry.record.id)) throw usageRefusal("The child is still working or starting.", "Stop its turn or wait for it before releasing its checkout.");
    if (!copy.git) throw usageRefusal("This child holds a folder whose work has no Git branch to return.", "Export its folder before deleting its runtime.");
    const changed = await read(entry, `git -C ${q(copy.checkout)} status --porcelain=v1`);
    const head = await read(entry, `git -C ${q(copy.checkout)} rev-parse HEAD`);
    const stashes = await read(entry, `git -C ${q(copy.checkout)} stash list --format=%H`);
    if (stashes !== "") throw usageRefusal("This child holds stashed work.", "Restore and return its stashes, or export them, before releasing this checkout.");
    const saved = await read(entry, `git -C ${q(copy.checkout)} rev-list --max-count=1 --exclude='refs/wsp/*' --all --not ${q(copy.base)}${copy.returned === undefined ? "" : ` ${q(copy.returned)}`}`);
    if (saved !== "") throw usageRefusal("This child holds saved Git commits that have not returned to its lead.", "Merge its saved branches into its result branch and use merge_in from the lead before releasing this checkout.");
    const ignored = await checkoutIgnoredHash(entry.machine, copy.checkout);
    if (ignored !== copy.ignoredHash || changed !== "" || (head !== copy.base && head !== copy.returned)) throw usageRefusal("This child holds changes that have not returned to its lead.", "Commit its work and use merge_in from the lead before releasing this checkout.");
  };
  const release = async (entry: LiveWorkspace, idleOnly = false): Promise<void> => serial(entry.record.isolated!.group, async () => {
    if (detached.has(entry.record.id) || entry.record.phase === "released") return;
    const copy = entry.record.isolated!;
    closing.add(copy.group);
    try {
      const group = await groupOf(copy.group);
      if (group === undefined) throw new Error("The isolated runtime's ownership record is missing. Keep its checkout and inspect the computer before retrying.");
      if (idleOnly && (activity.has(copy.group) || groupBusy(group) || group.idleSince === undefined || clock.now() < group.idleSince + ISOLATED_IDLE_MS)) return;
      await clean(entry);
      const stored = new Set((await store.list(WORKSPACES) as WorkspaceRecord[]).filter(r => r.phase !== "released").map(r => r.id));
      const others = group.members.filter(id => id !== entry.record.id && !detached.has(id) && ((live.has(id) && live.get(id)!.record.phase !== "released") || stored.has(id) || allocating.has(id)));
      copy.state = "releasing";
      // A reconnect may have replaced the live entry while clean read the checkout.
      const current = live.get(entry.record.id);
      if (current !== undefined && current !== entry) current.record = entry.record;
      await ctx.persist(entry.record);
      if (others.length === 0) {
        const actual = actualAt(group.place);
        try { await killUntilGone(actual, await actual.get(entry.machine.id), ctx.opts.killConfirm); } catch (e) { if (!isMissing(e)) throw e; }
      } else {
        const result = await entry.machine.exec(`rm -rf -- ${q(copy.checkout)}`, { timeoutMs: 30_000 });
        if (result.exitCode !== 0) throw new Error("The child checkout was not removed. Keep the runtime and retry deleting this child.");
      }
      entry.record.phase = "released";
      copy.releasedAt = clock.now();
      copy.state = "ready";
      await ctx.persist(entry.record);
      if (others.length === 0) await dropGroup(group.id);
      else await saveGroup({ ...group, members: others });
      detached.add(entry.record.id);
    } finally { closing.delete(copy.group); }
  });
  const copyInto = async (entry: LiveWorkspace, report?: StageReport): Promise<void> => {
    const meta = entry.record.isolated!;
    let source = live.get(meta.sourceWorkspaceId);
    if (source?.record.phase === "released" && source.record.isolated !== undefined) source = await revive(source);
    if (source === undefined) throw usageRefusal("The source checkout is no longer available.", "Start the child from an available thread of this project.");
    await ctx.copyBlocked(source);
    const project = ctx.projectHeld(entry.record.project);
    const choices = await store.get(SEED_CHOICES, project.path) as SeedChoice | undefined;
    report?.("project-cloned", `Copying the current checkout to ${ctx.placeName(entry.record.place!)}.`);
    if (meta.spawn !== undefined) meta.spawn.files = { since: clock.now(), paths: [meta.sourceFolder], bytes: 0 };
    const made = await transferCheckout(source.machine, entry.machine, { sourceFolder: meta.sourceFolder, destinationFolder: meta.checkout, branch: meta.branch, ...(choices !== undefined ? { picked: choices.files } : {}), onProgress: (bytes, total) => { if (meta.spawn?.files !== undefined) meta.spawn.files.bytes = bytes; report?.("project-cloned", `Copying project: ${fmtBytes(bytes)} of ${fmtBytes(total)}.`, { waiting: true }); } });
    Object.assign(meta, { folder: made.folder, base: made.head, git: made.git, sourceBranch: made.sourceBranch, sent: { paths: [meta.sourceFolder, ...made.sent], bytes: made.bytes }, state: "ready" });
    if (meta.spawn?.files !== undefined) Object.assign(meta.spawn.files, { paths: meta.sent!.paths, bytes: made.bytes, ms: clock.now() - meta.spawn.files.since });
    if (meta.git) meta.ignoredHash = await checkoutIgnoredHash(entry.machine, meta.checkout);
    await ctx.writeDaemonRoots(entry, true);
    await ctx.persist(entry.record);
  };
  const spawn = async (opts: CreateWorkspaceOptions, origin: Caller | undefined, place: string): Promise<CreatedWorkspace> => {
    const scope = scopeOf(origin);
    if (scope === undefined) throw subNoThreadRefusal(opts.sub!.on);
    if (place === HERE_PLACE_ID) throw usageRefusal("That is the computer this host runs on.", "Choose a connected computer for an isolated child, or use --branch for a local worktree.");
    if (explicitlySettled(scope.rootThreadId)) throw usageRefusal("The lead's tree is already settled.", "Send a new message to its lead before starting another child.");
    const source = await ctx.entryOf(scope.workspaceId, origin);
    if (!startsIsolatedChildren(source.record.kind)) throw usageRefusal("A cloud sub-thread cannot allocate a connected computer.", "Ask its lead to start the child on that computer.");
    const project = ctx.projectHeld(source.record.project);
    if (opts.project !== "" && opts.project !== project.id && opts.project !== project.name) throw usageRefusal("The child must use its asking thread's project.", "Omit the project, or name the project this thread runs in.");
    if (opts.sub!.from !== undefined) throw usageRefusal("An isolated computer child copies the current checkout, so --from does not apply.", "Start from the thread whose current checkout you want to copy.");
    if (ctx.placeAway(place)) throw usageRefusal(`${ctx.placeName(place)} is not connected.`, "Reconnect that computer and retry the child.");
    const actual = actualAt(place);
    await ctx.placeRefuses(place);
    const groupId = createHash("sha256").update(`${scope.rootThreadId}\0${place}`).digest("hex").slice(0, 24);
    return serial(`spawn/${groupId}`, async () => {
      const group = await groupOf(groupId);
      const free = ctx.spawnGuard(group === undefined ? "fork" : "thread_new", origin);
      const id = `ws_${randomUUID().replaceAll("-", "").slice(0, 12)}`;
      const name = ctx.nameGiven(opts.name);
      const branch = opts.sub!.branch ?? `wsp/${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 45) || "child"}-${id.slice(-6)}`;
      if (!subBranchNameOk(branch)) { free(); throw usageRefusal("The child branch name is not valid.", "Choose a valid branch name and retry."); }
      const refused = ctx.nameRefusal(name);
      if (refused !== undefined) { free(); throw usageRefusal(refused, "Choose a different child title."); }
      const size = opts.sub!.size === undefined ? undefined : subSizeOf(opts.sub!.size);
      if (opts.sub!.size !== undefined && size === undefined) { free(); throw usageRefusal("The requested size is not valid.", "Use a size such as 2x4, or omit --size."); }
      if (group !== undefined && size !== undefined && (size.cpu !== group.spec.cpu || size.memMb !== group.spec.memMb)) { free(); throw usageRefusal("This lead already has a runtime on that computer at another size.", "Use its existing size for siblings, or start a new lead."); }
      const chosen = size ?? (group === undefined ? actual.pricing.defaultSize : { cpu: group.spec.cpu ?? actual.pricing.defaultSize.cpu, memMb: group.spec.memMb ?? actual.pricing.defaultSize.memMb });
      const checkout = `/tmp/wsp-children/${groupId}/${id}`;
      const harness = opts.sub!.harness ?? ctx.defaultAgentOf(ctx.state.preferencesHeld ?? await ctx.preferences.get(), source);
      const start = opts.sub!.start === undefined ? undefined : { ...opts.sub!.start, harness };
      const meta: IsolatedCheckout & { place: string } = { place, on: ctx.placeName(place), spawn: { startedAt: clock.now(), machine: { since: clock.now(), states: [] }, log: [] }, group: groupId, rootThreadId: scope.rootThreadId, sourceWorkspaceId: source.record.id, sourceFolder: ctx.checkoutOf(source.record), checkout, folder: checkout, branch, base: "", git: false, state: "copying" };
      const latest = ctx.rowsOn(scope.threadId).at(-1);
      if (latest?.cwd !== undefined) meta.sourceFolder = latest.cwd;
      const publish = ctx.stageReporter(id, name, clock.now(), scope, place, start);
      const report: StageReport = (stage, message, said) => {
        const spawn = meta.spawn!;
        spawn.log = [...(said?.waiting ? spawn.log.slice(0, -1) : spawn.log), message].slice(-SUB_SPAWN_LOG_LINES);
        publish(stage, message, said);
      };
      allocating.add(id);
      ctx.forking.add(name);
      try {
        await ctx.createStaged({ ...opts, ...(start === undefined ? {} : { sub: { ...opts.sub!, start } }), name, cpu: chosen.cpu, memMb: chosen.memMb, parent: source.record.id, idleWindowMs: null }, project, id, report, scope, free, undefined, meta);
        const entry = live.get(id)!;
        Object.assign(meta.spawn!.machine!, { id: entry.machine.id, ms: clock.now() - meta.spawn!.machine!.since });
        await copyInto(entry, report);
        if (explicitlySettled(scope.rootThreadId)) throw usageRefusal("The lead was settled while its child was being copied.", "Send a new message to the lead before retrying this child.");
        meta.spawn!.readyAt = clock.now();
        await ctx.persist(entry.record);
        delete entry.creating;
        report("ready", CREATE_READY);
        ctx.bus.emit({ type: "workspace.created", workspace: ctx.view(entry.record) });
        const preamble = `This child runs in an isolated copy on ${ctx.placeName(place)}. Its current project, including uncommitted work and selected ignored files, was copied to ${meta.folder}. Its branch is ${branch}. Siblings share compute but have separate checkouts. Commit your result here; the lead can use merge_in to receive it over wsp's link. Do not push merely to transfer the result.`;
        const view = ctx.view(entry.record);
        return { ...view, sub: { harness, folder: meta.folder, preamble, out: { mode: "isolated", on: ctx.placeName(place), machine: name, size: chosen, rateUsdPerHour: 0, repo: project.remote || project.name, base: { branch: meta.sourceBranch ?? "", sha: meta.base }, branch, sent: { paths: meta.sent?.paths ?? [], bytes: meta.sent?.bytes ?? 0, missing: [] }, notSent: { uncommitted: 0 } } } };
      } catch (e) {
        const entry = live.get(id);
        if (entry !== undefined) {
          if (meta.spawn !== undefined) meta.spawn.failed = { step: meta.spawn.files === undefined ? "machine" : "files", said: e instanceof Error ? e.message : String(e) };
          entry.record.isolated!.state = "failed";
          delete entry.creating;
          await ctx.persist(entry.record);
          try { await release(entry); await ctx.drop(id); } catch (cleanup) { console.warn(`isolated failed-start cleanup: ${String(cleanup)}`); }
        }
        throw e;
      } finally { allocating.delete(id); ctx.forking.delete(name); free(); void sweep().catch(e => console.warn(`isolated cleanup: ${String(e)}`)); }
    });
  };
  const revive = async (entry: LiveWorkspace): Promise<LiveWorkspace> => serial(`revive/${entry.record.id}`, async () => {
    const current = live.get(entry.record.id) ?? entry;
    if (current.record.phase !== "released") return current;
    const copy = entry.record.isolated!;
    allocating.add(entry.record.id);
    detached.delete(entry.record.id);
    try {
      copy.state = "copying";
      copy.spawn = { startedAt: clock.now(), machine: { since: clock.now(), states: [] }, log: [] };
      entry.record.phase = "running";
      entry.creating = true;
      await ctx.persist(entry.record);
      await ctx.fork(entry.record, machine => { entry.record.machineId = machine.id; ctx.attach(entry.record, machine); }, undefined, undefined, false);
      const renewed = live.get(entry.record.id)!;
      Object.assign(copy.spawn!.machine!, { id: renewed.machine.id, ms: clock.now() - copy.spawn!.machine!.since });
      await copyInto(renewed);
      copy.spawn!.readyAt = clock.now();
      await ctx.persist(renewed.record);
      delete renewed.creating;
      return renewed;
    } catch (e) {
      const failed = live.get(entry.record.id) ?? entry;
      if (copy.spawn !== undefined) copy.spawn.failed = { step: copy.spawn.files === undefined ? "machine" : "files", said: e instanceof Error ? e.message : String(e) };
      failed.record.isolated!.state = "failed";
      await ctx.persist(failed.record);
      try {
        await release(failed);
        failed.record.phase = "released";
        await ctx.persist(failed.record);
      } catch (cleanup) { console.warn(`isolated restart cleanup: ${String(cleanup)}`); }
      delete failed.creating;
      for (const [threadId, thread] of ctx.threadRecords) if (thread.workspaceId === failed.record.id) ctx.pushHead(threadId);
      await ctx.rowsSent();
      throw e;
    } finally { allocating.delete(entry.record.id); }
  });
  const returned = async (lead: LiveWorkspace, child: LiveWorkspace) => {
    const copy = child.record.isolated!;
    if (!copy.git) throw usageRefusal("The source project is a plain folder, so it has no branch to merge into.", "Export the child's folder to receive its files.");
    if (ctx.turnRuns(child.record.id)) throw usageRefusal("The child is still working.", "Wait for its turn to finish before merging its branch.");
    const current = await read(child, `git -C ${q(copy.checkout)} symbolic-ref --quiet --short HEAD`);
    if (current !== copy.branch) throw usageRefusal("The child is no longer on its assigned result branch.", `Switch it back to ${copy.branch} before returning its work.`);
    return transferReturnedBranch(child.machine, lead.machine, { sourceFolder: copy.checkout, destinationFolder: ctx.checkoutOf(lead.record), branch: copy.branch, base: copy.base });
  };
  const tell = (entry: LiveWorkspace, text: string, includeLead = false): void => {
    const child = [...ctx.sessions.values()].filter(s => s.view.workspaceId === entry.record.id).at(-1);
    const lead = includeLead ? [...ctx.sessions.values()].filter(s => s.view.threadId === entry.record.isolated!.rootThreadId).at(-1) : undefined;
    for (const row of new Set([child, lead])) if (row?.view.threadId !== undefined) ctx.record({ type: "session.notice", workspaceId: row.view.workspaceId, sessionId: row.view.claudeSessionId ?? row.turnId, turnId: row.turnId, threadId: row.view.threadId, text });
  };
  let sweeping: Promise<void> | undefined;
  let again = false;
  const busy = (e: LiveWorkspace): boolean => e.creating === true || allocating.has(e.record.id) || ctx.turnRuns(e.record.id) || starts.has(e.record.id);
  const groupBusy = (group: Group): boolean => group.members.some(id => allocating.has(id) || starts.has(id) || ctx.turnRuns(id)) || [...live.values()].some(e => e.record.isolated?.group === group.id && busy(e));
  const arm = (): void => {
    timer?.(); timer = undefined;
    if (stopped || ctx.state.closed) return;
    const now = clock.now();
    const due = Math.min(retryAt, ...[...groups.values()].flatMap(g => g.idleSince === undefined ? [] : [g.idleSince + ISOLATED_IDLE_MS > now ? g.idleSince + ISOLATED_IDLE_MS : now + CLEANUP_RETRY_MS]));
    if (Number.isFinite(due)) timer = clock.schedule(() => { timer = undefined; void sweep().catch(e => console.warn(`isolated cleanup: ${String(e)}`)); }, Math.max(0, due - now), { unref: true });
  };
  const pass = async (): Promise<void> => {
    retryAt = Infinity;
    const stored = new Set((await store.list(WORKSPACES) as WorkspaceRecord[]).filter(r => r.phase !== "released").map(r => r.id));
    for (const known of [...groups.values()]) {
      await serial(known.id, async () => {
        const group = await groupOf(known.id);
        if (group === undefined) return;
        const members = group.members.filter(id => (live.has(id) && live.get(id)!.record.phase !== "released") || stored.has(id) || allocating.has(id));
        if (members.length !== group.members.length) { group.members = members; await saveGroup(group); }
        if (members.length > 0) return;
        const root = ctx.threadRecords.get(group.root);
        if (group.machineId === undefined && !explicitlySettled(group.root) && (root !== undefined || ctx.rowsOn(group.root).length > 0)) return;
        // No child ever got a checkout. Replay the same durable allocation key to find a lost create answer.
        const actual = actualAt(group.place);
        try {
          const machine = group.machineId === undefined ? await actual.create(group.spec) : await actual.get(group.machineId);
          await killUntilGone(actual, machine, ctx.opts.killConfirm);
        } catch (e) { if (!isMissing(e)) throw e; }
        await dropGroup(group.id);
      });
    }
    const entries = [...live.values()].filter(e => e.record.isolated !== undefined && e.record.phase !== "released");
    const seen = new Set<string>();
    for (const entry of entries) {
      const copy = entry.record.isolated!;
      if (seen.has(copy.group)) continue;
      seen.add(copy.group);
      const group = await serial(copy.group, async () => {
        const current = await groupOf(copy.group);
        if (current === undefined) return undefined;
        const touched = activity.delete(copy.group);
        if (groupBusy(current)) {
          if (current.idleSince !== undefined) { delete current.idleSince; await saveGroup(current); }
        } else if (touched || current.idleSince === undefined) {
          current.idleSince = clock.now(); await saveGroup(current);
        }
        return current;
      });
      if (group === undefined) continue;
      const peers = entries.filter(e => e.record.isolated!.group === copy.group);
      const root = ctx.threadRecords.get(copy.rootThreadId);
      const orphan = root === undefined && ctx.rowsOn(copy.rootThreadId).length === 0;
      const expired = !groupBusy(group) && group.idleSince !== undefined && clock.now() >= group.idleSince + ISOLATED_IDLE_MS;
      for (const peer of peers) {
        const meta = peer.record.isolated!;
        const threads = [...ctx.threadRecords].filter(([, t]) => t.workspaceId === peer.record.id);
        const settled = threads.length > 0 && threads.every(([id]) => explicitlySettled(id));
        if (busy(peer) || !(expired || settled || explicitlySettled(copy.rootThreadId) || orphan || meta.state === "releasing")) continue;
        try {
          await release(peer, !settled && !explicitlySettled(copy.rootThreadId) && !orphan && meta.state !== "releasing");
          if (peer.record.phase !== "released") continue;
          delete meta.held;
          await ctx.persist(peer.record);
          await ctx.emitStatus(peer, "gone");
          again = true;
          tell(peer, `The checkout on ${ctx.placeName(peer.record.place!)} was released. This conversation is kept; another message starts a fresh copy of its source project.`);
        } catch (e) {
          retryAt = Math.min(retryAt, clock.now() + CLEANUP_RETRY_MS);
          const message = e instanceof Error ? e.message : String(e);
          if (meta.held === message) continue;
          meta.held = message; await ctx.persist(peer.record);
          tell(peer, `The isolated checkout on ${ctx.placeName(peer.record.place!)} is kept: ${message}`, true);
        }
      }
    }
  };
  const sweep = (): Promise<void> => {
    if (stopped || ctx.state.closed) return Promise.resolve();
    if (sweeping !== undefined) { again = true; return sweeping; }
    sweeping = (async () => { do { again = false; await pass(); } while (again && !stopped); })().catch(e => { retryAt = Math.min(retryAt, clock.now() + CLEANUP_RETRY_MS); throw e; }).finally(() => { sweeping = undefined; arm(); });
    return sweeping;
  };
  const request = (): void => { void sweep().catch(e => console.warn(`isolated cleanup: ${String(e)}`)); };
  const close = async (): Promise<void> => { stopped = true; timer?.(); timer = undefined; await sweeping?.catch(e => console.warn(`isolated cleanup at close: ${String(e)}`)); };
  ctx.bus.on("thread.marked", request);
  ctx.bus.on("workspace.deleted", request);
  ctx.bus.on("session.end", request);
  return { initialize, close, slotKeys, starting, backend, spawn, checkRelease: clean, release, revive, returned, sweep };
}
