// SPDX-License-Identifier: AGPL-3.0-only
//! The question a quit asks (`desktop:quit.ts`). wsp runs as this computer's own service, so quitting the app leaves
//! every thread working; the second answer stops the turns on this computer, then wsp, keeping its unit for the next
//! launch. A person who
//! always gives the same answer picks it on General and is asked nothing. Linux has no menu and no menu bar item, so
//! a quit there asks nothing, and a window on a host somewhere else quits with nothing to ask.

use std::collections::HashSet;
use std::path::Path;

use gpui_kit::{App, AsyncApp};
use serde_json::{json, Value};
use wsp_host_client::protocol::SessionsInterruptRequest;
use wsp_host_client::start::{self, Start, System};
use wsp_host_client::{Env, HostClient};

use super::feed::{self, OnQuit, QuitHost};
use crate::screens::sidebar::host::ask as call;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuitChoice {
    Quit,
    Stop,
    Cancel,
}

const CHOICES: [(QuitChoice, &str); 3] =
    [(QuitChoice::Quit, "Quit"), (QuitChoice::Stop, "Quit and stop wsp"), (QuitChoice::Cancel, "Cancel")];

pub const MESSAGE: &str = "Quit wsp?";
/// The box a stop that failed raises, with the stop's own words.
pub const NOT_STOPPED: &str = "wsp did not stop";

/// `QuitPrompt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuitPrompt {
    pub message: &'static str,
    pub detail: String,
    pub buttons: [&'static str; 3],
    pub default_id: usize,
    pub cancel_id: usize,
}

/// `quitPrompt`: the box for a quit while `working` turns run on this computer's workspaces.
pub fn quit_prompt(working: usize) -> QuitPrompt {
    let carry_on = match working {
        0 => "your threads carry on and the wsp command still answers. Quit and stop wsp stops it too.".to_owned(),
        1 => "the thread working on this computer carries on. Quit and stop wsp stops that thread, then wsp.".to_owned(),
        n => format!("the {n} threads working on this computer carry on. Quit and stop wsp stops those threads, then wsp."),
    };
    QuitPrompt {
        message: MESSAGE,
        detail: format!("wsp keeps running in the background after the app quits, so {carry_on}"),
        buttons: CHOICES.map(|(_, label)| label),
        default_id: 0,
        cancel_id: CHOICES.iter().position(|(c, _)| *c == QuitChoice::Cancel).unwrap_or(2),
    }
}

/// `quitChoice`: the answer a button's index is.
pub fn quit_choice(response: usize) -> QuitChoice {
    CHOICES.get(response).map_or(QuitChoice::Cancel, |(choice, _)| *choice)
}

/// `quitAnswer`: the person's standing answer on General, or none where the question is asked.
pub fn standing(on_quit: OnQuit) -> Option<QuitChoice> {
    match on_quit {
        OnQuit::Keep => Some(QuitChoice::Quit),
        OnQuit::Stop => Some(QuitChoice::Stop),
        OnQuit::Ask => None,
    }
}

