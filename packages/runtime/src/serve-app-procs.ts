// SPDX-License-Identifier: AGPL-3.0-only
// The desktop app's processes, named by the app and held here for the pages
// that list this computer's processes. The app's socket offers them and is
// told when the first page starts watching and when the last one stops, so
// Electron is read only while somebody looks; each list it sends goes to every
// socket that watches, and goes with the socket that sent it.
import type { AppProc, AppProcsEvent, AppProcsWantedEvent, RuntimeRequest } from "@wsp/protocol";

export type AppProcsRequest = Extract<RuntimeRequest, { op: `appProcs.${string}` }>;

export const isAppProcsRequest = (msg: RuntimeRequest): msg is AppProcsRequest => msg.op.startsWith("appProcs.");

export const APP_PROCS_OFF_ROAD = "only the wsp app on this computer names its processes, over the host's own road";
export const APP_PROCS_WATCH_OFF_ROAD = "the app's processes name the sites in the person's browser tabs, so only the host's own road reads them";
export const APP_PROCS_UNOFFERED = "appProcs.set comes after appProcs.offer on the same socket";

/** One socket as the hub needs it: what pushes to it, and what its close runs, which is also how the hub knows it
 * again, since a frame's own send is made for that frame. */
export interface AppProcsSocket {
  send(frame: AppProcsEvent | AppProcsWantedEvent): void;
  detaches: (() => void)[];
  /** Whether the socket came in on the host's own road rather than through the relay. */
  own: boolean;
}

type Push = AppProcsSocket["send"];
type Key = AppProcsSocket["detaches"];

export interface AppProcsHub {
  answer(msg: AppProcsRequest, socket: AppProcsSocket): Record<string, unknown>;
}

export function appProcsHub(): AppProcsHub {
  const watchers = new Map<Key, Push>();
  /** Each offering socket's push and newest list; two apps on one host each name their own. */
  const offers = new Map<Key, { push: Push; procs: AppProc[] }>();
  /** The sockets whose close already lets go of both, so a pane opened and shut again adds nothing to keep. */
  const joined = new WeakSet<Key>();
  const all = (): AppProc[] => [...offers.values()].flatMap(o => o.procs);
  const turned = (wanted: boolean): void => {
    for (const offer of offers.values()) {
      // A list read for pages that have all gone is stale before the next one watches.
      if (!wanted) offer.procs = [];
      offer.push({ type: "app.procs.wanted", wanted });
    }
  };
  const unwatch = (key: Key): void => {
    if (watchers.delete(key) && watchers.size === 0) turned(false);
  };
  const join = (key: Key): void => {
    if (joined.has(key)) return;
    joined.add(key);
    key.push(() => {
      unwatch(key);
      if (offers.delete(key)) for (const push of watchers.values()) push({ type: "app.procs", procs: all() });
    });
  };
  return {
    answer(msg, { send, detaches: key, own }) {
      switch (msg.op) {
        case "appProcs.watch": {
          if (!own) throw new Error(APP_PROCS_WATCH_OFF_ROAD);
          join(key);
          const first = watchers.size === 0;
          watchers.set(key, send);
          if (first) turned(true);
          return { procs: all() };
        }
        case "appProcs.unwatch":
          unwatch(key);
          return {};
        case "appProcs.offer": {
          if (!own) throw new Error(APP_PROCS_OFF_ROAD);
          join(key);
          offers.set(key, { push: send, procs: offers.get(key)?.procs ?? [] });
          return { wanted: watchers.size > 0 };
        }
        case "appProcs.set": {
          const offer = offers.get(key);
          if (offer === undefined) throw new Error(APP_PROCS_UNOFFERED);
          if (watchers.size === 0) return {};
          offer.procs = msg.procs;
          const procs = all();
          for (const push of watchers.values()) push({ type: "app.procs", procs });
          return {};
        }
      }
    },
  };
}

const hubs = new WeakMap<object, AppProcsHub>();

/** The one hub of a host, by the runtime it serves. */
export function appProcsHubOf(owner: object): AppProcsHub {
  let hub = hubs.get(owner);
  if (hub === undefined) {
    hub = appProcsHub();
    hubs.set(owner, hub);
  }
  return hub;
}
