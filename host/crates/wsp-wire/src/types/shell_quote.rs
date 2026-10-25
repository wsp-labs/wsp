// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/shell-quote.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/shell-quote.ts:16 at a3b870a6e.
pub const ENV_FROM_INPUT: &str = "while IFS= read -r -d '' wsp_kv; do export \"$wsp_kv\"; done; unset wsp_kv";

// ---- Hand-written below: the functions of the unit.

// shell-quote.ts:4 at a3b870a6e
// export function shellQuote(value: string): string {
//   return `'${value.replaceAll("'", String.raw`'\''`)}'`;
// }
/// One POSIX sh word: the value in single quotes, each `'` in it written `'\''`.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

// shell-quote.ts:10 at a3b870a6e
// export function inFolder(cwd: string | undefined, command: string): string {
//   return `cd ${cwd === undefined ? "~" : shellQuote(cwd)} && ${command}`;
// }
/// `cd <folder quoted> && <command>`, or `cd ~ && <command>` where no folder is named.
pub fn in_folder(cwd: Option<&str>, command: &str) -> String {
    match cwd {
        None => format!("cd ~ && {command}"),
        Some(folder) => format!("cd {} && {command}", shell_quote(folder)),
    }
}

// shell-quote.ts:19 at a3b870a6e (private)
// const BARE_WORD = /^[A-Za-z0-9_@%+=:,./-]+$/;

// shell-quote.ts:23 at a3b870a6e
// export function shellLine(words: readonly string[]): string {
//   return words.map(w => (BARE_WORD.test(w) ? w : shellQuote(w))).join(" ");
// }
/// Each word bare where it is one or more of `A-Za-z0-9_@%+=:,./-` and nothing else, quoted otherwise (the empty word
/// too), joined by single spaces.
pub fn shell_line(words: &[&str]) -> String {
    let bare = |w: &str| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    words.iter().map(|w| if bare(w) { w.to_string() } else { shell_quote(w) }).collect::<Vec<_>>().join(" ")
}
