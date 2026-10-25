// SPDX-License-Identifier: AGPL-3.0-only
// A cold writer's runs.json read as the runs' records, for the harness and the glance count alike.
import type { SlateJson } from "../../../../../packages/protocol/src/slate/index.js";

/** A writer's runs.json as each run's record: an entry already shaped as one (json, out or state) as it stands,
 * text as the run's out and its json where it parses, anything else as its json. */
export function runResults(runs: Record<string, unknown>, given: Record<string, unknown>, now: number): Record<string, SlateJson> {
  const out: Record<string, SlateJson> = {};
  for (const run of Object.keys(runs)) {
    const v = given[run] ?? given[`$${run}`];
    if (v === undefined) continue;
    const done = { state: "done", runs: 1, exit: 0, startedAt: now - 2_000, endedAt: now };
    if (typeof v === "object" && v !== null && !Array.isArray(v) && ["json", "out", "state", "exit"].some(k => k in v)) out[run] = { ...done, ...(v as Record<string, SlateJson>), ...jsonOf(v as Record<string, unknown>) };
    else if (typeof v === "string") {
      let json: SlateJson | undefined;
      try {
        json = JSON.parse(v) as SlateJson;
      } catch {
        json = undefined;
      }
      out[run] = { ...done, out: v, ...(json !== undefined ? { json } : {}) };
    } else out[run] = { ...done, json: v as SlateJson };
  }
  return out;
}

/** A run's json, read off its out where the writer gave only that, as the host parses a run's stdout. */
function jsonOf(rec: Record<string, unknown>): { json?: SlateJson } {
  if (rec["json"] !== undefined || typeof rec["out"] !== "string") return {};
  try {
    return { json: JSON.parse(rec["out"]) as SlateJson };
  } catch {
    return {};
  }
}
