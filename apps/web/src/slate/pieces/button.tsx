// SPDX-License-Identifier: AGPL-3.0-only
import { slateLinkDomain } from "@wsp/protocol/slate";
import { stepsOf, type SlateStep } from "../model.js";
import { Button, DANGER_BUTTON } from "../../components/ui/button.js";
import type { PieceView } from "../SlateView.js";
import { NOTE, str, heldBy } from "./look.js";
import { placeOf } from "./runs.js";
import { Outcome } from "./outcome.js";
import { usePress } from "./press.js";

/** The hover title a press shows before it is pressed: what each step does, a send's literal text and the paths it
 * carries. */
export function pressTitle(steps: SlateStep | SlateStep[] | undefined, resolve?: (value: unknown) => unknown): string | undefined {
  const lines = stepsOf(steps).map(step => {
    switch (step.do) {
      case "send":
      case "steer":
      case "queue":
      case "fill":
        return `${step.text}${step.with !== undefined && step.with.length > 0 ? ` (with ${step.with.join(", ")})` : ""}`;
      case "start":
        return `Runs ${step.run}`;
      case "cancel":
        return `Stops ${step.run}`;
      case "set":
      case "toggle":
        return `Changes ${step.path}`;
      case "open": {
        // The domain a press would contact, said before it is pressed.
        const href = resolve?.(step.target);
        const domain = typeof href === "string" ? slateLinkDomain(href) : undefined;
        return domain === undefined ? "Opens a link" : `Opens ${domain}`;
      }
      case "copy":
        return "Copies";
      case "pane":
        return `Opens ${step.kind}`;
    }
  });
  return lines.length === 0 ? undefined : lines.join("\n");
}

/** A note replaces the step list on hover, except the domain a link would open, which is always said. */
function hoverTitle(note: string | undefined, steps: SlateStep | SlateStep[] | undefined, resolve: (value: unknown) => unknown): string | undefined {
  if (note === undefined) return pressTitle(steps, resolve);
  const opens = stepsOf(steps).filter(step => step.do === "open");
  return opens.length === 0 ? note : `${note}\n${pressTitle(opens, resolve)}`;
}

export const button: PieceView = {
  type: "button",
  card: false,
  control: true,
  component: function ButtonPiece({ id, piece, props, slate, raise }) {
    const { busy, said, refused, press } = usePress(() => raise("press"));
    const label = str(props["label"]) ?? "";
    const held = heldBy(props["held"]);
    const kind = props["variant"];
    // The place sizes a button: the xs keycap in a card row's slot, the full keycap under a card or in the toolbar.
    const parent = slate.parentId(id);
    const holder = parent === undefined ? undefined : slate.piece(parent)?.type;
    const small = (holder === "row" || holder === "empty") && placeOf(slate, id) === "inside";
    const variant = kind === "primary" && slate.isLoud("primary", id) ? "default" : kind === "quiet" ? "ghost" : "outline";
    const control = (
      <Button
        variant={variant}
        size={small ? "xs" : "default"}
        className={kind === "danger" ? DANGER_BUTTON : undefined}
        held={held !== undefined}
        disabled={busy}
        aria-busy={busy || undefined}
        title={held === undefined ? hoverTitle(str(props["note"]), piece.on?.press, value => slate.resolve(value as never)) : undefined}
        onClick={press}
      >
        {label}
      </Button>
    );
    // A held button says why under itself, where the person looks for it, not in a tooltip a disabled button never opens.
    return (
      <div data-slate-held-button={held} className="flex min-w-0 flex-col items-start gap-1">
        {control}
        {held === undefined ? null : <span data-slate-held-why className={NOTE}>{held}</span>}
        <Outcome said={said} refused={refused} />
      </div>
    );
  },
};
