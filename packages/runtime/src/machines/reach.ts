// SPDX-License-Identifier: AGPL-3.0-only
import { remoteHost } from "@wsp/catalog";
import { INLINE_EXEC_MS, DISK_USE_CMD, diskUseOf, sweepLeftovers } from "@wsp/engine";
import { type DaemonEvent, type ReachState, GitStatusReply, GitBranchCompareReply, DETACHED_HEAD, type TreeChild, type TreeFact, type Checkout, napRefusedLine, WAKE_STOPPED, DISK_FULL_PCT, DISK_REARM_PCT, diskFullLine, diskMarkLine, providerKeyName, stopRefusedLine, type DiskMove } from "@wsp/protocol";
import { connectDaemon, type DaemonReach } from "../reach.js";
import { machineStateOf } from "../status.js";
import { CHECKOUT_TTL_MS } from "../types/events.js";
import { type LiveWorkspace, until } from "../types/wiring.js";
import type { SessionEntry } from "../types/internal.js";
import type { RuntimeContext, ReachArea } from "../context.js";

/** How often a running machine whose stop snapshots its disk has it read between turn ends: one long turn's build can
 * fill the last tenth of a disk, and a read is one command at the provider, which moves no stop timer of Boat's ("your
 * own jobs do not move the deadline", docs.boat.dev, auto-stop). */
export const DISK_READ_MS = 2 * 60_000;

/** How long a host closing waits on a disk read still out: a read that crossed the mark holds the cache sweep for up to
 * its five minute guard, and the next host reads the disk again anyway. */
export const DISK_CLOSE_WAIT_MS = 2_000;

