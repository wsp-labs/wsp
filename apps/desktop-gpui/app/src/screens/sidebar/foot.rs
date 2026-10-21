// SPDX-License-Identifier: AGPL-3.0-only
//! The foot's sections over the corner (`sidebar/SettingUpSection.tsx`, `UpdateCard.tsx`) and the forwarded ports
//! under the list (`sidebar/ForwardsList.tsx`).

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, svg, AnyElement, App, Context, SharedString, TestSupportExt as _};
use serde_json::{json, Value};
use wsp_host_client::protocol::{ForwardsStopRequest, HostRestartRequest, PreferencesSetRequest};

use super::age::{Count, Room};
use super::chrome::{self, ROW_RADIUS};
use super::{looks, model, Asked, Sidebar};
use crate::screens::notices::words::wait_line;
use crate::ui::icons::MARK;
use crate::ui::{ActivePalette, Button, Glyph};
use crate::update::{self, words, Act, Stage};

/// How a setup card reads: working, needing the person, or failed.
#[derive(Clone, Copy, PartialEq)]
enum Tone {
    Working,
    NeedsYou,
    Failed,
}

/// One computer's setup as its card draws it.
struct SetupCard {
    place: Value,
    tone: Tone,
    line: String,
    done: usize,
    of: usize,
}

/// The places with a setup the section shows (`settingUp`, off `placeWord`): failed, waiting on a sign-in, or
/// running, each with its line and how many of its steps are done, the row "wsp installed" counted as one.
fn setup_cards(places: &[Value]) -> Vec<SetupCard> {
    places
        .iter()
        .filter_map(|place| {
            let setup = place.get("setup").filter(|s| !s.is_null())?;
            let state = setup.get("state").and_then(Value::as_str).unwrap_or_default();
            let steps = setup.get("steps").and_then(Value::as_array).cloned().unwrap_or_default();
            let wait = setup.get("waiting").and_then(Value::as_array).and_then(|w| w.first().cloned());
            let absent = place.get("present").and_then(Value::as_bool) == Some(false);
            let (tone, line) = if state == "failed" {
                (Tone::Failed, setup.get("said").and_then(Value::as_str).map_or_else(|| "Setup failed".to_owned(), model::capitalised))
            } else if let Some(w) = wait {
                (Tone::NeedsYou, model::capitalised(&wait_line(&w)))
            } else if absent || state != "running" {
                return None;
            } else {
                let under = steps.iter().find(|s| s.get("state").and_then(Value::as_str) == Some("running"));
                let word = under
                    .and_then(|s| s.get("step")?.as_str())
                    .and_then(model::setup_step_words)
                    .map_or_else(|| "setting up".to_owned(), |w| format!("setting up {w}"));
                (Tone::Working, model::capitalised(&word))
            };
            // `setupRows`: "wsp installed", then a row for each of the setup's nine steps, listed or not yet.
            let done = 1 + model::SETUP_ROWS
                .iter()
                .filter(|(r, _)| {
                    steps.iter().any(|s| {
                        s.get("step").and_then(Value::as_str) == Some(*r) && s.get("state").and_then(Value::as_str) == Some("done")
                    })
                })
                .count();
            Some(SetupCard { place: place.clone(), tone, line, done, of: 1 + model::SETUP_ROWS.len() })
        })
        .collect()
}

/// What the update card says for a stage (`UpdateCard`): the title, the line under it, the act's word, the act's
/// hover, and whether the act is held while the download runs.
#[derive(Debug, PartialEq)]
pub(crate) struct CardWords {
    pub title: String,
    pub line: Option<String>,
    pub act: &'static str,
    pub hover: Option<&'static str>,
    pub held: bool,
}