/// `runningHere`: the turns running on this computer's own workspaces, by session id.
pub fn running_here(sessions: &[Value], workspaces: &[Value]) -> Vec<String> {
    let here: HashSet<&str> = workspaces
        .iter()
        .filter(|w| w.get("kind").and_then(Value::as_str) == Some("local"))
        .filter_map(|w| w.get("id").and_then(Value::as_str))
        .collect();
    sessions
        .iter()
        .filter(|s| s.get("status").and_then(Value::as_str) == Some("running"))
        .filter(|s| s.get("workspaceId").and_then(Value::as_str).is_some_and(|w| here.contains(w)))
        .filter_map(|s| s.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

/// The turns running on this computer and the client that reads them; none where no host answers.
async fn running(state: &Path, env: &Env) -> Option<(std::sync::Arc<HostClient>, Vec<String>)> {
    let client = std::sync::Arc::new(HostClient::connect(state, env).await.ok()?);
    let sessions = feed::listed(&client, "sessions.list", "sessions").await.ok()?;
    let workspaces = feed::listed(&client, "workspaces.list", "workspaces").await.ok()?;
    let running = running_here(&sessions, &workspaces);
    Some((client, running))
}

/// The turns running on this computer under a host of any release, or nothing where it would not say.
pub async fn working_any_release(state: &Path, env: &Env) -> Option<usize> {
    let sessions = wsp_host_client::listed_any_release(state, env, "sessions.list", "sessions").await.ok()?;
    let workspaces = wsp_host_client::listed_any_release(state, env, "workspaces.list", "workspaces").await.ok()?;
    Some(running_here(&sessions, &workspaces).len())
}

/// How many turns stopping wsp would stop; a count that fails reads as none.
pub async fn working_here(state: &Path, env: &Env) -> usize {
    running(state, env).await.map_or(0, |(_, running)| running.len())
}

/// `stopWsp`: every turn on this computer's workspaces interrupted, then the host stopped with its unit kept
/// ([`start::stop`]). Its words are the answer where the stop was refused or a host still serves after it.
pub async fn stop_wsp(state: &Path, env: &Env) -> Result<(), String> {
    if let Some((client, running)) = running(state, env).await {
        for session in running {
            if let Err(failure) = call::<SessionsInterruptRequest>(client.clone(), json!({ "sessionId": session })).await {
                super::log::error(&format!("quit: {}", failure.message));
            }
        }
    }
    start::stop(&Start::here(state, env), &System::new(env)).await
}

/// The menu's Quit and the menu bar's Quit wsp: asked per General's pick on this computer's own host, and never on
/// Linux, before the app's window first stood, or on a host somewhere else.
pub fn ask(cx: &mut App) {
    if cfg!(target_os = "linux") {
        cx.quit();
        return;
    }
    let Some(host) = feed::quit_host(cx).filter(|h| !h.remote) else {
        cx.quit();
        return;
    };
    match standing(host.on_quit) {
        Some(QuitChoice::Stop) => stop(host, cx),
        Some(_) => cx.quit(),
        None => {
            cx.spawn(async move |cx: &mut AsyncApp| {
                let (state, env) = (host.state.clone(), host.env.clone());
                let working = host.runtime.spawn(async move { working_here(&state, &env).await }).await.unwrap_or(0);
                let prompt = quit_prompt(working);
                let Some(picked) = asked(&prompt, cx).await else { return };
                cx.update(|cx| match quit_choice(picked) {
                    QuitChoice::Quit => cx.quit(),
                    QuitChoice::Stop => stop(host, cx),
                    QuitChoice::Cancel => {}
                });
            })
            .detach();
        }
    }
}

/// The question put to the person: an app-modal alert on a Mac, GPUI's own box on the window elsewhere.
async fn asked(prompt: &QuitPrompt, cx: &mut AsyncApp) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        _ = &cx;
        Some(super::mac::alert(super::mac::Style::Question, prompt.message, &prompt.detail, &prompt.buttons, Some(prompt.cancel_id)))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let buttons: Vec<gpui_kit::PromptButton> = prompt
            .buttons
            .iter()
            .enumerate()
            .map(
                |(at, label)| {
                    if at == prompt.cancel_id {
                        gpui_kit::PromptButton::cancel(*label)
                    } else {
                        gpui_kit::PromptButton::new(*label)
                    }
                },
            )
            .collect();
        let answer = cx.update(|cx| {
            cx.activate(true);
            let window = cx.windows().last().copied()?;
            window
                .update(cx, |_, window, cx| window.prompt(gpui_kit::PromptLevel::Info, prompt.message, Some(&prompt.detail), &buttons, cx))
                .ok()
        });
        match answer {
            Some(answer) => answer.await.ok(),
            None => Some(prompt.default_id),
        }
    }
}

