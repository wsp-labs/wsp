// SPDX-License-Identifier: AGPL-3.0-only
// The words the Image card says and the readings behind them: the record as
// chips, when and where it was built, what one copy of it holds, and what a
// copy build takes. Every word here is "image"; the person never reads golden, setup or
// copy as a noun for it. Neither of the two rules under this section lives
// here: whether a copy stands on the record is copyStanding in the protocol,
// which the command line reads too, and where today ends is daysBack in the
// app's timestamp reading, which every surface that says "today" reads.
import { catalogEntry } from "@wsp/catalog";
import { BUILD_TAKES_UNMEASURED, NO_IMAGE_FIX, chargesNothing, customRows, fmtBytes, fmtRate, plural, sealedLoginsHeld, subThreadsOnly, type PlaceView, type Recipe, type SealedImage, type SealedImageCopy } from "@wsp/protocol";
import { HardDriveIcon, KeyRoundIcon, TagIcon } from "lucide-react";
import type { ChipItem } from "../components/ui/chips.js";
import { placeNamed } from "./places.js";
import { APP_LOCALE, daysBack, parseTimestampDate } from "../lib/timestampFormat.js";

export const IMAGE_WORDS = {
  title: "Image",
  copies: "Copies",
  /** The chips of a copy standing on a computer's card. */
  builtChip: (when: string): string => `built ${when}`,
  behindChip: (version: number): string => `behind your image v${version}`,
  /** The Image card on a computer's page: its head, the title each state stands under, and its presses. */
  head: (computer: string): string => `Your image on ${computer}`,
  state: {
    nothing: "Not built yet",
    notHere: "Not here yet",
    reading: "Reading the image",
    copying: "Copying your image here",
    stale: "An older copy here",
    ready: "Ready",
  },
  noImage: `No image yet. ${NO_IMAGE_FIX}`,
  /** The row under the state that says what the image holds. */
  holds: "On your image",
  noRecipe: (version: number): string => `v${version} was sealed before what goes on it was kept.`,
  copyComes: (version: number): string => `v${version} comes with your sign-ins. Nothing is asked again.`,
  copyAsks: (version: number): string => `v${version} holds no sign-ins, so each is asked again here.`,
  copyHere: "Copy your image here",
  copyAnyway: "Copy anyway",
  tryAgain: "Try again",
  startTask: "New thread here",
  /** Said under the card while New thread here is held: a thread goes where its project lands, and none lands here. */
  startHeld: (computer: string): string => `No project lands its threads on ${computer} yet.`,
} as const;

/** Whether a ready card offers New thread here: a cloud met through a lead's sub-threads alone takes no thread
 * started by hand. */
export const startsThreadsHere = (place: Pick<PlaceView, "id" | "kind">): boolean => !(place.kind === "provider" && subThreadsOnly(place.id));

/** What a copy build takes, said beside the press that starts one, a line each: the time, and the rate where the
 * place bills. Stacked rather than joined, so the slot is as wide on a cloud that bills as on a computer that does
 * not and the chips beside it keep one line on both. */
export const copyCost = (rateUsdPerHour: number | undefined): string[] =>
  chargesNothing(rateUsdPerHour) ? [BUILD_TAKES_UNMEASURED] : [BUILD_TAKES_UNMEASURED, fmtRate(rateUsdPerHour)];

/** The clock and the date this section spells a stamp with, both in the app's own locale, as every other stamp in
 * the app reads: the shape of a stamp is the app's and not the shell's the run happens to start in. */
export const CLOCK = new Intl.DateTimeFormat(APP_LOCALE, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
const DAY = new Intl.DateTimeFormat(APP_LOCALE, { month: "short", day: "numeric" });

/** The record as chips: which version it is, what the image came to on disk and how many sign-ins it holds. A field
 * the record does not carry is left out rather than drawn as unknown. What the image carries is the Agents list's
 * to say, further down the same page, so the chips keep one line beside the press. */
export function recordChips(image: SealedImage): ChipItem[] {
  const size: ChipItem[] = image.usedBytes === undefined ? [] : [{ text: fmtBytes(image.usedBytes), icon: HardDriveIcon }];
  return [{ text: `v${image.version}`, icon: TagIcon }, ...size, { text: plural(sealedLoginsHeld(image), "sign-in"), icon: KeyRoundIcon }];
}

/** What the recipe puts on the image, as one comma list of names: the agents ticked, then the tools ticked and the
 * rows added by hand. A row outside the catalog reads by its own id. */
export function recipeNames(recipe: Recipe): string {
  const named = (kind: "agent" | "tool"): string[] => recipe.rows.filter(row => row.on && row.kind === kind).map(row => catalogEntry(row.id)?.name ?? row.id);
  return [named("agent"), [...named("tool"), ...customRows(recipe).map(row => row.name)]]
    .flat()
    .join(", ");
}

/** A stamp as the section reads one: the clock alone on the day it happened, the day and the clock before that, so
 * the two rows that carry a time read the same. Which day it belongs to is the app's own reading; these are only
 * the words this section says for it. */
export function builtWhen(at: string, now: number = Date.now()): string {
  const when = parseTimestampDate(at);
  if (when === null) return at;
  const clock = CLOCK.format(when);
  const back = daysBack(when, now);
  if (back <= 0) return `today ${clock}`;
  if (back === 1) return `yesterday ${clock}`;
  return `${DAY.format(when)} ${clock}`;
}

/** The copy one computer or provider holds, or nothing where it holds none. A copy names the place by the word a
 * person types for it, which is the id on some rows and the name on others, so the row's own reading answers it.
 * The one match, read by the Remove sentence, the copies table and the New workspace caption alike. */
export const copyOn = (copies: readonly SealedImageCopy[], place: PlaceView): SealedImageCopy | undefined => copies.find(copy => placeNamed(place, copy.place));
