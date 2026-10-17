// SPDX-License-Identifier: AGPL-3.0-only
//! Each transcript row as the web app's timeline draws it (`apps/web/src/components/chat/timeline/rows.tsx`,
//! `workGroup.tsx`, `workEntry.tsx`, `callBlock.tsx`, `working.tsx`): every size below is that class's, in pixels.

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Context, FontWeight, Hsla, Pixels, Rgba, SharedString, Task, Window};

use super::markdown;
use crate::thread_view::ThreadView;
use crate::transcript::{Call, FooterWord, Glyph as RowGlyph, Row, RowKind, Tone};
use crate::ui::{ActivePalette, Glyph, Palette};

/// `max-w-3xl`: the transcript column, so both apps wrap the same text at the same width.
pub const COLUMN: Pixels = px(768.);
/// `sm:px-5` inside the 6 px scrollbar gutter `scrollbar-gutter: stable both-edges` keeps on each side.
pub const SIDE: Pixels = px(26.);
/// The list's header and footer spacers, `h-3 sm:h-4`.
pub const END_SPACE: Pixels = px(16.);
/// `--chat-composer-inset`: the web composer dock's height with an empty draft, which the list ends with so its last
/// row clears the composer floating over it.
pub const COMPOSER_INSET: Pixels = px(110.);
/// `text-sm leading-relaxed`.
const SM: (Pixels, Pixels) = (px(14.), px(22.75));
/// `max-h-72` on a command's output, and `min(18rem, 50dvh)` on an open group's list.
const OUTPUT_MAX: Pixels = px(288.);
/// A command's output folds past 14 lines to its first (or, after a failure, last) 10.
const FOLD_PAST: usize = 14;
const FOLD_TO: usize = 10;

/// The text a row's parsed document is built from.
pub fn body_text(row: &Row) -> String {
    match row.kind {
        RowKind::Person { .. } => markdown::person_text(&row.text),
        RowKind::Agent { streaming: false, .. } if row.text.trim().is_empty() => "(empty response)".to_owned(),
        _ => row.text.clone(),
    }
}

/// `color-mix(in srgb, a share, b)`.
pub fn mix(a: Hsla, b: Hsla, share: f32) -> Hsla {
    let (a, b) = (Rgba::from(a), Rgba::from(b));
    let lerp = |x: f32, y: f32| x * share + y * (1. - share);
    Rgba { r: lerp(a.r, b.r), g: lerp(a.g, b.g), b: lerp(a.b, b.b), a: lerp(a.a, b.a) }.into()
}

fn glyph(g: RowGlyph) -> Glyph {
    match g {
        RowGlyph::Eye => Glyph::Eye,
        RowGlyph::SquarePen => Glyph::SquarePen,
        RowGlyph::FilePlus => Glyph::FilePlus,
        RowGlyph::Terminal => Glyph::Terminal,
        RowGlyph::Globe => Glyph::Globe,
        RowGlyph::Search => Glyph::Search,
        RowGlyph::Wrench => Glyph::Wrench,
        RowGlyph::Hammer => Glyph::Hammer,
        RowGlyph::Bot => Glyph::Bot,
        RowGlyph::Zap => Glyph::Zap,
        RowGlyph::Brain => Glyph::Brain,
        RowGlyph::Info => Glyph::Info,
        RowGlyph::CircleAlert => Glyph::CircleAlert,
        RowGlyph::Minimize => Glyph::Minimize2,
    }
}

/// `flex size-6 items-center justify-center` holding a 16 px glyph at opacity 0.7.
fn glyph_slot(g: RowGlyph, ink: Hsla) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(glyph(g)).with_size(px(16.)).text_color(ink.alpha(ink.a * 0.7)))
}

trait Alpha {
    fn alpha(self, a: f32) -> Self;
}

impl Alpha for Hsla {
    fn alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }
}

/// `ChevronDownIcon size-3 opacity-70`, turned over when open.
fn chevron(open: bool, ink: Hsla) -> impl IntoElement {
    let g = if open { Glyph::ChevronUp } else { Glyph::ChevronDown };
    Icon::new(g).with_size(px(12.)).text_color(ink.alpha(ink.a * 0.7))
}

