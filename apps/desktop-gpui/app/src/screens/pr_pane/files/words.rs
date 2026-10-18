// SPDX-License-Identifier: AGPL-3.0-only
//! The Files pane's words, as `files/` and `components/files/` in the web app say them.

use crate::screens::usage::words::group_thousands;
use crate::ui::fmt_bytes;

/// `FILES_NOT_RUNNING`: the pane's sentence while no daemon lists the files.
pub const NOT_RUNNING: &str = "Files are read on the thread's computer; wake it to read them.";

pub const FOLLOW: &str = "Follow the agent's folder again";
pub const STAY: &str = "Stay here when the agent moves";
pub const NEW_THREAD_HERE: &str = "New thread in this folder";
pub const PICK_ROOT: &str = "Pick a browsable folder";

pub const REFRESH: &str = "Refresh files";
pub const REFRESHING: &str = "Refreshing…";
pub const SEARCH_FILES: &str = "Search files";

pub const RELOAD: &str = "Reload file";
pub const SHOW_SOURCE: &str = "Show markdown source";
pub const SHOW_RENDERED: &str = "Show rendered markdown";
pub const OPEN_IN_EDITOR: &str = "Open in editor";

/// `FILE_WORDS`, a row's right-click menu, and why a folder's first two are held.
pub const OPEN_FILE: &str = "Open file";
pub const SHOW_IN_CHANGES: &str = "Show in Changes";
pub const COPY_PATH: &str = "Copy path";
pub const FOLDER_OPENS_IN_TREE: &str = "A folder opens in the tree";
pub const ONLY_FILES_HAVE_CHANGES: &str = "Only a file has changes";

/// The note row under a folder the daemon cut at its cap.
pub fn more_entries(hidden: u64) -> String {
    format!("… {} more {} not shown", group_thousands(&hidden.to_string()), if hidden == 1 { "entry" } else { "entries" })
}

/// The note over a file the daemon read only the start of.
pub fn truncated(shown: u64, size: u64) -> String {
    format!("Showing the first {} of {}; the daemon caps reads at 2 MB.", fmt_bytes(shown), fmt_bytes(size))
}

pub fn binary(size: u64) -> String {
    format!("Binary file, {}. Nothing to preview.", fmt_bytes(size))
}

/// `FINDER_WORDS`.
pub mod finder {
    pub const FILES_LABEL: &str = "Find a file";
    pub const FILES_PLACEHOLDER: &str = "Find a file by name";
    pub const FILES_GROUP: &str = "Files";
    pub const FILES_NONE: &str = "No file matches.";
    pub const TEXT_LABEL: &str = "Search in files";
    pub const TEXT_PLACEHOLDER: &str = "Search the files for a word";
    pub const TEXT_GROUP: &str = "Lines";
    pub const TEXT_NONE: &str = "No line holds that.";
    pub const TYPE_WORD: &str = "Type a word to search the files.";
    pub const NOT_RUNNING: &str = "Open a running thread to search its files.";
    pub const MORE_FILES: &str = "More files match than one search reads; type more of the name.";
    pub const MORE_LINES: &str = "More lines match than the first 500; type more of the word.";
    pub const NAVIGATE: &str = "Navigate";
    pub const OPEN: &str = "Open";
    pub const CLOSE: &str = "Close";
}

/// `OPEN_WORDS` and `EDITOR_SSH_WORDS`.
pub mod open {
    pub const OPEN: &str = "Open";
    pub const CHOOSE: &str = "Choose an editor";
    pub fn open_in(editor: &str) -> String {
        format!("Open in {editor}")
    }
    pub const SSH_TITLE: &str = "Open in your editor over SSH";
    pub const SSH_SAYS: &str =
        "wsp adds one line to ~/.ssh/config. It covers only hosts named wsp-*, and nothing else in the file changes.";
    pub fn ssh_reach(computer: &str) -> String {
        let computer = if computer.is_empty() { "your computer" } else { computer };
        format!(
            "While your editor is connected, the workspace can reach {computer} through it: ports it forwards, files it asks to open, and your editor's git sign-in."
        )
    }
    pub const SSH_OFF: &str = "Remove it any time in Settings, General.";
    pub const SSH_ADD: &str = "Add to SSH config";
    pub const CANCEL: &str = "Cancel";
    pub const SMALL: &str = "An editor here takes about what one more thread does, or more.";
    /// `editorOpensHereLine`.
    pub fn opens_here(name: &str) -> String {
        format!("These files are on {name}, so they open here.")
    }
}
