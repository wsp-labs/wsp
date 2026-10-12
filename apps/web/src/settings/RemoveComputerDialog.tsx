// SPDX-License-Identifier: AGPL-3.0-only
// The one confirmation before a computer or a provider is taken back out. The
// sentence is computed from what that computer holds right now, so a person
// reads what leaves rather than a warning; the host's own refusal lands under
// it in the refusal slot, with the host's fix.
//
// What the remove takes with the computer is read from the host as the dialog
// opens, the confirm held until it lands: work a task or a project folder there
// holds that no remote has is the slot's muted note, kept to the slot's two
// lines so nothing moves when it arrives, with the confirm reading Remove
// anyway, which takes it too. The refusal ink is the host's refusal alone. The
// projects the sentence names are the store's, there before the dialog opens.
//
// A computer that is not answering cannot be swept from here, so the dialog
// hands over the line that sweeps it on the computer itself. One with forks or
// projects on it, which go over its link, is only forgotten here, and so is a
// cloud that refused its key: the key and the records go and nothing there is
// asked. Where the login it was added over runs sudo only with a password, the
// refusal asks for it in a field under the slot, held in this dialog alone and
// sent with the next Remove.
import { useEffect, useState } from "react";
import { useSettingsStore } from "./settingsStore.js";
import { PLACE_SUDO_KIND, PLACES_WORDS, isProviderPlace, placeForgetsOnly, placeUnsavedRefusal, subHeldOnRemoveLine, type PlaceHolds, type PlaceView } from "@wsp/protocol";
import { AlertDialog, AlertDialogClose, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogPopup, AlertDialogTitle } from "../components/ui/alert-dialog.js";
import { Button, NEUTRAL_RING } from "../components/ui/button.js";
import { Input } from "../components/ui/input.js";
import { cn } from "../lib/utils.js";
import { failureOf } from "../protocol/failure.js";
import { useSidebarProjects, useStore } from "../protocol/store.js";
import { ADD_COMPUTER_WORDS, WHERE_WORDS } from "./format.js";
import { ROW_FIELD } from "./layout.js";
import { forgetSentence, forgetTitle, hereName, placeIsOffline, placeName, providerForgetSentence, removeSentence, removeTitle, removedTitle, type PlaceHolding } from "./places.js";
import { CopyRow, RefusalSlot } from "./sheetParts.js";

/** What the app says when its own client carries no remove road: the same shape the forget dialog's refusal has. */
export const CANNOT_REMOVE = "this wsp cannot take a computer back out from here";

