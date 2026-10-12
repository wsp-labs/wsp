// SPDX-License-Identifier: AGPL-3.0-only
// Activity's four views over one computer's groups, in the Settings grid: a
// group row folds open onto its processes, each with its kind's glyph, its
// figures inked only past their marks, and its one act while pointed at.
import { memo, useCallback, useMemo, useState, type CSSProperties, type MouseEvent, type ReactNode } from "react";
import { ActivityIcon, AppWindowIcon, ArrowDownIcon, BotIcon, ChevronRightIcon, CircleDashedIcon, CogIcon, ContainerIcon, EllipsisIcon, EthernetPortIcon, FlaskConicalIcon, Globe2Icon, GaugeIcon, HammerIcon, PanelsTopLeftIcon, ServerIcon, TerminalSquareIcon, type LucideIcon } from "lucide-react";
import { agentName } from "@wsp/catalog";
import { fmtBytes, fmtDuration, fmtTokens, fmtUptime } from "@wsp/protocol";
import { HarnessMark } from "../../components/chat/HarnessMark.js";
import { useSubagentModel } from "../../components/threads/SubagentCard.js";
import { LINE_SLOT_CLASS, ThreadStatus } from "../../components/status/ThreadStatus.js";
import type { ThreadStatusInput } from "../../components/status/kinds/index.js";
import { openContextMenu, runAction } from "../../actions/contextMenu.js";
import { resolveActions } from "../../actions/registry.js";
import { Button, DANGER_BUTTON } from "../../components/ui/button.js";
import { Spinner } from "../../components/ui/spinner.js";
import { TONE_TEXT } from "../../lib/tone.js";
import { cn } from "../../lib/utils.js";
import { ACTIVITY_WORDS as W } from "../format.js";
import { ACTIVITY_COLUMNS, ACTIVITY_FLAT_COLUMNS, ACTIVITY_PORT_COLUMNS, GlyphFrame, GridRow } from "../grid.js";
import { CARD_INSET, GLYPH, LIST_TITLE, NOTE } from "../layout.js";
import { CARD_SURFACE } from "../rows.js";
import { FACT, VALUE } from "../format.js";
import { containerActions, groupActions, procActions, type ContainerVerbs, type GroupVerbs, type ProcTarget, type ProcVerbs } from "./actions.js";
import {
  cpuTint,
  cpuWord,
  energyTint,
  energyWord,
  flat,
  groupSpent,
  KIND_WORDS,
  memoryTint,
  portsWord,
  spent,
  totalSpent,
  type ActivityContainer,
  type ActivityGroup,
  type ActivityPort,
  type ActivityProc,
  type ActivitySort,
  type ActivitySubagent,
  type PlacedProc,
  type Spent,
  type ProcKind,
  type Tint,
} from "./model.js";

export const KIND_GLYPHS: Record<ProcKind, LucideIcon> = {
  agent: BotIcon,
  tool: ServerIcon,
  dev: EthernetPortIcon,
  test: FlaskConicalIcon,
  build: HammerIcon,
  slate: PanelsTopLeftIcon,
  terminal: TerminalSquareIcon,
  tab: Globe2Icon,
  app: AppWindowIcon,
  wsp: CogIcon,
  other: CircleDashedIcon,
};

/** Each kind as the Kinds view heads its group. */
const KIND_HEADS: Record<ProcKind, string> = {
  agent: "Agents",
  tool: "Tool servers",
  dev: "Dev servers",
  test: "Test runs",
  build: "Builds",
  slate: "Slate runs",
  terminal: "Terminals",
  tab: "Browser tabs",
  app: "The app",
  wsp: "wsp",
  other: "Other",
};

/** Warn and danger take the design's caution and error inks; a figure under its high mark keeps the row's own. */
export const TINT_INK: Record<Exclude<Tint, null>, string> = { warn: TONE_TEXT.warning, danger: TONE_TEXT.danger };
const inked = (tint: Tint): string | undefined => (tint === null ? undefined : TINT_INK[tint]);

/** Name, memory, CPU, energy, the act, and the fold or the menu. Below 768 px the name, memory and the last stand. */
const TREE = ACTIVITY_COLUMNS;
const FLAT = ACTIVITY_FLAT_COLUMNS;
const PORTS = ACTIVITY_PORT_COLUMNS;
const WIDE = "narrow:hidden";
const NUM = cn(VALUE, "min-w-0 truncate text-right");
const LIST = cn(CARD_SURFACE, "flex flex-col [&>*+*]:border-t [&>*+*]:border-border/50");
const HEAD = cn("mb-4 grid min-h-7 items-end gap-x-4 border-x border-transparent text-sm leading-5", CARD_INSET);
const HEAD_NAME = "-ml-[calc(var(--settings-inset,20px)+1px)] min-w-0 truncate whitespace-nowrap font-normal text-foreground/70";
const SUB_ROW = cn("group/row grid items-center gap-x-4 py-2.5 transition-colors duration-150 hover:bg-accent/60", CARD_INSET);
/** A row's act stands while the row is pointed at or focused. */
const ACT = "flex justify-end gap-1.5 opacity-0 transition-opacity duration-150 group-hover/row:opacity-100 group-focus-within/row:opacity-100 narrow:hidden";
/** The row's menu: shown while the row is pointed at, and always below 768 px, where it is the only road to the acts. */
const MENU = "flex justify-end opacity-0 transition-opacity duration-150 group-hover/row:opacity-100 group-focus-within/row:opacity-100 narrow:opacity-100";
const MENU_BUTTON = "-mx-1 flex size-5 items-center justify-center rounded text-muted-foreground transition-colors duration-150 hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
/** The acts a row shows inline: a process's End, a working agent's Stop or a kept one's End; a port's Open beside its End. */
const TREE_ACTS: readonly string[] = ["stop", "end", "free"];
const PORT_ACTS: readonly string[] = ["open-port", "end"];

