// SPDX-License-Identifier: AGPL-3.0-only
// The design page for wsp-map#1977: a list of recent notices behind a bell at the right end of the centre's header
// row, where the toasts already drop from, drawn inside the app's real shell over the records in ./fixtures.ts.
// Nothing here lands: the bell and its rows are what a build would move into src/notices/NoticesBell.tsx.
//   ?state=shut     the bell with four new notices (the default)
//   ?state=open     the list, the count gone, older rows receded
//   ?state=empty    the list after Clear all
//   ?state=toast    a notice landing under the bell while the list is shut
//   ?state=settings Settings open on General, with the bell where it was
// ?theme=light for Paper; ?bar=0 hides the switcher for a shot. The bell works on the page: opening it reads the
// list, a row runs its act and shuts it, Clear all empties it.
import { BellIcon, GaugeIcon, ListXIcon } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { createRoot } from "react-dom/client";
import { agentName } from "@wsp/catalog";
import { DEFAULT_PREFERENCES, foldThreads, setupNeedsYouLine, type HarnessCatalog, type SessionEvent } from "@wsp/protocol";
import { Mark } from "../../src/brand/Brand";
import { HarnessMark } from "../../src/components/chat/HarnessMark";
import { Button } from "../../src/components/ui/button";
import { Popover, PopoverPopup, PopoverTrigger } from "../../src/components/ui/popover";
import { Tooltip, TooltipPopup, TooltipProvider, TooltipTrigger } from "../../src/components/ui/tooltip";
import { cn } from "../../src/lib/utils";
import { NOTICE_KINDS } from "../../src/notices/Notice";
import { useNotices } from "../../src/notices/store";
import type { Api, ProtocolEvent } from "../../src/protocol/client";
import { useStore } from "../../src/protocol/store";
import { useRightPanelStore } from "../../src/rightPanelStore";
import { ProjectGlyph } from "../../src/projects/look";
import { ComputerGlyph } from "../../src/settings/ComputerGlyph";
import { NOTE } from "../../src/settings/layout";
import { openSettingsGroup } from "../../src/settings/openAt";
import { applyTheme } from "../../src/settings/theme";
import { AppShell } from "../../src/shell/AppShell";
import { useShortcutLabel } from "../../src/shell/useKeybindings";
import { WorkspaceThread } from "../../src/shell/WorkspaceThread";
import { TILE_ROW_ONE_CLASS, TILE_ROW_TWO_CLASS, TILE_TITLE_CLASS } from "../../src/sidebar/rowGrammar";
import { compactTimeLabel, computerName } from "../../src/sidebar/workspaceRows";
import "../../src/index.css";
import "../../src/themes/index";
import { caps } from "../caps.js";
import { noDaemonApi } from "../fake-daemon-api.js";
import { statusOf } from "../workspace-status";
import { ARRIVING, NOTICES, OPEN_THREAD, PLACES, SEEN_BEFORE, SESSIONS, UNREAD, WORKSPACES, type ListNotice } from "./fixtures";

const params = new URLSearchParams(window.location.search);
const theme = params.get("theme") === "light" ? "light" : "dark";
const STATES = ["shut", "open", "empty", "toast", "settings"] as const;
type State = (typeof STATES)[number];
const state: State = STATES.find(s => s === params.get("state")) ?? "shut";
applyTheme({ theme, lightTheme: DEFAULT_PREFERENCES.lightTheme, darkTheme: DEFAULT_PREFERENCES.darkTheme }, theme === "dark");

// ---------------------------------------------------------------------------
// The words the build adds to src/notices.
// ---------------------------------------------------------------------------

export const BELL_WORDS = {
  label: "Notices",
  unread: (n: number): string => `Notices, ${n} new`,
  clear: "Clear all",
  empty: "No notices",
  update: "Update",
  usage: "Usage",
} as const;

