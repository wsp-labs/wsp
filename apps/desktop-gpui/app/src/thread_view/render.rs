// SPDX-License-Identifier: AGPL-3.0-only
//! The thread view drawn: the transcript's list with its rail, find bar, toolbar and the states before it loads.

use std::time::Instant;

use gpui_kit::component::{v_flex, ActiveTheme};
use gpui_kit::{canvas, div, prelude::*, px, Context, DispatchPhase, FollowMode, ScrollWheelEvent, StyleRefinement, Window};

use super::{Glide, Link, Reveal, ThreadView, REVEAL_TRIES};
use crate::screens::app_shell::keys;
use crate::screens::thread_view::{
    cards, find,
    minimap::minimap,
    rows,
    shimmer::{self, Follow},
    toolbar,
};
use crate::transcript::{Row, RowKind};
use crate::ui::ActivePalette;

impl Render for ThreadView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let began = Instant::now();
        self.renders += 1;
        self.frame += 1;
        if self._activation.is_none() {
            self._activation = Some(cx.observe_window_activation(window, |this, window, cx| {
                this.active = window.is_window_active();
                this.stamp(cx);
            }));
        }
        if self.active != window.is_window_active() {
            self.active = window.is_window_active();
            self.stamp(cx);
        }
        if window.focused(cx).is_none() {
            // Find's keys reach the view only through focus, and a window nobody clicked yet holds none.
            self.focus.focus(window, cx);
        }
        self.trim();
        self.window = Some(window.window_handle());
        cards::let_go(std::mem::take(&mut self.released), window, cx);
        if let Some(Reveal { ix, y, tries, from }) = self.revealing.take() {
            let top = self.list.viewport_bounds().origin.y;
            let off = self.list.bounds_for_item(ix).map(|b| b.origin.y - top - y);
            let at = self.list.scroll_px_offset_for_scrollbar().y;
            // The list clamps at its ends, so a row near one may never reach its height.
            let stuck = from.is_some_and(|from| (from - at).abs() < px(0.5))
                || off.is_some_and(|off| off > px(0.) && self.list.is_scrolled_to_end() == Some(true));
            match off {
                _ if tries >= REVEAL_TRIES => {}
                Some(off) if off.abs() < px(0.5) || stuck => {}
                Some(off) => {
                    self.list.scroll_by(off);
                    self.revealing = Some(Reveal { ix, y, tries: tries + 1, from: Some(at) });
                }
                None => self.revealing = Some(Reveal { ix, y, tries: tries + 1, from: None }),
            }
            window.request_animation_frame();
        }
        if let Some(Glide { from, start, took }) = &self.glide {
            let t = (cx.background_executor().now().saturating_duration_since(*start).as_secs_f32() / took.as_secs_f32()).min(1.);
            if t >= 1. {
                self.glide = None;
                self.list.set_follow_mode(FollowMode::Tail);
            } else {
                let end = self.list.max_offset_for_scrollbar().y;
                let target = *from + (end - *from) * toolbar::ease_in_out(t);
                self.list.scroll_by(target + self.list.scroll_px_offset_for_scrollbar().y);
                window.request_animation_frame();
            }
        }
        if let Some(drive) = &mut self.drive {
            if !drive.done {
                // The end check stays unanswered while a row is not laid out, and for a thread shorter than the list.
                if self.list.is_scrolled_to_end() == Some(true) || self.list.max_offset_for_scrollbar().y <= px(0.) {
                    drive.done = true;
                } else {
                    // The list counts a row it has not laid out as no height, so a scroll from such a row, or further
                    // than the rows laid out under it, would pass over every one of them in one frame.
                    if self.list.bounds_for_item(self.list.logical_scroll_top().item_ix).is_some() {
                        let at = -self.list.scroll_px_offset_for_scrollbar().y;
                        let target = px(drive.px_per_s * drive.start.elapsed().as_secs_f32());
                        self.list.scroll_by((target - at).min(self.list.viewport_bounds().size.height));
                    }
                    window.request_animation_frame();
                }
            }
        }
        let frames = self.drive.as_ref().filter(|d| !d.done).map(|d| d.frames.clone());
        let shown = self.loaded && !self.transcript.rows().is_empty();
        let height = self.list.viewport_bounds().size.height;
        if shown && !self.settled && height > px(0.) && self.list.is_scrolled_to_end() != Some(false) {
            self.settled = true;
        }
        // An older page once the list first stood at its end and the reader is within two screens of the top;
        // every one while find is open.
        let near_top = -self.list.scroll_px_offset_for_scrollbar().y < height * 2.;
        if self.loaded && (self.whole || self.find.is_open() || (self.settled && near_top)) {
            self.older(cx);
        }
        let presented = match self.presented.take() {
            Some((thread, cell)) if shown && self.thread.as_deref() == Some(thread.as_str()) => Some(cell),
            waiting => {
                self.presented = waiting;
                None
            }
        };
        let drawn = self.drawn.take_if(|_| self.loaded);
        let muted = cx.theme().muted_foreground;
        // With no thread `WorkspaceCenter` draws nothing; a host holding a project shows its New thread page instead.
        let status = self.failure.clone();
        // `TRANSCRIPT_LOADING` over the composer's room, and list.tsx's empty line in the placeholder ink, each centred.
        let loading = self.placeholder().is_some() && !self.loaded;
        let loading_seen = self.loading_seen.clone().filter(|_| loading);
        let words = match self.placeholder() {
            Some(_) if loading => Some(cards::loading(self.composer_inset, cx).into_any_element()),
            Some(said) => Some(
                div()
                    .debug_selector(|| "transcript-words".into())
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(cx.palette().placeholder)
                    .child(said)
                    .into_any_element(),
            ),
            None => None,
        };
        // useChatThread's local error row, a runtime error where the transcript would be.
        let refused = self.refused.clone().map(|said| {
            let row = Row { key: "local-error:0".to_owned(), kind: RowKind::Error, text: said, turn: None };
            div()
                .debug_selector(|| "history-refused".into())
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .child(rows::row(self, 0, &row, window, cx))
        });
        let lost = match &self.link {
            Link::Lost(_) => Some(rows::lost_line(cx)),
            Link::Live => None,
        };
        // On screen means at or past the list's top and not yet past its bottom; past the first row below, none are.
        let top = self.list.logical_scroll_top().item_ix;
        let mut below = false;
        let marks: Vec<(usize, bool)> = self
            .transcript
            .rows()
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.kind, RowKind::Person { .. }))
            .map(|(ix, _)| {
                below = below || (ix >= top && self.list.item_is_below_viewport(ix) != Some(false));
                (ix, ix >= top && !below && self.list.item_is_above_viewport(ix) == Some(false))
            })
            .collect();
        let rail = minimap(self, marks, self.list.viewport_bounds().size.width, window.viewport_size().height, window, cx);
        let rows = self.transcript.rows();
        self.shimmers
            .retain(|k, _| rows.iter().any(|r| &r.key == k && matches!(r.kind, RowKind::Live { active: true, .. } | RowKind::Thinking)));
        let built = self.span_frame;
        self.clock_spots.retain(|_, (_, _, claimed)| *claimed == built);
        let at = |key: &str| self.transcript.rows().iter().position(|r| r.key == key);
        let mut over: Vec<Follow> = Vec::new();
        for (key, band) in &self.shimmers {
            if let Some(ix) = at(key) {
                let (spot, band) = shimmer::band(band, cx);
                over.push(Follow::new(spot, self.list.clone(), ix, band));
            }
        }
        for (key, (spot, look, _)) in &self.clock_spots {
            if let (Some(ix), Some((_, clock))) = (at(key), self.clocks.get(key)) {
                // A retried model call's clock and a machine wait's are in the mono face, as `WorkingTimelineRow` draws them.
                let mono = matches!(
                    self.transcript.rows()[ix].kind,
                    RowKind::Working { retry: Some(_), .. } | RowKind::Working { wait: Some(_), .. }
                );
                let text = div()
                    .debug_selector(|| "clock".into())
                    .flex()
                    .items_center()
                    .h(look.height)
                    .whitespace_nowrap()
                    .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
                    .text_size(look.size)
                    .when_some(look.line_height, |el, lh| el.line_height(lh))
                    .text_color(cx.theme().muted_foreground)
                    .child(clock.clone());
                over.push(Follow::new(spot.clone(), self.list.clone(), ix, text.into_any_element()));
            }
        }
        find::paint(self, cx);
        let find_bar = find::bar(self, window, cx);
        let pill = toolbar::scroll_to_end(self, shown, cx);
        let quote = toolbar::toolbar(self, cx);
        // The last thread's rows held through a switch act for the thread they were drawn for, so they take no input.
        let held = self.holding().then(|| div().debug_selector(|| "transcript-held".into()).absolute().inset_0().occlude());
        let me = cx.entity().downgrade();
        let heard = crate::scroll_trace::recording().then(|| (self.list.clone(), me.clone()));
        let top_key = self.transcript.rows().get(top).map(|r| r.key.clone());
        crate::scroll_trace::note(|w| (w.view_ms, w.top) = (w.view_ms + began.elapsed().as_secs_f64() * 1000., top_key));
        v_flex()
            .size_full()
            .relative()
            .when(self.find.is_open(), |el| el.key_context(keys::THREAD_FIND))
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &keys::FindInThread, window, cx| find::open(this, window, cx)))
            .on_action(cx.listener(|this, _: &keys::FindOlder, window, cx| find::step(this, -1, window, cx)))
            .on_action(cx.listener(|this, _: &keys::FindNewer, window, cx| find::step(this, 1, window, cx)))
            .on_scroll_wheel(cx.listener(|this, _, _, _| {
                this.revealing = None;
                this.glide = None;
            }))
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                if event.keystroke.key != "escape" {
                    return;
                }
                if toolbar::dismiss(this, cx) {
                    cx.stop_propagation();
                } else if this.find.is_open() {
                    find::close(this, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_mouse_down(
                gpui_kit::MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    // A press a field over the transcript took, the docked composer's, leaves the focus there.
                    if !this.focus.contains_focused(window, cx) && !window.default_prevented() {
                        this.focus.focus(window, cx);
                    }
                }),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(self.rows_view.clone().cached(StyleRefinement::default().size_full()))
                    .child(div().absolute().top_0().left_0().size_0().children(over))
                    .children(words)
                    .children(refused)
                    // `z-40` in minimap.tsx: the ruler stands over the chat's drop frame.
                    .children(rail.map(gpui_kit::deferred))
                    .children(pill)
                    .children(held)
                    .children(find_bar),
            )
            .children(quote)
            .children(self.row_menu.overlay())
            .child(canvas(|_, _, _| {}, move |_, _, window, _| toolbar::listen(me.clone(), window)).absolute().size_0())
            .children(status.map(|s| div().absolute().top_4().left_4().text_sm().text_color(muted).child(s)))
            .children(heard.map(|(list, me)| {
                canvas(
                    |_, _, _| {},
                    move |_, _, window, _| {
                        let list = list.clone();
                        let me = me.clone();
                        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                            let bounds = list.viewport_bounds();
                            if phase == DispatchPhase::Capture && bounds.contains(&event.position) {
                                let spot = me.upgrade().map(|view| view.read(cx).spot(event.position.y));
                                crate::scroll_trace::heard(event, bounds.origin, spot, window, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_0()
            }))
            .children(lost)
            .when(frames.is_some() || presented.is_some() || drawn.is_some() || loading_seen.is_some(), |this| {
                this.child(
                    canvas(
                        |_, _, _| {},
                        // A deferred callback runs when the update that paints ends, right after GPUI presents the frame.
                        move |_, _, _, cx| {
                            if let Some(frames) = frames.clone() {
                                cx.defer(move |_| frames.borrow_mut().push(Instant::now()));
                            }
                            if let Some(cell) = presented.clone() {
                                cx.defer(move |_| {
                                    cell.borrow_mut().get_or_insert_with(Instant::now);
                                });
                            }
                            if let Some(drawn) = drawn {
                                cx.defer(move |_| _ = drawn.send(()));
                            }
                            if let Some(seen) = loading_seen.clone() {
                                cx.defer(move |_| seen.borrow_mut().push(Instant::now()));
                            }
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
    }
}
