// SPDX-License-Identifier: AGPL-3.0-only
//! A reply's shell block that a person can run where it stands: which labels run, the command a block holds, and
//! the text a finished run leaves behind it. Port of packages/protocol/src/run-block.ts at the cut (383708c78); spec chapter 13
//! carries it as the worked example. Each TS statement stands as a comment above its stub. See spec/run_block.md.

use wsp_wire::types::run_block::{RUN_SENT_LINES, SHELL_FENCES};

use crate::js::{js_trim, js_trim_end, utf16};

// A reply's shell block that a person can run where it stands: which labels run, the command a block holds, and the
// text a finished run leaves behind it, read by the app that draws the block and the host that keeps the record.
// The fence labels a reply's block runs under. Text, no label and every other language get no Run: the one thing
// that makes a block a command for the person is the agent saying so.
// run-block.ts:7
// export const SHELL_FENCES: ReadonlySet<string> = new Set(["sh", "bash", "zsh", "shell", "console"]);
// in wsp_wire::types::run_block: SHELL_FENCES

// run-block.ts:11
// in wsp_wire::types::run_block: RUN_BLOCK_WORDS

// Characters a page draws reordered, not at all, or as nothing: bidirectional overrides and isolates, zero-width
// marks, and every control but tab and newline. A block holding one can run something other than what it shows.
// run-block.ts:15
// private in the TS: port it as a private helper
// const HIDDEN = /[\u0000-\u0008\u000b-\u001f\u007f\u00ad\u061c\u200b-\u200f\u202a-\u202e\u2060-\u2064\u2066-\u2069\ufeff]/;

/// The characters a page would draw reordered or as nothing, and every control but tab and newline (run-block.ts:15).
fn is_hidden(c: char) -> bool {
    matches!(c as u32, 0x00..=0x08 | 0x0B..=0x1F | 0x7F | 0xAD | 0x61C | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2064 | 0x2066..=0x2069 | 0xFEFF)
}

