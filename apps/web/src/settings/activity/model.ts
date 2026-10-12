// SPDX-License-Identifier: AGPL-3.0-only
// What Activity draws from one computer's process frame: wsp's own parts, each
// thread with what it started, what wsp started with no thread known, and the
// rest of the computer. Whose each process is and what it is come from
// placeProcs() in @wsp/protocol, the rule `wsp activity`, settle and delete
// read too; this file names the rows, folds a thread's run into its agent's row
// and holds the slate's heading. Sort, filter, the Kinds and Ports views and the
// tints are pure functions of that, so the page draws and the tests read one rule.
import { agentName, CATALOG_AGENTS } from "@wsp/catalog";
import { ACTIVITY_WORDS } from "../format.js";
import { APP_PROC_WORDS, cgroupMemory, PROC_KINDS, placeProcs, procBins, SCRIPT_RUNTIMES, type AppProc, type ContainerPlacedBy, type ProcContainer, type ProcEntry, type ProcKind, type ProcSnapshot, type SubagentState, type SubagentView } from "@wsp/protocol";

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
  /** The agent its turn's run recorded as it started it, under pid. */
  readonly agentPid?: number;
  readonly cgroup?: string;
  /** The thread that started it, under whose row it folds. */
  readonly parent?: string;
  /** Its slate's title, where this window has read the slate. */
  readonly slate?: string;
  /** Its agent's own subagents of every turn, as its listing carries them. */
  readonly subagents?: readonly SubagentView[];
}

/** One of a thread's subagents as Activity draws it: it runs inside the agent's own process, so it has no figures and
 * no act of its own. */
export interface ActivitySubagent {
  readonly id: string;
  readonly title: string;
  readonly state: SubagentState;
  /** How long it ran, or has run so far. */
  readonly ms: number;
  /** What its own calls read and wrote; null where its agent named none. */
  readonly tokens: number | null;
  readonly model: string | null;
}

/** How long a subagent that ended stays drawn. */
export const SUBAGENT_SHOWN_MS = 60_000;

/** The subagents Activity draws for a thread at `now`: those running and those that ended in the last minute, whichever
 * of its turns started them, since an agent's background subagents outlive the turn. An ended one's time is the run
 * time its agent said where it said one. */
export function subagentsOf(subagents: readonly SubagentView[], now: number): ActivitySubagent[] {
  return subagents.flatMap(s => {
    const end = s.state === "running" ? now : s.endedAt;
    if (end === undefined || now - end > SUBAGENT_SHOWN_MS) return [];
    const tokens = s.usage?.tokens;
    const ms = s.state === "running" ? end - s.startedAt : (s.usage?.durationMs ?? end - s.startedAt);
    return [{ id: s.id, title: s.title, state: s.state, ms: Math.max(0, ms), tokens: tokens === undefined ? null : tokens.input + tokens.output, model: s.model ?? null }];
  });
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
  /** A process a thread or wsp started, which End reaches; never an agent, what wsp needs, or a row only wsp's launch
   * on a Mac places. */
  readonly endable?: boolean;
  /** A heading over processes and no process itself: a thread's slate, its runs under it. It takes no act, and its
   * figures are its runs' sum, which no total counts twice. */
  readonly heading?: boolean;
  readonly children: readonly ActivityProc[];
}

/** One running container on the computer's engine, under the thread the daemon placed it with, or under No thread. Its
 * figures are absent until the daemon read them. */
export interface ActivityContainer {
  readonly id: string;
  readonly name: string;
  readonly image: string;
  /** How long it has run at the time the page was drawn; null where the engine did not say. */
  readonly upMs: number | null;
  readonly mem: number | null;
  readonly cpu: number | null;
  /** Compose's folder for its project, which names one under No thread. */
  readonly folder: string | null;
  readonly placedBy: ContainerPlacedBy | null;
  /** Under the thread a label or note placed it with, the only containers End reaches: one under No thread was started
   * by the person, by a fork's engine, or by a thread wsp no longer knows. */
  readonly owned: boolean;
}

export type GroupKind = "wsp" | "thread" | "loose";

