// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's machine from its done to its delete. At each done the host
// reads off the machine what was pushed, keeps the agent's session file, arms
// the delete or holds the machine over work not pushed, and naps it. At the
// delete the box goes at the cloud and the record stays, released, with its
// thread, transcript, result and session. The timer is the record's deleteAt,
// or for a held machine the day its cloud deletes the paused disk, so a
// restarted host arms it again.
import { INLINE_EXEC_MS, failureWords, isMissing } from "@wsp/engine";
import { DELETE_AFTER_MS, THREAD_WORKING_LINE, cloudSaid, fmtBytes, foldThreads, scopeOf, shellQuote, subAllPushed, subCloudGoneRefusal, subDeleteQuestion, subDeleteRefusedLine, subHeldDeleteRefusal, subReleasedRefusal, subKeepRanOutLine, subResultLine, isGitPush, pushRefusedOf, subSessionNotKeptLine, subSessionTooLargeLine, subSpawning, type SessionEvent, type SubResult, type SubRow, type SubState, type SubUnpushed } from "@wsp/protocol";
import { GONE_REASON, deadMachine } from "../types/internal.js";
import type { GoneOutcome, LiveWorkspace, SubSession, WorkspaceRecord } from "../types/wiring.js";
import type { RuntimeContext, SubLifeArea } from "../context.js";
import { cutOff } from "../pipe.js";

/** The host blobs a sub-thread's kept session goes under, by thread id, gzipped. */
export const SUB_SESSIONS = "sub-sessions";
/** A session past this packed is not kept: a send then goes on from the transcript. */
export const SUB_SESSION_MAX_BYTES = 64 * 1024 * 1024;
/** Boat's own stop timer on a done machine whose nap did not land: this long past its delete, so a host that died with
 * the delete armed leaves a stopped box rather than a running one. */
export const SUB_STOP_AFTER_DELETE_MS = 10 * 60_000;
/** How long a delete the cloud refused waits before it is asked again. */
export const SUB_DELETE_RETRY_MS = 10 * 60_000;
/** The longest a lead's line waits on the done read after its sub-thread's turn; past it the line goes without. */
export const SUB_LINE_WAIT_MS = 5 * 60_000;
/** How far back the done looks for the agent's last push: the newest events of the thread, and their bytes. */
const PUSH_READ_EVENTS = 400;
const PUSH_READ_BYTES = 2_000_000;
/** The row's reason for the nap a done takes. */
const DONE_NAP = "its sub-thread is done";

/** One command in the sub-thread's folder, off local refs alone, so it needs no network and no credential: where HEAD
 * is, the sha the remote held for the result branch at the last push or fetch, the commits neither a remote-tracking
 * ref nor the commit it started from holds, the files changed or untracked, and how many commits the pushed branch
 * adds to the lead's. */
export function subResultCommand(folder: string, branch: string, baseSha: string): string {
  return [
    `cd ${shellQuote(folder)} 2>/dev/null && git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo norepo; exit 0; }`,
    `echo "branch $(git rev-parse --abbrev-ref HEAD 2>/dev/null)"`,
    `p=$(git rev-parse --verify -q ${shellQuote(`refs/remotes/origin/${branch}`)} || echo none); echo "pushed $p"`,
    // A clone that fetched its start by id holds no remote-tracking ref for it; a new machine on a pushed branch lacks it.
    `b=$(git rev-parse --verify -q ${shellQuote(`${baseSha}^{commit}`)}); echo "unpushed $(git rev-list --count --branches --not --remotes $b)"`,
    `echo "changed $(git status --porcelain=v1 | wc -l | tr -d ' ')"`,
    `[ "$p" = none ] || echo "commits $(git rev-list --count ${shellQuote(baseSha)}..$p 2>/dev/null || echo 0)"`,
  ].join("\n");
}

