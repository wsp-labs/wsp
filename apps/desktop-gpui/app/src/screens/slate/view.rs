// SPDX-License-Identifier: AGPL-3.0-only
//! The panel as drawn: its header and menu, the empty state, and the slate's column.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, SharedString, Window};

use super::model::Record;
use super::{consent, pieces, words, Load, SlateScreen, KEY_CONTEXT};
use crate::ui::{icon_menu, scroll_area, ActivePalette, Button, MenuAct};

impl SlateScreen {
    fn menu_acts(&self, cx: &mut Context<Self>) -> Rc<Vec<MenuAct>> {
        let Some(slate) = self.slate() else { return Rc::new(Vec::new()) };
        let this = cx.entity().downgrade();
        let act = |title: String, held: bool, run: fn(&mut SlateScreen, &mut Context<SlateScreen>)| {
            let this = this.clone();
            MenuAct {
                group: 0,
                glyph: None,
                title: title.into(),
                destructive: false,
                held,
                why: None,
                keys: None,
                run: Rc::new(move |_, cx| {
                    _ = this.update(cx, run);
                }),
            }
        };
        let mut acts = vec![act(words::undo(slate.record.version), !slate.record.can_undo, |s, cx| s.menu_write("<undo />", cx))];
        acts.push(act(words::CLEAR.into(), false, |s, cx| s.menu_write("<clear />", cx)));
        let doc = slate.doc();
        if doc.is_some_and(|d| !d.runs.is_empty()) {
            acts.push(act(words::STOP.into(), false, |s, cx| s.stop_runs(cx)));
        }
        if doc.is_some_and(|d| !d.secrets().is_empty()) {
            acts.push(act(words::FORGET.into(), false, |s, cx| {
                s.consent.forgetting = Some(consent::Forget::default());
                cx.notify();
            }));
        }
        acts.push(act(words::APPROVALS.into(), false, |s, cx| {
            s.consent.standing = Some(consent::Standing::default());
            cx.notify();
        }));
        acts.push(act(words::COPY.into(), false, |s, cx| s.copy_sketch(cx)));
        Rc::new(acts)
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let slate = self.slate().expect("a drawn slate");
        let title = slate.doc().and_then(|d| d.title.clone()).unwrap_or_else(|| words::TITLE.to_owned());
        let acts = self.menu_acts(cx);
        let refreshing = consent::refreshing_under_root(slate);
        v_flex()
            .child(
                h_flex()
                    .mt(px(12.))
                    .when(self.refused.is_none(), |el| el.mb(px(16.)))
                    .h(px(28.))
                    .flex_none()
                    .gap(px(8.))
                    .debug_selector(|| "slate-header".into())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(p.muted_foreground)
                            .child(SharedString::from(title)),
                    )
                    .when(refreshing, |el| el.child(crate::ui::spinner(cx).size(px(12.))))
                    .child(div().debug_selector(|| "slate-menu".into()).child(icon_menu("slate-menu", words::MENU, acts))),
            )
            .children(self.refused.clone().map(|r| {
                div()
                    .mb(px(16.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(p.error_foreground)
                    .debug_selector(|| "slate-refused".into())
                    .child(r)
            }))
            .into_any_element()
    }

    fn empty(&self, record: Option<&Record>, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let Some(record) = record else {
            return empty_frame(div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(words::NO_THREAD), cx);
        };
        let first = match (record.newer, record.empty.as_deref()) {
            (Some(schema), _) => Some(words::newer(schema)),
            (None, Some("cleared")) => Some(words::CLEARED.to_owned()),
            (None, Some("rewound-before")) => Some(words::REWOUND.to_owned()),
            _ => None,
        };
        let mut lead = String::new();
        if let Some(first) = first {
            lead.push_str(&first);
            lead.push_str(". ");
        }
        if record.newer.is_none() {
            lead.push_str(words::ASK);
        }
        let mark = record.empty.clone().unwrap_or_else(|| if record.newer.is_some() { "newer".into() } else { "none".into() });
        let this = cx.entity().downgrade();
        let thread = record.thread_id.clone();
        // A prompt in the composer's place leaves no composer to take the words or the caret.
        let held = crate::screens::composer::docked(&thread, cx);
        let ask_for_one =
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| fill_composer(&thread, words::ASK_PROMPT, window, cx);
        let undo = record.can_undo.then(|| {
            let this = this.clone();
            div().debug_selector(|| "slate-empty-undo".into()).child(Button::ghost("slate-empty-undo", "Undo").xs().on_click(
                move |_, _, cx| {
                    _ = this.update(cx, |s, cx| {
                        s.empty_refused = None;
                        s.menu_write("<undo />", cx);
                    });
                },
            ))
        });
        let body = v_flex()
            .items_center()
            .gap(px(24.))
            .debug_selector(move || format!("slate-empty-{mark}"))
            .child(
                v_flex()
                    .max_w(px(384.))
                    .items_center()
                    .text_center()
                    .child(div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(words::NOTHING))
                    .child(div().mt(px(4.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(lead)),
            )
            .when(record.newer.is_none(), |el| {
                el.child(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            div()
                                .debug_selector(|| "slate-ask".into())
                                .child(Button::outline("slate-ask", words::ASK_BUTTON).xs().held(held).on_click(ask_for_one)),
                        )
                        .children(undo),
                )
            })
            .when(held && record.newer.is_none(), |el| {
                el.child(
                    div()
                        .debug_selector(|| "slate-held".into())
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.muted_foreground)
                        .child(words::ASK_HELD),
                )
            })
            .children(
                self.refused
                    .clone()
                    .or(self.empty_refused.clone())
                    .map(|r| div().text_size(px(13.)).line_height(px(20.)).text_color(p.error_foreground).child(r)),
            );
        empty_frame(body, cx)
    }
}

/// Puts words in the thread's composer: the open one, else its kept draft, which it reads as it opens the thread.
fn fill_composer(thread: &str, text: &str, window: &mut Window, cx: &mut App) {
    if !crate::screens::composer::fill(thread, text, window, cx) && cx.has_global::<crate::local_store::LocalStore>() {
        _ = cx.update_global(|s: &mut crate::local_store::LocalStore, _| {
            crate::screens::composer::store::set_draft(s, thread, text, text.len())
        });
    }
}

/// A fill's block at the caret of the thread's composer: the open one, else its kept draft.
fn insert_in_composer(thread: &str, block: &str, window: &mut Window, cx: &mut App) {
    if !crate::screens::composer::insert(thread, block, window, cx) && cx.has_global::<crate::local_store::LocalStore>() {
        _ = cx.update_global(|s: &mut crate::local_store::LocalStore, _| {
            let (text, at) = crate::screens::composer::store::draft(s, thread).unwrap_or_default();
            let (text, caret) = crate::screens::composer::with_block(&text, at, block);
            crate::screens::composer::store::set_draft(s, thread, &text, caret)
        });
    }
}

/// The panel's own empty state, centred in the tab: `Empty` with its 24 px gap and `md:p-12`.
fn empty_frame(body: impl IntoElement, _cx: &App) -> AnyElement {
    v_flex().size_full().flex_1().items_center().justify_center().p(px(48.)).child(body).into_any_element()
}

impl Render for SlateScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let back = !self.on_screen();
        self.mark_drawn(window, cx);
        if let (Some(text), Some(thread)) = (self.filling.take(), self.opened.thread_id.clone()) {
            insert_in_composer(&thread, &text, window, cx);
        }
        if back {
            self.sync_hold();
            self.resolve(cx);
        }
        let surface = match &self.load {
            Load::NoThread => self.empty(None, cx),
            Load::Waiting => div().into_any_element(),
            Load::Empty(record) => {
                let record = record.clone();
                self.empty(Some(&record), cx)
            }
            Load::Drawn(_) => {
                let header = self.header(cx);
                let held = consent::held_rows(self, window, cx);
                let screen = cx.entity().downgrade();
                let slate = self.slate().expect("drawn");
                let body = slate.doc().and_then(|d| pieces::draw(slate, &d.root, &screen, window, cx));
                v_flex()
                    .size_full()
                    .min_h_0()
                    .debug_selector(|| "slate-surface".into())
                    .child(column().child(header))
                    .children(held)
                    .child(scroll_area("slate-scroll", &self.scroll, column().pb(px(32.)).children(body), window, cx).flex_1().min_h_0())
                    .into_any_element()
            }
        };
        let overlays = consent::overlays(self, window, cx);
        div().key_context(KEY_CONTEXT).size_full().flex().flex_col().child(surface).children(overlays)
    }
}

/// The panel's one column, 16 px in, capped at the settings pages' 760 px.
pub fn column() -> gpui_kit::Div {
    v_flex().w_full().max_w(px(760.)).mx_auto().px(px(16.))
}
