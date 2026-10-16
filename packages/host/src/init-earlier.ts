// SPDX-License-Identifier: AGPL-3.0-only
import type { Manifest } from "@wsp/collect";
import type { RecipeDigest } from "@wsp/engine";
import type { GoldenBuilderView } from "@wsp/runtime";
import { recipeChanges } from "./init-recipe.js";

/** What an earlier run left on the account for this recipe. */
export interface Earlier {
  /** The builder of this setup carrying this recipe that can still be sealed, when there is one. */
  attach?: GoldenBuilderView;
  /** This setup's builders that cannot be attached to; the boot question offers to stop them first. */
  stop: GoldenBuilderView[];
  /** Where this run's builder is made, the one place a builder is attached to. */
  at: { place: string; name: string };
}

/** Why an earlier builder is not the one this run attaches to, as its line in the list says it. */
export function earlierReason(b: GoldenBuilderView, at: Earlier["at"], recipe: RecipeDigest | undefined, manifest: Manifest): string {
  if (b.foreignOwner !== undefined) return "not this setup's builder";
  if (b.heldBy !== undefined) return `in use by another wsp process (pid ${b.heldBy.pid})`;
  if (b.building === true) return "its setup never finished";
  if (b.sealable !== true) return "cannot be sealed after a restart";
  if (b.placeId !== at.place) return `at ${b.placeName}, and this run builds at ${at.name}`;
  const changes = b.recipe !== undefined && recipe !== undefined ? recipeChanges(b.recipe, recipe, manifest) : [];
  if (changes.length === 0) return "built from a different recipe";
  const named = changes.slice(0, 4);
  if (changes.length > named.length) named.push(`${changes.length - named.length} more`);
  return `built from a different recipe: ${named.join(", ")}`;
}
