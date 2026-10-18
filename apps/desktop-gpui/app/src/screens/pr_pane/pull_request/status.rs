// SPDX-License-Identifier: AGPL-3.0-only
//! Status: where the pull request stands, first under the tabs, a Settings head over one card of rows
//! (`pull-request/Status.tsx`, `MergeControls.tsx`). The checks are its first row, folded under one line that counts
//! them with a failure open with its fix; the review says each reviewer's verdict; the base says how far behind it is
//! with the update; one sentence says what holds the merge; the buttons stand in the last row. A settled pull request
//! says it merged or closed, with when, and the link out.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Hsla, SharedString};

use super::model::{self, Check, CheckState, Seen, State, Tone};
use super::parts::{self, tone_ink};
use super::words::{self, PR_WORDS};
use super::PullRequestScreen;
use crate::screens::pr_pane::files::markdown::open_web;
use crate::ui::{self, menu_on, section_head, ActivePalette, Button, CardItem, Glyph, MenuAct};

/// The box's buttons: 28 px, 12.5 px medium (`BOX_BUTTON`).
const BOX_BUTTON: gpui_kit::Pixels = px(28.);

fn check_mark(state: CheckState) -> (Glyph, Tone) {
    match state {
        CheckState::Pass => (Glyph::CircleCheck, Tone::Ok),
        CheckState::Fail => (Glyph::CircleX, Tone::Bad),
        CheckState::Pending => (Glyph::CircleDashed, Tone::Run),
        CheckState::Skipped | CheckState::Cancelled => (Glyph::CircleMinus, Tone::Quiet),
    }
}

/// Words the card draws, named with them for the tests that read what a row says.
fn said(key: &'static str, words: impl Into<SharedString>) -> gpui_kit::Div {
    let words: SharedString = words.into();
    let named = words.clone();
    div().debug_selector(move || format!("{key}:{named}")).child(words)
}

/// One row of the card, every part on its first 20 px line (`BoxRow`): a lead glyph, the label, a muted value, an act.
struct BoxRow {
    key: SharedString,
    lead: Option<(Glyph, Hsla)>,
    label: AnyElement,
    value: Option<AnyElement>,
    end: Option<AnyElement>,
    sub: bool,
    on_click: Option<ui::act::Run>,
}

impl BoxRow {
    fn new(key: impl Into<SharedString>, label: impl IntoElement) -> Self {
        BoxRow { key: key.into(), lead: None, label: label.into_any_element(), value: None, end: None, sub: false, on_click: None }
    }

    fn lead(mut self, glyph: Glyph, ink: Hsla) -> Self {
        self.lead = Some((glyph, ink));
        self
    }

    fn value(mut self, value: impl IntoElement) -> Self {
        self.value = Some(value.into_any_element());
        self
    }

    fn end(mut self, end: impl IntoElement) -> Self {
        self.end = Some(end.into_any_element());
        self
    }

    fn sub(mut self) -> Self {
        self.sub = true;
        self
    }

    fn item(self, cx: &App) -> CardItem {
        let p = cx.palette().clone();
        CardItem::placed(move |corners| {
            let glyph = if self.sub { px(13.) } else { px(15.) };
            let named = format!("pr-row:{}", self.key);
            let row = h_flex()
                .id(self.key.clone())
                .debug_selector(move || named.clone())
                .w_full()
                .items_start()
                .gap_3()
                .text_size(px(13.))
                .line_height(px(20.))
                .map(|el| {
                    if self.sub {
                        el.min_h(px(36.)).bg(p.foreground.opacity(0.02)).py_2().pr(px(14.)).pl(px(39.))
                    } else {
                        el.min_h(px(48.)).p(px(14.))
                    }
                })
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_start()
                        .gap(if self.sub { px(8.) } else { px(10.) })
                        .children(self.lead.map(|(g, ink)| {
                            div()
                                .flex_none()
                                .pt(if self.sub { px(3.5) } else { px(2.5) })
                                .child(Icon::new(g).with_size(glyph).text_color(ink))
                        }))
                        .child(div().min_w_0().flex_1().child(self.label)),
                )
                .children(self.value.map(|v| div().flex_none().text_color(p.muted_foreground).child(v)))
                .children(self.end.map(|e| div().flex_none().my(px(-4.)).child(e)));
            let row = match self.on_click {
                Some(f) => row.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.6))).on_click(move |_, window, cx| f(window, cx)),
                None => row,
            };
            corners.apply(div()).child(row).into_any_element()
        })
    }
}

