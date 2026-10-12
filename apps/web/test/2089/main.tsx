// SPDX-License-Identifier: AGPL-3.0-only
// Served by Vite to a real browser: the app shell over a fake host where a
// lead thread on zingzy's MacBook Pro has started five sub-threads on Boat, one
// state per ?state=, in either theme (?theme=light). The open sub-thread is
// "Fix the flaky upload test" in the state named; its four siblings stand in
// the states around it, so the sidebar shows every row at once. The right
// panel is on the Machine tab. boat opens Settings on Boat's page, remove its
// Remove dialog over two sub-threads holding work not pushed; Remove anyway
// there removes Boat the way the host does, with place.removed alone, and the
// app reads the rows again into the forgotten state. Every screen is the
// product's own components over the wire's shapes. A switcher floats at the top
// right; ?bar=0 hides it for a shot.
import { createRoot } from "react-dom/client";
import { DEFAULT_PREFERENCES, subHeldLine, subOomLine, type HarnessCatalog, type PlaceSettingWord, type PlaceSettingsAsk, type PlaceView, type ProjectView, type SessionEvent, type SessionView, type SubRow, type ThreadCapWait, type WorkspaceView } from "@wsp/protocol";
import { TooltipProvider } from "../../src/components/ui/tooltip";
import type { Api, ProtocolEvent } from "../../src/protocol/client";
import { getLive } from "../../src/machine/live";
import { useStore } from "../../src/protocol/store";
import { useRightPanelStore } from "../../src/rightPanelStore";
import { AppShell } from "../../src/shell/AppShell";
import { WorkspaceThread } from "../../src/shell/WorkspaceThread";
import { openComputerSettings, openSettingsGroup } from "../../src/settings/openAt";
import { applyTheme } from "../../src/settings/theme";
import { statusOf } from "../workspace-status";
import { caps } from "../caps.js";
import { noDaemonApi } from "../fake-daemon-api.js";
import { ACCESS_MODES } from "../fixtures/access-modes";
import "../../src/index.css";
import "../../src/themes/index";

const params = new URLSearchParams(window.location.search);
const theme: "light" | "dark" = params.get("theme") === "light" ? "light" : "dark";
const picks = { lightTheme: DEFAULT_PREFERENCES.lightTheme, darkTheme: DEFAULT_PREFERENCES.darkTheme };
applyTheme({ theme, ...picks }, theme === "dark");
const STATES = ["starting", "waiting-machines", "waiting-minute", "running", "not-pushed", "deleting", "released", "oom", "forgotten", "boat", "remove"] as const;
type State = (typeof STATES)[number];
const state: State = (STATES as readonly string[]).includes(params.get("state") ?? "") ? (params.get("state") as State) : "running";

const MIN = 60_000;
const ago = (m: number): number => Date.now() - m * MIN;
const SMALL = { cpu: 2, memMb: 4096 };
const RATE = 0.018;

const HERE: PlaceView = { id: "here", kind: "computer", name: "zingzy-mbp", label: "zingzy's MacBook Pro", mac: "macbook", default: true, present: true, shape: { cpu: 10, memMb: 16384 }, takesForks: false, agentVersions: { claude: "2.1.286" } };
/** Boat as `wsp computers` reads it on the $20 plan: three machines at once of the plan's hundred, 187 of the day's
 * 200 starts used, small by default and a 30 minute delete. */
