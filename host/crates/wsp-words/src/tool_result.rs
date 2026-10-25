// SPDX-License-Identifier: AGPL-3.0-only
//! An edit's hunks as a tool result carries them: a whole file as one hunk, a unified diff's hunks, and the lines that
//! fit a count of characters. Port of packages/protocol/src/tool-result.ts at a3b870a6e. See spec/shell_and_text.md.

use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::session_events::{FilePatch, PatchHunk};

use crate::js::{to_number, utf16};

/// A file written whole or deleted, as the one hunk of its every line added or removed.
// tool-result.ts:7
// export function wholeFileHunk(content: string, sign: "+" | "-"): PatchHunk {
//   const lines = content === "" ? [] : content.replace(/\n$/, "").split("\n");
//   const count = lines.length;
//   const at = count === 0 ? 0 : 1;
//   return sign === "+"
//     ? { oldStart: 0, oldLines: 0, newStart: at, newLines: count, lines: lines.map(l => `+${l}`) }
//     : { oldStart: at, oldLines: count, newStart: 0, newLines: 0, lines: lines.map(l => `-${l}`) };
// }
/// `added` true is the TS sign "+", false is "-".
pub fn whole_file_hunk(content: &str, added: bool) -> PatchHunk {
    let lines: Vec<&str> = if content.is_empty() { Vec::new() } else { content.strip_suffix('\n').unwrap_or(content).split('\n').collect() };
    let count = JsInt::from(lines.len() as i64);
    let at = JsInt::from(i64::from(!lines.is_empty()));
    let zero = JsInt::from(0);
    if added {
        PatchHunk { old_start: zero, old_lines: zero, new_start: at, new_lines: count, lines: lines.iter().map(|l| format!("+{l}")).collect() }
    } else {
        PatchHunk { old_start: at, old_lines: count, new_start: zero, new_lines: zero, lines: lines.iter().map(|l| format!("-{l}")).collect() }
    }
}

// A unified diff's hunk head; a count of one may be left out.
// tool-result.ts:17
// export const HUNK_HEAD = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;
// HUNK_HEAD is a regex; ported by hand in unified_hunks (rule 8.7).

/// The leading run of ASCII digits of `s`, and the rest, where there is at least one.
fn digits(s: &str) -> Option<(&str, &str)> {
    let end = s.bytes().take_while(u8::is_ascii_digit).count();
    (end > 0).then(|| s.split_at(end))
}

/// An optional `,<digits>` count after a start: the count, or 1 where it is left out (HUNK_HEAD's `(?:,(\d+))?`).
fn count(s: &str) -> Option<(&str, &str)> {
    match s.strip_prefix(',') {
        Some(rest) => digits(rest),
        None => Some(("1", s)),
    }
}

/// `HUNK_HEAD` matched at the start of a line: the four numbers, old start, old lines, new start, new lines.
fn hunk_head(line: &str) -> Option<[JsInt; 4]> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_start, rest) = digits(rest)?;
    let (old_lines, rest) = count(rest)?;
    let (new_start, rest) = digits(rest.strip_prefix(" +")?)?;
    let (new_lines, rest) = count(rest)?;
    rest.starts_with(" @@").then(|| [old_start, old_lines, new_start, new_lines].map(|n| JsInt(JsNumber(to_number(n)))))
}

/// The hunks of a unified diff; a line before the first hunk head is a header and is skipped.
// tool-result.ts:20
// export function unifiedHunks(diff: string): PatchHunk[] {
//   const hunks: PatchHunk[] = [];
//   for (const line of diff.replace(/\n$/, "").split("\n")) {
//     const head = HUNK_HEAD.exec(line);
//     if (head !== null) hunks.push({ oldStart: Number(head[1]), oldLines: Number(head[2] ?? 1), newStart: Number(head[3]), newLines: Number(head[4] ?? 1), lines: [] });
//     else hunks.at(-1)?.lines.push(line);
//   }
//   return hunks;
// }
pub fn unified_hunks(diff: &str) -> Vec<PatchHunk> {
    let mut hunks: Vec<PatchHunk> = Vec::new();
    for line in diff.strip_suffix('\n').unwrap_or(diff).split('\n') {
        match hunk_head(line) {
            Some([old_start, old_lines, new_start, new_lines]) => hunks.push(PatchHunk { old_start, old_lines, new_start, new_lines, lines: Vec::new() }),
            None => {
                if let Some(hunk) = hunks.last_mut() {
                    hunk.lines.push(line.to_string());
                }
            }
        }
    }
    hunks
}

/// The hunks of a patch that fit within `chars` characters of lines, in order, by the event's own fields: the hunk
/// that crosses the cap keeps its lines before it, counted again, and patchCut says lines were left out. A file none
/// of whose lines fit is left out with them.
// tool-result.ts:33
// export function keptPatch(patch: readonly FilePatch[], chars: number): { patch: FilePatch[]; patchCut?: true } {
//   const kept: FilePatch[] = [];
//   let used = 0;
//   for (const file of patch) {
//     const hunks: PatchHunk[] = [];
//     for (const hunk of file.hunks) {
//       const size = hunk.lines.reduce((n, l) => n + l.length, 0);
//       if (used + size > chars) {
//         const lines: string[] = [];
//         for (const l of hunk.lines) {
//           if ((used += l.length) > chars) break;
//           lines.push(l);
//         }
//         const counted = (other: "+" | "-") => lines.filter(l => !l.startsWith(other) && !l.startsWith("\\")).length;
//         if (lines.length > 0) hunks.push({ ...hunk, oldLines: counted("+"), newLines: counted("-"), lines });
//         return { patch: hunks.length > 0 ? [...kept, { ...file, hunks }] : kept, patchCut: true };
//       }
//       used += size;
//       hunks.push(hunk);
//     }
//     kept.push(file);
//   }
//   return { patch: kept };
// }
/// What `kept_patch` answers: the files kept, and `patchCut: true` only where something was cut.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptPatch {
    pub patch: Vec<FilePatch>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub patch_cut: bool,
}

/// `chars` counts UTF-16 units of the hunks' lines (rule 8.4).
pub fn kept_patch(patch: &[FilePatch], chars: usize) -> KeptPatch {
    let mut kept: Vec<FilePatch> = Vec::new();
    let mut used = 0;
    for file in patch {
        let mut hunks: Vec<PatchHunk> = Vec::new();
        for hunk in &file.hunks {
            let size: usize = hunk.lines.iter().map(|l| utf16::len(l)).sum();
            if used + size > chars {
                let mut lines: Vec<String> = Vec::new();
                for line in &hunk.lines {
                    used += utf16::len(line);
                    if used > chars {
                        break;
                    }
                    lines.push(line.clone());
                }
                let counted = |other: char| lines.iter().filter(|l| !l.starts_with(other) && !l.starts_with('\\')).count();
                if !lines.is_empty() {
                    let (old_lines, new_lines) = (JsInt::from(counted('+') as i64), JsInt::from(counted('-') as i64));
                    hunks.push(PatchHunk { old_lines, new_lines, lines, ..hunk.clone() });
                }
                let patch = if hunks.is_empty() {
                    kept
                } else {
                    kept.push(FilePatch { hunks, ..file.clone() });
                    kept
                };
                return KeptPatch { patch, patch_cut: true };
            }
            used += size;
            hunks.push(hunk.clone());
        }
        kept.push(FilePatch { hunks, ..file.clone() });
    }
    KeptPatch { patch: kept, patch_cut: false }
}
