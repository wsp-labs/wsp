// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-refused.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 9.
use wsp_words::exit::HostError;

use crate::pending::slate::SlateProblem;

// slate-refused.ts:6
// const ERRORS_LISTED = SLATE_LIMITS.errorsPerPass;
//
// /** A write the parser or the validator refused, with every error it found and the warnings beside them. */
// export function invalid(errors: SlateProblem[], warnings: SlateProblem[]): Error {
//   const said = (p: SlateProblem): string => {
//     const where = [p.line !== undefined ? `line ${p.line}` : undefined, p.piece !== undefined ? (p.prop !== undefined ? `${p.piece}.${p.prop}` : p.piece) : undefined].filter(w => w !== undefined).join(", ");
//     return `${where === "" ? "" : `${where}: `}${p.code} ${p.name} ${p.message}${p.fix !== undefined ? `. Did you mean ${p.fix}?` : ""}`;
//   };
//   // Every error at once, so a slate with seven mistakes takes one rewrite and not seven; one line, as every refusal is.
//   const listed = errors.slice(0, ERRORS_LISTED);
//   const more = errors.length > ERRORS_LISTED ? ` And ${errors.length - ERRORS_LISTED} more.` : "";
//   const message = `slate refused: ${errors.length} error${errors.length === 1 ? "" : "s"}: ${listed.length === 1 ? said(listed[0]!) : listed.map((p, i) => `${i + 1}) ${said(p)}`).join(" ")}${more}`;
//   return Object.assign(new Error(message), { kind: "invalid", errors: errors.slice(0, ERRORS_LISTED), warnings });
// }
/// Kind `invalid`, the first 20 errors and every warning as JSON, no fix.
pub fn invalid(errors: &[SlateProblem], warnings: &[SlateProblem]) -> HostError {
    todo!()
}