/** Kinds whose word says nothing beside the name or the group: an agent is named, wsp's parts stand under wsp. */
export const UNTAGGED: ReadonlySet<ProcKind> = new Set(["agent", "other", "wsp", "app"]);

/** What a row's acts reach; the page holds the dialogs, the signals and where a browser tab opens. */
export interface Acts extends ProcVerbs, GroupVerbs, ContainerVerbs {
  /** The workspace a browser tab at a port of this group's opens in, where one is known. */
  workspaceOf(group: ActivityGroup | null): string | undefined;
}

const targetOf = (p: ActivityProc, group: ActivityGroup | null, acts: Acts, port?: ActivityPort): ProcTarget => {
  const workspaceId = acts.workspaceOf(group);
  return { p, group, ...(port === undefined ? {} : { port }), ...(workspaceId === undefined ? {} : { workspaceId }) };
};

export interface Look {
  readonly sort: ActivitySort;
  readonly onSort: (s: ActivitySort) => void;
  readonly energy: boolean;
}

function SortCell({ k, word, look, wide = false }: { k: ActivitySort; word: string; look: Look; wide?: boolean }) {
  const on = look.sort === k;
  return (
    <button
      type="button"
      aria-pressed={on}
      data-sort={k}
      onClick={() => look.onSort(k)}
      className={cn("flex items-center justify-end gap-1 whitespace-nowrap text-sm leading-5 font-normal transition-colors duration-150 hover:text-foreground", on ? "text-foreground" : "text-muted-foreground", wide && WIDE)}
    >
      {on ? <ArrowDownIcon aria-hidden className="size-3" /> : null}
      {word}
    </button>
  );
}

/** A row's figures; `tinted` false for a sum across threads, which says nothing past one thread's marks. */
function Figures({ mem, cpu, energy, look, tinted = true }: { mem: number; cpu: number; energy: number | null; look: Look; tinted?: boolean }) {
  return (
    <>
      <span data-col="mem" className={cn(NUM, tinted && inked(memoryTint(mem)))}>
        {fmtBytes(mem)}
      </span>
      <span data-col="cpu" className={cn(NUM, WIDE, tinted && inked(cpuTint(cpu)))}>
        {cpuWord(cpu)}
      </span>
      <span data-col="energy" className={cn(NUM, WIDE, tinted && inked(energyTint(energy)))}>
        {look.energy ? energyWord(energy) : ""}
      </span>
    </>
  );
}

function ProcGlyph({ p }: { p: ActivityProc }) {
  if (p.harness !== undefined) return <HarnessMark harness={p.harness} label={agentName(p.harness)} className="mt-0.5 size-4 shrink-0" />;
  const Glyph = KIND_GLYPHS[p.kind];
  return <Glyph aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />;
}

/** The acts a row shows inline, from its registry; an act after which the process does not come back takes the danger
 * ink under the pointer. A process being ended shows the spinner in their place. */
function ProcAct({ target, acts, inline = TREE_ACTS }: { target: ProcTarget; acts: Acts; inline?: readonly string[] }) {
  if (acts.ending.has(target.p.pid)) return <Spinner className="size-3.5 text-muted-foreground" />;
  return (
    <>
      {resolveActions(procActions, target, acts)
        .filter(a => inline.includes(a.id) && a.rowLabel !== null)
        .map(a => (
          <Button key={a.id} variant="outline" size="xs" data-act={a.id} className={a.destructive ? DANGER_BUTTON : undefined} onClick={() => void runAction(a)}>
            {a.rowLabel}
          </Button>
        ))}
    </>
  );
}

const openMenu = (event: MouseEvent<HTMLElement>, target: ProcTarget, acts: Acts): void => void openContextMenu(event, resolveActions(procActions, target, acts));

/** The row's menu: every act it takes, the copies and Open thread among them. */
function RowMenu({ target, acts }: { target: ProcTarget; acts: Acts }) {
  if (target.p.heading === true) return <span />;
  return (
    <span className={MENU}>
      <button type="button" aria-label={W.more} data-act="menu" onClick={e => openMenu(e, target, acts)} className={MENU_BUTTON}>
        <EllipsisIcon aria-hidden className="size-3.5" />
      </button>
    </span>
  );
}

