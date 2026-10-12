// SPDX-License-Identifier: AGPL-3.0-only
// `wsp activity` as Settings > Activity reads: the totals, then wsp's own, each
// thread heaviest first and No thread last, every process by the name the page
// gives it, a turn's run script and input pump folded into its agent's row, the
// figures in aligned columns and nothing wider than the terminal. --wide adds
// the pid and the whole command line and folds nothing.
import { agentName, CATALOG_AGENTS } from "@wsp/catalog";
import { ACTIVITY_GROUP_WORDS, ACTIVITY_KIND_WORDS, activityName, agentByBin, fmtBytes, foldTree, isTurnPlumbing, procBins, UNTAGGED_KINDS, type ActivityProc, type ActivityView, type Folded } from "@wsp/protocol";
import { cellWidth, ellipsize, GUTTER, padCells } from "../init-layout.js";

/** The narrowest a name column is cut to before the kind column goes instead. */
const NAME_MIN = 16;
/** The widest the names and kinds stand, whatever the terminal, so the figures stay beside them. */
const LEFT_MAX = 72;

interface Row {
  /** A group's head, or a process `depth` steps under one. */
  readonly label: string;
  readonly tag: string;
  readonly pid: string;
  readonly mem: string;
  readonly cpu: string;
  readonly command: string;
}

interface Node {
  readonly p: ActivityProc;
  readonly mem: number;
  readonly cpu: number;
  readonly kids: readonly Node[];
}

const asEntry = (p: ActivityProc): { pid: number; comm: string; cmdline: string } => ({ pid: p.pid, comm: p.name, cmdline: p.command });
const agentOf = (p: ActivityProc): string => {
  const id = agentByBin(asEntry(p), CATALOG_AGENTS);
  return id === undefined ? ACTIVITY_KIND_WORDS.agent : agentName(id);
};

const sorted = (list: Node[]): Node[] => list.sort((a, b) => b.mem - a.mem || a.p.pid - b.p.pid);

/** A group's rows as a tree. Folded, every row of the agent's stretch (kind agent) joins the agent's own row, the
 * deepest of it, which takes their figures and what they started; the run's pump and leftover shells, which stand under
 * pid 1, join the group's first agent. The view carries kinds and no fold map, so the map is read off them. */
function treeOf(rows: readonly ActivityProc[], fold: boolean): Node[] {
  const kidsOf = new Map<number, ActivityProc[]>();
  for (const p of rows) if (p.ppid !== p.pid) (kidsOf.get(p.ppid) ?? kidsOf.set(p.ppid, []).get(p.ppid)!).push(p);
  const isAgent = (p: ActivityProc): boolean => p.kind === "agent" && !isTurnPlumbing(p.command) && !(kidsOf.get(p.pid) ?? []).some(k => k.kind === "agent");
  const into = new Map<number, number>();
  if (fold) {
    const below = (p: ActivityProc): ActivityProc | undefined => {
      for (const k of kidsOf.get(p.pid) ?? []) {
        if (k.kind !== "agent") continue;
        const hit = isAgent(k) ? k : below(k);
        if (hit !== undefined) return hit;
      }
      return undefined;
    };
    const first = rows.find(isAgent) ?? rows.find(p => p.kind === "agent");
    for (const p of rows) if (p.kind === "agent" && !isAgent(p) && p !== first) into.set(p.pid, (below(p) ?? first!).pid);
  }
  const node = ({ p, held, kids }: Folded<ActivityProc>): Node => ({
    p,
    mem: held.reduce((n, q) => n + q.memory, p.memory),
    cpu: held.reduce((n, q) => n + q.cpu, p.cpu),
    kids: sorted(kids.map(node)),
  });
  return sorted(foldTree(rows, pid => into.get(pid)).map(node));
}

const cpuWord = (cpu: number): string => `${cpu.toFixed(1)}%`;

function procRows(heads: readonly Node[], wide: boolean, at: { daemon?: number; host?: number }): Row[] {
  const row = (n: Node, depth: number): Row[] => {
    const { p } = n;
    const name = activityName(asEntry(p), { ...at, ...(p.kind === "agent" && (!wide || !isTurnPlumbing(p.command)) ? { agent: agentOf(p) } : {}) });
    const ports = p.ports.length > 0 ? ` :${p.ports.join(" :")}` : "";
    const me: Row = { label: `${"  ".repeat(depth)}${name}${ports}`, tag: UNTAGGED_KINDS.has(p.kind) ? "" : ACTIVITY_KIND_WORDS[p.kind], pid: String(p.pid), mem: fmtBytes(n.mem), cpu: cpuWord(n.cpu), command: wide ? p.command : "" };
    return [me, ...n.kids.flatMap(k => row(k, depth + 1))];
  };
  return heads.flatMap(n => row(n, 1));
}

