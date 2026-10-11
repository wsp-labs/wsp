// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's start on a cloud: every check that can refuse runs before the
// cloud is asked for a start, then one machine forked from the image head, an
// empty folder named after the repository with the picked git-ignored files in
// it, and nothing else. wsp pushes nothing for the lead and clones nothing: the
// preamble hands the agent the git lines. A machine that fails before the first
// turn holds nothing, so it is deleted and replaced once.
import { randomBytes } from "node:crypto";
import { gitHostOf, remoteHost, type GitHost } from "@wsp/catalog";
import { CreateCancelledError, EnvNotLandedError, GuestUnusableError, INLINE_EXEC_MS, LOCAL_MACHINE_ID, goldenHead, isMissing, type CloudLimits, type MachineBackend } from "@wsp/engine";
import {
  DETACHED_HEAD, GitStatusReply, SUB_DELETE_AFTER_MIN, SUB_SEED_MAX_BYTES, branchUnreadRefusal, copiesFolder, fmtBytes, kindForComputer, namesPlace, runsInFolder, ownerRepoOf,
  providerKeyName, scopeOf, shellLine, shellQuote, startingLine, subAheadRefusal, subAheadUnknownRefusal, subBranchNameOk, subBranchNameRefusal, subBranchOf, subCopyingLine, subDayStartsRefusal, subDetachedRefusal, subFolderOf,
  subHourStartsRefusal, subLeadBranchRefusal, subLeadOnMachineRefusal, subNoAgentRefusal, subNoCloudRefusal, subNoImageRefusal, subNoRemoteRefusal, subNoSignInRefusal, subNoThreadRefusal, subNoUpstreamRefusal,
  subNotSentLine, subNothingPickedWhy, subOtherHostRefusal, subPickedElsewhereWhy, subPlanWord, subPreamble, subReplacedLine, subSeedTooBigRefusal, subSizeOf, subSizeRefusal,
  subSizeWord, subTwiceRefusal, subUncommittedLine, type Caller, type SeedChoice, type SubFault, type SubOut, type SubSize, type WorkspaceAgents, AGENTS_ON, CREATE_READY,
} from "@wsp/protocol";
import { SEED_CHOICES, CREATES } from "../types/internal.js";
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