export function reachArea(ctx: RuntimeContext): ReachArea {
  const { bus, clock, daemonHelloTimeoutMs, live, sessions } = ctx;
  /** A status pushed outside the poll, for a phase change the poller would show
   * late. Machine state is what the phase implies: asking the provider here
   * would reset its idle timer for a fact the runtime already knows. The nap
   * countdown rides along as the poller sends it: a client replaces the whole
   * status, so leaving it out would blank the row until the next poll. */
  /** The reach the poll last measured for a workspace's running machine; a machine that is not running, or one
   * replaced since, leaves nothing here. Held because a status pushed between polls has to say something about the
   * reach and the runtime has no probe of its own. */
  const polledReach = new Map<string, { machineId: string; reach: ReachState }>();

  /** A pause the provider refused outright stands for that machine until one lands: Solari refuses every pause past its size cap (measured 2026-09-23). */
  const napRefusals = new Map<string, { machineId: string; said: string }>();
  const napRefusedOf = (entry: LiveWorkspace): string | undefined => {
    const refused = napRefusals.get(entry.record.id);
    if (refused === undefined) return undefined;
    if (refused.machineId === entry.machine.id) return refused.said;
    napRefusals.delete(entry.record.id);
    return undefined;
  };
  const napRefusedReason = (entry: LiveWorkspace): string | undefined => {
    const said = napRefusedOf(entry);
    return said === undefined ? undefined : napRefusedLine(said);
  };
  /** A stop the provider would not take while the snapshot of the disk fails, by workspace, held like a refused pause:
   * on the row from the first one, for the machine it was said of, until a pause lands. */
  const stopRefusals = new Map<string, { machineId: string; said: string }>();
  const stopRefusedReason = (entry: LiveWorkspace): string | undefined => {
    const refused = stopRefusals.get(entry.record.id);
    return refused === undefined || refused.machineId !== entry.machine.id ? undefined : stopRefusedLine(refused.said);
  };
  /** How full the disk of a machine whose stop snapshots it last read, by workspace and machine, and whether its last
   * crossing was said and not yet taken back by a fall to DISK_REARM_PCT. */
  const disks = new Map<string, { machineId: string; pct: number; told: boolean }>();
  const diskReason = (entry: LiveWorkspace): string | undefined => {
    const read = disks.get(entry.record.id);
    return read === undefined || read.machineId !== entry.machine.id || !(read.pct > DISK_FULL_PCT) ? undefined : diskFullLine(read.pct);
  };
  const watchesDisk = (entry: LiveWorkspace): boolean => ctx.pauseKeepsDisk(ctx.backendFor(entry.record)) && entry.record.phase === "running";
  /** One reading of the disk where the stop snapshots it, onto the row: a row crossing the line either way is pushed at
   * once. Answers the reading and whether this one crossed the line with no crossing said since the last fall. */
  const diskRead = async (entry: LiveWorkspace): Promise<{ use: NonNullable<ReturnType<typeof diskUseOf>>; crossed: boolean } | undefined> => {
    const res = await entry.machine.exec(DISK_USE_CMD, { timeoutMs: INLINE_EXEC_MS }).catch(() => undefined);
    const use = res?.exitCode === 0 ? diskUseOf(res.stdout) : undefined;
    if (use === undefined || !watchesDisk(entry)) return undefined;
    const was = diskReason(entry);
    const held = disks.get(entry.record.id);
    const told = held !== undefined && held.machineId === entry.machine.id && held.told && use.pct > DISK_REARM_PCT;
    const crossed = !told && use.pct > DISK_FULL_PCT;
    disks.set(entry.record.id, { machineId: entry.machine.id, pct: use.pct, told: told || crossed });
    if (diskReason(entry) !== was) await emitStatus(entry, reachOf(entry), ctx.rowReason(entry));
    return { use, crossed };
  };
  /** The disk read at a turn's end and on the watch's beat. Past the line, once per crossing: what wsp made there is
   * cleared, the disk read again, and every thread on the machine told how full it is, what was cleared and what the
   * person can do. */
  const readDisk = (entry: LiveWorkspace): Promise<void> => {
    if (!watchesDisk(entry)) return Promise.resolve();
    armDiskWatch();
    const id = entry.record.id;
    const held = diskReads.get(id);
    if (held !== undefined) return held;
    const read = readDiskNow(entry).finally(() => diskReads.delete(id));
    diskReads.set(id, read);
    return read;
  };
  const readDiskNow = async (entry: LiveWorkspace): Promise<void> => {
    const first = await diskRead(entry);
    if (first?.crossed !== true || ctx.state.closing) return;
    await sweepLeftovers(entry.machine).catch((e: unknown) => console.warn(`what wsp made on ${entry.record.name} was not cleared: ${e instanceof Error ? e.message : String(e)}`));
    const after = (await diskRead(entry))?.use ?? first.use;
    const to = await moveFor(entry, after.usedBytes + after.freeBytes);
    const words = {
      machine: entry.record.name,
      on: providerKeyName(ctx.providerOf(entry.record) ?? "the provider"),
      pct: after.pct,
      usedBytes: after.usedBytes,
      sizeBytes: after.usedBytes + after.freeBytes,
      freedBytes: Math.max(0, first.use.usedBytes - after.usedBytes),
      machineGoes: entry.record.sub !== undefined,
    };
    console.warn(`${entry.record.name}: ${diskMarkLine(words)}`);
    // Each thread's line names that thread, which is what wsp resize takes.
    tellThreads(entry.record.id, thread => diskMarkLine({ ...words, ...(to !== undefined ? { move: { ...to, thread } } : {}) }));
  };
  const tellThreads = (workspaceId: string, line: (thread: string) => string): void => {
    for (const s of lastTurns(workspaceId)) {
      const thread = s.view.threadId ?? s.view.id;
      ctx.record({ type: "session.notice", workspaceId, sessionId: s.view.claudeSessionId ?? s.turnId, turnId: s.turnId, threadId: thread, text: line(thread) });
    }
  };
  /** The first size up the plan sells whose measured root disk grows the one this machine reads, which is not always
   * its size's: Boat makes a small box on an older 12 GB disk when it runs short. df counts 6% under the disk's own
   * size (70.6 of 75.2 GB on a small box, 2026-10-12), so a size grows it only past a tenth more. */
  const moveFor = async (entry: LiveWorkspace, readBytes: number): Promise<Omit<DiskMove, "thread"> | undefined> => {
    const at = ctx.backendFor(entry.record);
    if (at.rootDiskOf === undefined) return undefined;
    await at.limits?.().catch(() => undefined);
    const up = at.capabilities.sizes.filter(s => s.memMb > entry.record.size.memMb).sort((a, b) => a.memMb - b.memMb);
    for (const s of up) {
      const disk = at.rootDiskOf(s);
      if (disk !== undefined && disk.diskGb * 1e9 > readBytes * 1.1) return { name: disk.name, cpu: s.cpu, memMb: s.memMb, diskGb: disk.diskGb };
    }
    return undefined;
  };
  /** The latest turn of each thread on a workspace, which a line about its machine is written under. */
  const lastTurns = (workspaceId: string): SessionEntry[] => {
    const latest = new Map<string, SessionEntry>();
    for (const s of sessions.values()) {
      if (s.view.workspaceId !== workspaceId) continue;
      const key = s.view.threadId ?? s.view.id;
      const held = latest.get(key);
      if (held === undefined || (s.view.startedAt ?? 0) >= (held.view.startedAt ?? 0)) latest.set(key, s);
    }
    return [...latest.values()];
  };
  /** The read in flight per workspace, which a turn's end and the beat share: one machine's minutes of clearing never
   * hold back the beat of another. */
  const diskReads = new Map<string, Promise<void>>();
  /** Armed only while a running machine whose stop snapshots its disk stands. */
  let diskTimer: (() => void) | undefined;
  const armDiskWatch = (): void => {
    if (diskTimer !== undefined || ctx.state.closing || ![...live.values()].some(watchesDisk)) return;
    diskTimer = clock.schedule(
      () => {
        diskTimer = undefined;
        for (const entry of live.values()) void readDisk(entry).catch((e: unknown) => console.warn(`disk of ${entry.record.id} not read on the watch: ${e instanceof Error ? e.message : String(e)}`));
        armDiskWatch();
      },
      DISK_READ_MS,
      { unref: true },
    );
  };
  const stopDiskWatch = async (): Promise<void> => {
    diskTimer?.();
    diskTimer = undefined;
    await Promise.race([Promise.allSettled([...diskReads.values()]), new Promise(resolve => setTimeout(resolve, DISK_CLOSE_WAIT_MS).unref())]);
  };

  /** The reach a status pushed for a running machine carries: what the poll last measured, and where it has
   * measured nothing, the claim this kind's road makes. A measurement outranks the claim because the pushes that
   * carry a line about the daemon happen exactly when the daemon is dead: claiming reachable there paints the row
   * as answering, and leaves the poll's own no-daemon looking like a repeat of the claim, which the bus drops. */
  const reachOf = (entry: LiveWorkspace): ReachState => {
    if (ctx.unreachedOf(entry) !== undefined) return "unreachable";
    const seen = polledReach.get(entry.record.id);
    if (seen !== undefined && seen.machineId === entry.machine.id) return seen.reach;
    // The same reading the status poll makes: a machine wsp can ask at all, by a route or by its own answer.
    return ctx.moduleOf(entry.record.kind).hasDaemon(entry) || entry.machine.daemonAnswers !== undefined ? "reachable" : "unsupported";
  };
  const emitStatus = async (entry: LiveWorkspace, reach: ReachState, reason?: string): Promise<void> => {
    const size = entry.record.size;
    const idleAt = entry.record.phase === "running" ? ctx.idle.idleAt(entry.record.id) : undefined;
    bus.emit({
      type: "workspace.status",
      status: {
        ...ctx.view(entry.record),
        machineState: machineStateOf(entry.record.phase),
        reach: { state: reach },
        size,
        rateUsdPerHour: ctx.backendFor(entry.record).pricing.rateUsdPerHour(size),
        ...(reason !== undefined ? { reason } : {}),
        ...(entry.wakeAsk !== undefined ? { wakeAsk: entry.wakeAsk } : {}),
        ...(idleAt !== undefined ? { idleAt } : {}),
        ...(entry.checkout !== undefined ? { checkout: entry.checkout } : {}),
        ...(entry.pr !== undefined ? { pr: entry.pr } : {}),
        ...(entry.tree !== undefined ? { tree: entry.tree } : {}),
      },
    });
  };

  /** One line about the wake in flight, pushed now and kept on the entry so the poll's own statuses carry it too. */
  const saysWaking = async (entry: LiveWorkspace, words: string): Promise<void> => {
    entry.wakeSaid = words;
    await emitStatus(entry, "napping", words);
  };

  /** The one preamble every dial this runtime makes to a machine's daemon repeats: the preview route, then this
   * runtime's token on the guest, then the link. null when the guest holds no daemon token, which each caller reads
   * its own way. The caller owns the link and closes it; the previewUrl guard stays with the caller, which knows
   * what a backend without preview routes means for it. */
  const dialDaemon = async (entry: LiveWorkspace, deadline: number, o: { onEvent?: (e: DaemonEvent) => void; heartbeatMs?: number } = {}): Promise<DaemonReach | null> => {
    const reach = await until(entry.ws.daemonReach(), deadline, "preview route");
    const token = await until(ctx.daemonTokenOf(entry.machine), deadline, "daemon token");
    if (token === undefined) return null;
    return connectDaemon({ previewUrl: reach.url, token, onEvent: o.onEvent ?? (() => {}), ...(o.heartbeatMs !== undefined ? { heartbeatMs: o.heartbeatMs } : {}) });
  };

  /** How long one ask of a machine's own daemon check gets, and how long before the next one: the budget is the
   * whole of what the daemon is given, and a boot that is still coming up answers no rather than nothing. */
  const ASK_DAEMON_MS = 5_000;
  const ASK_AGAIN_MS = 500;

  /** The daemon answering is what proves a resumed guest serves; resume() returning does not (a zombie reports
   * running for 10+ minutes while exec and the edge 502). Asked over the machine's own road where it has one and
   * through the edge where the route is the only way in, since the two readings of one machine's reach would
   * otherwise disagree: a container's published port is on the loopback of the box that runs it, and a host that
   * is not that computer would fail this check on a live guest, which on a backend whose wake takes one attempt
   * throws the container away and forks the golden again. A machine with neither road has nothing to ask, so the
   * check falls back to the shape comparison. */
  const daemonAnswer = async (entry: LiveWorkspace, quit?: AbortSignal): Promise<string | undefined> => {
    const machine = entry.machine;
    const answersMs = ctx.lifecycleOf(entry).budgets.daemonAnswersMs;
    // The person's stop on a wake ends the wait here too: the budget runs to minutes, and the row's toggle waits for
    // the wake to let go. So does `quit`, the caller's own reason to stop waiting.
    const stopped = entry.wakeStop?.signal;
    const stop = stopped === undefined ? quit : quit === undefined ? stopped : AbortSignal.any([stopped, quit]);
    const orStopped = <T>(p: Promise<T>): Promise<T> =>
      stop === undefined
        ? p
        : Promise.race([p, new Promise<never>((_, reject) => (stop.aborted ? reject(new Error(WAKE_STOPPED)) : stop.addEventListener("abort", () => reject(new Error(WAKE_STOPPED)), { once: true })))]);
    if (machine.daemonAnswers !== undefined) {
      const deadline = clock.now() + answersMs;
      try {
        // Asked again until the budget is out rather than once at the start of it: a machine that was stopped for
        // its nap rather than frozen comes back with its boot still running, and the budget is what the daemon is
        // given to answer in. A machine that answers at once costs one ask, as it always did.
        for (;;) {
          const up = await orStopped(until(machine.daemonAnswers({ timeoutMs: Math.min(answersMs, ASK_DAEMON_MS) }), deadline, "daemon answer"));
          if (up) return undefined;
          if (clock.now() >= deadline) return `nothing listens on the daemon's port inside ${machine.id}`;
          await orStopped(new Promise<void>(done => clock.schedule(done, ASK_AGAIN_MS, { unref: true })));
        }
      } catch (e) {
        // The error is in hand here, so it is what the row says: only the edge road, which learns nothing but that
        // it waited, reports the budget.
        return `the daemon on ${machine.id} could not be asked (${e instanceof Error ? e.message : String(e)})`;
      }
    }
    if (!machine.previewUrl) return undefined;
    const deadline = clock.now() + answersMs;
    let link: DaemonReach | null = null;
    try {
      const dialled = dialDaemon(entry, deadline, { heartbeatMs: answersMs });
      // A dial the stop walked away from still lets go of its link once it lands.
      dialled.then(l => (stop?.aborted === true ? l?.close() : undefined), () => {});
      link = await orStopped(dialled);
      if (link === null) {
        // No daemon to ask; an exec that returns is the guest's own answer.
        await orStopped(until(machine.exec("true"), deadline, "guest exec", clock));
        return undefined;
      }
      await orStopped(until(link.ready, deadline, "daemon link", clock));
      await orStopped(until(link.request("ping"), deadline, "daemon ping", clock));
      return undefined;
    } catch (e) {
      return `daemon on ${machine.id} did not answer within ${answersMs} ms (${e instanceof Error ? e.message : String(e)})`;
    } finally {
      link?.close();
    }
  };

  /** How long a wait on a daemon runs before a machine that can start its own daemon is asked to, for a provider that
   * can leave it down after a restore however long the wait, and how long after a start that failed it is asked
   * again: a command on a box whose disk is still streaming in can outlast its own timeout. Each start is one more
   * try inside the same budget, which stays the outer cut. */
  const START_DAEMON_AFTER_MS = 60_000;

  const startingDaemon = async (entry: LiveWorkspace, quit: AbortSignal): Promise<string | undefined> => {
    const machine = entry.machine;
    if (machine.startDaemon === undefined) return daemonAnswer(entry, quit);
    const start = machine.startDaemon.bind(machine);
    const began = clock.now();
    let over = false;
    let cancel = (): void => {};
    const arm = (): void => {
      cancel = clock.schedule(
        () => {
          const late = `daemon on ${machine.id} (workspace ${entry.record.id}) had not answered ${Math.round((clock.now() - began) / 1000)} s into the wait, so wsp started it`;
          const again = (): void => (over ? undefined : arm());
          void start().then(
            r => {
              const tail = r.exitCode === 0 ? "" : r.stderr.trim().slice(-200);
              console.warn(`${late} (exit ${r.exitCode}${tail === "" ? "" : `: ${tail}`})`);
              if (r.exitCode !== 0) again();
            },
            (e: unknown) => {
              console.warn(`${late}, and the start failed (${e instanceof Error ? e.message : String(e)})`);
              again();
            },
          );
        },
        START_DAEMON_AFTER_MS,
        { unref: true },
      );
    };
    arm();
    try {
      return await daemonAnswer(entry, quit);
    } finally {
      over = true;
      cancel();
    }
  };

  /** How often a wait on a daemon asks a machine that can tell whether its daemon keeps exiting as it starts: a look
   * takes five seconds on the machine, and a daemon that cannot start is otherwise waited on for the whole budget. */
  const LOOK_EXITS_EVERY_MS = 10_000;

  const pingDaemon = async (entry: LiveWorkspace): Promise<string | undefined> => {
    const machine = entry.machine;
    const quit = new AbortController();
    let exited: string | undefined;
    let over = false;
    let cancel = (): void => {};
    const look = (exits: () => Promise<string | undefined>): void => {
      cancel = clock.schedule(
        () =>
          void exits().then(
            said => {
              if (over) return;
              if (said === undefined) return look(exits);
              exited = `the daemon on ${machine.id} exits as it starts (${said})`;
              quit.abort();
            },
            // A look the machine could not answer, as a box still streaming its disk may not, is asked again.
            () => (over ? undefined : look(exits)),
          ),
        LOOK_EXITS_EVERY_MS,
        { unref: true },
      );
    };
    if (machine.daemonExits !== undefined) look(machine.daemonExits.bind(machine));
    try {
      const fault = await startingDaemon(entry, quit.signal);
      return fault === undefined ? undefined : (exited ?? fault);
    } finally {
      over = true;
      cancel();
    }
  };

  /** The version the machine's daemon announces in its hello, null when no daemon answers within the bound: a
   * daemon says what it is on connect and answers no op for it, so reading the version is one dial and one frame.
   * A machine whose daemon is gone, whose backend mints no preview route or whose guest holds no token has none. */
  const helloVersion = async (entry: LiveWorkspace): Promise<number | null> => {
    if (!entry.machine.previewUrl) return null;
    const deadline = Date.now() + daemonHelloTimeoutMs;
    let link: DaemonReach | null = null;
    try {
      let announce: (v: number) => void = () => {};
      const hello = new Promise<number>(done => (announce = done));
      link = await dialDaemon(entry, deadline, { onEvent: e => (e.type === "daemon.hello" ? announce(e.version) : undefined) });
      if (link === null) return null;
      return await until(hello, deadline, "daemon hello");
    } catch {
      return null;
    } finally {
      link?.close();
    }
  };

  /** The machine's row now, rather than at the next poll. A machine that stopped running is left to the poller:
   * only it knows what that machine's reach is by then. */
  const pushStatus = async (entry: LiveWorkspace): Promise<void> => {
    if (entry.record.phase !== "running") return;
    await emitStatus(entry, reachOf(entry));
  };

  /** The copy's checkout read through its own daemon, kept on the entry and pushed on its status: at a turn's end, on
   * view, after a write, and forced every few seconds by a composer about to open a thread; the host keeps no timer
   * of its own. Within CHECKOUT_TTL_MS the fact held answers unless the caller forces a read. A machine that is not
   * running is asked nothing unless its computer answers for it, which reads a stopped copy off its files; one that
   * cannot answer keeps its last fact and the time git gave it. */
  const readCheckout = (entry: LiveWorkspace, force: boolean): Promise<Checkout | undefined> => {
    const held = entry.checkout;
    if (!force && held !== undefined && clock.now() - held.readAt < CHECKOUT_TTL_MS) return Promise.resolve(held);
    if (entry.record.phase !== "running" && ctx.servedByItsComputer(entry) === undefined) return Promise.resolve(held);
    if (entry.checkoutReading !== undefined) return entry.checkoutReading;
    const reading = (async (): Promise<Checkout | undefined> => {
      try {
        const said = GitStatusReply.parse(await ctx.withDaemon(entry, ask => ask({ op: "git.status", cwd: ctx.checkoutOf(entry.record) })));
        entry.checkout = {
          branch: said.branch.head,
          ahead: said.branch.ahead,
          behind: said.branch.behind,
          changed: said.entries.filter(e => e.xy !== "!!").length,
          ...(said.editsUnread === true ? { editsUnread: true } : {}),
          ...(said.countsUnknown === true ? { countsUnknown: true } : {}),
          ...(said.stashes !== undefined ? { stashes: said.stashes } : {}),
          ...(/^[0-9a-f]{7,40}$/.test(said.branch.oid) ? { head: said.branch.oid } : {}),
          readAt: clock.now(),
        };
        const { readAt: _was, ...kept } = entry.record.checkout ?? { readAt: 0 };
        const { readAt: _now, ...read } = entry.checkout;
        if (JSON.stringify(kept) !== JSON.stringify(read)) {
          entry.record.checkout = entry.checkout;
          await ctx.persist(entry.record);
        }
        await statusNow(entry);
      } catch {
        // A copy git could not read keeps the last fact it gave; the time on it says how old it is.
      }
      return entry.checkout;
    })().finally(() => {
      delete entry.checkoutReading;
    });
    entry.checkoutReading = reading;
    return reading;
  };

  /** A lead's children read against the lead's branch as the git host holds it, kept on the lead and pushed on its
   * status: each child's branch off its own checkout fact, one compare on this computer's command line with the
   * repository off the child's project, and what the child's record keeps. Read at a child's turn end, after its bring
   * back, after a merge into the lead and when the lead's thread opens; never on a timer. A lead whose children are all
   * gone has its tree taken off. */
  const readTree = (lead: LiveWorkspace): Promise<TreeFact | undefined> => {
    if (lead.treeReading !== undefined) return lead.treeReading;
    const reading = (async (): Promise<TreeFact | undefined> => {
      const children = [...live.values()].filter(e => e.record.parentWorkspaceId === lead.record.id && e.creating !== true);
      if (children.length === 0) {
        if (lead.tree !== undefined) {
          delete lead.tree;
          await statusNow(lead);
        }
        return undefined;
      }
      const project = ctx.projectHeld(lead.record.project);
      const leadBranch = (await readCheckout(lead, false))?.branch ?? lead.record.base ?? project.base ?? project.defaultBranch;
      const rows = await Promise.all(children.map(child => treeChildOf(child, leadBranch)));
      lead.tree = { leadBranch, children: rows, readAt: clock.now() };
      await statusNow(lead);
      return lead.tree;
    })().finally(() => {
      delete lead.treeReading;
    });
    lead.treeReading = reading;
    return reading;
  };

  /** One child's row: its branch, the host's count of it against what the lead's copy holds of it where this computer
   * could ask, and what its record keeps. A merge into the lead stays in the lead's copy until its bring back, so
   * after one the count is against the child's commit it took. A compare nothing could answer leaves the counts out,
   * which the row reads as not counted. */
  const treeChildOf = async (child: LiveWorkspace, leadBranch: string): Promise<TreeChild> => {
    const branch = (await readCheckout(child, false))?.branch ?? child.record.worktree?.branch ?? child.record.base ?? "";
    const thread = [...sessions.values()].map(v => v.view).find(v => v.workspaceId === child.record.id && v.threadId !== undefined)?.threadId;
    const remote = ctx.projectHeld(child.record.project).remote;
    const asked =
      branch === "" || branch === DETACHED_HEAD || remoteHost(remote) === undefined
        ? undefined
        : await ctx.onThisComputer((ask, home) => ask({ op: "git.branchCompare", cwd: home, remote, base: child.record.tree?.merged?.head ?? leadBranch, head: branch }))
            .then(r => GitBranchCompareReply.parse(r))
            .catch(() => undefined);
    const counted = asked === undefined ? {} : asked.pushed ? { pushed: true, ...(asked.aheadBy !== undefined ? { aheadOfLead: asked.aheadBy } : {}), ...(asked.behindBy !== undefined ? { behindLead: asked.behindBy } : {}) } : { pushed: false };
    return { workspaceId: child.record.id, ...(thread !== undefined ? { threadId: thread } : {}), branch, ...counted, ...(child.record.tree ?? {}) };
  };

  /** The tree a workspace is a child in, read again where something about the child moved. */
  const readLeadOf = (child: LiveWorkspace): void => {
    const lead = child.record.parentWorkspaceId === undefined ? undefined : live.get(child.record.parentWorkspaceId);
    if (lead !== undefined) void readTree(lead);
  };

  /** A workspace's status pushed now, with the reach its phase implies, for a fact read outside the poll. */
  const statusNow = (entry: LiveWorkspace): Promise<void> => emitStatus(entry, entry.record.phase === "running" ? reachOf(entry) : "napping");
  return {
    polledReach, napRefusals, napRefusedOf, napRefusedReason, stopRefusals, stopRefusedReason, diskReason, readDisk,
    reachOf, emitStatus, saysWaking, pingDaemon, helloVersion, pushStatus, readCheckout, readTree, readLeadOf,
    statusNow, armDiskWatch, stopDiskWatch, tellThreads,
  };
}
