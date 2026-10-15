// SPDX-License-Identifier: AGPL-3.0-only
// The queue every sub-thread's spawn goes through: its record stands queued
// with no machine, under its lead, until its thread's start comes, and past
// that while its cloud has no machine slot or no start left this minute (05,
// Q4 to Q9), its thread's row reading Waiting; spawns start first in, first
// out across the host when a machine stops holding a slot, a setting moves or
// a read of the cloud's limits shows room. The start is written down, so a
// host that restarts starts it all the same; a stop drops it with no start spent.
import { randomUUID } from "node:crypto";
import { failureWords, type BoxLimits } from "@wsp/engine";
import { CLOUD_CAP_DEFAULT, SUB_DROPPED_LINE, capWaitLine, deletedBeforeStartLine, keptOpening, listedFailure, providerKeyName, roadOf, scopeOf, subWaitingRefusal, type Caller, type SessionView, type ThreadCapWait } from "@wsp/protocol";
import { SUB_STARTS, absentMachine, deadMachine, isCapRefusal, type SessionEntry, type SubStartRecord } from "../types/internal.js";
import type { LiveWorkspace, SessionHandle, WorkspaceRecord } from "../types/wiring.js";
import type { RuntimeContext, SubQueueArea } from "../context.js";
import { cutOff } from "../pipe.js";

type StartOptions = Parameters<RuntimeContext["sessionsApi"]["start"]>[1];

/** How often a held spawn reads its cloud's limits again for room another client freed (Q5), and for the next minute
 * of starts, which the cloud's own read keeps no longer than its cache (`LIMITS_KEPT_MS`). */
export const QUEUE_SLOT_READ_MS = 30_000;
export const QUEUE_MINUTE_READ_MS = 10_000;
/** How long a queued record waits for its first turn, which the line that made it sends straight after the create;
 * one whose start never came goes, as at boot. */
export const QUEUE_START_WAIT_MS = 30_000;

/** What of a start rides the written record: the words and picks the person or the lead gave, never a callback or the
 * asking turn's token, which a restart outlives. */
const KEPT = ["prompt", "harness", "model", "effort", "permissionMode", "access", "contextWindow", "fast", "startedBy", "requestId", "attempt", "title", "cwd", "attachments"] as const;

/** One held spawn's start: the caller still waiting on it, where one is. */
interface Held {
  record: SubStartRecord;
  /** The machine's name, which a delete of its record names. */
  name: string;
  started?: (handle: SessionHandle) => void;
  failed?: (e: Error) => void;
  /** Answers the caller held, once, for the first wait the start meets once it is written down: a start that came
   * with room can meet one in the queue, where a spawn asked beside it took the room first. */
  tell?: (wait: ThreadCapWait) => void;
  written?: true;
  /** Settled once the start leaves the queue either way, which a stop of its row waits on as it waits on a launch. */
  left: () => void;
  gone: Promise<void>;
  /** A stop that came once its machine was being made: the machine goes as it lands, and no thread starts on it. */
  stopped?: string;
}

