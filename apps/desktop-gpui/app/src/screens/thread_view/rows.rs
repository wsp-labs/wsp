// SPDX-License-Identifier: AGPL-3.0-only
//! Each transcript row as the web app's timeline draws it (`timeline/rows.tsx` and its kin); every size is that class's, in px.

mod block;
mod calls;
mod insights;
mod opens;

pub use block::{bytes_of, code_surface, command_duration};
pub use insights::{insights, Segment};
pub use opens::{named_file, turn_diff, Opens};

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, AnyElement, App, Context, ElementId, Entity, FontWeight, Hsla, Pixels, Rgba, SharedString, Task, Window,
};

use super::{markdown, shimmer};
use crate::thread_view::ThreadView;
use crate::transcript::{FooterWord, Glyph as RowGlyph, Row, RowKind};
use crate::ui::{motion, ActivePalette, Glyph, Palette};
#[cfg(test)]
use block::{diff_lines, thousands};
use calls::{calls_list, work};
use insights::{fences_before, insight_note, segment_body};

/// `max-w-3xl`: the transcript column, so both apps wrap the same text at the same width.
pub const COLUMN: Pixels = px(768.);
/// `sm:px-5` inside the 6 px scrollbar gutter `scrollbar-gutter: stable both-edges` keeps on each side; `px-3` under
/// 640 px.
pub const SIDE: Pixels = px(26.);
const SIDE_NARROW: Pixels = px(18.);
/// The list's header and footer spacers, `h-3 sm:h-4`.
pub const END_SPACE: Pixels = px(16.);
/// `--chat-composer-inset` until the composer is drawn: the empty dock's height, so the last row clears the composer.
pub const COMPOSER_INSET: Pixels = px(110.);
/// `text-sm leading-relaxed`.
pub(super) const SM: (Pixels, Pixels) = (px(14.), px(22.75));
/// `max-h-72` on a command's output, and `min(18rem, 50dvh)` on an open group's list.
const OUTPUT_MAX: Pixels = px(288.);
/// A command's output folds past 14 lines to its first (or, after a failure, last) 10.
const FOLD_PAST: usize = 14;
const FOLD_TO: usize = 10;

