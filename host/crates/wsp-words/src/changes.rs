// SPDX-License-Identifier: AGPL-3.0-only
//! What a workspace's checkout holds and the lines a commit, a discard or a push prints. Port of the functions of packages/protocol/src/changes.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::changes. See spec/changes.md.

use wsp_wire::types::changes::Checkout;

// A copy's checkout as the host last read it: the branch git is on, its counts against its upstream or the default
// branch, and how many files differ from HEAD. editsUnread is a stopped copy whose branch was read off its files and
// whose edits were not; countsUnknown is one whose history was too long to walk. readAt is when git answered, so a
// computer whose link went down shows the last fact for what it is.
// changes.ts:11
// in wsp_wire::types::changes: Checkout

// changes.ts:24
// in wsp_wire::types::changes: Checkout

/// One word table for the counts a checkout carries on the composer's checkout row.
// changes.ts:27
// export const CHECKOUT_WORDS = {
//   changed: (n: number): string => `${n} changed`,
//   unread: "changes not read",
// } as const;
/// The TS CHECKOUT_WORDS.
pub mod checkout_words {
    pub const UNREAD: &str = "changes not read";
    pub fn changed(n: f64) -> String {
        todo!()
    }
}

/// The count that follows a branch on the composer's row: what the checkout holds uncommitted, a zero left out, and a
/// copy whose edits were not read saying so in place of a count it does not have. How far the branch is ahead of or
/// behind anything is not said there; Update from main in the pane is how a person acts on behind.
// changes.ts:35
// export function checkoutCounts(c: Pick<Checkout, "changed" | "editsUnread">): string[] {
//   if (c.editsUnread === true) return [CHECKOUT_WORDS.unread];
//   return c.changed > 0 ? [CHECKOUT_WORDS.changed(c.changed)] : [];
// }
pub fn checkout_counts(c: &Checkout) -> Vec<String> {
    todo!()
}

// What workspaces.checkout answers: the fact as the host holds it, absent where git has never answered.
// changes.ts:41
// in wsp_wire::types::changes: CheckoutReply

// changes.ts:42
// in wsp_wire::types::changes: CheckoutReply

// A drafted message, or none with the one line that says why.
// changes.ts:45
// in wsp_wire::types::changes: CommitDraft

// changes.ts:46
// in wsp_wire::types::changes: CommitDraft

// Every file a person marked viewed in a workspace, by path, against the blob id its contents had then.
// changes.ts:49
// in wsp_wire::types::changes: ViewedMarks

// changes.ts:50
// in wsp_wire::types::changes: ViewedMarks

// The most of a diff a draft carries: past it the question costs more than the answer is worth.
// changes.ts:53
// in wsp_wire::types::changes: DRAFT_DIFF_MAX_BYTES

/// A diff cut to the bytes given at its last line end inside them, as the daemon cuts a patch: never through a letter.
// changes.ts:56
// export function cutDiff(text: string, max: number = DRAFT_DIFF_MAX_BYTES): string {
//   const bytes = new TextEncoder().encode(text);
//   if (bytes.length <= max) return text;
//   const head = bytes.subarray(0, max);
//   const nl = head.lastIndexOf(10);
//   return new TextDecoder().decode(nl > 0 ? head.subarray(0, nl + 1) : head).replace(/\uFFFD+$/, "");
// }
pub fn cut_diff(text: &str, max: Option<f64>) -> String {
    todo!()
}

/// How much of the task a draft carries: the words a change is for are at the top of a brief.
// changes.ts:65
const DRAFT_TASK_MAX: i64 = 2000;

/// The one question every harness is asked for a commit message, on its own CLI with no thread and no tool, from the
/// diff and the task the workspace's thread was opened with.
// changes.ts:69
// export function draftPrompt(diff: string, task?: string): string {
//   const said = task?.trim();
//   return [
//     "Write a git commit message for the changes in the diff below.",
//     "The first line is the subject: under 72 characters, sentence case, no full stop, saying what the change does.",
//     "Then one blank line, then two to five lines saying why the change was made, in plain words.",
//     "No em dashes, no bullet points, no markdown and no quotes around the message.",
//     "Answer with the message alone and nothing else.",
//     ...(said === undefined || said === "" ? [] : ["", "The task the agent was given:", said.length <= DRAFT_TASK_MAX ? said : said.slice(0, DRAFT_TASK_MAX)]),
//     "",
//     "The diff:",
//     diff,
//   ].join("\n");
// }
pub fn draft_prompt(diff: &str, task: Option<&str>) -> String {
    todo!()
}

/// The longest subject a drafted message keeps; a longer one is cut to the words that fit.
// changes.ts:85
const SUBJECT_MAX: i64 = 72;

/// A harness's answer to draftPrompt as a commit message, or none when it answered nothing: a fence or quotes around
/// the message come off, an em dash becomes a comma, the subject is cut to the words that fit, and the body follows
/// one blank line.
// changes.ts:90
// export function commitMessage(answer: string): string | null {
//   let text = answer.trim().replace(/^```[^\n]*\n([\s\S]*?)\n?```$/, "$1").trim();
//   const quoted = /^(["'\u201C\u2018])([\s\S]*)(["'\u201D\u2019])$/.exec(text);
//   if (quoted !== null) text = quoted[2]!.trim();
//   text = text.replace(/\s*\u2014\s*/g, ", ");
//   const lines = text.split(/\r?\n/);
//   const at = lines.findIndex(line => line.trim() !== "");
//   if (at < 0) return null;
//   const first = lines[at]!.trim();
//   const subject = first.length <= SUBJECT_MAX ? first : (wordsWithin(first, SUBJECT_MAX) ?? first.slice(0, SUBJECT_MAX));
//   const body = lines.slice(at + 1).join("\n").trim();
//   return body === "" ? subject : `${subject}\n\n${body}`;
// }
pub fn commit_message(answer: &str) -> Option<String> {
    todo!()
}

/// What `wsp discard` says once a file is back as HEAD has it.
// changes.ts:105
// export function discardedLine(workspace: string, path: string): string {
//   return `${workspace}: ${path} discarded`;
// }
pub fn discarded_line(workspace: &str, path: &str) -> String {
    todo!()
}

/// What a commit of every change says when the checkout has none.
// changes.ts:110
// export function cleanCheckoutLine(workspace: string): string {
//   return `${workspace}: nothing to commit, the checkout has no changes`;
// }
pub fn clean_checkout_line(workspace: &str) -> String {
    todo!()
}

/// What `wsp commit` says once the commit stands: the short id and the subject.
// changes.ts:115
// export function committedLine(workspace: string, commit: { oid: string; subject: string; filesChanged: number }): string {
//   return `${workspace}: committed ${commit.oid.slice(0, 7)} ${commit.subject} (${commit.filesChanged} file${commit.filesChanged === 1 ? "" : "s"})`;
// }
/// The commit fields `committedLine` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MadeCommit {
    pub oid: String,
    pub subject: String,
    pub files_changed: u64,
}

pub fn committed_line(workspace: &str, commit: &MadeCommit) -> String {
    todo!()
}

// Why a draft came back empty, as the one line the commit box shows under it.
// changes.ts:120
// in wsp_wire::types::changes: DRAFT_NOTES

