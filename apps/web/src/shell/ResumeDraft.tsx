// SPDX-License-Identifier: AGPL-3.0-only
// A project's New thread page holding a conversation picked off the resume picker: its earlier messages drawn by the
// thread's own timeline, shut in the Earlier fold a thread on it keeps, and the page's composer docked under them,
// whose send opens the thread. The read of those messages is the one thing it asks the host before the send.
import { useLayoutEffect, useMemo, useRef, type ReactNode } from "react";
import type { LegendListRef } from "@legendapp/list/react";
import { refusalLine, type ConversationRead, type SessionEvent } from "@wsp/protocol";
import { createSessionFold } from "../adapt/session.js";
import { DEFAULT_TIMESTAMP_FORMAT, type TimelineEntry, type TurnSummary } from "../components/chat/adapt.js";
import { MessagesTimeline } from "../components/chat/MessagesTimeline.js";
import { useAppDark } from "../settings/theme.js";
import { TRANSCRIPT_LOADING } from "../transcript-words.js";
import { isRead, useConversationsStore } from "./conversations.js";

const NO_TURNS: ReadonlyArray<TurnSummary> = [];
const noop = (): void => {};

/** The rows a thread on the conversation opens with, as its transcript's events would draw them. */
export function earlierEntries(read: ConversationRead, sessionId: string): ReadonlyArray<TimelineEntry> {
  const fold = createSessionFold();
  const at = Date.now();
  read.rows.forEach((row, i) => fold.add({ type: "session.earlier", workspaceId: "", sessionId, at, pos: i + 1, who: row.who, text: row.text } as SessionEvent));
  return fold.model().timeline;
}

export function ResumeDraft({ projectId, children }: { projectId: string; children: ReactNode }) {
  const draft = useConversationsStore(s => s.drafts[projectId]);
  const read = draft?.read;
  const appDark = useAppDark();
  const listRef = useRef<LegendListRef | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const composerRef = useRef<HTMLDivElement | null>(null);
  const entries = useMemo(() => (draft !== undefined && isRead(read) ? earlierEntries(read, draft.pick.row.id) : []), [draft, read]);
  useLayoutEffect(() => {
    const root = rootRef.current;
    const composer = composerRef.current;
    if (root === null || composer === null) return;
    const observer = new ResizeObserver(() => root.style.setProperty("--chat-composer-inset", `${composer.offsetHeight}px`));
    observer.observe(composer);
    return () => observer.disconnect();
  }, []);
  return (
    <div ref={rootRef} data-k="resume-draft" data-chat-view className="relative isolate min-h-0 flex-1 text-foreground">
      <div className="absolute inset-0">
        {read === undefined ? (
          <div className="flex h-full items-center justify-center pb-(--chat-composer-inset) text-sm text-muted-foreground">{TRANSCRIPT_LOADING}</div>
        ) : !isRead(read) ? (
          <div data-refused className="flex h-full items-center justify-center px-6 pb-(--chat-composer-inset) text-center text-sm text-muted-foreground">
            {read.fix === undefined ? read.said : refusalLine(read.said, read.fix)}
          </div>
        ) : (
          <MessagesTimeline
            isWorking={false}
            activeTurnStartedAt={null}
            listRef={listRef}
            timelineEntries={entries}
            turns={NO_TURNS}
            threadKey={`resume/${projectId}/${draft?.pick.row.id ?? ""}`}
            onImageExpand={noop}
            markdownCwd={read.cwd}
            workspaceRoot={read.cwd}
            resolvedTheme={appDark ? "dark" : "light"}
            timestampFormat={DEFAULT_TIMESTAMP_FORMAT}
          />
        )}
      </div>
      <div ref={composerRef} data-chat-composer-dock className="pointer-events-none absolute inset-x-0 bottom-0 z-10 *:pointer-events-auto">
        {children}
      </div>
    </div>
  );
}