const BOAT_DEFAULTS = { cap: { machines: 3, spendPerDayUsd: 10 }, size: SMALL, deleteAfterMs: 30 * MIN };
let boat: PlaceView = {
  id: "box",
  kind: "provider",
  name: "box",
  default: false,
  takesForks: true,
  rateUsdPerHour: RATE,
  sizes: [
    { cpu: 2, memMb: 4096, rateUsdPerHour: 0.018 },
    { cpu: 4, memMb: 8192, rateUsdPerHour: 0.036 },
    { cpu: 8, memMb: 16384, rateUsdPerHour: 0.072 },
    { cpu: 16, memMb: 32768, rateUsdPerHour: 0.2 },
  ],
  cap: BOAT_DEFAULTS.cap,
  capDefault: BOAT_DEFAULTS.cap,
  settings: {},
  running: 3,
  plan: { key: "box_20", trial: false, machinesAtOnce: 100, sizes: ["2x4", "4x8", "8x16"] },
  starts: { minute: { used: 3, limit: 12 }, hour: { used: 41, limit: 60 }, day: { used: 187, limit: 200 } },
  size: SMALL,
  sizeDefault: SMALL,
  deleteAfterMs: BOAT_DEFAULTS.deleteAfterMs,
  napMs: 20 * MIN,
  napDefault: 20 * MIN,
  turnLimitMs: 4 * 60 * MIN,
  turnLimitDefault: 4 * 60 * MIN,
} as PlaceView;

const PROJECT: ProjectView = { id: "pr_web", name: "web", computer: "here", source: { kind: "folder", path: "/Users/dev/web" }, path: "/Users/dev/web", remote: "github.com/acme/web", defaultBranch: "main", memoryKey: "pr_web", memoryDir: "/m" } as ProjectView;
const project = { id: PROJECT.id, name: PROJECT.name, path: PROJECT.path, computer: "here" };
/** A sub-machine works in the repository's own name under root's home, as subFolderOf names it. */
const subProject = { ...project, path: "/root/web" };
const LEAD_WS: WorkspaceView = { id: "ws_lead", name: "web", kind: "local", machineId: "local", project, phase: "running", golden: "", createdAt: "2026-10-12T08:00:00Z", place: "here" } as WorkspaceView;

/** One sub-thread as the host lists it: its machine's record, its row and its transcript. */
interface Sub {
  readonly key: string;
  readonly title: string;
  readonly branch: string;
  readonly phase: WorkspaceView["phase"];
  readonly status: SessionView["status"];
  readonly sub: Omit<SubRow, "on" | "machine" | "size" | "rateUsdPerHour" | "branch">;
  readonly startedAt: number;
  readonly endedAt?: number;
  readonly unread?: boolean;
  readonly capped?: ThreadCapWait;
  readonly queue?: WorkspaceView["queue"];
  readonly failure?: string;
  readonly reply?: string;
  readonly accruedUsd?: number;
}

const pushed = (branch: string) => ({ result: { branch, pushedSha: "4f1c2a9", commits: 3, unpushed: 0, changed: 0, readAt: new Date(ago(6)).toISOString() } });
const FOCUS_TITLE = "Fix the flaky upload test";
const FOCUS_BRANCH = "wsp/fix-the-flaky-upload-test";

