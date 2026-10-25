// SPDX-License-Identifier: AGPL-3.0-only
//! The Hosts menu and the window's move between hosts (`desktop:host-switch.ts`, and `hostsMenuItems` in
//! `packages/protocol/src/words/places.ts`): this computer first, then every host on the account this computer holds a
//! record of less its own, a check on the one the window is on. A pick of a host on the account dials it once, which
//! admits this computer there or renews a token it no longer takes, then builds the window's shell again on it, and
//! the feed behind the menu bar, the Dock's badge and notifications follows. A refused move goes to the app's log and
//! leaves the window where it was. The host here is never stopped by a move.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{Action, AnyWindowHandle, App, AppContext as _, AsyncApp, Global};
use tokio::runtime::Runtime;
use wsp_host_client::{Env, Failure, HostClient};

use super::{feed, links, log};
use crate::screens::app_shell::{menu, vibrancy};
use crate::shell::{Launch, Shell};

/// `HostsView`: what this computer is called, the alias of the host the window is on (none for here), and the
/// aliases of the hosts on the account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostsView {
    pub here: String,
    pub current: Option<String>,
    pub hosts: Vec<String>,
}

/// One row of the Hosts menu: its label, the host it moves the window to (none for this computer), and its check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub alias: Option<String>,
    pub checked: bool,
}

/// `hostsMenuItems`: this computer first, then every host on the account, the current one checked.
pub fn rows(view: &HostsView) -> Vec<Row> {
    let here = Row { label: view.here.clone(), alias: None, checked: view.current.is_none() };
    let account = view.hosts.iter().map(|alias| Row {
        label: alias.clone(),
        alias: Some(alias.clone()),
        checked: view.current.as_ref() == Some(alias),
    });
    std::iter::once(here).chain(account).collect()
}

/// `hereWord`: this computer's row where its name cannot be read.
pub fn here_word(mac: bool) -> &'static str {
    if mac {
        "This Mac"
    } else {
        "This computer"
    }
}

/// A Hosts row: moves the window to the host on the account under this alias, or to this computer for none.
#[derive(Clone, Debug, PartialEq, Eq, Action)]
#[action(namespace = app, no_json)]
pub struct SwitchHost(pub Option<String>);

struct Hosts {
    state: PathBuf,
    env: Env,
    runtime: Arc<Runtime>,
    here: String,
    /// The alias of the host on the account the window is on, none for here: the one the person picked, or the one the
    /// launch's dial found, kept once the first shell is built so every later dial goes to the same host.
    picked: Option<String>,
    /// Counts the picks, so a dial's answer that a later pick overtook moves nothing.
    asked: u64,
    /// Every window an app shell was built in, which a move builds again.
    windows: Vec<AnyWindowHandle>,
    /// Whether an app window has stood, from when today's app draws its Hosts menu on.
    shown: bool,
}

impl Global for Hosts {}

/// Keeps the window's host for `state`, answers the Hosts rows, and draws the menu again each time an app window
/// stands.
pub fn init(state: PathBuf, env: Env, runtime: Arc<Runtime>, cx: &mut App) {
    let here = crate::screens::dialogs::startup::computer_name().unwrap_or_else(|| here_word(cfg!(target_os = "macos")).to_owned());
    cx.set_global(Hosts { state, env, runtime, here, picked: None, asked: 0, windows: Vec::new(), shown: false });
    cx.on_action(|SwitchHost(alias): &SwitchHost, cx| to(alias.clone(), cx));
    cx.observe_new::<Shell>(|_, window, cx| {
        let Some(window) = window else { return };
        let handle = window.window_handle();
        let live = cx.windows();
        let hosts = cx.global_mut::<Hosts>();
        hosts.shown = true;
        hosts.windows.retain(|window| live.contains(window));
        if !hosts.windows.contains(&handle) {
            hosts.windows.push(handle);
        }
        cx.defer(menu::refresh);
    })
    .detach();
}

/// The alias of the host on the account the window is on, which every shell and the feed dial; none for here.
pub fn picked(cx: &App) -> Option<String> {
    cx.try_global::<Hosts>().and_then(|h| h.picked.clone())
}