/** What a process row draws and acts on, as one string: the figures as their words, so a reading that moves none of
 * them draws nothing again, and the start and identity an End sends. */
const procSig = (p: ActivityProc): string =>
  `${p.pid}:${p.startedAt ?? ""}:${p.id ?? ""}:${p.name}:${p.kind}:${fmtBytes(p.mem)}:${cpuWord(p.cpu)}:${energyWord(p.energy)}:${portsWord(p.ports)}:${p.command}:${p.harness ?? ""}:${p.needed === true}:${p.endable === true}:${p.heading === true}`;

interface ProcRowProps {
  readonly p: ActivityProc;
  readonly depth: number;
  readonly group: ActivityGroup | null;
  readonly acts: Acts;
  readonly look: Look;
}

/** One process under a group: its glyph, its name in the sub row's ink, its kind and ports as tags, then its figures.
 * Drawn again only when its words, its acts or its thread moved. */
const ProcRow = memo(ProcRowView, (a, b) => procSig(a.p) === procSig(b.p) && a.depth === b.depth && a.acts === b.acts && a.look === b.look && a.group?.id === b.group?.id && a.group?.thread === b.group?.thread);

function ProcRowView({ p, depth, group, acts, look }: ProcRowProps) {
  const ending = acts.ending.has(p.pid);
  const heading = p.heading === true;
  const tag = ending ? W.ending : UNTAGGED.has(p.kind) || heading ? undefined : KIND_WORDS[p.kind];
  const target = targetOf(p, group, acts);
  return (
    <div
      data-proc-row={heading ? `${p.kind}-heading` : p.pid}
      data-kind={p.kind}
      title={heading ? undefined : p.needed === true ? W.needed : p.command}
      onContextMenu={heading ? undefined : e => openMenu(e, target, acts)}
      className={cn(SUB_ROW, TREE)}
    >
      <span className="flex min-w-0 items-start gap-2 ps-[calc(2.75rem+var(--depth)*1.25rem)] narrow:ps-[calc(0.75rem+var(--depth)*0.75rem)]" style={{ "--depth": depth } as CSSProperties}>
        <ProcGlyph p={p} />
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{heading ? W.slate(p.name) : p.name}</span>
          {tag === undefined ? null : <span className="min-w-0 shrink-[3] truncate text-xs text-muted-foreground max-sm:hidden">{tag}</span>}
          {p.ports.length === 0 ? null : <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{portsWord(p.ports)}</span>}
        </span>
      </span>
      <Figures mem={p.mem} cpu={p.cpu} energy={p.energy} look={look} />
      <span className={ending ? cn("flex justify-end", WIDE) : ACT}>
        <ProcAct target={target} acts={acts} />
      </span>
      <RowMenu target={target} acts={acts} />
    </div>
  );
}

function procRows(procs: readonly ActivityProc[], depth: number, group: ActivityGroup, acts: Acts, look: Look): ReactNode[] {
  return procs.flatMap(p => [<ProcRow key={p.heading === true ? `${p.kind}-heading` : p.pid} p={p} depth={depth} group={group} acts={acts} look={look} />, ...procRows(p.children, depth + 1, group, acts, look)]);
}

/** One of a thread's subagents under it, beside its agent and not under it: its title, then its state, how long it ran,
 * what it spent and its model as tags. It holds no memory or CPU of its own and takes no act. */
function ActivitySubagentRow({ s, harness, workspaceId }: { s: ActivitySubagent; harness: string; workspaceId: string }) {
  const model = useSubagentModel(s.model ?? undefined, harness, workspaceId);
  const tags = [W.subagentStates[s.state], fmtDuration(s.ms), ...(s.tokens === null ? [] : [W.tokens(fmtTokens(s.tokens))]), ...(s.model === null || model === null ? [] : [model])];
  return (
    <div data-subagent-row={s.id} data-state={s.state} title={W.subagentNote} className={cn(SUB_ROW, TREE)}>
      <span className="flex min-w-0 items-start gap-2 ps-[calc(2.75rem+var(--nest)*1.25rem)] narrow:ps-[calc(0.75rem+var(--nest)*0.75rem)]">
        <BotIcon aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{s.title}</span>
          <span className="shrink-0 text-xs text-muted-foreground max-sm:hidden">{W.subagent}</span>
          <span data-subagent-facts className="flex min-w-0 shrink-[3] items-baseline gap-2 overflow-hidden text-xs tabular-nums text-muted-foreground">
            {tags.map(tag => (
              <span key={tag} className="shrink-0 whitespace-nowrap">
                {tag}
              </span>
            ))}
          </span>
        </span>
      </span>
      <span data-col="mem" />
      <span data-col="cpu" className={WIDE} />
      <span data-col="energy" className={WIDE} />
      <span className={WIDE} />
      <span />
    </div>
  );
}

