// SPDX-License-Identifier: AGPL-3.0-only
// The broker's off switch: the machines whose every call is refused, kept in a
// file beside the state where one is named, so a machine whose provider delete
// failed is still off after the host restarts.
import { readFileSync } from "node:fs";
import { basename, dirname } from "node:path";
import { writeOwn } from "@wsp/own-file";

/** How many revoked machines the off switch remembers, on disk too: a machine whose provider delete failed stays off
 * until this many later revokes push it out. */
export const OFF_SWITCH_MAX = 1024;

export interface OffSwitch {
  has(workspaceId: string): boolean;
  add(workspaceId: string): void;
  delete(workspaceId: string): void;
}

const readIds = (file: string): string[] => {
  try {
    const ids: unknown = JSON.parse(readFileSync(file, "utf8"));
    return Array.isArray(ids) ? ids.filter((id): id is string => typeof id === "string") : [];
  } catch {
    return [];
  }
};

/** Switches one id on or off in `ids`, oldest first under the cap; answers whether the set may have changed. */
const switched = (ids: Set<string>, id: string, on: boolean): boolean => {
  const had = ids.delete(id);
  if (on) ids.add(id);
  while (ids.size > OFF_SWITCH_MAX) ids.delete(ids.values().next().value!);
  return on || had;
};

/** The switch, oldest first; in memory alone with no file. Each change is made to the file as it stands then and
 * written whole, since a terminal's own runtime (`wsp doctor`) holds a switch on the same file while the host serves. */
export function offSwitch(file?: string): OffSwitch {
  const off = new Set(file === undefined ? [] : readIds(file).slice(-OFF_SWITCH_MAX));
  const change = (id: string, on: boolean): void => {
    switched(off, id, on);
    if (file === undefined) return;
    const kept = new Set(readIds(file));
    if (!switched(kept, id, on)) return;
    try {
      writeOwn(dirname(file), basename(file), JSON.stringify([...kept]));
    } catch (e) {
      console.warn(`the broker's off switch was not kept on disk, so a host restart forgets it: ${e instanceof Error ? e.message : String(e)}`);
    }
  };
  return {
    has: id => off.has(id),
    add: id => change(id, true),
    delete: id => change(id, false),
  };
}
