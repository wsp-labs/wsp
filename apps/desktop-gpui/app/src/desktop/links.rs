// SPDX-License-Identifier: AGPL-3.0-only
//! The wsp:// links the system hands the app (`desktop:deep-link.ts`): a Mac's open-url, and on Linux the argument of
//! a second launch, carried over the lock's socket. A link is read down to the kind and id it names
//! (`protocol:app-address.ts` `addressFromLink`) and only picks what the window shows (`web:shell/links.ts`). A link
//! that arrives before the app window stands, or while none does, is held, the last of several winning, and the next
//! app window opens on it; one with the window up raises it and opens there.

use gpui_kit::{AnyWindowHandle, App, Entity, Global, Subscription, WeakEntity, Window};

use crate::screens::Route;
use crate::shell::{Shell, SETTINGS};
use crate::ui::toast::{self, Kind, Notice};

/// `LINK_KINDS`: what a link can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Thread,
    Workspace,
    Project,
    Settings,
}

impl LinkKind {
    fn of(word: &str) -> Option<LinkKind> {
        Some(match word {
            "thread" => LinkKind::Thread,
            "workspace" => LinkKind::Workspace,
            "project" => LinkKind::Project,
            "settings" => LinkKind::Settings,
            _ => return None,
        })
    }

    pub fn word(self) -> &'static str {
        match self {
            LinkKind::Thread => "thread",
            LinkKind::Workspace => "workspace",
            LinkKind::Project => "project",
            LinkKind::Settings => "settings",
        }
    }
}

/// `LinkTarget`: a kind and the id of the thread, workspace, project or Settings group.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub kind: LinkKind,
    pub id: String,
}

/// `LINK_ID`: letters, digits, dashes and underscores, a letter or digit first, at most 128.
fn link_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && id.len() <= 128
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `addressFromLink`: `wsp://<kind>/<id>`, a trailing slash allowed, and nothing looser.
pub fn address_from_link(url: &str) -> Option<Target> {
    let scheme = url.get(..6).filter(|s| s.eq_ignore_ascii_case("wsp://"))?;
    let rest = &url[scheme.len()..];
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let (kind, id) = rest.split_once('/')?;
    let kind = LinkKind::of(kind)?;
    (!id.contains('/') && link_id(id)).then(|| Target { kind, id: id.to_owned() })
}

/// `linkInArgv`: the first word that starts `wsp://`, in any case.
pub fn link_in_argv<'a>(words: impl IntoIterator<Item = &'a String>) -> Option<&'a String> {
    words.into_iter().find(|word| word.get(..6).is_some_and(|s| s.eq_ignore_ascii_case("wsp://")))
}

/// `LINK_WORDS.gone`.
pub fn gone(kind: LinkKind) -> String {
    let what = if kind == LinkKind::Settings { "Settings page" } else { kind.word() };
    format!("That link names a {what} this wsp does not have.")
}

/// `SETTINGS_GROUP_IDS` in `web:settings/groupIds.ts`.
pub const SETTINGS_GROUPS: [&str; 12] = [
    "general",
    "appearance",
    "agents",
    "computers",
    "recipes",
    "projects",
    "usage",
    "activity",
    "keybindings",
    "privacy",
    "devices",
    "account",
];

/// The app window a link opens in, once it stands.
struct Main {
    window: AnyWindowHandle,
    shell: WeakEntity<Shell>,
}

/// The link held for the next app window, the app window, and the link it waits to open until the host's rows land.
#[derive(Default)]
struct Links {
    held: Option<Target>,
    main: Option<Main>,
    waiting: Option<(Target, Vec<Subscription>)>,
}

impl Global for Links {}

/// The link held for the next app window, if any.
pub fn held(cx: &App) -> Option<Target> {
    cx.try_global::<Links>().and_then(|l| l.held.clone())
}

/// A link the system handed over: nothing for one that names nothing; raised and opened in the app window where it
/// stands; else held for the next one.
pub fn open(url: &str, cx: &mut App) {
    let Some(target) = address_from_link(url) else { return };
    let main = cx.default_global::<Links>().main.as_ref().map(|m| (m.window, m.shell.clone()));
    match main.filter(|(window, _)| cx.windows().contains(window)).and_then(|(window, shell)| Some((window, shell.upgrade()?))) {
        // The raise waits for whatever update handed the link over to let go of its window.
        Some((window, shell)) => cx.defer(move |cx| {
            _ = window.update(cx, |_, window, cx| super::frame::raise(window, cx));
            deliver(target, window, shell, cx);
        }),
        None => {
            let links = cx.global_mut::<Links>();
            links.main = None;
            links.held = Some(target);
        }
    }
}

/// The app window stands: links open in it from now on, first the one held or left waiting by a window now gone.
pub fn attach(window: &Window, shell: &Entity<Shell>, cx: &mut App) {
    let handle = window.window_handle();
    let links = cx.default_global::<Links>();
    links.main = Some(Main { window: handle, shell: shell.downgrade() });
    if let Some(target) = links.held.take().or_else(|| links.waiting.take().map(|(target, _)| target)) {
        deliver(target, handle, shell.clone(), cx);
    }
}

