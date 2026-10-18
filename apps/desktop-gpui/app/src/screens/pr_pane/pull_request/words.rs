// SPDX-License-Identifier: AGPL-3.0-only
//! The pane's words, as `pull-request/words.ts` says them.

use super::model::MergeMethod;

pub struct Hold {
    pub conflicts: &'static str,
    pub checks: &'static str,
    pub draft: &'static str,
    pub review: &'static str,
    pub unknown: &'static str,
    pub ready: &'static str,
}

pub struct Cut {
    pub commits: &'static str,
    pub reviews: &'static str,
    pub threads: &'static str,
    pub comments: &'static str,
}

pub struct Words {
    pub fix: &'static str,
    pub merge: &'static str,
    pub merge_when_checks_pass: &'static str,
    pub send_all: &'static str,
    pub refresh: &'static str,
    pub status: &'static str,
    pub activity: &'static str,
    pub draft_review: &'static str,
    pub conversation: &'static str,
    pub commits: &'static str,
    pub files: &'static str,
    pub show_more: &'static str,
    pub show_less: &'static str,
    pub deleted_account: &'static str,
    pub cut: Cut,
    pub left_notice: &'static str,
    pub show: &'static str,
    pub hide: &'static str,
    pub review_asked_of: &'static str,
    pub review_required: &'static str,
    pub hold: Hold,
    pub merged_by: &'static str,
    pub reviewed: &'static str,
    pub commented: &'static str,
    pub commented_on_line: &'static str,
    pub no_commits: &'static str,
    pub no_files: &'static str,
    pub no_diff: &'static str,
    pub no_diff_read: &'static str,
    pub diff_cut: &'static str,
    pub expand_folders: &'static str,
    pub collapse_folders: &'static str,
    pub open_in_changes: &'static str,
    pub open_on_github: &'static str,
    pub checks: &'static str,
    pub review: &'static str,
    pub comments: &'static str,
    pub conflicts: &'static str,
    pub merged: &'static str,
    pub closed_not_merged: &'static str,
    pub no_description: &'static str,
    pub quote_reply: &'static str,
    pub leave_comment: &'static str,
    pub add_reaction: &'static str,
    pub review_with_agent: &'static str,
}

pub const PR_WORDS: Words = Words {
    fix: "Ask your agent to fix",
    merge: "Merge",
    merge_when_checks_pass: "Merge when checks pass",
    send_all: "Send to agent",
    refresh: "Refresh",
    status: "Status",
    activity: "Activity",
    draft_review: "Draft review",
    conversation: "Conversation",
    commits: "Commits",
    files: "Files",
    show_more: "Show more",
    show_less: "Show less",
    deleted_account: "a deleted account",
    cut: Cut {
        commits: "Showing the first 100 commits",
        reviews: "Showing the latest 100 reviews",
        threads: "Showing the latest 100 review threads",
        comments: "Showing the latest 100 comments",
    },
    left_notice: "left a notice",
    show: "Show",
    hide: "Hide",
    review_asked_of: "Review asked of",
    review_required: "Review required",
    hold: Hold {
        conflicts: "Held until the conflicts are fixed",
        checks: "Held until the checks pass",
        draft: "Held while it is a draft",
        review: "Held until a review approves it",
        unknown: "GitHub is still working out whether it merges",
        ready: "Ready to merge",
    },
    merged_by: "Merged by",
    reviewed: "reviewed",
    commented: "commented",
    commented_on_line: "commented on a line",
    no_commits: "No commits yet",
    no_files: "No files changed",
    no_diff: "GitHub sent no diff for this file",
    no_diff_read: "This host does not read a pull request's diff",
    diff_cut: "The diff was cut at 2 MB before this file",
    expand_folders: "Expand all folders",
    collapse_folders: "Collapse all folders",
    open_in_changes: "Open in Changes",
    open_on_github: "Open on GitHub",
    checks: "Checks",
    review: "Review",
    comments: "Comments",
    conflicts: "Conflicts",
    merged: "Merged",
    closed_not_merged: "Closed, not merged",
    no_description: "No description.",
    quote_reply: "Quote reply",
    leave_comment: "Leave a comment",
    add_reaction: "Add a reaction",
    review_with_agent: "Review with an agent",
};