/** One container under its group: its image, how long it has run and, under No thread, its compose folder as tags, then
 * the memory and CPU of its own cgroup, and End, which stops it and keeps its data. */
function ContainerRow({ c, loose, acts }: { c: ActivityContainer; loose: boolean; acts: Acts }) {
  const stopping = acts.stopping.has(c.id);
  const facts = [c.image, ...(c.upMs === null ? [] : [W.up(fmtUptime(c.upMs))]), ...(loose && c.folder !== null ? [c.folder] : [])].filter(f => f !== "");
  const menu = (e: MouseEvent<HTMLElement>): void => void openContextMenu(e, resolveActions(containerActions, c, acts));
  const note = !loose ? (c.placedBy === "compose" ? W.containerNotes.compose : W.containerNotes.label) : c.placedBy === "label" ? W.containerNotes.gone : c.placedBy === "compose" ? W.containerNotes.goneCompose : W.containerNotes.loose;
  return (
    <div data-container-row={c.id} title={note} onContextMenu={menu} className={cn(SUB_ROW, TREE)}>
      <span className="flex min-w-0 items-start gap-2 ps-[calc(2.75rem+var(--nest)*1.25rem)] narrow:ps-[calc(0.75rem+var(--nest)*0.75rem)]">
        <ContainerIcon aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{c.name}</span>
          <span className="shrink-0 text-xs text-muted-foreground max-sm:hidden">{stopping ? W.ending : W.container}</span>
          <span data-container-facts className="flex min-w-0 shrink-[3] items-baseline gap-2 overflow-hidden text-xs tabular-nums text-muted-foreground">
            {facts.map((fact, i) => (
              <span key={fact} className={cn("min-w-0 truncate", i > 0 && "narrow:hidden")}>
                {fact}
              </span>
            ))}
          </span>
        </span>
      </span>
      <span data-col="mem" className={cn(NUM, c.mem !== null && inked(memoryTint(c.mem)))}>
        {c.mem === null ? "" : fmtBytes(c.mem)}
      </span>
      <span data-col="cpu" className={cn(NUM, WIDE, c.cpu !== null && inked(cpuTint(c.cpu)))}>
        {c.cpu === null ? "" : cpuWord(c.cpu)}
      </span>
      <span data-col="energy" className={WIDE} />
      <span className={stopping ? cn("flex justify-end", WIDE) : ACT}>
        {stopping ? (
          <Spinner className="size-3.5 text-muted-foreground" />
        ) : c.owned ? (
          <Button variant="outline" size="xs" data-act="end-container" className={DANGER_BUTTON} onClick={() => acts.endContainer(c)}>
            {W.end}
          </Button>
        ) : null}
      </span>
      <span className={MENU}>
        <button type="button" aria-label={W.more} data-act="menu" onClick={menu} className={MENU_BUTTON}>
          <EllipsisIcon aria-hidden className="size-3.5" />
        </button>
      </span>
    </div>
  );
}

/** The sampler's own row, last under wsp: what reading this page costs the daemon. */
export interface SamplerCost {
  readonly cpu: number;
  readonly held: number;
}

function SamplerRow({ cost, look }: { cost: SamplerCost; look: Look }) {
  return (
    <div data-proc-row="sampler" title={W.samplerNote} className={cn(SUB_ROW, TREE)}>
      <span className="flex min-w-0 items-start gap-2 ps-11 narrow:ps-3">
        <GaugeIcon aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{W.sampler}</span>
          <span className="min-w-0 shrink-[3] truncate text-xs text-muted-foreground max-sm:hidden">{W.samplerNote}</span>
        </span>
      </span>
      <Figures mem={cost.held} cpu={cost.cpu} energy={null} look={look} />
      <span className={WIDE} />
      <span />
    </div>
  );
}

/** A head's menu, which a thread's carries: right click at every width, and below 768 px a button by its name, where
 * the act cell stands down. */
interface HeadMenu {
  readonly group: ActivityGroup;
  readonly acts: Acts;
}

const openHeadMenu = (event: MouseEvent<HTMLElement>, menu: HeadMenu): void => void openContextMenu(event, resolveActions(groupActions, menu.group, menu.acts));

