// SPDX-License-Identifier: AGPL-3.0-only
// What one of Activity's rows offers, one registry: the act its row shows
// while pointed at, and its menu, which below 768 px is the only road to the
// acts. An agent is stopped through its thread and never ended; what wsp
// needs takes no act; a heading over a slate's runs is no process. A
// thread's head has its own two, End all and Open thread.
import { CircleStopIcon, ClipboardCopyIcon, Globe2Icon, MessageSquareIcon, SquareXIcon } from "lucide-react";
import type { ActionEntry } from "../../actions/registry.js";
import { ACTIVITY_WORDS as W } from "../format.js";
import type { ActivityGroup, ActivityPort, ActivityProc } from "./model.js";

export interface ProcTarget {
  readonly p: ActivityProc;
  /** Null for a row of the rest of the computer, which wsp did not start and offers nothing to end. */
  readonly group: ActivityGroup | null;
  /** The port a Ports row stands for; a row elsewhere opens its process's first. */
  readonly port?: ActivityPort;
  /** The workspace a browser tab at its port opens in; absent where none is known. */
  readonly workspaceId?: string;
}

export interface ProcVerbs {
  readonly ending: ReadonlySet<number>;
  end(p: ActivityProc): void;
  stop(p: ActivityProc, group: ActivityGroup): void;
  openPort(workspaceId: string, port: number, threadId: string | null): void;
  openThread(group: ActivityGroup): void;
  copy(text: string): void;
}

const isProc = (t: ProcTarget): boolean => t.p.heading !== true;
const portOf = (t: ProcTarget): ActivityPort | undefined => t.port ?? t.p.ports[0];
const busy = (t: ProcTarget, v: ProcVerbs): string | null => (v.ending.has(t.p.pid) ? W.ending : null);

export const procActions: ReadonlyArray<ActionEntry<ProcTarget, ProcVerbs>> = [
  {
    id: "open-port",
    group: "act",
    icon: () => Globe2Icon,
    applies: t => isProc(t) && portOf(t) !== undefined && t.workspaceId !== undefined,
    title: t => W.openPort(portOf(t)!.port),
    rowLabel: () => W.open,
    refusal: () => null,
    run: (t, v) => v.openPort(t.workspaceId!, portOf(t)!.port, t.group?.thread?.id ?? null),
  },
  {
    id: "stop",
    group: "act",
    icon: () => CircleStopIcon,
    applies: t => isProc(t) && t.p.kind === "agent" && t.p.needed !== true && t.group?.thread?.working === true,
    title: () => W.stopTurn,
    rowLabel: () => W.stop,
    refusal: busy,
    run: (t, v) => v.stop(t.p, t.group!),
  },
  {
    id: "end",
    group: "act",
    icon: () => SquareXIcon,
    destructive: true,
    applies: t => isProc(t) && t.group !== null && t.p.kind !== "agent" && t.p.needed !== true,
    title: () => W.end,
    rowLabel: () => W.end,
    refusal: busy,
    run: (t, v) => v.end(t.p),
  },
  {
    id: "open-thread",
    group: "go",
    icon: () => MessageSquareIcon,
    applies: t => t.group?.thread !== undefined,
    title: () => W.openThread,
    refusal: () => null,
    run: (t, v) => v.openThread(t.group!),
  },
  {
    id: "copy-command",
    group: "copy",
    icon: () => ClipboardCopyIcon,
    applies: t => isProc(t) && t.p.command !== "",
    title: () => W.copyCommand,
    refusal: () => null,
    run: (t, v) => v.copy(t.p.command),
  },
  {
    id: "copy-pid",
    group: "copy",
    icon: () => ClipboardCopyIcon,
    applies: isProc,
    title: () => W.copyPid,
    refusal: () => null,
    run: (t, v) => v.copy(String(t.p.pid)),
  },
];

export interface GroupVerbs {
  endAll(group: ActivityGroup): void;
  openThread(group: ActivityGroup): void;
}

/** What a thread's head offers: End all, which asks first, and Open thread. Below 768 px its menu is the road to them. */
export const groupActions: ReadonlyArray<ActionEntry<ActivityGroup, GroupVerbs>> = [
  {
    id: "end-all",
    group: "act",
    icon: () => SquareXIcon,
    destructive: true,
    applies: g => g.thread !== undefined,
    title: () => W.endAll,
    rowLabel: () => W.endAll,
    refusal: () => null,
    run: (g, v) => v.endAll(g),
  },
  {
    id: "open-thread",
    group: "go",
    icon: () => MessageSquareIcon,
    applies: g => g.thread !== undefined,
    title: () => W.openThread,
    refusal: () => null,
    run: (g, v) => v.openThread(g),
  },
];
