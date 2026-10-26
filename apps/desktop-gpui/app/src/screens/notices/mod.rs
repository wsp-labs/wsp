// SPDX-License-Identifier: AGPL-3.0-only
//! Notices the app raises on its own, after `apps/web/src/notices/`: [`host`] turns the host's pushes into toasts,
//! [`words`] holds their sentences, and the window's title carries the mark while a need stands. The toasts
//! themselves are `ui::toast`'s; this module tells them where the centre column is and what else holds Escape.

pub mod host;
pub mod words;

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::base::Root;
use gpui_kit::{px, App, Context, Entity, Pixels, Subscription, Task, WeakEntity, Window};
use serde_json::{json, Value};
use wsp_host_client::HostClient;

use crate::screens::app_shell::frame;
use crate::screens::palette::overlays;
use crate::screens::pr_pane::files::open_split;
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
    marked: bool,
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
            marked: false,
            _follow: Vec::new(),
            _subscriptions: Vec::new(),
        }
    }

    /// Starts following the host's pushes for the notices they raise.
    pub fn attach(&mut self, host: Host, window: &mut Window, cx: &mut Context<Self>) {
        self.host = Some(host.clone());
        self.toasts = toast::of(window, cx).map(|t| t.downgrade());
        if let Some(notice) = version_notice(host.client().host_release().as_deref(), host.client().alias().as_deref()) {
            self.add(notice, cx);
        }
        self._follow = host::follow(host, window, cx);
        // The overlays and the shell are mid-update while they hand the host over.
        cx.defer_in(window, |this, window, cx| this.watch(window, cx));
    }

    /// Follows the sidebar's threads and the shell's route, for the title's mark and the notices about what shows.
    fn watch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let sidebar = overlays::of(window, cx).and_then(|o| o.read(cx).sidebar.clone());
        if let Some(sidebar) = sidebar.as_ref().and_then(WeakEntity::upgrade) {
            self._subscriptions.push(cx.observe_in(&sidebar, window, |this, sidebar, window, cx| {
                this.mark(window, cx);
                if sidebar.read(cx).around().places != this.rules.places {
                    host::take_places(this, sidebar.read(cx).around().places.clone(), cx);
                }
            }));
        }
        self.sidebar = sidebar;
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

    /// Puts the mark on the window's title while a thread's prompt stands, and takes it off after.
    fn mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let needed = self.sidebar.as_ref().and_then(WeakEntity::upgrade).is_some_and(|s| {
            let s = s.read(cx);
            s.threads().iter().any(|t| t.asks)
        });
        // Off the Mac a window reads no title back, so the mark goes on the app's own title.
        if needed != self.marked {
            self.marked = needed;
            window.set_window_title(&words::title_with_need(crate::TITLE, needed));
        }
    }
}