/** A group's head: its name and note, and `total`, the figures it draws, a lead's holding the threads folded under it. */
function GroupHead({ glyph, name, status, note, total: t, open, onOpen, act, menu, look, id, tinted = true }: { glyph: ReactNode; name: string; status?: ReactNode; note: string; total: Spent; open: boolean; onOpen: () => void; act?: ReactNode; menu?: HeadMenu; look: Look; id: string; tinted?: boolean }) {
  return (
    <div className="group/row">
      <GridRow columns={TREE} open={onOpen} {...(menu === undefined ? {} : { onContextMenu: e => openHeadMenu(e, menu) })} attrs={{ "aria-expanded": String(open), "data-group": id }}>
        <span className="flex min-w-0 items-center gap-3 ps-[calc(var(--nest)*1.25rem)] narrow:ps-[calc(var(--nest)*0.75rem)]">
          <GlyphFrame>{glyph}</GlyphFrame>
          <span className="flex min-w-0 flex-col">
            <span className="flex min-w-0 items-center gap-2">
              <span data-grid-name className={cn(LIST_TITLE, "min-w-0 truncate")}>
                {name}
              </span>
              {status}
              {menu === undefined ? null : (
                <button type="button" aria-label={W.more} data-act="head-menu" onClick={e => openHeadMenu(e, menu)} className={cn(MENU_BUTTON, "mx-0 hidden shrink-0 narrow:flex")}>
                  <EllipsisIcon aria-hidden className="size-3.5" />
                </button>
              )}
            </span>
            <span className={cn(NOTE, "truncate")}>{note}</span>
          </span>
        </span>
        <Figures mem={t.mem} cpu={t.cpu} energy={t.energy} look={look} tinted={tinted} />
        <span className={ACT}>{act}</span>
        <ChevronRightIcon aria-hidden className={cn("size-3.5 shrink-0 justify-self-end text-muted-foreground transition-transform duration-150", open && "rotate-90")} />
      </GridRow>
    </div>
  );
}

export interface ThreadsProps {
  readonly name: string;
  readonly groups: readonly ActivityGroup[];
  readonly statusOf: (threadId: string) => ThreadStatusInput | undefined;
  readonly ageOf: (threadId: string) => string | undefined;
  readonly box: boolean;
  readonly cost: SamplerCost | null;
  readonly filtering: boolean;
  readonly acts: Acts;
  readonly look: Look;
  /** The groups drawn open before any press; none on the page. */
  readonly opened?: readonly string[];
}

/** What a group draws, as one string of the words it draws: a frame that moved none of them leaves the group's block as
 * it was. The head's totals are over `under` too, the threads folded under it, each group by its own figures. */
const groupSig = (g: ActivityGroup, under: readonly ActivityGroup[]): string => {
  const t = totalSpent([g, ...under]);
  return (
    `${fmtBytes(t.mem)}:${cpuWord(t.cpu)}:${energyWord(t.energy)}|${g.memory === undefined ? "" : fmtBytes(g.memory)}|` +
    g.procs.flatMap(flat).map(procSig).join("|") +
    `|${g.subagents.map(s => `${s.id}:${s.title}:${s.state}:${fmtDuration(s.ms)}:${s.tokens}:${s.model}`).join("|")}|${g.thread?.working ?? ""}|${g.thread?.sessionId ?? ""}|${g.thread?.title ?? ""}|${g.thread?.slate ?? ""}` +
    `|${g.containers.map(c => `${c.id}:${c.name}:${c.image}:${c.upMs === null ? "" : fmtUptime(c.upMs)}:${c.mem === null ? "" : fmtBytes(c.mem)}:${c.cpu === null ? "" : cpuWord(c.cpu)}:${c.folder}:${c.placedBy}:${c.owned}`).join("|")}`
  );
};

interface BlockProps {
  readonly g: ActivityGroup;
  /** The threads folded under it, at every depth, whose figures its head's totals hold. */
  readonly under: readonly ActivityGroup[];
  /** How many leads it stands under. */
  readonly nest: number;
  readonly sig: string;
  readonly opened: boolean;
  readonly toggle: (id: string) => void;
  readonly status: ThreadStatusInput | undefined;
  readonly age: string | undefined;
  readonly box: boolean;
  readonly cost: SamplerCost | null;
  readonly filtering: boolean;
  readonly acts: Acts;
  readonly look: Look;
}

