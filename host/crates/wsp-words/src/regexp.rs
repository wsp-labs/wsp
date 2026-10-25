// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/regexp.ts at a3b870a6e. Each TS statement stands as a comment above its stub.

/// Text matched as itself inside a RegExp.
// regexp.ts:4
// export const escapeRegExp = (s: string): string => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
pub fn escape_reg_exp(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if ".*+?^${}()|[]\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
