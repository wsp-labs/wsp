// SPDX-License-Identifier: AGPL-3.0-only
import { UploadIcon } from "lucide-react";
import type { StatusKind } from "./kind.js";

/** A sub-thread done with work its last read found not pushed: its machine stays until the work is pushed, so it
 * calls for the person in Needs you's ink, read before Done so the held work is never a plain finish. */
export const NOT_PUSHED: StatusKind = {
  id: "not-pushed",
  is: thread => thread.status !== "running" && thread.sub?.held !== undefined,
  tone: "input",
  ink: "text-status-input",
  glyph: UploadIcon,
  word: "Not pushed",
};