/// The row's wrapper: the centred column, and the bottom padding the web's `TimelineRowContent` picks by kind.
fn frame(ix: usize, last: bool, foot: Pixels, content: impl IntoElement) -> AnyElement {
    let column = div()
        .debug_selector(|| format!("row-{ix}"))
        .mx_auto()
        .w_full()
        .max_w(COLUMN)
        .when(ix == 0, |el| el.pt(END_SPACE))
        .pb(foot)
        .when(last, |el| el.pb(foot + END_SPACE + COMPOSER_INSET))
        .child(content);
    div().w_full().px(SIDE).child(column).into_any_element()
}

fn foot(kind: &RowKind) -> Pixels {
    match kind {
        RowKind::Calls { .. } => px(4.),
        RowKind::Group { open: true, .. } | RowKind::Live { open: true, .. } => px(0.),
        RowKind::Fold { .. } | RowKind::Stopped | RowKind::Working { .. } => px(6.),
        RowKind::Agent { meta: false, .. } | RowKind::Group { .. } | RowKind::Live { .. } | RowKind::Thinking => px(8.),
        RowKind::Work { .. } | RowKind::Error => px(8.),
        RowKind::Footer { .. } => px(0.),
        RowKind::Person { .. } | RowKind::Agent { .. } => px(16.),
    }
}

pub fn row(view: &mut ThreadView, ix: usize, row: &Row, _: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let last = ix + 1 == view.rows().len();
    let shown = match &row.kind {
        RowKind::Person { .. } => person(view, row, &p, cx),
        RowKind::Agent { meta, .. } => agent(view, row, *meta, &p, cx),
        RowKind::Fold { open } => fold(row, *open, &p, cx),
        RowKind::Stopped => notice_line(&row.text, &p).into_any_element(),
        RowKind::Group { group, glyph, open } => group_line(row, group, *glyph, *open, &p, cx),
        RowKind::Live { group, glyph, verb, mono, since, open } => {
            live_line(view, row, group, *glyph, verb.as_deref(), *mono, *since, *open, &p, cx)
        }
        RowKind::Calls { calls, .. } => calls_list(calls, &p, cx),
        RowKind::Work { tone, glyph } => {
            div().py(px(2.)).child(plain_line(&row.text, None, false, *tone, *glyph, false, None, &p, cx)).into_any_element()
        }
        RowKind::Error => error_line(&row.text, &p, cx),
        RowKind::Working { since } => working(view, row, *since, &p, cx),
        RowKind::Thinking => {
            div().min_h(px(28.)).child(activity(RowGlyph::Brain, p.foreground, None, "Thinking".into(), false, &p, cx)).into_any_element()
        }
        RowKind::Footer { word } => footer(*word, &p, cx),
    };
    frame(ix, last, foot(&row.kind), shown)
}

/// `PERSON_BUBBLE`: right-aligned, at most 80% of the column, 18 px round on `--message-surface`, then the meta line
/// that shows on hover.
fn person(view: &mut ThreadView, row: &Row, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let body = view.body(row, cx);
    let group = SharedString::from(format!("person-{}", row.key));
    v_flex()
        .id(SharedString::from(row.key.clone()))
        .group(group.clone())
        .w_full()
        .items_end()
        .gap(px(4.))
        .child(
            div()
                .max_w(relative(0.8))
                .rounded(px(18.))
                .bg(p.accent)
                .p(px(12.))
                .text_color(p.foreground)
                .child(markdown::view(&body, true, cx)),
        )
        .child(meta_line(row, true, &group, p))
        .into_any_element()
}

/// The reply's markdown, `px-1 py-0.5`, and under the settled turn's last reply its hover-only meta line.
fn agent(view: &mut ThreadView, row: &Row, meta: bool, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let body = view.body(row, cx);
    let group = SharedString::from(format!("agent-{}", row.key));
    v_flex()
        .id(SharedString::from(row.key.clone()))
        .group(group.clone())
        .w_full()
        .child(div().px(px(4.)).py(px(2.)).child(markdown::view(&body, false, cx)))
        .when(meta, |el| el.child(div().px(px(4.)).mt(px(6.)).child(meta_line(row, false, &group, p))))
        .into_any_element()
}

