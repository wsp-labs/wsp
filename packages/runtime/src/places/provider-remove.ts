// SPDX-License-Identifier: AGPL-3.0-only
import { PROVIDER_LEFT_WORDS, providerKeptLine, providerKeyName, providerKeyRefused, refusal, type PlaceRemoved } from "@wsp/protocol";
import { MachineAliveError, OWNER_LABEL, killUntilGone, sightMachine, type KillConfirm, type MachineBackend } from "@wsp/engine";

/** How many deletes run at once: each is one ask and a dozen reads, and a provider rate-limits the account as a whole. */
export const KILLS_AT_ONCE = 4;

/** How many times the listing is read again after the deletes, for machines a create in flight made meanwhile. */
export const RELISTS = 3;

const messageOf = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** Takes a provider back out: every machine this host made there, by its owner mark and by its own records, is
 * deleted and read back gone, and only then is the key forgotten. One the provider will not delete is named and
 * keeps the key, since a machine left there bills and only that key reaches it. */
export async function removeProvider(o: { placeId: string; backend: MachineBackend; owner: string; known: readonly string[]; forgetKey?: (placeId: string) => Promise<void>; confirm?: KillConfirm }): Promise<PlaceRemoved> {
  const name = providerKeyName(o.placeId);
  // A backend's list may ignore the labels it is handed, so the mark is read again on every row.
  const listMarked = async (): Promise<string[]> => {
    const rows = await o.backend.list({ [OWNER_LABEL]: o.owner }).catch((e: unknown) => {
      if ((e as { kind?: unknown }).kind !== "auth") throw e;
      const refused = providerKeyRefused(name, messageOf(e));
      throw refusal(refused.said, refused.fix, "auth");
    });
    return rows.filter(m => m.labels[OWNER_LABEL] === o.owner).map(m => m.id);
  };
  const left: { id: string; said: string }[] = [];
  const handled = new Set<string>();
  let deleted = 0;
  const deleteEach = async (ids: readonly string[]): Promise<void> => {
    const queue = [...ids];
    const worker = async (): Promise<void> => {
      for (let id = queue.shift(); id !== undefined; id = queue.shift()) {
        // A first 404 can come from a copy that never held the machine while another keeps it running, so only a run
        // of gone reads skips it.
        const seen = await sightMachine(o.backend, id).catch((e: unknown) => ({ unread: messageOf(e) }));
        if ("unread" in seen) {
          left.push({ id, said: PROVIDER_LEFT_WORDS.unread(seen.unread) });
          continue;
        }
        if (seen.machine === undefined) continue;
        try {
          await killUntilGone(o.backend, seen.machine, o.confirm);
          deleted++;
        } catch (e) {
          left.push({ id, said: e instanceof MachineAliveError ? PROVIDER_LEFT_WORDS.running(e.state) : messageOf(e) });
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(KILLS_AT_ONCE, queue.length) }, worker));
  };
  // A listing can miss a row, so the records' own ids are asked for as well; and a create in flight can land after
  // it, so it is read again until it shows nothing new, a bounded number of times.
  let ids = [...new Set([...(await listMarked()), ...o.known])];
  for (let pass = 0; ids.length > 0; pass++) {
    if (pass > RELISTS) {
      left.push(...ids.map(id => ({ id, said: PROVIDER_LEFT_WORDS.late })));
      break;
    }
    for (const id of ids) handled.add(id);
    await deleteEach(ids);
    ids = (await listMarked()).filter(id => !handled.has(id));
  }
  if (left.length > 0) return { removed: false, swept: [], deleted, note: providerKeptLine(name, deleted, left) };
  await o.forgetKey?.(o.placeId);
  return { removed: true, swept: [], deleted };
}
