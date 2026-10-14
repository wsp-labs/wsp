// SPDX-License-Identifier: AGPL-3.0-only
// The sidebar's cloud card, in the update card's shape in its foot: an offer
// to run threads on a cloud, shown while no cloud is added or being set up and
// the person has not dismissed it. Dismissed, it is gone from every window,
// since the host keeps the dismissal. The foot holds one card: this one waits
// while the Setting up section or the update card stands.
import { CloudIcon, XIcon } from "lucide-react";
import { PROVIDER_KEY_WORDS, type InitJob, type PlaceView } from "@wsp/protocol";
import { Button } from "../components/ui/button.js";
import { usePlaces, useStore } from "../protocol/store.js";
import { CLOUD_KEY_STEPS, FIRST_CLOUD } from "../settings/add/cloudKey.js";
import { openCloudAdd } from "../settings/add/cloudFlow.js";
import { ADD_COMPUTER_WORDS } from "../settings/format.js";
import { isProviderPlace } from "../settings/places.js";
import { cloudBuilding, settingUp } from "./SettingUpSection.js";
import { useUpdateCardShown } from "./UpdateCard.js";

/** Whether the card stands for these places and this setup: the places read, no cloud among them, and no setup
 * started for one. */
export const offersCloud = (places: readonly PlaceView[], job: InitJob | null): boolean => places.length > 0 && !places.some(isProviderPlace) && job?.place?.id !== FIRST_CLOUD;

export function CloudCard() {
  const places = usePlaces();
  const job = useStore(s => s.initJob);
  const updateShown = useUpdateCardShown();
  // Unknown until the host's record is read, so a dismissed card never shows for the first paint.
  const dismissed = useStore(s => (s.preferencesRead ? s.preferences.cloudCardDismissed === true : null));
  const name = PROVIDER_KEY_WORDS[FIRST_CLOUD]?.name ?? FIRST_CLOUD;
  const rate = CLOUD_KEY_STEPS[FIRST_CLOUD]?.from;
  if (dismissed !== false || updateShown || settingUp(places).length > 0 || cloudBuilding(job, places) !== undefined || !offersCloud(places, job)) return null;
  const dismiss = (): void => void useStore.getState().setPreferences({ cloudCardDismissed: true });
  return (
    <div data-cloud-card role="note" className="flex min-w-0 flex-col gap-1 rounded-[var(--control-radius)] bg-sidebar-row-hover px-2 pt-2 pb-2.5 inset-ring inset-ring-[var(--sidebar-row-edge)]">
      <span className="flex h-5 min-w-0 items-center gap-1.5 text-sm text-sidebar-foreground">
        <CloudIcon aria-hidden className="size-3.5 shrink-0 text-sidebar-muted-foreground" />
        <span data-k="cloud-card-title" className="min-w-0 flex-1 truncate">
          Run threads on a cloud
        </span>
        <Button variant="ghost" size="icon-micro" data-k="cloud-card-dismiss" aria-label="Dismiss" className="-me-1 text-sidebar-muted-foreground" onClick={dismiss}>
          <XIcon aria-hidden />
        </Button>
      </span>
      {rate === undefined ? null : (
        <span data-k="cloud-card-line" className="text-meta leading-3.5 text-sidebar-muted-foreground">
          {name} {rate}.
        </span>
      )}
      <span className="flex items-center gap-2 pt-1.5">
        <Button size="xs" variant="outline" data-k="cloud-card-add" onClick={() => openCloudAdd(FIRST_CLOUD)}>
          {ADD_COMPUTER_WORDS.addCloud}
        </Button>
      </span>
    </div>
  );
}