/// The text a row's parsed document is built from.
pub fn body_text(row: &Row) -> String {
    match row.kind {
        RowKind::Person { .. } => markdown::person_text(&row.text),
        RowKind::Plan { .. } => super::cards::plan_text(row),
        RowKind::Agent { streaming: false, .. } if row.text.is_empty() => "(empty response)".to_owned(),
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
pub(super) fn icon_slot(g: Glyph, ink: Hsla) -> impl IntoElement {
    div()
        .flex_none()
        .size(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(g).with_size(px(16.)).text_color(ink.opacity(0.7)))
}

pub(super) fn glyph_slot(g: RowGlyph, ink: Hsla) -> impl IntoElement {
    icon_slot(glyph(g), ink)
}

/// `ChevronDownIcon size-3 opacity-70`, turned over when open.
fn chevron(open: bool, ink: Hsla) -> impl IntoElement {
    let g = if open { Glyph::ChevronUp } else { Glyph::ChevronDown };
    Icon::new(g).with_size(px(12.)).text_color(ink.opacity(0.7))
}

/// `focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring/70` on a row that opens: Tab reaches it, and
/// Enter or Space presses it as a click does.
fn focusable<E: StatefulInteractiveElement + Styled>(el: E, p: &Palette) -> E {
    let ring = p.ring.opacity(0.7);
    el.tab_index(0).focus_visible(move |s| s.shadow(vec![crate::ui::focus::ring_inset(ring)]))
}

/// The row's wrapper: the centred column, and the bottom padding the web's `TimelineRowContent` picks by kind.
fn frame(ix: usize, last: bool, foot: Pixels, inset: Pixels, side: Pixels, content: impl IntoElement) -> AnyElement {
    let column = div()
        .debug_selector(|| format!("row-{ix}"))
        .mx_auto()
        .w_full()
        .max_w(COLUMN)
        .when(ix == 0, |el| el.pt(END_SPACE))
        .pb(foot)
        .when(last, |el| el.pb(foot + END_SPACE + inset))
        .child(content);
    div().w_full().px(side).child(column).into_any_element()
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
        RowKind::Permission { .. }
        | RowKind::Question { .. }
        | RowKind::Plan { .. }
        | RowKind::Subagent { .. }
        | RowKind::Spawn { .. }
        | RowKind::ChangedFiles { .. }
        | RowKind::Fork { .. }
        | RowKind::StartFailed { .. }
        | RowKind::Threads { .. }
        | RowKind::Rule
        | RowKind::Slate => super::cards::foot(kind),
    }
}

pub fn row(view: &mut ThreadView, ix: usize, row: &Row, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let last = ix + 1 == view.rows().len();
    let (trailed, reply) = (trailed(view.rows(), ix), reply_of(view.rows(), ix));
    let shown = match &row.kind {
        RowKind::Person { .. } => person(view, row, &p, window, cx),
        RowKind::Agent { meta, .. } => agent(view, row, *meta && trailed.is_none(), trailed.is_some(), &p, window, cx),
        RowKind::Fold { open } => fold(row, *open, &p, cx),
        RowKind::Stopped => notice_line(&row.text, &p).into_any_element(),
        RowKind::Group { group, glyph, .. } => group_line(row, group, *glyph, &p, cx),
        RowKind::Live { group, glyph, verb, mono, since, .. } => {
            live_line(view, row, group, *glyph, verb.as_deref(), *mono, *since, &p, cx)
        }
        RowKind::Calls { calls, .. } => calls_list(view, row, calls, &p, window, cx),
        RowKind::Work { tone, glyph, body, open } => work(view, row, *tone, *glyph, body.as_deref(), *open, &p, cx),
        RowKind::Error => error_line(&row.text, &p, cx),
        RowKind::Working { since, .. } => working(view, row, *since, &p, cx),
        RowKind::Thinking if row.text.is_empty() => div().min_h(px(28.)).into_any_element(),
        RowKind::Thinking => {
            let spec = shimmer::Spec { glyph: RowGlyph::Brain, glyph_ink: p.foreground, verb: None, text: "Thinking".into(), mono: false };
            div().flex().items_start().min_h(px(28.)).child(shimmer::line(view, row, spec, cx)).into_any_element()
        }
        RowKind::Footer { word, why } => footer(*word, why.as_deref(), &p, cx),
        RowKind::Permission { .. }
        | RowKind::Question { .. }
        | RowKind::Plan { .. }
        | RowKind::Subagent { .. }
        | RowKind::Spawn { .. }
        | RowKind::ChangedFiles { .. }
        | RowKind::Fork { .. }
        | RowKind::StartFailed { .. }
        | RowKind::Threads { .. }
        | RowKind::Rule
        | RowKind::Slate => {
            let card = super::cards::card(view, ix, row, window, cx);
            match reply {
                Some(reply) => under_reply(view, reply, row, trailed.is_none(), card, &p, window, cx),
                None => card,
            }
        }
    };
    let foot = match (&row.kind, trailed, reply) {
        // `mt-4` over the files card, and none over the slate line, which keeps its own `mt-1`.
        (_, Some(Under::Files), _) => px(16.),
        (_, Some(Under::Slate), _) => px(0.),
        (_, None, Some(reply)) => foot(&view.rows()[reply].kind),
        (kind, None, None) => foot(kind),
    };
    let side = if crate::ui::narrow(window) { SIDE_NARROW } else { SIDE };
    frame(ix, last, foot, view.composer_inset(), side, shown)
}

/// A row today's `AssistantTimelineRow` draws inside the reply above it, over the reply's meta line.
#[derive(Clone, Copy, PartialEq)]
enum Under {
    /// The files its turn changed.
    Files,
    /// "Updated the slate".
    Slate,
}

/// The row after `ix`, where it stands inside the same reply as the row at `ix`.
fn trailed(rows: &[Row], ix: usize) -> Option<Under> {
    let (here, next) = (rows.get(ix)?, rows.get(ix + 1)?);
    let inside = matches!(here.kind, RowKind::Agent { .. } | RowKind::ChangedFiles { .. } | RowKind::Slate) && next.turn == here.turn;
    match next.kind {
        RowKind::ChangedFiles { .. } if inside => Some(Under::Files),
        RowKind::Slate if inside => Some(Under::Slate),
        _ => None,
    }
}

/// The reply a files card or slate line at `ix` stands under, as `trailed` reads it.
fn reply_of(rows: &[Row], ix: usize) -> Option<usize> {
    if !matches!(rows.get(ix)?.kind, RowKind::ChangedFiles { .. } | RowKind::Slate) {
        return None;
    }
    let mut at = ix;
    while at > 0 && trailed(rows, at - 1).is_some() {
        at -= 1;
        if matches!(rows[at].kind, RowKind::Agent { .. }) {
            return Some(at);
        }
    }
    None
}

/// A files card or slate line inside its reply: the pointer over it shows the reply's meta line, which the last of
/// them draws under itself, then the reply's `py-0.5`.
#[allow(clippy::too_many_arguments)]
fn under_reply(
    view: &ThreadView,
    reply: usize,
    row: &Row,
    last: bool,
    card: AnyElement,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let rows = view.rows();
    let own = message_hover(row, window, cx);
    let hovered = rows[reply..].iter().take_while(|r| r.turn == row.turn).any(|r| *message_hover(r, window, cx).read(cx));
    let meta = matches!(rows[reply].kind, RowKind::Agent { meta: true, .. }) && last;
    let thread = view.thread().unwrap_or_default().to_owned();
    // Inside the reply's `px-1`, as today's card, line and meta line are.
    v_flex()
        .id(SharedString::from(format!("under:{}", row.key)))
        .on_hover(motion::track_hover(own))
        .w_full()
        .px(px(4.))
        .child(card)
        .when(meta, |el| el.child(div().mt(px(6.)).child(meta_line(&rows[reply], &thread, false, hovered, p, window, cx))))
        .when(last, |el| el.pb(px(2.)))
        .into_any_element()
}

/// `shouldClampText`: a message past 600 characters (UTF-16 units, as JS counts) or 8 lines clamps.
pub fn clamps(text: &str) -> bool {
    !text.trim().is_empty() && (text.encode_utf16().count() > 600 || text.split('\n').count() > 8)
}

/// `max-h-44`: a clamped message's height.
const CLAMP_HEIGHT: Pixels = px(176.);

/// `CLAMP_FADE_MASK`: whole until 5rem above the bottom, gone 1.5rem above it; drawn as the bubble's own fill over the
/// text, since this base has no masks.
fn clamp_fade(fill: Hsla) -> impl IntoElement {
    let (from, to) = (CLAMP_HEIGHT - px(80.), CLAMP_HEIGHT - px(24.));
    div()
        .absolute()
        .left_0()
        .right_0()
        .top(from)
        .bottom_0()
        .child(div().h(to - from).bg(gpui_kit::linear_gradient(
            180.,
            gpui_kit::linear_color_stop(fill.opacity(0.), 0.),
            gpui_kit::linear_color_stop(fill, 1.),
        )))
        .child(div().flex_1().h(px(24.)).bg(fill))
}

/// `PERSON_BUBBLE`: right-aligned, at most 80% wide, 18 px round on `--message-surface`, then its hover meta line; a
/// long message clamps under a fade with "Show full message" (`CollapsibleUserMessageBody`).
fn person(view: &mut ThreadView, row: &Row, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let body = view.body(row, cx);
    let cwd = view.transcript().cwd().map(str::to_owned);
    let (files, full, steered) = match &row.kind {
        RowKind::Person { files, full, steered, .. } => (super::cards::files(view, row, files, p, cx), *full, *steered),
        _ => (None, false, false),
    };
    let clamped = clamps(&row.text);
    let collapsed = clamped && !full;
    let toggle = clamped.then(|| {
        let id = format!("full:{}", row.key);
        h_flex()
            .id(SharedString::from(format!("show-full:{}", row.key)))
            .debug_selector(move || if collapsed { "show-full" } else { "show-less" }.into())
            .h(px(24.))
            .px(px(6.))
            .mr(px(-4.))
            .rounded(px(8.))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .cursor_pointer()
            .hover(|s| s.bg(p.muted.opacity(0.55)).text_color(p.foreground))
            .tab_index(0)
            .focus_visible(|s| s.shadow(vec![crate::ui::focus::ring(p.ring)]))
            .child(if collapsed { "Show full message" } else { "Show less" })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle(&id, cx);
            }))
    });
    let text = div()
        .relative()
        .when(collapsed, |el| el.max_h(CLAMP_HEIGHT).overflow_hidden())
        .child(markdown::reply(&body, true, None, cwd.as_deref(), Some(p.accent), cx))
        .when(collapsed, |el| el.child(clamp_fade(p.accent)));
    let hover = message_hover(row, window, cx);
    let thread = view.thread().unwrap_or_default().to_owned();
    let meta = meta_line(row, &thread, true, *hover.read(cx), p, window, cx);
    v_flex()
        .id(SharedString::from(row.key.clone()))
        .on_hover(motion::track_hover(hover))
        .w_full()
        .items_end()
        .gap(px(4.))
        .child(
            div()
                .id(SharedString::from(format!("bubble:{}", row.key)))
                .debug_selector(move || if steered { "bubble-steered" } else { "bubble" }.into())
                .max_w(relative(0.8))
                .rounded(px(18.))
                .bg(p.accent)
                .p(px(12.))
                .text_color(p.foreground)
                .when(steered, |el| el.tooltip(crate::ui::tip("Sent while the agent was working")))
                .children(files)
                .child(div().relative().child(text).children(toggle.map(|toggle| {
                    h_flex()
                        .justify_end()
                        .map(|el| if collapsed { el.absolute().left_0().right_0().bottom_0() } else { el.mt(px(6.)) })
                        .child(toggle)
                }))),
        )
        .child(meta)
        .into_any_element()
}

