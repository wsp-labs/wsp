// SPDX-License-Identifier: AGPL-3.0-only
// What wsp init says before anything boots: the Found card, the Summary card
// and the build question's own line, off the rows as the screens left them.
import { styleText } from "node:util";
import { LOGIN_CHOICES, MCP_REMOTE_ID, RUNGS, type Manifest, type ManifestEntry } from "@wsp/collect";
import { agentInstallsFor, brewfileFor, estimateDisk, isMcpRow, PACK_BUDGET_BYTES, plural, shownOf, toolInstallsFor, type BrewTable } from "@wsp/engine";
import { SIGN_IN_ANSWERS, fmtBytes, type RecipeCustomRow } from "@wsp/protocol";
import { RUNG_TITLE, isLoginChoice, isTickable } from "./init-recipe.js";
import { CARD_FRAME, GUTTER, ellipsize, table, wrap } from "./init-layout.js";
import type { Tone } from "./init-select.js";
import { diskLine, diskTone } from "./init-weight.js";

const bytesOf = (entries: readonly ManifestEntry[]): string => {
  const n = entries.reduce((sum, e) => sum + e.bytes, 0);
  return n > 0 ? fmtBytes(n) : "";
};

/** One row per rung: name, how many rows, their size, and how many can come when some cannot. */
export function detectionNote(manifest: Manifest, source: string): string[] {
  if (manifest.entries.length === 0) return ["Nothing found to bring.", "", "Nothing has left this computer."];
  const rows = RUNGS.map(rung => manifest.entries.filter(e => e.rung === rung))
    .filter(entries => entries.length > 0)
    .map(entries => {
      const canCome = entries.filter(isTickable).length;
      return [RUNG_TITLE[entries[0]!.rung], String(entries.length), bytesOf(entries), canCome < entries.length ? `${canCome} can come` : ""];
    });
  return [...table(rows, ["left", "right", "right"]), "", `${manifest.entries.length} ${source}. Nothing has left this computer.`];
}

/** The label on the first of the summary card's rows outside the catalog; the rest sit under it in the same column. */
export const ADDED_LABEL = "Added";

/** What the ticked rows install, counted once for the summary card and the build screen's own line: the agents by
 * name, the tools the person ticked and the rows outside the catalog (Homebrew's toolchain named apart), and the
 * MCP servers that travel. */
function buildCounts(
  manifest: Manifest,
  ticks: ReadonlySet<string>,
  brew: BrewTable,
  custom: readonly RecipeCustomRow[],
): { agents: string[]; tools: number; toolchain: string; servers: number } {
  const bring = manifest.entries.filter(e => ticks.has(e.id)).map(e => ({ ...e, bring: true }));
  const steps = toolInstallsFor(bring, brew, custom).installs;
  return {
    agents: agentInstallsFor(bring).installs.map(a => a.name),
    // The rows outside the catalog are the person's tools too, and no tick names them.
    tools: steps.filter(t => ticks.has(t.id)).length + custom.length,
    toolchain: steps.some(t => t.id.startsWith("tools/brew-toolchain/")) ? " plus Homebrew's toolchain" : "",
    servers: bring.filter(e => isMcpRow(e) && e.id !== MCP_REMOTE_ID).length,
  };
}

/** The build screen's own line: what the screens before it settled on, how long the build takes, how big the
 * image lands, and that a machine left alone stops billing. What it costs an hour is the boot question's, under it. */
export function readyLine(
  manifest: Manifest,
  ticks: ReadonlySet<string>,
  choices: ReadonlyMap<string, string>,
  upload: number,
  brew: BrewTable,
  takes: string,
  custom: readonly RecipeCustomRow[] = [],
): string {
  const { agents, tools } = buildCounts(manifest, ticks, brew, custom);
  const est = estimateDisk(manifest.entries.filter(e => ticks.has(e.id)).map(e => ({ ...e, bring: true })), upload, brew, custom);
  const machine = manifest.entries.filter(e => e.rung === "logins" && choices.get(e.id) === "machine").length;
  return [
    `Ready to build. ${plural(agents.length, "agent")}, ${plural(tools, "tool")}, ${fmtBytes(est.total)} on the image.`,
    ...(machine > 0 ? [`${plural(machine, "sign-in")} on the machine during the build.`] : []),
    `The build takes ${takes}; a workspace naps when it is idle and stops billing.`,
  ].join(" ");
}

/** One row per rung with its ticks and size, the sign-ins under theirs with the answer each
 * got, a credential-shaped row under its rung when it got an answer other than skip,
 * then what uploads and what installs, wrapped under their own column. `upload` is the plan's own
 * count of the bytes that travel; without it the ticked rows' sizes stand in. */