/// Quit and stop wsp: the stop, its failure said in a box, then every window closed unasked and the app gone.
fn stop(host: QuitHost, cx: &mut App) {
    cx.spawn(async move |cx: &mut AsyncApp| {
        let (state, env) = (host.state.clone(), host.env.clone());
        let stopped = host.runtime.spawn(async move { stop_wsp(&state, &env).await }).await.unwrap_or_else(|e| Err(e.to_string()));
        if let Err(said) = stopped {
            super::log::error(&format!("{NOT_STOPPED}: {said}"));
            failed(&said, cx).await;
        }
        cx.update(|cx| {
            for window in cx.windows() {
                _ = window.update(cx, |_, window, _| window.remove_window());
            }
            cx.quit();
        });
    })
    .detach();
}

/// The "wsp did not stop" box, with the stop's error.
async fn failed(said: &str, cx: &mut AsyncApp) {
    #[cfg(target_os = "macos")]
    {
        _ = &cx;
        super::mac::alert(super::mac::Style::Failure, NOT_STOPPED, said, &["OK"], Some(0));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let answer = cx.update(|cx| {
            let window = cx.windows().last().copied()?;
            window
                .update(cx, |_, window, cx| {
                    window.prompt(gpui_kit::PromptLevel::Critical, NOT_STOPPED, Some(said), &[gpui_kit::PromptButton::ok("OK")], cx)
                })
                .ok()
        });
        if let Some(answer) = answer {
            _ = answer.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `quit.test.ts:6`.
    #[test]
    fn offers_quit_quit_and_stop_and_cancel_with_quit_the_default() {
        let prompt = quit_prompt(0);
        assert_eq!(prompt.buttons, ["Quit", "Quit and stop wsp", "Cancel"]);
        assert_eq!((0..3).map(quit_choice).collect::<Vec<_>>(), [QuitChoice::Quit, QuitChoice::Stop, QuitChoice::Cancel]);
        assert_eq!(quit_choice(prompt.default_id), QuitChoice::Quit);
        assert_eq!(quit_choice(prompt.cancel_id), QuitChoice::Cancel);
        assert_eq!(quit_choice(9), QuitChoice::Cancel);
    }

    /// `quit.test.ts:14`.
    #[test]
    fn counts_the_threads_working_on_this_computer() {
        assert_eq!(
            quit_prompt(0).detail,
            "wsp keeps running in the background after the app quits, so your threads carry on and the wsp command still answers. Quit and stop wsp stops it too."
        );
        assert!(quit_prompt(1)
            .detail
            .contains("the thread working on this computer carries on. Quit and stop wsp stops that thread, then wsp."));
        assert!(quit_prompt(3)
            .detail
            .contains("the 3 threads working on this computer carry on. Quit and stop wsp stops those threads, then wsp."));
    }

    /// `quit.test.ts:20`.
    #[test]
    fn never_names_the_parts_a_person_does_not_see() {
        for n in 0..3 {
            let said = format!("{} {}", quit_prompt(n).message, quit_prompt(n).detail).to_lowercase();
            assert!(!["host", "daemon", "service"].iter().any(|w| said.contains(w)), "{said}");
        }
    }

    /// `quit.test.ts:26`: asks only on Ask each time; Keep quits and Stop stops, unasked.
    #[test]
    fn the_standing_answer_asks_only_on_ask() {
        assert_eq!(standing(OnQuit::Ask), None);
        assert_eq!(standing(OnQuit::Keep), Some(QuitChoice::Quit));
        assert_eq!(standing(OnQuit::Stop), Some(QuitChoice::Stop));
    }

    #[test]
    fn only_turns_running_on_this_computers_own_workspaces_are_stopped() {
        let workspaces = [json!({ "id": "ws_mac", "kind": "local" }), json!({ "id": "ws_box", "kind": "cloud" })];
        let sessions = [
            json!({ "id": "s1", "workspaceId": "ws_mac", "status": "running" }),
            json!({ "id": "s2", "workspaceId": "ws_box", "status": "running" }),
            json!({ "id": "s3", "workspaceId": "ws_mac", "status": "completed" }),
        ];
        assert_eq!(running_here(&sessions, &workspaces), ["s1"]);
    }
}