/// The command a block holds, or nothing where the block is not one to run. A console block is a transcript: its
/// `$ ` lines are the commands and the lines between them are output, left out.
// run-block.ts:19
// export function runnableCommand(language: string, code: string): string | null {
//   const label = language.trim().toLowerCase();
//   if (!SHELL_FENCES.has(label)) return null;
//   const text = code.replace(/\r\n/g, "\n");
//   if (HIDDEN.test(text)) return null;
//   const lines = text.split("\n");
//   const prompted = label === "console" ? lines.filter(l => l.startsWith("$ ")).map(l => l.slice(2)) : [];
//   const command = (prompted.length > 0 ? prompted.join("\n") : text).trim();
//   return command === "" ? null : command;
// }
pub fn runnable_command(language: &str, code: &str) -> Option<String> {
    let label = js_trim(language).to_lowercase();
    if !SHELL_FENCES.contains(&label.as_str()) {
        return None;
    }
    let text = code.replace("\r\n", "\n");
    if text.chars().any(is_hidden) {
        return None;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let prompted: Vec<&str> = if label == "console" { lines.iter().filter_map(|l| l.strip_prefix("$ ")).collect() } else { Vec::new() };
    let command = js_trim(&if prompted.is_empty() { text.clone() } else { prompted.join("\n") }).to_string();
    (!command.is_empty()).then_some(command)
}

// run-block.ts:32
// in wsp_wire::types::run_block: PTY_RUN_DAEMON_VERSION

// The most of a run's output the thread keeps: the tail, which is where a failure says why.
// run-block.ts:35
// export const RUN_OUTPUT_MAX_CHARS = 64 * 1024;
// in wsp_wire::types::run_block: RUN_OUTPUT_MAX_CHARS (65,536 UTF-16 units, rule 8.4)

/// The tail of a run's output under the cap, starting at a line's start so the first line is never half of one.
// run-block.ts:38
// export function runOutputTail(text: string, max: number = RUN_OUTPUT_MAX_CHARS): string {
//   if (text.length <= max) return text;
//   const cut = text.slice(-max);
//   const newline = cut.indexOf("\n");
//   return newline < 0 ? cut : cut.slice(newline + 1);
// }
/// `max` is RUN_OUTPUT_MAX_CHARS where the TS leaves it out; counts UTF-16 units (rule 8.4).
pub fn run_output_tail(text: &str, max: usize) -> String {
    if utf16::len(text) <= max {
        return text.to_string();
    }
    let cut = if max == 0 { text.to_string() } else { utf16::tail(text, max) };
    match cut.find('\n') {
        Some(newline) => cut[newline + 1..].to_string(),
        None => cut,
    }
}

/// A terminal's bytes as the lines a person saw: escape sequences carry no text, a carriage return starts its line
/// over, and a backspace takes the letter before it. What a cursor move redraws in the middle of the screen is not
/// followed; a finished run's text is its lines, not its last frame.
// run-block.ts:48
// export function terminalText(raw: string): string {
//   const out: string[] = [];
//   let line: string[] = [];
//   let col = 0;
//   const put = (ch: string): void => {
//     line[col] = ch;
//     col += 1;
//   };
//   for (let i = 0; i < raw.length; i++) {
//     const ch = raw[i]!;
//     if (ch === "\x1b") {
//       const next = raw[i + 1];
//       if (next === "[") {
//         // CSI: parameters and intermediates up to the final byte, 0x40 to 0x7e.
//         let j = i + 2;
//         while (j < raw.length && !/[\x40-\x7e]/.test(raw[j]!)) j++;
//         i = j;
//       } else if (next === "]" || next === "P" || next === "_" || next === "^") {
//         // OSC, DCS, APC and PM: a string up to BEL or ST.
//         let j = i + 2;
//         while (j < raw.length && raw[j] !== "\x07" && !(raw[j] === "\x1b" && raw[j + 1] === "\\")) j++;
//         i = raw[j] === "\x07" ? j : j + 1;
//       } else {
//         // Two bytes, or an intermediate run (0x20 to 0x2f) and its final: ESC ( B, ESC # 8.
//         let j = i + 1;
//         while (j < raw.length && raw.charCodeAt(j) >= 0x20 && raw.charCodeAt(j) <= 0x2f) j++;
//         i = j;
//       }
//       continue;
//     }
//     if (ch === "\r") {
//       col = 0;
//       continue;
//     }
//     if (ch === "\n") {
//       out.push(line.join(""));
//       line = [];
//       col = 0;
//       continue;
//     }
//     if (ch === "\b") {
//       col = Math.max(0, col - 1);
//       line.length = Math.min(line.length, col);
//       continue;
//     }
//     if (ch < " " && ch !== "\t") continue;
//     put(ch);
//   }
//   out.push(line.join(""));
//   while (out.length > 0 && out[out.length - 1]!.trim() === "") out.pop();
//   return out.map(l => l.replace(/\s+$/, "")).join("\n");
// }
pub fn terminal_text(raw: &str) -> String {
    // The TS walks UTF-16 units, so a character above U+FFFF fills two columns, and a backspace takes one of them.
    let raw: Vec<u16> = raw.encode_utf16().collect();
    // A lone half of a surrogate pair has no text in Rust and is dropped (rule 8.5).
    let text = |units: &[u16]| char::decode_utf16(units.iter().copied()).filter_map(Result::ok).collect::<String>();
    let is = |at: usize, c: char| raw.get(at) == Some(&(c as u16));
    let mut out: Vec<String> = Vec::new();
    let mut line: Vec<u16> = Vec::new();
    let mut col: usize = 0;
    let mut i = 0;
    while i < raw.len() {
        let ch = raw[i];
        if ch == 0x1b {
            if is(i + 1, '[') {
                let mut j = i + 2;
                while j < raw.len() && !(0x40..=0x7e).contains(&raw[j]) {
                    j += 1;
                }
                i = j;
            } else if is(i + 1, ']') || is(i + 1, 'P') || is(i + 1, '_') || is(i + 1, '^') {
                let mut j = i + 2;
                while j < raw.len() && raw[j] != 0x07 && !(raw[j] == 0x1b && is(j + 1, '\\')) {
                    j += 1;
                }
                i = if raw.get(j) == Some(&0x07) { j } else { j + 1 };
            } else {
                let mut j = i + 1;
                while j < raw.len() && (0x20..=0x2f).contains(&raw[j]) {
                    j += 1;
                }
                i = j;
            }
            i += 1;
            continue;
        }
        match ch {
            0x0d => col = 0,
            0x0a => {
                out.push(text(&line));
                line.clear();
                col = 0;
            }
            0x08 => {
                col = col.saturating_sub(1);
                line.truncate(col);
            }
            _ if ch < 0x20 && ch != 0x09 => {}
            _ => {
                if col < line.len() {
                    line[col] = ch;
                } else {
                    line.push(ch);
                }
                col += 1;
            }
        }
        i += 1;
    }
    out.push(text(&line));
    while out.last().is_some_and(|l| js_trim(l).is_empty()) {
        out.pop();
    }
    out.iter().map(|l| js_trim_end(l)).collect::<Vec<_>>().join("\n")
}

/// A reply block's name on the thread's record: the reply's message and where the block's fence starts in its text,
/// which stays put for as long as the reply does.
// run-block.ts:103
// export const runBlockKey = (messageId: string, offset: number): string => `${messageId}@${offset}`;
pub fn run_block_key(message_id: &str, offset: usize) -> String {
    format!("{message_id}@{offset}")
}

// run-block.ts:107
// in wsp_wire::types::run_block: RUN_MOVE_AFTER_MS

// The words a run's block draws, one table for the app and the tests that read it.
// run-block.ts:110
// export const RUN_WORDS = {
//   run: "Run",
//   runAgain: "Run again",
//   running: "Running",
//   move: "Move to terminal",
//   send: "Send output to the agent",
//   openTab: "Open the tab",
// } as const;
// in wsp_wire::types::run_block: RUN_WORDS

/// How a run ended, in the line under its output.
// run-block.ts:120
// export function runEndedWords(run: { state: "exited" | "moved" | "lost" | "running"; exitCode?: number; signal?: number }): string {
//   if (run.state === "moved") return "Moved to a terminal tab";
//   if (run.state === "lost") return "Ended unseen: its terminal went away first";
//   if (run.state === "running") return RUN_WORDS.running;
//   if (run.signal !== undefined) return `Stopped by signal ${run.signal}`;
//   return `Exited ${run.exitCode ?? 0}`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Exited,
    Moved,
    Lost,
    Running,
}

