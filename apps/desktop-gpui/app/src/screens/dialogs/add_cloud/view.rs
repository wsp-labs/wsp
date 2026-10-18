// SPDX-License-Identifier: AGPL-3.0-only
//! What the dialog draws for each step: the provider radios, the key step with its check line, a pick screen, the
//! summary with the sign-ins card, the build rows and the ready page, each in the step dialog with its foot.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::input::Paste;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, img, prelude::*, px, AnyElement, App, Context, Hsla, Image, IntoElement, SharedString, Window};

use super::flow::{build_rows, key_works_line, Check, RowState, Step};
use super::sign_in::{self, Flow as SignIn, RunState, SignInView, Start};
use super::words::{self, ALWAYS_ON, ALWAYS_ON_WHY, SIGN_IN_STAGE_ID};
use super::AddCloud;
use crate::screens::dialogs::layer;
use crate::screens::dialogs::picks::{Changes, PickItem, PickList, RANGE_HOVER};
use crate::screens::dialogs::steps::{ReadyPage, StepLog};
use crate::ui::card::CardItem;
use crate::ui::{catalog, size, spinner, text, ActivePalette, Button, Card, Choice, CopyRow, Field, Glyph, Line, PickRow, Refusal, Row};
use crate::ui::{Dialog, Step as StepState, StepRow};

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

