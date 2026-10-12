// SPDX-License-Identifier: AGPL-3.0-only
// The Processes pane of the right panel: every process the daemon lists in
// one dense mono table, a tree by parent with the harness pieces named,
// sortable by cpu or memory, filtered by a box. The selected row is the only
// lit one and opens into the daemon's inspect fields with the kill controls;
// a running turn's agent row offers its thread's stop instead, asked first.
// A thread on this computer or on a box the person joined draws its own tree
// instead (ThreadProcesses.tsx); this table is this computer's own panel and a
// machine wsp forks. The count is the rows on the screen, never the machine's total.
import { useCallback, useEffect, useMemo, useState, useSyncExternalStore, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { folderOnJoined, foldThreads, hereName, isLocalWorkspace, killTooOldLine, workspaceComputerName, workspaceKind, type ProcEntry, type ProcInspectReply, type ProcKillReply } from "@wsp/protocol";
import { cn, errorText } from "../../lib/utils.js";
import { staleWord, type StaleWord } from "../../machine/live.js";
import { holdAppProcs, useAppProcs } from "../../machine/appProcs.js";
import { getProcs, useWorkspaceProcs } from "../../machine/procs.js";
import { useAbsentComputer, useStore, useWorkspace } from "../../protocol/store.js";
import { StartDaemonButton } from "../DaemonDown.js";
import { HERE_KEY } from "../../terminal/computer.js";
import { getTerminals, onTerminals } from "../../terminal/link.js";
import { clockLabel } from "../../lib/timestampFormat.js";
import { compactBytes } from "@wsp/protocol";
import { ScrollArea } from "../ui/scroll-area.js";
import { terminalLabels } from "../WorkspaceTerminalDrawer.js";
import { AlertDialog, AlertDialogClose, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogPopup, AlertDialogTitle } from "../ui/alert-dialog.js";
import { Button, NEUTRAL_RING } from "../ui/button.js";
import { IDLE, offered, press, settle, termed, type KillState } from "./kill.js";
import { agentRows, procLabel, procTable, runAgentPids, type ProcRow, type ProcSort, type ProcThread } from "./tree.js";
import { ThreadProcesses } from "./ThreadProcesses.js";

/** What a turn's agent row says where its thread cannot be named, and so offers no stop of its own. */
const AGENT_UNNAMED_LINE = "a thread's agent: stop its thread to end it";
/** What a kept agent's End says once its thread let it go, and where this window cannot ask. */
const KEPT_FREED_LINE = "freed; the thread's next send starts a new one";
const KEPT_UNREACHED_LINE = "this window cannot end a thread's agent";

/** Every row is this tall, so the table reads as a grid and a selection moves nothing. */
export const ROW_PX = 22;
const COLUMNS = "grid grid-cols-[3.5rem_3.25rem_3.75rem_minmax(0,1fr)] items-center px-2";
const NO_TABS: readonly never[] = [];
const NO_THREADS: readonly ProcThread[] = [];

/** The threads with a turn running, each with the process the host started for it, so a turn's agent row offers its
 * thread's stop. This computer's own panel lists every process here, so it reads every local workspace's threads. */
function useThreadRoots(workspaceId: string): readonly ProcThread[] {
  const sessions = useStore(s => s.sessions);
  const workspaces = useStore(s => s.workspaces);
  const rows = useMemo(
    () => (workspaceId === HERE_KEY ? workspaces.filter(isLocalWorkspace).flatMap(w => sessions[w.id] ?? []) : sessions[workspaceId]),
    [workspaceId, sessions, workspaces],
  );
  return useMemo(() => {
    if (rows === undefined) return NO_THREADS;
    const threads = foldThreads(rows);
    const running = (t: (typeof threads)[number]) => (t.status === "running" ? { sessionId: t.sessionId } : {});
    return threads.filter(t => t.pid !== undefined).map(t => ({ threadId: t.id, title: t.title, pid: t.pid!, ...running(t), ...(t.agentPid !== undefined ? { agentPid: t.agentPid } : {}) }));
  }, [rows]);
}

function useTerminalTitles(workspaceId: string): ReadonlyMap<string, string> {
  const terms = useSyncExternalStore(onTerminals, () => getTerminals(workspaceId));
  const subscribe = useCallback((fn: () => void) => (terms ? terms.onTabs(fn) : () => {}), [terms]);
  const tabs = useSyncExternalStore(subscribe, () => (terms ? terms.tabs() : NO_TABS));
  return useMemo(() => terminalLabels(tabs), [tabs]);
}

export function ProcessesSurface({ workspaceId }: { workspaceId: string }) {
  const workspace = useWorkspace(workspaceId);
  if (workspace !== null && (isLocalWorkspace(workspace) || folderOnJoined(workspaceKind(workspace)))) return <ThreadProcesses workspaceId={workspaceId} workspace={workspace} />;
  return <ComputerProcesses workspaceId={workspaceId} />;
}

function ComputerProcesses({ workspaceId }: { workspaceId: string }) {
  const workspace = useWorkspace(workspaceId);
  const absent = useAbsentComputer(workspaceId);
  const procs = useWorkspaceProcs(workspaceId);
  const titles = useTerminalTitles(workspaceId);
  const [sort, setSort] = useState<ProcSort>("cpu");
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<number | null>(null);

  useEffect(() => getProcs(workspaceId).watch(), [workspaceId]);
  // The app runs where the host runs, so only this computer's own panel lists its processes.
  const local = workspaceId === HERE_KEY;
  useEffect(() => (local ? holdAppProcs() : undefined), [local]);
  const appProcs = useAppProcs();

  const snapshot = procs.snapshot;
  const threads = useThreadRoots(workspaceId);
  const table = useMemo(() => procTable(snapshot?.procs ?? [], sort, filter), [snapshot, sort, filter]);
  const agents = useMemo(() => agentRows(snapshot?.procs ?? [], threads), [snapshot, threads]);
  const runAgents = useMemo(() => runAgentPids(snapshot?.procs ?? []), [snapshot]);
  // This computer never reads unreachable: its panes say pending until the first frame.
  const stale = workspace === null ? null : (absent?.away ?? staleWord(workspace.phase, procs.reach === "live"));

  // A process that went away takes its selection with it.
  useEffect(() => {
    if (selected !== null && snapshot !== null && !snapshot.procs.some(p => p.pid === selected)) setSelected(null);
  }, [snapshot, selected]);

  const shown = table.length;
  const count = snapshot === null ? "pending" : shown < snapshot.total ? `${shown} of ${snapshot.total}` : `${shown} processes`;
  const unavailable = stale === null && snapshot === null ? procs.unavailable : null;
  const draw = (rows: readonly ProcRow[]) =>
    rows.map(({ proc, depth }) => {
      const lit = proc.pid === selected;
      const label = snapshot ? procLabel(proc, snapshot.daemon, titles, local ? appProcs : undefined) : null;
      return (
        <div key={proc.pid}>
          <Row proc={proc} depth={depth} label={label} lit={lit} onSelect={() => setSelected(lit ? null : proc.pid)} />
          {lit && <Details workspaceId={workspaceId} proc={proc} stale={stale} agent={agents.get(proc.pid)} runAgent={runAgents.has(proc.pid)} here={local} />}
        </div>
      );
    });

  return (
    <div className="flex min-h-0 flex-1 flex-col font-mono text-[11px]" data-procs>
      <div className="flex h-8 shrink-0 items-center gap-2 border-b border-border/50 px-2">
        <input
          aria-label="Filter processes"
          placeholder="filter"
          value={filter}
          onChange={e => setFilter(e.target.value)}
          className="h-6 min-w-0 flex-1 bg-transparent text-foreground outline-none placeholder:text-placeholder"
        />
        <span className={cn("shrink-0 tabular-nums", stale !== null ? "text-muted-foreground" : "text-muted-foreground")} data-procs-count>
          {stale ?? (unavailable !== null ? "unavailable" : count)}
        </span>
      </div>
      <div className={cn(COLUMNS, "h-6 shrink-0 border-b border-border/50 text-xs text-muted-foreground")} role="row">
        <span className="text-right">pid</span>
        <SortButton k="cpu" sort={sort} onSort={setSort}>
          cpu
        </SortButton>
        <SortButton k="mem" sort={sort} onSort={setSort}>
          mem
        </SortButton>
        <span className="pl-2">command</span>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div role="table" aria-label="Processes" className={cn(stale !== null && "text-muted-foreground")} {...(stale !== null ? { "data-stale": stale } : {})}>
          {absent !== null ? (
            <div className="flex flex-col items-start gap-2 px-2 py-2 font-sans text-[13px] text-muted-foreground" data-procs-unavailable>
              <p>{absent.said}</p>
              {absent.will !== undefined ? <p>{absent.will}</p> : null}
              <StartDaemonButton absent={absent} workspaceId={workspaceId} />
            </div>
          ) : (
            unavailable !== null && (
              <p className="truncate px-2 font-sans text-[13px] text-muted-foreground" style={{ lineHeight: `${ROW_PX}px` }} title={unavailable} data-procs-unavailable>
                {unavailable}
              </p>
            )
          )}
          {draw(table)}
        </div>
      </ScrollArea>
    </div>
  );
}

function SortButton({ k, sort, onSort, children }: { k: ProcSort; sort: ProcSort; onSort: (k: ProcSort) => void; children: string }) {
  return (
    <button
      type="button"
      aria-pressed={sort === k}
      onClick={() => onSort(k)}
      className={cn("cursor-pointer text-right hover:text-foreground", sort === k ? "text-foreground" : "text-muted-foreground")}
    >
      {children}
    </button>
  );
}

interface RowProps {
  proc: ProcEntry;
  depth: number;
  label: string | null;
  lit: boolean;
  onSelect: () => void;
}

function Row({ proc, depth, label, lit, onSelect }: RowProps) {
  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>): void => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onSelect();
    }
  };
  const command = proc.cmdline !== "" ? proc.cmdline : `[${proc.comm}]`;
  return (
    <div
      role="row"
      tabIndex={0}
      data-proc-row={proc.pid}
      {...(lit ? { "data-selected": "" } : {})}
      onClick={onSelect}
      onKeyDown={onKeyDown}
      style={{ height: ROW_PX }}
      className={cn(COLUMNS, "cursor-default outline-none", lit ? "bg-accent text-foreground" : "hover:bg-accent/40 focus-visible:bg-accent/40")}
    >
      <span className={cn("text-right tabular-nums", !lit && "text-muted-foreground")}>{proc.pid}</span>
      <span className="text-right tabular-nums" data-col="cpu">
        {proc.cpu.toFixed(1)}
      </span>
      <span className="text-right tabular-nums" data-col="mem">
        {compactBytes(proc.rss)}
      </span>
      <span className="min-w-0 truncate" style={{ paddingLeft: 8 + depth * 12 }} title={command}>
        {label !== null && (
          <span className="mr-1.5 text-muted-foreground" data-proc-label>
            {label}
          </span>
        )}
        {command}
      </span>
    </div>
  );
}