// ---------------------------------------------------------------------------
// The bell and its list: the build's NoticesBell.tsx.
// ---------------------------------------------------------------------------

/** The header row's right group, after the context ring, Git, Open and the layout controls, or Settings' actions. */
const BELL_HOST = "[data-shell-center] header .ml-auto";

/** The thread a notice is about, read off the store as the sidebar's tile reads it, so its title is the live one. */
function useNoticeThread(notice: ListNotice) {
  const rows = useStore(s => (notice.thread === undefined ? undefined : s.sessions[notice.thread.workspaceId]));
  const places = useStore(s => s.places);
  const workspace = useStore(s => s.workspaces.find(w => w.id === notice.thread?.workspaceId));
  const status = useStore(s => (notice.thread === undefined ? undefined : s.statuses[notice.thread.workspaceId]));
  if (notice.thread === undefined || rows === undefined || workspace === undefined) return undefined;
  const thread = foldThreads(rows).find(t => t.id === notice.thread?.threadId);
  if (thread === undefined) return undefined;
  return { thread, projectId: workspace.project.id, where: `${workspace.project.name} @ ${computerName(places, { workspace, status: status ?? null })}` };
}

/** Row one's glyph and words for a notice about no thread: the computer it is about, or the page its act reaches. */
function SourceLine({ notice }: { notice: ListNotice }): ReactNode {
  const place = useStore(s => s.places.find(p => p.id === notice.placeId));
  if (place !== undefined)
    return (
      <>
        <ComputerGlyph place={place} className="size-3" />
        <span className="min-w-0 flex-1 truncate">{notice.where ?? place.name}</span>
      </>
    );
  if (notice.source === "usage")
    return (
      <>
        <GaugeIcon aria-hidden className="size-3 shrink-0" />
        <span className="min-w-0 flex-1 truncate">{BELL_WORDS.usage}</span>
      </>
    );
  if (notice.source === "update")
    return (
      <>
        <Mark className="size-3 shrink-0" />
        <span className="min-w-0 flex-1 truncate">{BELL_WORDS.update}</span>
      </>
    );
  return null;
}

const hasSource = (notice: ListNotice): boolean => notice.placeId !== undefined || notice.source !== undefined;

/** The sentence a row draws: a computer's row drops the "<name> needs you:" its first line and glyph already say. */
function rowSentence(notice: ListNotice): string {
  const lead = notice.placeId === undefined || notice.where === undefined ? undefined : setupNeedsYouLine(notice.where, "");
  return lead !== undefined && notice.text.startsWith(lead) ? notice.text.slice(lead.length) : notice.text;
}

/** An act that only opens where the notice points is the whole row; any other word is its own button. */
const opensOnly = (notice: ListNotice): boolean => notice.action?.word === "Open";

/** One notice: the tile's two rows, the kind's glyph and the age in the tile's status slot, and the sentence on a
 * third line where it says what the tile does not. */
