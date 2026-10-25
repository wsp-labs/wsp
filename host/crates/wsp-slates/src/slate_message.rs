// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-message.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 5-7.
use serde_json::Value;

pub use crate::pending::slate::PathSeg;

// slate-message.ts:5
// /** A carried value with any line that would read as a second `slate:` line made harmless (12, injection). */
// export function defanged(value: SlateJson): SlateJson {
//   if (typeof value === "string") return value.replace(/^(\s*slate)\s*:(?=\s*\{)/gm, "$1：");
//   if (Array.isArray(value)) return value.map(defanged);
//   if (value !== null && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, defanged(v)]));
//   return value;
// }
pub fn defanged(value: &Value) -> Value {
    todo!()
}

/// The longest string inside a value: its path and its length in UTF-16 units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Longest {
    pub at: Vec<PathSeg>,
    pub length: usize,
}

// slate-message.ts:14
// /** The longest string inside a value, by the path to it, for cutting a message down to its cap. */
// export function longest(value: SlateJson, at: (string | number)[] = []): { at: (string | number)[]; length: number } | undefined {
//   if (typeof value === "string") return { at, length: value.length };
//   const kids = Array.isArray(value) ? value.map((v, i) => longest(v, [...at, i])) : value !== null && typeof value === "object" ? Object.entries(value).map(([k, v]) => longest(v, [...at, k])) : [];
//   return kids.reduce<{ at: (string | number)[]; length: number } | undefined>((best, k) => (k !== undefined && (best === undefined || k.length > best.length) ? k : best), undefined);
// }
pub fn longest(value: &Value, at: &[PathSeg]) -> Option<Longest> {
    todo!()
}

// slate-message.ts:21
// export function cutAt(value: SlateJson, at: (string | number)[], keep: number): SlateJson {
//   if (at.length === 0) return typeof value === "string" ? `${value.slice(0, keep)} (cut)` : value;
//   const [head, ...rest] = at;
//   if (Array.isArray(value)) return value.map((v, i) => (i === head ? cutAt(v, rest, keep) : v));
//   if (value !== null && typeof value === "object") return { ...value, [head as string]: cutAt(value[head as string]!, rest, keep) };
//   return value;
// }
/// `keep` in UTF-16 units.
pub fn cut_at(value: &Value, at: &[PathSeg], keep: usize) -> Value {
    todo!()
}
