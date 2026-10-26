// SPDX-License-Identifier: AGPL-3.0-only
// wsp slate look: the thread's slate drawn by the app on this computer, through its own renderer, so the picture is
// the window's. The app offers on its own socket to draw looks; each look is pushed to the newest offer and waits for
// its answer by id. A host no app on this computer is attached to has nothing to draw with, and says so.
import { randomUUID } from "node:crypto";
import { SlatesLookAnswer, type RuntimeRequest, type SlateLookWantedEvent } from "@wsp/protocol";

export type SlateLookRequest = Extract<RuntimeRequest, { op: `slateLook.${string}` }>;

export const isSlateLookRequest = (msg: RuntimeRequest): msg is SlateLookRequest => msg.op.startsWith("slateLook.");

/** Said where no app offered: only the classic app draws a look, so the GPUI app open here still reads as none. */
export const SLATE_LOOK_NO_APP = "Nothing on this computer can draw the slate for a look: it needs the classic wsp app open here; open it and look again.";
export const SLATE_LOOK_OFF_ROAD = "only the wsp app on this computer draws slates, over the host's own road";
/** How long a look waits for the app: a first draw loads the window's code and every image and frame on the slate. */
export const SLATE_LOOK_WAIT_MS = 30_000;
export const slateLookLate = (seconds: number): string => `The app did not draw the slate within ${seconds} s; look again once the app has finished starting.`;

export interface SlateLookSocket {
  send(frame: SlateLookWantedEvent): void;
  detaches: (() => void)[];
  own: boolean;
}

type Waiting = { done(answer: SlatesLookAnswer): void; failed(why: string): void; by: SlateLookSocket["detaches"] };

export interface SlateLookHub {
  answer(msg: SlateLookRequest, socket: SlateLookSocket): Record<string, unknown>;
  /** Whether an app on this computer has offered to draw looks, so slate_look and the look rule are offered. */
  lookable(): boolean;
  /** Draws the thread's slate on the newest app that offered, at width where one is asked for. */
  look(threadId: string, width?: number): Promise<SlatesLookAnswer>;
}

export function slateLookHub(waitMs = SLATE_LOOK_WAIT_MS): SlateLookHub {
  /** The sockets that offered, newest last; a look goes to the newest, the window opened most recently. */
  const offers: SlateLookSocket[] = [];
  const waiting = new Map<string, Waiting>();
  return {
    answer(msg, socket) {
      switch (msg.op) {
        case "slateLook.offer": {
          if (!socket.own) throw new Error(SLATE_LOOK_OFF_ROAD);
          if (!offers.includes(socket)) {
            offers.push(socket);
            socket.detaches.push(() => {
              offers.splice(offers.indexOf(socket), 1);
              for (const [id, w] of waiting) if (w.by === socket.detaches) (waiting.delete(id), w.failed(SLATE_LOOK_NO_APP));
            });
          }
          return {};
        }
        case "slateLook.answer": {
          const w = waiting.get(msg.lookId);
          if (w === undefined || w.by !== socket.detaches) return {};
          waiting.delete(msg.lookId);
          if (msg.look !== undefined) w.done(msg.look);
          else w.failed(msg.problem ?? "The app could not draw the slate.");
          return {};
        }
      }
    },
    lookable: () => offers.length > 0,
    look(threadId, width) {
      const app = offers.at(-1);
      if (app === undefined) return Promise.reject(new Error(SLATE_LOOK_NO_APP));
      const lookId = randomUUID();
      return new Promise<SlatesLookAnswer>((resolve, reject) => {
        const timer = setTimeout(() => {
          waiting.delete(lookId);
          reject(new Error(slateLookLate(Math.round(waitMs / 1000))));
        }, waitMs);
        timer.unref?.();
        waiting.set(lookId, {
          by: app.detaches,
          done: answer => (clearTimeout(timer), resolve(answer)),
          failed: why => (clearTimeout(timer), reject(new Error(why))),
        });
        app.send({ type: "slate.look.wanted", lookId, threadId, ...(width !== undefined ? { width } : {}) });
      });
    },
  };
}

const hubs = new WeakMap<object, SlateLookHub>();

/** The one hub of a host, by its runtime's slates, which the launch and the socket both hold. */
export function slateLookHubOf(owner: object): SlateLookHub {
  let hub = hubs.get(owner);
  if (hub === undefined) {
    hub = slateLookHub();
    hubs.set(owner, hub);
  }
  return hub;
}