/** The open sub-thread in the state the page is on. */
function focus(): Sub {
  const base = { key: "fix", title: FOCUS_TITLE, branch: FOCUS_BRANCH };
  switch (state) {
    case "starting":
      return { ...base, phase: "waking", status: "running", sub: { state: "starting" }, startedAt: ago(0.2) };
    case "waiting-machines":
      return { ...base, phase: "queued", status: "running", sub: { state: "waiting" }, startedAt: ago(1), queue: { since: new Date(ago(1)).toISOString(), why: "slot" }, capped: { placeId: "box", place: "Boat", running: 3, atOnce: 3, why: "slot" } };
    case "waiting-minute":
      return { ...base, phase: "queued", status: "running", sub: { state: "waiting" }, startedAt: ago(0.4), queue: { since: new Date(ago(0.4)).toISOString(), why: "minute" }, capped: { placeId: "box", place: "Boat", running: 12, atOnce: 12, why: "minute" } };
    case "running":
    case "boat":
      return { ...base, phase: "running", status: "running", sub: { state: "running" }, startedAt: ago(13), accruedUsd: 0.004 };
    case "not-pushed":
    case "remove":
      return { ...base, phase: "napping", status: "completed", sub: { state: "napping", held: { commits: 2, files: 3 }, result: { branch: FOCUS_BRANCH, pushedSha: null, commits: 2, unpushed: 2, changed: 3, readAt: new Date(ago(4)).toISOString() } }, startedAt: ago(31), endedAt: ago(4), unread: true, reply: "The test now waits for the upload's own done event instead of a fixed 200 ms. Two commits on the branch; I could not push them, the remote refused my token.", accruedUsd: 0.009 };
    case "deleting":
      return { ...base, phase: "napping", status: "completed", sub: { state: "napping", deleteAt: Date.now() + 18 * MIN, ...pushed(FOCUS_BRANCH) }, startedAt: ago(26), endedAt: ago(12), unread: true, reply: "Pushed wsp/fix-the-flaky-upload-test: the test waits for the upload's done event. 40 runs green in a row.", accruedUsd: 0.008 };
    case "released":
      return { ...base, phase: "released", status: "completed", sub: { state: "released", releasedAt: ago(9), ...pushed(FOCUS_BRANCH) }, startedAt: ago(72), endedAt: ago(39), reply: "Pushed wsp/fix-the-flaky-upload-test: the test waits for the upload's done event. 40 runs green in a row." };
    case "oom":
      return { ...base, phase: "napping", status: "failed", sub: { state: "napping", deleteAt: Date.now() + 27 * MIN }, startedAt: ago(9), endedAt: ago(3), failure: oomLine(), accruedUsd: 0.003 };
    case "forgotten":
      return { ...base, phase: "released", status: "completed", sub: { state: "released", releasedAt: ago(2), cloudGone: true, ...pushed(FOCUS_BRANCH) }, startedAt: ago(72), endedAt: ago(39), reply: "Pushed wsp/fix-the-flaky-upload-test: the test waits for the upload's done event. 40 runs green in a row." };
  }
}

function oomLine(): string {
  const line = subOomLine(FOCUS_TITLE, "Boat", { name: "small", ...SMALL }, "4x8");
  return `${line.charAt(0).toUpperCase()}${line.slice(1)}`;
}

/** The siblings, the same on every page but the forgotten one, where Boat took every machine with it. */
function siblings(): Sub[] {
  const gone = state === "forgotten";
  const released = (key: string, title: string, branch: string, minutes: number): Sub => ({ key, title, branch, phase: "released", status: "completed", sub: { state: "released", releasedAt: ago(minutes), ...(gone ? { cloudGone: true as const } : {}), ...pushed(branch) }, startedAt: ago(minutes + 50), endedAt: ago(minutes + 30), reply: `Pushed ${branch}.` });
  if (gone) return [released("retry", "Retry the multipart upload", "wsp/retry-the-multipart-upload", 2), released("cap", "Bump the upload size cap", "wsp/bump-the-upload-size-cap", 2), released("route", "Drop the old upload route", "wsp/drop-the-old-upload-route", 2), released("events", "Upload progress events", "wsp/upload-progress-events", 40)];
  return [
    { key: "retry", title: "Retry the multipart upload", branch: "wsp/retry-the-multipart-upload", phase: "running", status: "running", sub: { state: "running" }, startedAt: ago(7), accruedUsd: 0.002 },
    { key: "cap", title: "Bump the upload size cap", branch: "wsp/bump-the-upload-size-cap", phase: "napping", status: "completed", sub: { state: "napping", held: { commits: 1, files: 0 }, result: { branch: "wsp/bump-the-upload-size-cap", pushedSha: null, commits: 1, unpushed: 1, changed: 0, readAt: new Date(ago(20)).toISOString() } }, startedAt: ago(45), endedAt: ago(20), reply: "One commit raises the cap to 5 GB.", accruedUsd: 0.014 },
    { key: "route", title: "Drop the old upload route", branch: "wsp/drop-the-old-upload-route", phase: "napping", status: "completed", sub: { state: "napping", deleteAt: Date.now() + 7 * MIN, ...pushed("wsp/drop-the-old-upload-route") }, startedAt: ago(40), endedAt: ago(23), reply: "Pushed wsp/drop-the-old-upload-route.", accruedUsd: 0.011 },
    released("events", "Upload progress events", "wsp/upload-progress-events", 40),
  ];
}

