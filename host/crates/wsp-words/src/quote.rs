// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/quote.ts at a3b870a6e. Each TS statement stands as a comment above its stub.

/// Words off the network as a message to an agent carries them: folded onto one line where they name something, and
/// fenced where they are quoted, so nothing in them reads as the person's own line or closes the fence early.
/// Text folded onto one line: every run of whitespace, line breaks included, as one space.
// quote.ts:6
// export const oneLine = (text: string): string => text.replace(/\s+/g, " ").trim();
pub fn one_line(text: &str) -> String {
    todo!()
}

/// A fence one backtick longer than any run of backticks inside the text, so nothing in it can close the fence.
// quote.ts:9
// export function fenceFor(text: string): string {
//   const longest = Math.max(0, ...[...text.matchAll(/`+/g)].map(m => m[0].length));
//   return "`".repeat(Math.max(3, longest + 1));
// }
pub fn fence_for(text: &str) -> String {
    todo!()
}