/// The stamp and Copy under a message, laid out always and shown only while the row is hovered, as the web's
/// `opacity-0 group-hover:opacity-100` keeps its height.
fn meta_line(row: &Row, person: bool, group: &SharedString, p: &Palette) -> impl IntoElement {
    let at = match row.kind {
        RowKind::Person { at, .. } | RowKind::Agent { at, .. } => at,
        _ => None,
    };
    let stamp = at.map(stamp).unwrap_or_default();
    let text = row.text.clone();
    let copy = crate::ui::Button::icon_xs(SharedString::from(format!("copy-{}", row.key)), Glyph::Copy)
        .on_click(move |_, _, cx| cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text.clone())));
    let buttons = h_flex().gap(px(2.)).when(!row.text.trim().is_empty(), |el| el.child(copy));
    let line = h_flex().h(px(24.)).gap(px(8.)).opacity(0.).group_hover(group.clone(), |s| s.opacity(1.));
    if person {
        line.w_full()
            .max_w(relative(0.8))
            .justify_end()
            .pr(px(4.))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .child(stamp)
            .child(buttons)
    } else {
        line.text_size(px(13.)).text_color(p.muted_foreground).child(buttons).child(stamp)
    }
}

/// `formatChatTimestamp`: `Oct 1, 3:54 PM` in the machine's own zone, with the year when it is not this one.
pub fn stamp(ms: f64) -> String {
    use chrono::{Datelike, Local, TimeZone};
    let Some(at) = Local.timestamp_millis_opt(ms as i64).single() else { return String::new() };
    if at.year() == Local::now().year() {
        at.format("%b %-d, %-I:%M %p").to_string()
    } else {
        at.format("%b %-d, %Y, %-I:%M %p").to_string()
    }
}

/// `Worked for 12s` and its chevron: the button hugs its text, muted, the ink going to the foreground on hover.
fn fold(row: &Row, open: bool, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let turn = row.turn.clone().unwrap_or_default();
    let g = if open { Glyph::ChevronDown } else { Glyph::ChevronRight };
    div()
        .pt(px(4.))
        .pb(px(8.))
        .child(
            h_flex()
                .id(SharedString::from(row.key.clone()))
                .debug_selector(|| "fold".into())
                .cursor_pointer()
                .gap(px(4.))
                .px(px(4.))
                .rounded(px(8.))
                .text_size(SM.0)
                .line_height(SM.1)
                .text_color(p.muted_foreground)
                .hover(|s| s.text_color(cx.theme().foreground))
                .child(row.text.clone())
                .child(Icon::new(g).with_size(px(14.)))
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&turn, cx))),
        )
        .into_any_element()
}

/// `You stopped after 4.2s`: the fold's look, with no chevron and no click.
fn notice_line(text: &str, p: &Palette) -> impl IntoElement {
    div()
        .pt(px(4.))
        .pb(px(8.))
        .child(div().px(px(4.)).text_size(SM.0).line_height(SM.1).text_color(p.muted_foreground).child(text.to_owned()))
}

/// A tool group's summary line: the whole line toggles its list, with no chevron.
fn group_line(row: &Row, group: &str, g: RowGlyph, _open: bool, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let id = group.to_owned();
    h_flex()
        .id(SharedString::from(row.key.clone()))
        .debug_selector(|| "group".into())
        .w_full()
        .min_h(px(24.))
        .p(px(2.))
        .gap(px(6.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|s| s.bg(p.accent.alpha(p.accent.a * 0.2)))
        .text_size(SM.0)
        .line_height(SM.1)
        .child(glyph_slot(g, p.muted_foreground))
        .child(div().flex_1().min_w_0().truncate().text_color(p.muted_foreground).child(row.text.clone()))
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&id, cx)))
        .into_any_element()
}

