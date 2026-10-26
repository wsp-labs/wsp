// SPDX-License-Identifier: AGPL-3.0-only
//! What a press on a changed file or Open diff opens: the turn's range in Changes, or the file in Files.

use gpui_kit::{prelude::*, App};

use crate::local_store::LocalStore;
use crate::screens::pr_pane::changes::model::{self as changes_model, TurnRange};
use crate::thread_view::ThreadView;
use crate::transcript::RowKind;

/// What a press on a changed file or Open diff opens.
#[derive(Clone, Debug, PartialEq)]
pub enum Opens {
    /// The turn's range in the Changes pane, for the workspace.
    Turn {
        workspace_id: String,
        range: TurnRange,
    },
    /// The file at the line in the Files pane, by its whole path.
    File {
        workspace_id: String,
        path: String,
        line: Option<usize>,
    },
    Nothing,
}

impl Opens {
    pub fn run(self, cx: &mut App) {
        match self {
            Opens::Turn { workspace_id, range } => {
                if cx.try_global::<LocalStore>().is_some() {
                    if let Err(e) = cx.update_global(|store: &mut LocalStore, _| changes_model::open_turn(store, &workspace_id, range)) {
                        eprintln!("desktop-gpui: changes: keeping the turn: {e}");
                    }
                }
                super::super::open_pane(crate::screens::Pane::Changes, None, cx);
            }
            Opens::File { workspace_id, path, line } => crate::screens::pr_pane::files::open_file(&workspace_id, &path, line, cx),
            Opens::Nothing => {}
        }
    }
}

/// The folder the thread worked in, else the workspace's own (`cwd ?? threadFolderOf(workspaceId)`: the copy's folder,
/// else the project's; the port keeps no chosen folder and no daemon root).
fn thread_folder(view: &ThreadView, workspace_id: &str, cx: &App) -> Option<String> {
    if let Some(cwd) = view.transcript().cwd() {
        return Some(cwd.to_owned());
    }
    let sidebar = crate::screens::terminal::workspaces_sidebar(cx)?;
    let around = sidebar.read(cx).around();
    let record = around.workspaces.get(workspace_id)?;
    record["folder"].as_str().or(record["project"]["path"].as_str()).map(str::to_owned)
}

/// `openNamedFile`: a path a message or a call named, read against the thread's folder, in Files at `line`.
pub fn named_file(view: &ThreadView, named: &str, line: Option<u64>, cx: &App) -> Opens {
    let Some(workspace_id) = view.followed_workspace() else { return Opens::Nothing };
    let Some(cwd) = thread_folder(view, workspace_id, cx) else { return Opens::Nothing };
    let path = crate::screens::terminal::links::resolve_path(named, &cwd);
    Opens::File { workspace_id: workspace_id.to_owned(), path, line: line.map(|l| l as usize) }
}

/// `onOpenTurnDiff`: the turn's range in Changes at the file `named`, narrowed to the list of the card it is in; where
/// the turn recorded no change to it, the file in Files at `line`.
pub fn turn_diff(view: &ThreadView, turn: &str, named: Option<&str>, line: Option<u64>, cx: &App) -> Opens {
    let (Some(workspace_id), Some(thread_id)) = (view.followed_workspace(), view.thread()) else { return Opens::Nothing };
    let Some(cwd) = thread_folder(view, workspace_id, cx) else { return Opens::Nothing };
    let changes = view.rows().iter().find_map(|r| match &r.kind {
        RowKind::ChangedFiles { files, others, range, .. } if r.turn.as_deref() == Some(turn) => Some((files, others, range)),
        _ => None,
    });
    let Some((files, others, (from, to))) = changes else { return named.map_or(Opens::Nothing, |n| named_file(view, n, line, cx)) };
    let (own, others): (Vec<String>, Vec<String>) =
        (files.iter().map(|f| f.path.clone()).collect(), others.iter().map(|f| f.path.clone()).collect());
    let all: Vec<String> = own.iter().chain(&others).cloned().collect();
    let path = match named {
        Some(named) => match changes_model::changed_path_of(&all, named) {
            Some(path) => Some(path),
            None => return named_file(view, named, line, cx),
        },
        None => None,
    };
    let only = changes_model::only_of(&own, (!others.is_empty()).then_some(others.as_slice()), path.as_deref());
    let range =
        TurnRange { thread_id: thread_id.to_owned(), turn_id: turn.to_owned(), cwd, from: from.clone(), to: to.clone(), path, only };
    Opens::Turn { workspace_id: workspace_id.to_owned(), range }
}
