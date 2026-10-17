// SPDX-License-Identifier: AGPL-3.0-only
//! Each transcript row as the web app's timeline draws it (`timeline/rows.tsx` and its kin); every size is that class's, in px.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::base::text::{TextView, TextViewState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, AnyElement, App, Context, ElementId, Entity, FontWeight, HighlightStyle, Hsla, Pixels, Rgba,
    ScrollHandle, SharedString, Task, Window,
};

use super::{markdown, shimmer};
use crate::thread_view::ThreadView;
use crate::transcript::{Call, FilePatch, FooterWord, Glyph as RowGlyph, Row, RowKind, Tone};
use crate::ui::{motion, ActivePalette, Glyph, Palette};

/// `max-w-3xl`: the transcript column, so both apps wrap the same text at the same width.
pub const COLUMN: Pixels = px(768.);
/// `sm:px-5` inside the 6 px scrollbar gutter `scrollbar-gutter: stable both-edges` keeps on each side.
pub const SIDE: Pixels = px(26.);
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
fn frame(ix: usize, last: bool, foot: Pixels, inset: Pixels, content: impl IntoElement) -> AnyElement {
    let column = div()
        .debug_selector(|| format!("row-{ix}"))
        .mx_auto()
        .w_full()
        .max_w(COLUMN)
        .when(ix == 0, |el| el.pt(END_SPACE))
        .pb(foot)
        .when(last, |el| el.pb(foot + END_SPACE + inset))
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
    let shown = match &row.kind {
        RowKind::Person { .. } => person(view, row, &p, window, cx),
        RowKind::Agent { meta, .. } => agent(view, row, *meta, &p, window, cx),
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
        RowKind::Footer { word } => footer(*word, &p, cx),
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
        | RowKind::Slate => super::cards::card(view, ix, row, window, cx),
    };
    frame(ix, last, foot(&row.kind), view.composer_inset(), shown)
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
    let (files, full) = match &row.kind {
        RowKind::Person { files, full, .. } => (super::cards::files(view, row, files, p, cx), *full),
        _ => (None, false),
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
        .child(markdown::reply(&body, true, None, cwd.as_deref(), cx))
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
        .child(div().max_w(relative(0.8)).rounded(px(18.)).bg(p.accent).p(px(12.)).text_color(p.foreground).children(files).child(
            div().relative().child(text).children(toggle.map(|toggle| {
                h_flex()
                    .justify_end()
                    .map(|el| if collapsed { el.absolute().left_0().right_0().bottom_0() } else { el.mt(px(6.)) })
                    .child(toggle)
            })),
        ))
        .child(meta)
        .into_any_element()
}

/// A reply cut where Claude Code's Explanatory and Learning output styles wrote an insight block (`splitInsights` in
/// components/chat/insight.ts): the marker lines go and the block between them is a note.
#[derive(Debug, PartialEq)]
pub struct Segment<'a> {
    pub insight: bool,
    pub text: &'a str,
}

fn opener(line: &str) -> bool {
    marker(line, true)
}

fn closer(line: &str) -> bool {
    marker(line, false)
}

/// `^ {0,3}(`?)[★✶]\s?Insights?[ \t]+─{3,}\1[ \t]*$` for an opener, `^ {0,3}(`?)─{3,}\1[ \t]*$` for a closer.
fn marker(line: &str, open: bool) -> bool {
    let rest = line.trim_end_matches([' ', '\t']);
    let indent = rest.len() - rest.trim_start_matches(' ').len();
    if indent > 3 {
        return false;
    }
    let rest = &rest[indent..];
    let (tick, rest) = match rest.strip_prefix('`') {
        Some(inner) => match inner.strip_suffix('`') {
            Some(inner) => (true, inner),
            None => return false,
        },
        None => (false, rest),
    };
    if !tick && rest.ends_with('`') {
        return false;
    }
    let rest = if open {
        let Some(rest) = rest.strip_prefix(['★', '✶']) else { return false };
        let rest = rest.strip_prefix(char::is_whitespace).unwrap_or(rest);
        let Some(rest) = rest.strip_prefix("Insight") else { return false };
        let rest = rest.strip_prefix('s').unwrap_or(rest);
        let gap = rest.len() - rest.trim_start_matches([' ', '\t']).len();
        if gap == 0 {
            return false;
        }
        &rest[gap..]
    } else {
        rest
    };
    rest.chars().count() >= 3 && rest.chars().all(|c| c == '─')
}

/// Each line inside a fenced code block, fence lines included, by CommonMark's rules (`fencedLines`).
fn fenced(lines: &[&str]) -> Vec<bool> {
    let mut open: Option<(char, usize)> = None;
    lines
        .iter()
        .map(|line| {
            let trimmed = line.trim_start_matches(' ');
            let fence = (line.len() - trimmed.len() <= 3)
                .then(|| trimmed.chars().next().filter(|c| *c == '`' || *c == '~'))
                .flatten()
                .map(|c| (c, trimmed.chars().take_while(|&d| d == c).count()))
                .filter(|(_, n)| *n >= 3);
            match (open, fence) {
                (None, Some((c, n))) if !(c == '`' && trimmed[n..].contains('`')) => {
                    open = Some((c, n));
                    true
                }
                (None, _) => false,
                (Some((c, n)), Some((d, m))) if c == d && m >= n && trimmed[m..].trim().is_empty() => {
                    open = None;
                    true
                }
                (Some(_), _) => true,
            }
        })
        .collect()
}

/// `splitInsights`: None for a reply with no insight block, so it stays one document.
pub fn insights(text: &str, streaming: bool) -> Option<Vec<Segment<'_>>> {
    if !text.contains("Insight") {
        return None;
    }
    let raw: Vec<&str> = text.split('\n').collect();
    let lines: Vec<&str> = raw.iter().map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let mut starts = Vec::with_capacity(raw.len());
    let mut at = 0;
    for line in &raw {
        starts.push(at);
        at += line.len() + 1;
    }
    let fenced = fenced(&lines);
    let mut segments = Vec::new();
    let mut push = |insight: bool, mut from: usize, mut to: isize| {
        while (from as isize) <= to && lines[from].trim().is_empty() {
            from += 1;
        }
        while to >= from as isize && lines[to as usize].trim().is_empty() {
            to -= 1;
        }
        if (from as isize) > to {
            if insight {
                let at = starts.get(from).copied().unwrap_or(text.len()).min(text.len());
                segments.push(Segment { insight, text: &text[at..at] });
            }
            return;
        }
        let end = starts[to as usize] + lines[to as usize].len();
        segments.push(Segment { insight, text: &text[starts[from]..end] });
    };
    let (mut start, mut i) = (0, 0);
    while i < lines.len() {
        if fenced[i] || !opener(lines[i]) {
            i += 1;
            continue;
        }
        let mut close = i + 1;
        while close < lines.len() && (fenced[close] || !closer(lines[close])) {
            close += 1;
        }
        if close == lines.len() && !streaming {
            break;
        }
        push(false, start, i as isize - 1);
        push(true, i + 1, close as isize - 1);
        start = close + 1;
        i = close + 1;
    }
    if start == 0 {
        return None;
    }
    if start < lines.len() {
        push(false, start, lines.len() as isize - 1);
    }
    Some(segments)
}

/// How many fenced blocks open before byte `end` of a reply, by `block_at`'s rules.
fn fences_before(text: &str, end: usize) -> usize {
    let mut open: Option<(char, usize)> = None;
    let mut count = 0;
    for line in text[..end].split('\n') {
        let bare = line.trim_start_matches([' ', '>', '\t']);
        let fence = bare.chars().next().filter(|c| *c == '`' || *c == '~').map(|c| (c, bare.chars().take_while(|d| *d == c).count()));
        match (open, fence) {
            (Some((c, n)), Some((d, m))) if c == d && m >= n && bare.trim_end().chars().all(|e| e == c) => open = None,
            (Some(_), _) => {}
            (None, Some((c, n))) if n >= 3 && !(c == '`' && bare[n..].contains('`')) => {
                count += 1;
                open = Some((c, n));
            }
            (None, _) => {}
        }
    }
    count
}