/// `LiveActivityContent`: a glyph, the verb in the label ink and the rest after it, the mono face for a command.
fn activity(g: RowGlyph, glyph_ink: Hsla, verb: Option<&str>, text: SharedString, mono: bool, p: &Palette, cx: &App) -> impl IntoElement {
    h_flex()
        .min_h(px(24.))
        .min_w_0()
        .py(px(2.))
        .px(px(2.))
        .gap(px(6.))
        .text_size(SM.0)
        .line_height(SM.1)
        .text_color(p.muted_foreground)
        .child(glyph_slot(g, glyph_ink))
        .child(
            h_flex()
                .min_w_0()
                .flex_1()
                .gap(px(4.))
                .children(verb.map(|v| div().flex_none().child(v.to_owned())))
                .child(div().min_w_0().truncate().when(mono, |el| el.font_family(cx.theme().mono_font_family.clone())).child(text)),
        )
}

#[allow(clippy::too_many_arguments)]
fn live_line(
    view: &mut ThreadView,
    row: &Row,
    group: &str,
    g: RowGlyph,
    verb: Option<&str>,
    mono: bool,
    since: Option<f64>,
    _open: bool,
    p: &Palette,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let id = group.to_owned();
    let timer = since.map(|since| view.clock(row, since, cx));
    h_flex()
        .id(SharedString::from(row.key.clone()))
        .w_full()
        .min_h(px(24.))
        .rounded(px(8.))
        .cursor_pointer()
        .gap(px(12.))
        .child(activity(g, p.muted_foreground, verb, row.text.clone().into(), mono, p, cx))
        .children(timer.map(|t| div().flex_none().text_size(px(12.)).text_color(p.muted_foreground).child(t)))
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&id, cx)))
        .into_any_element()
}

/// An open group's calls, `ms-3`, capped at `min(18rem, 50dvh)` and scrolling past it.
fn calls_list(calls: &[Call], p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let opened = calls.iter().any(|c| c.open);
    v_flex()
        .id("calls")
        .ml(px(12.))
        .rounded(px(8.))
        .when(!opened, |el| el.max_h(OUTPUT_MAX).overflow_y_scroll())
        .children(calls.iter().map(|call| call_row(call, p, cx)))
        .into_any_element()
}

fn call_row(call: &Call, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    if call.command.is_some() || call.patch.is_some() {
        return block(call, p, cx);
    }
    let opens = call.body.as_ref().is_some_and(|b| !b.trim().is_empty());
    let key = call.key.clone();
    let line = plain_line(&call.label, call.verb.as_deref(), call.mono, call.tone, call.glyph, opens, Some(call.open), p, cx);
    v_flex()
        .id(SharedString::from(call.key.clone()))
        .px(px(2.))
        .rounded(px(8.))
        .when(opens, |el| {
            el.cursor_pointer()
                .hover(|s| s.bg(p.accent.alpha(p.accent.a * 0.2)))
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
        })
        .child(line)
        .when(call.open, |el| el.children(call.body.clone().map(|b| detail(b, p, cx))))
        .into_any_element()
}

/// A tool row's line (`PlainWorkEntryRow`): glyph, label, and the 16 px chevron slot kept even when it cannot open.
#[allow(clippy::too_many_arguments)]
fn plain_line(
    text: &str,
    verb: Option<&str>,
    mono: bool,
    tone: Tone,
    g: RowGlyph,
    opens: bool,
    open: Option<bool>,
    p: &Palette,
    cx: &App,
) -> impl IntoElement {
    let glyph_ink = match tone {
        Tone::Thinking | Tone::Error => p.foreground,
        _ => p.muted_foreground,
    };
    let mono = mono || tone == Tone::Notice;
    h_flex()
        .gap(px(6.))
        .text_size(SM.0)
        .line_height(SM.1)
        .child(glyph_slot(g, glyph_ink))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap(px(6.))
                .text_color(p.muted_foreground)
                .children(verb.map(|v| div().flex_none().child(v.to_owned())))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
                        .child(text.to_owned()),
                ),
        )
        .when_some(open, |el, open| {
            el.child(
                div()
                    .flex_none()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!opens, |el| el.invisible())
                    .child(chevron(open, p.muted_foreground)),
            )
        })
}