/** One group and, folded open, its rows; drawn again only when what it shows moved. */
const GroupBlock = memo(
  function GroupBlock({ g, under, nest, opened, toggle, status, age, box, cost, filtering, acts, look }: BlockProps) {
    if (g.kind === "wsp")
      return (
        <div data-activity-group="wsp" style={{ "--nest": 0 } as CSSProperties}>
          <GroupHead id={g.id} glyph={<ActivityIcon aria-hidden className={GLYPH} />} name={W.wsp} note={box ? W.wspNoteBox : W.wspNote} total={groupSpent(g)} open={opened} onOpen={() => toggle(g.id)} look={look} />
          {opened ? procRows(g.procs, 0, g, acts, look) : null}
          {opened && cost !== null && !filtering ? <SamplerRow cost={cost} look={look} /> : null}
        </div>
      );
    if (g.kind === "loose")
      return (
        <div data-activity-group="loose" style={{ "--nest": 0 } as CSSProperties}>
          <GroupHead id={g.id} glyph={<CircleDashedIcon aria-hidden className={GLYPH} />} name={W.loose} note={W.looseNote} total={groupSpent(g)} open={opened} onOpen={() => toggle(g.id)} look={look} />
          {opened ? procRows(g.procs, 0, g, acts, look) : null}
          {opened ? g.containers.map(c => <ContainerRow key={c.id} c={c} loose acts={acts} />) : null}
        </div>
      );
    const t = g.thread!;
    return (
      <div data-activity-group={t.id} style={{ "--nest": nest } as CSSProperties}>
        <GroupHead
          id={g.id}
          glyph={<HarnessMark harness={t.harness} label={agentName(t.harness)} className="size-4" />}
          name={t.title}
          {...(status === undefined ? {} : { status: <ThreadStatus thread={status} {...(age === undefined ? {} : { age })} still className={LINE_SLOT_CLASS} /> })}
          note={t.project}
          total={totalSpent([g, ...under])}
          open={opened}
          onOpen={() => toggle(g.id)}
          act={
            g.procs.length === 0 && !g.containers.some(c => c.owned) ? undefined : (
              <Button variant="outline" size="xs" data-act="end-all" className={DANGER_BUTTON} onClick={() => acts.endAll(g)}>
                {W.endAll}
              </Button>
            )
          }
          menu={{ group: g, acts }}
          look={look}
        />
        {opened ? procRows(g.procs, nest, g, acts, look) : null}
        {opened ? g.subagents.map(s => <ActivitySubagentRow key={s.id} s={s} harness={t.harness} workspaceId={t.workspaceId} />) : null}
        {opened ? g.containers.map(c => <ContainerRow key={c.id} c={c} loose={false} acts={acts} />) : null}
      </div>
    );
  },
  (a, b) =>
    a.sig === b.sig &&
    a.nest === b.nest &&
    a.opened === b.opened &&
    a.acts === b.acts &&
    a.look === b.look &&
    a.box === b.box &&
    a.filtering === b.filtering &&
    a.age === b.age &&
    JSON.stringify(a.status) === JSON.stringify(b.status) &&
    (a.g.kind !== "wsp" || (a.cost?.cpu === b.cost?.cpu && a.cost?.held === b.cost?.held)),
);

const NONE_OPEN: readonly string[] = [];

/** Each thread group under the lead that started it, where the lead has a group too; the rest stand at the top. */
function nestGroups(groups: readonly ActivityGroup[]): { tops: ActivityGroup[]; kids: ReadonlyMap<string, ActivityGroup[]> } {
  const shown = new Set(groups.map(g => g.id));
  const leadOf = (g: ActivityGroup): string | undefined => {
    const lead = g.thread?.parent;
    return lead !== undefined && shown.has(lead) && lead !== g.id ? lead : undefined;
  };
  const tops: ActivityGroup[] = [];
  const kids = new Map<string, ActivityGroup[]>();
  for (const g of groups) {
    const lead = leadOf(g);
    // A lead whose chain comes back to it stands at the top, so no group hides under itself.
    const seen = new Set([g.id]);
    let up = lead;
    while (up !== undefined && !seen.has(up)) {
      seen.add(up);
      up = leadOf(groups.find(x => x.id === up)!);
    }
    if (lead === undefined || up !== undefined) tops.push(g);
    else (kids.get(lead) ?? kids.set(lead, []).get(lead)!).push(g);
  }
  return { tops, kids };
}

/** The Threads view: wsp's own parts, each thread heaviest first with the threads it started folded under it, then No
 * thread. A filter opens every group it left. */
export function ThreadsTable({ name, groups, statusOf, ageOf, box, cost, filtering, acts, look, opened = NONE_OPEN }: ThreadsProps) {
  const [open, setOpen] = useState<readonly string[]>(opened);
  const toggle = useCallback((id: string) => setOpen(o => (o.includes(id) ? o.filter(x => x !== id) : [...o, id])), []);
  const { tops, kids } = useMemo(() => nestGroups(groups), [groups]);
  const below = (id: string): ActivityGroup[] => (kids.get(id) ?? []).flatMap(k => [k, ...below(k.id)]);
  const block = (g: ActivityGroup, nest: number): ReactNode => {
    const under = below(g.id);
    const isOpen = filtering || open.includes(g.id);
    const drawn = (
      <GroupBlock
        key={g.id}
        g={g}
        under={under}
        nest={nest}
        sig={groupSig(g, under)}
        opened={isOpen}
        toggle={toggle}
        status={g.thread === undefined ? undefined : statusOf(g.thread.id)}
        age={g.thread === undefined ? undefined : ageOf(g.thread.id)}
        box={box}
        cost={cost}
        filtering={filtering}
        acts={acts}
        look={look}
      />
    );
    const mine = kids.get(g.id);
    if (mine === undefined) return drawn;
    return (
      <div key={g.id} data-activity-lead={g.id}>
        {drawn}
        {isOpen ? mine.map(k => block(k, nest + 1)) : null}
      </div>
    );
  };
  return (
    <div data-grid="activity" className="flex flex-col">
      <div data-grid-head className={cn(HEAD, TREE)}>
        <span className={HEAD_NAME}>{name}</span>
        <SortCell k="mem" word={W.memory} look={look} />
        <SortCell k="cpu" word={W.cpu} look={look} wide />
        {look.energy ? <SortCell k="energy" word={W.energy} look={look} wide /> : <span className={WIDE} />}
        <span className={WIDE} />
        <span />
      </div>
      <div className={LIST}>
        {tops.map(g => block(g, 0))}
        {groups.length === 0 ? (
          <div className={cn("flex min-h-12 items-center", CARD_INSET)}>
            <span className={NOTE}>{filtering ? W.nothingMatches : W.nothing}</span>
          </div>
        ) : null}
      </div>
    </div>
  );
}

