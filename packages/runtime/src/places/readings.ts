// SPDX-License-Identifier: AGPL-3.0-only
// A computer's live readings over the one link it holds to this host. Its
// daemon keeps one watch per socket and every reader on this host rides that
// socket: a computer's row on the list, a folder's panes there, a channel. So
// the watch is counted here, asked on the first reader and stopped with
// sys.unwatch after the last, and asked again on the link that replaces one
// that dropped while somebody was still reading.
import { noSuchPlaceRefusal, type DaemonEvent, type SysSample } from "@wsp/protocol";
import type { DaemonReach } from "../reach.js";
import type { PlaceDoorContext } from "./context.js";

type Reader = (sample: SysSample) => void;

interface Watched {
  readers: Set<Reader>;
  /** The link the watch was asked on, and its answer; a link that replaced it has no watch until it is asked. */
  asked?: { on: DaemonReach; answer: Promise<Record<string, unknown>> };
}

export interface PlaceReadings {
  /** Hands `fn` every sample the computer pushes until the returned release runs. Held while the computer is away,
   * so its readers carry on when it dials back. Rejects with the daemon's refusal of the watch. */
  watch(placeId: string, fn: Reader): Promise<() => void>;
  /** A link that just attached: asked for the readings where somebody is still reading them. */
  linked(placeId: string): void;
  heard(placeId: string, event: DaemonEvent): void;
}

export function placeReadings(ctx: PlaceDoorContext): PlaceReadings {
  const watched = new Map<string, Watched>();

  const ask = (w: Watched, on: DaemonReach): Promise<Record<string, unknown>> => {
    const answer: Promise<Record<string, unknown>> = on
      .request("sys.watch")
      .catch((e: unknown) => ({ ok: false, error: e instanceof Error ? e.message : String(e) }))
      .then(said => {
        // A watch the daemon refused is not kept: the next reader asks again rather than inheriting the refusal.
        if (said["ok"] !== true && w.asked?.answer === answer) w.asked = undefined;
        return said;
      });
    w.asked = { on, answer };
    return answer;
  };

  return {
    async watch(placeId, fn) {
      if (!ctx.kept.has(placeId)) throw new Error(noSuchPlaceRefusal(placeId, [...ctx.kept.values()].map(r => r.name)));
      const w = watched.get(placeId) ?? { readers: new Set<Reader>() };
      watched.set(placeId, w);
      w.readers.add(fn);
      const release = (): void => {
        if (!w.readers.delete(fn) || w.readers.size > 0 || watched.get(placeId) !== w) return;
        watched.delete(placeId);
        const on = w.asked?.on;
        if (on !== undefined && ctx.live.get(placeId)?.reach === on) void on.request("sys.unwatch").catch(() => undefined);
      };
      const on = ctx.live.get(placeId)?.reach;
      if (on === undefined) return release;
      const said = await (w.asked?.on === on ? w.asked.answer : ask(w, on));
      if (said["ok"] !== true) {
        release();
        throw new Error(typeof said["error"] === "string" ? said["error"] : "the computer would not send its readings");
      }
      return release;
    },

    linked(placeId) {
      const w = watched.get(placeId);
      const on = ctx.live.get(placeId)?.reach;
      if (w === undefined || on === undefined || w.asked?.on === on) return;
      void ask(w, on);
    },

    heard(placeId, event) {
      if (event.type !== "sys.sample") return;
      for (const fn of [...(watched.get(placeId)?.readers ?? [])]) fn(event);
    },
  };
}