/// An open tool row's detail: `mt-1 ms-7 border-s border-border/45 ps-3 pt-0.5`, mono at 11 px.
fn detail(text: String, p: &Palette, cx: &App) -> impl IntoElement {
    div().mt(px(4.)).ml(px(28.)).border_l_1().border_color(p.border.alpha(p.border.a * 0.45)).pl(px(12.)).pt(px(2.)).child(
        div()
            .id("detail")
            .max_h(px(256.))
            .overflow_y_scroll()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(11.))
            .line_height(px(17.875))
            .text_color(p.muted_foreground)
            .child(text),
    )
}

/// `CODE_BLOCK_SURFACE`: light, a 1 px border at 70% on `--secondary`; dark, no border on `--input` at 32%.
pub fn code_surface(p: &Palette, cx: &App) -> (Hsla, Hsla) {
    if cx.theme().is_dark() {
        (gpui_kit::transparent_black(), p.input.alpha(p.input.a * 0.32))
    } else {
        (p.border.alpha(p.border.a * 0.7), p.muted)
    }
}

/// A shell or edit call (`CallRow`): a 28 px line in a 10 px round block that takes the code block surface when open.
fn block(call: &Call, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let (edge, fill) = code_surface(p, cx);
    let key = call.key.clone();
    let mono = cx.theme().mono_font_family.clone();
    let (g, verb, target) = match (&call.command, &call.patch) {
        (Some(command), _) => {
            let line = if call.open { command.clone() } else { command.lines().next().unwrap_or_default().to_owned() };
            (if call.failed && call.exit_code.is_none() { RowGlyph::CircleAlert } else { RowGlyph::Terminal }, None, line)
        }
        _ => (call.glyph, call.verb.clone(), call.label.clone()),
    };
    let mut facts: Vec<AnyElement> = Vec::new();
    if let Some(patch) = &call.patch {
        let (added, removed) = stat(patch);
        facts.push(
            h_flex()
                .gap(px(4.))
                .font_family(mono.clone())
                .text_size(px(12.))
                .line_height(px(16.))
                .child(div().text_color(p.success).child(format!("+{added}")))
                .child(div().text_color(p.error_foreground).child(format!("-{removed}")))
                .into_any_element(),
        );
    }
    if let Some(code) = call.exit_code.filter(|c| *c != 0) {
        facts.push(div().text_color(p.error_foreground).child(format!("Exited {code}")).into_any_element());
    }
    if let Some(ms) = call.duration_ms {
        facts.push(div().child(command_duration(ms)).into_any_element());
    }
    let heading = h_flex()
        .flex_1()
        .min_w_0()
        .gap(px(4.))
        .text_size(SM.0)
        .line_height(px(24.))
        .text_color(p.muted_foreground)
        .children(verb.map(|v| div().flex_none().child(v)))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .font_family(mono.clone())
                .when(call.patch.is_some(), |el| el.text_color(p.foreground))
                .when(!call.open, |el| el.truncate())
                .child(target),
        );
    let line = h_flex()
        .id(SharedString::from(call.key.clone()))
        .items_start()
        .min_h(px(28.))
        .p(px(2.))
        .gap(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(p.accent.alpha(p.accent.a * 0.2)))
        .child(glyph_slot(g, p.muted_foreground))
        .child(heading)
        .child(h_flex().h(px(24.)).flex_none().gap(px(12.)).text_size(px(12.)).text_color(p.muted_foreground).children(facts))
        .child(div().flex_none().h(px(24.)).w(px(16.)).flex().items_center().justify_center().child(chevron(call.open, p.muted_foreground)))
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)));
    let body = call.open.then(|| match (&call.patch, &call.body) {
        (Some(patch), _) => diff(patch, &call.label, p, cx).into_any_element(),
        (None, Some(output)) if !output.trim().is_empty() => output_band(output, call.exit_code.unwrap_or(0) != 0, p, cx),
        _ => run_footer(div().child("No output"), None, p).into_any_element(),
    });
    v_flex()
        .rounded(px(10.))
        .border_1()
        .border_color(if call.open { edge } else { gpui_kit::transparent_black() })
        .when(call.open, |el| el.bg(fill))
        .overflow_hidden()
        .child(line)
        .children(body)
        .into_any_element()
}

