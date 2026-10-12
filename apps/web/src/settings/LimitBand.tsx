// SPDX-License-Identifier: AGPL-3.0-only
// The band under a cloud's row on the Computers list, in the place and the
// layout of a computer's CPU and memory band: one lane a limit, a meter
// against it where it has a ceiling and its figure at the right, in the
// Usage chart's inks and the warning ink from nine tenths of the way; on a
// narrow window each lane's figure stands over its meter. Read off
// the row the places list carries (the plan, the starts, the cap, the credit
// and the day's spend), never asked for again.
import type { PlaceView } from "@wsp/protocol";
import { fmtCost } from "@wsp/protocol";
import { cn } from "../lib/utils.js";
import { SPLIT_INKS } from "./usage.js";

/** The share from which a lane takes the warning ink, as memory near full does. */
const NEAR_LIMIT = 0.9;

const PLOT = "col-span-3 max-md:col-span-full flex h-4.5 items-center pl-11";
const FIGURE = "col-span-3 max-md:col-span-full max-md:order-first max-md:pl-11 flex h-4.5 items-center gap-1.5 whitespace-nowrap font-mono text-xs leading-4 tabular-nums";
/** The word takes a fixed slot, so every lane's figure starts on one line. */
const WORD = "w-16 shrink-0 font-sans text-muted-foreground";

export const LIMIT_BAND_WORDS = {
  starts: "Starts",
  startsTitle: "Starts today against the plan's day",
  machines: "Machines",
  machinesTitle: "Machines running against machines at once",
  credit: "Credit",
  creditTitle: "Machine hours the account's credit buys",
  spend: "Spend",
  spendTitle: "Spent today against the spend per day",
  of: (used: string, limit: string): string => `${used} of ${limit}`,
  plan: (n: number): string => `plan ${n}`,
  hoursLeft: (h: number): string => `${h < 10 ? h.toFixed(1) : Math.round(h)} h left`,
} as const;

interface Lane {
  readonly k: string;
  readonly word: string;
  readonly title: string;
  readonly figure: string;
  /** How far toward its limit, 0 to 1; absent on a lane with no ceiling, which draws its figure alone. */
  readonly share?: number;
  readonly ink: string;
  /** A quiet fact after the figure: the plan's own ceiling beside the person's. */
  readonly beside?: string;
}

function lanesOf(place: PlaceView): Lane[] {
  const lanes: Lane[] = [];
  const day = place.starts?.day;
  if (day !== undefined) lanes.push({ k: "starts", word: LIMIT_BAND_WORDS.starts, title: LIMIT_BAND_WORDS.startsTitle, figure: LIMIT_BAND_WORDS.of(String(day.used), String(day.limit)), share: day.used / Math.max(1, day.limit), ink: SPLIT_INKS[0]! });
  const cap = place.cap !== undefined && "machines" in place.cap ? place.cap : undefined;
  if (cap !== undefined && place.running !== undefined)
    lanes.push({ k: "machines", word: LIMIT_BAND_WORDS.machines, title: LIMIT_BAND_WORDS.machinesTitle, figure: LIMIT_BAND_WORDS.of(String(place.running), String(cap.machines)), share: place.running / Math.max(1, cap.machines), ink: SPLIT_INKS[1]!, ...(place.plan === undefined ? {} : { beside: LIMIT_BAND_WORDS.plan(place.plan.machinesAtOnce) }) });
  if (place.creditHours !== undefined) lanes.push({ k: "credit", word: LIMIT_BAND_WORDS.credit, title: LIMIT_BAND_WORDS.creditTitle, figure: LIMIT_BAND_WORDS.hoursLeft(place.creditHours), ink: SPLIT_INKS[4]! });
  if (cap !== undefined && place.spentTodayUsd !== undefined)
    lanes.push({ k: "spend", word: LIMIT_BAND_WORDS.spend, title: LIMIT_BAND_WORDS.spendTitle, figure: LIMIT_BAND_WORDS.of(fmtCost(place.spentTodayUsd), Number.isInteger(cap.spendPerDayUsd) ? `$${cap.spendPerDayUsd}` : fmtCost(cap.spendPerDayUsd)), share: place.spentTodayUsd / Math.max(0.01, cap.spendPerDayUsd), ink: SPLIT_INKS[3]! });
  return lanes;
}

export function LimitBand({ place }: { place: PlaceView }) {
  const lanes = lanesOf(place);
  if (lanes.length === 0) return null;
  return (
    <div data-limit-band className="col-span-full mt-3 mb-1 grid grid-cols-subgrid gap-y-2.5">
      {lanes.map(lane => {
        const near = lane.share !== undefined && lane.share >= NEAR_LIMIT;
        const ink = near ? "text-warning-foreground" : lane.ink;
        return (
          <div key={lane.k} data-limit-lane={lane.k} title={lane.title} className="col-span-full grid grid-cols-subgrid">
            <span className={PLOT}>
              {lane.share === undefined ? null : (
                <span aria-hidden className="block h-1 w-full overflow-hidden rounded-full bg-foreground/10">
                  <span className={cn("block h-full rounded-full bg-current transition-[width] duration-200 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none", ink)} style={{ width: `${Math.min(1, lane.share) * 100}%` }} />
                </span>
              )}
            </span>
            <span data-k={`limit-${lane.k}`} className={FIGURE}>
              <span className={WORD}>{lane.word}</span>
              <span className={near ? "text-warning-foreground" : "text-foreground"}>{lane.figure}</span>
              {lane.beside === undefined ? null : <span className="font-sans text-muted-foreground">{lane.beside}</span>}
            </span>
          </div>
        );
      })}
    </div>
  );
}
