// SPDX-License-Identifier: AGPL-3.0-only
// The one confirmation before a thread that runs in a folder goes alone: its
// turns and checkpoints leave, the folder and the folder's other threads stay,
// and the worktree wsp made goes only with the last thread in it. It asks the
// host once and shows the host's refusal in place; the row leaves on the
// session.row events the host sends for each of the thread's rows.
import { useState } from "react";
import { threadDeleteQuestion } from "@wsp/protocol";
import { CLIENT_CANNOT_DELETE_THREAD } from "../actions/format.js";
import { errorText } from "../lib/utils.js";
import { useStore } from "../protocol/store.js";
import { RefusalSlot } from "../settings/sheetParts.js";
import { AlertDialog, AlertDialogClose, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogPopup, AlertDialogTitle } from "./ui/alert-dialog.js";
import { Button, NEUTRAL_RING } from "./ui/button.js";

export interface ThreadToDelete {
  readonly threadId: string;
  readonly workspaceId: string;
  readonly title: string;
  /** The worktree wsp made that goes with this thread, the last one in it. */
  readonly worktree?: string;
}

export function DeleteThreadDialog({ thread, onClose }: { thread: ThreadToDelete; onClose: () => void }) {
  const api = useStore(s => s.api);
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);

  const remove = async (): Promise<void> => {
    if (api?.deleteThread === undefined) {
      setRefusal(CLIENT_CANNOT_DELETE_THREAD);
      return;
    }
    setBusy(true);
    setRefusal(null);
    try {
      await api.deleteThread(thread.threadId);
      if (useStore.getState().selectedThreadId === thread.threadId) useStore.setState({ selectedThreadId: null, selectedSubagent: null });
      onClose();
    } catch (e) {
      setRefusal(errorText(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <AlertDialog open onOpenChange={next => !next && onClose()}>
      <AlertDialogPopup>
        <AlertDialogHeader>
          <AlertDialogTitle>{`Delete ${thread.title}?`}</AlertDialogTitle>
          <AlertDialogDescription>{threadDeleteQuestion(thread.threadId, thread.worktree).split("\n")[1]}</AlertDialogDescription>
        </AlertDialogHeader>
        <div className="px-5 pt-2">
          <RefusalSlot k="delete-thread-refusal" {...(refusal ? { said: refusal } : {})} />
        </div>
        <AlertDialogFooter>
          <AlertDialogClose render={<Button variant="outline" className={NEUTRAL_RING} />}>Cancel</AlertDialogClose>
          <Button variant="destructive" disabled={busy} onClick={() => void remove()} data-k="delete-thread">
            {busy ? "Deleting…" : "Delete"}
          </Button>
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}
