// SPDX-License-Identifier: AGPL-3.0-only
// What Activity draws from one computer's process frame: wsp's own parts, each
// thread with what it started, what wsp started with no thread known, and the
// rest of the computer. A row names its thread and kind where the daemon
// stamped them; until it does the page reads them off what it already knows,
// the thread's own process, its cgroup on a box, its slate's folder, the daemon
// and the host, and the parent chain under each. Sort, filter, the Kinds and Ports views and the
// tints are pure functions of that, so the page draws and the tests read one rule.
import { agentName, CATALOG_AGENTS } from "@wsp/catalog";
import { agentPath, APP_PROC_WORDS, inCgroup, PROC_KINDS, procBins, SCRIPT_RUNTIMES, type AppProc, type ProcEntry, type ProcKind, type ProcSnapshot } from "@wsp/protocol";

export type { ProcKind };

/** One port a process listens on, and whether only this computer can reach it. */
export interface ActivityPort {
  readonly port: number;
  /** Null where the daemon did not say. */
  readonly loopback: boolean | null;
}

/** A thread that runs on the computer the page reads: its own process where the host started it here, or on a box the
 * cgroup every process it started stands in. */
export interface ActivityThread {
  readonly id: string;
  readonly title: string;
  readonly harness: string;
  readonly project: string;
  readonly workspaceId: string;
  readonly sessionId: string;
  /** A turn runs on it now: its agent's act is the thread's stop. */
  readonly working: boolean;
  readonly pid?: number;
  readonly cgroup?: string;
  /** Its slate's title, where this window has read the slate. */
  readonly slate?: string;
}

export interface ActivityProc {
  readonly pid: number;
  readonly name: string;
  readonly kind: ProcKind;
  readonly mem: number;
  readonly cpu: number;
  /** Milliwatts over the interval; null where the computer keeps no energy counter. */
  readonly energy: number | null;
  readonly ports: readonly ActivityPort[];
  readonly command: string;
  /** The start and the identity the row was read with, which an End sends so the daemon never signals a pid handed
   * on since; absent on a heading, which is no process. */
  readonly startedAt?: number;
  readonly id?: string;
  /** The harness of an agent row, whose mark it draws. */
  readonly harness?: string;
  /** wsp needs it to run: no End. */
  readonly needed?: boolean;
  /** A heading over processes and no process itself: a thread's slate, its runs under it. It takes no act, and its
   * figures are its runs' sum, which no total counts twice. */
  readonly heading?: boolean;
  readonly children: readonly ActivityProc[];
}

export type GroupKind = "wsp" | "thread" | "loose";

export interface ActivityGroup {
  readonly id: string;
  readonly kind: GroupKind;
  readonly thread?: ActivityThread;
  /** The heads of the group's trees. */
  readonly procs: readonly ActivityProc[];
}

export interface ActivityScope {
  /** wsp's own parts first, then each thread, then what wsp started with no thread known; empty groups left out. */
  readonly groups: readonly ActivityGroup[];
  /** Every other process on the computer, flat. */
  readonly rest: readonly ActivityProc[];
  /** Whether any row carries an energy figure: a Mac does, a box never. */
  readonly energy: boolean;
}

export interface ScopeInput {
  readonly snapshot: ProcSnapshot;
  readonly threads: readonly ActivityThread[];
  /** Listening ports by the pid holding them. */
  readonly ports: ReadonlyMap<number, readonly ActivityPort[]>;
  /** The host's own pid, which on the computer it runs on is the daemon's parent; absent on a box. */
  readonly host?: number;
  /** The desktop app's own processes by pid, as the app names them, on the computer the host runs on. */
  readonly app?: ReadonlyMap<number, AppProc>;
}

const NO_APP: ReadonlyMap<number, AppProc> = new Map();

const siteOf = (origin: string): string => {
  try {
    return new URL(origin).host || origin;
  } catch {
    return origin;
  }
};

