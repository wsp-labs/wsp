// SPDX-License-Identifier: AGPL-3.0-only
// What the runtime's slates answer, which serve-slates.ts and the runtime call; slates.ts builds it.
import type { Caller, SlateEventAnswer, SlateReadAnswer, SlateStateAnswer, SlateView, SlateWriteAnswer, SlatesImageAnswer, SlatesPageAnswer } from "@wsp/protocol";
import type { SlateJson } from "@wsp/protocol/slate";

export interface SlateTarget {
  threadId?: string;
  turnToken?: string;
}

export interface Slates {
  get(threadId: string): Promise<SlateView | null>;
  write(p: SlateTarget & { text?: string; document?: Record<string, unknown>; check?: boolean; ifVersion?: number; values?: Record<string, unknown>; press?: { piece: string; index?: number; action?: number } }, caller?: Caller): Promise<SlateWriteAnswer>;
  state(p: SlateTarget & { values?: Record<string, unknown>; start?: string[]; ifVersion?: number }, caller?: Caller): Promise<SlateStateAnswer>;
  read(p: SlateTarget & { values?: string[]; text?: boolean; sketch?: boolean; document?: boolean }, caller?: Caller): Promise<SlateReadAnswer>;
  /** With a thread, a name that is one of its agent's MCP servers answers that server's tools; `look` says the look
   * rule, where an app here draws looks. */
  catalog(p: SlateTarget & { name?: string; look?: boolean }, caller?: Caller): Promise<{ text: string }>;
  shown(threadId: string): Promise<void>;
  /** The person withdraws one standing approval: the runs it covered stop, leave the queue and their timers, and the
   * next start asks again. */
  revoke(p: { threadId: string; key: string }): Promise<void>;
  event(p: { threadId: string; version: number; piece: string; event: "press" | "submit" | "change"; requestId: string; scope?: { item?: unknown; index: number }; rowAction?: number }): Promise<SlateEventAnswer>;
  approve(p: { threadId: string; key: string; scope: "once" | "thread" | "refuse" }): Promise<void>;
  cancel(p: { threadId: string; run: string }): Promise<void>;
  /** A window's hold on sources for a thread's slate; the release goes when the window lets go or its socket closes. */
  subscribe(p: { threadId: string; sources: string[] }): () => void;
  resolve(p: { threadId: string; paths: string[] }): Promise<{ values: Record<string, SlateJson> }>;
  image(p: { threadId: string; src: string; have?: string }): Promise<SlatesImageAnswer>;
  page(p: { threadId: string; src: string; have?: string }): Promise<SlatesPageAnswer>;
  /** The thread whose slate a look draws, read as a read is, refused where it has no slate yet. */
  lookAt(p: SlateTarget, caller?: Caller): Promise<string>;
  /** A turn ended: its snapshot is written. */
  turnEnded(o: { threadId: string; turnId: string }): Promise<void>;
  /** The runtime rewound a thread to the end of turnId, cutting the turns named. */
  rewound(o: { threadId: string; turnId: string; cut: readonly string[] }): Promise<void>;
  /** With `keep`, the thread's running runs and kept servers are left running. */
  forget(threadId: string, o?: { keep?: boolean }): Promise<void>;
  /** The thread's kept MCP servers closed, as a settle that stops what the thread left running asks. */
  closeServers(threadId: string): void;
  /** Whether a slate in the workspace watches the pull request's checks. */
  watchesPr(workspaceId: string): boolean;
  /** The workspace's machine napped: its threads' MCP servers there are closed, and the next run starts them again. */
  napped(workspaceId: string): void;
  /** The stored slates loaded, runs that were running when the host stopped marked failed and their done fired. */
  ready(): Promise<void>;
  /** Every batch queued so far has run. */
  settled(): Promise<void>;
  close(): void;
}
