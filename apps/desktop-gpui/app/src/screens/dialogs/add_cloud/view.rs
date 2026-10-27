// SPDX-License-Identifier: AGPL-3.0-only
//! What the dialog draws for each step: the provider radios and the key step with its check line, each in the step
//! dialog with its foot.

use gpui_kit::component::input::Paste;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Hsla, IntoElement, SharedString, Window};

use super::flow::{key_works_line, Check, Step};
use super::words;
use super::AddCloud;
use crate::screens::dialogs::layer;
use crate::ui::card::CardItem;
use crate::ui::Dialog;
use crate::ui::{size, spinner, text, ActivePalette, Button, Card, Choice, CopyRow, Field, Glyph, Refusal, Row};

/// `NOTE`: 13 px on a 20 px line.
fn note(words: impl Into<SharedString>, ink: Hsla) -> gpui_kit::Div {
    div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(ink).child(words.into())
}

/// A 14 px glyph in its ink.
fn mark(glyph: Glyph, ink: Hsla) -> Icon {
    Icon::new(glyph).with_size(px(14.)).text_color(ink)
}

/// The block under a row in the same card item: at the card's inset, 4 px up into the row's foot.
fn under_row(el: impl IntoElement + 'static) -> CardItem {
    CardItem::placed(move |corners| div().px(corners.inset).mt(px(-4.)).pb_4().child(el).into_any_element())
}

impl AddCloud {
    fn check_line(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = cx.palette().clone();
        let name = self.flow.cloud();
        let (glyph, words, bad): (AnyElement, String, bool) = match &self.flow.check {
            _ if self.flow.held() && self.flow.key.is_empty() => {
                (mark(Glyph::Check, p.foreground.opacity(0.8)).into_any_element(), "A key is saved. Continue uses it.".into(), false)
            }
            Check::Idle => return None,
            Check::Checking => (spinner(cx).into_any_element(), format!("Checking the key with {name}"), false),
            Check::Works(facts) => (mark(Glyph::Check, p.foreground.opacity(0.8)).into_any_element(), key_works_line(facts), false),
            Check::Refused(r) | Check::Unreachable(r) => {
                (mark(Glyph::CircleAlert, p.error_foreground).into_any_element(), r.said.clone(), true)
            }
        };
        let again = matches!(self.flow.check, Check::Unreachable(_)) && (!self.flow.held() || !self.flow.key.is_empty());
        let retry = again.then(|| {
            Button::outline("check-again", "Try again").xs().on_click(cx.listener(|this, _, window, cx| {
                let typed = this.flow.key.clone();
                if let Some(ask) = this.flow.type_key(&typed, true) {
                    this.send(ask, window, cx);
                }
                cx.notify();
            }))
        });
        Some(
            h_flex()
                .debug_selector(|| "key-check".into())
                .items_start()
                .gap_2()
                .child(div().flex().flex_none().items_center().justify_center().w(px(16.)).h(px(20.)).child(glyph))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .items_start()
                        .gap_2()
                        .child(note(words, if bad { p.error_foreground } else { p.muted_foreground }))
                        .children(retry),
                )
                .into_any_element(),
        )
    }

    fn key_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let name = self.flow.cloud();
        let mut out = Vec::new();
        if let Some(facts) = words::key_step(&self.flow.provider) {
            let page = facts.keys_page;
            out.push(
                Card::new("make-key")
                    .head("Make a key")
                    .inset(size::DIALOG_INSET)
                    .item(
                        Row::new("keys-page", format!("Open {name}'s API keys page")).note(page).control(
                            Button::outline("open-keys-page", "Open")
                                .xs()
                                .glyph(Glyph::ExternalLink)
                                .on_click(move |_, _, cx| cx.open_url(&format!("https://{page}"))),
                        ),
                    )
                    .item(Row::new("admin-key", "Make an admin key").note(format!(
                        "Pick the admin preset. Or make a new key with the {} preset, which holds every action wsp needs:",
                        facts.preset
                    )))
                    .item(under_row(CopyRow::new("key-actions", facts.actions.join(", "))))
                    .item(Row::new("key-cost", "What it costs").note(facts.cost).fact(facts.from))
                    .into_any_element(),
            );
        }
        let held = self.flow.held();
        let placeholder: SharedString =
            if held { "Paste a new key to replace the saved one".into() } else { words::key_name(&self.flow.provider).into() };
        if placeholder != self.placeholder {
            self.placeholder = placeholder.clone();
            self.key_input.update(cx, |s, cx| s.set_placeholder(placeholder, window, cx));
        }
        let refused = matches!(self.flow.check, Check::Refused(_));
        let field = div()
            .debug_selector(|| "cloud-key".into())
            .capture_action(cx.listener(|this, _: &Paste, _, _| this.pasted = true))
            .child(Field::new(&self.key_input).sheet().lead(Glyph::KeyRound).invalid(refused));
        let line = self.check_line(cx);
        out.push(
            Card::new("key")
                .head("Key")
                .inset(size::DIALOG_INSET)
                .item(CardItem::placed(move |corners| {
                    v_flex().px(corners.inset).py_4().gap_2().child(field).children(line).into_any_element()
                }))
                .into_any_element(),
        );
        if let Some(refused) = &self.flow.refused {
            let refusal = Refusal::new().said(refused.said.clone());
            let refusal = match refused.fix.clone() {
                Some(fix) => refusal.fix(fix),
                None => refusal,
            };
            out.push(div().debug_selector(|| "key-refused".into()).child(refusal).into_any_element());
        }
        out
    }

    fn provider_step(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let items: Vec<Choice> = self
            .flow
            .offered()
            .into_iter()
            .map(|id| {
                let choice = Choice::new(id, words::cloud_name(id), self.flow.provider == id).glyph(Glyph::Cloud).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.flow.pick_provider(id);
                        cx.notify();
                    },
                ));
                match words::key_step(id) {
                    Some(step) => choice.fact(step.from),
                    None => choice,
                }
            })
            .collect();
        Card::new("providers").inset(size::DIALOG_INSET).items(items).into_any_element()
    }
}