/// What one pick row draws besides its name and tick, worked out before the list draws.
struct RowLook {
    note: Option<String>,
    tag: Option<String>,
    size: Option<(String, Hsla)>,
    mark: Option<Arc<Image>>,
    hover: String,
    lock_on: bool,
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
            Button::outline("check-again", "Try again").xs().on_click(cx.listener(|this, _, _, cx| {
                let typed = this.flow.key.clone();
                if let Some(ask) = this.flow.type_key(&typed, true) {
                    this.send(ask, cx);
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
            out.push(div().debug_selector(|| "key-refused".into()).child(Refusal::new().said(refused.said.clone())).into_any_element());
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

    fn loading(&self, cx: &App) -> AnyElement {
        v_flex()
            .debug_selector(|| "options-loading".into())
            .min_h(px(420.))
            .items_center()
            .justify_center()
            .gap_2()
            .child(spinner(cx))
            .child(note(format!("Reading what {} has", self.flow.here), cx.palette().muted_foreground))
            .into_any_element()
    }

    fn pick_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(screen) = self.flow.screen().cloned().filter(|_| !self.flow.reading()) else { return self.loading(cx) };
        let ticks = self.flow.ticks_at(&screen);
        let dark = cx.theme().is_dark();
        let p = cx.palette().clone();
        let looks: BTreeMap<String, RowLook> = screen
            .items
            .iter()
            .map(|item| {
                let agent_note = (screen.id == "agents").then_some(self.flow.report.as_ref()).flatten().and_then(|report| {
                    let row = report.iter().find(|a| a.id == item.id)?;
                    Some(if row.ready() {
                        format!("Uses your sign-in on {}.", self.flow.here)
                    } else {
                        format!("Needs a sign-in on {} first.", self.flow.here)
                    })
                });
                let signs_in = screen.id == "tools" && catalog::tool_signs_in(&item.id);
                let note = agent_note.or_else(|| {
                    if item.locked_on() {
                        Some(ALWAYS_ON.into())
                    } else if signs_in {
                        Some("Installed signed out.".into())
                    } else if screen.id == "tools" {
                        None
                    } else {
                        item.why.clone()
                    }
                });
                let id = item.mark.clone().unwrap_or_else(|| item.id.clone());
                let look = RowLook {
                    note,
                    tag: (screen.id == "tools").then(|| item.why.clone()).flatten(),
                    size: item.size.map(|b| (words::fmt_bytes(b as u64), crate::ui::size_ink(b as u64, cx))),
                    mark: catalog::image(&id, dark, p.foreground),
                    hover: if item.detail.is_empty() { RANGE_HOVER.into() } else { item.detail.join("\n") },
                    lock_on: item.locked_on(),
                };
                (item.id.clone(), look)
            })
            .collect();
        let items: Vec<PickItem> = screen
            .items
            .iter()
            .map(|i| PickItem::new(i.id.clone(), i.label.clone(), i.locked_on() || ticks.contains(&i.id)).locked(i.lock.is_some()))
            .collect();
        let state = self.list(&screen.id, window, cx);
        let this = cx.entity().downgrade();
        let on_set = move |changes: Changes, _: &mut Window, cx: &mut App| {
            _ = this.update(cx, |this, cx| {
                let changes: Vec<(String, bool)> = changes.into_iter().map(|(k, on)| (k.to_string(), on)).collect();
                if let Some(ask) = this.flow.set_ticks(&changes) {
                    this.send(ask, cx);
                }
                cx.notify();
            });
        };
        let looks = Rc::new(looks);
        let mono = cx.theme().mono_font_family.clone();
        let row = move |item: &PickItem, tick: crate::screens::dialogs::picks::Tick| -> Vec<CardItem> {
            let look = looks.get(item.key().as_ref());
            let mut row =
                PickRow::new(item.key().clone(), item.name().clone(), item.is_on()).locked(item.is_locked()).on_click(tick.press());
            if let Some(look) = look {
                let said = if tick.told() && look.lock_on { Some(ALWAYS_ON_WHY.to_owned()) } else { look.note.clone() };
                if let Some(said) = said {
                    row = row.note(said);
                }
                if let Some(tag) = &look.tag {
                    row = row.tag(tag.clone());
                }
                row = match &look.mark {
                    Some(image) => row.framed(img(image.clone()).size(px(18.)).flex_none()),
                    None => row.glyph(Glyph::Terminal),
                };
                if let Some((bytes, ink)) = &look.size {
                    row = row.slot(
                        div().font_family(mono.clone()).text_size(px(13.)).line_height(px(16.)).text_color(*ink).child(bytes.clone()),
                    );
                }
                row = row.hover(look.hover.clone());
            }
            vec![row.into()]
        };
        PickList::new(format!("cloud-{}", screen.id), &state, items, on_set, row).into_any_element()
    }

    fn summary_step(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.flow.mine().is_none() {
            return Vec::new();
        }
        let lines: Vec<Line> =
            self.flow.summary_lines().into_iter().map(|(label, value)| Line::new(label.clone(), label).fact(value)).collect();
        let cloud = self.flow.cloud();
        let here = self.flow.here.clone();
        let p = cx.palette().clone();
        let mut items: Vec<CardItem> = Vec::new();
        for line in self.flow.sign_in_lines() {
            if line.id == "github" && !line.ready {
                items.push(Row::new("github", "GitHub").note(line.word.clone()).into());
                items.push(under_row(CopyRow::new("gh-auth-login", "gh auth login")));
                continue;
            }
            let row = line.id.clone();
            let control: AnyElement = if line.ready {
                Icon::new(Glyph::Check).with_size(px(16.)).text_color(p.foreground.opacity(0.8)).into_any_element()
            } else if self.flow.sign_ins.running(&row) {
                Button::outline(SharedString::from(format!("sign-in-{row}")), sign_in::CANCEL)
                    .xs()
                    .glyph(Glyph::X)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ask) = this.flow.sign_ins.cancel(&row) {
                            this.send_sign_in(ask, cx);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                let start = line.start.clone();
                Button::outline(SharedString::from(format!("sign-in-{row}")), sign_in::SIGN_IN)
                    .xs()
                    .glyph(Glyph::LogIn)
                    .held(start.is_none())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let Some(start) = &start else { return };
                        if let Start::Vault { .. } = start {
                            this.fields.remove(&row);
                            let field = this.field(&row, window, cx);
                            field.update(cx, |s, cx| s.toggle_masked(window, cx));
                        }
                        if let Some(ask) = this.flow.sign_ins.start(&row, start) {
                            this.send_sign_in(ask, cx);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            };
            items.push(Row::new(SharedString::from(line.id.clone()), line.name.clone()).note(line.word.clone()).control(control).into());
            if let Some(flow) = self.flow.sign_ins.of(&line.id).cloned() {
                let view = self.sign_in_view(&line.id, &flow, window, cx);
                items.push(under_row(view));
            }
        }
        vec![
            Card::new("summary-lines").inset(size::DIALOG_INSET).items(lines).into_any_element(),
            Card::new("cloud-sign-ins")
                .head(format!("Sign-ins on {here}"))
                .lede(format!("Threads on {cloud} use these through wsp. Nothing is signed in on {cloud}."))
                .inset(size::DIALOG_INSET)
                .items(items)
                .into_any_element(),
        ]
    }

    /// One sign-in as it stands under its row, with the field its token, key or code is typed in.
    fn sign_in_view(&mut self, row: &str, flow: &SignIn, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let typed = matches!(flow, SignIn::Vault { .. } | SignIn::Run { paste: true, state: RunState::Waiting, .. });
        let mut view = SignInView::new(SharedString::from(format!("sign-in-{row}")), flow);
        if typed {
            view = view.field(&self.field(row, window, cx));
        }
        let (saving, coding) = (cx.entity().downgrade(), cx.entity().downgrade());
        let (save_row, code_row) = (row.to_owned(), row.to_owned());
        view.on_save(move |typed, _, cx| {
            _ = saving.update(cx, |this, cx| {
                if let Some(ask) = this.flow.sign_ins.save(&save_row, &typed) {
                    this.send_sign_in(ask, cx);
                }
                cx.notify();
            });
        })
        .on_code(move |typed, window, cx| {
            _ = coding.update(cx, |this, cx| {
                if let Some(field) = this.fields.get(&code_row).cloned() {
                    field.update(cx, |s, cx| s.set_value("", window, cx));
                }
                if let Some(ask) = this.flow.sign_ins.code(&code_row, &typed) {
                    this.send_sign_in(ask, cx);
                }
                cx.notify();
            });
        })
        .into_any_element()
    }

    fn building(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(job) = self.flow.mine().cloned() else { return Vec::new() };
        let rows: Vec<CardItem> = build_rows(&job)
            .into_iter()
            .map(|row| {
                let state = match row.state {
                    RowState::Waiting => StepState::NotStarted,
                    RowState::Working => StepState::Running,
                    RowState::NeedsYou => StepState::NeedsYou,
                    RowState::Done => StepState::Done,
                    RowState::Failed => StepState::Failed,
                };
                let opens = row.state != RowState::Waiting && row.id != SIGN_IN_STAGE_ID;
                let open = opens && self.turned.get(&row.id).copied().unwrap_or(row.state == RowState::Failed);
                let mut step = StepRow::new(SharedString::from(row.id.clone()), state, row.name.clone());
                if let Some(ms) = row.ms {
                    step = step.took(ms.max(0.) as u64);
                }
                if let Some(since) = row.since {
                    step = step.since(since);
                }
                if let Some(said) = row.said.clone() {
                    step = step.said(said, None);
                }
                let id = row.id.clone();
                step = if opens {
                    step.toggle(
                        open,
                        Some(cx.listener(move |this, _, _, cx| {
                            this.turned.insert(id.clone(), !open);
                            cx.notify();
                        })),
                    )
                } else {
                    step.toggle(false, None::<fn(&gpui_kit::ClickEvent, &mut Window, &mut App)>)
                };
                if open {
                    step = step.under(StepLog::new(
                        SharedString::from(format!("{}-log", row.id)),
                        row.lines.iter().cloned().map(Into::into).collect(),
                    ));
                }
                step.into()
            })
            .collect();
        let mut out = vec![Card::new("cloud-setup").inset(size::DIALOG_INSET).items(rows).into_any_element()];
        if let (true, Some(error)) = (self.flow.failed(), job.error.clone()) {
            out.push(div().debug_selector(|| "build-failed".into()).child(Refusal::new().said(error)).into_any_element());
        }
        out
    }
}

impl Render for AddCloud {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, line) = self.flow.head();
        if self.flow.step == Step::Ready {
            let glyph = Icon::new(Glyph::Cloud).with_size(px(24.)).text_color(cx.palette().foreground.opacity(0.8));
            let start = Button::primary("start-thread", "Start a thread")
                .on_click(cx.listener(|this, _, window, cx| this.start_thread(window, cx)));
            let page = ReadyPage::new(glyph, self.flow.cloud(), start)
                .line(self.flow.ready_line())
                .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)));
            return Dialog::new(title).frame().on_outside(layer::dismiss).page(page).into_any_element();
        }
        let figure = self.flow.figure();
        let why = if self.flow.step == Step::Building { format!("{figure} stages") } else { format!("Step {figure}") };
        let mut dialog = Dialog::new(title).frame().count_words(figure, why).on_outside(layer::dismiss);
        if let Some(line) = line {
            dialog = dialog.line(line);
        }
        let body: Vec<AnyElement> = match self.flow.step {
            Step::Provider => vec![self.provider_step(cx)],
            Step::Key => self.key_step(window, cx),
            Step::Pick(_) => vec![self.pick_step(window, cx)],
            Step::Summary => self.summary_step(window, cx),
            Step::Building | Step::Ready => self.building(cx),
        };
        dialog = dialog.children(body);
        let refusal = self.flow.recipe_refusal.clone().or_else(|| self.flow.setup_refused.clone());
        if let Some(said) = refusal.filter(|_| self.flow.step != Step::Key) {
            dialog = dialog.child(div().debug_selector(|| "cloud-refusal".into()).child(Refusal::new().said(said)));
        }
        let (acts, primary) = self.foot(cx);
        for act in acts {
            dialog = dialog.act(act);
        }
        if let Some(primary) = primary {
            dialog = dialog.act(div().debug_selector(|| "cloud-continue".into()).child(primary));
        }
        dialog.into_any_element()
    }
}

