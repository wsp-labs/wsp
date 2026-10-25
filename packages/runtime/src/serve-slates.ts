// SPDX-License-Identifier: AGPL-3.0-only
// The slate ops a socket asks, answered off the runtime's slates: what the reply carries beside id and ok.
import type { RuntimeRequest } from "@wsp/protocol";
import { isSlateLookRequest, type SlateLookHub, type SlateLookSocket } from "./serve-slate-look.js";
import type { Slates } from "./slates.js";

export { slateLookHubOf } from "./serve-slate-look.js";

/** The window's and the verbs' slate ops, and the app's offer and answer of looks. */
export type SlateRequest = Extract<RuntimeRequest, { op: `slates.${string}` | `slateLook.${string}` }>;

export const isSlateRequest = (msg: RuntimeRequest): msg is SlateRequest => msg.op.startsWith("slates.") || msg.op.startsWith("slateLook.");

/** The road a look takes: the host's hub, and the socket an app offers on. */
export interface SlateLookRoad {
  hub: SlateLookHub;
  send: SlateLookSocket["send"];
  own: boolean;
}

/** The holds one socket took, by thread and sources, so an unsubscribe lets go of one of them; every hold also goes
 * on detaches, which the socket's close runs. */
export interface SlateHolds {
  held: Map<string, (() => void)[]>;
  detaches: (() => void)[];
}

export async function answerSlate(slates: Slates, msg: SlateRequest, origin: Parameters<Slates["write"]>[1], holds: SlateHolds, look: SlateLookRoad): Promise<Record<string, unknown>> {
  if (isSlateLookRequest(msg)) return look.hub.answer(msg, { send: look.send, detaches: holds.detaches, own: look.own });
  const { id: _id, origin: _sent, ...params } = msg;
  switch (params.op) {
    case "slates.get":
      return { slate: await slates.get(params.threadId) };
    case "slates.write": {
      const { op: _op, ...p } = params;
      return { ...(await slates.write(p, origin)) };
    }
    case "slates.state": {
      const { op: _op, ...p } = params;
      return { ...(await slates.state(p, origin)) };
    }
    case "slates.read": {
      const { op: _op, ...p } = params;
      return { ...(await slates.read(p, origin)) };
    }
    case "slates.catalog": {
      const { op: _op, ...p } = params;
      return { ...(await slates.catalog({ ...p, look: look.hub.lookable() }, origin)) };
    }
    case "slates.shown":
      await slates.shown(params.threadId);
      return {};
    case "slates.event": {
      const { op: _op, ...p } = params;
      return { ...(await slates.event(p)) };
    }
    case "slates.approve":
      await slates.approve({ threadId: params.threadId, key: params.key, scope: params.scope });
      return {};
    case "slates.cancel":
      await slates.cancel({ threadId: params.threadId, run: params.run });
      return {};
    case "slates.revoke":
      await slates.revoke({ threadId: params.threadId, key: params.key });
      return {};
    case "slates.subscribe": {
      // Held until the window lets go, or its socket closes and every hold it took goes with it.
      const release = slates.subscribe({ threadId: params.threadId, sources: params.sources });
      const key = `${params.threadId}\u0000${[...params.sources].sort().join(",")}`;
      holds.held.set(key, [...(holds.held.get(key) ?? []), release]);
      holds.detaches.push(release);
      return {};
    }
    case "slates.unsubscribe": {
      const key = `${params.threadId}\u0000${[...params.sources].sort().join(",")}`;
      const held = holds.held.get(key) ?? [];
      held.shift()?.();
      if (held.length === 0) holds.held.delete(key);
      return {};
    }
    case "slates.resolve":
      return { ...(await slates.resolve({ threadId: params.threadId, paths: params.paths })) };
    case "slates.image":
      return { ...(await slates.image({ threadId: params.threadId, src: params.src, ...(params.have !== undefined ? { have: params.have } : {}) })) };
    case "slates.look": {
      const threadId = await slates.lookAt({ ...(params.threadId !== undefined ? { threadId: params.threadId } : {}), ...(params.turnToken !== undefined ? { turnToken: params.turnToken } : {}) }, origin);
      return { ...(await look.hub.look(threadId, params.width)) };
    }
    case "slates.page":
      return { ...(await slates.page({ threadId: params.threadId, src: params.src, ...(params.have !== undefined ? { have: params.have } : {}) })) };
  }
}