let subs: Sub[] = [focus(), ...siblings()];
const machineOf = (s: Sub): string => `bx_${s.key}7q2`;
const HEX: Record<string, string> = { fix: "e17a90c3", retry: "5c2be841", cap: "9b04d1f2", route: "a83f6e10", events: "27d9c5b4" };
/** A thread id as the host mints one, so a line that names a sub-thread by its first eight says what the host would. */
const threadOf = (s: Sub): string => `${HEX[s.key]}-4d1e-9f2a-${s.key}`;
const wsOf = (s: Sub): WorkspaceView =>
  ({ id: `ws_${s.key}`, name: `web-${s.key}`, kind: "cloud", provider: "box", place: "box", machineId: machineOf(s), project: subProject, phase: s.phase, golden: "img_1", createdAt: new Date(s.startedAt).toISOString(), ...(s.queue === undefined ? {} : { queue: s.queue }) }) as WorkspaceView;
const subRowOf = (s: Sub): SubRow => ({ on: "box", machine: machineOf(s), size: SMALL, rateUsdPerHour: RATE, branch: s.branch, ...s.sub });
const PROMPT_OF: Record<string, string> = {
  fix: "Fix the flaky upload test in packages/upload: it fails about one run in ten on CI. Push wsp/fix-the-flaky-upload-test when it is green 40 times in a row.",
};
const sessionOf = (s: Sub): SessionView =>
  ({
    id: `s_${s.key}`,
    threadId: threadOf(s),
    workspaceId: `ws_${s.key}`,
    harness: "claude",
    status: s.status,
    prompt: PROMPT_OF[s.key] ?? s.title,
    harnessTitle: s.title,
    startedBy: "agent",
    parentThreadId: "thr_lead",
    rootThreadId: "thr_lead",
    startedAt: s.startedAt,
    ...(s.endedAt === undefined ? {} : { endedAt: s.endedAt }),
    ...(s.unread === true || s.endedAt === undefined ? {} : { readAt: s.endedAt + MIN }),
    ...(s.failure === undefined ? {} : { failure: s.failure }),
    ...(s.reply === undefined ? {} : { lastLine: s.reply }),
    ...(s.capped === undefined ? {} : { capped: s.capped }),
    model: "claude-opus-5-5",
    project: { id: PROJECT.id, name: PROJECT.name },
    computerName: "Boat",
    sub: subRowOf(s),
  }) as SessionView;

const LEAD: SessionView = { id: "s_lead", threadId: "thr_lead", workspaceId: LEAD_WS.id, harness: "claude", status: "completed", prompt: "Fan out the upload fixes to Boat, one sub-thread each, and merge what comes back.", harnessTitle: "Fan out the upload fixes", startedBy: "person", startedAt: ago(80), endedAt: ago(70), readAt: ago(69), model: "claude-opus-5-5" } as SessionView;