function NoticeRow({ notice, onAct }: { notice: ListNotice; onAct: () => void }) {
  const { word, Icon, tone } = NOTICE_KINDS[notice.kind];
  // A row that came before the list was last opened recedes, as a read tile's title does in the sidebar.
  const ink = notice.at > SEEN_BEFORE ? "text-foreground" : "text-muted-foreground";
  const about = useNoticeThread(notice);
  const age = compactTimeLabel(new Date(notice.at).toISOString());
  const slot = (
    <span data-notice-slot className="inline-flex shrink-0 items-center gap-1 tabular-nums">
      {notice.kind === "note" ? null : <Icon role="img" aria-label={word} className={cn("size-3 shrink-0", tone)} />}
      {age}
    </span>
  );
  const { action } = notice;
  const act =
    action === undefined || opensOnly(notice) ? null : (
      <Button
        size="xs"
        variant="outline"
        data-notice-act=""
        className="shrink-0"
        onClick={() => {
          action.run();
          onAct();
        }}
      >
        {action.word}
      </Button>
    );
  const body =
    about === undefined && !hasSource(notice) ? (
      <span className="flex min-h-5 min-w-0 items-center gap-1.5">
        <span data-notice-text className={cn("line-clamp-2 min-w-0 flex-1 text-sm leading-[18px]", ink)}>
          {notice.text}
        </span>
        <span className="flex h-[18px] items-center text-meta leading-[14px] text-muted-foreground">{slot}</span>
      </span>
    ) : about === undefined ? (
      <>
        <span className={cn(TILE_ROW_ONE_CLASS, "text-muted-foreground")}>
          <SourceLine notice={notice} />
          {slot}
        </span>
        <span className="flex min-w-0 items-center gap-2">
          {/* Three lines, so a sign-in's code at the end of its address is never cut. */}
          <span data-notice-text className={cn("line-clamp-3 min-w-0 flex-1 text-sm leading-[18px]", ink)}>
            {rowSentence(notice)}
          </span>
          {act}
        </span>
      </>
    ) : (
      <>
        <span className={cn(TILE_ROW_ONE_CLASS, "text-muted-foreground")}>
          <ProjectGlyph projectId={about.projectId} className="size-3" />
          <span className="min-w-0 flex-1 truncate">{about.where}</span>
          {slot}
        </span>
        <span className={TILE_ROW_TWO_CLASS}>
          <HarnessMark harness={about.thread.harness} label={agentName(about.thread.harness)} className="size-3" />
          <span className={cn(TILE_TITLE_CLASS, ink)}>{about.thread.title}</span>
        </span>
        <span data-notice-text className="line-clamp-2 min-w-0 text-xs leading-4 text-muted-foreground">
          {notice.text}
        </span>
      </>
    );
  const frame = "flex w-full min-w-0 flex-col items-stretch gap-1 rounded-[var(--control-radius)] p-2 text-left";
  return (
    <li data-notice-row={notice.id} data-kind={notice.kind}>
      {action === undefined || !opensOnly(notice) ? (
        <div className={frame}>{body}</div>
      ) : (
        <button
          type="button"
          className={cn(frame, "cursor-pointer outline-none transition-colors duration-150 hover:bg-accent focus-visible:bg-accent")}
          onClick={() => {
            action.run();
            onAct();
          }}
        >
          {body}
        </button>
      )}
    </li>
  );
}

/** 360 px, as the toasts are, and never wider than a phone's window less the toasts' own sides. */
const LIST_WIDTH = { width: 360, maxWidth: "calc(100vw - 24px)" } as const;

function NoticeList({ onAct }: { onAct: () => void }) {
  const notices = useNotices(s => s.notices) as ListNotice[];
  if (notices.length === 0)
    return (
      <p data-k="notices-empty" className={cn(NOTE, "px-3 py-6 text-center")} style={LIST_WIDTH}>
        {BELL_WORDS.empty}
      </p>
    );
  return (
    <div data-notices-list className="flex max-h-[min(30rem,var(--available-height))] flex-col" style={LIST_WIDTH}>
      <ul className="flex min-h-0 flex-col overflow-y-auto p-1">
        {notices.map(n => (
          <NoticeRow key={n.id} notice={n} onAct={onAct} />
        ))}
      </ul>
      <div className="border-t border-border p-1">
        <button
          type="button"
          data-k="notices-clear"
          className="flex h-9 w-full cursor-pointer items-center gap-2.5 rounded-[var(--control-radius)] px-2 text-left text-sm text-foreground outline-none transition-colors duration-150 hover:bg-accent"
          onClick={() => useNotices.getState().clear()}
        >
          <ListXIcon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1 truncate">{BELL_WORDS.clear}</span>
        </button>
      </div>
    </div>
  );
}

