// SPDX-License-Identifier: AGPL-3.0-only
// The line under a sub-thread's brief while its machine is made: one centered
// quiet line in the meta size, the spinner, the words and the time, and a
// chevron that opens the steps beneath, one compact line each with its mark,
// its words, its facts and its time, in the transcript's own quiet ink; the raw
// log opens one level further down. Once the agent answers it stands as the
// done line between the brief and the reply. A spawn that failed says so in
// the refusal ink, open on the failed step with Try again.
import { useEffect, useState } from "react";
import { ChevronRightIcon, CircleAlertIcon, CircleCheckIcon, CircleIcon, TriangleAlertIcon } from "lucide-react";
import { SUB_SPAWN_WORDS, fmtDuration, fmtElapsed } from "@wsp/protocol";
import { cn } from "../../lib/utils.js";
import { useStore } from "../../protocol/store.js";
import type { StepLine } from "../../settings/add/setup.js";
import { Button } from "../ui/button.js";
import { Spinner } from "../ui/spinner.js";
import { useSpawnLog } from "./spawnLog.js";

/** Now, once a second while `on`; the live line's time and its running step's. */
function useSeconds(on: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!on) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [on]);
  return now;
}

const MARK = "size-3 shrink-0";

function Mark({ state }: { state: StepLine["state"] }) {
  if (state === "working") return <Spinner aria-label="Under way" role="img" className={MARK} />;
  if (state === "done") return <CircleCheckIcon role="img" aria-label="Done" className={MARK} />;
  if (state === "warned") return <TriangleAlertIcon role="img" aria-label="Passed with a warning" className={cn(MARK, "text-warning")} />;
  if (state === "failed") return <CircleAlertIcon role="img" aria-label="Failed" className={cn(MARK, "text-destructive-foreground")} />;
  return <CircleIcon role="img" aria-label="Not started" className={cn(MARK, "opacity-50")} />;
}

export function SpawnLine({ workspaceId }: { workspaceId: string }) {
  const spawn = useSpawnLog(workspaceId);
  const rows = useStore(s => s.sessions[workspaceId]);
  const failed = spawn?.steps.some(step => step.state === "failed") === true;
  const live = spawn !== undefined && spawn.tookMs === undefined && !failed;
  const [open, setOpen] = useState(failed);
  const [raw, setRaw] = useState(false);
  const now = useSeconds(live);
  if (spawn === undefined) return null;
  const words = failed ? SUB_SPAWN_WORDS.failed(spawn.on) : live ? SUB_SPAWN_WORDS.starting(spawn.on) : SUB_SPAWN_WORDS.started(spawn.on, fmtDuration(spawn.tookMs ?? 0));
  const tryAgain = (): void => {
    const row = rows?.at(-1);
    if (row !== undefined) void useStore.getState().api?.startSession({ workspaceId, prompt: row.prompt ?? "", ...(row.threadId === undefined ? {} : { thread: row.threadId }) });
  };
  return (
    <div data-spawn-line={failed ? "failed" : live ? "live" : "done"} className="flex flex-col items-center text-meta leading-4 text-muted-foreground">
      <button type="button" aria-expanded={open} data-k="spawn-line" onClick={() => setOpen(o => !o)} className={cn("inline-flex items-center gap-1.5 tabular-nums transition-colors duration-150 hover:text-foreground", failed && "text-destructive-foreground hover:text-destructive-foreground")}>
        {live ? <Spinner aria-hidden className={MARK} /> : failed ? <CircleAlertIcon aria-hidden className={MARK} /> : null}
        <span>{words}</span>
        {live ? <span className="font-mono">{fmtElapsed(now - spawn.since)}</span> : null}
        <ChevronRightIcon aria-hidden className={cn(MARK, "transition-transform duration-150", open && "rotate-90")} />
      </button>
      {!open ? null : (
        <div data-spawn-steps className="mt-2 flex w-full max-w-xl flex-col gap-1 ps-6">
          {spawn.steps.map(step => (
            <div key={step.id} data-spawn-step={step.id} data-state={step.state} className="flex flex-col gap-1">
              <div className="flex min-w-0 items-baseline gap-2">
                <span className="flex translate-y-0.5 self-start">
                  <Mark state={step.state} />
                </span>
                <span className={cn(step.note === undefined ? "min-w-0 break-words" : "shrink-0", step.state === "failed" && "text-destructive-foreground")}>{step.name}</span>
                {step.note === undefined ? null : <span className="min-w-0 truncate opacity-70" title={step.note}>{step.note}</span>}
                <span className="ms-auto shrink-0 font-mono tabular-nums">{step.since !== undefined ? fmtElapsed(now - step.since) : step.ms === undefined ? "" : fmtDuration(step.ms)}</span>
              </div>
              {step.said === undefined ? null : (
                <div className="flex flex-col items-start gap-2 ps-5">
                  <p className="text-destructive-foreground">
                    {step.said}
                    {step.fix === undefined ? null : <span className="text-foreground"> {step.fix}</span>}
                  </p>
                  <Button size="xs" variant="outline" data-k="spawn-try-again" onClick={tryAgain}>
                    {SUB_SPAWN_WORDS.tryAgain}
                  </Button>
                </div>
              )}
            </div>
          ))}
          <button type="button" aria-expanded={raw} data-k="spawn-details" onClick={() => setRaw(r => !r)} className="mt-1 inline-flex items-center gap-1.5 self-start transition-colors duration-150 hover:text-foreground">
            <ChevronRightIcon aria-hidden className={cn(MARK, "transition-transform duration-150", raw && "rotate-90")} />
            {raw ? SUB_SPAWN_WORDS.hideDetails : SUB_SPAWN_WORDS.details}
          </button>
          {raw ? <pre data-spawn-log className="overflow-x-auto ps-5 font-mono text-meta leading-4 whitespace-pre-wrap">{spawn.log.join("\n")}</pre> : null}
        </div>
      )}
    </div>
  );
}