export function RemoveComputerDialog({ place, holding, imageBytes, open, onOpenChange, onRemoved }: { place: PlaceView; holding: PlaceHolding; imageBytes?: number; open: boolean; onOpenChange: (open: boolean) => void; onRemoved?: () => void }) {
  const api = useStore(s => s.api);
  const places = useStore(s => s.places);
  const [busy, setBusy] = useState(false);
  const [refusal, setRefusal] = useState<{ said: string; fix?: string } | null>(null);
  const [asksSudo, setAsksSudo] = useState(false);
  const [password, setPassword] = useState("");
  const [holds, setHolds] = useState<PlaceHolds | null>(null);
  const [keyRefused, setKeyRefused] = useState(false);
  const [madeNote, setMadeNote] = useState<string | null>(null);
  const holdParting = useSettingsStore(s => s.holdParting);
  const unsaved = holds?.unsaved ?? [];
  const forgets = keyRefused || (holds !== null && placeForgetsOnly(holds));
  const reading = open && api?.placeHolds !== undefined && holds === null && refusal === null;
  const projects = useStore(s => s.projects).filter(p => p.computer === place.id).map(p => p.name);
  // The sub-threads on a cloud whose work holds their machines, by the titles the sidebar shows; the host's lines name
  // them by id, and are said instead where the rows here have not caught up.
  const heldTitles = useSidebarProjects()
    .flatMap(p => p.threads)
    .filter(t => t.sub?.held !== undefined && (t.sub.on === place.id || t.sub.on === place.name))
    .map(t => t.title);

  useEffect(() => {
    if (!open || api?.placeHolds === undefined) return;
    let gone = false;
    api.placeHolds(place.id).then(
      read => {
        if (!gone) setHolds(read);
      },
      (e: unknown) => {
        if (gone) return;
        const failure = failureOf(e);
        setRefusal({ said: failure.said, ...(failure.fix === undefined ? {} : { fix: failure.fix }) });
      },
    );
    return () => {
      gone = true;
    };
  }, [open, api, place.id]);

  useEffect(() => () => holdParting(null), [holdParting]);

  const change = (next: boolean): void => {
    if (!next && madeNote !== null) {
      holdParting(null);
      onRemoved?.();
    }
    if (!next) {
      setRefusal(null);
      setAsksSudo(false);
      setPassword("");
      setKeyRefused(false);
    }
    onOpenChange(next);
  };

  const remove = async (): Promise<void> => {
    if (api?.removePlace === undefined) {
      setRefusal({ said: CANNOT_REMOVE });
      return;
    }
    const typed = asksSudo && password !== "" ? password : undefined;
    setPassword("");
    setBusy(true);
    setRefusal(null);
    // Held from the ask on: the host drops the place from the list before its answer lands.
    holdParting(place.id);
    try {
      const answer = await api.removePlace(place.id, typed, unsaved.length > 0, forgets);
      // A remove made with something still to say, where the key still is, stays open on it until Done.
      if (answer.removed && answer.note !== undefined) {
        setMadeNote(answer.note);
        return;
      }
      holdParting(null);
      // A remove the host did not make stays on screen with its reason: a provider that would not delete a machine
      // keeps its key, and a dialog that closed read as done while that machine billed on.
      if (!answer.removed) setRefusal({ said: answer.note ?? WHERE_WORDS.removeUnmade(placeName(place)) });
      else {
        onRemoved?.();
        change(false);
      }
    } catch (e) {
      holdParting(null);
      const failure = failureOf(e);
      // The host's fix names the terminal and this confirm for a client with no field; here the field is the fix.
      const sudo = failure.kind === PLACE_SUDO_KIND;
      // A cloud that refused its key cannot be read or emptied, so the confirm turns to its Forget, which asks nothing there.
      const refusedKey = failure.kind === "auth" && isProviderPlace(place);
      setAsksSudo(sudo);
      if (refusedKey) setKeyRefused(true);
      setRefusal({ said: failure.said, ...(sudo ? { fix: ADD_COMPUTER_WORDS.sudoFix } : refusedKey ? { fix: WHERE_WORDS.keyRefusedFix(placeName(place)) } : failure.fix === undefined ? {} : { fix: failure.fix }) });
      // The link may have come or gone since the dialog opened, which moves the road the host takes.
      api.placeHolds?.(place.id).then(setHolds, () => undefined);
    } finally {
      setBusy(false);
    }
  };

  return (
    <AlertDialog open={open} onOpenChange={change}>
      <AlertDialogPopup data-remove-place-dialog>
        <AlertDialogHeader>
          <AlertDialogTitle data-k="remove-title">{madeNote !== null ? removedTitle(place, forgets) : forgets ? forgetTitle(place) : removeTitle(place)}</AlertDialogTitle>
          <AlertDialogDescription data-k="remove-sentence">{keyRefused ? providerForgetSentence(place, holding, hereName(places)) : forgets && holds !== null ? forgetSentence(place, holds) : removeSentence(place, { ...holding, projects }, hereName(places), imageBytes)}</AlertDialogDescription>
        </AlertDialogHeader>
        {placeIsOffline(place) ? (
          <div className="flex flex-col gap-3 px-5 pt-2">
            <CopyRow k="leave-line" value={PLACES_WORDS.remove.leaveLine} />
            <p className="text-[13px] text-muted-foreground">{PLACES_WORDS.remove.leaveTakes}</p>
          </div>
        ) : null}
        <div className="flex flex-col gap-2 px-5 pt-2">
          <RefusalSlot
            k="remove-refusal"
            {...(madeNote !== null
              ? { note: <span data-k="remove-made-note">{madeNote}</span> }
              : (refusal ??
                (reading
                  ? { waiting: WHERE_WORDS.readingHolds }
                  : unsaved.length > 0
                    ? { note: <span data-k="unsaved" className={cn("block", !isProviderPlace(place) && "max-h-9 overflow-y-auto")}>{isProviderPlace(place) ? (heldTitles.length > 0 ? WHERE_WORDS.subsHeld(heldTitles) : `${subHeldOnRemoveLine(unsaved) ?? ""} ${WHERE_WORDS.unsavedFixCloud}`) : `${placeUnsavedRefusal(placeName(place), unsaved).said}. ${WHERE_WORDS.unsavedFix}`}</span> }
                    : {})))}
          />
          {asksSudo ? <Input data-k="sudo-password" aria-label="Password for sudo" type="password" autoFocus autoComplete="off" value={password} onChange={e => setPassword(e.target.value)} onKeyDown={e => e.key === "Enter" && password !== "" && void remove()} className={cn(ROW_FIELD, "w-44")} /> : null}
        </div>
        <AlertDialogFooter>
          {madeNote !== null ? (
            <Button data-k="remove-done" variant="outline" className={NEUTRAL_RING} onClick={() => change(false)}>
              {WHERE_WORDS.done}
            </Button>
          ) : (
            <>
              <AlertDialogClose render={<Button variant="outline" className={NEUTRAL_RING} />}>{WHERE_WORDS.cancel}</AlertDialogClose>
              <Button data-k="remove-confirm" variant="destructive" disabled={busy || reading || (asksSudo && password === "")} onClick={() => void remove()}>
                {busy ? (forgets ? WHERE_WORDS.forgetting : WHERE_WORDS.removing) : forgets ? WHERE_WORDS.forget : unsaved.length > 0 ? WHERE_WORDS.removeAnyway : WHERE_WORDS.remove}
              </Button>
            </>
          )}
        </AlertDialogFooter>
      </AlertDialogPopup>
    </AlertDialog>
  );
}