/// The reply's markdown, `px-1 py-0.5`, and under the settled turn's last reply its hover-only meta line; a reply with
/// rows drawn inside it leaves its foot to the last of them.
fn agent(
    view: &mut ThreadView,
    row: &Row,
    meta: bool,
    trailed: bool,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let (runs, streaming) = match &row.kind {
        RowKind::Agent { runs, streaming, .. } => (runs.as_slice(), *streaming),
        _ => (&[][..], false),
    };
    let scope = super::run::Scope::of(view, row, cx);
    let can_start = scope.is_some();
    let on_run = super::run::on_run(scope.clone(), 0);
    let replies = || (!streaming).then(|| markdown::ReplyRuns { runs, on_run: on_run.clone(), can_start });
    let cwd = view.transcript().cwd().map(str::to_owned);
    let reply = match insights(&row.text, streaming) {
        None => div().child(markdown::reply(&view.body(row, cx), false, replies(), cwd.as_deref(), Some(p.background), cx)),
        Some(segments) => div().flex().flex_col().gap(px(10.)).children(segments.iter().enumerate().map(|(n, segment)| {
            let body = segment_body(row, n, segment.text, cwd.as_deref(), window, cx);
            // A segment numbers its shell blocks from its own start, as its ReplyRunScope's offset does.
            let start = (segment.text.as_ptr() as usize).saturating_sub(row.text.as_ptr() as usize).min(row.text.len());
            let before = fences_before(&row.text, start);
            let own: Vec<_> = runs
                .iter()
                .filter(|run| run.block >= before)
                .map(|run| crate::transcript::Run { block: run.block - before, ..run.clone() })
                .collect();
            let on_run = super::run::on_run(scope.clone(), before);
            let replies = (!streaming).then(|| markdown::ReplyRuns { runs: &own, on_run, can_start });
            let note = markdown::reply(&body, false, replies, cwd.as_deref(), Some(p.background), cx);
            if segment.insight {
                insight_note(note, p).into_any_element()
            } else {
                note.into_any_element()
            }
        })),
    };
    let hover = message_hover(row, window, cx);
    let thread = view.thread().unwrap_or_default().to_owned();
    let meta = meta.then(|| div().px(px(4.)).mt(px(6.)).child(meta_line(row, &thread, false, *hover.read(cx), p, window, cx)));
    v_flex()
        .id(SharedString::from(row.key.clone()))
        .on_hover(motion::track_hover(hover))
        .w_full()
        .child(div().px(px(4.)).pt(px(2.)).when(!trailed, |el| el.pb(px(2.))).child(reply))
        .children(meta)
        .into_any_element()
}

