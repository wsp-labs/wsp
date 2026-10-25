// SPDX-License-Identifier: AGPL-3.0-only
//! A lead thread's tree of sub-threads as a row reads it: what each branch holds against the lead's, and the prompt that merges one in. Port of the functions of packages/protocol/src/tree.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::tree. See spec/tree.md.

use wsp_wire::types::tree::MergeInResult;

use crate::js::{number_string, utf16};
use crate::words::computer::thread_word;

// tree.ts:8
// private in the TS: port it as a private helper
// const text = z.string();

// tree.ts:9
// private in the TS: port it as a private helper
// const count = z.number().int().nonnegative();

// The branch a copy was put on as the remote holds it, and the commit it now stands at.
// tree.ts:12
// in wsp_wire::types::tree: GitStartOnReply

// tree.ts:13
// in wsp_wire::types::tree: GitStartOnReply

// How far a head is from a base on the git host: not pushed where the host lacks the head, else the commits each
// has that the other lacks and the host's word for the two.
// tree.ts:17
// in wsp_wire::types::tree: GitBranchCompareReply

// tree.ts:18
// in wsp_wire::types::tree: GitBranchCompareReply

// Merged with the commits it brought, the commit it left and the child's commit it took, or nothing merged and the
// files that conflict.
// tree.ts:22
// in wsp_wire::types::tree: GitMergeInReply

// tree.ts:23
// in wsp_wire::types::tree: GitMergeInReply

// What a child's record keeps of the tree: its merge into the lead with the child's commit it took, the files a
// merge stopped on, and why its last push was refused, cleared by the next push that lands.
// tree.ts:27
// in wsp_wire::types::tree: TreeRecord

// tree.ts:32
// in wsp_wire::types::tree: TreeRecord

// One child as its card reads it: its branch, whether the git host holds that branch, how far it is from what
// the lead's copy holds of it, and what its record keeps. That is the lead's branch as the git host holds it until a
// merge, and the child's commit the merge took after one, since the lead's copy holds that before any push. The
// counts are absent where nothing could count them.
// tree.ts:38
// in wsp_wire::types::tree: TreeChild

// tree.ts:47
// in wsp_wire::types::tree: TreeChild

// A lead's children as the host last read them, against the lead's branch as the remote holds it; carried on the
// status of a workspace that has children, and on no other.
// tree.ts:51
// in wsp_wire::types::tree: TreeFact

// tree.ts:52
// in wsp_wire::types::tree: TreeFact

// What a merge into the lead answers: merged with the commits it brought, merged nothing, or the files it stopped on.
// tree.ts:55
// in wsp_wire::types::tree: MergeInResult

// tree.ts:65
// in wsp_wire::types::tree: MergeInResult

/// One word table for the tree, on a child's card, the command line and the fork's reply.
// tree.ts:68
// export const TREE_WORDS = {
//   aheadOf: (n: number, lead: string): string => `${n} ahead of ${lead}`,
//   notPushed: "not pushed",
//   notCounted: "not counted",
//   mergedIntoLead: "merged into lead",
//   conflictsIn: (n: number): string => `conflicts in ${n} ${n === 1 ? "file" : "files"}`,
// } as const;
/// The TS TREE_WORDS.
pub mod tree_words {
    use crate::js::number_string;

    pub const NOT_PUSHED: &str = "not pushed";
    pub const NOT_COUNTED: &str = "not counted";
    pub const MERGED_INTO_LEAD: &str = "merged into lead";
    pub fn ahead_of(n: f64, lead: &str) -> String {
        format!("{} ahead of {lead}", number_string(n))
    }
    pub fn conflicts_in(n: f64) -> String {
        format!("conflicts in {} {}", number_string(n), if n == 1.0 { "file" } else { "files" })
    }
}

/// The one line a fork says once its child's copy stands on the branch the lead pushed.
// tree.ts:77
// export function childStartedLine(child: string, branch: string): string {
//   return `${child} starts on ${branch} as pushed`;
// }
pub fn child_started_line(child: &str, branch: &str) -> String {
    format!("{child} starts on {branch} as pushed")
}

/// The line the lead's thread reads when a fork pushed its branch first, so the child starts from the lead's work.
// tree.ts:82
// export function pushedForChildLine(branch: string, child: string): string {
//   return `pushed ${branch} so ${child} starts from your work`;
// }
pub fn pushed_for_child_line(branch: &str, child: &str) -> String {
    format!("pushed {branch} so {child} starts from your work")
}

