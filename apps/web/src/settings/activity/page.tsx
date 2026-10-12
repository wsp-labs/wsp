// SPDX-License-Identifier: AGPL-3.0-only
// Settings > Activity: every thread's processes on one computer, with their
// memory, CPU, energy and ports. Opening the page holds that computer's daemon
// link and starts its process watch; leaving it stops the watch, so nothing
// samples while it is closed. The totals carry a line of the last minutes,
// kept here for as long as the page stands and dropped with it.
import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { SearchIcon } from "lucide-react";
import { fmtBytes, folderOnJoined, THREAD_CGROUPS, HERE_PLACE_ID, isProviderPlace, placeOf, workspaceKind, type PlaceView, type WorkspaceView } from "@wsp/protocol";
import { ChartPlot, spanPoints } from "../../components/machine/MachineSurface.js";
import { CHIP } from "../../components/ui/chips.js";
import { Input } from "../../components/ui/input.js";
import { SegmentedControl } from "../../components/ui/segmented-control.js";
import { Select, SelectItem, SelectPopup, SelectTrigger, SelectValue } from "../../components/ui/select.js";
import { Skeleton } from "../../components/ui/skeleton.js";
import { Switch } from "../../components/ui/switch.js";
import { useDaemonWire } from "../../files/wire.js";
import { cn } from "../../lib/utils.js";
import { getLive, watchLive } from "../../machine/live.js";
import { getProcs, type WorkspaceProcs } from "../../machine/procs.js";
import { useAbsentComputer } from "../../protocol/store.js";
import { HERE_KEY } from "../../terminal/computer.js";
import { ComputerGlyph } from "../ComputerGlyph.js";
import { ACTIVITY_WORDS as W } from "../format.js";
import { NOTE, ROW_FIELD, SELECT_WIDTH } from "../layout.js";
import { placeName } from "../places.js";
import type { SettingsCardData } from "../rows.js";
import { CARD_SURFACE } from "../rows.js";
import type { SettingsContext } from "../settingsContext.js";
import { useSettingsStore, type ActivityView } from "../settingsStore.js";
import { SPLIT_INKS, STAT } from "../usage.js";
import { threadLooks, useActivityActs, useActivityScope, useActivityThreads } from "./live.js";
import {
  cpuTint,
  energyTint,
  energyWord,
  filterGroups,
  flat,
  flatRows,
  HISTORY_POINTS,
  KIND_WORDS,
  kindGroups,
  memoryShareTint,
  NO_PICKS,
  portRows,
  pushHistory,
  sortGroups,
  spent,
  totalSpent,
  type ActivityPicks,
  type ActivityScope,
  type ActivitySort,
  type HistoryPoint,
  type ProcKind,
  type Tint,
} from "./model.js";
import { KindsTable, PortsTable, ProcessesTable, ThreadsTable, TINT_INK, type Look, type SamplerCost } from "./tables.js";

const VIEWS: ReadonlyArray<{ value: ActivityView; label: ReactNode }> = (["threads", "processes", "kinds", "ports"] as const).map(value => ({ value, label: W.views[value] }));

/** The page's views, at the top bar's right end; at a phone's width, where four segments would cover the crumbs, the
 * same views in a select. */
