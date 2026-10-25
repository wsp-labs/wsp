// SPDX-License-Identifier: AGPL-3.0-only
// What stands of your image on one computer, as imageState reads it, and the
// one press that state invites, drawn two ways. As a card, under Add a
// computer once that computer is added: under the state what the image holds.
// As the Image row of a computer's or a cloud's page: one line of where the
// copy stands and what the image holds, and its press. A copy is built only on
// Copy, never on opening the page, and on the card the time and the rate stand
// beside the press, since a build bills where it runs.
import { FileIcon } from "lucide-react";
import { useState } from "react";
import { copyAsksSignIns, type PlaceView, type SealedImageView } from "@wsp/protocol";
import { Button } from "../components/ui/button.js";
import type { ChipItem } from "../components/ui/chips.js";
import { Spinner } from "../components/ui/spinner.js";
import { failureOf, type Failure } from "../protocol/failure.js";
import { useGoldenFrames, useStore } from "../protocol/store.js";
import { FACT, WHERE_WORDS } from "./format.js";
import { builtWhen, copyCost, IMAGE_WORDS, recipeNames, startsThreadsHere } from "./image.js";
import { GlyphFrame, Grid, GridHead, GridName, GridRow, Num, PAGE_COLUMNS } from "./grid.js";
import { imageChips, imageState, type ImageState } from "./imageState.js";
import { placeName, projectOn } from "./places.js";
import { RefusalSlot } from "./sheetParts.js";
import { CARD_SURFACE, Row, type SettingsCardData, type SettingsRowData } from "./rows.js";
import type { SettingsContext } from "./settingsContext.js";
import { cn } from "../lib/utils.js";

/** The Image card under its head for one computer, drawn by Add a computer once that computer is added: nothing
 * until the host has read the image, and nothing where the computer can hold none. */
export function useImageCard(place: PlaceView, ctx: SettingsContext): SettingsCardData | undefined {
  const standing = useImageStanding(place, ctx);
  if (standing === undefined) return undefined;
  const { name, state, view } = standing;
  return { id: "image", head: IMAGE_WORDS.head(name), items: [], body: <ImageCard place={place} name={name} state={state} view={view} ctx={ctx} /> };
}

/** Where your image stands on one computer, the one reading the card and the line under a held cloud key share. */
export function useImageStanding(place: PlaceView, ctx: SettingsContext): { name: string; state: ImageState; view: SealedImageView; title: string } | undefined {
  const frames = useGoldenFrames();
  const view = ctx.reads.image;
  const read = view === null ? undefined : imageState(place, { view, frames });
  // An answer from before the latest seal holds no word on what stands, but a copy its frames still show does; with
  // no read out to replace it nothing stands, as on the Computers row.
  const stale = ctx.reads.imageStale && read !== undefined && read.kind !== "copying";
  if (view === null || read === undefined || (stale && !ctx.reads.imageReading)) return undefined;
  const state: ImageState = stale ? { kind: "reading" } : read;
  return { name: placeName(place), state, view, title: standingTitle(state, view.image !== null) };
}

/** The title a state stands under: with no image anywhere, nothing here is a copy yet. */
function standingTitle(state: ImageState, imageExists: boolean): string {
  switch (state.kind) {
    case "none":
    case "stopped":
      return imageExists ? IMAGE_WORDS.state.notHere : IMAGE_WORDS.state.nothing;
    case "reading":
      return IMAGE_WORDS.state.reading;
    case "copying":
      return IMAGE_WORDS.state.copying;
    case "stale":
      return IMAGE_WORDS.state.stale;
    case "ready":
      return IMAGE_WORDS.state.ready;
  }
}

/** The one press a state invites, and why it is held where it is: said under the card, never on a hover. */
type Press = { word: string; heldWhy?: string; run?: () => void };