impl AddCloud {
    /// The foot's acts: Back, Cancel or Close, then the one primary.
    fn foot(&mut self, cx: &mut Context<Self>) -> (Vec<Button>, Option<Button>) {
        let flow = &self.flow;
        let close = |id: &'static str, word: &'static str, cx: &mut Context<Self>| {
            Button::outline(id, word).on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))
        };
        let back = (flow.at().is_some_and(|a| a > 0) && flow.step != Step::Building).then(|| {
            Button::outline("back", "Back").held(flow.recipe_busy).on_click(cx.listener(|this, _, _, cx| {
                if let Some(ask) = this.flow.back() {
                    this.send(ask, cx);
                }
                cx.notify();
            }))
        });
        let primary = |word: &'static str, held: bool| Button::primary("continue", word).held(held);
        match flow.step {
            Step::Provider => (
                vec![close("cancel", "Cancel", cx)],
                Some(primary("Continue", false).on_click(cx.listener(|this, _, _, cx| {
                    this.flow.go(Step::Key);
                    cx.notify();
                }))),
            ),
            Step::Key => {
                let held = !flow.key_ready() || flow.busy;
                let first = back.unwrap_or_else(|| close("cancel", "Cancel", cx));
                let next = primary("Continue", held).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(ask) = this.flow.key_on() {
                        this.send(ask, cx);
                    }
                    cx.notify();
                }));
                (vec![first], Some(next))
            }
            Step::Summary => {
                let held = flow.build_held();
                let build = primary("Build image", held).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(ask) = this.flow.build() {
                        this.send(ask, cx);
                    }
                    cx.notify();
                }));
                (back.into_iter().collect(), Some(build))
            }
            Step::Building | Step::Ready if flow.sealed() => (
                vec![close("close", "Close", cx)],
                Some(primary("Next", false).on_click(cx.listener(|this, _, _, cx| {
                    this.flow.go(Step::Ready);
                    cx.notify();
                }))),
            ),
            Step::Building | Step::Ready if flow.failed() => {
                let held = flow.recipe_busy;
                let again = primary(words::BUILD_AGAIN, held).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(ask) = this.flow.start_over() {
                        this.send(ask, cx);
                    }
                    cx.notify();
                }));
                (vec![close("close", "Close", cx)], Some(again))
            }
            Step::Building | Step::Ready => (vec![close("close", "Run in background", cx)], None),
            Step::Pick(_) => {
                let held = flow.reading() || flow.recipe_busy;
                let next = primary("Continue", held).on_click(cx.listener(|this, _, _, cx| {
                    if let Some(ask) = this.flow.go_on() {
                        this.send(ask, cx);
                    }
                    cx.notify();
                }));
                (back.into_iter().collect(), Some(next))
            }
        }
    }
}
