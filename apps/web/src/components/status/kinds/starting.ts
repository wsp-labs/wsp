// SPDX-License-Identifier: AGPL-3.0-only
// A workspace being made, before it holds a thread to read a status off, and a
// sub-thread whose machine its cloud is making: in Working's tone, with its crab.
import type { StatusKind } from "./kind.js";

export const STARTING: StatusKind = {
  id: "starting",
  is: thread => thread.status === "running" && thread.sub?.state === "starting",
  tone: "working",
  ink: "text-status-working",
  word: "Starting",
  crab: true,
};
