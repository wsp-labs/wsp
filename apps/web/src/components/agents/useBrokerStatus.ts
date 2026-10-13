// SPDX-License-Identifier: AGPL-3.0-only
// The kind of credential this host adds for machines at a provider, read when
// this computer's Agents rows show and a provider is set up, and again when
// the host says the agents here changed. A host whose broker is not wired
// refuses the read, and the rows then say what they said before it.
import { useCallback, useEffect, useState } from "react";
import { HERE_PLACE_ID, isProviderPlace, type AgentsTarget, type BrokerStatus } from "@wsp/protocol";
import { useHeld } from "../../protocol/held.js";
import { useStore } from "../../protocol/store.js";
import { FRESH_MS } from "./useAgentsReport.js";

const KEY = "broker:status";

/** The broker's answer for the rows of this computer while a provider is set up; undefined anywhere else, before the
 * first answer and from a host that refused it. */
export function useBrokerStatus(target: AgentsTarget): BrokerStatus | undefined {
  const api = useStore(s => s.api);
  const clouds = useStore(s => s.places.some(isProviderPlace));
  const asks = clouds && "placeId" in target && target.placeId === HERE_PLACE_ID && api?.brokerStatus !== undefined;
  const [asked, setAsked] = useState(0);
  const held = useHeld<BrokerStatus>(asks ? KEY : null, api?.brokerStatus, { holdMs: FRESH_MS, asked });
  const refresh = useCallback(() => setAsked(n => n + 1), []);
  // A sign-in or a key here changes what the broker adds, so it is read again.
  useEffect(() => {
    if (!asks || api?.subscribe === undefined) return;
    return api.subscribe(event => {
      if (event.type === "agents.changed") refresh();
    });
  }, [api, asks, refresh]);
  return asks && held.error === undefined ? held.value : undefined;
}
