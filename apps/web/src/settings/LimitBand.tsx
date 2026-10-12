// SPDX-License-Identifier: AGPL-3.0-only
// The band under a cloud's row on the Computers list, after the cloud's own
// plan panel: one row of columns, each a muted label, its figure under it in
// the mono and one thin neutral bar under that, filled in the foreground ink
// at low weight, the warning ink from nine tenths and the danger ink at the
// limit; then one quiet line on how the starts are counted. The credit and the
// day's spend have nothing to fill toward and stand as plain figures in the
// row's own note. Read off the row the places list carries, never asked again.
import { fmtCost, type PlaceView } from "@wsp/protocol";
import { cn } from "../lib/utils.js";

/** The share from which a bar takes the warning ink, as the Usage page's meters do. */
const NEAR_LIMIT = 0.9;

export const LIMIT_BAND_WORDS = {
  machines: "Machines at once",
  minute: "Starts this minute",
  hour: "Starts this hour",
  day: "Starts today",
  of: (used: number, limit: number): string => `${used} of ${limit}`,
  counted: "Starts count over rolling windows; a create, a fork and a wake each count as one.",
  credit: (hours: number): string => `${hours < 10 ? hours.toFixed(1) : Math.round(hours)} h of credit left`,
  spend: (today: number, cap: number): string => `${fmtCost(today)} of ${Number.isInteger(cap) ? `$${cap}` : fmtCost(cap)} today`,
} as const;

/** The cloud row's note: the credit left and the day's spend against the spend per day, where the row carries them. */
export function limitNote(place: PlaceView): string | undefined {
  const cap = place.cap !== undefined && "machines" in place.cap ? place.cap : undefined;
  const parts = [place.creditHours === undefined ? null : LIMIT_BAND_WORDS.credit(place.creditHours), cap === undefined || place.spentTodayUsd === undefined ? null : LIMIT_BAND_WORDS.spend(place.spentTodayUsd, cap.spendPerDayUsd)].filter((p): p is string => p !== null);
  return parts.length === 0 ? undefined : parts.join(", ");
}

interface Column {
  readonly k: string;
  readonly label: string;
  readonly used: number;
  readonly limit: number;
}

function columnsOf(place: PlaceView): Column[] {
  const out: Column[] = [];
  const plan = place.plan;
  if (plan?.running !== undefined) out.push({ k: "machines", label: LIMIT_BAND_WORDS.machines, used: plan.running, limit: plan.machinesAtOnce });
  const starts = place.starts;
  if (starts !== undefined) {
    out.push({ k: "minute", label: LIMIT_BAND_WORDS.minute, used: starts.minute.used, limit: starts.minute.limit });
    out.push({ k: "hour", label: LIMIT_BAND_WORDS.hour, used: starts.hour.used, limit: starts.hour.limit });
    out.push({ k: "day", label: LIMIT_BAND_WORDS.day, used: starts.day.used, limit: starts.day.limit });
  }
  return out;
}

export function LimitBand({ place }: { place: PlaceView }) {
  const columns = columnsOf(place);
  if (columns.length === 0) return null;
  return (
    <div data-limit-band className="col-span-full mt-3 mb-1 flex flex-col gap-3 pl-11">
      <div className="grid grid-cols-4 gap-x-6 gap-y-4 max-md:grid-cols-2">
        {columns.map(column => {
          const share = column.limit > 0 ? column.used / column.limit : 0;
          const fill = share >= 1 ? "bg-destructive" : share >= NEAR_LIMIT ? "bg-warning" : "bg-foreground/55";
          const ink = share >= 1 ? "text-destructive-foreground" : share >= NEAR_LIMIT ? "text-warning-foreground" : "text-foreground";
          return (
            <div key={column.k} data-limit-column={column.k} className="flex min-w-0 flex-col gap-1">
              <span className="truncate text-xs text-muted-foreground">{column.label}</span>
              <span data-k={`limit-${column.k}`} className={cn("font-mono text-xs leading-4 tabular-nums", ink)}>
                {LIMIT_BAND_WORDS.of(column.used, column.limit)}
              </span>
              <span aria-hidden className="mt-1 block h-1 w-full overflow-hidden rounded-full bg-foreground/10">
                <span className={cn("block h-full rounded-full transition-[width] duration-200 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none", fill)} style={{ width: `${Math.min(1, share) * 100}%` }} />
              </span>
            </div>
          );
        })}
      </div>
      {place.starts === undefined ? null : <p className="text-xs text-muted-foreground">{LIMIT_BAND_WORDS.counted}</p>}
    </div>
  );
}