pub fn update(base: &str) -> String {
    format!("Update from {base}")
}

pub fn row(n: u64) -> String {
    format!("Pull request #{n}")
}

pub fn number(n: u64) -> String {
    format!("#{n}")
}

pub fn send_to_agent(agent: &str) -> String {
    format!("Send to {agent}")
}

pub fn opened(ago: &str) -> String {
    format!("opened {ago}")
}

pub fn sent(ago: &str) -> String {
    format!("Sent {ago}")
}

pub fn unresolved(n: usize, word: &str) -> String {
    format!("{n} {word}")
}

pub fn landed_as(base: &str) -> String {
    format!("Landed on {base} as")
}

pub fn method_word(method: MergeMethod) -> &'static str {
    match method {
        MergeMethod::Merge => "Merge commit",
        MergeMethod::Squash => "Squash and merge",
        MergeMethod::Rebase => "Rebase and merge",
    }
}

pub fn merges_when_checks_pass(method: MergeMethod) -> String {
    let how = match method {
        MergeMethod::Squash => "by squash",
        MergeMethod::Rebase => "by rebase",
        MergeMethod::Merge => "with a merge commit",
    };
    format!("Merges {how} when checks pass")
}

pub fn failed_after(workflow: Option<&str>, took: &str) -> String {
    match workflow {
        Some(w) => format!("{w}, failed after {took}"),
        None => format!("failed after {took}"),
    }
}

pub fn running_for(took: &str) -> String {
    format!("running {took}")
}

pub fn pushed(n: usize) -> String {
    if n == 1 {
        "pushed a commit".into()
    } else {
        format!("pushed {n} commits")
    }
}

pub fn files(n: usize) -> String {
    format!("{n} {}", if n == 1 { "file" } else { "files" })
}

pub fn behind(n: u64) -> String {
    format!("{n} {} behind", if n == 1 { "commit" } else { "commits" })
}

/// `START_WORDS.fromIssue` and `fromPullRequest`: the head's source link.
pub fn from_issue(number: u64, title: &str) -> String {
    format!("From issue #{number} {}", title.split_whitespace().collect::<Vec<_>>().join(" "))
}

pub fn from_pull_request(number: u64) -> String {
    format!("From pull request #{number}")
}

/// What the toasts say, as `wsp fix`, `wsp update` and `wsp merge` print it (`packages/protocol/src/pull-request.ts`).
pub fn fix_asked(workspace: &str, agent: &str, check: &str) -> String {
    format!("{workspace}: asked {agent} to fix {check}")
}

pub fn fix_conflicts(workspace: &str, agent: &str, base: &str) -> String {
    format!("{workspace}: asked {agent} to fix the conflicts with {base}")
}

pub fn fix_nothing(workspace: &str, base: &str) -> String {
    format!("{workspace}: updated from {base}, nothing to fix")
}

pub fn merged_line(workspace: &str, number: u64, method: &str, merged: bool) -> String {
    if merged {
        format!("{workspace}: merged #{number} ({method})")
    } else {
        format!("{workspace}: #{number} merges when its checks pass")
    }
}

pub fn updated_line(workspace: &str, base: &str, commits: u64) -> String {
    if commits == 0 {
        return format!("{workspace}: already has everything in {base}");
    }
    format!("{workspace}: updated from {base}, {commits} commit{}", if commits == 1 { "" } else { "s" })
}

pub fn update_conflicts(workspace: &str, base: &str, files: &[String]) -> String {
    format!("{workspace}: conflicts with {base} in {}", files.join(", "))
}
