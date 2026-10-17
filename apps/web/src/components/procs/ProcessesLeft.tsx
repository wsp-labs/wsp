// SPDX-License-Identifier: AGPL-3.0-only
// The processes a settle or a delete would leave running, in Activity's row
// grammar: each one's kind glyph, its name as Activity reads it off the
// command line, its kind and port, its memory, nested as its thread's tree
// holds them, under the thread's title where more than one thread runs any.
// Only what the host listed for the threads it takes, never a process of
// another thread in the same folder; on a box, what stands in the thread's
// cgroup there, which Stop ends whole.
import { useState, type CSSProperties } from "react";
import { ACTIVITY_KIND_WORDS, fmtBytes, procName, UNTAGGED_KINDS, type ActivityProc, type LeftThread, type ProcessesChoice } from "@wsp/protocol";
import { errorText, cn } from "../../lib/utils.js";
import { ACTIVITY_WORDS as W, VALUE } from "../../settings/format.js";
import { CARD_INSET } from "../../settings/layout.js";
import { CARD_SURFACE } from "../../settings/rows.js";
import { RefusalSlot } from "../../settings/sheetParts.js";
import { KIND_GLYPHS, TINT_INK } from "../../settings/activity/tables.js";
import { memoryTint, portsWord } from "../../settings/activity/model.js";
import { AlertDialog, AlertDialogClose, AlertDialogFooter, AlertDialogHeader, AlertDialogPopup, AlertDialogTitle } from "../ui/alert-dialog.js";
import { Button, NEUTRAL_RING } from "../ui/button.js";
import { useSettleAsk, type SettleAsk } from "./leftAsk.js";

function LeftRow({ p, depth }: { p: ActivityProc; depth: number }) {
  const Glyph = KIND_GLYPHS[p.kind];
  const tint = memoryTint(p.memory);
  return (
    <div
      data-left-proc={p.pid}
      data-kind={p.kind}
      title={p.command}
      className="flex items-center gap-3 py-2.5 pe-(--settings-inset,20px) ps-[calc(var(--settings-inset,20px)+var(--depth)*1.25rem)]"
      style={{ "--depth": depth } as CSSProperties}
    >
      <Glyph aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      <span className="flex min-w-0 flex-1 items-baseline gap-2">
        <span className="min-w-0 truncate text-sm leading-5 text-foreground/80">{procName({ comm: p.name, cmdline: p.command })}</span>
        {UNTAGGED_KINDS.has(p.kind) ? null : <span className="min-w-0 shrink-[3] truncate text-xs text-muted-foreground">{ACTIVITY_KIND_WORDS[p.kind]}</span>}
        {p.ports.length === 0 ? null : <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{portsWord(p.ports.map(port => ({ port, loopback: null })))}</span>}
      </span>
      <span data-col="mem" className={cn(VALUE, "shrink-0", tint === null ? undefined : TINT_INK[tint])}>
        {fmtBytes(p.memory)}
      </span>
    </div>
  );
}

export function ProcessesLeftList({ left }: { left: readonly LeftThread[] }) {
  const many = left.length > 1;
  return (
    <div data-k="processes-left" className={cn(CARD_SURFACE, "flex max-h-64 flex-col overflow-y-auto [&>*+*]:border-t [&>*+*]:border-border/50")}>
      {left.flatMap(t => {
        const top = Math.min(...t.processes.map(p => p.depth));
        return [
          ...(many
            ? [
                <div key={t.threadId} data-left-thread={t.threadId} className={cn(CARD_INSET, "truncate py-2.5 text-sm leading-5 font-medium text-foreground")}>
                  {t.title}
                </div>,
              ]
            : []),
          ...t.processes.map(p => <LeftRow key={p.pid} p={p} depth={p.depth - top + (many ? 1 : 0)} />),
        ];
      })}
    </div>
  );
}

/** How many processes the threads still run. */
export const leftCount = (left: readonly LeftThread[]): number => left.reduce((n, t) => n + t.processes.length, 0);

/** What Stop ends for these threads, a box's named where one runs there. */
export const leftStops = (left: readonly LeftThread[]): string =>
  W.leftStops([...new Set(left.flatMap(t => (t.on === undefined ? [] : [t.on])))], left.some(t => t.on === undefined));

export function SettleProcessesConfirm() {
  const ask = useSettleAsk(s => s.ask);
  return ask === null ? null : <SettleConfirm key={ask.left.map(t => t.threadId).join(" ")} ask={ask} />;
}

function SettleConfirm({ ask }: { ask: SettleAsk }) {
  const [busy, setBusy] = useState<ProcessesChoice | null>(null);
  const [refusal, setRefusal] = useState<string | null>(null);
  const go = async (choice: ProcessesChoice): Promise<void> => {
    setBusy(choice);
    setRefusal(null);
    try {
      ask.done(await ask.settle(choice));
    } catch (e) {
      setRefusal(errorText(e));
      setBusy(null);
    }
  };
  return (
    <AlertDialog open onOpenChange={open => (open || busy !== null ? undefined : ask.done(undefined))}>
      <AlertDialogPopup data-k="settle-processes">
        <AlertDialogHeader>
          <AlertDialogTitle>{W.leftTitle(ask.left.length, ask.left[0]?.title ?? "")}</AlertDialogTitle>
        </AlertDialogHeader>
        <div className="px-5 pt-2">
          <ProcessesLeftList left={ask.left} />
        </div>
        <div className="px-5 pt-2">
          <RefusalSlot k="settle-processes-refusal" {...(refusal !== null ? { said: refusal } : { note: W.leftSettle(leftStops(ask.left)) })} />
        </div>
        <AlertDialogFooter>
          <AlertDialogClose render={<Button variant="outline" className={NEUTRAL_RING} />} disabled={busy !== null}>
            {W.cancel}
          </AlertDialogClose>
          <Button variant="outline" className={NEUTRAL_RING} disabled={busy !== null} data-k="settle-keep" onClick={() => void go("keep")}>
            {W.settleKeep}
          </Button>
          <Button variant="destructive" disabled={busy !== null} data-k="settle-stop" onClick={() => void go("stop")}>
            {busy === "stop" ? W.stopping : W.settleStop}
          </Button>
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}