const head = (label: string, mem: number, cpu: number): Row => ({ label, tag: "", pid: "", mem: fmtBytes(mem), cpu: cpuWord(cpu), command: "" });
const sum = (rows: readonly ActivityProc[]): { mem: number; cpu: number } => ({ mem: rows.reduce((n, p) => n + p.memory, 0), cpu: rows.reduce((n, p) => n + p.cpu, 0) });

/** The read as the page lays it out, no line wider than `width` cells unless `wide`, which cuts no command line. */
export function activityTable(view: ActivityView, o: { width: number; wide: boolean }): string {
  const fold = !o.wide;
  const rows: Row[] = [];
  let mem = 0;
  let cpu = 0;
  let processes = 0;
  const group = (label: string, figures: { mem: number; cpu: number }, procs: readonly Row[]): void => {
    rows.push(head(label, figures.mem, figures.cpu), ...procs);
    mem += figures.mem;
    cpu += figures.cpu;
    processes += procs.length;
  };
  if (view.wsp.length > 0) {
    // The daemon runs the binary of that name; the host, on the computer it runs on, is the other row of kind wsp. A
    // box runs no host, so a group with none is a box's.
    const daemon = view.wsp.find(p => p.kind === "wsp" && procBins(asEntry(p)).includes("wsp-daemon"))?.pid;
    const host = view.wsp.find(p => p.kind === "wsp" && p.pid !== daemon)?.pid;
    const note = host === undefined ? ACTIVITY_GROUP_WORDS.wspNoteBox : ACTIVITY_GROUP_WORDS.wspNote;
    group(`${ACTIVITY_GROUP_WORDS.wsp}  ${note}`, sum(view.wsp), procRows(treeOf(view.wsp, fold), o.wide, { ...(daemon !== undefined ? { daemon } : {}), ...(host !== undefined ? { host } : {}) }));
  }
  const threads = [...view.threads].sort((a, b) => b.memory - a.memory || a.threadId.localeCompare(b.threadId));
  for (const t of threads) group(t.title, { mem: t.memory, cpu: t.cpu }, procRows(treeOf(t.processes, fold), o.wide, {}));
  if (view.unowned.length > 0) group(`${ACTIVITY_GROUP_WORDS.loose}  ${ACTIVITY_GROUP_WORDS.looseNote}`, sum(view.unowned), procRows(treeOf(view.unowned, fold), o.wide, {}));
  if (rows.length === 0) return `no thread runs a process on ${view.computer}`;

  const totals = [view.computer, `${ACTIVITY_GROUP_WORDS.wsp} ${fmtBytes(mem)}`, `CPU ${Math.round(cpu)}%`, `${processes} ${processes === 1 ? "process" : "processes"}`, `${threads.length} ${threads.length === 1 ? "thread" : "threads"}`].join("   ");
  const titles: Row = { label: "", tag: "", pid: o.wide ? "PID" : "", mem: "MEMORY", cpu: "CPU", command: o.wide ? "COMMAND" : "" };
  const all = [titles, ...rows];
  const widest = (list: readonly Row[], f: (r: Row) => string): number => Math.max(0, ...list.map(r => cellWidth(f(r))));
  const [pidW, memW, cpuW] = [widest(all, r => r.pid), widest(all, r => r.mem), widest(all, r => r.cpu)];
  const figures = (o.wide ? pidW + GUTTER.length : 0) + memW + GUTTER.length + cpuW;
  const procs = rows.filter(r => r.pid !== "");
  let tagW = widest(procs, r => r.tag);
  const tagPart = (): number => (tagW > 0 ? GUTTER.length + tagW : 0);
  const room = o.wide ? Infinity : o.width;
  // A group's head spans the name and kind columns rather than sizing them, so one long title does not push every
  // figure to the far edge.
  const left = Math.max(1, Math.min(Math.max(widest(procs, r => r.label) + tagPart(), widest(rows, r => r.label)), LEFT_MAX, room - GUTTER.length - figures));
  if (left - tagPart() < Math.min(NAME_MIN, widest(procs, r => r.label))) tagW = 0;
  const nameW = Math.min(widest(procs, r => r.label), left - tagPart());
  const line = (r: Row): string => {
    const lead = r.pid === "" ? ellipsize(r.label, left) : `${padCells(ellipsize(r.label, nameW), nameW)}${tagW > 0 ? `${GUTTER}${r.tag}` : ""}`;
    const cells = [padCells(lead, left), ...(o.wide ? [r.pid.padStart(pidW)] : []), r.mem.padStart(memW), r.cpu.padStart(cpuW), ...(o.wide && r.command !== "" ? [r.command] : [])];
    return ellipsize(cells.join(GUTTER).trimEnd(), room);
  };
  return [ellipsize(totals, room), "", ...all.map(line)].join("\n");
}