/// How a run ended, as `runEndedWords` and `runOutputMessage` read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunEnd {
    pub state: RunState,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
}

pub fn run_ended_words(run: &RunEnd) -> String {
    match run.state {
        RunState::Moved => "Moved to a terminal tab".to_string(),
        RunState::Lost => "Ended unseen: its terminal went away first".to_string(),
        RunState::Running => "Running".to_string(),
        RunState::Exited => match run.signal {
            Some(signal) => format!("Stopped by signal {signal}"),
            None => format!("Exited {}", run.exit_code.unwrap_or(0)),
        },
    }
}

// run-block.ts:129
// in wsp_wire::types::run_block: RUN_SENT_LINES

/// What Send output to the agent sends into the thread as the person's message: the command, how it ended and the
/// tail of what it printed.
// run-block.ts:133
// export function runOutputMessage(command: string, run: { state: "exited" | "moved" | "lost" | "running"; exitCode?: number; signal?: number }, output: string): string {
//   const ended = run.state === "exited" && run.signal === undefined ? `it exited ${run.exitCode ?? 0}` : runEndedWords(run).toLowerCase();
//   const lines = output.split("\n");
//   const tail = lines.slice(-RUN_SENT_LINES).join("\n");
//   if (output.trim() === "") return `I ran \`${command}\` and ${ended}. It printed nothing.`;
//   // Longer than any run of backticks the output holds, so no line of it closes the fence and reads as the person's.
//   const longest = Math.max(0, ...[...tail.matchAll(/`+/g)].map(m => m[0].length));
//   const fence = "`".repeat(Math.max(3, longest + 1));
//   return `I ran \`${command}\` and ${ended}. The last lines it printed:\n\n${fence}text\n${tail}\n${fence}`;
// }
pub fn run_output_message(command: &str, run: &RunEnd, output: &str) -> String {
    let ended = match (run.state, run.signal) {
        (RunState::Exited, None) => format!("it exited {}", run.exit_code.unwrap_or(0)),
        _ => run_ended_words(run).to_lowercase(),
    };
    if js_trim(output).is_empty() {
        return format!("I ran `{command}` and {ended}. It printed nothing.");
    }
    let lines: Vec<&str> = output.split('\n').collect();
    let tail = lines[lines.len().saturating_sub(RUN_SENT_LINES as usize)..].join("\n");
    let longest = tail.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(3.max(longest + 1));
    format!("I ran `{command}` and {ended}. The last lines it printed:\n\n{fence}text\n{tail}\n{fence}")
}