/** An app part by what the app calls it, wsp itself for its main process; a tab or a frame by the site it shows. */
function appName(part: AppProc): string {
  if (part.kind === "main") return "wsp";
  if (part.kind !== "tab" && part.kind !== "frame") return APP_PROC_WORDS[part.kind];
  const [first, ...more] = part.origins ?? [];
  return first === undefined ? APP_PROC_WORDS[part.kind] : `${siteOf(first)}${more.length > 0 ? ` +${more.length}` : ""}`;
}

const SHELLS = new Set(["sh", "bash", "zsh", "fish", "dash"]);
/** Named with the subcommand they run: cargo test, pnpm dev. */
const TOOLS = new Set(["cargo", "npm", "npx", "pnpm", "yarn", "uv", "go", "bunx"]);
const GENERIC_SCRIPTS = new Set(["bin", "cli", "index", "main"]);
const BUILD_DIRS = new Set(["dist", "lib", "build", "out", "src", "bin", ".bin", "node_modules"]);
const TEST_LINE = /\b(vitest|jest|mocha|pytest|playwright test|cargo test|cargo nextest|go test|bun test|node --test)\b/;
const BUILD_LINE = /\b(rustc|cargo build|cargo check|cc1|cc1plus|clang|ld|tsc|go build|webpack(?! serve)|swiftc)\b/;
const DEV_LINE = /\b(vite|next dev|next-server|webpack serve|astro dev|nuxt dev|remix dev|wrangler dev|electron-vite|http\.server|flask run|uvicorn|rails server)\b/;
/** A slate's runs read their files from the slate's folder, the host's `slates/<thread>` or a box's `wsp-slates/<thread>`. */
const SLATE_FOLDER = /(^|\/)(wsp-)?slates\/[^/\s]+\//;
const slateFolderOf = (threadId: string): string => `slates/${threadId}/`;

/** What a process is called: the script a runtime runs, a tool with its subcommand, or the command. */
export function procName(p: ProcEntry): string {
  const argv = p.cmdline.split(" ").filter(w => w !== "");
  const bin = (argv[0] ?? p.comm).split("/").at(-1) ?? p.comm;
  const comm = p.comm !== "" ? p.comm : bin;
  if (TOOLS.has(comm)) {
    const sub = argv.slice(1).find(w => !w.startsWith("-"));
    return sub === undefined ? comm : `${comm} ${sub}`;
  }
  if (!SCRIPT_RUNTIMES.has(comm) && !SCRIPT_RUNTIMES.has(bin)) return comm;
  const at = argv.findIndex((w, i) => i > 0 && !w.startsWith("-"));
  if (at === -1) return comm;
  const parts = argv[at]!.split("/");
  const base = (parts.at(-1) ?? "").replace(/\.(m?js|cjs|ts)$/, "");
  if (/\.py$/.test(base)) return `${comm} ${base}`;
  if (!GENERIC_SCRIPTS.has(base)) return base;
  // bin.js, cli.js and index.js say nothing: the package folder above them does, with the subcommand it was given.
  const pkg = parts.slice(0, -1).reverse().find(d => d !== "" && !BUILD_DIRS.has(d));
  const sub = argv.slice(at + 1).find(w => !w.startsWith("-"));
  return [pkg ?? base, sub].filter(w => w !== undefined).join(" ");
}

/** The kind a row reads where the daemon named none: test runs and builds by their command, a dev server by its port
 * or its command, a terminal by its pty, a tool server as an agent's direct child that is not a shell; what a dev
 * server, a test run or a build started is one too. */
export function kindOf(p: ProcEntry, ports: readonly ActivityPort[], parent: ProcKind | null): ProcKind {
  if (p.kind !== undefined) return p.kind;
  if (p.pty !== undefined) return "terminal";
  if (SLATE_FOLDER.test(p.cmdline)) return "slate";
  if (TEST_LINE.test(p.cmdline)) return "test";
  if (BUILD_LINE.test(p.cmdline)) return "build";
  if (ports.length > 0 || DEV_LINE.test(p.cmdline)) return "dev";
  if (parent === "dev" || parent === "test" || parent === "build") return parent;
  if (parent === "agent" && !SHELLS.has(p.comm)) return "tool";
  return "other";
}

