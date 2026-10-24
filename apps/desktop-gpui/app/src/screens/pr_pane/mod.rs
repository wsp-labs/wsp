// SPDX-License-Identifier: AGPL-3.0-only
//! The right panel's Changes, Pull request and Files panes as today's app draws them (`apps/web/src/diffs/`,
//! `pull-request/`, `files/`, `components/diffs/`, `components/files/`), with the code view they share in [`diff`] and
//! the workspace's daemon link the Changes and Files panes read git and files over in [`daemon`].

pub mod changes;
pub mod comments;
pub mod daemon;
pub mod diff;
pub mod files;
pub mod pull_request;

use std::sync::OnceLock;

/// The state a capture opens a pane straight into, such as `commit` or `discard:src/added.ts`, from `WSP_GPUI_SHOT`,
/// which only the capture scripts set.
pub fn shot() -> Option<&'static str> {
    static SHOT: OnceLock<Option<String>> = OnceLock::new();
    SHOT.get_or_init(|| std::env::var("WSP_GPUI_SHOT").ok().filter(|s| !s.is_empty())).as_deref()
}
