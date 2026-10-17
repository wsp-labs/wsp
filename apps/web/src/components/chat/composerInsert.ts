// SPDX-License-Identifier: AGPL-3.0-only
// The one road a block chip takes into a thread's draft from outside the
// composer: a terminal's "Add to chat", a reply's Quote and a slate's fill, to
// the thread named or else the one on screen. The block lands at
// the draft's caret on lines of its own, the caret goes after it, and the
// composer is asked for focus, so the next keystroke writes the message.
import { collapseExpandedComposerCursor, expandCollapsedComposerCursor, insertComposerBlock } from "../../composer-logic";
import { requestComposerFocus } from "../../shell/shellRequests";
import { composerKeyIn, EMPTY_DRAFT, useComposerDraftStore } from "./composerDraftStore";

export function insertIntoComposer(workspaceId: string, block: string, threadKey: string = composerKeyIn(workspaceId)): void {
  const store = useComposerDraftStore.getState();
  const draft = store.drafts[threadKey] ?? EMPTY_DRAFT;
  const next = insertComposerBlock(draft.prompt, expandCollapsedComposerCursor(draft.prompt, draft.cursor), block);
  store.setDraft(threadKey, { prompt: next.text, cursor: collapseExpandedComposerCursor(next.text, next.cursor) });
  requestComposerFocus(workspaceId);
}
