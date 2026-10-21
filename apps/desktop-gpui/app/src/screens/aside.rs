// SPDX-License-Identifier: AGPL-3.0-only
//! The side question `/btw` opens on its own tab of the right panel (`AsideSurface.tsx`, `asideStore.ts`): the
//! question, the crab with Asking and the seconds since it was asked until the first words, then the words as the
//! harness writes them with the crab and the seconds under them, then the whole answer, or the host's refusal in its
//! place. Esc while it shows, or its tab's close, takes it away; nothing of it is kept.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use futures::StreamExt as _;
use gpui_kit::base::text::TextViewState;
use gpui_kit::component::{v_flex, ActiveTheme as _, Root};
use gpui_kit::{div, prelude::*, px, relative, App, Context, Entity, Global, KeystrokeEvent, SharedString, Subscription, Task, Window};
use serde_json::json;
use wsp_host_client::protocol::SessionsAsideRequest;
use wsp_host_client::Event;

use crate::screens::thread_view::markdown;
use crate::screens::thread_view::rows::timer;
use crate::screens::{Host, Opened, Pane, Place, Screen};
use crate::shell::Shell;
use crate::ui::{ActivePalette, Crab, Refusal};

/// `ASIDE_WORDS`.
pub mod words {
    pub const NOTE: &str = "Answered from this thread's conversation. Nothing here is kept.";
    pub const ASKING: &str = "Asking";
    pub const WRITING: &str = "Answering";
    /// `ASIDE_NO_SESSION_LINE`.
    pub const NO_SESSION: &str =
        "this thread's agent has not started a conversation yet, so there is nothing to ask about; wait for its first reply";
}

/// `asideQuestion`: the question of a `/btw <question>` draft.
pub fn question(prompt: &str) -> Option<String> {
    let rest = prompt.trim().strip_prefix("/btw")?;
    let question = rest.trim_start();
    (rest.starts_with(char::is_whitespace) && !question.is_empty()).then(|| question.to_owned())
}

/// The side question standing in a workspace's panel.
#[derive(Clone)]
struct Aside {
    id: u64,
    question: String,
    asked: Instant,
    partial: Option<String>,
    answer: Option<String>,
    error: Option<String>,
    /// Whether the panel was open before the question, so closing it knows whether to shut the panel again.
    panel_was_open: bool,
    /// The tab that showed before the question, which closing it shows again while that tab still stands.
    before: Option<(Pane, SharedString)>,
}

/// Each workspace's standing side question, in memory only and app-wide: every window draws the same one.
#[derive(Default)]
struct Asides {
    by_workspace: HashMap<String, Aside>,
    asked: u64,
}

impl Global for Asides {}

fn standing(workspace: &str, cx: &App) -> Option<Aside> {
    cx.try_global::<Asides>()?.by_workspace.get(workspace).cloned()
}

fn shell_of(window: &Window, cx: &App) -> Option<Entity<Shell>> {
    window.root::<Root>().flatten()?.read(cx).view().clone().downcast::<Shell>().ok()
}

