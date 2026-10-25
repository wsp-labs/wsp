// SPDX-License-Identifier: AGPL-3.0-only
// The host's events that become notices, one rule per event type. A notice is
// for what happened away from where the person is looking: an event whose home
// is on screen (that thread open, that computer's row showing, that build's
// rows drawn) is the panel's to show and says nothing here, except how a
// setup ended, said once wherever the person is unless Add a computer is in
// front of that computer, whose close says it instead. A computer going
// quiet is the machine's life and never a notice: the thread on screen and the
// computer's row say it where it matters. A wait (a build's
// need, a thread's prompt) is keyed and stands until the event that closes it,
// and a build's need is said when its rows leave the screen with it standing.
// The need, the prompt, a machine that came up and a finished turn also go out
// on the shell's own road, which speaks only while the app is not in front of
// the person, each as the person chose for its kind on General, by the one
// rule in the protocol: a need and a prompt as one that needs them, a finish,
// a failure and a machine that came up as things that finished. An account's
// plan running low, blocked or back is a notice and a notification with no
// sound while the person keeps that switch on. The
// dock's badge counts the threads waiting on the person. A newer release is
// never a notice: the sidebar's update card says it.
import { NOTIFY_ME, waitLine, setupNeedsYouLine, setupReadyLine, askingLine, exitLine, foldThreads, needsYouCount, outsideLine, planAlertLine, threadFinishedLine, threadStoppedLine, titleWithNeed, type OutsideLine, type OutsideMoment, type PlaceView, type TurnResult } from "@wsp/protocol";
import { useCallback, useEffect, useRef } from "react";
import type { ProtocolEvent } from "../protocol/client.js";
import { threadOnScreen, useProtocolEvents, useStore } from "../protocol/store.js";
import { desktopBridge } from "../lib/desktopShell.js";
import { placeName } from "../settings/places.js";
import { openSetup, sayEnded, useAddFlow } from "../settings/add/addFlow.js";
import { useSettingsStore } from "../settings/settingsStore.js";
import { needsYouRoad, type NeedsYouRoad } from "../shell/needsYou.js";
import { copyName } from "../sidebar/workspaceRows.js";
import { addNotice, useNotices, type NoticeAction, type NoticeInput } from "./store.js";

/** A turn whose session.end never arrived (a socket gap) is not held past this many later turns. */
const RESULTS_HELD = 64;

export const HOST_NOTICE_WORDS = {
  open: "Open",
  notAdded: (said: string): string => `Computer not added: ${said}`,
  joined: (name: string): string => `${name} joined`,
  notSetUp: (name: string, said: string): string => `${name} not set up: ${said}`,
  aThread: "A thread",
  threadStopped: (title: string, said: string | undefined): string => `${title} stopped before it replied${said === undefined ? "" : `: ${said}`}`,
  threadFinished: threadFinishedLine,
  threadFailed: threadStoppedLine,
  gone: (name: string, reason: string): string => `${name} is gone: ${reason}`,
};

const askKey = (workspaceId: string, threadId: string | undefined, askId: string): string => `${askPrefix(workspaceId, threadId)}${askId}`;
const askPrefix = (workspaceId: string, threadId: string | undefined): string => `ask:${workspaceId}:${threadId ?? workspaceId}:`;

function workspaceNamed(id: string): string | undefined {
  const { workspaces, statuses, places } = useStore.getState();
  const workspace = workspaces.find(w => w.id === id);
  return workspace === undefined ? undefined : copyName(places, { workspace, status: statuses[id] ?? null, displayName: workspace.name });
}

function threadTitle(workspaceId: string, threadId: string | undefined): string {
  return threadOf(workspaceId, threadId)?.title ?? HOST_NOTICE_WORDS.aThread;
}

const threadOf = (workspaceId: string, threadId: string | undefined) => foldThreads(useStore.getState().sessions[workspaceId] ?? []).find(t => t.id === threadId);

/** The line a moment makes outside the app under the person's choices, by the one rule the desktop shell reads too. */
const lineFor = (moment: OutsideMoment): OutsideLine | undefined => outsideLine(moment, useStore.getState().preferences);

const onScreenNow = (e: { workspaceId: string; sessionId: string; threadId?: string | undefined }): boolean => threadOnScreen(useStore.getState(), e);

const workspaceOnScreen = (workspaceId: string): boolean => !useStore.getState().settingsOpen && useStore.getState().selectedId === workspaceId;