/** The daemon's inspect fields for the lit row, read once when it opens, and the kill controls. */
function Details({ workspaceId, proc, stale, agent, runAgent, here }: { workspaceId: string; proc: ProcEntry; stale: StaleWord; agent: ProcThread | undefined; runAgent: boolean; here: boolean }) {
  const [inspect, setInspect] = useState<ProcInspectReply | string | null>(null);
  useEffect(() => {
    let gone = false;
    setInspect(null);
    getProcs(workspaceId)
      .inspect(proc.pid)
      .then(r => !gone && setInspect(r), e => !gone && setInspect(errorText(e)));
    return () => {
      gone = true;
    };
  }, [workspaceId, proc.pid]);

  const detail = typeof inspect === "object" && inspect !== null ? inspect : null;
  const list = (xs: number[] | undefined): string => (xs === undefined ? "" : xs.length === 0 ? "none" : xs.join(" "));
  return (
    <div className="border-y border-border/50 bg-accent/30 px-2 py-1.5" data-proc-details={proc.pid}>
      <dl className="grid grid-cols-[4.5rem_minmax(0,1fr)] gap-x-2 gap-y-0.5">
        <Field k="user">{proc.user}</Field>
        <Field k="state">{proc.state}</Field>
        <Field k="started">{clockLabel(new Date(proc.startedAt).toISOString())}</Field>
        <Field k="threads">{detail?.threads === undefined ? "" : String(detail.threads)}</Field>
        <Field k="cwd">{detail ? (detail.cwd ?? "unreadable") : ""}</Field>
        <Field k="ports">{list(detail?.ports)}</Field>
        <Field k="children">{list(detail?.children)}</Field>
        <Field k="command">{proc.cmdline !== "" ? proc.cmdline : `[${proc.comm}]`}</Field>
        {typeof inspect === "string" && <Field k="inspect">{inspect}</Field>}
      </dl>
      {agent?.sessionId !== undefined ? (
        <StopTurn title={agent.title} sessionId={agent.sessionId} disabled={stale !== null} />
      ) : runAgent && here ? (
        <EndKept proc={proc} disabled={stale !== null} />
      ) : runAgent ? (
        <p className="mt-1.5 h-5 truncate text-muted-foreground" data-proc-agent-unnamed>
          {AGENT_UNNAMED_LINE}
        </p>
      ) : (
        <Kill workspaceId={workspaceId} proc={proc} disabled={stale !== null} />
      )}
    </div>
  );
}