/// Asks the host beside the thread's latest session and opens the side question's tab on the answer; a question asked
/// over one still standing keeps the panel's state from before the first.
pub fn ask(host: &Host, workspace: &str, session: &str, question: String, window: &mut Window, cx: &mut App) {
    let panel = shell_of(window, cx).map(|shell| shell.read(cx).panel().clone());
    let before = standing(workspace, cx).map(|a| (a.panel_was_open, a.before)).unwrap_or_else(|| match &panel {
        Some(panel) => {
            let p = panel.read(cx);
            (p.is_open(), p.active_index().and_then(|ix| p.tabs().get(ix).cloned()))
        }
        None => (false, None),
    });
    let asides = cx.default_global::<Asides>();
    asides.asked += 1;
    let id = asides.asked;
    let ask_id = crate::screens::composer::link::request_id();
    let aside = Aside {
        id,
        question: question.clone(),
        asked: Instant::now(),
        partial: None,
        answer: None,
        error: None,
        panel_was_open: before.0,
        before: before.1,
    };
    cx.default_global::<Asides>().by_workspace.insert(workspace.to_owned(), aside);
    match panel {
        Some(panel) => panel.update(cx, |p, cx| p.add(Pane::Aside, None, window, cx)),
        None => crate::screens::navigate(crate::screens::Route::Screen(Pane::Aside.name().into()), cx),
    }
    let (grown, mut pieces) = futures::channel::mpsc::unbounded::<String>();
    let watched = ask_id.clone();
    let heard = host.run(move |client| async move {
        let mut events = client.events().await?;
        while let Some(next) = events.next().await {
            let frame = match next {
                Ok(Event::Host(event)) => serde_json::to_value(&event).unwrap_or_default(),
                Ok(Event::Other(frame)) => frame,
                _ => continue,
            };
            if frame["type"] == "aside.text"
                && frame["askId"] == watched.as_str()
                && grown.unbounded_send(frame["text"].as_str().unwrap_or_default().to_owned()).is_err()
            {
                break;
            }
        }
        Ok(())
    });
    let workspace = workspace.to_owned();
    let request: Result<SessionsAsideRequest, _> =
        serde_json::from_value(json!({ "sessionId": session, "question": question, "askId": ask_id }));
    let answered = host.run(move |client| async move {
        let request = request.map_err(|e| wsp_host_client::Failure::new(e.to_string()))?;
        client.call(request).await
    });
    // One task: once the answer lands it ends, and the host's event stream with it.
    cx.spawn(async move |cx| {
        let _heard = heard;
        let mut answered = Box::pin(answered);
        let reply = loop {
            match futures::future::select(pieces.next(), answered).await {
                futures::future::Either::Left((Some(text), rest)) => {
                    cx.update(|cx| grow(&workspace, id, &text, cx));
                    answered = rest;
                }
                futures::future::Either::Left((None, rest)) => break rest.await,
                futures::future::Either::Right((reply, _)) => break reply,
            }
        };
        let reply = match reply {
            Ok(answer) => Ok(serde_json::to_value(&answer).ok().and_then(|v| v["text"].as_str().map(str::to_owned)).unwrap_or_default()),
            Err(failure) => Err(failure.message),
        };
        cx.update(|cx| settle(&workspace, id, reply, cx));
    })
    .detach();
}

/// A piece of the standing question's answer; one for a question no longer standing lands nowhere.
fn grow(workspace: &str, id: u64, text: &str, cx: &mut App) {
    let Some(asides) = cx.try_global::<Asides>() else { return };
    if !asides.by_workspace.get(workspace).is_some_and(|a| a.id == id && a.answer.is_none() && a.error.is_none()) {
        return;
    }
    if let Some(aside) = cx.global_mut::<Asides>().by_workspace.get_mut(workspace) {
        aside.partial.get_or_insert_with(String::new).push_str(text);
    }
}

fn settle(workspace: &str, id: u64, reply: Result<String, String>, cx: &mut App) {
    let Some(asides) = cx.try_global::<Asides>() else { return };
    if !asides.by_workspace.get(workspace).is_some_and(|a| a.id == id) {
        return;
    }
    if let Some(aside) = cx.global_mut::<Asides>().by_workspace.get_mut(workspace) {
        match reply {
            Ok(answer) => aside.answer = Some(answer),
            Err(error) => aside.error = Some(error),
        }
    }
}

/// Takes the workspace's side question away: its tab goes, the tab from before shows again while it stands, and the
/// panel shuts again where the question opened it.
fn close(workspace: &str, window: &mut Window, cx: &mut App) {
    let Some(aside) = cx.try_global::<Asides>().and_then(|a| a.by_workspace.get(workspace).cloned()) else { return };
    cx.global_mut::<Asides>().by_workspace.remove(workspace);
    let Some(shell) = shell_of(window, cx) else { return };
    shell.read(cx).panel().clone().update(cx, |p, cx| {
        if let Some(ix) = p.tabs().iter().position(|(pane, _)| *pane == Pane::Aside) {
            p.close(ix, window, cx);
        }
        if let Some(ix) = aside.before.as_ref().and_then(|before| p.tabs().iter().position(|tab| tab == before)) {
            p.activate(ix, window, cx);
        }
        if !aside.panel_was_open && p.is_open() {
            p.toggle(window, cx);
        }
    });
}

/// `AsideSurface`: the tab's view of its workspace's side question.
pub struct AsideScreen {
    workspace: Option<String>,
    body: Option<(String, Entity<TextViewState>)>,
    _store: Subscription,
    _keys: Subscription,
    _tick: Task<()>,
}