/// The stamp and the message's buttons, always laid out, fading in over 200 ms while the pointer is over the message or
/// focus is inside the line (`transition-opacity duration-200 focus-within:opacity-100 group-hover:opacity-100`).
fn meta_line(
    row: &Row,
    thread: &str,
    person: bool,
    hovered: bool,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> impl IntoElement {
    let at = match row.kind {
        RowKind::Person { at, .. } | RowKind::Agent { at, .. } => at,
        _ => None,
    };
    let stamp = at.map(stamp).unwrap_or_default();
    let buttons = super::rewind::buttons(row, thread, cx);
    let id = SharedString::from(format!("meta:{}", row.key));
    let focus =
        window.use_keyed_state(ElementId::from((ElementId::from(id.clone()), "focus")), cx, |_, cx| cx.focus_handle()).read(cx).clone();
    let shown = hovered || focus.contains_focused(window, cx);
    let opacity = motion::ease_px(id, "opacity", px(if shown { 1. } else { 0. }), Duration::from_millis(200), window, cx);
    let line = h_flex()
        .debug_selector(move || if shown { "meta-shown" } else { "meta" }.into())
        .track_focus(&focus)
        .h(px(24.))
        .gap(px(8.))
        .opacity(f32::from(opacity));
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

/// Whether the pointer is over a message, kept between frames for its meta line.
fn message_hover(row: &Row, window: &mut Window, cx: &mut App) -> Entity<bool> {
    motion::hovered(SharedString::from(format!("message:{}", row.key)), window, cx)
}

/// `formatChatTimestamp`: `Oct 1, 3:54 PM` in the computer's own zone, with the year when it is not this one.
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
                .map(|el| focusable(el, p))
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
fn group_line(row: &Row, group: &str, g: RowGlyph, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
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
        .hover(|s| s.bg(p.accent.opacity(0.2)))
        .map(|el| focusable(el, p))
        .text_size(SM.0)
        .line_height(SM.1)
        .child(glyph_slot(g, p.muted_foreground))
        .child(div().flex_1().min_w_0().truncate().text_color(p.muted_foreground).child(row.text.clone()))
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&id, cx)))
        .into_any_element()
}