fn stat(patch: &str) -> (usize, usize) {
    let lines = patch.lines().filter(|l| !l.starts_with("+++") && !l.starts_with("---"));
    lines.fold((0, 0), |(a, r), l| match l.as_bytes().first() {
        Some(b'+') => (a + 1, r),
        Some(b'-') => (a, r + 1),
        _ => (a, r),
    })
}

/// `commandDuration` in `timeline/callFacts.ts`: tenths floored under 10 s.
pub fn command_duration(ms: f64) -> String {
    if ms < 1_000. {
        return format!("{}ms", ms.round().max(0.) as u64);
    }
    if ms < 10_000. {
        return format!("{:.1}s", (ms / 100.).floor() / 10.);
    }
    let s = (ms / 1_000.).round() as u64;
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => {
            if s % 60 == 0 {
                format!("{}m", s / 60)
            } else {
                format!("{}m {}s", s / 60, s % 60)
            }
        }
        s => {
            let m = (s % 3_600) / 60;
            if m == 0 {
                format!("{}h", s / 3_600)
            } else {
                format!("{}h {m}m", s / 3_600)
            }
        }
    }
}

/// A finished command's output (`OutputBand`): past 14 lines its first 10, or after a failure its last 10.
fn output_band(output: &str, failed: bool, p: &Palette, cx: &App) -> AnyElement {
    let text = output.trim_end().trim_start_matches('\n');
    let lines: Vec<&str> = text.lines().collect();
    let (shown, hidden) = if lines.len() > FOLD_PAST {
        let kept = if failed { &lines[lines.len() - FOLD_TO..] } else { &lines[..FOLD_TO] };
        (kept.join("\n"), lines.len() - FOLD_TO)
    } else {
        (text.to_owned(), 0)
    };
    let more = (hidden > 0).then(|| {
        let n = thousands(hidden);
        div().child(if failed { format!("+{n} earlier lines") } else { format!("+{n} lines") })
    });
    v_flex()
        .child(
            div()
                .id("output")
                .max_h(OUTPUT_MAX)
                .overflow_y_scroll()
                .bg(p.background)
                .px(px(12.))
                .py(px(8.))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(p.foreground)
                .child(shown),
        )
        .child(run_footer(div().children(more), Some("Copy output"), p))
        .into_any_element()
}

fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `RunFooter`: at least 36 px, 12 px words on the left and a ghost xs act on the right.
fn run_footer(left: impl IntoElement, act: Option<&'static str>, p: &Palette) -> impl IntoElement {
    h_flex()
        .min_h(px(36.))
        .justify_between()
        .gap(px(12.))
        .py(px(4.))
        .pr(px(6.))
        .pl(px(12.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .child(h_flex().min_w_0().gap(px(12.)).child(left))
        .children(act.map(|label| crate::ui::Button::text(label, label).xs().glyph(Glyph::Copy)))
}

/// A diff folds past this many lines to its first `DIFF_FOLD_TO`.
const DIFF_FOLD_PAST: usize = 20;
const DIFF_FOLD_TO: usize = 16;

/// One line of a unified patch as the diff draws it: its number on its own side, its mark, its text.
#[derive(Debug, PartialEq)]
struct DiffLine<'a> {
    number: usize,
    mark: Option<char>,
    text: &'a str,
}

/// The lines of a unified patch with each one's number, old for a removed line and new for the rest; hunk headers
/// set the numbers and draw nothing.
fn diff_lines(patch: &str) -> Vec<DiffLine<'_>> {
    let (mut old, mut new) = (1, 1);
    let mut lines = Vec::new();
    for line in patch.lines() {
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") || line.starts_with("index ") {
            continue;
        }
        if let Some(hunk) = line.strip_prefix("@@ ") {
            let start = |sign: char| {
                hunk.split_whitespace().find_map(|w| w.strip_prefix(sign)).and_then(|w| w.split(',').next()?.parse().ok()).unwrap_or(1)
            };
            (old, new) = (start('-'), start('+'));
            continue;
        }
        let (mark, text) = match line.chars().next() {
            Some(c @ ('+' | '-')) => (Some(c), &line[1..]),
            Some(' ') => (None, &line[1..]),
            _ => (None, line),
        };
        let number = if mark == Some('-') { old } else { new };
        match mark {
            Some('-') => old += 1,
            Some(_) => new += 1,
            None => {
                old += 1;
                new += 1;
            }
        }
        lines.push(DiffLine { number, mark, text });
    }
    lines
}