function computerOnScreen(placeId: string | undefined): boolean {
  const s = useStore.getState();
  const flow = useAddFlow.getState();
  if (flow.open && placeId !== undefined && flow.placeId === placeId) return true;
  if (!s.settingsOpen) return false;
  const at = useSettingsStore.getState().at;
  return s.addComputerOpen || (at.kind === "group" && at.group === "computers") || (at.kind === "computer" && at.id === placeId);
}

const openUsage: NoticeAction = {
  word: HOST_NOTICE_WORDS.open,
  run: () => {
    useSettingsStore.getState().go({ kind: "group", group: "usage" });
    useStore.getState().openSettings();
  },
};

const openThread = (workspaceId: string, threadId: string | undefined): NoticeAction => ({ word: HOST_NOTICE_WORDS.open, run: () => useStore.getState().select(workspaceId, threadId ?? null) });
/** A setup's notice opens Add a computer on that computer's running steps, where whatever it asks is answered. */
const openSetupOf = (placeId: string): NoticeAction => ({ word: HOST_NOTICE_WORDS.open, run: () => openSetup(placeId) });

const openComputer = (placeId: string | undefined): NoticeAction => ({
  word: HOST_NOTICE_WORDS.open,
  run: () => {
    useSettingsStore.getState().go(placeId === undefined ? { kind: "group", group: "computers" } : { kind: "computer", id: placeId });
    useStore.getState().openSettings();
  },
});

/** What the rules keep between events, for one mounted effect. */
interface Held {
  road: NeedsYouRoad | null;
  /** Each running turn's result, by turn id, from its session.done until the session.end that always follows it. */
  results: Map<string, TurnResult>;
  /** The sign-in waits said, by computer and sign-in row, with the computer and the run they were said for: each is
   * said once while it stands. */
  waits: Map<string, { placeId: string; addId: string }>;
  /** The setups that ended, by computer, with the record as it stood at the end: the end rides ahead of the rows it
   * ended with, so how it ended waits for the list's next read. */
  ready: Map<string, { addId: string; seen: PlaceView }>;
}

/** The key a sign-in's wait is said under, one per computer and sign-in, so the frames it rides say it once. */
const waitKey = (placeId: string, row: string): string => `${placeId}/${row}`;
const waitNoticeKey = (key: string): string => `sign-in:${key}`;

/** A wait said before that left its computer's record, the person signed in or the run moved on: its toast goes and
 * the next wait for that sign-in is said again. */
function settleWaits(h: Held, places: readonly PlaceView[]): void {
  for (const [key, { placeId, addId }] of h.waits) {
    const setup = places.find(p => p.id === placeId)?.setup;
    // A record that has not caught up with the run that said the wait says nothing about it yet.
    if (setup?.addId !== addId || setup.waiting.some(w => waitKey(placeId, w.row) === key && w.state === "waiting")) continue;
    endWait(h, key);
  }
}

/** The read after a setup's end: how it ended, said once a run off the rows it brings, unless the dialog is in front
 * of that computer, whose close says it. */
function settleReady(h: Held, places: readonly PlaceView[]): void {
  for (const [placeId, { addId, seen }] of h.ready) {
    const place = places.find(p => p.id === placeId);
    // The read brings the run afresh: the computer coming or going keeps the run it had, and a read asked before the
    // end, which the client shares, can answer after it with the run still going.
    if (place !== undefined && (place.setup === seen.setup || (place.setup?.addId === addId && place.setup.state === "running"))) continue;
    h.ready.delete(placeId);
    const flow = useAddFlow.getState();
    if (place?.setup?.addId === addId && place.setup.state === "done" && place.setup.waiting.length === 0 && !(flow.open && flow.placeId === placeId)) sayEnded(place);
  }
}

function endWait(h: Held, key: string): void {
  h.waits.delete(key);
  useNotices.getState().end(waitNoticeKey(key));
}

function sayOutside(held: Held, opens: () => void, line: OutsideLine | undefined): void {
  if (line !== undefined) held.road?.say(line, opens);
}

function endAsks(workspaceId: string, threadId: string | undefined): void {
  const prefix = askPrefix(workspaceId, threadId);
  const notices = useNotices.getState();
  for (const key of new Set(notices.notices.map(n => n.key).filter((k): k is string => k?.startsWith(prefix) === true))) notices.end(key);
}

type Rule<T extends ProtocolEvent["type"]> = (e: Extract<ProtocolEvent, { type: T }>, held: Held) => void;