/** The result command's answer read into the record; nothing where it carries no counts. */
export function subResultOf(stdout: string, branch: string, readAt: string): SubResult | undefined {
  const said = new Map<string, string>();
  for (const line of stdout.split("\n").map(l => l.trim())) {
    if (line === "") continue;
    const at = line.indexOf(" ");
    said.set(at < 0 ? line : line.slice(0, at), at < 0 ? "" : line.slice(at + 1).trim());
  }
  if (said.has("norepo")) return { branch, pushedSha: null, commits: 0, unpushed: 0, changed: 0, norepo: true, readAt };
  const count = (key: string): number | undefined => {
    const n = Number(said.get(key));
    return said.get(key) !== "" && Number.isInteger(n) && n >= 0 ? n : undefined;
  };
  const unpushed = count("unpushed");
  const changed = count("changed");
  if (unpushed === undefined || changed === undefined) return undefined;
  const pushed = said.get("pushed");
  const onBranch = said.get("branch");
  return {
    branch,
    pushedSha: pushed === undefined || pushed === "" || pushed === "none" ? null : pushed,
    commits: count("commits") ?? 0,
    unpushed,
    changed,
    ...(onBranch !== undefined && onBranch !== "" ? { onBranch } : {}),
    readAt,
  };
}

/** One command that finds a session's file under its folder and packs it where the cloud's file read reaches, then
 * says where it found it and its packed size; `nofile` where the agent wrote none. */
export function subSessionPackCommand(file: { folder: string; name: string; depth: number }, to: string): string {
  return [
    `f=$(find ${shellQuote(file.folder)} -maxdepth ${file.depth + 1} -type f -name ${shellQuote(`*${file.name}`)} 2>/dev/null | head -n 1)`,
    `[ -n "$f" ] || { echo nofile; exit 0; }`,
    `echo "path $f"`,
    `gzip -c "$f" > ${shellQuote(to)} && chmod 0644 ${shellQuote(to)} && echo "size $(stat -c %s ${shellQuote(to)})"`,
  ].join("\n");
}

/** One command that lands a kept session at the path it was kept from, unpacked from where the cloud's file write put
 * it, after taking away any other file of the same name under the agent's folder, so a resume reads the one kept. */
export function subSessionLandCommand(file: { folder: string; name: string; depth: number }, path: string, from: string): string {
  const dir = path.slice(0, path.lastIndexOf("/"));
  return [
    `install -d -m 0700 ${shellQuote(dir)}`,
    `find ${shellQuote(file.folder)} -maxdepth ${file.depth + 1} -type f -name ${shellQuote(`*${file.name}`)} ! -path ${shellQuote(path)} -delete 2>/dev/null`,
    `gunzip -c ${shellQuote(from)} > ${shellQuote(path)}; s=$?; rm -f ${shellQuote(from)}; exit $s`,
  ].join("\n");
}

/** A sub-machine's record as its thread's row reads it. */
export function subRowOf(record: WorkspaceRecord): SubRow | undefined {
  const sub = record.sub;
  if (sub === undefined) return undefined;
  const state: SubState =
    record.phase === "released" || record.phase === "gone"
      ? "released"
      : record.phase === "queued"
        ? record.queue !== undefined
          ? "waiting"
          : "starting"
        : record.phase === "running"
          ? subSpawning(sub.spawn)
            ? "starting"
            : "running"
          : "napping";
  return {
    state,
    on: sub.on,
    machine: record.name,
    size: { cpu: record.size.cpu, memMb: record.size.memMb, ...(sub.sizeName !== undefined ? { name: sub.sizeName } : {}) },
    ...(sub.rateUsdPerHour !== undefined ? { rateUsdPerHour: sub.rateUsdPerHour } : {}),
    branch: sub.branch,
    ...(sub.result !== undefined ? { result: sub.result } : {}),
    ...(sub.held !== undefined ? { held: { files: sub.held.files, commits: sub.held.commits } } : {}),
    ...(sub.deleteAt !== undefined ? { deleteAt: sub.deleteAt } : {}),
    ...(sub.releasedAt !== undefined ? { releasedAt: sub.releasedAt } : {}),
    ...(sub.cloudGone === true ? { cloudGone: true as const } : {}),
    ...(sessionNotKept(sub.session) !== undefined ? { sessionNotKept: sessionNotKept(sub.session)! } : {}),
    ...(sub.spawn !== undefined ? { spawn: sub.spawn } : {}),
  };
}

/** What the row says of a session the host holds no copy of; nothing where one is kept or none was asked for yet. */
function sessionNotKept(session: SubSession | undefined): string | undefined {
  if (session === undefined || session.kept !== null) return undefined;
  if (session.bytes !== undefined && session.bytes > SUB_SESSION_MAX_BYTES) return subSessionTooLargeLine(fmtBytes(session.bytes));
  return subSessionNotKeptLine(session.why ?? "no reason was given");
}

