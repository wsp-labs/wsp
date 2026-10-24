// SPDX-License-Identifier: AGPL-3.0-only
// Loaded into the fixture's host with NODE_OPTIONS=--import, never into the person's: gives this computer the display
// name a Mac's `scutil --get ComputerName` gives it (packages/host/src/places/this-computer.ts:57), so a Linux fixture
// has a computer whose name on screen differs from its hostname, as the owner's Mac has. No file on disk changes.
import { register } from "node:module";
import { isMainThread } from "node:worker_threads";

export const LABEL_ENV = "WSP_FIXTURE_LABEL";
/** The host's one line that keeps a label off every computer but a Mac, as its bundle spells it. */
export const MAC_ONLY = 'if (platform() !== "darwin") return labelHere;';

export const relabel = (source, label) => source.replace(MAC_ONLY, `return labelHere = ${JSON.stringify(label)};`);

let label;

export function initialize(data) {
  label = data.label;
}

export async function load(url, context, nextLoad) {
  const loaded = await nextLoad(url, context);
  if (label === undefined || !url.startsWith("file:") || loaded.source == null) return loaded;
  const source = typeof loaded.source === "string" ? loaded.source : Buffer.from(loaded.source).toString("utf8");
  return source.includes(MAC_ONLY) ? { ...loaded, source: relabel(source, label) } : loaded;
}

// Taken out of the environment once read, so the agents and shells the host starts load nothing of this.
if (isMainThread && process.env[LABEL_ENV]) {
  register(import.meta.url, { data: { label: process.env[LABEL_ENV] } });
  delete process.env[LABEL_ENV];
  delete process.env.NODE_OPTIONS;
}
