// SPDX-License-Identifier: AGPL-3.0-only
// A cold writer's slate as it stands, for the harness: node extract.cjs <writer folder> <name> prints the stored slate
// in this revision's own form to written/<name>.slate and copies the writer's runs.json beside it.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { printSlate, type SlateDoc } from "../../../../../packages/protocol/src/slate/index.js";

const [folder, name, into] = process.argv.slice(2) as [string, string, string];
const state = join(folder, ".slate-state.json");
const records = existsSync(state) ? (Object.values(JSON.parse(readFileSync(state, "utf8")) as Record<string, { document: SlateDoc | null; version: number }>)) : [];
const doc = records[0]?.document ?? null;
mkdirSync(into, { recursive: true });
if (doc !== null) writeFileSync(join(into, `${name}.slate`), `${printSlate(doc)}\n`);

/** Every runs.json under the folder, the newest first. */
const found: string[] = [];
const walk = (at: string): void => {
  for (const e of readdirSync(at, { withFileTypes: true })) {
    if (e.name.startsWith(".") || e.name === "node_modules") continue;
    if (e.isDirectory()) walk(join(at, e.name));
    else if (e.name === "runs.json") found.push(join(at, e.name));
  }
};
walk(folder);
found.sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
if (found[0] !== undefined) copyFileSync(found[0], join(into, `${name}.runs.json`));
process.stdout.write(`${JSON.stringify({ name, version: records[0]?.version ?? 0, slate: doc !== null, runs: found })}\n`);
