// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's start on a cloud: every check that can refuse runs before the
// cloud is asked for a start, then one machine forked from the image head, an
// empty folder named after the repository with the picked git-ignored files in
// it, and nothing else. wsp pushes nothing for the lead and clones nothing: the
// preamble hands the agent the git lines. A machine that fails before the first
// turn holds nothing, so it is deleted and replaced once.
import { randomBytes } from "node:crypto";
import { gitHostOf, remoteHost, type GitHost } from "@wsp/catalog";
import { BOX_CLASSES, CreateCancelledError, EnvNotLandedError, GuestUnusableError, INLINE_EXEC_MS, LOCAL_MACHINE_ID, goldenHead, isMissing, type BoxLimits, type MachineBackend } from "@wsp/engine";
import {
  DELETE_AFTER_MS, DETACHED_HEAD, GitStatusReply, SUB_SEED_MAX_BYTES, branchUnreadRefusal, copiesFolder, fmtBytes, kindForComputer, namesPlace, runsInFolder, ownerRepoOf,
  providerKeyName, scopeOf, shellLine, capWaitLine, shellQuote, startingLine, startsUsedLine, boatPlanWords, subAheadRefusal, subAheadUnknownRefusal, subBranchNameOk, subBranchNameRefusal, subBranchOf, subCopyingLine, subDetachedRefusal, subFolderOf,
  subLeadBranchRefusal, subLeadOnMachineRefusal, subNoAgentRefusal, subNoCloudRefusal, subNoImageRefusal, subNoRemoteRefusal, subNoSignInRefusal, subNoThreadRefusal, subNoUpstreamRefusal,
  subNotSentLine, subNothingPickedWhy, subOomLine, subOtherHostRefusal, subPickedElsewhereWhy, subPreamble, subReplacedLine, subSeedTooBigRefusal, subSizeOf, subSizeRefusal,
  subSizeWord, subNextSize, subTwiceRefusal, subUncommittedLine, refusalSaid, SUB_SPAWN_LOG_LINES, type SubSpawn, subCloudGoneRefusal, subResumePreamble, cloudSaid, type Caller, type SeedChoice, type SubFault, type SubOut, type SubSize, type WorkspaceAgents, AGENTS_ON, CREATE_READY,
} from "@wsp/protocol";
import { SEED_CHOICES, CREATES, deadMachine, isAbsentMachine, isCapRefusal } from "../types/internal.js";
import { SUB_SESSIONS, subSessionLandCommand } from "./sub-life.js";
import { folderNamed, type CreateWorkspaceOptions, type CreatedWorkspace, type LiveWorkspace, type StageReport, type WorkspaceRecord } from "../types/wiring.js";
import type { RuntimeContext, SubSpawnArea } from "../context.js";

/** The image every sub-machine forks: the person's setup image, never a project's. */
const SETUP_IMAGE = "default";
/** The words a git host's sign-in goes by on an image's logins: the command, or the label its seal recorded. */
const HOST_WORDS: Readonly<Record<GitHost["id"], string>> = { github: "GitHub", gitlab: "GitLab" };
/** The states a sealed login stands in on a fork. */
const LOGIN_HELD = new Set(["copied", "signed-in"]);

/** Which of the three faults a fresh machine cures a failure is, with the machine it names; nothing for any other. */
function faultOf(e: unknown): SubFault | undefined {
  if (e instanceof GuestUnusableError) return { kind: "unusable", box: e.machineId, why: e.detail };
  if (e instanceof EnvNotLandedError && e.exitCode === 124) return { kind: "env", box: e.machineId };
  if (e instanceof CreateCancelledError) return { kind: "cancelled", box: e.machineId, why: e.why === "" ? "no reason given" : e.why };
  return undefined;
}

/** The picked files a start lands: packed, counted and named, or nothing and why. */
type Seed = { tar?: Buffer; files: number; paths: string[]; bytes: number; missing: string[]; why?: string };