function NoticesBell({ startOpen }: { startOpen: boolean }) {
  const unread = useNotices(s => s.unread);
  const [open, setOpen] = useState(startOpen);
  // Jump to latest is the chord that already opens the next thread waiting on the person; the tooltip says it.
  const chord = useShortcutLabel("thread.nextNeedsYou");
  return (
    <Popover
      open={open}
      onOpenChange={next => {
        setOpen(next);
        if (next) useNotices.getState().read();
      }}
    >
      <Tooltip>
        <TooltipTrigger
          render={
            <PopoverTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon"
                  data-k="notices-bell"
                  aria-label={unread === 0 ? BELL_WORDS.label : BELL_WORDS.unread(unread)}
                  className={cn("shrink-0 [-webkit-app-region:no-drag]", unread > 0 && "w-auto gap-1 px-1.5 text-foreground")}
                />
              }
            />
          }
        >
          <BellIcon aria-hidden />
          {unread === 0 ? null : (
            <span data-k="notices-unread" className="text-xs font-medium tabular-nums">
              {unread}
            </span>
          )}
        </TooltipTrigger>
        <TooltipPopup side="bottom">
          {BELL_WORDS.label}
          {chord === null ? null : <span className="ms-2 font-mono text-muted-foreground">{chord}</span>}
        </TooltipPopup>
      </Tooltip>
      {/* A list the page opens on load had no press, so it takes no focus; a press focuses its first row. */}
      <PopoverPopup side="bottom" align="end" sideOffset={6} initialFocus={startOpen ? false : undefined} className="p-0" viewportClassName="p-0 [--viewport-inline-padding:0]">
        <NoticeList onAct={() => setOpen(false)} />
      </PopoverPopup>
    </Popover>
  );
}

/** The header's right group, found once the shell has drawn it and again whenever it is drawn anew (opening Settings
 * swaps the group for Settings' own). */
function useHost(selector: string): Element | null {
  const [host, setHost] = useState<Element | null>(null);
  useEffect(() => {
    const look = (): void => setHost(now => {
      const found = document.querySelector(selector);
      return found === now ? now : found;
    });
    look();
    const timer = setInterval(look, 200);
    return () => clearInterval(timer);
  }, [selector]);
  return host;
}

const listOpen = state === "open" || state === "empty";

function PlacedBell(): ReactNode {
  const host = useHost(BELL_HOST);
  return host === null ? null : createPortal(<NoticesBell startOpen={listOpen} />, host);
}

// ---------------------------------------------------------------------------
// The prototype's own switcher, not a piece of the design.
// ---------------------------------------------------------------------------

const STATE_LABELS: Record<State, string> = { shut: "Four new", open: "Open", empty: "Empty", toast: "A toast lands", settings: "Settings open" };

const go = (over: Record<string, string>): void => {
  const next = new URLSearchParams(window.location.search);
  for (const [k, v] of Object.entries(over)) next.set(k, v);
  window.location.search = next.toString();
};