/** The computer's frame read into groups. A process belongs to the first of: the owner the daemon stamped (an owner
 * that names no thread is No thread), the thread whose process it is or descends from, wsp's parts (the daemon and the
 * host, the processes themselves), the thread whose cgroup it stands in, the thread whose slate folder its command
 * names, and what the daemon, the host or a terminal started with no thread known. */
export function scopeOf({ snapshot, threads, ports, host, app = NO_APP }: ScopeInput): ActivityScope {
  // A zombie holds no memory and takes no signal: it waits on its parent and goes with it.
  const procs = snapshot.procs.filter(p => !p.state.startsWith("Z"));
  const byPid = new Map(procs.map(p => [p.pid, p]));
  const threadById = new Map(threads.map(t => [t.id, t]));
  const kids = new Map<number, ProcEntry[]>();
  for (const p of procs) if (p.ppid !== p.pid && byPid.has(p.ppid)) (kids.get(p.ppid) ?? kids.set(p.ppid, []).get(p.ppid)!).push(p);
  const kidsOf = (pid: number): readonly ProcEntry[] => kids.get(pid) ?? [];
  const rootOf = new Map<number, ActivityThread>();
  for (const t of threads) if (t.pid !== undefined && byPid.has(t.pid)) rootOf.set(t.pid, t);
  // The agent is the process the daemon names as the thread's agent, or else the shallowest under one of the thread's
  // roots that runs its harness's binary, the run script and any shell on the way folded into its row. Here a thread is
  // rooted at its own process, which stands for the agent until the binary shows; on a box, whose pids the app is never
  // told, at the heads of its cgroup: the run script the launch records, and whatever left it.
  const agentOf = new Map<number, ActivityThread>();
  /** Each wrapper on the way down from a thread's root to its agent, by the next process on that way. */
  const wrapperOf = new Map<number, number>();
  const findAgent = (root: number, t: ActivityThread, standsIn: boolean): void => {
    const bin = CATALOG_AGENTS.find(a => a.id === t.harness)?.bin;
    const isAgent = (p: ProcEntry): boolean => (p.kind === "agent" && p.owner?.thread === t.id) || (bin !== undefined && procBins(p).includes(bin));
    const path = agentPath(root, kidsOf, isAgent, byPid);
    if (path === undefined) {
      if (standsIn) agentOf.set(root, t);
      return;
    }
    agentOf.set(path.at(-1)!, t);
    for (let i = 0; i + 1 < path.length; i++) wrapperOf.set(path[i]!, path[i + 1]!);
  };
  for (const [pid, t] of rootOf) findAgent(pid, t, true);
  for (const t of threads) {
    const cgroup = t.cgroup;
    if (t.pid !== undefined || cgroup === undefined) continue;
    const inside = procs.filter(p => inCgroup(p.cgroup, cgroup));
    const members = new Set(inside.map(p => p.pid));
    for (const p of inside) if (p.ppid === p.pid || !members.has(p.ppid)) findAgent(p.pid, t, false);
  }
  const wspPids = new Set<number>([snapshot.daemon, ...(host === undefined ? [] : [host])]);

  const owner = new Map<number, string>();
  const ownerOf = (p: ProcEntry, seen: Set<number> = new Set()): string | null => {
    const held = owner.get(p.pid);
    if (held !== undefined) return held === "" ? null : held;
    let said: string | null = null;
    const stamped = p.owner?.thread;
    if (stamped !== undefined && threadById.has(stamped)) said = `t:${stamped}`;
    else if (rootOf.has(p.pid)) said = `t:${rootOf.get(p.pid)!.id}`;
    // Before an owner that names no thread: a Mac's daemon stamps wsp's own parts with wsp's coalition and none.
    else if (wspPids.has(p.pid) || app.has(p.pid) || p.kind === "app" || p.kind === "wsp") said = "wsp";
    else if (p.owner !== undefined && p.owner.by !== "coalition") said = "loose";
    else {
      const cg = threads.find(t => t.cgroup !== undefined && inCgroup(p.cgroup, t.cgroup));
      const slate = SLATE_FOLDER.test(p.cmdline) ? threads.find(t => p.cmdline.includes(slateFolderOf(t.id))) : undefined;
      if (cg !== undefined) said = `t:${cg.id}`;
      else if (slate !== undefined) said = `t:${slate.id}`;
      else if (SLATE_FOLDER.test(p.cmdline)) said = "loose";
      else if (p.pty !== undefined) said = "loose";
      else {
        const parent = p.ppid !== p.pid ? byPid.get(p.ppid) : undefined;
        if (parent !== undefined && !seen.has(parent.pid)) {
          seen.add(p.pid);
          // What the host and the app start outside a thread, the guest tool server among them, is wsp's own.
          said = ownerOf(parent, seen);
        }
      }
      if (said === null && p.owner !== undefined) said = "loose";
    }
    owner.set(p.pid, said ?? "");
    return said;
  };
  for (const p of procs) ownerOf(p);

  const members = new Map<string, ProcEntry[]>();
  const rest: ProcEntry[] = [];
  for (const p of procs) {
    const o = owner.get(p.pid) ?? "";
    if (o === "") rest.push(p);
    else (members.get(o) ?? members.set(o, []).get(o)!).push(p);
  }

  const energy = procs.some(p => p.energyMw !== undefined);
  const kindHeld = new Map<number, ProcKind>();
  const kindAt = (p: ProcEntry): ProcKind => {
    const held = kindHeld.get(p.pid);
    if (held !== undefined) return held;
    let kind: ProcKind;
    const part = app.get(p.pid);
    if (part !== undefined) kind = part.kind === "tab" || part.kind === "frame" ? "tab" : "app";
    else if (agentOf.has(p.pid)) kind = "agent";
    else if (wspPids.has(p.pid) && p.kind === undefined) kind = "wsp";
    else {
      kindHeld.set(p.pid, "other");
      const parent = byPid.get(p.ppid);
      kind = kindOf(p, ports.get(p.pid) ?? [], parent === undefined || parent.pid === p.pid ? null : kindAt(parent));
    }
    kindHeld.set(p.pid, kind);
    return kind;
  };
  const agentByBin = (p: ProcEntry): string | undefined => {
    const bins = procBins(p);
    return CATALOG_AGENTS.find(a => bins.includes(a.bin))?.id;
  };
  const leaf = (p: ProcEntry, children: readonly ActivityProc[]): ActivityProc => {
    const kind = kindAt(p);
    const thread = agentOf.get(p.pid);
    const part = app.get(p.pid);
    const name =
      part !== undefined
        ? appName(part)
        : thread !== undefined
          ? agentName(thread.harness)
          : p.pid === snapshot.daemon
            ? "Daemon"
            : p.pid === host
              ? "Host"
              : kind === "agent" && agentByBin(p) !== undefined
                ? agentName(agentByBin(p)!)
                : procName(p);
    return {
      pid: p.pid,
      name,
      kind,
      mem: p.rss,
      cpu: p.cpu,
      energy: p.energyMw ?? (energy ? 0 : null),
      ports: ports.get(p.pid) ?? [],
      command: p.cmdline !== "" ? p.cmdline : `[${p.comm}]`,
      startedAt: p.startedAt,
      ...(p.id !== undefined ? { id: p.id } : {}),
      ...(thread !== undefined ? { harness: thread.harness } : {}),
      ...(wspPids.has(p.pid) || kind === "app" || p.kind === "wsp" ? { needed: true } : {}),
      children,
    };
  };
  const forest = (list: readonly ProcEntry[]): ActivityProc[] => {
    const inside = new Set(list.map(p => p.pid));
    const kids = new Map<number, ProcEntry[]>();
    const heads: ProcEntry[] = [];
    for (const p of list) {
      if (p.ppid !== p.pid && inside.has(p.ppid)) (kids.get(p.ppid) ?? kids.set(p.ppid, []).get(p.ppid)!).push(p);
      else heads.push(p);
    }
    const build = (p: ProcEntry): ActivityProc => leaf(p, (kids.get(p.pid) ?? []).map(build));
    return heads.map(build).map(foldWrapper);
  };
  /** The run script and the agent under it as the one row a person knows: the agent's, holding what both spend. */
  const foldWrapper = (head: ActivityProc): ActivityProc => {
    const nextPid = wrapperOf.get(head.pid);
    const next = head.children.find(c => c.pid === nextPid);
    if (next === undefined) return head;
    const agent = foldWrapper(next);
    const energySum = agent.energy === null && head.energy === null ? null : (agent.energy ?? 0) + (head.energy ?? 0);
    return { ...agent, mem: agent.mem + head.mem, cpu: agent.cpu + head.cpu, energy: energySum, children: [...agent.children, ...head.children.filter(c => c !== next)] };
  };
  /** A thread's slate runs under one heading beside its agent, since the host and not the agent starts them. */
  const withSlate = (heads: readonly ActivityProc[], t: ActivityThread): ActivityProc[] => {
    const runs = heads.filter(p => p.kind === "slate");
    if (runs.length === 0) return [...heads];
    const sum = spent(runs);
    const slate: ActivityProc = { pid: -1, name: t.slate ?? "", kind: "slate", mem: sum.mem, cpu: sum.cpu, energy: sum.energy, ports: [], command: "", heading: true, children: runs };
    return [...heads.filter(p => p.kind !== "slate"), slate];
  };

  const groups: ActivityGroup[] = [];
  const wsp = members.get("wsp");
  if (wsp !== undefined) groups.push({ id: "wsp", kind: "wsp", procs: forest(wsp) });
  for (const t of threads) {
    const mine = members.get(`t:${t.id}`);
    if (mine !== undefined) groups.push({ id: t.id, kind: "thread", thread: t, procs: withSlate(forest(mine), t) });
  }
  const loose = members.get("loose");
  if (loose !== undefined) groups.push({ id: "loose", kind: "loose", procs: forest(loose) });
  return { groups, rest: rest.map(p => leaf(p, [])), energy };
}