export function subSpawnArea(ctx: RuntimeContext): SubSpawnArea {
  const { live, store, places, placeDoor, bus, clock } = ctx;

  /** The backend a provider row forks on, or nothing where this host holds no key for it. */
  const providerAt = (id: string): MachineBackend | undefined => places.backend(id) ?? placeDoor?.backendOf(id);

  /** The sizes a cloud sells now: its plan's where it reads one, else every size it offers. */
  const soldBy = (at: MachineBackend, limits: CloudLimits | undefined): SubSize[] =>
    limits?.sizes ?? at.capabilities.sizes.map(s => ({ name: subSizeWord(s), cpu: s.cpu, memMb: s.memMb, rateUsdPerHour: s.rateUsdPerHour ?? at.pricing.rateUsdPerHour(s) }));

  /** A setting a person keeps on the cloud's row beyond the ones every place reads, where one is set. */
  const settingOf = (placeId: string, key: "size" | "deleteAfterMs"): unknown => {
    const held = placeDoor?.settingsAt(placeId);
    return held !== undefined && key in held ? (held as Record<string, unknown>)[key] : undefined;
  };

  const spawnSub = async (opts: CreateWorkspaceOptions, origin: Caller | undefined): Promise<CreatedWorkspace> => {
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
      const head = goldenHead(await ctx.copyOf(place, SETUP_IMAGE));
      if (head === undefined) throw subNoImageRefusal(on);
      const record = await ctx.recordOf(SETUP_IMAGE);
      const label = HOST_WORDS[host.id];
      const logins = head.logins ?? record?.logins ?? [];
      if (!logins.some(l => (l.name === host.cli || l.name.startsWith(`${label} CLI`)) && LOGIN_HELD.has(l.state))) {
        throw subNoSignInRefusal(on, { label, cli: host.cli, login: `${label} CLI login` }, ownerRepoOf(project.remote)!);
      }
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
      const carried = (record?.recipe?.rows ?? []).filter(r => r.kind === "agent" && r.on).map(r => r.id);
      if (carried.length > 0 && !carried.includes(harness)) throw subNoAgentRefusal(on, harness, carried);
      // S12: the picked files, packed fresh from the lead's folder on this computer.
      const seed = await seedOf(project);
      // S13: the hour's and the day's starts.
      const starts = limits?.starts;
      const plan = subPlanWord({ trial: limits?.trial === true, key: limits?.planKey });
      if (starts !== undefined && starts.day.remaining <= 0) throw subDayStartsRefusal(on, starts.day.used, starts.day.limit, plan, name);
      if (starts !== undefined && starts.hour.remaining <= 0) throw subHourStartsRefusal(on, starts.hour.used, starts.hour.limit, plan, name);

      const nameTaken = ctx.nameRefusal(name);
      if (nameTaken !== undefined) throw Object.assign(new Error(nameTaken), { kind: "conflict" });
      const folder = subFolderOf(project.remote);
      const deleteAfterMs = settingOf(place, "deleteAfterMs");
      const deleteAfterMinutes = typeof deleteAfterMs === "number" ? Math.round(deleteAfterMs / 60_000) : SUB_DELETE_AFTER_MIN;
      const id = `ws_${randomBytes(4).toString("hex")}`;
      const report = ctx.stageReporter(id, name, clock.now(), scope);
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
        sub: { on: on.toLowerCase(), repo, base, branch: result, folder },
      };
      ctx.forking.add(name);
      ctx.supersedeFailed(name);
      try {
        let first: SubFault | undefined;
        for (;;) {
          try {
            await standUp(machineRecord, on, seed, report, freePlace);
            break;
          } catch (e) {
            const fault = faultOf(e);
            await dropMachine(at, machineRecord, fault?.box);
            if (fault === undefined) throw e;
            if (first !== undefined) throw subTwiceRefusal(first, fault, name, on);
            first = fault;
            const line = subReplacedLine(fault, name, on);
            notices.push(line);
            report("fork-requested", line, { notice: line });
            console.warn(`workspace ${id}: ${line}`);
            // A fresh key, so the next create is a new machine and not a replay of the one deleted.
            await store.delete(CREATES, `workspace/${id}`);
          }
        }
      } catch (e) {
        report("failed", e instanceof Error ? e.message : String(e));
        throw e;
      } finally {
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

  /** The default result branch, numbered past a name the remote holds or another sub-machine of this host was given.
   * The remote is read from the lead's folder on this computer, with no prompt and a bounded wait; a lead elsewhere,
   * or a remote that does not answer, leaves the records alone to say what is taken. */
  const freeBranch = async (wanted: string, lead: LiveWorkspace): Promise<string> => {
    const taken = new Set([...live.values()].map(e => e.record.sub?.branch).filter(b => b !== undefined));
    if (copiesFolder(kindForComputer(ctx.projectHeld(lead.record.project).computer))) {
      const machine = await ctx.moduleOf("local").backend({ kind: "local" } as WorkspaceRecord).get(LOCAL_MACHINE_ID);
      const heads = await machine.exec(`GIT_TERMINAL_PROMPT=0 ${shellLine(["git", "-C", ctx.checkoutOf(lead.record), "ls-remote", "--heads", "origin"])}`, { timeoutMs: INLINE_EXEC_MS }).catch(() => undefined);
      for (const line of heads?.exitCode === 0 ? heads.stdout.split("\n") : []) {
        const ref = line.split("\t")[1];
        if (ref?.startsWith("refs/heads/") === true) taken.add(ref.slice("refs/heads/".length));
      }
    }
    if (!taken.has(wanted)) return wanted;
    for (let n = 2; ; n++) if (!taken.has(`${wanted}-${n}`)) return `${wanted}-${n}`;
  };

  /** The files picked for the lead's project folder on this computer, packed now; nothing and why for a project with
   * no picks or one whose folder is elsewhere. Refuses a pack past what a sub-thread carries. */
  const seedOf = async (project: ReturnType<RuntimeContext["projectHeld"]>): Promise<{ tar?: Buffer; files: number; paths: string[]; bytes: number; missing: string[]; why?: string }> => {
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
  const standUp = async (record: WorkspaceRecord, on: string, seed: { tar?: Buffer; files: number }, report: StageReport, landed: () => void): Promise<void> => {
    report("fork-requested", startingLine(record.name, on));
    await ctx.fork(
      record,
      m => {
        record.machineId = m.id;
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
    if (seed.tar !== undefined) {
      if (entry.machine.putBytes === undefined) throw new Error(`${on} takes no file onto ${entry.machine.id}, so the picked files cannot land`);
      report("project-cloned", subCopyingLine(seed.files));
      const at = `/tmp/wsp-seed-${record.id}.tgz`;
      await entry.machine.putBytes(at, seed.tar);
      const unpacked = await entry.machine.exec(`tar -xzf ${shellQuote(at)} -C ${shellQuote(folder)} && rm -f ${shellQuote(at)}`);
      if (unpacked.exitCode !== 0) throw new Error(`the picked files did not land in ${folder} (exit ${unpacked.exitCode}): ${unpacked.stderr.trim() || unpacked.stdout.trim()}`);
    }
    await ctx.persist(record);
    delete entry.creating;
    report("ready", CREATE_READY);
    bus.emit({ type: "workspace.created", workspace: ctx.view(record) });
  };

  /** The machine a failed attempt made goes: the record's own where it bound one, else the box the fault names, which
   * the provider made under a replayed key and so did not delete itself. A machine already gone is gone. */
  const dropMachine = async (at: MachineBackend, record: WorkspaceRecord, box: string | undefined): Promise<void> => {
    const entry = live.get(record.id);
    if (entry !== undefined) {
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

  return { spawnSub };
}
