// SPDX-License-Identifier: AGPL-3.0-only
//! `SignInFlowView`: one sign-in as it stands under its row or its detail's head: the line for a terminal, the paste
//! of a token or key under the line that mints it, a login that asks the person to pick in a terminal drawn in place,
//! or a watched run's code with Open, the wait on the browser, and the field its page's answer goes back through; a
//! failure in the refusal slot in the tool's own words.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};

use super::keyed;
use super::model::w;
use super::parts::{code_field, field, refusal, xs_label};
use super::store::{Flow, Store};
use crate::ui::{spinner, Button, CopyRow, Glyph};

/// The sign-in of `row` on `place`; `reserve` keeps the refusal slot's room while it is empty.
pub fn view(store: &Entity<Store>, place: &str, row: &str, flow: &Flow, reserve: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let ground = crate::ui::ActivePalette::palette(cx).card;
    let k = |part: &str| SharedString::from(format!("sign-in-{place}-{row}-{part}"));
    match flow {
        Flow::Copy { line, why } => v_flex()
            .gap(px(8.))
            .child(xs_label(why.clone().unwrap_or_else(|| w::RUN_IN_TERMINAL.to_owned()), cx))
            .child(CopyRow::new(k("line"), line.clone()).on(ground))
            .into_any_element(),
        Flow::Vault { mint, word, saving, refused, .. } => {
            let ask = if *word == "token" { w::PASTE_TOKEN } else { w::PASTE_KEY };
            let save = {
                let (store, place, row) = (store.clone(), place.to_owned(), row.to_owned());
                move |key: String, cx: &mut App| store.update(cx, |s, cx| s.save(&place, &row, key, cx))
            };
            let enter = {
                let save = save.clone();
                move |field: &Entity<gpui_kit::component::input::InputState>, _: &mut Window, cx: &mut App| {
                    let typed = keyed::words(field, cx).trim().to_owned();
                    if !typed.is_empty() {
                        save(typed, cx);
                    }
                }
            };
            let input = keyed::typed(k("key"), "", "", true, enter, window, cx);
            let typed = keyed::words(&input, cx).trim().to_owned();
            let held = typed.is_empty() || *saving;
            let mut button = Button::outline(k("save"), w::SAVE).xs().glyph(Glyph::Check).busy(*saving).held(held);
            if !held {
                button = button.on_click(move |_, _, cx| save(typed.clone(), cx));
            }
            let mut parts = Vec::new();
            if let Some(mint) = mint {
                parts.push(
                    v_flex()
                        .gap(px(8.))
                        .child(xs_label(w::RUN_IN_TERMINAL, cx))
                        .child(CopyRow::new(k("mint"), mint.clone()).on(ground))
                        .into_any_element(),
                );
            }
            let mono = cx.theme().mono_font_family.clone();
            parts.push(
                v_flex()
                    .gap(px(8.))
                    .child(xs_label(ask, cx))
                    .child(h_flex().gap_3().child(div().flex_1().min_w_0().font_family(mono).child(field(&input, None, cx))).child(button))
                    .into_any_element(),
            );
            if reserve || refused.is_some() {
                parts.push(refusal(refused.as_deref()));
            }
            v_flex().gap(px(12.)).children(parts).into_any_element()
        }
        Flow::Run { state, pty: Some(pty), .. } if state != "failed" => {
            let host = store.read(cx).acts().host(cx).map(|(client, runtime)| crate::screens::Host::new(client, runtime));
            v_flex().gap(px(8.)).child(super::terminal::view(host, k("terminal"), place, pty, window, cx)).into_any_element()
        }
        Flow::Run { state, finish, pastes, url, code, paste, said, .. } => {
            let failed = state == "failed";
            let open = |word: &str| {
                url.clone().map(|url| {
                    Button::outline(k("open"), word.to_owned())
                        .xs()
                        .glyph(Glyph::ExternalLink)
                        .on_click(move |_, _, cx| cx.open_url(&url))
                        .into_any_element()
                })
            };
            let browser = *finish == Some("callback") && !failed;
            let line = reserve || !failed || url.is_some();
            let paste_line = (*pastes || *paste) && (reserve || !failed);
            let mut parts = Vec::new();
            if line {
                let mut row = h_flex().h(px(40.)).gap_3();
                if browser {
                    row = row.child(spinner(cx).into_any_element()).child(xs_label(w::FINISH_IN_BROWSER, cx)).children(open(w::OPEN_PAGE));
                } else if url.is_none() {
                    if state == "running" {
                        row = row.child(spinner(cx).into_any_element()).children(said.clone().map(|s| xs_label(s, cx)));
                    }
                } else {
                    if let Some(code) = code {
                        row = row
                            .child(div().font_family(cx.theme().mono_font_family.clone()).text_size(px(20.)).child(code.clone()))
                            .child(CopyCode { id: k("copy-code"), code: code.clone() });
                    }
                    row = row.children(open(w::OPEN));
                }
                parts.push(row.into_any_element());
            }
            if paste_line {
                let mut pasting = h_flex().h(px(40.));
                if *paste && state == "waiting" {
                    let ask = if finish.is_some() { w::LANDED_ADDRESS } else { w::CODE_ASK };
                    let send = {
                        let (store, place, row) = (store.clone(), place.to_owned(), row.to_owned());
                        move |input: &Entity<gpui_kit::component::input::InputState>, window: &mut Window, cx: &mut App| {
                            let typed = keyed::words(input, cx).trim().to_owned();
                            if typed.is_empty() {
                                return;
                            }
                            keyed::clear(input, window, cx);
                            store.update(cx, |s, cx| s.code(&place, &row, typed, cx));
                        }
                    };
                    let input = keyed::typed(k("code"), ask, "", false, send.clone(), window, cx);
                    let held = keyed::words(&input, cx).trim().is_empty();
                    let mut submit = Button::outline(k("submit"), w::CODE_SUBMIT).xs().held(held);
                    if !held {
                        let input = input.clone();
                        submit = submit.on_click(move |_, window, cx| send(&input, window, cx));
                    }
                    let submit = div().debug_selector(move || if held { "code-submit-held" } else { "code-submit" }.into()).child(submit);
                    pasting = pasting.flex_1().min_w_0().gap_3().child(code_field(&input, cx)).child(submit);
                }
                parts.push(pasting.into_any_element());
            }
            if reserve || failed {
                parts.push(refusal(if failed { said.as_deref() } else { None }));
            }
            v_flex().gap(px(8.)).children(parts).into_any_element()
        }
    }
}

