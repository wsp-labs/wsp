// SPDX-License-Identifier: AGPL-3.0-only
import { CREATED_AT_LABEL, GOLDEN_LABEL, WORKSPACE_LABEL, lostWorkspace, goldenHead, isMissing, type ListedMachine, type Machine, type MachineBackend, type ReapFailure, type ReapedMachine } from "@wsp/engine";
import { RECORD_RESTORED } from "@wsp/protocol";
import { createStatusTracker } from "../status.js";
import type { WorkspaceRecord, AdoptedMachine } from "../types/wiring.js";
import { WORKSPACES, WORKSPACE_NAMES, DROPPED, DROPPED_WATCH_MS, UNRECORDED, UNRECORDED_DELETE_MS, type NamedWorkspace, type DroppedMachine, type UnrecordedMachine } from "../types/internal.js";
import type { RuntimeContext, StatusArea } from "../context.js";

export function statusArea(ctx: RuntimeContext): StatusArea {
  const { opts, backend, store, bus, clock, live, projectsHeld } = ctx;
  const status = createStatusTracker({
    store,
    records: async () => {
      await ctx.ready();
      return ctx.held().map(e => ({
        ...ctx.view(e.record),
        size: e.record.size,
        // The rate follows the machine's kind: a local workspace's backend prices it at zero, so no cost line rides its row.
        rateUsdPerHour: ctx.backendFor(e.record).pricing.rateUsdPerHour(e.record.size),
        generation: e.generation,
        // The wake's own line while one is in flight, and the words it left behind once its asking ran out: the poll
        // builds every status from the record, so a row that carried only what was pushed would fall silent between
        // two asks and forget the rebuild road at the next tick. A delete the provider sat on outranks both.
        // A pause or a stop the provider refused stays on the row as long as it stands, for the same reason, and so
        // does a disk past the line, which is said before a stop can fail on it.
        ...(ctx.rowReason(e) !== undefined ? { reason: ctx.rowReason(e)! } : {}),
        ...(e.wakeAsk !== undefined ? { wakeAsk: e.wakeAsk } : {}),
        ...(e.checkout !== undefined ? { checkout: e.checkout } : {}),
        ...(e.pr !== undefined ? { pr: e.pr } : {}),
        ...(e.record.phase === "running" && ctx.idle.idleAt(e.record.id) !== undefined ? { idleAt: ctx.idle.idleAt(e.record.id)! } : {}),
        ...(ctx.awayLine(e.record) !== undefined ? { away: ctx.awayLine(e.record)! } : {}),
        ...(ctx.unreachedOf(e) !== undefined ? { unreached: ctx.unreachedOf(e)! } : {}),
        ...(ctx.moduleOf(e.record.kind).hasDaemon(e) ? { daemonReach: () => ctx.moduleOf(e.record.kind).daemonRoad(e) } : {}),
        ...(e.machine.daemonAnswers !== undefined ? { daemonAnswers: e.machine.daemonAnswers.bind(e.machine) } : {}),
        ...(e.machine.daemonAnswersPromptMs !== undefined ? { daemonAnswersPromptMs: e.machine.daemonAnswersPromptMs } : {}),
        providerState: () => e.machine.state(),
        ...(e.machine.metrics !== undefined ? { metrics: e.machine.metrics.bind(e.machine) } : {}),
        // A machine the poll last found unreachable is not asked what it is: over ssh that read is a dial of its
        // own, so a box that is off would pay one every tick beside the dial the reach already makes.
        ...(e.machine.facts !== undefined && ctx.reachOf(e) !== "unreachable" ? { facts: e.machine.facts.bind(e.machine) } : {}),
        exec: (cmd, o) => e.machine.exec(cmd, o),
      }));
    },
    emit: e => bus.emit(e),
    pass: e => bus.pass(e),
    on: (type, l) => bus.on(type, l),
    // Every tick, not every change: this is where the runtime learns what its machines' reach actually is, and a
    // machine parked in one state is the case both readers of it exist for.
    onPolled: statuses => {
      for (const s of statuses) {
        const entry = live.get(s.id);
        if (entry === undefined || s.machineId !== entry.machine.id) continue;
        if (s.phase === "running") ctx.polledReach.set(s.id, { machineId: s.machineId, reach: s.reach.state });
        else ctx.polledReach.delete(s.id);
        // A daemon deploy is a run, which the sweep the boot still has out on that machine would end.
        if (ctx.bootWork.has(s.id)) continue;
        ctx.reviveDaemon(entry, s.reach.state);
        ctx.offerDaemonAgain(entry, s);
        ctx.readVersionAgain(entry, s);
      }
    },
    onGone: (id, machineId, reason) => {
      const entry = live.get(id);
      // A sighting of a machine since replaced says nothing about the one now under the record.
      if (entry !== undefined && entry.record.machineId === machineId) void ctx.adoptGone(entry, reason).catch((e: unknown) => console.warn(`${entry.record.id}'s gone reading was not confirmed: ${e instanceof Error ? e.message : String(e)}`));
    },
    ...(opts.status !== undefined ? { defaults: opts.status } : {}),
    clock,
  });

  /** A workspace machine of this setup's that no record claims is recorded again rather than killed: its record was
   * lost (a store the machine outlived), and it bills until a person can see and delete it. A running one whose
   * workspace was deleted here is left unclaimed, so the engine kills it. One this host cannot name is reported off
   * its listing row alone: a get() resets the provider's idle timer (measured), so a read every sweep would keep awake
   * the very machines it reports. A row about to be recorded is confirmed with one get(), so a row the listing lags on
   * after a kill is skipped; a create in flight elsewhere is left its minute. A row the provider would not confirm (a
   * failed read, a state that is neither running nor paused) is claimed in `known` all the same, so the engine spares
   * it this sweep and the next one records it: a kill never rides on one read. A row whose workspace this host keeps
   * no record or name for, or whose settled record names another machine, nothing here reaches: it is named with the
   * time it goes and deleted a day after the first sweep that listed it. `at` is the provider the listing came from. */
  const adoptLost = async (listing: ListedMachine[], known: Set<string>, failed: ReapFailure[], at: { place: string; backend: MachineBackend }): Promise<{ adopted: AdoptedMachine[]; reaped: ReapedMachine[] }> => {
    const adopted: AdoptedMachine[] = [];
    const reaped: ReapedMachine[] = [];
    const unreached = new Set<string>();
    const now = Date.now();
    const dropped = new Set<string>();
    for (const d of (await store.list(DROPPED)) as DroppedMachine[]) {
      if (clock.now() - Date.parse(d.at) < DROPPED_WATCH_MS) dropped.add(d.machineId);
      else await store.delete(DROPPED, d.machineId);
    }
    for (const row of listing) {
      // The engine kills only a running row, so a paused one stays reported rather than silently left.
      if (known.has(row.id) || (dropped.has(row.id) && row.state === "running") || !lostWorkspace(row, ctx.state.owner, now)) continue;
      // A stamped id another machine now holds is a body a rebuild replaced and failed to stop: the engine kills a
      // running one, and a paused one is left nothing but the delete below.
      const stamped = row.labels[WORKSPACE_LABEL];
      const holder = stamped === undefined ? undefined : live.get(stamped);
      // A record the start left as it was, since its place forked nothing then, still holds its machine.
      if (stamped !== undefined && holder === undefined && (await store.get(WORKSPACES, stamped)) !== undefined) {
        known.add(row.id);
        failed.push({ id: row.id, message: `not recorded: ${stamped} is stored here but was not read at start, so its ${row.state} machine is left as it is until the host starts again` });
        continue;
      }
      if (holder !== undefined && (row.state === "running" || holder.creating || holder.waking || holder.deleting || holder.napping)) continue;
      const kept = stamped === undefined || holder !== undefined ? undefined : ((await store.get(WORKSPACE_NAMES, stamped)) as NamedWorkspace | undefined);
      if (stamped !== undefined && kept === undefined) {
        known.add(row.id);
        unreached.add(row.id);
        let seenAt = ((await store.get(UNRECORDED, row.id)) as UnrecordedMachine | undefined)?.seenAt;
        if (seenAt === undefined) {
          seenAt = new Date(clock.now()).toISOString();
          await store.put(UNRECORDED, row.id, { machineId: row.id, place: at.place, seenAt } satisfies UnrecordedMachine);
        }
        const goesAt = Date.parse(seenAt) + UNRECORDED_DELETE_MS;
        if (clock.now() < goesAt) {
          const whose = holder === undefined ? `no workspace of this host is ${stamped}` : holder.record.machineId === "" ? `${stamped} holds no machine` : `${stamped} runs on ${holder.record.machineId}`;
          failed.push({ id: row.id, message: `not recorded: ${whose}, so nothing here reaches this ${row.state} machine of yours; deleted at ${new Date(goesAt).toISOString()}, a day after this host first saw it` });
          continue;
        }
        try {
          await ctx.gone.stop(at.backend, await at.backend.get(row.id));
        } catch (e) {
          if (!isMissing(e)) {
            failed.push({ id: row.id, message: `could not delete: ${e instanceof Error ? e.message : String(e)}; no record of this host's reaches it, retried next sweep` });
            continue;
          }
        }
        await store.delete(UNRECORDED, row.id);
        const bornAt = Date.parse(row.labels[CREATED_AT_LABEL] ?? "");
        reaped.push({ id: row.id, labels: row.labels, builder: false, reason: "unrecorded", ...(Number.isNaN(bornAt) ? {} : { ageMs: now - bornAt }) });
        continue;
      }
      // A workspace is one project's copy, so a machine whose project this host cannot name is not a workspace
      // here: it is reported rather than recorded, and the sweep's own --older-than is the road that ends it.
      if (stamped === undefined || kept?.project === undefined || !projectsHeld.has(kept.project)) {
        known.add(row.id);
        failed.push({ id: row.id, message: `not recorded: this host holds no project for it, and a workspace is one project's copy; it is a machine of yours still ${row.state}` });
        continue;
      }
      let machine: Machine;
      try {
        machine = ctx.observed(await at.backend.get(row.id));
      } catch (e) {
        if (isMissing(e)) continue;
        known.add(row.id);
        failed.push({ id: row.id, message: `not recorded: ${e instanceof Error ? e.message : String(e)}; retried next sweep` });
        continue;
      }
      const state = machine.seen?.state ?? (await machine.state());
      if (state !== "running" && state !== "paused") {
        known.add(row.id);
        continue;
      }
      const bornAt = row.labels[CREATED_AT_LABEL];
      const record: WorkspaceRecord = {
        id: stamped,
        name: ctx.nameRefusal(kept.name) === undefined ? kept.name : row.id,
        kind: "cloud",
        project: kept.project,
        ...(at.place !== ctx.places.wired ? { place: at.place } : {}),
        machineId: row.id,
        phase: state === "paused" ? "napping" : "running",
        golden: row.labels[GOLDEN_LABEL] ?? goldenHead(await ctx.golden.get())?.snapshotId ?? "",
        createdAt: bornAt !== undefined && !Number.isNaN(Date.parse(bornAt)) ? bornAt : new Date().toISOString(),
        spec: { labels: row.labels },
        size: ctx.sizeBuilt(await ctx.shapeOf(machine), at.backend.pricing.defaultSize),
        firstLife: false,
        ...(machine.streamUrl !== undefined ? { screen: { streamUrl: machine.streamUrl } } : {}),
      };
      const entry = ctx.attach(record, machine);
      await ctx.persist(record);
      if (record.phase === "running") void ctx.syncDaemon(entry);
      bus.emit({ type: "workspace.created", workspace: ctx.view(record) });
      await ctx.emitStatus(entry, record.phase === "running" ? ctx.reachOf(entry) : "napping", RECORD_RESTORED);
      adopted.push({ id: row.id, workspaceId: stamped, name: record.name, phase: record.phase });
    }
    // A machine this listing no longer carries unrecorded is forgotten, so the set holds only what the account holds.
    for (const u of (await store.list(UNRECORDED)) as UnrecordedMachine[]) if (u.place === at.place && !unreached.has(u.machineId)) await store.delete(UNRECORDED, u.machineId);
    return { adopted, reaped };
  };
  return { status, adoptLost };
}
