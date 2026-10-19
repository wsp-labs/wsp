// SPDX-License-Identifier: AGPL-3.0-only
//! The Files pane: the tree on the `files` tab, a file on a `file:<path>` tab (`files/`, `components/files/`), the
//! finder over the window, and the header's Open.

pub mod at;
pub mod crumbs;
pub mod file_tab;
pub mod finder;
pub mod markdown;
pub mod open_split;
pub mod paths;
pub mod pierre_icons;
pub mod rank;
pub mod rows;
pub mod tree;
pub mod words;

use std::rc::Rc;

use gpui_kit::{div, prelude::*, AnyView, App, Context, Entity, Global, SharedString, Window};

use crate::local_store::LocalStore;
use crate::screens::{navigate, Host, Opened, Pane, Parts, Place, Route, Screen};
use file_tab::FileTab;
use tree::Tree;

/// A file asked open at a line: the file tab of that path lights and scrolls to it, asked again or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reveal {
    pub workspace_id: String,
    pub path: String,
    pub line: Option<usize>,
    /// Bumped by each ask, so the same line asked twice scrolls twice.
    pub seq: u64,
}

impl Global for Reveal {}

/// Reading markdown rendered is a preference, not a property of one file: on until the person turns it off.
#[derive(Clone, Copy, Debug)]
pub struct RenderMarkdown(pub bool);

impl Global for RenderMarkdown {}

impl RenderMarkdown {
    /// The key today's app keeps it under, in the app's own store here.
    pub const KEY: &'static str = "wsp:render-markdown";

    pub fn on(cx: &App) -> bool {
        match cx.try_global::<RenderMarkdown>() {
            Some(r) => r.0,
            None => cx.try_global::<LocalStore>().and_then(|s| s.get_as::<bool>(Self::KEY)).unwrap_or(true),
        }
    }

    pub fn set(on: bool, cx: &mut App) {
        cx.set_global(RenderMarkdown(on));
        if cx.try_global::<LocalStore>().is_some() {
            if let Err(e) = cx.global_mut::<LocalStore>().set(Self::KEY, serde_json::Value::Bool(on)) {
                eprintln!("desktop-gpui: store: {e}");
            }
        }
    }
}

/// A file the Files pane asked the Changes pane to show (`Show in Changes`), by its absolute path; bumped per ask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShowInChanges {
    pub workspace_id: String,
    pub path: String,
    pub seq: u64,
}

impl Global for ShowInChanges {}

/// Asks the Changes pane to show `path`, opening it.
pub fn show_in_changes(workspace_id: &str, path: &str, cx: &mut App) {
    let seq = cx.try_global::<ShowInChanges>().map_or(0, |r| r.seq + 1);
    cx.set_global(ShowInChanges { workspace_id: workspace_id.to_owned(), path: path.to_owned(), seq });
    navigate(Route::Screen("changes".into()), cx);
}

/// Opens `path` as its own tab in the Files pane, at `line` where one is given.
pub fn open_file(workspace_id: &str, path: &str, line: Option<usize>, cx: &mut App) {
    let seq = cx.try_global::<Reveal>().map_or(0, |r| r.seq + 1);
    cx.set_global(Reveal { workspace_id: workspace_id.to_owned(), path: path.to_owned(), line, seq });
    navigate(Route::Screen(format!("files:file:{path}").into()), cx);
}

/// The tab a file opens on.
pub fn file_tab(path: &str) -> SharedString {
    format!("file:{path}").into()
}

enum Body {
    Tree(Entity<Tree>),
    File(Entity<FileTab>),
    /// Opened with no thread: the launcher keeps the pane closed here, so nothing reads.
    None,
}

pub struct FilesScreen {
    body: Body,
    title: Option<SharedString>,
}

impl FilesScreen {
    pub fn tree(&self) -> Option<&Entity<Tree>> {
        match &self.body {
            Body::Tree(tree) => Some(tree),
            _ => None,
        }
    }

    pub fn file(&self) -> Option<&Entity<FileTab>> {
        match &self.body {
            Body::File(file) => Some(file),
            _ => None,
        }
    }
}

impl Screen for FilesScreen {
    const NAME: &'static str = "files";
    const PLACE: Place = Place::Panel(Pane::Files);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let file = opened.tab.as_deref().and_then(|t| t.strip_prefix("file:")).map(str::to_owned);
        let Some(workspace_id) = opened.workspace_id.clone() else { return FilesScreen { body: Body::None, title: None } };
        let thread_id = opened.thread_id.clone();
        match file {
            Some(path) => {
                let title = file_tab::title(&path);
                let file = cx.new(|cx| FileTab::new(host, workspace_id, thread_id, path, cx));
                FilesScreen { body: Body::File(file), title: Some(title) }
            }
            None => {
                if let Some(query) = crate::screens::pr_pane::shot().and_then(|s| s.strip_prefix("finder:")) {
                    let (host, workspace) = (host.clone(), workspace_id.clone());
                    window.defer(cx, move |window, cx| {
                        let finder = finder::open(finder::Mode::Files, &host, &workspace, window, cx);
                        finder.update(cx, |f, cx| f.set_query(query, window, cx));
                    });
                }
                let tree = cx.new(|cx| Tree::new(host, workspace_id, thread_id, window, cx));
                FilesScreen { body: Body::Tree(tree), title: None }
            }
        }
    }

    fn parts(this: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
        let title = this.read(cx).title.clone();
        Parts { title: title.map(|t| Rc::new(move |_: &App| t.clone()) as crate::screens::app_shell::Title), ..Parts::default() }
    }
}

impl Render for FilesScreen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let view: Option<AnyView> = match &self.body {
            Body::Tree(tree) => Some(tree.clone().into()),
            Body::File(file) => Some(file.clone().into()),
            Body::None => None,
        };
        div().size_full().children(view)
    }
}