impl Render for AddCloud {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, line) = self.flow.head();
        let figure = self.flow.figure();
        let mut dialog =
            Dialog::new(title).layered().frame().count_words(figure.clone(), format!("Step {figure}")).on_outside(layer::dismiss);
        if let Some(line) = line {
            dialog = dialog.line(line);
        }
        let body: Vec<AnyElement> = match self.flow.step {
            Step::Provider => vec![self.provider_step(cx)],
            Step::Key => self.key_step(window, cx),
        };
        dialog = dialog.children(body);
        if let Some(said) = self.flow.setup_refused.clone().filter(|_| self.flow.step != Step::Key) {
            dialog = dialog.child(div().debug_selector(|| "cloud-refusal".into()).child(Refusal::new().said(said)));
        }
        let (acts, primary) = self.foot(cx);
        for act in acts {
            dialog = dialog.act(act);
        }
        dialog = dialog.act(div().debug_selector(|| "cloud-continue".into()).child(primary));
        dialog.into_any_element()
    }
}

impl AddCloud {
    /// The foot's acts: Back or Cancel, then the one primary.
    fn foot(&mut self, cx: &mut Context<Self>) -> (Vec<Button>, Button) {
        let flow = &self.flow;
        let cancel = Button::outline("cancel", "Cancel").on_click(cx.listener(|this, _, window, cx| this.close(window, cx)));
        match flow.step {
            Step::Provider => (
                vec![cancel],
                Button::primary("continue", "Continue").on_click(cx.listener(|this, _, _, cx| {
                    this.flow.go(Step::Key);
                    cx.notify();
                })),
            ),
            Step::Key => {
                let held = !flow.key_ready() || flow.busy;
                let first = if flow.offered().len() > 1 {
                    Button::outline("back", "Back").on_click(cx.listener(|this, _, _, cx| {
                        this.flow.go(Step::Provider);
                        cx.notify();
                    }))
                } else {
                    cancel
                };
                let next = Button::primary("continue", "Continue").held(held).on_click(cx.listener(|this, _, window, cx| {
                    if let Some(ask) = this.flow.key_on() {
                        this.send(ask, window, cx);
                    }
                    cx.notify();
                }));
                (vec![first], next)
            }
        }
    }
}
