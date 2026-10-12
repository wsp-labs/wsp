// SPDX-License-Identifier: AGPL-3.0-only
// A kept server's commands still going, by item id, across its turns: codex-cli 0.155.1 completes a background
// command's item only once it exits, and ends every one when it unloads the thread, 60 s after the last subscriber
// lets go (measured 2026-10-12).
import type { ExecStream } from "@wsp/protocol";
import { readMessage, threadUnsubscribeLine } from "./rpc.js";

export interface Background {
  commands: Set<string>;
  /** The thread a turn's end kept subscribed for the commands, let go once the last one ends. */
  held: string | undefined;
}

/** A line the server printed between turns, where a command a turn left going may end. */
export function endedBetween(run: ExecStream, background: Background, raw: string): void {
  const message = readMessage(raw);
  if (message?.kind !== "notification" || message.method !== "item/completed") return;
  const item = message.params.item as { type?: unknown; id?: unknown } | undefined;
  if (item?.type !== "commandExecution" || typeof item.id !== "string") return;
  background.commands.delete(item.id);
  if (background.commands.size > 0 || background.held === undefined) return;
  void run.write(threadUnsubscribeLine(background.held)).catch(() => {});
  background.held = undefined;
}