export function subQueueArea(ctx: RuntimeContext): SubQueueArea {
  const { live, store, sessions, bus, clock } = ctx;
  const held = new Map<string, Held>();
  /** The count each held record's row says, as the queue last read it. */
  const lastWait = new Map<string, ThreadCapWait>();
  /** The spawns let past the count whose machine has no record live yet, by workspace id, with their place, and whether
   * their machine has bound: each holds a slot from the read on, and a start of this minute until its machine binds.
   * The cloud's kept read counts the start once its create is posted, which is before the box is ready, so until then
   * the start counts in both places and a spawn at the minute's last start may wait one read longer than it needs. */
  const starting = new Map<string, { placeId: string; posted: boolean }>();
  /** A spawn the cloud refused at its cap, by when it is let through again: the slot went to another client, and asking
   * again at once would only be refused again. */
  const refusedUntil = new Map<string, number>();
  /** When each record was queued on this host, which its start's bound counts from; its `since` may sit earlier, at the
   * head of the line. */
  const queuedAt = new Map<string, number>();

  const queued = (): LiveWorkspace[] =>
    [...live.values()].filter(e => e.record.phase === "queued" && e.record.queue !== undefined).sort((a, b) => a.record.queue!.since.localeCompare(b.record.queue!.since) || a.record.id.localeCompare(b.record.id));

  /** This host's machines on a cloud that hold a slot, against the machines at once the person set there. */
  const slots = (placeId: string): ThreadCapWait => ({
    placeId,
    place: ctx.placeDoor?.nameOf(placeId) ?? providerKeyName(placeId),
    running:
      [...live.values()].filter(e => ctx.placeIdOf(e.record) === placeId && ctx.holdsSlot(e.record)).length +
      [...starting].filter(([id, s]) => s.placeId === placeId && !live.has(id)).length,
    atOnce: ctx.placeDoor?.machinesAt(placeId) ?? CLOUD_CAP_DEFAULT.machines,
    why: "slot",
  });

  /** What holds a spawn back on a cloud now: this host's machines there against `--machines`, the account's boxes
   * against the plan's machines at once, then the minute's starts. Nothing where there is room. */
  const waitOn = (placeId: string, limits: BoxLimits | undefined): ThreadCapWait | undefined => {
    const mine = slots(placeId);
    if (mine.running >= mine.atOnce) return mine;
    const atOnce = limits?.machinesAtOnce;
    const active = limits?.activeBoxes;
    if (atOnce !== undefined && active !== undefined && active >= atOnce) return { ...mine, running: active, atOnce };
    const minute = limits?.starts?.minute;
    const used = minute === undefined ? 0 : minute.used + [...starting.values()].filter(s => s.placeId === placeId && !s.posted).length;
    if (minute !== undefined && used >= minute.limit) return { ...mine, running: used, atOnce: minute.limit, why: "minute" };
    return undefined;
  };

  /** Whether a spawn about to start on a cloud has to wait: no room there, or spawns already waiting ahead of it, whose
   * wait it says, so a start limit is never called a slot (Q18). */
  const queueWait = (placeId: string, limits: BoxLimits | undefined): ThreadCapWait | undefined => {
    const wait = waitOn(placeId, limits);
    if (wait !== undefined) return wait;
    const ahead = queued().find(e => ctx.placeIdOf(e.record) === placeId && lastWait.has(e.record.id));
    return ahead === undefined ? undefined : lastWait.get(ahead.record.id);
  };

  /** A spawn let past the count takes its slot and its start in the same step, so one asked beside it reads them; the
   * create's answer gives the start back to the cloud's read, and the release drops both once its record is live or
   * it failed. */
  const claimStart: SubQueueArea["claimStart"] = (workspaceId, placeId) => {
    starting.set(workspaceId, { placeId, posted: false });
    return {
      posted: () => {
        const s = starting.get(workspaceId);
        if (s !== undefined) s.posted = true;
      },
      release: () => void starting.delete(workspaceId),
    };
  };

  /** The cloud refused a create at its machines at once, which another client took between the read and the create:
   * the count its plan holds full. */
  const cloudFull = (placeId: string, limits: BoxLimits | undefined): ThreadCapWait => {
    const atOnce = limits?.machinesAtOnce;
    return atOnce === undefined ? slots(placeId) : { ...slots(placeId), running: atOnce, atOnce };
  };

  /** The record of a spawn, stored queued with no machine until its thread's start comes, and past that for the wait
   * it met where it met one: it counts against its root's machines from here. One the cloud refused at its cap goes
   * back at the head of the line, not the tail, and waits a read (Q9). */
  const enqueue = async (record: WorkspaceRecord, wait: ThreadCapWait | undefined, refused = false): Promise<void> => {
    const line = queued().map(e => Date.parse(e.record.queue!.since));
    // Two spawns in one millisecond still keep the order they came in.
    const since = new Date(refused && line.length > 0 ? line[0]! - 1 : Math.max(clock.now(), (line.at(-1) ?? 0) + 1)).toISOString();
    if (refused) refusedUntil.set(record.id, clock.now() + QUEUE_SLOT_READ_MS);
    queuedAt.set(record.id, clock.now());
    record.phase = "queued";
    record.queue = { since, why: wait?.why ?? "slot" };
    record.machineId = "";
    if (wait !== undefined) lastWait.set(record.id, wait);
    attachQueued(record, wait);
    await ctx.persist(record);
    bus.emit({ type: "workspace.created", workspace: ctx.view(record) });
    void pump();
  };

  const attachQueued = (record: WorkspaceRecord, wait: ThreadCapWait | undefined): void => {
    const said = wait === undefined ? `${record.name} is waiting for a machine` : capWaitLine(wait);
    ctx.attach(record, absentMachine("", "sandbox", () => {
      throw new Error(said);
    }));
  };

  /** The waiting row of a held spawn's thread, Waiting in the sidebar, and its wait said into the thread. */
  const showRow = (h: Held, wait: ThreadCapWait | undefined, gone: Promise<void>): void => {
    const r = h.record;
    const view: SessionView = {
      id: r.turnId,
      workspaceId: r.workspaceId,
      harness: r.opts.harness ?? "",
      status: "running",
      startedBy: r.opts.startedBy ?? "person",
      threadId: r.threadId,
      ...r.tree,
      prompt: keptOpening(r.opts.prompt),
      startedAt: Date.parse(r.at),
      ...(r.opts.title !== undefined ? { harnessTitle: r.opts.title, titleSource: "person" as const } : {}),
      ...(wait !== undefined ? { capped: wait } : {}),
    };
    const row: SessionEntry = { view, turnId: r.turnId, launch: gone, queued: true, ...(r.notify !== undefined ? { notify: r.notify } : {}), ...(r.by !== undefined ? { notifyBy: r.by } : {}), ...(r.road !== undefined ? { notifyRoad: r.road } : {}) };
    sessions.set(r.turnId, row);
    bus.emit({ type: "session.held", workspaceId: r.workspaceId, threadId: r.threadId });
    if (wait !== undefined) ctx.record({ type: "session.capped", workspaceId: r.workspaceId, sessionId: r.turnId, turnId: r.turnId, threadId: r.threadId, ...wait });
  };

  const holdOf = (record: SubStartRecord, caller?: Pick<Held, "started" | "failed" | "tell">): Held => {
    let left!: () => void;
    const gone = new Promise<void>(r => (left = r));
    const h: Held = { record, name: live.get(record.workspaceId)?.record.name ?? record.workspaceId, left, gone, ...caller };
    held.set(record.workspaceId, h);
    showRow(h, lastWait.get(record.workspaceId), gone);
    return h;
  };

  /** The first turn of a queued sub-machine's thread: written down, its row shown as Waiting, answered held where the
   * caller asked to hear it, and started by the queue once a machine stands. A second start before then is refused. */
  const holdQueued: SubQueueArea["holdQueued"] = (at, origin) => {
    const entry = live.get(at.workspaceId)!;
    if (held.has(at.workspaceId)) throw subWaitingRefusal(entry.record.name);
    const opts: Record<string, unknown> = {};
    for (const key of KEPT) if (at.o[key] !== undefined) opts[key] = at.o[key];
    const scope = scopeOf(origin);
    const road = roadOf(origin);
    const record: SubStartRecord = {
      workspaceId: at.workspaceId,
      threadId: at.threadId,
      turnId: randomUUID(),
      at: new Date(clock.now()).toISOString(),
      opts: opts as SubStartRecord["opts"],
      tree: at.tree,
      ...(at.notify !== undefined ? { notify: at.notify } : {}),
      ...(scope !== undefined ? { by: scope } : {}),
      ...(road !== undefined ? { road } : {}),
    };
    const onHeld = at.o.onHeld;
    return new Promise<SessionHandle>((started, failed) => {
      const tell = (wait: ThreadCapWait): void => {
        delete h.tell;
        onHeld?.({ view: sessions.get(record.turnId)!.view, turnId: record.turnId, wait });
      };
      const h = holdOf(record, { started, failed, tell });
      void store.put(SUB_STARTS, at.workspaceId, record).then(
        () => {
          h.written = true;
          const wait = lastWait.get(at.workspaceId);
          if (wait !== undefined) h.tell?.(wait);
          void pump();
        },
        (e: unknown) => void drop(h, `the held start was not written down: ${e instanceof Error ? e.message : String(e)}`, true),
      );
    });
  };

  const forget = (workspaceId: string): void => {
    held.delete(workspaceId);
    lastWait.delete(workspaceId);
    refusedUntil.delete(workspaceId);
    queuedAt.delete(workspaceId);
  };

  /** The thread's targets hear a spawn that failed as its turn's end, since no turn of it will tell them. */
  const tellFailed = (r: SubStartRecord, view: SessionView, reason: string): void => {
    if (r.notify !== undefined) ctx.notifyEnd({ view, turnId: r.turnId }, r.notify, ctx.tellAs({ ...(r.by !== undefined ? { notifyBy: r.by } : {}), ...(r.road !== undefined ? { notifyRoad: r.road } : {}) }), { status: "failed", error: reason });
  };

  /** A held spawn goes with no start spent: its row, its written start and its record, and the caller still waiting
   * is told why. A spawn that failed tells the thread's targets too, since nobody else will. */
  const drop = async (h: Held, reason: string, failedSpawn: boolean): Promise<void> => {
    const r = h.record;
    if (held.get(r.workspaceId) !== h) return;
    forget(r.workspaceId);
    const row = sessions.get(r.turnId);
    if (failedSpawn && row !== undefined) tellFailed(r, row.view, reason);
    sessions.delete(r.turnId);
    h.left();
    h.failed?.(new Error(reason));
    await store.delete(SUB_STARTS, r.workspaceId);
    await ctx.drop(r.workspaceId);
  };

  /** A first start whose machine failed stays where the person reads it: the record released under its lead, as a
   * failed start of a released sub-thread is, and the row failed with why beside the start's steps, out before the
   * caller hears the refusal. A send into it, the line's Try again, starts a machine again. */
  const failFirst = async (h: Held, record: WorkspaceRecord, e: unknown): Promise<void> => {
    const r = h.record;
    const why = failureWords(e);
    forget(r.workspaceId);
    record.phase = "released";
    record.machineId = "";
    // The message the start was for: its line's Try again sends it, and any other send opens with it.
    if (r.opts.prompt !== undefined) {
      record.sub!.opening = r.opts.prompt;
      if (record.sub!.spawn !== undefined) record.sub!.spawn.asked = r.opts.prompt;
    }
    ctx.attach(record, deadMachine(""));
    await ctx.persist(record);
    const row = sessions.get(r.turnId);
    if (row !== undefined) {
      Object.assign(row.view, { status: "failed", failure: listedFailure(why), endedAt: clock.now() });
      delete row.view.capped;
      delete row.launch;
      tellFailed(r, row.view, why);
      await ctx.persistSessions(r.workspaceId);
    }
    ctx.pushHead(r.threadId);
    await ctx.rowsSent();
    h.left();
    await store.delete(SUB_STARTS, r.workspaceId);
    h.failed?.(e instanceof Error ? e : new Error(why));
  };

  /** A stop on a held spawn's row drops it, or once its machine is being made, drops that machine as it lands; false
   * for any other row. */
  const dropQueued: SubQueueArea["dropQueued"] = async turnId => {
    const h = [...held.values()].find(x => x.record.turnId === turnId);
    if (h === undefined) return false;
    await stop(h, SUB_DROPPED_LINE);
    return true;
  };

  const stop = async (h: Held, reason: string): Promise<void> => {
    if (live.get(h.record.workspaceId)?.record.queue === undefined) h.stopped = reason;
    else await drop(h, reason, false);
  };

  /** A delete of a held spawn's record stops it as a stop does, and answers once it is gone, its machine with it where
   * one was being made; nothing for a record the queue holds no start of. */
  const deleteQueued: SubQueueArea["deleteQueued"] = workspaceId => {
    const h = held.get(workspaceId);
    if (h === undefined) return undefined;
    return stop(h, deletedBeforeStartLine(h.name)).then(() => h.gone);
  };

  /** The head of the queue, let through where its cloud has room: its machine made, then its thread started as asked.
   * Only the head moves, so spawns start in the order they came; one let through counts at once, so the next looks
   * again straight after it. */
  let pumping: Promise<void> | undefined;
  let again = false;
  const pump = (): Promise<void> => {
    if (pumping !== undefined) {
      again = true;
      return pumping;
    }
    pumping = (async () => {
      try {
        do {
          again = false;
          while (await pumpOnce());
        } while (again);
      } finally {
        pumping = undefined;
      }
    })().catch((e: unknown) => console.warn(`the queue of held spawns did not move: ${failureWords(e)}`));
    return pumping;
  };

  let timer: (() => void) | undefined;
  const arm = (ms: number): void => {
    timer?.();
    timer = clock.schedule(() => {
      timer = undefined;
      void pump();
    }, ms, { unref: true });
  };

  const pumpOnce = async (): Promise<boolean> => {
    // A record whose first turn has not reached the host yet is passed over, since the line that made it sends it
    // straight after; one whose start never came within the bound goes, as at boot.
    const line = queued();
    const startless = line.filter(e => !held.has(e.record.id));
    const due = (e: LiveWorkspace): number => (queuedAt.get(e.record.id) ?? Date.parse(e.record.queue!.since)) + QUEUE_START_WAIT_MS;
    for (const e of startless.filter(x => clock.now() >= due(x))) {
      console.warn(`workspace ${e.record.id}: its first turn never came, so its queued spawn is dropped`);
      forget(e.record.id);
      await ctx.drop(e.record.id);
      return true;
    }
    const head = line.find(e => held.has(e.record.id));
    if (head === undefined) {
      timer?.();
      timer = undefined;
      if (startless.length > 0) arm(Math.min(...startless.map(due)) - clock.now());
      return false;
    }
    const h = held.get(head.record.id)!;
    const until = refusedUntil.get(head.record.id);
    if (until !== undefined && clock.now() < until) {
      arm(until - clock.now());
      return false;
    }
    refusedUntil.delete(head.record.id);
    const placeId = ctx.placeIdOf(head.record)!;
    const at = ctx.backendFor(head.record);
    const limits = at.limits === undefined ? undefined : await at.limits().catch(() => undefined);
    if (live.get(head.record.id) !== head || held.get(head.record.id) !== h) return true;
    const wait = waitOn(placeId, limits);
    if (wait !== undefined) {
      // Every held row on the place says the head's count, so no two figures of one moment disagree (U5).
      for (const e of line.filter(x => ctx.placeIdOf(x.record) === placeId)) {
        const x = held.get(e.record.id);
        const said = lastWait.get(e.record.id);
        if (x === undefined || (said !== undefined && said.running === wait.running && said.atOnce === wait.atOnce && said.why === wait.why)) continue;
        lastWait.set(e.record.id, wait);
        if (x.written === true) x.tell?.(wait);
        const row = sessions.get(x.record.turnId);
        if (row !== undefined) row.view.capped = wait;
        if (e.record.queue!.why !== wait.why) {
          e.record.queue = { ...e.record.queue!, why: wait.why ?? "slot" };
          await ctx.persist(e.record);
        }
        ctx.record({ type: "session.capped", workspaceId: e.record.id, sessionId: x.record.turnId, turnId: x.record.turnId, threadId: x.record.threadId, ...wait });
      }
      arm(wait.why === "minute" ? QUEUE_MINUTE_READ_MS : QUEUE_SLOT_READ_MS);
      return false;
    }
    void letThrough(head, h);
    return true;
  };

  /** One held spawn out of the queue: no longer waiting, holding its slot while its machine is made, and its thread
   * started on it under the caller it came from. */
  const letThrough = async (entry: LiveWorkspace, h: Held): Promise<void> => {
    const record = entry.record;
    const waited = lastWait.has(record.id);
    delete record.queue;
    lastWait.delete(record.id);
    const claim = claimStart(record.id, ctx.placeIdOf(record)!);
    // Its row waits no more: the machine is being made.
    const waiting = sessions.get(h.record.turnId);
    if (waiting !== undefined) {
      delete waiting.view.capped;
      bus.emit({ type: "session.held", workspaceId: record.id, threadId: h.record.threadId });
    }
    let handle: SessionHandle;
    try {
      await ctx.persist(record);
      await ctx.standUpQueued(record, claim.posted, waited);
      claim.release();
      if (held.get(record.id) !== h) return;
      if (h.stopped !== undefined) {
        const standing = live.get(record.id);
        if (standing !== undefined) {
          await cutOff(ctx, standing);
          await ctx.unfork(standing).catch((e: unknown) => console.warn(`workspace ${record.id}: its machine was not deleted after the stop: ${failureWords(e)}`));
        }
        await drop(h, h.stopped, false);
        return;
      }
      forget(record.id);
      sessions.delete(h.record.turnId);
      h.left();
      await store.delete(SUB_STARTS, record.id);
      const r = h.record;
      const origin: Caller | undefined = r.by !== undefined ? { origin: r.road ?? "here", by: r.by } : r.road;
      handle = await ctx.sessionsApi.start(record.id, { ...r.opts, ...(r.notify !== undefined ? { notify: r.notify } : {}), as: { threadId: r.threadId, turnId: r.turnId } } as StartOptions, origin);
    } catch (e) {
      claim.release();
      if (h.stopped !== undefined && held.get(record.id) === h) {
        await drop(h, h.stopped, false);
        return;
      }
      if (isCapRefusal(e) && held.get(record.id) === h && h.stopped === undefined) {
        const at = ctx.backendFor(record);
        const wait = cloudFull(ctx.placeIdOf(record)!, at.limits === undefined ? undefined : await at.limits().catch(() => undefined));
        await enqueue(record, wait, true);
        const row = sessions.get(h.record.turnId);
        if (row !== undefined) row.view.capped = wait;
        return;
      }
      const why = failureWords(e);
      console.warn(`workspace ${record.id}: the held spawn did not start: ${why}`);
      if (held.get(record.id) === h) await failFirst(h, record, e);
      else {
        // Its machine stands and its thread did not open on it: the lead hears it, and the idle nap takes the machine.
        if (waiting !== undefined) tellFailed(h.record, waiting.view, why);
        h.failed?.(e instanceof Error ? e : new Error(why));
      }
      return;
    } finally {
      void pump();
    }
    h.started?.(handle);
  };

  /** At boot: each queued record's written start shown as Waiting again with the count it waits on and put back in
   * line in its order; one the old host was making a machine for goes back too, and its create's key replays the same
   * box. A record whose start never reached the host, or a start whose record is gone, goes. */
  const resumeQueue: SubQueueArea["resumeQueue"] = async () => {
    const kept = new Map(((await store.list(SUB_STARTS)) as SubStartRecord[]).map(r => [r.workspaceId, r]));
    for (const id of kept.keys()) if (live.get(id)?.record.phase !== "queued") await store.delete(SUB_STARTS, id);
    for (const entry of [...live.values()].filter(e => e.record.phase === "queued")) {
      const r = kept.get(entry.record.id);
      if (r === undefined) {
        await ctx.drop(entry.record.id);
        continue;
      }
      if (entry.record.queue === undefined) {
        entry.record.queue = { since: r.at, why: "slot" };
        await ctx.persist(entry.record);
      }
      const placeId = ctx.placeIdOf(entry.record)!;
      const at = ctx.backendFor(entry.record);
      const limits = at.limits === undefined ? undefined : await at.limits().catch(() => undefined);
      lastWait.set(entry.record.id, waitOn(placeId, limits) ?? slots(placeId));
      holdOf(r);
    }
    void pump();
  };

  // A machine stopping, going or a setting moving may free a slot; a lead's workspace going drops what it was owed,
  // and a held spawn's own record deleted takes its start with it.
  bus.on("*", e => {
    if (e.type === "workspace.deleted") queuedAt.delete(e.workspaceId);
    if (held.size === 0) return;
    if (e.type === "workspace.deleted") {
      const own = held.get(e.workspaceId);
      if (own !== undefined) {
        forget(e.workspaceId);
        own.left();
        own.failed?.(new Error(deletedBeforeStartLine(own.name)));
        void store.delete(SUB_STARTS, e.workspaceId);
      }
      for (const h of [...held.values()]) {
        const lead = live.get(h.record.workspaceId)?.record.lead;
        if (lead !== undefined && lead.workspaceId === e.workspaceId) void drop(h, "its lead's folder was deleted", false);
      }
    }
    if (e.type === "workspace.napped" || e.type === "workspace.deleted" || e.type === "workspace.gone" || e.type === "workspace.released" || e.type === "place.changed") void pump();
  });

  return { queueWait, claimStart, deleteQueued, cloudFull, enqueue, attachQueued, holdQueued, dropQueued, resumeQueue, pumpQueue: pump };
}
