// SPDX-License-Identifier: AGPL-3.0-only
// The band under a computer's row on the Computers list: a CPU lane over a memory lane, the last fifteen minutes in
// ten second steps, each lane's figure at its right. It is the row's own second line on the list's template, so the
// plot ends under the Memory column and the figures stand under the state and the chevron. It never animates.
import { fmtMemGb, memoryNearFull } from "@wsp/protocol";
import { ChartPlot, spanPoints } from "../components/machine/MachineSurface.js";
import { BAND_WINDOW_MS, useComputerReadings } from "../machine/computerLive.js";
import { cn } from "../lib/utils.js";
import { SPLIT_INKS } from "./usage.js";

const clock = new Intl.DateTimeFormat("en-GB", { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
const day = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" });
const lastRead = (at: number): string => `Last read ${new Date(at).toDateString() === new Date().toDateString() ? clock.format(at) : `${day.format(at)} ${clock.format(at)}`}`;

const AWAY_LANE = "text-muted-foreground/45";
/** The Usage chart's first two series inks, so CPU and memory read apart in both themes. */
const [CPU_INK, MEM_INK] = SPLIT_INKS;
/** The word takes a fixed slot, so both values start on one line. */
const WORD = "w-12.5 shrink-0 font-sans text-muted-foreground";
const PLOT = "col-span-3 max-md:col-span-2 pl-11";
const FIGURE = "col-span-3 max-md:col-span-2 flex items-center gap-1.5 whitespace-nowrap font-mono text-xs leading-4 tabular-nums";

/** away: the computer stopped answering, so the window is the last one it sent. */
export function LiveBand({ placeId, away }: { placeId: string; away: boolean }) {
  const points = useComputerReadings(placeId);
  const last = points.at(-1);
  if (last === undefined) return null;
  const to = last.at;
  const from = to - BAND_WINDOW_MS;
  const full = memoryNearFull(last.mem);
  const cpuInk = away ? AWAY_LANE : CPU_INK;
  const memInk = away ? AWAY_LANE : full ? "text-warning-foreground" : MEM_INK;
  return (
    <div data-live-band title="CPU and memory over the last 15 minutes" className="col-span-full mt-3 mb-1 grid grid-cols-subgrid grid-rows-[18px_18px] gap-y-2.5">
      <ChartPlot runs={[spanPoints(points, from, to, s => s.cpu / 100)]} label="CPU over the last 15 minutes" end={false} rules={false} fillOpacity={0.4} height="h-4.5" className={cn(PLOT, cpuInk)} />
      <span data-k="cpu" className={FIGURE}>
        <span className={WORD}>CPU</span>
        <span className={away ? "text-muted-foreground" : "text-foreground"}>{Math.round(last.cpuNow)}%</span>
      </span>
      <ChartPlot runs={[spanPoints(points, from, to, s => (s.mem.total > 0 ? s.mem.used / s.mem.total : 0))]} label="Memory over the last 15 minutes" end={false} rules={false} fillOpacity={0.4} height="h-4.5" className={cn(PLOT, memInk)} />
      <span data-k="mem" className={FIGURE}>
        {away ? (
          <span className="font-sans text-muted-foreground">{lastRead(to)}</span>
        ) : (
          <>
            <span className={WORD}>Memory</span>
            <span className={full ? "text-warning-foreground" : "text-foreground"}>{fmtMemGb(last.mem.used / 1024 / 1024)}</span>
          </>
        )}
      </span>
    </div>
  );
}
