// SPDX-License-Identifier: AGPL-3.0-only
// The one tile a figure draws as, wherever it stands: a stat's tile, and a number, a ring or a meter in a strip. Its
// label 13 px muted on one line; the figure 26 px mono rolling to a new value, with a ring beside it where it has a max,
// filling to its share and easing from where it stood; its window under it on one line; a trend draws a sparkline at
// the figure's right. A cut label or window says itself whole on hover.
import { useEffect, useState } from "react";
import { DigitRoll } from "../../components/ui/digit-roll.js";
import { cn } from "../../lib/utils.js";
import { STAT } from "../../settings/usage.js";
import { NOTE, TONE_INK, type Tone } from "./look.js";
import { Sparkline } from "./sparkline.js";

/** A ring 20 px across beside the figure: a 12 percent track and its share in the tone's ink, filling from empty when it
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

/** Per rem of its row under which a tile draws as one line, or always: whole class names, so Tailwind finds them, for its
 * box, label, figure and window. */
const LINE: Record<number | "always", readonly [string, string, string, string]> = {
  30: ["@max-[30rem]:grid-cols-[minmax(0,1fr)_auto] @max-[30rem]:content-center @max-[30rem]:gap-x-4 @max-[30rem]:gap-y-0.5 @max-[30rem]:px-(--settings-inset,20px)!", "@max-[30rem]:col-start-1 @max-[30rem]:row-start-1", "@max-[30rem]:col-start-2 @max-[30rem]:row-span-2 @max-[30rem]:row-start-1", "@max-[30rem]:col-start-1 @max-[30rem]:row-start-2"],
  34: ["@max-[34rem]:grid-cols-[minmax(0,1fr)_auto] @max-[34rem]:content-center @max-[34rem]:gap-x-4 @max-[34rem]:gap-y-0.5 @max-[34rem]:px-(--settings-inset,20px)!", "@max-[34rem]:col-start-1 @max-[34rem]:row-start-1", "@max-[34rem]:col-start-2 @max-[34rem]:row-span-2 @max-[34rem]:row-start-1", "@max-[34rem]:col-start-1 @max-[34rem]:row-start-2"],
  38: ["@max-[38rem]:grid-cols-[minmax(0,1fr)_auto] @max-[38rem]:content-center @max-[38rem]:gap-x-4 @max-[38rem]:gap-y-0.5 @max-[38rem]:px-(--settings-inset,20px)!", "@max-[38rem]:col-start-1 @max-[38rem]:row-start-1", "@max-[38rem]:col-start-2 @max-[38rem]:row-span-2 @max-[38rem]:row-start-1", "@max-[38rem]:col-start-1 @max-[38rem]:row-start-2"],
  42: ["@max-[42rem]:grid-cols-[minmax(0,1fr)_auto] @max-[42rem]:content-center @max-[42rem]:gap-x-4 @max-[42rem]:gap-y-0.5 @max-[42rem]:px-(--settings-inset,20px)!", "@max-[42rem]:col-start-1 @max-[42rem]:row-start-1", "@max-[42rem]:col-start-2 @max-[42rem]:row-span-2 @max-[42rem]:row-start-1", "@max-[42rem]:col-start-1 @max-[42rem]:row-start-2"],
  46: ["@max-[46rem]:grid-cols-[minmax(0,1fr)_auto] @max-[46rem]:content-center @max-[46rem]:gap-x-4 @max-[46rem]:gap-y-0.5 @max-[46rem]:px-(--settings-inset,20px)!", "@max-[46rem]:col-start-1 @max-[46rem]:row-start-1", "@max-[46rem]:col-start-2 @max-[46rem]:row-span-2 @max-[46rem]:row-start-1", "@max-[46rem]:col-start-1 @max-[46rem]:row-start-2"],
  always: ["grid-cols-[minmax(0,1fr)_auto] content-center gap-x-4 gap-y-0.5 px-(--settings-inset,20px)!", "col-start-1 row-start-1", "col-start-2 row-span-2 row-start-1", "col-start-1 row-start-2"],
};

/** The figure's px: 26, or smaller where the widest of its row would not fit the narrow panel's tile. */
export type FigureSize = 26 | 20 | 16;
const SIZE: Record<FigureSize, string> = { 26: "", 20: "text-xl leading-7", 16: "text-base leading-6" };

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
  size?: FigureSize;
  /** Under this many rem of its row, or always, one line: the label over its window at the left, the figure and its
   * ring at the right. */
  line?: number | "always";
}

export function Tile({ label, figure, unit, window, share, trend = [], tone, foot, size = 26, line }: TileProps) {
  const [box, labelAt, figureAt, windowAt] = line === undefined ? [] : (LINE[line] ?? []);
  return (
    <div data-slate-tile className={cn("grid min-w-0 content-start gap-y-1.5", box)}>
      <span data-slate-label className={cn(STAT.label, "min-h-5 min-w-0 truncate leading-5", labelAt)} title={label}>{label}</span>
      <span className={cn("flex min-h-8 min-w-0 items-center justify-between gap-2", figureAt)}>
        <span className="flex shrink-0 items-center gap-2.5">
          <span data-slate-figure className={cn("flex min-w-0 flex-wrap items-baseline gap-x-1 tabular-nums", STAT.figure, SIZE[size], tone !== "default" && share === undefined && TONE_INK[tone])}>
            {figure === undefined ? <span data-slate-unread className={cn(NOTE, "self-center font-sans font-normal")}>Not read yet</span> : (
              <>
                <DigitRoll rollIn value={figure} />
                {unit === undefined ? null : <span data-slate-unit className="font-sans text-xs leading-4 font-normal text-muted-foreground">{unit}</span>}
              </>
            )}
          </span>
          {share !== undefined ? <Ring share={share} tone={tone} /> : null}
        </span>
        {share === undefined && trend.length > 1 ? <Sparkline values={trend} ink={tone === "default" ? "text-foreground/55" : TONE_INK[tone]} /> : null}
      </span>
      {window === undefined && foot === undefined ? null : (
        <span className={cn("flex min-w-0 gap-x-3 text-xs leading-4 text-muted-foreground", windowAt)}>
          {foot}
          {window === undefined ? null : <span data-slate-window className="min-w-0 truncate" title={window}>{window}</span>}
        </span>
      )}
    </div>
  );
}
