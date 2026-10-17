// SPDX-License-Identifier: AGPL-3.0-only
import { isMissing } from "./errors.js";
import { templatesOf } from "./golden.js";
import type { MachineBackend } from "./machine.js";

/** The template standing on a snapshot, deleted before the snapshot is: the provider refuses a snapshot's delete while
 * one stands on it, and a template already gone is no failure. */
export async function dropTemplate(backend: MachineBackend, templateId: string | undefined): Promise<void> {
  if (templateId === undefined) return;
  await templatesOf(backend)?.delete(templateId).catch((e: unknown) => {
    if (!isMissing(e)) throw e;
  });
}