/// A reply segment's own document, kept with the row while it is drawn and given the segment's text as it grows.
fn segment_body(row: &Row, n: usize, text: &str, cwd: Option<&str>, window: &mut Window, cx: &mut App) -> Entity<TextViewState> {
    let id = SharedString::from(format!("segment:{}:{n}", row.key));
    let body = window.use_keyed_state(id, cx, |_, cx| TextViewState::markdown_with_extensions(text, markdown::extensions(cwd, cx), cx));
    if body.read(cx).rendered_text().source() != text {
        body.update(cx, |body, cx| body.set_text(text, cx));
    }
    body
}

/// `InsightNote`: a 2 px rule in the border ink, "Insight" in the group label's 12 px over the note.
fn insight_note(note: TextView, p: &Palette) -> impl IntoElement {
    div()
        .debug_selector(|| "insight".into())
        .border_l_2()
        .border_color(p.border)
        .pl(px(12.))
        .child(div().mb(px(4.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child("Insight"))
        .child(note)
}

/// The reply's markdown, `px-1 py-0.5`, and under the settled turn's last reply its hover-only meta line.
fn agent(view: &mut ThreadView, row: &Row, meta: bool, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let (runs, streaming) = match &row.kind {
        RowKind::Agent { runs, streaming, .. } => (runs.as_slice(), *streaming),
        _ => (&[][..], false),
    };
    let weak = cx.entity().downgrade();
    let on_run: markdown::OnRun = Arc::new(move |action, _, cx| {
        if let markdown::RunAction::Send { message } = action {
            _ = weak.update(cx, |view, cx| view.send(message, cx));
        }
    });
    // A run starts in a workspace pty over the daemon's channel, which the terminal screen brings.
    let replies = || (!streaming).then(|| markdown::ReplyRuns { runs, on_run: on_run.clone(), can_start: false });
    let cwd = view.transcript().cwd().map(str::to_owned);
    let reply = match insights(&row.text, streaming) {
        None => div().child(markdown::reply(&view.body(row, cx), false, replies(), cwd.as_deref(), cx)),
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
            let replies = (!streaming).then(|| markdown::ReplyRuns { runs: &own, on_run: on_run.clone(), can_start: false });
            let note = markdown::reply(&body, false, replies, cwd.as_deref(), cx);
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
        .child(div().px(px(4.)).py(px(2.)).child(reply))
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

/// A closed call row's height, which an open one grows the group's cap by the rest of (`CLOSED_ROW_PX`).
const CLOSED_ROW: Pixels = px(30.);
/// `--fade-size`: the open group's edge fades while it scrolls.
const EDGE_FADE: Pixels = px(24.);

/// An open group's calls, `ms-3`, capped at `min(18rem, 50dvh)` plus what its open calls grew by, so a call opened
/// grows the list rather than scrolling inside it, and fading at an edge it scrolls past (`ExpandedWorkGroupEntries`).
fn calls_list(
    view: &mut ThreadView,
    row: &Row,
    calls: &[Call],
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let id = SharedString::from(format!("calls:{}", calls.first().map_or("", |c| c.key.as_str())));
    let scroll =
        window.use_keyed_state(ElementId::from((ElementId::from(id.clone()), "scroll")), cx, |_, _| ScrollHandle::new()).read(cx).clone();
    let grown =
        window.use_keyed_state(ElementId::from((ElementId::from(id.clone()), "grown")), cx, |_, _| Heights::default()).read(cx).clone();
    let open: Vec<&str> = calls.iter().filter(|c| c.open).map(|c| c.key.as_str()).collect();
    grown.borrow_mut().retain(|key, _| open.contains(&key.as_str()));
    let growth = grown.borrow().values().map(|h| (*h - CLOSED_ROW).max(px(0.))).fold(px(0.), |a, b| a + b);
    let cap = OUTPUT_MAX.min(window.viewport_size().height / 2.) + growth;
    let (offset, most) = (-scroll.offset().y, scroll.max_offset().y);
    let (top, bottom) = (offset > px(1.), most - offset > px(1.));
    let ground = p.background;
    let fade = |down: bool| {
        let (from, to) = if down { (ground, ground.opacity(0.)) } else { (ground.opacity(0.), ground) };
        div().absolute().left_0().right_0().h(EDGE_FADE).bg(gpui_kit::linear_gradient(
            180.,
            gpui_kit::linear_color_stop(from, 0.),
            gpui_kit::linear_color_stop(to, 1.),
        ))
    };
    let shown: Vec<AnyElement> = calls.iter().map(|call| call_row(view, row, call, &grown, p, window, cx)).collect();
    // The list's extent is known only once it is laid out, so a frame that drew the wrong fades draws again.
    let (handle, me) = (scroll.clone(), cx.weak_entity());
    let recheck = gpui_kit::canvas(
        move |_, _, cx| {
            let (offset, most) = (-handle.offset().y, handle.max_offset().y);
            if (offset > px(1.), most - offset > px(1.)) != (top, bottom) {
                redraw(me.clone(), cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_0();
    div()
        .relative()
        .ml(px(12.))
        .child(
            v_flex()
                .id(id)
                .debug_selector(|| "calls".into())
                .rounded(px(8.))
                .max_h(cap)
                .overflow_y_scroll()
                .track_scroll(&scroll)
                .children(shown.into_iter().map(|call| div().flex_none().child(call))),
        )
        .child(recheck)
        .when(top, |el| el.child(fade(true).debug_selector(|| "calls-fade-top".into()).top_0()))
        .when(bottom, |el| el.child(fade(false).debug_selector(|| "calls-fade-bottom".into()).bottom_0()))
        .into_any_element()
}

/// Draws the rows again once this frame ends: what a frame measured while it drew changes how they lay out.
fn redraw(view: gpui_kit::WeakEntity<ThreadView>, cx: &mut App) {
    cx.defer(move |cx| _ = view.update(cx, |_, cx| cx.notify()));
}

/// Each open call's height as it last drew, by its key.
type Heights = Rc<RefCell<HashMap<String, Pixels>>>;

/// A canvas filling an open call that keeps its height, and draws the rows again when it moved.
fn measure(key: &str, grown: &Heights, cx: &mut Context<ThreadView>) -> impl IntoElement {
    let (key, grown, me) = (key.to_owned(), grown.clone(), cx.weak_entity());
    gpui_kit::canvas(
        move |bounds, _, cx| {
            let was = grown.borrow_mut().insert(key.clone(), bounds.size.height);
            if was != Some(bounds.size.height) {
                redraw(me.clone(), cx);
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

fn call_row(
    view: &mut ThreadView,
    row: &Row,
    call: &Call,
    grown: &Heights,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let shown = if call.command.is_some() || call.patch.is_some() {
        block(view, row, call, p, window, cx)
    } else {
        let opens = call.body.as_ref().is_some_and(|b| !b.trim().is_empty());
        let key = call.key.clone();
        let label = label_text(call.verb.as_deref(), &call.label);
        let marks = super::find::marks(view, &row.key, &format!("{}:label", call.key), &label, cx);
        let line = plain_line(&call.label, call.verb.as_deref(), call.mono, call.tone, call.glyph, opens, Some(call.open), &marks, p, cx);
        let body = call.body.as_deref().filter(|_| call.open).map(|b| {
            let marks = super::find::marks(view, &row.key, &format!("{}:body", call.key), b, cx);
            detail(&call.key, b, marks, p, cx)
        });
        v_flex()
            .id(SharedString::from(call.key.clone()))
            .px(px(2.))
            .rounded(px(8.))
            .when(opens, |el| {
                focusable(el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.2))), p)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
            })
            .child(line)
            .children(body)
            .into_any_element()
    };
    if !call.open {
        return shown;
    }
    div().relative().child(shown).child(measure(&call.key, grown, cx)).into_any_element()
}

/// A work row standing on its own (`PlainWorkEntryRow`, `py-0.5`): it opens to its body as a call in a group does.
#[allow(clippy::too_many_arguments)]
fn work(
    view: &ThreadView,
    row: &Row,
    tone: Tone,
    g: RowGlyph,
    body: Option<&str>,
    open: bool,
    p: &Palette,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let opens = body.is_some_and(|b| !b.trim().is_empty());
    let marks = super::find::marks(view, &row.key, "label", &row.text, cx);
    let body = body.filter(|_| open).map(|b| {
        let marks = super::find::marks(view, &row.key, "body", b, cx);
        detail(&row.key, b, marks, p, cx)
    });
    let key = row.key.clone();
    v_flex()
        .id(SharedString::from(format!("work:{}", row.key)))
        .debug_selector(|| "work".into())
        .py(px(2.))
        .px(px(2.))
        .rounded(px(8.))
        .when(opens, |el| {
            focusable(el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.2))), p)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
        })
        .child(plain_line(&row.text, None, false, tone, g, opens, Some(open), &marks, p, cx))
        .children(body)
        .into_any_element()
}

/// Highlights over a row's plain text, by byte range.
type Marks = Vec<(Range<usize>, HighlightStyle)>;

/// `base` with `over` laid on it, cut wherever either changes so no two ranges overlap, as StyledText needs.
fn layered(base: &[(Range<usize>, HighlightStyle)], over: &[(Range<usize>, HighlightStyle)]) -> Marks {
    if over.is_empty() {
        return base.to_vec();
    }
    let mut cuts: Vec<usize> = base.iter().chain(over).flat_map(|(r, _)| [r.start, r.end]).collect();
    cuts.sort_unstable();
    cuts.dedup();
    let at = |list: &[(Range<usize>, HighlightStyle)], x: usize| list.iter().find(|(r, _)| r.contains(&x)).map(|(_, s)| *s);
    cuts.windows(2)
        .filter_map(|w| {
            let style = match (at(base, w[0]), at(over, w[0])) {
                (Some(b), Some(o)) => b.highlight(o),
                (b, o) => b.or(o)?,
            };
            Some((w[0]..w[1], style))
        })
        .collect()
}

/// The marks of `marks` that fall in `from..from + len`, moved to start at 0.
fn shifted(marks: &[(Range<usize>, HighlightStyle)], from: usize, len: usize) -> Marks {
    let end = from + len;
    marks
        .iter()
        .filter(|(r, _)| r.start < end && r.end > from)
        .map(|(r, s)| (r.start.max(from) - from..r.end.min(end) - from, *s))
        .collect()
}

/// Text with Find's marks painted on it; `marks` covers `"{verb} {text}"` when there is a verb.
fn marked(text: &str, at: usize, marks: &[(Range<usize>, HighlightStyle)]) -> gpui_kit::StyledText {
    gpui_kit::StyledText::new(text.to_owned()).with_highlights(shifted(marks, at, text.len()))
}

/// The label a call's or a work row's find part reads: the verb, a space, the label.
fn label_text(verb: Option<&str>, text: &str) -> String {
    verb.map_or_else(|| text.to_owned(), |verb| format!("{verb} {text}"))
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
    marks: &[(Range<usize>, HighlightStyle)],
    p: &Palette,
    cx: &App,
) -> impl IntoElement {
    let after = verb.map_or(0, |v| v.len() + 1);
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
                .children(verb.map(|v| div().flex_none().child(marked(v, 0, marks))))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
                        .child(marked(text, after, marks)),
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
fn detail(key: &str, text: &str, marks: Marks, p: &Palette, cx: &App) -> impl IntoElement {
    let text = gpui_kit::StyledText::new(text.to_owned()).with_highlights(marks);
    div().mt(px(4.)).ml(px(28.)).border_l_1().border_color(p.border.opacity(0.45)).pl(px(12.)).pt(px(2.)).child(
        div()
            .id(SharedString::from(format!("detail:{key}")))
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
        (gpui_kit::transparent_black(), p.input.opacity(0.32))
    } else {
        (p.border.opacity(0.7), p.muted)
    }
}

/// `commandText`: a command as its open heading draws it, less blank lines before it and space after it.
fn command_text(command: &str) -> &str {
    let lead = command.len() - command.trim_start().len();
    let start = command[..lead].rfind('\n').map_or(0, |at| at + 1);
    command[start..].trim_end()
}

/// `DiffStatLabel`, inline: the added count in the success ink and the removed in the error ink, mono at 12 px.
fn stat_label(added: usize, removed: usize, p: &Palette, cx: &App) -> AnyElement {
    h_flex()
        .gap(px(4.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(12.))
        .line_height(px(16.))
        .child(div().text_color(p.success).child(format!("+{added}")))
        .child(div().text_color(p.error_foreground).child(format!("-{removed}")))
        .into_any_element()
}

/// `FileFacts`: a file written whole counts its lines, any other its added and removed lines.
fn file_facts(file: &FilePatch, p: &Palette, cx: &App) -> AnyElement {
    if file.created {
        div().child(format!("{} lines", thousands(file.added))).into_any_element()
    } else {
        stat_label(file.added, file.removed, p, cx)
    }
}

/// `OpenDiff`: the ghost `icon-xs` FileDiff, opening the file at its first hunk in Files (the turn's range in Changes
/// is the Changes pane's, which the frame does not have yet).
fn open_diff(file: &FilePatch, cx: &App) -> impl IntoElement {
    let path = file.path.clone();
    markdown::ghost_button("open-diff", Glyph::FileDiff, false, cx).debug_selector(|| "open-diff".into()).on_click(move |_, _, cx| {
        cx.stop_propagation();
        markdown::open_file(&path, cx);
    })
}

/// A shell or edit call (`CallRow`): a 28 px line in a 10 px round block that takes the code block surface when open;
/// a running command turns its spinner and counts its time, and opens only to show a command of more than one line.
fn block(view: &mut ThreadView, row: &Row, call: &Call, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let (edge, fill) = code_surface(p, cx);
    let key = call.key.clone();
    let mono = cx.theme().mono_font_family.clone();
    let running = call.command.is_some() && call.running;
    let command = call.command.as_deref().map(command_text);
    let can_open = !running || command.is_some_and(|c| c.contains('\n'));
    let open = call.open && can_open;
    let one = (call.files.len() == 1).then(|| &call.files[0]);
    let (g, verb, target) = match command {
        Some(command) => {
            let line = if open { command.to_owned() } else { command.lines().next().unwrap_or_default().to_owned() };
            (if call.failed && call.exit_code.is_none() { RowGlyph::CircleAlert } else { RowGlyph::Terminal }, None, line)
        }
        None => (call.glyph, call.verb.clone(), call.label.clone()),
    };
    let mut facts: Vec<AnyElement> = Vec::new();
    match (one, call.patch.is_some()) {
        (Some(file), _) => facts.push(file_facts(file, p, cx)),
        (None, true) => {
            let (added, removed) = call.files.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
            facts.push(stat_label(added, removed, p, cx));
        }
        _ => {}
    }
    if running {
        if let Some(since) = call.since {
            view.clock(row, since, cx);
            let look = shimmer::ClockLook { size: px(12.), line_height: None, height: px(24.) };
            facts.push(shimmer::clock_slot(view, row, look, cx));
        }
    } else {
        if let Some(code) = call.exit_code.filter(|c| *c != 0) {
            facts.push(div().text_color(p.error_foreground).child(format!("Exited {code}")).into_any_element());
        }
        if let Some(ms) = call.duration_ms {
            facts.push(div().child(command_duration(ms)).into_any_element());
        }
    }
    // The web draws a command in Shiki's bash colors, the same pierre theme as the code blocks.
    let colors = if command.is_some() { markdown::highlight_code("bash", &target, cx.theme().is_dark()) } else { Vec::new() };
    let (marks, after) = match command {
        Some(_) => (super::find::marks(view, &row.key, &format!("{}:command", call.key), &target, cx), 0),
        None => {
            let label = label_text(verb.as_deref(), &target);
            (super::find::marks(view, &row.key, &format!("{}:label", call.key), &label, cx), verb.as_ref().map_or(0, |v| v.len() + 1))
        }
    };
    let verb_marks = shifted(&marks, 0, verb.as_ref().map_or(0, |v| v.len()));
    let painted = !marks.is_empty();
    let colors = layered(&colors, &shifted(&marks, after, target.len()));
    let heading = h_flex()
        .flex_1()
        .min_w_0()
        .gap(px(4.))
        .text_size(SM.0)
        .line_height(px(24.))
        .text_color(p.muted_foreground)
        .children(verb.map(|v| div().flex_none().child(gpui_kit::StyledText::new(v).with_highlights(verb_marks))))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .when(command.is_some() || one.is_some(), |el| el.font_family(mono.clone()))
                .when(call.patch.is_some(), |el| el.text_color(p.foreground))
                .when(painted, |el| el.debug_selector(|| "find-marked".into()))
                .when(!open, |el| el.truncate())
                .child(gpui_kit::StyledText::new(target.clone()).with_highlights(colors)),
        );
    let actions = open.then(|| match (command, one) {
        (Some(command), _) => Some(copy_command(&call.key, command, window, cx).into_any_element()),
        (None, Some(file)) => Some(open_diff(file, cx).into_any_element()),
        _ => None,
    });
    let glyph = if running {
        div().flex_none().size(px(24.)).flex().items_center().justify_center().child(crate::ui::status::spinner(cx)).into_any_element()
    } else {
        glyph_slot(g, p.muted_foreground).into_any_element()
    };
    let line = h_flex()
        .id(SharedString::from(call.key.clone()))
        .debug_selector(|| "call-line".into())
        .items_start()
        .min_h(px(28.))
        .p(px(2.))
        .gap(px(6.))
        .child(glyph)
        .child(heading)
        .child(h_flex().h(px(24.)).flex_none().gap(px(12.)).text_size(px(12.)).text_color(p.muted_foreground).children(facts))
        .children(actions.flatten().map(|a| div().flex_none().h(px(24.)).flex().items_center().child(a)))
        .child(
            div()
                .flex_none()
                .h(px(24.))
                .w(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .when(!can_open, |el| el.invisible())
                .child(chevron(open, p.muted_foreground)),
        )
        .when(can_open, |el| {
            focusable(el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.2))), p)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
        });
    let body = (open && !running).then(|| match (&call.patch, &call.body) {
        (Some(_), _) => edit_body(call, p, window, cx),
        (None, Some(output)) if !output.trim().is_empty() => {
            let part = format!("{}:body", call.key);
            let mark = |shown: &str, cx: &App| super::find::marks(view, &row.key, &part, shown, cx);
            output_band(&call.key, output, call.exit_code.unwrap_or(0) != 0, call.bytes, &mark, p, window, cx)
        }
        _ => run_footer(div().debug_selector(|| "no-output".into()).child("No output"), None, p).into_any_element(),
    });
    v_flex()
        .rounded(px(10.))
        .border_1()
        .border_color(if open { edge } else { gpui_kit::transparent_black() })
        .when(open, |el| el.bg(fill))
        .overflow_hidden()
        .child(line)
        .children(body)
        .into_any_element()
}

/// Copy command: the whole command to the clipboard, and a check in place of the copy glyph for 1.5 s.
fn copy_command(key: &str, command: &str, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let copied = window.use_keyed_state(SharedString::from(format!("copied:{key}")), cx, |_, _| false);
    let done = *copied.read(cx);
    let command = command.to_owned();
    markdown::ghost_button("copy-command", if done { Glyph::Check } else { Glyph::Copy }, false, cx).on_click(move |_, _, cx| {
        cx.stop_propagation();
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(command.clone()));
        copied.update(cx, |copied, cx| {
            *copied = true;
            cx.notify();
            cx.spawn(async move |copied, cx| {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                _ = copied.update(cx, |copied, cx| {
                    *copied = false;
                    cx.notify();
                });
            })
            .detach();
        });
    })
}

/// An open edit's hunks (`EditCallRow`): one file's straight under the line; several, each under a head with its path,
/// its facts and Open diff, 8 px apart; then "The transcript kept part of this change" where the host cut the patch.
fn edit_body(call: &Call, p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let many = call.files.len() > 1;
    let files = call.files.iter().enumerate().map(|(at, file)| {
        let head = many.then(|| {
            h_flex()
                .debug_selector(|| "file-head".into())
                .min_h(px(32.))
                .py(px(4.))
                .pr(px(6.))
                .pl(px(12.))
                .gap(px(12.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(p.foreground)
                        .child(file.path.clone()),
                )
                .child(file_facts(file, p, cx))
                .child(open_diff(file, cx))
        });
        v_flex().id(SharedString::from(format!("file:{}:{at}", call.key))).children(head).child(diff(&call.key, at, file, p, window, cx))
    });
    v_flex()
        .child(v_flex().gap(px(8.)).children(files))
        .when(call.cut, |el| {
            el.child(run_footer(div().debug_selector(|| "edit-cut".into()).child("The transcript kept part of this change"), None, p))
        })
        .into_any_element()
}

/// `commandDuration` in `timeline/callFacts.ts`: at least 1 ms, tenths floored under 10 s, whole seconds floored.
pub fn command_duration(ms: f64) -> String {
    if ms < 1_000. {
        return format!("{}ms", ms.round().max(1.) as u64);
    }
    if ms < 10_000. {
        return format!("{:.1}s", (ms / 100.).floor() / 10.);
    }
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

/// A finished command's output (`OutputBand`): past 14 lines its first 10, or after a failure its last 10, until
/// "+N lines" shows it whole and "Fewer lines" folds it again; "First 12 KB of 40 KB" where the host kept part of it.
#[allow(clippy::too_many_arguments)]
fn output_band(
    key: &str,
    output: &str,
    failed: bool,
    bytes: Option<u64>,
    mark: &dyn Fn(&str, &App) -> Marks,
    p: &Palette,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let text = output.trim_end().trim_start_matches('\n');
    let lines: Vec<&str> = text.lines().collect();
    let hidden = if lines.len() > FOLD_PAST { lines.len() - FOLD_TO } else { 0 };
    let all = window.use_keyed_state(SharedString::from(format!("output-all:{key}")), cx, |_, _| false);
    let whole = *all.read(cx);
    let shown = match (hidden > 0 && !whole, failed) {
        (false, _) => text.to_owned(),
        (true, true) => lines[lines.len() - FOLD_TO..].join("\n"),
        (true, false) => lines[..FOLD_TO].join("\n"),
    };
    let more = (hidden > 0).then(|| {
        let n = thousands(hidden);
        let word = match (whole, failed) {
            (true, _) => "Fewer lines".to_owned(),
            (false, true) => format!("+{n} earlier lines"),
            (false, false) => format!("+{n} lines"),
        };
        div().debug_selector(|| "output-more".into()).child(crate::ui::Button::text(format!("output-more:{key}"), word).xs().on_click(
            move |_, _, cx| {
                cx.stop_propagation();
                all.update(cx, |all, cx| {
                    *all = !*all;
                    cx.notify();
                });
            },
        ))
    });
    v_flex()
        .child(
            div()
                .id(SharedString::from(format!("output:{key}")))
                .debug_selector(|| "output".into())
                .max_h(OUTPUT_MAX)
                .overflow_y_scroll()
                .bg(p.background)
                .px(px(12.))
                .py(px(8.))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(p.foreground)
                .child({
                    let marks = mark(&shown, cx);
                    gpui_kit::StyledText::new(shown).with_highlights(marks)
                }),
        )
        .child(run_footer(
            h_flex().gap(px(12.)).children(more).children(bytes.map(|total| {
                div().debug_selector(|| "output-kept".into()).child(format!("First {}", bytes_of(output.len() as u64, total)))
            })),
            Some(copy_output(key, output, window, cx)),
            p,
        ))
        .into_any_element()
}

/// `fmtBytesOf`: two sizes against each other, the first without the unit it shares with the total.
pub fn bytes_of(used: u64, total: u64) -> String {
    let (u, t) = (super::cards::bytes(used), super::cards::bytes(total));
    let unit = &t[t.rfind(' ').unwrap_or(0)..];
    format!("{} of {t}", u.strip_suffix(unit).unwrap_or(&u))
}

/// Copy output: the whole output to the clipboard, and a check in place of the copy glyph for 1.5 s.
fn copy_output(key: &str, output: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    let copied = window.use_keyed_state(SharedString::from(format!("copied-output:{key}")), cx, |_, _| false);
    let done = *copied.read(cx);
    let output = output.to_owned();
    let button = crate::ui::Button::text(format!("copy-output:{key}"), "Copy output")
        .xs()
        .glyph(if done { Glyph::Check } else { Glyph::Copy })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(output.clone()));
            copied.update(cx, |copied, cx| {
                *copied = true;
                cx.notify();
                cx.spawn(async move |copied, cx| {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    _ = copied.update(cx, |copied, cx| {
                        *copied = false;
                        cx.notify();
                    });
                })
                .detach();
            });
        });
    div().debug_selector(move || if done { "output-copied" } else { "copy-output" }.into()).child(button).into_any_element()
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
fn run_footer(left: impl IntoElement, act: Option<AnyElement>, p: &Palette) -> impl IntoElement {
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
        .children(act)
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

/// A unified patch's lines numbered on their own side; hunk headers set the numbers and draw nothing.
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

/// A file's patch in an open edit block (`@pierre/diffs`): a bar and tint per change, syntax colors, 13/20 px mono,
/// folded past 20 lines to its first 16 until "+N lines" shows it whole and "Fewer lines" folds it again (`HunksBand`).
fn diff(key: &str, at: usize, file: &FilePatch, p: &Palette, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let dark = cx.theme().is_dark();
    let ground = mix(p.card, p.background, 0.9);
    let (add, del) = (mix(p.success, ground, 0.06), mix(p.error, ground, 0.06));
    let lines = diff_lines(&file.text);
    let hidden = if lines.len() > DIFF_FOLD_PAST { lines.len() - DIFF_FOLD_TO } else { 0 };
    let all = window.use_keyed_state(SharedString::from(format!("diff-all:{key}:{at}")), cx, |_, _| false);
    let whole = *all.read(cx);
    let shown = if whole { &lines[..] } else { &lines[..lines.len() - hidden] };
    let code: String = shown.iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    let path = file.path.as_str();
    let lang = path.rsplit('.').next().filter(|ext| *ext != path).unwrap_or("");
    let styles = if lang.is_empty() { Vec::new() } else { markdown::highlight_code(lang, &code, dark) };
    let digits = shown.iter().map(|l| l.number).max().unwrap_or(1).to_string().len();
    let mut start = 0;
    let rows = shown.iter().map(|line| {
        let range = start..start + line.text.len();
        start = range.end + 1;
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
    let more = (hidden > 0).then(|| {
        let word = if whole { "Fewer lines".to_owned() } else { format!("+{} lines", thousands(hidden)) };
        let id = format!("diff-more:{key}:{at}");
        let act = crate::ui::Button::text(id, word).xs().on_click(move |_, _, cx| {
            cx.stop_propagation();
            all.update(cx, |all, cx| {
                *all = !*all;
                cx.notify();
            });
        });
        run_footer(div().debug_selector(|| "diff-more".into()).child(act), None, p)
    });
    v_flex()
        .debug_selector(|| "diff".into())
        .child(
            v_flex()
                .bg(ground)
                .pt(px(7.))
                .pb(px(8.))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(markdown::diff_size(cx))
                .line_height(px(20.))
                .text_color(p.foreground)
                .children(rows),
        )
        .children(more)
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
        RowKind::Working { waiting, retry, .. } => (*waiting, retry.clone()),
        _ => (false, None),
    };
    if let Some(retry) = retry {
        let clock = since.map(|since| {
            view.clock(row, since, cx);
            let look = shimmer::ClockLook { size: px(11.), line_height: None, height: px(24.) };
            div().flex_none().ml(px(4.)).child(shimmer::clock_slot(view, row, look, cx))
        });
        return h_flex()
            .debug_selector(|| "retry".into())
            .my(px(4.))
            .h(px(24.))
            .min_w_0()
            .gap(px(8.))
            .px(px(4.))
            .text_size(px(11.))
            .text_color(p.muted_foreground)
            .child(div().min_w_0().truncate().child(retry))
            .children(clock)
            .into_any_element();
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
    use gpui_kit::component::Root;
    use gpui_kit::{size, Entity, Modifiers, TestAppContext, VisualTestContext};
    use serde_json::json;
    use wsp_host_client::protocol::SessionEvent;

    use super::*;

    fn event(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
        let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let view = cx.new(ThreadView::new);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
        cx.simulate_resize(size(px(900.), px(800.)));
        (view, cx)
    }

    fn load(view: &Entity<ThreadView>, events: Vec<SessionEvent>, cx: &mut VisualTestContext) {
        view.update(cx, |v, cx| {
            v.show("t", cx);
            v.load(events, cx);
            v.scroll_to_top(cx);
        });
        cx.run_until_parked();
    }

    /// A reply holding a shell block and a run of it, settled or still streaming.
    fn ran(settled: bool) -> Vec<SessionEvent> {
        let mut events = vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event("session.delta", 2, json!({"kind": "text", "text": "Try:\n\n```sh\nls\n```\n"})),
            event(
                "session.run",
                3,
                json!({"runId": "r1", "block": "u1:m@2@6", "command": "ls", "state": "exited", "exitCode": 0, "output": "a"}),
            ),
        ];
        if settled {
            events.push(event("session.done", 4, json!({"result": {"status": "completed"}})));
            events.push(event("session.end", 5, json!({"exitCode": 0, "sawResult": true})));
        }
        events
    }

    #[gpui_kit::test]
    fn only_a_settled_reply_draws_its_shell_blocks_run(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        load(&view, ran(true), cx);
        assert!(cx.debug_bounds("code-run-output").is_some(), "the settled reply's run");
        load(&view, ran(false), cx);
        let streaming = view.read_with(cx, |v, _| {
            v.rows().iter().any(|r| matches!(&r.kind, RowKind::Agent { streaming: true, runs, .. } if !runs.is_empty()))
        });
        assert!(streaming, "the streaming reply holds the run");
        assert!(cx.debug_bounds("code-run-output").is_none(), "a streaming reply drew a run");
    }

    #[gpui_kit::test]
    fn copy_output_copies_the_whole_output_and_holds_its_check_for_a_second_and_a_half(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let output: String = (1..=20).map(|n| format!("line {n}\n")).collect();
        let events = vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"seq 20\"}"}),
            ),
            event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0})),
            event("session.done", 4, json!({"result": {"status": "completed"}})),
            event("session.end", 5, json!({"exitCode": 0, "sawResult": true})),
        ];
        load(&view, events, cx);
        let shut = |v: &ThreadView| -> Vec<String> {
            let rows = v.rows().iter();
            let groups = rows.clone().filter_map(|r| match &r.kind {
                RowKind::Fold { open: false } => r.turn.clone(),
                RowKind::Group { group, open: false, .. } => Some(group.clone()),
                _ => None,
            });
            let calls = rows.flat_map(|r| match &r.kind {
                RowKind::Calls { calls, .. } => calls.iter().filter(|c| !c.open).map(|c| c.key.clone()).collect(),
                _ => Vec::new(),
            });
            groups.chain(calls).collect()
        };
        for _ in 0..4 {
            let Some(key) = view.read_with(cx, |v, _| shut(v).first().cloned()) else { break };
            view.update(cx, |v, cx| v.toggle(&key, cx));
            cx.run_until_parked();
        }
        let copy = cx.debug_bounds("copy-output").expect("Copy output under the open command");
        cx.simulate_click(copy.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some(output.trim_end()), "all 20 lines, not the 10 shown");
        assert!(cx.debug_bounds("output-copied").is_some(), "no check after the copy");
        cx.executor().advance_clock(Duration::from_millis(1_400));
        cx.run_until_parked();
        assert!(cx.debug_bounds("output-copied").is_some(), "the check did not hold");
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        assert!(cx.debug_bounds("copy-output").is_some(), "the check did not go back to Copy output");
    }

    #[gpui_kit::test]
    fn a_turn_stopped_on_a_prompt_reads_waiting_for_you_before_its_clock(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let start = event("session.start", 1, json!({"prompt": "go", "at": 1_000.0}));
        load(&view, vec![start.clone()], cx);
        let working = cx.debug_bounds("clock-slot").expect("the Working line's clock");
        let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}]);
        let ask =
            event("session.permission", 2, json!({"askId": "k", "toolName": "Bash", "input": "{\"command\":\"ls\"}", "options": options}));
        load(&view, vec![start, ask], cx);
        let waits = view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Working { waiting: true, .. })));
        assert!(waits, "the open prompt left the turn working");
        let waiting = cx.debug_bounds("clock-slot").expect("the waiting line's clock");
        assert!(waiting.left() > working.left(), "the clock stands where it does after \"Working for\", at {:?}", waiting.left());
    }

    #[gpui_kit::test]
    fn plus_n_lines_shows_the_whole_output_and_fewer_lines_folds_it_again(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let output: String = (1..=40).map(|n| format!("line {n}\n")).collect();
        let events = vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"seq 40\"}"}),
            ),
            event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0})),
            event("session.done", 4, json!({"result": {"status": "completed"}})),
            event("session.end", 5, json!({"exitCode": 0, "sawResult": true})),
        ];
        load(&view, events, cx);
        for key in ["u1", "group:tool:u1:a", "u1:call:a"] {
            view.update(cx, |v, cx| v.toggle(key, cx));
            cx.run_until_parked();
        }
        let folded = cx.debug_bounds("output").expect("the open command's output");
        let more = cx.debug_bounds("output-more").expect("no +N lines act under 40 lines of output");
        cx.simulate_click(more.center(), Modifiers::none());
        cx.run_until_parked();
        let whole = cx.debug_bounds("output").expect("the output after +N lines");
        assert!(whole.size.height > folded.size.height, "+N lines left the output at {:?}", whole.size.height);
        let fewer = cx.debug_bounds("output-more").expect("no Fewer lines act");
        cx.simulate_click(fewer.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("output").map(|b| b.size.height), Some(folded.size.height), "Fewer lines did not fold it again");
    }

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
        assert_eq!(command_duration(0.2), "1ms");
        assert_eq!(command_duration(10_600.), "10s");
        assert_eq!(command_duration(59_700.), "59s");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn a_patch_numbers_each_line_on_its_own_side() {
        let lines = diff_lines("--- a/x\n+++ b/x\n@@ -10,3 +10,3 @@\n keep\n-old\n+new\n tail");
        let read: Vec<(usize, Option<char>, &str)> = lines.iter().map(|l| (l.number, l.mark, l.text)).collect();
        assert_eq!(read, [(10, None, "keep"), (11, Some('-'), "old"), (11, Some('+'), "new"), (12, None, "tail")]);
    }

    /// A key pressed and let go, as the keyboard's click on a focused piece needs both.
    fn press(cx: &mut VisualTestContext, key: &str) {
        let keystroke = gpui_kit::Keystroke::parse(key).unwrap();
        cx.simulate_event(gpui_kit::KeyDownEvent { keystroke: keystroke.clone(), is_held: false, prefer_character_input: false });
        cx.simulate_event(gpui_kit::KeyUpEvent { keystroke });
        cx.run_until_parked();
    }

    fn settled(mut events: Vec<SessionEvent>, pos: u64) -> Vec<SessionEvent> {
        events.push(event("session.done", pos, json!({"result": {"status": "completed"}})));
        events.push(event("session.end", pos + 1, json!({"exitCode": 0, "sawResult": true})));
        events
    }

    fn open_all(view: &Entity<ThreadView>, keys: &[&str], cx: &mut VisualTestContext) {
        for key in keys {
            view.update(cx, |v, cx| v.toggle(key, cx));
            cx.run_until_parked();
        }
    }

    #[gpui_kit::test]
    fn a_long_message_clamps_under_its_fade_and_show_full_message_opens_it(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let long = "word ".repeat(400);
        load(&view, vec![event("session.start", 1, json!({"prompt": long}))], cx);
        let clamped = cx.debug_bounds("row-0").expect("the person's row");
        // The bubble's 12 px padding round 176 px of text, the 24 px meta line under it, 4 px apart, and the first row's 16 px top.
        assert!((clamped.size.height - px(16. + 24. + 176. + 4. + 24. + 16.)).abs() < px(1.), "clamped to {:?}", clamped.size.height);
        let more = cx.debug_bounds("show-full").expect("Show full message on a 2,000 character message");
        cx.simulate_click(more.center(), Modifiers::none());
        cx.run_until_parked();
        let whole = cx.debug_bounds("row-0").expect("the person's row");
        assert!(whole.size.height > clamped.size.height + px(100.), "Show full message left it at {:?}", whole.size.height);
        let less = cx.debug_bounds("show-less").expect("Show less under the whole message");
        cx.simulate_click(less.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("row-0").map(|b| b.size.height), Some(clamped.size.height), "Show less did not clamp it again");
        load(&view, vec![event("session.start", 1, json!({"prompt": "short"}))], cx);
        assert!(cx.debug_bounds("show-full").is_none(), "a short message offers Show full message");
    }

    #[test]
    fn a_message_clamps_past_600_characters_or_8_lines() {
        assert!(!clamps(&"a".repeat(600)));
        assert!(clamps(&"a".repeat(601)));
        assert!(!clamps(&"é".repeat(400)), "600 is counted in UTF-16 units, as JS counts, not bytes");
        assert!(!clamps("1\n2\n3\n4\n5\n6\n7\n8"));
        assert!(clamps("1\n2\n3\n4\n5\n6\n7\n8\n9"));
        assert!(!clamps(&" ".repeat(900)));
    }

    #[gpui_kit::test]
    fn a_retried_model_call_reads_its_retry_line_in_the_working_lines_place(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let start = event("session.start", 1, json!({"prompt": "go", "at": 1_000.0}));
        let retry = event("session.retry", 2, json!({"attempt": 2, "of": 10, "why": "overloaded"}));
        load(&view, vec![start.clone(), retry.clone()], cx);
        assert!(cx.debug_bounds("retry").is_some(), "no retry line");
        assert!(cx.debug_bounds("clock-slot").is_some(), "the retry line's clock");
        assert!(cx.debug_bounds("shimmer").is_none(), "Thinking shimmers while the call is made again");
        let words = view.read_with(cx, |v, _| {
            v.rows().iter().find_map(|r| match &r.kind {
                RowKind::Working { retry, .. } => retry.clone(),
                _ => None,
            })
        });
        assert_eq!(words.as_deref(), Some("Overloaded, trying again (2 of 10)"));
        let wrote = event("session.delta", 3, json!({"kind": "text", "text": "Here"}));
        load(&view, vec![start, retry, wrote], cx);
        assert!(cx.debug_bounds("retry").is_none(), "the retry line outlived the agent's reply");
    }

    #[test]
    fn an_insight_block_is_cut_out_of_its_reply() {
        let text = "Before.\n\n`★ Insight ─────────────────────────────────────`\n- one\n- two\n`─────────────────────────────────────────────────`\n\nAfter.";
        let read = insights(text, false).expect("an insight block");
        assert_eq!(
            read,
            [
                Segment { insight: false, text: "Before." },
                Segment { insight: true, text: "- one\n- two" },
                Segment { insight: false, text: "After." },
            ]
        );
        assert!(insights("no block here", false).is_none());
        let open = "Before.\n\n✶ Insights ───\nhalf";
        assert!(insights(open, false).is_none(), "an unclosed block stays text once the reply is done");
        assert_eq!(insights(open, true).map(|s| s.len()), Some(2), "and is drawn open while it streams");
        let fenced = "```\n★ Insight ───\nx\n───\n```";
        assert!(insights(fenced, false).is_none(), "a marker inside a fence is code");
    }

    #[test]
    fn a_segment_numbers_its_shell_blocks_from_its_own_start() {
        let text = "```sh\nls\n```\n\n`★ Insight ─────`\n```sh\npwd\n```\n`─────`";
        let segments = insights(text, false).expect("an insight block");
        let start = segments[1].text.as_ptr() as usize - text.as_ptr() as usize;
        assert_eq!(fences_before(text, start), 1, "the block above the note comes first");
        assert_eq!(fences_before(text, 0), 0);
    }

    #[gpui_kit::test]
    fn an_insight_block_draws_as_a_note_without_its_marker_lines(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let reply = "Before.\n\n`★ Insight ─────`\nThe point.\n`─────`\n\nAfter.";
        let events = settled(
            vec![event("session.start", 1, json!({"prompt": "go"})), event("session.delta", 2, json!({"kind": "text", "text": reply}))],
            3,
        );
        load(&view, events, cx);
        let note = cx.debug_bounds("insight").expect("the Insight note");
        assert!(note.size.height > px(16. + 4. + 22.), "the note holds its label and its line: {:?}", note.size.height);
    }

    #[test]
    fn only_an_empty_reply_reads_empty_response() {
        let row = |text: &str| Row {
            key: "k".into(),
            kind: RowKind::Agent { at: None, meta: false, streaming: false, rewind: None, fork: None, runs: Vec::new() },
            text: text.into(),
            turn: None,
        };
        assert_eq!(body_text(&row("")), "(empty response)");
        assert_eq!(body_text(&row("  \n")), "  \n");
    }

    #[gpui_kit::test]
    fn a_running_command_turns_its_spinner_counts_its_time_and_opens_to_no_output(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let events = vec![
            event("session.start", 1, json!({"prompt": "go"})),
            event(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"make\\nmake test\"}", "at": 5_000.0}),
            ),
        ];
        load(&view, events, cx);
        open_all(&view, &["group:tool:u1:a", "u1:call:a"], cx);
        assert!(cx.debug_bounds("call-line").is_some(), "the open group's command row");
        assert!(cx.debug_bounds("clock-slot").is_some(), "the running command's timer");
        assert!(cx.debug_bounds("no-output").is_none(), "a running command opened to No output");
    }

    fn edited(patch: serde_json::Value, extra: serde_json::Value) -> Vec<SessionEvent> {
        let mut result = json!({"kind": "tool_result", "toolUseId": "a", "text": "ok", "patch": patch});
        result.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        settled(
            vec![
                event("session.start", 1, json!({"prompt": "go"})),
                event(
                    "session.delta",
                    2,
                    json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": "a", "text": "{\"file_path\":\"a.rs\"}"}),
                ),
                event("session.delta", 3, result),
            ],
            4,
        )
    }

    fn lines(n: usize, sign: char) -> serde_json::Value {
        json!((0..n).map(|i| format!("{sign}line {i}")).collect::<Vec<_>>())
    }

    #[gpui_kit::test]
    fn an_edit_row_says_a_new_files_lines_and_opens_a_head_per_file(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let one =
            json!([{"path": "a.rs", "hunks": [{"oldStart": 0, "oldLines": 0, "newStart": 1, "newLines": 3, "lines": lines(3, '+')}]}]);
        load(&view, edited(one, json!({})), cx);
        open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
        let facts = view.read_with(cx, |v, _| {
            v.rows().iter().find_map(|r| match &r.kind {
                RowKind::Calls { calls, .. } => calls.first().map(|c| c.files.clone()),
                _ => None,
            })
        });
        assert_eq!(facts.map(|f| (f.len(), f[0].created, f[0].added)), Some((1, true, 3)), "one new file of 3 lines");
        assert!(cx.debug_bounds("open-diff").is_some(), "Open diff on the open one-file edit");
        assert!(cx.debug_bounds("file-head").is_none(), "a one-file edit has no head of its own");
        let two = json!([
            {"path": "a.rs", "hunks": [{"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a", "+b"]}]},
            {"path": "b.rs", "hunks": [{"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-c", "+d"]}]},
        ]);
        load(&view, edited(two, json!({"patchCut": true})), cx);
        open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
        assert!(cx.debug_bounds("file-head").is_some(), "no head per file in a two-file edit");
        assert!(cx.debug_bounds("edit-cut").is_some(), "no word that the transcript kept part of the change");
    }

    #[gpui_kit::test]
    fn a_long_hunk_folds_to_its_first_16_lines_and_fewer_lines_folds_it_again(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let long =
            json!([{"path": "a.rs", "hunks": [{"oldStart": 1, "oldLines": 0, "newStart": 1, "newLines": 30, "lines": lines(30, '+')}]}]);
        load(&view, edited(long, json!({})), cx);
        open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
        let folded = cx.debug_bounds("diff").expect("the open edit's hunks");
        let more = cx.debug_bounds("diff-more").expect("+N lines under 30 lines of hunks");
        cx.simulate_click(more.center(), Modifiers::none());
        cx.run_until_parked();
        let whole = cx.debug_bounds("diff").expect("the hunks after +N lines");
        assert!(whole.size.height > folded.size.height + px(200.), "+N lines left the hunks at {:?}", whole.size.height);
        view.update(cx, |v, cx| v.scroll_to_end(cx));
        cx.run_until_parked();
        let whole = cx.debug_bounds("diff").expect("the hunks at the end");
        let fewer = cx.debug_bounds("diff-more").expect("Fewer lines under the whole hunks");
        cx.simulate_click(fewer.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("diff").is_some_and(|b| b.size.height < whole.size.height), "Fewer lines did not fold the hunks");
    }

    #[gpui_kit::test]
    fn a_cut_output_says_how_much_of_it_the_host_kept(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let output = "x".repeat(12 * 1024);
        let events = settled(
            vec![
                event("session.start", 1, json!({"prompt": "go"})),
                event(
                    "session.delta",
                    2,
                    json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"cat big\"}"}),
                ),
                event(
                    "session.delta",
                    3,
                    json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0, "bytes": 40 * 1024}),
                ),
            ],
            4,
        );
        load(&view, events, cx);
        open_all(&view, &["u1", "group:tool:u1:a", "u1:call:a"], cx);
        assert!(cx.debug_bounds("output-kept").is_some(), "no First 12 KB of 40 KB");
        assert_eq!(bytes_of(12 * 1024, 40 * 1024), "12 of 40 KB");
        assert_eq!(bytes_of(900, 40 * 1024), "900 B of 40 KB");
    }

    #[gpui_kit::test]
    fn an_open_groups_list_keeps_its_cap_and_grows_by_what_a_call_opened(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let mut events = vec![event("session.start", 1, json!({"prompt": "go"}))];
        for n in 0..20u64 {
            let id = format!("r{n}");
            events.push(event(
                "session.delta",
                2 + 2 * n,
                json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": id, "text": format!("{{\"command\":\"echo {n}\"}}")}),
            ));
            events.push(event(
                "session.delta",
                3 + 2 * n,
                json!({"kind": "tool_result", "toolUseId": id, "text": "line one\nline two", "exitCode": 0}),
            ));
        }
        load(&view, settled(events, 60), cx);
        open_all(&view, &["u1", "group:tool:u1:r0"], cx);
        let capped = cx.debug_bounds("calls").expect("the open group's list");
        // min(18rem, 50dvh) in an 800 px window.
        assert!((capped.size.height - px(288.)).abs() < px(1.), "the cap is {:?}", capped.size.height);
        assert!(cx.debug_bounds("calls-fade-bottom").is_some(), "no fade over the rows below the cap");
        assert!(cx.debug_bounds("calls-fade-top").is_none(), "a fade over the top of a list scrolled to its top");
        open_all(&view, &["u1:call:r0"], cx);
        cx.run_until_parked();
        let grown = cx.debug_bounds("calls").expect("the list with a call open");
        assert!(
            grown.size.height > capped.size.height && grown.size.height < px(400.),
            "opening a call left the list at {:?}",
            grown.size.height
        );
    }

    #[gpui_kit::test]
    fn a_lone_work_row_opens_to_its_body(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let events = settled(
            vec![
                event("session.start", 1, json!({"prompt": "go"})),
                event("session.notify", 2, json!({"notify": "me", "text": "Built it.\nAll 40 tests pass.\n"})),
                event("session.delta", 3, json!({"kind": "text", "text": "Done."})),
            ],
            4,
        );
        load(&view, events, cx);
        view.update(cx, |v, cx| v.toggle("u1", cx));
        cx.run_until_parked();
        let key = view.read_with(cx, |v, _| v.rows().iter().find(|r| matches!(r.kind, RowKind::Work { .. })).map(|r| r.key.clone()));
        let kinds = view.read_with(cx, |v, _| v.rows().iter().map(|r| format!("{:?}", r.kind)).collect::<Vec<_>>());
        let key = key.unwrap_or_else(|| panic!("the thinking row stands alone: {kinds:?}"));
        let shut = cx.debug_bounds("work").expect("the work row");
        cx.simulate_click(shut.center(), Modifiers::none());
        cx.run_until_parked();
        let open = view.read_with(cx, |v, _| v.rows().iter().any(|r| r.key == key && matches!(r.kind, RowKind::Work { open: true, .. })));
        assert!(open, "a press left the lone row shut: {kinds:?}");
        assert!(cx.debug_bounds("work").is_some_and(|b| b.size.height > shut.size.height), "the open row shows no body");
    }

    #[gpui_kit::test]
    fn tab_reaches_a_fold_and_enter_opens_it(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let events = settled(
            vec![
                event("session.start", 1, json!({})),
                event(
                    "session.delta",
                    2,
                    json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "a", "text": "{\"file_path\":\"a.rs\"}"}),
                ),
                event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "x"})),
                event("session.delta", 4, json!({"kind": "text", "text": "Done."})),
            ],
            5,
        );
        load(&view, events, cx);
        // With nothing focused a test window's keys reach no key context; Root's Tab runs `focus_next`.
        cx.update(|window, cx| window.focus_next(cx));
        press(cx, "enter");
        let open = view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: true })));
        assert!(open, "Tab then Enter left the fold shut");
        press(cx, "space");
        let shut = view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Fold { open: false })));
        assert!(shut, "Space left the fold open");
    }

    #[gpui_kit::test]
    fn a_replys_meta_line_shows_under_the_pointer_and_with_focus_inside_it(cx: &mut TestAppContext) {
        let (view, cx) = window(cx);
        let events =
            settled(vec![event("session.start", 1, json!({})), event("session.delta", 2, json!({"kind": "text", "text": "Done."}))], 3);
        load(&view, events, cx);
        assert!(cx.debug_bounds("meta").is_some() && cx.debug_bounds("meta-shown").is_none(), "the meta line shows at rest");
        let reply = cx.debug_bounds("meta").expect("the reply's meta line");
        cx.simulate_mouse_move(reply.center(), None, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("meta-shown").is_some(), "the pointer over the reply left its meta line hidden");
        cx.simulate_mouse_move(reply.center() + gpui_kit::point(px(0.), px(600.)), None, Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("meta-shown").is_none(), "the meta line stayed after the pointer left");
        cx.update(|window, cx| window.focus_next(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("meta-shown").is_some(), "focus on the reply's copy button left its meta line hidden");
    }

    /// The shell's root as far as the thread's keys read it.
    struct Frame(Entity<ThreadView>);

    impl Render for Frame {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            use crate::screens::app_shell::keys;
            div().size_full().key_context(format!("{} {}", keys::SHELL, keys::THREAD_OPEN).as_str()).child(self.0.clone())
        }
    }

    #[gpui_kit::test]
    fn find_paints_its_match_in_a_command_row(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
            cx.bind_keys(crate::screens::app_shell::keys::bindings(cfg!(target_os = "macos"), &Default::default()));
        });
        let view = cx.new(ThreadView::new);
        let frame = cx.new(|_| Frame(view.clone()));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(frame, window, cx));
        cx.simulate_resize(size(px(900.), px(800.)));
        let events = settled(
            vec![
                event("session.start", 1, json!({"prompt": "go"})),
                event(
                    "session.delta",
                    2,
                    json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"grep kiwi notes\"}"}),
                ),
                event("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "a", "text": "found", "exitCode": 0})),
            ],
            4,
        );
        load(&view, events, cx);
        open_all(&view, &["u1", "group:tool:u1:a"], cx);
        assert!(cx.debug_bounds("find-marked").is_none(), "a mark while the bar is shut");
        cx.update(|window, cx| window.focus(&gpui_kit::Focusable::focus_handle(view.read(cx), cx), cx));
        cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-f" } else { "ctrl-f" });
        cx.simulate_input("kiwi");
        let tools = cx.debug_bounds("find-toggle-3").expect("Include tool calls");
        cx.simulate_click(tools.center(), Modifiers::none());
        for _ in 0..3 {
            cx.executor().advance_clock(Duration::from_millis(16));
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
        }
        let status = view.read_with(cx, |v, _| v.find_state().status());
        assert!(cx.debug_bounds("find-marked").is_some(), "the command row's match is not painted: {status:?}");
    }
}