/// A restart into the checked copy carries the running threads on; an app that cannot replace itself says why beside
/// Get.
pub(crate) fn card_words(stage: &Stage, running: usize, why: Option<&'static str>, hover: Option<&'static str>) -> CardWords {
    let title = if update::ready(stage.act) { words::ready(&stage.version) } else { words::out(&stage.version) };
    let line = match stage.act {
        Act::Restart => (running > 0).then(|| words::keep_running(running)),
        Act::Get => why.map(str::to_owned),
        _ => None,
    };
    let act = match stage.act {
        Act::Link | Act::Get => words::GET,
        Act::Downloading => words::DOWNLOADING,
        Act::Open => words::QUIT_AND_OPEN,
        Act::Restart | Act::RestartHost => words::RESTART,
    };
    CardWords { title, line, act, hover: hover.filter(|_| stage.act == Act::Get), held: stage.act == Act::Downloading }
}

impl Sidebar {
    /// The release this sidebar's host last read, as the update card and the background download take it.
    pub(crate) fn update_stage(&self, cx: &App) -> Option<Stage> {
        let host = self.link.as_ref().and_then(|l| l.host.host_release());
        update::stage(self.around.release.as_ref(), host.as_deref(), self.here, cx)
    }

    /// The release the person dismissed, or nothing until the host's preferences are read (`useDismissed`).
    fn update_dismissed(&self) -> Option<Option<&str>> {
        self.around.preferences.as_deref().map(|p| p.get("updateDismissed").and_then(Value::as_str))
    }

    /// Whether the update card stands: a stage, once the preferences are read, the person has not dismissed.
    pub(crate) fn update_shown(&self, cx: &App) -> Option<Stage> {
        let dismissed = self.update_dismissed()?;
        self.update_stage(cx).filter(|s| dismissed != Some(s.version.as_str()))
    }

    /// `useUpdateInBackground`, run as the release or the preferences land.
    pub(crate) fn update_in_background(&self, cx: &mut Context<Self>) {
        let stage = self.update_stage(cx);
        update::background(stage.as_ref(), self.update_dismissed(), cx);
    }

    /// `dismissUpdate`: the host keeps the version for every window, and a copy the shell checked is deleted.
    pub(crate) fn dismiss_update(&mut self, stage: &Stage, cx: &mut Context<Self>) {
        if let Some(prefs) = self.around.preferences.as_mut().map(std::rc::Rc::make_mut).and_then(Value::as_object_mut) {
            prefs.insert("updateDismissed".to_owned(), json!(stage.version));
        }
        self.write_op::<PreferencesSetRequest>(json!({ "patch": { "updateDismissed": stage.version } }), cx);
        update::dismiss(stage, cx);
        cx.notify();
    }