export function ImageCard({ place, name, state, view, ctx, row: asRow = false }: { place: PlaceView; name: string; state: ImageState; view: SealedImageView; ctx: SettingsContext; /** Drawn as the IMAGE list of a computer's or a cloud's page rather than as a card. */ row?: boolean }) {
  const [pressing, setPressing] = useState(false);
  const [refused, setRefused] = useState<Failure | null>(null);
  const build = ctx.api?.imageBuild;
  const image = view.image;
  const force = image !== null && copyAsksSignIns(image);
  const copy = (): void => {
    if (build === undefined) return;
    setPressing(true);
    setRefused(null);
    void build(place.id, force || undefined)
      .catch((e: unknown) => setRefused(failureOf(e)))
      .finally(() => setPressing(false));
  };
  const copyPress: Press = { word: force ? IMAGE_WORDS.copyAnyway : IMAGE_WORDS.copyHere, ...(build === undefined ? { heldWhy: WHERE_WORDS.notYet } : { run: copy }) };
  const project = projectOn(place, ctx.projects);
  const cost = copyCost(place.rateUsdPerHour);
  const title = standingTitle(state, image !== null);

  const row = ((): { title: string; description?: string; mono?: boolean; chips?: ChipItem[]; cost?: string[]; note?: string; press?: Press; busy?: boolean } => {
    switch (state.kind) {
      case "none":
        if (image === null) return { title, description: IMAGE_WORDS.noImage };
        return { title, description: force ? IMAGE_WORDS.copyAsks(image.version) : IMAGE_WORDS.copyComes(image.version), cost, press: copyPress };
      case "copying":
        return { title, description: state.line, mono: true, busy: true };
      case "reading":
        return { title, busy: true };
      case "stopped":
        return image === null
          ? { title, description: state.said, mono: true }
          : { title, description: state.said, mono: true, cost, ...(force ? { note: IMAGE_WORDS.copyAsks(image.version) } : {}), press: { ...copyPress, word: force ? IMAGE_WORDS.copyAnyway : IMAGE_WORDS.tryAgain } };
      case "stale":
        return { title, chips: imageChips(state, ctx.now), cost, press: copyPress };
      case "ready":
        if (!startsThreadsHere(place)) return { title, chips: imageChips(state, ctx.now) };
        return project === undefined
          ? { title, chips: imageChips(state, ctx.now), press: { word: IMAGE_WORDS.startTask, heldWhy: IMAGE_WORDS.startHeld(name) } }
          : {
              title,
              chips: imageChips(state, ctx.now),
              press: {
                word: IMAGE_WORDS.startTask,
                run: () => useStore.getState().openProjectHome(project.id),
              },
            };
    }
  })();

  const press = row.press;
  const control = row.busy ? (
    <Spinner className="size-4 text-muted-foreground" />
  ) : press === undefined ? undefined : (
    <>
      {row.cost === undefined || asRow ? null : (
        <span data-k="image-cost" className={cn(FACT, "flex shrink-0 flex-col items-end whitespace-nowrap leading-4")}>
          {row.cost.map(line => (
            <span key={line}>{line}</span>
          ))}
        </span>
      )}
      <Button data-k="image-press" size="xs" variant={asRow ? "outline" : "default"} held={press.heldWhy !== undefined} disabled={pressing} onClick={press.run}>
        {press.word}
      </Button>
    </>
  );
  const waiting = press?.heldWhy ?? row.note;
  const stateRow: Omit<SettingsRowData, "kind"> = {
    id: "image-state",
    title: row.title,
    description: row.description ?? row.chips?.map(chip => chip.text) ?? "",
    mono: row.mono === true,
    ...(row.chips === undefined ? {} : { chips: row.chips }),
    ...(control === undefined ? {} : { control }),
    attrs: { "data-k": "image-state", "data-state": state.kind },
  };
  const holds: Omit<SettingsRowData, "kind"> | undefined =
    image === null
      ? undefined
      : {
          id: "image-holds",
          title: IMAGE_WORDS.holds,
          description: image.recipe === undefined ? IMAGE_WORDS.noRecipe(image.version) : recipeNames(image.recipe),
          attrs: { "data-k": "image-holds" },
        };
  // The row's note: where the copy here stands, then what the image holds on one line.
  const standingNote = state.kind === "ready" ? IMAGE_WORDS.builtChip(builtWhen(state.copy.builtAt, ctx.now)) : (row.description ?? row.chips?.map(chip => chip.text).join(", ") ?? row.title);
  const sentence = (part: string): string => (/[.!?]$/.test(part) ? part : `${part}.`);
  const note = [standingNote, ...(image?.recipe === undefined ? [] : [`${IMAGE_WORDS.holds}: ${recipeNames(image.recipe)}`])].filter(part => part !== "").map(sentence).join(" ");
  const list = (
    <Grid id="image" head={<GridHead columns={PAGE_COLUMNS} cells={[{ word: WHERE_WORDS.heads.image }]} />}>
      <GridRow columns={PAGE_COLUMNS} tight attrs={{ "data-k": "image-state", "data-state": state.kind }}>
        <GridName glyph={<GlyphFrame><FileIcon aria-hidden className="size-4 text-foreground/80" /></GlyphFrame>} name={WHERE_WORDS.yourImage} note={note} />
        <Num wideOnly />
        <span className="flex min-w-0 items-center justify-self-end gap-3">{control}</span>
      </GridRow>
    </Grid>
  );
  return (
    <div className="flex flex-col gap-3">
      {asRow ? (
        list
      ) : (
        <div className={cn(CARD_SURFACE, "flex flex-col [&>*+*]:border-t [&>*+*]:border-border/50")}>
          <Row {...stateRow} />
          {holds === undefined ? null : <Row {...holds} />}
        </div>
      )}
      {asRow && refused === null && waiting === undefined ? null : (
        // The slot stands in every state, so a refusal or a held press's reason arriving moves nothing under the card.
        <RefusalSlot k="image-refusal" {...(refused === null ? {} : { said: refused.said, ...(refused.fix === undefined ? {} : { fix: refused.fix }) })} {...(waiting === undefined ? {} : { waiting })} />
      )}
    </div>
  );
}