export function subSpawnArea(ctx: RuntimeContext): SubSpawnArea {
  const { live, store, places, placeDoor, bus, clock, threadRecords } = ctx;

  /** The thread's row reads the start again where the record has a thread yet: a first start has none until it stands. */
  const told = (record: WorkspaceRecord): void => {
    for (const [threadId, held] of threadRecords) if (held.workspaceId === record.id) return ctx.pushHead(threadId);
  };
  /** One step of the record's start moved: written on its spawn and told to its row. */
  const step = (record: WorkspaceRecord, move: (spawn: SubSpawn) => void): void => {
    const spawn = record.sub?.spawn;
    if (spawn === undefined) return;
    move(spawn);
    told(record);
  };
  /** A start's report that writes each line on the record's spawn log too, with the cloud's machine and its states. */
  const logged = (record: WorkspaceRecord, report: StageReport): StageReport => (stage, message, said) => {
    report(stage, message, said);
    step(record, spawn => {
      spawn.log = [...spawn.log, message].slice(-SUB_SPAWN_LOG_LINES);
      const box = said?.box;
      if (box === undefined || spawn.machine === undefined) return;
      spawn.machine.id = box.id;
      if (spawn.machine.states.at(-1) !== box.state) spawn.machine.states.push(box.state);
    });
  };
  /** The step a start failed on, with the two halves of why, on its spawn. */
  const spawnFailed = (record: WorkspaceRecord, e: unknown): void =>
    step(record, spawn => {
      const message = e instanceof Error ? e.message : String(e);
      const fix = typeof (e as { fix?: unknown } | undefined)?.fix === "string" ? (e as { fix: string }).fix : undefined;
      spawn.failed = { step: spawn.files !== undefined && spawn.files.ms === undefined ? "files" : "machine", said: refusalSaid(message, fix), ...(fix !== undefined ? { fix } : {}) };
    });

  /** The backend a provider row forks on, or nothing where this host holds no key for it. */
  const providerAt = (id: string): MachineBackend | undefined => places.backend(id) ?? placeDoor?.backendOf(id);

  /** The sizes a cloud sells now: the classes its plan's read names, else every size it offers. */
  const soldBy = (at: MachineBackend, limits: BoxLimits | undefined): SubSize[] =>
    limits !== undefined
      ? BOX_CLASSES.filter(c => limits.sizes.includes(c.type)).map(c => ({ name: c.name, cpu: c.cpu, memMb: c.memMb, rateUsdPerHour: c.rateUsdPerHour }))
      : at.capabilities.sizes.map(s => ({ name: s.name ?? subSizeWord(s), cpu: s.cpu, memMb: s.memMb, rateUsdPerHour: s.rateUsdPerHour ?? at.pricing.rateUsdPerHour(s) }));

  /** A setting a person keeps on the cloud's row beyond the ones every place reads, where one is set. */
  const settingOf = (placeId: string, key: "size"): unknown => {
    const held = placeDoor?.settingsAt(placeId);
    return held !== undefined && key in held ? (held as Record<string, unknown>)[key] : undefined;
  };

  /** S6: the setup image's head on the cloud, holding the sign-in of the remote's git host. */
  const imageAt = async (place: string, on: string, host: GitHost, remote: string) => {
    const head = goldenHead(await ctx.copyOf(place, SETUP_IMAGE));
    if (head === undefined) throw subNoImageRefusal(on);
    const record = await ctx.recordOf(SETUP_IMAGE);
    const label = HOST_WORDS[host.id];
    const logins = head.logins ?? record?.logins ?? [];
    if (!logins.some(l => (l.name === host.cli || l.name.startsWith(`${label} CLI`)) && LOGIN_HELD.has(l.state))) {
      throw subNoSignInRefusal(on, { label, cli: host.cli, login: `${label} CLI login` }, ownerRepoOf(remote)!);
    }
    return { head, record };
  };

  /** S11: the agent on the image, where the image says which it carries. */
  const agentOn = (record: Awaited<ReturnType<RuntimeContext["recordOf"]>>, on: string, harness: string): void => {
    const carried = (record?.recipe?.rows ?? []).filter(r => r.kind === "agent" && r.on).map(r => r.id);
    if (carried.length > 0 && !carried.includes(harness)) throw subNoAgentRefusal(on, harness, carried);
  };

  /** S13: the hour's and the day's starts. */
  const startsLeft = (limits: BoxLimits | undefined, name: string): void => {
    const starts = limits?.starts;
    const spent = (["day", "hour"] as const).find(w => starts !== undefined && starts[w].used >= starts[w].limit);
    if (spent !== undefined) throw new Error(startsUsedLine(spent, starts![spent].used, starts![spent].limit, boatPlanWords(limits!.trial, limits!.planUsd), `sub-thread ${name}`));
  };

  const spawnSub = async (opts: CreateWorkspaceOptions, origin: Caller | undefined): Promise<CreatedWorkspace> => {
    const began = clock.now();
    const ask = opts.sub!;
    const name = ctx.nameGiven(opts.name);
    // S1: a sub-thread is a child, with a lead to notify and to merge into.
    const scope = scopeOf(origin);
    if (scope === undefined) throw subNoThreadRefusal(ask.on);
    // S2: the switch, the depth and the machines under the root, the place taken until the record holds it. Today's
    // lines, in the usage class here: nothing was asked of the cloud.
    const freePlace = ((): (() => void) => {
      try {
        return ctx.spawnGuard("fork", origin);
      } catch (e) {
        throw Object.assign(e instanceof Error ? e : new Error(String(e)), { kind: (e as { kind?: string }).kind ?? "usage" });
      }
    })();
    const notices: string[] = [];
    try {
      // S3: a cloud this host holds a key for.
      const providers = (await ctx.computerRows()).filter(r => r.kind === "provider");
      const row = providers.find(r => namesPlace(r, ask.on));
      const at = row === undefined ? undefined : providerAt(row.id);
      if (row === undefined || at === undefined) throw subNoCloudRefusal(ask.on, providers.filter(r => providerAt(r.id) !== undefined).map(r => providerKeyName(r.name)));
      const on = providerKeyName(row.name);
      const { placeId } = await ctx.landingPlace(row.id);
      const place = placeId ?? row.id;
      const lead = live.get(scope.workspaceId)!;
      // A lead is a thread on this computer or on a computer the person added (01, M4).
      if (!runsInFolder(lead.record.kind)) throw subLeadOnMachineRefusal(on, scope.threadId, lead.record.name);
      const project = ctx.projectHeld(lead.record.project);
      // S4: a remote on a host whose command the image signs in to.
      if (project.remote === "") throw subNoRemoteRefusal(project.name, on);
      const host = gitHostOf(project.remote);
      if (host === undefined) throw subOtherHostRefusal(project.name, remoteHost(project.remote) ?? project.remote, on);
      const repo = host.httpsUrl(ownerRepoOf(project.remote)!);
      // S5: the lead's branch, pushed, with nothing ahead of the remote, read through the lead's own daemon.
      await ctx.copyBlocked(lead);
      const said = await ctx.withDaemon(lead, send => send({ op: "git.status", cwd: ctx.checkoutOf(lead.record) })).catch((e: unknown) => {
        throw new Error(branchUnreadRefusal(lead.record.name, e instanceof Error ? e.message : String(e)));
      });
      const read = GitStatusReply.parse(said);
      const branch = read.branch.head;
      if (branch === "" || branch === DETACHED_HEAD) throw subDetachedRefusal(project.name);
      if (read.branch.upstream === undefined) throw subNoUpstreamRefusal(branch);
      if (read.branch.ahead > 0) throw subAheadRefusal(branch, read.branch.ahead);
      if (read.countsUnknown === true) throw subAheadUnknownRefusal(branch);
      const base = { branch, sha: read.branch.oid };
      const uncommitted = read.entries.filter(e => e.xy !== "!!").length;
      // S6: the image on the cloud, holding the sign-in of the remote's host.
      const { head, record } = await imageAt(place, on, host, project.remote);
      // S7: a size the plan sells, read off the cloud now.
      const limits = at.limits === undefined ? undefined : await at.limits();
      const sold = soldBy(at, limits);
      const kept = settingOf(place, "size") as { cpu: number; memMb: number } | undefined;
      const word = ask.size ?? subSizeWord(kept ?? at.pricing.defaultSize);
      const asked = subSizeOf(word);
      const size = asked === undefined ? undefined : sold.find(s => s.cpu === asked.cpu && s.memMb === asked.memMb);
      if (size === undefined) throw subSizeRefusal(on, word, sold);
      // S8: a result branch git takes that is not the lead's own; the default numbered past one taken.
      if (ask.branch !== undefined && !subBranchNameOk(ask.branch)) throw subBranchNameRefusal(ask.branch);
      if (ask.branch === branch) throw subLeadBranchRefusal(ask.branch);
      const result = ask.branch ?? (await freeBranch(subBranchOf(name), lead));
      // S10: the cloud's spend today.
      await ctx.placeGuard(place);
      // S11: the agent on the image, where the image says which it carries.
      const harness = ask.harness ?? ctx.defaultAgentOf(ctx.state.preferencesHeld ?? (await ctx.preferences.get()), lead);
      agentOn(record, on, harness);
      // S12: the picked files, packed fresh from the lead's folder on this computer.
      const seed = await seedOf(project);
      // S13: the hour's and the day's starts.
      startsLeft(limits, name);

      const nameTaken = ctx.nameRefusal(name);
      if (nameTaken !== undefined) throw Object.assign(new Error(nameTaken), { kind: "conflict" });
      const folder = subFolderOf(project.remote);
      const deleteAfterMinutes = Math.round((ctx.settingsAt(place)?.deleteAfterMs ?? DELETE_AFTER_MS) / 60_000);
      const id = `ws_${randomBytes(4).toString("hex")}`;
      const machineRecord: WorkspaceRecord = {
        id,
        name,
        kind: "cloud",
        machineId: "",
        phase: "running",
        golden: head.snapshotId,
        createdAt: new Date(clock.now()).toISOString(),
        project: project.id,
        spec: {},
        ...(placeId !== undefined ? { place: placeId } : {}),
        size: { cpu: size.cpu, memMb: size.memMb },
        firstLife: true,
        ...ctx.treeOf(scope),
        // Its own switch, off: a sub-thread starts nothing of its own.
        agents: { ...AGENTS_ON, spawn: false } satisfies WorkspaceAgents,
        lead: { threadId: scope.threadId, workspaceId: lead.record.id },
        sub: {
          on: on.toLowerCase(),
          repo,
          base,
          branch: result,
          folder,
          rateUsdPerHour: size.rateUsdPerHour,
          sizeName: size.name,
          spawn: { startedAt: began, branch: { ...base, ms: clock.now() - began }, ...(uncommitted > 0 ? { uncommitted } : {}), ...(seed.why !== undefined ? { notSent: seed.why } : {}), log: [] },
        },
      };
      const report = logged(machineRecord, ctx.stageReporter(id, name, clock.now(), scope));
      // Past the machines at once or the minute's starts the spawn waits its turn, its record queued with no machine
      // (05, Q4); the queue makes the machine and starts the thread when its turn comes.
      const wait = ctx.queueWait(place, limits);
      // Taken in the same step as the read, with no await between, so a spawn asked beside this one counts it.
      const claim = wait === undefined ? ctx.claimStart(id, place) : undefined;
      ctx.forking.add(name);
      ctx.supersedeFailed(name);
      try {
        if (wait !== undefined) {
          report("fork-requested", capWaitLine(wait), { waiting: true });
          await ctx.enqueue(machineRecord, wait);
        } else {
          // Another client took the last slot between the read and the create: the spawn waits at the head (Q9).
          const made = await machineFor(machineRecord, at, on, seed, report, () => (freePlace(), claim!.posted())).catch(async (e: unknown) => {
            if (!isCapRefusal(e)) throw e;
            const full = ctx.cloudFull(place, limits);
            report("fork-requested", capWaitLine(full), { waiting: true });
            await ctx.enqueue(machineRecord, full, true);
            return [];
          });
          notices.push(...made);
        }
      } catch (e) {
        report("failed", e instanceof Error ? e.message : String(e));
        throw e;
      } finally {
        claim?.release();
        ctx.forking.delete(name);
      }
      if (uncommitted > 0) notices.push(subUncommittedLine(uncommitted, project.name, base.branch, base.sha));
      if (seed.why !== undefined) notices.push(subNotSentLine(seed.why, project.name));
      const out: SubOut = {
        on: on.toLowerCase(),
        machine: name,
        size: { cpu: size.cpu, memMb: size.memMb },
        rateUsdPerHour: size.rateUsdPerHour,
        repo,
        base,
        branch: result,
        sent: { paths: seed.paths, bytes: seed.bytes, missing: seed.missing },
        notSent: { uncommitted, ...(seed.why !== undefined ? { why: seed.why } : {}) },
        deleteAfterMinutes,
      };
      const preamble = subPreamble({ on, size, lead: scope.threadId, folder, sent: seed.paths, setupGit: host.setupGit, repo, base, branch: result, deleteAfterMinutes });
      const view = ctx.view(machineRecord);
      return { ...view, ...(notices.length > 0 ? { notice: notices.join("\n") } : {}), sub: { out, folder, preamble, harness } };
    } finally {
      freePlace();
    }
  };

  /** A machine for the record, deleted and replaced once where it fails before the first turn in a way a fresh one
   * cures; the lines of a replacement, which the spawn's answer carries. */
  const machineFor = async (record: WorkspaceRecord, at: MachineBackend, on: string, seed: Seed, report: StageReport, landed: () => void): Promise<string[]> => {
    const notices: string[] = [];
    let first: SubFault | undefined;
    for (;;) {
      try {
        await standUp(record, on, seed, report, landed);
        return notices;
      } catch (e) {
        const fault = faultOf(e);
        await dropMachine(at, record, fault?.box);
        if (fault === undefined) throw e;
        if (first !== undefined) throw subTwiceRefusal(first, fault, record.name, on);
        first = fault;
        const line = subReplacedLine(fault, record.name, on);
        notices.push(line);
        step(record, spawn => {
          spawn.replaced = [...(spawn.replaced ?? []), line];
          delete spawn.files;
        });
        report("fork-requested", line, { notice: line });
        console.warn(`workspace ${record.id}: ${line}`);
        // A fresh key, so the next create is a new machine and not a replay of the one deleted.
        await store.delete(CREATES, `workspace/${record.id}`);
      }
    }
  };

  /** The machine of a spawn the queue let through: the cloud and the picked files read again now, since the files
   * are packed fresh at each start (S21), and the stages said under the lead that asked. */
  const standUpQueued = async (record: WorkspaceRecord, posted: () => void): Promise<void> => {
    const placeId = ctx.placeIdOf(record)!;
    const row = (await ctx.computerRows()).find(r => r.id === placeId || r.id === record.provider);
    const on = row === undefined ? record.sub!.on : providerKeyName(row.name);
    const at = ctx.backendFor(record);
    const seed = await seedOf(ctx.projectHeld(record.project));
    const lead = record.lead!;
    // The queue's wait is the strip's to say: the start's own time counts from here.
    step(record, spawn => {
      spawn.startedAt = clock.now();
      if (seed.why !== undefined) spawn.notSent = seed.why;
      else delete spawn.notSent;
    });
    const report = logged(record, ctx.stageReporter(record.id, record.name, clock.now(), { kind: "thread", threadId: lead.threadId, workspaceId: lead.workspaceId, rootThreadId: record.rootThreadId ?? lead.threadId }));
    ctx.forking.add(record.name);
    try {
      await machineFor(record, at, on, seed, report, posted);
    } catch (e) {
      spawnFailed(record, e);
      report("failed", e instanceof Error ? e.message : String(e));
      throw e;
    } finally {
      ctx.forking.delete(record.name);
    }
  };

  /** The default result branch, numbered past a name the remote holds or another sub-machine of this host was given:
   * a record deleted with its thread no longer names its branch, and the remote still holds it. The remote is read
   * from the lead's folder where it is, on this computer or over its box's own road, with no prompt and a bounded
   * wait; a remote that does not answer leaves the records alone to say what is taken. */
  const freeBranch = async (wanted: string, lead: LiveWorkspace): Promise<string> => {
    const taken = new Set([...live.values()].map(e => e.record.sub?.branch).filter(b => b !== undefined));
    const machine = copiesFolder(kindForComputer(ctx.projectHeld(lead.record.project).computer)) ? await ctx.moduleOf("local").backend({ kind: "local" } as WorkspaceRecord).get(LOCAL_MACHINE_ID) : lead.machine;
    const heads = await machine.exec(`GIT_TERMINAL_PROMPT=0 ${shellLine(["git", "-C", ctx.checkoutOf(lead.record), "ls-remote", "--heads", "origin"])}`, { timeoutMs: INLINE_EXEC_MS }).catch(() => undefined);
    for (const line of heads?.exitCode === 0 ? heads.stdout.split("\n") : []) {
      const ref = line.split("\t")[1];
      if (ref?.startsWith("refs/heads/") === true) taken.add(ref.slice("refs/heads/".length));
    }
    if (!taken.has(wanted)) return wanted;
    for (let n = 2; ; n++) if (!taken.has(`${wanted}-${n}`)) return `${wanted}-${n}`;
  };

  /** The files picked for the lead's project folder on this computer, packed now; nothing and why for a project with
   * no picks or one whose folder is elsewhere. Refuses a pack past what a sub-thread carries. */
  const seedOf = async (project: ReturnType<RuntimeContext["projectHeld"]>): Promise<Seed> => {
    const none = { files: 0, paths: [], bytes: 0, missing: [] };
    if (!copiesFolder(kindForComputer(project.computer))) return { ...none, why: subPickedElsewhereWhy(project.name, ctx.placeName(project.computer)) };
    const folder = folderNamed(project.path);
    const choice = (await store.get(SEED_CHOICES, folder)) as SeedChoice | undefined;
    if (choice === undefined || ctx.opts.seed === undefined) return { ...none, why: subNothingPickedWhy(project.name) };
    const homes = await ctx.homesHere();
    const plan = await ctx.opts.seed.plan(folder, homes);
    const rows = new Map(plan.files.filter(f => f.kind !== "never").map(f => [f.path, f]));
    const paths = choice.files.filter(p => rows.has(p));
    const missing = choice.files.filter(p => !rows.has(p));
    if (paths.length === 0) return { ...none, missing };
    const packed = await ctx.opts.seed.pack({ plan, choice: { files: paths, memory: false, commits: false }, homes });
    if (packed.tar.length > SUB_SEED_MAX_BYTES) {
      const largest = paths.map(p => rows.get(p)!).sort((a, b) => b.bytes - a.bytes).slice(0, 2).map(f => `${f.path} (${fmtBytes(f.bytes)})`);
      throw subSeedTooBigRefusal(project.name, fmtBytes(packed.tar.length), fmtBytes(SUB_SEED_MAX_BYTES), largest);
    }
    return { tar: packed.tar, files: packed.files, paths, bytes: packed.bytes, missing };
  };

  /** One machine for the record: forked from the image head, its daemon asked, the folder made and the picked files
   * landed through the provider's file road, never inside a command. The record is stored once all of it stands. */
  const standUp = async (record: WorkspaceRecord, on: string, seed: Seed, report: StageReport, landed: () => void): Promise<void> => {
    step(record, spawn => {
      spawn.machine = { since: clock.now(), states: [] };
    });
    report("fork-requested", startingLine(record.name, on));
    await ctx.fork(
      record,
      m => {
        record.machineId = m.id;
        record.phase = "running";
        ctx.attach(record, m).creating = true;
        landed();
      },
      undefined,
      report,
      false,
    );
    const entry = live.get(record.id)!;
    if (entry.machine.previewUrl !== undefined || entry.machine.daemonAnswers !== undefined) {
      const fault = await ctx.pingDaemon(entry);
      report("daemon-answering", fault === undefined ? "Daemon answered." : "Daemon did not answer.", { notice: fault });
      void ctx.syncDaemon(entry);
    }
    const folder = record.sub!.folder;
    const made = await entry.machine.exec(`install -d -m 0755 ${shellQuote(folder)}`);
    if (made.exitCode !== 0) throw new Error(`${folder} was not made on ${entry.machine.id} (exit ${made.exitCode}): ${made.stderr.trim() || made.stdout.trim()}`);
    step(record, spawn => {
      if (spawn.machine !== undefined) spawn.machine.ms = clock.now() - spawn.machine.since;
    });
    if (seed.tar !== undefined) {
      step(record, spawn => {
        spawn.files = { since: clock.now(), paths: seed.paths, bytes: seed.bytes };
      });
      if (entry.machine.putBytes === undefined) throw new Error(`${on} takes no file onto ${entry.machine.id}, so the picked files cannot land`);
      report("project-cloned", subCopyingLine(seed.files));
      const at = `/tmp/wsp-seed-${record.id}.tgz`;
      await entry.machine.putBytes(at, seed.tar);
      const unpacked = await entry.machine.exec(`tar -xzf ${shellQuote(at)} -C ${shellQuote(folder)} && rm -f ${shellQuote(at)}`);
      if (unpacked.exitCode !== 0) throw new Error(`the picked files did not land in ${folder} (exit ${unpacked.exitCode}): ${unpacked.stderr.trim() || unpacked.stdout.trim()}`);
      step(record, spawn => {
        if (spawn.files !== undefined) spawn.files.ms = clock.now() - spawn.files.since;
      });
    }
    step(record, spawn => {
      spawn.readyAt = clock.now();
      delete spawn.asked;
    });
    await ctx.persist(record);
    delete entry.creating;
    report("ready", CREATE_READY);
    if (record.sub!.releasedAt === undefined) bus.emit({ type: "workspace.created", workspace: ctx.view(record) });
    else await ctx.emitStatus(entry, "reachable");
  };

  /** The machine a failed attempt made goes: the record's own where it bound one, else the box the fault names, which
   * the provider made under a replayed key and so did not delete itself. A machine already gone is gone. */
  const dropMachine = async (at: MachineBackend, record: WorkspaceRecord, box: string | undefined): Promise<void> => {
    const entry = live.get(record.id);
    // A queued record's stand-in has no machine to delete.
    if (entry !== undefined && !isAbsentMachine(entry.machine)) {
      await ctx.unfork(entry).catch((e: unknown) => {
        if (!isMissing(e)) console.warn(`workspace ${record.id}: ${entry.machine.id} was not deleted: ${e instanceof Error ? e.message : String(e)}`);
      });
      live.delete(record.id);
    }
    if (box !== undefined && box !== entry?.machine.id) {
      await at
        .get(box)
        .then(m => m.kill())
        .catch((e: unknown) => {
          if (!isMissing(e)) console.warn(`workspace ${record.id}: ${box} was not deleted: ${e instanceof Error ? e.message : String(e)}`);
        });
    }
    record.machineId = "";
  };

  const resuming = new Map<string, Promise<unknown>>();
  /** A send into a released sub-thread: the checks that still apply (the cloud held, the image and its sign-in, the
   * spend, the agent, the picked files, the starts), then the same record on a machine forked from the image head as
   * it stands now, at its size, the picked files and the kept session landed before the turn. The lead's branch is
   * not read again: the thread goes on from its own result branch. */
  const resumeSub: SubSpawnArea["resumeSub"] = async (asked, threadId, harness, prompt) => {
    const id = asked.record.id;
    const ahead = resuming.get(id);
    if (ahead !== undefined) {
      await ahead;
      return { entry: live.get(id)!, notices: [] };
    }
    if (asked.record.phase !== "released") return { entry: asked, notices: [] };
    const work = resumeOnce(asked, threadId, harness, prompt);
    resuming.set(id, work);
    try {
      return await work;
    } finally {
      resuming.delete(id);
    }
  };

  const resumeOnce = async (asked: LiveWorkspace, threadId: string, harness: string, prompt: string | undefined) => {
    const record = asked.record;
    const sub = record.sub!;
    const row = (await ctx.computerRows()).find(r => r.kind === "provider" && namesPlace(r, sub.on));
    const at = row === undefined ? undefined : providerAt(row.id);
    if (row === undefined || at === undefined) throw subCloudGoneRefusal(cloudSaid(sub.on));
    const on = providerKeyName(row.name);
    const place = record.place ?? row.id;
    const project = ctx.projectHeld(record.project);
    const host = gitHostOf(sub.repo);
    if (host === undefined) throw subOtherHostRefusal(project.name, remoteHost(sub.repo) ?? sub.repo, on);
    const { head, record: image } = await imageAt(place, on, host, sub.repo);
    await ctx.placeGuard(place);
    agentOn(image, on, harness);
    const seed = await seedOf(project);
    const limits = at.limits === undefined ? undefined : await at.limits();
    startsLeft(limits, record.name);
    const size = soldBy(at, limits).find(s => s.cpu === record.size.cpu && s.memMb === record.size.memMb) ?? { name: subSizeWord(record.size), ...record.size };
    record.golden = head.snapshotId;
    record.phase = "running";
    record.firstLife = true;
    delete sub.lost;
    sub.spawn = { startedAt: clock.now(), ...(prompt !== undefined ? { asked: prompt } : {}), ...(seed.why !== undefined ? { notSent: seed.why } : {}), log: [] };
    told(record);
    const report = logged(record, ctx.stageReporter(record.id, record.name, clock.now(), undefined));
    ctx.forking.add(record.name);
    let notices: string[];
    try {
      notices = await machineFor(record, at, on, seed, report, () => {});
    } catch (e) {
      spawnFailed(record, e);
      report("failed", e instanceof Error ? e.message : String(e));
      record.phase = "released";
      record.machineId = "";
      ctx.attach(record, deadMachine(""));
      await ctx.persist(record);
      // The row saying the start failed goes out before the refusal, so the app's start line holds it when the send's
      // refusal lands and says it there alone.
      await ctx.rowsSent();
      throw e;
    } finally {
      ctx.forking.delete(record.name);
    }
    const entry = live.get(record.id)!;
    await landSession(entry, threadId);
    if (seed.why !== undefined) notices.push(subNotSentLine(seed.why, project.name));
    const deleteAfterMinutes = Math.round((ctx.settingsAt(place)?.deleteAfterMs ?? DELETE_AFTER_MS) / 60_000);
    const pushed = sub.result?.pushedSha != null;
    const preamble = subResumePreamble({ on, size, folder: sub.folder, sent: seed.paths, setupGit: host.setupGit, repo: sub.repo, base: sub.base, branch: sub.branch, pushed, deleteAfterMinutes });
    return { entry, preamble, notices };
  };

  /** The session the host kept, landed where it stood on the machine it was kept from and nowhere else under the
   * agent's folder. A landing that fails leaves the turn to go on from the transcript, which the agent's own fallback
   * hands it. */
  const landSession = async (entry: LiveWorkspace, threadId: string): Promise<void> => {
    const kept = entry.record.sub!.session;
    if (kept?.kept == null || kept.path === undefined) return;
    const file = ctx.adapterFor(entry, kept.agent).adapter.sessionFile?.(kept.id);
    const bytes = await store.getBlob(SUB_SESSIONS, threadId);
    if (file === undefined || bytes === undefined || entry.machine.putBytes === undefined || !kept.path.startsWith(`${file.folder}/`)) return;
    const at = `/tmp/wsp-session-${threadId}.gz`;
    try {
      await entry.machine.putBytes(at, bytes);
      const landed = await entry.machine.exec(subSessionLandCommand(file, kept.path, at), { timeoutMs: INLINE_EXEC_MS });
      if (landed.exitCode !== 0) throw new Error(`exit ${landed.exitCode}: ${(landed.stderr.trim() || landed.stdout.trim()).slice(-200)}`);
    } catch (e) {
      console.warn(`the kept session of sub-thread ${threadId} did not land on ${entry.machine.id}, so its turn goes on from the transcript: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  /** What a sub-machine's run the kernel killed for memory fails with, off one reading of its kernel log since the
   * turn started: the memory killer writes "Out of memory: Killed process" for the machine and "Memory cgroup out of
   * memory: Killed process" for a cgroup. A reading that fails or counts none keeps the agent's own exit line. */
  const subKilledWhy = async (entry: LiveWorkspace, startedAt: number): Promise<string | undefined> => {
    const { record } = entry;
    const read = await entry.machine.exec(`journalctl -k --since @${Math.floor(startedAt / 1000)} --no-pager | grep -ci 'out of memory'`, { timeoutMs: INLINE_EXEC_MS }).catch((e: unknown) => {
      console.warn(`the kernel log of ${record.name} was not read after its agent was killed: ${e instanceof Error ? e.message : String(e)}`);
      return undefined;
    });
    if (read === undefined || !(Number.parseInt(read.stdout, 10) > 0)) return undefined;
    const at = ctx.backendFor(record);
    const sold = soldBy(at, at.limits === undefined ? undefined : await at.limits().catch(() => undefined));
    const size = sold.find(s => s.cpu === record.size.cpu && s.memMb === record.size.memMb) ?? { name: subSizeWord(record.size), ...record.size };
    const bigger = subNextSize(record.size, sold);
    const row = (await ctx.computerRows().catch(() => [])).find(r => r.kind === "provider" && namesPlace(r, record.sub!.on));
    return subOomLine(record.name, row === undefined ? record.sub!.on : providerKeyName(row.name), size, bigger === undefined ? undefined : subSizeWord(bigger));
  };

  return { spawnSub, standUpQueued, subKilledWhy, resumeSub };
}