/** One rule per event type that says anything; a type with no rule is noise. */
const RULES: { [T in ProtocolEvent["type"]]?: Rule<T> } = {
  "place.stage": e => {
    if (e.state === "running" || (e.state === "done" && e.step !== "join")) return;
    if (computerOnScreen(e.placeId)) return;
    const place = e.placeId === undefined ? undefined : useStore.getState().places.find(p => p.id === e.placeId);
    const name = place === undefined ? undefined : placeName(place);
    const where = name === undefined ? {} : { where: name };
    if (e.state === "failed") {
      const said = e.note ?? e.step;
      addNotice({ kind: "error", text: HOST_NOTICE_WORDS.notAdded(said), ...where, action: openComputer(e.placeId) });
      return;
    }
    if (name === undefined) return;
    addNotice({ kind: "done", text: HOST_NOTICE_WORDS.joined(name), ...where, action: openComputer(e.placeId) });
  },
  // A setup's end, said once it is over: failed, waiting on the person, or ready, inside the app and outside it.
  "place.setup": (e, held) => {
    // A wait that ran out, or a run that ended, is said by the run's own words: the wait's toast goes.
    if (e.wait?.state === "expired") endWait(held, waitKey(e.placeId, e.wait.row));
    if (e.end !== undefined) for (const [key, at] of held.waits) if (at.placeId === e.placeId) endWait(held, key);
    if (e.end === undefined && e.wait?.state !== "waiting") return;
    const place = useStore.getState().places.find(p => p.id === e.placeId);
    if (place === undefined) return;
    const name = placeName(place);
    const open = openSetupOf(e.placeId);
    // The shell is told even with the dialog on screen, since only it knows whether the window still has the
    // person's eyes; the toast is for a computer whose own rows are not showing.
    const toast = (notice: NoticeInput): void => {
      if (!computerOnScreen(e.placeId)) addNotice(notice);
    };
    if (e.end === undefined && e.wait !== undefined) {
      // The runtime says a wait again as the relay reads its page and then its code: the toast takes the newest, and
      // the person is told outside the app once.
      const key = waitKey(e.placeId, e.wait.row);
      const what = waitLine(e.wait);
      toast({ kind: "waiting", key: waitNoticeKey(key), text: setupNeedsYouLine(name, what), where: name, action: open });
      if (held.waits.has(key)) return;
      held.waits.set(key, { placeId: e.placeId, addId: e.addId });
      sayOutside(held, open.run, lineFor({ kind: "signIn", what: `${name}: ${what}` }));
      return;
    }
    if (e.end === "failed") {
      toast({ kind: "error", text: HOST_NOTICE_WORDS.notSetUp(name, e.said ?? ""), where: name, action: open });
      sayOutside(held, open.run, lineFor({ kind: "setupFailed", computer: name, said: e.said ?? "" }));
    } else if (e.end === "needs-you") {
      // A sign-in still waiting is said now; a folder or GitHub that did not land waits for the read that brings it.
      const run = place.setup?.addId === e.addId ? place.setup : undefined;
      if (run === undefined || run.waiting.length > 0) toast({ kind: "note", text: setupNeedsYouLine(name, e.said ?? ""), where: name, action: open });
      else held.ready.set(e.placeId, { addId: e.addId, seen: place });
      sayOutside(held, open.run, lineFor({ kind: "setupNeedsYou", computer: name, what: e.said ?? "" }));
    } else {
      held.ready.set(e.placeId, { addId: e.addId, seen: place });
      sayOutside(held, open.run, lineFor({ kind: "setupReady", computer: name, ...(e.said !== undefined ? { missed: e.said } : {}) }));
    }
  },
  "session.done": (e, held) => {
    if (e.turnId === undefined) return;
    held.results.set(e.turnId, e.result);
    if (held.results.size > RESULTS_HELD) held.results.delete(held.results.keys().next().value!);
  },
  "session.end": (e, held) => {
    endAsks(e.workspaceId, e.threadId);
    const result = e.turnId === undefined ? undefined : held.results.get(e.turnId);
    if (e.turnId !== undefined) held.results.delete(e.turnId);
    // A finish is said for a thread a person or the command line opened: an agent's own threads report to it.
    const thread = threadOf(e.workspaceId, e.threadId);
    if (result?.status === "completed" && e.reason === undefined && thread !== undefined && thread.startedBy !== "agent") {
      const { workspaceId, threadId } = e;
      sayOutside(held, () => useStore.getState().select(workspaceId, threadId ?? null), lineFor({ kind: "finished", thread: thread.title, where: workspaceNamed(workspaceId) ?? "" }));
    }
    // A failure is said the same way, with its error on the line under it. A reason means the runtime ended it (a
    // pause, a delete, the machine gone, which is its own notice), and an interrupted turn is one somebody stopped:
    // neither is a failure to say.
    if (result?.status === "failed" && e.reason === undefined && thread !== undefined && thread.startedBy !== "agent") {
      const { workspaceId, threadId } = e;
      const error = result.error ?? exitLine(e.exitCode);
      sayOutside(held, () => useStore.getState().select(workspaceId, threadId ?? null), lineFor({ kind: "failed", thread: thread.title, where: workspaceNamed(workspaceId) ?? "", error }));
    }
    if (e.exitCode === 0 || e.sawResult || e.reason !== undefined || result?.status === "interrupted" || onScreenNow(e)) return;
    const where = workspaceNamed(e.workspaceId);
    const said = result?.error ?? exitLine(e.exitCode);
    addNotice({ kind: "error", text: HOST_NOTICE_WORDS.threadStopped(threadTitle(e.workspaceId, e.threadId), said), ...(where === undefined ? {} : { where }), action: openThread(e.workspaceId, e.threadId) });
  },
  "session.permission": (e, held) => {
    const { workspaceId, threadId } = e;
    sayOutside(held, () => useStore.getState().select(workspaceId, threadId ?? null), lineFor({ kind: "asks", line: askingLine(e) }));
    if (onScreenNow(e)) return;
    const where = workspaceNamed(workspaceId);
    addNotice({ kind: "waiting", key: askKey(workspaceId, threadId, e.askId), text: askingLine(e), ...(where === undefined ? {} : { where }), action: openThread(workspaceId, threadId) });
  },
  "session.permission.closed": e => useNotices.getState().end(askKey(e.workspaceId, e.threadId, e.askId)),
  "session.notify": e => {
    if (e.notify !== NOTIFY_ME || onScreenNow(e)) return;
    const where = workspaceNamed(e.workspaceId);
    addNotice({ kind: "note", text: e.text, ...(where === undefined ? {} : { where }), action: openThread(e.workspaceId, e.threadId) });
  },
  "workspace.gone": e => {
    const name = workspaceNamed(e.workspaceId);
    if (name === undefined || workspaceOnScreen(e.workspaceId)) return;
    addNotice({ kind: "error", text: HOST_NOTICE_WORDS.gone(name, e.reason), where: name, action: openThread(e.workspaceId, undefined) });
  },
  "usage.alert": (e, held) => {
    if (!useStore.getState().preferences.planAlerts) return;
    const text = planAlertLine(e.label, e.alert);
    addNotice({ kind: e.alert.kind === "back" ? "done" : "note", text, action: openUsage });
    sayOutside(held, openUsage.run, lineFor({ kind: "plan", label: e.label, alert: e.alert }));
  },
  "host.notice": e => addNotice({ kind: "note", text: e.message }),
  "workspace.woken": (e, held) => {
    const name = workspaceNamed(e.workspaceId);
    if (name !== undefined) sayOutside(held, () => useStore.getState().select(e.workspaceId), lineFor({ kind: "awake", workspace: name }));
  },
};

