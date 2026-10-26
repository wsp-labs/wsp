// SPDX-License-Identifier: AGPL-3.0-only
//! `SpawnLine`: the line under the message a sub-thread's machine was made for, and its steps.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Hsla, SharedString, Window};

use super::META;
use crate::thread_view::ThreadView;
use crate::transcript::{self, Row, StartFailed};
use crate::ui::{tip, Button, Glyph, Palette};

/// `SpawnLine`: one centred quiet line under the message a sub-thread's machine was made for: the spinner, the words and
/// the elapsed while under way, "Started on Boat in 42s" once the agent answered, the refusal ink where it failed, and a
/// chevron opening the steps, each with its mark, words, facts and time, then the raw log one level further down. A
/// failed start opens on its failed step with Try again.
pub(super) fn start_failed(
    view: &mut ThreadView,
    row: &Row,
    failed: &StartFailed,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let key = row.key.as_str();
    let live = !failed.failed && failed.took.is_none();
    let open_state = window.use_keyed_state(SharedString::from(format!("spawn-open-{key}")), cx, |_, _| None::<bool>);
    let raw_state = window.use_keyed_state(SharedString::from(format!("spawn-raw-{key}")), cx, |_, _| false);
    let (open, raw) = (open_state.read(cx).unwrap_or(failed.failed), *raw_state.read(cx));
    let flip_open = {
        let state = open_state.clone();
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |b, cx| {
                *b = Some(!open);
                cx.notify();
            })
        }
    };
    let flip_raw = {
        let state = raw_state.clone();
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |b, cx| {
                *b = !*b;
                cx.notify();
            })
        }
    };
    let error = p.error_foreground;
    let ink = if failed.failed { error } else { p.muted_foreground };
    let hover = if failed.failed { error } else { p.foreground };
    let mark = |g: Glyph, ink: Hsla| Icon::new(g).with_size(px(12.)).text_color(ink).into_any_element();
    let lead = if live {
        Some(crate::ui::spinner(cx).size(px(12.)).into_any_element())
    } else {
        failed.failed.then(|| mark(Glyph::CircleAlert, ink))
    };
    // The elapsed ticks on the row's clock over the cached rows; its slot keeps room for "59m 59s".
    let clock = live.then(|| {
        view.clock(row, failed.since, cx);
        let look = super::super::shimmer::ClockLook { size: META.0, line_height: Some(META.1), height: META.1 };
        div().flex_none().w(px(44.)).child(super::super::shimmer::clock_slot(view, row, look, cx))
    });
    let head = h_flex()
        .id(SharedString::from(format!("spawn-line-{key}")))
        .debug_selector(|| "spawn-line".into())
        .gap(px(6.))
        .text_color(ink)
        .cursor_pointer()
        .hover(move |s| s.text_color(hover))
        .children(lead)
        .child(row.text.clone())
        .children(clock)
        .child(mark(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }, ink))
        .on_click(flip_open);
    let now = view
        .frozen()
        .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64));
    let prompt = failed.prompt.clone();
    let steps = open.then(|| {
        let mono = cx.theme().mono_font_family.clone();
        // A failed start read with no steps says its failed step alone, with what the host said.
        let lone = (failed.failed && !failed.steps.iter().any(|s| s.said.is_some())).then(|| transcript::SpawnStep {
            id: "failed".to_owned(),
            name: failed.step.clone(),
            note: None,
            state: transcript::StepState::Failed,
            ms: None,
            since: None,
            said: Some(failed.said.clone()),
            fix: failed.fix.clone(),
        });
        let lines = lone.iter().chain(failed.steps.iter().filter(|_| lone.is_none())).map(|step| {
            let (glyph, mark_ink) = match step.state {
                transcript::StepState::Done => (Some(Glyph::CircleCheck), p.muted_foreground),
                transcript::StepState::Warned => (Some(Glyph::TriangleAlert), p.warning_foreground),
                transcript::StepState::Failed => (Some(Glyph::CircleAlert), error),
                transcript::StepState::Waiting => (Some(Glyph::Circle), p.muted_foreground.opacity(0.5)),
                transcript::StepState::Working => (None, p.muted_foreground),
            };
            let step_mark = glyph.map_or_else(|| crate::ui::spinner(cx).size(px(12.)).into_any_element(), |g| mark(g, mark_ink));
            let time = match (step.since, step.ms) {
                (Some(since), _) => crate::ui::status::fmt_elapsed(now - since),
                (None, Some(ms)) => crate::words::fmt_duration(ms),
                _ => String::new(),
            };
            let name_ink = if step.state == transcript::StepState::Failed { error } else { p.muted_foreground };
            let said = step.said.clone().map(|said| {
                let again = prompt.clone().map(|prompt| {
                    let again = Button::outline("spawn-try-again", "Try again").xs().on_click(cx.listener(move |view, _, _, cx| {
                        view.send(prompt.clone(), cx);
                        if let Some(thread) = view.thread().map(str::to_owned) {
                            cx.emit(crate::thread_view::ThreadViewEvent::SentElsewhere { thread, prompt: prompt.clone() });
                        }
                    }));
                    div().debug_selector(|| "spawn-try-again".into()).child(again)
                });
                v_flex()
                    .items_start()
                    .gap(px(8.))
                    .pl(px(20.))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .text_color(error)
                            .child(said)
                            .children(step.fix.clone().map(|fix| div().text_color(p.foreground).child(format!(" {fix}")))),
                    )
                    .children(again)
            });
            let selector = format!("spawn-step-{}", step.id);
            v_flex()
                .debug_selector(move || selector.clone())
                .gap(px(4.))
                .child(
                    h_flex()
                        .min_w_0()
                        .gap(px(8.))
                        .child(div().flex_none().child(step_mark))
                        .child(
                            div()
                                .when(step.note.is_none(), |el| el.min_w_0())
                                .when(step.note.is_some(), |el| el.flex_none())
                                .text_color(name_ink)
                                .child(step.name.clone()),
                        )
                        .children(step.note.clone().map(|note| {
                            div()
                                .id(SharedString::from(format!("spawn-note-{}", step.id)))
                                .min_w_0()
                                .truncate()
                                .opacity(0.7)
                                .tooltip(tip(note.clone()))
                                .child(note)
                        }))
                        .child(div().ml_auto().flex_none().font_family(mono.clone()).child(time)),
                )
                .children(said)
        });
        v_flex()
            .debug_selector(|| "spawn-steps".into())
            .mt(px(8.))
            .w_full()
            .max_w(px(576.))
            .gap(px(4.))
            .pl(px(24.))
            .children(lines)
            .child(
                h_flex()
                    .id(SharedString::from(format!("spawn-details-{key}")))
                    .mt(px(4.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.text_color(p.foreground))
                    .child(mark(if raw { Glyph::ChevronDown } else { Glyph::ChevronRight }, p.muted_foreground))
                    .child(if raw { "Hide details" } else { "Show details" })
                    .on_click(flip_raw),
            )
            .when(raw, |el| el.child(div().pl(px(20.)).font_family(mono.clone()).child(failed.log.join("\n"))))
    });
    v_flex()
        .debug_selector(|| "start-failed".into())
        .items_center()
        .text_size(META.0)
        .line_height(META.1)
        .text_color(p.muted_foreground)
        .child(head)
        .children(steps)
        .into_any_element()
}