export function ActivityTabs() {
  const view = useSettingsStore(s => s.activityView);
  const pick = useSettingsStore(s => s.pickActivityView);
  return (
    <span className="flex items-center [-webkit-app-region:no-drag]">
      <SegmentedControl data-k="activity-views" aria-label={W.view} value={view} segments={VIEWS} onChange={pick} className="h-9 max-sm:hidden" segmentClassName="px-3.5 text-sm" />
      <Select value={view} onValueChange={v => pick(v as ActivityView)}>
        <SelectTrigger size="sm" aria-label={W.view} data-k="activity-views-narrow" className={cn(SELECT_WIDTH, "w-auto gap-2 sm:hidden")}>
          <SelectValue>{(v: ActivityView) => W.views[v]}</SelectValue>
        </SelectTrigger>
        <SelectPopup>
          {VIEWS.map(v => (
            <SelectItem key={v.value} value={v.value}>
              {v.label}
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
    </span>
  );
}

export function activityCards(ctx: SettingsContext): SettingsCardData[] {
  return [{ id: "activity", items: [], body: <ActivityPage ctx={ctx} /> }];
}

/** A computer Activity reads: the one the host runs on over its own link, or a box over a link of one of its folders,
 * since every folder there shares the box's one daemon. */
export interface ActivityComputer {
  readonly place: PlaceView;
  readonly key: string;
  readonly workspaceId: string | null;
}

export function activityComputers(places: readonly PlaceView[], workspaces: readonly WorkspaceView[]): ActivityComputer[] {
  return places.flatMap((place): ActivityComputer[] => {
    if (place.id === HERE_PLACE_ID) return [{ place, key: HERE_KEY, workspaceId: null }];
    if (isProviderPlace(place)) return [];
    const folder = workspaces.find(w => folderOnJoined(workspaceKind(w)) && placeOf(places, w)?.id === place.id);
    return folder === undefined ? [] : [{ place, key: folder.id, workspaceId: folder.id }];
  });
}

function ActivityPage({ ctx }: { ctx: SettingsContext }) {
  const picked = useSettingsStore(s => s.activityPlace);
  const computers = useMemo(() => activityComputers(ctx.places, ctx.workspaces), [ctx.places, ctx.workspaces]);
  const computer = computers.find(c => c.place.id === (picked ?? HERE_PLACE_ID)) ?? computers[0];
  if (computer === undefined) return null;
  return <Scoped key={computer.key} computer={computer} computers={computers} ctx={ctx} />;
}

/** Two seconds, the sampler's cadence, until two frames say what it is. */
const FIRST_INTERVAL_MS = 2000;

function Scoped({ computer, computers, ctx }: { computer: ActivityComputer; computers: readonly ActivityComputer[]; ctx: SettingsContext }) {
  const { key } = computer;
  const here = computer.place.id === HERE_PLACE_ID;
  useDaemonWire(key);
  // On a box the daemon reads only the threads' cgroups each tick, and the whole table once Every process asks for it.
  const holdRef = useRef<ReturnType<WorkspaceProcs["hold"]> | null>(null);
  const scopeRef = useRef<readonly string[] | null>(null);
  useEffect(() => {
    const held = getProcs(key).hold(scopeRef.current);
    holdRef.current = held;
    return () => {
      holdRef.current = null;
      held.release();
    };
  }, [key]);
  const mem = useMemoryInUse(key);
  const absent = useAbsentComputer(computer.workspaceId);
  const { threads, views } = useActivityThreads(computer.place.id, here, ctx);
  const { procs, scope } = useActivityScope(key, here, threads);
  const snapshot = procs.snapshot;

  const view = useSettingsStore(s => s.activityView);
  const [sort, setSort] = useState<ActivitySort>("mem");
  const [filter, setFilter] = useState("");
  const [picks, setPicks] = useState<ActivityPicks>(NO_PICKS);
  const [every, setEvery] = useState(false);
  // The cgroup every thread's own stands under, which the daemon reads down through: however many threads the box has.
  const wholeTable = here || (view === "processes" && every);
  const reads = useMemo(() => (wholeTable ? null : [THREAD_CGROUPS]), [wholeTable]);
  scopeRef.current = reads;
  useEffect(() => holdRef.current?.set(reads), [reads]);

  // One point a frame for the tiles' lines, from the page's open to its close, taken as the frame is drawn so a frame
  // costs one render and not a second one from an effect.
  const [history, setHistory] = useState<{ points: readonly HistoryPoint[]; at: number | null; interval: number }>({ points: [], at: null, interval: FIRST_INTERVAL_MS });
  if (snapshot !== null && scope !== null && snapshot.at !== history.at) {
    const t = totalSpent(scope.groups);
    setHistory(h => ({ points: pushHistory(h.points, { at: snapshot.at, mem: t.mem, cpu: t.cpu, energy: t.energy }), at: snapshot.at, interval: h.at === null ? FIRST_INTERVAL_MS : Math.max(1, snapshot.at - h.at) }));
  }
  const cost: SamplerCost | null = snapshot?.cost === undefined ? null : { cpu: (snapshot.cost.cpuMs / history.interval) * 100, held: snapshot.cost.held };

  const { acts, dialog } = useActivityActs(key, computer.workspaceId, snapshot);

  const energy = scope?.energy ?? false;
  const look: Look = useMemo(() => ({ sort, onSort: setSort, energy }), [sort, energy]);
  const filtering = filter.trim() !== "" || picks.threads.length + picks.kinds.length + picks.projects.length > 0;
  const groups = useMemo(() => (scope === null ? [] : sortGroups(filterGroups(scope.groups, filter, picks), sort)), [scope, filter, picks, sort]);
  const name = placeName(computer.place);
  const unavailable = snapshot === null && absent === null ? procs.unavailable : null;

  return (
    <div data-k="activity" data-activity-at={computer.place.id} className="flex animate-settle-in flex-col gap-10 motion-reduce:animate-none">
      {absent !== null ? null : scope === null ? <TilesSkeleton /> : <Tiles scope={scope} history={history.points} mem={mem} />}
      <Controls computer={computer} computers={computers} filter={filter} onFilter={setFilter} {...(view === "processes" ? { every: { on: every, set: setEvery } } : {})} />
      {scope === null || absent !== null ? null : <Chips scope={scope} picks={picks} onPicks={setPicks} />}
      {absent !== null ? (
        <div data-k="activity-away" className="flex flex-col gap-1 py-2">
          <p className={NOTE}>{absent.said}</p>
          {absent.will === undefined ? null : <p className={NOTE}>{absent.will}</p>}
        </div>
      ) : unavailable !== null ? (
        <p data-k="activity-unavailable" className={NOTE}>
          {unavailable}
        </p>
      ) : scope === null ? (
        <RowsSkeleton />
      ) : view === "threads" ? (
        <ThreadsTable
          name={name}
          groups={groups}
          {...threadLooks(views)}
          box={!here}
          cost={cost}
          filtering={filtering}
          acts={acts}
          look={look}
        />
      ) : view === "processes" ? (
        <ProcessesTable name={name} rows={flatRows(groups, scope.rest, every, sort, filter)} acts={acts} look={look} filtering={filtering} />
      ) : view === "kinds" ? (
        <KindsTable name={name} kinds={kindGroups(groups, sort)} acts={acts} look={look} filtering={filtering} />
      ) : (
        <PortsTable rows={portRows(groups)} acts={acts} filtering={filtering} />
      )}
      {dialog}
    </div>
  );
}

/** What the computer holds in use and in all, from its sys readings: the note under wsp's memory, and the mark its
 * share is inked by. Read as words and a number, so a reading that moves neither draws nothing. */
function useMemoryInUse(key: string): { used: string; total: number } | null {
  const subscribe = useCallback(
    (fn: () => void) => {
      const offs = [getLive(key).onChange(fn), watchLive(key)];
      return () => offs.forEach(off => off());
    },
    [key],
  );
  const used = useSyncExternalStore(subscribe, () => {
    const mem = getLive(key).snapshot().samples.at(-1)?.mem;
    return mem === undefined ? null : fmtBytes(mem.used);
  });
  const total = useSyncExternalStore(subscribe, () => getLive(key).snapshot().samples.at(-1)?.mem.total ?? null);
  return useMemo(() => (used === null || total === null ? null : { used, total }), [used, total]);
}

const TILE_COLUMNS: Record<number, string> = { 3: "grid-cols-3", 4: "grid-cols-4" };
const inked = (tint: Tint): string | undefined => (tint === null ? undefined : TINT_INK[tint]);

/** wsp's totals on this computer, each figure over a line of the last minutes; the process count stands alone. */
function Tiles({ scope, history, mem }: { scope: ActivityScope; history: readonly HistoryPoint[]; mem: { used: string; total: number } | null }) {
  const t = totalSpent(scope.groups);
  const threads = scope.groups.filter(g => g.kind === "thread").length;
  const memTint = memoryShareTint(t.mem, mem?.total ?? null);
  const tiles = [
    <Tile key="mem" k="mem" label={W.wsp} value={fmtBytes(t.mem)} note={mem === null ? W.processes : W.of(mem.used)} tint={memTint} history={history} y={h => h.mem} floor={1024 ** 3} />,
    <Tile key="cpu" k="cpu" label={W.cpu} value={`${Math.round(t.cpu)}%`} note={W.oneCore} tint={cpuTint(t.cpu)} history={history} y={h => h.cpu} floor={100} />,
    ...(scope.energy ? [<Tile key="energy" k="energy" label={W.energy} value={energyWord(t.energy)} note={W.energyNote} tint={energyTint(t.energy)} history={history} y={h => h.energy ?? 0} floor={1000} />] : []),
    <Tile key="procs" k="procs" label={W.processes} value={String(t.count)} note={W.threads(threads)} />,
  ];
  return (
    <div data-k="activity-totals" className={cn(CARD_SURFACE, "grid divide-x divide-border/50 max-sm:grid-cols-2 max-sm:divide-x-0", TILE_COLUMNS[tiles.length])}>
      {tiles}
    </div>
  );
}

/** One total: its label, the figure inked past its mark, a quiet note, and where it has a history, its line. */
function Tile({ k, label, value, note, tint = null, history, y, floor }: { k: string; label: string; value: string; note: string; tint?: Tint; history?: readonly HistoryPoint[]; y?: (h: HistoryPoint) => number; floor?: number }) {
  return (
    <div data-k={`activity-${k}`} className={cn(STAT.cell, "px-5 py-5")}>
      <span className={STAT.label}>{label}</span>
      <span data-tint={tint ?? "none"} className={cn(STAT.figure, "tabular-nums", inked(tint))}>
        {value}
      </span>
      <span className={STAT.note}>{note}</span>
      {history === undefined || y === undefined ? null : <TileLine history={history} y={y} floor={floor ?? 1} label={W.graph(label)} k={k} />}
    </div>
  );
}

/** Each line's ink, the Computers band's and the Usage page's: CPU sky, memory violet, and Power teal, since amber is
 * every theme's warning and a line at rest is past no mark. The figure above a line carries its tint; the line keeps
 * its own ink. */
const LINE_INKS: Record<string, string | undefined> = { cpu: SPLIT_INKS[0], mem: SPLIT_INKS[1], energy: SPLIT_INKS[4] };

/** The last minutes as a line, scaled to the highest point in the window and never below the floor, so a quiet
 * computer reads as low rather than filling the box. */
function TileLine({ history, y, floor, label, k }: { history: readonly HistoryPoint[]; y: (h: HistoryPoint) => number; floor: number; label: string; k: string }) {
  const to = history.at(-1)?.at ?? 0;
  const from = to - (HISTORY_POINTS - 1) * FIRST_INTERVAL_MS;
  const top = Math.max(floor, ...history.map(y)) * 1.15;
  const points = spanPoints(history, from, to, h => y(h) / top);
  return (
    <div data-tile-line={k} data-points={history.length} className="mt-2">
      <ChartPlot runs={[points]} label={label} end={false} height="h-10" className={LINE_INKS[k]} />
    </div>
  );
}

function TilesSkeleton() {
  return (
    <div data-k="activity-totals-loading" className={cn(CARD_SURFACE, "grid grid-cols-4 divide-x divide-border/50 max-sm:grid-cols-2 max-sm:divide-x-0")}>
      {[0, 1, 2, 3].map(i => (
        <div key={i} className="flex flex-col gap-1.5 px-5 py-5">
          <Skeleton className="my-1 h-3 w-16" />
          <Skeleton className="my-0.5 h-7 w-28" />
          <Skeleton className="my-0.5 h-3 w-24" />
          <Skeleton className="mt-2 h-10 w-full" />
        </div>
      ))}
    </div>
  );
}

function RowsSkeleton() {
  return (
    <div data-k="activity-loading" className="flex flex-col">
      <p className={cn(NOTE, "mb-4")}>{W.pending}</p>
      <div className={cn(CARD_SURFACE, "flex flex-col [&>*+*]:border-t [&>*+*]:border-border/50")}>
        {[0, 1, 2].map(i => (
          <div key={i} className="flex min-h-15 items-center gap-3 px-5 py-3">
            <Skeleton className="size-8 rounded-md" />
            <Skeleton className="h-4 w-48" />
          </div>
        ))}
      </div>
    </div>
  );
}

function Controls({ computer, computers, filter, onFilter, every }: { computer: ActivityComputer; computers: readonly ActivityComputer[]; filter: string; onFilter: (v: string) => void; every?: { on: boolean; set: (v: boolean) => void } }) {
  const pick = useSettingsStore(s => s.pickActivityPlace);
  return (
    <div data-k="activity-controls" className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
      <Select value={computer.place.id} onValueChange={id => pick(id as string)}>
        <SelectTrigger size="sm" aria-label={W.computer} data-k="activity-computer" className={cn(SELECT_WIDTH, "w-56 gap-2 max-sm:w-full")}>
          <SelectValue>
            {(id: string) => {
              const shown = computers.find(c => c.place.id === id)?.place;
              return shown === undefined ? null : (
                <span className="flex min-w-0 items-center gap-2">
                  <ComputerGlyph place={shown} className="size-3.5 shrink-0 text-foreground/80" />
                  <span className="truncate">{placeName(shown)}</span>
                </span>
              );
            }}
          </SelectValue>
        </SelectTrigger>
        <SelectPopup>
          {computers.map(c => (
            <SelectItem key={c.place.id} value={c.place.id}>
              <span className="flex min-w-0 items-center gap-2">
                <ComputerGlyph place={c.place} className="size-4 text-foreground/80" />
                {placeName(c.place)}
              </span>
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
      <span className="flex items-center gap-4 max-sm:w-full">
        {every === undefined ? null : (
          <label className={cn(NOTE, "flex shrink-0 items-center gap-2")}>
            <Switch data-k="activity-every" checked={every.on} onCheckedChange={every.set} />
            {W.every}
          </label>
        )}
        <span className="relative flex w-64 items-center max-sm:w-full">
          <SearchIcon aria-hidden className="pointer-events-none absolute left-2.5 z-10 size-3.5 text-muted-foreground" />
          <Input data-k="activity-filter" value={filter} onChange={e => onFilter(e.target.value)} placeholder={W.filter} aria-label={W.filter} className={cn(ROW_FIELD, "w-full [&_input]:pl-8")} />
        </span>
      </span>
    </div>
  );
}

/** The thread, kind and project chips: each a press that narrows the view to it, pressed again to let it go. A set with
 * one choice says nothing and is not drawn. */
function Chips({ scope, picks, onPicks }: { scope: ActivityScope; picks: ActivityPicks; onPicks: (p: ActivityPicks) => void }) {
  const owned = scope.groups.flatMap(g => g.procs.flatMap(flat));
  const kinds = [...new Set(owned.map(p => p.kind))];
  const threadGroups = scope.groups.filter(g => g.thread !== undefined);
  const projects = [...new Set(threadGroups.map(g => g.thread!.project))];
  const toggle = <K extends keyof ActivityPicks>(k: K, v: ActivityPicks[K][number]): void => {
    const list = picks[k] as readonly string[];
    onPicks({ ...picks, [k]: list.includes(v) ? list.filter(x => x !== v) : [...list, v] });
  };
  const sets: { k: keyof ActivityPicks; word: string; items: { v: string; text: string; glyph?: ReactNode }[] }[] = [
    { k: "kinds", word: W.chips.kinds, items: kinds.map(v => ({ v, text: KIND_WORDS[v as ProcKind] })) },
    { k: "projects", word: W.chips.projects, items: projects.map(v => ({ v, text: v })) },
    { k: "threads", word: W.chips.threads, items: threadGroups.map(g => ({ v: g.id, text: g.thread!.title })) },
  ];
  const drawn = sets.filter(s => s.items.length > 1 || (picks[s.k] as readonly string[]).length > 0);
  if (drawn.length === 0) return null;
  return (
    <div data-k="activity-chips" className="flex flex-col gap-2">
      {drawn.map(s => (
        <div key={s.k} data-chip-set={s.k} className="flex flex-wrap items-center gap-2 max-sm:flex-nowrap max-sm:overflow-x-auto max-sm:[scrollbar-width:none]">
          <span className="w-16 shrink-0 text-xs text-muted-foreground">{s.word}</span>
          {s.items.map(item => {
            const on = (picks[s.k] as readonly string[]).includes(item.v);
            return (
              <button
                key={item.v}
                type="button"
                aria-pressed={on}
                data-chip={item.v}
                onClick={() => toggle(s.k, item.v as never)}
                className={cn(CHIP, "max-w-56 shrink-0 cursor-pointer font-sans text-xs transition-colors duration-150 hover:text-foreground", on ? "border-foreground/40 bg-foreground/[0.1] text-foreground" : "text-muted-foreground")}
              >
                <span className="truncate">{item.text}</span>
              </button>
            );
          })}
        </div>
      ))}
    </div>
  );
}
