// SPDX-License-Identifier: AGPL-3.0-only
import type { GoldenManifest } from "@wsp/engine";
import type { RuntimeContext } from "../context.js";
import type { HeldImage } from "../places/types.js";
import { GOLDENS, PROJECT_GOLDENS, copyKeyParts } from "../types/internal.js";

/** The image snapshots this host's records say it saved at one place, and the drop of one's record once the provider
 * deleted it: what a provider's remove takes before its key, since nothing here reaches them after. */
export function heldImages(ctx: RuntimeContext): { on(placeId: string): Promise<HeldImage[]>; drop(placeId: string, id: string): Promise<void> } {
  const copiesAt = async (placeId: string): Promise<{ key: string; name: string; manifest: GoldenManifest }[]> => {
    const copies: { key: string; name: string; manifest: GoldenManifest }[] = [];
    for (const key of await ctx.store.keys(GOLDENS)) {
      const parts = copyKeyParts(key);
      if ((parts.place ?? ctx.places.wired) !== placeId) continue;
      const manifest = (await ctx.store.get(GOLDENS, key)) as GoldenManifest | undefined;
      if (manifest !== undefined) copies.push({ key, name: parts.name, manifest });
    }
    return copies;
  };
  const projectsAt = async (placeId: string): Promise<string[]> =>
    (await ctx.store.list(PROJECT_GOLDENS))
      .map(ctx.projectGoldenOf)
      .filter(p => (p.place ?? ctx.places.wired) === placeId)
      .map(p => p.snapshotId);
  // A project seeded onto a cloud keeps the image its add sealed there on its own record, and nowhere else.
  const seededAt = (placeId: string) => [...ctx.projectsHeld.values()].filter(p => p.computer === placeId && p.image !== undefined);
  return {
    async on(placeId) {
      await ctx.ready();
      const versions = (await copiesAt(placeId)).flatMap(c => c.manifest.versions.map(v => ({ id: v.snapshotId, ...(v.templateId === undefined ? {} : { templateId: v.templateId }) })));
      return [...versions, ...(await projectsAt(placeId)).map(id => ({ id })), ...seededAt(placeId).map(p => ({ id: p.image!.snapshotId }))];
    },
    async drop(placeId, id) {
      if ((await projectsAt(placeId)).includes(id)) await ctx.store.delete(PROJECT_GOLDENS, id);
      for (const { image, ...project } of seededAt(placeId)) if (image!.snapshotId === id) await ctx.rememberProject(project);
      for (const { key, name, manifest } of await copiesAt(placeId)) {
        const gone = manifest.versions.filter(v => v.snapshotId === id);
        if (gone.length === 0) continue;
        const versions = manifest.versions.filter(v => v.snapshotId !== id);
        if (versions.length === 0) await ctx.store.delete(GOLDENS, key);
        else await ctx.store.put(GOLDENS, key, { ...manifest, versions });
        for (const v of gone) await ctx.dropCopyRecipe(placeId, name, v.version);
      }
    },
  };
}
