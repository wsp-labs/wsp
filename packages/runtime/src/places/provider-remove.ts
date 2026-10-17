// SPDX-License-Identifier: AGPL-3.0-only
import { PROVIDER_LEFT_WORDS, providerImagesKeptLine, providerKeptLine, providerKeyName, providerKeyRefused, providerUnlistedLine, refusal, type PlaceRemoved } from "@wsp/protocol";
import { MachineAliveError, OWNER_LABEL, isMissing, killUntilGone, sightMachine, dropTemplate, snapshotUntilGone, type KillConfirm, type Machine, type MachineBackend } from "@wsp/engine";
import type { HeldImage } from "./types.js";

/** How many deletes run at once: each is one ask and a dozen reads, and a provider rate-limits the account as a whole. */
export const KILLS_AT_ONCE = 4;

/** How many times the listing is read again after the deletes, for machines a create in flight made meanwhile. */
export const RELISTS = 3;

const messageOf = (e: unknown): string => (e instanceof Error ? e.message : String(e));

/** Takes a provider back out: every machine this host made there, by its owner mark and by its own records, is
 * deleted and read back gone, then every image snapshot its records hold there, and only then is the key forgotten.
 * One the provider will not delete is named and keeps the key, since what is left there bills and only that key
 * reaches it. */
export async function removeProvider(o: {
  placeId: string;
  backend: MachineBackend;
  owner: string;
  known: readonly string[];
  forgetKey?: (placeId: string) => Promise<string | undefined>;
  confirm?: KillConfirm;
  images?: { held(): Promise<readonly HeldImage[]>; drop(id: string): Promise<void> };
}): Promise<PlaceRemoved> {
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
        // The kill both deletes and reads back, so a failure is told apart by which of the two raised it.
        const machine = seen.machine;
        let deleteFailed = false;
        const asked: Machine = Object.create(machine);
        asked.kill = () =>
          machine.kill().catch((e: unknown) => {
            deleteFailed = !isMissing(e);
            throw e;
          });
        try {
          await killUntilGone(o.backend, asked, o.confirm);
          deleted++;
        } catch (e) {
          left.push({ id, said: e instanceof MachineAliveError ? PROVIDER_LEFT_WORDS.running(e.state) : deleteFailed ? PROVIDER_LEFT_WORDS.undeleted(messageOf(e)) : PROVIDER_LEFT_WORDS.unread(messageOf(e)) });
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
    let listed: string[];
    try {
      listed = await listMarked();
    } catch (e) {
      if (left.length > 0) break;
      return { removed: false, swept: [], deleted, note: providerUnlistedLine(name, deleted, messageOf(e)) };
    }
    ids = listed.filter(id => !handled.has(id));
  }
  if (left.length > 0) return { removed: false, swept: [], deleted, note: providerKeptLine(name, deleted, left) };
  const images = await deleteImages(o.backend, o.images, o.confirm);
  const snapshots = images.deleted === 0 ? {} : { snapshots: images.deleted };
  if (images.left.length > 0) return { removed: false, swept: [], deleted, ...snapshots, note: providerImagesKeptLine(name, deleted, images.left) };
  const still = await o.forgetKey?.(o.placeId);
  return { removed: true, swept: [], deleted, ...snapshots, ...(still === undefined ? {} : { note: still }) };
}

/** Deletes each image snapshot and reads it gone, its record dropped as it goes; answers the ones still there. */
async function deleteImages(backend: MachineBackend, images: { held(): Promise<readonly HeldImage[]>; drop(id: string): Promise<void> } | undefined, confirm: KillConfirm | undefined): Promise<{ left: { id: string; said: string }[]; deleted: number }> {
  const left: { id: string; said: string }[] = [];
  let deleted = 0;
  for (const image of (await images?.held()) ?? []) {
    try {
      await dropTemplate(backend, image.templateId);
      const read = await snapshotUntilGone(backend, image.id, confirm);
      if (read.verdict === "listed") {
        left.push({ id: image.id, said: PROVIDER_LEFT_WORDS.listed(read.graceMs) });
        continue;
      }
      await images!.drop(image.id);
      deleted++;
    } catch (e) {
      left.push({ id: image.id, said: PROVIDER_LEFT_WORDS.undeleted(messageOf(e)) });
    }
  }
  return { left, deleted };
}