const historyOf = (s: Sub): SessionEvent[] => {
  const scope = { workspaceId: `ws_${s.key}`, sessionId: `s_${s.key}`, turnId: `turn_${s.key}`, threadId: threadOf(s) };
  const start: SessionEvent = { type: "session.start", ...scope, agent: "claude", model: "claude-opus-5-5", cwd: "/root/web", prompt: PROMPT_OF[s.key] ?? s.title, at: s.startedAt } as SessionEvent;
  if (s.sub.state === "waiting" || s.sub.state === "starting") return [start];
  const work: SessionEvent = { type: "session.delta", ...scope, kind: "text", at: s.startedAt + MIN, text: "Fetched the branch and ran the upload suite 20 times: 2 failures, both in `resumes after a dropped part`, which sleeps 200 ms and then asserts the part was sent again." } as SessionEvent;
  if (s.endedAt === undefined) return [start, work];
  const reply: SessionEvent[] = s.reply === undefined ? [] : [{ type: "session.delta", ...scope, kind: "text", at: s.endedAt - 1000, text: `\n\n${s.reply}` } as SessionEvent];
  const done: SessionEvent = { type: "session.done", ...scope, at: s.endedAt, result: s.status === "failed" ? { status: "failed", error: s.failure, durationMs: s.endedAt - s.startedAt } : { status: "completed", durationMs: s.endedAt - s.startedAt, costUsd: 0.31 } } as SessionEvent;
  return [start, work, ...reply, done, { type: "session.end", ...scope, at: s.endedAt, exitCode: s.status === "failed" ? null : 0, sawResult: s.status !== "failed" } as SessionEvent];
};

const catalogs: HarnessCatalog[] = [
  {
    harness: "claude",
    label: "Claude Code",
    source: "harness",
    version: "2.1.286",
    models: [{ value: "claude-opus-5-5", label: "Opus 5.5", isDefault: true, contextWindows: [] }],
    efforts: [{ value: "high", label: "High", isDefault: true }],
    contextWindows: [],
    permissionModes: ACCESS_MODES,
    steers: true,
    renames: true,
    images: true,
    movesAccess: true,
    access: { ask: "default", "auto-edit": "acceptEdits", full: "bypassPermissions" },
    bypassMode: "bypassPermissions",
  },
];

const workspaces = (): WorkspaceView[] => [LEAD_WS, ...subs.map(wsOf)];
const sessions = (): SessionView[] => [LEAD, ...subs.map(sessionOf)];
const places = (): PlaceView[] => (state === "forgotten" || removed ? [HERE] : [HERE, boat]);
let removed = false;

const watching = new Set<(event: ProtocolEvent) => void>();
const statusFor = (w: WorkspaceView) =>
  w.kind === "local"
    ? statusOf(w, { kind: "local", size: { cpu: 10, memMb: 16384 }, rateUsdPerHour: 0, facts: { os: "macOS 26.1", uptimeMs: 3 * 86_400_000, folder: "/Users/dev/web" } })
    : statusOf(w, { size: SMALL, rateUsdPerHour: RATE, ...(w.phase === "released" ? { machineState: "gone", reach: { state: "gone" } } : w.phase === "queued" || w.phase === "waking" ? { machineState: "starting" } : {}) });

/** What places.set does on Boat's row: a set takes its word's value and marks it set, a reset puts the default back. */
function setBoat(ask: PlaceSettingsAsk, reset: ReadonlyArray<PlaceSettingWord> = []): PlaceView {
  const cap = { ...(boat.cap as { machines: number; spendPerDayUsd: number }) };
  const settings = { ...boat.settings };
  const next: Partial<PlaceView> = {};
  if (ask.machines !== undefined) (cap.machines = ask.machines), (settings.machines = ask.machines);
  if (ask.spendPerDayUsd !== undefined) (cap.spendPerDayUsd = ask.spendPerDayUsd), (settings.spendPerDayUsd = ask.spendPerDayUsd);
  if (ask.size !== undefined) (next.size = ask.size), (settings.size = ask.size);
  if (ask.deleteAfterMs !== undefined) (next.deleteAfterMs = ask.deleteAfterMs), (settings.deleteAfterMs = ask.deleteAfterMs);
  if (ask.napMs !== undefined) (next.napMs = ask.napMs), (settings.napMs = ask.napMs);
  if (ask.turnLimitMs !== undefined) (next.turnLimitMs = ask.turnLimitMs), (settings.turnLimitMs = ask.turnLimitMs);
  for (const word of reset) {
    if (word === "machines") (cap.machines = BOAT_DEFAULTS.cap.machines), delete settings.machines;
    if (word === "spend") (cap.spendPerDayUsd = BOAT_DEFAULTS.cap.spendPerDayUsd), delete settings.spendPerDayUsd;
    if (word === "size") (next.size = BOAT_DEFAULTS.size), delete settings.size;
    if (word === "delete-after") (next.deleteAfterMs = BOAT_DEFAULTS.deleteAfterMs), delete settings.deleteAfterMs;
    if (word === "nap") (next.napMs = boat.napDefault ?? null), delete settings.napMs;
    if (word === "turn-limit") (next.turnLimitMs = boat.turnLimitDefault ?? null), delete settings.turnLimitMs;
  }
  boat = { ...boat, ...next, cap, settings } as PlaceView;
  return boat;
}

