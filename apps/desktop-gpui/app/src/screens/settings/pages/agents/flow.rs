// SPDX-License-Identifier: AGPL-3.0-only
//! `SignInFlowView`: one sign-in as it stands under its row or its detail's head: the line for a terminal, the paste
//! of a token or key under the line that mints it, or a watched run's code with Open, the wait on the browser, and
//! the field its page's answer goes back through; a failure in the refusal slot in the tool's own words.

use gpui_kit::component::{h_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};

use super::keyed;
use super::model::w;
use super::parts::{field, refusal, spin, stack, xs_label};
use super::store::{Flow, Store};
use crate::ui::{Button, CopyRow, Glyph};

/// The sign-in of `row` on `place`; `reserve` keeps the refusal slot's room while it is empty.
pub fn view(store: &Entity<Store>, place: &str, row: &str, flow: &Flow, reserve: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let ground = crate::ui::ActivePalette::palette(cx).card;
    let k = |part: &str| SharedString::from(format!("sign-in-{place}-{row}-{part}"));
    match flow {
        Flow::Copy { line, why } => stack(
            px(8.),
            vec![
                xs_label(why.clone().unwrap_or_else(|| w::RUN_IN_TERMINAL.to_owned()), cx),
                CopyRow::new(k("line"), line.clone()).on(ground).into_any_element(),
            ],
        ),
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
            let mut button =
                Button::outline(k("save"), w::SAVE).xs().glyph(if *saving { Glyph::LoaderCircle } else { Glyph::Check }).held(held);
            if !held {
                button = button.on_click(move |_, _, cx| save(typed.clone(), cx));
            }
            let mut parts = Vec::new();
            if let Some(mint) = mint {
                parts.push(stack(
                    px(8.),
                    vec![xs_label(w::RUN_IN_TERMINAL, cx), CopyRow::new(k("mint"), mint.clone()).on(ground).into_any_element()],
                ));
            }
            let mono = cx.theme().mono_font_family.clone();
            parts.push(stack(
                px(8.),
                vec![
                    xs_label(ask, cx),
                    h_flex()
                        .gap_3()
                        .child(div().flex_1().min_w_0().font_family(mono).child(field(&input, None, cx)))
                        .child(button)
                        .into_any_element(),
                ],
            ));
            if reserve || refused.is_some() {
                parts.push(refusal(refused.as_deref()));
            }
            stack(px(12.), parts)
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
                    row = row.child(spin(cx)).child(xs_label(w::FINISH_IN_BROWSER, cx)).children(open(w::OPEN_PAGE));
                } else if url.is_none() {
                    if state == "running" {
                        row = row.child(spin(cx)).children(said.clone().map(|s| xs_label(s, cx)));
                    }
                } else {
                    if let Some(code) = code {
                        let copy = code.clone();
                        row =
                            row.child(div().font_family(cx.theme().mono_font_family.clone()).text_size(px(20.)).child(code.clone())).child(
                                Button::outline(k("copy-code"), w::COPY)
                                    .xs()
                                    .glyph(Glyph::Copy)
                                    .on_click(move |_, _, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(copy.clone()))),
                            );
                    }
                    row = row.children(open(w::OPEN));
                }
                parts.push(row.into_any_element());
            }
            if paste_line {
                let mut pasting = h_flex().h(px(40.));
                if *paste && state == "waiting" {
                    let ask = if finish.is_some() { w::LANDED_ADDRESS } else { "Paste the code from the page" };
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
                    let mut submit = Button::outline(k("submit"), "Submit").xs().held(held);
                    if !held {
                        let input = input.clone();
                        submit = submit.on_click(move |_, window, cx| send(&input, window, cx));
                    }
                    pasting = pasting.gap_3().child(field(&input, None, cx)).child(submit);
                }
                parts.push(pasting.into_any_element());
            }
            if reserve || failed {
                parts.push(refusal(if failed { said.as_deref() } else { None }));
            }
            stack(px(8.), parts)
        }
    }
}
