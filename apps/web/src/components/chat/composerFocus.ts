// SPDX-License-Identifier: AGPL-3.0-only
import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import type { ComposerPromptEditorHandle } from "../ComposerPromptEditor";
import { handComposerFocus, onComposerFocusRequest } from "../../shell/shellRequests";

/** Takes the workspace's focus requests into its composer's editor. The focus waits for the render the request
 * came with: a road writes the draft and asks for focus on one tick, and the editor's focus reports the text it
 * holds, which before that render is the old text and would land back on the draft. Each thread has its own
 * composer, so a switch mounts the next one in this one's place: a caret that stood in `surface`, or a request taken
 * here with no render left to serve it, goes on to that one. */
export function useComposerFocusRequest(workspaceId: string, editorRef: RefObject<ComposerPromptEditorHandle | null>, surfaceRef: RefObject<HTMLElement | null>): void {
  const [asked, setAsked] = useState(0);
  const owed = useRef(false);
  useLayoutEffect(() => {
    if (asked === 0) return;
    owed.current = false;
    editorRef.current?.focus();
  }, [asked, editorRef]);
  // Layout effects, so the focus is read before the box leaves the page and this composer stops taking requests
  // before it hands one on.
  useLayoutEffect(() => {
    const surface = surfaceRef.current;
    const off = onComposerFocusRequest(workspaceId, () => {
      owed.current = true;
      setAsked(n => n + 1);
    });
    return () => {
      off();
      if (owed.current || (surface !== null && surface.contains(document.activeElement))) handComposerFocus(workspaceId);
    };
  }, [surfaceRef, workspaceId]);
}