const api: Api = {
  listWorkspaces: async () => workspaces(),
  getWorkspace: async id => workspaces().find(w => w.id === id) ?? LEAD_WS,
  createWorkspace: async () => LEAD_WS,
  watchStatuses: async () => workspaces().map(statusFor),
  forget: async () => {},
  nap: async () => LEAD_WS,
  wake: async () => LEAD_WS,
  capabilities: async () => caps(),
  startSession: async () => ({ id: "s_fix", workspaceId: "ws_fix", harness: "claude", status: "running" }),
  portReach: async (_id, port) => ({ url: `http://127.0.0.1:${port}`, expiresAt: Date.now() + 3_600_000 }),
  daemon: noDaemonApi,
  sessionHistory: async (workspaceId: string) => {
    const s = subs.find(x => `ws_${x.key}` === workspaceId);
    return s === undefined ? [] : historyOf(s);
  },
  markThreads: async () => {},
  setSessionAccess: async () => "set",
  listSnapshots: async () => ({ name: "default", head: null, versions: [] }),
  snapshotStorage: async () => null,
  rollbackSnapshot: async () => ({ lineage: { name: "default", head: null, versions: [] }, existingWorkspaces: "untouched" }),
  listSessions: async () => sessions(),
  renameSession: async () => ({ outcome: "renamed" }),
  renameWorkspace: async () => LEAD_WS,
  setWorkspaceLook: async () => LEAD_WS,
  listHarnesses: async () => catalogs,
  placesList: async () => ({ places: places(), adds: [] }),
  placesSet: async (_id: string, ask: PlaceSettingsAsk, reset?: ReadonlyArray<PlaceSettingWord>) => setBoat(ask, reset),
  placeHolds: async () => ({
    forks: [],
    projects: [],
    unsaved: subs.flatMap(s => (s.sub.held === undefined ? [] : [subHeldLine(threadOf(s), s.sub.held, s.branch)])),
  }),
  // The host's remove of a cloud: every machine deleted, the key forgotten, and place.removed the one event, after
  // which the app reads the rows again and finds each sub-thread released with its cloud gone.
  removePlace: async () => {
    removed = true;
    subs = subs.map(s => ({ ...s, phase: "released", status: s.status === "running" ? "completed" : s.status, endedAt: s.endedAt ?? Date.now(), sub: { state: "released", releasedAt: Date.now(), cloudGone: true, ...(s.sub.result === undefined ? {} : { result: s.sub.result }) } }));
    setTimeout(() => {
      for (const fn of watching) fn({ type: "place.removed", placeId: "box" } as ProtocolEvent);
    }, 0);
    return { removed: true, swept: [], deleted: 5 };
  },
  subscribe: fn => {
    watching.add(fn);
    return () => watching.delete(fn);
  },
  getGolden: async () => undefined,
  listProjectGoldens: async () => [],
} as Api;