/** Every process under a row, the row included; a heading is no process and only its children count. */
export function flat(p: ActivityProc): ActivityProc[] {
  const under = p.children.flatMap(flat);
  return p.heading === true ? under : [p, ...under];
}

export interface Spent {
  readonly mem: number;
  readonly cpu: number;
  readonly energy: number | null;
  readonly count: number;
}

export function spent(procs: readonly ActivityProc[]): Spent {
  const all = procs.flatMap(flat);
  const energies = all.flatMap(p => (p.energy === null ? [] : [p.energy]));
  return { mem: all.reduce((n, p) => n + p.mem, 0), cpu: all.reduce((n, p) => n + p.cpu, 0), energy: energies.length === 0 ? null : energies.reduce((n, e) => n + e, 0), count: all.length };
}

export type ActivitySort = "mem" | "cpu" | "energy";

const figure = (sort: ActivitySort) => (s: { mem: number; cpu: number; energy: number | null }) => (sort === "mem" ? s.mem : sort === "cpu" ? s.cpu : (s.energy ?? 0));

/** Heaviest first on the column, ties by pid so one frame always lays out the same way. */
export function sortProcs(procs: readonly ActivityProc[], sort: ActivitySort): ActivityProc[] {
  const of = figure(sort);
  return [...procs].sort((a, b) => of(b) - of(a) || a.pid - b.pid).map(p => (p.children.length === 0 ? p : { ...p, children: sortProcs(p.children, sort) }));
}

