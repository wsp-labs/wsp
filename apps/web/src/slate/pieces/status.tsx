// SPDX-License-Identifier: AGPL-3.0-only
// A state is its word, capitalised, in its tone and weight 500 when toned, never a dot (the design law's State). Alone
// on the page it stands bare as the slate's state line rather than fill a card with one short line; after a card's
// rows it is a row of that card.
import { cn } from "../../lib/utils.js";
import type { PieceView } from "../SlateView.js";
import { str, TONE_INK, toneOf } from "./look.js";
import { riddenBy } from "./riders.js";
import { capitalised } from "../../settings/format.js";


export const status: PieceView = {
  type: "status",
  card: false,
  component: ({ id, props, slate }) => {
    // A status beside a number rides that number's note.
    if (riddenBy(slate, id) !== undefined) return null;
    const tone = toneOf(props["tone"], slate, id);
    const toned = tone !== "default" && tone !== "muted";
    return (
      <span data-slate-status={tone} className={cn("inline-flex min-w-0 text-[13px] leading-5", toned ? cn(TONE_INK[tone], "font-medium") : "text-muted-foreground")}>
        <span className="min-w-0 break-words">{capitalised(str(props["value"]) ?? "")}</span>
      </span>
    );
  },
};
