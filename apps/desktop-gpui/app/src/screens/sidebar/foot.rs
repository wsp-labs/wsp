// SPDX-License-Identifier: AGPL-3.0-only
//! The foot's sections over the corner (`sidebar/SettingUpSection.tsx`, `UpdateCard.tsx`) and the forwarded ports
//! under the list (`sidebar/ForwardsList.tsx`).

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, svg, AnyElement, Context, SharedString};
use serde_json::{json, Value};
use wsp_host_client::protocol::{ForwardsStopRequest, HostRestartRequest, PreferencesSetRequest};

use super::age::{Count, Room};
use super::chrome::{self, ROW_RADIUS};
use super::{looks, model, Asked, Sidebar};
use crate::screens::notices::words::wait_line;
use crate::ui::icons::LOCKUP;
use crate::ui::{ActivePalette, Button, Glyph};

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

/// The update the foot offers (`shell/update.ts` updateStage, for a page that is no desktop bundle): newer files
/// under the host, which a restart brings in, else a newer release the host is behind.
fn update_stage(release: Option<&Value>) -> Option<(String, String, bool)> {
    let release = release?;
    let url = release.pointer("/latest/url").and_then(Value::as_str).unwrap_or("https://github.com/wsp-labs/wsp/releases").to_owned();
    if let Some(installed) = release.get("installed").and_then(Value::as_str) {
        if release.get("restartRefusal").is_none() {
            return Some((installed.to_owned(), url, true));
        }
    }
    release.get("update")?;
    Some((release.pointer("/latest/version")?.as_str()?.to_owned(), url, false))
}

impl Sidebar {
    /// Whether the update card stands: a stage the person has not dismissed for its version.
    pub(crate) fn update_shown(&self) -> Option<(String, String, bool)> {
        let dismissed = self.around.preferences.as_ref()?.get("updateDismissed").and_then(Value::as_str).map(str::to_owned);
        update_stage(self.around.release.as_ref()).filter(|(v, _, _)| dismissed.as_deref() != Some(v.as_str()))
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
        let head = chrome::section_head("head-setting-up".into(), "Setting up", cards.len(), folded, cx).on_click(cx.listener(
            |this, _, _, cx| {
                if !this.foot_folds.remove("setting-up") {
                    this.foot_folds.insert("setting-up".to_owned());
                }
                cx.notify();
            },
        ));
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

    /// The update card, in the cloud card's shape: the mark, "wsp {v} is out" or "is ready", Get or Restart, and
    /// What's new.
    pub(crate) fn update_card(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (version, url, restart) = self.update_shown()?;
        let p = cx.palette().clone();
        let title = format!("wsp {version} {}", if restart { "is ready" } else { "is out" });
        let (open, news) = (url.clone(), url);
        let dismissed = version.clone();
        let act = if restart {
            Button::outline("update-act", "Restart")
                .xs()
                .on_click(cx.listener(|this, _, _, cx| this.write_op::<HostRestartRequest>(json!({}), cx)))
        } else {
            Button::outline("update-act", "Get").xs().on_click(move |_, _, cx| cx.open_url(&open))
        };
        Some(
            v_flex()
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
                        // The tilde alone: the lockup's first 16 of its 51 px.
                        .child(
                            div()
                                .flex_none()
                                .w(px(14.))
                                .h(px(14.))
                                .overflow_hidden()
                                .child(svg().path(LOCKUP).w(px(51.)).h(px(14.)).text_color(p.muted_foreground)),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(title))
                        .child(
                            div()
                                .id("update-dismiss")
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(20.))
                                .mr(px(-4.))
                                .rounded(px(6.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(p.accent))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(prefs) =
                                        this.around.preferences.as_mut().map(std::rc::Rc::make_mut).and_then(Value::as_object_mut)
                                    {
                                        prefs.insert("updateDismissed".to_owned(), json!(dismissed));
                                    }
                                    this.write_op::<PreferencesSetRequest>(json!({ "patch": { "updateDismissed": dismissed } }), cx);
                                }))
                                .child(Icon::new(Glyph::X).with_size(px(12.)).text_color(p.muted_foreground)),
                        ),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .pt(px(6.))
                        .child(act)
                        .child(Button::ghost("update-news", "What's new").on_click(move |_, _, cx| cx.open_url(&news))),
                )
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

    #[test]
    fn the_update_card_offers_a_restart_before_a_release() {
        assert_eq!(update_stage(Some(&json!({ "state": "read", "installed": "0.4.1" }))).map(|s| s.2), Some(true));
        let behind = json!({ "state": "read", "latest": { "version": "0.4.2", "url": "u" }, "update": "npm i -g" });
        assert_eq!(update_stage(Some(&behind)), Some(("0.4.2".to_owned(), "u".to_owned(), false)));
        assert_eq!(update_stage(Some(&json!({ "state": "read", "latest": { "version": "0.4.2", "url": "u" } }))), None);
    }
}