/// A unified patch inside an open edit block (`@pierre/diffs` FileDiff): `--code-background`, a 4 px bar and a tint on
/// each changed line, its number in the change's ink, the file's syntax colors, 13/20 px mono, folded past 20 lines.
fn diff(patch: &str, path: &str, p: &Palette, cx: &App) -> impl IntoElement {
    let dark = cx.theme().is_dark();
    let ground = mix(p.card, p.background, 0.9);
    let (add, del) = (mix(p.success, ground, 0.06), mix(p.error, ground, 0.06));
    let lines = diff_lines(patch);
    let hidden = if lines.len() > DIFF_FOLD_PAST { lines.len() - DIFF_FOLD_TO } else { 0 };
    let shown = &lines[..lines.len() - hidden];
    let code: String = shown.iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    let lang = path.rsplit('.').next().filter(|ext| *ext != path).unwrap_or("");
    let styles = if lang.is_empty() { Vec::new() } else { markdown::highlight_code(lang, &code, dark) };
    let digits = shown.iter().map(|l| l.number).max().unwrap_or(1).to_string().len();
    let mut at = 0;
    let rows = shown.iter().map(|line| {
        let range = at..at + line.text.len();
        at = range.end + 1;
        let own: Vec<_> = styles
            .iter()
            .filter(|(r, _)| r.start < range.end && r.end > range.start)
            .map(|(r, style)| (r.start.max(range.start) - range.start..r.end.min(range.end) - range.start, *style))
            .collect();
        let text = SharedString::from(if line.text.is_empty() { " ".to_owned() } else { line.text.to_owned() });
        let (bar, fill, number_ink) = match line.mark {
            Some('+') => (Some(p.success), Some(add), p.success),
            Some(_) => (Some(p.error), Some(del), p.error),
            None => (None, None, p.muted_foreground),
        };
        // A removed line's bar is dashed, an added line's solid.
        let bar = match (line.mark, bar) {
            (Some('-'), Some(b)) => v_flex().gap(px(1.)).children((0..10).map(move |_| div().h(px(1.)).bg(b))).into_any_element(),
            (_, Some(b)) => div().size_full().bg(b).into_any_element(),
            _ => div().into_any_element(),
        };
        h_flex()
            .items_start()
            .child(div().w(px(4.)).h(px(20.)).flex_none().child(bar))
            .child(
                div()
                    .flex_none()
                    .w(px(8. * digits as f32 + 17.))
                    .pr(px(6.))
                    .text_right()
                    .text_color(number_ink)
                    .child(line.number.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(px(12.))
                    .when_some(fill, |el, f| el.bg(f))
                    .child(gpui_kit::StyledText::new(text).with_highlights(own)),
            )
    });
    v_flex()
        .bg(ground)
        .pt(px(7.))
        .pb(px(8.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(p.foreground)
        .children(rows)
        .when(hidden > 0, |el| el.child(run_footer(div().child(format!("+{} lines", thousands(hidden))), None, p)))
}

/// A failed turn's error: the error glyph and its words in the destructive ink at weight 500, wrapping.
fn error_line(text: &str, p: &Palette, _: &App) -> AnyElement {
    h_flex()
        .items_start()
        .py(px(2.))
        .px(px(2.))
        .gap(px(6.))
        .text_size(SM.0)
        .line_height(SM.1)
        .child(glyph_slot(RowGlyph::CircleAlert, p.destructive))
        .child(div().flex_1().min_w_0().font_weight(FontWeight::MEDIUM).text_color(p.destructive).child(text.to_owned()))
        .into_any_element()
}

/// `Working for 12s` at the start of the running turn.
fn working(view: &mut ThreadView, row: &Row, since: Option<f64>, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let line = h_flex().h(px(24.)).px(px(4.)).text_size(SM.0).line_height(SM.1).text_color(p.muted_foreground);
    let line = match since {
        Some(since) => line.child("Working for ").child(view.clock(row, since, cx)),
        None => line.child("Working..."),
    };
    div().pt(px(4.)).pb(px(8.)).child(line).into_any_element()
}

/// The settled footer's one mono word.
fn footer(word: FooterWord, p: &Palette, cx: &App) -> AnyElement {
    let (text, ink) = match word {
        FooterWord::Interrupted => ("interrupted", p.destructive),
        FooterWord::Failed => ("failed", p.destructive),
        FooterWord::LimitReached => ("limit reached", p.muted_foreground),
    };
    div()
        .w_full()
        .px(px(4.))
        .pb(px(8.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(11.))
        .line_height(px(16.5))
        .text_color(ink)
        .child(text)
        .into_any_element()
}

/// The line shown over the transcript while the host's socket is closed and the view follows again.
pub fn lost_line(cx: &App) -> impl IntoElement {
    let p = cx.palette();
    h_flex().absolute().top(px(8.)).left_0().right_0().justify_center().child(
        div()
            .debug_selector(|| "lost".into())
            .px(px(12.))
            .h(px(28.))
            .flex()
            .items_center()
            .rounded_full()
            .border_1()
            .border_color(p.border)
            .bg(p.popover)
            .text_size(px(12.))
            .text_color(p.muted_foreground)
            .child("Reconnecting to the host"),
    )
}

/// A live timer's text, ticking once a second in its own entity so the transcript does not draw again with it.
pub struct Clock {
    since: f64,
    now: Option<f64>,
    _tick: Option<Task<()>>,
}

impl Clock {
    pub fn new(since: f64, now: Option<f64>, cx: &mut Context<Self>) -> Self {
        let tick = now.is_none().then(|| {
            cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            })
        });
        Clock { since, now, _tick: tick }
    }
}

impl Render for Clock {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let now = self
            .now
            .unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0., |d| d.as_millis() as f64));
        div().child(timer((now - self.since).max(0.)))
    }
}