/** wsp's parts stay first and No thread last; the threads between them heaviest first by their totals. */
export function sortGroups(groups: readonly ActivityGroup[], sort: ActivitySort): ActivityGroup[] {
  const of = figure(sort);
  const rank = (g: ActivityGroup): number => (g.kind === "wsp" ? 0 : g.kind === "thread" ? 1 : 2);
  return [...groups]
    .map(g => ({ g: { ...g, procs: sortProcs(g.procs, sort) }, total: of(spent(g.procs)) }))
    .sort((a, b) => rank(a.g) - rank(b.g) || b.total - a.total || a.g.id.localeCompare(b.g.id))
    .map(x => x.g);
}

/** The words a filter reads a process by: its name, command, kind and ports. */
export const KIND_WORDS: Record<ProcKind, string> = {
  agent: "Agent",
  tool: "Tool server",
  dev: "Dev server",
  test: "Test run",
  build: "Build",
  slate: "Slate run",
  terminal: "Terminal",
  tab: "Browser tab",
  app: "App",
  wsp: "wsp",
  other: "Other",
};

const procHit = (p: ActivityProc, needle: string): boolean => [p.name, p.command, KIND_WORDS[p.kind], String(p.pid), ...p.ports.map(x => String(x.port))].some(s => s.toLowerCase().includes(needle));

