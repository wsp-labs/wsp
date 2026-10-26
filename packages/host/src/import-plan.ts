// SPDX-License-Identifier: AGPL-3.0-only
// What a computer's setup is planned from: this computer's rows with a recipe's picks written over them, and the files,
// installs and MCP plan those rows come to. The computer's own setup is the one road that plans from them.
import type { Manifest, ManifestEntry, Platform } from "@wsp/collect";
import type { BrewTable } from "@wsp/engine";
import { BREW_ID_PREFIX, customRows, type Recipe } from "@wsp/protocol";
import type { GoldenImport } from "@wsp/runtime";
import { importFor, refusedIsDir, type ImportOptions } from "./init-import.js";
import { answeredRows, defaultAnswers, lockRefused, manifestFor, tickLoginTools, withoutAgentTools } from "./init-recipe.js";

/** What a plan is made against beyond the rows themselves: this computer, its Homebrew table for the rows a formula
 * installs, and the Keychain values already read for it. A setup reads no Keychain, so its values are empty and the
 * pack finds none to carry. */
export interface BuildContext {
  home: string;
  platform: Platform;
  brew: BrewTable;
  secrets: ReadonlyMap<string, string>;
  /** Where the values the copied MCP servers read by name are kept. */
  vault?: ImportOptions["vault"];
  /** The build lands at a provider, whose machines get no secret value the copied configs hold (see cloudCut).
   * Absent keeps every value, the road to a computer the person owns. */
  cloud?: boolean;
}

/** What the ticked rows come to on the machine: the files plan, the installs and the MCP plan. `picked` is the rows
 * this import carries; `rows` is every row with its tick and its answer, since the MCP stage reads the unticked ones
 * too. */
export function planImport(picked: readonly ManifestEntry[], o: BuildContext & { rows: readonly ManifestEntry[]; small: Recipe } & Pick<ImportOptions, "keepFile" | "onResult" | "onContext" | "path" | "prefix">): GoldenImport {
  return importFor(picked, {
    home: o.home,
    ...(o.path !== undefined ? { path: o.path } : {}),
    ...(o.prefix !== undefined ? { prefix: o.prefix } : {}),
    secrets: o.secrets,
    ...(o.vault !== undefined ? { vault: o.vault } : {}),
    ...(o.cloud === true ? { cloud: true } : {}),
    platform: o.platform,
    rows: o.rows,
    brew: o.brew,
    custom: customRows(o.small),
    ...(o.keepFile !== undefined ? { keepFile: o.keepFile } : {}),
    ...(o.onResult !== undefined ? { onResult: o.onResult } : {}),
    ...(o.onContext !== undefined ? { onContext: o.onContext } : {}),
  });
}

/** Whether this computer's rows need Homebrew read at all: a row of its own is here. The one reading, so a
 * computer's provisioning asks it one way. */
export const wantsBrew = (manifest: Manifest): boolean => manifest.entries.some(e => e.id.startsWith(BREW_ID_PREFIX));

/** The Homebrew table a plan off this computer runs against: read only where a brew row of its own is here, and an
 * empty one where brew will not answer, since a formula's size decides no tick on a plan already settled. */
export async function brewTableFor(manifest: Manifest, brew: (() => Promise<BrewTable>) | undefined): Promise<BrewTable> {
  return (wantsBrew(manifest) ? await brew?.().catch(() => undefined) : undefined) ?? new Map();
}

/** The rows a setup is planned from: this computer as it is now with the recipe written over it, and every sign-in
 * row set to skip and unticked. The computer gets the person's files, tools and agents from this computer and its
 * sign-ins are the person's own to run there, so no Keychain is read here.
 *
 * The tool each answered sign-in needs is ticked before the answers go: the recipe records the answer and not the
 * tick it caused, so a plan off the answers as written would land a login on a machine with nothing to read it. */
export function copyRows(manifest: Manifest, image: { recipe: Recipe }, o: { home: string; brew: BrewTable }): ManifestEntry[] {
  const here = lockRefused({ ...manifest, entries: withoutAgentTools(manifest.entries) }, refusedIsDir(o.home));
  const applied = manifestFor({ manifest: here }, image.recipe);
  const { ticks, choices } = defaultAnswers(applied, o.brew);
  tickLoginTools(applied, choices, ticks, o.brew);
  for (const e of applied.entries) {
    if (e.rung !== "logins") continue;
    choices.set(e.id, "skip");
    ticks.delete(e.id);
  }
  return answeredRows(applied, ticks, choices);
}