function Field({ k, children }: { k: string; children: string }) {
  return (
    <>
      <dt className="text-muted-foreground">{k}</dt>
      <dd className="min-w-0 truncate text-foreground" title={children} data-k={k}>
        {children}
      </dd>
    </>
  );
}

/** The row's one act, at the table's own height: quiet until it is the confirming press. */
function PaneButton({ disabled, onPress, confirm, children }: { disabled: boolean; onPress: () => void; confirm: boolean; children: string }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onPress}
      data-confirm={confirm ? "" : undefined}
      className={cn(
        "h-5 cursor-pointer rounded-sm border px-1.5 disabled:cursor-default disabled:opacity-50",
        confirm ? "border-transparent bg-destructive text-white hover:bg-destructive/90" : "border-border text-muted-foreground transition-colors duration-150 hover:border-destructive/50 hover:text-destructive-foreground",
      )}
    >
      {children}
    </button>
  );
}

/** What the daemon's answer to a kill says on the row. */
function killedWord(reply: ProcKillReply): string {
  if ((reply.left?.length ?? 0) > 0) return "still running after a forced end";
  if ((reply.forced?.length ?? 0) > 0) return "ended by KILL after TERM's 2 s";
  return "ended";
}

/** Two presses, no dialog: kill arms, confirm within two seconds asks the daemon to end the process and everything
 * under it, TERM then KILL, checked against the row's identity and start so a pid reused since is never signalled. */