/// The alias a shell built now dials: the window's host, which before the first shell is the one the launch's dial
/// finds, kept from then on.
pub fn target(cx: &mut App) -> Option<String> {
    if !cx.has_global::<Hosts>() {
        return None;
    }
    let hosts = cx.global_mut::<Hosts>();
    if hosts.picked.is_none() {
        hosts.picked = wsp_host_client::default_alias(&hosts.state, &hosts.env);
    }
    hosts.picked.clone()
}

/// Whether the window is on a host on the account other than the one a move home lands on, which is told nothing of
/// this computer's links.
pub fn away(cx: &App) -> bool {
    let Some(hosts) = cx.try_global::<Hosts>() else { return false };
    hosts.picked.is_some() && hosts.picked != wsp_host_client::default_alias(&hosts.state, &hosts.env)
}

/// What the Hosts menu draws, once an app window has stood; nothing before, as today's app draws no Hosts menu over
/// onboarding or the starting window.
pub fn view(cx: &App) -> Option<HostsView> {
    let hosts = cx.try_global::<Hosts>().filter(|h| h.shown)?;
    let current = hosts.picked.clone();
    let account = wsp_host_client::account_hosts(&hosts.state, &hosts.env).into_iter().map(|(alias, _)| alias).collect();
    Some(HostsView { here: hosts.here.clone(), current, hosts: account })
}

/// `switcher.to`: the window on the host on the account under `alias` once one dial there answers, or back on this
/// computer for none; a refusal in the host's or the dial's words goes to the log and the window stays.
pub fn to(alias: Option<String>, cx: &mut App) {
    if !cx.has_global::<Hosts>() {
        return;
    }
    let hosts = cx.global_mut::<Hosts>();
    hosts.asked += 1;
    let asked = hosts.asked;
    let latest = move |cx: &App| cx.global::<Hosts>().asked == asked;
    // Deferred, as a pick made inside a window's update would find that window taken.
    let Some(alias) = alias else { return cx.defer(move |cx| moved(None, cx)) };
    let (state, env) = (hosts.state.clone(), hosts.env.clone());
    let dialled = hosts.runtime.spawn(async move { HostClient::connect_on(&state, &env, Some(&alias), None).await.map(|_| alias) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let answer = dialled.await.unwrap_or_else(|e| Err(Failure::new(e.to_string())));
        cx.update(|cx| match answer {
            _ if !latest(cx) => {}
            Ok(alias) => moved(Some(alias), cx),
            Err(failure) => {
                log::error(&failure.message);
                menu::refresh(cx);
            }
        });
    })
    .detach();
}

fn moved(alias: Option<String>, cx: &mut App) {
    let hosts = cx.global_mut::<Hosts>();
    // Home is the host the launch's dial finds, which on a computer serving no host is the account's one host.
    let alias = alias.or_else(|| wsp_host_client::default_alias(&hosts.state, &hosts.env));
    hosts.picked = alias.clone();
    let (state, env, runtime) = (hosts.state.clone(), hosts.env.clone(), hosts.runtime.clone());
    let at = match &alias {
        Some(alias) => wsp_host_client::account_hosts(&state, &env).into_iter().find(|(a, _)| a == alias).map(|(_, url)| url),
        None => crate::sidebar::host_page(&state),
    };
    let label = alias.clone().unwrap_or_else(|| hosts.here.clone());
    log::info(&format!("on {label} at {}", at.unwrap_or_default()));
    let mut windows = std::mem::take(&mut hosts.windows);
    windows.retain(|window| {
        let (state, env, runtime) = (state.clone(), env.clone(), runtime.clone());
        let built = window.update(cx, |_, window, cx| {
            window.replace_root(cx, |window, cx| {
                let shell = cx.new(|cx| Shell::new(state, env, runtime, Launch::default(), window, cx));
                links::attach(window, &shell, cx);
                vibrancy::clear_root(shell, window, cx)
            })
        });
        built.is_ok()
    });
    let kept = &mut cx.global_mut::<Hosts>().windows;
    windows.retain(|window| !kept.contains(window));
    kept.extend(windows);
    feed::follow_window(cx);
    menu::refresh(cx);
}
