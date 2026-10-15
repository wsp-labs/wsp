// SPDX-License-Identifier: AGPL-3.0-only
import { CloudOffIcon } from "lucide-react";
import type { StatusKind } from "./kind.js";

/** A sub-thread whose machine was deleted once it was done: nothing is asked of the person, so it keeps the row's
 * muted ink, and a finish nobody has read still reads Done first. */
export const RELEASED: StatusKind = {
  id: "released",
  is: thread => thread.sub?.state === "released" && (thread.status === "completed" || thread.status === "interrupted") && !thread.unread,
  glyph: CloudOffIcon,
  word: "Released",
  aged: true,
};
