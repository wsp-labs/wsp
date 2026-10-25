// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/quote.ts at the cut (383708c78). Each TS statement stands as a comment above its stub.

use crate::js::{js_collapse_whitespace, js_trim};

/// Words off the network as a message to an agent carries them: folded onto one line where they name something, and
/// fenced where they are quoted, so nothing in them reads as the person's own line or closes the fence early.
/// Text folded onto one line: every run of whitespace, line breaks included, as one space.
// quote.ts:6
// export const oneLine = (text: string): string => text.replace(/\s+/g, " ").trim();
pub fn one_line(text: &str) -> String {
    js_trim(&js_collapse_whitespace(text)).to_string()
}

/// A fence one backtick longer than any run of backticks inside the text, so nothing in it can close the fence.
// quote.ts:9
// export function fenceFor(text: string): string {
//   const longest = Math.max(0, ...[...text.matchAll(/`+/g)].map(m => m[0].length));
//   return "`".repeat(Math.max(3, longest + 1));
// }
pub fn fence_for(text: &str) -> String {
    let (mut longest, mut run) = (0, 0);
    for c in text.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat(3.max(longest + 1))
}