/** Whether two readings of a process draw the same row. */
const sameProc = (a: ActivityProc, b: ActivityProc): boolean =>
  a.pid === b.pid && a.name === b.name && a.kind === b.kind && a.mem === b.mem && a.cpu === b.cpu && a.energy === b.energy && a.command === b.command && portsWord(a.ports) === portsWord(b.ports);

/** Where a flat row came from, in the note under its name. */
const whereOf = (group: ActivityGroup | null): string => (group === null ? "" : group.kind === "wsp" ? W.wsp : group.kind === "loose" ? W.loose : group.thread!.title);

const FlatRow = memo(FlatRowView, (a, b) => sameProc(a.row.p, b.row.p) && a.row.group?.thread?.working === b.row.group?.thread?.working && a.acts === b.acts && a.look === b.look);

function FlatRowView({ row, acts, look, sub = false }: { row: PlacedProc; acts: Acts; look: Look; sub?: boolean }) {
  const { p, group } = row;
  const outside = group === null;
  const ending = acts.ending.has(p.pid);
  const target = targetOf(p, group, acts);
  return (
    <div data-proc-row={p.pid} data-kind={p.kind} title={p.command} onContextMenu={e => openMenu(e, target, acts)} className={cn(SUB_ROW, FLAT)}>
      <span className={cn("flex min-w-0 items-start gap-2", sub && "ps-11 narrow:ps-3")}>
        <ProcGlyph p={p} />
        <span className="flex min-w-0 flex-col">
          <span className="flex min-w-0 items-baseline gap-2">
            <span className={cn("min-w-0 truncate text-sm leading-5", outside ? "text-muted-foreground" : "text-foreground/80")}>{p.name}</span>
            {outside || sub || UNTAGGED.has(p.kind) ? null : <span className="min-w-0 shrink-[3] truncate text-xs text-muted-foreground max-sm:hidden">{ending ? W.ending : KIND_WORDS[p.kind]}</span>}
            {p.ports.length === 0 ? null : <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{portsWord(p.ports)}</span>}
          </span>
          {outside ? null : <span className="truncate text-xs leading-4 text-muted-foreground">{whereOf(group)}</span>}
        </span>
      </span>
      <span className={cn(FACT, "text-right", WIDE)}>{p.pid}</span>
      <Figures mem={p.mem} cpu={p.cpu} energy={p.energy} look={look} />
      <span className={ending ? cn("flex justify-end", WIDE) : ACT}>
        <ProcAct target={target} acts={acts} />
      </span>
      <RowMenu target={target} acts={acts} />
    </div>
  );
}

function FlatHead({ name, look }: { name: string; look: Look }) {
  return (
    <div data-grid-head className={cn(HEAD, FLAT)}>
      <span className={HEAD_NAME}>{name}</span>
      <span className={cn("text-right text-muted-foreground", WIDE)}>{W.pid}</span>
      <SortCell k="mem" word={W.memory} look={look} />
      <SortCell k="cpu" word={W.cpu} look={look} wide />
      {look.energy ? <SortCell k="energy" word={W.energy} look={look} wide /> : <span className={WIDE} />}
      <span className={WIDE} />
      <span />
    </div>
  );
}

/** The Processes view: every process wsp owns flat, and with Every process the rest of the computer in the muted ink. */
export function ProcessesTable({ name, rows, acts, look, filtering }: { name: string; rows: readonly PlacedProc[]; acts: Acts; look: Look; filtering: boolean }) {
  return (
    <div data-grid="activity-processes" className="flex flex-col">
      <FlatHead name={name} look={look} />
      <div className={LIST}>
        {rows.map(row => (
          <FlatRow key={row.p.pid} row={row} acts={acts} look={look} />
        ))}
        {rows.length === 0 ? <Empty filtering={filtering} /> : null}
      </div>
    </div>
  );
}

function Empty({ filtering }: { filtering: boolean }) {
  return (
    <div className={cn("flex min-h-12 items-center", CARD_INSET)}>
      <span className={NOTE}>{filtering ? W.nothingMatches : W.nothing}</span>
    </div>
  );
}

