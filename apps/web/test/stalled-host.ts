// SPDX-License-Identifier: AGPL-3.0-only
// A host whose loop stood still while a page authed, as a host with its swap near full does.

/** A browser WebSocket after whose first auth frame this whole process stands still for `ms`, as a host whose loop
 * stalled with that frame already on its socket does; every socket it opens leaves its close code in `closes`. */
export function stallingAfterFirstAuth(ms: number, closes: number[] = []): typeof WebSocket {
  let stalled = false;
  return class extends WebSocket {
    constructor(url: string | URL, protocols?: string | string[]) {
      super(url, protocols);
      this.addEventListener("close", e => closes.push(e.code));
    }
    override send(data: Parameters<WebSocket["send"]>[0]): void {
      super.send(data);
      if (stalled || typeof data !== "string" || !data.includes('"op":"auth"')) return;
      stalled = true;
      const until = Date.now() + ms;
      while (Date.now() < until);
    }
  };
}
