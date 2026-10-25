// SPDX-License-Identifier: AGPL-3.0-only
// A run's output summary first (#2468): a row of the card with its label and its state as one word, a quiet Cancel in
// the slot while it runs, then what passed and failed as the runner tallied it and the lines that name a failure, or
// the last line where no tally reads; the raw lines fold under "All N lines", `lines` rows tall once open, following
// the end while it runs. Why under them when held or failed.
// A finished run keeps its last lines until it next starts; after the agent changed its command they read as stale.
import { ChevronRight } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { slateRunSummary, slateSummaryCounts, type SlateJson } from "@wsp/protocol/slate";
import { Crab } from "../../components/status/Crab.js";
import { Button } from "../../components/ui/button.js";
import { cn } from "../../lib/utils.js";
import { isRunRecord, type SlateRunState } from "../model.js";
import { ResultByShape } from "../shape.js";
import type { PieceView } from "../SlateView.js";
import { CARD_SURFACE } from "../../settings/rows.js";
import { str } from "./look.js";
import { placeOf } from "./runs.js";
import { Quiet } from "./quiet.js";
import { Refreshing } from "./refreshing.js";
import { isOwnName } from "../paths.js";

/** A run state as the status mark's grammar says it. */
export const RUN_WORDS: Record<SlateRunState, string> = {
  idle: "Not run yet",
  held: "Held",
  running: "Running",
  done: "Done",
  failed: "Failed",
  cancelled: "Cancelled",
};

const RUN_INK: Record<SlateRunState, string> = {
  idle: "text-muted-foreground",
  held: "text-status-input",
  running: "text-status-working",
  done: "text-muted-foreground",
  failed: "text-status-failed",
  cancelled: "text-muted-foreground",
};

const textOf = (value: SlateJson | undefined): string => (value === undefined || value === null ? "" : typeof value === "string" ? value : JSON.stringify(value, null, 2));

/** The record's own lines, else the tail of out and err split at line ends. */
function recordLines(record: { lines?: string[]; out?: SlateJson; err?: string }): string[] {
  if (Array.isArray(record.lines)) return record.lines;
  const text = [textOf(record.out), record.err ?? ""].filter(part => part !== "").join("\n");
  return text === "" ? [] : text.replace(/\n$/, "").split("\n");
}

const runName = (value: unknown): string | undefined => (isOwnName(value) ? value.slice(1) : undefined);

const COUNT_INK = { failed: "text-error-foreground font-medium", passed: "text-foreground", skipped: "text-muted-foreground" } as const;