export interface ActivityGroup {
  readonly id: string;
  readonly kind: GroupKind;
  readonly thread?: ActivityThread;
  /** The heads of the group's trees. */
  readonly procs: readonly ActivityProc[];
  /** A thread's subagents drawn now; none for any other group. */
  readonly subagents: readonly ActivitySubagent[];
  /** The containers a thread's docker started, or under No thread those no thread's did. */
  readonly containers: readonly ActivityContainer[];
  /** A box thread's processes' memory as its cgroup counts it, which stands for its rows' sum (`cgroupMemory`). */
  readonly memory?: number;
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
  /** The time the subagents are drawn at; now where left out. */
  readonly now?: number;
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

/** Named with the subcommand they run: cargo test, pnpm dev. */
const TOOLS = new Set(["cargo", "npm", "npx", "pnpm", "yarn", "uv", "go", "bunx"]);
const GENERIC_SCRIPTS = new Set(["bin", "cli", "index", "main"]);
const BUILD_DIRS = new Set(["dist", "lib", "build", "out", "src", "bin", ".bin", "node_modules"]);

/** What a process is called: the script a runtime runs, a tool with its subcommand, or the command. */
export function procName(p: Pick<ProcEntry, "comm" | "cmdline">): string {
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

/** The computer's frame read into groups by `placeProcs`: wsp's parts, each thread, and No thread. A thread is rooted at
 * its own process here, which stands for the agent until the agent's binary shows; on a box, whose pids the app is
 * never told, at the heads of its cgroup. */
export function scopeOf({ snapshot, threads, ports, host, app = NO_APP, now = Date.now() }: ScopeInput): ActivityScope {
  const threadById = new Map(threads.map(t => [t.id, t]));
  const placed = placeProcs({
    procs: snapshot.procs,
    daemon: snapshot.daemon,
    ...(host === undefined ? {} : { host }),
    threads: threads.map(t => {
      const bin = CATALOG_AGENTS.find(a => a.id === t.harness)?.bin;
      return { id: t.id, roots: t.pid === undefined ? [] : [t.pid], ...(t.agentPid !== undefined ? { agents: [t.agentPid] } : {}), ...(t.cgroup !== undefined ? { cgroup: t.cgroup } : {}), ...(bin !== undefined ? { bin } : {}) };
    }),
    agentBins: CATALOG_AGENTS.map(a => a.bin),
    listens: pid => (ports.get(pid)?.length ?? 0) > 0,
    app: new Map([...app].map(([pid, part]) => [pid, part.kind === "tab" || part.kind === "frame" ? "tab" : "app"])),
  });
  const { procs, byPid } = placed;
  const keyOf = (pid: number): string => {
    const w = placed.whose.get(pid);
    return w === undefined ? "" : typeof w === "object" ? `t:${w.thread}` : w;
  };

  const members = new Map<string, ProcEntry[]>();
  const rest: ProcEntry[] = [];
  for (const p of procs) {
    const o = keyOf(p.pid);
    if (o === "") rest.push(p);
    else (members.get(o) ?? members.set(o, []).get(o)!).push(p);
  }

  const energy = procs.some(p => p.energyMw !== undefined);
  const agentByBin = (p: ProcEntry): string | undefined => {
    const bins = procBins(p);
    return CATALOG_AGENTS.find(a => bins.includes(a.bin))?.id;
  };
  const leaf = (p: ProcEntry, children: readonly ActivityProc[]): ActivityProc => {
    const kind = placed.kindOf(p.pid);
    const agentOf = placed.agent.get(p.pid);
    const thread = agentOf === undefined ? undefined : threadById.get(agentOf);
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
      ...(placed.needed(p.pid) ? { needed: true } : {}),
      ...(placed.endable(p.pid) ? { endable: true } : {}),
      children,
    };
  };
  /** A group's trees. Each process its agent's row holds (the run script and shells on the way down, the input pump
   * left in its group) is no row of its own: its figures join the agent's, and what it started hangs under the agent. */
  const forest = (list: readonly ProcEntry[]): ActivityProc[] => {
    const inside = new Set(list.map(p => p.pid));
    const into = (pid: number): number | undefined => {
      const at = placed.foldsInto.get(pid);
      return at !== undefined && inside.has(at) ? at : undefined;
    };
    const parentOf = (p: ProcEntry): number | undefined => {
      let at = p.ppid !== p.pid && inside.has(p.ppid) ? p.ppid : undefined;
      for (let n = 0; at !== undefined && n <= list.length; n++) {
        const agent = into(at);
        if (agent === undefined) return at;
        if (agent !== p.pid) return agent;
        const q = byPid.get(at)!;
        at = q.ppid !== q.pid && inside.has(q.ppid) ? q.ppid : undefined;
      }
      return undefined;
    };
    const kids = new Map<number, ProcEntry[]>();
    const held = new Map<number, ProcEntry[]>();
    const heads: ProcEntry[] = [];
    for (const p of list) {
      const agent = into(p.pid);
      if (agent !== undefined) {
        (held.get(agent) ?? held.set(agent, []).get(agent)!).push(p);
        continue;
      }
      const parent = parentOf(p);
      if (parent === undefined) heads.push(p);
      else (kids.get(parent) ?? kids.set(parent, []).get(parent)!).push(p);
    }
    const build = (p: ProcEntry): ActivityProc => {
      const row = leaf(p, (kids.get(p.pid) ?? []).map(build));
      const folded = held.get(p.pid) ?? [];
      if (folded.length === 0) return row;
      const energies = [row.energy, ...folded.map(q => q.energyMw ?? (energy ? 0 : null))].filter((e): e is number => e !== null);
      return { ...row, mem: folded.reduce((n, q) => n + q.rss, row.mem), cpu: folded.reduce((n, q) => n + q.cpu, row.cpu), energy: energies.length === 0 ? null : energies.reduce((n, e) => n + e, 0) };
    };
    return heads.map(build);
  };
  /** A thread's slate runs under one heading beside its agent, since the host and not the agent starts them. */
  const withSlate = (heads: readonly ActivityProc[], t: ActivityThread): ActivityProc[] => {
    const runs = heads.filter(p => p.kind === "slate");
    if (runs.length === 0) return [...heads];
    const sum = spent(runs);
    const slate: ActivityProc = { pid: -1, name: t.slate ?? "", kind: "slate", mem: sum.mem, cpu: sum.cpu, energy: sum.energy, ports: [], command: "", heading: true, children: runs };
    return [...heads.filter(p => p.kind !== "slate"), slate];
  };

  // A container stands under the thread the daemon named where this page knows that thread, else under No thread.
  const containers = new Map<string, ActivityContainer[]>();
  for (const c of snapshot.containers ?? []) {
    const at = c.thread !== undefined && threadById.has(c.thread) ? `t:${c.thread}` : "loose";
    (containers.get(at) ?? containers.set(at, []).get(at)!).push(containerOf(c, now, at !== "loose"));
  }

  const groups: ActivityGroup[] = [];
  const wsp = members.get("wsp");
  if (wsp !== undefined) groups.push({ id: "wsp", kind: "wsp", procs: forest(wsp), subagents: [], containers: [] });
  for (const t of threads) {
    const mine = members.get(`t:${t.id}`);
    const subagents = subagentsOf(t.subagents ?? [], now);
    const held = containers.get(`t:${t.id}`) ?? [];
    const memory = cgroupMemory(snapshot.cgroups, t.cgroup);
    if (mine !== undefined || subagents.length > 0 || held.length > 0) groups.push({ id: t.id, kind: "thread", thread: t, procs: mine === undefined ? [] : withSlate(forest(mine), t), subagents, containers: held, ...(memory === undefined ? {} : { memory }) });
  }
  const loose = members.get("loose");
  const looseContainers = containers.get("loose") ?? [];
  if (loose !== undefined || looseContainers.length > 0) groups.push({ id: "loose", kind: "loose", procs: loose === undefined ? [] : forest(loose), subagents: [], containers: looseContainers });
  return { groups, rest: rest.map(p => leaf(p, [])), energy };
}

function containerOf(c: ProcContainer, now: number, owned: boolean): ActivityContainer {
  return {
    id: c.id,
    name: c.name,
    image: c.image,
    upMs: c.startedAt === undefined ? null : Math.max(0, now - c.startedAt),
    mem: c.memory ?? null,
    cpu: c.cpu ?? null,
    folder: c.folder ?? null,
    placedBy: c.placedBy ?? null,
    owned,
  };
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

/** A group's figures: its processes' and its containers', whose memory and CPU are their own and no process's. A box
 * thread's processes hold the memory its cgroup counts, where the daemon read it. The count stays its processes'. */
export function groupSpent(g: Pick<ActivityGroup, "procs" | "containers" | "memory">): Spent {
  const p = spent(g.procs);
  return { ...p, mem: g.containers.reduce((n, c) => n + (c.mem ?? 0), g.memory ?? p.mem), cpu: g.containers.reduce((n, c) => n + (c.cpu ?? 0), p.cpu) };
}

/** Every group's figures together, what the page's totals read. */
export function totalSpent(groups: readonly ActivityGroup[]): Spent {
  const each = groups.map(groupSpent);
  const energies = each.flatMap(s => (s.energy === null ? [] : [s.energy]));
  return { mem: each.reduce((n, s) => n + s.mem, 0), cpu: each.reduce((n, s) => n + s.cpu, 0), energy: energies.length === 0 ? null : energies.reduce((n, e) => n + e, 0), count: each.reduce((n, s) => n + s.count, 0) };
}

export type ActivitySort = "mem" | "cpu" | "energy";

const figure = (sort: ActivitySort) => (s: { mem: number; cpu: number; energy: number | null }) => (sort === "mem" ? s.mem : sort === "cpu" ? s.cpu : (s.energy ?? 0));

/** Heaviest first on the column, ties by pid so one frame always lays out the same way. */
export function sortProcs(procs: readonly ActivityProc[], sort: ActivitySort): ActivityProc[] {
  const of = figure(sort);
  return [...procs].sort((a, b) => of(b) - of(a) || a.pid - b.pid).map(p => (p.children.length === 0 ? p : { ...p, children: sortProcs(p.children, sort) }));
}

/** Heaviest first on the column, a container having no energy figure; ties by name. */
export function sortContainers(containers: readonly ActivityContainer[], sort: ActivitySort): ActivityContainer[] {
  const of = (c: ActivityContainer): number => (sort === "mem" ? (c.mem ?? 0) : sort === "cpu" ? (c.cpu ?? 0) : 0);
  return [...containers].sort((a, b) => of(b) - of(a) || a.name.localeCompare(b.name));
}

/** wsp's parts stay first and No thread last; the threads between them heaviest first by their totals. */
export function sortGroups(groups: readonly ActivityGroup[], sort: ActivitySort): ActivityGroup[] {
  const of = figure(sort);
  const rank = (g: ActivityGroup): number => (g.kind === "wsp" ? 0 : g.kind === "thread" ? 1 : 2);
  return [...groups]
    .map(g => ({ g: { ...g, procs: sortProcs(g.procs, sort), containers: sortContainers(g.containers, sort) }, total: of(groupSpent(g)) }))
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

/** What a filter reads a container by: its name, image and folder, and the word its row draws. */
const containerHit = (c: ActivityContainer, needle: string): boolean => [c.name, c.image, c.folder ?? "", ACTIVITY_WORDS.container].some(s => s.toLowerCase().includes(needle));

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

/** The groups a filter and the chips leave. The text matches a process, a subagent's title, a container or its thread's
 * title; a match stays under its group with its parents. A thread or project chip keeps only those threads, and drops
 * wsp's parts and No thread; a kind chip keeps only processes of those kinds, and no subagent or container, neither
 * being a process of a kind. */
export function filterGroups(groups: readonly ActivityGroup[], text: string, picks: ActivityPicks): ActivityGroup[] {
  const needle = text.trim().toLowerCase();
  const narrowed = picks.threads.length > 0 || picks.projects.length > 0;
  return groups.flatMap(g => {
    if (narrowed && (g.thread === undefined || (picks.threads.length > 0 && !picks.threads.includes(g.id)) || (picks.projects.length > 0 && !picks.projects.includes(g.thread.project)))) return [];
    let procs: readonly ActivityProc[] = g.procs;
    let subagents = picks.kinds.length > 0 ? [] : g.subagents;
    let containers = picks.kinds.length > 0 ? [] : g.containers;
    if (picks.kinds.length > 0) procs = prune(procs, p => picks.kinds.includes(p.kind));
    const titled = needle !== "" && g.thread !== undefined && g.thread.title.toLowerCase().includes(needle);
    if (needle !== "" && !titled) {
      procs = prune(procs, p => procHit(p, needle));
      subagents = subagents.filter(s => s.title.toLowerCase().includes(needle));
      containers = containers.filter(c => containerHit(c, needle));
    }
    return procs.length > 0 || subagents.length > 0 || containers.length > 0 || (titled && picks.kinds.length === 0) ? [{ ...g, procs, subagents, containers }] : [];
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
