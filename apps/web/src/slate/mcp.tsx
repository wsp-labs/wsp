// SPDX-License-Identifier: AGPL-3.0-only
// The person's word before a slate's tool or resource run goes, asked exactly where the thread's agent would ask
// before that tool: one plain sheet naming the tool and its server, the run's one line for what it does, where and how
// often it runs, its code folded behind Show code, with Not now and Allow. Allow keeps what it sends allowed, so its
// next starts ask nothing. A run that names confirm asks on every start in the red confirm a command's confirm uses.
import { useState } from "react";
import { ChevronRightIcon } from "lucide-react";
import { AlertDialog, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogPopup, AlertDialogTitle } from "../components/ui/alert-dialog.js";
import { Button, NEUTRAL_RING } from "../components/ui/button.js";
import { Collapsible, CollapsiblePanel, CollapsibleTrigger } from "../components/ui/collapsible.js";
import { Dialog, DialogFooter, DialogHeader, DialogPanel, DialogPopup, DialogTitle } from "../components/ui/dialog.js";
import { cn } from "../lib/utils.js";
import { capitalised } from "../settings/format.js";
import type { SlateApproval, SlateAsk } from "./model.js";

export type SlateToolAsk = Extract<SlateAsk, { kind: "tool" }>;

export const MCP_WORDS = {
  run: "Let this panel run",
  read: "Let this panel read",
  rowRun: "Run",
  rowRead: "Read",
  on: (server: string) => `on ${capitalised(server)}`,
  where: (computer: string, cadence: string) => `On ${computer}. ${cadence}.`,
  showCode: "Show code",
  allow: "Allow",
  confirmRun: "Run",
  notNow: "Not now",
  then: "Then its result goes on stdin to",
  file: (name: string) => `It reads ${name}, which holds`,
} as const;

/** A value the host sent as dots: a secret, shown muted. */
export const dots = (value: unknown): boolean => typeof value === "string" && /^•+/.test(value);
const shown = (value: unknown): string => (typeof value === "string" ? value : JSON.stringify(value));

/** Each argument by name with the value it carries, a secret as dots, in mono. */
export function ArgList({ args }: { args: Record<string, unknown> | undefined }) {
  const entries = Object.entries(args ?? {});
  if (entries.length === 0) return null;
  return (
    <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1 font-mono text-xs leading-4 tabular-nums">
      {entries.map(([name, value]) => (
        <div key={name} data-slate-arg={name} className="contents">
          <dt className="text-muted-foreground">{name}</dt>
          <dd className={cn("min-w-0 break-all", dots(value) ? "text-muted-foreground" : "text-foreground")}>{shown(value)}</dd>
        </div>
      ))}
    </dl>
  );
}

/** Each of the slate's files a run reads, by name with its whole text, since an approval covers that text too. */
export function AskFiles({ files }: { files: Record<string, string> | undefined }) {
  const entries = Object.entries(files ?? {});
  if (entries.length === 0) return null;
  return (
    <>
      {entries.map(([name, text]) => (
        <div key={name} data-slate-consent-file={name} className="flex min-w-0 flex-col gap-1">
          <p className="text-muted-foreground">{MCP_WORDS.file(name)}</p>
          <pre className="max-h-[calc(12*1rem+1rem)] overflow-auto whitespace-pre-wrap break-all rounded-md bg-accent px-2.5 py-2 font-mono text-xs leading-4 tabular-nums text-foreground">{text}</pre>
        </div>
      ))}
    </>
  );
}

/** Why the host holds the run: waiting for approval, or what changed since the person allowed it. */
export function AskWhy({ why }: { why: string }) {
  return <p data-slate-consent-why className="text-muted-foreground">{capitalised(why)}.</p>;
}

/** A run's reshape: the literal command its raw result is piped to, as it will run. */
export function ThenCommand({ then }: { then: string | undefined }) {
  if (then === undefined) return null;
  return (
    <div data-slate-consent-then className="flex min-w-0 flex-col gap-1">
      <p className="text-muted-foreground">{MCP_WORDS.then}</p>
      <pre className="max-h-[calc(6*1rem+1rem)] overflow-auto whitespace-pre-wrap break-all rounded-md bg-accent px-2.5 py-2 font-mono text-xs leading-4 tabular-nums text-foreground">{then}</pre>
    </div>
  );
}

/** Calls answer and closes, or keeps the sheet open with the host's refusal under it. */
export function useAnswer<T>(answer: (scope: T) => Promise<unknown>, onClose: () => void) {
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | undefined>(undefined);
  const decide = (scope: T) => {
    setBusy(true);
    setRefused(undefined);
    void answer(scope).then(
      () => onClose(),
      (error: unknown) => {
        setBusy(false);
        setRefused(error instanceof Error ? error.message : String(error));
      },
    );
  };
  return { busy, refused, decide };
}