/// `LiveActivityContent`: a glyph, the verb in the label ink and the rest after it, the mono face for a command;
/// `highlighted`, the shimmer's copy, all in the foreground's ink with the glyph at full opacity.
#[allow(clippy::too_many_arguments)]
pub(super) fn activity(
    g: RowGlyph,
    glyph_ink: Hsla,
    verb: Option<&str>,
    text: SharedString,
    mono: bool,
    highlighted: bool,
    p: &Palette,
    cx: &App,
) -> impl IntoElement {
    let glyph = if highlighted {
        div()
            .flex_none()
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .child(Icon::new(glyph(g)).with_size(px(16.)).text_color(p.foreground))
            .into_any_element()
    } else {
        glyph_slot(g, glyph_ink).into_any_element()
    };
    h_flex()
        .min_h(px(24.))
        .min_w_0()
        .py(px(2.))
        .px(px(2.))
        .gap(px(6.))
        .text_size(SM.0)
        .line_height(SM.1)
        .text_color(if highlighted { p.foreground } else { p.muted_foreground })
        .child(glyph)
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
        .map(|el| focusable(el, p))
        .gap(px(12.))
        .map(|el| {
            if matches!(row.kind, RowKind::Live { active: true, .. }) {
                let spec = shimmer::Spec {
                    glyph: g,
                    glyph_ink: p.muted_foreground,
                    verb: verb.map(str::to_owned),
                    text: row.text.clone().into(),
                    mono,
                };
                el.child(shimmer::line(view, row, spec, cx))
            } else {
                el.child(activity(g, p.muted_foreground, verb, row.text.clone().into(), mono, false, p, cx))
            }
        })
        .children(
            timer.map(|_| shimmer::clock_slot(view, row, shimmer::ClockLook { size: px(12.), line_height: None, height: px(24.) }, cx)),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&id, cx)))
        .into_any_element()
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