/// Opens `target` once the host's rows have landed, which a later link waiting with it replaces. A host on the account
/// is told nothing of this computer's links: the window moves home first, and the link opens there.
fn deliver(target: Target, window: AnyWindowHandle, shell: Entity<Shell>, cx: &mut App) {
    if super::hosts::away(cx) {
        cx.global_mut::<Links>().held = Some(target);
        return super::hosts::to(None, cx);
    }
    if landed(&shell, cx) {
        cx.global_mut::<Links>().waiting = None;
        return go(target, window, shell, cx);
    }
    let (sidebar, weak) = (shell.read(cx).sidebar().clone(), shell.downgrade());
    let follow = move |cx: &mut App| {
        if cx.global::<Links>().waiting.is_none() {
            return;
        }
        let Some(shell) = weak.upgrade() else { return };
        if landed(&shell, cx) {
            if let Some((target, _)) = cx.global_mut::<Links>().waiting.take() {
                go(target, window, shell, cx);
            }
        }
    };
    let again = follow.clone();
    let watch = vec![cx.observe(&shell, move |_, cx| again(cx)), cx.observe(&sidebar, move |_, cx| follow(cx))];
    cx.global_mut::<Links>().waiting = Some((target, watch));
}

/// `wireLinks`' `landed`: the host answered and the sidebar has had its first whole read.
fn landed(shell: &Entity<Shell>, cx: &App) -> bool {
    let shell = shell.read(cx);
    shell.connected() && shell.sidebar().read(cx).around().preferences.is_some()
}

/// `openLink`: where the window goes for `target`, or the line that it names nothing here.
fn route_of(target: &Target, shell: &Shell, cx: &App) -> Result<Option<Route>, String> {
    let sidebar = shell.sidebar().read(cx);
    let id = target.id.as_str();
    let gone = || gone(target.kind);
    match target.kind {
        LinkKind::Thread => {
            let workspace = sidebar
                .items()
                .iter()
                .find(|i| i.thread_id == id)
                .map(|i| i.workspace_id.clone())
                .or_else(|| sidebar.threads().iter().find(|t| t.key == id).map(|t| t.workspace_id.clone()))
                .ok_or_else(gone)?;
            Ok(Some(Route::Thread { workspace_id: workspace, thread_id: id.to_owned() }))
        }
        LinkKind::Workspace => {
            let known = sidebar.around().workspaces.contains_key(id) || sidebar.items().iter().any(|i| i.workspace_id == id);
            if !known {
                return Err(gone());
            }
            Ok(Some(match sidebar.items().iter().find(|i| i.workspace_id == id) {
                Some(item) => Route::Thread { workspace_id: item.workspace_id.clone(), thread_id: item.thread_id.clone() },
                None => Route::Screen(
                    format!("{}:{id}", <crate::screens::composer::screen::ComposerScreen as crate::screens::Screen>::NAME).into(),
                ),
            }))
        }
        // The app has no project home yet, so a project it holds moves nothing past the raise.
        LinkKind::Project => sidebar.around().projects.iter().any(|(p, _)| p == id).then_some(None).ok_or_else(gone),
        LinkKind::Settings => {
            SETTINGS_GROUPS.contains(&id).then(|| Some(Route::Screen(format!("{SETTINGS}:{id}").into()))).ok_or_else(gone)
        }
    }
}

fn go(target: Target, window: AnyWindowHandle, shell: Entity<Shell>, cx: &mut App) {
    cx.defer(move |cx| {
        _ = window.update(cx, |_, window, cx| match route_of(&target, shell.read(cx), cx) {
            Ok(Some(route)) => shell.update(cx, |shell, cx| shell.go(route, window, cx)),
            Ok(None) => {}
            Err(said) => toast::notify_in(window, cx, Notice::new(Kind::Error, said)),
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(kind: LinkKind, id: &str) -> Option<Target> {
        Some(Target { kind, id: id.to_owned() })
    }

    #[test]
    fn a_link_names_a_kind_and_an_id_and_nothing_looser() {
        assert_eq!(address_from_link("wsp://thread/t_1-a"), target(LinkKind::Thread, "t_1-a"));
        assert_eq!(address_from_link("WSP://workspace/w1/"), target(LinkKind::Workspace, "w1"));
        assert_eq!(address_from_link("wsp://project/p1"), target(LinkKind::Project, "p1"));
        assert_eq!(address_from_link("wsp://settings/general"), target(LinkKind::Settings, "general"));
        for nothing in [
            "wsp://thread/a/b",
            "wsp://thread/",
            "wsp://thread",
            "wsp://thread//",
            "wsp://thread/a?b=1",
            "wsp://thread/a#b",
            "wsp://thread/a%20b",
            "wsp://thread/-a",
            "wsp://Thread/a",
            "wsp://computer/a",
            "http://thread/a",
            "wsp:/thread/a",
            "wsp://",
            "",
        ] {
            assert_eq!(address_from_link(nothing), None, "{nothing}");
        }
        assert_eq!(address_from_link(&format!("wsp://thread/{}", "a".repeat(128))), target(LinkKind::Thread, &"a".repeat(128)));
        assert_eq!(address_from_link(&format!("wsp://thread/{}", "a".repeat(129))), None);
    }

    #[test]
    fn the_link_in_a_launchs_words_is_the_first_wsp_one() {
        let words: Vec<String> = ["--state", "/s.json", "WSP://thread/a", "wsp://thread/b"].map(str::to_owned).into();
        assert_eq!(link_in_argv(&words).map(String::as_str), Some("WSP://thread/a"));
        assert_eq!(link_in_argv(&["wsp:/x".to_owned()]), None);
    }

    #[test]
    fn a_link_to_nothing_here_says_so_in_todays_words() {
        assert_eq!(gone(LinkKind::Thread), "That link names a thread this wsp does not have.");
        assert_eq!(gone(LinkKind::Settings), "That link names a Settings page this wsp does not have.");
    }
}
