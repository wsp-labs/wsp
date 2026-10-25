// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-read.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 1-4.
use serde_json::Value;

use crate::pending::slate::{SlateDoc, SlateProblem};

// slate-read.ts:6
// export const isRunRecord = (v: SlateJson | undefined): v is SlateJson & { state: string; runs: number; why?: string } =>
//   typeof v === "object" && v !== null && !Array.isArray(v) && typeof v["state"] === "string" && typeof v["runs"] === "number";
pub fn is_run_record(v: Option<&Value>) -> bool {
    todo!()
}

// slate-read.ts:9
// /** How much of a run's output a read carries before it says where the rest is. */
// export const READ_OUTPUT_CHARS = 4_000;
/// UTF-16 units.
pub const READ_OUTPUT_CHARS: usize = 4_000;

// slate-read.ts:12
// /** A run's record as a read answers it: its summary alone when the read named paths, else the record with out left
//  * out where json was parsed from it and each long output cut, saying how to read it whole. */
// export function runShown(rec: SlateJson | undefined, name: string, named: boolean): SlateJson {
//   if (!isRunRecord(rec)) return rec ?? null;
//   const full = rec as unknown as Record<string, SlateJson>;
//   if (named) return Object.fromEntries(Object.entries(full).filter(([k]) => ["state", "why", "exit", "ms", "runs", "stale", "refreshing", "endedAt"].includes(k)));
//   const cut = (field: string, text: string): string => (text.length <= READ_OUTPUT_CHARS ? text : `${text.slice(0, READ_OUTPUT_CHARS)}… (${text.length} characters; values ["$${name}.${field}"] reads it whole)`);
//   const out: Record<string, SlateJson> = {};
//   for (const [k, v] of Object.entries(full)) {
//     if (k === "out" && full["json"] !== undefined) continue;
//     if ((k === "out" || k === "err") && typeof v === "string") out[k] = cut(k, v);
//     else if (k === "lines" && Array.isArray(v)) out[k] = v.slice(-50);
//     else out[k] = v;
//   }
//   return out;
// }
//
/// `rec` none is the TS `undefined`.
pub fn run_shown(rec: Option<&Value>, name: &str, named: bool) -> Value {
    todo!()
}

// slate-read.ts:30
// export function misread(doc: SlateDoc | null, path: string): SlateProblem[] {
//   if (!/^[A-Za-z_]\w*(\.\w+|\[\d+\])*$/.test(path)) return [];
//   const root = path.split(/[.[]/)[0]!;
//   if (doc?.runs[root] !== undefined) return [slateProblem("X401", `${path} is not a source: $${root} is a run, read as $${path}; its record is also under runs.$${root}`, { fix: `$${path}` })];
//   if (doc?.values[root] !== undefined || doc?.derived[root] !== undefined) return [slateProblem("X401", `${path} is not a source: $${root} is a value, read as $${path}`, { fix: `$${path}` })];
//   if (SLATE_SOURCES[root] === undefined) return [slateProblem("X401", `${root} is not a source and not this slate's; a slate's own names start with $`)];
//   return [];
// }
pub fn misread(doc: Option<&SlateDoc>, path: &str) -> Vec<SlateProblem> {
    todo!()
}
