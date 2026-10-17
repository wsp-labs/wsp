// SPDX-License-Identifier: AGPL-3.0-only
// The one confirmation before a thread that runs in a folder goes alone: its
// turns and checkpoints leave, the folder and the folder's other threads stay,
// and the worktree wsp made goes only with the last thread in it. Where the
// thread still runs processes beside its agent it lists them, as a settle does,
// and the delete stops them or keeps them running; a stop that left one running
// says so before the dialog closes. It shows the host's refusal in place; the row
// leaves on the session.row events the host sends for each of the thread's rows.
import { useEffect, useState } from "react";
import { threadDeleteQuestion, type LeftThread, type ProcessesAsk } from "@wsp/protocol";
import { CLIENT_CANNOT_DELETE_THREAD } from "../actions/format.js";
import { errorText } from "../lib/utils.js";
import { RequestError } from "../protocol/client.js";
import { useStore } from "../protocol/store.js";
import { ACTIVITY_WORDS as W } from "../settings/format.js";
import { RefusalSlot } from "../settings/sheetParts.js";
import { standingLine } from "./procs/leftAsk.js";
import { leftCount, leftStops, ProcessesLeftList } from "./procs/ProcessesLeft.js";
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
  const [busy, setBusy] = useState<ProcessesAsk | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const [left, setLeft] = useState<readonly LeftThread[] | null>(null);
  const [standing, setStanding] = useState<string | null>(null);
  const leftOf = (e: unknown): readonly LeftThread[] | undefined => (e instanceof RequestError ? e.left : undefined);

  // A check deletes nothing: it reads whether the thread still runs processes, so the question names them at once.
  useEffect(() => {
    let live = true;
    api?.deleteThread?.(thread.threadId, { processes: "ask", check: true }).catch((e: unknown) => {
      const found = leftOf(e);
      if (live && found !== undefined) setLeft(found);
    });
    return () => {
      live = false;
    };
  }, [api, thread.threadId]);

  const remove = async (processes: ProcessesAsk): Promise<void> => {
    if (api?.deleteThread === undefined) {
      setRefusal(CLIENT_CANNOT_DELETE_THREAD);
      return;
    }
    setBusy(processes);
    setRefusal(null);
    try {
      const gone = await api.deleteThread(thread.threadId, { processes });
      if (useStore.getState().selectedThreadId === thread.threadId) useStore.setState({ selectedThreadId: null, selectedSubagent: null });
      const still = standingLine(gone.ended);
      if (still === null) onClose();
      else setStanding(still);
    } catch (e) {
      const found = leftOf(e);
      if (found !== undefined) setLeft(found);
      else setRefusal(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <AlertDialog open onOpenChange={next => !next && onClose()}>
      <AlertDialogPopup>
        <AlertDialogHeader>
          <AlertDialogTitle>{`Delete ${thread.title}?`}</AlertDialogTitle>
          <AlertDialogDescription>{threadDeleteQuestion(thread.threadId, thread.worktree).split("\n")[1]}</AlertDialogDescription>
        </AlertDialogHeader>
        {left === null ? null : (
          <div className="px-5 pt-2">
            <ProcessesLeftList left={left} />
          </div>
        )}
        <div className="px-5 pt-2">
          <RefusalSlot k="delete-thread-refusal" {...(standing !== null ? { said: standing } : refusal ? { said: refusal } : left !== null ? { note: W.leftDelete(leftCount(left), leftStops(left)) } : {})} />
        </div>
        <AlertDialogFooter>
          {standing !== null ? (
            <Button variant="outline" className={NEUTRAL_RING} onClick={onClose} data-k="delete-close">
              {W.close}
            </Button>
          ) : (
            <AlertDialogClose render={<Button variant="outline" className={NEUTRAL_RING} />}>Cancel</AlertDialogClose>
          )}
          {standing !== null ? null : left === null ? (
            <Button variant="destructive" disabled={busy !== null} onClick={() => void remove("ask")} data-k="delete-thread">
              {busy !== null ? "Deleting…" : "Delete"}
            </Button>
          ) : (
            <>
              <Button variant="outline" className={NEUTRAL_RING} disabled={busy !== null} onClick={() => void remove("keep")} data-k="delete-keep">
                {W.deleteKeep}
              </Button>
              <Button variant="destructive" disabled={busy !== null} onClick={() => void remove("stop")} data-k="delete-stop">
                {busy === "stop" ? W.stopping : W.deleteStop}
              </Button>
            </>
          )}
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}
