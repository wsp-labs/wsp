// SPDX-License-Identifier: AGPL-3.0-only
// One socket's live readings: a workspace's, or a computer's off the one watch
// the host holds of it. One subscription per workspace or computer per socket,
// so a pane opened twice reads the same stream; the socket closing stops every
// one it holds.
import { SYS_SUBSCRIBE_ONE_OF, type Caller, type RuntimeRequest } from "@wsp/protocol";
import type { Runtime } from "./runtime.js";

type Send = (payload: Record<string, unknown>) => void;
type SysOp = Extract<RuntimeRequest, { op: "sys.subscribe" | "sys.unsubscribe" }>;

/** Where a subscription is filed, a workspace's apart from a computer's. */
const keyOf = (ask: SysOp): string => (ask.placeId !== undefined ? `place:${ask.placeId}` : `workspace:${ask.workspaceId!}`);

export function sysSocket(rt: Pick<Runtime, "workspaces" | "watchPlaceSys">, socket: { open(): boolean; detach(leave: () => void): void }): (msg: SysOp, send: Send, origin: Caller | undefined) => Promise<void> {
  const watched = new Map<string, () => void>();
  socket.detach(() => {
    for (const detach of watched.values()) detach();
    watched.clear();
  });
  return async (msg, send, origin) => {
    if ((msg.workspaceId === undefined) === (msg.placeId === undefined)) throw new Error(SYS_SUBSCRIBE_ONE_OF);
    const key = keyOf(msg);
    if (msg.op === "sys.unsubscribe") {
      watched.get(key)?.();
      watched.delete(key);
      send({ id: msg.id, ok: true });
      return;
    }
    if (watched.has(key)) {
      send({ id: msg.id, ok: true });
      return;
    }
    const { workspaceId, placeId } = msg;
    let stopped = false;
    const pending = (): void => void (stopped = true);
    const leave = (): void => void (watched.get(key) === pending && watched.delete(key));
    watched.set(key, pending);
    const watching =
      placeId !== undefined
        ? rt.watchPlaceSys(placeId, sample => send({ type: "place.sys", placeId, sample }))
        : rt.workspaces.watchSys(workspaceId!, sample => send({ type: "workspace.sys", workspaceId: workspaceId!, sample }), origin);
    const detach = await watching.catch((e: unknown) => {
      leave();
      throw e;
    });
    // The page left, or stopped watching, while the first reading was in flight: nothing keeps sampling for it.
    if (stopped || !socket.open()) {
      detach();
      leave();
      send({ id: msg.id, ok: true });
      return;
    }
    watched.set(key, detach);
    send({ id: msg.id, ok: true });
  };
}
