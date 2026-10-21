// SPDX-License-Identifier: AGPL-3.0-only
// The sidebar's two picks, the project and the computer its list is filtered
// to: where this window keeps them, and what the list holds under them, and
// the sections it folds. The sidebar draws that list and the jumps along it
// walk it, so both read it here and neither reaches a thread the picks or a
// fold hide.
import type { PlaceView, ProjectView } from "@wsp/protocol";
import type { SidebarProjectSnapshot } from "../adapt/index.js";
import { getLocalStorageItem, type Codec } from "../hooks/useLocalStorage.js";
import { workspacesOn } from "./computerPick.js";
import { projectGroups, type ProjectGroup, type SidebarSection } from "./threadTree.js";

/** The project the list is filtered to. A view of this window alone, so it never follows a person to another one. */
export const PROJECT_PICK_KEY = "wsp:sidebar-project";
/** The computer the list is filtered to, of this window alone as the project pick is. */
export const COMPUTER_PICK_KEY = "wsp:sidebar-computer";

export const pickCodec: Codec<string | null> = {
  decode: raw => {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "string") throw new Error(`Expected an id, got ${raw}.`);
    return parsed;
  },
  encode: value => JSON.stringify(value),
};

export type Fold = SidebarSection | "settled" | "setting-up";
/** The folds a head can make: the one list has no head, so it never folds. */
const FOLDS: readonly Fold[] = ["pinned", "needs-you", "working", "settled", "setting-up"];
/** The sections a person has folded by their heads. Settled starts folded, since it holds the tiles a person has
 * stopped looking at; every live section starts open. */
export const FOLDED_KEY = "wsp:sidebar-folded";
export const DEFAULT_FOLDS: readonly Fold[] = ["settled"];
export const foldedCodec: Codec<readonly Fold[]> = {
  // A section the sidebar no longer has is dropped, not a refusal of the list: a refusal pins every fold at its
  // default and no click can store another (the cut to three sections stranded Settled folded on 2026-10-01).
  decode: raw => {
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) throw new Error(`Expected a list of section ids, got ${raw}.`);
    return parsed.filter((id): id is Fold => (FOLDS as readonly unknown[]).includes(id));
  },
  encode: value => JSON.stringify(value),
};

/** The sections folded as this window stored them, read outside the sidebar. */
export const storedFolds = (): readonly Fold[] => getLocalStorageItem(FOLDED_KEY, foldedCodec) ?? DEFAULT_FOLDS;

/** The picks as this window stored them, read outside the sidebar. */
export const storedPicks = (): { project: string | null; computer: string | null } => ({
  project: getLocalStorageItem(PROJECT_PICK_KEY, pickCodec),
  computer: getLocalStorageItem(COMPUTER_PICK_KEY, pickCodec),
});

/** The workspaces and projects the list holds under the stored picks, and the picks that hold: a pick for a computer
 * or a project this host no longer holds reads as every one. */
export function underPicks(
  fleet: SidebarProjectSnapshot[],
  { places, recorded, order, stored }: { places: readonly PlaceView[]; recorded: ReadonlyArray<ProjectView>; order: ReadonlyArray<string>; stored: { project: string | null; computer: string | null } },
): { projects: SidebarProjectSnapshot[]; groups: ProjectGroup[]; computer: string | null; picked: ProjectGroup | null } {
  const computer = places.some(place => place.id === stored.computer) ? stored.computer : null;
  const projects = workspacesOn(fleet, places, computer);
  const groups = projectGroups(recorded, projects, order);
  const picked = stored.project === null ? null : (groups.find(group => group.project.id === stored.project) ?? null);
  return { projects, groups, computer, picked };
}