const focused = subs[0]!;
useStore.setState({
  conn: "live",
  selectedId: `ws_${focused.key}`,
  places: places(),
  placesRead: true,
  projects: [PROJECT],
  projectsRead: true,
  costs: Object.fromEntries(subs.filter(s => s.accruedUsd !== undefined).map(s => [`ws_${s.key}`, { rateUsdPerHour: RATE, accruedUsd: s.accruedUsd!, at: new Date().toISOString() }])),
  preferences: { ...DEFAULT_PREFERENCES, theme, ...picks, labs: false, projectLook: { pr_web: { icon: "globe" as const, hue: "teal" as const } } },
});
useStore.getState().bind(api);
useStore.getState().select(`ws_${focused.key}`, threadOf(focused));
useRightPanelStore.setState({ byWorkspaceId: {} });
useRightPanelStore.getState().open(`ws_${focused.key}`, "machine");
/** Presses the first element a selector finds once the page has drawn it. */
function pressWhenDrawn(selector: string, tries = 40): void {
  const el = document.querySelector<HTMLElement>(selector);
  if (el !== null) el.click();
  else if (tries > 0) setTimeout(() => pressWhenDrawn(selector, tries - 1), 100);
}
// The Finished fold opens, as a person would, so every sibling's row is on the page.
if (state !== "boat" && state !== "remove" && params.get("fold") !== "shut") setTimeout(() => pressWhenDrawn('[data-child-fold="finished"]'), 200);
if (state === "boat" || state === "remove") {
  openSettingsGroup("computers");
  openComputerSettings("box");
  if (state === "remove") setTimeout(() => pressWhenDrawn('[data-k="remove"]'), 200);
}

/** A running sub-machine's live readings, two minutes of them and one more every two seconds: a test run's load and
 * memory moving on a small machine's 3,916 MB, the disk its 12 GB holds for files. */
if (focused.sub.state === "running") {
  const GiB = 1024 ** 3;
  const live = getLive(`ws_${focused.key}`);
  const sample = (i: number) => ({ type: "sys.sample" as const, cpu: 60 + 30 * Math.sin(i / 6), load1: 1.6 + 0.9 * Math.sin(i / 7) + 0.3 * Math.sin(i / 2.3), mem: { used: (2.2 + 0.5 * Math.sin(i / 9) + 0.12 * Math.sin(i / 2)) * GiB, total: (3916 / 1024) * GiB }, disk: { used: (6.1 + i * 0.002) * GiB, total: 12 * GiB }, at: Date.now() - (60 - i) * 2000 });
  setTimeout(() => {
    live.feedStatus("live");
    for (let i = 0; i < 60; i++) live.feedSample(sample(i));
    let i = 60;
    setInterval(() => live.feedSample({ ...sample(i++), at: Date.now() }), 2000);
  }, 300);
}

/** The floating switcher: one link per state and one per theme, gone with ?bar=0. */
function Switcher() {
  if (params.get("bar") === "0") return null;
  const to = (next: Partial<Record<"state" | "theme", string>>): string => {
    const q = new URLSearchParams(params);
    for (const [k, v] of Object.entries(next)) if (v !== undefined) q.set(k, v);
    return `?${q.toString()}`;
  };
  const link = (href: string, word: string, on: boolean) => (
    <a key={word} href={href} data-switch={word} className={on ? "text-foreground" : "text-muted-foreground hover:text-foreground"}>
      {word}
    </a>
  );
  return (
    <nav data-screen-switcher className="fixed right-4 bottom-4 z-50 flex max-w-[calc(100vw-2rem)] flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border border-border bg-popover px-3 py-1.5 font-mono text-[11px] tabular-nums shadow-[var(--popover-shadow)]">
      {STATES.map(name => link(to({ state: name }), name, name === state))}
      <span aria-hidden className="h-3 w-px bg-border" />
      {(["dark", "light"] as const).map(name => link(to({ theme: name }), name, name === theme))}
    </nav>
  );
}

createRoot(document.getElementById("root")!).render(
  <TooltipProvider>
    <AppShell>
      <div className="flex min-h-0 flex-1 flex-col" data-terminal-beside>
        <WorkspaceThread workspaceId={`ws_${focused.key}`} threadId={threadOf(focused)} />
      </div>
    </AppShell>
    <Switcher />
  </TooltipProvider>,
);
