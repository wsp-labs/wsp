// SPDX-License-Identifier: AGPL-3.0-only
//! A reply cut at its insight blocks (`components/chat/insight.ts`), each block drawn as a note.

use gpui_kit::base::text::{TextView, TextViewState};
use gpui_kit::{div, prelude::*, px, App, Entity, SharedString, Window};

use crate::screens::thread_view::markdown;
use crate::transcript::Row;
use crate::ui::Palette;

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
pub(super) fn fences_before(text: &str, end: usize) -> usize {
    crate::transcript::fence_openings(text).iter().filter(|opening| opening.byte < end).count()
}

/// A reply segment's own document, kept with the row while it is drawn and given the segment's text as it grows.
pub(super) fn segment_body(row: &Row, n: usize, text: &str, cwd: Option<&str>, window: &mut Window, cx: &mut App) -> Entity<TextViewState> {
    let id = SharedString::from(format!("segment:{}:{n}", row.key));
    let body = window.use_keyed_state(id, cx, |_, cx| TextViewState::markdown_with_extensions(text, markdown::extensions(cwd, cx), cx));
    if body.read(cx).rendered_text().source() != text {
        body.update(cx, |body, cx| body.set_text(text, cx));
    }
    body
}

/// `InsightNote`: a 2 px rule in the border ink, "Insight" in the group label's 12 px over the note.
pub(super) fn insight_note(note: TextView, p: &Palette) -> impl IntoElement {
    div()
        .debug_selector(|| "insight".into())
        .border_l_2()
        .border_color(p.border)
        .pl(px(12.))
        .child(div().mb(px(4.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child("Insight"))
        .child(note)
}