impl AsideScreen {
    /// Whether its question still waits on the reply, whose seconds count up.
    fn asking(&self, cx: &App) -> bool {
        self.workspace.as_deref().and_then(|w| standing(w, cx)).is_some_and(|a| a.answer.is_none() && a.error.is_none())
    }

    fn shown(&self, window: &Window, cx: &App) -> bool {
        let Some(shell) = shell_of(window, cx) else { return false };
        let panel = shell.read(cx).panel().read(cx);
        panel.is_open() && panel.active() == Some(Pane::Aside)
    }

    fn on_key(&mut self, event: &KeystrokeEvent, window: &mut Window, cx: &mut Context<Self>) {
        let escape = event.keystroke.key == "escape" && !event.keystroke.modifiers.modified();
        let Some(workspace) = self.workspace.clone().filter(|_| escape && event.action.is_none()) else { return };
        if self.shown(window, cx) {
            close(&workspace, window, cx);
        }
    }

    fn body(&mut self, text: &str, cx: &mut Context<Self>) -> Entity<TextViewState> {
        if let Some((shown, state)) = &self.body {
            if shown == text {
                return state.clone();
            }
            let state = state.clone();
            state.update(cx, |s, cx| s.set_text(text, cx));
            self.body = Some((text.to_owned(), state.clone()));
            return state;
        }
        let state = cx.new(|cx| TextViewState::markdown(text, cx));
        self.body = Some((text.to_owned(), state.clone()));
        state
    }
}

impl Screen for AsideScreen {
    const NAME: &'static str = "aside";
    const PLACE: Place = Place::Panel(Pane::Aside);

    fn new(_host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        AsideScreen::on(opened.workspace_id, window, cx)
    }

    fn closed(_host: &Host, opened: Opened, cx: &mut App) {
        let Some(workspace) = opened.workspace_id else { return };
        if cx.try_global::<Asides>().is_some_and(|a| a.by_workspace.contains_key(&workspace)) {
            cx.global_mut::<Asides>().by_workspace.remove(&workspace);
        }
    }
}

impl AsideScreen {
    /// The tab of `workspace`'s side question.
    pub fn on(workspace: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let keys = cx.observe_keystrokes(|this, event, window, cx| this.on_key(event, window, cx));
        let tick = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            if this.update(cx, |this, cx| this.asking(cx).then(|| cx.notify_in_place())).is_err() {
                return;
            }
        });
        AsideScreen { workspace, body: None, _store: cx.observe_global::<Asides>(|_, cx| cx.notify()), _keys: keys, _tick: tick }
    }
}

