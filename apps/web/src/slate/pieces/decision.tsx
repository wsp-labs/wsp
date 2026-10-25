// SPDX-License-Identifier: AGPL-3.0-only
// A question the person answers with one press: the question as a setting's title, the answers as keycaps, the
// agent's pick named under them with its reason, the other answers' notes under that, quieter. A press writes the answer to the piece's $value and raises change,
// which sends it; once answered, the answer stands in the row with a check and a quiet Change. A held decision says
// why under itself, as a held button does.
import { useRef } from "react";
import { Check } from "lucide-react";
import type { SlateJson } from "@wsp/protocol/slate";
import { Button } from "../../components/ui/button.js";
import { cn } from "../../lib/utils.js";
import { NOTE, SETTING_TITLE } from "../../settings/layout.js";
import type { PieceView } from "../SlateView.js";
import { getOwn } from "../paths.js";
import { heldBy, present, str } from "./look.js";
import { Outcome } from "./outcome.js";
import { twoWayPath, usePress } from "./press.js";

interface Answer { value: SlateJson; label: string; note: string | undefined }

function answersOf(value: SlateJson | undefined): Answer[] {
  if (!Array.isArray(value)) return [];
  return value.map(option =>
    option !== null && typeof option === "object" && !Array.isArray(option) && "value" in option
      ? { value: option["value"] ?? null, label: str(option["label"]) ?? str(option["value"]) ?? "", note: str(option["note"]) }
      : { value: option, label: str(option) ?? "", note: undefined },
  );
}

const same = (a: SlateJson | undefined, b: SlateJson | undefined): boolean => a !== undefined && a !== null && a !== "" && JSON.stringify(a) === JSON.stringify(b);

export const decision: PieceView = {
  type: "decision",
  component: function DecisionPiece({ piece, props, slate, sender, raise }) {
    const path = twoWayPath(piece.props?.["value"]);
    const picked = path === undefined ? props["value"] : getOwn(slate.values, path);
    const held = heldBy(props["held"]);
    const answers = answersOf(Array.isArray(props["options"]) ? present(slate, piece.props?.["options"], props["options"]) : undefined).slice(0, 4);
    const mine = answers.find(answer => same(answer.value, props["pick"]));
    const answered = answers.find(answer => same(answer.value, picked));
    const others = answers.filter(answer => answer !== mine && answer.note !== undefined);
    const next = useRef<SlateJson>(null);
    const { busy, said, refused, press } = usePress(async () => {
      if (path === undefined) return {};
      await sender.now(path, next.current);
      return next.current === "" || piece.on?.change === undefined ? {} : raise("change");
    });
    const choose = (value: SlateJson) => {
      next.current = value;
      press();
    };
    return (
      <div data-slate-decision={answered === undefined ? "open" : "answered"} className="flex min-w-0 flex-col gap-2.5">
        <span className={cn(SETTING_TITLE, "break-words", answered !== undefined && "font-normal text-muted-foreground")}>{str(props["label"])}</span>
        {answered === undefined ? (
          <>
            <span role="group" aria-label={str(props["label"])} className="flex min-w-0 flex-wrap gap-2">
              {answers.map((answer, at) => (
                <Button key={at} data-slate-answer={at} data-slate-mine={answer === mine ? "" : undefined} variant="outline" disabled={busy || held !== undefined || path === undefined} onClick={() => choose(answer.value)} className={cn(answer === mine && "border-foreground/30")}>
                  {answer.label}
                </Button>
              ))}
            </span>
            {/* The other answers' notes stand under the pick's, quieter, so each answer's cost reads without a hover. */}
            {mine === undefined && others.length === 0 ? null : (
              <span className="flex min-w-0 flex-col gap-1">
                {mine === undefined ? null : <span className={NOTE}>My pick: {mine.label}{mine.note === undefined ? "" : `. ${mine.note}`}</span>}
                {others.map((answer, at) => (
                  <span key={at} data-slate-answer-note className={cn(NOTE, "text-muted-foreground/70")}>{answer.label}: {answer.note}</span>
                ))}
              </span>
            )}
          </>
        ) : (
          <span className="flex min-w-0 items-center gap-2">
            <Check aria-hidden className="size-4 shrink-0 text-success" />
            <span className="min-w-0 flex-1 text-sm leading-5 text-foreground">{answered.label}</span>
            <Button variant="ghost" size="xs" disabled={busy || held !== undefined} onClick={() => choose("")} className="h-auto px-0 text-muted-foreground hover:bg-transparent hover:text-foreground">
              Change
            </Button>
          </span>
        )}
        {held === undefined ? null : <span data-slate-held-why className={NOTE}>{held}</span>}
        <Outcome said={said} refused={refused} />
      </div>
    );
  },
};
