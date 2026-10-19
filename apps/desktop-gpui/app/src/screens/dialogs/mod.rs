// SPDX-License-Identifier: AGPL-3.0-only
//! The flows that add things and the windows around a launch: Add a computer and Add a cloud provider
//! (`settings/add/`), Add a project and Export project (`sidebar/`), onboarding and the starting window
//! (`desktop:onboarding.html`, `desktop:starting.html`), first run, the pair screen and a failed start. Each flow opens
//! through one `open_*` here, which the sidebar's buttons, Settings and the palette call; a dialog stands on [`layer`].

pub mod add_cloud;
pub mod add_computer;
pub mod add_project;
pub mod export_project;
pub mod failed_start;
pub mod first_run;
pub mod layer;
pub mod onboarding;
pub mod pair;
pub mod picks;
pub mod sign_in;
pub mod starting;
pub mod startup;
pub mod steps;

use gpui_kit::{div, prelude::*, Context, Window};

use super::{Host, Opened, Place, Screen};

/// The name each flow's dialog stands on the layer under.
pub const ADD_COMPUTER: &str = "add-computer";
pub const ADD_CLOUD: &str = "add-cloud";
pub const ADD_PROJECT: &str = "add-project";
pub const EXPORT_PROJECT: &str = "export-project";

/// Add a computer, on its first step, or on the running view of the setup `setup` names (a sidebar setup card, a
/// pending row, a notice).
pub fn open_add_computer(host: &Host, setup: Option<String>, window: &mut Window, cx: &mut gpui_kit::App) {
    add_computer::open(host, setup, window, cx);
}

/// Add a cloud provider: the cloud card's Add, Settings' Computers page.
pub fn open_add_cloud(host: &Host, window: &mut Window, cx: &mut gpui_kit::App) {
    add_cloud::open(host, window, cx);
}

/// Add a project: the sidebar's Add, first run, the palette's last row.
pub fn open_add_project(host: &Host, window: &mut Window, cx: &mut gpui_kit::App) {
    add_project::open(host, window, cx);
}

/// Export project for the workspace `workspace_id`: the project row's menu.
pub fn open_export_project(host: &Host, workspace_id: String, window: &mut Window, cx: &mut gpui_kit::App) {
    export_project::open(host, workspace_id, window, cx);
}

/// A route that opens a flow over an empty page, so `desktop-gpui --screen add-computer` starts on the dialog.
macro_rules! dialog_screen {
    ($ty:ident, $name:literal, |$host:ident, $opened:ident, $window:ident, $cx:ident| $open:expr) => {
        pub struct $ty;

        impl Screen for $ty {
            const NAME: &'static str = $name;
            const PLACE: Place = Place::Main;

            fn new($host: Host, $opened: Opened, $window: &mut Window, $cx: &mut Context<Self>) -> Self {
                $open;
                $ty
            }
        }

        impl Render for $ty {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full()
            }
        }
    };
}

dialog_screen!(AddComputerScreen, "add-computer", |host, opened, window, cx| open_add_computer(
    &host,
    opened.arg.map(|a| a.to_string()),
    window,
    cx
));
dialog_screen!(AddCloudScreen, "add-cloud", |host, _opened, window, cx| open_add_cloud(&host, window, cx));
dialog_screen!(AddProjectScreen, "add-project", |host, _opened, window, cx| open_add_project(&host, window, cx));
dialog_screen!(ExportProjectScreen, "export-project", |host, opened, window, cx| open_export_project(
    &host,
    opened.arg.map(|a| a.to_string()).unwrap_or_default(),
    window,
    cx
));
