// SPDX-License-Identifier: AGPL-3.0-only
// Two to four figures as equal tiles in one card, hairlines between them: all of them across from 34rem of the card,
// and four as two rows of two under it, so no figure is squeezed in the 400 px panel.
import { slateFigureValue, type SlateJson } from "@wsp/protocol/slate";
import { cn } from "../../lib/utils.js";
import { CARD_SURFACE } from "../../settings/rows.js";
import type { PieceView } from "../SlateView.js";
import { figure, num, present, str, toneOf } from "./look.js";
import { placeOf } from "./runs.js";
import { Tile } from "./tile.js";

/** Whole class names, so Tailwind finds them: per count, the columns and the hairlines of the narrow and the wide card. */
const CELLS: Record<number, string> = {
  2: "grid-cols-2 [&>*+*]:border-l",
  3: "grid-cols-3 [&>*+*]:border-l",
  4: "grid-cols-2 [&>*:nth-child(even)]:border-l [&>*:nth-child(n+3)]:border-t @min-[34rem]:grid-cols-4 @min-[34rem]:[&>*:nth-child(n+3)]:border-t-0 @min-[34rem]:[&>*+*]:border-l",
};

/** The narrow panel card's inner width, one character of the 26 px mono figure, and of its 12 px unit. */
const NARROW = 366;
const FIGURE_CH = 15.6;
const UNIT_CH = 7.2;

const record = (v: SlateJson | undefined): Record<string, SlateJson> => (v !== null && typeof v === "object" && !Array.isArray(v) ? v : {});

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
    // The narrow panel's card holds 366 px; a tile pads 12 px a side once three share a row.
    const across = tiles.length === 4 ? 2 : tiles.length;
    const small = shown.some((text, at) => (text ?? "").length * FIGURE_CH + (str(tiles[at]?.["unit"])?.length ?? 0) * UNIT_CH > NARROW / across - 24);
    return (
      <div data-slate-stats className={cn("@container min-w-0", placeOf(slate, id) !== "inside" && CARD_SURFACE)}>
        <div className={cn("grid min-w-0 [&>*]:border-border/50 [&>*]:pt-4 [&>*]:pb-3.5", tiles.length >= 3 ? "[&>*]:px-3" : "[&>*]:px-(--settings-inset,20px)", CELLS[tiles.length] ?? CELLS[4])}>
          {tiles.map((tile, at) => {
            const value = slateFigureValue(tile["value"]);
            const max = num(tile["max"]);
            const trend = Array.isArray(tile["trend"]) ? tile["trend"].filter((v): v is number => typeof v === "number" && Number.isFinite(v)) : [];
            return (
              <Tile
                key={at}
                label={str(tile["label"])}
                figure={shown[at]}
                small={small}
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
