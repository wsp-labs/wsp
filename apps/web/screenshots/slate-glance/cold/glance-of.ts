// SPDX-License-Identifier: AGPL-3.0-only
// The new glance warnings on any written slate with its runs.json as the runs' results, the harness's way:
// node glance-of.cjs <file.slate> [<runs.json>] prints the codes and the sketch.
import { existsSync, readFileSync } from "node:fs";
import { parseSlate, sketchSlate, slateStartValues } from "../../../../../packages/protocol/src/slate/index.js";
import { runResults } from "./run-results.js";

const [file, runsFile] = process.argv.slice(2) as [string, string | undefined];
const parsed = parseSlate(readFileSync(file, "utf8"));
if (parsed.document === undefined) {
  process.stdout.write(`${JSON.stringify({ errors: parsed.errors.map(e => `${e.code} ${e.message}`) })}\n`);
  process.exit(0);
}
const doc = parsed.document;
const now = Date.parse("2026-10-25T16:30:00Z");
const given = runsFile !== undefined && existsSync(runsFile) ? (JSON.parse(readFileSync(runsFile, "utf8")) as Record<string, unknown>) : {};
const values = runResults(doc.runs, given, now);
const sketch = sketchSlate(doc, { ...slateStartValues(doc), ...values }, { version: 1, now });
const codes = [...sketch.matchAll(/^ {2}(W03\d) /gm)].map(m => m[1]!);
process.stdout.write(`${JSON.stringify({ codes, sketch, runsMatched: Object.keys(values), runsDeclared: Object.keys(doc.runs) })}\n`);

