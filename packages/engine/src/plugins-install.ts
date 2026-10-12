// SPDX-License-Identifier: AGPL-3.0-only
// An agent's plugins put on a machine by the agent's own commands, one at a
// time: on a computer somebody owns from the picks, and on the image from the
// plugins the person's computer switches on. A plugin that fails is named and
// the rest go on; none fails the job it rides in.
import { shellQuote, type PlaceProvisionRow } from "@wsp/protocol";
import type { Machine } from "./machine.js";

/** One plugin's lines, by its agent's own commands; `asked` reads off what one printed why it is set aside. */
export interface PluginInstall {
  id: string;
  label: string;
  cmd: string;
  asked?(out: string): string | undefined;
}

/** The plugins an image carries for the one ticked agent that takes them: the folder its store keeps them in, emptied
 * first so the image holds exactly these, each plugin's lines, and the plugins set aside before anything ran. */
export interface ImagePlugins {
  /** The agent's row id, whose install the plugins wait on. */
  agent: string;
  folder: string;
  installs: readonly PluginInstall[];
  skipped: readonly { id: string; label: string; note: string }[];
}

/** How long one plugin's install gets: its marketplace's clone and the plugin's own files. */
export const PLUGIN_MS = 300_000;

/** The last line a command said, for a row's note. */
export const lastWords = (text: string): string | undefined => text.trim().split("\n").at(-1)?.slice(0, 300) || undefined;

/** Runs one plugin's lines after the job's PATH line and answers its row. */
export async function installPlugin(machine: Machine, plugin: PluginInstall, pathLine: string): Promise<PlaceProvisionRow> {
  const started = Date.now();
  const res = await machine.run(`${pathLine}\n${plugin.cmd}`, { deadlineMs: PLUGIN_MS }).catch((e: unknown) => ({ exitCode: -1, stdout: "", stderr: e instanceof Error ? e.message : String(e) }));
  if (res.exitCode === 0) return { id: plugin.id, label: plugin.label, outcome: "installed", ms: Date.now() - started };
  const asked = plugin.asked?.(res.stdout);
  return asked !== undefined
    ? { id: plugin.id, label: plugin.label, outcome: "skipped", note: asked }
    : { id: plugin.id, label: plugin.label, outcome: "failed", note: lastWords(res.stderr || res.stdout) ?? `exit ${res.exitCode}` };
}

/** The image's plugins on the builder, its folder emptied first: an image sealed before plugins installed carries the
 * person's own index, which names every plugin they ever installed, and one install off it fetched them all (189 MB
 * on the owner's, Claude Code 2.1.296). Answers the line the stage closes with. */
export async function putImagePlugins(machine: Machine, plugins: ImagePlugins, pathLine: string, stage: (detail: string) => void): Promise<string> {
  await machine.run(`rm -rf -- ${shellQuote(plugins.folder)}`, { deadlineMs: 60_000 });
  const rows: PlaceProvisionRow[] = plugins.skipped.map(s => ({ ...s, outcome: "skipped" }));
  for (const [i, plugin] of plugins.installs.entries()) {
    stage(`plugin ${plugin.label} (${i + 1}/${plugins.installs.length})`);
    rows.push(await installPlugin(machine, plugin, pathLine));
  }
  const ok = rows.filter(r => r.outcome === "installed").map(r => r.label);
  const not = rows.filter(r => r.outcome !== "installed").map(r => `${r.label} ${r.outcome} (${r.note})`);
  return [ok.length > 0 ? `plugins ${ok.join(", ")} installed` : "", ...not].filter(s => s !== "").join("; ");
}
