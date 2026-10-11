// SPDX-License-Identifier: AGPL-3.0-only
import { providerKeptLine, providerKeyName, type PlaceRemoved } from "@wsp/protocol";
import { MachineAliveError, OWNER_LABEL, isMissing, killUntilGone, type KillConfirm, type MachineBackend } from "@wsp/engine";

/** How many deletes run at once: each is one ask and a dozen reads, and a provider rate-limits the account as a whole. */
export const KILLS_AT_ONCE = 4;

/** Takes a provider back out: every machine this host made there, by its owner mark and by its own records, is
 * deleted and read back gone, and only then is the key forgotten. One the provider will not delete is named and
 * keeps the key, since a machine left there bills and only that key reaches it. */
export async function removeProvider(o: { placeId: string; backend: MachineBackend; owner: string; known: readonly string[]; forgetKey?: (placeId: string) => Promise<void>; confirm?: KillConfirm }): Promise<PlaceRemoved> {
  // A backend's list may ignore the labels it is handed, so the mark is read again on every row; and a listing can
  // miss a row, so the records' own ids are asked for by get as well.
  const marked = (await o.backend.list({ [OWNER_LABEL]: o.owner })).filter(m => m.labels[OWNER_LABEL] === o.owner);
  const left: { id: string; said: string }[] = [];
  let deleted = 0;
  const queue = [...new Set([...marked.map(m => m.id), ...o.known])];
  const worker = async (): Promise<void> => {
    for (let id = queue.shift(); id !== undefined; id = queue.shift()) {
      try {
        const machine = await o.backend.get(id).catch((e: unknown) => (isMissing(e) ? undefined : Promise.reject(e)));
        if (machine === undefined) continue;
        await killUntilGone(o.backend, machine, o.confirm);
        deleted++;
      } catch (e) {
        left.push({ id, said: e instanceof MachineAliveError ? `still ${e.state} after three asks` : e instanceof Error ? e.message : String(e) });
      }
    }
  };
  await Promise.all(Array.from({ length: Math.min(KILLS_AT_ONCE, queue.length) }, worker));
  if (left.length > 0) return { removed: false, swept: [], deleted, note: providerKeptLine(providerKeyName(o.placeId), deleted, left) };
  await o.forgetKey?.(o.placeId);
  return { removed: true, swept: [], deleted };
}