function Bar() {
  const [shut, setShut] = useState(false);
  if (params.get("bar") === "0" || shut) return null;
  return (
    <div data-proto-bar className="fixed bottom-3 left-3 z-[200] flex max-w-[22rem] flex-col gap-2 rounded-xl border border-(--popover-edge) bg-popover p-3 text-xs text-muted-foreground shadow-(--dialog-shadow)">
      <p className="text-foreground">#1977 prototype</p>
      <p>The bell is at the right end of the header row. Open it, click a row, or Clear all.</p>
      <div className="flex flex-wrap gap-1.5">
        {STATES.map(s => (
          <Button key={s} size="xs" variant={s === state ? "default" : "outline"} onClick={() => go({ state: s })}>
            {STATE_LABELS[s]}
          </Button>
        ))}
      </div>
      <div className="flex flex-wrap gap-1.5">
        <Button size="xs" variant="outline" onClick={() => go({ theme: theme === "dark" ? "light" : "dark" })}>
          {theme === "dark" ? "Paper" : "Graphite"}
        </Button>
        <Button size="xs" variant="ghost" onClick={() => setShut(true)}>
          Hide
        </Button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// The shell's records, over a host that answers reads and nothing else.
// ---------------------------------------------------------------------------

const turn = { workspaceId: OPEN_THREAD.workspaceId, sessionId: OPEN_THREAD.threadId, turnId: "turn_docs", threadId: OPEN_THREAD.threadId };
const HISTORY: SessionEvent[] = [
  { type: "session.start", ...turn, prompt: "Rewrite the install page for the new CLI." },
  { type: "session.delta", ...turn, kind: "text", text: "The install page now leads with the one line installer and drops the old brew tap. The two flags that changed name are in a table under it." },
  { type: "session.done", ...turn, result: { status: "completed", durationMs: 412_000 } },
  { type: "session.end", ...turn, exitCode: 0, sawResult: true },
] as SessionEvent[];

const CATALOGS: HarnessCatalog[] = [
  { harness: "claude", label: "Claude Code", source: "harness", version: "2.1.296", models: [{ value: "claude-opus-5-5", label: "Opus 5.5", isDefault: true }], efforts: [], contextWindows: [], permissionModes: [], steers: true, renames: true, images: true },
  { harness: "codex", label: "Codex", source: "table", version: "0.162.1", models: [{ value: "gpt-5.6", label: "GPT-5.6", isDefault: true }], efforts: [], contextWindows: [], permissionModes: [], steers: false, renames: false, images: false },
] as HarnessCatalog[];

const watching = new Set<(event: ProtocolEvent) => void>();
const api = {
  listWorkspaces: async () => WORKSPACES,
  getWorkspace: async (id: string) => WORKSPACES.find(w => w.id === id),
  watchStatuses: async () => WORKSPACES.map(w => statusOf(w, w.kind === "local" ? { kind: "local", size: { cpu: 10, memMb: 16384 }, rateUsdPerHour: 0 } : {})),
  capabilities: async () => caps(),
  listSessions: async () => SESSIONS,
  sessionHistory: async (id: string) => (id === OPEN_THREAD.workspaceId ? HISTORY : []),
  listHarnesses: async () => CATALOGS,
  subscribe: (fn: (e: ProtocolEvent) => void) => {
    watching.add(fn);
    return () => watching.delete(fn);
  },
  daemon: noDaemonApi,
  getGolden: async () => undefined,
  listProjectGoldens: async () => [],
  listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
  snapshotStorage: async () => null,
} as unknown as Api;

useStore.setState({ conn: "live", places: PLACES, preferences: { ...DEFAULT_PREFERENCES, theme, projectLook: { pr_wsp: { icon: "rocket", hue: "teal" }, pr_wsp_box: { icon: "rocket", hue: "teal" }, pr_landing: { icon: "globe", hue: "violet" } } } });
useStore.getState().bind(api);
useStore.getState().select(OPEN_THREAD.workspaceId, OPEN_THREAD.threadId);
useStore.setState({ places: PLACES });

// Opening the list reads it, so the count is gone once it is open.
useNotices.setState({ notices: state === "empty" ? [] : NOTICES, toasts: [], unread: listOpen || state === "empty" ? 0 : UNREAD });
if (state === "toast")
  setTimeout(() => {
    const { id: _id, at: _at, ...input } = ARRIVING;
    useNotices.getState().add(input);
  }, 600);
if (state === "settings") openSettingsGroup("general");
// A narrow window draws the right panel as a sheet over the page; the list is what these shots are of.
if (window.innerWidth < 640) useRightPanelStore.getState().close(OPEN_THREAD.workspaceId);

createRoot(document.getElementById("root")!).render(
  <TooltipProvider>
    <AppShell>
      <div className="flex min-h-0 flex-1 flex-col" data-terminal-beside>
        <WorkspaceThread workspaceId={OPEN_THREAD.workspaceId} />
      </div>
    </AppShell>
    <PlacedBell />
    <Bar />
  </TooltipProvider>,
);