impl Render for AsideScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("AsideScreen");
        let p = cx.palette().clone();
        let aside = self.workspace.as_deref().and_then(|w| standing(w, cx));
        let mut body = v_flex().id("aside-body").size_full().min_h_0().overflow_y_scroll().px(px(16.)).pt(px(8.)).pb(px(16.));
        body = body.child(
            div()
                .debug_selector(|| "aside-note".into())
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .child(words::NOTE),
        );
        let Some(aside) = aside else { return div().debug_selector(|| "aside-surface".into()).size_full().child(body) };
        body = body.child(
            div().w_full().flex().justify_end().mt(px(16.)).child(
                div()
                    .debug_selector(|| "aside-question".into())
                    .max_w(relative(0.85))
                    .rounded(px(16.))
                    .bg(p.accent)
                    .px(px(12.))
                    .py(px(8.))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(p.foreground)
                    .child(aside.question.clone()),
            ),
        );
        let answer = match (&aside.answer, &aside.error) {
            (Some(answer), _) => {
                let state = self.body(answer, cx);
                div().debug_selector(|| "aside-answer".into()).min_w_0().child(markdown::view(&state, false, None, cx)).into_any_element()
            }
            (None, Some(error)) => {
                div().debug_selector(|| "aside-refused".into()).child(Refusal::new().said(error.clone())).into_any_element()
            }
            (None, None) => {
                let partial = aside.partial.as_deref().map(|text| {
                    let state = self.body(text, cx);
                    div().debug_selector(|| "aside-partial".into()).mb(px(12.)).min_w_0().child(markdown::view(&state, false, None, cx))
                });
                let elapsed = timer(aside.asked.elapsed().as_millis() as f64);
                v_flex()
                    .children(partial)
                    .child(
                        gpui_kit::component::h_flex()
                            .debug_selector(|| "aside-asking".into())
                            .h(px(24.))
                            .gap(px(8.))
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(p.muted_foreground)
                            .child(Crab::new(p.status_working))
                            .child(if aside.partial.is_none() { words::ASKING } else { words::WRITING })
                            .child(
                                div()
                                    .debug_selector(|| "aside-elapsed".into())
                                    .font_family(cx.theme().mono_font_family.clone())
                                    .child(elapsed),
                            ),
                    )
                    .into_any_element()
            }
        };
        body = body.child(div().mt(px(16.)).child(answer));
        div().debug_selector(|| "aside-surface".into()).size_full().child(body)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use gpui_kit::component::Root;
    use gpui_kit::{px, size, AppContext as _, TestAppContext};

    use super::{question, settle, words, Aside, AsideScreen, Asides};
    use crate::ui::Side;

    #[gpui_kit::test]
    fn the_tab_asks_with_the_crab_then_says_the_refusal_in_its_place(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        cx.update(|cx| crate::ui::init(Side::Light, cx));
        let aside = Aside {
            id: 1,
            question: "what is this?".into(),
            asked: Instant::now(),
            partial: None,
            answer: None,
            error: None,
            panel_was_open: false,
            before: None,
        };
        cx.update(|cx| cx.default_global::<Asides>().by_workspace.insert("w".into(), aside));
        let mut made = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let screen = cx.new(|cx| AsideScreen::on(Some("w".into()), window, cx));
            made = Some(screen.clone());
            Root::new(screen, window, cx)
        });
        made.unwrap();
        let three_seconds = |cx: &mut gpui_kit::VisualTestContext| {
            crate::frames::take();
            cx.executor().advance_clock(std::time::Duration::from_secs(3));
            cx.run_until_parked();
            crate::frames::take().get("AsideScreen").copied().unwrap_or(0)
        };
        cx.simulate_resize(size(px(420.), px(700.)));
        cx.run_until_parked();
        for shown in ["aside-note", "aside-question", "aside-asking", "aside-elapsed"] {
            assert!(cx.debug_bounds(shown).is_some(), "no {shown} while it asks");
        }
        let (note, question) = (cx.debug_bounds("aside-note").unwrap(), cx.debug_bounds("aside-question").unwrap());
        assert!(question.top() >= note.bottom() + px(16.), "the bubble stands 16 px under the note");
        assert!(question.right() > px(380.), "the bubble keeps to the right");
        assert!(three_seconds(cx) >= 3, "the seconds did not count while it asks");
        cx.update(|_, cx| settle("w", 1, Err("the agent keeps no session to ask beside".into()), cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("aside-refused").is_some(), "the refusal in the answer's place");
        assert!(cx.debug_bounds("aside-asking").is_none(), "the crab went with the reply");
        assert_eq!(three_seconds(cx), 0, "the tab drew again each second after the reply landed");
        cx.update(|_, cx| settle("w", 7, Ok("late".into()), cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("aside-answer").is_none(), "an answer to another question landed here");
    }

    #[test]
    fn a_side_question_is_btw_and_words_after_it() {
        assert_eq!(question("/btw what is this?").as_deref(), Some("what is this?"));
        assert_eq!(question("  /btw   two\nlines ").as_deref(), Some("two\nlines"));
        assert_eq!(question("/btw"), None);
        assert_eq!(question("/btw   "), None);
        assert_eq!(question("/btwx no"), None);
        assert_eq!(question("ask /btw no"), None);
    }

    #[test]
    fn the_side_questions_words_are_todays() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let surface = std::fs::read_to_string(root.join("apps/web/src/components/chat/AsideSurface.tsx")).unwrap();
        for word in [words::NOTE, words::ASKING, words::WRITING] {
            assert!(surface.contains(&format!("\"{word}\"")), "{word}");
        }
        let thread = std::fs::read_to_string(root.join("packages/protocol/src/words/thread.ts")).unwrap();
        assert!(thread.contains(&format!("ASIDE_NO_SESSION_LINE = \"{}\"", words::NO_SESSION)));
    }
}