function Kill({ workspaceId, proc, disabled }: { workspaceId: string; proc: ProcEntry; disabled: boolean }) {
  const workspace = useWorkspace(workspaceId);
  const places = useStore(s => s.places);
  const computer = workspace === null ? hereName(places) : workspaceComputerName(places, workspace);
  const [state, setState] = useState<KillState>(IDLE);
  const [now, setNow] = useState(() => Date.now());
  const [said, setSaid] = useState<string | null>(null);

  useEffect(() => {
    if (state.step !== "armed" && state.step !== "termed") return;
    const t = setInterval(() => {
      const at = Date.now();
      setNow(at);
      setState(s => settle(s, at));
    }, 250);
    return () => clearInterval(t);
  }, [state.step]);

  const onPress = (): void => {
    const at = Date.now();
    const next = press(state, at);
    setNow(at);
    setState(next.state);
    if (next.send === undefined) return;
    // The row's id is what the daemon checks the pid still holds; its start time stands in where the reader has none.
    const listed = { startedAt: proc.startedAt, ...(proc.id !== undefined ? { identity: proc.id } : {}) };
    const failed = (e: unknown): void => {
      setState(IDLE);
      setSaid(errorText(e));
    };
    if (next.send === "force") {
      getProcs(workspaceId)
        .kill(proc.pid, { signal: "KILL", ...listed })
        .then(() => setSaid("KILL sent"), failed);
      return;
    }
    setSaid("ending");
    getProcs(workspaceId)
      .kill(proc.pid, { signal: "TERM", mode: "tree", ...listed })
      .then(reply => {
        // A daemon from before kill modes answers with none of the reply's fields, having sent TERM to the pid alone.
        if (reply.signalled !== undefined) return setSaid(killedWord(reply));
        setState(termed(Date.now()));
        setSaid(killTooOldLine(computer));
      }, failed);
  };

  const offer = offered(state, now);
  const word = offer?.send === "force" ? "kill -9" : "kill";
  return (
    <div className="mt-1.5 flex h-5 items-center gap-2" data-proc-kill>
      {offer !== null && (
        <PaneButton disabled={disabled} onPress={onPress} confirm={offer.confirm}>
          {offer.confirm ? `confirm ${word}` : word}
        </PaneButton>
      )}
      {said !== null && (
        <span className="min-w-0 truncate text-muted-foreground" title={said} data-proc-said>
          {said}
        </span>
      )}
    </div>
  );
}

