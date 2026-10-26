// SPDX-License-Identifier: AGPL-3.0-only
//! The script a command ran, out of the `<shell> -lc <script>` argv codex names it by, joined with the shlex crate.
//! TS adapter-codex/src/shell-script.ts. Spec: spec/codex_rpc.md rule 14.

/// The shells whose `-lc` or `-c` is taken off. TS shell-script.ts:8.
pub const SHELLS: [&str; 4] = ["sh", "bash", "zsh", "dash"];

/// The words of a line as a POSIX shell splits them, quotes taken off; `None` for a line it would refuse.
/// TS shell-script.ts:11 (private there).
pub fn shell_words(line: &str) -> Option<Vec<String>> {
    let chars: Vec<char> = line.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut word: Option<String> = None;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' | '\n' => {
                if let Some(done) = word.take() {
                    words.push(done);
                }
            }
            '\'' => {
                let end = (i + 1..chars.len()).find(|&j| chars[j] == '\'')?;
                word.get_or_insert_with(String::new).extend(&chars[i + 1..end]);
                i = end;
            }
            '"' => {
                let current = word.get_or_insert_with(String::new);
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < chars.len() && "\"\\$`\n".contains(chars[i + 1]) {
                        i += 1;
                    }
                    current.push(chars[i]);
                    i += 1;
                }
                if i >= chars.len() {
                    return None;
                }
            }
            '\\' => {
                if i + 1 >= chars.len() {
                    return None;
                }
                i += 1;
                word.get_or_insert_with(String::new).push(chars[i]);
            }
            c => word.get_or_insert_with(String::new).push(c),
        }
        i += 1;
    }
    if let Some(done) = word {
        words.push(done);
    }
    Some(words)
}

/// The script a command ran, where it is a shell's `-lc` or `-c` around one; any other command as it came.
/// TS shell-script.ts:43.
pub fn shell_script_of(command: &str) -> String {
    let Some(words) = shell_words(command).filter(|w| w.len() == 3) else {
        return command.to_string();
    };
    let [shell, flag, script] = &words[..] else {
        return command.to_string();
    };
    let name = shell.rsplit('/').next().unwrap_or(shell);
    if SHELLS.contains(&name) && (flag == "-lc" || flag == "-c") {
        script.clone()
    } else {
        command.to_string()
    }
}
