// SPDX-License-Identifier: AGPL-3.0-only
// Two to four figures as equal tiles in one card, hairlines between them. Under 30rem of the card two a row, or three as
// one line each; all of them across from the least width that holds the widest tile, so no figure is squeezed.
import { slateFigureValue, type SlateJson } from "@wsp/protocol/slate";
import { cn } from "../../lib/utils.js";
import { CARD_SURFACE } from "../../settings/rows.js";
import type { PieceView } from "../SlateView.js";
import { figure, num, present, str, toneOf } from "./look.js";
import { placeOf } from "./runs.js";
import { Tile, type FigureSize } from "./tile.js";

/** Three tiles under 30rem: "lines", one a row as label, figure and ring; or "pairs", two a row and the third spanning. */
export const STATS_THREE: { narrow: "lines" | "pairs" } = { narrow: "lines" };

/** Whole class names, so Tailwind finds them: per count, the columns and the hairlines of the narrow card. */
const NARROW_CELLS: Record<number | "3-pairs", string> = {
  2: "grid-cols-2 [&>*+*]:border-l",
  3: "grid-cols-1 [&>*+*]:border-t",
  "3-pairs": "grid-cols-2 [&>*:nth-child(2)]:border-l [&>*:nth-child(3)]:col-span-2 [&>*:nth-child(3)]:border-t",
  4: "grid-cols-2 [&>*:nth-child(even)]:border-l [&>*:nth-child(n+3)]:border-t",
};

/** Per rem of the card from which three or four tiles stand across, their columns and hairlines. */
const ACROSS: Record<number, { 3: string; 4: string }> = {
  30: { 3: "@min-[30rem]:grid-cols-3 @min-[30rem]:[&>*+*]:border-t-0 @min-[30rem]:[&>*:nth-child(3)]:border-t-0 @min-[30rem]:[&>*+*]:border-l @min-[30rem]:[&>*:nth-child(3)]:col-span-1", 4: "@min-[30rem]:grid-cols-4 @min-[30rem]:[&>*:nth-child(n+3)]:border-t-0 @min-[30rem]:[&>*+*]:border-l" },
  34: { 3: "@min-[34rem]:grid-cols-3 @min-[34rem]:[&>*+*]:border-t-0 @min-[34rem]:[&>*:nth-child(3)]:border-t-0 @min-[34rem]:[&>*+*]:border-l @min-[34rem]:[&>*:nth-child(3)]:col-span-1", 4: "@min-[34rem]:grid-cols-4 @min-[34rem]:[&>*:nth-child(n+3)]:border-t-0 @min-[34rem]:[&>*+*]:border-l" },
  38: { 3: "@min-[38rem]:grid-cols-3 @min-[38rem]:[&>*+*]:border-t-0 @min-[38rem]:[&>*:nth-child(3)]:border-t-0 @min-[38rem]:[&>*+*]:border-l @min-[38rem]:[&>*:nth-child(3)]:col-span-1", 4: "@min-[38rem]:grid-cols-4 @min-[38rem]:[&>*:nth-child(n+3)]:border-t-0 @min-[38rem]:[&>*+*]:border-l" },
  42: { 3: "@min-[42rem]:grid-cols-3 @min-[42rem]:[&>*+*]:border-t-0 @min-[42rem]:[&>*:nth-child(3)]:border-t-0 @min-[42rem]:[&>*+*]:border-l @min-[42rem]:[&>*:nth-child(3)]:col-span-1", 4: "@min-[42rem]:grid-cols-4 @min-[42rem]:[&>*:nth-child(n+3)]:border-t-0 @min-[42rem]:[&>*+*]:border-l" },
  46: { 3: "@min-[46rem]:grid-cols-3 @min-[46rem]:[&>*+*]:border-t-0 @min-[46rem]:[&>*:nth-child(3)]:border-t-0 @min-[46rem]:[&>*+*]:border-l @min-[46rem]:[&>*:nth-child(3)]:col-span-1", 4: "@min-[46rem]:grid-cols-4 @min-[46rem]:[&>*:nth-child(n+3)]:border-t-0 @min-[46rem]:[&>*+*]:border-l" },
};
const STEPS = Object.keys(ACROSS).map(Number);