/** A kept agent between its thread's turns, or a warm one before its first: no turn runs on it, so one press frees it
 * through its thread, as `wsp activity end` does, and the thread's next send starts a fresh one. The host refuses a
 * running turn's agent, whose row this computer's panel shows as Stop. */
function EndKept({ proc, disabled }: { proc: ProcEntry; disabled: boolean }) {
  const api = useStore(s => s.api);
  const [said, setSaid] = useState<string | null>(null);
  const end = (): void => {
    const call = api?.endActivity;
    if (call === undefined) {
      setSaid(KEPT_UNREACHED_LINE);
      return;
    }
    setSaid("ending");
    call(proc.pid, proc.startedAt).then(
      reply => setSaid(reply.freed !== undefined ? KEPT_FREED_LINE : "ended"),
      e => setSaid(errorText(e)),
    );
  };
  return (
    <div className="mt-1.5 flex h-5 items-center gap-2" data-proc-kill data-proc-kept>
      <PaneButton disabled={disabled || said === "ending"} onPress={end} confirm={false}>
        end
      </PaneButton>
      {said !== null && (
        <span className="min-w-0 truncate text-muted-foreground" title={said} data-proc-said>
          {said}
        </span>
      )}
    </div>
  );
}

/** A running turn's agent is never signalled: a signal reads as a failed turn. Its button asks, then stops the
 * thread, and the turn reads interrupted; what the agent started keeps running. */
function StopTurn({ title, sessionId, disabled }: { title: string; sessionId: string; disabled: boolean }) {
  const api = useStore(s => s.api);
  const [open, setOpen] = useState(false);
  const [said, setSaid] = useState<string | null>(null);

  const stop = (): void => {
    setOpen(false);
    const interrupt = api?.interruptSession;
    if (interrupt === undefined) {
      setSaid("this window cannot stop a thread");
      return;
    }
    setSaid("stopping");
    interrupt(sessionId).then(
      ({ outcome, left, error }) => setSaid(error ?? left ?? (outcome === "not-found" ? "no turn is running" : "stopped")),
      e => setSaid(errorText(e)),
    );
  };

  return (
    <div className="mt-1.5 flex h-5 items-center gap-2" data-proc-kill data-proc-agent>
      <PaneButton disabled={disabled || said === "stopping"} onPress={() => setOpen(true)} confirm={false}>
        stop turn
      </PaneButton>
      {said !== null && (
        <span className="min-w-0 truncate text-muted-foreground" title={said} data-proc-said>
          {said}
        </span>
      )}
      <AlertDialog open={open} onOpenChange={setOpen}>
        <AlertDialogPopup>
          <AlertDialogHeader>
            <AlertDialogTitle>Stop {title}?</AlertDialogTitle>
            <AlertDialogDescription>This process is the thread's agent. Its turn ends as interrupted; what it started keeps running.</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogClose render={<Button variant="outline" className={NEUTRAL_RING} />}>Cancel</AlertDialogClose>
            <Button variant="destructive" onClick={stop} data-k="stop-turn">
              Stop
            </Button>
          </AlertDialogFooter>
        </AlertDialogPopup>
      </AlertDialog>
    </div>
  );
}
