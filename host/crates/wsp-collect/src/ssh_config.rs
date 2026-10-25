// SPDX-License-Identifier: AGPL-3.0-only
//! OpenSSH stops reading a config file at the first keyword it does not know, and the machine's ssh is not the one on
//! the person's computer, so a copied config gets IgnoreUnknown at the top. Port of packages/collect/src/ssh-config.ts
//! at a3b870a6e; the spec is spec/small.md.

pub const SSH_CONFIG_PATH: &str = "~/.ssh/config";

/// ssh-config.ts:13, said once: the scan's row and the copied file's header line both read from it.
macro_rules! why {
    () => {
        "ssh on the machine does not know every option yours does, and one unknown option would stop it reading the file"
    };
}

/// The lines put above everything the file holds (ssh-config.ts:17).
pub const PRELUDE: &str = concat!("# wsp: ", why!(), ".\nIgnoreUnknown *\n");

/// The ssh config row's line in the scan (ssh-config.ts:20).
pub const SSH_IGNORE_UNKNOWN_DETAIL: &str = concat!("copied with IgnoreUnknown at the top, since ", why!());

/// The config as the machine can parse it, the rest of the file untouched; a config that already starts with the
/// prelude is left alone.
// ssh-config.ts:22
// /** The config as the machine can parse it, the rest of the file untouched. A config that has already been through
//  * here is left alone, so a re-import does not stack the line. */
// export function withIgnoreUnknown(text: string): string {
//   return text.startsWith(PRELUDE) ? text : PRELUDE + text;
// }
pub fn with_ignore_unknown(text: &str) -> String {
    todo!()
}