/** Mounted once under the store: every rule above on the host's events, the window's title carrying the mark while
 * a prompt stands. */
export function useHostNotices(): void {
  // A thread stopped on a permission prompt is a fact to a person looking somewhere else, so the window's own title
  // carries the mark.
  const needed = useStore(s => Object.values(s.sessions).some(rows => rows.some(row => row.asking !== undefined)));
  // Counted where the rows land, so a window showing a thread, which moves its read stamp, takes it off the dock.
  const waiting = useStore(s => needsYouCount(Object.values(s.sessions).flat()));
  const held = useRef<Held>({ road: null, results: new Map(), waits: new Map(), ready: new Map() });
  useEffect(() => {
    const h = held.current;
    const built = needsYouRoad();
    h.road = built;
    return () => {
      built.close();
      h.road = null;
    };
  }, []);
  useEffect(() => {
    document.title = titleWithNeed(document.title, needed);
  }, [needed]);
  useEffect(() => {
    desktopBridge()?.setBadge?.(waiting);
  }, [waiting]);
  useEffect(() => {
    const h = held.current;
    const offStore = useStore.subscribe((s, prev) => {
      if (s.places !== prev.places && h.waits.size > 0) settleWaits(h, s.places);
      if (s.places !== prev.places && h.ready.size > 0) settleReady(h, s.places);
    });
    return () => {
      offStore();
    };
  }, []);
  const onEvent = useCallback((e: ProtocolEvent) => {
    (RULES[e.type] as ((e: ProtocolEvent, held: Held) => void) | undefined)?.(e, held.current);
  }, []);
  useProtocolEvents(onEvent);
}