impl PullRequestScreen {
    fn box_button(id: impl Into<SharedString>, label: &str, glyph: Option<Glyph>) -> Button {
        let b = Button::outline(id, label.to_owned()).height(BOX_BUTTON);
        match glyph {
            Some(g) => b.glyph(g),
            None => b,
        }
    }

    fn agent_mark(&self, cx: &App) -> AnyElement {
        ui::catalog::mark(&self.agent, &ui::catalog::name(&self.agent), px(14.), cx.palette().muted_foreground, cx)
    }

    fn check_rows(&self, checks: &[Check], rows: &mut Vec<CardItem>, cx: &mut Context<Self>) {
        let p = cx.palette().clone();
        let (glyph, tone) = check_mark(model::worst_check(checks));
        let open = self.checks_open();
        let me = cx.weak_entity();
        let toggle: ui::act::Run = Rc::new(move |_, cx| {
            _ = me.update(cx, |this, cx| this.toggle_checks(cx));
        });
        let mut head = BoxRow::new("pr-checks", PR_WORDS.checks).lead(glyph, tone_ink(tone, &p)).value(model::checks_line(checks)).end(
            Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight })
                .with_size(px(15.))
                .text_color(p.muted_foreground.opacity(0.7)),
        );
        head.on_click = Some(toggle);
        rows.push(head.item(cx));
        if !open {
            return;
        }
        for c in checks.iter().filter(|c| c.state == CheckState::Fail) {
            let me = cx.weak_entity();
            let name = c.name.clone();
            let fix = Self::box_button(SharedString::from(format!("pr-fix-{}", c.name)), PR_WORDS.fix, None).held(self.acting).on_click(
                move |_, window, cx| {
                    let name = name.clone();
                    _ = me.update(cx, |this, cx| this.fix(Some(name), window, cx));
                },
            );
            let label = v_flex()
                .debug_selector(|| "pr-check".into())
                .items_start()
                .gap(px(6.))
                .child(c.name.clone())
                .children(
                    model::failed_note(c).map(|n| div().text_size(px(12.)).line_height(px(18.)).text_color(p.muted_foreground).child(n)),
                )
                .child(div().mt(px(2.)).child(h_flex().gap(px(6.)).child(fix).child(self.agent_mark(cx))));
            rows.push(
                BoxRow::new(SharedString::from(format!("pr-check-{}", c.name)), label)
                    .sub()
                    .lead(Glyph::CircleX, tone_ink(Tone::Bad, &p))
                    .item(cx),
            );
        }
        let now = model::now();
        for c in checks.iter().filter(|c| c.state == CheckState::Pending) {
            let value = match c.started_at.as_deref().and_then(model::parse_time) {
                Some(at) => words::running_for(&model::span_word((now - at).num_milliseconds())),
                None => model::count_word(CheckState::Pending).to_owned(),
            };
            rows.push(
                BoxRow::new(SharedString::from(format!("pr-check-{}", c.name)), div().truncate().child(c.name.clone()))
                    .sub()
                    .lead(Glyph::CircleDashed, tone_ink(Tone::Run, &p))
                    .value(value)
                    .item(cx),
            );
        }
        for state in [CheckState::Pass, CheckState::Skipped, CheckState::Cancelled] {
            let held: Vec<&Check> = checks.iter().filter(|c| c.state == state).collect();
            if held.is_empty() {
                continue;
            }
            let mut where_: Vec<String> = Vec::new();
            for w in held.iter().filter_map(|c| c.workflow.clone()) {
                if !where_.contains(&w) {
                    where_.push(w);
                }
            }
            let (glyph, tone) = check_mark(state);
            let mut row = BoxRow::new(
                SharedString::from(format!("pr-checks-{}", model::count_word(state))),
                format!("{} {}", held.len(), model::count_word(state)),
            )
            .sub()
            .lead(glyph, tone_ink(tone, &p));
            if !where_.is_empty() {
                row = row.value(where_.join(", "));
            }
            rows.push(row.item(cx));
        }
    }

    pub(super) fn status_card(&self, seen: &Seen, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let page = self.page.as_ref();
        let mut rows: Vec<CardItem> = Vec::new();
        let checks = seen.checks();
        if !checks.is_empty() {
            self.check_rows(checks, &mut rows, cx);
        }
        let now = model::now();
        let state = seen.state();
        if let Some(settled) = model::settled(seen, page, now) {
            let merged = settled.merged;
            let label = match settled.by {
                Some(by) => h_flex()
                    .gap(px(6.))
                    .child(said("pr-settled-word", PR_WORDS.merged_by))
                    .child(
                        div()
                            .debug_selector({
                                let by = by.clone();
                                move || format!("pr-settled-by:{by}")
                            })
                            .child(parts::who(&by, px(18.), cx)),
                    )
                    .into_any_element(),
                None => said("pr-settled-word", if merged { PR_WORDS.merged } else { PR_WORDS.closed_not_merged }).into_any_element(),
            };
            let mut row = BoxRow::new("pr-settled", label).lead(
                if merged { Glyph::GitMerge } else { Glyph::GitPullRequestClosed },
                tone_ink(if merged { Tone::Merged } else { Tone::Quiet }, &p),
            );
            if let Some(ago) = settled.ago {
                row = row.value(said("pr-settled-ago", ago));
            }
            rows.push(row.item(cx));
            if let Some((words, sha)) = settled.landed {
                let mono = cx.theme().mono_font_family.clone();
                let label = h_flex().gap(px(4.)).child(said("pr-landed-words", words)).child(said("pr-landed-sha", sha).font_family(mono));
                rows.push(BoxRow::new("pr-landed", label).sub().item(cx));
            }
        }
        if let Some(fact) = seen.fact().filter(|f| f.state == State::Open) {
            let verdicts = page.map(|p| model::verdicts_of(&p.latest_reviews)).unwrap_or_default();
            let asked = verdicts.iter().any(|(_, approved)| !approved);
            let tone = if asked {
                Tone::Warn
            } else if verdicts.is_empty() {
                Tone::Quiet
            } else {
                Tone::Ok
            };
            let mut review = BoxRow::new("pr-review", PR_WORDS.review).lead(Glyph::ScanEye, tone_ink(tone, &p));
            if !verdicts.is_empty() {
                let logins: Vec<String> = verdicts.iter().map(|(a, _)| a.clone()).collect();
                review = review.value(h_flex().child(parts::faces("pr-verdicts", &logins, cx)).child(model::review_sentence(&verdicts)));
            } else if fact.review == model::Review::Required {
                review = review.value(PR_WORDS.review_required);
            }
            if !self.status.as_ref().is_some_and(|s| matches!(s.from, Some(model::From::Review))) {
                let host = self.host.clone();
                let workspace_id = self.workspace_id().unwrap_or_default().to_owned();
                review =
                    review.end(Button::icon("pr-review-agent", Glyph::ScanEye).side(px(28.)).tooltip(PR_WORDS.review_with_agent).on_click(
                        move |_, window, cx| {
                            super::review::ReviewDialog::open(&host, &workspace_id, window, cx);
                        },
                    ));
            }
            rows.push(review.item(cx));
            if page.is_some() {
                let (items, word) = self.derived.open.clone();
                if !items.is_empty() {
                    let me = cx.weak_entity();
                    let send = items.clone();
                    let sending = items.iter().any(|i| self.is_sending(*i));
                    let button = Self::box_button("pr-send-all", PR_WORDS.send_all, None).held(sending).on_click(move |_, window, cx| {
                        let items = send.clone();
                        _ = me.update(cx, |this, cx| this.send(items, window, cx));
                    });
                    rows.push(
                        BoxRow::new("pr-comments", PR_WORDS.comments)
                            .lead(Glyph::MessageSquare, p.muted_foreground)
                            .value(words::unresolved(items.len(), word))
                            .end(h_flex().gap(px(6.)).child(self.agent_mark(cx)).child(button))
                            .item(cx),
                    );
                }
            }
            let base = fact.base.clone();
            let behind = fact.behind_base.unwrap_or(0);
            if fact.mergeable == model::Mergeable::Conflicting {
                let me = cx.weak_entity();
                let fix = Self::box_button("pr-fix-conflicts", PR_WORDS.fix, None).held(self.acting).on_click(move |_, window, cx| {
                    _ = me.update(cx, |this, cx| this.fix(None, window, cx));
                });
                rows.push(
                    BoxRow::new("pr-base", base.clone())
                        .lead(Glyph::GitBranch, p.muted_foreground)
                        .value(PR_WORDS.conflicts)
                        .end(h_flex().gap(px(6.)).child(self.agent_mark(cx)).child(fix))
                        .item(cx),
                );
            } else if behind > 0 {
                let me = cx.weak_entity();
                let update = Self::box_button("pr-update", &words::update(&base), Some(Glyph::ArrowUp)).held(self.acting).on_click(
                    move |_, window, cx| {
                        _ = me.update(cx, |this, cx| this.update(window, cx));
                    },
                );
                rows.push(
                    BoxRow::new("pr-base", base.clone())
                        .lead(Glyph::GitBranch, p.muted_foreground)
                        .value(words::behind(behind))
                        .end(update)
                        .item(cx),
                );
            }
            rows.push(
                BoxRow::new("pr-hold", div().debug_selector(|| "pr-hold".into()).child(model::hold_line(fact)))
                    .lead(Glyph::GitMerge, p.muted_foreground)
                    .item(cx),
            );
            rows.push(CardItem::any(
                h_flex()
                    .debug_selector(|| "pr-box-end".into())
                    .min_h(px(44.))
                    .flex_wrap()
                    .justify_end()
                    .gap_2()
                    .px(px(14.))
                    .py_2()
                    .children(self.merge_controls(fact, cx)),
            ));
        } else if state != State::Open {
            let url = seen.url().to_owned();
            rows.push(CardItem::any(
                h_flex().min_h(px(44.)).justify_end().gap_2().px(px(14.)).py_2().child(
                    div().debug_selector(|| "pr-open-github".into()).child(
                        Self::box_button("pr-open-github", PR_WORDS.open_on_github, Some(Glyph::ExternalLink))
                            .on_click(move |_, _, cx| open_web(&url, cx)),
                    ),
                ),
            ));
        }
        v_flex()
            .debug_selector(|| "pr-status".into())
            .gap(px(10.))
            .child(section_head(PR_WORDS.status, cx))
            .child(ui::Card::new("pr-status-card").items(rows))
            .into_any_element()
    }

    /// The box's last row on an open pull request: Merge, held where it cannot merge now, a menu of the methods the
    /// repository allows where it allows more than one, and Merge when checks pass beside it while checks run.
    fn merge_controls(&self, fact: &model::Fact, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let repo = self.page.as_ref().and_then(|p| p.merge.as_ref());
        let running = fact.checks.iter().any(|c| c.state == CheckState::Pending);
        let merges = model::mergeable(fact) && !running;
        let methods = model::methods(repo);
        let mut out = Vec::new();
        if model::waits(fact, repo) {
            let me = cx.weak_entity();
            out.push(
                Button::ghost("pr-merge-when", PR_WORDS.merge_when_checks_pass)
                    .height(BOX_BUTTON)
                    .held(self.acting)
                    .on_click(move |_, window, cx| {
                        _ = me.update(cx, |this, cx| this.merge(None, true, window, cx));
                    })
                    .into_any_element(),
            );
        }
        if methods.len() > 1 && merges {
            let acts: Vec<MenuAct> = methods
                .iter()
                .map(|method| {
                    let me = cx.weak_entity();
                    let method = *method;
                    let run: ui::act::Run = Rc::new(move |window, cx| {
                        _ = me.update(cx, |this, cx| this.merge(Some(method), false, window, cx));
                    });
                    MenuAct {
                        group: 0,
                        glyph: None,
                        title: words::method_word(method).into(),
                        destructive: false,
                        held: self.acting,
                        why: None,
                        keys: None,
                        run,
                    }
                })
                .collect();
            let trigger = Button::primary("pr-merge", PR_WORDS.merge).height(BOX_BUTTON).glyph(Glyph::ChevronDown);
            out.push(
                div()
                    .debug_selector(|| "pr-merge-menu".into())
                    .child(menu_on("pr-merge", trigger, gpui_kit::Anchor::TopRight, Rc::new(acts)))
                    .into_any_element(),
            );
        } else if methods.len() > 1 {
            out.push(
                Button::primary("pr-merge", PR_WORDS.merge).height(BOX_BUTTON).glyph(Glyph::ChevronDown).held(true).into_any_element(),
            );
        } else {
            let me = cx.weak_entity();
            out.push(
                Button::primary("pr-merge", PR_WORDS.merge)
                    .height(BOX_BUTTON)
                    .held(!merges || self.acting)
                    .on_click(move |_, window, cx| {
                        _ = me.update(cx, |this, cx| this.merge(None, false, window, cx));
                    })
                    .into_any_element(),
            );
        }
        out
    }
}
