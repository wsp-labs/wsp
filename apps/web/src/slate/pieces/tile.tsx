// SPDX-License-Identifier: AGPL-3.0-only
// The one tile a figure draws as, wherever it stands: a stat's tile, and a number, a ring or a meter in a strip. Its
// label 13 px muted with a ring beside it where it has a max, filling to its share and easing from where it stood; the
// figure 26 px mono rolling to a new value, its window under it; a trend draws a sparkline beside the figure.
import { useEffect, useState } from "react";
import { DigitRoll } from "../../components/ui/digit-roll.js";
import { cn } from "../../lib/utils.js";
import { STAT } from "../../settings/usage.js";
import { NOTE, TONE_INK, type Tone } from "./look.js";
import { Sparkline } from "./sparkline.js";

/** A ring 20 px across beside the label: a 12 percent track and its share in the tone's ink, filling from empty when it
 * first draws. */
export function Ring({ share, tone }: { share: number; tone: Tone }) {
  const [drawn, setDrawn] = useState(0);
  useEffect(() => {
    const frame = requestAnimationFrame(() => setDrawn(Math.max(0, Math.min(1, share))));
    return () => cancelAnimationFrame(frame);
  }, [share]);
  const r = 13;
  const round = 2 * Math.PI * r;
  return (
    <svg data-slate-ring={Math.round(share * 100)} aria-hidden viewBox="0 0 32 32" className={cn("size-5 shrink-0 -rotate-90", tone === "default" ? "text-foreground/55" : TONE_INK[tone])}>
      <circle cx="16" cy="16" r={r} fill="none" stroke="currentColor" strokeOpacity={0.12} strokeWidth={4} />
      <circle
        cx="16"
        cy="16"
        r={r}
        fill="none"
        stroke="currentColor"
        strokeWidth={4}
        strokeLinecap="round"
        strokeDasharray={round}
        strokeDashoffset={round * (1 - drawn)}
        className="transition-[stroke-dashoffset] duration-700 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none"
      />
    </svg>
  );
}

export interface TileProps {
  label: string | undefined;
  /** The figure as words, undefined while its data has not come. */
  figure: string | undefined;
  unit?: string | undefined;
  window?: string | undefined;
  /** A share of 0 to 1 draws the ring. */
  share?: number | undefined;
  trend?: readonly number[];
  tone: Tone;
  /** Words riding under the figure, a status first: what a number's note carries. */
  foot?: React.ReactNode;
  /** A 20 px figure, where the widest of its row would not fit the narrow panel's tile at 26 px. */
  small?: boolean;
}

export function Tile({ label, figure, unit, window, share, trend = [], tone, foot, small = false }: TileProps) {
  return (
    <div data-slate-tile className={STAT.cell}>
      <span className="flex min-w-0 items-start justify-between gap-2">
        <span className={cn(STAT.label, "min-w-0 leading-5")}>{label}</span>
        {share !== undefined ? <Ring share={share} tone={tone} /> : null}
      </span>
      <span className="flex min-h-8 min-w-0 items-center justify-between gap-2">
        <span className={cn("flex min-w-0 flex-wrap items-baseline gap-x-1 tabular-nums", STAT.figure, small && "text-xl leading-7", tone !== "default" && share === undefined && TONE_INK[tone])}>
          {figure === undefined ? <span data-slate-unread className={cn(NOTE, "self-center font-sans font-normal")}>Not read yet</span> : (
            <>
              <DigitRoll rollIn value={figure} />
              {unit === undefined ? null : <span data-slate-unit className="font-sans text-xs leading-4 font-normal text-muted-foreground">{unit}</span>}
            </>
          )}
        </span>
        {share === undefined && trend.length > 1 ? <Sparkline values={trend} ink={tone === "default" ? "text-foreground/55" : TONE_INK[tone]} /> : null}
      </span>
      {window === undefined && foot === undefined ? null : (
        <span className="flex flex-wrap gap-x-3 text-xs leading-4 text-muted-foreground">
          {foot}
          {window === undefined ? null : <span>{window}</span>}
        </span>
      )}
    </div>
  );
}
