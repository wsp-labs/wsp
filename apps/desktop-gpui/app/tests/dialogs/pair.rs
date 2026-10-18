// SPDX-License-Identifier: AGPL-3.0-only
//! The pair screen, a failed start, the confirm frame, Add a project and Export project's states for `dialogs_shots`,
//! each staged from fixture answers with no host behind it.

use std::rc::Rc;

use desktop_gpui::screens::dialogs::add_project::{AddProject, Folder, Listing};
use desktop_gpui::screens::dialogs::export_project::{ExportProject, Refusal as ExportRefusal};
use desktop_gpui::screens::dialogs::failed_start::{Failed, FailedStart};
use desktop_gpui::screens::dialogs::layer::{self, Closes};
use desktop_gpui::screens::dialogs::pair::{PairScreen, Road};
use desktop_gpui::screens::dialogs::{ADD_PROJECT, EXPORT_PROJECT};
use desktop_gpui::ui::{ActivePalette, AlertDialog, Button, Refusal};
use gpui_kit::{div, prelude::*, AnyView, App, Context, Task, Window};
use serde_json::{json, Value};

use super::{page, State};

/// A road nothing is asked of: the states below are drawn before any press.
pub fn idle_road() -> Road {
    Road { redeem: Rc::new(|_, _| Task::ready(Err(String::new()))), ask: Rc::new(|_| Task::ready(false)), paired: Rc::new(|_, _| {}) }
}

fn pair(typed: Option<&'static str>, window: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|cx| {
        let mut screen = PairScreen::new(idle_road(), window, cx);
        if let Some(typed) = typed {
            screen.set_code(typed, window, cx);
            screen.submit(window, cx);
        }
        screen
    })
    .into()
}

struct Failing(Failed);

impl Render for Failing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(cx.palette().background).child(FailedStart::new(self.0.clone(), |_, _, _| {}))
    }
}

/// The fork a thread asked for, gone before its machine stood.
pub fn failed_fork() -> Value {
    json!({ "id": "w9", "name": "fix-login", "phase": "gone", "machineId": "", "parentWorkspaceId": "w1", "parentThreadId": "t1",
        "gone": "Boat refused the fork: the account has no room for another machine now" })
}

struct Confirm;

impl Render for Confirm {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        AlertDialog::new("Delete Parity state W2-DLG-031.?")
            .sentence("Its turns and checkpoints leave this computer; the folder it worked in is left as it is.")
            .child(Refusal::new())
            .act(Button::outline("dialog-cancel", "Cancel").neutral_ring())
            .act(Button::destructive("delete-thread", "Delete"))
    }
}

/// This computer and a box somebody joined, as `places.list` gives them.
pub fn computers() -> Vec<Value> {
    vec![
        json!({ "id": "here", "kind": "here", "name": "Studio", "default": true }),
        json!({ "id": "p1", "kind": "computer", "name": "build-box", "default": false, "takesForks": true }),
    ]
}

pub fn repos() -> Listing {
    let folder = |path: &str, branch: Option<&str>| Folder { path: path.into(), repo: true, branch: branch.map(Into::into) };
    Listing {
        dir: "/Users/me".into(),
        folders: vec![
            folder("/Users/me/code/wsp", Some("main")),
            folder("/Users/me/code/api", Some("feat/limits")),
            folder("/Users/me/code/site", Some("main")),
            folder("/Users/me/notes", None),
        ],
    }
}

fn add_project(window: &mut Window, cx: &mut App) -> AnyView {
    let view = cx.new(|cx| AddProject::staged(computers(), Some(repos()), &["/Users/me/code/site"], window, cx));
    layer::open(ADD_PROJECT, view.into(), Closes::Outside, None, window, cx);
    page(window, cx)
}

fn export(end: Option<Result<Value, ExportRefusal>>, window: &mut Window, cx: &mut App) -> AnyView {
    let view = cx.new(|cx| {
        let mut dialog = ExportProject::staged("fix-login", "Studio", "/root/wsp", &["claude", "codex"], window, cx);
        if let Some(end) = end {
            dialog.stage_end(end, cx);
        }
        dialog
    });
    layer::open(EXPORT_PROJECT, view.into(), Closes::Outside, None, window, cx);
    page(window, cx)
}

pub fn states() -> Vec<State> {
    vec![
        // The pair page is a window of its own, whose page Electron draws 788 px tall under its frame.
        State::new("W2-DLG-041", |window, cx| pair(None, window, cx)).size(1280., 788.),
        State::new("W2-DLG-042", |window, cx| pair(Some("000000"), window, cx)).size(1280., 788.),
        State::new("W2-DLG-043", |_, cx| cx.new(|_| Failing(Failed::of(&failed_fork()).unwrap())).into()),
        State::new("W2-DLG-012", |window, cx| {
            layer::open("confirm", cx.new(|_| Confirm).into(), Closes::Escape, None, window, cx);
            page(window, cx)
        }),
        State::new("W2-DLG-ADD-PROJECT", add_project),
        State::new("W2-DLG-EXPORT", |window, cx| export(None, window, cx)),
        State::new("W2-DLG-EXPORT-exists", |window, cx| {
            let exists = "/Users/me/code/wsp already exists on this computer with 212 files; export with replace to overwrite it";
            export(Some(Err(ExportRefusal { message: exists.into(), exists: true })), window, cx)
        }),
        State::new("W2-DLG-EXPORT-landed", |window, cx| {
            let landed = json!({ "dest": "/Users/me/code/wsp", "files": 1204, "bytes": 48_234_496., "excluded": ["node_modules", "target"],
                "agents": [{ "agent": "claude", "files": 3, "bytes": 1024., "outcome": "moved", "sessions": 4 },
                    { "agent": "codex", "files": 0, "bytes": 0., "outcome": "nothing" }] });
            export(Some(Ok(landed)), window, cx)
        }),
    ]
}
