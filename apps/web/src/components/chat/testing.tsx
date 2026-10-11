// SPDX-License-Identifier: AGPL-3.0-only
// The real prompt editor wired to a thread's draft and its workspace's focus requests the way ChatComposer wires it,
// for tests of the roads that write into the composer from outside it.
import { useCallback, useEffect, useRef } from "react";
import { ComposerPromptEditor, type ComposerPromptEditorHandle } from "../ComposerPromptEditor";
import { useComposerDraft, useComposerDraftStore } from "./composerDraftStore";
import { useComposerFocusRequest } from "./composerFocus";

export function MountedComposer({ workspaceId, threadKey = workspaceId }: { workspaceId: string; threadKey?: string }) {
  const draft = useComposerDraft(threadKey);
  const setDraft = useComposerDraftStore(s => s.setDraft);
  const editorRef = useRef<ComposerPromptEditorHandle | null>(null);
  const surfaceRef = useRef<HTMLDivElement | null>(null);
  useComposerFocusRequest(workspaceId, editorRef, surfaceRef);
  useEffect(() => {
    const drafts = useComposerDraftStore.getState();
    drafts.show(workspaceId, threadKey);
    return () => drafts.hide(workspaceId, threadKey);
  }, [threadKey, workspaceId]);
  const onChange = useCallback((value: string, cursor: number) => setDraft(threadKey, { prompt: value, cursor }), [setDraft, threadKey]);
  return (
    <div ref={surfaceRef}>
      <ComposerPromptEditor editorRef={editorRef} value={draft.prompt} cursor={draft.cursor} disabled={false} placeholder="Ask" onChange={onChange} />
    </div>
  );
}