export const output: PieceView = {
  type: "output",
  saysRefreshing: true,
  rowScoped: ["run"],
  fills: true,
  component: function OutputPiece({ id, piece, props, slate, raise, cancel, sender }) {
    const run = runName(piece.props?.["run"]);
    const record = run === undefined ? undefined : slate.values[run];
    const box = useRef<HTMLDivElement>(null);
    const [unfolded, setUnfolded] = useState(false);
    const rows = typeof props["lines"] === "number" ? Math.max(3, Math.min(40, Math.floor(props["lines"]))) : 8;
    const state: SlateRunState = isRunRecord(record) && record.state in RUN_WORDS ? record.state : "idle";
    const lines = run === undefined ? [] : (slate.lines(run) ?? (isRunRecord(record) ? recordLines(record) : []));
    // A refresh draws the last result with a quiet mark, never the running row's crab and Cancel.
    const refreshing = run !== undefined && slate.refreshing(run);
    const running = state === "running" && !refreshing;
    const stale = isRunRecord(record) && record.stale === true && !running;
    useEffect(() => {
      if (state === "running" && box.current !== null) box.current.scrollTop = box.current.scrollHeight;
    }, [state, lines.length, unfolded]);
    if (run === undefined) return <Quiet>{RUN_WORDS.idle}</Quiet>;
    const why = isRunRecord(record) && (state === "held" || state === "failed") ? record.why : undefined;
    // A tool or resource run's result draws by its shape; its failure and a command's output stay lines.
    const kind = slate.document?.runs?.[run]?.kind;
    const result = kind !== undefined && kind !== "cmd" && isRunRecord(record) && state !== "failed" ? (record.json ?? record.out) : undefined;
    const label = str(props["label"]);
    const place = placeOf(slate, id);
    const inset = place === "inside" ? "" : "px-(--settings-inset,20px)";
    return (
      <div data-slate-output={run} className={cn("flex min-w-0 flex-col [&>*+*]:border-t [&>*+*]:border-border/50", place === "page" && CARD_SURFACE)}>
        <div className={cn("flex min-h-11 min-w-0 items-center gap-3 py-3", inset)}>
          {label === undefined ? null : <span className="min-w-0 break-words text-sm leading-5 text-foreground">{label}</span>}
          <span className="flex-1" />
          <span data-slate-run-state={state} className={cn("inline-flex shrink-0 items-center gap-1.5 text-[13px] leading-5 whitespace-nowrap", refreshing ? "text-muted-foreground" : RUN_INK[state], state === "failed" || state === "held" ? "font-medium" : "")}>
            {running ? <Crab className="text-status-working" /> : null}
            {refreshing ? <Refreshing /> : RUN_WORDS[state]}
          </span>
          {stale ? (
            <span data-slate-stale className="min-w-0 break-words text-[13px] text-muted-foreground">
              from before the command changed
            </span>
          ) : null}
          {running ? (
            <Button data-slate-cancel={run} variant="outline" size="xs" onClick={() => void cancel(run)}>
              Cancel
            </Button>
          ) : null}
        </div>
        {result !== undefined ? (
          <div data-slate-result={run} className={cn("min-w-0 py-3", inset)}>
            <ResultByShape value={result} shared={{ id, slate, raise, cancel, sender }} />
          </div>
        ) : lines.length === 0 ? null : (
          <Summary lines={lines} stale={stale} inset={inset} />
        )}
        {result !== undefined || lines.length === 0 ? null : (
          <div className={cn("flex min-w-0 flex-col py-1.5", inset)}>
            <button type="button" data-slate-fold={run} aria-expanded={unfolded} onClick={() => setUnfolded(open => !open)} className="-mx-1 inline-flex h-7 items-center gap-1 self-start rounded-md px-1 text-[13px] leading-5 text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring">
              <ChevronRight aria-hidden className={cn("size-3.5 shrink-0 transition-transform duration-150 motion-reduce:transition-none", unfolded && "rotate-90")} />
              {`All ${lines.length.toLocaleString("en-US")} line${lines.length === 1 ? "" : "s"}`}
            </button>
            {unfolded ? (
              <div
                ref={box}
                role="log"
                aria-live={running ? "polite" : "off"}
                className={cn("overflow-y-auto pt-1 pb-1.5 font-mono text-xs leading-4 tabular-nums", stale ? "text-muted-foreground" : "text-foreground", props["wrap"] === true ? "whitespace-pre-wrap break-words" : "whitespace-pre")}
                style={{ maxHeight: `calc(${rows} * 1rem + 0.625rem)` }}
              >
                {lines.map((line, at) => (
                  <div key={at}>{line === "" ? " " : line}</div>
                ))}
              </div>
            ) : null}
          </div>
        )}
        {why === undefined ? null : <Quiet data-slate-why className={cn("py-2.5", inset)}>{why}</Quiet>}
      </div>
    );
  },
};

/** What the lines come to, ahead of them: the runner's counts, failed first, then the lines that name a failure; the
 * last line alone where neither reads. */
function Summary({ lines, stale, inset }: { lines: readonly string[]; stale: boolean; inset: string }) {
  const summary = slateRunSummary(lines);
  const counts = slateSummaryCounts(summary);
  const lead = counts.length > 0 || summary.matters.length > 0 ? summary.matters : summary.last !== undefined ? [summary.last] : [];
  return (
    <div data-slate-summary className={cn("flex min-w-0 flex-col gap-1.5 py-3", inset, stale && "text-muted-foreground")}>
      {counts.length === 0 ? null : (
        <div className="flex min-w-0 flex-wrap gap-x-4 gap-y-1 text-sm leading-5 tabular-nums">
          {counts.map(c => (
            <span key={c.count} data-slate-count={c.count} className={stale ? undefined : COUNT_INK[c.count]}>
              {c.words}
            </span>
          ))}
        </div>
      )}
      {lead.length === 0 ? null : (
        <div data-slate-matters className={cn("flex min-w-0 flex-col gap-0.5 font-mono text-xs leading-4 break-words whitespace-pre-wrap", counts.length > 0 || summary.matters.length > 0 || stale ? "text-muted-foreground" : "text-foreground")}>
          {lead.map((line, at) => (
            <div key={at}>{line}</div>
          ))}
        </div>
      )}
    </div>
  );
}