/// How long Copy shows its check after it took (`COPIED_MS`).
const COPIED: std::time::Duration = std::time::Duration::from_millis(1_400);

/// `DeviceCode`'s Copy: the code to the clipboard, its glyph the check a moment after it took.
#[derive(IntoElement)]
struct CopyCode {
    id: SharedString,
    code: String,
}

impl RenderOnce for CopyCode {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let copied = window.use_keyed_state(SharedString::from(format!("{}-copied", self.id)), cx, |_, _| (0u32, false));
        let shown = copied.read(cx).1;
        let code = self.code;
        let button =
            Button::outline(self.id, w::COPY).xs().glyph(if shown { Glyph::Check } else { Glyph::Copy }).on_click(move |_, window, cx| {
                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(code.clone()));
                let press = copied.update(cx, |c, cx| {
                    *c = (c.0 + 1, true);
                    cx.notify();
                    c.0
                });
                let copied = copied.clone();
                window
                    .spawn(cx, async move |cx| {
                        cx.background_executor().timer(COPIED).await;
                        copied.update(cx, |c, cx| {
                            if c.0 == press {
                                c.1 = false;
                                cx.notify();
                            }
                        });
                    })
                    .detach();
            });
        div().debug_selector(move || if shown { "copy-code-copied" } else { "copy-code" }.to_owned()).child(button)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gpui_kit::{Context, TestAppContext};

    use super::*;
    use crate::screens::settings::Acts;

    struct Shown {
        store: Entity<Store>,
        flow: Flow,
    }

    impl Render for Shown {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().w(px(600.)).child(view(&self.store, "here", "agent-claude", &self.flow, false, window, cx))
        }
    }

    #[gpui_kit::test]
    fn a_device_codes_copy_turns_to_a_check_and_the_paste_field_is_the_recipe_fields(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let store = cx.update(|cx| cx.new(|_| Store::new(Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime))));
        let flow = Flow::Run {
            id: Some("s1".into()),
            state: "waiting".into(),
            finish: None,
            pastes: true,
            url: Some("https://example.com/device".into()),
            code: Some("ABCD-1234".into()),
            paste: true,
            said: None,
            pty: None,
        };
        let shown = cx.update(|cx| cx.new(|_| Shown { store, flow }));
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(shown, window, cx));
        cx.simulate_resize(gpui_kit::size(px(800.), px(400.)));
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("code-field").expect("the paste field").size.height, px(32.), "FIELD's h-8");
        let copy = cx.debug_bounds("copy-code").expect("Copy beside the code").center();
        cx.simulate_click(copy, gpui_kit::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("ABCD-1234"));
        assert!(cx.debug_bounds("copy-code-copied").is_some(), "the copy shows its check");
        cx.executor().advance_clock(COPIED + std::time::Duration::from_millis(100));
        cx.run_until_parked();
        assert!(cx.debug_bounds("copy-code").is_some(), "and the copy glyph again after a moment");

        assert!(cx.debug_bounds("code-submit-held").is_some(), "Submit is held while the paste field is empty");
        let field = cx.debug_bounds("code-field").expect("the paste field").center();
        cx.simulate_click(field, gpui_kit::Modifiers::none());
        cx.simulate_input("4F2-K9");
        cx.run_until_parked();
        assert!(cx.debug_bounds("code-submit").is_some() && cx.debug_bounds("code-submit-held").is_none(), "and live once it holds a code");
    }
}