/// `useShellVersionEffect`: an app of another release than the host it attached to is a note, naming a host elsewhere
/// by its alias, with the releases page behind Get where there is a newer app to get.
fn version_notice(host: Option<&str>, alias: Option<&str>) -> Option<Notice> {
    let said = words::version_notice(Some(&HostClient::app_release()), host?, alias)?;
    let notice = Notice::note(said.line);
    Some(if said.update { notice.action(Action::new(words::GET_THE_APP, |_, cx| cx.open_url(words::RELEASES))) } else { notice })
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

/// Opens the workspace's copy in the person's editor (`files/openCopy.ts`): a local workspace's folder here, a running
/// one's over ssh in the host's default editor. A wait on the computer's update is a waiting toast, any other refusal
/// the host's sentence.
pub fn open_in_editor(workspace_id: String, window: &mut Window, cx: &mut App) {
    open_copy_in_editor(workspace_id, None, None, window, cx);
}

/// Whether an open of the workspace's copy waits on the host's answer, which a second press waits on too.
pub fn opening(workspace_id: &str, window: &Window, cx: &App) -> bool {
    overlays::of(window, cx).is_some_and(|o| o.read(cx).notices.read(cx).opening.contains(workspace_id))
}

/// [`open_in_editor`] as `openCopyInEditor`: `pick` becomes the default first, inside the open's wait, so a press
/// meanwhile waits too; a running workspace on another computer opens in the editor picked or named, a local one in the
/// host's default.
pub fn open_copy_in_editor(workspace_id: String, pick: Option<String>, editor: Option<String>, window: &mut Window, cx: &mut App) {
    let Some(overlays) = overlays::of(window, cx) else { return };
    let (host, notices, sidebar) = {
        let o = overlays.read(cx);
        (o.host.clone(), o.notices.clone(), o.sidebar.clone())
    };
    let Some(host) = host else { return };
    let workspace = sidebar.and_then(|s| s.upgrade()).and_then(|s| s.read(cx).around().workspaces.get(&workspace_id).cloned());
    let Some(workspace) = workspace.filter(|w| w["kind"] == "local" || w["phase"] == "running") else { return };
    let fresh = notices.update(cx, |n, cx| {
        cx.notify();
        n.opening.insert(workspace_id.clone())
    });
    if !fresh {
        return;
    }
    let named = pick.clone().or(editor).filter(|_| workspace["kind"] != "local");
    let (asked_again, again_with, handle) = (workspace_id.clone(), named.clone(), window.window_handle());
    let id = workspace_id.clone();
    let folder = workspace["folder"].as_str().or(workspace["project"]["path"].as_str()).unwrap_or_default().to_owned();
    let mut asked = json!({ "workspaceId": id, "path": folder });
    if let Some(editor) = named {
        asked["editor"] = editor.into();
    }
    notices.update(cx, |_, cx| {
        cx.spawn(async move |this, cx| {
            let answer = async {
                if let Some(pick) = pick {
                    open_split::set_preference(host.clone(), pick.clone()).await?;
                    cx.update(|cx| open_split::set_editors(None, Some(pick), cx));
                }
                let asking = host.clone();
                asking
                    .run(move |client| async move {
                        crate::screens::sidebar::host::ask::<wsp_host_client::protocol::EditorOpenRequest>(client, asked).await.map(drop)
                    })
                    .await
            }
            .await;
            _ = this.update(cx, |n, cx| {
                n.opening.remove(&workspace_id);
                cx.notify();
                if let Err(failure) = answer {
                    if failure.kind.as_deref() == Some(open_split::SSH_INCLUDE_KIND) {
                        let again: crate::ui::Run =
                            Rc::new(move |window, cx| open_copy_in_editor(asked_again.clone(), None, again_with.clone(), window, cx));
                        _ = handle.update(cx, |_, window, cx| open_split::ask_ssh(host, again, window, cx));
                        return;
                    }
                    n.add(open_split::failed_notice(&failure), cx);
                }
            });
        })
        .detach();
    });
}

/// Something of the app's that reads no key context holds Escape: a dialog on the layer, the palette, the sidebar's
/// dialog, menu or switcher.
fn holds_escape(window: &Window, cx: &App) -> bool {
    if crate::screens::dialogs::layer::top(cx).is_some() {
        return true;
    }
    let Some(overlays) = overlays::of(window, cx) else { return false };
    let o = overlays.read(cx);
    o.palette.is_some()
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

#[cfg(test)]
mod tests {
    use wsp_host_client::HostClient;

    use super::{version_notice, words, Kind};

    /// Catches: a newer host attached in silence, a host elsewhere not named, or Get offered where there is no newer
    /// app to get.
    #[test]
    fn a_host_of_another_release_is_a_note_with_get_only_where_the_app_is_behind() {
        let app = HostClient::app_release();
        assert!(version_notice(Some(&app), None).is_none(), "one release says nothing");
        assert!(version_notice(None, None).is_none(), "a host that named no release says nothing");
        let behind = version_notice(Some("999.0.0"), Some("studio")).expect("a newer host is said");
        assert_eq!(behind.kind, Kind::Note);
        assert_eq!(behind.text.as_ref(), format!("this app is {app}, the host studio is 999.0.0: get the new app"));
        assert_eq!(behind.action.map(|a| a.word.to_string()).as_deref(), Some(words::GET_THE_APP));
        let ahead = version_notice(Some("0.0.1"), None).expect("an older host is said");
        assert_eq!(ahead.text.as_ref(), format!("this app is {app}, the host is 0.0.1: run the app's own host"));
        assert!(ahead.action.is_none(), "nothing to get when the app is the newer half");
    }
}