/** A tree cut to what matches and the parents above each match. */
function prune(procs: readonly ActivityProc[], keep: (p: ActivityProc) => boolean): ActivityProc[] {
  return procs.flatMap(p => {
    const children = prune(p.children, keep);
    return keep(p) || children.length > 0 ? [{ ...p, children }] : [];
  });
}

export interface ActivityPicks {
  /** Thread ids; empty picks every thread. */
  readonly threads: readonly string[];
  readonly kinds: readonly ProcKind[];
  /** Project names. */
  readonly projects: readonly string[];
}

export const NO_PICKS: ActivityPicks = { threads: [], kinds: [], projects: [] };

/** The groups a filter and the chips leave. The text matches a process or its thread's title; a match stays under its
 * group with its parents. A thread or project chip keeps only those threads, and drops wsp's parts and No thread; a
 * kind chip keeps only processes of those kinds. */
export function filterGroups(groups: readonly ActivityGroup[], text: string, picks: ActivityPicks): ActivityGroup[] {
  const needle = text.trim().toLowerCase();
  const narrowed = picks.threads.length > 0 || picks.projects.length > 0;
  return groups.flatMap(g => {
    if (narrowed && (g.thread === undefined || (picks.threads.length > 0 && !picks.threads.includes(g.id)) || (picks.projects.length > 0 && !picks.projects.includes(g.thread.project)))) return [];
    let procs: readonly ActivityProc[] = g.procs;
    if (picks.kinds.length > 0) procs = prune(procs, p => picks.kinds.includes(p.kind));
    const titled = needle !== "" && g.thread !== undefined && g.thread.title.toLowerCase().includes(needle);
    if (needle !== "" && !titled) procs = prune(procs, p => procHit(p, needle));
    return procs.length > 0 || (titled && picks.kinds.length === 0) ? [{ ...g, procs }] : [];
  });
}

/** One row of the flat views, with the group it belongs to. */
export interface PlacedProc {
  readonly p: ActivityProc;
  readonly group: ActivityGroup | null;
}

/** Every owned process flat, and with `every` the rest of the computer after them, heaviest first. */
export function flatRows(groups: readonly ActivityGroup[], rest: readonly ActivityProc[], every: boolean, sort: ActivitySort, text: string): PlacedProc[] {
  const of = figure(sort);
  const needle = text.trim().toLowerCase();
  const owned = groups.flatMap(g => g.procs.flatMap(flat).map(p => ({ p: { ...p, children: [] }, group: g })));
  const others = every ? rest.filter(p => needle === "" || procHit(p, needle)).map(p => ({ p, group: null })) : [];
  return [...owned, ...others].sort((a, b) => of(b.p) - of(a.p) || a.p.pid - b.p.pid);
}

/** The Kinds view: one group per kind, in PROC_KINDS order, each holding its processes flat with the group they came
 * from. */
export function kindGroups(groups: readonly ActivityGroup[], sort: ActivitySort): { kind: ProcKind; rows: PlacedProc[] }[] {
  const of = figure(sort);
  const rows = groups.flatMap(g => g.procs.flatMap(flat).map(p => ({ p: { ...p, children: [] }, group: g })));
  return PROC_KINDS.flatMap(kind => {
    const mine = rows.filter(r => r.p.kind === kind).sort((a, b) => of(b.p) - of(a.p) || a.p.pid - b.p.pid);
    return mine.length === 0 ? [] : [{ kind, rows: mine }];
  });
}