/** The tool and its server as the sheet's title says them: "Let this panel run execute on Cloudflare?". */
function ToolTitle({ ask, lead }: { ask: SlateToolAsk; lead: string }) {
  return (
    <>
      {lead} <code data-slate-consent-tool className="font-mono">{ask.tool}</code> {MCP_WORDS.on(ask.server)}
    </>
  );
}

/** What the run does, where and how often it runs, and its code folded: the arguments as sent, its reshape and the
 * files that reads, since allowing it allows all of that. */
function ToolBody({ ask, cadence }: { ask: SlateToolAsk; cadence: string }) {
  return (
    <>
      {ask.description === undefined ? null : (
        <p data-slate-consent-description className="text-foreground">
          {ask.description}
        </p>
      )}
      <p data-slate-consent-where className="text-muted-foreground">
        {MCP_WORDS.where(ask.computer, cadence)}
      </p>
      <Collapsible>
        <CollapsibleTrigger data-slate-consent-code-trigger className="group flex items-center gap-1 text-left text-xs leading-4 text-muted-foreground hover:text-foreground">
          <ChevronRightIcon aria-hidden className="size-3 shrink-0 transition-transform duration-150 group-data-panel-open:rotate-90" />
          {MCP_WORDS.showCode}
        </CollapsibleTrigger>
        <CollapsiblePanel>
          <div data-slate-consent-code className="flex min-w-0 flex-col gap-2 pt-2">
            <code className="min-w-0 break-all font-mono text-xs leading-4 text-foreground">{ask.resource === true ? `${ask.server}:${ask.tool}` : `${ask.server}.${ask.tool}`}</code>
            <ArgList args={ask.args} />
            <ThenCommand then={ask.then} />
            <AskFiles files={ask.files} />
          </div>
        </CollapsiblePanel>
      </Collapsible>
    </>
  );
}

export function ToolSheet({ ask, cadence, answer, onClose }: { ask: SlateToolAsk; cadence: string; answer(scope: SlateApproval): Promise<unknown>; onClose(): void }) {
  const { busy, refused, decide } = useAnswer(answer, onClose);
  const title = (
    <>
      <ToolTitle ask={ask} lead={ask.resource === true ? MCP_WORDS.read : MCP_WORDS.run} />?
    </>
  );
  const body = (
    <div data-slate-consent={ask.run} data-slate-consent-server={ask.server} className="flex min-w-0 flex-col gap-3 text-[13px] leading-5">
      <ToolBody ask={ask} cadence={cadence} />
      {refused === undefined ? null : <p className="text-error-foreground">{refused}</p>}
    </div>
  );
  if (ask.confirm !== undefined) {
    return (
      <AlertDialog open onOpenChange={open => (open ? undefined : onClose())}>
        <AlertDialogPopup>
          <AlertDialogHeader>
            <AlertDialogTitle>{title}</AlertDialogTitle>
            <AlertDialogDescription>{ask.confirm}</AlertDialogDescription>
          </AlertDialogHeader>
          <div className="px-5 pt-2">{body}</div>
          <AlertDialogFooter>
            <Button variant="outline" className={NEUTRAL_RING} disabled={busy} onClick={() => decide("refuse")}>
              {MCP_WORDS.notNow}
            </Button>
            <Button variant="destructive" disabled={busy} onClick={() => decide("once")}>
              {MCP_WORDS.confirmRun}
            </Button>
          </AlertDialogFooter>
        </AlertDialogPopup>
      </AlertDialog>
    );
  }
  return (
    <Dialog open onOpenChange={open => (open ? undefined : onClose())}>
      <DialogPopup>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <DialogPanel className="pt-1 pb-0">{body}</DialogPanel>
        <DialogFooter>
          <Button variant="outline" disabled={busy} onClick={() => decide("refuse")}>
            {MCP_WORDS.notNow}
          </Button>
          <Button disabled={busy} onClick={() => decide("thread")}>
            {MCP_WORDS.allow}
          </Button>
        </DialogFooter>
      </DialogPopup>
    </Dialog>
  );
}

/** A tool run waiting for the person as a row of the several-at-once sheet: what its own sheet would show. */
export function ToolRow({ ask, cadence }: { ask: SlateToolAsk; cadence: string }) {
  return (
    <>
      <p className="text-foreground">
        <ToolTitle ask={ask} lead={ask.resource === true ? MCP_WORDS.rowRead : MCP_WORDS.rowRun} />
      </p>
      <ToolBody ask={ask} cadence={cadence} />
    </>
  );
}