/** A tile of the narrow panel's 366 px card, two a row; a ring's room beside its figure, a trend's, a unit character's. */
const NARROW_TILE = 183;
const RING = 30;
const TREND = 72;
const UNIT_CH = 7.2;
const SIZES: readonly FigureSize[] = [26, 20, 16];

const record = (v: SlateJson | undefined): Record<string, SlateJson> => (v !== null && typeof v === "object" && !Array.isArray(v) ? v : {});

/** What a tile's figure row needs across at a figure size: its mono figure at 0.6 of the size a character, its unit,
 * and its ring or trend. */
export function tileNeed(text: string, unit: string | undefined, ring: boolean, trend: boolean, size: number): number {
  return text.length * size * 0.6 + (unit === undefined ? 0 : unit.length * UNIT_CH + 4) + (ring ? RING : trend ? TREND : 0);
}

/** The figure size of a row, the largest whose widest tile fits two a row; and the rem from which all stand across, or
 * none when even the widest step cannot hold them. */
export function statsFit(needs: (size: number) => number, count: number): { size: FigureSize; across: number | undefined } {
  // Two tiles pad 20 px a side, more pad 12.
  const pad = count === 2 ? 40 : 24;
  const size = SIZES.find(s => needs(s) + pad <= NARROW_TILE) ?? SIZES.at(-1)!;
  const room = (count * (needs(size) + pad) + 2) / 16;
  return { size, across: count === 2 ? undefined : STEPS.find(step => step >= (count === 4 ? 34 : 30) && step >= room) };
}

export const stats: PieceView = {
  type: "stats",
  card: false,
  component: function StatsPiece({ id, piece, props, slate }) {
    const tiles = (Array.isArray(props["stats"]) ? present(slate, piece.props?.["stats"], props["stats"]) : []).slice(0, 4).map(record);
    const shown = tiles.map(tile => {
      const value = slateFigureValue(tile["value"]);
      const max = num(tile["max"]);
      const format = tile["format"] ?? (max !== undefined ? "fraction" : "plain");
      if (typeof value !== "number") return str(value);
      if (format === "fraction" && max !== undefined) return `${figure(value, "plain")}/${figure(max, "plain")}`;
      if (format === "percent" && max !== undefined) return `${Math.round((value / max) * 100)}%`;
      return figure(value, format);
    });
    const lines = tiles.length === 3 && STATS_THREE.narrow === "lines";
    const parts = tiles.map((tile, at) => ({ text: shown[at] ?? "", unit: str(tile["unit"]), ring: num(tile["max"]) !== undefined, trend: Array.isArray(tile["trend"]) && tile["trend"].length > 1 }));
    const { size, across } = statsFit(s => Math.max(...parts.map(p => tileNeed(p.text, p.unit, p.ring, p.trend, s))), tiles.length);
    return (
      <div data-slate-stats className={cn("@container min-w-0", placeOf(slate, id) !== "inside" && CARD_SURFACE)}>
        <div className={cn("grid min-w-0 [&>*]:border-border/50 [&>*]:pt-4 [&>*]:pb-3.5", tiles.length >= 3 ? "[&>*]:px-3" : "[&>*]:px-(--settings-inset,20px)", NARROW_CELLS[tiles.length === 3 && !lines ? "3-pairs" : tiles.length], across === undefined ? null : ACROSS[across]?.[tiles.length as 3 | 4])}>
          {tiles.map((tile, at) => {
            const value = slateFigureValue(tile["value"]);
            const max = num(tile["max"]);
            const trend = Array.isArray(tile["trend"]) ? tile["trend"].filter((v): v is number => typeof v === "number" && Number.isFinite(v)) : [];
            return (
              <Tile
                key={at}
                label={str(tile["label"])}
                figure={shown[at]}
                size={size}
                line={lines ? (across ?? "always") : undefined}
                unit={str(tile["unit"])}
                window={str(tile["window"])}
                share={max !== undefined && max > 0 && typeof value === "number" ? value / max : undefined}
                trend={trend}
                tone={toneOf(tile["tone"], slate, id)}
              />
            );
          })}
        </div>
      </div>
    );
  },
};
