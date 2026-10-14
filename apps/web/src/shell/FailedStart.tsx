// SPDX-License-Identifier: AGPL-3.0-only
// The centre for a workspace whose create failed before any machine stood: a
// fork a thread asked for that the host keeps under that thread, failed with
// why. It has no history to read and nothing to send to, so the page is the
// card a refused create ends on, the step it stopped on with the reason, and
// what to do next, which is the asking thread's: start it again from there.
import type { WorkspaceView } from "@wsp/protocol";
import { Button } from "../components/ui/button.js";
import { SetupRoom } from "../components/chat/SetupCard.js";
import { useStore } from "../protocol/store.js";
import { couldNotStart } from "../protocol/store/creations.js";
import { StepRow } from "../settings/add/StepRow.js";
import { Card } from "../settings/rows.js";
import { creationSteps } from "./creationLog.js";

export const FAILED_START_WORDS = {
  next: "Start it again from the thread that asked for it.",
  openLead: "Open that thread",
} as const;

export function FailedStart({ workspace }: { workspace: WorkspaceView }) {
  const select = useStore(s => s.select);
  const lead = workspace.parentWorkspaceId !== undefined && workspace.parentThreadId !== undefined ? { workspaceId: workspace.parentWorkspaceId, threadId: workspace.parentThreadId } : null;
  const rows = creationSteps({ lines: [], askedAt: 0, failed: { title: couldNotStart(workspace.name), detail: workspace.gone ?? "" } });
  const acts =
    lead === null ? undefined : (
      <Button data-k="failed-start-lead" variant="ghost" size="xs" onClick={() => select(lead.workspaceId, lead.threadId)}>
        {FAILED_START_WORDS.openLead}
      </Button>
    );
  return (
    <div data-testid="failed-start" className="flex min-h-0 flex-1 flex-col" data-terminal-beside>
      <div className="relative isolate h-full min-h-0 text-foreground">
        <SetupRoom>
          <div className="mx-auto w-full min-w-0 max-w-3xl">
            <Card id="setting-up" head={couldNotStart(workspace.name)} {...(lead === null ? {} : { lede: FAILED_START_WORDS.next })}>
              {rows.map(row => (
                <StepRow key={row.id} row={row} {...(row.state === "failed" && acts !== undefined ? { acts } : {})} />
              ))}
            </Card>
          </div>
        </SetupRoom>
      </div>
    </div>
  );
}