/** The Ports view: every port an owned process listens on, lowest first. */
export function portRows(groups: readonly ActivityGroup[]): { port: ActivityPort; p: ActivityProc; group: ActivityGroup }[] {
  return groups
    .flatMap(g => g.procs.flatMap(flat).flatMap(p => p.ports.map(port => ({ port, p, group: g }))))
    .sort((a, b) => a.port.port - b.port.port || a.p.pid - b.p.pid);
}

// ---------------------------------------------------------------------------
// Tints: a figure is inked only when it means something.
// ---------------------------------------------------------------------------

/** An agent holds 0.5 to 1.5 GB at rest (the research read Claude Code at 1.5 GB); past 2 GB a process or a thread holds
 * more than a busy agent does. */
export const MEMORY_HIGH = 2 * 1024 ** 3;
/** A quarter of a 32 GB computer in one place: the size the 2026-10-09 leak of test workers reached per group. */
export const MEMORY_VERY_HIGH = 8 * 1024 ** 3;
/** wsp holding half the computer's memory pushes everything else toward swap. */
export const MEMORY_SHARE_HIGH = 0.5;
/** Three quarters, and the computer is short of memory for anything else. */
export const MEMORY_SHARE_VERY_HIGH = 0.75;
/** One core flat out: a loop or a build, not an agent waiting on the model. */
export const CPU_HIGH = 100;
/** Four cores: the computer feels it whatever it is. */
export const CPU_VERY_HIGH = 400;
/** A performance core at about a third, or an efficiency core flat out (the research read 389 mW at 50%). */
export const ENERGY_HIGH_MW = 1000;
/** A performance core flat out: the research's one core busy loop read 3,106 mW. */
export const ENERGY_VERY_HIGH_MW = 3000;

export type Tint = "warn" | "danger" | null;

const past = (value: number, high: number, veryHigh: number): Tint => (value > veryHigh ? "danger" : value > high ? "warn" : null);

export function memoryTint(bytes: number): Tint {
  return past(bytes, MEMORY_HIGH, MEMORY_VERY_HIGH);
}
/** wsp's total against what the computer has. */
export function memoryShareTint(bytes: number, total: number | null): Tint {
  return total === null || total <= 0 ? null : past(bytes / total, MEMORY_SHARE_HIGH, MEMORY_SHARE_VERY_HIGH);
}
export function cpuTint(percent: number): Tint {
  return past(percent, CPU_HIGH, CPU_VERY_HIGH);
}
export function energyTint(mw: number | null): Tint {
  return mw === null ? null : past(mw, ENERGY_HIGH_MW, ENERGY_VERY_HIGH_MW);
}

// ---------------------------------------------------------------------------
// The live graphs' history, one point per frame, bounded.
// ---------------------------------------------------------------------------

/** Five minutes of frames at the sampler's two seconds. */
export const HISTORY_POINTS = 150;

export interface HistoryPoint {
  readonly at: number;
  readonly mem: number;
  readonly cpu: number;
  readonly energy: number | null;
}

/** The history with a frame's totals added, the oldest dropped past the cap; a frame already held changes nothing. */
export function pushHistory(history: readonly HistoryPoint[], point: HistoryPoint): readonly HistoryPoint[] {
  if (history.at(-1)?.at === point.at) return history;
  const next = [...history, point];
  return next.length > HISTORY_POINTS ? next.slice(next.length - HISTORY_POINTS) : next;
}

// ---------------------------------------------------------------------------
// Words for figures
// ---------------------------------------------------------------------------

/** Percent of one core, one decimal, as Activity Monitor writes it. */
export const cpuWord = (cpu: number): string => `${cpu.toFixed(1)}%`;
/** Milliwatts, watts with one decimal from 1,000. */
export const energyWord = (mw: number | null): string => (mw === null ? "" : mw >= 1000 ? `${(mw / 1000).toFixed(1)} W` : `${mw > 0 && mw < 1 ? mw.toFixed(1) : Math.round(mw)} mW`);
export const portsWord = (ports: readonly ActivityPort[]): string => (ports.length === 0 ? "" : ports.length === 1 ? `:${ports[0]!.port}` : `:${ports[0]!.port} +${ports.length - 1}`);
