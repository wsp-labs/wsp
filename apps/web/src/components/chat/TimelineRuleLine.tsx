// SPDX-License-Identifier: AGPL-3.0-only
// One line the thread says about its workspace rather than about the turn:
// the row's meta, muted, on its own line, which is what parts it from what the
// agent wrote. The waking line a send puts there and the paused line under a
// settled thread are the same shape, so a person reads one grammar for what
// the workspace is doing.
import type { ComponentProps, ReactNode } from "react";
import { cn } from "../../lib/utils";

export function TimelineRuleLine({ line, children, end, wraps = false, className, ...rest }: ComponentProps<"div"> & { line: string; end?: ReactNode; /** A line of two sentences wraps under itself rather than clipping. */ wraps?: boolean }) {
  return (
    <div {...rest} className={cn("flex min-w-0 items-center gap-2 px-1 text-[11px] text-muted-foreground", wraps ? "min-h-6 py-1" : "h-6", className)}>
      <span className={wraps ? "min-w-0" : "shrink-0 whitespace-nowrap"}>{line}</span>
      {children}
      {end === undefined ? null : <span className="ms-auto shrink-0 whitespace-nowrap font-mono tabular-nums">{end}</span>}
    </div>
  );
}
