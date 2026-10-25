// SPDX-License-Identifier: AGPL-3.0-only
// A count of rows done over a list, then only the rows that are not: the count's figure and what it counts over a
// track that fills to its share, easing from where it stood, and under it a row per exception, its state's empty
// circle, its name and its note. With none left the count says All and the card holds the head alone.
import { Circle } from "lucide-react";
import { cn } from "../../lib/utils.js";
import { CARD_SURFACE } from "../../settings/rows.js";
import type { PieceView } from "../SlateView.js";
import { truthy } from "../actions.js";
import { rowKeys, str } from "./look.js";
import { placeOf } from "./runs.js";

/** Past this many exceptions the card says how many more there are. */
const SHOWN = 8;

export const progress: PieceView = {
  type: "progress",
  card: false,
  rowScoped: ["name", "done", "note", "key"],
  component: function ProgressPiece({ id, piece, props, slate }) {
    const items = Array.isArray(props["items"]) ? props["items"] : [];
    const read = items.map((item, index) => {
      const at = { item, index };
      return { done: truthy(slate.resolve(piece.props?.["done"], at)), name: str(slate.resolve(piece.props?.["name"], at)) ?? "", note: str(slate.resolve(piece.props?.["note"], at)), key: piece.props?.["key"] === undefined ? undefined : slate.resolve(piece.props["key"], at) };
    });
    const open = read.filter(row => !row.done);
    const done = read.length - open.length;
    const keys = rowKeys(open.map(row => row.key));
    const share = read.length === 0 ? 0 : done / read.length;
    const label = str(props["label"]) ?? "";
    const inset = placeOf(slate, id) === "inside" ? "" : "px-(--settings-inset,20px)";
    return (
      <div data-slate-progress={`${done}/${read.length}`} className={cn("flex min-w-0 flex-col [&>*+*]:border-t [&>*+*]:border-border/50", placeOf(slate, id) !== "inside" && CARD_SURFACE)}>
        <div className={cn("flex min-w-0 flex-col gap-2.5 py-3.5", inset)}>
          <span className="flex min-w-0 flex-wrap items-baseline gap-x-1.5">
            <span className="font-mono text-sm leading-6 font-medium tabular-nums text-foreground">{items.length === 0 ? "None yet" : open.length === 0 ? `All ${read.length}` : `${done} of ${read.length}`}</span>
            {items.length === 0 ? null : <span className="text-sm leading-5 text-muted-foreground">{label}</span>}
          </span>
          <span aria-hidden className="block h-1 overflow-hidden rounded-full bg-foreground/10">
            <span data-slate-fill className={cn("block h-full rounded-full transition-[width] duration-700 ease-[cubic-bezier(0.23,1,0.32,1)] motion-reduce:transition-none", open.length === 0 ? "bg-success" : "bg-foreground/55")} style={{ width: `${Math.round(share * 1000) / 10}%` }} />
          </span>
        </div>
        {open.slice(0, SHOWN).map((row, at) => (
          <div key={keys[at]} data-slate-exception className={cn("flex min-h-11 min-w-0 items-start gap-3 py-3", inset)}>
            <Circle aria-hidden className="mt-0.5 size-4 shrink-0 text-muted-foreground/70" />
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-sm leading-5 break-words text-foreground">{row.name}</span>
              {row.note === undefined || row.note === "" ? null : <span className="text-[13px] leading-5 break-words text-muted-foreground">{row.note}</span>}
            </span>
          </div>
        ))}
        {open.length > SHOWN ? <p className={cn("py-2.5 text-xs leading-4 text-muted-foreground", inset)}>and {open.length - SHOWN} more not done</p> : null}
      </div>
    );
  },
};
