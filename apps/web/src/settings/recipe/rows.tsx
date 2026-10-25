// SPDX-License-Identifier: AGPL-3.0-only
// A size cell: tabular mono in the tone the protocol gives it.
import type { ReactNode } from "react";
import type { SizeTone } from "@wsp/protocol";
import { TONE_TEXT } from "../../lib/tone.js";
import { cn } from "../../lib/utils.js";

export const FIELD = "h-8 w-full rounded-md border border-input bg-background font-mono text-xs [&_input]:h-8 [&_input]:leading-8";

export function SizeCell({ tone, children, className }: { tone: SizeTone; children: ReactNode; className?: string }) {
  return (
    <span data-k="size" data-tone={tone} className={cn("font-mono text-xs tabular-nums", TONE_TEXT[tone], className)}>
      {children}
    </span>
  );
}