export function summaryNote(
  manifest: Manifest,
  ticks: ReadonlySet<string>,
  choices: ReadonlyMap<string, string>,
  width: number,
  upload: number = manifest.entries.filter(e => ticks.has(e.id)).reduce((n, e) => n + e.bytes, 0),
  brew: BrewTable = new Map(),
  custom: readonly RecipeCustomRow[] = [],
  /** The disk the provider gives a builder, where it gives a figure; without one the Disk line names no cap and
   * carries no weight, since a container's disk is the box's. */
  builderDiskGb?: number,
): string[] {
  const perRung = RUNGS.map(rung => manifest.entries.filter(e => e.rung === rung)).filter(entries => entries.length > 0);
  const answer = (e: ManifestEntry): string => { const c = choices.get(e.id); return isLoginChoice(c) ? SIGN_IN_ANSWERS[c].short : "skip"; };
  // The sign-ins row counts the answers that bring something, as its screen's header does: a sign-in is chosen though nothing is ticked.
  const loginSpread = (entries: readonly ManifestEntry[]): string =>
    LOGIN_CHOICES.filter(c => c !== "skip")
      .map(c => ({ n: entries.filter(e => answer(e) === SIGN_IN_ANSWERS[c].short).length, label: SIGN_IN_ANSWERS[c].short }))
      .filter(p => p.n > 0)
      .map(p => `${p.n} ${p.label}`)
      .join(", ");
  const rows = table(
    perRung.map(entries => {
      const on = entries.filter(e => ticks.has(e.id));
      const spread = entries[0]!.rung === "logins" ? loginSpread(entries) : "";
      return [RUNG_TITLE[entries[0]!.rung], spread !== "" ? spread : `${on.length} of ${entries.length}`, bytesOf(on)];
    }),
    ["left", "right", "right"],
  );
  const listed = manifest.entries.filter(e => e.rung === "logins" || (e.consent === true && answer(e) !== "skip"));
  // Lines are wrapped here to what the card leaves, so the card prints them as they are; the label keeps room for the widest answer.
  const inner = width - CARD_FRAME;
  const labelRoom = inner - 2 - GUTTER.length - Math.max(...Object.values(SIGN_IN_ANSWERS).map(w => w.short.length));
  const answered = new Map(table(listed.map(e => [`  ${ellipsize(e.label, labelRoom)}`, answer(e)])).map((line, i) => [listed[i]!.id, line]));
  const lines = perRung.flatMap((entries, i) => [rows[i]!, ...entries.flatMap(e => answered.get(e.id) ?? [])]);
  const bring = manifest.entries.filter(e => ticks.has(e.id)).map(e => ({ ...e, bring: true }));
  const { agents, tools, toolchain, servers } = buildCounts(manifest, ticks, brew, custom);
  const roads = brewfileFor(bring, brew).roads;
  const RELEASE_PIN_WORDS = "tag and checksum recorded at the seal, checked on every copy";
  const fromReleases = roads.length === 0 ? "" : `, ${roads.length} from ${roads.length === 1 ? "its" : "their"} GitHub release${roads.length === 1 ? "" : "s"} (${RELEASE_PIN_WORDS})`;
  const installs = [...agents, ...(tools > 0 ? [`${tools} tool${tools === 1 ? "" : "s"}${toolchain}${fromReleases}`] : []), ...(servers > 0 ? [`${servers} MCP server${servers === 1 ? "" : "s"}`] : [])];
  const est = estimateDisk(bring, upload, brew, custom);
  // The Disk line takes its weight's colour here too: the card is the last thing read before the confirm.
  const closing: [string, string, Tone | undefined][] = [
    ["Upload", upload > PACK_BUDGET_BYTES ? `${fmtBytes(upload)}, over the ${fmtBytes(PACK_BUDGET_BYTES)} the machine's disk allows` : `${fmtBytes(upload)}, nothing has left this computer yet`, undefined],
    ["Installs", installs.length > 0 ? installs.join(", ") : "nothing; the machine boots bare", undefined],
    // A row outside the catalog is a line the person's own agent wrote, run as root on the builder: the card is the
    // last thing read before the boot, so each one is named here with the command it runs, never only counted.
    ...custom.map((c, i): [string, string, Tone | undefined] => [i === 0 ? ADDED_LABEL : "", `${c.name} runs ${shownOf(c.install)}`, undefined]),
    ["Disk", diskLine(est, builderDiskGb), builderDiskGb === undefined ? undefined : diskTone(est.total, est.room)],
  ];
  const column = Math.max(...closing.map(([label]) => label.length)) + GUTTER.length;
  return [...lines, "", ...closing.flatMap(([label, text, tone]) => wrap(`${label.padEnd(column)}${text}`, inner, " ".repeat(column)).map(l => (tone === undefined ? l : styleText(tone, l))))];
}
