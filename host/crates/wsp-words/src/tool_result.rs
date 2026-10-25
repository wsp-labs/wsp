// SPDX-License-Identifier: AGPL-3.0-only
//! An edit's hunks as a tool result carries them: a whole file as one hunk, a unified diff's hunks, and the lines that
//! fit a count of characters. Port of packages/protocol/src/tool-result.ts at a3b870a6e. See spec/shell_and_text.md.

use wsp_wire::views::session_events::{FilePatch, PatchHunk};

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
    todo!()
}

// A unified diff's hunk head; a count of one may be left out.
// tool-result.ts:17
// export const HUNK_HEAD = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;
// HUNK_HEAD is a regex; ported by hand in unified_hunks (rule 8.7).

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
    todo!()
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
    todo!()
}

