// SPDX-License-Identifier: AGPL-3.0-only
//! Notices the app raises on its own, after `apps/web/src/notices/`: [`host`] turns the host's pushes into toasts,
//! [`words`] holds their sentences, and the window's title carries the mark while a need stands. The toasts
//! themselves are `ui::toast`'s; this module tells them where the centre column is and what else holds Escape.

pub mod host;
pub mod words;

use std::collections::HashSet;

use gpui_kit::base::Root;
use gpui_kit::{px, App, Context, Entity, Pixels, Subscription, Task, WeakEntity, Window};
use serde_json::{json, Value};

use crate::screens::app_shell::frame;
use crate::screens::palette::overlays;
use crate::screens::{Host, Route};
use crate::shell::Shell;
use crate::sidebar::Sidebar;
use crate::ui::toast::{self, Action, Hooks, Kind, Notice, Toasts};

/// Tells the toasts where the shell's centre column is and what of the app holds Escape; call once after `ui::init`.
pub fn init(cx: &mut App) {
    toast::set_hooks(Hooks { column, page_open, holds_escape }, cx);
}

/// The host's notices for one window, the title's mark, and the editor opens in flight.
pub struct Notices {
    host: Option<Host>,
    rules: host::Rules,
    sidebar: Option<WeakEntity<Sidebar>>,
    toasts: Option<WeakEntity<Toasts>>,
    /// The workspaces an open in the editor waits on, so a second press waits too.
    opening: HashSet<String>,
    _follow: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl Notices {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Notices {
            host: None,
            rules: host::Rules::default(),
            sidebar: None,
            toasts: None,
            opening: HashSet::new(),
            _follow: Vec::new(),
            _subscriptions: Vec::new(),
        }
    }

    /// Starts following the host's pushes for the notices they raise.
    pub fn attach(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        self.host = Some(host.clone());
        self.toasts = toast::of(window, cx).map(|t| t.downgrade());
        self._follow = host::follow(host, window, cx);
        // The overlays and the shell are mid-update while they hand the host over.
        cx.defer_in(window, |this, window, cx| this.watch(window, cx));
    }

    /// Follows the sidebar's threads and the shell's route, for the title's mark and the notices about what shows.
    fn watch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = overlays::of(window, cx).and_then(|o| o.read(cx).sidebar.clone());
        if let Some(sidebar) = sidebar.as_ref().and_then(WeakEntity::upgrade) {
            self._subscriptions.push(cx.observe_in(&sidebar, window, |this, _, window, cx| this.mark(window, cx)));
        }
        self.sidebar = sidebar;
        if let Some(shell) = shell(window, cx) {
            self._subscriptions.push(cx.observe_in(&shell, window, |this, _, window, cx| this.route_moved(window, cx)));
        }
        self.mark(window, cx);
    }

    /// Says `notice` in this window's toasts.
    pub fn add(&mut self, notice: Notice, cx: &mut Context<Self>) {
        match self.toasts.as_ref().and_then(WeakEntity::upgrade) {
            Some(toasts) => toasts.update(cx, |t, cx| t.add(notice, cx)),
            None => toast::notify(cx, notice),
        }
    }

    pub fn end(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(toasts) = self.toasts.as_ref().and_then(WeakEntity::upgrade) {
            toasts.update(cx, |t, cx| t.end(key, cx));
        }
    }

    fn end_prefixed(&mut self, prefix: &str, cx: &mut Context<Self>) {
        if let Some(toasts) = self.toasts.as_ref().and_then(WeakEntity::upgrade) {
            toasts.update(cx, |t, cx| t.end_prefixed(prefix, cx));
        }
    }

    /// Puts the mark on the window's title while a build's need or a thread's prompt stands, and takes it off after.
    fn mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let asks = self.sidebar.as_ref().and_then(WeakEntity::upgrade).is_some_and(|s| s.read(cx).threads().iter().any(|t| t.asks));
        let title = window.window_title();
        let marked = words::title_with_need(&title, asks || self.rules.job_needs_you());
        if marked != title {
            window.set_window_title(&marked);
        }
    }

    fn route_moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        host::settle_shown(self, window, cx);
    }
}

/// A settle's answer as `settleSaying` says it: done where it took threads and left nothing running, else a note,
/// with Undo restoring what it took.
pub fn settled(reply: &Value, restore: impl Fn(Vec<String>, &mut App) + 'static) -> Notice {
    let (text, moved, clean) = words::settled(reply);
    let notice = Notice::new(if !moved.is_empty() && clean { Kind::Done } else { Kind::Note }, text);
    if moved.is_empty() {
        return notice;
    }
    notice.action(Action::new(words::UNDO, move |_, cx| restore(moved.clone(), cx)))
}