    /// Restart host, its refusal said as a notice.
    fn restart_host(&mut self, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        let task = link.runtime.spawn(super::host::write::<HostRestartRequest>(link.host.clone(), json!({})));
        cx.spawn(async move |_, cx| {
            if let Err(failure) = task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                cx.update(|cx| crate::ui::notify(cx, crate::ui::Notice::error(words::not_restarted(&failure.message))));
            }
        })
        .detach();
    }

    /// The Setting up section: its head and, unfolded, a card per computer being set up, the list held to 196 px.
    pub(crate) fn setting_up_section(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let cards = setup_cards(&self.around.places);
        if cards.is_empty() {
            return None;
        }
        let p = cx.palette().clone();
        let folded = self.foot_folds.contains("setting-up");
        let mono = cx.theme().mono_font_family.clone();
        let prefs = self.around.preferences.clone().unwrap_or_default();
        let head = chrome::section_head("head-setting-up".into(), "Setting up", cards.len(), folded, cx)
            .debug_selector(|| "head-setting-up".into())
            .on_click(cx.listener(|this, _, _, cx| {
                let folded = !this.foot_folds.remove("setting-up");
                if folded {
                    this.foot_folds.insert("setting-up".to_owned());
                }
                this.keep_fold("setting-up", folded, cx);
                cx.notify();
            }));
        let rows = cards.into_iter().map(|c| {
            let (line_ink, bar) = match c.tone {
                Tone::Working => (p.sidebar_prose, p.sidebar_foreground),
                Tone::NeedsYou => (p.status_input, p.status_input),
                Tone::Failed => (p.status_failed, p.status_failed),
            };
            let id = c.place.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            let right: AnyElement = match c.tone {
                Tone::Working => div()
                    .flex_none()
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(p.sidebar_muted_foreground)
                    .child(format!("{}/{}", c.done, c.of))
                    .into_any_element(),
                Tone::Failed => div()
                    .flex_none()
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(p.status_failed)
                    .child("Setup failed")
                    .into_any_element(),
                Tone::NeedsYou => {
                    Icon::new(Glyph::MessageCircleQuestionMark).with_size(px(14.)).text_color(p.status_input).into_any_element()
                }
            };
            let share = if c.of == 0 { 0. } else { c.done as f32 / c.of as f32 };
            v_flex()
                .id(SharedString::from(format!("setup-{id}")))
                .gap(px(6.))
                .p(px(8.))
                .rounded(ROW_RADIUS)
                .cursor_pointer()
                .hover(|s| s.bg(p.sidebar_row_hover))
                .on_click(cx.listener(move |_, _, _, cx| cx.emit(Asked::Setup(id.clone()))))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(looks::computer_glyph(&c.place, &prefs, px(14.), p.sidebar_muted_foreground))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(14.))
                                .line_height(px(18.))
                                .text_color(p.sidebar_foreground)
                                .child(looks::place_name(&c.place)),
                        )
                        .child(right),
                )
                .child(div().truncate().text_size(px(12.)).line_height(px(16.)).text_color(line_ink).child(c.line))
                .child(
                    div()
                        .h(px(2.))
                        .w_full()
                        .rounded_full()
                        .bg(p.sidebar_border)
                        .child(div().h_full().w(relative(share)).rounded_full().bg(bar)),
                )
        });
        Some(
            v_flex()
                .child(head)
                .when(!folded, |el| el.child(v_flex().id("setting-up-list").gap(px(2.)).max_h(px(196.)).overflow_y_scroll().children(rows)))
                .into_any_element(),
        )
    }

    /// The update card (`UpdateCard.tsx`), in the cloud card's shape: the mark, "wsp {v} is out" or "is ready", the
    /// line under it where there is one, the stage's act and What's new.
    pub(crate) fn update_card(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let stage = self.update_shown(cx)?;
        let p = cx.palette().clone();
        let running = self.rows.iter().filter(|r| r.status == model::Status::Running).count();
        let said = card_words(&stage, running, update::why(cx), update::hover(cx));
        let me = cx.weak_entity();
        let pressed = stage.clone();
        let mut act = Button::outline("update-act", said.act).xs().held(said.held).on_click(move |_, _, cx| {
            if pressed.act == Act::RestartHost {
                _ = me.update(cx, |this, cx| this.restart_host(cx));
            } else {
                update::run(&pressed, cx);
            }
        });
        if let Some(hover) = said.hover {
            act = act.tooltip(hover);
        }
        let news = stage.url.clone();
        Some(
            v_flex()
                .id("update-card")
                .debug_selector(|| "update-card".into())
                .flex_none()
                .relative()
                .gap(px(4.))
                .px(px(8.))
                .pt(px(8.))
                .pb(px(10.))
                .rounded(ROW_RADIUS)
                .bg(p.sidebar_row_hover)
                .child(div().absolute().inset_0().rounded(ROW_RADIUS).border_1().border_color(p.sidebar_row_edge))
                .child(
                    h_flex()
                        .h(px(20.))
                        .gap(px(6.))
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(p.sidebar_foreground)
                        .child(svg().flex_none().path(MARK).size(px(14.)).text_color(p.sidebar_muted_foreground))
                        .child(div().debug_selector(|| "update-title".into()).flex_1().min_w_0().truncate().child(said.title))
                        .child(
                            div()
                                .id("update-dismiss")
                                .debug_selector(|| "update-dismiss".into())
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(20.))
                                .mr(px(-4.))
                                .rounded(px(6.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(p.accent))
                                .on_click(cx.listener(move |this, _, _, cx| this.dismiss_update(&stage, cx)))
                                .child(Icon::new(Glyph::X).with_size(px(12.)).text_color(p.muted_foreground)),
                        ),
                )
                .children(said.line.map(|line| {
                    div()
                        .debug_selector(|| "update-line".into())
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(p.sidebar_muted_foreground)
                        .child(line)
                }))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .pt(px(6.))
                        .child(act)
                        .child(Button::ghost("update-news", words::WHATS_NEW).xs().on_click(move |_, _, cx| cx.open_url(&news))),
                )
                .test_support()
                .into_any_element(),
        )
    }

    /// The forwarded ports' head, "Forwarded ports (n)", a row of the list.
    pub(crate) fn forwards_head(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let n = self.forwards_shown().count();
        let folded = self.foot_folds.contains("forwards");
        div()
            .pt(px(8.))
            .px(px(8.))
            .child(chrome::section_head("head-forwards".into(), "Forwarded ports", n, folded, cx).on_click(cx.listener(
                |this, _, _, cx| {
                    if !this.foot_folds.remove("forwards") {
                        this.foot_folds.insert("forwards".to_owned());
                    }
                    this.rebuild(cx);
                },
            )))
            .into_any_element()
    }

    /// The ports a person reads in the list: every forward but an editor's.
    pub(crate) fn forwards_shown(&self) -> impl Iterator<Item = &Value> {
        self.around.forwards.iter().filter(|f| f.get("kind").and_then(Value::as_str) != Some("editor"))
    }

    /// One forwarded port: `localhost:{port}` over its name and age, a link for a url, with the always-there X.
    pub(crate) fn forward_row(&mut self, at: usize, window: &mut gpui_kit::Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(f) = self.forwards_shown().nth(at).cloned() else { return div().into_any_element() };
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let port = f.get("port").and_then(Value::as_u64).unwrap_or_default();
        let workspace = f.get("workspaceId").and_then(Value::as_str).unwrap_or_default().to_owned();
        let name = f.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
        let started = f
            .get("startedAt")
            .and_then(Value::as_str)
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.timestamp_millis() as f64);
        let since = started.unwrap_or(self.now);
        let age = super::age::age(format!("forward-age-{port}:{since}"), Count::Age, super::now_ms() - since, Room::Start, window, cx);
        let age = super::age::rest(&age);
        let callback = f.get("kind").and_then(Value::as_str) == Some("callback");
        let url = format!("http://localhost:{port}/");
        let (first, second): (AnyElement, AnyElement) = if callback {
            (
                div()
                    .text_size(px(14.))
                    .line_height(px(18.))
                    .text_color(p.sidebar_foreground)
                    .child(format!("sign-in callback for {name}"))
                    .into_any_element(),
                h_flex()
                    .gap(px(8.))
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(p.sidebar_muted_foreground)
                    .child(format!("localhost:{port}"))
                    .child(age)
                    .into_any_element(),
            )
        } else {
            (
                div()
                    .font_family(mono.clone())
                    .text_size(px(13.))
                    .line_height(px(16.))
                    .text_color(p.sidebar_foreground)
                    .child(format!("localhost:{port}"))
                    .into_any_element(),
                h_flex()
                    .gap(px(8.))
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .text_color(p.sidebar_muted_foreground)
                    .child(name)
                    .child(age)
                    .into_any_element(),
            )
        };
        let stop = div()
            .id(("forward-stop", at))
            .absolute()
            .right(px(6.))
            .top(px(14.))
            .flex()
            .items_center()
            .justify_center()
            .size(px(20.))
            .rounded(px(10.))
            .cursor_pointer()
            .hover(|s| s.bg(p.sidebar_row_hover))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.write_op::<ForwardsStopRequest>(json!({ "workspaceId": workspace, "port": port }), cx);
            }))
            .child(Icon::new(Glyph::X).with_size(px(16.)).text_color(p.sidebar_foreground));
        let row = v_flex()
            .id(("forward", at))
            .h(px(48.))
            .justify_center()
            .gap(px(2.))
            .p(px(8.))
            .pr(px(32.))
            .rounded(px(10.))
            .when(!callback, |el| el.cursor_pointer().hover(|s| s.bg(p.sidebar_row_hover)).on_click(move |_, _, cx| cx.open_url(&url)))
            .child(first)
            .child(second);
        div().px(px(8.)).pt(px(4.)).child(div().relative().child(row).child(stop)).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setup_reads_failed_then_waiting_then_running() {
        let running = json!([{ "id": "b", "name": "box", "setup": { "state": "running", "steps": [
            { "step": "floor", "state": "done" }, { "step": "clis", "state": "running" }, { "step": "skills", "state": "running" } ], "waiting": [] } }]);
        let cards = setup_cards(running.as_array().unwrap());
        assert_eq!(cards[0].line, "Setting up the CLIs");
        assert_eq!((cards[0].done, cards[0].of), (2, 10));
        let waiting =
            json!([{ "id": "b", "setup": { "state": "running", "steps": [], "waiting": [{ "label": "Claude", "state": "waiting" }] } }]);
        assert!(setup_cards(waiting.as_array().unwrap())[0].tone == Tone::NeedsYou);
        let off = json!([{ "id": "b", "present": false, "setup": { "state": "running", "steps": [], "waiting": [] } }]);
        assert!(setup_cards(off.as_array().unwrap()).is_empty(), "a computer not answering reads its absence, not its setup");
    }

    /// Catches: an act under another word than `ACT_WORDS` gives it, a title that calls a release ready before it is
    /// on this computer, a hover beside any act but Get, a Downloading that takes a press, and the line under the title
    /// standing anywhere but under a Restart with threads running or a Get that cannot replace the app.
    #[test]
    fn the_update_card_says_each_acts_word_title_and_line() {
        let at = |act| Stage { version: "0.5.0".into(), url: "u".into(), act };
        let said = |act, running| card_words(&at(act), running, Some("Move wsp out of Downloads."), Some("hover"));
        let acts = [Act::Link, Act::Get, Act::Downloading, Act::Open, Act::Restart, Act::RestartHost];
        let words: Vec<_> = acts.iter().map(|&a| said(a, 0).act).collect();
        assert_eq!(words, ["Get", "Get", "Downloading", "Quit and open", "Restart", "Restart"]);
        let titles: Vec<_> = acts.iter().map(|&a| said(a, 0).title).collect();
        assert_eq!(titles[..3], ["wsp 0.5.0 is out", "wsp 0.5.0 is out", "wsp 0.5.0 is out"]);
        assert_eq!(titles[3..], ["wsp 0.5.0 is ready", "wsp 0.5.0 is ready", "wsp 0.5.0 is ready"]);
        let hovers: Vec<_> = acts.iter().map(|&a| said(a, 0).hover).collect();
        assert_eq!(hovers, [None, Some("hover"), None, None, None, None], "the road's hover stands over Get alone");
        let held: Vec<_> = acts.iter().map(|&a| said(a, 0).held).collect();
        assert_eq!(held, [false, false, true, false, false, false]);

        assert_eq!(said(Act::Restart, 2).line.as_deref(), Some("Your 2 running threads carry on. Open terminals close."));
        assert_eq!(said(Act::Restart, 1).line.as_deref(), Some("Your running thread carries on. Open terminals close."));
        assert_eq!(said(Act::Restart, 0).line, None, "no running thread, no line");
        assert_eq!(said(Act::RestartHost, 2).line, None, "a host restart says nothing of the threads");
        assert_eq!(
            said(Act::Get, 2).line.as_deref(),
            Some("Move wsp out of Downloads."),
            "why the app cannot replace itself stands beside Get"
        );
        assert_eq!(card_words(&at(Act::Get), 0, None, None).line, None);
        for act in [Act::Link, Act::Downloading, Act::Open] {
            assert_eq!(said(act, 2).line, None, "{act:?} has no line");
        }
    }
}
