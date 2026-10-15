// SPDX-License-Identifier: AGPL-3.0-only
import { CalendarX2Icon } from "lucide-react";
import { DISK_EXPIRED_WORD, diskExpired } from "@wsp/protocol";
import type { StatusKind } from "./kind.js";

/** A thread whose napping machine is past the days its cloud keeps a paused disk: a send may start a new machine, so
 * it reads before a stop, and a finish nobody has read still reads Done first. */
export const EXPIRED: StatusKind = {
  id: "expired",
  is: thread => diskExpired(thread, Date.now()) && !thread.unread,
  glyph: CalendarX2Icon,
  word: DISK_EXPIRED_WORD,
  aged: true,
};