/// A stop on a computer whose daemon is too old to end a process's tree (`settings/activity/live.tsx`): what TERM
/// reached, and Force end where any of them still runs.
pub fn too_old(computer: &str, procs: &[(u32, String)], force: Option<Action>) -> Notice {
    let notice = Notice::error(words::kill_too_old(computer, procs)).place(computer.to_owned());
    match force {
        Some(force) => notice.action(force),
        None => notice,
    }
}

/// Opens the workspace's copy in the person's editor (`files/openCopy.ts`): a local workspace's folder here, a running
/// one's over ssh in the host's default editor. A wait on the computer's update is a waiting toast, any other refusal
/// the host's sentence.
pub fn open_in_editor(workspace_id: String, window: &mut Window, cx: &mut App) {
    let Some(overlays) = overlays::of(window, cx) else { return };
    let (host, notices) = {
        let o = overlays.read(cx);
        (o.host.clone(), o.notices.clone())
    };
    let Some(host) = host else { return };
    if !notices.update(cx, |n, _| n.opening.insert(workspace_id.clone())) {
        return;
    }
    let id = workspace_id.clone();
    let open = host.run(move |client| async move {
        let listed =
            crate::screens::sidebar::host::ask::<wsp_host_client::protocol::WorkspacesListRequest>(client.clone(), json!({})).await?;
        let workspace = serde_json::to_value(&listed.workspaces)
            .unwrap_or_default()
            .as_array()
            .and_then(|all| all.iter().find(|w| w["id"] == id.as_str()).cloned());
        let Some(workspace) = workspace.filter(|w| w["kind"] == "local" || w["phase"] == "running") else { return Ok(()) };
        let folder = workspace["folder"].as_str().or(workspace["project"]["path"].as_str()).unwrap_or_default().to_owned();
        crate::screens::sidebar::host::ask::<wsp_host_client::protocol::EditorOpenRequest>(
            client,
            json!({ "workspaceId": id, "path": folder }),
        )
        .await
        .map(drop)
    });
    notices.update(cx, |_, cx| {
        cx.spawn(async move |this, cx| {
            let answer = open.await;
            _ = this.update(cx, |n, cx| {
                n.opening.remove(&workspace_id);
                if let Err(failure) = answer {
                    let notice = if failure.kind.as_deref() == Some("sshBehind") {
                        Notice::waiting(failure.message)
                    } else {
                        Notice::error(failure.message)
                    };
                    n.add(notice, cx);
                }
            });
        })
        .detach();
    });
}

/// Something of the app's that reads no key context holds Escape: the palette or the switcher, the sidebar's dialog,
/// menu or switcher.
fn holds_escape(window: &Window, cx: &App) -> bool {
    let Some(overlays) = overlays::of(window, cx) else { return false };
    let o = overlays.read(cx);
    o.palette.is_some()
        || o.switcher.is_some()
        || o.sidebar.as_ref().and_then(WeakEntity::upgrade).is_some_and(|s| {
            let s = s.read(cx);
            s.dialog.is_some() || s.switcher.is_some() || s.menu.is_open()
        })
}

/// A page is open in the main column, whose Escape goes back.
fn page_open(window: &Window, cx: &App) -> bool {
    matches!(route(window, cx), Some(Route::Screen(_)))
}

/// The window's shell, where the window is the app's.
pub(crate) fn shell(window: &Window, cx: &App) -> Option<Entity<Shell>> {
    window.root::<Root>().flatten()?.read(cx).view().clone().downcast::<Shell>().ok()
}

pub(crate) fn route(window: &Window, cx: &App) -> Option<Route> {
    shell(window, cx).map(|s| s.read(cx).route().clone())
}

/// The centre column, where the toasts stand: between the sidebar and an inline panel, under the header row.
fn column(window: &Window, cx: &App) -> (Pixels, Pixels, Pixels) {
    let viewport = window.viewport_size().width;
    let Some(shell) = shell(window, cx) else { return (px(0.), viewport, frame::TOPBAR) };
    let shell = shell.read(cx);
    let panel = shell.panel().read(cx);
    let inline = !matches!(shell.route(), Route::Screen(_)) && panel.is_open() && viewport > frame::SHEET_MAX_VIEWPORT;
    let left = if shell.sidebar_open() { frame::clamp_sidebar(shell.sidebar_width(), viewport, inline) } else { px(0.) };
    let right = if inline { viewport - frame::clamp_panel(panel.width(), viewport, viewport - left) } else { viewport };
    (left, right, frame::TOPBAR)
}
