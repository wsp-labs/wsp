// SPDX-License-Identifier: AGPL-3.0-only
import { z } from "zod";

/** What a place is: a computer, this one or one somebody joined, or a provider's cloud. Its own module, imported by
 * the setup frames and the views alike, so neither reaches the other for it. */
export const PlaceKind = z.enum(["computer", "provider"]);
export type PlaceKind = z.infer<typeof PlaceKind>;