/// `Working for 12s` at the start of the running turn, or `Waiting for you 12s` while a prompt of it is open; while the
/// agent makes its model call again, the retry's rule line and its clock in that place (`WorkingTimelineRow`).
fn working(view: &mut ThreadView, row: &Row, since: Option<f64>, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let (waiting, retry) = match &row.kind {
        RowKind::Working { wait: Some(wait), .. } => {
            return super::cards::machine_wait(view, row, &wait.clone(), since, "machine-wait", cx)
        }
        RowKind::Working { waiting, retry, .. } => (*waiting, retry.clone()),
        _ => (false, None),
    };
    if let Some(retry) = retry {
        // `machineWait` for a model call made again: the rule line with its count, no act.
        let wait = crate::transcript::Wait { label: retry, wake: false, elapsed: true };
        return super::cards::machine_wait(view, row, &wait, since, "retry", cx);
    }
    let line = h_flex().h(px(24.)).px(px(4.)).text_size(SM.0).line_height(SM.1).text_color(p.muted_foreground);
    let line = match since {
        Some(since) => {
            view.clock(row, since, cx);
            let slot = shimmer::clock_slot(view, row, shimmer::ClockLook { size: SM.0, line_height: Some(SM.1), height: px(24.) }, cx);
            if waiting {
                line.child(format!("{} ", row.text)).child(div().flex_none().ml(px(8.)).child(slot))
            } else {
                line.child("Working for ").child(slot)
            }
        }
        None if waiting => line.child(row.text.clone()),
        None => line.child("Working..."),
    };
    div().pt(px(4.)).pb(px(8.)).child(line).into_any_element()
}

/// The settled footer's one mono word, and for a stop that never reached the computer why, 2 px under it in 12 px
/// muted (`SettledFooter`).
fn footer(word: FooterWord, why: Option<&str>, p: &Palette, cx: &App) -> AnyElement {
    let (text, ink) = match word {
        FooterWord::Interrupted => ("interrupted", p.destructive),
        FooterWord::Failed => ("failed", p.destructive),
        FooterWord::LimitReached => ("limit reached", p.muted_foreground),
    };
    div()
        .when(why.is_some(), |el| el.debug_selector(|| "settled-why".into()))
        .w_full()
        .px(px(4.))
        .pb(px(8.))
        .child(div().font_family(cx.theme().mono_font_family.clone()).text_size(px(11.)).line_height(px(16.5)).text_color(ink).child(text))
        .children(
            why.map(|why| div().mt(px(2.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(why.to_owned())),
        )
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
mod tests;
