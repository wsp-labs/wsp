// SPDX-License-Identifier: AGPL-3.0-only
//! The script a command ran, out of the `<shell> -lc <script>` argv codex names it by, joined with the shlex crate.
//! TS adapter-codex/src/shell-script.ts. Spec: spec/codex_rpc.md rule 14.

/// The shells whose `-lc` or `-c` is taken off. TS shell-script.ts:8.
pub const SHELLS: [&str; 4] = ["sh", "bash", "zsh", "dash"];

/// The words of a line as a POSIX shell splits them, quotes taken off; `None` for a line it would refuse.
/// TS shell-script.ts:11 (private there).
pub fn shell_words(line: &str) -> Option<Vec<String>> {
    todo!()
}

/// The script a command ran, where it is a shell's `-lc` or `-c` around one; any other command as it came.
/// TS shell-script.ts:43.
pub fn shell_script_of(command: &str) -> String {
    todo!()
}
