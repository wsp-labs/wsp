// SPDX-License-Identifier: AGPL-3.0-only
//! The Changes pane's words as today's app says them (`diffs/words.ts`, `diffs/model.ts`, `diffs/DiffSurface.tsx`,
//! `actions/format.ts`, `protocol/src/changes.ts`, `protocol/src/words/image.ts`).

pub const SEND_TO_THREAD: &str = "Send to thread";

pub const COMMIT: &str = "Commit";
pub const COMMIT_FIELD: &str = "Commit message";
pub const DRAFTING: &str = "Drafting a message";
pub const CANCEL: &str = "Cancel";
pub const AGENT_WORKING: &str = "The agent is still working";

pub fn commit_tick(file: &str) -> String {
    format!("Commit {file}")
}

pub fn committed(subject: &str) -> String {
    format!("Committed {subject}")
}

/// `DRAFT_NOTES`: why the message was left for the person to write.
pub const NO_DRAFTING_AGENT: &str = "No agent here drafts commit messages; write it yourself.";
pub const NO_DRAFT: &str = "The agent did not draft a message; write it yourself.";

pub fn discard_control(file: &str) -> String {
    format!("Discard changes to {file}")
}

pub fn discard_title(file: &str) -> String {
    format!("Discard changes to {file}?")
}

pub const DISCARD_NOTE: &str = "This cannot be undone.";

pub fn discard_deletes(file: &str) -> String {
    format!("No commit has {file}, so discarding deletes it. This cannot be undone.")
}

pub const DISCARD: &str = "Discard";

pub fn edit(file: &str) -> String {
    format!("Edit {file}")
}

pub fn save(file: &str) -> String {
    format!("Save {file}")
}

pub fn stop_editing(file: &str) -> String {
    format!("Stop editing {file}")
}

pub const LINE_ENDINGS: &str = "This file's line endings are not edited here yet.";
/// A whole-file read with no text in it, as a mode change gives; today's pane has no line for it.
pub const NO_TEXT: &str = "git sent no text for this file, so it is not edited here.";

pub fn viewed_tick(file: &str) -> String {
    format!("Viewed {file}")
}

pub fn viewed_count(viewed: usize, of: usize) -> String {
    format!("{viewed} of {of} viewed")
}

pub fn changed_files(n: usize) -> String {
    format!("{n} changed file{}", if n == 1 { "" } else { "s" })
}

pub fn narrowed(shown: usize, all: usize) -> String {
    format!("{shown} of {all} files")
}

pub const SHOW_ALL: &str = "Show all";

pub const TURN_SCOPE: &str = "This turn";
pub const TURN_NOUN: &str = "changes in this turn";
pub const SNAPSHOT_GONE: &str = "The snapshot for this turn is gone";

/// `noDiffLine`: Show in Changes asked for a file the diff does not touch.
pub fn no_diff(file: &str, scope_noun: &str) -> String {
    format!("{file} has no {scope_noun}")
}

pub fn empty(scope_noun: &str, folder: &str) -> String {
    format!("No {scope_noun} at {folder}.")
}

/// `REPO_STATE_WORDS.none.pane`.
pub const NOT_A_REPO: &str = "This folder is not inside a git repository, so there is nothing to diff.";

pub const NOT_RUNNING: &str = "The thread is not running.";
pub const READ_ON_COMPUTER: &str = "Changes are read on the thread's computer; wake it to read them.";
/// The launcher's reason on this computer (`NO_PROJECT_HERE` in `panes.ts`), where no thread is picked.
pub const PICK_A_PROJECT: &str = "Pick a project to review its changes.";

pub const SCOPE_MENU: &str = "Changes scope";
pub const REFRESH: &str = "Refresh changes";
pub const REFRESHING: &str = "Refreshing changes";
pub const EXPAND_ALL: &str = "Expand all files";
pub const COLLAPSE_ALL: &str = "Collapse all files";
pub const STACKED: &str = "Stacked view";
pub const SPLIT: &str = "Split view";
pub const PIN: &str = "Stay here when the agent moves";
pub const UNPIN: &str = "Follow the agent's folder again";
pub const EXPAND_FILE: &str = "Expand file";
pub const COLLAPSE_FILE: &str = "Collapse file";
pub const LOADING: &str = "Loading changes";

/// A turn card's head (`ChangedFilesCard`): the turn's own files, or the folder's where other threads worked in it.
pub fn turn_card_head(files: usize, folder: bool) -> String {
    match (files, folder) {
        (0, _) => "No files from this thread's edits".to_owned(),
        (n, true) => format!("{} in this folder", changed_files(n)),
        (n, false) => changed_files(n),
    }
}

pub const ALSO_CHANGED: &str = "Also changed in this folder";
pub const OPEN_DIFF: &str = "Open diff";
pub const OPEN_FULL_DIFF: &str = "Open the full diff";
pub const EXPAND_FOLDERS: &str = "Expand all folders";
pub const COLLAPSE_FOLDERS: &str = "Collapse all folders";
