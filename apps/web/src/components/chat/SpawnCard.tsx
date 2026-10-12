// SPDX-License-Identifier: AGPL-3.0-only
// A sub-thread's machine being made, in the thread's middle where the
// conversation will stand, as the setup card is: the thread's title with the
// machine's size and rate, then one step row per stage with what it read and
// how long it took, the lines it passed with as warnings, and the raw log
// under Show details. A stage that fails keeps its row with the reason and
// Try again. Once the agent starts the conversation takes its place.
import { useState } from "react";
import { ChevronRightIcon } from "lucide-react";
import { SUB_SPAWN_WORDS } from "@wsp/protocol";
import { cn } from "../../lib/utils.js";
import { StepRow } from "../../settings/add/StepRow.js";
import { FACT } from "../../settings/format.js";
import { CARD_INSET, LINE_FLOOR } from "../../settings/layout.js";
import { Card } from "../../settings/rows.js";
import { Button } from "../ui/button.js";
import type { SpawnLog } from "./spawnLog.js";

export function SpawnCard({ title, spawn, onTryAgain }: { title: string; spawn: SpawnLog; onTryAgain: () => void }) {
  const [open, setOpen] = useState(false);
  const head = (
    <>
      <span className="min-w-0 truncate">{title}</span>
      <span data-spawn-size className={cn(FACT, "ms-auto shrink-0 ps-4 font-normal")}>
        {spawn.size}
      </span>
    </>
  );
  return (
    <div className="mx-auto w-full min-w-0 max-w-3xl" data-spawn-card>
      <Card id="spawning" head={head}>
        {spawn.steps.map(row => (
          <StepRow
            key={row.id}
            row={row}
            {...(row.state === "failed"
              ? {
                  acts: (
                    <Button size="xs" variant="outline" data-k="spawn-try-again" onClick={onTryAgain}>
                      {SUB_SPAWN_WORDS.tryAgain}
                    </Button>
                  ),
                }
              : {})}
          />
        ))}
        <div className="flex flex-col">
          <button type="button" data-k="spawn-details" aria-expanded={open} onClick={() => setOpen(o => !o)} className={cn("flex items-center gap-2 py-3 text-left text-muted-foreground transition-colors duration-150 hover:text-foreground", CARD_INSET, LINE_FLOOR)}>
            <ChevronRightIcon aria-hidden className={cn("size-3.5 shrink-0 transition-transform duration-150", open && "rotate-90")} />
            <span className="text-xs">{open ? SUB_SPAWN_WORDS.hideDetails : SUB_SPAWN_WORDS.details}</span>
          </button>
          {open ? (
            <pre data-spawn-log className={`overflow-x-auto pb-4 font-mono text-meta leading-4 whitespace-pre-wrap text-muted-foreground ${CARD_INSET}`}>
              {spawn.log.join("\n")}
            </pre>
          ) : null}
        </div>
      </Card>
    </div>
  );
}
