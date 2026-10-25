// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/ps-time.ts at the cut (383708c78). Each TS statement stands as a comment above its stub.

use crate::js::to_number;

/// The cumulative cpu time ps prints, read once for everything on this computer
/// that asks for it: the turn activity clock in the runtime and the processes
/// module in the daemon. Neither package may import the other, and a second
/// copy of this parser is how the two would drift.
/// Seconds of cpu behind a ps TIME column, `[dd-][hh:]mm:ss[.cc]`: this Mac's ps carries hundredths and Linux's
/// whole seconds, so the same field is read to whatever precision the machine printed. Anything else reads as no
/// cpu at all, since a row that cannot be read must not look like a busy one.
// ps-time.ts:10
// export function psCpuSeconds(time: string): number {
//   const dash = time.indexOf("-");
//   const days = dash === -1 ? 0 : Number(time.slice(0, dash));
//   let seconds = 0;
//   for (const part of time.slice(dash + 1).split(":")) seconds = seconds * 60 + Number(part);
//   const total = days * 86_400 + seconds;
//   return Number.isFinite(total) && total >= 0 ? total : 0;
// }
pub fn ps_cpu_seconds(time: &str) -> f64 {
    let (days, rest) = match time.split_once('-') {
        Some((days, rest)) => (to_number(days), rest),
        None => (0.0, time),
    };
    let seconds = rest.split(':').fold(0.0, |seconds, part| seconds * 60.0 + to_number(part));
    let total = days * 86_400.0 + seconds;
    if total.is_finite() && total >= 0.0 {
        total
    } else {
        0.0
    }
}