/// `WorkingTimer`: `12s`, `3m 5s` or `3m`, then `1h 4m` or `1h`.
pub fn timer(ms: f64) -> String {
    let s = (ms / 1_000.).floor() as u64;
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => {
            if s % 60 == 0 {
                format!("{}m", s / 60)
            } else {
                format!("{}m {}s", s / 60, s % 60)
            }
        }
        s => {
            let m = (s % 3_600) / 60;
            if m == 0 {
                format!("{}h", s / 3_600)
            } else {
                format!("{}h {m}m", s / 3_600)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timers_and_durations_read_as_the_web_app_writes_them() {
        assert_eq!(timer(12_900.), "12s");
        assert_eq!(timer(185_000.), "3m 5s");
        assert_eq!(timer(180_000.), "3m");
        assert_eq!(timer(3_840_000.), "1h 4m");
        assert_eq!(command_duration(850.), "850ms");
        assert_eq!(command_duration(2_390.), "2.3s");
        assert_eq!(command_duration(45_000.), "45s");
        assert_eq!(command_duration(65_000.), "1m 5s");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn a_patch_numbers_each_line_on_its_own_side() {
        let lines = diff_lines("--- a/x\n+++ b/x\n@@ -10,3 +10,3 @@\n keep\n-old\n+new\n tail");
        let read: Vec<(usize, Option<char>, &str)> = lines.iter().map(|l| (l.number, l.mark, l.text)).collect();
        assert_eq!(read, [(10, None, "keep"), (11, Some('-'), "old"), (11, Some('+'), "new"), (12, None, "tail")]);
    }

    #[test]
    fn a_patch_counts_its_added_and_removed_lines() {
        assert_eq!(stat("--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@\n-old\n+new\n+more\n same"), (2, 1));
    }
}