/// What a fork says of the changes the lead had not committed, which the push could not carry.
// tree.ts:87
// export function uncommittedStayed(n: number, lead: string): string {
//   return `${n} uncommitted ${n === 1 ? "change" : "changes"} in ${lead} did not travel`;
// }
pub fn uncommitted_stayed(n: f64, lead: &str) -> String {
    format!("{} uncommitted {} in {lead} did not travel", number_string(n), if n == 1.0 { "change" } else { "changes" })
}

/// Why a child could not be made on its lead's branch: the push or the start on it refused, in its own sentence.
// tree.ts:92
// export function forkNeedsPushLine(lead: string, why: string): string {
//   return `the child was not made, since ${lead}'s branch could not reach it: ${why}`;
// }
pub fn fork_needs_push_line(lead: &str, why: &str) -> String {
    format!("the child was not made, since {lead}'s branch could not reach it: {why}")
}

/// What `wsp merge in` says once the child's branch is in the lead's.
// tree.ts:97
// export function mergedInLine(lead: string, branch: string, child: string, commits: number): string {
//   return `${lead}: merged ${branch} from ${child}, ${commits} ${commits === 1 ? "commit" : "commits"}`;
// }
pub fn merged_in_line(lead: &str, branch: &str, child: &str, commits: f64) -> String {
    format!("{lead}: merged {branch} from {child}, {} {}", number_string(commits), if commits == 1.0 { "commit" } else { "commits" })
}

/// What it says where the merge stopped on conflicts and was taken back.
// tree.ts:102
// export function mergeConflictsLine(lead: string, branch: string, paths: readonly string[]): string {
//   return `${lead}: conflicts with ${branch} in ${paths.join(", ")}`;
// }
pub fn merge_conflicts_line(lead: &str, branch: &str, paths: &[impl AsRef<str>]) -> String {
    let paths: Vec<&str> = paths.iter().map(AsRef::as_ref).collect();
    format!("{lead}: conflicts with {branch} in {}", paths.join(", "))
}

/// What `wsp fix --child` says once the lead's agent is asked to merge the child.
// tree.ts:107
// export function fixMergeChildLine(lead: string, agent: string, child: string): string {
//   return `${lead}: asked ${agent} to merge ${child}`;
// }
pub fn fix_merge_child_line(lead: &str, agent: &str, child: &str) -> String {
    format!("{lead}: asked {agent} to merge {child}")
}

/// What a merge of a sub-thread adds when its machine holds commits no remote has: the merge took what was pushed.
// tree.ts:112
// export const notInMergeLine = (threadId: string, commits: number): string =>
//   `${threadId.slice(0, 8)} holds ${commits} ${commits === 1 ? "commit" : "commits"} not pushed on its machine; ${commits === 1 ? "it is" : "they are"} not in this merge.`;
pub fn not_in_merge_line(thread_id: &str, commits: f64) -> String {
    let (noun, verb) = if commits == 1.0 { ("commit", "it is") } else { ("commits", "they are") };
    format!("{} holds {} {noun} not pushed on its machine; {verb} not in this merge.", utf16::cut(thread_id, 8), number_string(commits))
}

/// The line a merge into the lead reads as, on the command line and in the tool's answer, and under it what a
/// sub-thread's machine holds that the merge could not take.
// tree.ts:117
// export function mergeInLine(done: MergeInResult, childThreadId?: string): string {
//   const line = !done.merged ? mergeConflictsLine(done.lead, done.branch, done.conflicts) : done.commits > 0 ? mergedInLine(done.lead, done.branch, done.child, done.commits) : nothingToMergeLine(done.lead, done.child);
//   return done.unpushed !== undefined && done.unpushed > 0 ? `${line}\n${notInMergeLine(childThreadId ?? done.child, done.unpushed)}` : line;
// }
pub fn merge_in_line(done: &MergeInResult, child_thread_id: Option<&str>) -> String {
    let line = if !done.merged {
        merge_conflicts_line(&done.lead, &done.branch, &done.conflicts)
    } else if done.commits.as_f64() > 0.0 {
        merged_in_line(&done.lead, &done.branch, &done.child, done.commits.as_f64())
    } else {
        nothing_to_merge_line(&done.lead, &done.child)
    };
    match done.unpushed {
        Some(unpushed) if unpushed.as_f64() > 0.0 => {
            format!("{line}\n{}", not_in_merge_line(child_thread_id.unwrap_or(&done.child), unpushed.as_f64()))
        }
        _ => line,
    }
}

