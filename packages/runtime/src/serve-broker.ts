// SPDX-License-Identifier: AGPL-3.0-only
// The broker ops a socket asks, answered off the host's broker: what the reply carries beside id and ok.
import { PLACES_TICKET_REFUSAL, type BrokerStatus, type ProviderKeyWorks, type RuntimeRequest } from "@wsp/protocol";
import type { CloudSignIn } from "./sign-in-socket.js";

/** What a host with no broker answers the broker ops with. */
export const BROKER_UNSERVED = "the host that answered wired no broker for machines at a cloud provider";

/** The host's broker as its socket answers it: the kind of credential it adds for each road (broker.status), a
 * provider key put to the provider (providers.checkKey), and an agent's own sign-in on this computer that machines at
 * a provider borrow (agents.cloudSignIn, which the sign-in ops answer). Each one absent is refused. */
export interface BrokerDoor {
  status?(): Promise<BrokerStatus>;
  checkKey?(provider: string, key: string): Promise<ProviderKeyWorks>;
  cloudSignIn?: CloudSignIn;
}

export type BrokerOp = Extract<RuntimeRequest, { op: "providers.checkKey" | "broker.status" }>;

export const isBrokerOp = (msg: RuntimeRequest): msg is BrokerOp => msg.op === "providers.checkKey" || msg.op === "broker.status";

export async function answerBroker(door: BrokerDoor | undefined, msg: BrokerOp, ownRoad: boolean): Promise<Record<string, unknown>> {
  if (msg.op === "broker.status") {
    if (door?.status === undefined) throw new Error(BROKER_UNSERVED);
    return { ...(await door.status()) };
  }
  // A provider key crosses this socket, as a key into the vault does: only the host's own road sends one.
  if (!ownRoad) throw Object.assign(new Error(PLACES_TICKET_REFUSAL), { kind: "ticket" });
  if (door?.checkKey === undefined) throw new Error(BROKER_UNSERVED);
  return { ...(await door.checkKey(msg.provider, msg.key)) };
}