/** The Kinds view: a group per kind, its processes flat under it, each naming the thread it belongs to. */
export function KindsTable({ name, kinds, acts, look, filtering }: { name: string; kinds: readonly { kind: ProcKind; rows: PlacedProc[] }[]; acts: Acts; look: Look; filtering: boolean }) {
  const [open, setOpen] = useState<readonly ProcKind[]>([]);
  const toggle = (k: ProcKind) => setOpen(o => (o.includes(k) ? o.filter(x => x !== k) : [...o, k]));
  return (
    <div data-grid="activity-kinds" className="flex flex-col">
      <div data-grid-head className={cn(HEAD, TREE)}>
        <span className={HEAD_NAME}>{name}</span>
        <SortCell k="mem" word={W.memory} look={look} />
        <SortCell k="cpu" word={W.cpu} look={look} wide />
        {look.energy ? <SortCell k="energy" word={W.energy} look={look} wide /> : <span className={WIDE} />}
        <span className={WIDE} />
        <span />
      </div>
      <div className={LIST}>
        {kinds.map(({ kind, rows }) => {
          const Glyph = KIND_GLYPHS[kind];
          const opened = filtering || open.includes(kind);
          return (
            <div key={kind} data-activity-kind={kind}>
              <GroupHead id={kind} glyph={<Glyph aria-hidden className={GLYPH} />} name={KIND_HEADS[kind]} note={W.processCount(rows.length)} total={spent(rows.map(r => r.p))} open={opened} onOpen={() => toggle(kind)} look={look} tinted={false} />
              {opened ? rows.map(row => <KindRow key={row.p.pid} row={row} acts={acts} look={look} />) : null}
            </div>
          );
        })}
        {kinds.length === 0 ? <Empty filtering={filtering} /> : null}
      </div>
    </div>
  );
}

function KindRow({ row, acts, look }: { row: PlacedProc; acts: Acts; look: Look }) {
  const { p, group } = row;
  const ending = acts.ending.has(p.pid);
  const target = targetOf(p, group, acts);
  return (
    <div data-proc-row={p.pid} data-kind={p.kind} title={p.command} onContextMenu={e => openMenu(e, target, acts)} className={cn(SUB_ROW, TREE)}>
      <span className="flex min-w-0 items-start gap-2 ps-11 narrow:ps-3">
        <ProcGlyph p={p} />
        <span className="flex min-w-0 items-baseline gap-2">
          <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{p.name}</span>
          <span className="min-w-0 shrink-[3] truncate text-xs text-muted-foreground max-sm:hidden">{ending ? W.ending : whereOf(group)}</span>
          {p.ports.length === 0 ? null : <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{portsWord(p.ports)}</span>}
        </span>
      </span>
      <Figures mem={p.mem} cpu={p.cpu} energy={p.energy} look={look} />
      <span className={ending ? cn("flex justify-end", WIDE) : ACT}>
        <ProcAct target={target} acts={acts} />
      </span>
      <RowMenu target={target} acts={acts} />
    </div>
  );
}

/** The Ports view: every port a process wsp owns listens on, its thread, and who can reach it. */
export function PortsTable({ rows, acts, filtering }: { rows: readonly { port: ActivityPort; p: ActivityProc; group: ActivityGroup }[]; acts: Acts; filtering: boolean }) {
  return (
    <div data-grid="activity-ports" className="flex flex-col">
      <div data-grid-head className={cn(HEAD, PORTS)}>
        <span className={HEAD_NAME}>{W.port}</span>
        <span className="text-muted-foreground">{W.process}</span>
        <span className={cn("text-muted-foreground", WIDE)}>{W.thread}</span>
        <span className={cn("text-muted-foreground", WIDE)}>{W.reach}</span>
        <span className={WIDE} />
        <span />
      </div>
      <div className={LIST}>
        {rows.map(({ port, p, group }) => {
          const target = targetOf(p, group, acts, port);
          return (
          <div key={`${p.pid}:${port.port}`} data-port-row={port.port} title={p.command} onContextMenu={e => openMenu(e, target, acts)} className={cn(SUB_ROW, PORTS)}>
            <span className={VALUE}>{port.port}</span>
            <span className="flex min-w-0 items-center gap-2">
              <ProcGlyph p={p} />
              <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{p.name}</span>
            </span>
            <span className={cn("flex min-w-0 items-center gap-2", WIDE)}>
              {group.thread === undefined ? (
                <span className={cn(NOTE, "truncate")}>{whereOf(group)}</span>
              ) : (
                <>
                  <HarnessMark harness={group.thread.harness} label={agentName(group.thread.harness)} className="size-3.5 shrink-0" />
                  <span className="min-w-0 truncate text-sm text-foreground/80">{group.thread.title}</span>
                </>
              )}
            </span>
            <span className={cn(NOTE, WIDE)}>{port.loopback === null ? "" : port.loopback ? W.loopback : W.everyAddress}</span>
            <span className={acts.ending.has(p.pid) ? cn("flex justify-end", WIDE) : ACT}>
              <ProcAct target={target} acts={acts} inline={PORT_ACTS} />
            </span>
            <RowMenu target={target} acts={acts} />
          </div>
          );
        })}
        {rows.length === 0 ? <Empty filtering={filtering} /> : null}
      </div>
    </div>
  );
}

/** Every process in a group, for End all's count and memory. */
export const groupProcs = (g: ActivityGroup): ActivityProc[] => g.procs.flatMap(flat);