/// What it says where the child has nothing the lead lacks.
// tree.ts:123
// export function nothingToMergeLine(lead: string, child: string): string {
//   return `${lead}: nothing to merge from ${child}`;
// }
pub fn nothing_to_merge_line(lead: &str, child: &str) -> String {
    format!("{lead}: nothing to merge from {child}")
}

/// Why a merge waits: a turn runs on the lead in a thread other than the one asking, and a merge would land under it.
// tree.ts:128
// export function leadBusyRefusal(lead: string, threadIds: readonly string[]): string {
//   const named = threadIds.map(threadWord).join(", ");
//   return `${lead} has a turn running in thread ${named}; merge once it ends, or from inside that turn`;
// }
pub fn lead_busy_refusal(lead: &str, thread_ids: &[impl AsRef<str>]) -> String {
    let named: Vec<String> = thread_ids.iter().map(|id| thread_word(id.as_ref())).collect();
    format!("{lead} has a turn running in thread {}; merge once it ends, or from inside that turn", named.join(", "))
}

/// Why a merge into a workspace is refused for one that is not its child.
// tree.ts:134
// export function notTheLeadsChildRefusal(child: string, lead: string): string {
//   return `${child} is not a child of ${lead}; a lead merges only its own children`;
// }
pub fn not_the_leads_child_refusal(child: &str, lead: &str) -> String {
    format!("{child} is not a child of {lead}; a lead merges only its own children")
}

/// Why a thread may merge only into the folder it works in.
// tree.ts:139
// export function mergeIntoOwnRefusal(threadId: string): string {
//   return `thread ${threadWord(threadId)} may merge a child only into its own folder`;
// }
pub fn merge_into_own_refusal(thread_id: &str) -> String {
    format!("thread {} may merge a child only into its own folder", thread_word(thread_id))
}

/// Why a child is not merged where its copy is on no branch at all.
// tree.ts:144
// export function childOnNoBranchRefusal(child: string): string {
//   return `${child} is on no branch, so there is nothing to merge from it`;
// }
pub fn child_on_no_branch_refusal(child: &str) -> String {
    format!("{child} is on no branch, so there is nothing to merge from it")
}

// tree.ts:149
// in wsp_wire::types::tree: FIX_CHECK_OR_CHILD

/// Why a merge needs a remote where the lead and the child sit on two computers.
// tree.ts:152
// export function noRemoteForTreeLine(project: string): string {
//   return `${project} has no remote, so a child on another computer cannot be merged; add a remote to the project to merge across computers`;
// }
pub fn no_remote_for_tree_line(project: &str) -> String {
    format!("{project} has no remote, so a child on another computer cannot be merged; add a remote to the project to merge across computers")
}

/// The row's note under a child whose push was refused, pointing at that computer's sign-in.
// tree.ts:157
// export function cannotPushFromLine(computer: string, why: string): string {
//   return `can't push from ${computer}: ${why}`;
// }
pub fn cannot_push_from_line(computer: &str, why: &str) -> String {
    format!("can't push from {computer}: {why}")
}

/// The message a lead's first thread is handed when a child's merge stopped on conflicts: fetch, merge with a merge
/// commit, resolve the files, test, commit.
// tree.ts:163
// export function mergeChildPrompt(o: { leadBranch: string; childBranch: string; remote: string; conflicts: readonly string[] }): string {
//   const files = o.conflicts.length > 0 ? `; resolve the conflicts in ${o.conflicts.join(", ")}` : "; resolve the conflicts";
//   return `Fetch ${o.remote} ${o.childBranch} and merge it into ${o.leadBranch} with a merge commit${files}; run the tests; commit.`;
// }
/// The TS object `mergeChildPrompt` reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeChild {
    pub lead_branch: String,
    pub child_branch: String,
    pub remote: String,
    pub conflicts: Vec<String>,
}

pub fn merge_child_prompt(o: &MergeChild) -> String {
    let files = if o.conflicts.is_empty() { "; resolve the conflicts".to_string() } else { format!("; resolve the conflicts in {}", o.conflicts.join(", ")) };
    format!("Fetch {} {} and merge it into {} with a merge commit{files}; run the tests; commit.", o.remote, o.child_branch, o.lead_branch)
}