export function subLifeArea(ctx: RuntimeContext): SubLifeArea {
  const { live, store, bus, clock, sessions, threadRecords } = ctx;
  const timers = new Map<string, () => void>();
  /** The done or delete in flight per record: the next waits for it, so a done and a delete never cross. */
  const working = new Map<string, Promise<void>>();
  /** The lines waiting on a done's read, per record, each with when it became owed: a done that started then or later
   * answers it, one already under way when the line came does not. */
  const readWaits = new Map<string, Set<{ since: number; go: () => void }>>();
  const readDone = (id: string, startedAt: number): void => {
    for (const w of readWaits.get(id) ?? []) if (w.since <= startedAt) w.go();
  };
  /** When the last done to finish started, per record: a wait on a turn that ended before it has had its done. */
  const doneStarted = new Map<string, number>();

  const serial = (id: string, work: () => Promise<void>): Promise<void> => {
    const next = (working.get(id) ?? Promise.resolve()).then(work, work);
    const kept = next.catch((e: unknown) => console.warn(`sub-thread machine ${id}: ${e instanceof Error ? e.message : String(e)}`));
    working.set(id, kept);
    void kept.finally(() => {
      if (working.get(id) === kept) working.delete(id);
    });
    return kept;
  };

  /** The one thread a sub-machine runs. */
  const threadOf = (workspaceId: string): string | undefined => {
    for (const [threadId, held] of threadRecords) if (held.workspaceId === workspaceId) return threadId;
    for (const s of sessions.values()) if (s.view.workspaceId === workspaceId && s.view.threadId !== undefined) return s.view.threadId;
    return undefined;
  };

  /** A turn running, queued or held on the thread: each holds its machine. */
  const busy = (threadId: string): boolean => ctx.threadRuns(threadId) || ctx.launchingOn(threadId) !== undefined;

  const cloudOf = (entry: LiveWorkspace): string => cloudSaid(entry.record.sub!.on);

  /** How long a done machine stays, read at each done off the cloud's setting, so a change applies to the next done. */
  const delayMs = (entry: LiveWorkspace): number => ctx.settingsAt(ctx.placeIdOf(entry.record))?.deleteAfterMs ?? DELETE_AFTER_MS;

  const readResult = async (entry: LiveWorkspace): Promise<SubResult> => {
    const sub = entry.record.sub!;
    const readAt = new Date(clock.now()).toISOString();
    const failed = (why: string): SubResult => ({ branch: sub.branch, pushedSha: sub.result?.pushedSha ?? null, commits: sub.result?.commits ?? 0, unpushed: 0, changed: 0, readFailed: why, readAt });
    if (entry.record.phase !== "running") return failed(`its machine was ${entry.record.phase}`);
    try {
      const said = await entry.machine.exec(subResultCommand(sub.folder, sub.branch, sub.base.sha), { timeoutMs: INLINE_EXEC_MS });
      if (said.exitCode !== 0) return failed(`exit ${said.exitCode}: ${(said.stderr.trim() || said.stdout.trim()).slice(-200)}`);
      const read = subResultOf(said.stdout, sub.branch, readAt);
      if (read === undefined) return failed(`it answered no counts: ${said.stdout.trim().slice(-200)}`);
      // A machine made after the delete holds no repository until its agent fetches one; the branch pushed before stands.
      if (read.norepo !== true || sub.result?.pushedSha == null) return read;
      const { norepo: _none, ...answered } = read;
      return { ...answered, pushedSha: sub.result.pushedSha, commits: sub.result.commits };
    } catch (e) {
      return failed(e instanceof Error ? e.message : String(e));
    }
  };

  /** Why the thread's newest git push was refused, off that push's own output in the transcript; nothing where it went
   * through, none is held, or no push failed. */
  const pushRefusedIn = async (workspaceId: string, threadId: string): Promise<string | undefined> => {
    const { events } = await ctx.transcriptReader.read(workspaceId, threadId, { limit: PUSH_READ_EVENTS, bytes: PUSH_READ_BYTES });
    const results = new Map<string, Extract<SessionEvent, { type: "session.delta" }>>();
    for (const e of events) if (e.type === "session.delta" && e.kind === "tool_result" && e.toolUseId !== undefined) results.set(e.toolUseId, e);
    for (let i = events.length - 1; i >= 0; i--) {
      const e = events[i]!;
      if (e.type !== "session.delta" || e.kind !== "tool_use" || e.toolUseId === undefined || !isGitPush(e.text)) continue;
      const out = results.get(e.toolUseId);
      const failed = out !== undefined && (out.isError === true || (out.exitCode ?? 0) !== 0);
      return failed ? pushRefusedOf(out.text) : undefined;
    }
    return undefined;
  };

  /** The agent's one session file, packed on the machine and kept on the host, so a machine lost later still leaves a
   * session to resume from. A keep that fails keeps none, and a send goes on from the transcript. */
  const keepSession = async (entry: LiveWorkspace, threadId: string): Promise<void> => {
    const sub = entry.record.sub!;
    const row = ctx.latestOn(threadId);
    const id = row?.claudeSessionId;
    if (id === undefined || row === undefined || entry.record.phase !== "running") return;
    const file = ctx.adapterFor(entry, row.harness).adapter.sessionFile?.(id);
    const read = entry.machine.readBytes?.bind(entry.machine);
    if (file === undefined || read === undefined) return;
    const at = `/tmp/wsp-session-${threadId}.gz`;
    try {
      const packed = await entry.machine.exec(subSessionPackCommand(file, at), { timeoutMs: INLINE_EXEC_MS });
      if (packed.exitCode === 0 && packed.stdout.includes("nofile")) {
        await store.deleteBlob(SUB_SESSIONS, threadId);
        sub.session = { agent: row.harness, id, kept: null, why: `no session file for ${id} under ${file.folder}` };
        return;
      }
      const size = Number(/^size (\d+)$/m.exec(packed.stdout)?.[1]);
      const path = /^path (.+)$/m.exec(packed.stdout)?.[1];
      if (packed.exitCode !== 0 || !Number.isInteger(size) || path === undefined) throw new Error(`the pack exited ${packed.exitCode}: ${(packed.stderr.trim() || packed.stdout.trim()).slice(-200)}`);
      if (size > SUB_SESSION_MAX_BYTES) {
        await store.deleteBlob(SUB_SESSIONS, threadId);
        sub.session = { agent: row.harness, id, kept: null, bytes: size, why: `the agent's session was too large to keep (${fmtBytes(size)})` };
        return;
      }
      await store.putBlob(SUB_SESSIONS, threadId, await read(at, { size }));
      sub.session = { agent: row.harness, id, kept: clock.now(), bytes: size, path };
    } catch (e) {
      const why = e instanceof Error ? e.message : String(e);
      console.warn(`the session of sub-thread ${threadId} was not kept: ${why}`);
      // A copy from an earlier done lacks the turns since, so a send goes on from the transcript instead (04).
      await store.deleteBlob(SUB_SESSIONS, threadId).catch(() => undefined);
      sub.session = { agent: row.harness, id, kept: null, why };
    } finally {
      await entry.machine.exec(`rm -f ${shellQuote(at)}`).catch(() => undefined);
    }
  };

  /** How long the record's cloud keeps a paused machine's disk; undefined where it keeps it as long as the machine. */
  const diskKeptMs = (entry: LiveWorkspace): number | undefined => {
    try {
      return ctx.backendFor(entry.record).lifecycle?.diskKeptMs;
    } catch {
      return undefined;
    }
  };

  /** When the cloud deletes a held machine's disk: the keep from its nap, or from the hold where `orHeld` asks and the
   * nap has not landed yet, as at the lead's line, which goes before the nap. */
  const keepEnds = (entry: LiveWorkspace, orHeld = false): number | undefined => {
    const held = entry.record.sub?.held;
    const ms = diskKeptMs(entry);
    if (held === undefined || ms === undefined) return undefined;
    if (entry.record.phase === "napping" && entry.record.nappedAt !== undefined) return entry.record.nappedAt + ms;
    return orHeld ? held.at + ms : undefined;
  };

  /** The timer on a sub-thread's record: the delete at deleteAt, or the release of a held one at the end of its keep. */
  const arm = (entry: LiveWorkspace, at = entry.record.sub?.deleteAt ?? keepEnds(entry)): void => {
    const id = entry.record.id;
    timers.get(id)?.();
    timers.delete(id);
    if (at === undefined || entry.record.phase === "released") return;
    timers.set(
      id,
      clock.schedule(
        () => {
          timers.delete(id);
          void serial(id, () => (live.get(id)?.record.sub?.deleteAt !== undefined ? expire(id) : lapse(id)));
        },
        Math.max(0, at - clock.now()),
        { unref: true },
      ),
    );
  };

  /** The record follows its box out: released, its machine gone, everything else kept. */
  const release = async (entry: LiveWorkspace, o: { lost?: string; cloudGone?: true } = {}): Promise<void> => {
    const id = entry.record.id;
    timers.get(id)?.();
    timers.delete(id);
    const record = entry.record;
    const sub = record.sub!;
    if (record.phase === "released") {
      if (o.cloudGone === true && sub.cloudGone !== true) {
        sub.cloudGone = true;
        await ctx.persist(record);
      }
      return;
    }
    ctx.endSessions(id, GONE_REASON);
    record.phase = "released";
    record.machineId = "";
    delete record.gone;
    sub.releasedAt = clock.now();
    delete sub.deleteAt;
    delete sub.held;
    if (o.lost !== undefined) sub.lost = o.lost;
    if (o.cloudGone === true) sub.cloudGone = true;
    entry.lateRead?.();
    ctx.napRefusals.delete(id);
    ctx.stopRefusals.delete(id);
    ctx.unreached.delete(id);
    const now = ctx.attach(record, deadMachine(""));
    await ctx.persist(record);
    ctx.idle.forget(id);
    bus.emit({ type: "workspace.released", workspaceId: id });
    await ctx.emitStatus(now, "gone");
  };

  const done = async (id: string): Promise<void> => {
    const startedAt = clock.now();
    try {
      await doneSteps(id);
    } finally {
      doneStarted.set(id, startedAt);
      readDone(id, startedAt);
    }
  };

  const doneSteps = async (id: string): Promise<void> => {
    const entry = live.get(id);
    if (entry?.record.sub === undefined || entry.record.phase === "released") return;
    const threadId = threadOf(id);
    if (threadId === undefined || busy(threadId)) return;
    const sub = entry.record.sub;
    // An end with no turn run since the last read, on a machine no longer running (a start a restart failed), leaves
    // that read's word standing: a read of the stopped machine would hold work it already found pushed.
    if (entry.record.phase !== "running" && sub.result !== undefined && (ctx.latestOn(threadId)?.startedAt ?? 0) <= Date.parse(sub.result.readAt)) return;
    const read = await readResult(entry);
    const refused = subAllPushed(read) ? undefined : await pushRefusedIn(id, threadId).catch(() => undefined);
    const result = refused === undefined ? read : { ...read, pushRefused: refused };
    await keepSession(entry, threadId);
    // A release re-attaches the record, so an entry no longer the live one was released meanwhile.
    if (busy(threadId) || live.get(id) !== entry) return;
    sub.result = result;
    if (subAllPushed(result)) {
      sub.deleteAt = clock.now() + delayMs(entry);
      delete sub.held;
    } else {
      sub.held = { files: result.changed, commits: result.unpushed, at: clock.now() };
      delete sub.deleteAt;
    }
    await ctx.persist(entry.record);
    // No event of the done carries the row, so every window is told it here: its tile reads Not pushed or Done.
    ctx.pushHead(threadId);
    arm(entry);
    // The lead's line goes on the read, not after the nap: a stop at the cloud takes minutes.
    readDone(id, clock.now());
    if (entry.record.phase !== "running") return;
    try {
      await ctx.napWith(id, DONE_NAP);
    } catch (e) {
      const until = sub.deleteAt;
      if (until === undefined || live.get(id) !== entry || entry.record.phase !== "running") return;
      console.warn(`sub-thread machine ${entry.machine.id} of ${id} did not nap (${e instanceof Error ? e.message : String(e)}); it runs until its delete`);
      await ctx
        .backendFor(entry.record)
        .lifecycle?.backstop?.(entry.machine, until + SUB_STOP_AFTER_DELETE_MS, { exact: true })
        .catch((x: unknown) => console.warn(`the stop timer of ${entry.machine.id} was not moved: ${x instanceof Error ? x.message : String(x)}`));
    }
  };

  /** The delete at deleteAt. A turn that started since cancels it; a machine still running is read again, and work it
   * now holds turns the delete into a hold; a napped machine's read stands, since nothing ran on it since. */
  const expire = async (id: string): Promise<void> => {
    const entry = live.get(id);
    const sub = entry?.record.sub;
    if (entry === undefined || sub?.deleteAt === undefined || entry.record.phase === "released") return;
    const threadId = threadOf(id);
    if (threadId !== undefined && busy(threadId)) {
      delete sub.deleteAt;
      await ctx.persist(entry.record);
      return;
    }
    // A nap or a wake in flight settles first: a machine a wake brought back runs, and is read again before it goes.
    await entry.napping?.catch(() => undefined);
    await entry.waking?.catch(() => undefined);
    if (entry.record.phase === "running") {
      const again = await readResult(entry);
      sub.result = again;
      if (!subAllPushed(again)) {
        sub.held = { files: again.changed, commits: again.unpushed, at: clock.now() };
        delete sub.deleteAt;
        await ctx.persist(entry.record);
        return;
      }
      if (threadId !== undefined) await keepSession(entry, threadId);
    }
    // A send after the wake can start a turn while the machine is read, and that turn takes it back.
    if (sub.deleteAt === undefined || (threadId !== undefined && busy(threadId))) return;
    await cutOff(ctx, entry);
    try {
      await ctx.unfork(entry);
    } catch (e) {
      if (!isMissing(e)) {
        const said = failureWords(e);
        console.warn(subDeleteRefusedLine(cloudOf(entry), entry.record.name, said, SUB_DELETE_RETRY_MS / 60_000));
        entry.deleteSaid = { phase: entry.record.phase, line: subDeleteRefusedLine(cloudOf(entry), entry.record.name, said, SUB_DELETE_RETRY_MS / 60_000) };
        sub.deleteAt = clock.now() + SUB_DELETE_RETRY_MS;
        await ctx.persist(entry.record);
        arm(entry);
        return;
      }
    }
    delete entry.deleteSaid;
    await release(entry);
  };

  /** The end of a held machine's keep at its cloud, which deletes the disk then: the box is deleted with it and the record
   * released, saying so on its row and its timeline. A turn since takes the machine back, and its nap counts anew. */
  const lapse = async (id: string): Promise<void> => {
    const entry = live.get(id);
    const sub = entry?.record.sub;
    const at = entry === undefined ? undefined : keepEnds(entry);
    const ms = entry === undefined ? undefined : diskKeptMs(entry);
    if (entry === undefined || sub?.held === undefined || at === undefined || ms === undefined || entry.record.phase === "released") return;
    const threadId = threadOf(id);
    if (threadId !== undefined && busy(threadId)) return;
    if (at > clock.now()) return arm(entry, at);
    const line = subKeepRanOutLine({ on: cloudOf(entry), keptMs: ms, at, held: { files: sub.held.files, commits: sub.held.commits, ...(sub.result?.readFailed !== undefined ? { readFailed: sub.result.readFailed } : {}) }, branch: sub.branch });
    await cutOff(ctx, entry);
    try {
      await ctx.unfork(entry);
    } catch (e) {
      if (!isMissing(e)) {
        console.warn(subDeleteRefusedLine(cloudOf(entry), entry.record.name, failureWords(e), SUB_DELETE_RETRY_MS / 60_000));
        return arm(entry, clock.now() + SUB_DELETE_RETRY_MS);
      }
    }
    entry.record.diskGone = line;
    await release(entry, { lost: line });
    ctx.tellThreads(id, () => line);
  };

  /** A turn starting on the thread takes back its machine: no delete, no hold. */
  const reclaim = (workspaceId: string): void => {
    const entry = live.get(workspaceId);
    const sub = entry?.record.sub;
    if (entry === undefined || sub === undefined || (sub.deleteAt === undefined && sub.held === undefined)) return;
    timers.get(workspaceId)?.();
    timers.delete(workspaceId);
    delete sub.deleteAt;
    delete sub.held;
    // A delete the provider refused left the machine cut off, and the turn runs on it.
    ctx.opts.broker?.lift?.(workspaceId);
    void ctx.persist(entry.record).catch((e: unknown) => console.warn(`sub-thread machine ${workspaceId}: ${e instanceof Error ? e.message : String(e)}`));
  };

  bus.on("session.end", e => {
    if (e.type !== "session.end" || live.get(e.workspaceId)?.record.sub === undefined) return;
    // After the turn's own settling: a send queued behind it takes the thread in the same tick, and is a turn.
    setImmediate(() => void serial(e.workspaceId, () => done(e.workspaceId)));
  });
  for (const type of ["session.start", "session.held", "session.queued"] as const) {
    bus.on(type, e => reclaim((e as { workspaceId: string }).workspaceId));
  }
  // A held machine's keep runs from its nap, which lands after the done that held it.
  bus.on("workspace.napped", e => {
    const entry = e.type === "workspace.napped" ? live.get(e.workspaceId) : undefined;
    if (entry?.record.sub?.held !== undefined && entry.record.sub.deleteAt === undefined) arm(entry);
  });
  bus.on("workspace.deleted", e => {
    if (e.type !== "workspace.deleted") return;
    timers.get(e.workspaceId)?.();
    timers.delete(e.workspaceId);
    doneStarted.delete(e.workspaceId);
  });

  /** What a sub-machine may hold that no remote has: the counts its last read found, that the read failed, or that a
   * turn ran since it, whose work nothing has read yet; nothing for a released box or a thread that never ran. */
  const subUnpushed = (entry: LiveWorkspace): SubUnpushed | undefined => {
    const sub = entry.record.sub;
    // A queued spawn has run nothing, so nothing of it waits to be pushed.
    if (sub === undefined || entry.record.phase === "released" || entry.record.phase === "queued") return undefined;
    const threadId = threadOf(entry.record.id);
    if (threadId === undefined) return undefined;
    const ran = ctx.latestOn(threadId)?.startedAt;
    if (sub.result === undefined ? ran !== undefined : ran !== undefined && ran > Date.parse(sub.result.readAt)) return { files: 0, commits: 0, unread: true };
    if (sub.held === undefined) return undefined;
    return { files: sub.held.files, commits: sub.held.commits, ...(sub.result?.readFailed !== undefined ? { readFailed: sub.result.readFailed } : {}) };
  };

  /** The cloud left this host by a forget: nothing there was deleted, so the record keeps its phase, says its cloud
   * is gone, and arms no delete it could not make. */
  const forgotten = async (entry: LiveWorkspace): Promise<void> => {
    const id = entry.record.id;
    timers.get(id)?.();
    timers.delete(id);
    const sub = entry.record.sub!;
    delete sub.deleteAt;
    sub.cloudGone = true;
    entry.lateRead?.();
    ctx.attach(entry.record, deadMachine(entry.record.machineId));
    await ctx.persist(entry.record);
    ctx.idle.forget(id);
  };

  /** What the done after a turn that ended at `since` read off the machine, once it has; nothing where no read
   * followed (a turn took the machine back, it was lost) or none came within SUB_LINE_WAIT_MS. */
  const subRead = async (workspaceId: string, since: number): Promise<SubResult | undefined> => {
    const read = (): SubResult | undefined => {
      const r = live.get(workspaceId)?.record.sub?.result;
      return r !== undefined && Date.parse(r.readAt) >= since ? r : undefined;
    };
    const entry = live.get(workspaceId);
    if (entry?.record.sub === undefined) return undefined;
    if (read() === undefined && entry.record.phase !== "released" && (doneStarted.get(workspaceId) ?? -Infinity) < since) {
      await new Promise<void>(resolve => {
        const waits = readWaits.get(workspaceId) ?? new Set();
        readWaits.set(workspaceId, waits);
        const w = {
          since,
          go: () => {
            cancel();
            waits.delete(w);
            if (waits.size === 0 && readWaits.get(workspaceId) === waits) readWaits.delete(workspaceId);
            resolve();
          },
        };
        const cancel = clock.schedule(() => w.go(), SUB_LINE_WAIT_MS, { unref: true });
        waits.add(w);
      });
    }
    return read();
  };

  /** The line a sub-thread's finished line to its lead ends with, off the read of the done after its turn; nothing
   * where none followed or a turn runs on it again, and the line goes as any thread's. */
  const subLine = async (workspaceId: string, since: number): Promise<string | undefined> => {
    const result = await subRead(workspaceId, since);
    const now = live.get(workspaceId);
    const threadId = threadOf(workspaceId);
    if (now?.record.sub === undefined || result === undefined || threadId === undefined || busy(threadId)) return undefined;
    const sub = now.record.sub;
    const deleteAt = sub.deleteAt;
    const keptUntil = now.record.phase === "released" ? undefined : keepEnds(now, true);
    return subResultLine({
      threadId,
      lead: now.record.lead?.threadId ?? threadId,
      result,
      base: sub.base.branch,
      ...(deleteAt !== undefined && now.record.phase !== "released" ? { deleteInMs: Math.max(0, deleteAt - clock.now()) } : {}),
      ...(keptUntil !== undefined ? { kept: { on: cloudOf(now), until: keptUntil } } : {}),
    });
  };

  return {
    subDone: id => serial(id, () => done(id)),
    subRead,
    subLine,
    subExpire: id => serial(id, () => expire(id)),
    armSubs: () => {
      for (const entry of live.values()) {
        const sub = entry.record.sub;
        if (sub === undefined || entry.record.phase === "released" || sub.cloudGone === true) continue;
        if (sub.deleteAt !== undefined || sub.held !== undefined) arm(entry);
        // A done a restart cut short wrote neither, and no end comes again: it runs now. One with a turn running, or
        // with none run yet, returns at once.
        else void serial(entry.record.id, () => done(entry.record.id));
      }
    },
    subLost: async (entry, reason): Promise<GoneOutcome> => {
      await release(entry, { lost: reason });
      return "settled";
    },
    releaseSubsOn: async (placeId, o = {}) => {
      for (const entry of [...live.values()]) {
        if (entry.record.sub === undefined || ctx.placeIdOf(entry.record) !== placeId) continue;
        if (o.forgot === true && entry.record.phase !== "released") await forgotten(entry);
        else await release(entry, { cloudGone: true });
      }
    },
    subUnpushed,
    subDelete: async (asked, threadId, origin, o) => {
      // A done still reading has the word on what the machine holds, so it is waited for.
      await working.get(asked.record.id);
      const entry = live.get(asked.record.id) ?? asked;
      // Never under a turn, and never by a thread's own token over work no remote holds; the caller's reach was read.
      // A held spawn's row reads running with no turn under it: the delete drops it from the queue.
      if (entry.record.phase !== "queued" && busy(threadId)) throw Object.assign(new Error(THREAD_WORKING_LINE), { kind: "conflict" });
      const held = subUnpushed(entry);
      if (held !== undefined && scopeOf(origin) !== undefined) throw subHeldDeleteRefusal(threadId, held, entry.record.sub!.branch);
      const workspaceId = entry.record.id;
      const on = cloudOf(entry);
      if (o.check === true) {
        const title = foldThreads([...sessions.values()].filter(s => s.view.threadId === threadId).map(s => s.view))[0]?.title ?? entry.record.name;
        return { workspaceId, threads: 1, on, question: subDeleteQuestion({ threadId, title, on, branch: entry.record.sub!.branch, ...(held !== undefined ? { held } : {}) }) };
      }
      await ctx.workspaces.delete(workspaceId);
      return { workspaceId, threads: 1, machine: "deleted" as const, on };
    },
    refuseSubSend: entry => {
      if (entry.record.sub?.cloudGone === true) throw subCloudGoneRefusal(cloudOf(entry));
    },
    refuseSubWake: entry => {
      const sub = entry.record.sub;
      if (sub?.cloudGone === true) throw subCloudGoneRefusal(cloudOf(entry));
      if (sub !== undefined && entry.record.phase === "released") throw subReleasedRefusal(threadOf(entry.record.id) ?? entry.record.name, cloudOf(entry));
    },
    leadOfSub: (workspaceId, origin) => {
      const lead = workspaceId === undefined ? undefined : live.get(workspaceId)?.record.lead;
      return lead !== undefined && scopeOf(origin)?.threadId === lead.threadId;
    },
  };
}
